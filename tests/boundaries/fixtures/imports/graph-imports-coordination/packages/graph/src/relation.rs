//! Le chemin le plus court et le plus discret : une variante d'énumération.
//!
//! `// use locus_coordination::AgentInstance;` en commentaire ne doit pas compter.

use locus_coordination::AgentInstance;
use locus_domain::RevisionId;

pub enum RelationKind {
    Supports,
    /// Ce que la règle 6 refuse : un objet de coordination dans le crate épistémique.
    ReviewedBy { reviewer: AgentInstance },
}

pub fn subject(_revision: &RevisionId) -> Option<RelationKind> {
    None
}
