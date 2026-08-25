//! La traduction : d'un `SandboxSpec` vers ce qu'un backend Linux rootless doit appliquer.
//!
//! # Pourquoi la traduction avant le fil
//!
//! ADR 0015 a établi la forme sur Temporal : « la traduction avant le fil ». On écrit d'abord ce
//! qu'on demandera au backend, on le teste en entier, et on branche le transport ensuite. Ici
//! l'enjeu est plus lourd que là-bas, parce que le plan de rollback d'ADR 0004 est explicite :
//! « aucun chemin de repli acceptable — un raccourci ici est exactement le *sandbox factice* que
//! le handoff interdit ». Un driver qui lancerait des processus avant que la traduction soit
//! vérifiée confinerait de travers sans que rien ne le dise.
//!
//! Ce module ne lance rien. Il rend, pour une spécification donnée, la liste exacte des
//! namespaces, des limites cgroup v2, de la posture seccomp, des capabilities retirées, des
//! montages et de la posture réseau qu'un backend rootless devra appliquer — ou une erreur
//! nommée quand ce que la mission demande ne peut pas être obtenu sans privilèges.

use std::collections::BTreeSet;
use std::fmt;

use locus_execution::{
    MountMode, NetworkMode, ResourceSpec, SandboxLevel, SandboxSpec, forbidden_marker,
};

/// Le plafond de ce backend.
///
/// Un conteneur rootless au réseau isolé est `S3` ; `S4` est une micro-VM et `S5` une enclave
/// distante, que rien de ce qui est écrit ici ne sait produire. Le plafond est une constante et
/// non une supposition : [`plan`] refuse au-delà, et le refus nomme les deux niveaux.
pub const BACKEND_CEILING: SandboxLevel = SandboxLevel::S3;

/// Un namespace Linux.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Namespace {
    /// `user` — celui qui rend tous les autres accessibles sans privilèges.
    User,
    /// `mount` — la vue du système de fichiers.
    Mount,
    /// `pid` — les processus visibles.
    Pid,
    /// `ipc` — files de messages et mémoire partagée.
    Ipc,
    /// `uts` — nom d'hôte et de domaine.
    Uts,
    /// `net` — interfaces, routes et ports.
    Network,
    /// `cgroup` — la racine de la hiérarchie vue de l'intérieur.
    Cgroup,
}

impl Namespace {
    /// Le nom que `unshare` et `/proc/self/ns/` emploient.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Mount => "mnt",
            Self::Pid => "pid",
            Self::Ipc => "ipc",
            Self::Uts => "uts",
            Self::Network => "net",
            Self::Cgroup => "cgroup",
        }
    }
}

/// Ce que le filtre d'appels système fait.
///
/// L'ordre est significatif — c'est ce qui permet de vérifier qu'un niveau ne relâche pas le
/// filtre du niveau inférieur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SeccompPosture {
    /// Aucun filtre. Seul `S0` y a droit, et il faut l'avoir demandé.
    Unconfined,
    /// Le profil par défaut du runtime : les appels manifestement dangereux sont refusés.
    Baseline,
    /// Le profil restreint : `Baseline`, plus le refus de tout ce qui crée un namespace ou
    /// charge du code noyau depuis l'intérieur.
    Restricted,
}

impl SeccompPosture {
    /// Le nom court, pour l'attestation.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Unconfined => "unconfined",
            Self::Baseline => "baseline",
            Self::Restricted => "restricted",
        }
    }
}

/// Ce que la sandbox voit du réseau.
///
/// L'ordre va du plus fermé au plus ouvert, et sert à vérifier qu'un niveau ne l'élargit pas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkPosture {
    /// Un namespace réseau vide : pas même de loopback configuré.
    Isolated,
    /// Un namespace réseau dont seuls les connecteurs déclarés sortent, par le proxy d'egress.
    ConnectorsOnly,
    /// Un namespace réseau dont seuls les hôtes listés sortent, par le proxy d'egress.
    ProxiedAllowlist {
        /// Les hôtes autorisés, dans l'ordre où la mission les a donnés.
        hosts: Vec<String>,
    },
    /// Le réseau de l'hôte, sans namespace.
    Host,
}

impl NetworkPosture {
    /// Le rang d'ouverture : plus il est haut, plus la sandbox voit le réseau.
    const fn openness(&self) -> u8 {
        match self {
            Self::Isolated => 0,
            Self::ConnectorsOnly => 1,
            Self::ProxiedAllowlist { .. } => 2,
            Self::Host => 3,
        }
    }

    /// Le nom du mode de §21.7 que cette posture réalise.
    #[must_use]
    pub const fn slug(&self) -> &'static str {
        match self {
            Self::Isolated => "deny",
            Self::ConnectorsOnly => "connector_only",
            Self::ProxiedAllowlist { .. } => "allowlist",
            Self::Host => "full",
        }
    }
}

/// Une limite cgroup v2 : le fichier de contrôle et la valeur à y écrire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CgroupLimit {
    /// Le nom du fichier, relatif au répertoire cgroup de la sandbox.
    pub file: &'static str,
    /// Ce qui y sera écrit, dans la syntaxe du contrôleur.
    pub value: String,
}

/// La période cgroup v2 employée pour `cpu.max`, en microsecondes.
///
/// C'est la valeur par défaut du noyau. La fixer ici plutôt que de la lire rend le quota
/// calculable et vérifiable ; le backend qui écrira le fichier écrira les deux nombres ensemble.
pub const CPU_PERIOD_MICROSECONDS: u64 = 100_000;

/// Où le quota disque s'applique, c'est-à-dire où la sandbox peut écrire.
///
/// # Ce que l'implicite disait, et pourquoi il était faux
///
/// `disk_bytes` devenait un `--storage-opt size=`, qui dimensionne la **couche inscriptible du
/// conteneur**. C'est juste tant que la racine est inscriptible — `S0`, `S1`. À partir de `S2`, le
/// plan monte la racine en lecture seule : la couche ainsi dimensionnée est alors une couche que
/// **personne n'écrit**, et le seul endroit inscriptible est l'espace de travail monté, qui hérite
/// du système de fichiers de l'hôte et n'est borné par rien.
///
/// Le quota était donc déclaré, accepté, transmis au runtime — et sans effet. C'est la forme la plus
/// tranquille d'une garantie absente : tout le chemin a l'air de fonctionner.
///
/// # Pourquoi un type et pas un booléen
///
/// Parce qu'il y a **trois** cas et que le troisième est celui qu'on oublierait. Une sandbox à `S2`
/// sans espace de travail monté n'a **aucun** endroit inscriptible : un quota y est sans objet, et
/// l'accepter en silence recommencerait exactement la faute qu'on répare. Il est refusé au plan.
///
/// # Ce que ce type ne décide pas
///
/// Le **mécanisme** de la borne sur l'espace de travail. Un bind mount d'un répertoire de l'hôte ne
/// se borne pas ; il faut un volume dimensionné — que Podman ne sait tenir que sur XFS avec les
/// quotas de projet, c'est-à-dire exactement le fait que `W5.g` fait déjà **lire** à l'hôte avant
/// toute création, et refuser à l'admission quand il manque. Un tmpfs borné marcherait sur tout
/// système de fichiers et serait le mauvais choix : c'est de la RAM, donc une réservation de disque
/// viendrait manger la réservation de mémoire — deux budgets, une ressource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuotaTarget {
    /// Aucun quota n'est réservé : il n'y a rien à appliquer nulle part.
    None,
    /// La couche inscriptible du conteneur — quand la racine est inscriptible.
    WritableRoot,
}

// `QuotaTarget::Workspace` a existé ici, de `W5.j` à `W5.y`, et **ne pouvait pas fonctionner**.
//
// Elle rendait un `--mount type=volume,destination={target}` sur un `target` **pris d'un montage
// déjà déclaré**, que `invocation::mount_argument` émet en `--mount type=bind`. Podman recevait donc
// deux montages sur la même destination et refusait la spécification entière :
// `Error: /work: duplicate mount destination`.
//
// Ce n'était pas rattrapable en changeant la destination : un volume dimensionné ailleurs ne borne
// pas l'endroit où la charge écrit. Et tout `Mount` de `packages/execution` est un **bind** — il
// porte une source et une cible, il n'y a pas d'autre forme —, donc la collision était certaine à
// chaque fois que la variante était choisie.
//
// Un type qui annonce un effet qui n'a pas lieu est ce que l'ADR 0022 décision 0 appelle une
// **promesse**. Elle est retirée, et le refus la remplace.

/// Un montage, tel que le backend devra le déclarer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountPlan {
    /// Le chemin côté hôte.
    pub source: String,
    /// Le chemin dans la sandbox.
    pub target: String,
    /// Vrai quand la sandbox ne pourra pas écrire.
    pub read_only: bool,
    /// Le marqueur interdit que ce montage porte, quand il n'existe que par dérogation.
    ///
    /// `None` est le cas ordinaire. `Some` signale un montage que `CLAUDE.md` interdit et qu'une
    /// approbation nommée a permis : le backend doit le voir, parce que c'est lui qui l'appliquera.
    pub deviation: Option<&'static str>,
}

/// Les capabilities retirées dès qu'un confinement est demandé.
///
/// Un processus rootless n'en détient déjà aucune dans le namespace initial ; les retirer
/// explicitement vise le namespace **interne**, où l'utilisateur est `root` et les détiendrait
/// toutes. C'est là qu'un `CAP_SYS_ADMIN` interne redonne le pouvoir de monter, donc de défaire.
pub const DANGEROUS_CAPABILITIES: [&str; 5] = [
    "CAP_SYS_ADMIN",
    "CAP_SYS_MODULE",
    "CAP_SYS_RAWIO",
    "CAP_SYS_PTRACE",
    "CAP_SYS_BOOT",
];

/// Les capabilities retirées en plus des précédentes à partir de `S2`.
pub const REMAINING_CAPABILITIES: [&str; 7] = [
    "CAP_NET_ADMIN",
    "CAP_NET_RAW",
    "CAP_MKNOD",
    "CAP_AUDIT_WRITE",
    "CAP_SETUID",
    "CAP_SETGID",
    "CAP_DAC_OVERRIDE",
];

/// Ce qu'un backend Linux rootless devra appliquer pour honorer une spécification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfinementPlan {
    level: SandboxLevel,
    namespaces: BTreeSet<Namespace>,
    cgroup: Vec<CgroupLimit>,
    disk_bytes: u64,
    quota_target: QuotaTarget,
    wall_clock_seconds: u32,
    seccomp: SeccompPosture,
    no_new_privileges: bool,
    dropped_capabilities: BTreeSet<&'static str>,
    read_only_rootfs: bool,
    mounts: Vec<MountPlan>,
    network: NetworkPosture,
}

impl ConfinementPlan {
    /// Le niveau que ce plan réalise — celui qu'exigeait la mission, jamais davantage.
    #[must_use]
    pub const fn level(&self) -> SandboxLevel {
        self.level
    }

    /// Les namespaces à créer.
    #[must_use]
    pub const fn namespaces(&self) -> &BTreeSet<Namespace> {
        &self.namespaces
    }

    /// Les limites cgroup v2 à écrire.
    #[must_use]
    pub fn cgroup(&self) -> &[CgroupLimit] {
        &self.cgroup
    }

    /// Le quota disque, en octets. Zéro quand la mission n'en réserve pas.
    ///
    /// cgroup v2 ne sait pas borner un **espace** — `io.max` borne un débit. Le quota vit donc
    /// hors de [`ConfinementPlan::cgroup`], et le dire évite qu'on le cherche dans une liste où il
    /// ne peut pas être.
    ///
    /// Combien, et non **où** : voir [`ConfinementPlan::quota_target`].
    #[must_use]
    pub const fn disk_bytes(&self) -> u64 {
        self.disk_bytes
    }

    /// **Où** le quota disque mord — c'est-à-dire où la sandbox peut écrire.
    ///
    /// Voir [`QuotaTarget`]. La question n'était pas posée avant `W5.j`, et l'implicite était faux
    /// à partir de `S2`.
    #[must_use]
    pub const fn quota_target(&self) -> &QuotaTarget {
        &self.quota_target
    }

    /// L'horizon d'exécution, en secondes.
    ///
    /// Le noyau ne l'applique pas : c'est le broker qui compte et qui annule. Le porter ici évite
    /// qu'un backend le croie appliqué par ce qu'il vient d'écrire dans cgroup.
    #[must_use]
    pub const fn wall_clock_seconds(&self) -> u32 {
        self.wall_clock_seconds
    }

    /// La posture du filtre d'appels système.
    #[must_use]
    pub const fn seccomp(&self) -> SeccompPosture {
        self.seccomp
    }

    /// Vrai quand `no_new_privs` doit être posé.
    #[must_use]
    pub const fn no_new_privileges(&self) -> bool {
        self.no_new_privileges
    }

    /// Les capabilities à retirer dans le namespace interne.
    #[must_use]
    pub const fn dropped_capabilities(&self) -> &BTreeSet<&'static str> {
        &self.dropped_capabilities
    }

    /// Vrai quand la racine doit être montée en lecture seule.
    #[must_use]
    pub const fn read_only_rootfs(&self) -> bool {
        self.read_only_rootfs
    }

    /// Les montages à appliquer.
    #[must_use]
    pub fn mounts(&self) -> &[MountPlan] {
        &self.mounts
    }

    /// Ce que la sandbox verra du réseau.
    #[must_use]
    pub const fn network(&self) -> &NetworkPosture {
        &self.network
    }

    /// Vrai quand ce plan confine au moins autant que l'autre, sur toutes les dimensions.
    ///
    /// # Ce que la comparaison ne regarde pas
    ///
    /// Les ressources. Elles viennent de la mission, pas du niveau : deux plans de niveaux
    /// différents peuvent réserver le même CPU, et un plan `S3` avec plus de mémoire ne confine
    /// pas moins. Les mêler ferait dire à la comparaison une chose qu'elle ne sait pas.
    #[must_use]
    pub fn confines_at_least(&self, other: &Self) -> bool {
        self.namespaces.is_superset(&other.namespaces)
            && self.seccomp >= other.seccomp
            && (self.no_new_privileges || !other.no_new_privileges)
            && (self.read_only_rootfs || !other.read_only_rootfs)
            && self
                .dropped_capabilities
                .is_superset(&other.dropped_capabilities)
            && self.network.openness() <= other.network.openness()
    }
}

/// Traduire une spécification en plan de confinement rootless.
///
/// # Ce que le niveau décide, et ce que la mission décide
///
/// Le **niveau** décide de l'isolation : namespaces, seccomp, capabilities, racine en lecture
/// seule. La **mission** décide des ressources et des montages. Les deux ne se mélangent pas, et
/// c'est pourquoi les limites cgroup sont écrites même en `S0` : l'invariant 6 dit que les
/// ressources sont réservées avant exécution, pas qu'elles le sont quand on isole.
///
/// # Errors
///
/// - [`PlanError::LevelBeyondBackend`] au-delà de [`BACKEND_CEILING`] ;
/// - [`PlanError::NetworkNeedsIsolation`] pour tout mode réseau autre que `full` en deçà de `S3` —
///   sans namespace réseau, un processus rootless voit le réseau de l'hôte, et prétendre le
///   contraire serait exactement le sandbox factice qu'ADR 0004 interdit ;
/// - [`PlanError::MountsNeedNamespace`] pour un montage demandé en `S0`, qui n'a pas de vue du
///   système de fichiers à lui ;
/// - [`PlanError::QuotaNeedsContainment`] pour un quota disque non nul en `S0`.
pub fn plan(spec: &SandboxSpec) -> Result<ConfinementPlan, PlanError> {
    let level = spec.minimum_level();
    if level > BACKEND_CEILING {
        return Err(PlanError::LevelBeyondBackend {
            required: level,
            ceiling: BACKEND_CEILING,
        });
    }

    let network = posture(spec.network(), level)?;
    if level == SandboxLevel::S0 && !spec.mounts().is_empty() {
        return Err(PlanError::MountsNeedNamespace);
    }
    if level == SandboxLevel::S0 && spec.resources().disk_bytes() > 0 {
        return Err(PlanError::QuotaNeedsContainment);
    }

    let read_only_rootfs = level >= SandboxLevel::S2;
    let quota_target = quota_target(spec, read_only_rootfs, level)?;

    Ok(ConfinementPlan {
        level,
        namespaces: namespaces(level, &network),
        cgroup: limits(spec.resources()),
        disk_bytes: spec.resources().disk_bytes(),
        quota_target,
        wall_clock_seconds: spec.resources().wall_clock_seconds(),
        seccomp: seccomp(level),
        no_new_privileges: level >= SandboxLevel::S1,
        dropped_capabilities: capabilities(level),
        read_only_rootfs,
        mounts: spec.mounts().iter().map(mount).collect(),
        network,
    })
}

/// Où le quota mord : là où la sandbox peut écrire.
///
/// # Le premier montage inscriptible, et pourquoi c'est bien « le premier »
///
/// Une mission peut monter plusieurs espaces de travail. Répartir un quota unique entre eux
/// demanderait de décider comment, et rien dans `SandboxSpec` ne le dit — inventer une règle ici
/// produirait une borne que personne n'a demandée. Le premier montage inscriptible est donc
/// **désigné**, et un test l'épingle : le jour où une mission en aura besoin de deux, ce sera un
/// item, pas une surprise.
///
/// # Errors
///
/// [`PlanError::QuotaWithoutWritableSpace`] quand un quota est réservé alors que la racine est en
/// lecture seule et qu'aucun montage n'est inscriptible.
fn quota_target(
    spec: &SandboxSpec,
    read_only_rootfs: bool,
    level: SandboxLevel,
) -> Result<QuotaTarget, PlanError> {
    if spec.resources().disk_bytes() == 0 {
        return Ok(QuotaTarget::None);
    }
    if !read_only_rootfs {
        return Ok(QuotaTarget::WritableRoot);
    }
    let Some(mount) = spec
        .mounts()
        .iter()
        .find(|mount| mount.mode() == MountMode::ReadWrite)
    else {
        return Err(PlanError::QuotaWithoutWritableSpace { level });
    };
    // Il y a bien un espace inscriptible — et le runtime ne sait pas le **borner**. `W5.y` : tout
    // `Mount` est un bind, et Podman ne dimensionne pas un bind ; superposer un volume dimensionné
    // à la même destination produisait une spécification qu'il refusait en bloc.
    //
    // Les deux refus ne se confondent pas, et c'est la règle de `W5.h` : « rien à borner » envoie
    // monter un espace de travail, « la borne n'est pas applicable » envoie changer d'hôte ou de
    // système de fichiers. Le second est le motif `disk_quota_not_enforceable` de §10.2, qui existe
    // dans `packages/lep` depuis `W5.g` — et que rien ne produisait.
    Err(PlanError::DiskQuotaNotEnforceable {
        level,
        target: mount.target().to_owned(),
    })
}

/// La posture réseau, et la seule dimension où le niveau et la mission ne sont pas indépendants.
///
/// `S3` s'appelle `container-isolated-network` : son contenu **est** le namespace réseau. D'où une
/// équivalence, et non deux règles :
///
/// - en deçà de `S3`, un mode autre que `full` n'a rien pour le porter — un processus rootless sans
///   namespace réseau voit le réseau de l'hôte, et dire « deny » là-dessus serait un mensonge ;
/// - en `S3`, `full` viderait le niveau de son contenu. Le test de stricte croissance l'a montré
///   avant que quiconque le remarque : sous `full`, le plan `S3` était identique au plan `S2`.
///
/// Une mission qui veut l'isolation des processus et le réseau de l'hôte demande `S2` et `full`,
/// et elle l'obtient. Ce qui est refusé est de l'appeler `S3`.
fn posture(mode: &NetworkMode, level: SandboxLevel) -> Result<NetworkPosture, PlanError> {
    match (mode, level) {
        (NetworkMode::Full, SandboxLevel::S3) => Err(PlanError::IsolationContradictsNetwork {
            level: SandboxLevel::S3,
        }),
        (NetworkMode::Full, _) => Ok(NetworkPosture::Host),
        (_, level) if level < SandboxLevel::S3 => Err(PlanError::NetworkNeedsIsolation {
            mode: mode.slug(),
            minimum: SandboxLevel::S3,
        }),
        (NetworkMode::Deny, _) => Ok(NetworkPosture::Isolated),
        (NetworkMode::ConnectorOnly, _) => Ok(NetworkPosture::ConnectorsOnly),
        (NetworkMode::Allowlist { hosts }, _) => Ok(NetworkPosture::ProxiedAllowlist {
            hosts: hosts.clone(),
        }),
    }
}

fn namespaces(level: SandboxLevel, network: &NetworkPosture) -> BTreeSet<Namespace> {
    let mut set = BTreeSet::new();
    if level >= SandboxLevel::S1 {
        set.insert(Namespace::User);
        set.insert(Namespace::Mount);
    }
    if level >= SandboxLevel::S2 {
        set.insert(Namespace::Pid);
        set.insert(Namespace::Ipc);
        set.insert(Namespace::Uts);
        set.insert(Namespace::Cgroup);
    }
    if level >= SandboxLevel::S3 && !matches!(network, NetworkPosture::Host) {
        set.insert(Namespace::Network);
    }
    set
}

fn seccomp(level: SandboxLevel) -> SeccompPosture {
    match level {
        SandboxLevel::S0 => SeccompPosture::Unconfined,
        SandboxLevel::S1 => SeccompPosture::Baseline,
        _ => SeccompPosture::Restricted,
    }
}

fn capabilities(level: SandboxLevel) -> BTreeSet<&'static str> {
    let mut dropped = BTreeSet::new();
    if level >= SandboxLevel::S1 {
        dropped.extend(DANGEROUS_CAPABILITIES);
    }
    if level >= SandboxLevel::S2 {
        dropped.extend(REMAINING_CAPABILITIES);
    }
    dropped
}

/// Les trois contrôleurs que §21.7 réserve, dans l'ordre où un opérateur les lit.
fn limits(resources: &ResourceSpec) -> Vec<CgroupLimit> {
    let quota = u64::from(resources.cpu_millis()) * CPU_PERIOD_MICROSECONDS / 1000;
    vec![
        CgroupLimit {
            file: "cpu.max",
            value: format!("{quota} {CPU_PERIOD_MICROSECONDS}"),
        },
        CgroupLimit {
            file: "memory.max",
            value: resources.memory_bytes().to_string(),
        },
        CgroupLimit {
            file: "pids.max",
            value: resources.pids().to_string(),
        },
    ]
}

fn mount(declared: &locus_execution::Mount) -> MountPlan {
    MountPlan {
        source: declared.source().to_owned(),
        target: declared.target().to_owned(),
        read_only: declared.mode() == MountMode::ReadOnly,
        deviation: forbidden_marker(declared.source()),
    }
}

/// Ce qui empêche une spécification d'être traduite pour ce backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    /// Le niveau exigé dépasse ce qu'un conteneur rootless sait faire.
    LevelBeyondBackend {
        /// Ce que la mission exige.
        required: SandboxLevel,
        /// Le plafond du backend.
        ceiling: SandboxLevel,
    },
    /// Un mode réseau autre que `full` sans namespace réseau pour le porter.
    NetworkNeedsIsolation {
        /// Le mode demandé.
        mode: &'static str,
        /// Le niveau à partir duquel il est réalisable.
        minimum: SandboxLevel,
    },
    /// Le réseau de l'hôte demandé au niveau dont la définition est le réseau isolé.
    IsolationContradictsNetwork {
        /// Le niveau concerné.
        level: SandboxLevel,
    },
    /// Un montage demandé sans vue du système de fichiers à lui.
    MountsNeedNamespace,
    /// Un quota disque demandé sans rien pour contenir les écritures.
    QuotaNeedsContainment,
    /// Un quota disque demandé là où **rien n'est inscriptible**.
    ///
    /// À partir de `S2` la racine est montée en lecture seule ; sans espace de travail monté, la
    /// sandbox n'a aucun endroit où écrire. Un quota y serait accepté, transmis au runtime, et
    /// n'aurait rien à borner — c'est la forme la plus tranquille d'une garantie absente, puisque
    /// tout le chemin a l'air de fonctionner.
    QuotaWithoutWritableSpace {
        /// Le niveau qui monte la racine en lecture seule.
        level: SandboxLevel,
    },
    /// Un quota disque demandé sur un espace que **le runtime ne sait pas borner** — `W5.y`.
    ///
    /// Distinct de [`PlanError::QuotaWithoutWritableSpace`], et la distinction envoie à des endroits
    /// opposés : « rien à borner » se répare en montant un espace de travail, « la borne n'est pas
    /// applicable » se répare en changeant d'hôte ou de système de fichiers. C'est la règle de
    /// `W5.h`, et c'est aussi ce que §10.2 sépare en `capacity_exceeded` et
    /// `disk_quota_not_enforceable`.
    ///
    /// Le cas : à partir de `S2` la racine est en lecture seule, donc le seul espace inscriptible
    /// est un montage — et tout montage est un **bind**, dont Podman ne dimensionne pas la taille.
    /// La borne existe côté hôte (quota de projet sur le répertoire source), jamais côté runtime.
    DiskQuotaNotEnforceable {
        /// Le niveau qui monte la racine en lecture seule.
        level: SandboxLevel,
        /// L'espace de travail concerné, vu de l'intérieur.
        target: String,
    },
}

impl fmt::Display for PlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LevelBeyondBackend { required, ceiling } => write!(
                formatter,
                "mission en {}, backend rootless au mieux en {}",
                required.code(),
                ceiling.code()
            ),
            Self::NetworkNeedsIsolation { mode, minimum } => write!(
                formatter,
                "le mode réseau « {mode} » exige un namespace réseau, donc {} au minimum",
                minimum.code()
            ),
            Self::IsolationContradictsNetwork { level } => write!(
                formatter,
                "{} est le niveau au réseau isolé : y brancher le réseau de l'hôte le viderait — demander {} et « full »",
                level.code(),
                SandboxLevel::S2.code()
            ),
            Self::MountsNeedNamespace => formatter.write_str(
                "un montage a été demandé en S0, qui n'a pas de vue du système de fichiers à lui",
            ),
            Self::QuotaNeedsContainment => formatter
                .write_str("un quota disque a été demandé en S0, qui ne contient aucune écriture"),
            Self::DiskQuotaNotEnforceable { level, target } => write!(
                formatter,
                "un quota disque a été demandé en {}, dont la racine est en lecture seule : le seul espace inscriptible est le montage « {target} », et un montage lié n'a pas de taille que le runtime sache borner — la borne se pose sur l'hôte, par un quota de projet sur le répertoire source",
                level.code()
            ),
            Self::QuotaWithoutWritableSpace { level } => write!(
                formatter,
                "un quota disque a été demandé en {}, qui monte la racine en lecture seule, et aucun espace de travail n'est monté : la borne n'aurait rien à borner",
                level.code()
            ),
        }
    }
}

impl std::error::Error for PlanError {}
