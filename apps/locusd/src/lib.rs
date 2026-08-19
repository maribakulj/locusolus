//! `locusd` — le daemon Locus Solus, côté domaine.
//!
//! # Ce que ce crate est aujourd'hui, et ce qu'il n'est pas
//!
//! `SPEC_V1.md` §22 décrit une API : des commandes, des queries, des événements clients. Ce crate
//! en porte la **première moitié du contrat** — la forme d'une commande et la forme d'un refus — et
//! **rien du transport**. Pas de runtime asynchrone, pas de cadre HTTP, pas de route.
//!
//! Ce n'est pas un découpage arbitraire : `CLAUDE.md` impose de « construire domain/protocol/
//! event-store d'abord, avec des ports purs », et le choix d'un runtime asynchrone et d'un cadre
//! HTTP est le plus gros choix de dépendance depuis l'ADR 0011 — il a son propre ADR, qui est
//! `W20.c`. Commencer par le domaine permet à cet ADR d'être écrit en connaissant ce qu'il doit
//! transporter, plutôt que l'inverse.
//!
//! # La règle 4, qui gardait le vide
//!
//! `boundaries.json` porte « `apps/locusd` n'importe aucun SDK de runtime de containers », et
//! `check:boundaries` l'imprimait jusqu'ici comme « vérifiée sur **0 fichier(s)** » — elle gardait
//! un répertoire qui n'existait pas, et l'outil le disait plutôt que de compter ça pour un succès.
//! Ce crate la rend non vide. C'est la séparation qu'ADR 0004 pose : `locusd` ne détient jamais de
//! socket de runtime, `locus-execd` le fait, et la tentation de parler à Podman « juste pour le
//! profil local » est exactement ce que la règle empêche.
//!
//! # Les dépendances, et leur absence
//!
//! `locus-protocol` pour les identifiants, `serde` pour la forme sur le fil. Rien d'autre. Le jour
//! où une dépendance de transport entre, elle entrera par `W20.c` et son ADR, pas par commodité.

pub mod command;
pub mod error;
pub mod handler;
pub mod outcome;
pub mod transaction;

pub use command::{CommandEnvelope, Draft};
pub use error::{CommandError, Conflict, EmptyResourceRef, Family, ResourceRef, Revision};
pub use handler::{Batch, Decide, IdempotencyScope, Submission};
pub use outcome::{Accepted, Outcome};
pub use transaction::Transaction;
