//! Le backend Linux rootless — W4.d.
//!
//! # Ce que ce module fait, et ce qu'il ne fait pas encore
//!
//! Il **traduit** et il **lit**. La traduction ([`plan`]) rend, pour une spécification de mission,
//! la liste exacte de ce qu'un conteneur rootless devra appliquer. La lecture ([`probe`]) va
//! demander au noyau ce qu'il permet, plutôt que de le supposer.
//!
//! Il ne lance aucun processus et n'ouvre aucun socket. C'est la forme d'ADR 0012 — le port avant
//! le driver — et celle d'ADR 0015 — la traduction avant le fil. Elle vaut ici plus qu'ailleurs :
//! le plan de rollback d'ADR 0004 dit qu'il n'y a « aucun chemin de repli acceptable », parce
//! qu'un raccourci sur ce sujet-là produit un sandbox factice, c'est-à-dire une garantie qu'on
//! croit avoir. Un driver écrit avant sa traduction confinerait de travers en silence.
//!
//! # Le plafond est une constante, pas une ambition
//!
//! Un conteneur rootless au réseau isolé est `S3`. `S4` est une micro-VM, `S5` une enclave
//! distante : ce module refuse de les revendiquer, et le refus les nomme.

pub mod plan;
pub mod probe;

pub use plan::{
    BACKEND_CEILING, CPU_PERIOD_MICROSECONDS, CgroupLimit, ConfinementPlan, DANGEROUS_CAPABILITIES,
    MountPlan, Namespace, NetworkPosture, PlanError, REMAINING_CAPABILITIES, SeccompPosture, plan,
};
pub use probe::{HostFacts, Missing, REQUIRED_CONTROLLERS, Support};
