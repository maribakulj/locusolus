//! Ce que l'institution voit des sous-agents internes du harnais — `W16.d`, tranche 4 du mineur
//! `lep/1.1` (ADR 0017 §5.4), tranché par l'ADR 0027 décision 7.
//!
//! # Le blocage que cet item lève, et ce qui l'a levé
//!
//! `W16.d` a attendu deux choses successives, et pas la même. D'abord **une décision** — « ce que
//! l'institution voit d'un sous-agent reste à trancher » —, que l'ADR 0027 décision 7 a prise :
//!
//! > L'institution voit **qu'un sous-agent a existé**, sa classe de cognition, son coût et son
//! > résultat. Elle ne voit son contexte et son raisonnement que par les décisions 1 à 5, comme pour
//! > n'importe quel agent.
//!
//! Puis **un lecteur**, que `W26.b` a livré. La ligne posait comme question — « voir qu'un
//! sous-agent existe et voir son contexte sont deux choses » — ce qui est devenu la réponse.
//!
//! # Facultative veut dire facultative
//!
//! Un harnais qui ne subdivise pas n'a **rien** à déclarer. [`seen`] rend donc [`Visibility::
//! NotDeclared`] quand le champ est absent, et ce n'est pas la même chose qu'un harnais qui
//! subdivise et n'a produit aucun sous-agent — celui-là déclare une liste vide.
//!
//! Confondre les deux est la faute que l'ADR 0017 décision 6 nomme sous un autre nom : « un `role`
//! qui vaudrait `research` faute de mieux rendrait *l'institution n'a pas dit* indiscernable de
//! *l'institution a dit `research`*, et c'est le second qui se croit tenu. » Ici, « ce harnais ne
//! sait pas subdiviser » et « ce harnais n'a pas subdivisé cette fois » appellent des questions
//! différentes, et une seule des deux se pose à l'exploitant.
//!
//! # Quatre choses, et pas une cinquième
//!
//! Existence, classe de cognition, coût, résultat. Le **contexte** et le **raisonnement** n'y sont
//! pas, et c'est la moitié de l'item : un sous-agent reviewer interne au harnais ne doit pas devenir
//! le chemin par lequel le raisonnement privé du générateur remonte, ce que l'invariant 11 interdit.
//!
//! Ce module n'expose donc **aucune** signature qui rende un contexte, un transcript ou un
//! raisonnement, et un test le tient par l'absence sur la source. La lecture du raisonnement d'un
//! sous-agent, quand elle est due, passe par `locus_memory::read` sous les trois classes de l'ADR
//! 0027 décision 2 — jamais par un chemin propre au harnais, qui serait exactement la quatrième
//! classe de lecteur que l'ADR refuse.

use locus_lep::{Attempt, AttemptSubagentsItem};

/// Ce qu'un sous-agent a rendu — le **résultat**, jamais le raisonnement qui y mène.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Abouti.
    Succeeded,
    /// Échoué.
    Failed,
    /// Interrompu avant terme.
    ///
    /// Distinct de `Failed`, et le schéma les sépare pour la même raison : confondre les deux ferait
    /// lire un budget épuisé comme une erreur de sous-agent, et chercher un défaut là où il n'y a
    /// qu'une borne.
    Cancelled,
}

impl Outcome {
    /// Lire un résultat déclaré.
    ///
    /// Rend `None` sur une valeur inconnue plutôt qu'un défaut. C'est la règle du dépôt —
    /// `SandboxLevel::parse` la pose : « un niveau inconnu traité comme `S0` ouvrirait la sandbox,
    /// et traité comme `S5` masquerait une configuration fausse en la rendant inoffensive. » Ici,
    /// un résultat inconnu compté comme `Failed` inventerait un échec, et comme `Succeeded` un
    /// succès ; l'aveu s'appelle l'absence.
    #[must_use]
    pub fn parse(declared: &str) -> Option<Self> {
        match declared {
            "succeeded" => Some(Self::Succeeded),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

/// Ce qu'un sous-agent a coûté, quand le harnais le mesure.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Cost {
    /// Combien d'appels.
    pub calls: Option<i64>,
    /// Combien de jetons.
    pub tokens: Option<i64>,
    /// Combien de temps.
    pub wall_time_seconds: Option<f64>,
}

/// Un sous-agent, tel que l'institution le voit.
#[derive(Debug, Clone, PartialEq)]
pub struct Subagent {
    /// Sa désignation dans le harnais.
    pub name: String,
    /// Sa classe de cognition — une **classe**, jamais un identifiant de modèle (`W25.a`).
    pub cognition: String,
    /// Ce qu'il a rendu, quand le mot déclaré est l'un des trois.
    pub outcome: Option<Outcome>,
    /// Ce qu'il a coûté, quand c'est mesuré.
    pub cost: Cost,
}

/// Ce que l'institution obtient d'un attempt.
///
/// Deux absences distinctes, et elles ne se confondent pas — voir l'en-tête du module.
#[derive(Debug, Clone, PartialEq)]
pub enum Visibility {
    /// Le harnais ne déclare pas ses sous-agents : la feature n'est pas négociée, ou il ne
    /// subdivise pas. **Rien n'est su**, et ce n'est pas « aucun sous-agent ».
    NotDeclared,
    /// Le harnais déclare — y compris une liste vide, qui dit « j'ai regardé, il n'y en a pas ».
    Declared(Vec<Subagent>),
}

impl Visibility {
    /// Combien de sous-agents sont **déclarés**.
    ///
    /// `None` quand rien n'est déclaré. Un `0` y serait le compteur qui n'a rien lu, et la règle 3
    /// du rythme de session vaut ici : « la réponse est zéro » et « il n'y a pas eu de réponse » ne
    /// se rendent pas par la même valeur.
    #[must_use]
    pub fn count(&self) -> Option<usize> {
        match self {
            Self::NotDeclared => None,
            Self::Declared(subagents) => Some(subagents.len()),
        }
    }
}

/// Lire ce qu'un attempt déclare de ses sous-agents.
#[must_use]
pub fn seen(attempt: &Attempt) -> Visibility {
    match attempt.subagents.as_ref() {
        None => Visibility::NotDeclared,
        Some(declared) => Visibility::Declared(declared.iter().map(read_one).collect::<Vec<_>>()),
    }
}

/// Un sous-agent déclaré, lu.
fn read_one(item: &AttemptSubagentsItem) -> Subagent {
    Subagent {
        name: item.name.clone(),
        cognition: item.cognition.clone(),
        outcome: Outcome::parse(&item.outcome),
        cost: item.cost.as_ref().map_or_else(Cost::default, |cost| Cost {
            calls: cost.calls,
            tokens: cost.tokens,
            wall_time_seconds: cost.wall_time_seconds,
        }),
    }
}
