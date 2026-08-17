//! Le journal canonique de Locus Solus — `docs/SPEC_V1.md` §10.
//!
//! W1.c livre l'enveloppe normative de §10.1, le **port** du journal, une implémentation de
//! référence en mémoire, et la suite de contract tests qui définit ce que « journal » veut dire
//! ici.
//!
//! # Pourquoi aucun `PostgreSQL` dans ce commit
//!
//! `CLAUDE.md` : « construire domain/protocol/event-store d'abord, avec des **ports purs**. Ne
//! brancher Temporal, containers ou cloud qu'après les interfaces et les contract tests. » Le
//! driver arrive ensuite et devra passer la même suite que l'implémentation en mémoire — c'est
//! elle qui décidera s'il est conforme, pas sa documentation.
//!
//! La règle 3 de `boundaries.json` autorise un client `PostgreSQL` **sous ce package** ; elle ne
//! demande pas qu'il y en ait un tout de suite.
//!
//! # Les quatre garanties que ce paquet porte aujourd'hui
//!
//! 1. **Ordre total par stream.** `stream_revision` est attribué par le journal, jamais par le
//!    producteur : c'est pourquoi [`envelope::Draft`] existe à côté de [`envelope::Envelope`].
//! 2. **Concurrence optimiste par `expected_stream_revision`.** [`store::Expected`] n'a pas de
//!    variante « peu importe » — un écrivain qui ne sait pas sur quelle révision il construit n'a
//!    rien vérifié.
//! 3. **Idempotence par commande.** Un rejeu rend le résultat d'origine plutôt qu'une erreur ; une
//!    réutilisation d'identifiant avec un contenu différent est refusée.
//! 4. **Immutabilité logique.** Aucune fonction n'écrase ni ne supprime un événement écrit, et un
//!    test le vérifie par l'absence.
//!
//! Les trois que §10.2 nomme et qui appartiennent aux items suivants : signature de fédération,
//! upcasters de migration (W1.h), snapshots reconstruisibles (W1.d).

pub mod envelope;
pub mod memory;
pub mod store;

pub use envelope::{
    Actor, ActorKind, Draft, EVENT_NAMESPACES, Envelope, EventId, EventType, ParseEventTypeError,
    StreamId,
};
pub use memory::MemoryEventStore;
pub use store::{Append, AppendError, Appended, EventStore, Expected, Sequenced};
