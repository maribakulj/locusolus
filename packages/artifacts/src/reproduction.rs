//! Comparer un rejeu à son original — `docs/SPEC_V1.md` §19.7, R3 et R4.
//!
//! # Ce que ce module rend possible, et que W6.d refusait
//!
//! W6.d a posé que `R3` et `R4` ne se lisent dans aucun manifeste seul : « reproduction
//! automatisée » et « reproduction indépendante » sont des **événements**. Ce module est la trace
//! de cet événement. Il ne lit pas un champ : il confronte deux runs.
//!
//! # Une divergence est un résultat, pas une panne
//!
//! [`compare`] rend un [`Comparison`] où la divergence est une **valeur**, détaillée artefact par
//! artefact. Invariant 12 : « les résultats négatifs et conflits ne sont jamais supprimés pour
//! rendre le graphe propre ». Un rejeu qui ne retrouve pas les mêmes sorties est une information
//! scientifique — souvent la plus intéressante des deux — et la traiter en erreur la ferait
//! remonter comme un incident technique, c'est-à-dire disparaître.
//!
//! Ce qui **est** une erreur : comparer deux runs qui ne font pas la même chose. Là, il n'y a rien
//! à conclure ni dans un sens ni dans l'autre, et [`NotAReproduction`] le dit.

use std::fmt;

use crate::reproducibility::Level;
use crate::run::RunManifest;

/// D'où vient la connaissance de l'indépendance du rejeu.
///
/// # Pourquoi ça n'est pas lu dans les manifestes
///
/// `run-manifest.schema.json` ne nomme **aucun worker**. Rien dans deux `RunManifest` ne dit s'ils
/// ont tourné sur la même machine, et R4 exige précisément un « worker distinct ». Cette
/// connaissance appartient au plan de contrôle, qui a émis les leases : c'est lui qui sait, et
/// c'est donc lui qui le dit — explicitement, par ce type, plutôt que par une déduction que les
/// documents ne permettent pas.
///
/// [`Independence::Unknown`] plafonne à `R3`, sur la règle qui traverse ce dépôt : l'absence de
/// preuve n'est pas une preuve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Independence {
    /// Personne n'a dit sur quoi les deux runs ont tourné.
    Unknown,
    /// Le même worker a rejoué.
    SameWorker,
    /// Un worker distinct a rejoué.
    DistinctWorker {
        /// Celui de l'original.
        original: String,
        /// Celui du rejeu.
        replay: String,
    },
}

/// Ce qui empêche deux runs d'être comparables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mismatch {
    /// L'image n'est pas la même.
    ImageDigest {
        /// Celle de l'original.
        original: String,
        /// Celle du rejeu.
        replay: String,
    },
    /// Les inputs ne sont pas les mêmes.
    Inputs,
    /// Les commandes ne sont pas les mêmes.
    Commands,
}

impl fmt::Display for Mismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ImageDigest { original, replay } => write!(
                formatter,
                "l'image diffère : {original} à l'origine, {replay} au rejeu"
            ),
            Self::Inputs => formatter.write_str("les inputs ne sont pas les mêmes"),
            Self::Commands => formatter.write_str("les commandes ne sont pas les mêmes"),
        }
    }
}

/// Deux runs qu'on ne peut pas confronter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotAReproduction {
    /// Ce qui diffère de ce qui aurait dû être identique.
    pub differences: Vec<Mismatch>,
}

impl fmt::Display for NotAReproduction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ce rejeu n'exécute pas le même run")?;
        for difference in &self.differences {
            write!(formatter, " ; {difference}")?;
        }
        Ok(())
    }
}

impl std::error::Error for NotAReproduction {}

/// Ce qu'un rejeu a rendu de différent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Divergence {
    /// Le même artefact, un autre contenu.
    ContentChanged {
        /// L'artefact.
        artifact_id: String,
        /// Le hash d'origine.
        original: String,
        /// Le hash du rejeu.
        replay: String,
    },
    /// Une sortie que le rejeu n'a pas produite.
    Missing {
        /// L'artefact.
        artifact_id: String,
    },
    /// Une sortie que seul le rejeu a produite.
    Unexpected {
        /// L'artefact.
        artifact_id: String,
    },
}

impl fmt::Display for Divergence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContentChanged {
                artifact_id,
                original,
                replay,
            } => write!(
                formatter,
                "« {artifact_id} » : {original} à l'origine, {replay} au rejeu"
            ),
            Self::Missing { artifact_id } => {
                write!(formatter, "« {artifact_id} » n'a pas été reproduit")
            }
            Self::Unexpected { artifact_id } => {
                write!(formatter, "« {artifact_id} » n'existait pas à l'origine")
            }
        }
    }
}

/// Le verdict d'une reproduction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Le rejeu a rendu exactement les mêmes sorties.
    Reproduced,
    /// Le rejeu a rendu autre chose, et voici quoi.
    Diverged {
        /// Sortie par sortie.
        divergences: Vec<Divergence>,
    },
}

/// Ce qu'une reproduction établit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    /// Ce que le rejeu a rendu.
    pub verdict: Verdict,
    /// Le niveau que cette reproduction soutient.
    pub attained: Level,
    /// D'où venait la connaissance de l'indépendance.
    pub independence: Independence,
}

impl Comparison {
    /// Vrai quand le rejeu a retrouvé les mêmes sorties.
    #[must_use]
    pub const fn is_reproduced(&self) -> bool {
        matches!(self.verdict, Verdict::Reproduced)
    }
}

impl fmt::Display for Comparison {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.verdict {
            Verdict::Reproduced => write!(formatter, "reproduit, {}", self.attained),
            Verdict::Diverged { divergences } => {
                write!(formatter, "divergent, {}", self.attained)?;
                for divergence in divergences {
                    write!(formatter, " ; {divergence}")?;
                }
                Ok(())
            }
        }
    }
}

/// Confronter un rejeu à son original.
///
/// # Ce que la comparaison exige avant de conclure
///
/// Même image, mêmes inputs, mêmes commandes. Sans cela on ne compare pas un rejeu à un original,
/// on compare deux runs — et « les sorties diffèrent » ne dirait alors rien sur la
/// reproductibilité du premier.
///
/// # Le niveau que le résultat soutient
///
/// - **`R4`** : le rejeu retrouve les mêmes sorties **et** un worker distinct l'a produit.
/// - **`R3`** : le rejeu retrouve les mêmes sorties, sur le même worker ou sur un worker qu'on ne
///   sait pas distinguer.
/// - **`R2`** en cas de divergence : la reproduction a bien eu lieu, et elle n'a rien établi de
///   plus que ce que le manifeste soutenait déjà. Le niveau ne descend pas — un rejeu raté ne
///   défait pas un environnement verrouillé — et il ne monte pas non plus.
///
/// # Errors
///
/// [`NotAReproduction`] quand les deux runs ne font pas la même chose.
pub fn compare(
    original: &RunManifest,
    replay: &RunManifest,
    independence: Independence,
) -> Result<Comparison, NotAReproduction> {
    let differences = comparability(original, replay);
    if !differences.is_empty() {
        return Err(NotAReproduction { differences });
    }

    let divergences = divergences(original, replay);
    let (verdict, attained) = if divergences.is_empty() {
        let level = match independence {
            Independence::DistinctWorker { .. } => Level::R4,
            Independence::SameWorker | Independence::Unknown => Level::R3,
        };
        (Verdict::Reproduced, level)
    } else {
        (
            Verdict::Diverged { divergences },
            Level::FROM_A_MANIFEST_ALONE,
        )
    };

    Ok(Comparison {
        verdict,
        attained,
        independence,
    })
}

/// Ce qui devait être identique pour que la comparaison ait un sens.
fn comparability(original: &RunManifest, replay: &RunManifest) -> Vec<Mismatch> {
    let mut differences = Vec::new();
    if original.image_digest() != replay.image_digest() {
        differences.push(Mismatch::ImageDigest {
            original: original.image_digest().to_owned(),
            replay: replay.image_digest().to_owned(),
        });
    }

    // Par hash, et sans tenir compte de l'ordre : deux runs qui consomment les mêmes contenus
    // consomment les mêmes contenus, quel que soit l'ordre où le manifeste les a listés.
    let mut original_inputs = hashes(original.to_wire().inputs.iter().map(|i| &i.content_hash));
    let mut replay_inputs = hashes(replay.to_wire().inputs.iter().map(|i| &i.content_hash));
    original_inputs.sort_unstable();
    replay_inputs.sort_unstable();
    if original_inputs != replay_inputs {
        differences.push(Mismatch::Inputs);
    }

    // Les commandes, elles, gardent leur ordre : `train` puis `evaluate` n'est pas `evaluate` puis
    // `train`, et un rejeu qui les intervertit n'exécute pas le même run.
    let commands = |run: &RunManifest| -> Vec<Vec<String>> {
        run.to_wire()
            .commands
            .iter()
            .map(|command| command.argv.clone())
            .collect()
    };
    if commands(original) != commands(replay) {
        differences.push(Mismatch::Commands);
    }
    differences
}

/// Ce que le rejeu a rendu de différent, sortie par sortie.
fn divergences(original: &RunManifest, replay: &RunManifest) -> Vec<Divergence> {
    let mut divergences = Vec::new();
    let replayed = replay.to_wire().outputs.iter().flatten();

    for output in original.to_wire().outputs.iter().flatten() {
        match replayed
            .clone()
            .find(|candidate| candidate.artifact_id == output.artifact_id)
        {
            Some(candidate) if candidate.content_hash == output.content_hash => {}
            Some(candidate) => divergences.push(Divergence::ContentChanged {
                artifact_id: output.artifact_id.clone(),
                original: output.content_hash.clone(),
                replay: candidate.content_hash.clone(),
            }),
            None => divergences.push(Divergence::Missing {
                artifact_id: output.artifact_id.clone(),
            }),
        }
    }

    for output in replayed {
        if !original
            .to_wire()
            .outputs
            .iter()
            .flatten()
            .any(|candidate| candidate.artifact_id == output.artifact_id)
        {
            // Une sortie de plus est une divergence : le rejeu n'a pas fait la même chose, et
            // l'ignorer parce que « rien ne manque » laisserait passer un run qui produit
            // silencieusement autre chose en plus.
            divergences.push(Divergence::Unexpected {
                artifact_id: output.artifact_id.clone(),
            });
        }
    }
    divergences
}

fn hashes<'a>(values: impl Iterator<Item = &'a String>) -> Vec<&'a str> {
    values.map(String::as_str).collect()
}
