//! La revue — `docs/SPEC_V1.md` §17, sous l'invariant 11.
//!
//! # Ce que ce paquet porte
//!
//! §17.1 : « la revue est un **protocole** d'évaluation, non un agent unique. Elle doit rendre
//! explicites : le dossier consulté, ce qui a été exclu, les questions posées, l'identité ou
//! l'indépendance du reviewer, les findings, les réponses et la décision finale. »
//!
//! Chacun de ces sept mots est un champ ou une méthode ici, et les quatre premiers sont décidés
//! **avant** que le relecteur commence — c'est le sens de « le dossier est figé avant attribution »
//! (§17.3), et c'est ce qu'une suite de types garantit mieux qu'une convention.
//!
//! # Les deux refus que porte ce paquet
//!
//! 1. **Un dossier attribué ne se modifie plus.** Ce qui vient après est une nouvelle version ou un
//!    addendum visible, jamais une retouche : sans cela, on ne saurait jamais si le relecteur a vu
//!    ce que le dossier dit aujourd'hui.
//! 2. **L'indépendance est constatée, jamais déclarée.** L'attestation se calcule en confrontant le
//!    relecteur au générateur, exigence par exigence. Quatrième occurrence de la même forme, après
//!    l'attestation de sandbox (W4.d.2), le digest de build (W5.e) et le niveau de reproductibilité
//!    (W6.d).
//!
//! # Ce que ce paquet ne fait pas
//!
//! Il n'exécute aucune revue, ne parle à aucun modèle et ne lit aucun fichier. Il dit ce qu'une
//! revue **est** et à quelles conditions elle est indépendante. Le rebuttal et la méta-revue de
//! §17.6 et §17.7 sont W7.d ; la prévention de contamination de §16.6 est W7.b, et elle s'écrit
//! **par cas adverses** — ADR 0016 et `docs/10` demandent tous deux qu'elle ne soit pas seulement
//! garantie par construction.

pub mod contamination;
pub mod context_view;
pub mod disclosure;
pub mod dossier;
pub mod human;
pub mod rebuttal;
pub mod reliability;
pub mod review;
pub mod routing;
pub mod subscription;

pub use contamination::{Contamination, ContextItem, Recipient, contradictions_dropped, inspect};
pub mod from_retrieval;

pub use context_view::{ContextView, ContextViewError, Redaction, Unrestricted, Visible};
pub use disclosure::{
    Contestation, Disclosure, DisclosureError, DisclosureGranted, Motive, Reason, Scope,
};
pub use dossier::{Blindness, DossierError, Draft, Frozen, IndependenceRequirement};
pub use human::{HumanReview, HumanReviewError, HumanVerdict};
pub use rebuttal::{
    MetaReview, Rebuttal, RebuttalError, RecheckPolicy, Recommendation, assign_recheck, meta_review,
};
pub use reliability::{Observation, Reliability};
pub use review::{
    Finding, IndependenceAttestation, Party, Review, ReviewError, Severity, Verdict, attest,
};
pub use routing::{Audience, Peer};
pub use subscription::Subscription;
