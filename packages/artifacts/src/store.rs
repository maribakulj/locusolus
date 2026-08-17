//! Le port de l'object store — ADR 0012 appliqué aux artefacts, `docs/SPEC_V1.md` §19.1.
//!
//! # Ce que ce module est
//!
//! Le **contrat**, et rien d'autre : aucun fichier n'est ouvert, aucun client S3 n'entre ici.
//! `CLAUDE.md` demande des ports purs avant tout branchement, et l'implémentation de référence de
//! [`crate::memory`] est le sujet de la suite de contract tests que tout driver devra passer.
//!
//! # Pourquoi un téléversement se fait en trois temps
//!
//! `begin` / `write` / `commit`, et non un `put(bytes)`. Un artefact peut peser des gigaoctets :
//! une API qui prend le contenu entier oblige à le tenir en mémoire avant de savoir s'il est
//! acceptable, ce qui est exactement l'inverse de la garantie recherchée. La borne de taille mord
//! **pendant** l'écriture, au moment où elle est dépassée, et pas après.
//!
//! # Et pourquoi le store ne hashe pas
//!
//! Choisir une implémentation de hash est une décision d'infrastructure — le domaine s'en garde
//! (`locus_domain::ContentHash` vérifie la forme et ne calcule rien), et ce paquet aussi. Le hash
//! observé arrive donc à `commit`, calculé par l'appelant à travers [`Digest`], et
//! [`crate::ingest`] est ce qui tient les deux bouts.

use std::fmt;

use locus_domain::ContentHash;

use crate::manifest::ArtifactManifest;

/// Le jeton d'un téléversement en cours.
///
/// Opaque : un appelant qui pourrait le fabriquer pourrait écrire dans un téléversement qu'il n'a
/// pas ouvert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UploadId(u64);

impl UploadId {
    /// Réservé aux implémentations du port.
    #[must_use]
    pub const fn from_raw(value: u64) -> Self {
        Self(value)
    }

    /// Sa valeur brute.
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

impl fmt::Display for UploadId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "upload-{}", self.0)
    }
}

/// Un calcul de hash incrémental, fourni par l'appelant.
///
/// Port et non implémentation, pour la raison du module : ce paquet ne choisit pas d'algorithme.
/// [`Digest::finish`] rend un [`ContentHash`] complet, préfixe compris — un digest nu ne dit pas
/// comment le recalculer.
pub trait Digest {
    /// Absorber un fragment.
    fn update(&mut self, chunk: &[u8]);

    /// Clore le calcul.
    fn finish(&mut self) -> ContentHash;
}

/// Le stockage de contenus, adressé par hash.
///
/// # Les garanties que tout backend doit tenir
///
/// 1. **Aucun octet n'entre sans manifeste déclaré** : [`ObjectStore::begin`] prend un manifeste,
///    et le refuse s'il n'est pas dans l'état où un contenu peut arriver.
/// 2. **La taille annoncée borne l'écriture** : [`ObjectStore::write`] échoue au fragment qui la
///    dépasse, sans avoir accepté le reste.
/// 3. **Un téléversement non conclu ne laisse rien** : ni sous le hash déclaré, ni sous le hash de
///    ce qui a été écrit. La seconde moitié est la moins évidente et la plus importante — sans
///    elle, déclarer un faux hash suffirait à faire entrer un contenu arbitraire dans le store,
///    adressable ensuite par qui connaît son hash.
pub trait ObjectStore {
    /// Ouvrir un téléversement pour un artefact **déclaré**.
    ///
    /// # Errors
    ///
    /// [`StoreError::NotDeclared`] quand l'artefact n'est pas dans un état d'où un contenu peut
    /// arriver — un artefact déjà promu ne se re-téléverse pas.
    fn begin(&mut self, manifest: &ArtifactManifest) -> Result<UploadId, StoreError>;

    /// Écrire un fragment.
    ///
    /// # Errors
    ///
    /// [`StoreError::UnknownUpload`] pour un jeton inconnu ou déjà conclu, et
    /// [`StoreError::SizeExceeded`] au fragment qui dépasse la taille annoncée.
    fn write(&mut self, upload: UploadId, chunk: &[u8]) -> Result<(), StoreError>;

    /// Conclure : le contenu écrit devient lisible sous `observed`.
    ///
    /// L'appelant a confronté `observed` au hash déclaré **avant** d'appeler — c'est le travail de
    /// [`crate::ingest`], et le manifeste rendu en est la preuve.
    ///
    /// # Errors
    ///
    /// [`StoreError::UnknownUpload`] pour un jeton inconnu, et [`StoreError::SizeMismatch`] quand
    /// il manque des octets : un contenu tronqué a un autre hash, mais un backend qui accepterait
    /// de le clore l'aurait déjà écrit.
    fn commit(&mut self, upload: UploadId, observed: &ContentHash) -> Result<(), StoreError>;

    /// Abandonner : rien n'est lisible, ni sous le hash déclaré ni sous aucun autre.
    fn abort(&mut self, upload: UploadId);

    /// Relire un contenu.
    fn read(&self, hash: &ContentHash) -> Option<Vec<u8>>;

    /// Vrai quand ce contenu est présent.
    fn contains(&self, hash: &ContentHash) -> bool {
        self.read(hash).is_some()
    }
}

/// Ce qu'un store refuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// L'artefact n'est pas dans un état d'où un contenu peut arriver.
    NotDeclared {
        /// L'état où il se trouve.
        state: &'static str,
    },
    /// Un jeton inconnu, ou déjà conclu.
    UnknownUpload {
        /// Lequel.
        upload: UploadId,
    },
    /// Le contenu dépasse la taille annoncée.
    SizeExceeded {
        /// Ce qui avait été annoncé.
        declared: u64,
        /// Ce que l'écriture aurait porté le total à.
        attempted: u64,
    },
    /// Le contenu est plus court que la taille annoncée.
    SizeMismatch {
        /// Ce qui avait été annoncé.
        declared: u64,
        /// Ce qui est arrivé.
        written: u64,
    },
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotDeclared { state } => write!(
                formatter,
                "aucun contenu n'entre pour un artefact « {state} »"
            ),
            Self::UnknownUpload { upload } => {
                write!(formatter, "« {upload} » n'est pas un téléversement ouvert")
            }
            Self::SizeExceeded {
                declared,
                attempted,
            } => write!(
                formatter,
                "{declared} octets annoncés, {attempted} déjà écrits : le reste n'a pas été lu"
            ),
            Self::SizeMismatch { declared, written } => write!(
                formatter,
                "{declared} octets annoncés, {written} reçus : le contenu est incomplet"
            ),
        }
    }
}

impl std::error::Error for StoreError {}
