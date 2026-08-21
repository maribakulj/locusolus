//! `Team` et les modes de coordination — `docs/SPEC_V1.md` §7.1, §14.3.

use std::collections::BTreeSet;
use std::fmt;

use locus_protocol::{
    Id,
    id::{Agent, Branch, provisional::Team as TeamKind},
};

use crate::version::{Version, VersionId};

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
///
/// L'agrégat de §7.1 — **son identité et son état, pas sa structure**.
///
/// # Ce qu'il ne stocke plus, et pourquoi (ADR 0021, décision 3)
///
/// `member_ids`, `coordination_mode` et `coordinator_id` sont des champs de §7.1, et ils y restent :
/// [`Team::members`], [`Team::mode`] et [`Team::coordinator`] les servent. Ce qui change est qu'ils
/// sont **servis depuis la version courante** au lieu d'être recopiés ici.
///
/// Les recopier, c'était deux stockages du même fait dans le même crate — donc deux vérités, qui
/// divergent le jour où l'une est corrigée. C'est l'argument par lequel l'ADR 0019 a écarté le
/// courtier de messages, et il ne dépend pas du sujet.
///
/// `docs/13` §3 nomme la taxonomie qui l'autorise : « version canonique immuable avec hash et
/// parent, **graphe réalisé comme projection**, trace comme histoire ». La [`Version`] est le
/// canonique ; `Team` est le réalisé.
///
/// # Pourquoi la version voyage avec l'appel plutôt que dans le champ
///
/// `Team` retient la **`VersionId`** de sa structure, pas la `Version` elle-même : la retenir en
/// entier reconstituerait le doublon sous un autre nom. Les accesseurs prennent donc la version en
/// argument et **refusent** celle qui n'est pas la sienne. Un accesseur qui l'accepterait sans
/// vérifier rendrait des membres plausibles — ceux d'une autre équipe, ou d'un autre instant de
/// celle-ci — et rien dans la réponse ne le dirait. C'est le mode d'échec de `W20.e` sur les
/// cursors, au même endroit du raisonnement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Team {
    id: Id<TeamKind>,
    branch_id: Id<Branch>,
    title: String,
    version: VersionId,
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
        structure: &Version,
    ) -> Result<Self, TeamError> {
        if title.trim().is_empty() {
            return Err(TeamError::EmptyTitle);
        }
        // Les trois règles de §14.3 ne sont plus vérifiées ici : elles le sont par `Version::root`
        // et après chaque application, donc une version qui existe les respecte déjà. Les revérifier
        // serait une seconde définition de la même règle, et deux définitions divergent.
        Ok(Self {
            id,
            branch_id,
            title: title.to_owned(),
            version: structure.id().clone(),
            revision: 1,
        })
    }

    /// La version dont sa structure est faite.
    #[must_use]
    pub const fn version(&self) -> &VersionId {
        &self.version
    }

    /// Vérifie que cette version est bien la sienne.
    fn owning<'a>(&self, at: &'a Version) -> Result<&'a Version, TeamError> {
        if at.id() != &self.version {
            return Err(TeamError::WrongVersion {
                expected: self.version.to_string(),
                given: at.id().to_string(),
            });
        }
        Ok(at)
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

    /// Son mode de coordination — §7.1 `coordination_mode`, servi depuis la version.
    ///
    /// # Errors
    ///
    /// [`TeamError::WrongVersion`] quand la version présentée n'est pas la sienne.
    pub fn mode(&self, at: &Version) -> Result<CoordinationMode, TeamError> {
        Ok(self.owning(at)?.mode())
    }

    /// Ses membres — §7.1 `member_ids`, servis depuis la version.
    ///
    /// # Errors
    ///
    /// [`TeamError::WrongVersion`] quand la version présentée n'est pas la sienne.
    pub fn members<'a>(&self, at: &'a Version) -> Result<&'a BTreeSet<Id<Agent>>, TeamError> {
        Ok(self.owning(at)?.members())
    }

    /// Son coordinateur — §7.1 `coordinator_id`, servi depuis la version.
    ///
    /// # Errors
    ///
    /// [`TeamError::WrongVersion`] quand la version présentée n'est pas la sienne.
    pub fn coordinator<'a>(&self, at: &'a Version) -> Result<Option<&'a Id<Agent>>, TeamError> {
        Ok(self.owning(at)?.coordinator())
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
    /// # Errors
    ///
    /// [`TeamError::WrongVersion`] quand la version présentée n'est pas la sienne.
    pub fn shares_before_delivery(&self, at: &Version) -> Result<bool, TeamError> {
        Ok(!self.mode(at)?.withholds_sharing())
    }
}

/// Ce qui empêche une équipe d'exister.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TeamError {
    /// Un titre vide.
    EmptyTitle,
    /// La version présentée n'est pas celle dont l'équipe tient sa structure.
    ///
    /// Le refus nomme les deux, parce qu'un appelant qui présente la mauvaise version a besoin de
    /// savoir laquelle il tenait. Sans ce refus, l'accesseur rendrait des membres **plausibles** —
    /// ceux d'une autre équipe, ou d'un autre instant de celle-ci — et rien dans la réponse ne le
    /// dirait. Même mode d'échec que le cursor présenté à la mauvaise collection (`W20.e`).
    WrongVersion {
        /// Celle dont l'équipe tient sa structure.
        expected: String,
        /// Celle qu'on lui présente.
        given: String,
    },
}

impl fmt::Display for TeamError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTitle => formatter.write_str("une équipe sans titre ne se désigne pas"),
            Self::WrongVersion { expected, given } => write!(
                formatter,
                "l'équipe tient sa structure de {expected} ; on lui présente {given} : une version \
                 n'a pas le même contenu d'une équipe à l'autre"
            ),
        }
    }
}

impl std::error::Error for TeamError {}
