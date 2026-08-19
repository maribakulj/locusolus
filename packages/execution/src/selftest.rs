//! La suite de self-tests de sandbox — ADR 0004, `docs/SPEC_V1.md` §21.6, §21.7, §32.3.
//!
//! # Ce que cette suite est
//!
//! « La suite de tests définit ce que "sandbox" veut dire dans ce projet, et un backend qui échoue
//! un test critique n'est pas `trusted` » (`docs/10_V1_ROADMAP.md`). Elle est donc écrite **avant**
//! le premier backend, et c'est l'ordre que l'ADR 0004 impose : un backend écrit d'abord définirait
//! la sandbox par ce qu'il sait faire, et la suite se contenterait ensuite de le décrire.
//!
//! Chaque sonde déclare **le niveau à partir duquel elle doit être contenue**. C'est cette
//! déclaration qui donne un contenu aux niveaux de §21.6 : `S3` ne veut rien dire de plus que `S2`
//! tant qu'aucune sonde ne les sépare.

use std::fmt;

use crate::level::SandboxLevel;
use crate::resources::ResourceSpec;

/// La dimension de sécurité qu'une sonde met à l'épreuve.
///
/// Les cinq premières viennent de §32.3 — « aucun home, socket de runtime ou secret hôte accessible
/// par défaut », « quotas CPU/RAM/PID/disque vérifiés par self-tests » — et les deux dernières de
/// §21.6 et §21.7. Le type existe pour que la **couverture** soit vérifiable : une suite qui aurait
/// oublié une dimension entière passerait tous ses tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Dimension {
    /// Écrire ou lire hors de l'espace de travail.
    HostFilesystem,
    /// Atteindre le socket d'un runtime de containers.
    RuntimeSocket,
    /// Lire un secret de l'hôte.
    HostSecret,
    /// Dépasser un quota de ressources.
    ResourceQuota,
    /// Voir ou toucher les processus de l'hôte.
    ProcessIsolation,
    /// Sortir sur le réseau.
    Network,
    /// Atteindre le noyau ou le matériel de l'hôte.
    HostKernel,
}

impl Dimension {
    /// Les sept.
    pub const ALL: [Self; 7] = [
        Self::HostFilesystem,
        Self::RuntimeSocket,
        Self::HostSecret,
        Self::ResourceQuota,
        Self::ProcessIsolation,
        Self::Network,
        Self::HostKernel,
    ];

    /// Le nom de la dimension.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::HostFilesystem => "host-filesystem",
            Self::RuntimeSocket => "runtime-socket",
            Self::HostSecret => "host-secret",
            Self::ResourceQuota => "resource-quota",
            Self::ProcessIsolation => "process-isolation",
            Self::Network => "network",
            Self::HostKernel => "host-kernel",
        }
    }
}

/// Une sonde : ce qu'elle tente, et le niveau à partir duquel elle ne doit plus y arriver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Probe {
    /// Son nom, stable — il apparaîtra dans les attestations et les rapports de conformance.
    pub name: &'static str,
    /// Ce qu'elle met à l'épreuve.
    pub dimension: Dimension,
    /// Le niveau à partir duquel elle **doit** être contenue.
    ///
    /// En dessous, elle doit **réussir** : une sonde contenue plus tôt que déclaré signale un
    /// backend plus strict que ce qu'il annonce, ce qui fera échouer des missions légitimes de
    /// façon inexplicable.
    pub contained_from: SandboxLevel,
    /// Pourquoi ce niveau et pas un autre.
    pub rationale: &'static str,
    /// Vrai quand un échec de cette sonde interdit de considérer le backend comme `trusted`.
    ///
    /// `docs/10_V1_ROADMAP.md` distingue les tests critiques des autres — « un backend qui échoue un
    /// test **critique** n'est pas `trusted` ». Les seize le sont aujourd'hui, et un test l'affirme :
    /// le jour où quelqu'un ajoutera une sonde non critique, il devra le décider explicitement au
    /// lieu de le laisser passer. Une sandbox n'a pas de contenu accessoire.
    pub critical: bool,
    /// Ce que la **mission** doit avoir déclaré pour que cette sonde mesure quelque chose.
    ///
    /// Voir [`Requirement`]. `Nothing` pour quinze des seize.
    pub requires: Requirement,
}

/// Ce que la mission doit avoir déclaré pour qu'une sonde ait un sujet.
///
/// # Pourquoi `contained_from` ne suffit pas toujours
///
/// `contained_from` dit à partir de quel **niveau** une sonde doit être contenue. Cela suppose que
/// ce qu'elle éprouve existe toujours — vrai pour quinze sondes sur seize, parce qu'un namespace,
/// un profil seccomp ou une capability retirée sont des propriétés du niveau.
///
/// Le quota disque est l'exception, et une seule chose la crée : [`crate::ResourceSpec`] **refuse**
/// un quota nul pour le CPU, la mémoire, les PID et l'horizon, et **accepte** zéro pour le disque.
/// C'est donc la seule ressource facultative du système. Une mission qui n'en réserve pas n'a rien
/// promis de borner, et la sonde qui écrit alors sans entrave ne révèle aucun défaut : elle
/// constate qu'on ne lui a rien demandé.
///
/// Sans ce champ, `exceed_disk_quota` serait `Contained` dès `S2` quoi qu'ait déclaré la mission —
/// donc `Escaped` chaque fois qu'une mission ordinaire, sans quota disque, écrit dans son espace de
/// travail. Le harnais l'a longtemps caché en la laissant se faire bloquer par la racine en lecture
/// seule : elle passait un test qu'elle ne faisait pas tourner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Requirement {
    /// Rien : le niveau seul décide de ce que la sonde doit produire.
    Nothing,
    /// Un quota disque non nul. Sans lui, la sonde doit **réussir** — à tout niveau.
    DeclaredDiskQuota,
}

/// La suite.
///
/// # Comment lire `contained_from`
///
/// `S1 os-write-contained` contient les **écritures**, pas les lectures : c'est ce que son nom dit.
/// `S2 container-rootless` ajoute les espaces de noms — montages, PID, utilisateurs — donc les
/// lectures de l'hôte, le socket de runtime, la vue sur les processus et les quotas cgroup.
/// `S3 container-isolated-network` ajoute le réseau. `S4 microvm-high-risk` ajoute un noyau propre.
/// `S5 remote-trusted-enclave-or-equivalent` ajoute la protection contre l'hôte lui-même.
///
/// Aucune sonde n'est déclarée contenue à partir de `S0` : `S0 unsandboxed-explicit` ne contient
/// rien, et c'est ce que « explicit » veut dire — on le demande, on sait ce qu'on demande.
pub const SUITE: [Probe; 16] = [
    Probe {
        name: "write_outside_workspace",
        dimension: Dimension::HostFilesystem,
        contained_from: SandboxLevel::S1,
        rationale: "S1 contient les écritures ; c'est la seule chose que son nom promet",
        critical: true,
        requires: Requirement::Nothing,
    },
    Probe {
        name: "write_host_home",
        dimension: Dimension::HostFilesystem,
        contained_from: SandboxLevel::S1,
        rationale: "le home de l'utilisateur ne se monte pas, et ne s'écrit donc jamais",
        critical: true,
        requires: Requirement::Nothing,
    },
    Probe {
        name: "persist_after_teardown",
        dimension: Dimension::HostFilesystem,
        contained_from: SandboxLevel::S1,
        rationale: "une écriture qui survit à la sandbox est une écriture hors de la sandbox",
        critical: true,
        requires: Requirement::Nothing,
    },
    Probe {
        name: "read_host_filesystem",
        dimension: Dimension::HostFilesystem,
        contained_from: SandboxLevel::S2,
        rationale: "il faut un espace de noms de montage pour que la lecture s'arrête aussi",
        critical: true,
        requires: Requirement::Nothing,
    },
    Probe {
        name: "read_host_secret_files",
        dimension: Dimension::HostSecret,
        contained_from: SandboxLevel::S2,
        rationale: "`/etc/shadow`, `~/.ssh`, `~/.aws` : lisibles tant qu'il n'y a pas de namespace",
        critical: true,
        requires: Requirement::Nothing,
    },
    Probe {
        name: "read_process_environment",
        dimension: Dimension::HostSecret,
        contained_from: SandboxLevel::S2,
        rationale: "l'environnement d'un autre processus porte des credentials plus souvent qu'on ne croit",
        critical: true,
        requires: Requirement::Nothing,
    },
    Probe {
        name: "access_container_runtime_socket",
        dimension: Dimension::RuntimeSocket,
        contained_from: SandboxLevel::S2,
        rationale: "le socket donne le pouvoir de créer un conteneur privilégié : c'est l'annulation du confinement par l'intérieur",
        critical: true,
        requires: Requirement::Nothing,
    },
    Probe {
        name: "escalate_to_root",
        dimension: Dimension::ProcessIsolation,
        contained_from: SandboxLevel::S2,
        rationale: "`container-rootless` : le nom du niveau est la promesse",
        critical: true,
        requires: Requirement::Nothing,
    },
    Probe {
        name: "observe_host_processes",
        dimension: Dimension::ProcessIsolation,
        contained_from: SandboxLevel::S2,
        rationale: "un espace de noms PID sépare ce qu'on voit de ce qui tourne",
        critical: true,
        requires: Requirement::Nothing,
    },
    Probe {
        name: "exceed_cpu_quota",
        dimension: Dimension::ResourceQuota,
        contained_from: SandboxLevel::S2,
        rationale: "§32.3 : quota CPU vérifié par self-test ; cgroups v2 arrive avec le conteneur",
        critical: true,
        requires: Requirement::Nothing,
    },
    Probe {
        name: "exceed_memory_quota",
        dimension: Dimension::ResourceQuota,
        contained_from: SandboxLevel::S2,
        rationale: "§32.3 : quota RAM vérifié par self-test",
        critical: true,
        requires: Requirement::Nothing,
    },
    Probe {
        name: "exceed_pid_quota",
        dimension: Dimension::ResourceQuota,
        contained_from: SandboxLevel::S2,
        rationale: "§32.3 : quota PID vérifié par self-test ; c'est celui qui manque le plus souvent",
        critical: true,
        requires: Requirement::Nothing,
    },
    Probe {
        name: "exceed_disk_quota",
        dimension: Dimension::ResourceQuota,
        contained_from: SandboxLevel::S2,
        rationale: "§32.3 : quota disque vérifié par self-test — mais seulement quand la mission en réserve un, voir `requires`",
        critical: true,
        requires: Requirement::DeclaredDiskQuota,
    },
    Probe {
        name: "open_outbound_connection",
        dimension: Dimension::Network,
        contained_from: SandboxLevel::S3,
        rationale: "`container-isolated-network` : c'est le niveau qui coupe le réseau, pas le précédent",
        critical: true,
        requires: Requirement::Nothing,
    },
    Probe {
        name: "reach_cloud_metadata_service",
        dimension: Dimension::Network,
        contained_from: SandboxLevel::S3,
        rationale: "`169.254.169.254` délivre des credentials d'instance à qui sait demander ; xiiif le refuse inconditionnellement, et pour la même raison",
        critical: true,
        requires: Requirement::Nothing,
    },
    Probe {
        name: "reach_host_kernel_interfaces",
        dimension: Dimension::HostKernel,
        contained_from: SandboxLevel::S4,
        rationale: "un conteneur partage le noyau de l'hôte ; seule une micro-VM en apporte un autre",
        critical: true,
        requires: Requirement::Nothing,
    },
];

/// Les niveaux que cette suite peut mettre à l'épreuve **depuis l'intérieur**.
///
/// # Pourquoi `S5` n'y est pas
///
/// `S5 remote-trusted-enclave-or-equivalent` promet une chose que les autres ne promettent pas :
/// une protection contre **l'hôte lui-même**. Or une suite de self-tests s'exécute dans la sandbox,
/// donc sur cet hôte, avec ce que cet hôte veut bien lui montrer. Une sonde qui prétendrait vérifier
/// « l'opérateur de la machine ne peut pas lire ma mémoire » rendrait le verdict que l'hôte aurait
/// choisi de lui rendre.
///
/// Ce n'est donc pas une sonde manquante, c'est une **limite de méthode** : la garantie de `S5` se
/// vérifie par attestation matérielle distante, pas par self-test. C'est aussi ce que
/// `docs/10_V1_ROADMAP.md` dit en écrivant « suite de self-tests indexée par niveau **S0–S4** » là où
/// §21.6 énumère six niveaux — l'écart relevé en W4.a n'en était pas un.
///
/// Un test vérifie que `S5` ne gagne aucune sonde, pour que personne ne « complète » la suite en
/// inventant celle qui ne peut pas exister.
pub const SELF_TESTABLE_LEVELS: [SandboxLevel; 5] = [
    SandboxLevel::S0,
    SandboxLevel::S1,
    SandboxLevel::S2,
    SandboxLevel::S3,
    SandboxLevel::S4,
];

/// Ce qu'une sonde doit produire à un niveau donné.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expectation {
    /// Elle doit réussir : ce niveau ne prétend pas contenir cela.
    Allowed,
    /// Elle doit échouer : le niveau le promet.
    Contained,
}

/// Ce que la sonde a réellement produit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observed {
    /// Elle a réussi.
    Succeeded,
    /// Elle a été bloquée.
    Blocked,
    /// Elle n'a pas pu être exécutée.
    ///
    /// **Distincte des deux autres, et c'est le point.** Une sonde qu'on n'a pas su lancer n'a rien
    /// prouvé ; la compter comme bloquée ferait d'un outil manquant une preuve d'isolation, ce qui
    /// est la façon la plus tranquille de croire une sandbox qu'on n'a jamais testée.
    NotRun {
        /// Ce qui a empêché de la lancer.
        reason: &'static str,
    },
}

/// Ce que la sonde doit produire au niveau demandé, sous cette réservation.
///
/// # Pourquoi la réservation entre ici
///
/// Le niveau décide seul pour quinze sondes sur seize. La seizième éprouve une ressource que la
/// mission peut ne pas avoir réservée — voir [`Requirement`] — et une borne que personne n'a
/// demandée ne peut pas être franchie.
///
/// C'est [`ResourceSpec`] tout entier qui est passé, et non un booléen « quota déclaré ou non » : le
/// booléen serait un second vocabulaire pour dire ce que la réservation dit déjà, et il faudrait le
/// tenir à jour le jour où une deuxième ressource devient facultative.
#[must_use]
pub const fn expectation(
    probe: &Probe,
    level: SandboxLevel,
    reserved: &ResourceSpec,
) -> Expectation {
    if matches!(probe.requires, Requirement::DeclaredDiskQuota) && reserved.disk_bytes() == 0 {
        return Expectation::Allowed;
    }
    if (level as u8) >= (probe.contained_from as u8) {
        Expectation::Contained
    } else {
        Expectation::Allowed
    }
}

/// Le verdict d'une sonde confrontée à son attente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Ce qui était attendu s'est produit.
    Holds,
    /// La sonde a réussi là où le niveau promettait de la contenir.
    ///
    /// C'est **l'échappement** : la sandbox ne tient pas ce qu'elle annonce, et une mission qui s'y
    /// est fiée a tourné sans le confinement qu'elle croyait avoir.
    Escaped {
        /// La sonde.
        probe: &'static str,
        /// Le niveau prétendu.
        level: SandboxLevel,
    },
    /// La sonde a été bloquée là où le niveau ne promettait rien.
    ///
    /// Ce n'est pas un trou de sécurité, et ce n'est pas rien : un backend plus strict que ce qu'il
    /// annonce fera échouer des missions légitimes de façon inexplicable, et personne ne cherchera
    /// la cause du côté de l'isolation puisque l'isolation « va bien ».
    OverContained {
        /// La sonde.
        probe: &'static str,
        /// Le niveau prétendu.
        level: SandboxLevel,
    },
    /// La sonde n'a pas pu être exécutée : rien n'est prouvé.
    Inconclusive {
        /// La sonde.
        probe: &'static str,
        /// Pourquoi.
        reason: &'static str,
    },
}

impl Verdict {
    /// Vrai quand ce verdict interdit d'accorder la confiance au backend.
    ///
    /// Un `Inconclusive` sur une sonde critique compte : ADR 0004 dit qu'« un backend qui échoue un
    /// test critique n'est pas `trusted` », et un test critique qu'on n'a pas su lancer n'a pas
    /// réussi. Le traiter comme neutre reviendrait à accorder la confiance faute de contre-preuve,
    /// alors que c'est la preuve qui manque.
    #[must_use]
    pub const fn denies_trust(&self, critical: bool) -> bool {
        match self {
            Self::Holds | Self::OverContained { .. } => false,
            Self::Escaped { .. } => true,
            Self::Inconclusive { .. } => critical,
        }
    }
}

/// Confronter une observation à ce que le niveau promettait.
#[must_use]
pub fn judge(
    probe: &Probe,
    level: SandboxLevel,
    reserved: &ResourceSpec,
    observed: Observed,
) -> Verdict {
    match (expectation(probe, level, reserved), observed) {
        (Expectation::Contained, Observed::Blocked)
        | (Expectation::Allowed, Observed::Succeeded) => Verdict::Holds,
        (Expectation::Contained, Observed::Succeeded) => Verdict::Escaped {
            probe: probe.name,
            level,
        },
        (Expectation::Allowed, Observed::Blocked) => Verdict::OverContained {
            probe: probe.name,
            level,
        },
        (_, Observed::NotRun { reason }) => Verdict::Inconclusive {
            probe: probe.name,
            reason,
        },
    }
}

/// Ce qu'un backend a le droit d'annoncer, après passage de la suite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Standing {
    /// Le niveau annoncé est tenu.
    Trusted {
        /// Le niveau.
        level: SandboxLevel,
    },
    /// Le niveau annoncé n'est pas tenu, et voici pourquoi.
    ///
    /// Aucune variante intermédiaire : « presque `trusted` » n'existe pas. Un backend qui laisse
    /// échapper une sonde critique n'est pas un backend légèrement moins bon, c'est un backend dont
    /// les missions ont tourné sans le confinement qu'elles croyaient avoir.
    NotTrusted {
        /// Le niveau annoncé.
        level: SandboxLevel,
        /// Ce qui l'empêche.
        blocking: Vec<Verdict>,
    },
}

/// Décider si un backend tient le niveau qu'il annonce.
///
/// `results` associe chaque sonde à ce qu'elle a produit. Une sonde **absente** de `results` rend un
/// `Inconclusive` : le silence n'est pas un succès, et une suite tronquée ne doit pas se lire comme
/// une suite passée.
#[must_use]
pub fn standing(
    level: SandboxLevel,
    reserved: &ResourceSpec,
    results: &[(&'static str, Observed)],
) -> Standing {
    let mut blocking = Vec::new();
    for probe in &SUITE {
        let observed = results.iter().find(|(name, _)| *name == probe.name).map_or(
            Observed::NotRun {
                reason: "sonde absente du rapport",
            },
            |(_, observed)| *observed,
        );
        let verdict = judge(probe, level, reserved, observed);
        if verdict.denies_trust(probe.critical) {
            blocking.push(verdict);
        }
    }
    if blocking.is_empty() {
        Standing::Trusted { level }
    } else {
        Standing::NotTrusted { level, blocking }
    }
}

/// Les sondes qu'un niveau contient et que le niveau précédent laissait passer.
///
/// # À quoi ça sert
///
/// À vérifier qu'un niveau **veut dire quelque chose**. `S3` ne se distingue de `S2` que si au moins
/// une sonde passe de l'un à l'autre ; sinon les deux nomment la même isolation, et un
/// `SandboxSpec` qui exige `S3` obtient `S2` sans que rien ne le signale.
#[must_use]
pub fn newly_contained(level: SandboxLevel) -> Vec<&'static Probe> {
    SUITE
        .iter()
        .filter(|probe| probe.contained_from == level)
        .collect()
}

impl fmt::Display for Verdict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Holds => formatter.write_str("conforme"),
            Self::Escaped { probe, level } => write!(
                formatter,
                "échappement : « {probe} » a réussi alors que {} promettait de la contenir",
                level.code()
            ),
            Self::OverContained { probe, level } => write!(
                formatter,
                "sur-confinement : « {probe} » a été bloquée alors que {} ne promettait rien",
                level.code()
            ),
            Self::Inconclusive { probe, reason } => write!(
                formatter,
                "non concluant : « {probe} » n'a pas pu être exécutée ({reason})"
            ),
        }
    }
}
