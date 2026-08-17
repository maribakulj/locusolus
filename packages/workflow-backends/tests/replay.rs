//! Test de sortie de W3.b — `docs/SPEC_V1.md` §11.1, §11.2, §11.3 ; ADR 0003.
//!
//! **Rejouer l'historique rend exactement l'état, sans qu'un seul effet soit réexécuté.**
//!
//! Et pas seulement à l'arrivée : le rejeu est confronté à l'état vivant **à chaque pas**. Un rejeu
//! qui ne tomberait juste qu'à la fin serait un rejeu qui devine bien, et la différence ne se
//! verrait qu'au premier redémarrage au milieu de quelque chose.

use locus_domain::StableId;
use locus_protocol::Timestamp;
use locus_workflow::{
    Activity, BackendError, Effect, Idempotency, Step, WorkflowBackend, WorkflowDefinition,
    WorkflowId, WorkflowKind, WorkflowSignal, WorkflowState, WorkflowVersion,
};
use locus_workflow_backends::{
    DeterministicBackend, HistoryEvent, Progress, ReplayError, block_on, replay,
};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn subject_id(seed: u8) -> StableId {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    StableId::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn task_workflow(version: u32) -> WorkflowDefinition {
    WorkflowDefinition::new(
        WorkflowKind::Task,
        WorkflowVersion::new(version),
        vec![subject_id(1)],
        vec![
            Step::deterministic("verify_prerequisites").expect("nom valide"),
            Step::Activity(
                Activity::new(
                    "reserve_resources",
                    [Effect::Network],
                    Idempotency::key("reserve:task-1").expect("clé non vide"),
                )
                .expect("activity valide"),
            ),
            Step::deterministic("decide_next_state").expect("nom valide"),
            Step::Activity(
                Activity::new(
                    "upload_artifacts",
                    [Effect::Network],
                    Idempotency::natural("adressé par empreinte de contenu (§19.1)")
                        .expect("raison non vide"),
                )
                .expect("activity valide"),
            ),
            Step::deterministic("record_outcome").expect("nom valide"),
        ],
    )
    .expect("définition valide")
}

/// Un moteur dont les deux activities ont un exécutant.
fn staffed_backend() -> DeterministicBackend {
    let mut backend = DeterministicBackend::new();
    backend.register_activity("reserve_resources", "lease-42");
    backend.register_activity("upload_artifacts", "sha256:abcd");
    backend
}

// ---------------------------------------------------------------------------------------------
// §11.1 — les six opérations, et le port utilisable comme objet-trait
// ---------------------------------------------------------------------------------------------

#[test]
fn les_six_operations_de_11_1_passent_par_un_objet_trait() {
    // Le choix du backend se fait par profil, donc à l'exécution : le port doit être `dyn`. C'est
    // toute la raison pour laquelle ses futures sont boxées plutôt qu'écrites en `async fn`.
    let mut backend: Box<dyn WorkflowBackend> = Box::new(staffed_backend());
    let definition = task_workflow(1);

    let handle = block_on(backend.start(&definition)).expect("démarrage");
    assert_eq!(handle.kind, WorkflowKind::Task);
    assert_eq!(handle.version, WorkflowVersion::new(1));

    let id = handle.id.clone();
    block_on(backend.signal(
        &id,
        WorkflowSignal::new("budget_raised", "{}").expect("signal"),
    ))
    .expect("signal");
    block_on(backend.suspend(&id)).expect("suspension");
    assert_eq!(
        block_on(backend.inspect(&id)).expect("inspection"),
        WorkflowState::Suspended { step: Some(0) }
    );
    block_on(backend.resume(&id)).expect("reprise");
    block_on(backend.terminate(&id, "budget épuisé")).expect("arrêt");

    assert_eq!(
        block_on(backend.inspect(&id)).expect("inspection"),
        WorkflowState::Terminated {
            reason: "budget épuisé".to_owned()
        },
        "§11.4 : le motif fait partie de l'histoire, et une terminaison muette la perd sans la \
         rendre fausse"
    );
}

#[test]
fn une_operation_sur_une_execution_terminee_est_refusee() {
    let mut backend = staffed_backend();
    let definition = task_workflow(1);
    let id = block_on(backend.start(&definition)).expect("démarrage").id;
    block_on(backend.terminate(&id, "arrêt manuel")).expect("arrêt");

    let refused = block_on(backend.signal(&id, WorkflowSignal::new("ping", "").expect("signal")))
        .expect_err("un signal à une exécution arrêtée n'a pas de destinataire");
    assert!(matches!(
        refused,
        BackendError::InvalidTransition {
            attempted: "signal",
            ..
        }
    ));
}

#[test]
fn un_identifiant_inconnu_est_refuse_et_non_creee() {
    let backend = staffed_backend();
    let unknown = WorkflowId::new("wf-9999").expect("identifiant valide");
    assert_eq!(
        block_on(backend.inspect(&unknown)),
        Err(BackendError::Unknown { id: unknown })
    );
}

#[test]
fn les_identifiants_sont_attribues_sans_horloge_ni_tirage() {
    let definition = task_workflow(1);
    let ids = |()| {
        let mut backend = staffed_backend();
        let first = block_on(backend.start(&definition)).expect("démarrage").id;
        let second = block_on(backend.start(&definition)).expect("démarrage").id;
        vec![first, second]
    };
    assert_eq!(
        ids(()),
        ids(()),
        "deux moteurs neufs à qui l'on demande la même chose doivent rendre les mêmes \
         identifiants : c'est ce qui permet de rejouer un test ligne à ligne"
    );
}

// ---------------------------------------------------------------------------------------------
// Le test de sortie
// ---------------------------------------------------------------------------------------------

#[test]
fn rejouer_l_historique_rend_exactement_l_etat_a_chaque_pas() {
    let definition = task_workflow(1);
    let mut backend = staffed_backend();
    let id = block_on(backend.start(&definition)).expect("démarrage").id;

    // À chaque arrêt sur image : l'historique tel qu'il est, et l'état vivant tel que le moteur le
    // connaît. Le rejeu doit retrouver le second à partir du premier, sans rien d'autre.
    let mut snapshots = vec![(
        backend.history(&id).expect("historique").to_vec(),
        block_on(backend.inspect(&id)).expect("inspection"),
    )];
    loop {
        let progress = backend.advance(&id).expect("avancement");
        snapshots.push((
            backend.history(&id).expect("historique").to_vec(),
            block_on(backend.inspect(&id)).expect("inspection"),
        ));
        if progress == Progress::Completed {
            break;
        }
    }

    assert_eq!(
        snapshots.last().map(|(_, state)| state),
        Some(&WorkflowState::Completed)
    );
    assert!(snapshots.len() > 3, "la fixture doit avoir plusieurs pas");

    // Le moteur — et avec lui le registre des activities — disparaît avant le rejeu. Ce qui suit
    // n'a plus accès à rien de vivant.
    drop(backend);

    for (index, (history, live)) in snapshots.iter().enumerate() {
        let replayed = replay(&definition, history)
            .unwrap_or_else(|error| panic!("rejeu du point {index} : {error}"));
        assert_eq!(
            &replayed.state, live,
            "au point {index}, le rejeu et le moteur vivant divergent"
        );
    }

    let complete = replay(&definition, &snapshots.last().expect("au moins un point").0)
        .expect("rejeu de l'historique complet");
    assert_eq!(
        complete.activity_results,
        vec![
            ("reserve_resources".to_owned(), "lease-42".to_owned()),
            ("upload_artifacts".to_owned(), "sha256:abcd".to_owned()),
        ],
        "les résultats viennent de l'historique : les redemander au monde les rendrait différents"
    );
}

#[test]
fn le_rejeu_est_idempotent() {
    let definition = task_workflow(1);
    let mut backend = staffed_backend();
    let id = block_on(backend.start(&definition)).expect("démarrage").id;
    backend.run(&id).expect("exécution");
    let history = backend.history(&id).expect("historique").to_vec();

    let first = replay(&definition, &history).expect("rejeu");
    let second = replay(&definition, &history).expect("rejeu");
    assert_eq!(first, second);
}

#[test]
fn le_rejeu_reproduit_la_suspension_et_l_arret() {
    let definition = task_workflow(1);
    let mut backend = staffed_backend();
    let id = block_on(backend.start(&definition)).expect("démarrage").id;
    backend.advance(&id).expect("un pas");
    block_on(backend.suspend(&id)).expect("suspension");

    let suspended = replay(&definition, backend.history(&id).expect("historique"))
        .expect("rejeu")
        .state;
    assert_eq!(suspended, WorkflowState::Suspended { step: Some(1) });

    block_on(backend.resume(&id)).expect("reprise");
    block_on(backend.terminate(&id, "arbitrage humain")).expect("arrêt");
    let terminated = replay(&definition, backend.history(&id).expect("historique"))
        .expect("rejeu")
        .state;
    assert_eq!(
        terminated,
        WorkflowState::Terminated {
            reason: "arbitrage humain".to_owned()
        }
    );
}

#[test]
fn le_rejeu_conserve_les_signaux_dans_l_ordre() {
    let definition = task_workflow(1);
    let mut backend = staffed_backend();
    let id = block_on(backend.start(&definition)).expect("démarrage").id;
    block_on(backend.signal(&id, WorkflowSignal::new("first", "a").expect("signal"))).expect("s1");
    backend.advance(&id).expect("un pas");
    block_on(backend.signal(&id, WorkflowSignal::new("second", "b").expect("signal"))).expect("s2");

    let replayed = replay(&definition, backend.history(&id).expect("historique")).expect("rejeu");
    assert_eq!(
        replayed.signals,
        vec![
            ("first".to_owned(), "a".to_owned()),
            ("second".to_owned(), "b".to_owned()),
        ]
    );
}

// ---------------------------------------------------------------------------------------------
// Ce que le rejeu refuse, plutôt que de rendre un état plausible
// ---------------------------------------------------------------------------------------------

#[test]
fn rejouer_avec_une_autre_version_est_refuse() {
    let started_as = task_workflow(1);
    let mut backend = staffed_backend();
    let id = block_on(backend.start(&started_as)).expect("démarrage").id;
    backend.run(&id).expect("exécution");
    let history = backend.history(&id).expect("historique").to_vec();

    assert_eq!(
        replay(&task_workflow(2), &history),
        Err(ReplayError::WrongVersion {
            recorded: WorkflowVersion::new(1),
            replayed: WorkflowVersion::new(2),
        }),
        "rejouer une exécution v1 avec le code v2 rendrait un état construit par des pas qu'elle \
         n'a jamais traversés"
    );
}

#[test]
fn rejouer_un_pas_renomme_est_refuse() {
    let definition = task_workflow(1);
    let mut backend = staffed_backend();
    let id = block_on(backend.start(&definition)).expect("démarrage").id;
    backend.advance(&id).expect("un pas");
    let history = backend.history(&id).expect("historique").to_vec();

    let renamed = WorkflowDefinition::new(
        WorkflowKind::Task,
        WorkflowVersion::new(1),
        vec![subject_id(1)],
        vec![Step::deterministic("check_prerequisites").expect("nom valide")],
    )
    .expect("définition valide");

    assert_eq!(
        replay(&renamed, &history),
        Err(ReplayError::RenamedStep {
            index: 0,
            recorded: "verify_prerequisites".to_owned(),
            expected: "check_prerequisites".to_owned(),
        }),
        "renommer un pas sans changer de version casse le rejeu des exécutions en cours, et le \
         refus le dit au lieu de le laisser passer"
    );
}

#[test]
fn un_historique_sans_demarrage_est_refuse() {
    assert_eq!(replay(&task_workflow(1), &[]), Err(ReplayError::NoStart));
}

#[test]
fn un_resultat_sans_pas_aborde_est_refuse() {
    let definition = task_workflow(1);
    let forged = vec![
        HistoryEvent::Started {
            kind: WorkflowKind::Task,
            version: WorkflowVersion::new(1),
        },
        HistoryEvent::ActivityCompleted {
            index: 1,
            name: "reserve_resources".to_owned(),
            result: "lease-inventé".to_owned(),
        },
    ];
    assert_eq!(
        replay(&definition, &forged),
        Err(ReplayError::ResultWithoutEntry { index: 1 })
    );
}

#[test]
fn une_activity_abordee_sans_resultat_est_refusee() {
    let definition = task_workflow(1);
    let truncated = vec![
        HistoryEvent::Started {
            kind: WorkflowKind::Task,
            version: WorkflowVersion::new(1),
        },
        HistoryEvent::StepEntered {
            index: 0,
            name: "verify_prerequisites".to_owned(),
        },
        HistoryEvent::StepEntered {
            index: 1,
            name: "reserve_resources".to_owned(),
        },
    ];
    assert_eq!(
        replay(&definition, &truncated),
        Err(ReplayError::MissingResult { index: 1 }),
        "une activity abordée et jamais finie n'est pas un pas franchi : le rejeu ne l'invente pas"
    );
}

// ---------------------------------------------------------------------------------------------
// Ce que le moteur refuse, et ce qu'il n'écrit pas en refusant
// ---------------------------------------------------------------------------------------------

#[test]
fn une_activity_sans_executant_fait_refuser_sans_salir_l_historique() {
    let definition = task_workflow(1);
    let mut backend = DeterministicBackend::new(); // aucune activity enregistrée
    let id = block_on(backend.start(&definition)).expect("démarrage").id;
    backend
        .advance(&id)
        .expect("le premier pas est déterministe");

    let before = backend.history(&id).expect("historique").to_vec();
    let refused = backend
        .advance(&id)
        .expect_err("aucun exécutant pour reserve_resources");
    assert!(matches!(refused, BackendError::UnregisteredActivity { .. }));
    assert_eq!(
        backend.history(&id).expect("historique"),
        before.as_slice(),
        "un refus qui aurait déjà écrit `StepEntered` laisserait un historique décrivant un pas \
         abordé et jamais fini — un historique faux produit par une erreur bénigne"
    );
    // Et l'historique reste rejouable : le refus n'a rien cassé.
    assert_eq!(
        replay(&definition, &before).expect("rejeu").state,
        WorkflowState::Running { step: Some(1) }
    );
}

#[test]
fn avancer_une_execution_suspendue_est_refuse() {
    let definition = task_workflow(1);
    let mut backend = staffed_backend();
    let id = block_on(backend.start(&definition)).expect("démarrage").id;
    block_on(backend.suspend(&id)).expect("suspension");
    assert!(matches!(
        backend.advance(&id),
        Err(BackendError::InvalidTransition {
            attempted: "advance",
            ..
        })
    ));
}

// ---------------------------------------------------------------------------------------------
// L'assertion que porte `block_on`
// ---------------------------------------------------------------------------------------------

#[test]
#[should_panic(expected = "le backend déterministe a rendu Pending")]
fn attendre_quoi_que_ce_soit_est_une_panique() {
    // Sans ce test, la garde de `block_on` serait vide de sens : rien ne dirait qu'elle se
    // déclenche. Un moteur de test qui se mettrait à attendre aurait cessé d'être déterministe, et
    // un exécuteur complet l'aurait patiemment laissé faire.
    block_on(std::future::pending::<()>());
}
