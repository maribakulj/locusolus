//! La compensation — `docs/SPEC_V1.md` §11.4.

use locus_workflow::{Step, WorkflowDefinition};

use crate::history::HistoryEvent;

/// Une compensation à appliquer : quelle activity défaire, et par laquelle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompensationStep {
    /// L'indice du pas à défaire.
    pub index: usize,
    /// L'activity qui a eu lieu.
    pub activity: String,
    /// Celle qui la défait.
    pub by: String,
}

/// Une activity abordée dont le résultat n'est jamais revenu.
///
/// # Pourquoi ce n'est pas une compensation de plus
///
/// Le pas a été abordé, et l'historique s'arrête là : un worker l'a peut-être exécuté avant de
/// mourir, ou peut-être pas. **On ne sait pas si l'effet a eu lieu**, et les deux erreurs coûtent :
/// compenser ce qui n'a pas eu lieu peut casser un état sain, ne pas compenser ce qui a eu lieu
/// laisse une réservation vivante que plus personne ne rendra.
///
/// Le moteur n'a aucun moyen de trancher, donc il ne tranche pas. Il nomme. C'est la même décision
/// qu'en W2.18 pour `UNKNOWN` et qu'en W3.d pour `WorkflowState::Unknown` : un inconnu qui aurait
/// l'air d'un résultat ne se remarque jamais.
///
/// W4 — l'Execution Fabric, qui tient les leases — saura poser la question au bon endroit. En
/// attendant, la liste sort de [`plan`] à part, et [`crate::DeterministicBackend::compensate`] la
/// rend à l'appelant plutôt que de la ranger sous les compensations faites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UncertainStep {
    /// L'indice du pas.
    pub index: usize,
    /// L'activity abordée dont le résultat manque.
    pub activity: String,
    /// Celle qui la défairait, si l'on décidait qu'elle a eu lieu.
    pub by: String,
}

/// Ce qu'il y a à défaire, et ce dont on ne sait pas s'il y a quelque chose à défaire.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompensationPlan {
    /// Ce qui a eu lieu et se défait, dans l'ordre inverse.
    pub steps: Vec<CompensationStep>,
    /// Ce qui a été abordé sans qu'on sache si l'effet a eu lieu.
    pub uncertain: Vec<UncertainStep>,
}

/// Ce qu'il y a à défaire, dans l'ordre inverse de ce qui a eu lieu.
///
/// # Le plan se lit dans l'historique, pas dans la définition
///
/// On ne compense que ce qui a **réellement eu lieu**. Une définition dit ce qui était prévu ;
/// l'historique dit ce qui s'est passé, et les deux diffèrent exactement quand la compensation
/// devient nécessaire — c'est-à-dire au milieu. Un plan tiré de la définition libérerait des
/// réservations jamais prises, et un moteur idempotent le laisserait passer sans rien dire.
///
/// # L'ordre inverse n'est pas une élégance
///
/// La sandbox se ferme avant que ses ressources soient rendues, parce que rendre des ressources
/// qu'un processus vivant occupe encore ne les rend pas. L'ordre d'annulation est l'ordre
/// d'acquisition retourné, et c'est la seule façon de défaire des effets qui se sont appuyés les
/// uns sur les autres.
#[must_use]
pub fn plan(definition: &WorkflowDefinition, history: &[HistoryEvent]) -> CompensationPlan {
    let already: Vec<usize> = history
        .iter()
        .filter_map(|event| match event {
            HistoryEvent::Compensated { index, .. } => Some(*index),
            _ => None,
        })
        .collect();

    let mut steps: Vec<CompensationStep> = history
        .iter()
        .filter_map(|event| match event {
            // Le pas doit être **fini**, pas seulement abordé : compenser une activity dont le
            // résultat n'est pas dans l'historique reviendrait à défaire ce dont on ne sait pas si
            // ça a eu lieu.
            HistoryEvent::ActivityCompleted { index, name, .. } => Some((*index, name.clone())),
            _ => None,
        })
        .filter(|(index, _)| !already.contains(index))
        .filter_map(|(index, name)| {
            let Some(Step::Activity(activity)) = definition.steps().get(index) else {
                return None;
            };
            if activity.name() != name {
                return None;
            }
            activity.compensated_by().map(|by| CompensationStep {
                index,
                activity: name,
                by: by.to_owned(),
            })
        })
        .collect();

    steps.reverse();

    // Un pas abordé dont le résultat n'est jamais revenu : le dernier `StepEntered` sans
    // `ActivityCompleted` correspondant. Il ne peut y en avoir qu'un — un moteur n'aborde pas deux
    // pas de front — mais la boucle ne le suppose pas.
    let completed: Vec<usize> = history
        .iter()
        .filter_map(|event| match event {
            HistoryEvent::ActivityCompleted { index, .. } => Some(*index),
            _ => None,
        })
        .collect();
    let uncertain = history
        .iter()
        .filter_map(|event| match event {
            HistoryEvent::StepEntered { index, name } => Some((*index, name.clone())),
            _ => None,
        })
        .filter(|(index, _)| !completed.contains(index) && !already.contains(index))
        .filter_map(|(index, name)| {
            let Some(Step::Activity(activity)) = definition.steps().get(index) else {
                return None;
            };
            if activity.name() != name {
                return None;
            }
            activity.compensated_by().map(|by| UncertainStep {
                index,
                activity: name,
                by: by.to_owned(),
            })
        })
        .collect();

    CompensationPlan { steps, uncertain }
}
