//! `failure_recovery_time` — combien de temps une panne dure. `W21.k`, ADR 0024.
//!
//! # Ce que le cadre d'endurance sait déjà, et ce qui lui manquait
//!
//! [`crate::endurance`] compte les **pertes de workers** et dit si la reprise a tenu sans perte ni
//! double application. Il ne dit pas combien de temps elle a pris — et deux campagnes qui tiennent
//! également peuvent reprendre en une seconde ou en une heure.
//!
//! Ce module ajoute la durée sans toucher au cadre : il produit un compte de pannes qu'une
//! [`crate::endurance::Campaign`] accepte tel quel pour
//! [`crate::endurance::Requirement::WorkerLosses`]. Un test le branche pour de vrai, plutôt que de
//! l'annoncer.
//!
//! # Une panne sans reprise n'est ni omise, ni une durée nulle
//!
//! C'est la règle de cet item. Une panne dont la reprise n'a pas eu lieu décrit un système **encore
//! à terre** — le fait le plus important qu'une campagne puisse produire. L'omettre le ferait
//! disparaître du relevé ; la compter comme une reprise instantanée le retournerait en son
//! contraire, et ferait même **baisser** la durée moyenne.
//!
//! [`Recovery::Unrecovered`] la nomme donc, et ne porte aucune durée.
//!
//! # Deux absences de durée, et elles ne se ressemblent pas
//!
//! [`Recoveries::longest`] rend `None` dans deux situations : aucune panne n'a eu lieu, ou aucune
//! panne n'a été reprise. La première est une bonne nouvelle, la seconde la pire possible.
//!
//! Le compte de pannes les sépare, et c'est pour cela qu'il est rendu à côté de la durée plutôt que
//! résumé dedans — même forme que les relais et les tentatives de `W21.i`.
//!
//! # Ce que la mesure vaut, et ce qu'elle ne prouve pas
//!
//! Elle se calcule sur fixtures, et elle y est testée. Son **interprétation**, elle, demande une
//! campagne longue : trois transitions dans un test ne disent rien de la robustesse d'un système en
//! production, et lire l'une pour l'autre est la faute que `docs/11` évite en exigeant sept jours.
//!
//! Le dire ici plutôt que de le supposer est la même discipline que la troisième condition de l'ADR
//! 0024 : une valeur mesurée sur trois transitions ne doit pas se lire comme un fait de production.
//!
//! # Aucune horloge
//!
//! Les instants entrent en **données**, comme dans `W21.j` et pour la même raison : une durée qui
//! dépendrait de l'instant de lecture changerait à chaque lecture. Il n'y a pas d'instant courant à
//! soustraire, donc `Unrecovered` ne peut pas devenir une durée, même par erreur.

use std::fmt;

/// Une panne, et sa reprise si elle a eu lieu.
///
/// Les instants sont des millisecondes, reçues en données : voir la documentation du module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Outage {
    failed_at: i64,
    recovered_at: Option<i64>,
}

impl Outage {
    /// Une panne dont la reprise **n'a pas eu lieu**.
    #[must_use]
    pub const fn ongoing(failed_at: i64) -> Self {
        Self {
            failed_at,
            recovered_at: None,
        }
    }

    /// Une panne suivie de sa reprise.
    ///
    /// # Errors
    ///
    /// [`RecoveryError::RecoveredBeforeFailing`] quand la reprise précède la panne — un ordre que le
    /// journal ne produit pas, et dont l'acceptation rendrait une durée qui aurait l'air juste.
    pub const fn recovered(failed_at: i64, recovered_at: i64) -> Result<Self, RecoveryError> {
        if recovered_at < failed_at {
            return Err(RecoveryError::RecoveredBeforeFailing {
                failed_at,
                recovered_at,
            });
        }
        Ok(Self {
            failed_at,
            recovered_at: Some(recovered_at),
        })
    }

    /// Ce que la panne a coûté — `failure_recovery_time` proprement dit.
    #[must_use]
    pub const fn recovery(self) -> Recovery {
        match self.recovered_at {
            Some(at) => Recovery::Recovered {
                millis: at.saturating_sub(self.failed_at),
            },
            None => Recovery::Unrecovered,
        }
    }
}

/// La durée d'une panne, ou son absence de fin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recovery {
    /// La reprise a eu lieu, et voici en combien de temps.
    Recovered {
        /// En millisecondes.
        millis: i64,
    },
    /// La reprise n'a **pas** eu lieu. Aucune durée : voir la documentation du module.
    Unrecovered,
}

impl Recovery {
    /// La durée, si la reprise a eu lieu.
    #[must_use]
    pub const fn millis(self) -> Option<i64> {
        match self {
            Self::Recovered { millis } => Some(millis),
            Self::Unrecovered => None,
        }
    }
}

impl fmt::Display for Recovery {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Recovered { millis } => write!(formatter, "reprise en {millis} ms"),
            Self::Unrecovered => {
                formatter.write_str("pas de reprise — le système est encore à terre")
            }
        }
    }
}

/// Ce qu'un ensemble de pannes permet de dire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Recoveries {
    failures: u64,
    unrecovered: u64,
    longest: Option<i64>,
}

impl Recoveries {
    /// Relever un ensemble de pannes.
    #[must_use]
    pub fn over<'a>(outages: impl IntoIterator<Item = &'a Outage>) -> Self {
        let mut counted = Self::default();
        for outage in outages {
            counted.failures += 1;
            match outage.recovery() {
                Recovery::Recovered { millis } => {
                    counted.longest =
                        Some(counted.longest.map_or(millis, |so_far| so_far.max(millis)));
                }
                // Aucune durée n'entre ici : compter une panne non reprise comme instantanée
                // ferait **baisser** la plus longue, et retournerait le pire fait en son contraire.
                Recovery::Unrecovered => counted.unrecovered += 1,
            }
        }
        counted
    }

    /// Combien de pannes ont eu lieu.
    ///
    /// Ce compte alimente [`crate::endurance::Requirement::WorkerLosses`] tel quel, sans que le
    /// cadre d'endurance ait à changer.
    #[must_use]
    pub const fn failures(self) -> u64 {
        self.failures
    }

    /// Combien sont **encore** sans reprise.
    #[must_use]
    pub const fn unrecovered(self) -> u64 {
        self.unrecovered
    }

    /// La plus longue reprise observée.
    ///
    /// `None` quand aucune panne n'a été reprise — ce qui recouvre deux situations opposées, et
    /// c'est [`Self::failures`] qui les sépare : voir la documentation du module.
    #[must_use]
    pub const fn longest(self) -> Option<i64> {
        self.longest
    }
}

/// Pourquoi une panne est refusée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryError {
    /// La reprise précède la panne.
    RecoveredBeforeFailing {
        /// Quand la panne a eu lieu.
        failed_at: i64,
        /// Quand la reprise prétend avoir eu lieu.
        recovered_at: i64,
    },
}

impl fmt::Display for RecoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecoveredBeforeFailing {
                failed_at,
                recovered_at,
            } => write!(
                formatter,
                "une reprise en {recovered_at} précède sa panne en {failed_at} : le journal ne \
                 produit pas cet ordre, et l'accepter rendrait une durée qui aurait l'air juste"
            ),
        }
    }
}

impl std::error::Error for RecoveryError {}
