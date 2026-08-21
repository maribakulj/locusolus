//! L'alignement d'ontologies comme **proposition** — `W14.e`, ADR 0023 décision 6.
//!
//! # Ce que dit la preuve, et pourquoi elle décide de la forme
//!
//! Le dépôt d'alignement examiné par l'ADR publie une ablation qui répond directement à « qu'est-ce
//! qui porte le résultat » : la **contrainte structurelle**, et non la pondération des similarités.
//! Retirer l'appariement un-à-un comme seule variable fait chuter le F1 de 0,829 à 0,728, tandis que
//! cinq configurations de pondération s'écartent de 0,0033.
//!
//! Autrement dit : l'identité entre régimes descriptifs est structurelle, difficile, et **non
//! résolue** par la similarité. Un matcher propose ; il ne décide jamais que deux choses sont la
//! même.
//!
//! `SPEC_V1.md` §18.4 le dit déjà de son côté, pour la fusion de branches : « ne jamais fusionner
//! automatiquement deux concepts sur seule similarité vectorielle ». Les deux textes se rencontrent
//! ici, et aucun n'a été écrit en connaissance de l'autre.
//!
//! # Ce que le refus doit dire
//!
//! **La contrainte non satisfaite, jamais un score.** Un refus qui rendrait « 0,62 » enverrait son
//! lecteur chercher un seuil, alors que le problème n'en est pas un : la paire est refusée parce
//! qu'un des deux termes est **déjà** apparié ailleurs, et aucune confiance supplémentaire ne
//! changerait cela.

use std::collections::BTreeMap;
use std::fmt;

/// La sorte d'équivalence proposée — les trois que les ontologies emploient.
///
/// Liste close. Elles ne sont **pas** interchangeables : `SameAs` porte sur des individus et se
/// propage par transitivité, `EquivalentClass` sur des classes, `ExactMatch` est une correspondance
/// de vocabulaire contrôlé qui ne promet aucune inférence. Les confondre ferait tirer d'un simple
/// rapprochement de thésaurus des conclusions logiques que personne n'a autorisées.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Equivalence {
    /// `owl:equivalentClass` — deux classes ont les mêmes instances.
    EquivalentClass,
    /// `skos:exactMatch` — deux concepts de vocabulaires contrôlés se correspondent.
    ExactMatch,
    /// `owl:sameAs` — deux individus sont le même.
    SameAs,
}

impl Equivalence {
    /// Les trois.
    pub const ALL: [Self; 3] = [Self::EquivalentClass, Self::ExactMatch, Self::SameAs];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::EquivalentClass => "owl:equivalentClass",
            Self::ExactMatch => "skos:exactMatch",
            Self::SameAs => "owl:sameAs",
        }
    }
}

impl fmt::Display for Equivalence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Un alignement **proposé** — jamais écrit.
///
/// Le type ne porte aucun score, et c'est délibéré : un score dans la proposition inviterait à la
/// trancher en le comparant à un seuil, c'est-à-dire à décider par similarité. Ce qui décide est la
/// politique, puis l'approbation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlignmentProposal {
    left: String,
    right: String,
    relation: Equivalence,
    author: String,
    base_revision: u64,
}

impl AlignmentProposal {
    /// Proposer un alignement.
    ///
    /// # Errors
    ///
    /// [`AlignmentError::EmptyTerm`] pour un terme vide, [`AlignmentError::Reflexive`] pour une
    /// paire dont les deux moitiés sont le même terme — dire qu'une chose est elle-même n'aligne
    /// rien et occuperait un appariement qu'un vrai alignement ne pourrait plus prendre.
    pub fn propose(
        left: impl Into<String>,
        right: impl Into<String>,
        relation: Equivalence,
        author: impl Into<String>,
        base_revision: u64,
    ) -> Result<Self, AlignmentError> {
        let (left, right, author) = (left.into(), right.into(), author.into());
        for (field, value) in [("left", &left), ("right", &right), ("author", &author)] {
            if value.trim().is_empty() {
                return Err(AlignmentError::EmptyTerm { field });
            }
        }
        if left == right {
            return Err(AlignmentError::Reflexive { term: left });
        }
        Ok(Self {
            left,
            right,
            relation,
            author,
            base_revision,
        })
    }

    /// Le terme de gauche.
    #[must_use]
    pub fn left(&self) -> &str {
        &self.left
    }

    /// Le terme de droite.
    #[must_use]
    pub fn right(&self) -> &str {
        &self.right
    }

    /// La sorte d'équivalence.
    #[must_use]
    pub const fn relation(&self) -> Equivalence {
        self.relation
    }

    /// Qui propose.
    #[must_use]
    pub fn author(&self) -> &str {
        &self.author
    }

    /// La révision sur laquelle elle est écrite.
    #[must_use]
    pub const fn base_revision(&self) -> u64 {
        self.base_revision
    }
}

/// Un alignement approuvé — le **seul** objet que le registre accepte.
///
/// Il ne se construit que par [`approve`], et il n'a aucun champ public : c'est ce qui rend « aucun
/// chemin n'écrit une équivalence sans décision » vrai par signature plutôt que par vigilance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovedAlignment {
    proposal: AlignmentProposal,
    approver: String,
}

impl ApprovedAlignment {
    /// Ce qui a été approuvé.
    #[must_use]
    pub const fn proposal(&self) -> &AlignmentProposal {
        &self.proposal
    }

    /// Qui a approuvé.
    #[must_use]
    pub fn approver(&self) -> &str {
        &self.approver
    }
}

/// Approuver un alignement.
///
/// # Errors
///
/// [`AlignmentError::SelfApproval`] quand l'approbateur est l'auteur — la même borne que
/// `coordination::approve`, et pour la même raison : elle empêche un proposeur de contrôler la
/// décision sur sa propre proposition.
pub fn approve(
    proposal: AlignmentProposal,
    approver: impl Into<String>,
) -> Result<ApprovedAlignment, AlignmentError> {
    let approver = approver.into();
    if approver == proposal.author {
        return Err(AlignmentError::SelfApproval { author: approver });
    }
    Ok(ApprovedAlignment { proposal, approver })
}

/// Les alignements retenus, et la contrainte qui les tient.
///
/// # L'appariement est un-à-un, et c'est la contrainte qui porte le résultat
///
/// Un terme déjà apparié ne s'apparie pas une seconde fois. Ce n'est pas une prudence : c'est ce
/// que l'ablation mesure. Sans elle, un matcher rapproche un terme de tout ce qui lui ressemble, et
/// l'ensemble cesse d'être une identité pour devenir un voisinage.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Alignments {
    by_term: BTreeMap<String, (String, Equivalence)>,
    revision: u64,
}

impl Alignments {
    /// Un registre vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// La révision courante.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Ce à quoi ce terme est apparié, s'il l'est.
    #[must_use]
    pub fn partner(&self, term: &str) -> Option<(&str, Equivalence)> {
        self.by_term
            .get(term)
            .map(|(other, relation)| (other.as_str(), *relation))
    }

    /// Commiter un alignement approuvé.
    ///
    /// # Errors
    ///
    /// [`AlignmentError::Stale`] quand la base de révision a bougé — deux propositions
    /// contradictoires sur la même paire ne committent donc pas toutes deux ;
    /// [`AlignmentError::AlreadyMatched`] quand l'un des deux termes est déjà apparié, en **nommant
    /// la contrainte** et le partenaire existant plutôt qu'en rendant un score.
    pub fn commit(&mut self, approved: &ApprovedAlignment) -> Result<u64, AlignmentError> {
        let proposal = approved.proposal();
        if proposal.base_revision != self.revision {
            return Err(AlignmentError::Stale {
                expected: proposal.base_revision,
                actual: self.revision,
            });
        }
        for term in [proposal.left(), proposal.right()] {
            if let Some((partner, relation)) = self.partner(term) {
                return Err(AlignmentError::AlreadyMatched {
                    term: term.to_owned(),
                    partner: partner.to_owned(),
                    relation,
                });
            }
        }
        self.by_term.insert(
            proposal.left.clone(),
            (proposal.right.clone(), proposal.relation),
        );
        self.by_term.insert(
            proposal.right.clone(),
            (proposal.left.clone(), proposal.relation),
        );
        self.revision += 1;
        Ok(self.revision)
    }
}

/// Pourquoi un alignement est refusé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlignmentError {
    /// Un terme ou un auteur vide.
    EmptyTerm {
        /// Lequel.
        field: &'static str,
    },
    /// Une paire dont les deux moitiés sont le même terme.
    Reflexive {
        /// Lequel.
        term: String,
    },
    /// Un auteur qui approuve sa propre proposition.
    SelfApproval {
        /// Lequel.
        author: String,
    },
    /// La base de révision a bougé.
    Stale {
        /// Celle sur laquelle la proposition était écrite.
        expected: u64,
        /// Celle du registre.
        actual: u64,
    },
    /// **La contrainte structurelle** : un des deux termes est déjà apparié.
    AlreadyMatched {
        /// Lequel.
        term: String,
        /// À quoi il l'est déjà.
        partner: String,
        /// Sous quelle sorte d'équivalence.
        relation: Equivalence,
    },
}

impl fmt::Display for AlignmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTerm { field } => write!(formatter, "« {field} » est vide"),
            Self::Reflexive { term } => write!(
                formatter,
                "« {term} » aligné sur lui-même : cela n'aligne rien et occuperait un appariement \
                 qu'un vrai alignement ne pourrait plus prendre"
            ),
            Self::SelfApproval { author } => write!(
                formatter,
                "« {author} » approuve sa propre proposition d'alignement"
            ),
            Self::Stale { expected, actual } => write!(
                formatter,
                "alignement écrit sur la révision {expected}, le registre est en {actual} : \
                 rebaser puis retenter"
            ),
            Self::AlreadyMatched {
                term,
                partner,
                relation,
            } => write!(
                formatter,
                "« {term} » est déjà apparié à « {partner} » par {relation} : l'appariement est \
                 un-à-un, et c'est cette contrainte — pas une pondération de similarité — qui porte \
                 le résultat"
            ),
        }
    }
}

impl std::error::Error for AlignmentError {}
