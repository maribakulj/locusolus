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
//!   runtime de containers. Le driver macOS arrive en W4.e ;
//! - [`admission`] : la décision **avant** exécution, sur des capacités déclarées ;
//! - [`linux`] : le backend Linux rootless — la traduction, la lecture de ce que l'hôte permet,
//!   et le driver Podman (W4.d).
//!
//! Depuis W4.d.2, ce paquet **lance** un runtime : [`linux::SystemRunner`] est la seule fonction du
//! dépôt qui exécute `podman`. C'est précisément ce que la séparation autorise ici et nulle part
//! ailleurs, et un test balaie l'arbre pour vérifier que personne d'autre n'en parle.

pub mod admission;
pub mod linux;
pub mod runtime;

pub use admission::{Admission, HostCapabilities, RefusalReason, admit};
pub use linux::{ConfinementPlan, HostFacts, PlanError, PodmanBackend, Workload, plan};
pub use runtime::{RuntimeError, RuntimePort, SandboxId};
