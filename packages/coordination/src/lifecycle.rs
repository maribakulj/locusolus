//! Les commandes de cycle de vie du scheduler — `docs/13` §3, `docs/SPEC_V1.md` §7.1 et §12.
//!
//! # Quatre commandes, et pourquoi pas neuf
//!
//! `docs/13` énumère ce que le scheduler doit savoir faire : « spawn, suspend, drain, kill,
//! replace, split, merge, connect, disconnect, rerouter l'état, rejouer, migrer le contexte, et
//! livrer les messages en connaissance de la version ». La liste mélange deux choses, et les
//! implémenter toutes ici en ferait treize alors qu'il y en a quatre à écrire.
//!
//! - `replace`, `split`, `merge`, `connect`, `disconnect` **sont déjà** `REPLACE_NODE`,
//!   `SPLIT_NODE`, `MERGE_NODES`, `ADD_EDGE` et `REMOVE_EDGE` de [`crate::version`]. Les réécrire
//!   ici produirait un second chemin qui divergerait du premier le jour où l'un des deux est
//!   corrigé — et personne ne saurait lequel décrit ce qui sera commité. Le scheduler les
//!   **compose**, il ne les redéfinit pas.
//! - `rerouter l'état`, `rejouer`, `migrer le contexte` et `livrer les messages` supposaient une
//!   messagerie inter-agents. Elle existe depuis l'ADR 0019 — [`crate::messaging`] — et cela ne les
//!   fait pas entrer ici pour autant : **livrer un message n'est pas une commande de scheduler**.
//!   Le scheduler pilote des instances ; la messagerie écrit et lit des faits. Les deux se croisent
//!   en un point, `drain`, et ce point a un nom : [`crate::messaging::Handover`], qui ne se
//!   construit que depuis un [`Outcome::Draining`].
//!
//!   Les trois autres restent dehors, et pour la raison que l'ADR 0019 a nommée en condition 3 :
//!   « nouvel attempt, nouvelle vue, nouveau hash » reste la règle V1, donc migrer le contexte d'une
//!   mission en cours n'a pas de sens à écrire. La condition se rouvrira le jour où des agents
//!   **persistants** apparaîtront.
//!
//! Restent quatre commandes qui n'ont aucun équivalent ailleurs, parce qu'elles portent sur
//! l'**instance qui tourne** et non sur la structure : `spawn`, `suspend`, `drain`, `kill`.
//!
//! # Ce module n'est pas une seconde machine à états
//!
//! Les états sont ceux de §7.1, et `AgentInstance::moved_to` reste seul à les porter. Ce qui est
//! écrit ici est ce que le **scheduler** a le droit de demander, et ce n'est pas la même question :
//! `waiting → active` est une transition légitime de l'instance, mais aucune commande de scheduler
//! ne s'appelle « reprendre » — c'est le lease qui la reprend.
//!
//! # La quiescence se constate, elle ne s'attend pas
//!
//! `docs/13` demande « quiescence locale d'un nœud plutôt que drain global ». La quiescence est donc
//! une **lecture** : [`Quiescence::of`] prend le nombre de tentatives en vol et rend un constat. Il
//! n'existe dans ce module aucune fonction qui attende — un `wait_for_quiescence` ferait tenir au
//! scheduler une promesse qu'il ne peut pas tenir, puisque rien n'oblige un nœud à devenir quiescent.
//! Un test le vérifie par l'absence.
//!
//! # Tuer et drainer ne disent pas la même chose
//!
//! Drainer laisse le nœud finir ; tuer abandonne ce qu'il avait en vol. Un `kill` qui rendrait le
//! même résultat sur un nœud quiescent et sur un nœud occupé cacherait exactement ce qu'un opérateur
//! doit savoir : combien de tentatives viennent d'être perdues. [`Outcome::Killed`] porte donc le
//! compte, y compris quand il vaut zéro.

use std::collections::BTreeMap;
use std::fmt;

use locus_protocol::{Id, id::Agent};

use crate::agent::InstanceState;

/// Ce qu'un scheduler demande à un nœud.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Command {
    /// Créer l'instance. Elle naît `provisioned`, jamais active.
    Spawn,
    /// L'écarter du tour sans la terminer.
    Suspend,
    /// Lui laisser finir ce qu'elle a commencé, sans rien lui donner de neuf.
    Drain,
    /// L'arrêter, en disant ce qui est abandonné.
    Kill,
}

impl Command {
    /// Les quatre, dans l'ordre de `docs/13`.
    pub const ALL: [Self; 4] = [Self::Spawn, Self::Suspend, Self::Drain, Self::Kill];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Spawn => "spawn",
            Self::Suspend => "suspend",
            Self::Drain => "drain",
            Self::Kill => "kill",
        }
    }
}

impl fmt::Display for Command {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Ce qu'un nœud a en vol, constaté.
///
/// Un **constat**, pas une attente. Rien n'oblige un nœud à devenir quiescent, et une fonction qui
/// attendrait ferait tenir au scheduler une promesse dont il n'a pas les moyens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quiescence {
    /// Plus rien en vol.
    Quiescent,
    /// Des tentatives sont en cours, et on dit combien.
    Busy {
        /// Combien.
        attempts: usize,
    },
}

impl Quiescence {
    /// Lire la quiescence d'un nœud depuis le nombre de tentatives en vol.
    #[must_use]
    pub const fn of(in_flight: usize) -> Self {
        if in_flight == 0 {
            Self::Quiescent
        } else {
            Self::Busy {
                attempts: in_flight,
            }
        }
    }

    /// Combien de tentatives sont en vol.
    #[must_use]
    pub const fn in_flight(self) -> usize {
        match self {
            Self::Quiescent => 0,
            Self::Busy { attempts } => attempts,
        }
    }
}

/// Ce qu'une commande a produit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Le nœud a changé d'état, et rien n'a été perdu.
    Settled(InstanceState),
    /// Le drain est **en cours** : le nœud finit ce qu'il a commencé.
    ///
    /// Rien d'autre n'est arrêté — c'est la quiescence locale de `docs/13`, par opposition au drain
    /// global. L'état ne change pas, et le dire est le résultat : un drain qui rendrait
    /// `Settled(Completed)` sur un nœud encore occupé mentirait sur ce qui tourne.
    Draining {
        /// Ce qu'il reste à finir.
        remaining: usize,
    },
    /// Le nœud est arrêté, et on dit ce qui a été abandonné.
    ///
    /// Le compte est porté **même quand il vaut zéro** : c'est ce qui distingue un arrêt propre d'un
    /// arrêt coûteux, et un opérateur qui ne lit pas la différence ne saura pas qu'il a perdu du
    /// travail.
    Killed {
        /// Combien de tentatives en vol ont été abandonnées.
        abandoned: usize,
    },
}

impl Outcome {
    /// L'état dans lequel le nœud se retrouve.
    #[must_use]
    pub const fn state(self, before: InstanceState) -> InstanceState {
        match self {
            Self::Settled(state) => state,
            Self::Draining { .. } => before,
            Self::Killed { .. } => InstanceState::Terminated,
        }
    }
}

/// Les instances qu'un scheduler pilote.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lifecycle {
    states: BTreeMap<Id<Agent>, InstanceState>,
}

impl Lifecycle {
    /// Un scheduler qui ne pilote rien.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// L'état d'un nœud, s'il en a un.
    #[must_use]
    pub fn state(&self, node: Id<Agent>) -> Option<InstanceState> {
        self.states.get(&node).copied()
    }

    /// Les nœuds pilotés.
    pub fn nodes(&self) -> impl Iterator<Item = (Id<Agent>, InstanceState)> {
        self.states.iter().map(|(node, state)| (*node, *state))
    }

    /// Déclarer l'état d'un nœud déjà en vie — pour reconstituer un scheduler depuis le journal.
    #[must_use]
    pub fn knowing(mut self, node: Id<Agent>, state: InstanceState) -> Self {
        self.states.insert(node, state);
        self
    }

    /// Adresser une commande à **un** nœud.
    ///
    /// Aucun autre n'est touché : c'est la quiescence locale de `docs/13`, et le test qui compte est
    /// celui qui vérifie que les voisins n'ont pas bougé.
    ///
    /// # Errors
    ///
    /// [`LifecycleError::AlreadySpawned`] pour un `spawn` sur une instance qui existe ;
    /// [`LifecycleError::NoSuchInstance`] pour toute autre commande sur une instance absente ;
    /// [`LifecycleError::AlreadyTerminal`] sur une instance terminée — la ranimer effacerait la
    /// trace de sa fin (§14.2) ; [`LifecycleError::Forbidden`] pour une transition que l'état de
    /// départ n'autorise pas, en nommant **les deux** états.
    pub fn command(
        &mut self,
        node: Id<Agent>,
        command: Command,
        quiescence: Quiescence,
    ) -> Result<Outcome, LifecycleError> {
        let current = self.states.get(&node).copied();
        let outcome = decide(node, current, command, quiescence)?;
        if let Some(before) = current {
            self.states.insert(node, outcome.state(before));
        } else {
            self.states
                .insert(node, outcome.state(InstanceState::Provisioned));
        }
        Ok(outcome)
    }
}

/// La décision, séparée de l'écriture pour qu'elle se lise seule.
fn decide(
    node: Id<Agent>,
    current: Option<InstanceState>,
    command: Command,
    quiescence: Quiescence,
) -> Result<Outcome, LifecycleError> {
    let Some(state) = current else {
        return match command {
            Command::Spawn => Ok(Outcome::Settled(InstanceState::Provisioned)),
            _ => Err(LifecycleError::NoSuchInstance {
                node: node.to_string(),
                command,
            }),
        };
    };
    if command == Command::Spawn {
        return Err(LifecycleError::AlreadySpawned {
            node: node.to_string(),
            state,
        });
    }
    if state.is_terminal() {
        return Err(LifecycleError::AlreadyTerminal {
            node: node.to_string(),
            state,
        });
    }
    match command {
        // Suspendre écarte du tour ce qui y était. Une instance seulement provisionnée n'y est pas
        // encore, et la « suspendre » laisserait croire qu'on a arrêté quelque chose.
        Command::Suspend => {
            if state == InstanceState::Active {
                Ok(Outcome::Settled(InstanceState::Waiting))
            } else {
                Err(LifecycleError::Forbidden {
                    node: node.to_string(),
                    command,
                    from: state,
                    to: InstanceState::Waiting,
                })
            }
        }
        // Drainer : finir ce qui est commencé, ne rien donner de neuf. Sur un nœud occupé, l'état
        // **ne change pas** et le drain se poursuit ; le dire est le résultat.
        Command::Drain => match quiescence {
            Quiescence::Quiescent => Ok(Outcome::Settled(InstanceState::Completed)),
            Quiescence::Busy { attempts } => Ok(Outcome::Draining {
                remaining: attempts,
            }),
        },
        Command::Kill => Ok(Outcome::Killed {
            abandoned: quiescence.in_flight(),
        }),
        Command::Spawn => unreachable!("traité plus haut"),
    }
}

/// Un nœud peut-il quitter la version ?
///
/// C'est la règle qui relie ce module à [`crate::version`] : `REMOVE_NODE` retire un nœud de
/// l'organisation, et le retirer pendant que son instance tourne ferait dire à la version qu'un
/// agent est parti alors qu'il travaille encore. La version ne peut pas le savoir seule — elle ne
/// détient que des identités — donc c'est ici que la question se pose, et le scheduler compose les
/// deux.
///
/// # Errors
///
/// [`LifecycleError::StillRunning`] tant que l'instance n'est pas terminée.
pub fn may_leave_the_version(node: Id<Agent>, state: InstanceState) -> Result<(), LifecycleError> {
    if state.is_terminal() {
        return Ok(());
    }
    Err(LifecycleError::StillRunning {
        node: node.to_string(),
        state,
    })
}

/// Ce qui empêche une commande d'aboutir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleError {
    /// Un `spawn` sur une instance qui existe déjà.
    AlreadySpawned {
        /// Lequel.
        node: String,
        /// Dans quel état elle est.
        state: InstanceState,
    },
    /// Une commande sur une instance qui n'existe pas.
    NoSuchInstance {
        /// Lequel.
        node: String,
        /// Ce qu'on lui demandait.
        command: Command,
    },
    /// Une commande sur une instance terminée.
    AlreadyTerminal {
        /// Lequel.
        node: String,
        /// L'état terminal.
        state: InstanceState,
    },
    /// Une transition que l'état de départ n'autorise pas.
    Forbidden {
        /// Lequel.
        node: String,
        /// Ce qu'on demandait.
        command: Command,
        /// D'où l'on part.
        from: InstanceState,
        /// Où l'on voulait aller.
        to: InstanceState,
    },
    /// Un nœud qu'on voudrait retirer de la version alors qu'il tourne.
    StillRunning {
        /// Lequel.
        node: String,
        /// Son état.
        state: InstanceState,
    },
}

impl fmt::Display for LifecycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadySpawned { node, state } => write!(
                formatter,
                "{node} existe déjà, en « {state} » : le respawn effacerait ce qui tourne"
            ),
            Self::NoSuchInstance { node, command } => {
                write!(formatter, "« {command} » sur {node}, qui n'existe pas")
            }
            Self::AlreadyTerminal { node, state } => write!(
                formatter,
                "{node} est en « {state} » : la ranimer effacerait la trace de sa fin"
            ),
            Self::Forbidden {
                node,
                command,
                from,
                to,
            } => write!(
                formatter,
                "« {command} » mènerait {node} de « {from} » à « {to} », ce que « {from} » \
                 n'autorise pas"
            ),
            Self::StillRunning { node, state } => write!(
                formatter,
                "{node} est en « {state} » : le retirer de la version ferait dire qu'il est parti \
                 alors qu'il travaille encore"
            ),
        }
    }
}

impl std::error::Error for LifecycleError {}
