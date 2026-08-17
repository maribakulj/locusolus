//! Le port `WorkflowBackend` — `docs/SPEC_V1.md` §11.1.

use std::fmt;
use std::future::Future;
use std::pin::Pin;

use crate::definition::{WorkflowDefinition, WorkflowVersion};
use crate::kind::WorkflowKind;

/// Ce que rend une opération du port.
///
/// # Pourquoi une future, alors que le backend de test n'attend rien
///
/// §11.1 écrit les six opérations en `Promise`. Un port synchrone serait plus court, et il
/// s'écrirait sans peine tant que le seul backend est en mémoire — puis Temporal arriverait, dont
/// le SDK est asynchrone, et l'adaptateur devrait bloquer un thread au milieu d'un exécuteur. Ce
/// serait le domaine s'adaptant au premier backend branché, c'est-à-dire exactement ce que l'ADR
/// 0003 range parmi les pannes.
///
/// La future est **boxée** plutôt qu'écrite en `async fn` : un `async fn` de trait n'est pas
/// compatible `dyn`, et §11.1 énumère trois implémentations choisies par profil — le choix se fait
/// donc à l'exécution, ce qui demande un objet-trait.
///
/// Aucun exécuteur n'entre ici pour autant : définir une future n'en demande pas, et ce crate n'a
/// toujours aucune dépendance.
pub type Outcome<'a, T> = Pin<Box<dyn Future<Output = Result<T, BackendError>> + Send + 'a>>;

/// L'identifiant d'une exécution, tel que le moteur le connaît.
///
/// §11.1 le donne en `string` : c'est l'identité d'une **exécution**, pas d'un objet scientifique.
/// Les deux ne se confondent pas — les objets ont leurs `StableId`, frappés avant l'entrée (§11.3),
/// et une exécution rejouée sur un autre moteur porte un autre identifiant de moteur sans que rien
/// d'épistémique n'ait changé.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkflowId(String);

impl WorkflowId {
    /// Lire un identifiant.
    ///
    /// # Errors
    ///
    /// [`BackendError::EmptyId`] pour un identifiant vide : il désignerait toutes les exécutions.
    pub fn new(value: &str) -> Result<Self, BackendError> {
        if value.trim().is_empty() {
            return Err(BackendError::EmptyId);
        }
        Ok(Self(value.to_owned()))
    }

    /// Sa forme textuelle.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WorkflowId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Ce que `start` rend : de quoi retrouver l'exécution, et sous quelle forme elle a démarré.
///
/// La version est portée par le handle parce qu'une exécution longue durée survit à la version
/// courante : savoir sous laquelle elle a démarré est ce qui permet de la rejouer plus tard avec le
/// bon code, et non avec celui d'aujourd'hui (§11.3, dernière règle).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowHandle {
    /// L'identifiant de l'exécution.
    pub id: WorkflowId,
    /// Lequel des onze.
    pub kind: WorkflowKind,
    /// La version sous laquelle elle a démarré.
    pub version: WorkflowVersion,
}

/// Où en est une exécution — ce que rend `inspect`.
///
/// # Trois choses que ce type a apprises de son deuxième moteur (W3.d)
///
/// La première version, écrite avec le seul backend déterministe sous les yeux, portait
/// `Running { step: usize }`, n'avait pas de `Failed` et pas d'`Unknown`. Les trois manques ne se
/// voyaient pas : un moteur en mémoire connaît toujours son indice de pas, ne casse jamais tout
/// seul, et n'ignore jamais son propre état. C'est **exactement** la panne que l'ADR 0003 nomme —
/// le domaine prenant la forme du premier backend branché — et elle a été trouvée en écrivant le
/// second, ce qui est la raison pour laquelle l'ADR demande deux.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowState {
    /// En cours.
    Running {
        /// L'indice du prochain pas, **quand le moteur le connaît**.
        ///
        /// Temporal ne le connaît pas : `DescribeWorkflowExecution` rend un statut, pas une
        /// position, et la position ne se déduirait qu'en tirant tout l'historique à chaque
        /// inspection. `None` dit « en cours, position non observable » plutôt que d'inventer un
        /// zéro qui aurait l'air d'un début.
        step: Option<usize>,
    },
    /// Suspendue.
    Suspended {
        /// L'indice du prochain pas, quand le moteur le connaît.
        step: Option<usize>,
    },
    /// Arrivée au bout.
    Completed,
    /// Cassée : l'exécution a échoué d'elle-même.
    ///
    /// Distincte de [`WorkflowState::Terminated`], et la distinction n'est pas cosmétique : « on
    /// l'a arrêtée » et « elle a cassé » appellent des compensations différentes (§11.4), et un
    /// moteur qui replierait l'une sur l'autre ferait disparaître la question.
    Failed {
        /// Ce qui a cassé.
        reason: String,
    },
    /// Arrêtée, et la raison est conservée.
    ///
    /// La raison n'est pas décorative : §11.4 dit que les compensations « ne réécrivent jamais
    /// l'histoire épistémique ». Une terminaison sans motif rendrait l'histoire illisible sans la
    /// rendre fausse, ce qui est la façon la plus discrète de la perdre.
    Terminated {
        /// Pourquoi.
        reason: String,
    },
    /// Le moteur n'a pas su dire où en était l'exécution.
    ///
    /// Ce n'est pas un état de l'exécution, c'est un état de la **connaissance** qu'on en a — et
    /// c'est la seule réponse honnête quand un moteur rend un statut que le port ne sait pas
    /// interpréter, ou quand la suspension n'est pas observable de l'extérieur. Rendre `Running`
    /// dans ce cas serait un défaut plausible, c'est-à-dire la forme la plus dangereuse d'inconnu.
    Unknown {
        /// Ce qui manquait pour conclure.
        detail: String,
    },
}

impl WorkflowState {
    /// Vrai quand l'exécution est **connue** vivante.
    ///
    /// [`WorkflowState::Unknown`] rend `false` : l'ignorance n'est pas une vivacité. Les appelants
    /// qui refusent une opération sur ce fondement doivent donc traiter l'inconnu à part, plutôt
    /// que de le confondre avec une exécution finie.
    #[must_use]
    pub const fn is_live(&self) -> bool {
        matches!(self, Self::Running { .. } | Self::Suspended { .. })
    }

    /// Le nom court de l'état, pour un message d'erreur.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Running { .. } => "running",
            Self::Suspended { .. } => "suspended",
            Self::Completed => "completed",
            Self::Failed { .. } => "failed",
            Self::Terminated { .. } => "terminated",
            Self::Unknown { .. } => "unknown",
        }
    }
}

/// Un signal envoyé à une exécution en cours.
///
/// La charge utile est un texte que le port **n'interprète pas**. Ce crate n'a aucune dépendance
/// de sérialisation et n'en gagnera pas une pour transporter un opaque : ce qu'un signal veut dire
/// est de la logique de workflow, et elle s'écrit en W3.c.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowSignal {
    /// Le nom du signal.
    pub name: String,
    /// Sa charge utile, non interprétée.
    pub payload: String,
}

impl WorkflowSignal {
    /// Un signal.
    ///
    /// # Errors
    ///
    /// [`BackendError::EmptyId`] si le nom est vide — un signal anonyme ne se corrèle à rien dans
    /// un historique.
    pub fn new(name: &str, payload: &str) -> Result<Self, BackendError> {
        if name.trim().is_empty() {
            return Err(BackendError::EmptyId);
        }
        Ok(Self {
            name: name.to_owned(),
            payload: payload.to_owned(),
        })
    }
}

/// Ce qu'un moteur peut refuser.
///
/// Les quatre variantes sont celles que **tout** moteur produit, y compris
/// [`BackendError::UnregisteredActivity`] : Temporal refuse de la même façon une activity qu'aucun
/// worker n'a enregistrée sur sa file. Un moteur qui inventerait un résultat plutôt que de refuser
/// rendrait une exécution qui n'a pas eu lieu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    /// Un identifiant vide.
    EmptyId,
    /// Aucune exécution de cet identifiant.
    Unknown {
        /// L'identifiant demandé.
        id: WorkflowId,
    },
    /// L'opération n'a pas de sens dans l'état courant.
    InvalidTransition {
        /// L'exécution.
        id: WorkflowId,
        /// Son état.
        from: WorkflowState,
        /// Ce qui a été demandé.
        attempted: &'static str,
    },
    /// Le moteur n'a rien qui sache exécuter cette activity.
    UnregisteredActivity {
        /// L'exécution.
        id: WorkflowId,
        /// L'activity.
        activity: String,
    },
}

impl fmt::Display for BackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => formatter.write_str("identifiant vide"),
            Self::Unknown { id } => write!(formatter, "aucune exécution « {id} »"),
            Self::InvalidTransition {
                id,
                from,
                attempted,
            } => write!(
                formatter,
                "« {id} » est {} : {attempted} n'a pas de sens ici",
                from.label()
            ),
            Self::UnregisteredActivity { id, activity } => write!(
                formatter,
                "« {id} » attend l'activity « {activity} », qu'aucun exécutant n'a enregistrée"
            ),
        }
    }
}

impl std::error::Error for BackendError {}

/// Le moteur de workflow, vu du domaine — `docs/SPEC_V1.md` §11.1.
///
/// Les six opérations sont celles du texte, et rien d'autre n'y entre. En particulier, **faire
/// avancer** une exécution n'en fait pas partie : un moteur durable avance seul, et le backend de
/// test expose sa propre commande pour le faire. La mettre ici obligerait Temporal à porter une
/// méthode que rien n'appellerait, et ce serait le port qui se plierait au backend de test.
pub trait WorkflowBackend: Send {
    /// Démarrer une exécution.
    fn start<'a>(&'a mut self, definition: &'a WorkflowDefinition) -> Outcome<'a, WorkflowHandle>;

    /// Envoyer un signal.
    fn signal<'a>(&'a mut self, id: &'a WorkflowId, signal: WorkflowSignal) -> Outcome<'a, ()>;

    /// Suspendre.
    fn suspend<'a>(&'a mut self, id: &'a WorkflowId) -> Outcome<'a, ()>;

    /// Reprendre.
    fn resume<'a>(&'a mut self, id: &'a WorkflowId) -> Outcome<'a, ()>;

    /// Arrêter, avec un motif.
    fn terminate<'a>(&'a mut self, id: &'a WorkflowId, reason: &'a str) -> Outcome<'a, ()>;

    /// Regarder où elle en est.
    fn inspect<'a>(&'a self, id: &'a WorkflowId) -> Outcome<'a, WorkflowState>;
}
