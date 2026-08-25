//! Ce qu'on demande à Podman, mot pour mot.
//!
//! # Pourquoi cette fonction est pure
//!
//! Elle rend un vecteur d'arguments et ne lance rien. C'est ce qui permet de vérifier, sans
//! Podman et sans privilèges, que le confinement demandé est bien celui que
//! [`super::plan::ConfinementPlan`] a décidé. Un driver qui construirait ses arguments au moment
//! de lancer ne serait testable que là où un runtime tourne — c'est-à-dire nulle part en CI, et
//! une garantie qui ne s'exécute pas n'est pas une garantie.

use std::fmt;

use super::plan::{
    ConfinementPlan, MountPlan, Namespace, NetworkPosture, QuotaTarget, SeccompPosture,
};
use super::seccomp::RestrictedProfile;

/// Ce que la sandbox exécutera.
///
/// L'image est désignée **par digest**. `docs/03` l'exige au titre de l'attestation, §19.3 au
/// titre de la reproductibilité : une étiquette désigne une image différente selon le jour, donc
/// un run qu'on ne saura pas refaire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workload {
    image: String,
    command: Vec<String>,
}

impl Workload {
    /// Déclarer l'image et la commande.
    ///
    /// # Errors
    ///
    /// [`InvocationError::ImageWithoutDigest`] pour une image qui ne porte pas de digest,
    /// [`InvocationError::EmptyCommand`] pour une commande vide — le point d'entrée de l'image
    /// serait alors implicite, et l'attestation ne dirait pas ce qui a tourné.
    pub fn new(image: &str, command: Vec<String>) -> Result<Self, InvocationError> {
        if !image.contains("@sha256:") {
            return Err(InvocationError::ImageWithoutDigest {
                image: image.to_owned(),
            });
        }
        if command.iter().all(|part| part.trim().is_empty()) {
            return Err(InvocationError::EmptyCommand);
        }
        Ok(Self {
            image: image.to_owned(),
            command,
        })
    }

    /// L'image, digest compris.
    #[must_use]
    pub fn image(&self) -> &str {
        &self.image
    }

    /// La commande.
    #[must_use]
    pub fn command(&self) -> &[String] {
        &self.command
    }
}

/// Le profil seccomp restreint, quand le déploiement en fournit un.
///
/// # Pourquoi c'est une configuration, et vérifiée
///
/// `SeccompPosture::Restricted` promet le refus, depuis l'intérieur, de la création de namespaces
/// et du chargement de code noyau. Ce refus vit dans un fichier de profil que ce dépôt n'écrit pas :
/// un profil par défaut-refus est une liste de plusieurs centaines d'appels autorisés dont
/// l'exactitude ne se démontre qu'en l'exécutant. Le déploiement l'apporte donc, et
/// [`super::seccomp::RestrictedProfile`] vérifie qu'il refuse bien ce que la posture promet.
///
/// Tant qu'aucun profil n'est fourni, le backend **refuse** les niveaux qui en dépendent au lieu de
/// les revendiquer avec le profil par défaut du runtime — dont ce dépôt ne vérifie pas le contenu,
/// et sur lequel il ne fait donc aucune promesse. C'est la règle du plafond `S3`, appliquée à une
/// capacité que l'opérateur apporte.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeccompProfiles {
    /// Le profil restreint, lu et vérifié, s'il y en a un.
    pub restricted: Option<RestrictedProfile>,
}

/// Les arguments de `podman create` qui réalisent ce plan.
///
/// # Errors
///
/// [`InvocationError::RestrictedProfileMissing`] quand le plan demande la posture restreinte et
/// que le déploiement n'a fourni aucun profil.
pub fn create_arguments(
    plan: &ConfinementPlan,
    workload: &Workload,
    profiles: &SeccompProfiles,
    name: &str,
) -> Result<Vec<String>, InvocationError> {
    let mut arguments = vec!["create".to_owned(), "--name".to_owned(), name.to_owned()];
    arguments.extend(namespace_arguments(plan));
    arguments.extend(quota_arguments(plan));
    arguments.extend(security_arguments(plan, profiles)?);
    arguments.extend(plan.mounts().iter().flat_map(mount_argument));
    arguments.push(workload.image().to_owned());
    arguments.extend(workload.command().iter().cloned());
    Ok(arguments)
}

/// Les arguments d'inspection, et le gabarit qui décide de ce qu'on relit.
///
/// Le gabarit rend une ligne `clé=valeur` par constat plutôt qu'un JSON : ce qui est relu est
/// alors une liste explicite, visible dans le diff, et non tout ce que Podman voudra bien dire.
#[must_use]
pub fn inspect_arguments(name: &str) -> Vec<String> {
    vec![
        "inspect".to_owned(),
        "--format".to_owned(),
        INSPECT_TEMPLATE.to_owned(),
        name.to_owned(),
    ]
}

/// Les champs relus de ce qui tourne, dans l'ordre du gabarit.
pub const INSPECTED_FIELDS: [&str; 12] = [
    "status",
    "memory",
    "pids",
    "cpu_quota",
    "cpu_period",
    "readonly",
    "network",
    "userns",
    "pidns",
    "ipcns",
    "utsns",
    "security",
];

const INSPECT_TEMPLATE: &str = concat!(
    "status={{.State.Status}}\n",
    "memory={{.HostConfig.Memory}}\n",
    "pids={{.HostConfig.PidsLimit}}\n",
    "cpu_quota={{.HostConfig.CpuQuota}}\n",
    "cpu_period={{.HostConfig.CpuPeriod}}\n",
    "readonly={{.HostConfig.ReadonlyRootfs}}\n",
    "network={{.HostConfig.NetworkMode}}\n",
    "userns={{.HostConfig.UsernsMode}}\n",
    "pidns={{.HostConfig.PidMode}}\n",
    "ipcns={{.HostConfig.IpcMode}}\n",
    "utsns={{.HostConfig.UTSMode}}\n",
    "security={{join .HostConfig.SecurityOpt \",\"}}",
);

/// `--foo=host` quand le namespace n'est **pas** demandé.
///
/// La forme négative est celle de Podman, et elle mérite d'être dite : ne rien passer ne laisse
/// pas le namespace partagé, ça le crée. Un plan qui oublierait un argument confinerait donc plus
/// que demandé — le sur-confinement de W4.b — et non moins.
fn namespace_arguments(plan: &ConfinementPlan) -> Vec<String> {
    let shared = |namespace: Namespace, flag: &str| -> Option<String> {
        (!plan.namespaces().contains(&namespace)).then(|| format!("{flag}=host"))
    };
    let mut arguments: Vec<String> = [
        shared(Namespace::User, "--userns"),
        shared(Namespace::Pid, "--pid"),
        shared(Namespace::Ipc, "--ipc"),
        shared(Namespace::Uts, "--uts"),
        shared(Namespace::Cgroup, "--cgroupns"),
    ]
    .into_iter()
    .flatten()
    .collect();
    arguments.push(match plan.network() {
        NetworkPosture::Isolated => "--network=none".to_owned(),
        NetworkPosture::ConnectorsOnly | NetworkPosture::ProxiedAllowlist { .. } => {
            "--network=slirp4netns".to_owned()
        }
        NetworkPosture::Host => "--network=host".to_owned(),
    });
    arguments
}

fn quota_arguments(plan: &ConfinementPlan) -> Vec<String> {
    let mut arguments: Vec<String> = plan
        .cgroup()
        .iter()
        .flat_map(|limit| match limit.file {
            "memory.max" => vec![format!("--memory={}", limit.value)],
            "pids.max" => vec![format!("--pids-limit={}", limit.value)],
            "cpu.max" => {
                let mut parts = limit.value.split_whitespace();
                match (parts.next(), parts.next()) {
                    (Some(quota), Some(period)) => vec![
                        format!("--cpu-quota={quota}"),
                        format!("--cpu-period={period}"),
                    ],
                    _ => Vec::new(),
                }
            }
            _ => Vec::new(),
        })
        .collect();
    arguments.extend(disk_quota_arguments(plan));
    arguments
}

/// Le quota disque, appliqué **là où le plan dit qu'il mord**.
///
/// # Ce que la rédaction précédente supposait
///
/// Elle écrivait `--storage-opt size=` dès que `disk_bytes > 0`, ce qui dimensionne la couche
/// inscriptible du conteneur. À partir de `S2` cette couche est montée en lecture seule : le quota
/// était transmis au runtime et ne bornait rien. Voir [`QuotaTarget`].
///
/// # Ce qui est appliqué, et ce qui reste dû
///
/// [`QuotaTarget::WritableRoot`] rend le `--storage-opt`, qui est juste là. [`QuotaTarget::None`] ne
/// rend rien, parce qu'il n'y a rien à borner.
///
/// Il n'y a **pas de troisième cas**, et c'est `W5.s` qui l'a retiré. Une variante `Workspace`
/// rendait ici un volume dimensionné monté au point de travail ; comme ce point venait d'un montage
/// déjà déclaré, Podman recevait deux montages sur la même destination et refusait la spécification
/// entière. Le plan refuse désormais en amont, par `PlanError::DiskQuotaNotEnforceable`, ce qui est
/// le chemin que `W5.g` décrivait déjà : l'hôte est interrogé, et la mission est refusée quand la
/// borne n'est pas applicable — au lieu d'être transmise sous une forme que le runtime rejette.
fn disk_quota_arguments(plan: &ConfinementPlan) -> Vec<String> {
    match plan.quota_target() {
        QuotaTarget::None => Vec::new(),
        QuotaTarget::WritableRoot => vec![
            "--storage-opt".to_owned(),
            format!("size={}", plan.disk_bytes()),
        ],
    }
}

/// L'horizon n'apparaît pas ici, et c'est délibéré : `ConfinementPlan::wall_clock_seconds` est
/// compté par le broker. Le passer à Podman ferait croire qu'un runtime le tient.
fn security_arguments(
    plan: &ConfinementPlan,
    profiles: &SeccompProfiles,
) -> Result<Vec<String>, InvocationError> {
    let mut arguments = Vec::new();
    if plan.read_only_rootfs() {
        arguments.push("--read-only".to_owned());
    }
    if plan.no_new_privileges() {
        arguments.push("--security-opt".to_owned());
        arguments.push("no-new-privileges".to_owned());
    }
    match plan.seccomp() {
        SeccompPosture::Unconfined => {
            arguments.push("--security-opt".to_owned());
            arguments.push("seccomp=unconfined".to_owned());
        }
        SeccompPosture::Baseline => {}
        SeccompPosture::Restricted => {
            let Some(profile) = profiles.restricted.as_ref() else {
                return Err(InvocationError::RestrictedProfileMissing);
            };
            arguments.push("--security-opt".to_owned());
            arguments.push(format!("seccomp={}", profile.path()));
        }
    }
    arguments.extend(
        plan.dropped_capabilities()
            .iter()
            .map(|capability| format!("--cap-drop={capability}")),
    );
    Ok(arguments)
}

fn mount_argument(mount: &MountPlan) -> Vec<String> {
    let mode = if mount.read_only { ",ro" } else { "" };
    vec![
        "--mount".to_owned(),
        format!(
            "type=bind,source={},destination={}{mode}",
            mount.source, mount.target
        ),
    ]
}

/// Ce qui empêche de construire une invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvocationError {
    /// Une image désignée par étiquette plutôt que par digest.
    ImageWithoutDigest {
        /// L'image refusée.
        image: String,
    },
    /// Une commande vide, qui laisserait le point d'entrée de l'image décider.
    EmptyCommand,
    /// La posture restreinte demandée sans profil pour la porter.
    RestrictedProfileMissing,
}

impl fmt::Display for InvocationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ImageWithoutDigest { image } => write!(
                formatter,
                "« {image} » n'a pas de digest : le run ne serait pas reproductible"
            ),
            Self::EmptyCommand => {
                formatter.write_str("commande vide : le point d'entrée de l'image déciderait")
            }
            Self::RestrictedProfileMissing => formatter.write_str(
                "aucun profil seccomp restreint n'est configuré : le déploiement ne peut pas \
                 revendiquer cette posture",
            ),
        }
    }
}

impl std::error::Error for InvocationError {}
