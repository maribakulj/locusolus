//! Le `RunManifest` — `docs/SPEC_V1.md` §19.6, et le niveau qu'il soutient (§19.7).
//!
//! # Un lecteur validant, pas un second modèle
//!
//! W6.b a montré ce que coûte un type de domaine écrit à côté du schéma plutôt que contre lui.
//! Ici le document **est** `locus_lep::RunManifest` — les types générés sont fidèles au fil, et le
//! recopier champ pour champ ne ferait que rouvrir la même dérive. Ce que ce module ajoute est ce
//! que le schéma ne peut pas dire : les refus, et le calcul du niveau de reproductibilité.
//!
//! [`RunManifest`] est donc un document **relu et jugé**. On ne peut pas en tenir un sans que ses
//! horodatages soient canoniques, ses commandes non vides, et son niveau déclaré soutenu par ce
//! qu'il consigne.

use std::fmt;

use locus_domain::ContentHash;
use locus_lep::RunManifest as WireRun;
use locus_protocol::Timestamp;

use crate::reproducibility::{Assessment, Caveat, Level, Missing};

/// Un run consigné, relu et jugé.
#[derive(Debug, Clone, PartialEq)]
pub struct RunManifest {
    document: WireRun,
    assessment: Assessment,
    started_at: Timestamp,
    completed_at: Option<Timestamp>,
}

impl RunManifest {
    /// Relire un run venu du fil.
    ///
    /// # Errors
    ///
    /// [`RunError`] pour ce que le schéma laisse passer et qu'un run ne peut pas être : un attempt
    /// zéro, une commande sans arguments, un hash ou un horodatage que le domaine ne relit pas,
    /// un niveau inconnu — et surtout un niveau **déclaré au-dessus de ce que le manifeste
    /// soutient**, qui est le refus pour lequel ce module existe.
    pub fn from_wire(document: &WireRun) -> Result<Self, RunError> {
        for (field, value) in [
            ("run_id", document.run_id.as_str()),
            ("task_id", document.task_id.as_str()),
            (
                "environment.environment_id",
                document.environment.environment_id.as_str(),
            ),
        ] {
            if value.trim().is_empty() {
                return Err(RunError::EmptyField { field });
            }
        }
        if document.attempt < 1 {
            return Err(RunError::ImpossibleAttempt {
                value: document.attempt,
            });
        }
        if document.commands.is_empty() {
            return Err(RunError::NoCommands);
        }
        for command in &document.commands {
            // Le schéma exige `minItems: 1` sur `argv`, et la raison vaut d'être tenue ici aussi :
            // c'est un tableau d'arguments, pas une ligne de shell. Une commande vide ne rejoue
            // rien, et une chaîne à réinterpréter par un shell n'est ni reproductible ni sûre.
            if command.argv.is_empty() || command.argv[0].trim().is_empty() {
                return Err(RunError::EmptyArgv);
            }
        }

        hash(&document.environment.image_digest)?;
        for input in &document.inputs {
            hash(&input.content_hash)?;
        }
        for output in document.outputs.iter().flatten() {
            hash(&output.content_hash)?;
        }

        let started_at = instant(&document.started_at)?;
        let completed_at = document.completed_at.as_deref().map(instant).transpose()?;
        if let Some(completed) = completed_at
            && completed < started_at
        {
            return Err(RunError::EndsBeforeItStarts);
        }

        let assessment = assess(document);
        if let Some(claimed) = document.reproducibility_level.as_deref() {
            let claimed = Level::parse(claimed).ok_or_else(|| RunError::UnknownLevel {
                value: claimed.to_owned(),
            })?;
            if !assessment.supports(claimed) {
                return Err(RunError::LevelNotSupported {
                    claimed,
                    attained: assessment.attained,
                    missing: assessment.missing.clone(),
                });
            }
        }

        Ok(Self {
            document: document.clone(),
            assessment,
            started_at,
            completed_at,
        })
    }

    /// Le document, tel qu'il s'écrit sur le fil.
    #[must_use]
    pub const fn to_wire(&self) -> &WireRun {
        &self.document
    }

    /// Le verdict rendu sur ce run.
    #[must_use]
    pub const fn assessment(&self) -> &Assessment {
        &self.assessment
    }

    /// L'identifiant du run.
    #[must_use]
    pub fn run_id(&self) -> &str {
        &self.document.run_id
    }

    /// Le digest de l'image, par lequel l'environnement est identifié.
    ///
    /// Toujours présent : le schéma l'exige, parce qu'un run dont l'image n'est pas identifiée par
    /// digest ne se rejoue pas — un tag peut désigner autre chose demain.
    #[must_use]
    pub fn image_digest(&self) -> &str {
        &self.document.environment.image_digest
    }

    /// Quand il a commencé.
    #[must_use]
    pub const fn started_at(&self) -> Timestamp {
        self.started_at
    }

    /// Quand il s'est terminé, s'il s'est terminé.
    #[must_use]
    pub const fn completed_at(&self) -> Option<Timestamp> {
        self.completed_at
    }
}

/// Calculer le niveau qu'un run soutient — §19.7, et le cœur de ce module.
///
/// # Ce que chaque cran demande, et d'où ça vient
///
/// - **R1**, « inputs et code identifiés » : au moins un input désigné par hash, une révision de
///   code avec un commit, et un arbre de travail propre. Le schéma dit lui-même qu'« un run dirty
///   ne peut pas prétendre à R1 ».
/// - **R2**, « environnement verrouillé » : l'image par digest et au moins une toolchain. Le
///   schéma les exige de tout manifeste, donc **tout run qui atteint R1 atteint R2**. Ce n'est pas
///   une simplification : c'est ce que le contrat garantit déjà, et prétendre le revérifier ici
///   ferait croire à une garde là où il n'y a qu'une conséquence.
/// - **R3 et R4** ne sont pas atteignables ici, et [`Level::FROM_A_MANIFEST_ALONE`] dit pourquoi.
fn assess(document: &WireRun) -> Assessment {
    let mut missing = Vec::new();
    if document.inputs.is_empty() {
        missing.push(Missing::Inputs);
    }
    match &document.code_revision {
        Some(revision) if revision.commit.is_some() => {
            if revision.dirty == Some(true) {
                missing.push(Missing::DirtyTree);
            }
        }
        _ => missing.push(Missing::CodeRevision),
    }

    let attained = if missing.is_empty() {
        Level::FROM_A_MANIFEST_ALONE
    } else {
        Level::R0
    };
    missing.push(Missing::ReproductionNotEvidenced);

    let mut caveats = Vec::new();
    if document
        .seeds
        .as_ref()
        .is_none_or(std::collections::BTreeMap::is_empty)
    {
        caveats.push(Caveat::NoSeeds);
    }

    Assessment {
        attained,
        missing,
        caveats,
    }
}

fn hash(value: &str) -> Result<ContentHash, RunError> {
    ContentHash::parse(value).map_err(|_| RunError::MalformedHash {
        value: value.to_owned(),
    })
}

fn instant(value: &str) -> Result<Timestamp, RunError> {
    Timestamp::parse(value).map_err(|_| RunError::MalformedTimestamp {
        value: value.to_owned(),
    })
}

/// Ce qu'un document du fil peut porter et qu'un run ne peut pas être.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunError {
    /// Un champ obligatoire vide.
    EmptyField {
        /// Lequel.
        field: &'static str,
    },
    /// Un numéro d'attempt qu'aucune exécution ne porte.
    ImpossibleAttempt {
        /// Ce qui a été lu.
        value: i64,
    },
    /// Aucune commande : rien à rejouer.
    NoCommands,
    /// Une commande sans arguments.
    EmptyArgv,
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
    /// Un run qui finit avant de commencer.
    EndsBeforeItStarts,
    /// Un niveau hors de l'énumération.
    UnknownLevel {
        /// Ce qui a été lu.
        value: String,
    },
    /// Un niveau déclaré au-dessus de ce que le manifeste soutient.
    LevelNotSupported {
        /// Ce qui était déclaré.
        claimed: Level,
        /// Ce que le manifeste soutient.
        attained: Level,
        /// Ce qui manque.
        missing: Vec<Missing>,
    },
}

impl fmt::Display for RunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(formatter, "le champ « {field} » est vide"),
            Self::ImpossibleAttempt { value } => {
                write!(formatter, "aucune exécution ne porte l'attempt {value}")
            }
            Self::NoCommands => {
                formatter.write_str("un run sans commande ne consigne rien à rejouer")
            }
            Self::EmptyArgv => formatter.write_str(
                "une commande est un tableau d'arguments : le premier ne peut pas être vide",
            ),
            Self::MalformedHash { value } => {
                write!(formatter, "« {value} » n'est pas un hash de contenu")
            }
            Self::MalformedTimestamp { value } => {
                write!(formatter, "« {value} » n'est pas un horodatage canonique")
            }
            Self::EndsBeforeItStarts => {
                formatter.write_str("un run ne se termine pas avant d'avoir commencé")
            }
            Self::UnknownLevel { value } => {
                write!(formatter, "« {value} » n'est pas un niveau de §19.7")
            }
            Self::LevelNotSupported {
                claimed,
                attained,
                missing,
            } => {
                write!(formatter, "{claimed} déclaré, {attained} soutenu")?;
                for reason in missing {
                    write!(formatter, " ; {reason}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for RunError {}
