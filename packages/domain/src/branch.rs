//! La branche et ses cinq invariants — `docs/SPEC_V1.md` §7.1.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ids::RevisionId;

/// Les dix états d'une branche, dans l'ordre du texte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BranchState {
    /// Créée, rien n'y a encore été fait.
    Seed,
    /// En exploration.
    Exploring,
    /// Étayée par des résultats.
    Substantiated,
    /// Contestée : une objection est ouverte.
    Contested,
    /// Bloquée.
    Blocked,
    /// En cours de formalisation.
    Formalizing,
    /// Validée — sous conditions, voir [`Branch::validate`].
    Validated,
    /// Fusionnée. Terminal, sauf `reopen` explicite.
    Merged,
    /// Suspendue.
    Suspended,
    /// Archivée. **Ne supprime aucun objet.**
    Archived,
}

impl BranchState {
    /// Les dix états, dans l'ordre du texte.
    pub const ALL: [Self; 10] = [
        Self::Seed,
        Self::Exploring,
        Self::Substantiated,
        Self::Contested,
        Self::Blocked,
        Self::Formalizing,
        Self::Validated,
        Self::Merged,
        Self::Suspended,
        Self::Archived,
    ];

    /// La forme textuelle canonique.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Seed => "seed",
            Self::Exploring => "exploring",
            Self::Substantiated => "substantiated",
            Self::Contested => "contested",
            Self::Blocked => "blocked",
            Self::Formalizing => "formalizing",
            Self::Validated => "validated",
            Self::Merged => "merged",
            Self::Suspended => "suspended",
            Self::Archived => "archived",
        }
    }
}

impl fmt::Display for BranchState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// D'où vient une branche.
///
/// §7.1, deuxième invariant : « un fork référence **exactement** la révision d'origine ». Les deux
/// champs `forked_from_branch_id` et `fork_revision` du YAML ne se remplissent donc jamais l'un
/// sans l'autre : une branche qui saurait de quelle branche elle est issue sans savoir à quelle
/// révision aurait un point de départ qui bouge quand l'origine avance.
///
/// L'énumération rend ce couplage vrai par construction, plutôt que par un contrôle que quelqu'un
/// devra penser à écrire dans chaque constructeur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Origin {
    /// Une branche racine.
    Root,
    /// Un fork : la branche d'origine **et** la révision exacte.
    Fork {
        /// La branche dont celle-ci est issue.
        branch_id: String,
        /// La révision exacte au point de fork.
        revision: RevisionId,
    },
}

/// Ce qui atteste que les conditions de validation d'une branche sont satisfaites.
///
/// §7.1, quatrième invariant : « une branche ne peut être `validated` si ses conditions de
/// validation ne sont pas satisfaites ». Les conditions elles-mêmes viennent d'une politique
/// (`review_policy_id`) que ce crate ne connaît pas — et il n'a pas à la connaître : §8.2 en fait
/// le travail des packs disciplinaires.
///
/// Ce que le domaine peut garantir, c'est qu'on ne passe pas à `validated` **sans avoir répondu à
/// la question**. D'où ce témoin : il porte la liste des conditions et leur verdict, et
/// [`Branch::validate`] refuse dès qu'une seule n'est pas satisfaite. Une liste vide est refusée
/// aussi — « aucune condition » n'est pas « toutes satisfaites », c'est une politique qu'on n'a
/// pas lue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationWitness {
    /// La politique qui a produit ces conditions.
    pub policy_id: String,
    /// Les conditions, avec leur verdict.
    pub conditions: Vec<Condition>,
}

/// Une condition de validation et son verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Condition {
    /// Ce qui était demandé.
    pub statement: String,
    /// Satisfaite ou non. Pas d'`Option` : « je n'ai pas regardé » se dit `false`, et se corrige.
    pub satisfied: bool,
}

impl ValidationWitness {
    /// Vrai quand la politique a été lue et que toutes ses conditions sont satisfaites.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        !self.conditions.is_empty() && self.conditions.iter().all(|condition| condition.satisfied)
    }

    /// Les conditions non satisfaites, pour que le refus dise lesquelles.
    #[must_use]
    pub fn unmet(&self) -> Vec<&str> {
        self.conditions
            .iter()
            .filter(|condition| !condition.satisfied)
            .map(|condition| condition.statement.as_str())
            .collect()
    }
}

/// Une branche de travail — §7.1.
///
/// Les cinq invariants du texte, et où chacun vit :
///
/// 1. « une branche possède un seul head canonique » — `head_revision` est un [`RevisionId`], pas
///    une collection. Le type suffit.
/// 2. « un fork référence exactement la révision d'origine » — voir [`Origin`].
/// 3. « `merged` est terminal sauf opération explicite `reopen` » — [`Branch::transition`] refuse
///    toute sortie de `merged`, et [`Branch::reopen`] est la seule porte.
/// 4. « une branche ne peut être `validated` si ses conditions de validation ne sont pas
///    satisfaites » — voir [`ValidationWitness`].
/// 5. « `archived` ne supprime aucun objet » — il n'existe dans ce crate aucune fonction qui
///    supprime quoi que ce soit, et un test le vérifie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Branch {
    /// L'identité de la branche.
    pub id: String,
    /// Le workstream auquel elle appartient.
    pub workstream_id: String,
    /// Son titre.
    pub title: String,
    /// Son objectif.
    pub objective: String,
    /// D'où elle vient.
    pub origin: Origin,
    /// Le head canonique. **Un seul** — invariant 1, tenu par le type.
    pub head_revision: RevisionId,
    /// Son état.
    pub state: BranchState,
    /// Le rang de révision de la branche elle-même.
    pub revision: u32,
}

/// Le résultat d'une transition d'état.
pub type TransitionResult = Result<Branch, TransitionError>;

/// Pourquoi une transition est refusée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionError {
    /// `merged` est terminal : seul `reopen` en sort.
    MergedIsTerminal,
    /// `validated` demande un témoin complet.
    ValidationConditionsUnmet {
        /// Les conditions qui n'étaient pas satisfaites.
        unmet: Vec<String>,
    },
    /// `validated` demandé sans témoin du tout.
    ValidationWitnessMissing,
}

impl fmt::Display for TransitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MergedIsTerminal => formatter
                .write_str("`merged` est terminal : seule une opération `reopen` explicite en sort"),
            Self::ValidationConditionsUnmet { unmet } => write!(
                formatter,
                "conditions de validation non satisfaites : {}",
                unmet.join(" ; ")
            ),
            Self::ValidationWitnessMissing => formatter.write_str(
                "passage à `validated` sans témoin : « aucune condition » n'est pas « toutes satisfaites »",
            ),
        }
    }
}

impl std::error::Error for TransitionError {}

impl Branch {
    /// Changer d'état.
    ///
    /// Refuse deux choses, et seulement deux : sortir de `merged`, et atteindre `validated`. Le
    /// reste du graphe d'états n'est pas donné par §7.1 — le texte liste les états, pas les
    /// flèches — et l'inventer ici interdirait des transitions que personne n'a interdites.
    /// Ce qui est écrit est ce qui est vérifié ; le reste attend un document.
    ///
    /// # Errors
    ///
    /// [`TransitionError::MergedIsTerminal`] pour toute sortie de `merged`,
    /// [`TransitionError::ValidationWitnessMissing`] pour un passage à `validated` par cette voie —
    /// il faut [`Branch::validate`].
    pub fn transition(&self, next: BranchState) -> TransitionResult {
        if self.state == BranchState::Merged && next != BranchState::Merged {
            return Err(TransitionError::MergedIsTerminal);
        }
        if next == BranchState::Validated {
            return Err(TransitionError::ValidationWitnessMissing);
        }
        Ok(Self {
            state: next,
            revision: self.revision.saturating_add(1),
            ..self.clone()
        })
    }

    /// Passer à `validated`, témoin à l'appui — invariant 4.
    ///
    /// # Errors
    ///
    /// [`TransitionError::ValidationConditionsUnmet`] dès qu'une condition n'est pas satisfaite,
    /// ou que le témoin est vide. [`TransitionError::MergedIsTerminal`] depuis `merged`.
    pub fn validate(&self, witness: &ValidationWitness) -> TransitionResult {
        if self.state == BranchState::Merged {
            return Err(TransitionError::MergedIsTerminal);
        }
        if !witness.is_complete() {
            return Err(TransitionError::ValidationConditionsUnmet {
                unmet: witness
                    .unmet()
                    .into_iter()
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>(),
            });
        }
        Ok(Self {
            state: BranchState::Validated,
            revision: self.revision.saturating_add(1),
            ..self.clone()
        })
    }

    /// L'opération explicite `reopen` — la seule sortie de `merged`, invariant 3.
    ///
    /// Nommée plutôt que permise : une transition ordinaire depuis `merged` serait une réouverture
    /// qui ne dit pas son nom, et le texte demande qu'elle soit explicite.
    #[must_use]
    pub fn reopen(&self, into: BranchState) -> Self {
        Self {
            state: into,
            revision: self.revision.saturating_add(1),
            ..self.clone()
        }
    }

    /// La révision d'origine, quand la branche est un fork — invariant 2.
    #[must_use]
    pub const fn fork_revision(&self) -> Option<&RevisionId> {
        match &self.origin {
            Origin::Root => None,
            Origin::Fork { revision, .. } => Some(revision),
        }
    }
}
