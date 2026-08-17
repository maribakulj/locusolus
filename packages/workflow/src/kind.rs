//! Les workflows obligatoires — `docs/SPEC_V1.md` §11.2.

use std::fmt;

/// Le nombre de workflows que §11.2 rend obligatoires.
///
/// La constante existe pour que retirer un workflow de la liste soit une **erreur de compilation**
/// et non un tableau plus court que personne ne compte. §11.2 les énumère : la liste n'est pas une
/// suggestion, et un moteur qui n'en offrirait que dix serait conforme à rien.
pub const MANDATORY_WORKFLOWS: usize = 11;

/// L'un des onze workflows de §11.2.
///
/// # Pourquoi un enum fermé
///
/// Un `String` laisserait passer `TaskWorklow` et le ferait découvrir en production. Surtout, un
/// enum rend la liste **dénombrable** : le test de sortie de W3.a compare [`WorkflowKind::ALL`] aux
/// onze noms du texte, et une disparition silencieuse devient rouge.
///
/// Les workflows propres à un déploiement, s'il en apparaît, ne se glissent pas ici : ce type dit
/// ce que la spécification exige, pas ce qu'une installation ajoute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WorkflowKind {
    /// `ProgramWorkflow` — la campagne scientifique dans son ensemble.
    Program,
    /// `WorkstreamWorkflow` — un axe de travail à l'intérieur d'un programme.
    Workstream,
    /// `BranchWorkflow` — la vie d'une branche épistémique (§7.1).
    Branch,
    /// `TaskWorkflow` — une tâche confiée à un worker.
    Task,
    /// `ReviewWorkflow` — la revue indépendante (invariant 11).
    Review,
    /// `ReproductionWorkflow` — la reproduction d'un résultat.
    Reproduction,
    /// `MemoryCurationWorkflow` — la curation de la mémoire du laboratoire.
    MemoryCuration,
    /// `PortfolioWorkflow` — l'arbitrage entre campagnes.
    Portfolio,
    /// `EnvironmentBuildWorkflow` — la construction d'un environnement d'exécution.
    EnvironmentBuild,
    /// `SandboxLifecycleWorkflow` — le cycle de vie d'une sandbox (invariant 5).
    SandboxLifecycle,
    /// `FederationWorkflow` — les échanges avec une instance fédérée.
    Federation,
}

impl WorkflowKind {
    /// Les onze, dans l'ordre où §11.2 les énumère.
    pub const ALL: [Self; MANDATORY_WORKFLOWS] = [
        Self::Program,
        Self::Workstream,
        Self::Branch,
        Self::Task,
        Self::Review,
        Self::Reproduction,
        Self::MemoryCuration,
        Self::Portfolio,
        Self::EnvironmentBuild,
        Self::SandboxLifecycle,
        Self::Federation,
    ];

    /// Le nom du workflow, tel que §11.2 l'écrit.
    ///
    /// Ces noms sont ceux qui apparaîtront dans un historique de moteur durable : les changer plus
    /// tard renommerait des exécutions déjà enregistrées, ce qui n'est pas un renommage mais une
    /// perte de correspondance entre l'histoire et le code.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Program => "ProgramWorkflow",
            Self::Workstream => "WorkstreamWorkflow",
            Self::Branch => "BranchWorkflow",
            Self::Task => "TaskWorkflow",
            Self::Review => "ReviewWorkflow",
            Self::Reproduction => "ReproductionWorkflow",
            Self::MemoryCuration => "MemoryCurationWorkflow",
            Self::Portfolio => "PortfolioWorkflow",
            Self::EnvironmentBuild => "EnvironmentBuildWorkflow",
            Self::SandboxLifecycle => "SandboxLifecycleWorkflow",
            Self::Federation => "FederationWorkflow",
        }
    }

    /// Relire un nom.
    ///
    /// Rend `None` plutôt qu'une valeur par défaut : un historique qui porte un nom inconnu doit
    /// être traité comme inconnu, pas rangé sous le workflow le plus proche.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }
}

impl fmt::Display for WorkflowKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}
