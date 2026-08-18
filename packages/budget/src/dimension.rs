//! Les dimensions bornables d'un budget — `docs/SPEC_V1.md` §7.2.
//!
//! §7.2 porte six champs `limit_*`. Ils ne sont pas interchangeables : épuiser les jetons et
//! épuiser le temps mural sont deux façons différentes de s'arrêter, et un budget qui n'en
//! distinguerait qu'une seule laisserait les autres sans borne — ce que l'invariant 6 refuse.

use std::collections::BTreeMap;
use std::fmt;

/// Une dimension bornable.
///
/// L'ordre de l'énumération est celui de §7.2, et il compte : `Ord` fixe l'ordre de parcours des
/// [`Amounts`], donc l'ordre dans lequel un dépassement est rapporté. Un ordre instable rendrait
/// deux registres identiques distinguables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Dimension {
    /// Un montant monétaire, en micro-unités de la devise du compte.
    Amount,
    /// Des appels modèle.
    ModelCalls,
    /// Des jetons.
    Tokens,
    /// Des secondes de calcul.
    ComputeSeconds,
    /// Des secondes de temps mural.
    WallTimeSeconds,
    /// Un degré de parallélisme.
    Parallelism,
}

impl Dimension {
    /// Les six de §7.2.
    pub const ALL: [Self; 6] = [
        Self::Amount,
        Self::ModelCalls,
        Self::Tokens,
        Self::ComputeSeconds,
        Self::WallTimeSeconds,
        Self::Parallelism,
    ];

    /// Son nom, celui du champ `limit_*` de §7.2 sans le préfixe.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Amount => "amount",
            Self::ModelCalls => "model_calls",
            Self::Tokens => "tokens",
            Self::ComputeSeconds => "compute_seconds",
            Self::WallTimeSeconds => "wall_time_seconds",
            Self::Parallelism => "parallelism",
        }
    }
}

impl fmt::Display for Dimension {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Des quantités, par dimension.
///
/// Une dimension absente vaut zéro — et non « sans limite ». C'est la distinction que l'invariant 6
/// rend nécessaire : ne rien dire d'une ressource ne l'autorise pas.
pub type Amounts = BTreeMap<Dimension, u64>;
