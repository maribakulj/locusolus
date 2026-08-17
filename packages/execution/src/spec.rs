//! Ce qu'une mission exige de sa sandbox — `docs/SPEC_V1.md` §21.6 et §21.7.

use std::fmt;

use crate::approval::Approval;
use crate::level::{SandboxLevel, SandboxProfile};
use crate::resources::ResourceSpec;

/// Le mode réseau — §21.7.
///
/// « Modes réseau : `deny`, `connector_only`, `allowlist`, `full` ». Les quatre sont transcrits, et
/// il n'y a pas de `Default` : CLAUDE.md dit « réseau deny-by-default pour code non fiable », ce qui
/// est une règle de **politique** et non une valeur par défaut de type. Un défaut ici la rendrait
/// invisible, et le jour où quelqu'un le changerait, plus rien ne dirait qu'une règle a bougé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkMode {
    /// Aucun réseau.
    Deny,
    /// Uniquement les connecteurs déclarés.
    ConnectorOnly,
    /// Une liste d'hôtes autorisés, passant par le proxy d'egress.
    Allowlist {
        /// Les hôtes autorisés.
        hosts: Vec<String>,
    },
    /// Réseau complet.
    Full,
}

impl NetworkMode {
    /// Le nom du mode, tel que §21.7 l'écrit.
    #[must_use]
    pub const fn slug(&self) -> &'static str {
        match self {
            Self::Deny => "deny",
            Self::ConnectorOnly => "connector_only",
            Self::Allowlist { .. } => "allowlist",
            Self::Full => "full",
        }
    }

    /// Une allowlist non vide.
    ///
    /// # Errors
    ///
    /// [`SpecError::EmptyAllowlist`] pour une liste vide. Elle serait un `deny` qui n'en a pas le
    /// nom : le mode dirait « allowlist » dans les journaux et dans les audits, et rien ne
    /// passerait — un refus qu'on chercherait ailleurs pendant des heures.
    pub fn allowlist(hosts: Vec<String>) -> Result<Self, SpecError> {
        if hosts.iter().all(|host| host.trim().is_empty()) {
            return Err(SpecError::EmptyAllowlist);
        }
        Ok(Self::Allowlist { hosts })
    }
}

impl fmt::Display for NetworkMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Comment un chemin est monté.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MountMode {
    /// Lecture seule.
    ReadOnly,
    /// Lecture et écriture.
    ReadWrite,
}

/// Ce qui ne se monte jamais dans une sandbox sans approbation nommée.
///
/// # D'où vient cette liste
///
/// CLAUDE.md, section Sécurité : « Ne monte jamais le home utilisateur, le socket Docker/Podman ou
/// un répertoire de secrets dans une sandbox par défaut. » Les trois familles sont ici, et le mot
/// « par défaut » est rendu par [`Mount::approved`] — la dérogation existe, elle porte un nom et
/// une raison, et elle produit un événement de sécurité.
///
/// Monter le socket du runtime est le cas qui mérite d'être nommé : il donne à la sandbox le
/// pouvoir de créer des conteneurs privilégiés, c'est-à-dire de se libérer elle-même. Ce n'est pas
/// une fuite de données, c'est l'annulation du confinement par l'intérieur.
pub const FORBIDDEN_MOUNT_MARKERS: [&str; 14] = [
    "/root",
    "/home/",
    "docker.sock",
    "podman.sock",
    "containerd.sock",
    "crio.sock",
    "/var/run/docker",
    "/.ssh",
    "/.aws",
    "/.kube",
    "/.gnupg",
    "/etc/shadow",
    "/secrets",
    "/run/secrets",
];

/// Un montage déclaré par la mission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mount {
    source: String,
    target: String,
    mode: MountMode,
    approval: Option<Approval>,
}

impl Mount {
    /// Déclarer un montage ordinaire.
    ///
    /// # Errors
    ///
    /// [`SpecError::ForbiddenMount`] quand la source touche l'une des familles de
    /// [`FORBIDDEN_MOUNT_MARKERS`], [`SpecError::EmptyPath`] pour un chemin vide,
    /// [`SpecError::RelativePath`] pour un chemin relatif — il se résoudrait contre un répertoire
    /// courant que la sandbox ne partage pas, donc contre autre chose que ce qui était voulu.
    pub fn new(source: &str, target: &str, mode: MountMode) -> Result<Self, SpecError> {
        Self::check_paths(source, target)?;
        if let Some(marker) = forbidden_marker(source) {
            return Err(SpecError::ForbiddenMount {
                source: source.to_owned(),
                marker,
            });
        }
        Ok(Self {
            source: source.to_owned(),
            target: target.to_owned(),
            mode,
            approval: None,
        })
    }

    /// Déclarer un montage que la liste interdit, sous approbation nommée.
    ///
    /// La dérogation ne se cache pas : elle porte qui l'a donnée et pourquoi, et
    /// [`crate::attestation::conformance`] en tire un événement de sécurité. C'est la même forme
    /// que le downgrade de niveau, et pour la même raison — les deux annulent une garantie, et une
    /// garantie annulée sans trace est une garantie qu'on croit encore avoir.
    ///
    /// # Errors
    ///
    /// [`SpecError::EmptyPath`] ou [`SpecError::RelativePath`] selon les chemins,
    /// [`SpecError::PointlessApproval`] si le montage n'avait besoin d'aucune dérogation —
    /// approuver ce qui est déjà permis banalise l'approbation.
    pub fn approved(
        source: &str,
        target: &str,
        mode: MountMode,
        approval: Approval,
    ) -> Result<Self, SpecError> {
        Self::check_paths(source, target)?;
        if forbidden_marker(source).is_none() {
            return Err(SpecError::PointlessApproval {
                source: source.to_owned(),
            });
        }
        Ok(Self {
            source: source.to_owned(),
            target: target.to_owned(),
            mode,
            approval: Some(approval),
        })
    }

    fn check_paths(source: &str, target: &str) -> Result<(), SpecError> {
        for path in [source, target] {
            if path.trim().is_empty() {
                return Err(SpecError::EmptyPath);
            }
            if !path.starts_with('/') {
                return Err(SpecError::RelativePath {
                    path: path.to_owned(),
                });
            }
        }
        Ok(())
    }

    /// Le chemin côté hôte.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Le chemin dans la sandbox.
    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Lecture seule ou lecture-écriture.
    #[must_use]
    pub const fn mode(&self) -> MountMode {
        self.mode
    }

    /// L'approbation qui a permis un montage interdit, s'il y en a une.
    #[must_use]
    pub const fn approval(&self) -> Option<&Approval> {
        self.approval.as_ref()
    }
}

/// Le marqueur interdit que ce chemin porte, s'il en porte un.
///
/// La comparaison est faite sur le chemin **en minuscules** et sur des sous-chaînes : un chemin
/// n'est pas un identifiant, et `/var/run/Docker.sock` monte le même socket.
#[must_use]
pub fn forbidden_marker(source: &str) -> Option<&'static str> {
    let lowered = source.to_lowercase();
    FORBIDDEN_MOUNT_MARKERS
        .into_iter()
        .find(|marker| lowered.contains(marker))
}

/// Ce qu'une mission exige de sa sandbox — §21.6.
///
/// « La mission impose un niveau minimal. » D'où `minimum_level` et non `level` : ce type dit un
/// plancher, pas une cible, et [`crate::attestation::SandboxAttestation`] dira ce qui a réellement
/// été appliqué.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxSpec {
    minimum_level: SandboxLevel,
    profile: SandboxProfile,
    network: NetworkMode,
    mounts: Vec<Mount>,
    resources: ResourceSpec,
}

impl SandboxSpec {
    /// Déclarer ce qu'une mission exige.
    ///
    /// # Errors
    ///
    /// [`SpecError::DuplicateTarget`] si deux montages visent le même point dans la sandbox — le
    /// second masquerait le premier, et lequel des deux dépendrait de l'ordre d'application.
    pub fn new(
        minimum_level: SandboxLevel,
        profile: SandboxProfile,
        network: NetworkMode,
        mounts: Vec<Mount>,
        resources: ResourceSpec,
    ) -> Result<Self, SpecError> {
        let mut targets: Vec<&str> = mounts.iter().map(Mount::target).collect();
        targets.sort_unstable();
        if let Some(window) = targets.windows(2).find(|window| window[0] == window[1]) {
            return Err(SpecError::DuplicateTarget {
                target: window[0].to_owned(),
            });
        }
        Ok(Self {
            minimum_level,
            profile,
            network,
            mounts,
            resources,
        })
    }

    /// Le plancher d'isolation exigé.
    #[must_use]
    pub const fn minimum_level(&self) -> SandboxLevel {
        self.minimum_level
    }

    /// Le profil demandé.
    #[must_use]
    pub const fn profile(&self) -> SandboxProfile {
        self.profile
    }

    /// Le mode réseau.
    #[must_use]
    pub const fn network(&self) -> &NetworkMode {
        &self.network
    }

    /// Les montages.
    #[must_use]
    pub fn mounts(&self) -> &[Mount] {
        &self.mounts
    }

    /// Les ressources réservées.
    #[must_use]
    pub const fn resources(&self) -> &ResourceSpec {
        &self.resources
    }

    /// Les montages qui n'existent que par dérogation.
    #[must_use]
    pub fn approved_mounts(&self) -> Vec<&Mount> {
        self.mounts
            .iter()
            .filter(|mount| mount.approval().is_some())
            .collect()
    }
}

/// Ce qui empêche une spécification d'exister.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecError {
    /// Un chemin vide.
    EmptyPath,
    /// Un chemin relatif, qui se résoudrait contre un répertoire courant non partagé.
    RelativePath {
        /// Le chemin fautif.
        path: String,
    },
    /// Une source que CLAUDE.md interdit de monter sans dérogation.
    ForbiddenMount {
        /// La source.
        source: String,
        /// Le marqueur reconnu.
        marker: &'static str,
    },
    /// Une approbation pour un montage qui n'en avait pas besoin.
    PointlessApproval {
        /// La source.
        source: String,
    },
    /// Deux montages visent le même point dans la sandbox.
    DuplicateTarget {
        /// Le point visé deux fois.
        target: String,
    },
    /// Une allowlist vide, qui serait un `deny` sous un autre nom.
    EmptyAllowlist,
}

impl fmt::Display for SpecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPath => formatter.write_str("chemin de montage vide"),
            Self::RelativePath { path } => write!(
                formatter,
                "« {path} » est relatif : il se résoudrait contre un répertoire que la sandbox ne partage pas"
            ),
            Self::ForbiddenMount { source, marker } => write!(
                formatter,
                "« {source} » touche « {marker} », que la politique interdit de monter sans approbation nommée"
            ),
            Self::PointlessApproval { source } => write!(
                formatter,
                "« {source} » n'a besoin d'aucune dérogation : approuver ce qui est permis banalise l'approbation"
            ),
            Self::DuplicateTarget { target } => write!(
                formatter,
                "deux montages visent « {target} » : lequel gagne dépendrait de l'ordre d'application"
            ),
            Self::EmptyAllowlist => formatter.write_str(
                "une allowlist vide est un deny qui n'en a pas le nom, et le refus se chercherait ailleurs",
            ),
        }
    }
}

impl std::error::Error for SpecError {}
