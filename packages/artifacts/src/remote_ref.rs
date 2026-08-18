//! La référence qu'un viewer externe reçoit — `xiiif/SPEC_V1.md` §19.
//!
//! # Ce que §19 refuse de laisser confondre
//!
//! « Une ressource distante modifiée après le run ne doit **jamais** faire croire que la preuve
//! historique a changé. Le snapshot/hash reste la référence de reproduction ; la ressource live
//! sert à constater l'évolution. »
//!
//! Deux choses différentes, donc, et la faute serait de n'en garder qu'une. Un viewer qui
//! afficherait « intégrité : divergente » sans dire *de quoi* laisserait croire que le résultat
//! scientifique est en cause, alors que c'est la source qui a bougé — et l'inverse, un viewer qui
//! tairait la divergence, ferait citer une source qui n'est plus celle qui a été lue.
//!
//! Le type porte donc **deux** verdicts distincts, et il n'existe aucun accesseur qui les
//! résumerait en un seul.
//!
//! # Un seul locator
//!
//! §19 en nomme cinq et n'en autorise qu'un. Deux locators laisseraient au viewer le soin de
//! choisir, donc de choisir différemment d'une fois sur l'autre — et deux ouvertures de la même
//! référence ne montreraient pas la même chose.

use std::fmt;

use locus_domain::ContentHash;
use locus_lep::RemoteArtifactRef as Wire;
use locus_protocol::Timestamp;

/// Comment atteindre la ressource — les cinq de §19, exclusifs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Locator {
    /// Un manifeste IIIF.
    ManifestUrl(String),
    /// Un canvas isolé.
    CanvasId(String),
    /// Un Content State IIIF.
    ContentState(String),
    /// La cible d'une annotation.
    AnnotationTarget(String),
    /// Un instantané local, hors réseau.
    LocalSnapshot(String),
}

impl Locator {
    /// Son nom de champ sur le fil.
    #[must_use]
    pub const fn slug(&self) -> &'static str {
        match self {
            Self::ManifestUrl(_) => "manifest_url",
            Self::CanvasId(_) => "canvas_id",
            Self::ContentState(_) => "content_state",
            Self::AnnotationTarget(_) => "annotation_target",
            Self::LocalSnapshot(_) => "local_snapshot",
        }
    }

    /// Vrai quand l'atteindre demande le réseau.
    ///
    /// Un instantané local se relit hors ligne : c'est ce qui permet à une preuve de rester
    /// consultable quand la source ne l'est plus.
    #[must_use]
    pub const fn needs_network(&self) -> bool {
        !matches!(self, Self::LocalSnapshot(_))
    }
}

/// Ce que le run a constaté.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observed {
    /// Le hash de l'instantané employé pendant le run — la référence de reproduction.
    pub snapshot_hash: ContentHash,
    /// Le hash de la ressource live **au moment du run**, s'il a été relevé.
    pub live_hash_at_run: Option<ContentHash>,
    /// Quand.
    pub captured_at: Option<Timestamp>,
}

/// Une référence structurée vers une ressource distante.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteArtifactRef {
    artifact_id: String,
    media_type: String,
    observed: Observed,
    locator: Locator,
    viewer_hint: Option<String>,
}

impl RemoteArtifactRef {
    /// Construire une référence.
    ///
    /// # Errors
    ///
    /// [`RemoteRefError::EmptyField`] pour une identité, un media type ou un locator vide, et
    /// [`RemoteRefError::MalformedMediaType`] pour un media type qui n'en est pas un.
    pub fn new(
        artifact_id: &str,
        media_type: &str,
        observed: Observed,
        locator: Locator,
    ) -> Result<Self, RemoteRefError> {
        if artifact_id.trim().is_empty() {
            return Err(RemoteRefError::EmptyField {
                field: "artifact_id",
            });
        }
        if !media_type.contains('/') || media_type.starts_with('/') || media_type.ends_with('/') {
            return Err(RemoteRefError::MalformedMediaType {
                value: media_type.to_owned(),
            });
        }
        if locator_value(&locator).trim().is_empty() {
            return Err(RemoteRefError::EmptyField { field: "locator" });
        }
        Ok(Self {
            artifact_id: artifact_id.to_owned(),
            media_type: media_type.to_owned(),
            observed,
            locator,
            viewer_hint: None,
        })
    }

    /// Poser une suggestion de viewer.
    ///
    /// Une suggestion, jamais une exigence : xiiif n'est pas requis par les agents (invariant 10).
    #[must_use]
    pub fn hinting(mut self, hint: &str) -> Self {
        self.viewer_hint = Some(hint.to_owned());
        self
    }

    /// L'identité canonique côté Locus.
    #[must_use]
    pub fn artifact_id(&self) -> &str {
        &self.artifact_id
    }

    /// Le media type.
    #[must_use]
    pub fn media_type(&self) -> &str {
        &self.media_type
    }

    /// Ce que le run a constaté.
    #[must_use]
    pub const fn observed(&self) -> &Observed {
        &self.observed
    }

    /// Comment atteindre la ressource.
    #[must_use]
    pub const fn locator(&self) -> &Locator {
        &self.locator
    }

    /// La suggestion de viewer, s'il y en a une.
    #[must_use]
    pub fn viewer_hint(&self) -> Option<&str> {
        self.viewer_hint.as_deref()
    }

    /// Ce que dit l'instantané, confronté à un contenu relu.
    ///
    /// **C'est le seul verdict qui parle de la preuve.** Un instantané qui ne correspond plus
    /// signifie que la reproduction est en cause ; c'est grave, et c'est rare.
    #[must_use]
    pub fn proof_standing(&self, replayed: &ContentHash) -> Standing {
        if *replayed == self.observed.snapshot_hash {
            Standing::Holds
        } else {
            Standing::Broken
        }
    }

    /// Ce que dit la ressource live, confrontée à ce qu'elle valait au run.
    ///
    /// **Ce verdict ne parle jamais de la preuve.** Une source qui a bougé depuis le run est un
    /// fait à constater, pas un résultat à remettre en cause : §19 l'écrit, et confondre les deux
    /// ferait douter d'un travail correct chaque fois qu'une bibliothèque remanie son site.
    ///
    /// Rend [`Drift::Unknown`] quand rien n'a été relevé au run : l'absence de relevé n'est pas
    /// l'absence de dérive.
    #[must_use]
    pub fn live_drift(&self, live_now: &ContentHash) -> Drift {
        match &self.observed.live_hash_at_run {
            None => Drift::Unknown,
            Some(at_run) if at_run == live_now => Drift::Unchanged,
            Some(_) => Drift::Moved,
        }
    }
}

impl RemoteArtifactRef {
    /// Relire un document — le lecteur validant de W6.b, appliqué ici.
    ///
    /// # Ce que le type généré ne peut pas dire
    ///
    /// Le schéma porte `maxProperties: 1` sur `locator` ; Rust ne sait pas l'exprimer, donc le
    /// type engendré offre **cinq champs facultatifs**. Un document à deux locators le traverse
    /// sans bruit, et l'exclusivité ne serait alors tenue que par le validateur JSON — c'est-à-dire
    /// nulle part, dès qu'un producteur construit la valeur en mémoire.
    ///
    /// C'est la faute que W6.a avait laissée passer pour le manifeste et que W6.b a corrigée : le
    /// document **est** le type engendré, et le domaine ajoute les refus que le schéma ne peut pas
    /// faire respecter à l'exécution.
    ///
    /// # Errors
    ///
    /// [`RemoteRefError::LocatorCount`] pour zéro ou plusieurs locators, [`RemoteRefError::MalformedHash`]
    /// pour un hash illisible, et ce que [`RemoteArtifactRef::new`] refuse.
    pub fn from_wire(wire: &Wire) -> Result<Self, RemoteRefError> {
        let mut found: Vec<Locator> = Vec::new();
        if let Some(value) = &wire.locator.manifest_url {
            found.push(Locator::ManifestUrl(value.clone()));
        }
        if let Some(value) = &wire.locator.canvas_id {
            found.push(Locator::CanvasId(value.clone()));
        }
        if let Some(value) = &wire.locator.content_state {
            found.push(Locator::ContentState(value.clone()));
        }
        if let Some(value) = &wire.locator.annotation_target {
            found.push(Locator::AnnotationTarget(value.clone()));
        }
        if let Some(value) = &wire.locator.local_snapshot {
            found.push(Locator::LocalSnapshot(value.clone()));
        }
        if found.len() > 1 {
            return Err(RemoteRefError::LocatorCount { found: found.len() });
        }
        // `next()` plutôt qu'un `expect` : le cas zéro est un refus, pas une supposition, et une
        // fonction de relecture qui pourrait paniquer sur un document mal formé serait précisément
        // le contraire de ce qu'elle est censée être.
        let locator = found
            .into_iter()
            .next()
            .ok_or(RemoteRefError::LocatorCount { found: 0 })?;

        let observed = Observed {
            snapshot_hash: parse_hash(&wire.expected.snapshot_hash)?,
            live_hash_at_run: wire
                .expected
                .live_hash_at_run
                .as_ref()
                .map(parse_hash)
                .transpose()?,
            captured_at: None,
        };

        let reference = Self::new(&wire.artifact_id, &wire.media_type, observed, locator)?;
        Ok(match &wire.viewer_hint {
            Some(hint) => reference.hinting(&format!("{hint:?}").to_lowercase()),
            None => reference,
        })
    }
}

fn parse_hash(value: &locus_lep::Hash) -> Result<ContentHash, RemoteRefError> {
    ContentHash::parse(value).map_err(|_| RemoteRefError::MalformedHash {
        value: value.clone(),
    })
}

fn locator_value(locator: &Locator) -> &str {
    match locator {
        Locator::ManifestUrl(value)
        | Locator::CanvasId(value)
        | Locator::ContentState(value)
        | Locator::AnnotationTarget(value)
        | Locator::LocalSnapshot(value) => value,
    }
}

/// Ce que dit l'instantané — le verdict qui porte sur la preuve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// La reproduction retrouve ce que le run avait lu.
    Holds,
    /// Elle ne le retrouve pas.
    Broken,
}

/// Ce que dit la ressource live — le verdict qui ne porte **pas** sur la preuve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drift {
    /// La source est telle qu'elle était.
    Unchanged,
    /// La source a bougé depuis le run.
    Moved,
    /// Rien n'avait été relevé : on ne peut rien en dire.
    Unknown,
}

/// Ce qui empêche une référence d'exister.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteRefError {
    /// Un champ obligatoire vide.
    EmptyField {
        /// Lequel.
        field: &'static str,
    },
    /// Un media type mal formé.
    MalformedMediaType {
        /// La valeur reçue.
        value: String,
    },
    /// Zéro locator, ou plusieurs.
    LocatorCount {
        /// Combien ont été trouvés.
        found: usize,
    },
    /// Un hash illisible.
    MalformedHash {
        /// La valeur reçue.
        value: String,
    },
}

impl fmt::Display for RemoteRefError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(formatter, "« {field} » est vide"),
            Self::MalformedMediaType { value } => {
                write!(formatter, "« {value} » n'est pas un media type")
            }
            Self::LocatorCount { found } => write!(
                formatter,
                "{found} locator(s) : §19 en veut exactement un, et le type engendré ne peut pas \
                 le dire — deux laisseraient au viewer le soin de choisir"
            ),
            Self::MalformedHash { value } => {
                write!(formatter, "« {value} » n'est pas un hash lisible")
            }
        }
    }
}

impl std::error::Error for RemoteRefError {}
