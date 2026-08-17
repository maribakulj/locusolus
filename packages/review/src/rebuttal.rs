//! Le rebuttal et la méta-revue — `docs/SPEC_V1.md` §17.6, §17.7.
//!
//! # Ce que ces deux objets ajoutent au protocole
//!
//! W7.a a livré la revue : un dossier figé, des constats, une attestation d'indépendance. Il y
//! manquait la **réponse**. §17.1 exige que la revue rende explicites « les findings, **les
//! réponses** et la décision finale » — sans rebuttal, un constat est un verdict sans recours, et
//! le protocole cesse d'en être un.
//!
//! La méta-revue, elle, ne relit pas le travail : elle relit **les revues**. C'est la différence
//! qui décide de tout ce qui suit — une méta-revue qui refait la revue n'est qu'une revue de plus.

use std::collections::BTreeSet;
use std::fmt;

use locus_domain::RevisionId;
use locus_protocol::{Id, id::Agent};

use crate::review::{Review, Verdict};

/// Une réponse à un constat — §17.6.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rebuttal {
    finding_target: RevisionId,
    author: Id<Agent>,
    response: String,
    accepted: Vec<String>,
    contested: Vec<String>,
    corrections: Vec<RevisionId>,
    requests_recheck: bool,
}

impl Rebuttal {
    /// Répondre à un constat.
    ///
    /// # Un rebuttal ne s'écrit pas sans constat
    ///
    /// §17.6 fait du `finding_id` un champ obligatoire : une réponse qui ne répond à rien est une
    /// prise de parole, pas un rebuttal. Le type l'exige donc à la construction — c'est la
    /// différence entre un protocole et un fil de discussion.
    ///
    /// # Errors
    ///
    /// [`RebuttalError::EmptyResponse`] pour une réponse vide : contester en silence laisserait le
    /// relecteur deviner, et la réponse est ce qui distingue un rebuttal d'un désaccord.
    pub fn to_finding(
        finding_target: RevisionId,
        author: Id<Agent>,
        response: &str,
    ) -> Result<Self, RebuttalError> {
        if response.trim().is_empty() {
            return Err(RebuttalError::EmptyResponse);
        }
        Ok(Self {
            finding_target,
            author,
            response: response.to_owned(),
            accepted: Vec::new(),
            contested: Vec::new(),
            corrections: Vec::new(),
            requests_recheck: false,
        })
    }

    /// Accepter une partie du constat.
    #[must_use]
    pub fn accepting(mut self, part: &str) -> Self {
        self.accepted.push(part.to_owned());
        self
    }

    /// En contester une autre.
    #[must_use]
    pub fn contesting(mut self, part: &str) -> Self {
        self.contested.push(part.to_owned());
        self
    }

    /// Citer une correction produite en réponse.
    #[must_use]
    pub fn corrected_by(mut self, revision: RevisionId) -> Self {
        self.corrections.push(revision);
        self
    }

    /// Demander un recheck.
    #[must_use]
    pub const fn requesting_recheck(mut self) -> Self {
        self.requests_recheck = true;
        self
    }

    /// Le constat visé.
    #[must_use]
    pub const fn finding_target(&self) -> &RevisionId {
        &self.finding_target
    }

    /// Qui répond.
    #[must_use]
    pub const fn author(&self) -> Id<Agent> {
        self.author
    }

    /// Ce qui est répondu.
    #[must_use]
    pub fn response(&self) -> &str {
        &self.response
    }

    /// Ce qui est accepté.
    #[must_use]
    pub fn accepted(&self) -> &[String] {
        &self.accepted
    }

    /// Ce qui est contesté.
    #[must_use]
    pub fn contested(&self) -> &[String] {
        &self.contested
    }

    /// Les corrections citées.
    #[must_use]
    pub fn corrections(&self) -> &[RevisionId] {
        &self.corrections
    }

    /// Vrai quand un recheck est demandé.
    #[must_use]
    pub const fn requests_recheck(&self) -> bool {
        self.requests_recheck
    }
}

/// Qui peut effectuer le recheck — §17.6.
///
/// « Le reviewer initial peut effectuer un recheck. La **politique peut imposer un nouveau
/// reviewer** pour éviter l'auto-justification. » Les deux existent, et le choix est une décision
/// de politique — pas un défaut caché dans le code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RecheckPolicy {
    /// Le relecteur initial reprend son constat.
    #[default]
    InitialReviewer,
    /// Un relecteur neuf, pour éviter l'auto-justification.
    FreshReviewer,
}

/// Vérifier qu'un recheck respecte la politique.
///
/// # Errors
///
/// [`RebuttalError::SelfJustification`] quand la politique exige un relecteur neuf et que le
/// relecteur initial reprend la main.
pub fn assign_recheck(
    policy: RecheckPolicy,
    initial_reviewer: Id<Agent>,
    assigned: Id<Agent>,
) -> Result<(), RebuttalError> {
    if policy == RecheckPolicy::FreshReviewer && initial_reviewer == assigned {
        return Err(RebuttalError::SelfJustification);
    }
    Ok(())
}

/// Ce qu'une méta-revue recommande — §17.7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Recommendation {
    /// Valider.
    Validate,
    /// Réviser.
    Revise,
    /// Contester.
    Contest,
    /// Rejeter.
    Reject,
    /// Reproduire avant de conclure.
    Reproduce,
    /// Escalader à un humain.
    HumanEscalation,
}

impl Recommendation {
    /// Les six de §17.7.
    pub const ALL: [Self; 6] = [
        Self::Validate,
        Self::Revise,
        Self::Contest,
        Self::Reject,
        Self::Reproduce,
        Self::HumanEscalation,
    ];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Validate => "validate",
            Self::Revise => "revise",
            Self::Contest => "contest",
            Self::Reject => "reject",
            Self::Reproduce => "reproduce",
            Self::HumanEscalation => "human_escalation",
        }
    }
}

impl fmt::Display for Recommendation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Ce qu'une méta-revue constate — §17.7.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaReview {
    recommendation: Recommendation,
    effective_independence: usize,
    minority_opinions: Vec<Id<Agent>>,
    correlated_reviewers: Vec<(Id<Agent>, Id<Agent>)>,
}

impl MetaReview {
    /// Ce qu'elle recommande.
    #[must_use]
    pub const fn recommendation(&self) -> Recommendation {
        self.recommendation
    }

    /// Combien de revues étaient effectivement indépendantes.
    ///
    /// §17.7 : la méta-revue « mesure l'indépendance **effective** ». Trois revues dont deux
    /// partagent un groupe n'en font pas trois indépendantes, et compter les revues plutôt que
    /// leur indépendance ferait passer un consensus pour une convergence.
    #[must_use]
    pub const fn effective_independence(&self) -> usize {
        self.effective_independence
    }

    /// Les relecteurs dont l'avis est minoritaire.
    ///
    /// §17.7 : elle « ne masque **jamais** les opinions minoritaires ». Un avis minoritaire écarté
    /// pour faire une recommandation nette est exactement ce que l'invariant 12 refuse ailleurs.
    #[must_use]
    pub fn minority_opinions(&self) -> &[Id<Agent>] {
        &self.minority_opinions
    }

    /// Les paires de relecteurs dont les constats se ressemblent trop.
    ///
    /// §17.7 : « détecte les findings corrélés ou copiés ». Deux relecteurs qui rendent exactement
    /// les mêmes constats sur les mêmes cibles n'apportent pas deux avis.
    #[must_use]
    pub fn correlated_reviewers(&self) -> &[(Id<Agent>, Id<Agent>)] {
        &self.correlated_reviewers
    }
}

/// Conduire une méta-revue sur un ensemble de revues.
///
/// # Ce qu'elle regarde, et ce qu'elle ne refait pas
///
/// Elle relit **les revues**, pas le travail. Elle ne rouvre aucun dossier, ne produit aucun
/// constat propre : elle mesure l'indépendance effective, repère les avis corrélés, garde les
/// minoritaires et recommande. Une méta-revue qui refait la revue n'est qu'une revue de plus, et
/// c'est la confusion que §17.7 évite en la décrivant par ce qu'elle compare.
///
/// # Errors
///
/// [`RebuttalError::MetaReviewsItself`] quand le méta-relecteur a signé l'une des revues examinées.
pub fn meta_review(
    reviews: &[Review],
    meta_reviewer: Id<Agent>,
) -> Result<MetaReview, RebuttalError> {
    if reviews
        .iter()
        .any(|review| review.reviewer() == meta_reviewer)
    {
        return Err(RebuttalError::MetaReviewsItself);
    }

    let effective_independence = reviews
        .iter()
        .filter(|review| review.is_independent())
        .count();

    // Deux revues sont corrélées quand elles portent exactement les mêmes verdicts sur les mêmes
    // cibles. Ce n'est pas une preuve de copie — deux relecteurs peuvent converger — mais c'est ce
    // que §17.7 demande de **signaler**, pas de conclure.
    let mut correlated = Vec::new();
    for (index, left) in reviews.iter().enumerate() {
        for right in reviews.iter().skip(index + 1) {
            if signature(left) == signature(right) && !signature(left).is_empty() {
                correlated.push((left.reviewer(), right.reviewer()));
            }
        }
    }

    let refuting: Vec<Id<Agent>> = reviews
        .iter()
        .filter(|review| {
            review
                .binding_findings()
                .iter()
                .any(|finding| finding.verdict() == Verdict::Refutes)
        })
        .map(Review::reviewer)
        .collect();

    // §17.7 : la méta-revue « distingue absence de preuve, contradiction et réfutation ». Un constat
    // liant qui dit « il n'y a pas de quoi conclure » n'est pas une réfutation, et ce n'est pas non
    // plus rien : les confondre transformerait un manque en résultat.
    let insufficient = reviews.iter().any(|review| {
        review
            .binding_findings()
            .iter()
            .any(|finding| finding.verdict() == Verdict::Insufficient)
    });

    // Minoritaire = du côté le moins nombreux, quel que soit ce côté. Ne garder que les
    // « opposants » ferait disparaître l'unique voix favorable au milieu de réfutations.
    let supporting: Vec<Id<Agent>> = reviews
        .iter()
        .map(Review::reviewer)
        .filter(|reviewer| !refuting.contains(reviewer))
        .collect();
    let minority_opinions = if refuting.len() <= supporting.len() {
        refuting.clone()
    } else {
        supporting
    };

    let recommendation = if effective_independence == 0 {
        // Aucune revue indépendante : il n'y a pas de quoi trancher, et prétendre le contraire
        // ferait d'un défaut de procédure un verdict scientifique.
        Recommendation::HumanEscalation
    } else if refuting.len() == reviews.len() && !refuting.is_empty() {
        Recommendation::Reject
    } else if !refuting.is_empty() {
        // Désaccord : il **survit** à la synthèse, il ne s'y résout pas.
        Recommendation::Contest
    } else if insufficient {
        // Personne ne réfute, mais quelqu'un dit qu'il n'y a pas de quoi conclure. Valider ferait
        // de l'absence de preuve une preuve — c'est la confusion que §17.7 nomme en premier.
        Recommendation::Revise
    } else {
        Recommendation::Validate
    };

    Ok(MetaReview {
        recommendation,
        effective_independence,
        minority_opinions: minority_opinions
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        correlated_reviewers: correlated,
    })
}

/// La signature d'une revue : ses couples cible/verdict, triés.
fn signature(review: &Review) -> Vec<(RevisionId, Verdict)> {
    let mut items: Vec<(RevisionId, Verdict)> = review
        .findings()
        .iter()
        .map(|finding| (*finding.target(), finding.verdict()))
        .collect();
    items.sort_by_key(|(target, verdict)| (*target, verdict.slug()));
    items
}

/// Ce qui empêche un rebuttal ou une méta-revue d'exister.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebuttalError {
    /// Une réponse vide.
    EmptyResponse,
    /// Le relecteur initial reprend un recheck que la politique lui refuse.
    SelfJustification,
    /// Le méta-relecteur a signé l'une des revues qu'il examine.
    MetaReviewsItself,
}

impl fmt::Display for RebuttalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::EmptyResponse => "une réponse vide ne répond pas",
            Self::SelfJustification => {
                "la politique exige un relecteur neuf : se relire soi-même est l'auto-justification \
                 que §17.6 nomme"
            }
            Self::MetaReviewsItself => {
                "une méta-revue de sa propre revue ne compare rien : elle se répète"
            }
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for RebuttalError {}
