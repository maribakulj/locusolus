//! Le backend Linux rootless — W4.d.
//!
//! # Ce que ce module fait, et ce qu'il ne fait pas encore
//!
//! Il **traduit** et il **lit**. La traduction ([`plan`]) rend, pour une spécification de mission,
//! la liste exacte de ce qu'un conteneur rootless devra appliquer. La lecture ([`probe`]) va
//! demander au noyau ce qu'il permet, plutôt que de le supposer.
//!
//! Il **construit** ensuite ce qu'il demandera au runtime ([`invocation`]) et le **lance**
//! ([`driver`]). L'ordre est celui d'ADR 0012 — le port avant le driver — et d'ADR 0015 — la
//! traduction avant le fil. Il vaut ici plus qu'ailleurs : le plan de rollback d'ADR 0004 dit
//! qu'il n'y a « aucun chemin de repli acceptable », parce qu'un raccourci sur ce sujet-là produit
//! un sandbox factice, c'est-à-dire une garantie qu'on croit avoir.
//!
//! # Ce que le driver n'a pas le droit de faire
//!
//! Composer son attestation à partir de ce qu'il a demandé. Le niveau attesté est dérivé des
//! **observations** rendues par le runtime, et c'est ce qui rend un downgrade visible plutôt que
//! silencieux.
//!
//! # Le plafond est une constante, pas une ambition
//!
//! Un conteneur rootless au réseau isolé est `S3`. `S4` est une micro-VM, `S5` une enclave
//! distante : ce module refuse de les revendiquer, et le refus les nomme.

pub mod bubblewrap;
pub mod bubblewrap_driver;
pub mod campaign;
pub mod driver;
pub mod invocation;
pub mod plan;
pub mod probe;
pub mod process;
pub mod seccomp;
pub mod selftest;

pub use bubblewrap_driver::{BubblewrapBackend, host_namespaces};
pub use campaign::ProbeHost;
pub use driver::{
    BACKEND, FIRST_LAUNCH_PAUSE, PodmanBackend, RUNNING_TEMPLATE, boot_id_from, host_boot_id,
};
pub use invocation::{
    INSPECTED_FIELDS, InvocationError, SeccompProfiles, Workload, create_arguments,
    inspect_arguments,
};
pub use plan::{
    BACKEND_CEILING, CPU_PERIOD_MICROSECONDS, CgroupLimit, ConfinementPlan, DANGEROUS_CAPABILITIES,
    MountPlan, Namespace, NetworkPosture, PlanError, QuotaTarget, REMAINING_CAPABILITIES,
    SeccompPosture, plan,
};
pub use probe::{
    HostFacts, LocalReader, Missing, NO_STORAGE_DECLARED, PROJECT_QUOTA_OPTIONS,
    QUOTA_CAPABLE_FILESYSTEMS, REQUIRED_CONTROLLERS, Reader, Support,
};
pub use process::{CALL_BUDGET, Execution, Runner, SystemRunner};
pub use seccomp::{MUST_DENY, ProfileError, RestrictedProfile};
pub use selftest::{
    BOOT_ID_PATH_VARIABLE, HOST_BOOT_ID_VARIABLE, INCONCLUSIVE_EXIT_CODE, LAUNCH_ATTEMPTS,
    PROBE_COMMANDS, ProbeContext, QUOTA_BYTES_VARIABLE, QUOTA_OVERSHOOT_MIB, QUOTA_TARGET_VARIABLE,
    SANDBOX_GONE, SANDBOX_REFUSED, TRANSIENT_EXIT_CODES, Trial, UNREACHABLE_RUNTIME,
    UNREACHABLE_TARGET_EXIT_CODE, UNRUNNABLE_EXIT_CODES, UNWRITABLE_TARGET_EXIT_CODE, certify,
    exec_arguments, probe_command, run_suite, unrunnable, verdicts,
};
