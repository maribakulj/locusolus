//! Le regret structurel — `docs/10_V1_ROADMAP.md`, item `R3`.
//!
//! `R_s = U(meilleur candidat disponible) − U(graphe choisi)`, « calculable en **rejeu** sur
//! fixtures identiques ».
//!
//! # « Disponible », et pas « imaginable »
//!
//! Le regret se mesure contre le meilleur du **menu**, jamais contre un optimum théorique. Comparer
//! à un idéal donnerait un nombre qu'aucune décision n'aurait pu améliorer, et qui grandirait à
//! mesure qu'on imagine mieux. D'où la conséquence : le choisi doit être **parmi** les candidats, et
//! [`regret`] refuse quand il n'y est pas. Calculer un regret contre un menu dont on n'a rien pris
//! n'a pas de sens, et rendrait quand même un nombre.
//!
//! # « Sur fixtures identiques » est une condition, pas une recommandation
//!
//! Deux utilités mesurées sur deux fixtures différentes comparent les fixtures autant que les
//! structures. Chaque [`Candidate`] porte donc le nom de la fixture sur laquelle il a été mesuré, et
//! le calcul refuse un lot qui n'en partage pas une seule. C'est la moitié de l'item : sans elle, le
//! regret est un nombre qu'on peut faire baisser en changeant de fixture.
//!
//! # Un regret plus petit que le bruit n'est pas un regret
//!
//! [`Regret::exceeds`] confronte le regret à la [`Baseline`](crate::credit::Baseline) de `R2` — la
//! bande mesurée par rejeu de la même configuration. Sans cette confrontation, un système
//! poursuivrait des écarts que la même structure produit toute seule d'une graine à l'autre, et
//! changerait d'organisation pour rien. Les deux items de recherche se tiennent par là.

use std::collections::BTreeSet;
use std::fmt;

use crate::credit::Baseline;

/// Une structure candidate, mesurée sur une fixture nommée.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    structure: String,
    fixture: String,
    utility: f64,
}

impl Candidate {
    /// Consigner une mesure.
    ///
    /// # Errors
    ///
    /// [`RegretError::Unnamed`] pour une structure ou une fixture sans nom : un regret qui ne dit
    /// pas contre quoi il se mesure ne se relit pas. [`RegretError::NotAMeasure`] pour une utilité
    /// non finie.
    pub fn measured(structure: &str, fixture: &str, utility: f64) -> Result<Self, RegretError> {
        if structure.trim().is_empty() {
            return Err(RegretError::Unnamed { field: "structure" });
        }
        if fixture.trim().is_empty() {
            return Err(RegretError::Unnamed { field: "fixture" });
        }
        if !utility.is_finite() {
            return Err(RegretError::NotAMeasure { value: utility });
        }
        Ok(Self {
            structure: structure.to_owned(),
            fixture: fixture.to_owned(),
            utility,
        })
    }

    /// La structure mesurée.
    #[must_use]
    pub fn structure(&self) -> &str {
        &self.structure
    }

    /// La fixture sur laquelle elle l'a été.
    #[must_use]
    pub fn fixture(&self) -> &str {
        &self.fixture
    }

    /// L'utilité observée.
    #[must_use]
    pub const fn utility(&self) -> f64 {
        self.utility
    }
}

/// Ce qu'on a laissé sur la table.
#[derive(Debug, Clone, PartialEq)]
pub struct Regret {
    value: f64,
    best: String,
    chosen: String,
    fixture: String,
    candidates: usize,
}

impl Regret {
    /// `R_s`, toujours positif ou nul.
    #[must_use]
    pub const fn value(&self) -> f64 {
        self.value
    }

    /// La structure qui aurait fait mieux — ou la choisie, quand le regret est nul.
    #[must_use]
    pub fn best(&self) -> &str {
        &self.best
    }

    /// Celle qui a été retenue.
    #[must_use]
    pub fn chosen(&self) -> &str {
        &self.chosen
    }

    /// La fixture commune sur laquelle tout a été mesuré.
    #[must_use]
    pub fn fixture(&self) -> &str {
        &self.fixture
    }

    /// Combien de candidats portaient la comparaison.
    ///
    /// Un regret nul sur deux candidats et un regret nul sur cinquante ne disent pas la même chose,
    /// et le nombre voyage avec la valeur pour qu'on n'ait pas à le retrouver — même raison que le
    /// nombre de rejeux d'une [`Baseline`].
    #[must_use]
    pub const fn candidates(&self) -> usize {
        self.candidates
    }

    /// Vrai quand le choix était le meilleur du menu.
    #[must_use]
    pub fn is_none(&self) -> bool {
        self.value <= 0.0
    }

    /// Vrai quand le regret sort de la bande de bruit mesurée par `R2`.
    ///
    /// Un regret qui tient dans la bande n'est pas un regret : la même structure, rejouée sous une
    /// autre graine, aurait produit cet écart-là. Changer d'organisation pour lui reviendrait à
    /// suivre le tirage.
    #[must_use]
    pub fn exceeds(&self, baseline: Baseline) -> bool {
        self.value > baseline.band()
    }
}

impl fmt::Display for Regret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_none() {
            return write!(
                formatter,
                "aucun regret : `{}` était le meilleur des {} candidats sur `{}`",
                self.chosen, self.candidates, self.fixture
            );
        }
        write!(
            formatter,
            "{:+.4} laissé sur la table : `{}` faisait mieux que `{}` sur `{}`",
            self.value, self.best, self.chosen, self.fixture
        )
    }
}

/// Calculer le regret structurel d'un choix.
///
/// # Errors
///
/// [`RegretError::NoCandidates`] pour un menu vide ; [`RegretError::DuplicateCandidate`] quand une
/// structure est mesurée deux fois — deux mesures de la même chose sont deux rejeux, et c'est une
/// [`Baseline`] qu'elles font, pas deux candidats ;
/// [`RegretError::DifferentFixtures`] quand le lot n'a pas été mesuré sur **une seule** fixture, en
/// les nommant ; [`RegretError::NotAmongCandidates`] quand le choisi n'est pas du menu.
pub fn regret(candidates: &[Candidate], chosen: &str) -> Result<Regret, RegretError> {
    let Some(first) = candidates.first() else {
        return Err(RegretError::NoCandidates);
    };

    let fixtures: BTreeSet<&str> = candidates.iter().map(Candidate::fixture).collect();
    if fixtures.len() > 1 {
        return Err(RegretError::DifferentFixtures {
            fixtures: fixtures.into_iter().map(ToOwned::to_owned).collect(),
        });
    }

    let mut seen = BTreeSet::new();
    for candidate in candidates {
        if !seen.insert(candidate.structure()) {
            return Err(RegretError::DuplicateCandidate {
                structure: candidate.structure().to_owned(),
            });
        }
    }

    let Some(taken) = candidates
        .iter()
        .find(|candidate| candidate.structure() == chosen)
    else {
        return Err(RegretError::NotAmongCandidates {
            chosen: chosen.to_owned(),
        });
    };

    let best = candidates.iter().fold(taken, |best, candidate| {
        if candidate.utility() > best.utility() {
            candidate
        } else {
            best
        }
    });

    Ok(Regret {
        // Jamais négatif, et **sans clamp** : le pli part du choisi et ne le remplace que par
        // strictement mieux, donc `best.utility() >= taken.utility()` par construction. Un
        // `.max(0.0)` ici serait une garde morte — elle rassurerait sur ce qui ne peut pas arriver,
        // et masquerait le jour où le pli cesserait de partir du choisi.
        value: best.utility() - taken.utility(),
        best: best.structure().to_owned(),
        chosen: taken.structure().to_owned(),
        fixture: first.fixture().to_owned(),
        candidates: candidates.len(),
    })
}

/// Ce qui empêche de calculer un regret.
#[derive(Debug, Clone, PartialEq)]
pub enum RegretError {
    /// Un champ d'identité vide.
    Unnamed {
        /// Lequel.
        field: &'static str,
    },
    /// Une utilité qui n'est pas un nombre fini.
    NotAMeasure {
        /// Ce qui a été donné.
        value: f64,
    },
    /// Aucun candidat.
    NoCandidates,
    /// Une structure mesurée deux fois.
    DuplicateCandidate {
        /// Laquelle.
        structure: String,
    },
    /// Le lot n'a pas été mesuré sur une seule fixture.
    DifferentFixtures {
        /// Lesquelles.
        fixtures: BTreeSet<String>,
    },
    /// Le choisi n'est pas du menu.
    NotAmongCandidates {
        /// Lequel.
        chosen: String,
    },
}

impl fmt::Display for RegretError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unnamed { field } => write!(
                formatter,
                "`{field}` est vide : un regret qui ne dit pas contre quoi il se mesure ne se relit pas"
            ),
            Self::NotAMeasure { value } => {
                write!(formatter, "`{value}` n'est pas une mesure exploitable")
            }
            Self::NoCandidates => formatter
                .write_str("un menu vide n'offre rien, et n'a donc rien laissé sur la table"),
            Self::DuplicateCandidate { structure } => write!(
                formatter,
                "`{structure}` est mesurée deux fois : deux mesures d'une même structure sont des rejeux, pas deux candidats"
            ),
            Self::DifferentFixtures { fixtures } => write!(
                formatter,
                "mesuré sur {} fixtures — {} — donc comparant les fixtures autant que les structures",
                fixtures.len(),
                fixtures
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::NotAmongCandidates { chosen } => write!(
                formatter,
                "`{chosen}` n'est pas du menu : un regret contre un menu dont on n'a rien pris ne veut rien dire"
            ),
        }
    }
}

impl std::error::Error for RegretError {}
