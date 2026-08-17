//! Les artefacts : le hash déclaré avant l'upload, la quarantaine puis la promotion.
//!
//! # Ce que ce paquet porte
//!
//! ADR 0005 : « les résultats durables sont des artefacts et des manifests, jamais seulement des
//! messages ». Ses conséquences y sont, et rien d'autre : aucun object store n'est branché, aucun
//! octet n'est écrit. Ce paquet dit ce qu'un artefact **est** et par quels états il passe ; W6.b
//! branchera le stockage derrière un port.
//!
//! # Les deux refus que porte ce paquet
//!
//! 1. **Le hash est déclaré avant l'upload.** Un manifeste créé après coup à partir du contenu reçu
//!    ne prouve rien : il dit que ce qui est arrivé est ce qui est arrivé. Déclarer d'abord fait du
//!    hash une promesse, que l'arrivée confronte — c'est la même forme que l'attestation de W4.d.2,
//!    qui vient de l'observation et non de la demande.
//! 2. **La promotion ne se saute pas.** `declared → promoted` n'existe pas dans la table : ADR 0005
//!    dit « quarantaine **puis** promotion », et sauter d'un bout à l'autre servirait un contenu que
//!    personne n'a vu.

pub mod manifest;
pub mod state;

pub use manifest::{ArtifactManifest, ContentHash, ManifestError, ProducedBy};
pub use state::{ArtifactState, ForbiddenTransition, transition};
