//! Passer la suite de W4.b contre le backend, et en tirer un `Standing`.
//!
//! # Ce que ce module ajoute à `locus_execution::selftest`
//!
//! La suite dit **ce qu'il faut tenter** et **à quel niveau ça doit échouer**. Elle ne dit pas
//! comment le tenter : `Probe` porte un nom, une dimension et un niveau, pas une commande. C'est
//! délibéré — la façon de tenter dépend du backend, et une commande dans le crate de vocabulaire
//! aurait supposé un Linux.
//!
//! Ce module fournit la moitié manquante pour le backend rootless : une commande par sonde, et de
//! quoi lire son code de sortie.
//!
//! # La convention de sortie, et pourquoi elle est dans ce sens
//!
//! Chaque commande **réussit quand la sonde réussit**, c'est-à-dire quand le confinement n'a pas
//! tenu. Code 0 devient [`Observed::Succeeded`], code non nul devient [`Observed::Blocked`]. Le
//! sens inverse aurait fait d'une commande absente de l'image — `sh` introuvable, code 127 — une
//! preuve d'isolation.
//!
//! # Ce qui n'a pas pu être lancé n'a rien prouvé
//!
//! Quand le runtime ne répond pas, la sonde est [`Observed::NotRun`] avec sa raison, jamais
//! `Blocked`. `standing` en fait un `Inconclusive`, et `Verdict::denies_trust` refuse la confiance
//! sur une sonde critique — les seize le sont. Un hôte sans Podman ne peut donc pas obtenir de
//! `Trusted` par absence de contre-preuve.

use locus_execution::{Observed, SUITE, SandboxLevel, Standing, standing};

use super::driver::PodmanBackend;
use super::driver::Runner;
use crate::runtime::{RuntimePort, SandboxId};

/// La commande qui tente une sonde, par nom de sonde.
///
/// Les seize sondes de `SUITE` sont couvertes, et un test l'affirme dans les deux sens : pas de
/// sonde sans commande, pas de commande orpheline. Une sonde sans commande serait silencieusement
/// absente du rapport, et `standing` la rendrait `Inconclusive` sans que personne sache pourquoi.
pub const PROBE_COMMANDS: [(&str, &[&str]); 16] = [
    (
        "write_outside_workspace",
        &["sh", "-c", "touch /locus-probe-outside"],
    ),
    (
        "write_host_home",
        &["sh", "-c", "touch /host-home/locus-probe"],
    ),
    (
        "persist_after_teardown",
        &["sh", "-c", "touch /locus-probe-persist"],
    ),
    (
        "read_host_filesystem",
        &["sh", "-c", "head -c 1 /host-root/etc/hostname"],
    ),
    (
        "read_host_secret_files",
        &["sh", "-c", "head -c 1 /host-root/etc/shadow"],
    ),
    (
        "read_process_environment",
        &["sh", "-c", "head -c 1 /proc/1/environ"],
    ),
    (
        "access_container_runtime_socket",
        &["sh", "-c", "test -S /run/podman/podman.sock"],
    ),
    ("escalate_to_root", &["sh", "-c", "su -c true root"]),
    (
        "observe_host_processes",
        &["sh", "-c", "test \"$(ls /proc | grep -c '^[0-9]')\" -gt 50"],
    ),
    (
        "exceed_cpu_quota",
        &["sh", "-c", "exec /usr/libexec/locus/probe-cpu"],
    ),
    (
        "exceed_memory_quota",
        &["sh", "-c", "exec /usr/libexec/locus/probe-memory"],
    ),
    (
        "exceed_pid_quota",
        &["sh", "-c", "exec /usr/libexec/locus/probe-pids"],
    ),
    (
        "exceed_disk_quota",
        &["sh", "-c", "exec /usr/libexec/locus/probe-disk"],
    ),
    (
        "open_outbound_connection",
        &["sh", "-c", "exec /usr/libexec/locus/probe-egress"],
    ),
    (
        "reach_cloud_metadata_service",
        &[
            "sh",
            "-c",
            "exec /usr/libexec/locus/probe-egress 169.254.169.254",
        ],
    ),
    (
        "reach_host_kernel_interfaces",
        &["sh", "-c", "head -c 1 /sys/kernel/vmcoreinfo"],
    ),
];

/// La raison inscrite quand le runtime n'a pas répondu.
///
/// `Observed::NotRun` porte un `&'static str` : la raison est donc une constante et non le message
/// de l'erreur. Ce n'est pas une perte — ce qu'il faut savoir est *qu'on n'a pas su lancer*, et le
/// détail vit dans l'erreur que [`run_suite`] rend par ailleurs au journal.
pub const UNREACHABLE_RUNTIME: &str = "le runtime n'a pas exécuté la sonde";

/// La commande d'une sonde, si elle en a une.
#[must_use]
pub fn probe_command(name: &str) -> Option<&'static [&'static str]> {
    PROBE_COMMANDS
        .into_iter()
        .find(|(probe, _)| *probe == name)
        .map(|(_, command)| command)
}

/// Les arguments de `podman exec` qui tentent cette sonde.
#[must_use]
pub fn exec_arguments(id: &SandboxId, command: &[&str]) -> Vec<String> {
    let mut arguments = vec!["exec".to_owned(), id.as_str().to_owned()];
    arguments.extend(command.iter().map(|part| (*part).to_owned()));
    arguments
}

/// Tenter les seize sondes dans une sandbox qui tourne, et rendre ce qu'elles ont produit.
///
/// L'ordre est celui de `SUITE`, et chaque sonde apparaît exactement une fois — y compris celles
/// qui n'ont pas pu être lancées. Une suite tronquée se lirait comme une suite passée.
pub fn run_suite<R: Runner>(
    backend: &PodmanBackend<R>,
    id: &SandboxId,
) -> Vec<(&'static str, Observed)> {
    SUITE
        .iter()
        .map(|probe| (probe.name, attempt(backend, id, probe.name)))
        .collect()
}

fn attempt<R: Runner>(backend: &PodmanBackend<R>, id: &SandboxId, name: &str) -> Observed {
    let Some(command) = probe_command(name) else {
        return Observed::NotRun {
            reason: "aucune commande n'est associée à cette sonde",
        };
    };
    match backend.runner().run(&exec_arguments(id, command)) {
        Ok(execution) if execution.code == 0 => Observed::Succeeded,
        Ok(_) => Observed::Blocked,
        Err(_) => Observed::NotRun {
            reason: UNREACHABLE_RUNTIME,
        },
    }
}

/// Ce que le backend a le droit d'annoncer à ce niveau, après passage de la suite.
pub fn assess<R: Runner>(
    backend: &PodmanBackend<R>,
    id: &SandboxId,
    level: SandboxLevel,
) -> Standing {
    standing(level, &run_suite(backend, id))
}

/// Créer, démarrer et éprouver une sandbox à ce niveau, puis l'arrêter.
///
/// La sandbox est arrêtée **même quand la suite s'est mal passée** : une sonde qui a échoué laisse
/// derrière elle un conteneur qui tourne, et un hôte qui accumule des conteneurs d'épreuve finit
/// par ne plus pouvoir en créer.
///
/// # Errors
///
/// L'erreur du runtime, quand la sandbox n'a pas pu être créée ou démarrée. Une sandbox qui n'a pas
/// démarré n'a rien à éprouver, et rendre un `Standing` sur zéro observation serait rendre un
/// verdict sur rien.
pub fn certify<R: Runner>(
    backend: &mut PodmanBackend<R>,
    spec: &locus_execution::SandboxSpec,
    level: SandboxLevel,
) -> Result<Standing, crate::runtime::RuntimeError> {
    let id = backend.create(spec)?;
    backend.start(&id)?;
    let verdict = assess(backend, &id, level);
    let _ = backend.stop(&id);
    Ok(verdict)
}
