//! Les projections reconstructibles — `docs/SPEC_V1.md` §9.3 et §9.5.
//!
//! # Ce qu'une projection est ici
//!
//! §9.1 : « les vecteurs, index plein texte, vues matérialisées, graph databases et caches sont des
//! projections **reconstruisibles** ». Le mot porte tout : une projection qu'on ne saurait pas
//! reconstruire serait une seconde source de vérité, et §9.1 réserve ce rôle au journal.
//!
//! D'où la forme du port : `reset` est dans le trait, et `checksum` avec lui. La première oblige
//! chaque projection à répondre à « peux-tu être détruite ? » ; la seconde donne à
//! [`verify::verify`] de quoi comparer une reconstruction à l'état courant, ce que §9.5 demande.
//!
//! # La quarantaine ne bloque pas l'écriture
//!
//! §9.5 : « les erreurs de projection sont mises en quarantaine sans bloquer l'écriture
//! canonique ». Ici, la promesse tient **par la forme** : [`runner::ProjectionRunner`] reçoit le
//! journal par référence partagée et n'a aucun chemin d'écriture. Une projection en défaut ne peut
//! pas empêcher un append parce qu'il n'existe pas de moyen par lequel elle l'atteindrait.
//!
//! Le cas réservé par le texte — « sauf si elles concernent une projection synchrone nécessaire à
//! un invariant » — n'est pas implémenté : aucune projection de ce paquet n'est synchrone, et
//! écrire le mécanisme avant d'avoir le cas produirait une abstraction que rien ne teste.
//!
//! # Cinq projections, pas douze
//!
//! §9.3 en liste douze. Ce paquet en porte cinq : « état de validation » et « registre des
//! conflits » d'abord — celles que le domaine de W1.a et W1.b permettait d'écrire honnêtement —,
//! puis les deux graphes d'exécution et d'organisation, et depuis `W20.u` le graphe épistémique.
//!
//! Le compte est écrit ici parce qu'il se périme : cette phrase a dit « deux » pendant que le
//! paquet en portait quatre. Une projection s'ajoute quand des faits existent à projeter, jamais
//! avant — les sept qui manquent attendent les leurs.

pub mod conflict_registry;
pub mod epistemic_graph;
pub mod execution_graph;
pub mod organisation_graph;
pub mod projection;
pub mod runner;
pub mod validation_state;
pub mod verify;

pub use conflict_registry::{ConflictEntry, ConflictRegistry};
pub use epistemic_graph::{
    ArtifactRecord, Cost, Dossier, EpistemicGraph, Experiment, Objection, Unreadable,
};
pub use execution_graph::{Edge, EdgeKind, ExecutionGraph, NodeKind};
pub use organisation_graph::{AssignmentRecord, OrganisationGraph};
pub use projection::{Projection, ProjectionError, Watermark};
pub use runner::{Health, Progress, ProjectionRunner};
pub use validation_state::{ObjectState, ValidationState};
pub use verify::{VerifyReport, verify};
