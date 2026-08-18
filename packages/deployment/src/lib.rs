//! Les profils de déploiement et le verdict de `locus doctor` — `docs/SPEC_V1.md` §27.
//!
//! # La phrase qui décide de la forme du crate
//!
//! §27.2 : « `locus doctor` **vérifie** dépendances, ports, versions, ressources, attestations et
//! accès. » Et `docs/05` : « `locus doctor` vérifie que le profil est réellement exécutable **avant
//! d'accepter des campagnes**. »
//!
//! Un profil qui se déclarerait exécutable est exactement ce que cette commande existe pour
//! empêcher. Le type le rend impossible : [`Profile`] ne sait pas dire s'il est exécutable, et
//! [`Readiness`] ne se construit qu'en confrontant ce que le profil **exige** à ce qu'un inventaire
//! **constate**. Cinquième occurrence de la même forme dans ce chantier, après l'attestation de
//! sandbox, le digest de build, le niveau de reproductibilité et l'attestation d'indépendance : ce
//! qui prouve ne peut pas être ce qui est demandé.
//!
//! # « Pas vérifié » n'est pas « présent »
//!
//! Un inventaire peut ne pas savoir. Un port qu'on n'a pas pu sonder, une version qu'on n'a pas pu
//! lire : ce n'est ni une présence ni une absence, et [`Presence::Unknown`] le dit. Un profil qui
//! compterait l'ignorance comme un succès serait déclaré exécutable par une panne de la sonde —
//! c'est-à-dire précisément quand il ne faut pas.
//!
//! # Le client voit une URL, pas une topologie
//!
//! `docs/05` : « les clients se connectent à une URL Locus. Ils ne connaissent pas la topologie
//! interne. » [`Profile::client_surface`] ne rend donc que l'URL publique et l'ensemble des
//! capabilities annoncées. Deux profils qui déploient la même API sur des topologies opposées
//! rendent la même valeur, et un test le vérifie par égalité — la fuite d'un nom d'hôte interne
//! serait une carte du réseau offerte à qui n'en a pas besoin.

pub mod config;

pub use config::{ConfigError, DeploymentConfig, SecretRef, SecretScheme};

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Les cinq profils obligatoires de §27.1, sous leur nom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProfileKind {
    /// Tout sur un poste personnel.
    PersonalLocal,
    /// Control plane sur une machine dédiée, cockpit ailleurs.
    PersonalNode,
    /// Une VM Linux qui héberge services et workers CPU.
    SingleNodeVm,
    /// Les ports d'infrastructure sur les services d'une plateforme.
    CloudPlatform,
    /// Control plane d'un côté, plusieurs workers LEP ailleurs.
    DistributedHybrid,
}

impl ProfileKind {
    /// Les cinq de §27.1, dans l'ordre où le texte les nomme.
    pub const ALL: [Self; 5] = [
        Self::PersonalLocal,
        Self::PersonalNode,
        Self::SingleNodeVm,
        Self::CloudPlatform,
        Self::DistributedHybrid,
    ];

    /// Son nom, tel que `--profile` l'attend.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::PersonalLocal => "personal-local",
            Self::PersonalNode => "personal-node",
            Self::SingleNodeVm => "single-node-vm",
            Self::CloudPlatform => "cloud-platform",
            Self::DistributedHybrid => "distributed-hybrid",
        }
    }

    /// Le relire depuis `--profile`.
    #[must_use]
    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.slug() == slug)
    }
}

impl fmt::Display for ProfileKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Ce qu'une sonde a constaté d'un adaptateur.
///
/// Trois valeurs, et la troisième est celle qui compte : une sonde qui n'a pas pu répondre n'a rien
/// constaté. La compter comme une présence ferait déclarer un profil exécutable par une panne de la
/// sonde — au moment précis où il ne faut pas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Presence {
    /// Constaté présent.
    Present,
    /// Constaté absent.
    Absent,
    /// La sonde n'a rien pu constater.
    Unknown,
}

/// Ce qu'un profil exige et ce qu'il expose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    kind: ProfileKind,
    endpoint: String,
    adapters: BTreeSet<String>,
    capabilities: BTreeSet<String>,
}

impl Profile {
    /// Déclarer un profil.
    ///
    /// # Errors
    ///
    /// [`ProfileError::EmptyField`] pour une URL ou un nom d'adaptateur vide, et
    /// [`ProfileError::NoAdapter`] pour un profil qui n'exige rien : un profil sans adaptateur
    /// passerait toute vérification sans rien avoir vérifié, ce qui est la façon la plus discrète
    /// de rendre `locus doctor` inutile.
    pub fn declare(
        kind: ProfileKind,
        endpoint: &str,
        adapters: &[&str],
    ) -> Result<Self, ProfileError> {
        if endpoint.trim().is_empty() {
            return Err(ProfileError::EmptyField { field: "endpoint" });
        }
        if adapters.is_empty() {
            return Err(ProfileError::NoAdapter);
        }
        let mut declared = BTreeSet::new();
        for adapter in adapters {
            if adapter.trim().is_empty() {
                return Err(ProfileError::EmptyField { field: "adapter" });
            }
            declared.insert((*adapter).to_owned());
        }
        Ok(Self {
            kind,
            endpoint: endpoint.to_owned(),
            adapters: declared,
            capabilities: BTreeSet::new(),
        })
    }

    /// Annoncer une capability.
    ///
    /// §27.1 pour `cloud-platform` : « les limites CPU/RAM/disque/absence de GPU sont déclarées
    /// comme capabilities, non contournées. »
    #[must_use]
    pub fn announcing(mut self, capability: &str) -> Self {
        self.capabilities.insert(capability.to_owned());
        self
    }

    /// Lequel des cinq.
    #[must_use]
    pub const fn kind(&self) -> ProfileKind {
        self.kind
    }

    /// Les adaptateurs qu'il exige.
    #[must_use]
    pub const fn adapters(&self) -> &BTreeSet<String> {
        &self.adapters
    }

    /// Ce qu'un client voit — une URL et des capabilities, jamais une topologie.
    ///
    /// Deux profils qui déploient la même API sur des topologies opposées rendent la **même**
    /// valeur. Laisser filtrer un nom d'hôte interne offrirait une carte du réseau à qui n'en a pas
    /// besoin, et rendrait un client dépendant d'un détail que §27.3 lui promet de ne pas voir.
    #[must_use]
    pub fn client_surface(&self) -> ClientSurface {
        ClientSurface {
            endpoint: self.endpoint.clone(),
            capabilities: self.capabilities.clone(),
        }
    }

    /// Confronter ce profil à ce qu'un inventaire a constaté.
    ///
    /// Le profil ne se déclare jamais exécutable : c'est ce croisement qui le dit, et lui seul.
    #[must_use]
    pub fn inspect(&self, inventory: &Inventory) -> Readiness {
        let mut missing = BTreeSet::new();
        let mut unverified = BTreeSet::new();
        for adapter in &self.adapters {
            match inventory.presence(adapter) {
                Presence::Present => {}
                Presence::Absent => {
                    missing.insert(adapter.clone());
                }
                Presence::Unknown => {
                    unverified.insert(adapter.clone());
                }
            }
        }
        Readiness {
            profile: self.kind,
            missing,
            unverified,
        }
    }
}

/// Ce qu'un client peut voir d'un déploiement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientSurface {
    /// L'URL Locus.
    pub endpoint: String,
    /// Les capabilities annoncées.
    pub capabilities: BTreeSet<String>,
}

/// Ce qu'une sonde a constaté de la machine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Inventory {
    findings: BTreeMap<String, Presence>,
}

impl Inventory {
    /// Un inventaire qui n'a encore rien constaté.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Consigner ce qu'une sonde a constaté.
    #[must_use]
    pub fn observing(mut self, adapter: &str, presence: Presence) -> Self {
        self.findings.insert(adapter.to_owned(), presence);
        self
    }

    /// Ce qui a été constaté de `adapter`.
    ///
    /// Un adaptateur dont l'inventaire ne parle pas est [`Presence::Unknown`], pas
    /// [`Presence::Absent`] : ne pas avoir regardé et avoir regardé sans rien trouver sont deux
    /// choses, et la première se corrige en sondant.
    #[must_use]
    pub fn presence(&self, adapter: &str) -> Presence {
        self.findings
            .get(adapter)
            .copied()
            .unwrap_or(Presence::Unknown)
    }
}

/// Le verdict de `locus doctor`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Readiness {
    profile: ProfileKind,
    missing: BTreeSet<String>,
    unverified: BTreeSet<String>,
}

impl Readiness {
    /// Vrai quand le profil est réellement exécutable.
    ///
    /// Il faut que **rien** ne manque et que **rien** ne soit inconnu. Accepter l'inconnu ferait
    /// dépendre l'acceptation d'une campagne du bon fonctionnement des sondes, ce qui inverse
    /// exactement le rapport que §27.2 établit.
    #[must_use]
    pub fn executable(&self) -> bool {
        self.missing.is_empty() && self.unverified.is_empty()
    }

    /// Ce qui a été constaté absent.
    #[must_use]
    pub const fn missing(&self) -> &BTreeSet<String> {
        &self.missing
    }

    /// Ce qu'aucune sonde n'a pu constater.
    #[must_use]
    pub const fn unverified(&self) -> &BTreeSet<String> {
        &self.unverified
    }

    /// Le profil examiné.
    #[must_use]
    pub const fn profile(&self) -> ProfileKind {
        self.profile
    }
}

impl fmt::Display for Readiness {
    /// Ce que `locus doctor` imprime.
    ///
    /// Un « non exécutable » sans raison est inexploitable : c'est la liste des noms qui permet
    /// d'agir, et les deux listes restent séparées parce qu'elles appellent des gestes différents —
    /// installer d'un côté, réparer une sonde de l'autre.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.executable() {
            return write!(formatter, "{} : exécutable", self.profile);
        }
        write!(formatter, "{} : non exécutable", self.profile)?;
        if !self.missing.is_empty() {
            write!(formatter, " ; absent : {}", join(&self.missing))?;
        }
        if !self.unverified.is_empty() {
            write!(formatter, " ; non vérifié : {}", join(&self.unverified))?;
        }
        Ok(())
    }
}

fn join(names: &BTreeSet<String>) -> String {
    names
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Ce qui empêche un profil d'exister.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    /// Un champ obligatoire vide.
    EmptyField {
        /// Lequel.
        field: &'static str,
    },
    /// Un profil qui n'exige aucun adaptateur.
    NoAdapter,
}

impl fmt::Display for ProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(formatter, "« {field} » est vide"),
            Self::NoAdapter => formatter.write_str(
                "un profil sans adaptateur passerait toute vérification sans rien avoir \
                 vérifié — c'est la façon la plus discrète de rendre `locus doctor` inutile",
            ),
        }
    }
}

impl std::error::Error for ProfileError {}
