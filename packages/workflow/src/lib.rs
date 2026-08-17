//! Ce qu'un workflow est, avant qu'un moteur existe — `docs/SPEC_V1.md` §11.
//!
//! # Pourquoi ce paquet vient en premier
//!
//! ADR 0003 fixe l'ordre : le backend déterministe de test s'écrit avant Temporal. W3.a va un cran
//! plus loin — les **définitions** s'écrivent avant les deux. §11.1 dit que Locus Solus « ne code
//! aucun invariant métier directement contre Temporal » ; ce crate est l'endroit où cette phrase
//! devient vérifiable, parce qu'il ne connaît aucun backend et que W3.b lui en donnera un sans le
//! modifier. Si les définitions venaient après, elles porteraient la forme du premier moteur
//! branché, et l'indépendance serait une intention.
//!
//! # Les six règles de §11.3, et la force inégale de leurs gardes
//!
//! [`determinism::Rule`] les transcrit et dit par quoi chacune tient — [`determinism::Enforcement`]
//! distingue trois forces, et les confondre reviendrait à croire que les six sont tenues pareil :
//!
//! 1. **effets encapsulés** — par construction : [`definition::Step::Deterministic`] n'a pas de
//!    champ où loger un effet. Et par filet : un pas nommé `fetch_manifest` mais déclaré
//!    déterministe est signalé, parce que le type ne voit pas ce que le nom avoue ;
//! 2. **IDs métier créés avant l'entrée** — par construction : [`definition::WorkflowDefinition`]
//!    exige son sujet. Et par filet : [`determinism::minting_findings`] cherche une frappe
//!    d'identifiant dans les sources du paquet ;
//! 3. **side effects idempotents** — par construction : [`definition::Idempotency`] a deux
//!    constructeurs et pas de troisième forme, comme `locus_migrations::Migration` ;
//! 4. **versions explicites** — par construction : [`definition::WorkflowVersion`] n'a pas de
//!    `Default` ;
//! 5. **tests de replay pour les versions supportées** — par décompte :
//!    [`versions::replay_coverage`] confronte deux listes, et ce qui manque est nommé ;
//! 6. **migrations contrôlées** — par construction : [`versions::VersionRegistry::retire`] refuse
//!    les deux retraits qui laisseraient une exécution en cours sans forme déclarée.
//!
//! Aucune de ces gardes ne voit le **corps** d'un pas : une définition est de la donnée, pas du
//! code. Ce que le corps fait se vérifie quand un moteur l'exécute, c'est-à-dire en W3.b.

pub mod backend;
pub mod catalog;
pub mod definition;
pub mod determinism;
pub mod kind;
pub mod versions;

pub use backend::{
    BackendError, Outcome, WorkflowBackend, WorkflowHandle, WorkflowId, WorkflowSignal,
    WorkflowState,
};
// `catalog::definition` n'est pas réexportée : le nom cohabiterait avec le module `definition`,
// et les deux namespaces de Rust rendraient la chose légale sans la rendre lisible.
pub use catalog::CATALOG_VERSION;
pub use definition::{
    Activity, DefinitionError, Effect, Idempotency, Step, WorkflowDefinition, WorkflowVersion,
};
pub use determinism::{
    DeterminismFinding, EFFECT_MARKERS, Enforcement, MINTING_MARKERS, MintingFinding, Rule,
    definition_findings, minting_findings, suspected_effects,
};
pub use kind::{MANDATORY_WORKFLOWS, WorkflowKind};
pub use versions::{CoverageFinding, RetirementError, VersionRegistry, replay_coverage};
