//! `locus-execd` — le broker d'exécution privilégié, seul détenteur du socket de runtime.
//!
//! # Pourquoi ce binaire est séparé
//!
//! ADR 0004 : « `locus-execd` est un service séparé. `locusd` ne détient **jamais** de socket
//! Docker/Podman. » CLAUDE.md ajoute la tentation à laquelle la séparation résiste : « parler à
//! Podman *juste pour le profil local* est exactement ce que cette séparation empêche ».
//!
//! La raison n'est pas esthétique. `locusd` parle au monde entier — cockpit, workers, pairs
//! fédérés, contenu récupéré sur le Web. Un socket de runtime dans ce processus-là donne à qui le
//! compromet le pouvoir de créer des conteneurs privilégiés, c'est-à-dire d'annuler tout le
//! confinement par l'intérieur. Le broker, lui, ne parle qu'à `locusd`, sur une surface étroite.
//!
//! # Ce que ce paquet contient aujourd'hui
//!
//! - [`runtime::RuntimePort`] : la seule description, dans tout le dépôt, de ce qu'on demande à un
//!   runtime de containers ;
//! - [`admission`] : la décision **avant** exécution, sur des capacités déclarées ;
//! - [`linux`] : le backend Linux rootless — la traduction, la lecture de ce que l'hôte permet,
//!   et le driver Podman (W4.d) ;
//! - [`macos`] : la machine qui porte l'invité Linux, et le plafond qui en découle (W4.e) ;
//! - [`build`] : construire une image, premier maillon de la chaîne de `packages/environments` (W5) ;
//! - [`placement`] : choisir un hôte parmi plusieurs, sur ce qu'il a **prouvé** (W4.g) ;
//! - [`reroute`] : replacer une tentative dont l'hôte est tombé, **sous le même numéro** (W4.g).
//!
//! Depuis W4.d.2, ce paquet **lance** un runtime : [`linux::SystemRunner`] est la seule fonction du
//! dépôt qui exécute `podman`. C'est précisément ce que la séparation autorise ici et nulle part
//! ailleurs, et un test balaie l'arbre pour vérifier que personne d'autre n'en parle.

pub mod admission;
pub mod build;
pub mod linux;
pub mod macos;
pub mod placement;
pub mod readiness;
pub mod reroute;
pub mod runtime;
pub mod wire;

pub use admission::{
    AcceleratorReach, Admission, DiskQuota, HostCapabilities, RefusalReason, admit,
};
pub use build::{BuildContext, BuildDriver, BuildDriverError, build_arguments};
pub use linux::Missing;
pub use linux::{ConfinementPlan, HostFacts, PlanError, PodmanBackend, Workload, plan};
pub use linux::{NO_STORAGE_DECLARED, PROJECT_QUOTA_OPTIONS, QUOTA_CAPABLE_FILESYSTEMS, Support};
pub use macos::{MachineFacts, MachineState};
pub use placement::{Candidate, Placement, place};
pub use readiness::Readiness;
pub use reroute::{Attempt, RerouteError, Rerouting, reroute};
pub use runtime::{RuntimeError, RuntimePort, SandboxId};
