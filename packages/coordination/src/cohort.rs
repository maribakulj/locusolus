//! `rollback_rate` — la part des mutations acceptées qu'on annule. `W21.e`, ADR 0024.
//!
//! # Un taux instantané baisse quand on accélère
//!
//! Une mutation acceptée aujourd'hui peut être annulée demain. Un taux calculé à l'instant `T`
//! divise donc des annulations qui ont eu le temps de survenir par des acceptations qui, pour les
//! plus récentes, ne l'ont pas eu.
//!
//! La conséquence est perverse : plus le système accepte vite, plus la proportion d'acceptations
//! trop jeunes pour avoir été annulées est grande, et plus le taux paraît bas. « On annule de moins
//! en moins » se produit tout seul dès qu'on accélère, sans qu'aucune décision ne se soit
//! améliorée.
//!
//! C'est le biais de censure à droite, et il ne se corrige pas par une moyenne : il se corrige en
//! disant **de quoi** on parle. D'où la cohorte.
//!
//! # Une cohorte est un ensemble délimité **plus** une fenêtre
//!
//! « Des mutations acceptées dans la fenêtre `W`, la part annulée dans les `N` opérations qui ont
//! suivi. » Les deux moitiés sont nécessaires, et [`Cohort::over`] ne se construit pas sans elles :
//! sans fenêtre d'observation, « annulée » n'a pas de borne et le taux recommence à dépendre de
//! l'instant de lecture.
//!
//! La fenêtre se compte en **opérations du journal**, jamais en temps. Une horloge ferait dépendre
//! la mesure d'un fait extérieur au journal, alors que la matrice exige des métriques « calculées
//! depuis le seul journal » — et deux rejeux du même préfixe rendraient deux valeurs différentes.
//!
//! # Une cohorte ouverte ne rend pas de taux
//!
//! Tant qu'une acceptation n'a pas été observée pendant toute sa fenêtre, son sort est inconnu.
//! [`Rollbacks::Open`] porte alors le nombre d'acceptations **encore observables**, et aucun `f64`
//! n'en sort.
//!
//! Rendre un taux provisoire serait pire que de ne rien rendre : il serait affiché à côté de taux
//! définitifs, comparé à eux, et personne ne saurait lequel des deux nombres a fini de bouger. Un
//! test refuse qu'un flottant sorte d'une cohorte ouverte.
//!
//! # Deux cohortes de fenêtres différentes ne se comparent pas
//!
//! Une part annulée « dans les dix opérations » et une part annulée « dans les mille » ne mesurent
//! pas la même chose, et la seconde est mécaniquement plus grande. L'API n'offre donc aucune
//! comparaison entre cohortes : ni ordre, ni fonction de rapprochement. Un test le tient sur les
//! dérivations du type.
//!
//! # Une annulation hors fenêtre n'est pas une annulation de cette cohorte
//!
//! Elle a eu lieu, elle est vraie, et elle appartient à une autre question. La compter ici ferait
//! dépendre le résultat de tout ce qui s'est passé après la fenêtre, ce qui est exactement le biais
//! que la cohorte supprime. Le fait n'est pas perdu — invariant 12 — il est simplement mesuré par la
//! cohorte dont la fenêtre le contient.

use std::fmt;

/// Une acceptation observée, et son sort.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Accepted {
    at: usize,
    reverted_at: Option<usize>,
}

impl Accepted {
    /// Une acceptation qui tient toujours, à la position `at` du journal.
    #[must_use]
    pub const fn holding(at: usize) -> Self {
        Self {
            at,
            reverted_at: None,
        }
    }

    /// Une acceptation annulée à la position `reverted_at`.
    ///
    /// # Errors
    ///
    /// [`CohortError::RevertedBeforeAccepted`] si l'annulation précède l'acceptation — un ordre que
    /// le journal ne peut pas produire, et dont l'acceptation silencieuse ferait compter une
    /// annulation pour une autre mutation.
    pub const fn reverted(at: usize, reverted_at: usize) -> Result<Self, CohortError> {
        if reverted_at < at {
            return Err(CohortError::RevertedBeforeAccepted { at, reverted_at });
        }
        Ok(Self {
            at,
            reverted_at: Some(reverted_at),
        })
    }

    /// Où elle a été acceptée.
    #[must_use]
    pub const fn at(self) -> usize {
        self.at
    }

    /// Où elle a été annulée, si elle l'a été.
    #[must_use]
    pub const fn reverted_at(self) -> Option<usize> {
        self.reverted_at
    }
}

/// Un ensemble d'acceptations délimité, et la fenêtre pendant laquelle on les observe.
///
/// Le type ne dérive **ni** `PartialOrd` **ni** `Ord`, et n'offre aucune fonction de comparaison :
/// deux cohortes de fenêtres différentes ne se comparent pas. Voir la documentation du module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cohort {
    window: usize,
    observed_through: usize,
    entries: Vec<Accepted>,
}

impl Cohort {
    /// Délimiter une cohorte.
    ///
    /// `window` est un nombre d'opérations du journal ; `observed_through` la position jusqu'à
    /// laquelle le journal a été lu.
    ///
    /// # Errors
    ///
    /// [`CohortError::EmptyWindow`] pour une fenêtre nulle : observer pendant zéro opération
    /// n'observe rien, et rendrait toute cohorte close avec zéro annulation — un taux de zéro qui
    /// n'a rien constaté.
    ///
    /// [`CohortError::AcceptedBeyondObservation`] pour une acceptation postérieure à ce qui a été
    /// lu : la cohorte contiendrait un fait que son propre journal ne porte pas.
    pub fn over(
        window: usize,
        observed_through: usize,
        entries: impl IntoIterator<Item = Accepted>,
    ) -> Result<Self, CohortError> {
        if window == 0 {
            return Err(CohortError::EmptyWindow);
        }
        let entries: Vec<Accepted> = entries.into_iter().collect();
        for entry in &entries {
            if entry.at > observed_through {
                return Err(CohortError::AcceptedBeyondObservation {
                    at: entry.at,
                    observed_through,
                });
            }
        }
        Ok(Self {
            window,
            observed_through,
            entries,
        })
    }

    /// La fenêtre d'observation, en opérations.
    #[must_use]
    pub const fn window(&self) -> usize {
        self.window
    }

    /// Combien d'acceptations la cohorte contient.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Vrai quand la cohorte ne contient aucune acceptation.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Ce que la cohorte permet de dire.
    #[must_use]
    pub fn rollbacks(&self) -> Rollbacks {
        let mut reverted = 0;
        let mut still_observable = 0;

        for entry in &self.entries {
            let deadline = entry.at.saturating_add(self.window);
            match entry.reverted_at {
                // Annulée dans sa fenêtre : son sort est connu, quelle que soit la lecture.
                Some(when) if when <= deadline => reverted += 1,
                // Jamais annulée et fenêtre non close : son sort est encore à venir.
                None if self.observed_through < deadline => still_observable += 1,
                // Les deux restants ont un sort **connu**, et le même : « a tenu pendant sa
                // fenêtre ». Une annulée après sa fenêtre appartient à une autre cohorte ; une
                // jamais annulée dont la fenêtre est close a fini d'être observée. Deux faits
                // distincts, une seule conséquence — les écrire en deux bras vides dirait le
                // contraire, et clippy a raison de le refuser.
                _ => {}
            }
        }

        if still_observable > 0 {
            return Rollbacks::Open {
                still_observable,
                reverted_so_far: reverted,
                accepted: self.entries.len(),
            };
        }
        Rollbacks::Closed {
            reverted,
            accepted: self.entries.len(),
        }
    }
}

/// Ce qu'une cohorte permet de dire — et ce qu'elle refuse de dire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rollbacks {
    /// Toutes les acceptations ont été observées pendant toute leur fenêtre.
    Closed {
        /// Combien ont été annulées dans leur fenêtre.
        reverted: usize,
        /// Combien la cohorte en contient.
        accepted: usize,
    },
    /// Au moins une acceptation n'a pas fini sa fenêtre. **Aucun taux n'en sort.**
    Open {
        /// Combien attendent encore la fin de leur fenêtre.
        still_observable: usize,
        /// Combien ont déjà été annulées — un **compte**, jamais un taux.
        reverted_so_far: usize,
        /// Combien la cohorte en contient.
        accepted: usize,
    },
}

impl Rollbacks {
    /// Le taux — `rollback_rate` proprement dit.
    ///
    /// `None` sur une cohorte ouverte, et `None` sur une cohorte close vide : diviser par zéro
    /// acceptation ne rend pas zéro, cela ne rend rien.
    #[must_use]
    pub fn rate(self) -> Option<f64> {
        let Self::Closed { reverted, accepted } = self else {
            return None;
        };
        if accepted == 0 {
            return None;
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "un compte d'acceptations ne franchit pas 2^53"
        )]
        Some(reverted as f64 / accepted as f64)
    }

    /// Vrai quand toutes les fenêtres sont observées.
    #[must_use]
    pub const fn is_closed(self) -> bool {
        matches!(self, Self::Closed { .. })
    }
}

impl fmt::Display for Rollbacks {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed { reverted, accepted } => {
                write!(formatter, "{reverted}/{accepted} annulées")
            }
            Self::Open {
                still_observable,
                reverted_so_far,
                accepted,
            } => write!(
                formatter,
                "cohorte ouverte : {still_observable}/{accepted} attendent encore la fin de leur \
                 fenêtre, {reverted_so_far} déjà annulées — pas de taux tant qu'elles attendent"
            ),
        }
    }
}

/// Pourquoi une cohorte est refusée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CohortError {
    /// Une fenêtre d'observation nulle.
    EmptyWindow,
    /// Une annulation antérieure à son acceptation.
    RevertedBeforeAccepted {
        /// Où l'acceptation a eu lieu.
        at: usize,
        /// Où l'annulation prétend avoir eu lieu.
        reverted_at: usize,
    },
    /// Une acceptation postérieure à ce que le journal a été lu.
    AcceptedBeyondObservation {
        /// Où elle prétend avoir eu lieu.
        at: usize,
        /// Jusqu'où le journal a été lu.
        observed_through: usize,
    },
}

impl fmt::Display for CohortError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyWindow => formatter.write_str(
                "une fenêtre nulle n'observe rien : toute cohorte serait close avec zéro annulation, \
                 et ce zéro n'aurait rien constaté",
            ),
            Self::RevertedBeforeAccepted { at, reverted_at } => write!(
                formatter,
                "une annulation en {reverted_at} précède son acceptation en {at} : le journal ne \
                 produit pas cet ordre, et l'accepter ferait compter une annulation pour une autre \
                 mutation"
            ),
            Self::AcceptedBeyondObservation {
                at,
                observed_through,
            } => write!(
                formatter,
                "une acceptation en {at} dépasse ce qui a été lu ({observed_through}) : la cohorte \
                 porterait un fait que son propre journal ne contient pas"
            ),
        }
    }
}

impl std::error::Error for CohortError {}
