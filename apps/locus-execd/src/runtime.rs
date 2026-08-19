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
    /// Le runtime n'est pas joignable : il n'a **pas répondu**.
    ///
    /// Le binaire est introuvable, le processus n'a pas pu être lancé, l'appel a été abandonné
    /// faute d'avoir rendu la main. Distinct de [`RuntimeError::Refused`], où il a répondu — et les
    /// deux ne se réparent pas pareil : l'une envoie installer ou réveiller un runtime, l'autre
    /// envoie lire ce qu'il reproche à la demande.
    Unavailable {
        /// Ce qui a empêché d'obtenir une réponse.
        detail: String,
    },
    /// Le runtime a répondu, et il a **refusé**.
    ///
    /// # Pourquoi cette variante a dû être séparée d'`Unavailable`
    ///
    /// `PodmanBackend::expect_success` rendait `Unavailable` dans les deux cas : « podman est
    /// introuvable » et « podman a répondu 125 ». `W5.r` a rendu la confusion visible en la faisant
    /// remonter jusqu'au rapport de sondes — un runtime tué produisait alors « la sandbox a été
    /// refusée », alors qu'il n'y avait eu aucun refus, seulement un silence. Le nom du motif a dû
    /// être élargi faute de pouvoir tenir la distinction ; elle se tient maintenant.
    ///
    /// Le verbe et le code voyagent séparément du texte : un appelant qui veut décider — retenter,
    /// abandonner, changer d'hôte — ne devrait pas avoir à lire une phrase pour retrouver un entier.
    Refused {
        /// Le verbe demandé au runtime : `create`, `start`, `exec`, `stop`, `rm`.
        verb: String,
        /// Le code qu'il a rendu.
        code: i32,
        /// Ce qu'il a écrit en refusant.
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
            Self::Refused { verb, code, detail } => {
                write!(formatter, "podman {verb} a rendu {code} : {detail}")
            }
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

    /// Arrêter l'exécution. **La sandbox existe toujours après.**
    ///
    /// La formulation compte, parce que la précédente promettait de « rendre ce qu'elle tenait » et
    /// que l'implémentation ne le faisait pas : `podman stop` arrête les processus et laisse
    /// derrière lui le **nom** et la **couche inscriptible**. Rendre est le travail de
    /// [`RuntimePort::remove`].
    ///
    /// Les deux restent séparés parce que l'entre-deux est un état légitime : une sandbox arrêtée
    /// se réinspecte, et [`RuntimePort::attestation`] se lit après l'arrêt.
    ///
    /// # Errors
    ///
    /// [`RuntimeError`] quand l'arrêt échoue.
    fn stop(&mut self, id: &SandboxId) -> Result<(), RuntimeError>;

    /// Retirer la sandbox : son nom redevient libre, sa couche inscriptible disparaît.
    ///
    /// # Ce que son absence coûtait
    ///
    /// `selftest` avait vu la conséquence sans en voir la cause : « un hôte qui accumule des
    /// conteneurs d'épreuve finit par ne plus pouvoir en créer ». La précaution qu'il en tirait —
    /// arrêter même quand la suite s'est mal passée — ne suffit pas, parce que **c'est le nom, pas
    /// l'exécution, qui manque au suivant**. Trois passages de CI l'ont montré : le second
    /// conteneur échouait avec « the container name `locus-0001` is already in use », et le harnais
    /// lisait cette erreur là où il attendait un verdict de confinement.
    ///
    /// # Et ce qu'elle rendait invérifiable
    ///
    /// La sonde `persist_after_teardown` demande qu'un fichier écrit dans la sandbox ne survive pas
    /// au démontage. Sans retrait, il n'y a **pas de démontage** : la sonde ne pouvait pas mesurer
    /// ce qu'elle annonce.
    ///
    /// # Errors
    ///
    /// [`RuntimeError`] quand le retrait échoue. Retirer une sandbox déjà inconnue rend
    /// [`RuntimeError::Unknown`] : « je ne l'ai jamais eue » et « je l'ai rendue » sont deux faits
    /// différents, et les confondre laisserait croire à un nettoyage qui n'a pas eu lieu.
    fn remove(&mut self, id: &SandboxId) -> Result<(), RuntimeError>;

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
