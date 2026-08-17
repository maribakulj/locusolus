//! La traversée du fil : `ArtifactManifest` ↔ `artifact-manifest.schema.json`.
//!
//! # Pourquoi cette traduction est écrite à la main
//!
//! Les types de `locus-lep` sont **générés** depuis les schémas, donc fidèles au fil et pas au
//! domaine : `state` y est une `String`, `relation` aussi, `size_bytes` un `i64` qui peut être
//! négatif. Rien là-dedans n'est un défaut du générateur — un schéma Draft 7 exprime ces bornes en
//! mots-clés que `typify` ne sait pas rendre en types. C'est exactement pourquoi la traduction
//! existe : elle est l'endroit **unique** où un document venu d'ailleurs devient un manifeste dont
//! les invariants tiennent, et où ce qui ne peut pas le devenir est refusé **par son nom**.
//!
//! C'est la discipline de l'ADR 0015, appliquée ici : la traduction d'abord, nommée traduction.

use std::fmt;

use locus_domain::{Confidentiality, ContentHash};
use locus_lep::{
    ArtifactManifest as WireManifest, ArtifactManifestDerivedFromItem as WireDerivation,
    ArtifactManifestIntegrity as WireIntegrity, ArtifactManifestProducedBy as WireProducedBy,
    ArtifactManifestRights as WireRights, ArtifactManifestViewerHints as WireViewerHints,
    DataClass,
};
use locus_protocol::Timestamp;

use crate::derivation::{Derivation, DerivationRelation};
use crate::manifest::{
    ArtifactManifest, Integrity, ManifestError, ProducedBy, Rights, ViewerHints,
};
use crate::state::ArtifactState;

impl ArtifactManifest {
    /// Écrire ce manifeste sous la forme que le schéma décrit.
    ///
    /// L'histoire des états ne traverse pas : elle n'est sur aucun schéma, et sa place est
    /// l'event store (invariant 2). Le document porte l'état, pas le chemin.
    #[must_use]
    pub fn to_wire(&self) -> WireManifest {
        WireManifest {
            artifact_id: self.artifact_id().to_owned(),
            content_hash: self.declared_hash().to_string(),
            media_type: self.media_type().to_owned(),
            // Total : `declare` refuse déjà ce qui ne tient pas sur le fil. Un `unwrap_or(MAX)`
            // ici écrirait une taille fausse dans un document qui prétend décrire un contenu.
            size_bytes: i64::try_from(self.size_bytes())
                .unwrap_or_else(|_| unreachable!("declare refuse une taille au-delà de i64::MAX")),
            filename: self.filename().map(ToOwned::to_owned),
            produced_by: WireProducedBy {
                task_id: self.produced_by().task_id.clone(),
                attempt: i64::from(self.produced_by().attempt),
                agent_id: self.produced_by().agent_id.clone(),
                worker_id: self.produced_by().worker_id.clone(),
                run_id: self.produced_by().run_id.clone(),
            },
            classification: data_class(self.classification()),
            rights: self.rights().map(|rights| WireRights {
                license: rights.license.clone(),
                holder: rights.holder.clone(),
                note: rights.note.clone(),
            }),
            derived_from: option_vec(
                self.derivations()
                    .iter()
                    .map(|parent| WireDerivation {
                        artifact_id: parent.artifact_id().to_owned(),
                        content_hash: parent.content_hash().map(ToString::to_string),
                        relation: parent.relation().slug().to_owned(),
                    })
                    .collect(),
            ),
            viewer_hints: self.viewer_hints().map(|hints| WireViewerHints {
                kind: hints.kind.clone(),
                iiif_manifest_url: hints.iiif_manifest_url.clone(),
                preview_artifact_id: hints.preview_artifact_id.clone(),
            }),
            state: self.state().slug().to_owned(),
            integrity: self.integrity().map(|integrity| WireIntegrity {
                verified_at: integrity.verified_at.map(|at| at.to_string()),
                verified_hash_matches: integrity.verified_hash_matches,
                scanner: integrity.scanner.clone(),
            }),
            declared_at: self.declared_at().map(|at| at.to_string()),
            uploaded_at: self.uploaded_at().map(|at| at.to_string()),
        }
    }

    /// Lire un manifeste venu du fil.
    ///
    /// # Errors
    ///
    /// [`WireError`] pour tout ce que le schéma laisse passer et que le domaine refuse : un état
    /// ou une relation hors énumération, une taille négative, un horodatage non canonique, et tout
    /// ce que [`ArtifactManifest::declare`] refuse déjà.
    pub fn from_wire(document: &WireManifest) -> Result<Self, WireError> {
        let state =
            ArtifactState::parse(&document.state).ok_or_else(|| WireError::UnknownState {
                value: document.state.clone(),
            })?;
        let size_bytes =
            u64::try_from(document.size_bytes).map_err(|_| WireError::NegativeSize {
                value: document.size_bytes,
            })?;
        let attempt = u32::try_from(document.produced_by.attempt).map_err(|_| {
            WireError::ImpossibleAttempt {
                value: document.produced_by.attempt,
            }
        })?;

        let mut parents = Vec::with_capacity(document.derived_from.as_ref().map_or(0, Vec::len));
        for parent in document.derived_from.iter().flatten() {
            let relation = DerivationRelation::parse(&parent.relation).ok_or_else(|| {
                WireError::UnknownRelation {
                    value: parent.relation.clone(),
                }
            })?;
            let hash = parent.content_hash.as_deref().map(hash).transpose()?;
            parents.push(
                Derivation::new(&parent.artifact_id, relation, hash).map_err(|error| {
                    WireError::Manifest {
                        error: ManifestError::from(error),
                    }
                })?,
            );
        }

        let manifest = ArtifactManifest::declare(
            &document.artifact_id,
            hash(&document.content_hash)?,
            &document.media_type,
            size_bytes,
            ProducedBy {
                task_id: document.produced_by.task_id.clone(),
                attempt,
                agent_id: document.produced_by.agent_id.clone(),
                worker_id: document.produced_by.worker_id.clone(),
                run_id: document.produced_by.run_id.clone(),
            },
            confidentiality(document.classification),
        )
        .map_err(|error| WireError::Manifest { error })?
        .with_derivations(parents);

        let manifest = match &document.filename {
            Some(filename) => manifest.with_filename(filename),
            None => manifest,
        };
        let manifest = match &document.rights {
            Some(rights) => manifest.with_rights(Rights {
                license: rights.license.clone(),
                holder: rights.holder.clone(),
                note: rights.note.clone(),
            }),
            None => manifest,
        };
        let manifest = match &document.viewer_hints {
            Some(hints) => manifest.with_viewer_hints(ViewerHints {
                kind: hints.kind.clone(),
                iiif_manifest_url: hints.iiif_manifest_url.clone(),
                preview_artifact_id: hints.preview_artifact_id.clone(),
            }),
            None => manifest,
        };
        let manifest = match &document.integrity {
            Some(integrity) => manifest.with_integrity(Integrity {
                verified_at: integrity.verified_at.as_deref().map(instant).transpose()?,
                verified_hash_matches: integrity.verified_hash_matches,
                scanner: integrity.scanner.clone(),
            }),
            None => manifest,
        };
        let manifest = match document.declared_at.as_deref().map(instant).transpose()? {
            Some(at) => manifest.with_declared_at(at),
            None => manifest,
        };
        let manifest = match document.uploaded_at.as_deref().map(instant).transpose()? {
            Some(at) => manifest.with_uploaded_at(at),
            None => manifest,
        };

        Ok(manifest.restored(state))
    }
}

/// Une liste vide s'écrit **absente**, jamais `[]`.
///
/// Les deux ont le même sens pour un lecteur, et des octets différents pour un hash de document.
/// §7.7 exige une canonicalisation stable ; réémettre `[]` là où l'entrée n'avait rien ferait
/// diverger deux pairs sur une donnée que ni l'un ni l'autre n'a écrite.
fn option_vec<T>(items: Vec<T>) -> Option<Vec<T>> {
    (!items.is_empty()).then_some(items)
}

fn hash(value: &str) -> Result<ContentHash, WireError> {
    ContentHash::parse(value).map_err(|_| WireError::MalformedHash {
        value: value.to_owned(),
    })
}

fn instant(value: &str) -> Result<Timestamp, WireError> {
    Timestamp::parse(value).map_err(|_| WireError::MalformedTimestamp {
        value: value.to_owned(),
    })
}

const fn data_class(confidentiality: Confidentiality) -> DataClass {
    match confidentiality {
        Confidentiality::Public => DataClass::Public,
        Confidentiality::Internal => DataClass::Internal,
        Confidentiality::Confidential => DataClass::Confidential,
        Confidentiality::Restricted => DataClass::Restricted,
    }
}

const fn confidentiality(class: DataClass) -> Confidentiality {
    match class {
        DataClass::Public => Confidentiality::Public,
        DataClass::Internal => Confidentiality::Internal,
        DataClass::Confidential => Confidentiality::Confidential,
        DataClass::Restricted => Confidentiality::Restricted,
    }
}

/// Ce qu'un document du fil peut porter et qu'un manifeste refuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireError {
    /// Un état hors de l'énumération.
    UnknownState {
        /// Ce qui a été lu.
        value: String,
    },
    /// Une relation de dérivation hors de l'énumération.
    UnknownRelation {
        /// Ce qui a été lu.
        value: String,
    },
    /// Une taille négative — le schéma dit `minimum: 0`, le type généré dit `i64`.
    NegativeSize {
        /// Ce qui a été lu.
        value: i64,
    },
    /// Un numéro d'attempt qu'aucune exécution ne porte.
    ImpossibleAttempt {
        /// Ce qui a été lu.
        value: i64,
    },
    /// Un hash que le domaine ne sait pas relire.
    MalformedHash {
        /// Ce qui a été lu.
        value: String,
    },
    /// Un horodatage hors de la forme canonique de §7.7.
    MalformedTimestamp {
        /// Ce qui a été lu.
        value: String,
    },
    /// Ce que la construction d'un manifeste refuse déjà.
    Manifest {
        /// La raison.
        error: ManifestError,
    },
}

impl fmt::Display for WireError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownState { value } => {
                write!(formatter, "« {value} » n'est pas un état d'artefact")
            }
            Self::UnknownRelation { value } => {
                write!(
                    formatter,
                    "« {value} » n'est pas une relation de dérivation"
                )
            }
            Self::NegativeSize { value } => {
                write!(formatter, "une taille de {value} octets n'existe pas")
            }
            Self::ImpossibleAttempt { value } => {
                write!(formatter, "aucune exécution ne porte l'attempt {value}")
            }
            Self::MalformedHash { value } => {
                write!(formatter, "« {value} » n'est pas un hash de contenu")
            }
            Self::MalformedTimestamp { value } => {
                write!(formatter, "« {value} » n'est pas un horodatage canonique")
            }
            Self::Manifest { error } => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for WireError {}
