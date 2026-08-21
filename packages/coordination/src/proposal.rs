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

use crate::diff::Diff;
use crate::version::{Digest, Version};

/// Les sortes de relation de coordination qui existent.
///
/// **Deux.** ADR 0016, décision 4 : « aucune sémantique inerte » — une sorte de relation n'entre
/// dans cette énumération que lorsqu'un consommateur exécutable et testé existe.
///
/// - `review` en a un depuis W13.e : l'indépendance de §14.4 et l'invariant 11 s'y appuient.
/// - `visibility` en a un depuis W15.e : la construction de `ContextView` (décision 11), par
///   [`crate::visibility`].
///
/// `mentors`, `delegates_to`, `supervises` n'en ont pas, et les écrire ici en ferait du vocabulaire
/// que rien ne vérifie. `role` n'est pas une sorte de relation du tout : §7.1 en fait un champ
/// d'`AgentTemplate`, et c'est l'opération attributaire `SET_ROLE` qui le portera.
///
/// # Deux sortes, et ce que la seconde a révélé
///
/// Tant qu'il n'y en avait qu'une, tout code parlant de « relations » parlait en fait de revues
/// sans le dire. L'arrivée de `visibility` a montré deux endroits où l'implicite était devenu une
/// hypothèse : le veto d'acyclicité de [`crate::region`] — un cycle de visibilité est normal, un
/// cycle de revue ne l'est pas — et le refus d'auto-relation de [`crate::version`], dont le message
/// ne parlait que de revue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RelationKind {
    /// « relit », au sens de §14.4 et de l'invariant 11.
    Review,
    /// « voit ce que l'autre a produit », au sens de §16.2 et §16.3.
    ///
    /// Elle **restreint**, elle n'élargit jamais : §16.3 exige que les embeddings ne contournent
    /// pas les ACL, et une relation de coordination ne saurait pas davantage les contourner. Ce
    /// qu'elle décide est ce qu'un destinataire **cesse** de voir, pas ce qu'il gagne.
    Visibility,
}

impl RelationKind {
    /// Les deux, dans l'ordre où elles ont obtenu un consommateur.
    pub const ALL: [Self; 2] = [Self::Review, Self::Visibility];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Review => "review",
            Self::Visibility => "visibility",
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
    diff: Diff,
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
        diff: Diff,
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
            diff,
            justification,
            cancels: None,
        })
    }

    /// Déclarer que cette proposition en annule une autre.
    ///
    /// Le diff porté doit être celui qui défait la proposition annulée ; l'appelant le compose avec
    /// [`Diff::inverse`], qui **refuse** quand une opération n'a pas d'inverse exact. ADR 0016
    /// décision 5 : « une modification non inversible ne peut être que compensée, et elle le déclare
    /// à la proposition » — c'est l'appelant qui écrit alors ce qu'il compense, parce que lui seul
    /// sait ce qu'il veut compenser.
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

    /// Ce qu'elle change — un diff d'opérations, ADR 0021 décision 1.
    ///
    /// C'était une `Change`, une énumération que rien n'appliquait : `commit()` la recevait et
    /// rendait `revision + 1`. Une proposition qui déclarait « ajouter ce membre » laissait le
    /// système exactement dans l'état où elle l'avait trouvé. Le diff, lui, se rejoue.
    #[must_use]
    pub const fn diff(&self) -> &Diff {
        &self.diff
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
/// seuil humain » est un **non-objectif explicite de la V1 ».
///
/// # Les quatre ne sont pas une échelle
///
/// L'ADR les présente dans un tableau, et un tableau se lit de haut en bas. La tentation est de leur
/// donner un rang — `Ord`, un `level()`, un `is_at_least()` — et c'est l'échelle d'autorité à
/// barreaux que `CLAUDE.md` interdit nommément. `operator` en est la réfutation : c'est le mode le
/// **plus** privilégié et celui qui permet à un agent le **moins** — rien. Il décrit la session d'un
/// humain nommé qui répare, pas une autonomie de plus accordée à la flotte.
///
/// Ce type ne dérive donc ni `PartialOrd` ni `Ord`, et n'expose aucun rang. Un test le tient.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Mode {
    /// L'agent signale un besoin ; il ne propose pas. **Le défaut.**
    #[default]
    Observed,
    /// L'agent propose ; un humain approuve.
    Assisted,
    /// L'agent commite dans une région, sous plafond de budget et classe de risque **dérivée**.
    ///
    /// Ce mode ne relâche pas `forbid_self_approval` : il **retire l'approbation**, il ne la confie
    /// pas au proposeur. La différence n'est pas rhétorique — un agent qui produirait une
    /// approbation à son nom mettrait un nom sur un jugement que personne n'a porté.
    Bounded,
    /// Opérations privilégiées, réparation, rollback forcé — **un humain nommé, jamais un agent**.
    Operator,
}

impl Mode {
    /// Les quatre de l'ADR 0016 décision 8, dans l'ordre de son tableau.
    ///
    /// L'ordre est celui du document, pour qu'on puisse comparer les deux ; il n'est pas un rang.
    pub const ALL: [Self; 4] = [
        Self::Observed,
        Self::Assisted,
        Self::Bounded,
        Self::Operator,
    ];

    /// Vrai quand cet auteur peut proposer sous ce mode.
    ///
    /// Un humain propose toujours : le mode borne ce que les **agents** peuvent faire, pas ce que
    /// l'institution peut décider d'elle-même.
    ///
    /// Un agent propose sous `assisted` et sous `bounded`. Sous `observed` il signale, sous
    /// `operator` il n'a rien à faire du tout.
    #[must_use]
    pub const fn allows(self, author: &Author) -> bool {
        match author {
            Author::Human(_) => true,
            Author::Agent(_) => matches!(self, Self::Assisted | Self::Bounded),
        }
    }

    /// Vrai quand ce mode dispense de l'approbation humaine.
    ///
    /// Seul `bounded`. `operator` n'en dispense pas : c'est déjà un humain qui agit, et il n'y a pas
    /// d'approbation à retirer — les confondre ferait croire que deux modes autorisent l'autonomie
    /// alors qu'un seul concerne les agents.
    #[must_use]
    pub const fn dispenses_with_approval(self) -> bool {
        matches!(self, Self::Bounded)
    }

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Observed => "observed",
            Self::Assisted => "assisted",
            Self::Bounded => "bounded",
            Self::Operator => "operator",
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
///
/// **Trois choses, et les deux premières ne se remplacent pas.** La `revision` est la concurrence
/// optimistique de §22.2 — elle dit que personne n'a écrit entre-temps. La `version` est l'identité
/// de contenu de `docs/13` §3 — elle dit *quoi*. Un système qui n'aurait que la révision ne saurait
/// pas dire ce qu'il a commité ; un système qui n'aurait que le contenu ne saurait pas dire qu'il
/// était seul à écrire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Committed {
    /// La proposition commitée.
    pub proposal: Proposal,
    /// La révision produite.
    pub revision: u64,
    /// La version produite, dont le parent est celle sur laquelle le diff était écrit.
    pub version: Version,
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
/// # Deux vérifications, et aucune ne couvre l'autre
///
/// La révision dit que personne n'a écrit entre-temps ; le rejeu du diff dit que ce qu'on applique
/// est bien ce qui a été approuvé. Une base de révision peut correspondre alors que le diff a été
/// écrit sur une **autre lignée** de versions — c'est [`Diff::replay`] qui l'attrape, et son refus
/// dit lui aussi qu'il faut rebaser.
///
/// # Errors
///
/// [`ProposalError::Stale`] quand la base de révision ne correspond plus, et
/// [`ProposalError::Inapplicable`] quand le diff ne se rejoue pas sur la version courante — en
/// portant le refus du diff, qui nomme l'opération fautive et sa position.
pub fn commit(
    approved: Approved,
    current_revision: u64,
    current: &Version,
    digest: &impl Digest,
) -> Result<Committed, ProposalError> {
    let base = approved.proposal().base_revision();
    if base != current_revision {
        return Err(ProposalError::Stale {
            expected: base,
            actual: current_revision,
        });
    }
    let version = approved
        .proposal()
        .diff()
        .replay(current, digest)
        .map_err(|because| ProposalError::Inapplicable {
            detail: because.to_string(),
            rebase: because.needs_rebase(),
        })?;
    Ok(Committed {
        proposal: approved.proposal,
        revision: current_revision + 1,
        version,
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
    /// Le diff ne se rejoue pas sur la version courante.
    ///
    /// Distincte de [`ProposalError::Stale`], et les fondre perdrait la consigne : une base de
    /// révision périmée se rebase, une opération inapplicable se réécrit. `rebase` porte ce que le
    /// diff en a dit plutôt que de le redéduire — la consigne appartient à celui qui sait.
    Inapplicable {
        /// Ce que le diff a répondu.
        detail: String,
        /// Vrai quand rebaser suffit.
        rebase: bool,
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
            Self::Inapplicable { detail, rebase } => {
                let consigne = if *rebase {
                    "rebaser puis retenter"
                } else {
                    "réécrire le diff"
                };
                write!(
                    formatter,
                    "le diff ne s'applique pas : {detail} — {consigne}"
                )
            }
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
