//! Le vocabulaire de l'Execution Fabric — `docs/SPEC_V1.md` §21, §12, §32.3.
//!
//! # Ce que ce paquet est, et ce qu'il n'exécute pas
//!
//! Aucune sandbox n'est démarrée ici, aucun conteneur n'est créé, aucun socket n'est ouvert. Ce
//! crate ne contient que ce qu'une mission **exige**, ce qu'un worker **atteste**, et la
//! confrontation des deux. L'ADR 0004 impose que la suite de self-tests (W4.b) vienne avant le
//! premier backend d'exécution (W4.c) ; W4.a va d'un cran plus haut encore — la suite de tests ne
//! peut pas dire ce que « sandbox » veut dire tant que les mots n'existent pas.
//!
//! # Les trois refus que porte ce paquet
//!
//! 1. **Un niveau appliqué sous le niveau exigé est refusé** (§21.6). Sauf approbation nommée — et
//!    dans ce cas [`attestation::conformance`] **produit** l'événement de sécurité et le met dans
//!    son verdict. On ne peut pas accepter un downgrade sans tenir l'événement en main, parce qu'il
//!    n'existe pas d'autre chemin qui accepte.
//! 2. **Le home, les sockets de runtime et les répertoires de secrets ne se montent pas** (CLAUDE.md,
//!    Sécurité). Même forme : la dérogation existe, elle porte un nom et une raison, et elle produit
//!    son propre événement même quand le niveau d'isolation, lui, est tenu.
//! 3. **Aucune ressource n'est supposée illimitée** (invariant 6). [`resources::ResourceSpec`] n'a
//!    ni `Default`, ni quota optionnel, ni variante « sans limite » : une borne absente n'est pas
//!    une borne large, c'est une borne que personne n'a choisie.
//!
//! # Une note sur les niveaux
//!
//! §21.6 énumère **six** niveaux, `S0` à `S5`. `docs/10_V1_ROADMAP.md` écrit « suite de self-tests
//! indexée par niveau S0–S4 » pour W4.b. La spécification étant normative, les six sont transcrits
//! ici ; ce que la suite de W4.b indexera se décidera là-bas, en connaissance de l'écart.

pub mod approval;
pub mod attestation;
pub mod level;
pub mod resources;
pub mod selftest;
pub mod spec;

pub use approval::{
    Approval, ApprovalError, SECRET_MARKERS, SecurityEvent, SecurityEventError, SecurityEventKind,
    secret_marker,
};
pub use attestation::{AttestationError, Conformance, SandboxAttestation, conformance};
pub use level::{SandboxLevel, SandboxProfile};
pub use resources::{Accelerator, ResourceError, ResourceSpec};
pub use selftest::{
    Dimension, Expectation, Observed, Probe, SELF_TESTABLE_LEVELS, SUITE, Standing, Verdict,
    expectation, judge, newly_contained, standing,
};
pub use spec::{
    FORBIDDEN_MOUNT_MARKERS, Mount, MountMode, NetworkMode, SandboxSpec, SpecError,
    forbidden_marker,
};
