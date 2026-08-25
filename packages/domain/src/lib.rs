//! Le domaine épistémique de Locus Solus.
//!
//! W1.a en livre l'enveloppe commune — `docs/SPEC_V1.md` §7.4 — et les deux vocabulaires qui la
//! gouvernent : les statuts du cycle de vie (§7.4) et les niveaux de validation (§8.1).
//!
//! # La frontière, et pourquoi elle est vérifiée par la CI
//!
//! L'invariant 1 dit que « le domaine ne dépend pas du backend de déploiement », et
//! `boundaries.json` en fait une règle opposable : `packages/domain` n'importe ni infrastructure,
//! ni client `PostgreSQL`, ni SDK Temporal, ni runtime de containers, ni primitive d'entrée-sortie
//! de l'hôte. Ce crate n'ouvre donc aucun fichier, ne lit pas l'heure et ne tire pas au sort.
//!
//! Ce n'est pas une coquetterie. Un domaine qui lit l'horloge produit des révisions dont
//! l'horodatage dépend de la machine qui les a fabriquées ; un domaine qui tire au sort produit
//! des identifiants qu'un test ne peut pas rejouer. Les deux se paient au moment exact où l'on
//! cherche à reconstruire un historique — c'est-à-dire quand ça compte.
//!
//! # Les deux distinctions que ce crate refuse d'effacer
//!
//! 1. **`status` n'est pas `validation_level`.** §7.4 : « `validation_level` décrit la force
//!    épistémique et ne doit pas être déduit du seul statut ». Il n'existe ici aucune conversion
//!    entre les deux, et un test property-based vérifie que toutes les combinaisons sont
//!    représentables — y compris `validated` avec `L0`, qui décrit un objet ayant traversé le
//!    processus sans qu'aucune preuve n'ait été produite.
//!
//! 2. **`stable_id` n'est pas `revision_id`.** Le premier identifie un concept à travers ses
//!    versions, le second une version immuable. Deux types distincts, parce que rien à
//!    l'exécution ne les distinguerait : ce sont deux ULID de même forme.

pub mod branch;
pub mod cognition;
pub mod conflict;
pub mod envelope;
pub mod hash;
pub mod ids;
pub mod lineage;
pub mod negative_result;
pub mod objects;
pub mod status;
pub mod task;
pub mod validation;

pub use branch::{Branch, BranchState, Condition, Origin, TransitionError, ValidationWitness};
pub use cognition::CognitionClass;
pub use conflict::{Conflict, ConflictLog, ConflictOrigin, Verdict, conflicts_from_merge};
pub use envelope::{Confidentiality, Envelope, Ref, Revision};
pub use hash::{ContentHash, Hasher, ParseHashError};
pub use ids::{RevisionId, RevisionKind, StableId, StableKind};
pub use lineage::Lineage;
pub use negative_result::{CONCLUSIVE_POWER, Exclusion, NegativeResult, Power};
pub use objects::{CoreObjectType, ObjectType, ParseObjectTypeError};
pub use status::Status;
pub use task::{ForbiddenTransition, TaskState, implies_validated_claims, transition};
pub use validation::ValidationLevel;
