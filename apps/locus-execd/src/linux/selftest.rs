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

use std::thread;

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
    ("reach_host_kernel_interfaces", &["sh", "-c", HOST_KERNEL]),
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

/// La raison inscrite quand la sandbox elle-même a disparu.
///
/// # Une troisième ignorance, et elle n'est pas des deux autres
///
/// [`INCONCLUSIVE_EXIT_CODE`] dit « ce que je devais lire n'était pas là ».
/// [`UNREACHABLE_TARGET_EXIT_CODE`] dit « ce que je devais atteindre n'a pas répondu ». Celle-ci dit
/// **« il n'y avait rien pour me lancer »** — et elle se répare encore ailleurs : pas en complétant
/// l'image, pas en changeant d'hôte, mais en comprenant ce qui a tué la sandbox.
///
/// `W5.o` a fait retenter les lancements que le runtime refusait, en supposant la cause
/// **transitoire** — un cgroup occupé se libère. Le premier passage réel a démenti la supposition :
/// les trois dernières sondes rendaient toujours 255 après six tentatives étalées sur plus de six
/// secondes. Ce qui ne se libère pas n'était pas occupé.
///
/// Rapporter ces sondes comme « le runtime n'a pas pu les lancer » enverrait chercher un runtime
/// fatigué là où il n'y a plus de conteneur.
pub const SANDBOX_GONE: &str =
    "la sandbox ne tournait plus : il n'y avait rien pour lancer la sonde";

/// La raison inscrite quand la sandbox de cette sonde n'a pas pu être **ouverte**.
///
/// # Une quatrième ignorance, et elle ne se range avec aucune des trois autres
///
/// [`SANDBOX_GONE`] dit « il n'y avait plus rien pour me lancer » — la sandbox a existé, puis a
/// cessé. Celle-ci dit **« il n'y a jamais rien eu »** : l'ouverture a échoué, avant toute sonde.
/// Les deux se réparent ailleurs — la première en cherchant ce qui a tué la sandbox, la seconde en
/// lisant ce qui a fait échouer l'ouverture.
///
/// # Ce qu'elle dit, et ce qu'elle ne dit plus
///
/// Elle couvre le cas où le runtime **a répondu** et a refusé, et celui où le backend lui-même a
/// refusé avant de demander — un niveau hors de son plafond. Elle ne couvre **plus** le runtime
/// absent : `W5.s` a séparé `RuntimeError::Refused` d'`Unavailable`, et une sandbox qu'aucun
/// runtime n'a pu ouvrir faute de runtime rend [`UNREACHABLE_RUNTIME`].
///
/// La distinction a mis un sprint à devenir possible. `W5.r` l'a rendue nécessaire en faisant
/// remonter le motif jusqu'au rapport de sondes : un Podman tué y produisait « la sandbox a été
/// refusée », alors qu'il n'y avait eu aucun refus, seulement un silence. Le nom a d'abord été
/// élargi faute de pouvoir tenir la distinction ; il la tient maintenant.
///
/// # Ce qu'elle remplace
///
/// Tant que les seize sondes partageaient une sandbox, un échec d'ouverture était une **erreur** qui
/// interrompait tout : `certify` rendait un `Err` et le rapport était vide. Avec une sandbox par
/// sonde, le même échec rend seize absences nommées, chacune portant le message en clair. Le
/// rapport reste complet, et il dit pourquoi — ce qu'un `Err` ne faisait pas.
pub const SANDBOX_REFUSED: &str = "la sandbox de cette sonde n'a pas pu être ouverte";

/// Les codes réservés qui peuvent **passer**, par opposition à ceux qui ne passeront pas.
///
/// # Deux façons de ne pas avoir été lancé
///
/// 126 et 127 sont des propriétés de l'**image** : une sonde absente ne le sera pas moins à la
/// deuxième tentative, et réessayer ne ferait que retarder l'aveu. 125 et 255 sont des échecs du
/// **runtime au moment où il a essayé** : il n'a pas pu forker, il n'a pas su démarrer la commande.
/// Ceux-là peuvent tenir à ce que la sonde précédente était en train de faire.
///
/// `W5.n` a montré ce que coûte de ne pas distinguer : `exceed_pid_quota` saturait le quota de PID,
/// et les quatre sondes suivantes ne pouvaient plus être lancées. Le catalogage de 255 les a fait
/// passer de fausse preuve à aveu d'ignorance — c'était la bonne valeur, mais ce n'est toujours pas
/// une mesure.
///
/// Séparer les deux familles est ce qui permet de réessayer là où ça a un sens, et seulement là.
pub const TRANSIENT_EXIT_CODES: [i32; 2] = [125, 255];

/// Combien de fois une sonde est retentée quand le runtime n'a pas pu la lancer.
///
/// Les pauses doublent, et leur somme couvre la seconde près le pire cas connu : `exceed_pid_quota`
/// tient le cgroup le temps de ses `sleep 5` si son propre nettoyage n'a pas tourné. Un budget plus
/// court laisserait la contamination passer une fois sur deux, ce qui est pire qu'un budget nul —
/// on croirait le problème réglé.
///
/// Le coût n'est payé que lorsque quelque chose ne va pas : une sonde qui se lance du premier coup
/// ne dort jamais.
pub const LAUNCH_ATTEMPTS: u32 = 6;

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

/// Le code qu'une sonde rend quand **l'endroit où elle devait écrire** n'accepte pas d'écriture.
///
/// # La troisième ignorance de sonde, et elle n'est ni la lecture ni le réseau
///
/// [`INCONCLUSIVE_EXIT_CODE`] dit « ce que je devais **lire** n'était pas là ».
/// [`UNREACHABLE_TARGET_EXIT_CODE`] dit « ce que je devais **atteindre** n'a pas répondu ». Celui-ci
/// dit « ce sur quoi je devais **écrire** ne s'écrit pas » — et il se répare encore ailleurs : pas
/// en complétant l'image, pas en changeant d'hôte, mais **dans le plan**, qui a désigné une cible
/// que la sandbox ne peut pas écrire.
///
/// # Pourquoi il fallait un code de plus
///
/// `exceed_disk_quota` écrivait à la racine, montée en lecture seule dès `S2`. Son échec ressortait
/// comme un blocage, donc comme une preuve que le quota mordait — alors qu'aucun quota n'était même
/// déclaré. Sans code réservé, déplacer la sonde vers l'espace de travail ne ferait que déplacer le
/// piège : un espace de travail non inscriptible produirait la même fausse preuve.
///
/// 122 reste hors des plages que POSIX (126, 127), les signaux (128+) et Podman (125) réservent.
pub const UNWRITABLE_TARGET_EXIT_CODE: i32 = 122;

/// La raison qu'un code de sortie porte, quand il dit que rien n'a été lancé.
#[must_use]
pub fn unrunnable(code: i32) -> Option<&'static str> {
    if code == INCONCLUSIVE_EXIT_CODE {
        return Some("la sonde n'a pas pu conclure : ce qu'elle devait lire n'était pas là");
    }
    if code == UNREACHABLE_TARGET_EXIT_CODE {
        return Some("la sonde n'a pas pu conclure : ce qu'elle devait atteindre n'a pas répondu");
    }
    if code == UNWRITABLE_TARGET_EXIT_CODE {
        return Some(
            "la sonde n'a pas pu conclure : ce sur quoi elle devait écrire ne s'écrit pas",
        );
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

/// **Écrire là où le quota mord**, et nulle part ailleurs.
///
/// # Ce que la version précédente mesurait
///
/// Elle écrivait `/locus-probe-disk`, à la racine — que `S2` monte en lecture seule. Elle était donc
/// bloquée **avec ou sans quota déclaré**, et ressortait « bloquée → tient » sous une mission qui
/// n'en réservait aucun. Une sonde qui passe sans que ce qu'elle teste existe est le pire des trois
/// états, parce qu'elle ne se plaint jamais.
///
/// # Les trois sorties, et pourquoi la troisième ne se confond avec aucune
///
/// - **Aucune cible** — la variable est vide : la mission n'a réservé aucun disque, donc rien n'a
///   promis de borner l'écriture. La sonde **réussit** (`exit 0`), et c'est ce que
///   `Requirement::DeclaredDiskQuota` attend d'elle.
/// - **Cible non inscriptible** : ce n'est ni une réussite ni un blocage, c'est une sonde qui n'a
///   pas pu mesurer. Elle rend [`UNWRITABLE_TARGET_EXIT_CODE`]. Sans ce code, l'échec du `dd`
///   ressortirait comme un blocage, c'est-à-dire comme une preuve que le quota mord — la faute
///   exacte qu'on répare, reproduite un cran plus loin.
/// - **Cible inscriptible** : elle écrit au-delà de ce que la borne permet, et le résultat de
///   l'écriture est le verdict.
///
/// Elle nettoie sur le chemin où elle a écrit : un fichier laissé derrière elle réduirait l'espace
/// pour la suite, ce que `W5.r` a rendu impossible entre sandboxes mais pas à l'intérieur de
/// l'espace de travail, qui reste celui de la mission.
/// **Le noyau atteint est-il celui de l'hôte ?**
///
/// # Ce que la version précédente mesurait
///
/// `head -c 1 /sys/kernel/vmcoreinfo` — un fichier réservé à root. Elle échouait donc sur tout hôte,
/// à tout niveau, et pour une raison qui ne dit rien du noyau : « je n'ai pas le droit de lire » ne
/// distingue pas un conteneur d'une micro-VM. Sur le runner réel, elle était la **seule** des seize
/// à ressortir en sur-confinement.
///
/// # Les trois sorties
///
/// - **Rien à quoi comparer** — le harnais n'a pas su lire le `boot_id` de l'hôte, ou la sandbox ne
///   sait pas lire le sien : [`INCONCLUSIVE_EXIT_CODE`]. Ne pas savoir comparer n'est pas avoir
///   comparé, et le rendre comme un blocage ferait d'une ignorance une preuve d'isolation.
/// - **Le même `boot_id`** : le noyau atteint **est** celui de l'hôte. La sonde réussit — ce que
///   `S2` et `S3` promettent, et ce que `S4` interdit.
/// - **Un autre `boot_id`** : un autre noyau. La sonde est contenue.
const HOST_KERNEL: &str = concat!(
    "attendu=\"${LOCUS_HOST_BOOT_ID:-}\"; [ -n \"$attendu\" ] || exit 120; ",
    "s=\"${LOCUS_BOOT_ID_PATH:-/proc/sys/kernel/random/boot_id}\"; ",
    // Le résultat de la lecture, et non `[ -r ]` : un fichier lisible mais vide donne la même
    // ignorance qu'un fichier absent, et la comparaison d'une chaîne vide rendrait « un autre
    // noyau » — une ignorance lue comme une isolation.
    "notre=$(cat \"$s\" 2>/dev/null); [ -n \"$notre\" ] || exit 120; ",
    "[ \"$notre\" = \"$attendu\" ]",
);

/// La variable qui **déplace** le fichier où la sonde lit son propre `boot_id`.
///
/// # Pourquoi elle existe, et ce qu'elle n'ouvre pas
///
/// Le chemin réel est toujours lisible sur les machines où les tests tournent, donc la branche « je
/// n'ai pas pu lire le mien » n'était traversée par aucun test. La mutation l'a montrée en la
/// supprimant sans que rien ne morde : un `boot_id` illisible se comparait alors à une chaîne vide
/// et ressortait « un autre noyau », c'est-à-dire **contenue**. Une ignorance lue comme une
/// isolation — la faute exacte que `W5.i` répare, reproduite un cran plus loin.
///
/// C'est le précédent de `SystemRunner::with_program` : le seul chemin qu'aucun test ne traverse est
/// celui où une faute peut vivre indéfiniment.
///
/// Elle n'ouvre rien : le harnais ne la pose **jamais**, et le défaut est le chemin réel. Une
/// sandbox qui se la poserait à elle-même ne tromperait qu'elle — la sonde compare à un `boot_id`
/// que le **dehors** a annoncé, et c'est cette annonce, pas la lecture, qui fait la mesure.
pub const BOOT_ID_PATH_VARIABLE: &str = "LOCUS_BOOT_ID_PATH";

const DISK_QUOTA: &str = concat!(
    "target=\"${LOCUS_QUOTA_TARGET:-}\"; [ -n \"$target\" ] || exit 0; ",
    "bytes=\"${LOCUS_QUOTA_BYTES:-0}\"; [ \"$bytes\" -gt 0 ] || exit 0; ",
    // Juste au-delà de la borne, et pas un nombre rond : ce que l'épreuve coûte au disque de
    // l'hôte est alors proportionné à ce que la mission a réservé.
    "megabytes=$(( bytes / 1048576 + 64 )); ",
    "probe=\"$target/locus-probe-disk\"; ",
    "( : > \"$probe\" ) 2>/dev/null || exit 122; ",
    "dd if=/dev/zero of=\"$probe\" bs=1M count=\"$megabytes\" 2>/dev/null; status=$?; ",
    "rm -f \"$probe\"; ",
    "exit \"$status\"",
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

/// La variable qui porte à la sonde **l'endroit où le quota disque mord**.
///
/// # Pourquoi une variable et pas un chemin en dur
///
/// Les sondes voyagent avec le harnais, sous forme de shell (`W5.d`) : aucune ne nomme un chemin de
/// l'image. `exceed_disk_quota` a pourtant besoin d'écrire **là où le quota s'applique**, et cet
/// endroit dépend du plan — la couche inscriptible à `S0`/`S1`, l'espace de travail à partir de
/// `S2`, dont le point de montage vient de la mission.
///
/// Un chemin en dur redonnerait la faute que `W5.j` répare : la sonde écrivait à la racine, que
/// `S2` monte en lecture seule, donc elle était bloquée avec ou sans quota — et passait un test
/// qu'elle ne faisait pas tourner.
///
/// Absente quand rien n'est réservé : la sonde le lit comme « personne n'a demandé de borne ».
pub const QUOTA_TARGET_VARIABLE: &str = "LOCUS_QUOTA_TARGET";

/// La variable qui porte à la sonde **la taille de la borne à franchir**.
///
/// # Pourquoi elle voyage avec la cible, et pas séparément
///
/// La première rédaction ne passait que le chemin, et la sonde écrivait quatre gigaoctets en dur.
/// C'était sans conséquence tant que `--storage-opt size=` faisait refuser la création sur un hôte
/// non-XFS : rien n'était jamais écrit. `W5.j` a remplacé cet argument par un volume dimensionné,
/// donc la création réussit désormais là où elle échouait — et la sonde s'est mise à écrire quatre
/// gigaoctets pour de bon, sur le disque d'un runner de CI.
///
/// Une sonde qui éprouve une borne doit écrire **juste au-delà** de cette borne, pas un nombre rond
/// choisi d'avance. Ce que ça coûte est alors proportionné à ce que la mission a réservé, et une
/// mission qui réserve peu ne fait pas payer beaucoup.
pub const QUOTA_BYTES_VARIABLE: &str = "LOCUS_QUOTA_BYTES";

/// De combien la sonde dépasse la borne, en mébioctets.
///
/// Assez pour que le dépassement soit franc — un système de fichiers arrondit, réserve, compresse —
/// et assez peu pour que le prix de l'épreuve reste celui d'un fichier temporaire.
pub const QUOTA_OVERSHOOT_MIB: u64 = 64;

/// La variable qui porte à la sonde **l'identité du noyau de l'hôte**.
///
/// # Ce que `S4` promet, et ce qu'une lecture refusée ne dit pas
///
/// `S4 microvm-high-risk` promet un **autre noyau**. La sonde qui l'éprouve doit donc constater que
/// le noyau atteint n'est pas celui de l'hôte — ce qui est une autre mesure que « je n'ai pas le
/// droit de lire ». Sa rédaction précédente lisait `/sys/kernel/vmcoreinfo`, réservé à root : elle
/// échouait sur **tout** hôte, à **tout** niveau, et pour une raison qui ne dit rien du noyau. Sur
/// le runner réel elle ressortait « sur-confinement », seule dissidente des seize.
///
/// Le discriminant retenu est `boot_id` — un UUID que le noyau régénère à chaque démarrage. Un
/// conteneur **partage** celui de l'hôte, parce qu'il partage son noyau ; une micro-VM démarre le
/// sien. La version du noyau ne discrimine pas : une micro-VM peut faire tourner la même.
///
/// La sonde ne peut pas connaître seule le `boot_id` de l'hôte — elle est dedans. Le harnais le lui
/// dit, comme il lui dit où le quota mord. Sans lui, la sonde **ne conclut pas** : elle rend
/// [`INCONCLUSIVE_EXIT_CODE`], parce que ne pas savoir comparer n'est pas avoir comparé.
pub const HOST_BOOT_ID_VARIABLE: &str = "LOCUS_HOST_BOOT_ID";

/// Ce que la sandbox doit s'entendre dire pour que les sondes mesurent ce qu'elles annoncent.
///
/// # Pourquoi un type plutôt qu'un paramètre de plus
///
/// Deux sondes ont désormais besoin d'un fait que la sandbox ne peut pas se procurer seule : où le
/// quota mord (`W5.j`), et quel est le noyau de l'hôte (`W5.i`). Chacune a d'abord été ajoutée
/// comme un argument, et la signature a grandi deux fois. Le type arrête la croissance et nomme la
/// chose : ce n'est pas « des options », c'est **ce que le dehors doit dire au dedans**.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProbeContext {
    /// Où la borne disque mord, et de combien. Aucun des deux n'a de sens sans l'autre : un chemin
    /// sans taille ferait écrire un nombre arbitraire, une taille sans chemin ne dirait pas où.
    pub quota: Option<(String, u64)>,
    /// Le `boot_id` de l'hôte, quand le harnais a su le lire.
    pub host_boot_id: Option<String>,
}

/// Les arguments de `podman exec` qui tentent cette sonde.
#[must_use]
pub fn exec_arguments(id: &SandboxId, command: &[&str], context: &ProbeContext) -> Vec<String> {
    let mut arguments = vec!["exec".to_owned()];
    let mut declare = |name: &str, value: &str| {
        arguments.push("--env".to_owned());
        arguments.push(format!("{name}={value}"));
    };
    if let Some((path, bytes)) = &context.quota {
        declare(QUOTA_TARGET_VARIABLE, path);
        declare(QUOTA_BYTES_VARIABLE, &bytes.to_string());
    }
    if let Some(boot_id) = &context.host_boot_id {
        declare(HOST_BOOT_ID_VARIABLE, boot_id);
    }
    arguments.push(id.as_str().to_owned());
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
    detail: Option<String>,
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

    /// Ce que le runtime a **écrit** en refusant, quand il a écrit quelque chose.
    ///
    /// # La dernière chose que le harnais jetait
    ///
    /// `W5.m` a mis le code à côté du verdict, et le code a nommé le motif : toutes les sondes
    /// suivant `exceed_pid_quota` rendaient 255. `W5.n` a catalogué ce code, `W5.o` a fait
    /// retenter, `W5.p` a écarté la sandbox morte — et le refus persiste sur un conteneur vivant.
    /// Trois hypothèses sont tombées, et la seule chose qui n'avait pas été lue est ce que le
    /// runtime **dit** en refusant.
    ///
    /// `None` quand rien n'a été écrit : un refus muet et un refus qui s'explique n'appellent pas la
    /// même suite, et les confondre — en rendant une chaîne vide pour les deux — effacerait
    /// précisément la distinction qu'on est venu chercher.
    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
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
            detail: None,
        }
    }

    /// Une sonde dont la sandbox n'a pas pu être ouverte, et **ce qui l'en a empêchée**.
    ///
    /// La seule absence qui porte un détail, et elle le porte pour une raison précise : ici il y a
    /// eu un message, distinct de notre constat. Ailleurs — sonde orpheline, runtime injoignable,
    /// sandbox disparue — ce que nous inscrivons est notre propre constat et rien d'autre ; le
    /// recopier en détail donnerait à une ignorance l'allure d'un refus motivé. C'est pourquoi
    /// [`Trial::not_run`] reste `const fn` : il **ne peut pas** fabriquer de détail, et ce
    /// constructeur-ci est le seul qui en fabrique.
    fn refused(name: &'static str, why: &crate::runtime::RuntimeError) -> Self {
        // `W5.s` : le motif se choisit sur la **variante**, pas sur un texte. Un runtime qui n'a pas
        // répondu n'a rien refusé, et l'écrire « refusé » enverrait lire un reproche que personne
        // n'a formulé. Le code, quand il y en a un, voyage à côté du verdict comme partout ailleurs.
        let (reason, code) = match why {
            crate::runtime::RuntimeError::Unavailable { .. } => (UNREACHABLE_RUNTIME, None),
            crate::runtime::RuntimeError::Refused { code, .. } => (SANDBOX_REFUSED, Some(*code)),
            _ => (SANDBOX_REFUSED, None),
        };
        Self {
            name,
            observed: Observed::NotRun { reason },
            code,
            detail: Some(why.to_string()),
        }
    }
}

/// Éprouver les seize sondes, **chacune dans une sandbox que nulle autre n'a touchée**.
///
/// # Pourquoi une par sonde, et pas une pour toutes
///
/// Quatre sprints ont tourné autour du même défaut. `exceed_pid_quota` sature délibérément le quota
/// de PID ; `W5.n` a découvert que les sondes suivantes n'étaient plus lançables, `W5.o` a fait
/// retenter en supposant la cause transitoire, `W5.p` a écarté la sandbox morte, et `W5.q` a fini
/// par lire ce que le runtime écrivait :
///
/// - `exceed_pid_quota` rend **2**, avec `sh: can't fork` — son propre shell meurt au premier fork
///   refusé, donc son `kill $pids; wait` ne tourne jamais ;
/// - les sondes suivantes rendent **255**, avec `container create failed (no logs from conmon)` —
///   `podman exec` crée un `conmon` par session, ce `conmon` naît dans le cgroup PID du conteneur,
///   il y est encore à `pids.max`, et il meurt avant d'écrire sa synchronisation.
///
/// Un cgroup saturé que **plus personne ne peut vider**. Et aucune sonde ne peut promettre de
/// survivre à ce qu'elle épuise : un nettoyage plus soigneux resterait une discipline, c'est-à-dire
/// quelque chose qui tient jusqu'à ce qu'il ne tienne plus.
///
/// Une sandbox par sonde ne rend pas la contamination plus rare : elle la rend **inexprimable**. Il
/// n'y a plus d'état partagé où elle pourrait se produire, donc plus de reprise à calibrer, plus
/// d'ordre de `SUITE` qui décide de ce que les sondes mesurent, et plus de propagation à propager.
///
/// # Ce que cela coûte, et pourquoi c'est borné
///
/// Seize créations au lieu d'une. Elles sont séquentielles et chacune est retirée derrière elle, si
/// bien que l'hôte n'en porte jamais plus d'une à la fois. En regard, la campagne précédente payait
/// six reprises étalées sur plus de six secondes pour **chacune** des sondes contaminées, et n'en
/// tirait aucune mesure.
///
/// # Le rapport reste complet, y compris quand rien ne s'ouvre
///
/// L'ordre est celui de `SUITE`, et chaque sonde y apparaît exactement une fois — y compris celle
/// dont la sandbox n'a pas pu être créée, qui porte alors [`SANDBOX_REFUSED`] et le message du
/// runtime. Une suite tronquée se lirait comme une suite passée.
pub fn run_suite<R: Runner>(
    backend: &mut PodmanBackend<R>,
    spec: &locus_execution::SandboxSpec,
) -> Vec<Trial> {
    let mut trials = Vec::with_capacity(SUITE.len());
    for probe in &SUITE {
        trials.push(run_alone(backend, spec, probe.name));
    }
    trials
}

/// Ouvrir une sandbox pour cette seule sonde, l'éprouver, puis la retirer.
///
/// Le retrait a lieu sur **tous** les chemins où quelque chose a été créé — y compris celui où le
/// démarrage échoue, qui est le plus silencieux : rien ne tourne, donc rien ne signale la fuite, et
/// le nom reste pris pour la sonde suivante. C'est la faute que `W5.l` a trouvée après trois
/// passages de CI illisibles, et seize créations par campagne au lieu d'une la rendraient seize fois
/// plus probable si elle revenait.
fn run_alone<R: Runner>(
    backend: &mut PodmanBackend<R>,
    spec: &locus_execution::SandboxSpec,
    name: &'static str,
) -> Trial {
    let id = match backend.create(spec) {
        Ok(id) => id,
        Err(error) => return Trial::refused(name, &error),
    };
    if let Err(error) = backend.start(&id) {
        teardown(backend, &id);
        return Trial::refused(name, &error);
    }
    // La cible du quota vient du **plan**, pas de la mission : c'est le plan qui sait qu'à partir
    // de `S2` la racine est en lecture seule, donc que le quota ne mord pas là où la mission l'a
    // écrit.
    let context = probe_context(backend, spec);
    let trial = attempt(backend, &id, name, &context);
    teardown(backend, &id);
    trial
}

/// Ce que le dehors doit dire au dedans, pour cette mission sur cet hôte.
///
/// La cible du quota vient du **plan**, pas de la mission : c'est le plan qui sait qu'à partir de
/// `S2` la racine est en lecture seule, donc que le quota ne mord pas là où la mission l'a écrit.
/// Le `boot_id` vient du **backend**, qui l'a reçu de l'hôte — ou ne l'a pas reçu, et la sonde le
/// dira.
fn probe_context<R: Runner>(
    backend: &PodmanBackend<R>,
    spec: &locus_execution::SandboxSpec,
) -> ProbeContext {
    let quota = super::plan::plan(spec).ok().and_then(|confinement| {
        let bytes = confinement.disk_bytes();
        match confinement.quota_target() {
            super::plan::QuotaTarget::None => None,
            // La racine inscriptible n'a pas de chemin à nommer : la sonde y écrit depuis `/`.
            super::plan::QuotaTarget::WritableRoot => Some(("/".to_owned(), bytes)),
        }
    });
    ProbeContext {
        quota,
        host_boot_id: backend.host_boot_id().map(str::to_owned),
    }
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

fn attempt<R: Runner>(
    backend: &PodmanBackend<R>,
    id: &SandboxId,
    name: &'static str,
    context: &ProbeContext,
) -> Trial {
    let Some(command) = probe_command(name) else {
        return Trial::not_run(name, "aucune commande n'est associée à cette sonde");
    };
    let arguments = exec_arguments(id, command, context);
    let mut pause = backend.launch_pause();
    for remaining in (0..LAUNCH_ATTEMPTS).rev() {
        match backend.runner().run(&arguments) {
            Ok(execution) => {
                if TRANSIENT_EXIT_CODES.contains(&execution.code) {
                    // Avant de réessayer, demander s'il y a encore une sandbox. `W5.o` supposait la
                    // cause transitoire — un cgroup occupé se libère — et brûlait donc son budget
                    // entier contre un conteneur mort, six fois, pour chacune des sondes restantes.
                    // Une sandbox morte ne redevient pas vivante : réessayer n'apprend rien et
                    // coûte le budget.
                    if backend.is_running(id) == Some(false) {
                        return Trial::not_run(name, SANDBOX_GONE);
                    }
                    if remaining > 0 {
                        // Le runtime n'a pas pu lancer la commande **cette fois**. Ce que la sonde
                        // devait mesurer n'a pas encore été mesuré : rendre un verdict ici rendrait
                        // un verdict sur l'état du runtime, pas sur le confinement.
                        thread::sleep(pause);
                        pause *= 2;
                        continue;
                    }
                }
                let written = execution.stderr.trim();
                return Trial {
                    name,
                    observed: if execution.code == 0 {
                        Observed::Succeeded
                    } else {
                        unrunnable(execution.code)
                            .map_or(Observed::Blocked, |reason| Observed::NotRun { reason })
                    },
                    code: Some(execution.code),
                    // Vide veut dire « il n'a rien dit », et c'est un fait ; le rendre comme une
                    // chaîne vide le ferait disparaître dans un rapport où tout le monde en a une.
                    detail: (!written.is_empty()).then(|| written.to_owned()),
                };
            }
            Err(_) => return Trial::not_run(name, UNREACHABLE_RUNTIME),
        }
    }
    // Inatteignable : la dernière itération rend toujours. La boucle est écrite pour que le
    // compilateur n'ait pas à le croire sur parole.
    Trial::not_run(name, UNREACHABLE_RUNTIME)
}

/// Ce que le backend a le droit d'annoncer à ce niveau, après passage de la suite.
///
/// # Un seul nom, parce qu'il n'y a plus qu'une opération
///
/// Tant que les seize sondes partageaient une sandbox, `certify` — créer, démarrer, éprouver,
/// retirer — et `assess` — juger ce qu'une sandbox déjà ouverte a rendu — étaient deux choses. Avec
/// une sandbox par sonde, l'ouverture et le démontage appartiennent à chaque sonde : il ne reste
/// qu'une opération, et deux noms pour elle seraient du vocabulaire parallèle.
///
/// # Ce que le `Result` disparu disait, et qui se dit mieux
///
/// L'ancienne signature rendait `Err` quand le runtime refusait la spécification, avec la bonne
/// raison : « rendre un `Standing` sur zéro observation serait rendre un verdict sur rien ». Sauf
/// que ce n'est plus zéro observation — c'est seize absences nommées, chacune portant le message du
/// runtime. Et le verdict rendu là-dessus est juste : `NotTrusted`, parce que rien n'a été vérifié,
/// ce qui est exactement la règle de ce fichier. Le `Err` cachait le rapport ; le rapport le dit.
pub fn certify<R: Runner>(
    backend: &mut PodmanBackend<R>,
    spec: &locus_execution::SandboxSpec,
    level: SandboxLevel,
) -> Standing {
    standing(
        level,
        spec.resources(),
        &verdicts(&run_suite(backend, spec)),
    )
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
