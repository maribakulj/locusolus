//! Les benchmarks scientifiques de §29.7 — six configurations, onze mesures.
//!
//! # Ce que la comparaison est censée établir
//!
//! §29.8 le dit pour les ablations et vaut ici : « l'objectif est de démontrer quelles parties
//! améliorent réellement la recherche, pas seulement la complexité du produit. » Une comparaison
//! sert donc à trancher entre six architectures, dont la dernière est celle qu'on construit — ce
//! qui rend la tentation de la faire gagner permanente, et les trous de mesure dangereux.
//!
//! # Une mesure absente n'est pas une mesure nulle
//!
//! C'est la faute que ce module existe pour empêcher, et elle est silencieuse : une configuration
//! dont on n'a pas relevé les faux positifs les aurait à zéro dans un classement naïf, donc
//! gagnerait. Le classement **refuse** de trancher tant qu'une lecture manque, et nomme qui manque.
//!
//! Refuser est ici le comportement utile : un classement partiel a l'air d'un résultat, et se cite
//! comme tel.
//!
//! # La direction de chaque mesure
//!
//! Plus n'est pas toujours mieux. Un coût plus élevé est pire, un temps vers validation plus long
//! aussi. Se tromper de sens ferait élire la configuration la plus chère, et le classement aurait
//! l'air parfaitement sain.

use std::collections::BTreeMap;
use std::fmt;

/// Les six configurations que §29.7 compare, dans l'ordre du texte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Configuration {
    /// Agent unique.
    SingleAgent,
    /// Agents parallèles sans mémoire commune.
    ParallelWithoutSharedMemory,
    /// Hiérarchie simple de sous-agents.
    SimpleHierarchy,
    /// Canterel seul.
    CanterelAlone,
    /// Locus Solus sans orchestrateur de portefeuille.
    LocusWithoutPortfolio,
    /// Locus Solus complet.
    LocusComplete,
}

impl Configuration {
    /// Les six, dans l'ordre où §29.7 les numérote.
    pub const ALL: [Self; 6] = [
        Self::SingleAgent,
        Self::ParallelWithoutSharedMemory,
        Self::SimpleHierarchy,
        Self::CanterelAlone,
        Self::LocusWithoutPortfolio,
        Self::LocusComplete,
    ];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::SingleAgent => "single-agent",
            Self::ParallelWithoutSharedMemory => "parallel-without-shared-memory",
            Self::SimpleHierarchy => "simple-hierarchy",
            Self::CanterelAlone => "canterel-alone",
            Self::LocusWithoutPortfolio => "locus-without-portfolio",
            Self::LocusComplete => "locus-complete",
        }
    }
}

impl fmt::Display for Configuration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Dans quel sens une mesure est meilleure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Plus est mieux.
    HigherIsBetter,
    /// Moins est mieux.
    LowerIsBetter,
}

/// Les onze mesures que §29.7 nomme, dans l'ordre du texte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Metric {
    /// Exactitude.
    Accuracy,
    /// Utilité.
    Usefulness,
    /// Nouveauté.
    Novelty,
    /// Faux positifs.
    FalsePositives,
    /// Diversité.
    Diversity,
    /// Coût.
    Cost,
    /// Reproductibilité.
    Reproducibility,
    /// Taux de rejet en revue.
    ReviewRejectionRate,
    /// Temps vers validation.
    TimeToValidation,
    /// Réutilisation des résultats négatifs.
    NegativeReuse,
    /// Capacité à détecter une impasse.
    DeadEndDetection,
}

impl Metric {
    /// Les onze, dans l'ordre où §29.7 les énumère.
    pub const ALL: [Self; 11] = [
        Self::Accuracy,
        Self::Usefulness,
        Self::Novelty,
        Self::FalsePositives,
        Self::Diversity,
        Self::Cost,
        Self::Reproducibility,
        Self::ReviewRejectionRate,
        Self::TimeToValidation,
        Self::NegativeReuse,
        Self::DeadEndDetection,
    ];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Accuracy => "accuracy",
            Self::Usefulness => "usefulness",
            Self::Novelty => "novelty",
            Self::FalsePositives => "false-positives",
            Self::Diversity => "diversity",
            Self::Cost => "cost",
            Self::Reproducibility => "reproducibility",
            Self::ReviewRejectionRate => "review-rejection-rate",
            Self::TimeToValidation => "time-to-validation",
            Self::NegativeReuse => "negative-reuse",
            Self::DeadEndDetection => "dead-end-detection",
        }
    }

    /// Dans quel sens elle est meilleure.
    ///
    /// Quatre mesures se lisent à l'envers des autres. Le taux de rejet en revue est la seule
    /// lecture interprétative : §29.7 ne dit pas son sens, et une architecture dont les productions
    /// se font rejeter plus souvent produit un travail moins bon — c'est ainsi qu'elle est prise
    /// ici, et le dire permet de le contester.
    #[must_use]
    pub const fn direction(self) -> Direction {
        match self {
            Self::FalsePositives
            | Self::Cost
            | Self::ReviewRejectionRate
            | Self::TimeToValidation => Direction::LowerIsBetter,
            Self::Accuracy
            | Self::Usefulness
            | Self::Novelty
            | Self::Diversity
            | Self::Reproducibility
            | Self::NegativeReuse
            | Self::DeadEndDetection => Direction::HigherIsBetter,
        }
    }
}

impl fmt::Display for Metric {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Une comparaison en cours.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Comparison {
    readings: BTreeMap<(Configuration, Metric), f64>,
}

impl Comparison {
    /// Une comparaison où rien n'a été relevé.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Consigner une lecture.
    ///
    /// # Errors
    ///
    /// [`BenchmarkError::NotFinite`] pour une valeur qui n'est pas un nombre fini : `NaN` ne se
    /// compare à rien et se propagerait dans un classement en le rendant muet plutôt que faux, ce
    /// qui est pire — on ne le verrait pas.
    pub fn measured(
        mut self,
        configuration: Configuration,
        metric: Metric,
        value: f64,
    ) -> Result<Self, BenchmarkError> {
        if !value.is_finite() {
            return Err(BenchmarkError::NotFinite {
                configuration,
                metric,
            });
        }
        self.readings.insert((configuration, metric), value);
        Ok(self)
    }

    /// Ce qui a été relevé, s'il l'a été.
    #[must_use]
    pub fn reading(&self, configuration: Configuration, metric: Metric) -> Option<f64> {
        self.readings.get(&(configuration, metric)).copied()
    }

    /// Ce qui manque à cette comparaison.
    #[must_use]
    pub fn coverage(&self) -> Coverage {
        let absent: Vec<(Configuration, Metric)> = Configuration::ALL
            .into_iter()
            .flat_map(|configuration| {
                Metric::ALL
                    .into_iter()
                    .map(move |metric| (configuration, metric))
            })
            .filter(|(configuration, metric)| {
                !self.readings.contains_key(&(*configuration, *metric))
            })
            .collect();

        if absent.is_empty() {
            Coverage::Complete
        } else {
            Coverage::Partial { missing: absent }
        }
    }

    /// Quelle configuration l'emporte sur `metric`.
    ///
    /// **Refuse de trancher tant qu'une lecture manque.** C'est le cœur du module : une
    /// configuration dont on n'a pas relevé les faux positifs les aurait à zéro dans un classement
    /// naïf, donc gagnerait. Un classement partiel a l'air d'un résultat et se cite comme tel.
    #[must_use]
    pub fn best_on(&self, metric: Metric) -> Ranking {
        let missing: Vec<Configuration> = Configuration::ALL
            .into_iter()
            .filter(|configuration| self.reading(*configuration, metric).is_none())
            .collect();
        if !missing.is_empty() {
            return Ranking::Incomparable { missing };
        }

        let mut best = Configuration::ALL[0];
        let mut best_value = self.reading(best, metric).unwrap_or_default();
        for configuration in Configuration::ALL.into_iter().skip(1) {
            let value = self.reading(configuration, metric).unwrap_or_default();
            let better = match metric.direction() {
                Direction::HigherIsBetter => value > best_value,
                Direction::LowerIsBetter => value < best_value,
            };
            if better {
                best = configuration;
                best_value = value;
            }
        }
        Ranking::Best {
            configuration: best,
            value: best_value,
        }
    }
}

/// Ce qui manque à une comparaison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Coverage {
    /// Les six configurations sur les onze mesures.
    Complete,
    /// Il manque des lectures.
    Partial {
        /// Lesquelles.
        missing: Vec<(Configuration, Metric)>,
    },
}

/// Ce qu'une mesure permet de conclure.
#[derive(Debug, Clone, PartialEq)]
pub enum Ranking {
    /// Une configuration l'emporte.
    Best {
        /// Laquelle.
        configuration: Configuration,
        /// Ce qu'elle vaut.
        value: f64,
    },
    /// On ne peut pas trancher : il manque des lectures, et elles sont nommées.
    Incomparable {
        /// Pour quelles configurations.
        missing: Vec<Configuration>,
    },
}

impl fmt::Display for Ranking {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Best {
                configuration,
                value,
            } => write!(formatter, "{configuration} ({value})"),
            Self::Incomparable { missing } => {
                formatter.write_str("incomparable ; non relevé pour")?;
                for configuration in missing {
                    write!(formatter, " {configuration}")?;
                }
                Ok(())
            }
        }
    }
}

/// Ce qui empêche une lecture d'être consignée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchmarkError {
    /// Une valeur qui n'est pas un nombre fini.
    NotFinite {
        /// Pour quelle configuration.
        configuration: Configuration,
        /// Sur quelle mesure.
        metric: Metric,
    },
}

impl fmt::Display for BenchmarkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFinite {
                configuration,
                metric,
            } => write!(
                formatter,
                "{configuration}/{metric} n'est pas un nombre fini : il se propagerait dans un \
                 classement en le rendant muet plutôt que faux, ce qui est pire — on ne le verrait \
                 pas"
            ),
        }
    }
}

impl std::error::Error for BenchmarkError {}
