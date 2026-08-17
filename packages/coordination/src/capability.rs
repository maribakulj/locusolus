//! Les capacités effectives d'une instance d'agent — `docs/SPEC_V1.md` §14.2.
//!
//! # La phrase que ce module rend opposable
//!
//! « Une instance n'hérite **jamais** tacitement des permissions du modèle ou du worker. Les
//! capacités effectives sont l'**intersection** de la mission, du template, de la politique locale
//! et de l'attestation du worker. »
//!
//! Intersection, et non union : une capacité doit être accordée **quatre fois** pour exister. La
//! différence est tout sauf théorique — sous l'union, une politique locale permissive suffirait à
//! rendre un outil accessible à une mission qui ne l'a jamais demandé, et l'attestation d'un worker
//! deviendrait une source de droits au lieu d'être une borne.
//!
//! # Pourquoi les quatre sources sont un type et non quatre paramètres
//!
//! Quatre `BTreeSet` en paramètres se permutent silencieusement, et surtout : rien n'empêche d'en
//! oublier un. [`Sources`] les nomme, et [`Sources::effective`] les traverse toutes — ce que le
//! test de sortie vérifie en retirant chacune à son tour.

use std::collections::BTreeSet;
use std::fmt;

/// Une capacité, désignée par son nom.
///
/// Un nom et rien d'autre : ce crate ne sait pas ce qu'un outil fait, seulement s'il est accordé.
/// Ce qu'un nom recouvre est la charge de `tool_policy_id`, ailleurs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Capability(String);

impl Capability {
    /// Nommer une capacité.
    ///
    /// # Errors
    ///
    /// [`CapabilityError::Empty`] pour un nom vide : une capacité anonyme ne s'accorde ni ne se
    /// refuse, elle se glisse.
    pub fn new(name: &str) -> Result<Self, CapabilityError> {
        if name.trim().is_empty() {
            return Err(CapabilityError::Empty);
        }
        Ok(Self(name.trim().to_owned()))
    }

    /// Son nom.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Ce qui empêche une capacité d'exister.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityError {
    /// Un nom vide.
    Empty,
}

impl fmt::Display for CapabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => {
                formatter.write_str("une capacité sans nom ne s'accorde ni ne se refuse")
            }
        }
    }
}

impl std::error::Error for CapabilityError {}

/// D'où vient l'autorisation d'une capacité — les quatre de §14.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Source {
    /// Ce que la mission demande.
    Mission,
    /// Ce que le template du rôle permet.
    Template,
    /// Ce que la politique locale du site permet.
    LocalPolicy,
    /// Ce que l'attestation du worker établit qu'il peut réellement faire.
    WorkerAttestation,
}

impl Source {
    /// Les quatre, dans l'ordre de §14.2.
    pub const ALL: [Self; 4] = [
        Self::Mission,
        Self::Template,
        Self::LocalPolicy,
        Self::WorkerAttestation,
    ];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Mission => "mission",
            Self::Template => "template",
            Self::LocalPolicy => "local_policy",
            Self::WorkerAttestation => "worker_attestation",
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Les quatre sources, ensemble.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sources {
    mission: BTreeSet<Capability>,
    template: BTreeSet<Capability>,
    local_policy: BTreeSet<Capability>,
    worker_attestation: BTreeSet<Capability>,
}

impl Sources {
    /// Déclarer les quatre.
    #[must_use]
    pub fn new(
        mission: BTreeSet<Capability>,
        template: BTreeSet<Capability>,
        local_policy: BTreeSet<Capability>,
        worker_attestation: BTreeSet<Capability>,
    ) -> Self {
        Self {
            mission,
            template,
            local_policy,
            worker_attestation,
        }
    }

    /// Ce qu'une source accorde.
    #[must_use]
    pub const fn granted_by(&self, source: Source) -> &BTreeSet<Capability> {
        match source {
            Source::Mission => &self.mission,
            Source::Template => &self.template,
            Source::LocalPolicy => &self.local_policy,
            Source::WorkerAttestation => &self.worker_attestation,
        }
    }

    /// Les capacités effectives : l'intersection des quatre.
    ///
    /// L'itération part de [`Source::ALL`], donc ajouter une cinquième source à l'énumération la
    /// fait entrer ici sans qu'on ait à y penser — et en oublier une demande de retirer une entrée
    /// d'une liste que le compilateur affiche.
    #[must_use]
    pub fn effective(&self) -> BTreeSet<Capability> {
        let mut sources = Source::ALL.into_iter();
        let Some(first) = sources.next() else {
            return BTreeSet::new();
        };
        let mut effective = self.granted_by(first).clone();
        for source in sources {
            let granted = self.granted_by(source);
            effective.retain(|capability| granted.contains(capability));
        }
        effective
    }

    /// Pourquoi une capacité n'est pas effective : les sources qui ne l'accordent pas.
    ///
    /// Vide quand elle l'est. Un refus qui ne dit pas d'où il vient oblige à interroger quatre
    /// politiques à la main — et la réponse « la mission ne l'a pas demandée » n'appelle pas du
    /// tout la même suite que « le worker ne peut pas le faire ».
    #[must_use]
    pub fn withholding(&self, capability: &Capability) -> Vec<Source> {
        Source::ALL
            .into_iter()
            .filter(|source| !self.granted_by(*source).contains(capability))
            .collect()
    }
}

/// Rassembler des noms en un ensemble de capacités.
///
/// # Errors
///
/// [`CapabilityError`] au premier nom vide.
pub fn capabilities<'a>(
    names: impl IntoIterator<Item = &'a str>,
) -> Result<BTreeSet<Capability>, CapabilityError> {
    names.into_iter().map(Capability::new).collect()
}
