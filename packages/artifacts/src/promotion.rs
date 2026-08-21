//! La promotion d'un artefact, sous condition de reproductibilité — `W6.g`.
//!
//! # Ce qui manquait
//!
//! `state::transition` savait dire qu'une transition est **permise** ; personne ne disait qu'elle
//! est **méritée**. Un artefact produit par un run pouvait donc passer à `Promoted` — « il peut
//! être cité, servi, dérivé » — sans qu'aucune reproductibilité ne le soutienne. L'invariant 4 dit
//! le contraire : « tout résultat scientifique majeur est artifact-first et **provenance-first** ».
//!
//! # Le gate ne s'applique pas à tout, et la frontière se lit dans le type
//!
//! Un artefact déposé par un humain n'a **aucun run** à reproduire : le gater sur `Missing::Inputs`
//! reviendrait à lui reprocher de ne pas être ce qu'il n'a jamais prétendu être. Ce qui distingue
//! les deux est déjà écrit — `ProducedBy::run_id`. Un artefact qui nomme un run **affirme** venir
//! d'une exécution ; c'est cette affirmation-là qui se vérifie.
//!
//! La 3D est l'occasion de cet item, pas sa portée : un maillage produit par un pipeline de
//! photogrammétrie est un résultat de calcul comme un autre, et rien ne justifierait qu'il soit
//! tenu à une exigence que les autres artefacts générés n'ont pas.
//!
//! # Le refus nomme ce qui manque
//!
//! `Assessment` porte déjà `missing`. Un refus qui dirait seulement « pas assez reproductible »
//! obligerait l'appelant à relire l'évaluation pour savoir quoi corriger — et, plus grave, laisserait
//! croire que la cause est une seule.

use std::fmt;

use crate::manifest::ArtifactManifest;
use crate::reproducibility::{Assessment, Missing};
use crate::state::{ArtifactState, ForbiddenTransition, transition};

/// Promouvoir un artefact — la transition **et** ce qui la mérite.
///
/// # Errors
///
/// [`PromotionError::Forbidden`] quand l'état de départ n'autorise pas `Promoted` — c'est la
/// machine à états, inchangée ; [`PromotionError::NotReproducible`] quand l'artefact **nomme un
/// run** et que son évaluation porte au moins un manque, lesquels sont tous rendus.
pub fn promote(
    manifest: &ArtifactManifest,
    assessment: &Assessment,
) -> Result<ArtifactState, PromotionError> {
    // La machine à états d'abord : un artefact en quarantaine n'a pas à être évalué pour être
    // refusé, et lui rendre un motif de reproductibilité masquerait la vraie raison.
    let promoted = transition(manifest.state(), ArtifactState::Promoted)?;

    if manifest.produced_by().run_id.is_none() {
        return Ok(promoted);
    }
    if assessment.missing.is_empty() {
        return Ok(promoted);
    }
    Err(PromotionError::NotReproducible {
        artifact_id: manifest.artifact_id().to_owned(),
        missing: assessment.missing.clone(),
    })
}

/// Pourquoi une promotion est refusée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromotionError {
    /// L'état de départ n'autorise pas `Promoted`.
    Forbidden(ForbiddenTransition),
    /// L'artefact vient d'un run, et son évaluation porte des manques.
    ///
    /// Les manques sont rendus **tous**, pas le premier : un appelant qui corrigerait l'un pour
    /// buter sur le suivant ferait autant d'allers-retours qu'il y a de causes.
    NotReproducible {
        /// Lequel.
        artifact_id: String,
        /// Ce qui manque, dans l'ordre où l'évaluation les a constatés.
        missing: Vec<Missing>,
    },
}

impl From<ForbiddenTransition> for PromotionError {
    fn from(error: ForbiddenTransition) -> Self {
        Self::Forbidden(error)
    }
}

impl fmt::Display for PromotionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Forbidden(error) => write!(formatter, "{error}"),
            Self::NotReproducible {
                artifact_id,
                missing,
            } => {
                write!(
                    formatter,
                    "« {artifact_id} » vient d'un run et ne se promeut pas — il manque :"
                )?;
                for manque in missing {
                    write!(formatter, " {manque};")?;
                }
                write!(
                    formatter,
                    " un artefact promu peut être cité, servi et dérivé, et ce qu'on cite doit \
                     pouvoir être refait"
                )
            }
        }
    }
}

impl std::error::Error for PromotionError {}
