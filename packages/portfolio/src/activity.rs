//! Ce qu'une branche a produit — la matière que le criblage de §13.6 examine.
//!
//! # Pourquoi ce type existe séparément
//!
//! Les détecteurs de §13.6 ne lisent pas le graphe : ils lisent un **relevé**. La distinction n'est
//! pas cosmétique. Un détecteur branché sur le graphe deviendrait dépendant de la forme du graphe,
//! et le criblage cesserait d'être rejouable dès qu'une projection change. Ici, le même relevé donne
//! toujours le même verdict.

use locus_protocol::{Id, id::Agent};

/// Une revendication produite par la branche.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimRecord {
    /// Son énoncé, tel qu'écrit.
    pub statement: String,
    /// Combien de preuves la soutiennent.
    pub evidence_count: usize,
    /// La confiance déclarée, en centièmes — 0 à 100.
    pub declared_confidence: u8,
    /// Vrai quand la revendication a été vérifiée par la suite.
    pub held_up: Option<bool>,
}

/// Une revue rendue, vue du portefeuille.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewRecord {
    /// Qui a relu.
    pub reviewer: Id<Agent>,
    /// Qui avait produit le travail relu.
    pub author: Id<Agent>,
    /// Vrai quand la revue valide.
    pub approves: bool,
}

/// Un artefact déclaré.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactRecord {
    /// Ce dont il fait partie — deux artefacts du même ensemble logique partagent cette clé.
    pub logical_unit: String,
    /// Sa taille en octets.
    pub size_bytes: u64,
}

/// Le relevé d'activité d'une branche.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BranchActivity {
    /// Les revendications produites.
    pub claims: Vec<ClaimRecord>,
    /// Les revues rendues à l'intérieur de la branche.
    pub reviews: Vec<ReviewRecord>,
    /// Les artefacts déclarés.
    pub artifacts: Vec<ArtifactRecord>,
    /// Les tâches créées.
    pub tasks_created: usize,
    /// Celles dont le résultat a été accepté.
    pub tasks_accepted: usize,
    /// Les métriques pré-enregistrées, avant de voir les résultats.
    pub preregistered_metrics: Vec<String>,
    /// Celles effectivement rapportées.
    pub reported_metrics: Vec<String>,
}
