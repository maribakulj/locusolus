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
        &["sh", "-c", PROCESS_ENVIRONMENT],
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
/// POSIX réserve 126 et 127 au shell, Podman réserve 125 et 255 à ses propres échecs. Aucun des
/// quatre n'est un verdict sur le confinement, et les lire comme tel ferait d'une image incomplète
/// une preuve d'isolation.
///
/// # 255 a coûté quatre faux sur-confinements
///
/// `W5.m` a mis le code de sortie à côté du verdict, et la table du premier passage a montré le
/// motif d'un coup : **toutes** les sondes situées après `exceed_pid_quota` rendaient 255.
/// Celle-ci sature délibérément le quota de PID en forkant jusqu'au plafond, et `podman exec` ne
/// peut plus forker tant que le cgroup n'a pas rendu ses processus — il abandonne alors avec son
/// code générique.
///
/// Les quatre sondes suivantes étaient donc rapportées « bloquées », c'est-à-dire **contenues**,
/// alors qu'elles n'avaient pas tourné du tout. Trois d'entre elles produisaient un
/// « sur-confinement » qui n'existait pas, et la quatrième — `exceed_disk_quota` — un « tient »
/// qu'elle n'avait pas mérité. Aucune sonde de la suite ne sort volontairement en 255.
///
/// Le catalogage ne suffit pas à rendre ces sondes mesurables : il les fait passer de « fausse
/// preuve » à « aveu d'ignorance », ce qui est la seule des deux valeurs qu'on ait le droit
/// d'écrire. Rendre la suite insensible à cette contamination est le sujet de `W5.o`.
pub const UNRUNNABLE_EXIT_CODES: [(i32, &str); 4] = [
    (
        125,
        "le runtime n'a pas su démarrer la commande dans la sandbox",
    ),
    (126, "la sonde existe mais n'est pas exécutable"),
    (127, "la sonde est absente de l'image"),
    (
        255,
        "le runtime a rendu son code d'erreur générique : la commande n'a pas été lancée",
    ),
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

/// Le code qu'une sonde rend quand **ce qu'elle voulait atteindre** n'a pas répondu.
///
/// # Une ignorance de plus, et elle ne se confond pas avec les autres
///
/// [`INCONCLUSIVE_EXIT_CODE`] dit « ce que je devais **lire** n'était pas là » — un `cpu.stat`
/// absent, un `curl` manquant. Celui-ci dit « ce que je devais **atteindre** n'a pas répondu » : un
/// hôte dont le réseau n'ouvre pas la route, un service que le déploiement filtre. Les deux sont
/// des ignorances, aucune n'est un blocage, et elles ne se réparent pas pareil — la première en
/// complétant l'image, la seconde en changeant d'hôte ou en renonçant à la mesure.
///
/// # Pourquoi il a fallu un hôte réel pour le trouver
///
/// `W5.f` a fait tourner la suite dans un conteneur rootless. Trois sondes « permises à `S2` » sont
/// ressorties **bloquées**, c'est-à-dire lues comme une preuve d'isolation, alors que le réseau de
/// l'hôte ne menait simplement nulle part. C'est le piège du 127 de `W5.c` et du 120 de `W5.d`, une
/// couche plus loin : cette fois ce n'est ni la sonde ni ce qu'elle lisait qui manque.
///
/// 121 reste hors des plages réservées, comme 120.
pub const UNREACHABLE_TARGET_EXIT_CODE: i32 = 121;

/// La raison qu'un code de sortie porte, quand il dit que rien n'a été lancé.
#[must_use]
pub fn unrunnable(code: i32) -> Option<&'static str> {
    if code == INCONCLUSIVE_EXIT_CODE {
        return Some("la sonde n'a pas pu conclure : ce qu'elle devait lire n'était pas là");
    }
    if code == UNREACHABLE_TARGET_EXIT_CODE {
        return Some("la sonde n'a pas pu conclure : ce qu'elle devait atteindre n'a pas répondu");
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

/// # Elle rend ce qu'elle a pris, quand elle en a l'occasion
///
/// La version précédente forkait des `sleep 5` et sortait sans les attendre : le cgroup restait
/// saturé cinq secondes, et `podman exec` ne pouvait plus forker pendant ce temps — soit, sur un
/// runner, les quatre sondes suivantes. Elles étaient rapportées **bloquées**, donc contenues,
/// alors qu'elles n'avaient pas tourné.
///
/// Les enfants sont donc tués puis attendus. Ce n'est pas une garantie : si le shell lui-même
/// meurt de ne pouvoir forker, le nettoyage ne tourne pas, et c'est pourquoi `W5.o` reste dû. Mais
/// sur le chemin où la sonde va au bout, elle ne laisse plus rien derrière elle.
const PID_QUOTA: &str = concat!(
    "s=/sys/fs/cgroup/pids.max; [ -r \"$s\" ] || exit 120; ",
    "limit=$(cat \"$s\"); [ \"$limit\" = max ] && exit 0; ",
    "i=0; pids=''; ",
    "while [ \"$i\" -le \"$limit\" ]; do sleep 5 & pids=\"$pids $!\"; i=$(( i + 1 )); done; ",
    "status=$?; ",
    "kill $pids 2>/dev/null; wait 2>/dev/null; ",
    "exit \"$status\"",
);

const DISK_QUOTA: &str = concat!(
    "dd if=/dev/zero of=/locus-probe-disk bs=1M count=64 2>/dev/null || exit 1; ",
    "rm -f /locus-probe-disk",
);

/// Lire l'environnement d'un processus que **la sandbox n'a pas créé**.
///
/// # Ce que la version précédente mesurait, et pourquoi c'était l'inverse
///
/// Elle lisait `/proc/1/environ`. Dans un namespace PID — que `S2` pose — `/proc/1` est l'init **du
/// conteneur**, c'est-à-dire le workload lui-même, appartenant à l'utilisateur mappé. La lecture
/// réussissait donc, et elle réussissait d'autant plus sûrement que le namespace faisait son
/// travail : la sonde ne pouvait pas échouer sur un hôte correctement confiné. Comme elle est
/// `critical`, tout hôte bien configuré se voyait refuser la confiance. `W5.f` l'a montré au
/// premier passage réel.
///
/// # Ce que celle-ci mesure
///
/// La dimension est `HostSecret`, et le motif dit « l'environnement d'un **autre** processus ».
/// « Autre » veut dire : hors de cette sandbox. Le discriminant est le cgroup — `S2` pose aussi un
/// namespace cgroup, donc tout ce que le conteneur voit porte le même chemin que lui. Un processus
/// dont le cgroup diffère du nôtre est, par construction, un processus que nous n'avons pas créé.
///
/// Sans namespace PID, les processus de l'hôte sont visibles avec leurs cgroups propres, et la
/// sonde en trouve un : elle réussit, comme le niveau le permet. Avec, il n'y a plus rien
/// d'étranger à lire, et **c'est cela, le confinement**.
const PROCESS_ENVIRONMENT: &str = concat!(
    "mine=$(cut -d: -f3 /proc/self/cgroup 2>/dev/null | head -n 1); ",
    "[ -n \"$mine\" ] || exit 120; ",
    "for entry in /proc/[0-9]*; do ",
    "theirs=$(cut -d: -f3 \"$entry/cgroup\" 2>/dev/null | head -n 1); ",
    "[ -n \"$theirs\" ] || continue; ",
    "[ \"$theirs\" = \"$mine\" ] && continue; ",
    "head -c 1 \"$entry/environ\" >/dev/null 2>&1 && exit 0; ",
    "done; exit 1",
);

/// Y a-t-il une route par défaut ? Sans elle, c'est le namespace réseau qui contient.
///
/// C'est le discriminant que les deux sondes réseau n'avaient pas. `S3` s'appelle
/// `container-isolated-network` : ce qu'il contient **est** le namespace, et un namespace réseau
/// vide n'a pas de route par défaut. Un `curl` qui échoue sans cette vérification ne distingue pas
/// « la sandbox a coupé le réseau » de « l'hôte ne mène nulle part », et le harnais lisait le
/// second comme une preuve d'isolation.
///
/// `/proc/net/route` plutôt qu'`ip route` : le fichier existe toujours, le binaire non — et une
/// sonde ne doit rien attendre de l'image (`W5.d`).
///
/// Une macro plutôt qu'une constante : `concat!` n'accepte que des littéraux, et les deux sondes
/// doivent partager ce fragment plutôt que d'en tenir deux copies qui divergeront.
macro_rules! default_route {
    () => {
        concat!(
            "[ -r /proc/net/route ] || exit 120; ",
            "awk 'NR>1 && $2 == \"00000000\" { found = 1 } END { exit !found }' /proc/net/route \
             || exit 1; ",
        )
    };
}

const EGRESS: &str = concat!(
    "command -v curl >/dev/null || exit 120; ",
    default_route!(),
    "curl -sS -m 5 -o /dev/null http://example.org/ && exit 0; ",
    "exit 121",
);

const METADATA: &str = concat!(
    "command -v curl >/dev/null || exit 120; ",
    default_route!(),
    "curl -sS -m 5 -o /dev/null http://169.254.169.254/ && exit 0; ",
    "exit 121",
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

/// Ce qu'une sonde a rendu, et ce que le harnais en a conclu.
///
/// # Deux choses, jamais une
///
/// [`Observed`] a trois valeurs, et c'est le bon compte pour un **verdict** : réussie, bloquée, pas
/// lancée. Mais trois valeurs ne suffisent pas à *diagnostiquer*, parce que plusieurs codes de
/// sortie très différents tombent dans « bloquée ». Quand `open_outbound_connection` est ressortie
/// bloquée sur un hôte dont un autre test montrait la route par défaut, rien ne permettait de dire
/// **où** la sonde s'était arrêtée : au constat de route, à `curl`, ou avant.
///
/// Le code brut voyage donc **à côté** du verdict, jamais dedans. L'y mettre ferait entrer un
/// détail de Podman dans le vocabulaire de `packages/execution`, qui ne connaît pas de runtime ;
/// et un verdict à quatre-vingt-dix valeurs n'est plus un verdict.
///
/// `code` est `Option` parce qu'un runtime qui n'a pas répondu **n'a pas** de code de sortie.
/// Inventer un `-1` produirait une valeur que quelqu'un finirait par lire comme un vrai code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trial {
    name: &'static str,
    observed: Observed,
    code: Option<i32>,
}

impl Trial {
    /// La sonde.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Ce que le harnais en a conclu.
    #[must_use]
    pub const fn observed(&self) -> Observed {
        self.observed
    }

    /// Le code que la commande a rendu, s'il y en a eu un.
    #[must_use]
    pub const fn code(&self) -> Option<i32> {
        self.code
    }

    /// Une sonde qui n'a pas tourné, et qui n'a donc **aucun** code.
    ///
    /// # Un constructeur, et pas deux affectations
    ///
    /// Il y a deux façons pour une sonde de ne pas tourner — aucune commande ne lui est associée, ou
    /// le runtime n'a pas répondu — et toutes deux doivent rendre `code: None`. Écrit deux fois, ce
    /// `None` est deux occasions de se tromper : la mutation l'a montré en prêtant un `127` à la
    /// première sans qu'aucun test ne morde, parce que ce chemin est **inatteignable** tant qu'aucune
    /// sonde n'est orpheline — ce qu'un autre test garantit.
    ///
    /// Passer par un constructeur ne rend pas ce chemin testable ; il rend la faute inexprimable
    /// sans réécrire ce constructeur, et le test qui couvre l'autre chemin garde alors les deux.
    const fn not_run(name: &'static str, reason: &'static str) -> Self {
        Self {
            name,
            observed: Observed::NotRun { reason },
            code: None,
        }
    }
}

/// Tenter les seize sondes dans une sandbox qui tourne, et rendre ce qu'elles ont produit.
///
/// L'ordre est celui de `SUITE`, et chaque sonde apparaît exactement une fois — y compris celles
/// qui n'ont pas pu être lancées. Une suite tronquée se lirait comme une suite passée.
pub fn run_suite<R: Runner>(backend: &PodmanBackend<R>, id: &SandboxId) -> Vec<Trial> {
    SUITE
        .iter()
        .map(|probe| attempt(backend, id, probe.name))
        .collect()
}

/// Les verdicts seuls, sous la forme que [`locus_execution::standing`] attend.
///
/// La conversion est explicite plutôt qu'implicite : `standing` juge, et juger se fait sur les trois
/// valeurs d'[`Observed`]. Lui passer les codes bruts l'inviterait à les regarder, et un jugement
/// qui dépendrait d'un code de Podman ne serait plus transposable à un autre runtime.
#[must_use]
pub fn verdicts(trials: &[Trial]) -> Vec<(&'static str, Observed)> {
    trials
        .iter()
        .map(|trial| (trial.name, trial.observed))
        .collect()
}

fn attempt<R: Runner>(backend: &PodmanBackend<R>, id: &SandboxId, name: &'static str) -> Trial {
    let Some(command) = probe_command(name) else {
        return Trial::not_run(name, "aucune commande n'est associée à cette sonde");
    };
    match backend.runner().run(&exec_arguments(id, command)) {
        Ok(execution) => Trial {
            name,
            observed: if execution.code == 0 {
                Observed::Succeeded
            } else {
                unrunnable(execution.code)
                    .map_or(Observed::Blocked, |reason| Observed::NotRun { reason })
            },
            code: Some(execution.code),
        },
        Err(_) => Trial::not_run(name, UNREACHABLE_RUNTIME),
    }
}

/// Ce que le backend a le droit d'annoncer à ce niveau, après passage de la suite.
pub fn assess<R: Runner>(
    backend: &PodmanBackend<R>,
    id: &SandboxId,
    level: SandboxLevel,
) -> Standing {
    standing(level, &verdicts(&run_suite(backend, id)))
}

/// Créer, démarrer et éprouver une sandbox à ce niveau, puis **la retirer**.
///
/// # Arrêter ne suffisait pas, et cette fonction le disait déjà à moitié
///
/// La rédaction précédente promettait que « la sandbox est arrêtée même quand la suite s'est mal
/// passée », avec la bonne raison : « un hôte qui accumule des conteneurs d'épreuve finit par ne
/// plus pouvoir en créer ». La raison était juste et la précaution insuffisante — `podman stop`
/// laisse le **nom** et la **couche inscriptible**, et c'est le nom qui manque au suivant. Trois
/// passages de CI ont échoué sur « the container name `locus-0001` is already in use » avant que
/// quiconque le remarque, et le harnais lisait cette erreur là où il attendait un verdict.
///
/// # Le démontage a lieu sur **tous** les chemins
///
/// Y compris celui où le démarrage échoue : la version précédente y rendait l'erreur par `?` et
/// abandonnait un conteneur créé mais jamais démarré, c'est-à-dire le cas le plus silencieux — rien
/// ne tournait, donc rien ne signalait la fuite, et le nom restait pris.
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
    if let Err(error) = backend.start(&id) {
        teardown(backend, &id);
        return Err(error);
    }
    let verdict = assess(backend, &id, level);
    teardown(backend, &id);
    Ok(verdict)
}

/// Arrêter puis retirer, en ignorant l'échec de l'un comme de l'autre.
///
/// Les erreurs sont écartées parce qu'un démontage est du **nettoyage** : le verdict qu'on est en
/// train de rendre porte sur le confinement, pas sur la capacité du runtime à ranger. Les masquer
/// serait grave si rien d'autre ne les voyait — mais un nom resté pris se signale au suivant, très
/// bruyamment, et c'est précisément comme cela que le défaut a été trouvé.
fn teardown<R: Runner>(backend: &mut PodmanBackend<R>, id: &SandboxId) {
    let _ = backend.stop(id);
    let _ = backend.remove(id);
}
