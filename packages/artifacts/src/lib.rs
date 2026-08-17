//! Les artefacts : le hash déclaré avant l'upload, la quarantaine puis la promotion.
//!
//! # Ce que ce paquet porte
//!
//! ADR 0005 : « les résultats durables sont des artefacts et des manifests, jamais seulement des
//! messages ». Ses conséquences y sont, et rien d'autre : aucun object store n'est branché, aucun
//! octet n'est écrit. Ce paquet dit ce qu'un artefact **est**, par quels états il passe, et
//! comment il traverse le fil sans rien perdre ; W6.c branchera le stockage derrière un port.
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
//!
//! # Le troisième, ajouté par W6.b
//!
//! **Rien ne se perd à la traversée.** Le manifeste porte tous les champs de
//! `artifact-manifest.schema.json`, y compris ceux dont ce crate ne fait rien — un service qui ne
//! connaît que le noyau ne doit pas rendre un manifeste amputé de sa licence. Et il ne se construit
//! rien que le schéma refuserait : la classification est une énumération, la dérivation porte sa
//! relation typée, le type MIME a la forme que le schéma exige. Voir [`wire`] pour ce que la
//! traduction refuse dans l'autre sens.

pub mod derivation;
pub mod manifest;
pub mod state;
pub mod wire;

pub use derivation::{Derivation, DerivationError, DerivationRelation};
pub use manifest::{ArtifactManifest, Integrity, ManifestError, ProducedBy, Rights, ViewerHints};
pub use state::{ArtifactState, ForbiddenTransition, transition};
pub use wire::WireError;
