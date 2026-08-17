//! La projection « graphe organisationnel réalisé » — W13.g, `docs/SPEC_V1.md` §9.3, §7.1.
//!
//! # Réalisé, et non prévu
//!
//! Un organigramme dit qui **devrait** faire quoi. Ce graphe-ci dit qui l'a **fait** : il se
//! reconstruit d'une suite d'assignations consignées, pas d'une déclaration d'intention. Les deux
//! divergent dès qu'un lease se perd et qu'une tâche change de main, et c'est précisément l'écart
//! qu'on veut pouvoir lire.
//!
//! # D'où vient l'information, et d'où elle ne vient pas
//!
//! W13.b l'avait établi : rien dans l'événement `lep/1.0` ne dit **quel agent** a agi. W13.d y a
//! répondu en faisant de l'assignation un **événement** plutôt qu'une transition d'état. Cette
//! projection joint donc `assigned_agent_id` au graphe d'exécution — et c'est la seule source.
//!
//! **Aucun instantané n'est reçu du worker.** Invariant 3 : « un worker ne modifie jamais
//! directement la base canonique ». Un événement dont l'acteur est un agent et qui prétendrait
//! annoncer une assignation décrirait sa propre affectation ; l'assignation est une décision du
//! plan de contrôle, et elle n'est retenue que quand l'acteur en est le système. Un graphe
//! organisationnel qui croirait les workers sur parole serait un graphe que les workers écrivent.

use std::collections::BTreeSet;

use locus_event_store::{ActorKind, Envelope};

use crate::projection::{Projection, ProjectionError, Watermark};

/// Une assignation lue dans le journal.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AssignmentRecord {
    /// La position dans le flux global — l'ordre des faits.
    pub position: u64,
    /// La tâche confiée.
    pub task_id: String,
    /// À qui.
    pub agent_id: String,
    /// Sur quelle machine.
    pub worker_id: String,
}

/// Le graphe organisationnel réalisé.
///
/// # L'histoire, pas l'état courant
///
/// Une tâche qui a changé de main trois fois porte trois assignations, et la dernière n'efface pas
/// les deux premières (invariant 12). `current_agent` répond à « qui la fait », `assignments`
/// répond à « qui l'a faite » — et c'est la seconde question qu'un graphe **réalisé** doit savoir
/// trancher.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OrganisationGraph {
    assignments: Vec<AssignmentRecord>,
    agents: BTreeSet<String>,
    workers: BTreeSet<String>,
    tasks: BTreeSet<String>,
    watermark: Watermark,
}

impl OrganisationGraph {
    /// Un graphe vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Toutes les assignations, dans l'ordre du journal.
    #[must_use]
    pub fn assignments(&self) -> &[AssignmentRecord] {
        &self.assignments
    }

    /// Les agents qui ont réellement travaillé.
    #[must_use]
    pub const fn agents(&self) -> &BTreeSet<String> {
        &self.agents
    }

    /// Les workers qui ont réellement exécuté.
    #[must_use]
    pub const fn workers(&self) -> &BTreeSet<String> {
        &self.workers
    }

    /// Les tâches qui ont été confiées à quelqu'un.
    #[must_use]
    pub const fn tasks(&self) -> &BTreeSet<String> {
        &self.tasks
    }

    /// L'agent qui a la tâche en dernier, s'il y en a un.
    #[must_use]
    pub fn current_agent(&self, task_id: &str) -> Option<&str> {
        self.assignments
            .iter()
            .rev()
            .find(|record| record.task_id == task_id)
            .map(|record| record.agent_id.as_str())
    }

    /// Toutes les tâches qu'un agent a portées, même celles qu'il n'a plus.
    #[must_use]
    pub fn tasks_of(&self, agent_id: &str) -> BTreeSet<&str> {
        self.assignments
            .iter()
            .filter(|record| record.agent_id == agent_id)
            .map(|record| record.task_id.as_str())
            .collect()
    }

    /// Les workers sur lesquels un agent a tourné.
    ///
    /// Deux identités distinctes (W13.d) : un agent peut avoir tourné sur plusieurs machines, et
    /// une machine avoir porté plusieurs agents. Confondre les deux rendrait indécidable l'une des
    /// deux questions.
    #[must_use]
    pub fn workers_of(&self, agent_id: &str) -> BTreeSet<&str> {
        self.assignments
            .iter()
            .filter(|record| record.agent_id == agent_id)
            .map(|record| record.worker_id.as_str())
            .collect()
    }
}

fn text<'a>(
    payload: &'a serde_json::Map<String, serde_json::Value>,
    field: &str,
) -> Option<&'a str> {
    payload
        .get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
}

impl Projection for OrganisationGraph {
    fn name(&self) -> &'static str {
        "organisation_graph"
    }

    fn apply(&mut self, position: u64, event: &Envelope) -> Result<(), ProjectionError> {
        self.watermark = position;
        if event.event_type.namespace() != "task" || event.event_type.verb() != "assigned" {
            return Ok(());
        }

        // Invariant 3, appliqué à la lecture. Le plan de contrôle décide des assignations ; un
        // agent qui en annoncerait une décrirait sa propre affectation, et un graphe qui le
        // croirait serait un graphe que les workers écrivent. L'événement n'est pas une erreur —
        // il est journalisé, et c'est bien ainsi — il n'est simplement pas une source.
        if event.actor.kind != ActorKind::System {
            return Ok(());
        }

        let payload = event.payload.as_object().ok_or_else(|| ProjectionError {
            position,
            reason: "charge d'assignation non objet".to_owned(),
        })?;
        let task_id = text(payload, "task_id").ok_or_else(|| ProjectionError {
            position,
            reason: "`task_id` absent : une assignation sans tâche ne confie rien".to_owned(),
        })?;
        let agent_id = text(payload, "agent_id").ok_or_else(|| ProjectionError {
            position,
            reason: "`agent_id` absent : c'est l'information que cette projection existe pour \
                     joindre"
                .to_owned(),
        })?;
        let worker_id = text(payload, "worker_id").ok_or_else(|| ProjectionError {
            position,
            reason: "`worker_id` absent : un agent tourne quelque part".to_owned(),
        })?;

        self.tasks.insert(task_id.to_owned());
        self.agents.insert(agent_id.to_owned());
        self.workers.insert(worker_id.to_owned());
        self.assignments.push(AssignmentRecord {
            position,
            task_id: task_id.to_owned(),
            agent_id: agent_id.to_owned(),
            worker_id: worker_id.to_owned(),
        });
        Ok(())
    }

    fn watermark(&self) -> Watermark {
        self.watermark
    }

    fn reset(&mut self) {
        self.assignments.clear();
        self.agents.clear();
        self.workers.clear();
        self.tasks.clear();
        self.watermark = 0;
    }

    fn checksum(&self) -> String {
        // L'ordre des assignations fait partie de l'état : « A puis B » n'est pas « B puis A »,
        // et un résumé qui les confondrait rendrait la reconstruction incapable de détecter une
        // inversion.
        let assignments: Vec<String> = self
            .assignments
            .iter()
            .map(|record| {
                format!(
                    "{}:{}→{}@{}",
                    record.position, record.task_id, record.agent_id, record.worker_id
                )
            })
            .collect();
        format!(
            "assignments={};agents={};workers={}",
            assignments.join(","),
            self.agents.len(),
            self.workers.len()
        )
    }
}
