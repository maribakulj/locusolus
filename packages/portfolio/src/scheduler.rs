//! Le scheduler qualité-diversité — `docs/SPEC_V1.md` §13.3.
//!
//! # La phrase qui interdit le tri simple
//!
//! « La V1 **NE DOIT PAS** sélectionner uniquement les branches au score moyen le plus élevé. » Un
//! scheduler qui trierait par `V(b)` et couperait à N serait conforme à §13.4 et en violation de
//! §13.3 — et c'est le comportement qu'on obtient sans y penser, parce que c'est le plus simple à
//! écrire et le plus facile à défendre ligne à ligne.
//!
//! Les sept exigences de §13.3 existent parce qu'un portefeuille n'est pas une liste des meilleurs
//! éléments : c'est un ensemble qui doit rester capable de se tromper ailleurs qu'au même endroit.
//!
//! # Rien n'est départagé au hasard
//!
//! L'ordre de sélection est **total** : valeur décroissante, puis diversité décroissante, puis
//! identifiant croissant. Le dernier barreau ne dit rien de scientifique, et c'est exactement son
//! rôle — il rend le choix reproductible là où les deux premiers ne tranchent pas. Un tirage
//! aléatoire y serait plus honnête en apparence et indéfendable en pratique : deux exécutions du
//! même portefeuille ne s'expliqueraient plus l'une par l'autre.
//!
//! Conséquence testée : **mélanger la liste d'entrée ne change pas la sélection**. Sans le dernier
//! barreau, c'est l'ordre d'arrivée qui déciderait, en silence.

use std::collections::BTreeSet;
use std::fmt;

use locus_protocol::{Id, id::Branch};

use crate::value::Valuation;

/// Une branche candidate à une place du portefeuille.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// La branche.
    pub branch: Id<Branch>,
    /// Sa valeur, criblée — voir [`crate::value`].
    pub valuation: Valuation,
    /// Sa famille de méthode.
    pub method_family: String,
    /// Sa famille de modèle.
    pub model_family: String,
    /// L'hypothèse majeure qu'elle fait avancer, s'il y en a une.
    pub hypothesis: Option<String>,
    /// L'hypothèse majeure qu'elle cherche à **falsifier**, s'il y en a une.
    pub falsifies: Option<String>,
    /// Vrai quand elle porte un résultat négatif informatif.
    pub informative_negative: bool,
    /// Les niches méthodologiques qu'elle occupe.
    pub niches: BTreeSet<String>,
}

/// La politique de sélection — les sept exigences de §13.3, chiffrées.
///
/// Comme les poids de §13.4, ce sont des **politiques** : elles sont explicites, remplaçables, et
/// enregistrées avec la sélection. Un portefeuille dont on ne peut plus retrouver la politique est
/// une liste, pas une décision.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Policy {
    /// Part des places attribuées au score, en pourcentage. Le reste est la réserve exploratoire.
    pub exploitation_percent: u8,
    /// Part maximale d'une même famille de modèle ou de méthode, en pourcentage.
    pub max_family_percent: u8,
    /// Ce qu'un point de corrélation avec le déjà-retenu retire à la valeur.
    pub correlation_penalty: f64,
    /// Ce qu'un résultat négatif informatif ajoute.
    pub negative_bonus: f64,
    /// Points de corrélation pour une famille de méthode partagée.
    pub same_method_points: u8,
    /// Points de corrélation pour une famille de modèle partagée.
    pub same_model_points: u8,
    /// Points de corrélation pour un recouvrement total de niches.
    pub shared_niche_points: u8,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            exploitation_percent: 60,
            max_family_percent: 50,
            correlation_penalty: 0.02,
            negative_bonus: 0.5,
            same_method_points: 50,
            same_model_points: 20,
            shared_niche_points: 30,
        }
    }
}

/// Pourquoi une branche a été retenue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reason {
    /// Pour son score — la part d'exploitation.
    Exploitation,
    /// Pour ce qu'elle explore et que rien d'autre n'explore — la réserve exploratoire.
    ExploratoryReserve,
    /// Parce qu'une hypothèse majeure retenue doit avoir sa contradiction.
    FalsificationDuty,
}

impl Reason {
    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Exploitation => "exploitation",
            Self::ExploratoryReserve => "exploratory_reserve",
            Self::FalsificationDuty => "falsification_duty",
        }
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Une place attribuée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    /// La branche retenue.
    pub branch: Id<Branch>,
    /// Pourquoi.
    pub reason: Reason,
}

/// Le portefeuille retenu, et ce qu'il n'a pas pu tenir.
#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    slots: Vec<Slot>,
    unfalsified: BTreeSet<String>,
    policy: Policy,
}

impl Selection {
    /// Les places, dans l'ordre où elles ont été attribuées.
    #[must_use]
    pub fn slots(&self) -> &[Slot] {
        &self.slots
    }

    /// Les branches retenues.
    pub fn branches(&self) -> impl Iterator<Item = &Id<Branch>> {
        self.slots.iter().map(|slot| &slot.branch)
    }

    /// Vrai quand cette branche est retenue.
    #[must_use]
    pub fn holds(&self, branch: &Id<Branch>) -> bool {
        self.slots.iter().any(|slot| slot.branch == *branch)
    }

    /// Les hypothèses majeures retenues qu'**aucun** candidat ne cherche à falsifier.
    ///
    /// §13.3 exige « au moins une branche de falsification pour toute hypothèse majeure ». Quand
    /// aucun candidat ne la propose, l'exigence ne peut pas être tenue — et la taire ferait passer
    /// un portefeuille incomplet pour un portefeuille conforme. Elle est donc rendue, pas résolue.
    #[must_use]
    pub fn unfalsified_hypotheses(&self) -> &BTreeSet<String> {
        &self.unfalsified
    }

    /// La politique employée.
    #[must_use]
    pub const fn policy(&self) -> &Policy {
        &self.policy
    }
}

/// Composer un portefeuille de `slots` places.
///
/// # L'ordre des phases
///
/// 1. la part d'exploitation, au meilleur score **ajusté** — corrélation retirée, négatif primé ;
/// 2. la réserve exploratoire, au plus **loin** de ce qui est déjà retenu, sans regarder le score ;
/// 3. le devoir de falsification, qui déplace au besoin la place la moins bien classée.
///
/// La réserve vient après l'exploitation et avant le devoir, pour une raison : une réserve remplie
/// en premier serait remplie contre rien, et un devoir servi en premier serait servi sans savoir
/// quelles hypothèses sont finalement retenues.
#[must_use]
pub fn schedule(candidates: &[Candidate], slots: usize, policy: &Policy) -> Selection {
    let ordered = ranked(candidates);
    // Au moins une place d'exploitation dès qu'il y a une place. Sans ce plancher, un portefeuille
    // d'une seule place serait **entièrement** exploratoire — l'inverse de « une part
    // d'exploitation ». La réserve, elle, apparaît à partir de deux places : à une seule, les deux
    // exigences de §13.3 ne peuvent pas tenir ensemble, et c'est l'exploitation qui l'emporte.
    let exploitation = (slots * usize::from(policy.exploitation_percent) / 100).max(slots.min(1));

    let mut chosen: Vec<(usize, Reason)> = Vec::new();

    fill(
        &ordered,
        &mut chosen,
        exploitation,
        policy,
        slots,
        Reason::Exploitation,
        |candidate, taken| adjusted_score(candidate, taken, policy),
    );
    fill(
        &ordered,
        &mut chosen,
        slots,
        policy,
        slots,
        Reason::ExploratoryReserve,
        |candidate, taken| distance(candidate, taken, policy),
    );

    let unfalsified = serve_falsification_duty(&ordered, &mut chosen, slots);

    Selection {
        slots: chosen
            .into_iter()
            .map(|(index, reason)| Slot {
                branch: ordered[index].branch,
                reason,
            })
            .collect(),
        unfalsified,
        policy: *policy,
    }
}

/// L'ordre total : valeur décroissante, diversité décroissante, identifiant croissant.
///
/// Le dernier barreau ne dit rien de scientifique. Il est là pour que deux exécutions du même
/// portefeuille se ressemblent — et pour que l'ordre d'arrivée des candidats ne décide de rien.
fn ranked(candidates: &[Candidate]) -> Vec<Candidate> {
    let mut ordered = candidates.to_vec();
    ordered.sort_by(|left, right| {
        right
            .valuation
            .value()
            .total_cmp(&left.valuation.value())
            .then_with(|| right.niches.len().cmp(&left.niches.len()))
            .then_with(|| left.branch.cmp(&right.branch))
    });
    ordered
}

/// Remplir jusqu'à `target` places selon un critère, sans dépasser la concentration permise.
fn fill(
    ordered: &[Candidate],
    chosen: &mut Vec<(usize, Reason)>,
    target: usize,
    policy: &Policy,
    slots: usize,
    reason: Reason,
    score: impl Fn(&Candidate, &[&Candidate]) -> f64,
) {
    while chosen.len() < target.min(slots) {
        let taken: Vec<&Candidate> = chosen.iter().map(|(index, _)| &ordered[*index]).collect();
        let best = ordered
            .iter()
            .enumerate()
            .filter(|(index, _)| !chosen.iter().any(|(taken, _)| taken == index))
            .filter(|(_, candidate)| room_for(candidate, &taken, policy, slots))
            .min_by(|(_, left), (_, right)| {
                // Comparateur inversé + `min_by` : le « minimum » est donc le meilleur score, et
                // `min_by` rend le **premier** — c'est-à-dire, `ordered` étant déjà trié, le mieux
                // classé. `max_by` rendrait le dernier, donc l'égalité irait au moins bien classé.
                score(right, &taken).total_cmp(&score(left, &taken))
            });
        let Some((index, _)) = best else { return };
        chosen.push((index, reason));
    }
}

/// La valeur, corrigée de ce qui est déjà retenu — §13.3, pénalité de corrélation et prime au
/// négatif informatif.
fn adjusted_score(candidate: &Candidate, taken: &[&Candidate], policy: &Policy) -> f64 {
    let bonus = if candidate.informative_negative {
        policy.negative_bonus
    } else {
        0.0
    };
    candidate.valuation.value() + bonus
        - policy.correlation_penalty * f64::from(worst_correlation(candidate, taken, policy))
}

/// La distance au portefeuille — ce que regarde la réserve exploratoire, et elle ne regarde que ça.
///
/// Y glisser la valeur ferait de la réserve une seconde part d'exploitation, un peu plus indulgente,
/// et §13.3 ne serait tenue qu'en apparence.
fn distance(candidate: &Candidate, taken: &[&Candidate], policy: &Policy) -> f64 {
    f64::from(100 - worst_correlation(candidate, taken, policy))
}

/// À quel point ce candidat ressemble à ce qui lui ressemble le plus, de 0 à 100.
fn worst_correlation(candidate: &Candidate, taken: &[&Candidate], policy: &Policy) -> u8 {
    taken
        .iter()
        .map(|other| correlation(candidate, other, policy))
        .max()
        .unwrap_or(0)
}

fn correlation(left: &Candidate, right: &Candidate, policy: &Policy) -> u8 {
    let mut points = 0_u16;
    if left.method_family == right.method_family {
        points += u16::from(policy.same_method_points);
    }
    if left.model_family == right.model_family {
        points += u16::from(policy.same_model_points);
    }
    let union = left.niches.union(&right.niches).count();
    if union > 0 {
        let shared = left.niches.intersection(&right.niches).count();
        points += u16::from(policy.shared_niche_points) * u16::try_from(shared).unwrap_or(u16::MAX)
            / u16::try_from(union).unwrap_or(1);
    }
    u8::try_from(points.min(100)).unwrap_or(100)
}

/// La limite de concentration par famille — §13.3, dernier point.
fn room_for(candidate: &Candidate, taken: &[&Candidate], policy: &Policy, slots: usize) -> bool {
    // Au moins une place par famille, sinon la limite interdirait toute sélection sur un petit
    // portefeuille — et une règle qui empêche de choisir ne protège rien.
    let ceiling = (slots * usize::from(policy.max_family_percent) / 100).max(1);
    let method = taken
        .iter()
        .filter(|other| other.method_family == candidate.method_family)
        .count();
    let model = taken
        .iter()
        .filter(|other| other.model_family == candidate.model_family)
        .count();
    method < ceiling && model < ceiling
}

/// « Au moins une branche de falsification pour toute hypothèse majeure » — §13.3.
///
/// Ce qui manque est **rendu**, pas comblé en silence : quand aucun candidat ne falsifie une
/// hypothèse retenue, l'exigence ne peut pas être tenue, et la taire ferait passer un portefeuille
/// incomplet pour un portefeuille conforme.
fn serve_falsification_duty(
    ordered: &[Candidate],
    chosen: &mut Vec<(usize, Reason)>,
    slots: usize,
) -> BTreeSet<String> {
    let mut missing = BTreeSet::new();
    let mut guard = 0_usize;

    loop {
        guard += 1;
        if guard > slots + 1 {
            break;
        }
        let held: BTreeSet<&str> = chosen
            .iter()
            .filter_map(|(index, _)| ordered[*index].falsifies.as_deref())
            .collect();
        let Some(orphan) = chosen
            .iter()
            .filter_map(|(index, _)| ordered[*index].hypothesis.as_deref())
            .find(|hypothesis| !held.contains(hypothesis) && !missing.contains(*hypothesis))
            .map(str::to_owned)
        else {
            break;
        };

        let candidate = ordered.iter().position(|candidate| {
            candidate.falsifies.as_deref() == Some(orphan.as_str())
                && !chosen
                    .iter()
                    .any(|(index, _)| ordered[*index].branch == candidate.branch)
        });
        let Some(index) = candidate else {
            missing.insert(orphan);
            continue;
        };

        if chosen.len() >= slots {
            // Le portefeuille est plein : la contradiction déplace la place la moins bien classée
            // qui n'est pas elle-même un devoir. `ordered` est trié, donc c'est le plus grand rang.
            let weakest = chosen
                .iter()
                .enumerate()
                .filter(|(_, (_, reason))| *reason != Reason::FalsificationDuty)
                .max_by_key(|(_, (rank, _))| *rank)
                .map(|(position, _)| position);
            let Some(position) = weakest else { break };
            chosen.remove(position);
        }
        chosen.push((index, Reason::FalsificationDuty));
    }

    missing
}
