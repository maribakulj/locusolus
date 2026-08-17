//! La force épistémique — `docs/SPEC_V1.md` §8.1.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Les sept niveaux de validation de §8.1.
///
/// # Pourquoi ce type n'est ni `Ord` ni `PartialOrd`
///
/// §8.1 : « ces niveaux **ne forment pas toujours une chaîne totale**. Une interprétation
/// historique peut atteindre L3 et L6 sans être “reproduite” au sens expérimental. »
///
/// Dériver `Ord` écrirait donc dans le type une affirmation que la spec dément. Et ce ne serait
/// pas un défaut théorique : `if level >= Reproduced` compilerait, se lirait bien, et refuserait
/// une interprétation historique parfaitement validée parce qu'elle n'a jamais été « reproduite »
/// — un test qu'aucune discipline non expérimentale ne peut passer.
///
/// Le numéro reste accessible par [`ValidationLevel::rank`], pour l'affichage et le tri d'une
/// liste. Il ne constitue pas une procédure de décision : §8.2 dit que ce sont **les packs
/// disciplinaires** qui définissent les chemins admissibles, et ce crate n'en connaît aucun.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationLevel {
    /// L0 — objet enregistré, non évalué.
    Unassessed,
    /// L1 — auteur, provenance et sources identifiables.
    Traceable,
    /// L2 — contrôles de cohérence et de forme passés.
    InternallyChecked,
    /// L3 — au moins une revue indépendante satisfaite.
    IndependentlyReviewed,
    /// L4 — résultat reproduit depuis les artefacts.
    Reproduced,
    /// L5 — obligation formelle vérifiée, lorsque applicable.
    FormallyVerified,
    /// L6 — critères du programme et approbations satisfaits.
    InstitutionallyAccepted,
}

impl ValidationLevel {
    /// Tous les niveaux, dans l'ordre du texte.
    pub const ALL: [Self; 7] = [
        Self::Unassessed,
        Self::Traceable,
        Self::InternallyChecked,
        Self::IndependentlyReviewed,
        Self::Reproduced,
        Self::FormallyVerified,
        Self::InstitutionallyAccepted,
    ];

    /// Le numéro du texte — `L0` à `L6`.
    ///
    /// **Étiquette, pas ordre.** Comparer deux rangs répond à « lequel porte le plus grand
    /// numéro », pas à « lequel est le mieux validé » : voir la note du type.
    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Self::Unassessed => 0,
            Self::Traceable => 1,
            Self::InternallyChecked => 2,
            Self::IndependentlyReviewed => 3,
            Self::Reproduced => 4,
            Self::FormallyVerified => 5,
            Self::InstitutionallyAccepted => 6,
        }
    }

    /// La forme textuelle canonique.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unassessed => "unassessed",
            Self::Traceable => "traceable",
            Self::InternallyChecked => "internally_checked",
            Self::IndependentlyReviewed => "independently_reviewed",
            Self::Reproduced => "reproduced",
            Self::FormallyVerified => "formally_verified",
            Self::InstitutionallyAccepted => "institutionally_accepted",
        }
    }
}

impl fmt::Display for ValidationLevel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
