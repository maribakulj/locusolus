//! Les artefacts : le hash déclaré avant l'upload, la quarantaine puis la promotion.
//!
//! # Ce que ce paquet porte
//!
//! ADR 0005 : « les résultats durables sont des artefacts et des manifests, jamais seulement des
//! messages ». Ce paquet dit ce qu'un artefact **est**, par quels états il passe, comment il
//! traverse le fil sans rien perdre, et — depuis W6.c — sous quel contrat ses octets entrent.
//!
//! Aucun stockage réel n'y est branché : [`store`] est un port, [`memory`] son implémentation de
//! référence, et un driver sur système de fichiers ou sur S3 passera la même suite de contract
//! tests. C'est la discipline de l'ADR 0012, appliquée aux artefacts.
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
//!
//! # Le quatrième, ajouté par W6.c
//!
//! **Aucun octet n'entre sans manifeste déclaré, et un contenu refusé ne laisse rien derrière
//! lui** — ni sous le hash promis, ni sous le sien. Voir [`store`] pour le contrat et [`ingest`]
//! pour l'ordre des appels, qui est la garantie elle-même.

pub mod derivation;
pub mod ingest;
pub mod manifest;
pub mod memory;
pub mod state;
pub mod store;
pub mod wire;

pub use derivation::{Derivation, DerivationError, DerivationRelation};
pub use ingest::{IngestError, ingest};
pub use manifest::{ArtifactManifest, Integrity, ManifestError, ProducedBy, Rights, ViewerHints};
pub use memory::MemoryObjectStore;
pub use state::{ArtifactState, ForbiddenTransition, transition};
pub use store::{Digest, ObjectStore, StoreError, UploadId};
pub use wire::WireError;
