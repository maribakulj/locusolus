//! `agent_lifetime` — combien de temps une instance reste en place. `W21.j`, ADR 0024.
//!
//! # Une instance encore en place n'a pas de durée
//!
//! C'est la décision de cet item, et elle se prend une fois pour éviter un défaut qui ne se voit
//! jamais : une durée arrêtée à l'instant de lecture **change à chaque lecture**. Deux rapports
//! produits à dix minutes d'intervalle donneraient deux valeurs pour le même passé, et le second
//! aurait l'air d'un fait nouveau. Ce n'est pas un fait du journal — c'est un fait de la montre de
//! celui qui lit.
//!
//! [`Lifetime::Standing`] dit donc « encore en place », et ne porte aucune durée. La question
//! « depuis combien de temps » a une réponse, mais elle appartient à l'appelant qui connaît son
//! propre instant, et elle ne se range pas à côté de durées closes sans qu'on sache laquelle a fini
//! de bouger. C'est la même règle que la cohorte ouverte de `W21.e`.
//!
//! # Ce module ne lit aucune horloge, et c'est ce qui rend la règle tenable
//!
//! Une règle qui dépendrait de la discipline d'appel tomberait au premier appelant pressé. Ici, la
//! seule façon d'obtenir un instant est de le **recevoir** : rien n'importe `std::time`, et un test
//! d'absence le tient sur les signatures.
//!
//! Conséquence directe : `Standing` ne peut pas être converti en durée, même par erreur. Il n'y a
//! pas d'instant courant à soustraire.
//!
//! # Ce que la mesure ne dit pas
//!
//! Rien de ce que l'instance a **accompli**. Une instance qui a tenu longtemps peut n'avoir rien
//! produit, et une instance courte peut avoir tout fait ; lire l'une pour l'autre est la faute que
//! le mot « lifetime » invite naturellement. Le module n'a donc aucun chemin vers un résultat, une
//! tâche ou un artefact, et un test d'absence le refuse.
//!
//! L'état de sortie, lui, est porté : terminer, échouer et être arrêtée sont trois façons de partir,
//! et les fondre ferait lire une flotte qu'on tue sans arrêt comme une flotte qui finit son travail.

use std::fmt;

use locus_protocol::Timestamp;

use crate::agent::InstanceState;

/// Ce qu'on sait du séjour d'une instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    entered: Timestamp,
    left: Option<(Timestamp, InstanceState)>,
}

impl Span {
    /// Une instance entrée et **toujours en place**.
    #[must_use]
    pub const fn standing(entered: Timestamp) -> Self {
        Self {
            entered,
            left: None,
        }
    }

    /// Une instance entrée puis sortie, dans l'état où elle est partie.
    ///
    /// # Errors
    ///
    /// [`LifetimeError::LeftBeforeEntering`] quand la sortie précède l'entrée — un ordre que le
    /// journal ne produit pas, et dont l'acceptation silencieuse rendrait une durée négative ou,
    /// pire, une durée absolue qui aurait l'air juste.
    ///
    /// [`LifetimeError::NotAnExit`] quand l'état déclaré n'est pas terminal : `provisioned`,
    /// `active` et `waiting` décrivent une instance qui est encore là, et les accepter comme sortie
    /// ferait clore un séjour qui continue.
    pub fn left(
        entered: Timestamp,
        left: Timestamp,
        ended_as: InstanceState,
    ) -> Result<Self, LifetimeError> {
        if left.millis() < entered.millis() {
            return Err(LifetimeError::LeftBeforeEntering { entered, left });
        }
        if !is_an_exit(ended_as) {
            return Err(LifetimeError::NotAnExit { state: ended_as });
        }
        Ok(Self {
            entered,
            left: Some((left, ended_as)),
        })
    }

    /// Quand l'instance est entrée.
    #[must_use]
    pub const fn entered(self) -> Timestamp {
        self.entered
    }

    /// La durée du séjour — `agent_lifetime` proprement dit.
    #[must_use]
    pub const fn lifetime(self) -> Lifetime {
        match self.left {
            Some((left, ended_as)) => Lifetime::Closed {
                millis: left.millis().saturating_sub(self.entered.millis()),
                ended_as,
            },
            None => Lifetime::Standing,
        }
    }
}

/// Vrai pour les trois états qui font partir une instance.
///
/// `provisioned`, `active` et `waiting` décrivent une instance encore là. La liste est **dérivée**
/// de `InstanceState::ALL` par exhaustivité du `match` : une septième valeur ferait échouer la
/// compilation plutôt que d'être silencieusement rangée du mauvais côté.
const fn is_an_exit(state: InstanceState) -> bool {
    match state {
        InstanceState::Completed | InstanceState::Failed | InstanceState::Terminated => true,
        InstanceState::Provisioned | InstanceState::Active | InstanceState::Waiting => false,
    }
}

/// Une durée de séjour, ou l'absence de durée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifetime {
    /// Le séjour est fini, et on dit **comment** il s'est terminé.
    Closed {
        /// Sa durée, en millisecondes.
        millis: i64,
        /// Dans quel état l'instance est partie.
        ended_as: InstanceState,
    },
    /// L'instance est encore en place. **Aucune durée** : voir la documentation du module.
    Standing,
}

impl Lifetime {
    /// La durée, si le séjour est fini.
    #[must_use]
    pub const fn millis(self) -> Option<i64> {
        match self {
            Self::Closed { millis, .. } => Some(millis),
            Self::Standing => None,
        }
    }

    /// Comment le séjour s'est terminé, s'il l'est.
    #[must_use]
    pub const fn ended_as(self) -> Option<InstanceState> {
        match self {
            Self::Closed { ended_as, .. } => Some(ended_as),
            Self::Standing => None,
        }
    }
}

impl fmt::Display for Lifetime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed { millis, ended_as } => write!(formatter, "{millis} ms, {ended_as}"),
            Self::Standing => {
                formatter.write_str("encore en place — pas de durée tant qu'elle n'est pas partie")
            }
        }
    }
}

/// Pourquoi un séjour est refusé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifetimeError {
    /// La sortie précède l'entrée.
    LeftBeforeEntering {
        /// Quand l'instance est entrée.
        entered: Timestamp,
        /// Quand elle prétend être sortie.
        left: Timestamp,
    },
    /// L'état déclaré n'est pas une sortie.
    NotAnExit {
        /// Lequel.
        state: InstanceState,
    },
}

impl fmt::Display for LifetimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LeftBeforeEntering { entered, left } => write!(
                formatter,
                "une sortie en {} précède son entrée en {} : le journal ne produit pas cet ordre, et \
                 l'accepter rendrait une durée qui aurait l'air juste",
                left.millis(),
                entered.millis()
            ),
            Self::NotAnExit { state } => write!(
                formatter,
                "« {state} » décrit une instance encore là : l'accepter comme sortie clôrait un \
                 séjour qui continue"
            ),
        }
    }
}

impl std::error::Error for LifetimeError {}
