//! L'anti-gaming du portefeuille — `docs/SPEC_V1.md` §13.6.
//!
//! # Pourquoi ce module vient en premier
//!
//! `docs/10` l'inscrit : « l'anti-gaming doit exister avant que la fonction de valeur pilote des
//! décisions automatiques ». La raison n'est pas la prudence, c'est l'ordre de dépendance : une
//! fonction de valeur mise en service avant ses garde-fous **enseigne** aux agents ce qu'il faut
//! optimiser, et ce qu'ils apprennent alors est précisément la faille. Ajouter les détecteurs
//! ensuite ne défait pas ce qui a été appris.
//!
//! # Ce que les détecteurs ne font pas
//!
//! Ils ne concluent pas à la fraude. §13.6 demande de « détecter **et pénaliser** » : un constat est
//! un signal chiffré, destiné à être appliqué comme une pénalité par ce qui valorisera les branches.
//! Traiter un signal comme une preuve punirait la coïncidence ; l'ignorer récompenserait la
//! stratégie.
//!
//! # La similarité est un port
//!
//! « Duplications paraphrastiques » demande de savoir si deux énoncés disent la même chose. Le
//! domaine ne le sait pas et ne le simule pas : il l'appelle. [`Similarity`] est un port, et
//! [`LexicalSimilarity`] en est une implémentation de référence — un **plancher** lexical, pas une
//! mesure sémantique. La nommer autrement ferait croire à une capacité qui n'existe pas ici.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use locus_protocol::{Id, id::Agent};

use crate::activity::BranchActivity;

/// Les sept formes de §13.6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Gaming {
    /// Multiplication artificielle de claims triviaux.
    TrivialClaimInflation,
    /// Inflation de confiance.
    ConfidenceInflation,
    /// Duplications paraphrastiques.
    ParaphraseDuplication,
    /// Production de tâches pour maximiser l'activité.
    ActivityInflation,
    /// Collusion de reviewers.
    ReviewerCollusion,
    /// Fragmentation artificielle d'artefacts.
    ArtifactFragmentation,
    /// Sélection opportuniste de métriques.
    MetricCherryPicking,
}

impl Gaming {
    /// Les sept.
    pub const ALL: [Self; 7] = [
        Self::TrivialClaimInflation,
        Self::ConfidenceInflation,
        Self::ParaphraseDuplication,
        Self::ActivityInflation,
        Self::ReviewerCollusion,
        Self::ArtifactFragmentation,
        Self::MetricCherryPicking,
    ];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::TrivialClaimInflation => "trivial_claim_inflation",
            Self::ConfidenceInflation => "confidence_inflation",
            Self::ParaphraseDuplication => "paraphrase_duplication",
            Self::ActivityInflation => "activity_inflation",
            Self::ReviewerCollusion => "reviewer_collusion",
            Self::ArtifactFragmentation => "artifact_fragmentation",
            Self::MetricCherryPicking => "metric_cherry_picking",
        }
    }
}

impl fmt::Display for Gaming {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Un constat de criblage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GamingFinding {
    /// Quelle forme.
    pub kind: Gaming,
    /// Ce qui a été vu.
    pub detail: String,
    /// L'intensité du signal, de 1 à 100 — jamais une probabilité de fraude.
    pub strength: u8,
}

impl fmt::Display for GamingFinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} ({}) : {}",
            self.kind, self.strength, self.detail
        )
    }
}

/// Savoir si deux énoncés disent la même chose.
///
/// Un port, pas une implémentation. Le score va de 0 à 100 : un entier, parce qu'un seuil de
/// politique comparé à un flottant serait sensible à l'ordre des opérations, et qu'un criblage doit
/// rendre le même verdict sur le même relevé.
pub trait Similarity {
    /// À quel point ces deux énoncés se recouvrent.
    fn score(&self, left: &str, right: &str) -> u8;
}

/// Un plancher lexical — le recouvrement des mots, normalisé.
///
/// Ce n'est **pas** une mesure sémantique, et le nom le dit. Deux paraphrases qui ne partagent aucun
/// mot lui échappent ; c'est la limite d'un plancher, et la raison pour laquelle [`Similarity`] est
/// un port. Ce qu'elle attrape — la reformulation cosmétique — est justement la forme la moins
/// coûteuse à produire, donc la plus probable.
#[derive(Debug, Clone, Copy, Default)]
pub struct LexicalSimilarity;

impl Similarity for LexicalSimilarity {
    fn score(&self, left: &str, right: &str) -> u8 {
        let left = tokens(left);
        let right = tokens(right);
        if left.is_empty() && right.is_empty() {
            return 100;
        }
        let union = left.union(&right).count();
        if union == 0 {
            return 0;
        }
        let shared = left.intersection(&right).count();
        u8::try_from(shared * 100 / union).unwrap_or(100)
    }
}

fn tokens(statement: &str) -> BTreeSet<String> {
    statement
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Les seuils du criblage.
///
/// §13.4 le dit de la fonction de valeur et cela vaut ici : ce sont des **politiques**, pas des
/// vérités. Ils sont donc explicites, portés par une valeur qu'on peut remplacer, et enregistrés
/// avec le criblage — un seuil enfoui dans le code serait une décision que personne n'a prise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thresholds {
    /// En deçà de ce nombre de revendications, le volume ne dit rien.
    pub min_claims: usize,
    /// Part maximale de revendications sans preuve, en pourcentage.
    pub max_unsupported_percent: u8,
    /// Écart maximal toléré entre confiance déclarée et taux de confirmation, en points.
    pub max_calibration_gap: u8,
    /// À partir de quel recouvrement deux énoncés comptent comme une duplication.
    pub duplicate_similarity: u8,
    /// En deçà de ce nombre de tâches, le volume ne dit rien.
    pub min_tasks: usize,
    /// Taux d'acceptation minimal, en pourcentage.
    pub min_acceptance_percent: u8,
    /// Nombre de revues croisées à partir duquel une réciprocité totale compte.
    pub min_mutual_reviews: usize,
    /// Taille en deçà de laquelle un artefact est un fragment.
    pub fragment_size_bytes: u64,
    /// Nombre de fragments d'un même ensemble à partir duquel la fragmentation compte.
    pub min_fragments: usize,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            min_claims: 5,
            max_unsupported_percent: 50,
            max_calibration_gap: 30,
            duplicate_similarity: 80,
            min_tasks: 5,
            min_acceptance_percent: 30,
            min_mutual_reviews: 3,
            fragment_size_bytes: 1024,
            min_fragments: 5,
        }
    }
}

/// Le résultat d'un criblage.
///
/// # Ce qu'il prouve, et ce qu'il ne prouve pas
///
/// Qu'une branche a été **regardée**. Un criblage sans constat n'est pas l'absence de criblage, et
/// c'est toute la différence : ce type n'a pas d'autre constructeur que [`screen`], de sorte que ce
/// qui valorisera les branches pourra l'exiger — et qu'une branche jamais criblée n'aura pas une
/// valeur haute, mais pas de valeur du tout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screening {
    findings: Vec<GamingFinding>,
    thresholds: Thresholds,
}

impl Screening {
    /// Ce qui a été vu.
    #[must_use]
    pub fn findings(&self) -> &[GamingFinding] {
        &self.findings
    }

    /// Les seuils employés — §13.4 exige que les paramètres soient enregistrés.
    #[must_use]
    pub const fn thresholds(&self) -> &Thresholds {
        &self.thresholds
    }

    /// La somme des signaux, bornée à 100.
    ///
    /// Bornée, parce qu'une pénalité sans plafond permettrait à un seul détecteur mal réglé
    /// d'annuler toute valeur — et un anti-gaming qui annule tout ne discrimine plus rien.
    #[must_use]
    pub fn pressure(&self) -> u8 {
        let total: u32 = self
            .findings
            .iter()
            .map(|finding| u32::from(finding.strength))
            .sum();
        u8::try_from(total.min(100)).unwrap_or(100)
    }

    /// Vrai quand rien n'a été signalé.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }
}

/// Cribler une branche.
///
/// Les sept détecteurs sont indépendants et tous appliqués : s'arrêter au premier constat ferait
/// réparer une stratégie en laissant les six autres, et le rapport donnerait l'impression du
/// contraire.
#[must_use]
pub fn screen(
    activity: &BranchActivity,
    similarity: &dyn Similarity,
    thresholds: Thresholds,
) -> Screening {
    let mut findings = Vec::new();
    findings.extend(trivial_claims(activity, thresholds));
    findings.extend(confidence_inflation(activity, thresholds));
    findings.extend(paraphrases(activity, similarity, thresholds));
    findings.extend(activity_inflation(activity, thresholds));
    findings.extend(collusion(activity, thresholds));
    findings.extend(fragmentation(activity, thresholds));
    findings.extend(cherry_picking(activity));
    Screening {
        findings,
        thresholds,
    }
}

// -- les sept -------------------------------------------------------------------------------

/// 1 — Multiplication artificielle de claims triviaux.
fn trivial_claims(activity: &BranchActivity, thresholds: Thresholds) -> Option<GamingFinding> {
    if activity.claims.len() < thresholds.min_claims {
        return None;
    }
    let unsupported = activity
        .claims
        .iter()
        .filter(|claim| claim.evidence_count == 0)
        .count();
    let percent = unsupported * 100 / activity.claims.len();
    if percent <= usize::from(thresholds.max_unsupported_percent) {
        return None;
    }
    Some(GamingFinding {
        kind: Gaming::TrivialClaimInflation,
        detail: format!(
            "{unsupported} revendications sans preuve sur {} — le compte monte, pas la connaissance",
            activity.claims.len()
        ),
        strength: strength(percent),
    })
}

/// 2 — Inflation de confiance.
///
/// La confiance déclarée se compare au taux de confirmation **observé**. Comparer la confiance à
/// elle-même ne dirait rien : c'est la calibration qui manque, pas l'assurance.
fn confidence_inflation(
    activity: &BranchActivity,
    thresholds: Thresholds,
) -> Option<GamingFinding> {
    let settled: Vec<_> = activity
        .claims
        .iter()
        .filter(|claim| claim.held_up.is_some())
        .collect();
    if settled.is_empty() {
        return None;
    }
    let declared: usize = settled
        .iter()
        .map(|claim| usize::from(claim.declared_confidence))
        .sum::<usize>()
        / settled.len();
    let confirmed = settled
        .iter()
        .filter(|claim| claim.held_up == Some(true))
        .count()
        * 100
        / settled.len();
    let gap = declared.saturating_sub(confirmed);
    if gap <= usize::from(thresholds.max_calibration_gap) {
        return None;
    }
    Some(GamingFinding {
        kind: Gaming::ConfidenceInflation,
        detail: format!(
            "confiance déclarée {declared} %, confirmée {confirmed} % — {gap} points d'écart"
        ),
        strength: strength(gap),
    })
}

/// 3 — Duplications paraphrastiques.
fn paraphrases(
    activity: &BranchActivity,
    similarity: &dyn Similarity,
    thresholds: Thresholds,
) -> Option<GamingFinding> {
    let mut duplicates = 0_usize;
    for (index, left) in activity.claims.iter().enumerate() {
        for right in activity.claims.iter().skip(index + 1) {
            if similarity.score(&left.statement, &right.statement)
                >= thresholds.duplicate_similarity
            {
                duplicates += 1;
            }
        }
    }
    if duplicates == 0 {
        return None;
    }
    Some(GamingFinding {
        kind: Gaming::ParaphraseDuplication,
        detail: format!("{duplicates} paires d'énoncés disent la même chose autrement"),
        strength: strength(duplicates * 20),
    })
}

/// 4 — Production de tâches pour maximiser l'activité.
fn activity_inflation(activity: &BranchActivity, thresholds: Thresholds) -> Option<GamingFinding> {
    if activity.tasks_created < thresholds.min_tasks {
        return None;
    }
    let accepted = activity.tasks_accepted * 100 / activity.tasks_created;
    if accepted >= usize::from(thresholds.min_acceptance_percent) {
        return None;
    }
    Some(GamingFinding {
        kind: Gaming::ActivityInflation,
        detail: format!(
            "{} tâches créées, {} acceptées — {accepted} % d'aboutissement",
            activity.tasks_created, activity.tasks_accepted
        ),
        strength: strength(100 - accepted),
    })
}

/// 5 — Collusion de reviewers.
///
/// La réciprocité **totale** dans les deux sens, au-delà d'un volume : deux relecteurs qui ne se
/// refusent jamais rien ne se relisent pas, ils se couvrent. Un seul sens ne suffit pas — approuver
/// systématiquement quelqu'un qui vous refuse parfois n'est pas une entente.
fn collusion(activity: &BranchActivity, thresholds: Thresholds) -> Option<GamingFinding> {
    let mut ledger: BTreeMap<(Id<Agent>, Id<Agent>), (usize, usize)> = BTreeMap::new();
    for review in &activity.reviews {
        let entry = ledger.entry((review.reviewer, review.author)).or_default();
        entry.0 += 1;
        if review.approves {
            entry.1 += 1;
        }
    }

    let mut pairs = Vec::new();
    for ((reviewer, author), (total, approvals)) in &ledger {
        if reviewer >= author {
            continue;
        }
        let Some((back_total, back_approvals)) = ledger.get(&(*author, *reviewer)) else {
            continue;
        };
        let both_ways =
            *total >= thresholds.min_mutual_reviews && *back_total >= thresholds.min_mutual_reviews;
        if both_ways && total == approvals && back_total == back_approvals {
            pairs.push((*reviewer, *author));
        }
    }

    if pairs.is_empty() {
        return None;
    }
    Some(GamingFinding {
        kind: Gaming::ReviewerCollusion,
        detail: format!("{} paire(s) ne se refusent jamais rien", pairs.len()),
        strength: strength(pairs.len() * 40),
    })
}

/// 6 — Fragmentation artificielle d'artefacts.
fn fragmentation(activity: &BranchActivity, thresholds: Thresholds) -> Option<GamingFinding> {
    let mut per_unit: BTreeMap<&str, usize> = BTreeMap::new();
    for artifact in &activity.artifacts {
        if artifact.size_bytes < thresholds.fragment_size_bytes {
            *per_unit.entry(artifact.logical_unit.as_str()).or_insert(0) += 1;
        }
    }
    let worst = per_unit
        .into_iter()
        .filter(|(_, count)| *count >= thresholds.min_fragments)
        .max_by_key(|(_, count)| *count)?;
    Some(GamingFinding {
        kind: Gaming::ArtifactFragmentation,
        detail: format!(
            "« {} » découpé en {} fragments sous le seuil",
            worst.0, worst.1
        ),
        strength: strength(worst.1 * 15),
    })
}

/// 7 — Sélection opportuniste de métriques.
///
/// Les deux sens comptent. Taire une métrique pré-enregistrée et en rapporter une qui ne l'était pas
/// sont la même manœuvre vue de deux côtés : choisir après avoir vu les résultats.
fn cherry_picking(activity: &BranchActivity) -> Option<GamingFinding> {
    if activity.preregistered_metrics.is_empty() {
        return None;
    }
    let preregistered: BTreeSet<&str> = activity
        .preregistered_metrics
        .iter()
        .map(String::as_str)
        .collect();
    let reported: BTreeSet<&str> = activity
        .reported_metrics
        .iter()
        .map(String::as_str)
        .collect();

    let dropped: Vec<&str> = preregistered.difference(&reported).copied().collect();
    let added: Vec<&str> = reported.difference(&preregistered).copied().collect();
    if dropped.is_empty() && added.is_empty() {
        return None;
    }
    Some(GamingFinding {
        kind: Gaming::MetricCherryPicking,
        detail: format!(
            "{} métrique(s) pré-enregistrée(s) tue(s), {} rapportée(s) sans l'avoir été",
            dropped.len(),
            added.len()
        ),
        strength: strength((dropped.len() + added.len()) * 25),
    })
}

/// Un signal, borné à 1..=100 — jamais nul, sans quoi un constat ne pèserait rien.
fn strength(raw: usize) -> u8 {
    u8::try_from(raw.clamp(1, 100)).unwrap_or(100)
}
