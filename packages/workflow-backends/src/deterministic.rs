//! Le backend déterministe de test — `docs/SPEC_V1.md` §11.1, ADR 0003.

use std::collections::BTreeMap;

use locus_workflow::{
    BackendError, Outcome, Step, WorkflowBackend, WorkflowDefinition, WorkflowHandle, WorkflowId,
    WorkflowSignal, WorkflowState,
};

use crate::history::HistoryEvent;

/// Ce qu'un pas a fait avancer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    /// Un pas est passé ; voici l'indice du suivant.
    Advanced {
        /// L'indice du prochain pas.
        step: usize,
    },
    /// L'exécution est arrivée au bout.
    Completed,
}

/// Une exécution en mémoire.
#[derive(Debug, Clone)]
struct Instance {
    definition: WorkflowDefinition,
    cursor: usize,
    state: WorkflowState,
    history: Vec<HistoryEvent>,
}

/// Un moteur de workflow qui n'attend rien, ne tire rien au sort et ne lit pas l'heure.
///
/// # Ce que « déterministe » veut dire ici
///
/// Deux exécutions de la même suite d'appels rendent le **même** historique, les mêmes
/// identifiants et le même état. Trois conséquences, et ce sont des propriétés, pas des intentions :
///
/// - les identifiants sont attribués par un compteur, pas par une horloge ni un tirage. Deux
///   moteurs neufs à qui l'on demande les mêmes démarrages rendent les mêmes identifiants, ce qui
///   est ce qui permet à un test d'être rejoué ;
/// - les résultats d'activity sont **enregistrés à l'avance**, jamais calculés. Une activity que
///   personne n'a enregistrée fait refuser le moteur : inventer un résultat rendrait une exécution
///   qui n'a pas eu lieu, et elle aurait l'air d'une vraie ;
/// - aucune opération n'attend. Les futures du port se résolvent au premier `poll`, et
///   [`crate::immediate::block_on`] panique si l'une rendait `Pending` — parce qu'attendre ici
///   voudrait dire attendre **quelque chose**, et il n'y a rien.
///
/// # Pourquoi il est écrit avant Temporal
///
/// ADR 0003. Si Temporal venait en premier, le domaine s'y adapterait sans que personne ne le
/// décide, et l'ADR deviendrait une intention. Ce moteur-ci n'a rien à quoi s'adapter.
#[derive(Debug, Default)]
pub struct DeterministicBackend {
    instances: BTreeMap<String, Instance>,
    activities: BTreeMap<String, String>,
    started: usize,
}

impl DeterministicBackend {
    /// Un moteur neuf.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enregistrer le résultat d'une activity.
    ///
    /// C'est l'équivalent d'un worker qui se déclare sur une file : sans lui, l'activity n'a pas
    /// d'exécutant et le moteur refuse. La différence est que le résultat est **fixé** — un test
    /// dit ce que le monde aurait rendu, au lieu de le demander au monde.
    pub fn register_activity(&mut self, name: &str, result: &str) {
        self.activities.insert(name.to_owned(), result.to_owned());
    }

    /// L'historique d'une exécution.
    ///
    /// # Errors
    ///
    /// [`BackendError::Unknown`] si l'identifiant n'est pas connu.
    pub fn history(&self, id: &WorkflowId) -> Result<&[HistoryEvent], BackendError> {
        Ok(&self.lookup(id)?.history)
    }

    /// Faire avancer l'exécution d'un pas.
    ///
    /// # Pourquoi cette commande n'est pas dans le port
    ///
    /// Un moteur durable avance seul : personne ne lui demande le pas suivant. Mettre `advance`
    /// dans [`WorkflowBackend`] obligerait Temporal à porter une méthode que rien n'appellerait —
    /// ce serait le port qui se plie au backend de test, exactement l'inversion que l'ADR 0003
    /// cherche à éviter.
    ///
    /// # Errors
    ///
    /// [`BackendError::Unknown`] pour un identifiant inconnu, [`BackendError::InvalidTransition`]
    /// si l'exécution n'est pas en cours, [`BackendError::UnregisteredActivity`] si le prochain pas
    /// est une activity dont personne n'a enregistré le résultat.
    pub fn advance(&mut self, id: &WorkflowId) -> Result<Progress, BackendError> {
        // Les deux champs sont empruntés séparément : le registre en lecture, l'instance en
        // écriture. Cloner le registre à chaque pas aurait marché aussi, et aurait fait payer une
        // copie par pas pour contourner l'emprunteur.
        let Self {
            instances,
            activities,
            ..
        } = self;
        let instance = instances
            .get_mut(id.as_str())
            .ok_or_else(|| BackendError::Unknown { id: id.clone() })?;

        if !matches!(instance.state, WorkflowState::Running { .. }) {
            return Err(BackendError::InvalidTransition {
                id: id.clone(),
                from: instance.state.clone(),
                attempted: "advance",
            });
        }

        let Some(step) = instance.definition.steps().get(instance.cursor).cloned() else {
            instance.state = WorkflowState::Completed;
            instance.history.push(HistoryEvent::Completed);
            return Ok(Progress::Completed);
        };

        match &step {
            Step::Deterministic { name } => {
                instance.history.push(HistoryEvent::StepEntered {
                    index: instance.cursor,
                    name: name.clone(),
                });
            }
            Step::Activity(activity) => {
                // Le résultat est cherché **avant** d'écrire quoi que ce soit : un refus qui aurait
                // déjà poussé `StepEntered` laisserait un historique décrivant un pas abordé et
                // jamais fini, c'est-à-dire un historique faux produit par une erreur bénigne.
                let Some(result) = activities.get(activity.name()) else {
                    return Err(BackendError::UnregisteredActivity {
                        id: id.clone(),
                        activity: activity.name().to_owned(),
                    });
                };
                instance.history.push(HistoryEvent::StepEntered {
                    index: instance.cursor,
                    name: activity.name().to_owned(),
                });
                instance.history.push(HistoryEvent::ActivityCompleted {
                    index: instance.cursor,
                    name: activity.name().to_owned(),
                    result: result.clone(),
                });
            }
        }

        instance.cursor += 1;
        instance.state = WorkflowState::Running {
            step: instance.cursor,
        };
        Ok(Progress::Advanced {
            step: instance.cursor,
        })
    }

    /// Avancer jusqu'à la fin.
    ///
    /// # Errors
    ///
    /// La première erreur rencontrée par [`DeterministicBackend::advance`].
    pub fn run(&mut self, id: &WorkflowId) -> Result<(), BackendError> {
        loop {
            if self.advance(id)? == Progress::Completed {
                return Ok(());
            }
        }
    }

    fn lookup(&self, id: &WorkflowId) -> Result<&Instance, BackendError> {
        self.instances
            .get(id.as_str())
            .ok_or_else(|| BackendError::Unknown { id: id.clone() })
    }

    fn lookup_mut(&mut self, id: &WorkflowId) -> Result<&mut Instance, BackendError> {
        self.instances
            .get_mut(id.as_str())
            .ok_or_else(|| BackendError::Unknown { id: id.clone() })
    }
}

impl WorkflowBackend for DeterministicBackend {
    fn start<'a>(&'a mut self, definition: &'a WorkflowDefinition) -> Outcome<'a, WorkflowHandle> {
        Box::pin(async move {
            self.started += 1;
            // Un compteur, pas une horloge ni un tirage : c'est ce qui rend deux exécutions du
            // même test comparables ligne à ligne.
            let id = WorkflowId::new(&format!("wf-{:04}", self.started))?;
            self.instances.insert(
                id.as_str().to_owned(),
                Instance {
                    definition: definition.clone(),
                    cursor: 0,
                    state: WorkflowState::Running { step: 0 },
                    history: vec![HistoryEvent::Started {
                        kind: definition.kind(),
                        version: definition.version(),
                    }],
                },
            );
            Ok(WorkflowHandle {
                id,
                kind: definition.kind(),
                version: definition.version(),
            })
        })
    }

    fn signal<'a>(&'a mut self, id: &'a WorkflowId, signal: WorkflowSignal) -> Outcome<'a, ()> {
        Box::pin(async move {
            let instance = self.lookup_mut(id)?;
            if !instance.state.is_live() {
                return Err(BackendError::InvalidTransition {
                    id: id.clone(),
                    from: instance.state.clone(),
                    attempted: "signal",
                });
            }
            instance.history.push(HistoryEvent::SignalReceived {
                name: signal.name,
                payload: signal.payload,
            });
            Ok(())
        })
    }

    fn suspend<'a>(&'a mut self, id: &'a WorkflowId) -> Outcome<'a, ()> {
        Box::pin(async move {
            let instance = self.lookup_mut(id)?;
            let WorkflowState::Running { step } = instance.state else {
                return Err(BackendError::InvalidTransition {
                    id: id.clone(),
                    from: instance.state.clone(),
                    attempted: "suspend",
                });
            };
            instance.state = WorkflowState::Suspended { step };
            instance.history.push(HistoryEvent::Suspended);
            Ok(())
        })
    }

    fn resume<'a>(&'a mut self, id: &'a WorkflowId) -> Outcome<'a, ()> {
        Box::pin(async move {
            let instance = self.lookup_mut(id)?;
            let WorkflowState::Suspended { step } = instance.state else {
                return Err(BackendError::InvalidTransition {
                    id: id.clone(),
                    from: instance.state.clone(),
                    attempted: "resume",
                });
            };
            instance.state = WorkflowState::Running { step };
            instance.history.push(HistoryEvent::Resumed);
            Ok(())
        })
    }

    fn terminate<'a>(&'a mut self, id: &'a WorkflowId, reason: &'a str) -> Outcome<'a, ()> {
        Box::pin(async move {
            let instance = self.lookup_mut(id)?;
            if !instance.state.is_live() {
                return Err(BackendError::InvalidTransition {
                    id: id.clone(),
                    from: instance.state.clone(),
                    attempted: "terminate",
                });
            }
            instance.state = WorkflowState::Terminated {
                reason: reason.to_owned(),
            };
            instance.history.push(HistoryEvent::Terminated {
                reason: reason.to_owned(),
            });
            Ok(())
        })
    }

    fn inspect<'a>(&'a self, id: &'a WorkflowId) -> Outcome<'a, WorkflowState> {
        Box::pin(async move { Ok(self.lookup(id)?.state.clone()) })
    }
}
