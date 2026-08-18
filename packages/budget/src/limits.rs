//! Les bornes dures d'un compte — `docs/SPEC_V1.md` §7.2, invariant 6.
//!
//! « Les ressources sont réservées avant exécution ; elles ne sont pas supposées illimitées. »
//! L'invariant se tient ou ne se tient pas ici : un compte sans aucune borne est un compte dont
//! aucun dépassement n'est constatable, et le constater plus tard ne sert à rien.

use std::fmt;

use crate::dimension::Dimension;

/// Ce qu'un compte ne dépassera pas.
///
/// # Ce qu'on ne peut pas en construire
///
/// Un jeu de bornes vide. Il n'existe pas de `Limits::default()`, pas de `Limits::unlimited()` :
/// le seul constructeur exige au moins une dimension bornée, et une dimension non nommée n'est pas
/// « libre », elle est **hors budget** — rien ne peut y être réservé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    bounds: crate::dimension::Amounts,
}

impl Limits {
    /// Borner un compte.
    ///
    /// Une borne de zéro est licite et veut dire « rien n'est permis sur cette dimension » — c'est
    /// une décision de politique, pas une absence de décision, et elle refuse toute réservation.
    ///
    /// # Errors
    ///
    /// [`Unbounded`] quand aucune dimension n'est bornée.
    pub fn bounding(bounds: impl IntoIterator<Item = (Dimension, u64)>) -> Result<Self, Unbounded> {
        let bounds: crate::dimension::Amounts = bounds.into_iter().collect();
        if bounds.is_empty() {
            return Err(Unbounded);
        }
        Ok(Self { bounds })
    }

    /// La borne d'une dimension, si elle en a une.
    ///
    /// `None` ne veut pas dire « illimitée » : voir [`Limits::bounds`] — rien ne peut être réservé
    /// sur une dimension sans borne.
    #[must_use]
    pub fn ceiling(&self, dimension: Dimension) -> Option<u64> {
        self.bounds.get(&dimension).copied()
    }

    /// Vrai quand cette dimension est bornée.
    #[must_use]
    pub fn bounds(&self, dimension: Dimension) -> bool {
        self.bounds.contains_key(&dimension)
    }

    /// Les dimensions bornées.
    pub fn dimensions(&self) -> impl Iterator<Item = Dimension> + '_ {
        self.bounds.keys().copied()
    }
}

/// Un compte sans aucune borne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unbounded;

impl fmt::Display for Unbounded {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "un compte sans aucune borne rend tout dépassement inconstatable : l'invariant 6 \
             refuse de supposer les ressources illimitées",
        )
    }
}

impl std::error::Error for Unbounded {}
