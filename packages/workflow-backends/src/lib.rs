//! Les implémentations du port `WorkflowBackend` — `docs/SPEC_V1.md` §11.1.
//!
//! # L'ordre est une décision, pas une commodité
//!
//! ADR 0003 : le backend déterministe de test s'écrit **avant** Temporal. Si Temporal venait en
//! premier, le domaine s'adapterait à lui sans que personne ne le décide — les invariants
//! prendraient la forme de ce que le SDK rend facile, et l'ADR deviendrait une intention qu'on cite
//! après coup. Le moteur de ce paquet n'a rien à quoi s'adapter : il tient en mémoire, n'attend
//! rien, ne tire rien au sort et ne lit pas l'heure.
//!
//! C'est aussi ce que `boundaries.json` vérifie dans l'autre sens : le SDK Temporal, quel que soit
//! son écosystème, n'a le droit d'apparaître que sous ce paquet.
//!
//! # Le rejeu est une fonction libre, et c'est le cœur
//!
//! [`history::replay`] ne prend que la définition et l'historique — ni le moteur, ni son registre
//! d'activities. Une méthode sur le backend pourrait regarder l'état courant au lieu de le
//! reconstruire, et le rejeu tomberait juste **pour la mauvaise raison** : en lisant la réponse au
//! lieu de la retrouver. La panne ne se verrait qu'au premier redémarrage réel, quand il n'y aurait
//! plus rien à lire.

pub mod deterministic;
pub mod history;
pub mod immediate;

pub use deterministic::{DeterministicBackend, Progress};
pub use history::{HistoryEvent, ReplayError, Replayed, replay};
pub use immediate::block_on;
