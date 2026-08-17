//! `Team` et les modes de coordination — `docs/SPEC_V1.md` §7.1, §14.3.

use std::collections::BTreeSet;
use std::fmt;

use locus_protocol::{
    Id,
    id::{Agent, Branch, provisional::Team as TeamKind},
};

/// Les cinq modes obligatoires de §14.3.
///
/// Fermé : §14.3 dit « modes **obligatoires** » et « le mode est enregistré et peut être comparé
/// dans les benchmarks ». Une valeur libre rendrait la comparaison impossible — deux campagnes
/// écriraient `debate` et `Debate` sans qu'aucune requête ne les rapproche.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoordinationMode {
    /// Un agent distribue et synthétise.
    Coordinator,
    /// Contributions sur une mémoire de branche partagée.
    Blackboard,
    /// Positions et objections structurées.
    Debate,
    /// Aucun partage avant remise.
    IndependentPool,
    /// Sorties typées enchaînées.
    Pipeline,
}

impl CoordinationMode {
    /// Les cinq, dans l'ordre de §14.3.
    pub const ALL: [Self; 5] = [
        Self::Coordinator,
        Self::Blackboard,
        Self::Debate,
        Self::IndependentPool,
        Self::Pipeline,
    ];

    /// Le nom employé par §14.3.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Coordinator => "coordinator",
            Self::Blackboard => "blackboard",
            Self::Debate => "debate",
            Self::IndependentPool => "independent_pool",
            Self::Pipeline => "pipeline",
        }
    }

    /// Relire un mode.
    ///
    /// `None` plutôt qu'un défaut : un mode inconnu rabattu sur `coordinator` ferait croire à une
    /// coordination centralisée là où il n'y en a pas, et fausserait la comparaison que §14.3
    /// annonce.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.slug() == value)
    }

    /// Vrai quand le mode suppose qu'un membre coordonne les autres.
    #[must_use]
    pub const fn needs_coordinator(self) -> bool {
        matches!(self, Self::Coordinator)
    }

    /// Vrai quand le mode interdit tout partage avant remise.
    ///
    /// C'est `independent_pool`, et lui seul. La distinction porte l'invariant 11 : un relecteur
    /// qui verrait les contributions des autres avant de rendre la sienne n'est plus indépendant.
    #[must_use]
    pub const fn withholds_sharing(self) -> bool {
        matches!(self, Self::IndependentPool)
    }
}

impl fmt::Display for CoordinationMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Une équipe — §14.2 : « `Team` définit coordination et partage d'information ».
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Team {
    id: Id<TeamKind>,
    branch_id: Id<Branch>,
    title: String,
    mode: CoordinationMode,
    members: BTreeSet<Id<Agent>>,
    coordinator: Option<Id<Agent>>,
    revision: u64,
}

impl Team {
    /// Constituer une équipe.
    ///
    /// # Errors
    ///
    /// [`TeamError::EmptyTitle`], [`TeamError::NoMembers`] — une équipe sans membre ne coordonne
    /// rien — et [`TeamError::CoordinatorNotAMember`] quand le coordinateur désigné n'est pas dans
    /// l'équipe. [`TeamError::CoordinatorRequired`] quand le mode `coordinator` n'en nomme aucun :
    /// §14.3 en fait la définition du mode, et l'omettre laisserait une équipe qui se dit
    /// coordonnée sans que personne ne coordonne.
    pub fn new(
        id: Id<TeamKind>,
        branch_id: Id<Branch>,
        title: &str,
        mode: CoordinationMode,
        members: BTreeSet<Id<Agent>>,
        coordinator: Option<Id<Agent>>,
    ) -> Result<Self, TeamError> {
        if title.trim().is_empty() {
            return Err(TeamError::EmptyTitle);
        }
        if members.is_empty() {
            return Err(TeamError::NoMembers);
        }
        match coordinator {
            Some(coordinator) if !members.contains(&coordinator) => {
                return Err(TeamError::CoordinatorNotAMember);
            }
            None if mode.needs_coordinator() => return Err(TeamError::CoordinatorRequired),
            _ => {}
        }
        Ok(Self {
            id,
            branch_id,
            title: title.to_owned(),
            mode,
            members,
            coordinator,
            revision: 1,
        })
    }

    /// Son identifiant.
    #[must_use]
    pub const fn id(&self) -> Id<TeamKind> {
        self.id
    }

    /// La branche où elle travaille.
    #[must_use]
    pub const fn branch_id(&self) -> Id<Branch> {
        self.branch_id
    }

    /// Son titre.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Son mode de coordination.
    #[must_use]
    pub const fn mode(&self) -> CoordinationMode {
        self.mode
    }

    /// Ses membres.
    #[must_use]
    pub const fn members(&self) -> &BTreeSet<Id<Agent>> {
        &self.members
    }

    /// Son coordinateur, quand le mode en a un.
    #[must_use]
    pub const fn coordinator(&self) -> Option<Id<Agent>> {
        self.coordinator
    }

    /// Sa révision, pour le CAS de W13.e.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Vrai quand deux membres peuvent se lire avant remise.
    ///
    /// La question que l'invariant 11 pose, formulée une seule fois plutôt que réécrite partout où
    /// on en a besoin.
    #[must_use]
    pub const fn shares_before_delivery(&self) -> bool {
        !self.mode.withholds_sharing()
    }
}

/// Ce qui empêche une équipe d'exister.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamError {
    /// Un titre vide.
    EmptyTitle,
    /// Aucun membre.
    NoMembers,
    /// Un coordinateur qui n'est pas membre.
    CoordinatorNotAMember,
    /// Le mode `coordinator` sans coordinateur.
    CoordinatorRequired,
}

impl fmt::Display for TeamError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::EmptyTitle => "une équipe sans titre ne se désigne pas",
            Self::NoMembers => "une équipe sans membre ne coordonne rien",
            Self::CoordinatorNotAMember => "le coordinateur doit être membre de l'équipe",
            Self::CoordinatorRequired => {
                "le mode « coordinator » suppose un coordinateur, sinon personne ne coordonne"
            }
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for TeamError {}
