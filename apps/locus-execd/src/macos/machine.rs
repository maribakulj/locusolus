//! La machine : son état, ce qu'on lit dedans, et le plafond qui en découle.

use std::fmt;

use locus_execution::SandboxLevel;

use crate::linux::driver::Runner;
use crate::linux::plan::BACKEND_CEILING;
use crate::linux::probe::{HostFacts, Missing, Reader};

/// Où en est la VM Linux qui porte les conteneurs.
///
/// Trois états, et le troisième est celui qu'on oublie. Une machine **arrêtée** existe : elle
/// apparaît dans les listes, elle a une configuration, un opérateur la croit là. Elle ne confine
/// rien. Les confondre ferait annoncer `S3` à un hôte où aucune mission ne peut tourner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineState {
    /// Aucune machine n'est définie.
    Absent,
    /// Une machine est définie mais ne tourne pas.
    Stopped {
        /// Son nom.
        name: String,
    },
    /// Une machine tourne.
    Running {
        /// Son nom.
        name: String,
    },
    /// L'état n'a pas pu être établi.
    Undetermined {
        /// Ce qui a empêché de savoir.
        reason: String,
    },
}

impl MachineState {
    /// Le nom de la machine, quand il y en a une.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Stopped { name } | Self::Running { name } => Some(name),
            Self::Absent | Self::Undetermined { .. } => None,
        }
    }

    /// Vrai seulement quand une machine tourne.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        matches!(self, Self::Running { .. })
    }
}

impl fmt::Display for MachineState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Absent => formatter.write_str("aucune machine définie"),
            Self::Stopped { name } => write!(formatter, "« {name} » est définie mais arrêtée"),
            Self::Running { name } => write!(formatter, "« {name} » tourne"),
            Self::Undetermined { reason } => write!(formatter, "état indéterminé : {reason}"),
        }
    }
}

/// Les arguments qui demandent l'état des machines.
///
/// Le gabarit rend une ligne `nom état` par machine : une liste explicite, comme pour
/// l'inspection des conteneurs, et non tout ce que Podman voudra bien dire.
#[must_use]
pub fn list_arguments() -> Vec<String> {
    vec![
        "machine".to_owned(),
        "list".to_owned(),
        "--format".to_owned(),
        "{{.Name}} {{.Running}}".to_owned(),
    ]
}

/// Les arguments qui lisent un fichier dans l'invité.
#[must_use]
pub fn read_arguments(machine: &str, path: &str) -> Vec<String> {
    vec![
        "machine".to_owned(),
        "ssh".to_owned(),
        machine.to_owned(),
        "cat".to_owned(),
        path.to_owned(),
    ]
}

/// Le nom de la machine, sans la marque de la machine par défaut.
///
/// # Un astérisque qui coûtait tout l'invité
///
/// `podman machine list --format "{{.Name}}"` rend `podman-machine-default*` — l'astérisque marque
/// la machine **par défaut**, et il fait partie du champ, pas du gabarit. Passé tel quel à
/// `podman machine ssh`, il n'est plus un nom de machine : le shell le reçoit comme une commande,
/// répond `podman-machine-default*: command not found`, et [`MachineReader`] rend `None` pour
/// **chaque** fichier.
///
/// L'effet était silencieux et trompeur : une machine correctement démarrée, reconnue comme telle
/// et annoncée « tourne », dont l'invité paraissait dépourvu de cgroup v2 et de seccomp. Le plafond
/// tombait à `S0` sur un hôte qui tient `S2`, et le diagnostic imprimé accusait le noyau invité —
/// c'est-à-dire l'endroit exact où personne n'irait chercher un défaut de parsing.
///
/// La marque est retirée à la lecture plutôt qu'à l'usage : un nom qui ne désigne pas une machine
/// ne doit pas circuler dans le reste du module, où chaque appel devrait alors s'en souvenir.
fn nom_de_machine(brut: &str) -> &str {
    brut.strip_suffix('*').unwrap_or(brut)
}

/// Lire l'état des machines.
pub fn state<R: Runner + ?Sized>(runner: &R) -> MachineState {
    let Ok(execution) = runner.run(&list_arguments()) else {
        return MachineState::Undetermined {
            reason: "podman machine list n'a pas répondu".to_owned(),
        };
    };
    if execution.code != 0 {
        return MachineState::Undetermined {
            reason: format!(
                "podman machine list a rendu {} : {}",
                execution.code,
                execution.stderr.trim()
            ),
        };
    }
    let mut stopped = None;
    for line in execution.stdout.lines() {
        let mut parts = line.split_whitespace();
        let (Some(name), Some(running)) = (parts.next(), parts.next()) else {
            continue;
        };
        let name = nom_de_machine(name);
        if running.eq_ignore_ascii_case("true") {
            return MachineState::Running {
                name: name.to_owned(),
            };
        }
        stopped.get_or_insert_with(|| name.to_owned());
    }
    stopped.map_or(MachineState::Absent, |name| MachineState::Stopped { name })
}

/// Lire un fichier de l'invité à travers la machine.
///
/// C'est l'implémentation de [`Reader`] qui permet à [`HostFacts::probe`] d'établir, sans une ligne
/// de logique dupliquée, les faits du noyau **qui confine** plutôt que ceux du noyau qui appelle.
pub struct MachineReader<'a, R: Runner + ?Sized> {
    runner: &'a R,
    machine: String,
}

impl<'a, R: Runner + ?Sized> MachineReader<'a, R> {
    /// Lire à travers cette machine.
    pub fn new(runner: &'a R, machine: &str) -> Self {
        Self {
            runner,
            machine: machine.to_owned(),
        }
    }
}

impl<R: Runner + ?Sized> Reader for MachineReader<'_, R> {
    fn read(&self, path: &str) -> Option<String> {
        let execution = self
            .runner
            .run(&read_arguments(&self.machine, path))
            .ok()
            .filter(|execution| execution.code == 0)?;
        Some(execution.stdout)
    }
}

/// Les fichiers que la sonde demande, plus celui qu'elle **dérive**.
///
/// La liste couple ce module à [`crate::linux::probe`], et c'est assumé : la raison est écrite sur
/// [`MachineSnapshot`], et elle tient en une phrase — un lecteur qui rouvre une session par fichier
/// ne peut pas répondre de façon cohérente à deux questions liées.
const FICHIERS: [&str; 7] = [
    "/proc/self/mountinfo",
    "/sys/fs/cgroup/cgroup.controllers",
    "/proc/self/cgroup",
    "/proc/sys/user/max_user_namespaces",
    "/proc/sys/kernel/unprivileged_userns_clone",
    "/proc/sys/kernel/seccomp/actions_avail",
    // Pas lu par `HostFacts::probe` : c'est [`MachineFacts::boot_id`] qui le sert, et il voyage
    // dans le même instantané parce qu'il décrit le même noyau au même instant.
    crate::linux::driver::BOOT_ID_PATH,
];

/// La marque qui sépare deux fichiers dans l'instantané.
const BORNE: &str = "===locus===";

/// Le script qui lit tout l'invité **en une fois**.
///
/// La dernière ligne est celle qui compte : elle résout le cgroup de la session courante et lit ses
/// contrôleurs *dans la même session*, sous exactement le chemin que
/// `crate::linux::probe::own_cgroup_path` reconstruira.
fn script() -> String {
    use std::fmt::Write as _;
    let mut script = String::new();
    for fichier in FICHIERS {
        let _ = write!(
            script,
            "printf '%s\\n%s\\n' '{BORNE}' '{fichier}'; cat '{fichier}' 2>/dev/null; "
        );
    }
    let _ = write!(
        script,
        "propre=$(sed -n 's|^0::/*||p' /proc/self/cgroup | head -1); \
         if [ -n \"$propre\" ]; then \
           chemin=\"/sys/fs/cgroup/$propre/cgroup.controllers\"; \
           printf '%s\\n%s\\n' '{BORNE}' \"$chemin\"; cat \"$chemin\" 2>/dev/null; \
         fi"
    );
    script
}

/// Les arguments qui prennent l'instantané.
#[must_use]
pub fn snapshot_arguments(machine: &str) -> Vec<String> {
    vec![
        "machine".to_owned(),
        "ssh".to_owned(),
        machine.to_owned(),
        "sh".to_owned(),
        "-c".to_owned(),
        script(),
    ]
}

/// Tout l'invité, lu en une session — et pourquoi c'est nécessaire.
///
/// # Deux lectures liées ne survivent pas à deux sessions
///
/// La sonde de cgroup demande d'abord `/proc/self/cgroup`, puis les contrôleurs du répertoire
/// qu'elle y trouve. Sur un système de fichiers local, « self » est le même processus aux deux
/// instants. À travers `podman machine ssh`, **chaque lecture est une session SSH distincte**, donc
/// un scope systemd distinct : la première répond `session-14.scope`, la seconde s'exécute dans
/// `session-19.scope`, et le répertoire de la première n'existe plus.
///
/// Le symptôme était trompeur au point de désigner le mauvais coupable : cgroup v2 « indéterminé,
/// ce chemin est illisible », sur une VM dont la racine `cgroup.controllers` liste pourtant
/// `cpuset cpu io memory pids`. Le plafond tombait à `S1` sur un hôte qui tient `S2`, et le
/// diagnostic accusait le noyau invité — c'est-à-dire l'endroit où personne n'irait chercher un
/// défaut de transport.
///
/// L'instantané rétablit ce que le lecteur local offrait gratuitement : **une vue cohérente**. Le
/// prix est la liste [`FICHIERS`], qui doit suivre la sonde ; le prix de l'autre choix était une
/// réponse fausse, ce qui n'est pas un prix mais une dette.
pub struct MachineSnapshot {
    fichiers: std::collections::BTreeMap<String, String>,
}

impl MachineSnapshot {
    /// Prendre l'instantané de MACHINE.
    pub fn read<R: Runner + ?Sized>(runner: &R, machine: &str) -> Self {
        let Ok(execution) = runner.run(&snapshot_arguments(machine)) else {
            return Self {
                fichiers: std::collections::BTreeMap::new(),
            };
        };
        Self {
            fichiers: Self::parse(&execution.stdout),
        }
    }

    /// Relire la sortie du script en (CHEMIN, CONTENU).
    ///
    /// Un fichier absent laisse un contenu vide plutôt qu'aucune entrée : `cat` a échoué, la
    /// question a bien été posée, et la sonde doit pouvoir distinguer « lu, vide » de « pas lu ».
    /// Le second cas est celui d'une machine qui ne répond pas du tout, et il rend une carte vide.
    fn parse(sortie: &str) -> std::collections::BTreeMap<String, String> {
        let mut fichiers = std::collections::BTreeMap::new();
        for bloc in sortie.split(BORNE).skip(1) {
            let mut lignes = bloc.trim_start_matches('\n').splitn(2, '\n');
            let (Some(chemin), contenu) = (lignes.next(), lignes.next().unwrap_or("")) else {
                continue;
            };
            let chemin = chemin.trim();
            if !chemin.is_empty() {
                fichiers.insert(chemin.to_owned(), contenu.to_owned());
            }
        }
        fichiers
    }
}

impl Reader for MachineSnapshot {
    fn read(&self, path: &str) -> Option<String> {
        let contenu = self.fichiers.get(path)?;
        if contenu.is_empty() {
            None
        } else {
            Some(contenu.clone())
        }
    }
}

/// Ce qu'un hôte macOS peut offrir : l'état de la machine, et ce que son invité permet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineFacts {
    state: MachineState,
    guest: Option<HostFacts>,
    boot_id: Option<String>,
}

impl MachineFacts {
    /// Interroger la machine, puis son invité si elle tourne.
    ///
    /// L'invité n'est lu que quand la machine tourne : interroger une machine arrêtée rendrait des
    /// lectures vides, que [`HostFacts`] lirait comme des indéterminations — un diagnostic exact sur
    /// une question qui n'avait pas lieu d'être posée, et qui ferait chercher un problème de noyau
    /// là où il suffit de démarrer la machine.
    pub fn read<R: Runner + ?Sized>(runner: &R) -> Self {
        let state = state(runner);
        let Some(machine) = state.name().filter(|_| state.is_running()) else {
            return Self {
                state,
                guest: None,
                boot_id: None,
            };
        };
        let instantane = MachineSnapshot::read(runner, machine);
        let boot_id = instantane
            .read(crate::linux::driver::BOOT_ID_PATH)
            .and_then(|contenu| crate::linux::driver::boot_id_from(&contenu));
        Self {
            guest: Some(HostFacts::probe(&instantane)),
            state,
            boot_id,
        }
    }

    /// Le `boot_id` du noyau **qui confine**, quand il se lit.
    ///
    /// # Pourquoi il ne peut pas venir de `host_boot_id`
    ///
    /// Celle-ci lit `/proc/sys/kernel/random/boot_id` du processus courant, et sa documentation
    /// prévoit déjà `None` « sur un hôte non-Linux ». Sur macOS c'est exactement le cas, et la
    /// conséquence était que `reach_host_kernel_interfaces` n'avait rien à quoi comparer : elle ne
    /// concluait pas, ce qui à `S2` compte comme non mesuré.
    ///
    /// Or le conteneur ne partage pas le noyau du Mac — il partage celui de la VM. La valeur qui
    /// discrimine est donc celle de l'invité, et elle est lue dans le **même instantané** que le
    /// reste : un `boot_id` pris dans une autre session décrirait le même noyau, mais rien ne
    /// l'aurait garanti.
    #[must_use]
    pub fn boot_id(&self) -> Option<&str> {
        self.boot_id.as_deref()
    }

    /// L'état de la machine.
    #[must_use]
    pub const fn state(&self) -> &MachineState {
        &self.state
    }

    /// Les faits de l'invité, quand la machine tourne.
    #[must_use]
    pub const fn guest(&self) -> Option<&HostFacts> {
        self.guest.as_ref()
    }

    /// Ce qui manque pour honorer ce niveau sur cet hôte.
    ///
    /// Deux familles de manques, et elles ne se confondent pas : la machine, et l'invité. Une
    /// machine arrêtée n'est pas un noyau incapable, c'est un service à démarrer, et le refus doit
    /// permettre de faire la différence sans lire le code.
    #[must_use]
    pub fn missing_for(&self, level: SandboxLevel) -> Vec<Missing> {
        if level == SandboxLevel::S0 {
            return Vec::new();
        }
        let Some(guest) = self.guest.as_ref() else {
            return vec![match &self.state {
                MachineState::Undetermined { reason } => Missing::Undetermined {
                    what: "machine",
                    reason: reason.clone(),
                },
                other => Missing::Unavailable {
                    what: "machine",
                    reason: other.to_string(),
                },
            }];
        };
        let mut missing = guest.missing_for(level);
        if level > BACKEND_CEILING {
            missing.push(Missing::Unavailable {
                what: "niveau",
                reason: format!(
                    "{} exige une VM par mission ; celle-ci est partagée",
                    level.code()
                ),
            });
        }
        missing
    }

    /// Le niveau le plus élevé que cet hôte peut soutenir.
    ///
    /// # Pourquoi une VM ne fait pas un `S4`
    ///
    /// `S4` s'appelle `microvm-high-risk` : sa promesse est qu'une mission à haut risque a **son
    /// propre** noyau. Un déploiement macOS ordinaire fait tourner toutes ses missions dans la même
    /// VM, où le voisin d'une mission est un conteneur et non une machine. Le plafond reste donc
    /// celui du backend rootless. Le jour où un déploiement créera une VM par mission, ce sera un
    /// autre backend, avec son propre plafond et sa propre suite de self-tests.
    #[must_use]
    pub fn ceiling(&self) -> SandboxLevel {
        SandboxLevel::ALL
            .into_iter()
            .filter(|level| *level <= BACKEND_CEILING)
            .rfind(|level| self.missing_for(*level).is_empty())
            .unwrap_or(SandboxLevel::S0)
    }

    /// Ce que ces faits valent comme preuve.
    #[must_use]
    pub fn evidence(&self) -> Vec<String> {
        let mut lines = vec![format!("machine : {}", self.state)];
        match self.guest.as_ref() {
            Some(guest) => lines.extend(
                guest
                    .evidence()
                    .into_iter()
                    .map(|line| format!("invité — {line}")),
            ),
            None => lines.push("invité : non interrogé, la machine ne tourne pas".to_owned()),
        }
        lines
    }
}
