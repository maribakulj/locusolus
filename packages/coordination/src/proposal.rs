//! La proposition de modification de coordination — ADR 0016, décisions 4, 5, 7 et 8 ;
//! `docs/SPEC_V1.md` §14.5, §20.3, §22.2.
//!
//! # Ce que ce module porte
//!
//! Une proposition, son auteur, sa justification, sa base de version, et le chemin unique par
//! lequel elle devient un fait : validation → politique → approbation → commit. Un agent et un
//! humain empruntent **le même** chemin (décision 7) ; ce qui les distingue est le mode du
//! déploiement et l'interdiction d'approuver sa propre proposition, pas un second circuit.
//!
//! # Ce que ce module ne crée pas
//!
//! Ni compteur de version, ni magasin, ni bus. La base d'une proposition **est**
//! l'`expected_revision` du `CommandEnvelope` de §22.2 ; ce module la compare à la révision
//! courante et refuse en nommant ce qu'il faut faire. Décision 5 : « aucun compteur, aucun
//! magasin, aucun bus n'est créé ».

use std::fmt;

use locus_domain::RevisionId;
use locus_protocol::{
    Id,
    id::{Agent, provisional::Approval, provisional::Decision as DecisionKind},
};

use crate::team::CoordinationMode;

/// Les sortes de relation de coordination qui existent.
///
/// **Une seule.** ADR 0016, décision 4 : « aucune sémantique inerte » — une sorte de relation
/// n'entre dans cette énumération que lorsqu'un consommateur exécutable et testé existe. `review`
/// en a un : l'indépendance de §14.4 et l'invariant 11 s'y appuient. `mentors`, `delegates_to`,
/// `supervises` n'en ont pas, et les écrire ici en ferait du vocabulaire que rien ne vérifie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RelationKind {
    /// « relit », au sens de §14.4 et de l'invariant 11.
    Review,
}

impl RelationKind {
    /// La seule, aujourd'hui.
    pub const ALL: [Self; 1] = [Self::Review];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Review => "review",
        }
    }

    /// Relire une sorte.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.slug() == value)
    }
}

impl fmt::Display for RelationKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Une relation entre deux agents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Relation {
    /// Qui.
    pub from: Id<Agent>,
    /// Envers qui.
    pub to: Id<Agent>,
    /// De quelle sorte.
    pub kind: RelationKind,
}

/// Ce qu'une proposition change — le payload de `team.modify` (§22.3).
///
/// Chaque variante a son inverse exact, et c'est ce qui rend l'annulation possible **sans rien
/// supprimer** (décision 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// Ajouter un membre à l'équipe.
    AddMember(Id<Agent>),
    /// Retirer un membre.
    RemoveMember(Id<Agent>),
    /// Ajouter une relation de coordination.
    AddRelation(Relation),
    /// Retirer une relation.
    RemoveRelation(Relation),
    /// Changer le mode de coordination.
    SetMode {
        /// Celui d'avant, pour que l'inverse existe.
        from: CoordinationMode,
        /// Celui d'après.
        to: CoordinationMode,
    },
}

impl Change {
    /// Le changement qui défait celui-ci.
    ///
    /// Une annulation est le **commit d'un changement inverse**, produisant une nouvelle version
    /// dont le parent est la version courante (décision 5). Retirer une version rendrait
    /// l'histoire fausse : on ne pourrait plus dire qu'une mission a tourné sous une organisation
    /// qui, désormais, n'aurait jamais existé.
    #[must_use]
    pub const fn inverse(self) -> Self {
        match self {
            Self::AddMember(agent) => Self::RemoveMember(agent),
            Self::RemoveMember(agent) => Self::AddMember(agent),
            Self::AddRelation(relation) => Self::RemoveRelation(relation),
            Self::RemoveRelation(relation) => Self::AddRelation(relation),
            Self::SetMode { from, to } => Self::SetMode { from: to, to: from },
        }
    }
}

/// Qui écrit la proposition.
///
/// Décision 7 : « une proposition écrite par un agent est **le même objet** qu'une proposition
/// humaine et suit le même chemin ». L'auteur n'ouvre donc pas un second circuit — il change ce
/// que le mode autorise, et rien d'autre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Author {
    /// Un humain, désigné par son principal.
    Human(String),
    /// Une instance d'agent.
    Agent(Id<Agent>),
}

impl Author {
    /// Vrai quand deux auteurs sont la même personne ou la même instance.
    #[must_use]
    pub fn is(&self, other: &Self) -> bool {
        self == other
    }
}

impl fmt::Display for Author {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Human(principal) => write!(formatter, "humain {principal}"),
            Self::Agent(agent) => write!(formatter, "agent {agent}"),
        }
    }
}

/// Ce qui motive une proposition.
///
/// Elle **cite un objet épistémique par sa révision**, jamais par son concept : §7.7 fait de
/// `revision_id` l'identité d'une version immuable, et citer un `stable_id` désignerait « la
/// dernière version, quelle qu'elle soit » — donc une justification qui change après coup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Justification {
    trigger: String,
    cites: RevisionId,
}

impl Justification {
    /// Justifier une proposition.
    ///
    /// # Errors
    ///
    /// [`ProposalError::EmptyTrigger`] pour un déclencheur vide. §14.5 en énumère onze ; la liste
    /// n'est pas fermée ici parce qu'elle relève de la politique, mais un déclencheur vide ne dit
    /// rien à personne.
    pub fn new(trigger: &str, cites: RevisionId) -> Result<Self, ProposalError> {
        if trigger.trim().is_empty() {
            return Err(ProposalError::EmptyTrigger);
        }
        Ok(Self {
            trigger: trigger.to_owned(),
            cites,
        })
    }

    /// Le déclencheur — un des onze de §14.5, ou un autre que la politique reconnaît.
    #[must_use]
    pub fn trigger(&self) -> &str {
        &self.trigger
    }

    /// L'objet épistémique cité, par révision.
    #[must_use]
    pub const fn cites(&self) -> &RevisionId {
        &self.cites
    }
}

/// Savoir si une révision épistémique existe.
///
/// # Pourquoi un port et non un accès au graphe
///
/// La sixième frontière interdit à ce crate d'importer `packages/graph`. Ce port pose la seule
/// question dont une proposition a besoin — « cette révision existe-t-elle ? » — sans traverser
/// quoi que ce soit. Le commentaire de `boundaries.json` l'annonçait déjà : « une justification de
/// proposition cite un objet épistémique par son `RevisionId`, obtenu de `locus-domain` : elle ne
/// traverse jamais le graphe ».
pub trait EpistemicIndex {
    /// Vrai quand cette révision existe.
    fn contains(&self, revision: &RevisionId) -> bool;
}

/// Une proposition de modification de coordination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    id: Id<DecisionKind>,
    author: Author,
    base_revision: u64,
    change: Change,
    justification: Justification,
    cancels: Option<Id<DecisionKind>>,
}

impl Proposal {
    /// Écrire une proposition.
    ///
    /// # Errors
    ///
    /// [`ProposalError::UncitedJustification`] quand la justification cite une révision que
    /// l'index ne connaît pas, et [`ProposalError::NotAllowedToPropose`] quand le mode du
    /// déploiement ne permet pas à cet auteur de proposer.
    ///
    /// # L'ordre des deux vérifications
    ///
    /// Le mode d'abord, la citation ensuite. Un agent en `observed` ne doit pas apprendre, par la
    /// nature du refus, quelles révisions existent — c'est peu, mais c'est gratuit à tenir.
    pub fn write(
        id: Id<DecisionKind>,
        author: Author,
        mode: Mode,
        base_revision: u64,
        change: Change,
        justification: Justification,
        index: &impl EpistemicIndex,
    ) -> Result<Self, ProposalError> {
        if !mode.allows(&author) {
            return Err(ProposalError::NotAllowedToPropose {
                mode,
                author: author.to_string(),
            });
        }
        if !index.contains(justification.cites()) {
            return Err(ProposalError::UncitedJustification {
                revision: justification.cites().to_string(),
            });
        }
        Ok(Self {
            id,
            author,
            base_revision,
            change,
            justification,
            cancels: None,
        })
    }

    /// Déclarer que cette proposition en annule une autre.
    ///
    /// Le changement porté doit être l'inverse de celui qu'elle annule ; l'appelant le compose
    /// avec [`Change::inverse`].
    #[must_use]
    pub const fn cancelling(mut self, cancelled: Id<DecisionKind>) -> Self {
        self.cancels = Some(cancelled);
        self
    }

    /// Son identifiant.
    #[must_use]
    pub const fn id(&self) -> Id<DecisionKind> {
        self.id
    }

    /// Son auteur.
    #[must_use]
    pub const fn author(&self) -> &Author {
        &self.author
    }

    /// La révision sur laquelle elle a été écrite.
    #[must_use]
    pub const fn base_revision(&self) -> u64 {
        self.base_revision
    }

    /// Ce qu'elle change.
    #[must_use]
    pub const fn change(&self) -> Change {
        self.change
    }

    /// Ce qui la motive.
    #[must_use]
    pub const fn justification(&self) -> &Justification {
        &self.justification
    }

    /// La proposition qu'elle annule, s'il y en a une.
    #[must_use]
    pub const fn cancels(&self) -> Option<Id<DecisionKind>> {
        self.cancels
    }
}

/// Le mode du déploiement — ADR 0016, décision 8.
///
/// Le défaut est `observed`, et c'est une exigence de §33 : « rendre toute action autonome sans
/// seuil humain » est un **non-objectif explicite de la V1 ». Les modes `bounded` et `operator`
/// n'existent pas ici : ils demandent la classe de risque dérivée et l'anti-gaming, qui sont W14
/// et W16.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Mode {
    /// L'agent signale un besoin ; il ne propose pas. **Le défaut.**
    #[default]
    Observed,
    /// L'agent propose ; un humain approuve.
    Assisted,
}

impl Mode {
    /// Vrai quand cet auteur peut proposer sous ce mode.
    ///
    /// Un humain propose toujours : le mode borne ce que les **agents** peuvent faire, pas ce que
    /// l'institution peut décider d'elle-même.
    #[must_use]
    pub const fn allows(self, author: &Author) -> bool {
        match author {
            Author::Human(_) => true,
            Author::Agent(_) => matches!(self, Self::Assisted),
        }
    }

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Observed => "observed",
            Self::Assisted => "assisted",
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Une proposition approuvée, prête à être commitée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Approved {
    proposal: Proposal,
    approver: Author,
    approval_id: Id<Approval>,
}

impl Approved {
    /// La proposition approuvée.
    #[must_use]
    pub const fn proposal(&self) -> &Proposal {
        &self.proposal
    }

    /// Qui a approuvé.
    #[must_use]
    pub const fn approver(&self) -> &Author {
        &self.approver
    }

    /// L'identifiant de l'approbation.
    #[must_use]
    pub const fn approval_id(&self) -> Id<Approval> {
        self.approval_id
    }
}

/// Approuver une proposition.
///
/// # `forbid_self_approval`
///
/// §20.3 le porte déjà, et ADR 0016 en fait une borne « qui ne se relâche dans aucun mode ». Elle
/// est plus générale qu'une détection de conflit d'intérêt au cas par cas : c'est ce qui empêche un
/// agent de contrôler les règles décidant de son propre remplacement.
///
/// # Errors
///
/// [`ProposalError::SelfApproval`] quand l'approbateur est l'auteur.
pub fn approve(
    proposal: Proposal,
    approver: Author,
    approval_id: Id<Approval>,
) -> Result<Approved, ProposalError> {
    if proposal.author().is(&approver) {
        return Err(ProposalError::SelfApproval {
            author: approver.to_string(),
        });
    }
    Ok(Approved {
        proposal,
        approver,
        approval_id,
    })
}

/// Ce qu'un commit produit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Committed {
    /// La proposition commitée.
    pub proposal: Proposal,
    /// La révision produite.
    pub revision: u64,
}

/// Commiter une proposition approuvée, par comparaison de révision.
///
/// # Le CAS, et ce que le refus doit dire
///
/// La base d'une proposition **est** l'`expected_revision` de §22.2, et `Expected` de
/// `packages/event-store` « n'a pas de variante “peu importe” ». Quand la révision courante a
/// bougé, la proposition a été écrite contre un monde qui n'existe plus : le refus le dit **et
/// dit quoi faire**, parce qu'un « conflit » sans consigne laisse l'appelant réessayer à
/// l'identique jusqu'à ce que quelqu'un lise le code.
///
/// # Errors
///
/// [`ProposalError::Stale`] quand la base ne correspond plus.
pub fn commit(approved: Approved, current_revision: u64) -> Result<Committed, ProposalError> {
    let base = approved.proposal().base_revision();
    if base != current_revision {
        return Err(ProposalError::Stale {
            expected: base,
            actual: current_revision,
        });
    }
    Ok(Committed {
        proposal: approved.proposal,
        revision: current_revision + 1,
    })
}

/// Ce qui empêche une proposition d'exister, d'être approuvée ou d'être commitée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposalError {
    /// Un déclencheur vide.
    EmptyTrigger,
    /// Une justification qui cite une révision inconnue.
    UncitedJustification {
        /// Ce qui a été cité.
        revision: String,
    },
    /// Un auteur que le mode n'autorise pas à proposer.
    NotAllowedToPropose {
        /// Le mode en vigueur.
        mode: Mode,
        /// L'auteur.
        author: String,
    },
    /// Un auteur qui approuve sa propre proposition.
    SelfApproval {
        /// L'auteur.
        author: String,
    },
    /// Une base de version dépassée.
    Stale {
        /// Ce sur quoi la proposition a été écrite.
        expected: u64,
        /// Ce qui est en vigueur.
        actual: u64,
    },
}

impl ProposalError {
    /// Vrai quand l'appelant doit rebaser avant de retenter.
    ///
    /// La consigne fait partie du refus : sans elle, un appelant retenterait à l'identique.
    #[must_use]
    pub const fn needs_rebase(&self) -> bool {
        matches!(self, Self::Stale { .. })
    }
}

impl fmt::Display for ProposalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTrigger => {
                formatter.write_str("un déclencheur vide ne justifie rien auprès de personne")
            }
            Self::UncitedJustification { revision } => write!(
                formatter,
                "la révision citée « {revision} » n'existe pas : une justification qui ne cite \
                 rien d'existant n'est pas vérifiable"
            ),
            Self::NotAllowedToPropose { mode, author } => write!(
                formatter,
                "en mode « {mode} », {author} signale un besoin mais ne propose pas"
            ),
            Self::SelfApproval { author } => write!(
                formatter,
                "{author} ne peut pas approuver sa propre proposition"
            ),
            Self::Stale { expected, actual } => write!(
                formatter,
                "proposition écrite sur la révision {expected}, la révision courante est {actual} \
                 : rebaser puis retenter"
            ),
        }
    }
}

impl std::error::Error for ProposalError {}
