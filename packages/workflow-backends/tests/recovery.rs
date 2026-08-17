//! Test de sortie de W3.e — `docs/SPEC_V1.md` §11.3, §11.4 ; ADR 0003.
//!
//! **Un redémarrage au milieu ne change pas l'histoire, et compenser n'en efface rien.**
//!
//! Deux moitiés, une par phrase.
//!
//! La première est vérifiée sur les deux backends, et « crash » n'y veut pas dire la même chose :
//! le moteur déterministe **est** la vérité, donc le perdre perd tout sauf l'historique ; le
//! cluster Temporal est la vérité, donc perdre l'adaptateur ne perd qu'une table de
//! correspondance. Les deux disent pourtant la même chose — ce qui survit à un crash est ce qui
//! n'était pas dans le processus qui a crashé.
//!
//! La seconde est une propriété de l'historique : après compensation, l'historique d'avant est un
//! **préfixe** de celui d'après. Un historique d'où l'on retirerait ce qui a été compensé décrirait
//! une exécution où la réservation n'a jamais eu lieu — et une réservation qui n'a jamais eu lieu
//! n'a pas consommé de capacité, ce qui est faux.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use locus_domain::StableId;
use locus_protocol::Timestamp;
use locus_workflow::{
    CATALOG_VERSION, DefinitionError, Step, WorkflowBackend, WorkflowDefinition, WorkflowKind,
    WorkflowState, WorkflowVersion, catalog,
};
use locus_workflow_backends::compensation::{CompensationStep, UncertainStep, plan};
use locus_workflow_backends::temporal::{
    Call, Description, ExecutionRef, ExecutionStatus, GatewayError, StartRequest, TemporalConfig,
    TemporalGateway, TemporalWorkflowBackend,
};
use locus_workflow_backends::{DeterministicBackend, HistoryEvent, block_on, replay};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn subject(seed: u8) -> Vec<StableId> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    vec![
        StableId::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
            .expect("l'instant de fixture tient sur 48 bits"),
    ]
}

fn sandbox_workflow() -> WorkflowDefinition {
    catalog::definition(WorkflowKind::SandboxLifecycle, CATALOG_VERSION, subject(10))
        .expect("définition valide")
}

fn staffed(definition: &WorkflowDefinition) -> DeterministicBackend {
    let mut backend = DeterministicBackend::new();
    for activity in definition.activities() {
        backend.register_activity(activity.name(), &format!("result:{}", activity.name()));
    }
    backend
}

// ---------------------------------------------------------------------------------------------
// Première moitié — le redémarrage, sur le moteur déterministe
// ---------------------------------------------------------------------------------------------

#[test]
fn un_redemarrage_au_milieu_ne_change_pas_l_histoire() {
    let definition = sandbox_workflow();

    // Une exécution qui va au bout sans incident.
    let mut uninterrupted = staffed(&definition);
    let id = block_on(uninterrupted.start(&definition))
        .expect("démarrage")
        .id;
    uninterrupted.run(&id).expect("exécution");
    let expected = uninterrupted.history(&id).expect("historique").to_vec();

    // La même, coupée à chaque pas possible. Le moteur disparaît réellement — un moteur en mémoire
    // simule un crash mieux qu'un vrai, puisqu'il perd tout ce qu'il avait.
    for cut in 0..definition.steps().len() {
        let mut before = staffed(&definition);
        let id = block_on(before.start(&definition)).expect("démarrage").id;
        for _ in 0..cut {
            before.advance(&id).expect("avancement");
        }
        let partial = before.history(&id).expect("historique").to_vec();
        drop(before);

        let mut after = staffed(&definition);
        let resumed = after
            .resume_from(&definition, partial)
            .unwrap_or_else(|error| panic!("reprise après {cut} pas : {error}"));
        after.run(&resumed).expect("suite de l'exécution");

        assert_eq!(
            after.history(&resumed).expect("historique"),
            expected.as_slice(),
            "coupée après {cut} pas puis reprise, l'exécution doit produire la même histoire"
        );
    }
}

#[test]
fn reprendre_sur_un_historique_qu_on_ne_sait_pas_rejouer_est_refuse() {
    let definition = sandbox_workflow();
    let mut backend = staffed(&definition);
    let id = block_on(backend.start(&definition)).expect("démarrage").id;
    backend.advance(&id).expect("un pas");
    let history = backend.history(&id).expect("historique").to_vec();

    // Une autre version : l'historique décrit des pas que ce code n'a peut-être jamais eus.
    let other = catalog::definition(
        WorkflowKind::SandboxLifecycle,
        WorkflowVersion::new(2),
        subject(10),
    )
    .expect("définition valide");
    let mut fresh = staffed(&other);
    fresh
        .resume_from(&other, history)
        .expect_err("reprendre à un endroit deviné serait pire que refuser");
}

// ---------------------------------------------------------------------------------------------
// Première moitié — le redémarrage, côté Temporal : ce n'est pas le même crash
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Default)]
struct ClusterState {
    executions: BTreeMap<String, (String, ExecutionStatus)>,
    runs: usize,
}

#[derive(Debug, Clone, Default)]
struct FakeCluster {
    state: Arc<Mutex<ClusterState>>,
}

impl TemporalGateway for FakeCluster {
    fn start_workflow(&mut self, request: StartRequest) -> Call<'_, ExecutionRef> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("verrou");
            state.runs += 1;
            let run_id = format!("run-{:04}", state.runs);
            state.executions.insert(
                request.workflow_id.clone(),
                (run_id.clone(), ExecutionStatus::Running),
            );
            Ok(ExecutionRef {
                workflow_id: request.workflow_id,
                run_id,
            })
        })
    }

    fn signal_workflow<'a>(
        &'a mut self,
        _execution: &'a ExecutionRef,
        _name: &'a str,
        _payload: &'a str,
    ) -> Call<'a, ()> {
        Box::pin(async move { Ok(()) })
    }

    fn terminate_workflow<'a>(
        &'a mut self,
        execution: &'a ExecutionRef,
        _reason: &'a str,
    ) -> Call<'a, ()> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("verrou");
            if let Some(entry) = state.executions.get_mut(&execution.workflow_id) {
                entry.1 = ExecutionStatus::Terminated;
            }
            Ok(())
        })
    }

    fn describe_workflow<'a>(&'a self, execution: &'a ExecutionRef) -> Call<'a, Description> {
        Box::pin(async move {
            let state = self.state.lock().expect("verrou");
            let Some((run_id, status)) = state.executions.get(&execution.workflow_id) else {
                return Err(GatewayError::NotFound {
                    execution: execution.clone(),
                });
            };
            if *run_id != execution.run_id {
                return Err(GatewayError::NotFound {
                    execution: execution.clone(),
                });
            }
            Ok(Description {
                status: *status,
                reason: None,
            })
        })
    }

    fn resolve_execution<'a>(&'a self, workflow_id: &'a str) -> Call<'a, ExecutionRef> {
        Box::pin(async move {
            let state = self.state.lock().expect("verrou");
            let Some((run_id, _)) = state.executions.get(workflow_id) else {
                return Err(GatewayError::NotFound {
                    execution: ExecutionRef {
                        workflow_id: workflow_id.to_owned(),
                        run_id: String::new(),
                    },
                });
            };
            Ok(ExecutionRef {
                workflow_id: workflow_id.to_owned(),
                run_id: run_id.clone(),
            })
        })
    }

    fn query_workflow<'a>(
        &'a self,
        _execution: &'a ExecutionRef,
        query: &'a str,
    ) -> Call<'a, String> {
        Box::pin(async move {
            Err(GatewayError::QueryUnsupported {
                query: query.to_owned(),
            })
        })
    }
}

fn config() -> TemporalConfig {
    TemporalConfig {
        namespace: "locus".to_owned(),
        task_queue: "locus-tasks".to_owned(),
    }
}

#[test]
fn un_control_plane_qui_redemarre_retrouve_l_execution() {
    let cluster = FakeCluster::default();
    let definition = sandbox_workflow();

    let mut before = TemporalWorkflowBackend::new(cluster.clone(), config());
    let started = block_on(before.start(&definition)).expect("démarrage");
    let run_id = before
        .execution(&started.id)
        .expect("référence")
        .run_id
        .clone();

    // Le control plane redémarre : l'adaptateur perd sa table, et c'est tout ce qu'il perd, parce
    // que c'est tout ce qu'il avait. L'exécution, elle, n'a rien perdu — le cluster n'a pas
    // redémarré.
    drop(before);

    let mut after = TemporalWorkflowBackend::new(cluster, config());
    let reattached = block_on(after.reattach(&definition)).expect("rattachement");

    assert_eq!(
        reattached.id, started.id,
        "le workflow_id est reconstructible"
    );
    assert_eq!(
        after.execution(&reattached.id).expect("référence").run_id,
        run_id,
        "le run_id, lui, appartient au cluster : il est redemandé, pas deviné"
    );
    assert!(matches!(
        block_on(after.inspect(&reattached.id)).expect("inspection"),
        WorkflowState::Unknown { .. } | WorkflowState::Running { .. }
    ));
}

#[test]
fn rattacher_une_execution_que_le_cluster_ne_connait_pas_est_refuse() {
    let mut backend = TemporalWorkflowBackend::new(FakeCluster::default(), config());
    block_on(backend.reattach(&sandbox_workflow()))
        .expect_err("aucun démarrage n'a eu lieu : il n'y a rien à rattacher");
}

// ---------------------------------------------------------------------------------------------
// Seconde moitié — compenser n'efface rien
// ---------------------------------------------------------------------------------------------

#[test]
fn compenser_ajoute_a_l_historique_et_n_en_retire_rien() {
    let definition = sandbox_workflow();
    let mut backend = staffed(&definition);
    let id = block_on(backend.start(&definition)).expect("démarrage").id;
    backend.run(&id).expect("exécution");

    let before = backend.history(&id).expect("historique").to_vec();
    let compensated = backend.compensate(&id).expect("compensation");
    let undone = compensated.undone;
    let after = backend.history(&id).expect("historique").to_vec();

    assert_eq!(
        &after[..before.len()],
        before.as_slice(),
        "l'historique d'avant doit être un préfixe de celui d'après : compenser ajoute, ne rature pas"
    );
    assert_eq!(
        undone,
        vec![
            "start_sandbox".to_owned(),
            "reserve_sandbox_resources".to_owned()
        ],
        "l'ordre inverse n'est pas une élégance : rendre des ressources qu'un processus vivant \
         occupe encore ne les rend pas"
    );

    // Et le rejeu voit toujours ce qui a eu lieu.
    let rejoined = replay(&definition, &after).expect("rejeu");
    assert!(
        rejoined
            .activity_results
            .iter()
            .any(|(name, _)| name == "reserve_sandbox_resources"),
        "la réservation a eu lieu ; la compenser ne la fait pas ne pas avoir eu lieu"
    );
    assert_eq!(
        rejoined.compensations,
        vec![
            ("start_sandbox".to_owned(), "stop_sandbox".to_owned()),
            (
                "reserve_sandbox_resources".to_owned(),
                "release_sandbox_resources".to_owned()
            ),
        ]
    );
    assert_eq!(
        rejoined.state,
        WorkflowState::Completed,
        "compenser ne défait pas l'exécution : elle a bien eu lieu, et elle est bien finie"
    );
}

#[test]
fn on_ne_compense_que_ce_qui_a_reellement_eu_lieu() {
    let definition = sandbox_workflow();
    let mut backend = staffed(&definition);
    let id = block_on(backend.start(&definition)).expect("démarrage").id;
    backend.advance(&id).expect("validate_sandbox_spec");
    backend.advance(&id).expect("reserve_sandbox_resources");

    // `start_sandbox` n'a pas eu lieu : le plan ne doit pas prétendre l'arrêter.
    let plan = plan(&definition, backend.history(&id).expect("historique"));
    assert_eq!(
        plan.steps,
        vec![CompensationStep {
            index: 1,
            activity: "reserve_sandbox_resources".to_owned(),
            by: "release_sandbox_resources".to_owned(),
        }],
        "un plan tiré de la définition libérerait des réservations jamais prises"
    );
    assert!(plan.uncertain.is_empty());
}

#[test]
fn compenser_deux_fois_ne_defait_pas_deux_fois() {
    let definition = sandbox_workflow();
    let mut backend = staffed(&definition);
    let id = block_on(backend.start(&definition)).expect("démarrage").id;
    backend.run(&id).expect("exécution");

    backend.compensate(&id).expect("première compensation");
    let after_first = backend.history(&id).expect("historique").to_vec();
    let second = backend.compensate(&id).expect("seconde compensation");

    assert!(second.undone.is_empty(), "tout était déjà défait");
    assert!(second.uncertain.is_empty(), "et rien n'était douteux");
    assert_eq!(
        backend.history(&id).expect("historique"),
        after_first.as_slice(),
        "une seconde compensation n'ajoute rien : le plan lit ce qui a déjà été défait"
    );
}

#[test]
fn une_compensation_sans_executant_ne_defait_rien_du_tout() {
    let definition = sandbox_workflow();
    let mut backend = DeterministicBackend::new();
    for activity in definition.activities() {
        // Tout le monde sauf celui qui libère les ressources.
        if activity.name() != "release_sandbox_resources" {
            backend.register_activity(activity.name(), "ok");
        }
    }
    let id = block_on(backend.start(&definition)).expect("démarrage").id;
    backend.advance(&id).expect("validate_sandbox_spec");
    backend.advance(&id).expect("reserve_sandbox_resources");
    backend.advance(&id).expect("start_sandbox");

    let before = backend.history(&id).expect("historique").to_vec();
    backend
        .compensate(&id)
        .expect_err("une compensation sans exécutant fait tout refuser");
    assert_eq!(
        backend.history(&id).expect("historique"),
        before.as_slice(),
        "une compensation partielle laisserait la moitié des réservations vivantes et \
         l'historique disant qu'elles ont été rendues"
    );
}

// ---------------------------------------------------------------------------------------------
// Ce que la définition refuse
// ---------------------------------------------------------------------------------------------

#[test]
fn une_compensation_qui_designe_un_pas_absent_ne_se_declare_pas() {
    let steps = vec![Step::Activity(
        match catalog::definition(WorkflowKind::Task, CATALOG_VERSION, subject(3))
            .expect("définition valide")
            .steps()[1]
            .clone()
        {
            Step::Activity(activity) => activity
                .compensating("libere_tout")
                .expect("nom bien formé"),
            Step::Deterministic { .. } => unreachable!("le pas 1 est une activity"),
        },
    )];

    let error = WorkflowDefinition::new(WorkflowKind::Task, CATALOG_VERSION, subject(3), steps)
        .expect_err("le nom ne désigne aucune activity de cette définition");
    assert!(matches!(error, DefinitionError::UnknownCompensation { .. }));
}

#[test]
fn les_faits_scientifiques_n_ont_aucune_compensation() {
    // §11.4 : les compensations annulent des réservations techniques et « ne réécrivent jamais
    // l'histoire épistémique ». Aucun pas qui enregistre un fait n'a donc de compensatrice — et ce
    // n'est pas un oubli : défaire un fait observé n'est pas une compensation, c'est une
    // falsification.
    let recording = [
        "record_review_decision",
        "record_reproduction_verdict",
        "record_branch_conclusion",
        "record_image_digest",
        "publish_program_outcome",
    ];
    for kind in WorkflowKind::ALL {
        let definition =
            catalog::definition(kind, CATALOG_VERSION, subject(1)).expect("définition valide");
        for activity in definition.activities() {
            if recording.contains(&activity.name()) {
                assert_eq!(
                    activity.compensated_by(),
                    None,
                    "{kind} / {} déclare une compensation",
                    activity.name()
                );
            }
        }
    }
}

#[test]
fn aucun_chemin_de_code_ne_retire_un_evenement_de_l_historique() {
    // Garantie tenue par l'absence de la fonction qui la violerait — même méthode qu'en W1.g pour
    // les conflits. Ce qui est cherché est un retrait appliqué à un historique, pas un retrait en
    // général : les tables d'instances, elles, ont le droit d'oublier.
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    for entry in std::fs::read_dir(&root).expect("les sources existent") {
        let path = entry.expect("entrée lisible").path();
        if path.extension().is_none_or(|extension| extension != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("source lisible");
        for (number, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            for verb in ["truncate", "clear", "drain", "pop", "remove", "retain"] {
                if line.contains(&format!("history.{verb}(")) {
                    offenders.push(format!("{}:{}", path.display(), number + 1));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "un historique se complète, il ne se corrige pas : {offenders:#?}"
    );
}

#[test]
fn un_pas_aborde_dont_le_resultat_n_est_jamais_revenu_est_nomme_et_non_devine() {
    // Le cas que le moteur en mémoire ne sait pas produire — il finit ses activities dans le même
    // appel — et qu'un vrai moteur produit dès qu'un worker meurt entre les deux. C'est aussi le
    // cas où une mutation avait été verte : le plan lisait `ActivityCompleted`, et rien ne testait
    // ce qui se passe quand il manque.
    //
    // On ne sait pas si l'effet a eu lieu. Compenser ce qui n'a pas eu lieu peut casser un état
    // sain ; ne pas compenser ce qui a eu lieu laisse une réservation que personne ne rendra. Le
    // moteur n'a aucun moyen de trancher, donc il nomme.
    let definition = sandbox_workflow();
    let forged = vec![
        HistoryEvent::Started {
            kind: WorkflowKind::SandboxLifecycle,
            version: CATALOG_VERSION,
        },
        HistoryEvent::StepEntered {
            index: 0,
            name: "validate_sandbox_spec".to_owned(),
        },
        HistoryEvent::StepEntered {
            index: 1,
            name: "reserve_sandbox_resources".to_owned(),
        },
    ];

    let plan = plan(&definition, &forged);
    assert!(
        plan.steps.is_empty(),
        "rien n'est **su** avoir eu lieu : {:#?}",
        plan.steps
    );
    assert_eq!(
        plan.uncertain,
        vec![UncertainStep {
            index: 1,
            activity: "reserve_sandbox_resources".to_owned(),
            by: "release_sandbox_resources".to_owned(),
        }],
        "l'inconnu se nomme, il ne se range pas sous le plus probable"
    );
}

#[test]
fn compenser_rend_les_deux_listes_ensemble() {
    // Un appelant qui ne verrait que `undone` croirait le ménage fini.
    let definition = sandbox_workflow();
    let mut backend = staffed(&definition);
    let id = block_on(backend.start(&definition)).expect("démarrage").id;
    backend.run(&id).expect("exécution");

    let compensated = backend.compensate(&id).expect("compensation");
    assert_eq!(compensated.undone.len(), 2);
    assert!(compensated.uncertain.is_empty());
}
