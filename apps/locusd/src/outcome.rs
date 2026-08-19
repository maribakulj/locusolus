//! Ce qu'une commande rend, et pourquoi un refus ne peut pas ressembler à un succès.
//!
//! # La propriété, et ce qu'elle interdit
//!
//! `W20.a` demande qu'« aucune variante ne permette à un refus de ressembler à un succès ». Trois
//! façons de la perdre, écartées ici par construction :
//!
//! 1. **Un succès par défaut.** [`Outcome`] n'implémente pas `Default`, et ne le peut pas : un
//!    `Outcome::default()` serait un succès obtenu sans commande.
//! 2. **Un succès dégradé.** Il n'existe pas de variante « accepté avec réserves », qu'un appelant
//!    lirait comme un accord en ignorant les réserves. Ce qui n'est pas accepté est refusé, et le
//!    refus porte sa famille.
//! 3. **Un accès qui ment.** [`Outcome::accepted`] rend `Option`, jamais un `bool` doublé d'un
//!    accesseur qui `panic!`erait — un appelant qui interroge un refus reçoit `None` et non une
//!    révision inventée.
//!
//! `#[must_use]` sur le type ferme la dernière : un `Outcome` ignoré est un refus non lu.

use serde::{Deserialize, Serialize};

use crate::error::{CommandError, Revision};

/// Ce qu'une commande acceptée a produit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Accepted {
    /// La révision de la ressource **après** la commande.
    ///
    /// Ce que le client passera en `expected_revision` à la commande suivante. La rendre ici lui
    /// évite une relecture qui n'apprendrait rien de plus.
    pub revision: Revision,
}

/// Le verdict d'une commande — accepté, ou refusé sous une famille de §22.5.
///
/// Deux variantes et pas trois : il n'y a pas d'état intermédiaire qu'un appelant pourrait prendre
/// pour un accord. Une commande longue rend `Accepted` avec la révision qu'elle a produite ; une
/// commande qui n'a pas abouti rend `Refused`, et la raison est lisible.
#[must_use = "un verdict ignoré est un refus non lu"]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    /// La commande a été appliquée.
    Accepted(Accepted),
    /// La commande a été refusée.
    Refused(CommandError),
}

impl Outcome {
    /// Ce que la commande a produit, si elle a été acceptée.
    ///
    /// `Option` plutôt qu'un accesseur qui échouerait : un appelant qui interroge un refus reçoit
    /// une absence, pas une révision inventée ni une panique.
    #[must_use]
    pub const fn accepted(&self) -> Option<&Accepted> {
        match self {
            Self::Accepted(accepted) => Some(accepted),
            Self::Refused(_) => None,
        }
    }

    /// Le refus, s'il y en a un.
    #[must_use]
    pub const fn refused(&self) -> Option<&CommandError> {
        match self {
            Self::Accepted(_) => None,
            Self::Refused(error) => Some(error),
        }
    }
}
