//! Ce que l'hôte permet réellement — lu, jamais supposé.
//!
//! # Pourquoi lire plutôt que déclarer
//!
//! [`crate::admission`] décide sur des capacités **déclarées**, et c'est la bonne forme : un
//! broker qui apprendrait ses limites en échouant les découvrirait après avoir créé la moitié
//! d'une sandbox. Reste à savoir d'où vient la déclaration. Si elle vient d'un fichier de
//! configuration, elle dit ce qu'un opérateur croyait vrai le jour où il l'a écrite.
//!
//! Ce module la fait venir du noyau. Il lit les fichiers qui disent si cgroup v2 est monté, quels
//! contrôleurs sont délégués, si un utilisateur non privilégié peut créer un namespace, et si
//! seccomp existe — puis il en tire le niveau que cet hôte peut honnêtement soutenir.
//!
//! # Le doute ne s'arrondit pas vers le haut
//!
//! Un fichier illisible n'est pas un « non » : c'est une question sans réponse. Les deux mènent au
//! même refus — [`HostFacts::ceiling`] est conservateur — mais ils ne se disent pas pareil, et
//! [`Missing`] les distingue. Confondre « le noyau refuse » et « je n'ai pas su regarder » ferait
//! chercher une configuration là où il n'y a qu'un montage manquant.

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::Path;

use locus_execution::SandboxLevel;

use super::plan::BACKEND_CEILING;

/// Les contrôleurs cgroup v2 sans lesquels les quotas de §21.7 ne sont pas applicables.
///
/// `cpu`, `memory` et `pids` portent les trois premiers quotas de `ResourceSpec`. Le quatrième,
/// le disque, ne se borne pas par cgroup — voir `ConfinementPlan::disk_bytes`.
pub const REQUIRED_CONTROLLERS: [&str; 3] = ["cpu", "memory", "pids"];

/// Ce qu'on a pu établir d'une capacité de l'hôte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Support {
    /// L'hôte l'offre.
    Available,
    /// L'hôte ne l'offre pas, et voici ce qui le dit.
    Unavailable {
        /// Ce qui a été lu, et pourquoi c'est un refus.
        reason: String,
    },
    /// On n'a pas pu savoir.
    Undetermined {
        /// Ce qui a empêché de savoir.
        reason: String,
    },
}

impl Support {
    /// Vrai seulement quand la capacité est établie.
    #[must_use]
    pub const fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }
}

/// Ce que l'hôte a répondu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostFacts {
    cgroup_v2: Support,
    controllers: BTreeSet<String>,
    unprivileged_userns: Support,
    seccomp: Support,
    disk_quota: Support,
}

impl HostFacts {
    /// Établir les faits à partir de n'importe quelle source de lecture.
    ///
    /// Le paramètre est un [`Reader`] et non un chemin parce que le noyau à interroger n'est pas
    /// toujours celui du processus : sur macOS, le confinement d'une mission est fourni par le
    /// noyau de la VM, et lire `/sys/fs/cgroup` de l'hôte y répondrait « rien » pour une machine
    /// parfaitement capable. Les deux lectures partagent donc la déduction, et ne diffèrent que par
    /// la façon d'obtenir les fichiers.
    #[must_use]
    pub fn probe<R: Reader + ?Sized>(reader: &R) -> Self {
        let (cgroup_v2, controllers) = cgroup(reader);
        Self {
            cgroup_v2,
            controllers,
            unprivileged_userns: userns(reader),
            seccomp: seccomp(reader),
            // Indéterminé tant que personne n'a dit **où** vivra la couche inscriptible : un quota
            // disque est une propriété d'un système de fichiers, pas d'un hôte en général, et
            // deviner un chemin de stockage rendrait un fait sur autre chose que ce qui sera écrit.
            disk_quota: Support::Undetermined {
                reason: NO_STORAGE_DECLARED.to_owned(),
            },
        }
    }

    /// Établir en plus si le système de fichiers qui portera le stockage peut tenir un quota.
    ///
    /// # Pourquoi c'est une seconde étape et non un champ de plus dans [`HostFacts::probe`]
    ///
    /// Les autres faits se lisent sur le noyau : cgroup v2 est monté ou non, seccomp existe ou non.
    /// Le quota disque, lui, est une propriété **du chemin** où le runtime écrira. Sans ce chemin,
    /// il n'y a pas de question bien posée, et un chemin deviné rendrait un fait sur un autre
    /// système de fichiers que celui qui sera réellement écrit.
    ///
    /// # Ce que ce fait a coûté à découvrir
    ///
    /// `W5.f` a fait tourner la suite de sondes contre un vrai Podman rootless, et `podman create`
    /// a rendu 125 : « storage option overlay.size and overlay.inodes only supported for backingFS
    /// XFS. Found extfs ». `ConfinementPlan::disk_bytes` devient un `--storage-opt size=`, que
    /// Podman ne sait appliquer que sur XFS. L'en-tête de ce module dit pourtant qu'« un broker qui
    /// apprendrait ses limites en échouant les découvrirait après avoir créé la moitié d'une
    /// sandbox » : c'était vrai de tous les faits sauf celui-là, et `REQUIRED_CONTROLLERS` frôlait
    /// le sujet — « le quatrième, le disque, ne se borne pas par cgroup » — sans en tirer la
    /// conséquence.
    #[must_use]
    pub fn with_storage<R: Reader + ?Sized>(mut self, reader: &R, storage_root: &str) -> Self {
        self.disk_quota = disk_quota(reader, storage_root);
        self
    }

    /// Lire un système de fichiers local à partir d'une racine.
    ///
    /// La racine est un paramètre pour que les tests puissent la remplacer par un arbre de
    /// fixtures. En production elle vaut `/` — voir [`HostFacts::read_host`].
    #[must_use]
    pub fn read(root: &Path) -> Self {
        Self::probe(&LocalReader {
            root: root.to_path_buf(),
        })
    }

    /// Lire l'hôte réel.
    #[must_use]
    pub fn read_host() -> Self {
        Self::read(Path::new("/"))
    }

    /// L'état de cgroup v2.
    #[must_use]
    pub const fn cgroup_v2(&self) -> &Support {
        &self.cgroup_v2
    }

    /// Les contrôleurs délégués au cgroup de ce processus.
    #[must_use]
    pub const fn controllers(&self) -> &BTreeSet<String> {
        &self.controllers
    }

    /// L'état des namespaces utilisateur non privilégiés.
    #[must_use]
    pub const fn unprivileged_userns(&self) -> &Support {
        &self.unprivileged_userns
    }

    /// L'état de seccomp.
    #[must_use]
    pub const fn seccomp(&self) -> &Support {
        &self.seccomp
    }

    /// L'état du quota disque sur le stockage déclaré.
    #[must_use]
    pub const fn disk_quota(&self) -> &Support {
        &self.disk_quota
    }

    /// Le système de fichiers à nommer dans un refus, quand le quota disque **n'est pas** tenable.
    ///
    /// # Le doute ne s'arrondit pas vers le haut, ici non plus
    ///
    /// `Undetermined` rend `Some` comme `Unavailable` : « je n'ai pas su regarder » ne vaut pas
    /// « c'est disponible ». Les deux mènent au même refus — mais pas au même texte, parce qu'ils
    /// ne s'inspectent pas au même endroit. C'est la règle que [`HostFacts::ceiling`] applique
    /// déjà aux autres faits.
    ///
    /// C'est ce que [`crate::HostCapabilities::without_disk_quota`] attend : la lecture devient
    /// une déclaration, et l'admission décide sur la déclaration. Sans ce pont, le fait serait lu
    /// et jamais consulté, ce qui reviendrait exactement à ne pas le lire.
    #[must_use]
    pub fn unenforceable_disk_quota(&self) -> Option<String> {
        match &self.disk_quota {
            Support::Available => None,
            Support::Unavailable { reason } | Support::Undetermined { reason } => {
                Some(reason.clone())
            }
        }
    }

    /// Ce qui manque pour honorer ce niveau, éventuellement rien.
    ///
    /// # `S2` et `S3` demandent les mêmes primitives
    ///
    /// Et c'est écrit plutôt que corrigé. Un namespace réseau non privilégié s'obtient par le
    /// namespace utilisateur, exactement comme le namespace de montage : il n'existe aucun fichier
    /// que l'on pourrait lire pour distinguer les deux. Ce qui les sépare est ce que le **plan**
    /// applique, pas ce que l'hôte permet. Inventer ici un test qui les distinguerait donnerait une
    /// fausse précision.
    #[must_use]
    pub fn missing_for(&self, level: SandboxLevel) -> Vec<Missing> {
        let mut missing = Vec::new();
        if level >= SandboxLevel::S1 {
            push(
                &mut missing,
                "namespace utilisateur",
                &self.unprivileged_userns,
            );
        }
        if level >= SandboxLevel::S2 {
            push(&mut missing, "seccomp", &self.seccomp);
            push(&mut missing, "cgroup v2", &self.cgroup_v2);
            for controller in REQUIRED_CONTROLLERS {
                if !self.controllers.contains(controller) {
                    missing.push(Missing::Unavailable {
                        what: "contrôleur cgroup",
                        reason: format!("« {controller} » n'est pas délégué à ce cgroup"),
                    });
                }
            }
        }
        if level > BACKEND_CEILING {
            missing.push(Missing::Unavailable {
                what: "niveau",
                reason: format!(
                    "{} dépasse le plafond {} d'un backend rootless",
                    level.code(),
                    BACKEND_CEILING.code()
                ),
            });
        }
        missing
    }

    /// Le niveau le plus élevé que cet hôte peut soutenir.
    ///
    /// Jamais au-dessus de `BACKEND_CEILING`, et `S0` au pire : ne rien confiner est toujours
    /// possible, et c'est précisément pourquoi `S0` doit être demandé explicitement.
    #[must_use]
    pub fn ceiling(&self) -> SandboxLevel {
        SandboxLevel::ALL
            .into_iter()
            .filter(|level| *level <= BACKEND_CEILING)
            .rfind(|level| self.missing_for(*level).is_empty())
            .unwrap_or(SandboxLevel::S0)
    }

    /// Ce que ces faits valent comme preuve, une ligne par constat.
    ///
    /// C'est ce que le backend joindra à son attestation. §21.6 veut un témoignage, pas une
    /// affirmation : « j'ai appliqué S3 » sans rien qui le montre ne vaut rien.
    #[must_use]
    pub fn evidence(&self) -> Vec<String> {
        let controllers = if self.controllers.is_empty() {
            "aucun".to_owned()
        } else {
            self.controllers
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        };
        vec![
            format!("cgroup v2 : {}", describe(&self.cgroup_v2)),
            format!("contrôleurs délégués : {controllers}"),
            format!(
                "namespace utilisateur non privilégié : {}",
                describe(&self.unprivileged_userns)
            ),
            format!("seccomp : {}", describe(&self.seccomp)),
            format!("quota disque : {}", describe(&self.disk_quota)),
        ]
    }
}

fn push(missing: &mut Vec<Missing>, what: &'static str, support: &Support) {
    match support {
        Support::Available => {}
        Support::Unavailable { reason } => missing.push(Missing::Unavailable {
            what,
            reason: reason.clone(),
        }),
        Support::Undetermined { reason } => missing.push(Missing::Undetermined {
            what,
            reason: reason.clone(),
        }),
    }
}

fn describe(support: &Support) -> String {
    match support {
        Support::Available => "disponible".to_owned(),
        Support::Unavailable { reason } => format!("indisponible ({reason})"),
        Support::Undetermined { reason } => format!("indéterminé ({reason})"),
    }
}

/// Ce qui manque, et si c'est un refus ou une question sans réponse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Missing {
    /// L'hôte ne l'offre pas.
    Unavailable {
        /// La capacité concernée.
        what: &'static str,
        /// Ce qui le dit.
        reason: String,
    },
    /// On n'a pas pu l'établir.
    Undetermined {
        /// La capacité concernée.
        what: &'static str,
        /// Ce qui a empêché de savoir.
        reason: String,
    },
}

impl fmt::Display for Missing {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable { what, reason } => write!(formatter, "{what} : {reason}"),
            Self::Undetermined { what, reason } => {
                write!(formatter, "{what} : indéterminé — {reason}")
            }
        }
    }
}

/// Ce qui est dit quand personne n'a déclaré de racine de stockage.
pub const NO_STORAGE_DECLARED: &str = "aucune racine de stockage n'a été déclarée : on ne sait pas quel système de fichiers portera \
     la couche inscriptible";

/// Les systèmes de fichiers sur lesquels un quota de projet s'applique.
///
/// Un seul, et c'est Podman qui le dit : « storage option overlay.size and overlay.inodes only
/// supported for backingFS XFS ». La liste est une constante nommée plutôt qu'un `==` en ligne
/// pour qu'en ajouter un soit un acte visible, avec la vérification qui va avec.
pub const QUOTA_CAPABLE_FILESYSTEMS: [&str; 1] = ["xfs"];

/// Les options de super-bloc qui disent qu'un quota de projet est **activé**.
///
/// XFS sans elles est un `xfs` sur lequel `--storage-opt size=` échouera quand même, plus tard et
/// ailleurs. Les exiger est la même règle que partout ici : ce qui n'a pas été constaté n'est pas
/// acquis.
pub const PROJECT_QUOTA_OPTIONS: [&str; 2] = ["prjquota", "pquota"];

/// Le système de fichiers qui portera `storage_root`, et s'il sait tenir un quota de projet.
///
/// # Ce qui est lu
///
/// `/proc/self/mountinfo`, et le montage retenu est celui dont le point de montage est le **plus
/// long préfixe** du chemin de stockage — c'est-à-dire le montage effectivement traversé. Prendre
/// le premier qui correspond rendrait `/` pour un stockage sur un volume monté plus bas, donc un
/// verdict sur le mauvais système de fichiers.
fn disk_quota<R: Reader + ?Sized>(reader: &R, storage_root: &str) -> Support {
    let Some(content) = reader.read("/proc/self/mountinfo") else {
        return Support::Undetermined {
            reason: "proc/self/mountinfo est illisible".to_owned(),
        };
    };
    let Some(mount) = backing_mount(&content, storage_root) else {
        return Support::Undetermined {
            reason: format!("aucun montage de proc/self/mountinfo ne porte « {storage_root} »"),
        };
    };
    if !QUOTA_CAPABLE_FILESYSTEMS.contains(&mount.filesystem.as_str()) {
        return Support::Unavailable {
            reason: format!(
                "« {storage_root} » est sur « {} » ; un quota de projet n'existe que sur {}",
                mount.filesystem,
                QUOTA_CAPABLE_FILESYSTEMS.join(", ")
            ),
        };
    }
    if !PROJECT_QUOTA_OPTIONS
        .iter()
        .any(|option| mount.options.iter().any(|present| present == option))
    {
        return Support::Unavailable {
            reason: format!(
                "« {storage_root} » est sur « {} » mais monté sans {} : le quota de projet n'est pas activé",
                mount.filesystem,
                PROJECT_QUOTA_OPTIONS.join(" ni ")
            ),
        };
    }
    Support::Available
}

/// Le montage traversé pour atteindre ce chemin.
struct Mount {
    filesystem: String,
    options: Vec<String>,
}

fn backing_mount(mountinfo: &str, path: &str) -> Option<Mount> {
    let mut best: Option<(usize, Mount)> = None;
    for line in mountinfo.lines() {
        // `mountinfo` sépare les champs fixes des champs variables par un « - » isolé. Découper
        // dessus est ce qui rend la lecture insensible au nombre de champs optionnels, qui varie.
        let (head, tail) = line.split_once(" - ")?;
        let point = head.split_whitespace().nth(4)?;
        let mut rest = tail.split_whitespace();
        let filesystem = rest.next()?;
        let options = rest.nth(1).unwrap_or_default();
        if !covers(point, path) {
            continue;
        }
        if best.as_ref().is_none_or(|(len, _)| point.len() > *len) {
            best = Some((
                point.len(),
                Mount {
                    filesystem: filesystem.to_owned(),
                    options: options.split(',').map(str::to_owned).collect(),
                },
            ));
        }
    }
    best.map(|(_, mount)| mount)
}

/// Ce point de montage est-il traversé pour atteindre ce chemin ?
///
/// `/var` couvre `/var/lib/containers` mais pas `/variable` : la comparaison se fait au **segment**,
/// pas au caractère, sans quoi un répertoire dont le nom commence par celui d'un montage passerait
/// pour être dessous.
fn covers(point: &str, path: &str) -> bool {
    if point == "/" || point == path {
        return true;
    }
    path.strip_prefix(point)
        .is_some_and(|rest| rest.starts_with('/'))
}

/// cgroup v2, et les contrôleurs délégués au cgroup de **ce** processus.
///
/// Ce que la racine offre ne dit pas ce que nous avons : `cgroup.subtree_control` d'un parent
/// décide de ce que ses enfants voient. On lit donc le `cgroup.controllers` de notre propre
/// répertoire, qui est la seule liste que nous pourrons effectivement écrire.
fn cgroup<R: Reader + ?Sized>(reader: &R) -> (Support, BTreeSet<String>) {
    let Some(_) = reader.read("/sys/fs/cgroup/cgroup.controllers") else {
        return (
            Support::Unavailable {
                reason: "sys/fs/cgroup/cgroup.controllers est absent : pas de hiérarchie unifiée"
                    .to_owned(),
            },
            BTreeSet::new(),
        );
    };
    let Some(own) = own_cgroup_path(reader) else {
        return (
            Support::Undetermined {
                reason: "proc/self/cgroup ne porte pas de ligne « 0:: »".to_owned(),
            },
            BTreeSet::new(),
        );
    };
    let controllers = format!("{own}/cgroup.controllers");
    let Some(listed) = reader.read(&controllers) else {
        return (
            Support::Undetermined {
                reason: format!("{controllers} est illisible"),
            },
            BTreeSet::new(),
        );
    };
    (
        Support::Available,
        listed.split_whitespace().map(str::to_owned).collect(),
    )
}

fn own_cgroup_path<R: Reader + ?Sized>(reader: &R) -> Option<String> {
    let content = reader.read("/proc/self/cgroup")?;
    let relative = content
        .lines()
        .find_map(|line| line.strip_prefix("0::"))?
        .trim()
        .trim_start_matches('/');
    Some(format!("/sys/fs/cgroup/{relative}"))
}

/// Les namespaces utilisateur non privilégiés.
///
/// Deux fichiers, et l'ordre compte. `proc/sys/user/max_user_namespaces` à zéro est un refus net.
/// `proc/sys/kernel/unprivileged_userns_clone` n'existe que sur les noyaux qui portent le correctif
/// Debian ; **son absence n'est donc pas un refus**, et la traiter comme tel refuserait S1 sur la
/// plupart des noyaux amont.
fn userns<R: Reader + ?Sized>(reader: &R) -> Support {
    let Some(maximum) = reader.read("/proc/sys/user/max_user_namespaces") else {
        return Support::Undetermined {
            reason: "proc/sys/user/max_user_namespaces est illisible".to_owned(),
        };
    };
    let Ok(allowed) = maximum.trim().parse::<u64>() else {
        return Support::Undetermined {
            reason: format!("max_user_namespaces vaut « {} »", maximum.trim()),
        };
    };
    if allowed == 0 {
        return Support::Unavailable {
            reason: "max_user_namespaces vaut 0".to_owned(),
        };
    }
    match reader.read("/proc/sys/kernel/unprivileged_userns_clone") {
        Some(toggle) if toggle.trim() == "0" => Support::Unavailable {
            reason: "unprivileged_userns_clone vaut 0".to_owned(),
        },
        _ => Support::Available,
    }
}

fn seccomp<R: Reader + ?Sized>(reader: &R) -> Support {
    match reader.read("/proc/sys/kernel/seccomp/actions_avail") {
        Some(actions) if actions.contains("errno") || actions.contains("kill") => {
            Support::Available
        }
        Some(actions) => Support::Unavailable {
            reason: format!("actions_avail ne porte aucune action de refus : « {actions} »"),
        },
        None => Support::Unavailable {
            reason: "proc/sys/kernel/seccomp/actions_avail est absent".to_owned(),
        },
    }
}

/// De quoi obtenir le contenu d'un fichier, d'où qu'il vienne.
///
/// `None` ne distingue pas « absent » de « illisible » : les deux mènent au même `Undetermined`,
/// et le détail qui les sépare n'est pas disponible de la même façon selon la source. Ce que la
/// distinction sert à préserver — le doute contre le refus — est porté par [`Support`], plus haut.
pub trait Reader {
    /// Le contenu du fichier, ou `None`.
    fn read(&self, path: &str) -> Option<String>;
}

/// La lecture d'un système de fichiers monté localement, sous une racine.
#[derive(Debug, Clone)]
pub struct LocalReader {
    /// La racine sous laquelle les chemins absolus sont résolus.
    pub root: std::path::PathBuf,
}

impl Reader for LocalReader {
    fn read(&self, path: &str) -> Option<String> {
        fs::read_to_string(self.root.join(path.trim_start_matches('/'))).ok()
    }
}
