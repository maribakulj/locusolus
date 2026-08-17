//! Le port par lequel — et uniquement par lequel — on parle à un runtime de containers.

use std::fmt;

use locus_execution::{SandboxAttestation, SandboxSpec};

/// L'identifiant d'une sandbox côté runtime.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SandboxId(String);

impl SandboxId {
    /// Lire un identifiant.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::EmptyId`] pour un identifiant vide.
    pub fn new(value: &str) -> Result<Self, RuntimeError> {
        if value.trim().is_empty() {
            return Err(RuntimeError::EmptyId);
        }
        Ok(Self(value.to_owned()))
    }

    /// Sa forme textuelle.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SandboxId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Ce qu'un runtime peut refuser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    /// Un identifiant vide.
    EmptyId,
    /// Le runtime n'est pas joignable.
    Unavailable {
        /// Ce qu'il a dit.
        detail: String,
    },
    /// Le runtime ne sait pas offrir ce que la spécification demande.
    Unsupported {
        /// Quoi.
        capability: String,
    },
    /// Aucune sandbox de cet identifiant.
    Unknown {
        /// L'identifiant demandé.
        id: SandboxId,
    },
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => formatter.write_str("identifiant de sandbox vide"),
            Self::Unavailable { detail } => write!(formatter, "runtime injoignable : {detail}"),
            Self::Unsupported { capability } => {
                write!(formatter, "le runtime ne sait pas offrir « {capability} »")
            }
            Self::Unknown { id } => write!(formatter, "aucune sandbox « {id} »"),
        }
    }
}

impl std::error::Error for RuntimeError {}

/// Le runtime de containers, vu du broker.
///
/// # Pourquoi ce port existe, et pourquoi il est ici
///
/// L'ADR 0004 : « `locus-execd` est un service séparé. `locusd` ne détient **jamais** de socket
/// Docker/Podman. » La séparation n'est pas une préférence de style : le daemon du control plane
/// parle au monde entier — cockpit, workers, fédération — et un socket de runtime dans ce
/// processus-là donne à qui le compromet le pouvoir de créer des conteneurs privilégiés.
///
/// Ce trait est le seul endroit du dépôt qui décrit ce qu'on demande à un runtime. Le driver qui
/// l'implémentera (W4.d pour Linux, W4.e pour macOS) sera le seul à ouvrir un socket, et un test
/// balaie l'arbre pour vérifier que personne d'autre n'en parle.
///
/// # Ce que le port ne fait pas
///
/// Il ne décide rien. L'admission — savoir si une mission peut être honorée — se décide **avant**,
/// dans [`crate::admission`], et sur des capacités déclarées plutôt qu'en essayant pour voir. Un
/// runtime auquel on demande l'impossible échoue à mi-chemin, et un échec à mi-chemin laisse
/// derrière lui ce qu'il avait déjà créé.
pub trait RuntimePort: Send + Sync {
    /// Créer une sandbox conforme à la spécification.
    ///
    /// # Errors
    ///
    /// [`RuntimeError`] quand le runtime ne peut pas la créer.
    fn create(&mut self, spec: &SandboxSpec) -> Result<SandboxId, RuntimeError>;

    /// Démarrer ce qui a été créé.
    ///
    /// # Errors
    ///
    /// [`RuntimeError`] quand le démarrage échoue.
    fn start(&mut self, id: &SandboxId) -> Result<(), RuntimeError>;

    /// Arrêter la sandbox et rendre ce qu'elle tenait.
    ///
    /// # Errors
    ///
    /// [`RuntimeError`] quand l'arrêt échoue.
    fn stop(&mut self, id: &SandboxId) -> Result<(), RuntimeError>;

    /// Ce que le runtime atteste avoir réellement appliqué.
    ///
    /// C'est le worker qui atteste (§21.6), pas le broker : cette méthode **transmet** un
    /// témoignage, elle n'en fabrique pas un. Un broker qui composerait l'attestation à partir de
    /// ce qu'il avait demandé attesterait de sa propre demande.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::Unknown`] pour une sandbox inconnue.
    fn attestation(&self, id: &SandboxId) -> Result<SandboxAttestation, RuntimeError>;
}
