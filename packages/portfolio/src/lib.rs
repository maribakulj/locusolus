//! Le portefeuille de Locus Solus — `docs/SPEC_V1.md` §13.
//!
//! # L'anti-gaming d'abord
//!
//! `docs/10` l'inscrit : « l'anti-gaming doit exister avant que la fonction de valeur pilote des
//! décisions automatiques ». Ce n'est pas de la prudence, c'est une dépendance : une fonction de
//! valeur mise en service avant ses garde-fous **enseigne** ce qu'il faut optimiser, et ce qu'elle
//! enseigne alors est la faille. Ajouter les détecteurs ensuite ne défait pas ce qui a été appris.
//!
//! L'ordre des commits l'atteste une fois ; un merge écrasé n'en garde rien. Ce qui survit est le
//! type : [`Screening`] n'a pas d'autre constructeur que [`screen`], et [`value`] l'exige. Une
//! branche jamais criblée n'a donc pas une valeur haute — elle n'a **pas de valeur**.

mod activity;
mod gaming;
mod scheduler;
mod value;

pub use activity::{ArtifactRecord, BranchActivity, ClaimRecord, ReviewRecord};
pub use gaming::{
    Gaming, GamingFinding, LexicalSimilarity, Screening, Similarity, Thresholds, screen,
};
pub use scheduler::{Candidate, Policy, Reason, Selection, Slot, schedule};
pub use value::{Indicators, Valuation, ValueError, Weights, value};
