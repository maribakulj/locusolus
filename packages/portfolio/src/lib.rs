//! Le portefeuille de Locus Solus — `docs/SPEC_V1.md` §13.
//!
//! # L'anti-gaming d'abord
//!
//! `docs/10` l'inscrit : « l'anti-gaming doit exister avant que la fonction de valeur pilote des
//! décisions automatiques ». Ce n'est pas de la prudence, c'est une dépendance : une fonction de
//! valeur mise en service avant ses garde-fous **enseigne** ce qu'il faut optimiser, et ce qu'elle
//! enseigne alors est la faille. Ajouter les détecteurs ensuite ne défait pas ce qui a été appris.
//!
//! Ce module ne porte, à ce commit, que le criblage de §13.6. La fonction de valeur de §13.4 n'a
//! pas encore de raison d'exister ici.

mod activity;
mod gaming;

pub use activity::{ArtifactRecord, BranchActivity, ClaimRecord, ReviewRecord};
pub use gaming::{
    Gaming, GamingFinding, LexicalSimilarity, Screening, Similarity, Thresholds, screen,
};
