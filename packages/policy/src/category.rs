//! Les catégories de politique et le dry-run — `docs/SPEC_V1.md` §20.1 et §20.2.
//!
//! # Une liste close de seize
//!
//! §20.1 énumère seize catégories. Elle est close, et c'est ce qui permet de dire qu'un déploiement
//! n'a **aucune** politique de secrets — un fait qu'aucune liste ouverte ne saurait produire. Le
//! risque n'est pas d'écrire une mauvaise politique de secrets, c'est de n'y pas penser.
//!
//! # Le dry-run n'est pas une seconde évaluation
//!
//! §20.2 : « supporter dry-run et simulation ». La faute que cette exigence prévient est courante :
//! un chemin « simulation » écrit à part, qui diverge du chemin réel le jour où l'un des deux est
//! corrigé. La simulation ne dirait alors plus ce que fera le run, ce qui est la seule chose qu'on
//! lui demande.
//!
//! Ici, [`Run::dry`] et [`Run::live`] partagent **exactement** le même calcul. Ce qui change est ce
//! que l'appelant a le droit d'en faire : un dry-run rend une [`Simulation`] dont on ne peut tirer
//! aucun effet, parce qu'elle n'expose rien qui en produise. La garantie n'est pas dans une
//! discipline d'appel, elle est dans le type.

use std::fmt;

use crate::{Evaluation, Explanation, Facts, Outcome, Policy};

/// Les seize catégories de §20.1, dans l'ordre du texte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Category {
    /// Création d'agents.
    Spawn,
    /// Routage de modèle.
    ModelRouting,
    /// Coordination d'équipe.
    TeamCoordination,
    /// Partage d'information.
    InformationSharing,
    /// Budget.
    Budget,
    /// Ordonnancement.
    Scheduling,
    /// Sandbox et réseau.
    SandboxAndNetwork,
    /// Secrets.
    Secrets,
    /// Revue.
    Review,
    /// Validation.
    Validation,
    /// Branche et terminaison.
    BranchAndTermination,
    /// Publication.
    Publication,
    /// Rétention.
    Retention,
    /// Fédération.
    Federation,
    /// Conformité disciplinaire.
    DisciplinaryCompliance,
    /// Escalade humaine.
    HumanEscalation,
}

impl Category {
    /// Les seize, dans l'ordre où §20.1 les énumère.
    pub const ALL: [Self; 16] = [
        Self::Spawn,
        Self::ModelRouting,
        Self::TeamCoordination,
        Self::InformationSharing,
        Self::Budget,
        Self::Scheduling,
        Self::SandboxAndNetwork,
        Self::Secrets,
        Self::Review,
        Self::Validation,
        Self::BranchAndTermination,
        Self::Publication,
        Self::Retention,
        Self::Federation,
        Self::DisciplinaryCompliance,
        Self::HumanEscalation,
    ];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Spawn => "spawn",
            Self::ModelRouting => "model-routing",
            Self::TeamCoordination => "team-coordination",
            Self::InformationSharing => "information-sharing",
            Self::Budget => "budget",
            Self::Scheduling => "scheduling",
            Self::SandboxAndNetwork => "sandbox-and-network",
            Self::Secrets => "secrets",
            Self::Review => "review",
            Self::Validation => "validation",
            Self::BranchAndTermination => "branch-and-termination",
            Self::Publication => "publication",
            Self::Retention => "retention",
            Self::Federation => "federation",
            Self::DisciplinaryCompliance => "disciplinary-compliance",
            Self::HumanEscalation => "human-escalation",
        }
    }

    /// La relire.
    #[must_use]
    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.slug() == slug)
    }
}

impl fmt::Display for Category {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Ce qu'un déploiement couvre — et surtout ce qu'il ne couvre pas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Coverage {
    uncovered: Vec<Category>,
}

impl Coverage {
    /// Confronter les catégories couvertes aux seize de §20.1.
    ///
    /// Le résultat qui compte est la liste des **absentes** : le risque n'est pas d'écrire une
    /// mauvaise politique de secrets, c'est de n'y pas penser.
    #[must_use]
    pub fn of(covered: &[Category]) -> Self {
        Self {
            uncovered: Category::ALL
                .into_iter()
                .filter(|category| !covered.contains(category))
                .collect(),
        }
    }

    /// Les catégories que rien ne couvre.
    #[must_use]
    pub fn uncovered(&self) -> &[Category] {
        &self.uncovered
    }

    /// Vrai quand les seize sont couvertes.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.uncovered.is_empty()
    }
}

impl fmt::Display for Coverage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.uncovered.is_empty() {
            return formatter.write_str("les seize catégories de §20.1 sont couvertes");
        }
        formatter.write_str("aucune politique pour :")?;
        for category in &self.uncovered {
            write!(formatter, " {category}")?;
        }
        Ok(())
    }
}

/// Une évaluation menée sans droit d'agir.
///
/// Elle porte exactement ce qu'un run réel aurait produit — même décision, même trace — et n'expose
/// **rien** qui produise un effet. Ce n'est pas une discipline d'appel : il n'y a pas de méthode à
/// ne pas appeler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Simulation {
    evaluation: Evaluation,
    explanation: Explanation,
}

impl Simulation {
    /// Ce qui aurait été décidé.
    #[must_use]
    pub const fn would_decide(&self) -> &Outcome {
        self.evaluation.outcome()
    }

    /// L'évaluation, trace comprise.
    #[must_use]
    pub const fn evaluation(&self) -> &Evaluation {
        &self.evaluation
    }

    /// L'exposé qu'un run réel aurait produit.
    #[must_use]
    pub const fn explanation(&self) -> &Explanation {
        &self.explanation
    }
}

/// Ce qu'une évaluation réelle produit — décision, exposé, et le droit d'en tirer des événements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    evaluation: Evaluation,
    explanation: Explanation,
}

impl Applied {
    /// Ce qui a été décidé.
    #[must_use]
    pub const fn outcome(&self) -> &Outcome {
        self.evaluation.outcome()
    }

    /// L'évaluation, trace comprise.
    #[must_use]
    pub const fn evaluation(&self) -> &Evaluation {
        &self.evaluation
    }

    /// L'exposé de §20.5, auquel l'appelant rattachera les événements produits.
    #[must_use]
    pub fn explanation(self) -> Explanation {
        self.explanation
    }
}

/// Évaluer une politique, en simulation ou pour de bon.
///
/// Les deux chemins partagent le **même** calcul. Un chemin « simulation » écrit à part divergerait
/// du chemin réel le jour où l'un des deux est corrigé, et la simulation ne dirait plus ce que fera
/// le run — la seule chose qu'on lui demande.
pub struct Run;

impl Run {
    /// Simuler : décider sans droit d'agir.
    #[must_use]
    pub fn dry(policy: &Policy, facts: &Facts) -> Simulation {
        let (evaluation, explanation) = decide(policy, facts);
        Simulation {
            evaluation,
            explanation,
        }
    }

    /// Décider pour de bon.
    #[must_use]
    pub fn live(policy: &Policy, facts: &Facts) -> Applied {
        let (evaluation, explanation) = decide(policy, facts);
        Applied {
            evaluation,
            explanation,
        }
    }
}

fn decide(policy: &Policy, facts: &Facts) -> (Evaluation, Explanation) {
    let evaluation = policy.evaluate(facts);
    let explanation = Explanation::of(facts, &evaluation);
    (evaluation, explanation)
}
