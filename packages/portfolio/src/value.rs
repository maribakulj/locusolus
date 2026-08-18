//! La fonction de valeur de référence — `docs/SPEC_V1.md` §13.4.
//!
//! `V(b) = p_s·I + λ·G + μ·R + ν·D + ξ·N − α·C − β·S − γ·ρ − δ·F`
//!
//! # Ce que §13.4 dit d'elle-même
//!
//! « Cette formule est une politique par défaut, **non une vérité scientifique**. Tous ses
//! paramètres, entrées, incertitudes et overrides sont enregistrés. » C'est pour cela que
//! [`Valuation`] transporte les poids et les indicateurs employés : une valeur dont on ne peut plus
//! retrouver les paramètres est un chiffre, pas une décision.
//!
//! # Ce que le criblage y fait
//!
//! [`value`] exige un [`Screening`]. Ce n'est pas une précaution de politesse : `Screening` n'a pas
//! d'autre constructeur que [`crate::screen`], donc **une branche jamais criblée n'a pas de valeur**
//! — pas une valeur haute, pas de valeur du tout. C'est la forme durable de l'ordre inscrit dans
//! `docs/10` : l'ordre des commits l'atteste une fois, le type l'atteste toujours.
//!
//! La pénalité porte sur les termes **positifs**. Une pénalité multiplicative sur la valeur nette
//! rapprocherait de zéro une branche de valeur négative, c'est-à-dire l'améliorerait : tricher
//! paierait sur les mauvaises branches. Ici, ce que la manœuvre gonfle est exactement ce que la
//! pénalité reprend.

use std::fmt;

use crate::gaming::Screening;

/// Les entrées de `V(b)`, dans l'ordre de §13.4.
///
/// Ce sont dix des quinze indicateurs de §13.2 — ceux que la formule consomme. Les cinq autres
/// (vélocité, couverture de l'espace des stratégies, risque de verrouillage conceptuel, niches
/// méthodologiques, part d'exploitation) appartiennent à la qualité-diversité de §13.3, qui décide
/// d'un **portefeuille** et non d'une branche : les valoriser ici reviendrait à noter une branche
/// sur ce que font les autres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Indicators {
    /// `p_s` — probabilité calibrée de progrès, entre 0 et 1.
    pub calibrated_progress: f64,
    /// `I` — impact.
    pub impact: f64,
    /// `G` — gain d'information.
    pub information_gain: f64,
    /// `R` — réutilisabilité.
    pub reusability: f64,
    /// `D` — diversité.
    pub diversity: f64,
    /// `N` — valeur du négatif attendu.
    pub negative_value: f64,
    /// `C` — coût marginal.
    pub marginal_cost: f64,
    /// `S` — similarité avec le portefeuille.
    pub portfolio_similarity: f64,
    /// `ρ` — corrélation d'erreur.
    pub error_correlation: f64,
    /// `F` — fragilité des dépendances.
    pub dependency_fragility: f64,
}

impl Indicators {
    fn all(&self) -> [f64; 10] {
        [
            self.calibrated_progress,
            self.impact,
            self.information_gain,
            self.reusability,
            self.diversity,
            self.negative_value,
            self.marginal_cost,
            self.portfolio_similarity,
            self.error_correlation,
            self.dependency_fragility,
        ]
    }
}

/// Les coefficients de §13.4.
///
/// # Pourquoi ils valent tous 1 par défaut
///
/// §13.4 donne la **forme** de la formule, pas ses nombres. Inventer ici des coefficients réglés
/// reviendrait à fabriquer une calibration que personne n'a mesurée, et à la faire passer pour la
/// spec parce qu'elle serait écrite en Rust. Le défaut est donc délibérément neutre : il dit « aucune
/// pondération n'a encore été décidée », ce qui est vrai, plutôt que de le cacher derrière des
/// chiffres plausibles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Weights {
    /// `λ`, sur le gain d'information.
    pub lambda: f64,
    /// `μ`, sur la réutilisabilité.
    pub mu: f64,
    /// `ν`, sur la diversité.
    pub nu: f64,
    /// `ξ`, sur la valeur du négatif.
    pub xi: f64,
    /// `α`, sur le coût marginal.
    pub alpha: f64,
    /// `β`, sur la similarité avec le portefeuille.
    pub beta: f64,
    /// `γ`, sur la corrélation d'erreur.
    pub gamma: f64,
    /// `δ`, sur la fragilité des dépendances.
    pub delta: f64,
}

impl Default for Weights {
    fn default() -> Self {
        Self {
            lambda: 1.0,
            mu: 1.0,
            nu: 1.0,
            xi: 1.0,
            alpha: 1.0,
            beta: 1.0,
            gamma: 1.0,
            delta: 1.0,
        }
    }
}

impl Weights {
    fn all(&self) -> [f64; 8] {
        [
            self.lambda,
            self.mu,
            self.nu,
            self.xi,
            self.alpha,
            self.beta,
            self.gamma,
            self.delta,
        ]
    }
}

/// Une valeur, avec de quoi la refaire.
///
/// §13.4 : « tous ses paramètres, entrées, incertitudes et overrides sont enregistrés ». Les
/// paramètres et les entrées sont ici. Une valeur dont on ne peut plus retrouver les paramètres est
/// un chiffre, pas une décision — et elle serait incontestable au mauvais sens du mot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Valuation {
    gross: f64,
    penalty: f64,
    value: f64,
    indicators: Indicators,
    weights: Weights,
    pressure: u8,
}

impl Valuation {
    /// `V(b)`, pénalité comprise — le nombre qui décide.
    #[must_use]
    pub const fn value(&self) -> f64 {
        self.value
    }

    /// `V(b)` avant pénalité.
    ///
    /// Gardée visible exprès : l'écart avec [`Valuation::value`] est ce que la manœuvre a rapporté
    /// avant d'être reprise, et le taire rendrait la pénalité invérifiable.
    #[must_use]
    pub const fn gross(&self) -> f64 {
        self.gross
    }

    /// Ce que le criblage a repris.
    #[must_use]
    pub const fn penalty(&self) -> f64 {
        self.penalty
    }

    /// La pression de criblage appliquée.
    #[must_use]
    pub const fn pressure(&self) -> u8 {
        self.pressure
    }

    /// Les entrées employées.
    #[must_use]
    pub const fn indicators(&self) -> &Indicators {
        &self.indicators
    }

    /// Les coefficients employés.
    #[must_use]
    pub const fn weights(&self) -> &Weights {
        &self.weights
    }
}

impl fmt::Display for Valuation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "V = {:.4} (brut {:.4}, pénalité {:.4} à {} %)",
            self.value, self.gross, self.penalty, self.pressure
        )
    }
}

/// Valoriser une branche **criblée**.
///
/// # Errors
///
/// [`ValueError::NotFinite`] pour une entrée ou un coefficient non fini : un `NaN` ne se compare à
/// rien, pas même à lui-même, et une branche portant un `NaN` deviendrait ni meilleure ni pire que
/// les autres — donc invisible au tri, sans qu'aucune erreur ne le dise.
/// [`ValueError::NotAProbability`] quand `p_s` sort de \[0, 1\] : §13.4 la dit « calibrée », et une
/// probabilité de 3 multiplierait l'impact au lieu de le pondérer.
pub fn value(
    indicators: &Indicators,
    weights: &Weights,
    screening: &Screening,
) -> Result<Valuation, ValueError> {
    if let Some(offender) = indicators
        .all()
        .into_iter()
        .chain(weights.all())
        .find(|number| !number.is_finite())
    {
        return Err(ValueError::NotFinite { offender });
    }
    if !(0.0..=1.0).contains(&indicators.calibrated_progress) {
        return Err(ValueError::NotAProbability {
            value: indicators.calibrated_progress,
        });
    }

    // L'ordre des additions est fixé, et il compte : deux sommes flottantes des mêmes termes dans
    // deux ordres ne donnent pas toujours le même bit, et W7.g trie sur ce nombre.
    let positive = indicators.calibrated_progress * indicators.impact
        + weights.lambda * indicators.information_gain
        + weights.mu * indicators.reusability
        + weights.nu * indicators.diversity
        + weights.xi * indicators.negative_value;
    let negative = weights.alpha * indicators.marginal_cost
        + weights.beta * indicators.portfolio_similarity
        + weights.gamma * indicators.error_correlation
        + weights.delta * indicators.dependency_fragility;

    let gross = positive - negative;
    let pressure = screening.pressure();
    // La pénalité porte sur les termes positifs : c'est ce que la manœuvre gonfle, donc c'est ce
    // qu'elle doit rendre. L'appliquer à la valeur nette rapprocherait de zéro une branche négative.
    let penalty = positive.max(0.0) * f64::from(pressure) / 100.0;

    Ok(Valuation {
        gross,
        penalty,
        value: gross - penalty,
        indicators: *indicators,
        weights: *weights,
        pressure,
    })
}

/// Ce qui empêche une valorisation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ValueError {
    /// Une entrée ou un coefficient non fini.
    NotFinite {
        /// Le nombre fautif.
        offender: f64,
    },
    /// `p_s` hors de \[0, 1\].
    NotAProbability {
        /// La valeur reçue.
        value: f64,
    },
}

impl fmt::Display for ValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFinite { offender } => write!(
                formatter,
                "« {offender} » n'est pas fini : une branche qui en porte un ne se compare à aucune \
                 autre, et disparaît du tri sans que rien ne le dise"
            ),
            Self::NotAProbability { value } => write!(
                formatter,
                "p_s vaut {value} : §13.4 la dit calibrée, et hors de [0, 1] elle multiplie \
                 l'impact au lieu de le pondérer"
            ),
        }
    }
}

impl std::error::Error for ValueError {}
