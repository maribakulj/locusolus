//! Ce qu'une mission exige de son environnement — `docs/SPEC_V1.md` §19.3, §19.4.
//!
//! # Ce que ce paquet est
//!
//! Le vocabulaire de l'environnement, et rien d'autre. Aucune image n'est construite ici, aucun
//! registre n'est joint, aucun scanner n'est lancé. C'est la même séparation que pour
//! `packages/execution` : ce qu'une mission **déclare** vient avant ce qu'un builder **fait**, sans
//! quoi un environnement ne serait descriptible qu'après avoir existé.
//!
//! # Les trois refus que porte ce paquet
//!
//! 1. **Un profil de toolchain inconnu est refusé.** §19.4 en fixe treize et dit que « les versions
//!    sont verrouillées ». Une chaîne libre ferait qu'un `pyhton-science` mal orthographié produise
//!    une image sans Python, pendant que le blueprint dirait qu'elle en a un.
//! 2. **Une image par tag est refusée.** §21.8 : par digest, jamais par tag. Un environnement dont
//!    l'image peut changer sous lui ne tient pas le niveau `R2` de §19.7, qui est « environnement
//!    verrouillé ».
//! 3. **Une variable qui porte un secret est refusée.** Le schéma de W0.5 écrit qu'il « ne peut pas
//!    empêcher d'y mettre un token, mais peut refuser de prévoir une place pour en mettre un ».
//!    Ici on peut refuser la valeur — par deux tables, parce qu'il y a deux questions : le **nom**
//!    annonce-t-il un secret (`HF_TOKEN`), et la **valeur** en porte-t-elle un (`Bearer …`). Voir
//!    [`blueprint::SECRET_NAME_MARKERS`], qui dit pourquoi les fondre obligerait l'une des deux à
//!    mentir.

pub mod blueprint;
pub mod build;
pub mod toolchain;

pub use blueprint::{
    BlueprintError, EnvironmentBlueprint, Image, Requirements, SECRET_NAME_MARKERS,
    secret_name_marker,
};
pub use build::{
    BuildError, Built, Finding, HealthOutcome, HealthResult, Inventoried, Locked, Lockfile,
    Published, Sbom, Scanned, Severity, Signature, Tested,
};
pub use toolchain::ToolchainProfile;
