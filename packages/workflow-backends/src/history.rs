//! L'historique d'une exécution, et son rejeu — `docs/SPEC_V1.md` §11.2 et §11.3.

use std::fmt;

use locus_workflow::{Step, WorkflowDefinition, WorkflowKind, WorkflowState, WorkflowVersion};

/// Ce qui est arrivé, dans l'ordre.
///
/// L'historique est la **seule** source du rejeu. Il porte les résultats d'activity parce qu'un
/// rejeu qui les redemanderait au monde les obtiendrait différents — et rendrait une exécution qui
/// n'a pas eu lieu, avec l'air d'une reprise fidèle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryEvent {
    /// L'exécution a démarré.
    Started {
        /// Lequel des onze.
        kind: WorkflowKind,
        /// Sous quelle version.
        version: WorkflowVersion,
    },
    /// Un pas a été abordé.
    StepEntered {
        /// Son indice dans la définition.
        index: usize,
        /// Son nom.
        name: String,
    },
    /// Une activity a rendu son résultat.
    ActivityCompleted {
        /// L'indice du pas.
        index: usize,
        /// Le nom de l'activity.
        name: String,
        /// Ce qu'elle a rendu, tel qu'enregistré.
        result: String,
    },
    /// Un signal est arrivé.
    SignalReceived {
        /// Son nom.
        name: String,
        /// Sa charge utile, non interprétée.
        payload: String,
    },
    /// L'exécution a été suspendue.
    Suspended,
    /// Elle a repris.
    Resumed,
    /// Elle a été arrêtée.
    Terminated {
        /// Pourquoi.
        reason: String,
    },
    /// Elle est arrivée au bout.
    Completed,
}

/// Ce qu'un rejeu reconstitue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replayed {
    /// L'état reconstruit.
    pub state: WorkflowState,
    /// Les résultats d'activity, lus dans l'historique.
    pub activity_results: Vec<(String, String)>,
    /// Les signaux reçus, dans l'ordre.
    pub signals: Vec<(String, String)>,
}

/// Ce qui rend un historique irrejouable.
///
/// Un rejeu qui « rattraperait » ces cas rendrait un état plausible pour une exécution qui n'a pas
/// eu lieu ainsi. Il refuse à la place, et le refus dit où l'historique et la définition divergent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayError {
    /// L'historique ne commence pas par un démarrage.
    NoStart,
    /// Le démarrage ne correspond pas à la définition rejouée.
    WrongDefinition {
        /// Ce que dit l'historique.
        recorded: WorkflowKind,
        /// Ce que dit la définition.
        replayed: WorkflowKind,
    },
    /// Le démarrage porte une autre version que la définition rejouée.
    ///
    /// C'est le cas qui compte pour §11.3 : rejouer une exécution v1 avec le code v2 rendrait un
    /// état construit par des pas qu'elle n'a jamais traversés.
    WrongVersion {
        /// La version enregistrée.
        recorded: WorkflowVersion,
        /// La version rejouée.
        replayed: WorkflowVersion,
    },
    /// Un pas hors de la définition.
    UnknownStep {
        /// Son indice.
        index: usize,
    },
    /// Un pas dont le nom ne correspond pas à celui de la définition.
    RenamedStep {
        /// Son indice.
        index: usize,
        /// Le nom enregistré.
        recorded: String,
        /// Le nom d'aujourd'hui.
        expected: String,
    },
    /// Les pas ne se suivent pas.
    OutOfOrder {
        /// L'indice attendu.
        expected: usize,
        /// Celui trouvé.
        found: usize,
    },
    /// Un résultat d'activity sans pas correspondant abordé.
    ResultWithoutEntry {
        /// L'indice.
        index: usize,
    },
    /// Un résultat d'activity sur un pas déterministe.
    ResultOnDeterministicStep {
        /// L'indice.
        index: usize,
    },
    /// Une activity abordée dont le résultat manque.
    MissingResult {
        /// L'indice.
        index: usize,
    },
    /// Un événement après la fin.
    AfterEnd,
}

impl fmt::Display for ReplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoStart => formatter.write_str("l'historique ne commence pas par un démarrage"),
            Self::WrongDefinition { recorded, replayed } => write!(
                formatter,
                "historique de {recorded} rejoué contre une définition de {replayed}"
            ),
            Self::WrongVersion { recorded, replayed } => write!(
                formatter,
                "exécution démarrée en {recorded}, rejouée avec le code {replayed}"
            ),
            Self::UnknownStep { index } => {
                write!(formatter, "pas d'indice {index} hors de la définition")
            }
            Self::RenamedStep {
                index,
                recorded,
                expected,
            } => write!(
                formatter,
                "le pas {index} s'appelait « {recorded} » et s'appelle « {expected} »"
            ),
            Self::OutOfOrder { expected, found } => {
                write!(
                    formatter,
                    "pas {found} abordé alors que {expected} était dû"
                )
            }
            Self::ResultWithoutEntry { index } => {
                write!(
                    formatter,
                    "résultat du pas {index} sans que le pas soit abordé"
                )
            }
            Self::ResultOnDeterministicStep { index } => {
                write!(
                    formatter,
                    "le pas {index} est déterministe et porte un résultat"
                )
            }
            Self::MissingResult { index } => {
                write!(formatter, "l'activity {index} est abordée sans résultat")
            }
            Self::AfterEnd => formatter.write_str("un événement après la fin de l'exécution"),
        }
    }
}

impl std::error::Error for ReplayError {}

/// Reconstituer l'état d'une exécution à partir de son seul historique.
///
/// # Pourquoi une fonction libre
///
/// Elle ne prend ni le moteur, ni son registre d'activities, ni rien de vivant : la définition et
/// l'historique, et c'est tout. Une méthode sur le backend pourrait regarder l'état courant au lieu
/// de le reconstruire — et le rejeu tomberait juste **pour la mauvaise raison**, en lisant la
/// réponse au lieu de la retrouver. La panne ne se verrait qu'au premier redémarrage réel, quand
/// il n'y aurait plus rien à lire.
///
/// C'est aussi ce qui rend vraie la phrase de §11.2 : « rejoué ou repris avec un autre backend ».
/// Un historique se rejoue ici sans le moteur qui l'a produit.
///
/// # Errors
///
/// [`ReplayError`] quand l'historique et la définition divergent — plutôt qu'un état plausible.
pub fn replay(
    definition: &WorkflowDefinition,
    history: &[HistoryEvent],
) -> Result<Replayed, ReplayError> {
    let Some(HistoryEvent::Started { kind, version }) = history.first() else {
        return Err(ReplayError::NoStart);
    };
    if *kind != definition.kind() {
        return Err(ReplayError::WrongDefinition {
            recorded: *kind,
            replayed: definition.kind(),
        });
    }
    if *version != definition.version() {
        return Err(ReplayError::WrongVersion {
            recorded: *version,
            replayed: definition.version(),
        });
    }

    let mut cursor = 0_usize;
    let mut awaiting: Option<usize> = None;
    let mut suspended = false;
    let mut ended: Option<WorkflowState> = None;
    let mut activity_results = Vec::new();
    let mut signals = Vec::new();

    for event in &history[1..] {
        if ended.is_some() {
            return Err(ReplayError::AfterEnd);
        }
        match event {
            HistoryEvent::Started { .. } => return Err(ReplayError::AfterEnd),
            HistoryEvent::StepEntered { index, name } => {
                if let Some(pending) = awaiting {
                    return Err(ReplayError::MissingResult { index: pending });
                }
                let step = definition
                    .steps()
                    .get(*index)
                    .ok_or(ReplayError::UnknownStep { index: *index })?;
                if *index != cursor {
                    return Err(ReplayError::OutOfOrder {
                        expected: cursor,
                        found: *index,
                    });
                }
                if step.name() != name {
                    return Err(ReplayError::RenamedStep {
                        index: *index,
                        recorded: name.clone(),
                        expected: step.name().to_owned(),
                    });
                }
                match step {
                    // Un pas déterministe est fini dès qu'il est abordé : il n'attend personne.
                    Step::Deterministic { .. } => cursor += 1,
                    Step::Activity(_) => awaiting = Some(*index),
                }
            }
            HistoryEvent::ActivityCompleted {
                index,
                name,
                result,
            } => {
                if awaiting != Some(*index) {
                    return Err(ReplayError::ResultWithoutEntry { index: *index });
                }
                let step = definition
                    .steps()
                    .get(*index)
                    .ok_or(ReplayError::UnknownStep { index: *index })?;
                if matches!(step, Step::Deterministic { .. }) {
                    return Err(ReplayError::ResultOnDeterministicStep { index: *index });
                }
                activity_results.push((name.clone(), result.clone()));
                awaiting = None;
                cursor += 1;
            }
            HistoryEvent::SignalReceived { name, payload } => {
                signals.push((name.clone(), payload.clone()));
            }
            HistoryEvent::Suspended => suspended = true,
            HistoryEvent::Resumed => suspended = false,
            HistoryEvent::Terminated { reason } => {
                ended = Some(WorkflowState::Terminated {
                    reason: reason.clone(),
                });
            }
            HistoryEvent::Completed => ended = Some(WorkflowState::Completed),
        }
    }

    if let Some(index) = awaiting {
        return Err(ReplayError::MissingResult { index });
    }

    let state = ended.unwrap_or(if suspended {
        WorkflowState::Suspended { step: Some(cursor) }
    } else {
        WorkflowState::Running { step: Some(cursor) }
    });

    Ok(Replayed {
        state,
        activity_results,
        signals,
    })
}
