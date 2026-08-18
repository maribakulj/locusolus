//! Les barrières par **invariant menacé** — `docs/13` §3.
//!
//! # La phrase, et ce qu'elle rejette
//!
//! « Barrières par invariant menacé plutôt que par lieu. »
//!
//! Une barrière ordinaire gèle un endroit : « personne ne touche à cette équipe pendant que je la
//! recompose ». C'est simple, et c'est faux dans les deux sens. Elle bloque trop — deux
//! reconfigurations au même endroit qui ne peuvent rien se casser attendent l'une l'autre sans
//! raison. Et elle bloque trop peu — deux reconfigurations à des endroits différents qui menacent
//! le même invariant passent toutes les deux, et l'invariant tombe entre elles.
//!
//! Une barrière par invariant se trompe dans aucun des deux sens, parce qu'elle nomme ce qui est
//! réellement en jeu.
//!
//! # La portée est dérivée, jamais déclarée
//!
//! C'est ce qui empêche la barrière par lieu de revenir déguisée. [`Barriers::raise`] **calcule** ce
//! qu'un diff menace, par [`crate::region::threatens`] — le même calcul que le plafond de risque de
//! W15.c, donc les deux ne peuvent pas diverger. Il n'existe aucun moyen de déclarer une portée, et
//! [`Barrier`] n'expose aucun nœud : un accesseur qui rendrait des identités ferait écrire, un jour,
//! « barrer aussi ceux-là ».
//!
//! # Une barrière sans invariant menacé est refusée
//!
//! Elle ne pourrait barrer que par lieu, faute d'autre chose à nommer — donc elle serait exactement
//! ce que `docs/13` écarte. Et un diff qui ne menace rien n'a besoin d'aucune barrière : le lui
//! refuser ne coûte rien et empêche la mauvaise habitude.

use std::collections::BTreeSet;
use std::fmt;

use crate::diff::Diff;
use crate::region::{Invariant, threatens};

/// Ce qu'un diff met en jeu.
///
/// Réutilise le calcul de W15.c plutôt que d'en écrire un second : une barrière qui protégerait
/// d'autres invariants que ceux dont la région tient le plafond ferait deux vérités sur la même
/// question.
#[must_use]
pub fn threatened_by(diff: &Diff) -> BTreeSet<Invariant> {
    diff.operations().iter().flat_map(threatens).collect()
}

/// Une barrière tenue sur un invariant.
///
/// Elle ne porte **aucun nœud**, et c'est le point : une barrière qui saurait nommer un lieu
/// finirait par en barrer un.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Barrier {
    invariant: Invariant,
    held_by: String,
}

impl Barrier {
    /// L'invariant protégé.
    #[must_use]
    pub const fn invariant(&self) -> Invariant {
        self.invariant
    }

    /// Qui la tient.
    ///
    /// Une barrière anonyme ne se relâche pas : personne ne sait à qui demander.
    #[must_use]
    pub fn held_by(&self) -> &str {
        &self.held_by
    }
}

/// Ce qu'une reconfiguration rencontre en se présentant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Passage {
    /// Rien ne s'y oppose.
    Clear,
    /// Une barrière tenue protège un invariant que ce diff menace aussi.
    ///
    /// Le refus nomme **l'invariant**, jamais le lieu : « cette équipe est gelée » n'apprend rien,
    /// alors que « l'acyclicité de revue est tenue par X » dit ce qu'il faut attendre et pourquoi.
    Held {
        /// Lequel.
        invariant: Invariant,
        /// Par qui.
        by: String,
    },
}

impl Passage {
    /// Vrai quand rien ne s'y oppose.
    #[must_use]
    pub const fn is_clear(&self) -> bool {
        matches!(self, Self::Clear)
    }
}

/// Les barrières tenues à un instant donné.
///
/// Au plus **une par invariant** : [`Barriers::raise`] refuse d'en poser une seconde sur un
/// invariant déjà tenu. Comme l'énumération n'en compte qu'un aujourd'hui, ce jeu n'en tient jamais
/// plus d'une — de sorte que relâcher une barrière et vider le jeu sont, pour l'instant,
/// indiscernables. La distinction deviendra observable le jour où un deuxième invariant entrera, et
/// c'est ce jour-là qu'un test devra la tenir.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Barriers {
    held: Vec<Barrier>,
}

impl Barriers {
    /// Aucune barrière tenue.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Celles qui sont tenues.
    #[must_use]
    pub fn held(&self) -> &[Barrier] {
        &self.held
    }

    /// Ce diff peut-il passer maintenant ?
    ///
    /// Il passe dès lors qu'aucune barrière tenue ne protège un invariant qu'il menace — quel que
    /// soit l'endroit où il opère. C'est là que la barrière par invariant se sépare de la barrière
    /// par lieu, dans les deux sens.
    #[must_use]
    pub fn admits(&self, diff: &Diff) -> Passage {
        let threatened = threatened_by(diff);
        self.held
            .iter()
            .find(|barrier| threatened.contains(&barrier.invariant))
            .map_or(Passage::Clear, |barrier| Passage::Held {
                invariant: barrier.invariant,
                by: barrier.held_by.clone(),
            })
    }

    /// Poser les barrières qu'un diff exige, et les rendre.
    ///
    /// # Errors
    ///
    /// [`BarrierError::NothingThreatened`] pour un diff qui ne met aucun invariant en jeu — la
    /// barrière ne pourrait alors barrer que par lieu, ce que `docs/13` écarte, et le diff n'en a de
    /// toute façon pas besoin ; [`BarrierError::EmptyHolder`] pour une barrière que personne ne
    /// tient, qui ne se relâcherait jamais ; [`BarrierError::AlreadyHeld`] quand un invariant est
    /// déjà tenu — poser une seconde barrière dessus ferait croire à deux protections là où la
    /// première suffit et où le second poseur devrait attendre.
    pub fn raise(&mut self, diff: &Diff, holder: &str) -> Result<Vec<Barrier>, BarrierError> {
        if holder.trim().is_empty() {
            return Err(BarrierError::EmptyHolder);
        }
        let threatened = threatened_by(diff);
        if threatened.is_empty() {
            return Err(BarrierError::NothingThreatened);
        }
        for invariant in &threatened {
            if let Some(barrier) = self
                .held
                .iter()
                .find(|barrier| barrier.invariant == *invariant)
            {
                return Err(BarrierError::AlreadyHeld {
                    invariant: *invariant,
                    by: barrier.held_by.clone(),
                });
            }
        }
        let raised: Vec<Barrier> = threatened
            .into_iter()
            .map(|invariant| Barrier {
                invariant,
                held_by: holder.to_owned(),
            })
            .collect();
        self.held.extend(raised.iter().cloned());
        Ok(raised)
    }

    /// Relâcher une barrière.
    ///
    /// # Errors
    ///
    /// [`BarrierError::NotHeld`] pour une barrière que ce jeu ne tient pas — relâcher dans le vide
    /// laisserait croire qu'on a rendu la main.
    pub fn release(&mut self, barrier: &Barrier) -> Result<(), BarrierError> {
        let before = self.held.len();
        self.held.retain(|held| held != barrier);
        if self.held.len() == before {
            return Err(BarrierError::NotHeld {
                invariant: barrier.invariant,
            });
        }
        Ok(())
    }
}

/// Ce qui empêche de poser ou de relâcher une barrière.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BarrierError {
    /// Un diff qui ne met aucun invariant en jeu.
    NothingThreatened,
    /// Une barrière que personne ne tient.
    EmptyHolder,
    /// Un invariant déjà protégé.
    AlreadyHeld {
        /// Lequel.
        invariant: Invariant,
        /// Par qui.
        by: String,
    },
    /// Une barrière qu'on relâche sans la tenir.
    NotHeld {
        /// Lequel.
        invariant: Invariant,
    },
}

impl fmt::Display for BarrierError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NothingThreatened => formatter.write_str(
                "ce lot ne menace aucun invariant : une barrière ne pourrait le protéger que par \
                 lieu, ce que `docs/13` écarte",
            ),
            Self::EmptyHolder => {
                formatter.write_str("une barrière que personne ne tient ne se relâche jamais")
            }
            Self::AlreadyHeld { invariant, by } => write!(
                formatter,
                "« {invariant} » est déjà tenu par {by} : attendre, plutôt qu'ajouter une seconde \
                 protection qui ne protège rien de plus"
            ),
            Self::NotHeld { invariant } => write!(
                formatter,
                "« {invariant} » n'est pas tenu ici : relâcher dans le vide laisserait croire qu'on \
                 a rendu la main"
            ),
        }
    }
}

impl std::error::Error for BarrierError {}
