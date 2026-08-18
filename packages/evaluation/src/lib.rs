//! Les épreuves closes de §29 et le verdict de préparation — `docs/SPEC_V1.md` §29.
//!
//! # Pourquoi des listes closes
//!
//! §29.4 nomme treize fautes à injecter, §29.5 quatorze attaques, §29.8 huit ablations. Ce sont des
//! listes **closes**, et c'est ce qui rend l'exercice vérifiable : une liste nommée permet de dire
//! ce qui n'a *pas* été éprouvé, ce qu'une intention générale — « on testera l'injection de
//! fautes » — ne permet jamais.
//!
//! Une release qui part sans avoir éprouvé le disque plein n'est pas nécessairement une faute. La
//! faute est de ne pas le savoir.
//!
//! # Trois états, et le troisième est celui qui compte
//!
//! Une épreuve est [`Standing::Exercised`] — quelqu'un l'a faite, et on dit par quoi —,
//! [`Standing::Waived`] — on a décidé de ne pas la faire, et on dit pourquoi — ou
//! [`Standing::Unaddressed`], qui est l'état par défaut et le seul qui bloque.
//!
//! L'écart entre `Waived` et `Unaddressed` est tout l'objet de ce module. Les deux se ressemblent
//! dans un rapport — aucune épreuve n'a été menée — et ne se ressemblent pas du tout : l'une est une
//! décision qu'on peut contester, l'autre est un oubli que personne ne voit. Une renonciation sans
//! raison n'en est donc pas une, et le registre la refuse.
//!
//! # Le verdict se calcule
//!
//! Il n'existe aucun champ « prêt ». Sixième occurrence de la même forme dans ce chantier, après
//! l'attestation de sandbox, le digest de build, le niveau de reproductibilité, l'attestation
//! d'indépendance et le verdict de `locus doctor`.

pub mod benchmark;
pub mod counterfactual;
pub mod credit;
pub mod endurance;
pub mod evolution;
pub mod regret;

pub use benchmark::{
    BenchmarkError, Comparison, Configuration, Coverage, Direction, Metric, Ranking,
};
pub use counterfactual::{
    CompareError, DomainEnvironment, Outcome, Trajectory, Unmeasured, compare, fidelity,
};
pub use credit::{Arm, Baseline, Credit, CreditError, Factor, attribute};
pub use endurance::{Campaign, Endurance, EnduranceError, Measure, Requirement, Shortfall};
pub use evolution::{Evolution, EvolutionError, Improvement, Occurrence, consider};
pub use regret::{Candidate, Regret, RegretError, regret};

use std::collections::BTreeMap;
use std::fmt;

/// De quoi une épreuve relève.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Family {
    /// Injection de fautes — §29.4.
    FaultInjection,
    /// Sécurité — §29.5.
    Security,
    /// Ablation — §29.8.
    Ablation,
}

impl Family {
    /// Les trois familles que ce registre couvre.
    pub const ALL: [Self; 3] = [Self::FaultInjection, Self::Security, Self::Ablation];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::FaultInjection => "fault-injection",
            Self::Security => "security",
            Self::Ablation => "ablation",
        }
    }

    /// La section qui la nomme.
    #[must_use]
    pub const fn section(self) -> &'static str {
        match self {
            Self::FaultInjection => "§29.4",
            Self::Security => "§29.5",
            Self::Ablation => "§29.8",
        }
    }

    /// Les épreuves que §29 nomme dans cette famille, dans l'ordre du texte.
    #[must_use]
    pub const fn trials(self) -> &'static [&'static str] {
        match self {
            Self::FaultInjection => &[
                "postgres-unavailable",
                "temporal-restarted",
                "object-store-unavailable",
                "network-partitioned",
                "worker-killed",
                "heartbeat-delayed",
                "duplicate-event",
                "corrupted-projection",
                "partial-upload",
                "clock-skew",
                "disk-full",
                "revoked-secret",
                "malicious-federated-peer",
            ],
            Self::Security => &[
                "write-outside-workspace",
                "forbidden-read",
                "network-exfiltration",
                "prompt-injection",
                "secret-leak",
                "path-traversal",
                "archive-bomb",
                "ssrf",
                "identity-confusion",
                "token-replay",
                "acl-bypass-via-vector-search",
                "sandbox-downgrade",
                "malicious-dependency",
                "cross-tenant-leakage",
            ],
            Self::Ablation => &[
                "without-graph",
                "without-negative-results",
                "without-blind-reviewers",
                "without-model-diversity",
                "without-adaptive-allocation",
                "without-formalisation",
                "without-bounded-context-view",
                "without-cross-programme-memory",
            ],
        }
    }
}

impl fmt::Display for Family {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Où en est une épreuve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Standing {
    /// Menée, et on dit par quoi.
    Exercised {
        /// Ce qui l'a menée — un test, une campagne, un rapport.
        by: String,
    },
    /// Écartée délibérément, et on dit pourquoi.
    ///
    /// Une renonciation est une décision qu'on peut contester. C'est ce qui la sépare de l'oubli,
    /// et c'est pourquoi la raison est obligatoire.
    Waived {
        /// Pourquoi.
        reason: String,
    },
    /// Personne ne s'en est occupé.
    ///
    /// L'état par défaut, et le seul qui bloque. Il ne se déclare pas : il se constate par absence.
    Unaddressed,
}

/// Le registre des épreuves de §29.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrialRegistry {
    standings: BTreeMap<(Family, &'static str), Standing>,
}

impl Default for TrialRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TrialRegistry {
    /// Un registre où rien n'a encore été fait.
    ///
    /// Toutes les épreuves de §29.4, §29.5 et §29.8 y figurent en [`Standing::Unaddressed`] : c'est
    /// ce qui fait qu'on ne peut pas oublier une épreuve en omettant de l'inscrire.
    #[must_use]
    pub fn new() -> Self {
        let mut standings = BTreeMap::new();
        for family in Family::ALL {
            for trial in family.trials() {
                standings.insert((family, *trial), Standing::Unaddressed);
            }
        }
        Self { standings }
    }

    /// Consigner qu'une épreuve a été menée.
    ///
    /// # Errors
    ///
    /// [`RegistryError::UnknownTrial`] pour une épreuve que §29 ne nomme pas, et
    /// [`RegistryError::EmptyField`] pour un porteur vide : « éprouvé » sans dire par quoi ne se
    /// vérifie pas, donc ne vaut pas mieux que « pas éprouvé ».
    pub fn exercised(self, family: Family, trial: &str, by: &str) -> Result<Self, RegistryError> {
        if by.trim().is_empty() {
            return Err(RegistryError::EmptyField { field: "by" });
        }
        self.record(family, trial, Standing::Exercised { by: by.to_owned() })
    }

    /// Consigner qu'une épreuve est délibérément écartée.
    ///
    /// # Errors
    ///
    /// [`RegistryError::UnknownTrial`], et [`RegistryError::EmptyField`] pour une raison vide : une
    /// renonciation sans raison est indiscernable d'un oubli, et c'est exactement la confusion que
    /// ce registre existe pour empêcher.
    pub fn waived(self, family: Family, trial: &str, reason: &str) -> Result<Self, RegistryError> {
        if reason.trim().is_empty() {
            return Err(RegistryError::EmptyField { field: "reason" });
        }
        self.record(
            family,
            trial,
            Standing::Waived {
                reason: reason.to_owned(),
            },
        )
    }

    fn record(
        mut self,
        family: Family,
        trial: &str,
        standing: Standing,
    ) -> Result<Self, RegistryError> {
        let known = family
            .trials()
            .iter()
            .find(|known| **known == trial)
            .ok_or_else(|| RegistryError::UnknownTrial {
                family,
                trial: trial.to_owned(),
            })?;
        self.standings.insert((family, *known), standing);
        Ok(self)
    }

    /// Où en est une épreuve.
    #[must_use]
    pub fn standing(&self, family: Family, trial: &str) -> Option<&Standing> {
        self.standings
            .iter()
            .find(|((known_family, known_trial), _)| {
                *known_family == family && *known_trial == trial
            })
            .map(|(_, standing)| standing)
    }

    /// Ce dont personne ne s'est occupé.
    #[must_use]
    pub fn unaddressed(&self) -> Vec<(Family, &'static str)> {
        self.standings
            .iter()
            .filter(|(_, standing)| **standing == Standing::Unaddressed)
            .map(|((family, trial), _)| (*family, *trial))
            .collect()
    }

    /// Ce qui a été écarté, avec la raison.
    #[must_use]
    pub fn waivers(&self) -> Vec<(Family, &'static str, &str)> {
        self.standings
            .iter()
            .filter_map(|((family, trial), standing)| match standing {
                Standing::Waived { reason } => Some((*family, *trial, reason.as_str())),
                Standing::Exercised { .. } | Standing::Unaddressed => None,
            })
            .collect()
    }

    /// Le verdict de préparation.
    ///
    /// Calculé, jamais déclaré. Une release peut partir avec des renonciations — c'est une décision
    /// — mais pas avec des oublis, parce qu'un oubli n'a été décidé par personne.
    #[must_use]
    pub fn readiness(&self) -> Readiness {
        let unaddressed = self.unaddressed();
        if unaddressed.is_empty() {
            Readiness::Ready {
                waivers: self.waivers().len(),
            }
        } else {
            Readiness::Blocked { unaddressed }
        }
    }
}

/// Ce que le registre dit d'une release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Readiness {
    /// Toute épreuve est menée ou écartée avec sa raison.
    Ready {
        /// Combien ont été écartées — un chiffre qu'un relecteur voudra regarder.
        waivers: usize,
    },
    /// Il reste des épreuves dont personne ne s'est occupé.
    Blocked {
        /// Lesquelles.
        unaddressed: Vec<(Family, &'static str)>,
    },
}

impl fmt::Display for Readiness {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready { waivers: 0 } => formatter.write_str("prête ; aucune renonciation"),
            Self::Ready { waivers } => {
                write!(formatter, "prête ; {waivers} renonciation(s) à relire")
            }
            Self::Blocked { unaddressed } => {
                write!(formatter, "bloquée ; non traité :")?;
                for (family, trial) in unaddressed {
                    write!(formatter, " {}/{trial}", family.section())?;
                }
                Ok(())
            }
        }
    }
}

/// Ce qui empêche une épreuve d'être consignée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// Un champ obligatoire vide.
    EmptyField {
        /// Lequel.
        field: &'static str,
    },
    /// Une épreuve que §29 ne nomme pas.
    UnknownTrial {
        /// Dans quelle famille on la cherchait.
        family: Family,
        /// Le nom reçu.
        trial: String,
    },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(
                formatter,
                "« {field} » est vide : sans lui, la consigne ne se vérifie pas et ne vaut pas \
                 mieux qu'une épreuve non menée"
            ),
            Self::UnknownTrial { family, trial } => write!(
                formatter,
                "« {trial} » n'est pas une épreuve que {} nomme — la liste est close, et c'est \
                 ce qui permet de dire ce qui manque",
                family.section()
            ),
        }
    }
}

impl std::error::Error for RegistryError {}
