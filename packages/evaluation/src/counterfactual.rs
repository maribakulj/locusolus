//! Le substitut d'environnement et les trajectoires contrefactuelles —
//! `docs/10_V1_ROADMAP.md`, item `R4`.
//!
//! « Générateur de trajectoires contrefactuelles, comparatif à graine et préfixe identiques,
//! **unilatéral en rejet, jamais un juge, jamais une preuve** ; et la fidélité sur les
//! environnements du domaine — IIIF, SPARQL, ALTO/PAGE, notebooks, prouveurs — est **inconnue**. »
//!
//! # Unilatéral, et c'est un chemin de types
//!
//! [`Outcome`] a deux variantes et une seule conclut : [`Outcome::Refuted`]. L'autre s'appelle
//! [`Outcome::NotRefuted`], **pas** « confirmé ». Ce n'est pas une nuance de vocabulaire : deux
//! trajectoires qui coïncident sur une graine et un préfixe donnés peuvent parfaitement diverger
//! sur la suivante, et rien dans la comparaison ne dit le contraire. Un accesseur
//! `is_confirmed()` ferait de l'absence de contre-exemple une preuve, ce que la roadmap interdit
//! en toutes lettres — et un test le tient par l'absence.
//!
//! `NotRefuted` porte donc le nombre de pas comparés. « Non réfuté sur trois pas » et « non réfuté
//! sur trois mille » ne sont pas la même chose, et un verdict qui tairait la différence les rendrait
//! interchangeables — même raison que le nombre de rejeux d'une [`Baseline`](crate::credit::Baseline)
//! et le nombre de candidats d'un [`Regret`](crate::regret::Regret).
//!
//! # Même graine, même préfixe, ou rien
//!
//! Deux trajectoires qui ne partagent ni la graine ni le début ne se comparent pas : leur divergence
//! s'explique par tout, donc par rien. La comparaison refuse, en disant **laquelle** des deux
//! conditions manque — les deux se réparent différemment, l'une en refixant la graine, l'autre en
//! rejouant le préfixe.
//!
//! # La fidélité est inconnue, et le type le dit
//!
//! La roadmap est explicite : « la fidélité sur les environnements du domaine est **inconnue** ».
//! Il n'existe donc aucun moyen d'exprimer une fidélité mesurée — pas d'énumération à deux
//! variantes dont une serait vide, pas de `f64` par défaut. [`fidelity`] rend un [`Unmeasured`], et
//! c'est le seul type qu'elle sait rendre. Le jour où quelqu'un mesure, le type change et **tous**
//! les appelants sont forcés de regarder ; un champ qui attendait déjà la valeur les aurait
//! dispensés de le faire.

use std::fmt;

/// Les environnements du domaine dont `R4` dit la fidélité inconnue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DomainEnvironment {
    /// Serveurs et clients IIIF.
    Iiif,
    /// Points d'accès SPARQL.
    Sparql,
    /// ALTO et PAGE XML.
    AltoPage,
    /// Notebooks.
    Notebook,
    /// Prouveurs — Lean, Z3, cvc5.
    Prover,
}

impl DomainEnvironment {
    /// Les cinq, dans l'ordre de la roadmap.
    pub const ALL: [Self; 5] = [
        Self::Iiif,
        Self::Sparql,
        Self::AltoPage,
        Self::Notebook,
        Self::Prover,
    ];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Iiif => "iiif",
            Self::Sparql => "sparql",
            Self::AltoPage => "alto_page",
            Self::Notebook => "notebook",
            Self::Prover => "prover",
        }
    }
}

impl fmt::Display for DomainEnvironment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Ce qu'on sait de la fidélité d'un substitut sur un environnement : rien.
///
/// Le type ne porte pas de nombre parce qu'aucun nombre n'a été mesuré. Il porte l'environnement,
/// pour que le constat soit adressable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unmeasured {
    environment: DomainEnvironment,
}

impl Unmeasured {
    /// De quel environnement on ne sait rien.
    #[must_use]
    pub const fn environment(self) -> DomainEnvironment {
        self.environment
    }
}

impl fmt::Display for Unmeasured {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "la fidélité d'un substitut sur `{}` n'a pas été mesurée",
            self.environment
        )
    }
}

/// Ce qu'on sait de la fidélité d'un substitut sur cet environnement.
#[must_use]
pub const fn fidelity(environment: DomainEnvironment) -> Unmeasured {
    Unmeasured { environment }
}

/// Une trajectoire : une graine, un préfixe rejoué, et la suite observée.
///
/// Le préfixe et la suite sont séparés parce qu'ils ne jouent pas le même rôle : le préfixe est la
/// **condition** de la comparaison, la suite en est l'objet. Les mettre dans une seule liste
/// obligerait à convenir d'un indice de coupure, que rien ne porterait.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trajectory {
    seed: u64,
    prefix: Vec<String>,
    tail: Vec<String>,
}

impl Trajectory {
    /// Consigner une trajectoire.
    #[must_use]
    pub fn observed(seed: u64, prefix: &[&str], tail: &[&str]) -> Self {
        Self {
            seed,
            prefix: prefix.iter().map(|step| (*step).to_owned()).collect(),
            tail: tail.iter().map(|step| (*step).to_owned()).collect(),
        }
    }

    /// La graine.
    #[must_use]
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    /// Le préfixe rejoué.
    #[must_use]
    pub fn prefix(&self) -> &[String] {
        &self.prefix
    }

    /// La suite observée.
    #[must_use]
    pub fn tail(&self) -> &[String] {
        &self.tail
    }
}

/// Ce qu'une comparaison conclut.
///
/// Deux variantes, **une seule conclusive**.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Les deux suites divergent : l'hypothèse est réfutée, et on dit où.
    Refuted {
        /// Le premier pas qui diffère.
        step: usize,
        /// Ce que la trajectoire réelle a fait.
        actual: String,
        /// Ce que la contrefactuelle a fait.
        counterfactual: String,
    },
    /// Aucune divergence sur les pas comparés.
    ///
    /// **Ce n'est pas une confirmation.** Les deux suites peuvent diverger au pas suivant, et rien
    /// ici ne dit le contraire. Le nombre de pas comparés voyage avec le constat, parce que « non
    /// réfuté sur trois pas » et « non réfuté sur trois mille » ne sont pas la même chose.
    NotRefuted {
        /// Combien de pas ont été comparés.
        compared: usize,
    },
}

impl Outcome {
    /// Vrai quand la comparaison a produit un contre-exemple.
    ///
    /// Il n'existe **pas** de méthode symétrique. Un `is_confirmed()` ferait de l'absence de
    /// contre-exemple une preuve, et c'est exactement ce que `R4` interdit.
    #[must_use]
    pub const fn refutes(&self) -> bool {
        matches!(self, Self::Refuted { .. })
    }
}

impl fmt::Display for Outcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refuted {
                step,
                actual,
                counterfactual,
            } => write!(
                formatter,
                "divergence au pas {step} : `{actual}` contre `{counterfactual}`"
            ),
            Self::NotRefuted { compared } => write!(
                formatter,
                "aucune divergence sur {compared} pas — ce qui ne prouve rien"
            ),
        }
    }
}

/// Comparer une trajectoire réelle à une contrefactuelle.
///
/// # Errors
///
/// [`CompareError::DifferentSeed`] et [`CompareError::DifferentPrefix`]. Deux trajectoires qui ne
/// partagent ni la graine ni le début ne se comparent pas : leur divergence s'explique par tout,
/// donc par rien. Les deux refus sont distincts parce qu'ils se réparent différemment — l'un en
/// refixant la graine, l'autre en rejouant le préfixe.
pub fn compare(actual: &Trajectory, counterfactual: &Trajectory) -> Result<Outcome, CompareError> {
    if actual.seed() != counterfactual.seed() {
        return Err(CompareError::DifferentSeed {
            actual: actual.seed(),
            counterfactual: counterfactual.seed(),
        });
    }
    if actual.prefix() != counterfactual.prefix() {
        let step = actual
            .prefix()
            .iter()
            .zip(counterfactual.prefix())
            .position(|(one, other)| one != other)
            .unwrap_or_else(|| actual.prefix().len().min(counterfactual.prefix().len()));
        return Err(CompareError::DifferentPrefix { step });
    }

    let compared = actual.tail().len().min(counterfactual.tail().len());
    for step in 0..compared {
        if actual.tail()[step] != counterfactual.tail()[step] {
            return Ok(Outcome::Refuted {
                step,
                actual: actual.tail()[step].clone(),
                counterfactual: counterfactual.tail()[step].clone(),
            });
        }
    }
    Ok(Outcome::NotRefuted { compared })
}

/// Ce qui empêche de comparer deux trajectoires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareError {
    /// Les graines diffèrent.
    DifferentSeed {
        /// Celle de la trajectoire réelle.
        actual: u64,
        /// Celle de la contrefactuelle.
        counterfactual: u64,
    },
    /// Les préfixes diffèrent, au pas donné.
    DifferentPrefix {
        /// Le premier pas qui diffère — ou la longueur du plus court, quand l'un est le début de
        /// l'autre.
        step: usize,
    },
}

impl fmt::Display for CompareError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DifferentSeed {
                actual,
                counterfactual,
            } => write!(
                formatter,
                "graines {actual} et {counterfactual} : la divergence s'expliquerait par le tirage"
            ),
            Self::DifferentPrefix { step } => write!(
                formatter,
                "les préfixes divergent au pas {step} : les deux trajectoires ne partent pas du même endroit"
            ),
        }
    }
}

impl std::error::Error for CompareError {}
