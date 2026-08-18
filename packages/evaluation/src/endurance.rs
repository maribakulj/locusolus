//! L'endurance de §29.6 — ce qu'une campagne doit avoir traversé.
//!
//! # Neuf exigences, dont huit en liste
//!
//! §29.6 : « campagne de sept jours minimum avec : 10 workstreams simultanés ; 30 branches au
//! total ; 100 instances d'agents successives ; 5 000 tâches ; 250 000 événements ; redémarrages
//! réguliers ; pertes de workers ; reprise sans perte ni double application. »
//!
//! Huit puces, et la durée dans la phrase qui les introduit. Elle est ici avec les autres : une
//! campagne de six jours qui aurait atteint les huit puces n'est pas la campagne que §29.6 demande,
//! et la faire vivre ailleurs que dans la liste reviendrait à la faire oublier.
//!
//! # Trois façons de ne pas tenir, et elles n'appellent pas le même geste
//!
//! Un seuil **mesuré et sous la barre** demande de prolonger la campagne. Un seuil **non relevé**
//! demande d'instrumenter — personne n'a compté, et c'est une panne de mesure, pas de tenue. Un
//! invariant **violé** — une perte, une double application — demande de corriger le produit, et ne
//! se rattrape pas en tournant plus longtemps.
//!
//! Les fondre en un seul « échec » ferait chercher au mauvais endroit dans deux cas sur trois. C'est
//! la même règle que pour `locus doctor` (W11.a) : « pas vérifié » n'est jamais « atteint ».

use std::collections::BTreeMap;
use std::fmt;

/// Ce que §29.6 exige d'une campagne d'endurance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Requirement {
    /// Sept jours au moins — la phrase qui introduit la liste.
    DurationDays,
    /// Dix workstreams simultanés.
    ConcurrentWorkstreams,
    /// Trente branches au total.
    Branches,
    /// Cent instances d'agents successives.
    AgentInstances,
    /// Cinq mille tâches.
    Tasks,
    /// Deux cent cinquante mille événements.
    Events,
    /// Des redémarrages.
    Restarts,
    /// Des pertes de workers.
    WorkerLosses,
    /// La reprise sans perte ni double application.
    ///
    /// La seule exigence qui ne se compte pas : elle tient ou elle ne tient pas.
    RecoveryIntact,
}

impl Requirement {
    /// Les neuf, dans l'ordre où §29.6 les donne.
    pub const ALL: [Self; 9] = [
        Self::DurationDays,
        Self::ConcurrentWorkstreams,
        Self::Branches,
        Self::AgentInstances,
        Self::Tasks,
        Self::Events,
        Self::Restarts,
        Self::WorkerLosses,
        Self::RecoveryIntact,
    ];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::DurationDays => "duration-days",
            Self::ConcurrentWorkstreams => "concurrent-workstreams",
            Self::Branches => "branches",
            Self::AgentInstances => "agent-instances",
            Self::Tasks => "tasks",
            Self::Events => "events",
            Self::Restarts => "restarts",
            Self::WorkerLosses => "worker-losses",
            Self::RecoveryIntact => "recovery-intact",
        }
    }

    /// Le minimum que §29.6 fixe, ou `None` pour l'exigence qui ne se compte pas.
    ///
    /// Les redémarrages et les pertes de workers valent un : §29.6 les veut « réguliers » sans
    /// chiffrer, et zéro est la seule valeur dont on soit sûr qu'elle ne les exerce pas.
    #[must_use]
    pub const fn minimum(self) -> Option<u64> {
        match self {
            Self::DurationDays => Some(7),
            Self::ConcurrentWorkstreams => Some(10),
            Self::Branches => Some(30),
            Self::AgentInstances => Some(100),
            Self::Tasks => Some(5_000),
            Self::Events => Some(250_000),
            Self::Restarts | Self::WorkerLosses => Some(1),
            Self::RecoveryIntact => None,
        }
    }
}

impl fmt::Display for Requirement {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Ce qu'une campagne a relevé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Measure {
    /// Un compteur.
    Counted(u64),
    /// Un invariant qui a tenu, ou non.
    Held(bool),
}

/// Le relevé d'une campagne d'endurance.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Campaign {
    measures: BTreeMap<Requirement, Measure>,
}

impl Campaign {
    /// Une campagne dont rien n'a encore été relevé.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Consigner un compteur.
    ///
    /// # Errors
    ///
    /// [`EnduranceError::NotCounted`] pour [`Requirement::RecoveryIntact`], qui ne se compte pas :
    /// « la reprise s'est bien passée 4 fois » ne dit rien de la cinquième, et c'est exactement la
    /// question.
    pub fn counted(mut self, requirement: Requirement, value: u64) -> Result<Self, EnduranceError> {
        if requirement.minimum().is_none() {
            return Err(EnduranceError::NotCounted { requirement });
        }
        self.measures.insert(requirement, Measure::Counted(value));
        Ok(self)
    }

    /// Consigner qu'un invariant a tenu, ou non.
    ///
    /// # Errors
    ///
    /// [`EnduranceError::NotAnInvariant`] pour une exigence qui se compte : répondre « oui » à
    /// « avez-vous eu 5 000 tâches ? » ne dit pas combien.
    pub fn held(mut self, requirement: Requirement, held: bool) -> Result<Self, EnduranceError> {
        if requirement.minimum().is_some() {
            return Err(EnduranceError::NotAnInvariant { requirement });
        }
        self.measures.insert(requirement, Measure::Held(held));
        Ok(self)
    }

    /// Ce qui a été relevé pour `requirement`.
    #[must_use]
    pub fn measure(&self, requirement: Requirement) -> Option<Measure> {
        self.measures.get(&requirement).copied()
    }

    /// Ce que §29.6 dit de cette campagne.
    ///
    /// Calculé, jamais déclaré : une campagne qui se dirait endurante le resterait jusqu'à la
    /// première panne en production.
    #[must_use]
    pub fn endurance(&self) -> Endurance {
        let mut short = Vec::new();
        let mut unmeasured = Vec::new();
        let mut violated = Vec::new();

        for requirement in Requirement::ALL {
            match (requirement.minimum(), self.measures.get(&requirement)) {
                (_, None) => unmeasured.push(requirement),
                (Some(minimum), Some(Measure::Counted(value))) if *value < minimum => {
                    short.push(Shortfall {
                        requirement,
                        reached: *value,
                        minimum,
                    });
                }
                (None, Some(Measure::Held(false))) => violated.push(requirement),
                (Some(_) | None, Some(Measure::Counted(_) | Measure::Held(_))) => {}
            }
        }

        if short.is_empty() && unmeasured.is_empty() && violated.is_empty() {
            Endurance::Held
        } else {
            Endurance::Fell {
                short,
                unmeasured,
                violated,
            }
        }
    }
}

/// Un seuil mesuré et manqué.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shortfall {
    /// Lequel.
    pub requirement: Requirement,
    /// Ce que la campagne a atteint.
    pub reached: u64,
    /// Ce que §29.6 demande.
    pub minimum: u64,
}

/// Ce que §29.6 dit d'une campagne.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Endurance {
    /// Les neuf exigences sont tenues.
    Held,
    /// Elle n'y est pas — et les trois causes restent séparées, parce qu'elles n'appellent pas le
    /// même geste : prolonger, instrumenter, ou corriger le produit.
    Fell {
        /// Mesuré, sous la barre : prolonger.
        short: Vec<Shortfall>,
        /// Non relevé : instrumenter.
        unmeasured: Vec<Requirement>,
        /// Invariant violé : corriger, et tourner plus longtemps n'y changera rien.
        violated: Vec<Requirement>,
    },
}

impl fmt::Display for Endurance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Held => formatter.write_str("endurante"),
            Self::Fell {
                short,
                unmeasured,
                violated,
            } => {
                formatter.write_str("non endurante")?;
                for fall in short {
                    write!(
                        formatter,
                        " ; {} : {}/{}",
                        fall.requirement, fall.reached, fall.minimum
                    )?;
                }
                for requirement in unmeasured {
                    write!(formatter, " ; {requirement} : non relevé")?;
                }
                for requirement in violated {
                    write!(formatter, " ; {requirement} : violé")?;
                }
                Ok(())
            }
        }
    }
}

/// Ce qui empêche un relevé d'être consigné.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnduranceError {
    /// Cette exigence ne se compte pas.
    NotCounted {
        /// Laquelle.
        requirement: Requirement,
    },
    /// Cette exigence se compte, elle ne se constate pas.
    NotAnInvariant {
        /// Laquelle.
        requirement: Requirement,
    },
}

impl fmt::Display for EnduranceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotCounted { requirement } => write!(
                formatter,
                "« {requirement} » ne se compte pas : « la reprise s'est bien passée 4 fois » ne \
                 dit rien de la cinquième, et c'est exactement la question"
            ),
            Self::NotAnInvariant { requirement } => write!(
                formatter,
                "« {requirement} » se compte : répondre « oui » ne dit pas combien"
            ),
        }
    }
}

impl std::error::Error for EnduranceError {}
