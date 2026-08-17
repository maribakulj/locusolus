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
//!   runtime de containers. Les drivers arrivent en W4.d (Linux rootless) et W4.e (macOS) ;
//! - [`admission`] : la décision **avant** exécution, sur des capacités déclarées.
//!
//! Aucun socket n'est ouvert ici. Le dire est important : ce paquet est le seul endroit où il sera
//! légitime d'en ouvrir un, et un test balaie l'arbre pour vérifier que personne d'autre n'en parle.

pub mod admission;
pub mod runtime;

pub use admission::{Admission, HostCapabilities, RefusalReason, admit};
pub use runtime::{RuntimeError, RuntimePort, SandboxId};
