//! Le graphe épistémique — `docs/SPEC_V1.md` §7.5 et §7.6.
//!
//! # Une seule phrase gouverne ce paquet
//!
//! §7.6 : « le système **NE DOIT PAS** réduire un raisonnement multi-prémisses à plusieurs arêtes
//! indépendantes. »
//!
//! Trois prémisses qui, ensemble, soutiennent une conclusion ne sont pas trois soutiens séparés, et
//! la différence n'est pas de présentation :
//!
//! - réfuter une prémisse sur trois casse l'inférence **entière** ; sur trois arêtes
//!   indépendantes, il en resterait deux, et la conclusion paraîtrait encore soutenue aux deux
//!   tiers ;
//! - une objection à la **règle** n'a aucun endroit où se poser sur des arêtes indépendantes,
//!   puisque la règle n'y existe pas ;
//! - « quelles sont les prémisses minimales de ce claim » (§9.4) rendrait trois réponses d'une
//!   prémisse au lieu d'une réponse de trois — c'est-à-dire « il suffit d'un des trois » au lieu de
//!   « il faut ces trois ».
//!
//! D'où la forme : [`graph::Graph`] range relations binaires et inférences **séparément**, et
//! n'offre aucun chemin de l'une vers l'autre. Pas de `flatten`, pas de `decompose`, pas
//! d'`as_edges` — un test le vérifie par l'absence, parce que c'est exactement la fonction de
//! commodité que quelqu'un finira par vouloir écrire.
//!
//! # La seconde phrase
//!
//! §7.5 : « les relations non symétriques ne doivent pas être inférées en sens inverse. » Chaque
//! relation déclare sa direction, et [`graph::Graph::traversable_backwards`] refuse vingt-deux
//! relations sur vingt-huit. `A supports B` lu à l'envers ferait de la preuve une thèse ; `cites`
//! ferait citer un article de 2026 par un article de 1890.

pub mod consensus;
pub mod graph;
pub mod inference;
pub mod relation;

pub use consensus::{CircularConsensus, circular_consensus, citation_cycles};
pub use graph::{Graph, Support};
pub use inference::{FormalizationStatus, Inference, InferenceFindings, ObjectionTarget};
pub use relation::{Direction, Relation, RelationKind, Strength};
