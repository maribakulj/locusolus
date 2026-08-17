//! Test de sortie de W3.d — `docs/SPEC_V1.md` §11.1, ADR 0003, ADR 0015.
//!
//! **Les six opérations de §11.1 tiennent le même contrat sur les deux backends, et la traduction
//! vers Temporal ne perd aucune distinction que le cluster faisait.**
//!
//! Ce qui est vérifié ici est une **traduction**, pas une liaison au fil : le cluster est un faux.
//! L'ADR 0015 dit pourquoi la liaison réelle n'est pas livrée et ce qui la débloquerait. Un test
//! qui laisserait croire le contraire serait pire que pas de test.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use locus_domain::StableId;
use locus_protocol::Timestamp;
use locus_workflow::{
    BackendError, CATALOG_VERSION, WorkflowBackend, WorkflowDefinition, WorkflowId, WorkflowKind,
    WorkflowSignal, WorkflowState, catalog,
};
use locus_workflow_backends::temporal::{
    Call, Description, ExecutionRef, ExecutionStatus, GatewayError, RESUME_SIGNAL, STATE_QUERY,
    SUSPEND_SIGNAL, SUSPENDED_ANSWER, StartRequest, TemporalConfig, TemporalGateway,
    TemporalWorkflowBackend, state_from,
};
use locus_workflow_backends::{DeterministicBackend, block_on};

// ---------------------------------------------------------------------------------------------
// Un faux cluster — et il s'appelle « faux », pas « local »
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Execution {
    run_id: String,
    status: ExecutionStatus,
    reason: Option<String>,
    suspended: bool,
}

#[derive(Debug, Default)]
struct ClusterState {
    executions: BTreeMap<String, Execution>,
    signals: Vec<(String, String, String)>,
    starts: Vec<StartRequest>,
    answers_state_query: bool,
    runs: usize,
}

/// Un cluster de mensonge, qui se comporte comme Temporal sur les seuls points qui nous concernent.
///
/// Il coopère : ses workflows répondent aux signaux réservés et à la requête d'état, comme le
/// feraient des workflows écrits pour ce control plane. `answers_state_query` permet de simuler
/// l'autre cas, tout aussi légitime — un workflow qui n'implémente pas la requête.
#[derive(Debug, Clone)]
struct FakeCluster {
    state: Arc<Mutex<ClusterState>>,
}

impl FakeCluster {
    fn new(answers_state_query: bool) -> Self {
        Self {
            state: Arc::new(Mutex::new(ClusterState {
                answers_state_query,
                ..ClusterState::default()
            })),
        }
    }

    fn set_status(&self, workflow_id: &str, status: ExecutionStatus, reason: Option<&str>) {
        let mut state = self.state.lock().expect("verrou");
        if let Some(execution) = state.executions.get_mut(workflow_id) {
            execution.status = status;
            execution.reason = reason.map(ToOwned::to_owned);
        }
    }

    fn signals(&self) -> Vec<String> {
        self.state
            .lock()
            .expect("verrou")
            .signals
            .iter()
            .map(|(_, name, _)| name.clone())
            .collect()
    }

    fn starts(&self) -> Vec<StartRequest> {
        self.state.lock().expect("verrou").starts.clone()
    }
}

impl TemporalGateway for FakeCluster {
    fn start_workflow(&mut self, request: StartRequest) -> Call<'_, ExecutionRef> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("verrou");
            state.runs += 1;
            let run_id = format!("run-{:04}", state.runs);
            state.executions.insert(
                request.workflow_id.clone(),
                Execution {
                    run_id: run_id.clone(),
                    status: ExecutionStatus::Running,
                    reason: None,
                    suspended: false,
                },
            );
            let reference = ExecutionRef {
                workflow_id: request.workflow_id.clone(),
                run_id,
            };
            state.starts.push(request);
            Ok(reference)
        })
    }

    fn signal_workflow<'a>(
        &'a mut self,
        execution: &'a ExecutionRef,
        name: &'a str,
        payload: &'a str,
    ) -> Call<'a, ()> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("verrou");
            let Some(target) = state.executions.get_mut(&execution.workflow_id) else {
                return Err(GatewayError::NotFound {
                    execution: execution.clone(),
                });
            };
            if target.run_id != execution.run_id {
                return Err(GatewayError::NotFound {
                    execution: execution.clone(),
                });
            }
            if target.status != ExecutionStatus::Running {
                return Err(GatewayError::NotRunning {
                    execution: execution.clone(),
                });
            }
            // Un workflow écrit pour ce control plane traite les deux signaux réservés ; c'est ce
            // que le faux cluster imite, parce que Temporal, lui, ne sait pas suspendre.
            match name {
                SUSPEND_SIGNAL => target.suspended = true,
                RESUME_SIGNAL => target.suspended = false,
                _ => {}
            }
            state.signals.push((
                execution.workflow_id.clone(),
                name.to_owned(),
                payload.to_owned(),
            ));
            Ok(())
        })
    }

    fn terminate_workflow<'a>(
        &'a mut self,
        execution: &'a ExecutionRef,
        reason: &'a str,
    ) -> Call<'a, ()> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("verrou");
            let Some(target) = state.executions.get_mut(&execution.workflow_id) else {
                return Err(GatewayError::NotFound {
                    execution: execution.clone(),
                });
            };
            target.status = ExecutionStatus::Terminated;
            target.reason = Some(reason.to_owned());
            Ok(())
        })
    }

    fn describe_workflow<'a>(&'a self, execution: &'a ExecutionRef) -> Call<'a, Description> {
        Box::pin(async move {
            let state = self.state.lock().expect("verrou");
            let Some(target) = state.executions.get(&execution.workflow_id) else {
                return Err(GatewayError::NotFound {
                    execution: execution.clone(),
                });
            };
            if target.run_id != execution.run_id {
                return Err(GatewayError::NotFound {
                    execution: execution.clone(),
                });
            }
            Ok(Description {
                status: target.status,
                reason: target.reason.clone(),
            })
        })
    }

    fn query_workflow<'a>(
        &'a self,
        execution: &'a ExecutionRef,
        query: &'a str,
    ) -> Call<'a, String> {
        Box::pin(async move {
            let state = self.state.lock().expect("verrou");
            if !state.answers_state_query {
                return Err(GatewayError::QueryUnsupported {
                    query: query.to_owned(),
                });
            }
            let Some(target) = state.executions.get(&execution.workflow_id) else {
                return Err(GatewayError::NotFound {
                    execution: execution.clone(),
                });
            };
            Ok(if target.suspended {
                SUSPENDED_ANSWER.to_owned()
            } else {
                "running".to_owned()
            })
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn subject() -> Vec<StableId> {
    let mut entropy = [0_u8; 10];
    entropy[9] = 7;
    vec![
        StableId::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
            .expect("l'instant de fixture tient sur 48 bits"),
    ]
}

fn task_definition() -> WorkflowDefinition {
    catalog::definition(WorkflowKind::Task, CATALOG_VERSION, subject()).expect("définition valide")
}

fn config() -> TemporalConfig {
    TemporalConfig {
        namespace: "locus".to_owned(),
        task_queue: "locus-tasks".to_owned(),
    }
}

fn staffed_deterministic(definition: &WorkflowDefinition) -> DeterministicBackend {
    let mut backend = DeterministicBackend::new();
    for activity in definition.activities() {
        backend.register_activity(activity.name(), "ok");
    }
    backend
}

// ---------------------------------------------------------------------------------------------
// La traduction du statut, sans cluster du tout
// ---------------------------------------------------------------------------------------------

fn describe(status: ExecutionStatus, reason: Option<&str>) -> Description {
    Description {
        status,
        reason: reason.map(ToOwned::to_owned),
    }
}

#[test]
fn le_statut_du_cluster_se_traduit_sans_perdre_de_distinction() {
    assert_eq!(
        state_from(&describe(ExecutionStatus::Running, None), Some(false)),
        WorkflowState::Running { step: None },
        "Temporal ne connaît pas la position : `None` le dit plutôt qu'un zéro qui aurait l'air \
         d'un début"
    );
    assert_eq!(
        state_from(&describe(ExecutionStatus::Running, None), Some(true)),
        WorkflowState::Suspended { step: None }
    );
    assert_eq!(
        state_from(&describe(ExecutionStatus::Completed, None), None),
        WorkflowState::Completed
    );

    // « On l'a arrêtée » et « elle a cassé » n'appellent pas la même compensation (§11.4). Les
    // replier l'une sur l'autre ferait disparaître la question.
    assert_eq!(
        state_from(&describe(ExecutionStatus::Failed, Some("panique")), None),
        WorkflowState::Failed {
            reason: "panique".to_owned()
        }
    );
    assert_eq!(
        state_from(&describe(ExecutionStatus::TimedOut, None), None),
        WorkflowState::Failed {
            reason: "expiration côté cluster".to_owned()
        },
        "une expiration est une casse, pas une décision"
    );
    assert_eq!(
        state_from(&describe(ExecutionStatus::Terminated, Some("budget")), None),
        WorkflowState::Terminated {
            reason: "budget".to_owned()
        }
    );
    assert_eq!(
        state_from(&describe(ExecutionStatus::Canceled, Some("annulée")), None),
        WorkflowState::Terminated {
            reason: "annulée".to_owned()
        }
    );

    // Deux inconnus, et ils sont nommés plutôt que rangés sous le plus proche.
    assert!(matches!(
        state_from(&describe(ExecutionStatus::Running, None), None),
        WorkflowState::Unknown { .. }
    ));
    assert!(matches!(
        state_from(&describe(ExecutionStatus::ContinuedAsNew, None), None),
        WorkflowState::Unknown { .. }
    ));
}

#[test]
fn un_workflow_muet_reste_inconnu_et_non_running() {
    let cluster = FakeCluster::new(false);
    let definition = task_definition();
    let mut backend = TemporalWorkflowBackend::new(cluster.clone(), config());
    let id = block_on(backend.start(&definition)).expect("démarrage").id;

    let state = block_on(backend.inspect(&id)).expect("inspection");
    assert!(
        matches!(state, WorkflowState::Unknown { .. }),
        "un workflow qui ne répond pas à « {STATE_QUERY} » rend la suspension inobservable ; \
         rendre `Running` serait un défaut plausible, la forme d'inconnu qu'on ne remarque jamais \
         — obtenu : {state:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// Ce que Temporal ne sait pas faire, et comment l'adaptateur le dit
// ---------------------------------------------------------------------------------------------

#[test]
fn suspendre_passe_par_un_signal_reserve() {
    let cluster = FakeCluster::new(true);
    let definition = task_definition();
    let mut backend = TemporalWorkflowBackend::new(cluster.clone(), config());
    let id = block_on(backend.start(&definition)).expect("démarrage").id;

    block_on(backend.suspend(&id)).expect("suspension");
    assert_eq!(
        block_on(backend.inspect(&id)).expect("inspection"),
        WorkflowState::Suspended { step: None }
    );
    block_on(backend.resume(&id)).expect("reprise");
    assert_eq!(
        block_on(backend.inspect(&id)).expect("inspection"),
        WorkflowState::Running { step: None }
    );

    assert_eq!(
        cluster.signals(),
        vec![SUSPEND_SIGNAL.to_owned(), RESUME_SIGNAL.to_owned()],
        "Temporal n'a pas de pause côté serveur : suspendre est de la logique de workflow, et la \
         seule façon de la demander de l'extérieur est un signal"
    );
}

#[test]
fn l_identifiant_temporal_est_compose_et_non_tire() {
    let definition = task_definition();
    let identifiers = |()| {
        let mut backend = TemporalWorkflowBackend::new(FakeCluster::new(true), config());
        block_on(backend.start(&definition))
            .expect("démarrage")
            .id
            .as_str()
            .to_owned()
    };
    assert_eq!(
        identifiers(()),
        identifiers(()),
        "Temporal se sert du workflow_id pour dédoublonner les démarrages : un identifiant tiré \
         ferait de chaque reprise un second workflow"
    );

    let cluster = FakeCluster::new(true);
    let mut backend = TemporalWorkflowBackend::new(cluster.clone(), config());
    block_on(backend.start(&definition)).expect("démarrage");
    let start = cluster.starts().first().cloned().expect("un démarrage");
    assert_eq!(start.workflow_type, "TaskWorkflow");
    assert_eq!(start.version, CATALOG_VERSION.number());
    assert_eq!(start.namespace, "locus");
    assert_eq!(start.task_queue, "locus-tasks");
    assert_eq!(
        start.subject,
        subject()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        "§11.3 : les identifiants métier traversent la frontière, ils n'y naissent pas"
    );
}

#[test]
fn l_adaptateur_ne_garde_aucun_etat_d_execution() {
    let cluster = FakeCluster::new(true);
    let definition = task_definition();
    let mut backend = TemporalWorkflowBackend::new(cluster.clone(), config());
    let handle = block_on(backend.start(&definition)).expect("démarrage");
    assert_eq!(
        block_on(backend.inspect(&handle.id)).expect("inspection"),
        WorkflowState::Running { step: None }
    );

    // Le cluster change sans passer par l'adaptateur — un worker a fini le workflow. Si
    // l'adaptateur tenait un cache, il continuerait d'annoncer « running », et cette seconde
    // vérité divergerait au premier redémarrage du control plane.
    cluster.set_status(handle.id.as_str(), ExecutionStatus::Completed, None);
    assert_eq!(
        block_on(backend.inspect(&handle.id)).expect("inspection"),
        WorkflowState::Completed
    );
}

#[test]
fn un_signal_a_une_execution_close_est_refuse() {
    let cluster = FakeCluster::new(true);
    let definition = task_definition();
    let mut backend = TemporalWorkflowBackend::new(cluster.clone(), config());
    let id = block_on(backend.start(&definition)).expect("démarrage").id;
    cluster.set_status(id.as_str(), ExecutionStatus::Completed, None);

    let refused = block_on(backend.signal(&id, WorkflowSignal::new("ping", "").expect("signal")))
        .expect_err("le cluster refuse, et l'adaptateur ne recouvre pas son refus");
    assert!(matches!(refused, BackendError::InvalidTransition { .. }));
}

#[test]
fn une_execution_inconnue_de_l_adaptateur_est_refusee() {
    let backend = TemporalWorkflowBackend::new(FakeCluster::new(true), config());
    let stranger = WorkflowId::new("TaskWorkflow/1/obj_inconnu").expect("identifiant valide");
    assert_eq!(
        block_on(backend.inspect(&stranger)),
        Err(BackendError::Unknown { id: stranger })
    );
}

// ---------------------------------------------------------------------------------------------
// Le contrat du port, tenu par les deux moteurs
// ---------------------------------------------------------------------------------------------

/// Les six opérations, dans l'ordre, et l'étiquette d'état après chacune.
///
/// Les **étiquettes** et non les états : le backend déterministe connaît son indice de pas et
/// Temporal ne le connaît pas, et exiger l'égalité stricte reviendrait à demander au second de
/// savoir ce que seul le premier peut savoir. Ce que le port promet est la suite d'étiquettes ; ce
/// qu'il ne promet pas est la précision, et la différence est écrite dans `WorkflowState`.
fn observed_labels(
    backend: &mut dyn WorkflowBackend,
    definition: &WorkflowDefinition,
) -> Vec<&'static str> {
    let id = block_on(backend.start(definition)).expect("démarrage").id;
    let mut labels = vec![block_on(backend.inspect(&id)).expect("inspection").label()];

    block_on(backend.suspend(&id)).expect("suspension");
    labels.push(block_on(backend.inspect(&id)).expect("inspection").label());

    block_on(backend.resume(&id)).expect("reprise");
    labels.push(block_on(backend.inspect(&id)).expect("inspection").label());

    block_on(backend.signal(
        &id,
        WorkflowSignal::new("budget_raised", "{}").expect("signal"),
    ))
    .expect("signal");
    labels.push(block_on(backend.inspect(&id)).expect("inspection").label());

    block_on(backend.terminate(&id, "arbitrage humain")).expect("arrêt");
    labels.push(block_on(backend.inspect(&id)).expect("inspection").label());

    labels
}

#[test]
fn le_contrat_des_six_operations_tient_sur_les_deux_backends() {
    let definition = task_definition();

    let mut deterministic = staffed_deterministic(&definition);
    let observed_deterministic = observed_labels(&mut deterministic, &definition);

    let mut temporal = TemporalWorkflowBackend::new(FakeCluster::new(true), config());
    let observed_temporal = observed_labels(&mut temporal, &definition);

    assert_eq!(
        observed_deterministic,
        vec!["running", "suspended", "running", "running", "terminated"]
    );
    assert_eq!(
        observed_temporal, observed_deterministic,
        "le port promet la même suite d'états observables quel que soit le moteur ; c'est tout ce \
         que §11.1 veut dire par « ne coder aucun invariant métier contre Temporal »"
    );
}

#[test]
fn la_raison_de_l_arret_survit_a_la_traduction() {
    let definition = task_definition();
    let mut temporal = TemporalWorkflowBackend::new(FakeCluster::new(true), config());
    let id = block_on(temporal.start(&definition)).expect("démarrage").id;
    block_on(temporal.terminate(&id, "budget épuisé")).expect("arrêt");

    assert_eq!(
        block_on(temporal.inspect(&id)).expect("inspection"),
        WorkflowState::Terminated {
            reason: "budget épuisé".to_owned()
        },
        "§11.4 : une terminaison sans motif rend l'histoire illisible sans la rendre fausse"
    );
}

#[test]
fn le_run_id_traverse_l_adaptateur() {
    // Temporal réutilise le `workflow_id` d'une exécution à l'autre : une opération qui omettrait
    // le `run_id` viserait « la dernière en date », c'est-à-dire une cible qui change toute seule.
    // Le faux cluster refuse une référence dont le `run_id` ne correspond pas ; si l'adaptateur
    // laissait tomber celui qu'il a reçu au démarrage, plus rien ici ne passerait.
    let cluster = FakeCluster::new(true);
    let definition = task_definition();
    let mut backend = TemporalWorkflowBackend::new(cluster, config());
    let id = block_on(backend.start(&definition)).expect("démarrage").id;

    assert_eq!(
        backend.execution(&id).expect("référence connue").run_id,
        "run-0001"
    );
    assert_eq!(
        block_on(backend.inspect(&id)).expect("inspection"),
        WorkflowState::Running { step: None }
    );
}
