//! Les objets de coordination — `docs/SPEC_V1.md` §7.1 et §14, ADR 0016.
//!
//! # Pourquoi un crate séparé
//!
//! ADR 0016, décision 1 : les agrégats de coordination vivent hors de `packages/graph`, et la
//! sixième frontière vérifiée par la CI l'impose dans les deux sens. Le graphe épistémique dit ce
//! qui est cru et pourquoi ; ces objets-ci disent **qui travaille**. Les mélanger ferait dépendre
//! une revue de la topologie d'une équipe, ce qui est précisément ce que l'invariant 11 refuse.
//!
//! # Le vocabulaire est celui de la spec, sous son nom
//!
//! `CLAUDE.md` : « les objets d'organisation, de coordination et de gouvernance sont ceux de
//! `SPEC_V1.md` §7.1, §13, §16, §20 et §22, **sous leur nom**. Aucun vocabulaire parallèle. » Ce
//! crate ne porte donc que des noms de la spec, et chaque énumération reprend la liste de la
//! section qui la définit — pas une liste voisine qui aurait l'air équivalente.
//!
//! # Une seule sorte de relation
//!
//! ADR 0016, décision 4 : une sorte de relation n'entre dans son énumération que lorsqu'un
//! consommateur exécutable et testé existe. `review` en a un — l'indépendance de §14.4 et
//! l'invariant 11 s'y appuient — et c'est la seule. `mentors`, `delegates_to`, `supervises`
//! seraient du vocabulaire que rien ne vérifie.

pub mod agent;
pub mod barrier;
pub mod capability;
pub mod decision;
pub mod diff;
pub mod lifecycle;
pub mod messaging;
pub mod metrics;
pub mod objection;
pub mod proposal;
pub mod region;
pub mod simulation;
pub mod task;
pub mod team;
pub mod version;
pub mod visibility;

pub use agent::{AgentError, AgentInstance, AgentTemplate, InstanceState, TemplateStatus};
pub use barrier::{Barrier, BarrierError, Barriers, Passage, threatened_by};
pub use capability::{Capability, CapabilityError, Source, Sources, capabilities};
pub use decision::{ApprovalRequest, ApprovalState, Decision, DecisionError, DecisionState};
pub use diff::{Diff, DiffError};
pub use lifecycle::{
    Command, Lifecycle, LifecycleError, Outcome, Quiescence, may_leave_the_version,
};
// `messaging::Message` et `messaging::Reception` ne sont **pas** remontés ici, pour la raison qui
// vaut déjà pour `simulation` : « message » et « réception » sont des mots que d'autres couches
// emploient — `locus_event_store` a ses enveloppes, `lep` a ses trames — et un nom court remonté à
// la racine ferait croire à un type unique là où il y en a plusieurs, chacun juste dans sa couche.
// Le chemin de module porte la distinction sans coûter un nom.
pub use messaging::{EpochError, Epochs, Handover, HandoverError};
pub use metrics::Metrics;
pub use objection::{Contestable, ObjectedTo, Objection, ObjectionError, Remedy};
pub use proposal::{
    Approved, Author, Change, Committed, EpistemicIndex, Justification, Mode, Proposal,
    ProposalError, Relation, RelationKind, approve, commit,
};
pub use region::{
    Acceptance, ApprovalMode, Invariant, Refusal, Region, RegionError, Verdict, threatens,
};
// `simulation::Verdict`, `simulation::Outcome` et `simulation::run` ne sont **pas** remontés
// ici : `region` et `lifecycle` disent déjà « verdict » et « outcome » de leur propre domaine,
// et aplatir les trois forcerait à renommer celui qui perdrait le mot juste. Le chemin de
// module porte la distinction sans coûter un nom.
pub use simulation::{Answer, Fidelity, Recorded};
pub use task::{Assignment, Task, TaskError};
pub use team::{CoordinationMode, Team, TeamError};
pub use version::{ContentDigest, Digest, Operation, Undo, Version, VersionError, VersionId};
pub use visibility::Visibility;
