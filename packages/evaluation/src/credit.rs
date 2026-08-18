//! Le crédit structurel — `docs/10_V1_ROADMAP.md`, item `R2`.
//!
//! « Attribuer une amélioration à une relation, un rôle, un budget ou **au hasard
//! d'échantillonnage** ; sans cela un système évolutionnaire accumule des changements inutiles. »
//!
//! # Le hasard est une issue nommée, jamais un reste
//!
//! C'est la phrase entière de l'item. Une attribution qui rend toujours l'un des trois facteurs
//! donne une histoire à chaque fluctuation : on a changé quelque chose, la mesure a bougé, donc le
//! changement a marché. Rien dans ce raisonnement ne distingue une amélioration d'un tirage
//! favorable — et un système qui l'applique en boucle garde tous ses changements, dont la moitié
//! n'a rien fait.
//!
//! [`Credit::SamplingNoise`] est donc une variante à part entière, qui porte l'écart **et** la bande
//! dans laquelle il tombe. Elle ne dit pas « on ne sait pas » : elle dit « voici de combien la même
//! configuration varie toute seule, et votre écart est dedans ».
//!
//! # La bande se mesure, elle ne se suppose pas
//!
//! [`Baseline`] se construit à partir de rejeux de la **même** configuration sous des graines
//! différentes, et il en faut au moins deux. Il n'existe ni bande par défaut, ni seuil constant :
//! une bande inventée ferait passer pour du bruit ce qui n'en est pas, ou l'inverse, selon un
//! chiffre que personne n'a mesuré.
//!
//! # Deux facteurs changés, aucune attribution
//!
//! Si deux bras diffèrent par la relation **et** par le budget, l'écart n'est attribuable à ni l'un
//! ni l'autre, et le dire serait pire que se taire. Le refus les nomme, parce que la suite est
//! d'aller mesurer chacun séparément, et qu'un « non attribuable » sans liste envoie tout remesurer.
//!
//! # Une régression s'attribue comme une amélioration
//!
//! [`Credit::Attributed`] porte un écart **signé**. Un changement qui a dégradé la mesure au-delà du
//! bruit est un résultat, et l'invariant 12 interdit de supprimer les résultats négatifs pour rendre
//! le dossier propre. C'est aussi le seul moyen de défaire un changement inutile plutôt que de
//! l'oublier.

use std::collections::BTreeSet;
use std::fmt;

/// Ce qui distingue deux bras d'un comparatif — les trois facteurs de `R2`.
///
/// Le hasard d'échantillonnage n'en est pas un : on ne le change pas, on le mesure. Le ranger ici
/// donnerait un quatrième bouton à tourner, qui n'existe pas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Factor {
    /// Une relation de coordination — qui relit qui, qui voit quoi.
    Relation,
    /// Le rôle tenu par l'agent.
    Role,
    /// Le budget accordé.
    Budget,
}

impl Factor {
    /// Les trois.
    pub const ALL: [Self; 3] = [Self::Relation, Self::Role, Self::Budget];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Relation => "relation",
            Self::Role => "role",
            Self::Budget => "budget",
        }
    }
}

impl fmt::Display for Factor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Un bras du comparatif : les trois facteurs, fixés.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arm {
    relation: String,
    role: String,
    budget: u64,
}

impl Arm {
    /// Décrire un bras.
    #[must_use]
    pub fn new(relation: &str, role: &str, budget: u64) -> Self {
        Self {
            relation: relation.to_owned(),
            role: role.to_owned(),
            budget,
        }
    }

    /// La relation de coordination en vigueur.
    #[must_use]
    pub fn relation(&self) -> &str {
        &self.relation
    }

    /// Le rôle tenu.
    #[must_use]
    pub fn role(&self) -> &str {
        &self.role
    }

    /// Le budget accordé.
    #[must_use]
    pub const fn budget(&self) -> u64 {
        self.budget
    }

    /// Les facteurs par lesquels ce bras diffère d'un autre.
    #[must_use]
    pub fn differing_factors(&self, other: &Self) -> BTreeSet<Factor> {
        let mut differing = BTreeSet::new();
        if self.relation != other.relation {
            differing.insert(Factor::Relation);
        }
        if self.role != other.role {
            differing.insert(Factor::Role);
        }
        if self.budget != other.budget {
            differing.insert(Factor::Budget);
        }
        differing
    }
}

/// De combien une configuration varie toute seule, d'une graine à l'autre.
///
/// Mesurée par rejeu, jamais posée. La bande est l'**étendue** des rejeux — le plus grand moins le
/// plus petit — et non un écart-type : trois rejeux ne renseignent pas un écart-type, et en calculer
/// un donnerait à trois mesures l'apparence d'une distribution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Baseline {
    band: f64,
    replays: usize,
}

impl Baseline {
    /// Mesurer la bande sur des rejeux de la **même** configuration.
    ///
    /// # Errors
    ///
    /// [`CreditError::TooFewReplays`] à moins de deux rejeux : un seul ne dit rien de la variation,
    /// et zéro encore moins. [`CreditError::NotAMeasure`] pour une valeur non finie — `NaN` compris,
    /// que la comparaison de plage laisserait passer sans ce refus, contrairement au cas d'une borne
    /// où elle suffit.
    pub fn from_replays(utilities: &[f64]) -> Result<Self, CreditError> {
        if utilities.len() < 2 {
            return Err(CreditError::TooFewReplays {
                given: utilities.len(),
            });
        }
        for value in utilities {
            if !value.is_finite() {
                return Err(CreditError::NotAMeasure { value: *value });
            }
        }
        let highest = utilities.iter().copied().fold(f64::MIN, f64::max);
        let lowest = utilities.iter().copied().fold(f64::MAX, f64::min);
        Ok(Self {
            band: highest - lowest,
            replays: utilities.len(),
        })
    }

    /// L'étendue observée.
    #[must_use]
    pub const fn band(self) -> f64 {
        self.band
    }

    /// Combien de rejeux la portent.
    ///
    /// Une bande tirée de deux rejeux et une bande tirée de deux cents ne se lisent pas pareil, et
    /// le nombre voyage avec la bande pour qu'on n'ait pas à le retrouver.
    #[must_use]
    pub const fn replays(self) -> usize {
        self.replays
    }
}

/// À quoi l'écart est attribué.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Credit {
    /// Un facteur, et un seul, a changé, et l'écart sort de la bande de bruit.
    Attributed {
        /// Lequel.
        factor: Factor,
        /// L'écart, **signé** : une régression s'attribue comme une amélioration.
        gain: f64,
    },
    /// L'écart tient dans ce que la même configuration produit d'une graine à l'autre.
    SamplingNoise {
        /// L'écart observé.
        gain: f64,
        /// La bande dans laquelle il tombe.
        band: f64,
    },
}

impl Credit {
    /// Vrai quand l'écart est attribué à un facteur **et** qu'il améliore.
    ///
    /// Deux questions distinctes réunies dans un seul accesseur seraient un piège ; celui-ci ne
    /// répond qu'à la seconde, et [`Credit::factor`] répond à la première.
    #[must_use]
    pub fn is_improvement(&self) -> bool {
        matches!(self, Self::Attributed { gain, .. } if *gain > 0.0)
    }

    /// Le facteur crédité, s'il y en a un.
    #[must_use]
    pub const fn factor(&self) -> Option<Factor> {
        match self {
            Self::Attributed { factor, .. } => Some(*factor),
            Self::SamplingNoise { .. } => None,
        }
    }
}

impl fmt::Display for Credit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Attributed { factor, gain } => {
                write!(formatter, "{gain:+.4} attribué à `{factor}`")
            }
            Self::SamplingNoise { gain, band } => write!(
                formatter,
                "{gain:+.4} tient dans une bande de {band:.4} : rien n'est attribué"
            ),
        }
    }
}

/// Attribuer l'écart entre deux bras.
///
/// # Errors
///
/// [`CreditError::Unchanged`] quand les deux bras sont identiques — il n'y a rien à attribuer, et
/// rendre `SamplingNoise` ferait croire qu'un facteur a été éprouvé.
///
/// [`CreditError::Confounded`] quand plus d'un facteur diffère, en les **nommant** : la suite est
/// d'aller mesurer chacun séparément, et un « non attribuable » sans liste envoie tout remesurer.
///
/// [`CreditError::NotAMeasure`] pour une utilité non finie.
pub fn attribute(
    before: &Arm,
    before_utility: f64,
    after: &Arm,
    after_utility: f64,
    baseline: Baseline,
) -> Result<Credit, CreditError> {
    for value in [before_utility, after_utility] {
        if !value.is_finite() {
            return Err(CreditError::NotAMeasure { value });
        }
    }
    let differing = before.differing_factors(after);
    let mut factors = differing.into_iter();
    let Some(factor) = factors.next() else {
        return Err(CreditError::Unchanged);
    };
    if factors.next().is_some() {
        return Err(CreditError::Confounded {
            factors: before.differing_factors(after),
        });
    }

    let gain = after_utility - before_utility;
    if gain.abs() <= baseline.band() {
        return Ok(Credit::SamplingNoise {
            gain,
            band: baseline.band(),
        });
    }
    Ok(Credit::Attributed { factor, gain })
}

/// Ce qui empêche d'attribuer un crédit.
#[derive(Debug, Clone, PartialEq)]
pub enum CreditError {
    /// Les deux bras sont identiques.
    Unchanged,
    /// Plus d'un facteur diffère.
    Confounded {
        /// Lesquels.
        factors: BTreeSet<Factor>,
    },
    /// Moins de deux rejeux : la bande de bruit n'est pas mesurée.
    TooFewReplays {
        /// Combien ont été donnés.
        given: usize,
    },
    /// Une utilité qui n'est pas un nombre fini.
    NotAMeasure {
        /// Ce qui a été donné.
        value: f64,
    },
}

impl fmt::Display for CreditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unchanged => formatter.write_str(
                "les deux bras sont identiques : il n'y a pas d'écart à attribuer, et pas de facteur éprouvé",
            ),
            Self::Confounded { factors } => {
                write!(formatter, "{} facteurs diffèrent — ", factors.len())?;
                let named: Vec<&'static str> =
                    factors.iter().map(|factor| factor.slug()).collect();
                write!(
                    formatter,
                    "{} — et l'écart n'est attribuable à aucun",
                    named.join(", ")
                )
            }
            Self::TooFewReplays { given } => write!(
                formatter,
                "{given} rejeu(x) ne mesurent pas une bande de bruit : il en faut au moins deux"
            ),
            Self::NotAMeasure { value } => {
                write!(formatter, "`{value}` n'est pas une mesure exploitable")
            }
        }
    }
}

impl std::error::Error for CreditError {}
