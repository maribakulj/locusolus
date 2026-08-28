//! `locusd` — le daemon Locus Solus, côté domaine.
//!
//! # Ce que ce crate est aujourd'hui, et ce qu'il n'est pas
//!
//! `SPEC_V1.md` §22 décrit une API : des commandes, des queries, des événements clients. Ce crate
//! en porte la forme d'une commande (`W20.a`), la forme d'un refus (`W20.a`), le handler
//! transactionnel (`W20.b`) et le composition root (`W20.d`) — et **rien du transport**. Pas de
//! runtime asynchrone, pas de cadre HTTP, pas de route.
//!
//! Ce n'est pas un découpage arbitraire : `CLAUDE.md` impose de « construire domain/protocol/
//! event-store d'abord, avec des ports purs », et le choix d'un runtime asynchrone et d'un cadre
//! HTTP est le plus gros choix de dépendance depuis l'ADR 0011. Il a eu son ADR — **0018**, écrit
//! en `W20.c`, une fois connu ce qu'il devait transporter — et il mesure ce qu'il coûte plutôt que
//! de l'estimer.
//!
//! # La règle 4, qui gardait le vide
//!
//! `boundaries.json` porte « `apps/locusd` n'importe aucun SDK de runtime de containers », et
//! `check:boundaries` l'imprimait avant `W20.a` comme « vérifiée sur **0 fichier(s)** » — elle
//! gardait un répertoire qui n'existait pas, et l'outil le disait plutôt que de compter ça pour un
//! succès. Ce crate la rend non vide, et `W20.d` a vérifié qu'elle **échoue** sur un import de
//! `bollard` ajouté au vrai crate, pas seulement sur une fixture. C'est la séparation qu'ADR 0004 pose : `locusd` ne détient jamais de
//! socket de runtime, `locus-execd` le fait, et la tentation de parler à Podman « juste pour le
//! profil local » est exactement ce que la règle empêche.
//!
//! # Les dépendances, et leur absence
//!
//! Six crates du workspace — `locus-protocol` pour les identifiants, `locus-event-store` pour le
//! journal que la transaction possède, `locus-projections` et `locus-policy` pour ce que le
//! composition root câble, `locus-coordination` et `locus-domain` pour les six capacités de branche
//! de `W17.f`.
//!
//! Quatre crates externes, et **pas une de plus que ce que `dependencies.json` autorise** :
//! `serde` et `serde_json` pour la forme sur le fil, `tokio` et `axum` pour le transport, entrés
//! avec `W20.g` sous l'ADR 0018. `check:deps` refuse le reste, et refuse aussi `tokio/full`.
//!
//! Ce que ce crate n'a **pas** : de quoi fabriquer un identifiant. Cela demanderait de l'entropie,
//! donc un crate, donc un ADR — voir [`branch::BranchContext`], qui préfère nommer la lacune plutôt
//! que la combler en passant.

pub mod administration;
pub mod artifacts;
pub mod assignment;
pub mod bootstrap;
pub mod branch;
pub mod broker;
pub mod command;
pub mod composition;
pub mod context_view;
pub mod cursor;
pub mod enrollment;
pub mod epistemic;
pub mod error;
pub mod handler;
pub mod http;
pub mod identities;
pub mod journal;
pub mod lep;
pub mod lifecycle;
pub mod messaging;
pub mod mission;
pub mod observations;
pub mod offload;
pub mod organisation;
pub mod outcome;
pub mod query;
pub mod stream;
pub mod subagents;
pub mod transaction;
pub mod writes;

pub use assignment::Assign;
pub use branch::{Approve, BranchContext, DiffView, HistoryEntry, Rollback};
pub use command::{CommandEnvelope, Draft};
pub use composition::{Readiness, Runtime, Wired};
pub use cursor::{Collection, Cursor, CursorError};
pub use epistemic::{Integrate, Stage, fact_type, fields, payload, staging};
pub use error::{CommandError, Conflict, EmptyResourceRef, Family, ResourceRef, Revision};
pub use handler::{Batch, Decide, IdempotencyScope, Submission};
pub use lep::{
    Claim, Complete, Desk, HEARTBEAT_INTERVAL_SECONDS, Identities, LEASE_TTL_SECONDS, LepContext,
    MemoryQueue, MemoryRegistry, MissionQueue, Offer, Queued, Rendered, Report, WorkerRegistry,
    expiration, stream_of_task,
};
pub use lifecycle::{Apply, event_type_of, stream_of_instance};
pub use messaging::{MessageContext, Send};
pub use offload::{Budget, MAX_BLOCKING, Offload, Offloaded, Permit};
pub use organisation::{
    Commit, Create, OrganisationContext, ReplayError, replay, resolve, resolve_at, stream_of,
};
pub use outcome::{Accepted, Outcome};
pub use query::{Page, TimelineEntry};
pub use stream::{ClientEvent, Delivery, Frame};
pub use transaction::Transaction;
