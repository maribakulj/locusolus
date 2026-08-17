//! L'`ArtifactManifest` — ADR 0005, `docs/SPEC_V1.md` §19.1, §19.2.

use std::fmt;

use crate::state::{ArtifactState, ForbiddenTransition, transition};

/// Un hash de contenu.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContentHash(String);

impl ContentHash {
    /// Lire un hash.
    ///
    /// # Errors
    ///
    /// [`ManifestError::MalformedHash`] pour ce qui n'est pas un `sha256:` suivi de soixante-quatre
    /// caractères hexadécimaux.
    pub fn new(value: &str) -> Result<Self, ManifestError> {
        let hex = value
            .strip_prefix("sha256:")
            .filter(|hex| hex.len() == 64 && hex.chars().all(|char| char.is_ascii_hexdigit()));
        if hex.is_none() {
            return Err(ManifestError::MalformedHash {
                value: value.to_owned(),
            });
        }
        Ok(Self(value.to_owned()))
    }

    /// Sa forme textuelle.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Qui a produit l'artefact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProducedBy {
    /// La tâche.
    pub task_id: String,
    /// Le numéro d'attempt.
    pub attempt: u32,
}

/// Le manifeste d'un artefact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactManifest {
    artifact_id: String,
    declared_hash: ContentHash,
    media_type: String,
    size_bytes: u64,
    produced_by: ProducedBy,
    classification: String,
    state: ArtifactState,
    derived_from: Vec<ContentHash>,
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
    /// [`ManifestError::EmptyField`] pour un identifiant, un media type ou une classification
    /// vides, et [`ManifestError::ZeroSize`] pour une taille nulle — un artefact vide n'est pas un
    /// artefact, et sa taille sert à borner l'upload avant de l'accepter.
    pub fn declare(
        artifact_id: &str,
        declared_hash: ContentHash,
        media_type: &str,
        size_bytes: u64,
        produced_by: ProducedBy,
        classification: &str,
    ) -> Result<Self, ManifestError> {
        for (field, value) in [
            ("artifact_id", artifact_id),
            ("media_type", media_type),
            ("classification", classification),
        ] {
            if value.trim().is_empty() {
                return Err(ManifestError::EmptyField { field });
            }
        }
        if size_bytes == 0 {
            return Err(ManifestError::ZeroSize);
        }
        Ok(Self {
            artifact_id: artifact_id.to_owned(),
            declared_hash,
            media_type: media_type.to_owned(),
            size_bytes,
            produced_by,
            classification: classification.to_owned(),
            state: ArtifactState::Declared,
            derived_from: Vec::new(),
            history: vec![ArtifactState::Declared],
        })
    }

    /// Déclarer de quoi cet artefact dérive, **par hash**.
    ///
    /// §19.2 : « relations de dérivation ». Par hash et non par nom : un chemin change, un contenu
    /// non. Nommer un parent par son chemin ferait pointer la provenance vers ce qui se trouve
    /// aujourd'hui à cet endroit, pas vers ce dont on a effectivement dérivé.
    #[must_use]
    pub fn derived_from(mut self, parents: Vec<ContentHash>) -> Self {
        self.derived_from = parents;
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

    /// Qui l'a produit.
    #[must_use]
    pub const fn produced_by(&self) -> &ProducedBy {
        &self.produced_by
    }

    /// Sa classification.
    #[must_use]
    pub fn classification(&self) -> &str {
        &self.classification
    }

    /// Son état.
    #[must_use]
    pub const fn state(&self) -> ArtifactState {
        self.state
    }

    /// Ce dont il dérive.
    #[must_use]
    pub fn parents(&self) -> &[ContentHash] {
        &self.derived_from
    }

    /// Les états traversés, dans l'ordre.
    ///
    /// Gardés parce qu'un artefact promu **après** un passage en quarantaine n'a pas la même
    /// histoire qu'un artefact promu directement, et que l'invariant 12 refuse qu'on efface le
    /// premier pour qu'il ressemble au second.
    #[must_use]
    pub fn history(&self) -> &[ArtifactState] {
        &self.history
    }

    /// Vrai quand le contenu peut être servi, cité, dérivé.
    #[must_use]
    pub fn is_servable(&self) -> bool {
        self.state.is_servable()
    }
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
    /// Un hash mal formé.
    MalformedHash {
        /// Ce qui a été donné.
        value: String,
    },
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

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(formatter, "le champ « {field} » est vide"),
            Self::ZeroSize => {
                formatter.write_str("un artefact de taille nulle n'est pas un artefact")
            }
            Self::MalformedHash { value } => {
                write!(formatter, "« {value} » n'est pas un hash sha256")
            }
            Self::HashMismatch { declared, observed } => write!(
                formatter,
                "le contenu reçu ({observed}) n'est pas celui qui avait été déclaré ({declared})"
            ),
            Self::Forbidden(refused) => write!(formatter, "{refused}"),
        }
    }
}

impl std::error::Error for ManifestError {}
