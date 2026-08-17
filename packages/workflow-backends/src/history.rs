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
    /// Une activity a été défaite par sa compensation — §11.4.
    ///
    /// Un événement **de plus**, jamais une rature. Le `ActivityCompleted` qu'il défait reste où il
    /// est : « les compensations annulent les réservations techniques […] elles ne réécrivent
    /// jamais l'histoire épistémique ». Un historique d'où l'on retirerait ce qui a été compensé
    /// décrirait une exécution où la réservation n'a jamais eu lieu — et une réservation qui n'a
    /// jamais eu lieu n'a pas consommé de capacité, ce qui est faux.
    Compensated {
        /// L'indice du pas défait.
        index: usize,
        /// L'activity défaite.
        activity: String,
        /// Celle qui l'a défaite.
        by: String,
        /// Ce qu'elle a rendu.
        result: String,
    },
    /// Elle est arrivée au bout.
    Completed,
}

/// Ce qu'un rejeu reconstitue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replayed {
    /// L'état reconstruit.
    pub state: WorkflowState,
    /// Le nombre de pas franchis — de quoi reprendre là où l'on s'était arrêté.
    pub cursor: usize,
    /// Les résultats d'activity, lus dans l'historique.
    pub activity_results: Vec<(String, String)>,
    /// Les signaux reçus, dans l'ordre.
    pub signals: Vec<(String, String)>,
    /// Les compensations appliquées, dans l'ordre où elles l'ont été.
    pub compensations: Vec<(String, String)>,
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
    check_header(definition, history)?;

    let mut cursor = 0_usize;
    let mut awaiting: Option<usize> = None;
    let mut suspended = false;
    let mut ended: Option<WorkflowState> = None;
    let mut activity_results = Vec::new();
    let mut signals = Vec::new();
    let mut compensations = Vec::new();

    for event in &history[1..] {
        // Une compensation a le droit d'arriver **après** la fin, et c'est §11.4 qui l'exige : une
        // exécution qui s'est terminée tient encore ses leases et ses fichiers temporaires jusqu'à
        // ce qu'on les rende. Le nettoyage n'est pas un événement d'exécution ; le refuser ici
        // obligerait à compenser avant de finir, c'est-à-dire à défaire une réservation dont on a
        // encore besoin.
        if ended.is_some() && !matches!(event, HistoryEvent::Compensated { .. }) {
            return Err(ReplayError::AfterEnd);
        }
        match event {
            HistoryEvent::Started { .. } => return Err(ReplayError::AfterEnd),
            HistoryEvent::StepEntered { index, name } => {
                if let Some(pending) = awaiting {
                    return Err(ReplayError::MissingResult { index: pending });
                }
                match enter_step(definition, *index, name, cursor)? {
                    // Un pas déterministe est fini dès qu'il est abordé : il n'attend personne.
                    Entered::Finished => cursor += 1,
                    Entered::Awaiting => awaiting = Some(*index),
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
            HistoryEvent::Compensated {
                index,
                activity,
                by,
                ..
            } => {
                // La compensation n'annule pas le pas : elle s'ajoute. Le curseur ne recule pas, et
                // le résultat de l'activity défaite reste dans `activity_results`. Reculer le
                // curseur referait le pas au redémarrage suivant.
                if *index >= cursor {
                    return Err(ReplayError::ResultWithoutEntry { index: *index });
                }
                compensations.push((activity.clone(), by.clone()));
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
        cursor,
        activity_results,
        signals,
        compensations,
    })
}

/// Ce qu'aborder un pas a produit.
enum Entered {
    /// Le pas est fini : c'était de la logique pure.
    Finished,
    /// Le pas attend son résultat : c'était une activity.
    Awaiting,
}

/// Vérifier que l'historique et la définition parlent bien de la même chose.
fn check_header(
    definition: &WorkflowDefinition,
    history: &[HistoryEvent],
) -> Result<(), ReplayError> {
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
    Ok(())
}

/// Confronter un pas abordé à la définition d'aujourd'hui.
fn enter_step(
    definition: &WorkflowDefinition,
    index: usize,
    name: &str,
    cursor: usize,
) -> Result<Entered, ReplayError> {
    let step = definition
        .steps()
        .get(index)
        .ok_or(ReplayError::UnknownStep { index })?;
    if index != cursor {
        return Err(ReplayError::OutOfOrder {
            expected: cursor,
            found: index,
        });
    }
    if step.name() != name {
        return Err(ReplayError::RenamedStep {
            index,
            recorded: name.to_owned(),
            expected: step.name().to_owned(),
        });
    }
    Ok(match step {
        Step::Deterministic { .. } => Entered::Finished,
        Step::Activity(_) => Entered::Awaiting,
    })
}
