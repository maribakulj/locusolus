//! Le budget de Locus Solus — `docs/SPEC_V1.md` §7.2, invariant 6.
//!
//! # Deux rôles, et il faut les deux
//!
//! **La réservation empêche.** Invariant 6 : « les ressources sont réservées avant exécution ;
//! elles ne sont pas supposées illimitées. » Une retenue refusée ne rend rien avec quoi exécuter —
//! littéralement : [`Reservation`] n'a pas de constructeur public, et seul [`BudgetAccount::reserve`]
//! en produit une.
//!
//! **Le registre constate.** §7.2 : « le budget est un registre, pas un compteur mutable isolé. »
//! Les soldes ne sont pas des champs, ils se déduisent des écritures — et une écriture ne se
//! rectifie pas, elle se compense. Un registre qui refuserait d'écrire un dépassement le rendrait
//! invisible exactement là où il fallait le voir.
//!
//! Confondre les deux casse l'un des deux : un registre qui empêche ment sur le passé, une
//! réservation qui constate n'empêche rien.

mod account;
mod dimension;
mod ledger;
mod limits;

pub use account::{BudgetAccount, BudgetError, Overrun, Reconciliation, Reservation, Settlement};
pub use dimension::{Amounts, Dimension};
pub use ledger::{Entry, EntryKind};
pub use limits::{Limits, Unbounded};
