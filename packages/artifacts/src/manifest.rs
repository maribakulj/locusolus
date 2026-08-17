//! L'`ArtifactManifest` — ADR 0005, `docs/SPEC_V1.md` §19.1, §19.2.

use std::fmt;

use locus_domain::{Confidentiality, ContentHash, ParseHashError};
use locus_protocol::Timestamp;

use crate::derivation::{Derivation, DerivationError};
use crate::state::{ArtifactState, ForbiddenTransition, transition};

/// Qui a produit l'artefact.
///
/// `task_id` et `attempt` sont exigés par le schéma ; les trois autres sont facultatifs parce
/// qu'ils le sont là-bas, et un manifeste qui les inventerait attesterait de ce qu'il ignore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProducedBy {
    /// La tâche.
    pub task_id: String,
    /// Le numéro d'attempt, à partir de 1.
    pub attempt: u32,
    /// L'agent, s'il est connu.
    pub agent_id: Option<String>,
    /// Le worker, s'il est connu.
    pub worker_id: Option<String>,
    /// Le run, s'il est connu.
    pub run_id: Option<String>,
}

impl ProducedBy {
    /// Le minimum que le schéma exige.
    #[must_use]
    pub fn new(task_id: &str, attempt: u32) -> Self {
        Self {
            task_id: task_id.to_owned(),
            attempt,
            agent_id: None,
            worker_id: None,
            run_id: None,
        }
    }
}

/// Licence et droits, quand ils sont connus.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rights {
    /// La licence.
    pub license: Option<String>,
    /// Le titulaire.
    pub holder: Option<String>,
    /// Une note libre. Donnée, jamais instruction.
    pub note: Option<String>,
}

/// Indications d'affichage.
///
/// Facultatives par construction : xiiif n'est pas requis par les agents (invariant 10), et un
/// artefact sans hint reste un artefact complet.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ViewerHints {
    /// Le genre d'affichage suggéré.
    pub kind: Option<String>,
    /// Un manifeste IIIF, s'il en existe un.
    pub iiif_manifest_url: Option<String>,
    /// Un aperçu, s'il en existe un.
    pub preview_artifact_id: Option<String>,
}

/// Ce qu'une vérification d'intégrité a constaté.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Integrity {
    /// Quand.
    pub verified_at: Option<Timestamp>,
    /// Si le hash correspondait.
    pub verified_hash_matches: Option<bool>,
    /// Par quoi.
    pub scanner: Option<String>,
}

/// Le manifeste d'un artefact.
///
/// # Ce que ce type porte, et pourquoi il les porte tous
///
/// Champ pour champ ce que `artifact-manifest.schema.json` déclare, y compris ce dont ce crate ne
/// fait rien : `rights`, `viewer_hints`, `filename`. W6.a n'en gardait que le noyau, et un
/// manifeste qui traverse un service qui ne connaît que le noyau ressortirait amputé de sa
/// licence. Un champ qu'on ne comprend pas se **transporte** ; il ne se laisse pas tomber.
///
/// L'exception est [`ArtifactManifest::history`], qui n'est sur aucun schéma — voir sa
/// documentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactManifest {
    artifact_id: String,
    declared_hash: ContentHash,
    media_type: String,
    size_bytes: u64,
    filename: Option<String>,
    produced_by: ProducedBy,
    classification: Confidentiality,
    rights: Option<Rights>,
    derived_from: Vec<Derivation>,
    viewer_hints: Option<ViewerHints>,
    state: ArtifactState,
    integrity: Option<Integrity>,
    declared_at: Option<Timestamp>,
    uploaded_at: Option<Timestamp>,
    history: Vec<ArtifactState>,
}

impl ArtifactManifest {
    /// Déclarer un artefact **avant** que son contenu arrive.
    ///
    /// # L'ordre est la garantie
    ///
    /// ADR 0005 : « hash déclaré **avant** upload ». Un manifeste créé après coup à partir du
    /// contenu reçu ne prouve rien : il dit que ce qui est arrivé est ce qui est arrivé. Déclarer
    /// d'abord fait du hash une **promesse**, que [`ArtifactManifest::uploaded`] confronte.
    ///
    /// # Errors
    ///
    /// [`ManifestError::EmptyField`] pour un identifiant ou une tâche vides,
    /// [`ManifestError::MalformedMediaType`] pour ce que le schéma n'accepterait pas comme type
    /// MIME, [`ManifestError::ZeroSize`] pour une taille nulle, [`ManifestError::SizeBeyondWire`]
    /// pour une taille qui ne tiendrait pas dans l'entier signé du schéma, et
    /// [`ManifestError::ZeroAttempt`] pour un attempt zéro — le schéma les compte à partir de 1,
    /// et un « attempt 0 » ne désignerait aucune exécution.
    pub fn declare(
        artifact_id: &str,
        declared_hash: ContentHash,
        media_type: &str,
        size_bytes: u64,
        produced_by: ProducedBy,
        classification: Confidentiality,
    ) -> Result<Self, ManifestError> {
        for (field, value) in [
            ("artifact_id", artifact_id),
            ("produced_by.task_id", produced_by.task_id.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(ManifestError::EmptyField { field });
            }
        }
        if !is_media_type(media_type) {
            return Err(ManifestError::MalformedMediaType {
                value: media_type.to_owned(),
            });
        }
        if size_bytes == 0 {
            return Err(ManifestError::ZeroSize);
        }
        if i64::try_from(size_bytes).is_err() {
            return Err(ManifestError::SizeBeyondWire { value: size_bytes });
        }
        if produced_by.attempt == 0 {
            return Err(ManifestError::ZeroAttempt);
        }
        Ok(Self {
            artifact_id: artifact_id.to_owned(),
            declared_hash,
            media_type: media_type.to_owned(),
            size_bytes,
            filename: None,
            produced_by,
            classification,
            rights: None,
            derived_from: Vec::new(),
            viewer_hints: None,
            state: ArtifactState::Declared,
            integrity: None,
            declared_at: None,
            uploaded_at: None,
            history: vec![ArtifactState::Declared],
        })
    }

    /// Déclarer de quoi cet artefact dérive.
    ///
    /// §19.2 : par hash et par relation typée — voir [`crate::derivation`] pour ce que la relation
    /// porte que le hash seul ne porte pas.
    #[must_use]
    pub fn with_derivations(mut self, parents: Vec<Derivation>) -> Self {
        self.derived_from = parents;
        self
    }

    /// Le nom de fichier d'origine, s'il en avait un.
    #[must_use]
    pub fn with_filename(mut self, filename: &str) -> Self {
        self.filename = Some(filename.to_owned());
        self
    }

    /// Les droits, quand ils sont connus.
    #[must_use]
    pub fn with_rights(mut self, rights: Rights) -> Self {
        self.rights = Some(rights);
        self
    }

    /// Les indications d'affichage.
    #[must_use]
    pub fn with_viewer_hints(mut self, hints: ViewerHints) -> Self {
        self.viewer_hints = Some(hints);
        self
    }

    /// Ce qu'une vérification a constaté.
    #[must_use]
    pub fn with_integrity(mut self, integrity: Integrity) -> Self {
        self.integrity = Some(integrity);
        self
    }

    /// Quand la déclaration a eu lieu.
    ///
    /// Fourni, jamais lu : ce crate n'ouvre pas d'horloge, pour la même raison que le domaine —
    /// un type qui lit l'heure n'est plus testable sans elle.
    #[must_use]
    pub const fn with_declared_at(mut self, at: Timestamp) -> Self {
        self.declared_at = Some(at);
        self
    }

    /// Quand le contenu est arrivé.
    #[must_use]
    pub const fn with_uploaded_at(mut self, at: Timestamp) -> Self {
        self.uploaded_at = Some(at);
        self
    }

    /// Constater l'arrivée du contenu, et confronter son hash à celui qui avait été promis.
    ///
    /// # Errors
    ///
    /// [`ManifestError::HashMismatch`] quand le contenu reçu n'est pas celui qui avait été
    /// déclaré — c'est le seul endroit où cette comparaison a lieu, et la seule raison pour
    /// laquelle la déclaration précède l'upload ; [`ManifestError::Forbidden`] quand l'artefact
    /// n'était pas dans un état d'où l'on peut téléverser.
    pub fn uploaded(mut self, observed: &ContentHash) -> Result<Self, ManifestError> {
        if observed != &self.declared_hash {
            return Err(ManifestError::HashMismatch {
                declared: self.declared_hash.clone(),
                observed: observed.clone(),
            });
        }
        self.state = transition(self.state, ArtifactState::Uploaded)?;
        self.history.push(self.state);
        Ok(self)
    }

    /// Franchir une transition quelconque.
    ///
    /// # Errors
    ///
    /// [`ManifestError::Forbidden`] quand la table de [`crate::state`] la refuse.
    pub fn moved_to(mut self, next: ArtifactState) -> Result<Self, ManifestError> {
        self.state = transition(self.state, next)?;
        self.history.push(self.state);
        Ok(self)
    }

    /// L'identifiant.
    #[must_use]
    pub fn artifact_id(&self) -> &str {
        &self.artifact_id
    }

    /// Le hash promis à la déclaration.
    #[must_use]
    pub const fn declared_hash(&self) -> &ContentHash {
        &self.declared_hash
    }

    /// Le type MIME.
    #[must_use]
    pub fn media_type(&self) -> &str {
        &self.media_type
    }

    /// La taille annoncée.
    #[must_use]
    pub const fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    /// Le nom de fichier d'origine.
    #[must_use]
    pub fn filename(&self) -> Option<&str> {
        self.filename.as_deref()
    }

    /// Qui l'a produit.
    #[must_use]
    pub const fn produced_by(&self) -> &ProducedBy {
        &self.produced_by
    }

    /// Sa classification.
    #[must_use]
    pub const fn classification(&self) -> Confidentiality {
        self.classification
    }

    /// Ses droits.
    #[must_use]
    pub const fn rights(&self) -> Option<&Rights> {
        self.rights.as_ref()
    }

    /// Ce dont il dérive.
    #[must_use]
    pub fn derivations(&self) -> &[Derivation] {
        &self.derived_from
    }

    /// Ses indications d'affichage.
    #[must_use]
    pub const fn viewer_hints(&self) -> Option<&ViewerHints> {
        self.viewer_hints.as_ref()
    }

    /// Son état.
    #[must_use]
    pub const fn state(&self) -> ArtifactState {
        self.state
    }

    /// Ce qu'une vérification a constaté.
    #[must_use]
    pub const fn integrity(&self) -> Option<&Integrity> {
        self.integrity.as_ref()
    }

    /// Quand il a été déclaré.
    #[must_use]
    pub const fn declared_at(&self) -> Option<Timestamp> {
        self.declared_at
    }

    /// Quand son contenu est arrivé.
    #[must_use]
    pub const fn uploaded_at(&self) -> Option<Timestamp> {
        self.uploaded_at
    }

    /// Les états traversés, dans l'ordre.
    ///
    /// # Ce qu'elle est, et où elle vit vraiment
    ///
    /// Gardée parce qu'un artefact promu **après** un passage en quarantaine n'a pas la même
    /// histoire qu'un artefact promu directement, et que l'invariant 12 refuse qu'on efface le
    /// premier pour qu'il ressemble au second.
    ///
    /// Mais elle n'est sur **aucun** schéma, et c'est délibéré : l'histoire des transitions est
    /// dans l'event store, qui est la vérité institutionnelle (invariant 2). Ce champ est ce qu'en
    /// sait un manifeste tenu en mémoire, pas un registre — un manifeste relu depuis le fil ne
    /// connaît que l'état où il se trouve, et [`crate::wire`] le dit sans le maquiller.
    #[must_use]
    pub fn history(&self) -> &[ArtifactState] {
        &self.history
    }

    /// Vrai quand le contenu peut être servi, cité, dérivé.
    #[must_use]
    pub fn is_servable(&self) -> bool {
        self.state.is_servable()
    }

    /// Reconstruire un manifeste lu ailleurs, avec l'état où il se trouve.
    ///
    /// Réservé à [`crate::wire`] : franchir les transitions depuis `Declared` rejouerait une
    /// histoire qu'on n'a pas vue.
    pub(crate) fn restored(mut self, state: ArtifactState) -> Self {
        self.state = state;
        self.history = vec![state];
        self
    }
}

/// La forme d'un type MIME, telle que le schéma l'exige : `^[a-z]+/[a-zA-Z0-9.+-]+$`.
fn is_media_type(value: &str) -> bool {
    let Some((kind, subtype)) = value.split_once('/') else {
        return false;
    };
    !kind.is_empty()
        && kind.bytes().all(|byte| byte.is_ascii_lowercase())
        && !subtype.is_empty()
        && subtype
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'+' | b'-'))
}

/// Ce qui empêche un manifeste d'exister ou d'avancer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// Un champ obligatoire vide.
    EmptyField {
        /// Lequel.
        field: &'static str,
    },
    /// Une taille nulle.
    ZeroSize,
    /// Une taille qui ne tient pas sur le fil, où `size_bytes` est un entier signé.
    SizeBeyondWire {
        /// Ce qui a été donné.
        value: u64,
    },
    /// Un attempt zéro.
    ZeroAttempt,
    /// Un type MIME que le schéma refuserait.
    MalformedMediaType {
        /// Ce qui a été donné.
        value: String,
    },
    /// Un hash mal formé.
    MalformedHash(ParseHashError),
    /// Une dérivation mal formée.
    Derivation(DerivationError),
    /// Le contenu reçu n'est pas celui qui avait été déclaré.
    HashMismatch {
        /// Ce qui avait été promis.
        declared: ContentHash,
        /// Ce qui est arrivé.
        observed: ContentHash,
    },
    /// Une transition que la table refuse.
    Forbidden(ForbiddenTransition),
}

impl From<ForbiddenTransition> for ManifestError {
    fn from(refused: ForbiddenTransition) -> Self {
        Self::Forbidden(refused)
    }
}

impl From<ParseHashError> for ManifestError {
    fn from(error: ParseHashError) -> Self {
        Self::MalformedHash(error)
    }
}

impl From<DerivationError> for ManifestError {
    fn from(error: DerivationError) -> Self {
        Self::Derivation(error)
    }
}

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(formatter, "le champ « {field} » est vide"),
            Self::ZeroSize => {
                formatter.write_str("un artefact de taille nulle n'est pas un artefact")
            }
            Self::SizeBeyondWire { value } => write!(
                formatter,
                "une taille de {value} octets ne tient pas dans l'entier signé du schéma"
            ),
            Self::ZeroAttempt => formatter
                .write_str("les attempts se comptent à partir de 1 : « 0 » ne désigne rien"),
            Self::MalformedMediaType { value } => {
                write!(formatter, "« {value} » n'est pas un type MIME")
            }
            Self::MalformedHash(error) => write!(formatter, "{error}"),
            Self::Derivation(error) => write!(formatter, "{error}"),
            Self::HashMismatch { declared, observed } => write!(
                formatter,
                "le contenu reçu ({observed}) n'est pas celui qui avait été déclaré ({declared})"
            ),
            Self::Forbidden(refused) => write!(formatter, "{refused}"),
        }
    }
}

impl std::error::Error for ManifestError {}
