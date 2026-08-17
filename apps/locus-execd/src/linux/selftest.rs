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
//! tenu. Code 0 devient [`Observed::Succeeded`]. Le sens inverse aurait fait d'une commande qui
//! échoue une preuve d'isolation.
//!
//! # Le code non nul ne suffit pas, et W4.d.3 s'était trompé
//!
//! W4.d.3 lisait **tout** code non nul comme [`Observed::Blocked`], et déclarait au ledger que six
//! sondes visant des binaires absents de l'image « échouent en 127 et se lisent comme des
//! blocages ». C'était noté comme une dette. C'en était une, mais dans le mauvais sens : une sonde
//! absente rendait le backend **plus** digne de confiance, puisque `Blocked` est exactement ce
//! qu'un niveau promet. Une image incomplète produisait un `Trusted` que personne n'avait mérité.
//!
//! Les codes que le shell et le runtime réservent à « je n'ai pas pu lancer » sont donc lus comme
//! [`Observed::NotRun`] : 127 pour une commande introuvable, 126 pour une commande non exécutable,
//! 125 pour un runtime qui n'a pas su démarrer le conteneur. Ce sont les trois seuls codes que
//! POSIX et Podman réservent, et les distinguer est exactement la différence entre « la sonde a été
//! contenue » et « la sonde n'existe pas ».
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
    ("exceed_cpu_quota", &["sh", "-c", CPU_QUOTA]),
    ("exceed_memory_quota", &["sh", "-c", MEMORY_QUOTA]),
    ("exceed_pid_quota", &["sh", "-c", PID_QUOTA]),
    ("exceed_disk_quota", &["sh", "-c", DISK_QUOTA]),
    ("open_outbound_connection", &["sh", "-c", EGRESS]),
    ("reach_cloud_metadata_service", &["sh", "-c", METADATA]),
    (
        "reach_host_kernel_interfaces",
        &["sh", "-c", "head -c 1 /sys/kernel/vmcoreinfo"],
    ),
];

/// Les codes de sortie qui disent « je n'ai pas pu lancer », et ce qu'ils disent.
///
/// POSIX réserve 126 et 127 au shell, Podman réserve 125 à son propre échec. Aucun des trois n'est
/// un verdict sur le confinement, et les lire comme tel ferait d'une image incomplète une preuve
/// d'isolation.
pub const UNRUNNABLE_EXIT_CODES: [(i32, &str); 3] = [
    (
        125,
        "le runtime n'a pas su démarrer la commande dans la sandbox",
    ),
    (126, "la sonde existe mais n'est pas exécutable"),
    (127, "la sonde est absente de l'image"),
];

/// Le code qu'une sonde rend quand elle n'a pas pu conclure.
///
/// # Pourquoi une sonde a besoin de le dire
///
/// Une sonde lit parfois quelque chose qui n'est pas là — `cpu.stat` absent, `/dev/zero`
/// illisible. Sans code réservé, elle rendrait un code non nul ordinaire, que le harnais lirait
/// comme un blocage, c'est-à-dire comme une preuve d'isolation. C'est le même piège que le 127 de
/// W5.c, une couche plus bas : cette fois ce n'est pas la sonde qui manque, c'est ce dont la sonde
/// avait besoin.
///
/// 120 est hors des plages que POSIX (126, 127), les signaux (128+) et Podman (125) réservent.
pub const INCONCLUSIVE_EXIT_CODE: i32 = 120;

/// La raison qu'un code de sortie porte, quand il dit que rien n'a été lancé.
#[must_use]
pub fn unrunnable(code: i32) -> Option<&'static str> {
    if code == INCONCLUSIVE_EXIT_CODE {
        return Some("la sonde n'a pas pu conclure : ce qu'elle devait lire n'était pas là");
    }
    UNRUNNABLE_EXIT_CODES
        .into_iter()
        .find(|(reserved, _)| *reserved == code)
        .map(|(_, reason)| reason)
}

/// Les cinq sondes que la version précédente confiait à des binaires de l'image.
///
/// # Pourquoi elles voyagent avec le harnais
///
/// W4.d.3 les visait à `/usr/libexec/locus/probe-*`, et W5.c a montré ce que coûtait leur absence :
/// un code 127 lu comme un blocage, donc une image incomplète plus flatteuse qu'une image
/// complète. Le correctif de W5.c rendait l'absence **visible** ; celui-ci la rend **impossible**.
/// Une sonde embarquée dans le harnais est en outre versionnée avec le code qui la juge : une image
/// construite il y a six mois est éprouvée par la suite d'aujourd'hui, ce qui est le bon sens de la
/// dépendance.
///
/// Chacune rend `0` quand la sonde **réussit**, c'est-à-dire quand le confinement n'a pas tenu, et
/// [`INCONCLUSIVE_EXIT_CODE`] quand ce qu'elle devait lire n'était pas là.
///
/// # Ce que ces scripts n'ont pas encore
///
/// Une validation contre une sandbox réelle. Leur syntaxe est vérifiée — un `sh -n` par sonde, dans
/// les tests — mais leur **sémantique** ne l'est pas : rien ici ne prouve que `nr_throttled` bouge
/// quand `cpu.max` mord. C'est écrit au ledger comme dette nommée, et c'est le premier travail d'un
/// hôte capable de S2.
const CPU_QUOTA: &str = concat!(
    "s=/sys/fs/cgroup/cpu.stat; [ -r \"$s\" ] || exit 120; ",
    "before=$(awk '/nr_throttled/{print $2}' \"$s\"); ",
    "end=$(( $(date +%s) + 2 )); while [ \"$(date +%s)\" -lt \"$end\" ]; do :; done; ",
    "after=$(awk '/nr_throttled/{print $2}' \"$s\"); ",
    "[ \"${after:-0}\" -eq \"${before:-0}\" ]",
);

const MEMORY_QUOTA: &str = concat!(
    "s=/sys/fs/cgroup/memory.max; [ -r \"$s\" ] || exit 120; ",
    "limit=$(cat \"$s\"); [ \"$limit\" = max ] && exit 0; ",
    "block=$(( limit / 1048576 + 8 )); ",
    "dd if=/dev/zero of=/dev/shm/locus-probe bs=1M count=\"$block\" 2>/dev/null",
);

const PID_QUOTA: &str = concat!(
    "s=/sys/fs/cgroup/pids.max; [ -r \"$s\" ] || exit 120; ",
    "limit=$(cat \"$s\"); [ \"$limit\" = max ] && exit 0; ",
    "i=0; while [ \"$i\" -le \"$limit\" ]; do sleep 5 & i=$(( i + 1 )); done",
);

const DISK_QUOTA: &str = concat!(
    "dd if=/dev/zero of=/locus-probe-disk bs=1M count=64 2>/dev/null || exit 1; ",
    "rm -f /locus-probe-disk",
);

const EGRESS: &str = concat!(
    "command -v curl >/dev/null || exit 120; ",
    "curl -sS -m 5 -o /dev/null http://example.org/",
);

const METADATA: &str = concat!(
    "command -v curl >/dev/null || exit 120; ",
    "curl -sS -m 5 -o /dev/null http://169.254.169.254/",
);

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
        Ok(execution) => unrunnable(execution.code)
            .map_or(Observed::Blocked, |reason| Observed::NotRun { reason }),
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
