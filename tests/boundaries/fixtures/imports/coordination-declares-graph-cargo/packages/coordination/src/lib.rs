//! Aucun `use` interdit : la dépendance déclarée suffit, et c'est le moment où quelqu'un a décidé.

use locus_domain::RevisionId;

pub struct Proposal {
    pub cites: Vec<RevisionId>,
}
