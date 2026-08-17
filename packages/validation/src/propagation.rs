//! La propagation de l'invalidation — `docs/SPEC_V1.md` §8.3.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use locus_domain::{RevisionId, ValidationLevel};
use locus_graph::{Graph, RelationKind};

use crate::policy::{InvalidatingEvent, TypePolicy};

/// Les relations par lesquelles une invalidation se propage.
///
/// La direction compte : on part de l'objet invalidé et on cherche **ce qui dépendait de lui**.
/// Pour `A depends_on B`, c'est `A` qui tombe quand `B` est réfuté — donc on remonte de la cible
/// vers la source.
///
/// La liste est **courte exprès**. `cites` n'y est pas : citer un article réfuté n'invalide pas
/// l'article citant, ça le rend seulement discutable, et marquer tout le corpus citant à chaque
/// rétractation noierait les vrais dépendants. §8.3 vise « une définition, une source, un dataset
/// ou une prémisse » — ce dont un objet **dépend**, pas ce qu'il mentionne.
pub const DEPENDENCY_RELATIONS: [RelationKind; 5] = [
    // `A depends_on B` : A tombe si B tombe. Le cas nominal.
    RelationKind::DependsOn,
    // `A derived_from B` : A est un produit de B.
    RelationKind::DerivedFrom,
    // `A instantiates B` : A est un cas de B ; B réfuté, A n'instancie plus rien.
    RelationKind::Instantiates,
    // `A formalizes B` : la formalisation d'un énoncé retiré n'a plus d'objet.
    RelationKind::Formalizes,
    // `A anchored_in B` : l'ancrage disparaît avec son point d'ancrage.
    RelationKind::AnchoredIn,
];

/// Ce qui déclenche une propagation — §8.3.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    /// La révision invalidée.
    pub revision_id: RevisionId,
    /// Ce qui lui est arrivé.
    pub event: InvalidatingEvent,
}

/// Ce qu'on savait d'un objet avant la propagation — §8.3, cinquième point.
///
/// « Conserve le niveau et la justification antérieurs dans l'historique. » Sans cette trace, une
/// réévaluation ne saurait pas ce qu'elle réévalue : elle repartirait de zéro, et le travail de
/// validation qui avait mené à L3 serait perdu au lieu d'être remis en question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriorAssessment {
    /// Le niveau atteint avant la propagation.
    pub level: ValidationLevel,
    /// Ce qui le justifiait.
    pub justification: String,
}

/// Un objet marqué à réévaluer — §8.3, troisième point.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReassessmentMark {
    /// L'objet à réévaluer.
    pub revision_id: RevisionId,
    /// La distance à l'objet invalidé. `1` = dépendant direct.
    pub distance: u32,
    /// Pourquoi il est marqué.
    pub reason: String,
    /// Ce qu'on savait avant — §8.3, cinquième point.
    ///
    /// `None` quand l'appelant n'a rien fourni pour cet objet. Le distinguer de « L0 » est le
    /// point : « je ne sais pas ce qu'il valait » et « il ne valait rien » ne sont pas la même
    /// information, et repartir de L0 sur la première effacerait un travail de validation.
    pub prior: Option<PriorAssessment>,
}

/// Une tâche de réévaluation — §8.3, quatrième point.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReassessmentTask {
    /// L'objet à réévaluer.
    pub revision_id: RevisionId,
    /// La discipline dont la politique l'ouvre.
    pub discipline: String,
    /// Ce qu'il faudra revérifier.
    pub requirements: Vec<String>,
}

/// Ce qu'une propagation a produit.
///
/// # Ce que ce type n'a pas
///
/// **Aucun champ ne porte un nouveau niveau de validation.** §8.3, deuxième point : Locus Solus
/// « **ne les réfute pas automatiquement** sans règle disciplinaire ». Une propagation qui
/// rendrait un niveau révisé aurait déjà pris la décision qu'elle a interdiction de prendre — et
/// l'appelant l'appliquerait, parce qu'un champ rendu par une fonction a l'air d'un résultat.
///
/// Ce que la propagation rend est une **question posée**, pas une réponse : des objets marqués et
/// des tâches ouvertes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Propagation {
    /// Ce qui a déclenché.
    pub trigger: Trigger,
    /// Les objets transitivement dépendants — §8.3, premier point.
    pub dependents: Vec<RevisionId>,
    /// Les marques `needs_reassessment` — troisième point.
    pub marks: Vec<ReassessmentMark>,
    /// Les tâches ouvertes — quatrième point.
    pub tasks: Vec<ReassessmentTask>,
    /// Ce qui n'a pas pu être fait, et pourquoi.
    ///
    /// Une propagation sans politique disciplinaire n'ouvre aucune tâche, et le **dit** plutôt que
    /// de rendre une liste vide qui se lirait « rien à réévaluer ».
    pub findings: Vec<String>,
}

/// Ce que l'appelant sait des objets avant la propagation.
pub type PriorAssessments = BTreeMap<RevisionId, PriorAssessment>;

/// Propager une invalidation — les cinq points de §8.3, dans l'ordre.
///
/// # Ce que la fonction fait, et dans quel ordre
///
/// 1. **identifie les objets transitivement dépendants** — parcours en largeur du graphe, par les
///    hyperarêtes d'inférence et par [`DEPENDENCY_RELATIONS`] ;
/// 2. **ne les réfute pas** — rien dans [`Propagation`] ne porte un niveau ;
/// 3. **les marque `needs_reassessment`** ;
/// 4. **ouvre des tâches selon la politique** — et signale quand il n'y a pas de politique ;
/// 5. **conserve le niveau et la justification antérieurs**, dans [`ReassessmentMark::prior`].
///
/// # La terminaison
///
/// Le parcours tient un ensemble de visités. Un graphe épistémique **contient** des cycles — deux
/// claims qui se soutiennent mutuellement, une définition qui s'appuie sur un cas qui l'instancie
/// — et une propagation qui ne les supporterait pas boucherait au premier corpus réel.
#[must_use]
pub fn propagate(
    graph: &Graph,
    trigger: Trigger,
    priors: &PriorAssessments,
    policy: Option<&TypePolicy>,
) -> Propagation {
    let mut findings = Vec::new();

    if let Some(policy) = policy {
        if !policy.invalidates(trigger.event) {
            findings.push(format!(
                "la politique de `{}` ne fait pas de cet événement un invalidant : aucun dépendant n'est marqué",
                policy.discipline
            ));
            return Propagation {
                trigger,
                dependents: Vec::new(),
                marks: Vec::new(),
                tasks: Vec::new(),
                findings,
            };
        }
    } else {
        // Sans politique, on marque quand même — §8.3 dit « marque `needs_reassessment` », pas
        // « marque si une discipline le demande ». Ce qui manque, ce sont les tâches du point 4,
        // et c'est ce que le constat dit.
        findings.push(
            "aucune politique disciplinaire : les dépendants sont marqués, aucune tâche n'est ouverte (§8.3, point 4)"
                .to_owned(),
        );
    }

    let mut visited: BTreeSet<RevisionId> = BTreeSet::new();
    visited.insert(trigger.revision_id);
    let mut queue: VecDeque<(RevisionId, u32)> = VecDeque::new();
    queue.push_back((trigger.revision_id, 0));

    let mut dependents = Vec::new();
    let mut marks = Vec::new();

    while let Some((current, distance)) = queue.pop_front() {
        for dependent in direct_dependents(graph, &current) {
            if !visited.insert(dependent) {
                continue;
            }
            let next = distance + 1;
            dependents.push(dependent);
            marks.push(ReassessmentMark {
                revision_id: dependent,
                distance: next,
                reason: format!(
                    "dépend de `{}`, {} à la distance {next}",
                    trigger.revision_id,
                    match trigger.event {
                        InvalidatingEvent::Refuted => "réfuté",
                        InvalidatingEvent::Withdrawn => "retiré",
                        InvalidatingEvent::Revised => "révisé",
                    }
                ),
                prior: priors.get(&dependent).cloned(),
            });
            queue.push_back((dependent, next));
        }
    }

    let tasks = policy.map_or_else(Vec::new, |policy| {
        dependents
            .iter()
            .map(|revision_id| ReassessmentTask {
                revision_id: *revision_id,
                discipline: policy.discipline.clone(),
                requirements: policy.minimal_evidence.clone(),
            })
            .collect()
    });

    Propagation {
        trigger,
        dependents,
        marks,
        tasks,
        findings,
    }
}

/// Ce qui dépend directement d'une révision.
///
/// Deux sources : les **conclusions** des inférences dont elle est prémisse — l'hyperarête de W1.e
/// — et les **sources** des relations de dépendance qui pointent vers elle.
fn direct_dependents(graph: &Graph, revision: &RevisionId) -> Vec<RevisionId> {
    let mut found: Vec<RevisionId> = graph
        .inferences_broken_by(revision)
        .into_iter()
        .flat_map(|inference| inference.conclusion_ids.clone())
        .collect();
    found.extend(
        graph
            .incoming(revision)
            .into_iter()
            .filter(|relation| DEPENDENCY_RELATIONS.contains(&relation.kind))
            .map(|relation| relation.from),
    );
    found.sort_unstable();
    found.dedup();
    found
}
