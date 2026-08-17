//! L'adaptateur Temporal — `docs/SPEC_V1.md` §11.1, ADR 0003, ADR 0015.
//!
//! # Ce qui est livré, et ce qui ne l'est pas
//!
//! Ce module contient **la traduction** des six opérations de §11.1 vers les concepts de Temporal,
//! et rien d'autre. La liaison au fil — gRPC, `temporal-sdk-core` — n'est pas livrée : l'ADR 0015
//! dit pourquoi et ce qui la débloquerait.
//!
//! Le nommer autrement serait le mensonge le plus facile de tout ce chantier. Un
//! `TemporalWorkflowBackend` qui ne parle à aucun cluster n'est pas un backend Temporal ; c'est une
//! traduction testée, ce qui est utile et n'est pas la même chose. [`TemporalGateway`] est la
//! couture par où le cluster entrera, et elle a la forme de l'API réelle — un nom de méthode par
//! RPC de `WorkflowService`.
//!
//! # Ce que la traduction a appris au port
//!
//! Écrire un second moteur est ce qui révèle ce que le premier avait imposé sans le dire. Trois
//! choses ont dû changer dans `WorkflowState`, et les trois étaient invisibles tant que le seul
//! moteur était en mémoire : l'indice de pas est devenu optionnel, `Failed` est apparu à côté de
//! `Terminated`, et `Unknown` est apparu tout court. Voir la documentation de
//! [`locus_workflow::WorkflowState`].

use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;

use locus_workflow::{
    BackendError, Outcome, WorkflowBackend, WorkflowDefinition, WorkflowHandle, WorkflowId,
    WorkflowSignal, WorkflowState,
};

/// Ce que rend un appel au cluster.
pub type Call<'a, T> = Pin<Box<dyn Future<Output = Result<T, GatewayError>> + Send + 'a>>;

/// Le signal réservé qui demande la suspension.
///
/// Temporal **n'a pas** de pause côté serveur pour une exécution de workflow : `PauseActivity`
/// existe, `PauseWorkflow` non. Suspendre est donc de la logique de workflow, et la seule façon de
/// la demander de l'extérieur est un signal. Le préfixe `__locus_` marque ce que le control plane
/// s'est réservé, pour qu'un signal métier ne le heurte pas par accident.
pub const SUSPEND_SIGNAL: &str = "__locus_suspend";

/// Le signal réservé qui demande la reprise.
pub const RESUME_SIGNAL: &str = "__locus_resume";

/// La requête réservée par laquelle un workflow dit s'il est suspendu.
///
/// Une **query** Temporal, parce que la suspension vit dans l'état du workflow et que le serveur
/// n'en sait rien. Un workflow qui n'y répond pas rend l'inspection incapable de conclure — et
/// c'est alors [`WorkflowState::Unknown`] qui est rendu, jamais `Running`.
pub const STATE_QUERY: &str = "__locus_state";

/// La réponse de [`STATE_QUERY`] quand le workflow s'est mis en pause.
pub const SUSPENDED_ANSWER: &str = "suspended";

/// De quel cluster et sur quelle file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemporalConfig {
    /// Le namespace Temporal.
    pub namespace: String,
    /// La file de tâches sur laquelle les workers écoutent.
    pub task_queue: String,
}

/// De quoi désigner une exécution côté cluster.
///
/// `run_id` est distinct de `workflow_id` : Temporal réutilise le second d'une exécution à l'autre,
/// et une opération qui l'omettrait viserait « la dernière en date », c'est-à-dire une cible qui
/// change toute seule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionRef {
    /// L'identifiant métier de l'exécution.
    pub workflow_id: String,
    /// L'identifiant de la tentative, attribué par le cluster.
    pub run_id: String,
}

/// Ce qu'il faut pour démarrer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartRequest {
    /// Le namespace.
    pub namespace: String,
    /// La file.
    pub task_queue: String,
    /// Le type de workflow — le nom de §11.2.
    pub workflow_type: String,
    /// L'identifiant métier choisi par l'appelant.
    pub workflow_id: String,
    /// La version sous laquelle l'exécution démarre.
    pub version: u32,
    /// Les identifiants métier du sujet, frappés avant l'entrée (§11.3).
    pub subject: Vec<String>,
}

/// Le statut que le cluster rend — les valeurs de `WorkflowExecutionStatus`.
///
/// Transcrites, et non réduites : c'est ici que se décide ce que le port croira. Replier
/// `Failed` sur `Terminated` ferait disparaître la question que §11.4 pose à la compensation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionStatus {
    /// `WORKFLOW_EXECUTION_STATUS_RUNNING`.
    Running,
    /// `WORKFLOW_EXECUTION_STATUS_COMPLETED`.
    Completed,
    /// `WORKFLOW_EXECUTION_STATUS_FAILED`.
    Failed,
    /// `WORKFLOW_EXECUTION_STATUS_CANCELED`.
    Canceled,
    /// `WORKFLOW_EXECUTION_STATUS_TERMINATED`.
    Terminated,
    /// `WORKFLOW_EXECUTION_STATUS_CONTINUED_AS_NEW`.
    ContinuedAsNew,
    /// `WORKFLOW_EXECUTION_STATUS_TIMED_OUT`.
    TimedOut,
}

/// Ce que `DescribeWorkflowExecution` rend, réduit à ce que le port utilise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Description {
    /// Le statut.
    pub status: ExecutionStatus,
    /// Le motif d'arrêt ou d'échec, quand le cluster en a un.
    pub reason: Option<String>,
}

/// Ce qui peut mal se passer côté cluster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayError {
    /// Aucune exécution de cet identifiant.
    NotFound {
        /// La référence demandée.
        execution: ExecutionRef,
    },
    /// L'exécution n'est plus en cours.
    NotRunning {
        /// La référence.
        execution: ExecutionRef,
    },
    /// Le workflow ne répond pas à cette requête.
    ///
    /// Le cas courant, et non un incident : un workflow qui n'implémente pas [`STATE_QUERY`] est
    /// parfaitement valide. C'est l'inspection qui doit alors le dire.
    QueryUnsupported {
        /// Le nom de la requête.
        query: String,
    },
    /// Le cluster a refusé ou n'a pas répondu.
    Unavailable {
        /// Ce qu'il a dit.
        detail: String,
    },
}

impl fmt::Display for GatewayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { execution } => {
                write!(formatter, "aucune exécution « {} »", execution.workflow_id)
            }
            Self::NotRunning { execution } => write!(
                formatter,
                "l'exécution « {} » n'est plus en cours",
                execution.workflow_id
            ),
            Self::QueryUnsupported { query } => {
                write!(formatter, "le workflow ne répond pas à « {query} »")
            }
            Self::Unavailable { detail } => write!(formatter, "cluster indisponible : {detail}"),
        }
    }
}

impl std::error::Error for GatewayError {}

/// Ce que le control plane demande à un cluster Temporal.
///
/// Une méthode par RPC de `WorkflowService`, et pas une de plus. La couture est étroite exprès :
/// ce qui la traverse est ce que la liaison au fil devra implémenter, et tout ce qu'on y ajouterait
/// par confort serait à écrire deux fois — une fois pour de vrai, une fois pour les tests.
///
/// `Sync` autant que `Send` : `inspect` prend `&self`, et la future qui en sort traverse les
/// threads. Un client gRPC réel l'est ; un client qui ne le serait pas ne pourrait pas servir un
/// control plane concurrent, ce que la borne dit à la compilation plutôt qu'à l'exécution.
pub trait TemporalGateway: Send + Sync {
    /// `StartWorkflowExecution`.
    fn start_workflow(&mut self, request: StartRequest) -> Call<'_, ExecutionRef>;

    /// `SignalWorkflowExecution`.
    fn signal_workflow<'a>(
        &'a mut self,
        execution: &'a ExecutionRef,
        name: &'a str,
        payload: &'a str,
    ) -> Call<'a, ()>;

    /// `TerminateWorkflowExecution`.
    fn terminate_workflow<'a>(
        &'a mut self,
        execution: &'a ExecutionRef,
        reason: &'a str,
    ) -> Call<'a, ()>;

    /// `DescribeWorkflowExecution`.
    fn describe_workflow<'a>(&'a self, execution: &'a ExecutionRef) -> Call<'a, Description>;

    /// `DescribeWorkflowExecution` **sans `run_id`** : le cluster rend la tentative courante.
    ///
    /// C'est ce qui rend un redémarrage du control plane possible. Le `workflow_id` est composé,
    /// donc reconstructible à partir de la définition seule ; le `run_id`, lui, appartient au
    /// cluster, et c'est à lui qu'on le redemande.
    fn resolve_execution<'a>(&'a self, workflow_id: &'a str) -> Call<'a, ExecutionRef>;

    /// `QueryWorkflow`.
    fn query_workflow<'a>(
        &'a self,
        execution: &'a ExecutionRef,
        query: &'a str,
    ) -> Call<'a, String>;
}

/// Le backend de §11.1 traduit vers Temporal.
///
/// Ce que l'adaptateur garde en mémoire est **uniquement** la correspondance entre l'identifiant du
/// port et la référence du cluster. Il ne garde pas d'état d'exécution : la vérité est dans le
/// cluster, et un cache local serait une seconde vérité qui diverge au premier redémarrage du
/// control plane.
pub struct TemporalWorkflowBackend<G: TemporalGateway> {
    gateway: G,
    config: TemporalConfig,
    executions: BTreeMap<String, ExecutionRef>,
    started: usize,
}

impl<G: TemporalGateway> TemporalWorkflowBackend<G> {
    /// Brancher l'adaptateur sur un cluster.
    pub fn new(gateway: G, config: TemporalConfig) -> Self {
        Self {
            gateway,
            config,
            executions: BTreeMap::new(),
            started: 0,
        }
    }

    /// L'identifiant que cette définition aura côté cluster.
    ///
    /// Composé et non tiré, donc **reconstructible sans mémoire** : c'est ce qui permet à
    /// [`TemporalWorkflowBackend::reattach`] de retrouver une exécution après un redémarrage du
    /// control plane.
    #[must_use]
    pub fn workflow_id(definition: &WorkflowDefinition) -> String {
        format!(
            "{}/{}/{}",
            definition.kind().name(),
            definition.version().number(),
            definition
                .subject()
                .first()
                .map_or_else(String::new, ToString::to_string)
        )
    }

    /// Retrouver une exécution après un redémarrage du control plane — W3.e.
    ///
    /// Un adaptateur neuf n'a plus sa table : c'est **tout** ce qu'il perd, parce que c'est tout ce
    /// qu'il avait. L'exécution, elle, n'a rien perdu — elle vit dans le cluster, qui n'a pas
    /// redémarré. Le rattachement recompose le `workflow_id` depuis la définition et redemande le
    /// `run_id` courant.
    ///
    /// C'est la contrepartie exacte de `resume_from` côté déterministe, et les deux disent la même
    /// chose sur des vérités différentes : ce qui survit à un crash est ce qui n'était pas dans le
    /// processus qui a crashé.
    ///
    /// # Errors
    ///
    /// [`BackendError::Unknown`] si le cluster ne connaît pas cette exécution.
    pub fn reattach(&mut self, definition: &WorkflowDefinition) -> Outcome<'_, WorkflowHandle> {
        let workflow_id = Self::workflow_id(definition);
        let kind = definition.kind();
        let version = definition.version();
        Box::pin(async move {
            let id = WorkflowId::new(&workflow_id)?;
            let execution = self
                .gateway
                .resolve_execution(&workflow_id)
                .await
                .map_err(|error| lift(&error, &id))?;
            self.executions.insert(id.as_str().to_owned(), execution);
            Ok(WorkflowHandle { id, kind, version })
        })
    }

    /// La référence cluster d'un identifiant du port.
    ///
    /// # Errors
    ///
    /// [`BackendError::Unknown`] si l'adaptateur n'a jamais démarré cette exécution.
    pub fn execution(&self, id: &WorkflowId) -> Result<&ExecutionRef, BackendError> {
        self.executions
            .get(id.as_str())
            .ok_or_else(|| BackendError::Unknown { id: id.clone() })
    }
}

/// Traduire le statut du cluster en état du port.
///
/// Séparée de l'adaptateur pour être testable sans cluster, même faux — et parce que c'est la seule
/// fonction de ce module où une erreur de jugement se paierait en information perdue plutôt qu'en
/// panne visible.
#[must_use]
pub fn state_from(description: &Description, suspended: Option<bool>) -> WorkflowState {
    let reason = || description.reason.clone().unwrap_or_default();
    match description.status {
        ExecutionStatus::Running => match suspended {
            Some(true) => WorkflowState::Suspended { step: None },
            Some(false) => WorkflowState::Running { step: None },
            // La suspension vit dans l'état du workflow ; un workflow qui ne répond pas à la
            // requête réservée la rend inobservable. Rendre `Running` serait un défaut plausible,
            // c'est-à-dire la forme d'inconnu qu'on ne remarque jamais.
            None => WorkflowState::Unknown {
                detail: format!(
                    "le cluster dit « running », mais le workflow ne répond pas à « {STATE_QUERY} » : \
                     suspension non observable"
                ),
            },
        },
        ExecutionStatus::Completed => WorkflowState::Completed,
        ExecutionStatus::Failed => WorkflowState::Failed { reason: reason() },
        ExecutionStatus::TimedOut => WorkflowState::Failed {
            reason: if description.reason.is_some() {
                reason()
            } else {
                "expiration côté cluster".to_owned()
            },
        },
        ExecutionStatus::Terminated | ExecutionStatus::Canceled => {
            WorkflowState::Terminated { reason: reason() }
        }
        // `ContinuedAsNew` n'est pas une fin : l'exécution continue sous un autre `run_id` que
        // l'adaptateur ne connaît pas. Prétendre `Completed` ferait croire à un aboutissement, et
        // prétendre `Running` ferait croire que la référence en main est encore la bonne.
        ExecutionStatus::ContinuedAsNew => WorkflowState::Unknown {
            detail:
                "l'exécution a continué sous un nouveau run_id, que cette référence ne désigne \
                     plus"
                    .to_owned(),
        },
    }
}

fn lift(error: &GatewayError, id: &WorkflowId) -> BackendError {
    match error {
        GatewayError::NotFound { .. } => BackendError::Unknown { id: id.clone() },
        GatewayError::NotRunning { .. } => BackendError::InvalidTransition {
            id: id.clone(),
            from: WorkflowState::Unknown {
                detail: "le cluster dit l'exécution close".to_owned(),
            },
            attempted: "signal",
        },
        GatewayError::QueryUnsupported { .. } | GatewayError::Unavailable { .. } => {
            BackendError::Unknown { id: id.clone() }
        }
    }
}

impl<G: TemporalGateway> WorkflowBackend for TemporalWorkflowBackend<G> {
    fn start<'a>(&'a mut self, definition: &'a WorkflowDefinition) -> Outcome<'a, WorkflowHandle> {
        Box::pin(async move {
            self.started += 1;
            // L'identifiant métier est composé, pas tiré : Temporal s'en sert pour dédoublonner les
            // démarrages, et un identifiant aléatoire ferait de chaque reprise un second workflow.
            let workflow_id = Self::workflow_id(definition);
            let id = WorkflowId::new(&workflow_id)?;
            let request = StartRequest {
                namespace: self.config.namespace.clone(),
                task_queue: self.config.task_queue.clone(),
                workflow_type: definition.kind().name().to_owned(),
                workflow_id,
                version: definition.version().number(),
                subject: definition
                    .subject()
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
            };
            let execution = self
                .gateway
                .start_workflow(request)
                .await
                .map_err(|error| lift(&error, &id))?;
            self.executions.insert(id.as_str().to_owned(), execution);
            Ok(WorkflowHandle {
                id,
                kind: definition.kind(),
                version: definition.version(),
            })
        })
    }

    fn signal<'a>(&'a mut self, id: &'a WorkflowId, signal: WorkflowSignal) -> Outcome<'a, ()> {
        Box::pin(async move {
            let execution = self.execution(id)?.clone();
            self.gateway
                .signal_workflow(&execution, &signal.name, &signal.payload)
                .await
                .map_err(|error| lift(&error, id))
        })
    }

    fn suspend<'a>(&'a mut self, id: &'a WorkflowId) -> Outcome<'a, ()> {
        Box::pin(async move {
            let execution = self.execution(id)?.clone();
            self.gateway
                .signal_workflow(&execution, SUSPEND_SIGNAL, "")
                .await
                .map_err(|error| lift(&error, id))
        })
    }

    fn resume<'a>(&'a mut self, id: &'a WorkflowId) -> Outcome<'a, ()> {
        Box::pin(async move {
            let execution = self.execution(id)?.clone();
            self.gateway
                .signal_workflow(&execution, RESUME_SIGNAL, "")
                .await
                .map_err(|error| lift(&error, id))
        })
    }

    fn terminate<'a>(&'a mut self, id: &'a WorkflowId, reason: &'a str) -> Outcome<'a, ()> {
        Box::pin(async move {
            let execution = self.execution(id)?.clone();
            self.gateway
                .terminate_workflow(&execution, reason)
                .await
                .map_err(|error| lift(&error, id))
        })
    }

    fn inspect<'a>(&'a self, id: &'a WorkflowId) -> Outcome<'a, WorkflowState> {
        Box::pin(async move {
            let execution = self.execution(id)?;
            let description = self
                .gateway
                .describe_workflow(execution)
                .await
                .map_err(|error| lift(&error, id))?;
            // La requête n'est posée que si le cluster dit « en cours » : interroger un workflow
            // clos ne rendrait rien, et le compter comme une suspension inobservable ferait passer
            // toutes les exécutions terminées pour inconnues.
            let suspended = if description.status == ExecutionStatus::Running {
                match self.gateway.query_workflow(execution, STATE_QUERY).await {
                    Ok(answer) => Some(answer == SUSPENDED_ANSWER),
                    Err(GatewayError::QueryUnsupported { .. }) => None,
                    Err(error) => return Err(lift(&error, id)),
                }
            } else {
                None
            };
            Ok(state_from(&description, suspended))
        })
    }
}
