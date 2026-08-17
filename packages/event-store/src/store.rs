//! Le port du journal — `docs/SPEC_V1.md` §10.2.

use std::fmt;

use locus_protocol::{Id, Timestamp, id::Command};

use crate::envelope::{Draft, Envelope, StreamId};

/// Ce sur quoi l'écrivain croit écrire — la concurrence optimiste de §10.2.
///
/// # Pourquoi il n'existe pas de variante « peu importe »
///
/// La plupart des journaux offrent un `Any` qui accepte l'append quel que soit l'état du stream.
/// Il n'y en a pas ici, et c'est le point : §10.2 dit « optimistic concurrency **par
/// `expected_stream_revision`** », et un écrivain qui ne sait pas sur quelle révision il construit
/// n'a rien vérifié. Ce qu'il produit n'est pas un append concurrent réussi, c'est un conflit
/// qu'on n'a pas regardé — et la mise à jour perdue qui va avec.
///
/// Le coût est réel : chaque écrivain doit lire avant d'écrire. C'est le prix de la garantie, et
/// il se paie une fois, à l'écriture, plutôt qu'indéfiniment en incohérences dont personne ne sait
/// d'où elles viennent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expected {
    /// Le stream n'existe pas encore. Le premier événement porte la révision 1.
    NoStream,
    /// Le stream est exactement à cette révision.
    Exact(u64),
}

impl fmt::Display for Expected {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoStream => formatter.write_str("stream inexistant"),
            Self::Exact(revision) => write!(formatter, "révision {revision}"),
        }
    }
}

/// Une écriture : un lot d'événements, une attente de révision, une commande.
///
/// Le lot est atomique — §9.2 exige « l'atomicité entre l'ajout des événements, la révision de
/// l'agrégat et l'outbox ». Un lot partiellement écrit laisserait un agrégat dont l'état ne
/// correspond à aucune décision prise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Append {
    /// Le stream visé.
    pub stream_id: StreamId,
    /// Ce sur quoi l'écrivain croit écrire.
    pub expected: Expected,
    /// La commande à l'origine du lot — c'est elle qui porte l'idempotence (§10.2).
    pub command_id: Id<Command>,
    /// Les événements, dans l'ordre.
    pub events: Vec<Draft>,
}

/// Ce qu'une écriture a produit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Appended {
    /// Les enveloppes scellées, avec leur rang.
    pub events: Vec<Envelope>,
    /// La révision du stream après l'écriture.
    pub revision: u64,
    /// Vrai quand la commande avait déjà été appliquée — §10.2, « idempotence par commande ».
    ///
    /// Le rejeu **rend le résultat d'origine** plutôt qu'une erreur : une commande réémise après
    /// une coupure réseau a déjà eu son effet, et le dire est plus utile que de faire échouer un
    /// appelant qui referait la même chose.
    pub replayed: bool,
}

/// Pourquoi une écriture est refusée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppendError {
    /// La révision attendue ne correspond pas — quelqu'un a écrit entre-temps.
    Conflict {
        /// Ce que l'écrivain attendait.
        expected: Expected,
        /// Ce que le stream porte réellement.
        actual: u64,
    },
    /// Un lot vide. Refusé : une commande sans effet n'a rien à journaliser, et l'écrire
    /// produirait une entrée dont aucune projection ne saurait quoi faire.
    EmptyBatch,
    /// Un événement du lot vise un autre stream que celui de l'écriture.
    StreamMismatch {
        /// Le stream de l'écriture.
        expected: StreamId,
        /// Celui que porte l'événement fautif.
        found: StreamId,
    },
    /// La même commande revient avec un contenu différent.
    ///
    /// Distinct du rejeu : deux lots différents sous un même identifiant de commande veulent dire
    /// que l'identifiant a été réutilisé, et l'accepter écrirait l'un des deux en croyant écrire
    /// l'autre.
    CommandReused {
        /// L'identifiant réutilisé.
        command_id: Id<Command>,
    },
}

impl fmt::Display for AppendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict { expected, actual } => write!(
                formatter,
                "conflit de concurrence : l'écrivain attendait {expected}, le stream est à {actual}"
            ),
            Self::EmptyBatch => formatter.write_str("lot vide : rien à journaliser"),
            Self::StreamMismatch { expected, found } => write!(
                formatter,
                "l'événement vise le stream `{found}`, l'écriture porte sur `{expected}`"
            ),
            Self::CommandReused { command_id } => write!(
                formatter,
                "commande `{command_id}` réutilisée avec un contenu différent"
            ),
        }
    }
}

impl std::error::Error for AppendError {}

/// Le port du journal canonique — §10.2.
///
/// # Pourquoi un trait, et pourquoi aucune implémentation `PostgreSQL` ici
///
/// `CLAUDE.md` : « construire domain/protocol/event-store d'abord, avec des **ports purs**. Ne
/// brancher Temporal, containers ou cloud qu'après les interfaces et les contract tests. » W1.c
/// livre donc le port, une implémentation de référence en mémoire, et la suite de contract tests
/// qui définit ce que « journal » veut dire ici. Le driver `PostgreSQL` viendra ensuite et devra
/// passer la même suite — c'est elle qui décidera s'il est conforme, pas sa documentation.
///
/// Les garanties de §10.2 que ce port porte : ordre total par stream, concurrence optimiste,
/// idempotence par commande, immutabilité logique. Celles qu'il ne porte pas encore, et qui
/// appartiennent aux items suivants : signature (fédération), upcasters de migration, snapshots.
pub trait EventStore {
    /// Écrire un lot.
    ///
    /// # Errors
    ///
    /// Voir [`AppendError`]. Un conflit n'est pas une panne : c'est le résultat normal d'une
    /// écriture concurrente, et l'appelant relit puis retente.
    fn append(&mut self, command: Append, recorded_at: Timestamp) -> Result<Appended, AppendError>;

    /// Relire un stream depuis une révision exclue.
    ///
    /// `from = 0` rend le stream entier. L'ordre est celui des révisions, et il est total : c'est
    /// la première garantie de §10.2, et c'est ce qui rend un replay reproductible.
    fn read_stream(&self, stream_id: &str, from: u64) -> Vec<Envelope>;

    /// La révision courante d'un stream, ou `None` s'il n'existe pas.
    ///
    /// `None` et non `0` : « ce stream n'existe pas » et « ce stream est vide » sont deux faits
    /// différents, et le second n'arrive jamais — un stream naît de son premier événement.
    fn revision(&self, stream_id: &str) -> Option<u64>;

    /// Tout le journal, dans l'ordre d'écriture — §10.2, « export brut ».
    ///
    /// L'ordre global est celui des écritures, pas celui des `occurred_at` : un worker hors ligne
    /// produit des actes anciens écrits tard, et trier par `occurred_at` ferait apparaître ses
    /// événements avant ceux qui les ont provoqués.
    fn export(&self) -> Vec<Envelope>;
}
