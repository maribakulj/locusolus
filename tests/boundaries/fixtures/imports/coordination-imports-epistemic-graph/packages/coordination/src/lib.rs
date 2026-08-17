//! L'autre sens : la proposition qui voudrait « juste lire » le graphe pour vérifier sa citation.

use locus_graph::Graph;

pub struct Proposal {
    pub rationale: String,
}

pub fn cites(graph: &Graph) -> bool {
    let _ = graph;
    true
}
