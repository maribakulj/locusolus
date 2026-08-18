//! Le `deployment.yaml` relu — `docs/SPEC_V1.md` §27.3 et `docs/05`.
//!
//! # Les secrets sont dehors, et il n'y a pas d'endroit où les mettre
//!
//! `docs/05` : « les secrets sont externes ». Ce n'est pas une consigne d'hygiène, c'est une
//! propriété du format : le document n'offre **aucun champ** où écrire une valeur. `secret_refs` ne
//! prend que des références, et le schéma refuse par motif ce qui n'en est pas une.
//!
//! La raison est qu'un secret écrit dans un fichier de configuration ne s'arrête pas là. Il part
//! dans un dépôt, dans une sauvegarde, dans un rapport de bug, dans le presse-papier de qui
//! diagnostique — et aucune de ces copies ne se révoque.
//!
//! Ce module ne résout aucune référence. `explain` imprime `nom ← référence`, jamais la valeur : une
//! commande de diagnostic qui résoudrait ses secrets pour « aider » les afficherait sur le terminal
//! de quiconque la lance, et dans le journal de session qui va avec.
//!
//! # Ce que le schéma ne peut pas dire
//!
//! Deux choses, et le lecteur les ajoute. Un rôle déclaré **deux fois** : la liste est une liste
//! justement pour que ce soit détectable — un objet JSON aurait laissé le second écraser le premier
//! sans bruit. Et un profil hors des cinq, puisque le type engendré ne porte l'énumération que
//! comme une chaîne.

use std::collections::BTreeSet;
use std::fmt;
use std::fmt::Write as _;

use locus_lep::Deployment as Wire;

use crate::{Profile, ProfileError, ProfileKind};

/// Où un secret se trouve — jamais ce qu'il vaut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SecretScheme {
    /// Une variable d'environnement.
    Env,
    /// Un fichier, hors du dépôt.
    File,
    /// Le trousseau du système.
    Keychain,
    /// Un coffre externe.
    Vault,
}

impl SecretScheme {
    /// Les quatre que le schéma autorise.
    pub const ALL: [Self; 4] = [Self::Env, Self::File, Self::Keychain, Self::Vault];

    /// Son préfixe.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Env => "env",
            Self::File => "file",
            Self::Keychain => "keychain",
            Self::Vault => "vault",
        }
    }

    /// Le relire depuis un préfixe.
    #[must_use]
    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|scheme| scheme.slug() == slug)
    }
}

impl fmt::Display for SecretScheme {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Où trouver un secret.
///
/// Il n'y a pas de champ « valeur », et il n'y en aura pas : ce type existe pour que la valeur
/// n'ait nulle part où se poser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretRef {
    name: String,
    scheme: SecretScheme,
    locator: String,
}

impl SecretRef {
    /// Le nom sous lequel le déploiement s'y réfère.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Où chercher.
    #[must_use]
    pub const fn scheme(&self) -> SecretScheme {
        self.scheme
    }

    /// La référence entière, telle qu'elle est écrite.
    #[must_use]
    pub fn reference(&self) -> String {
        format!("{}:{}", self.scheme, self.locator)
    }
}

/// Un déploiement configuré.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentConfig {
    profile: Profile,
    adapters: Vec<(String, String)>,
    secrets: Vec<SecretRef>,
}

impl DeploymentConfig {
    /// Relire un document.
    ///
    /// # Errors
    ///
    /// [`ConfigError::UnknownProfile`] pour un profil hors des cinq de §27.1,
    /// [`ConfigError::DuplicateRole`] pour un rôle déclaré deux fois,
    /// [`ConfigError::MalformedSecretRef`] pour une référence sans schéma reconnu, et
    /// [`ConfigError::Profile`] pour ce que [`Profile::declare`] refuse.
    pub fn from_wire(wire: &Wire) -> Result<Self, ConfigError> {
        let kind =
            ProfileKind::from_slug(&wire.profile).ok_or_else(|| ConfigError::UnknownProfile {
                value: wire.profile.clone(),
            })?;

        let mut roles = BTreeSet::new();
        let mut adapters = Vec::new();
        for adapter in &wire.adapters {
            if !roles.insert(adapter.role.clone()) {
                return Err(ConfigError::DuplicateRole {
                    role: adapter.role.clone(),
                });
            }
            adapters.push((adapter.role.clone(), adapter.implementation.clone()));
        }

        let implementations: Vec<&str> = adapters
            .iter()
            .map(|(_, implementation)| implementation.as_str())
            .collect();
        let mut profile = Profile::declare(kind, &wire.endpoint, &implementations)
            .map_err(ConfigError::Profile)?;
        for capability in wire.capabilities.as_deref().unwrap_or_default() {
            profile = profile.announcing(capability);
        }

        let mut secrets = Vec::new();
        for declared in wire.secret_refs.as_deref().unwrap_or_default() {
            secrets.push(read_secret(&declared.name, &declared.reference)?);
        }

        Ok(Self {
            profile,
            adapters,
            secrets,
        })
    }

    /// Le profil que ce document déclare.
    #[must_use]
    pub const fn profile(&self) -> &Profile {
        &self.profile
    }

    /// Les rôles et leurs implémentations, dans l'ordre du document.
    #[must_use]
    pub fn adapters(&self) -> &[(String, String)] {
        &self.adapters
    }

    /// Où trouver les secrets.
    #[must_use]
    pub fn secrets(&self) -> &[SecretRef] {
        &self.secrets
    }

    /// Ce que `locus deployment explain` imprime — §27.2.
    ///
    /// « Exactement quels backends sont actifs » : tous les rôles déclarés, et aucun autre. Les
    /// secrets y figurent par leur **référence**, parce que savoir où un déploiement va chercher son
    /// mot de passe fait partie du diagnostic ; les résoudre pour les afficher le mettrait sur le
    /// terminal de qui lance la commande, et dans le journal de session qui va avec.
    #[must_use]
    pub fn explain(&self) -> String {
        let mut rendu = format!("profil : {}\n", self.profile.kind());
        for (role, implementation) in &self.adapters {
            let _ = writeln!(rendu, "backend {role} : {implementation}");
        }
        for secret in &self.secrets {
            let _ = writeln!(rendu, "secret {} ← {}", secret.name, secret.reference());
        }
        rendu
    }
}

fn read_secret(name: &str, reference: &str) -> Result<SecretRef, ConfigError> {
    if name.trim().is_empty() {
        return Err(ConfigError::Profile(ProfileError::EmptyField {
            field: "secret.name",
        }));
    }
    let (scheme, locator) =
        reference
            .split_once(':')
            .ok_or_else(|| ConfigError::MalformedSecretRef {
                value: reference.to_owned(),
            })?;
    let scheme =
        SecretScheme::from_slug(scheme).ok_or_else(|| ConfigError::MalformedSecretRef {
            value: reference.to_owned(),
        })?;
    if locator.trim().is_empty() {
        return Err(ConfigError::MalformedSecretRef {
            value: reference.to_owned(),
        });
    }
    Ok(SecretRef {
        name: name.to_owned(),
        scheme,
        locator: locator.to_owned(),
    })
}

/// Ce qui empêche un document d'être relu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// Un profil hors des cinq de §27.1.
    UnknownProfile {
        /// La valeur reçue.
        value: String,
    },
    /// Un rôle déclaré deux fois.
    DuplicateRole {
        /// Lequel.
        role: String,
    },
    /// Une référence de secret que rien ne sait suivre.
    MalformedSecretRef {
        /// La valeur reçue.
        value: String,
    },
    /// Ce que le profil lui-même refuse.
    Profile(ProfileError),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownProfile { value } => write!(
                formatter,
                "« {value} » n'est pas un des cinq profils de §27.1"
            ),
            Self::DuplicateRole { role } => write!(
                formatter,
                "le rôle « {role} » est déclaré deux fois : lequel des deux backends est actif ?"
            ),
            Self::MalformedSecretRef { value } => write!(
                formatter,
                "« {value} » n'est pas une référence de secret — un secret ne s'écrit pas dans \
                 ce fichier, on dit où le trouver"
            ),
            Self::Profile(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for ConfigError {}
