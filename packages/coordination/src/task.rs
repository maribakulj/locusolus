//! L'agrégat `Task` de §7.1, et l'assignation — `docs/SPEC_V1.md` §7.1, ADR 0016 décision 13.
//!
//! # Ce que ce module ajoute, et ce qu'il ne touche pas
//!
//! `locus_domain::TaskState` porte la machine à états de §7.1 depuis W1. Ce module **ne la modifie
//! pas** : il l'emploie. Les quinze états et leur table de transitions restent où ils sont, avec
//! leurs tests, et ce module n'a aucun moyen de les contourner — `moved_to` délègue à
//! `locus_domain::transition`.
//!
//! # L'assignation est un événement, pas une transition
//!
//! C'est la décision du sprint, et elle mérite d'être dite en clair.
//!
//! Un état dit **où en est** le travail ; une assignation dit **qui le fait**. Les deux changent
//! indépendamment : une tâche `running` peut être réassignée après la perte d'un lease sans jamais
//! quitter `running`, et une tâche peut passer de `leased` à `running` sans changer d'exécutant.
//! Faire de l'assignation une transition obligerait à croiser quinze états avec autant d'agents,
//! et le premier changement d'agent en cours d'exécution rendrait la table fausse.
//!
//! Conséquence directe pour W13.g : le graphe organisationnel **réalisé** se dérive d'une suite
//! d'assignations, pas d'un champ courant. Une tâche qui a changé de main trois fois a trois faits
//! à consigner, et le dernier n'efface pas les deux premiers (invariant 12).

use std::fmt;

use locus_domain::{ForbiddenTransition, TaskState, transition};
use locus_protocol::{
    Id, Timestamp,
    id::{Agent, Branch, provisional::Task as TaskKind},
};

/// À qui une tâche a été confiée, et quand.
///
/// **Les deux identités, pas une.** Un worker est une machine, un agent est un rôle situé : deux
/// agents peuvent tourner sur le même worker, et un agent peut être réassigné d'un worker à un
/// autre. §7.1 porte les deux champs, et n'en garder qu'un rendrait indécidable l'une des deux
/// questions que W13.g doit trancher — « qui a fait ce travail » et « où a-t-il tourné ».
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
    agent_id: Id<Agent>,
    worker_id: String,
    at: Timestamp,
}

impl Assignment {
    /// Consigner une assignation.
    ///
    /// # Errors
    ///
    /// [`TaskError::EmptyWorker`] : un worker sans identité ne se retrouve pas dans un journal.
    pub fn new(agent_id: Id<Agent>, worker_id: &str, at: Timestamp) -> Result<Self, TaskError> {
        if worker_id.trim().is_empty() {
            return Err(TaskError::EmptyWorker);
        }
        Ok(Self {
            agent_id,
            worker_id: worker_id.to_owned(),
            at,
        })
    }

    /// L'agent à qui la tâche a été confiée.
    #[must_use]
    pub const fn agent_id(&self) -> Id<Agent> {
        self.agent_id
    }

    /// Le worker où il tourne.
    #[must_use]
    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    /// Quand.
    #[must_use]
    pub const fn at(&self) -> Timestamp {
        self.at
    }
}

/// Une tâche — §7.1, complétée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    id: Id<TaskKind>,
    branch_id: Id<Branch>,
    kind: String,
    objective: String,
    state: TaskState,
    attempt: u32,
    idempotency_key: String,
    assignments: Vec<Assignment>,
    revision: u64,
}

impl Task {
    /// Proposer une tâche.
    ///
    /// Elle naît `proposed` — le premier état de §7.1 — et sans assignation : proposer n'est pas
    /// confier.
    ///
    /// # Errors
    ///
    /// [`TaskError::EmptyField`] pour un genre, un objectif ou une clé d'idempotence vides. La clé
    /// est exigée dès la proposition : c'est elle qui empêche qu'une reprise après incident crée
    /// une seconde tâche pour le même travail, et une clé attribuée plus tard arriverait après le
    /// doublon.
    pub fn propose(
        id: Id<TaskKind>,
        branch_id: Id<Branch>,
        kind: &str,
        objective: &str,
        idempotency_key: &str,
    ) -> Result<Self, TaskError> {
        for (field, value) in [
            ("kind", kind),
            ("objective", objective),
            ("idempotency_key", idempotency_key),
        ] {
            if value.trim().is_empty() {
                return Err(TaskError::EmptyField { field });
            }
        }
        Ok(Self {
            id,
            branch_id,
            kind: kind.to_owned(),
            objective: objective.to_owned(),
            state: TaskState::Proposed,
            attempt: 0,
            idempotency_key: idempotency_key.to_owned(),
            assignments: Vec::new(),
            revision: 1,
        })
    }

    /// Franchir une transition d'état.
    ///
    /// Délègue à `locus_domain::transition` : ce module n'a pas de table à lui, donc pas de moyen
    /// de diverger de celle de §7.1.
    ///
    /// # Errors
    ///
    /// [`TaskError::Forbidden`] quand la table du domaine la refuse.
    pub fn moved_to(mut self, next: TaskState) -> Result<Self, TaskError> {
        self.state = transition(self.state, next)?;
        self.revision += 1;
        Ok(self)
    }

    /// Confier la tâche à un agent, sur un worker.
    ///
    /// **N'est pas une transition.** L'état ne bouge pas : une tâche `running` réassignée reste
    /// `running`. Ce qui change est la liste des assignations, à laquelle une entrée s'ajoute.
    ///
    /// # Errors
    ///
    /// [`TaskError::TerminalState`] quand la tâche est finie : confier un travail achevé
    /// n'assigne rien, et laisserait croire dans un journal que quelqu'un s'y est mis.
    pub fn assigned(mut self, assignment: Assignment) -> Result<Self, TaskError> {
        if self.state.is_terminal() {
            return Err(TaskError::TerminalState { state: self.state });
        }
        self.assignments.push(assignment);
        self.revision += 1;
        Ok(self)
    }

    /// Ouvrir un nouvel attempt.
    ///
    /// Le compteur ne redescend jamais : `orphaned → queued` fait repartir la tâche « sur un autre
    /// attempt », et réutiliser le numéro rendrait deux exécutions indiscernables dans le journal.
    #[must_use]
    pub const fn next_attempt(mut self) -> Self {
        self.attempt += 1;
        self.revision += 1;
        self
    }

    /// Son identifiant.
    #[must_use]
    pub const fn id(&self) -> Id<TaskKind> {
        self.id
    }

    /// La branche où elle vit.
    #[must_use]
    pub const fn branch_id(&self) -> Id<Branch> {
        self.branch_id
    }

    /// Son genre.
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Son objectif.
    #[must_use]
    pub fn objective(&self) -> &str {
        &self.objective
    }

    /// Son état, tel que le domaine le définit.
    #[must_use]
    pub const fn state(&self) -> TaskState {
        self.state
    }

    /// Son numéro d'attempt.
    #[must_use]
    pub const fn attempt(&self) -> u32 {
        self.attempt
    }

    /// Sa clé d'idempotence.
    #[must_use]
    pub fn idempotency_key(&self) -> &str {
        &self.idempotency_key
    }

    /// Toutes les assignations, dans l'ordre.
    ///
    /// L'histoire, pas l'état courant : une tâche qui a changé de main trois fois a trois faits à
    /// consigner, et le dernier n'efface pas les deux premiers.
    #[must_use]
    pub fn assignments(&self) -> &[Assignment] {
        &self.assignments
    }

    /// L'assignation en vigueur, s'il y en a une.
    #[must_use]
    pub fn current_assignment(&self) -> Option<&Assignment> {
        self.assignments.last()
    }

    /// L'agent à qui elle est confiée — `assigned_agent_id` de §7.1.
    #[must_use]
    pub fn assigned_agent_id(&self) -> Option<Id<Agent>> {
        self.current_assignment().map(Assignment::agent_id)
    }

    /// Le worker où elle tourne — `assigned_worker_id` de §7.1.
    #[must_use]
    pub fn assigned_worker_id(&self) -> Option<&str> {
        self.current_assignment().map(Assignment::worker_id)
    }

    /// Sa révision.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }
}

/// Ce qui empêche une tâche d'exister ou d'avancer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskError {
    /// Un champ obligatoire vide.
    EmptyField {
        /// Lequel.
        field: &'static str,
    },
    /// Un worker sans identité.
    EmptyWorker,
    /// Une transition que la table du domaine refuse.
    Forbidden(ForbiddenTransition),
    /// Une tâche déjà finie.
    TerminalState {
        /// Son état.
        state: TaskState,
    },
}

impl From<ForbiddenTransition> for TaskError {
    fn from(refused: ForbiddenTransition) -> Self {
        Self::Forbidden(refused)
    }
}

impl fmt::Display for TaskError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(formatter, "le champ « {field} » est vide"),
            Self::EmptyWorker => {
                formatter.write_str("un worker sans identité ne se retrouve pas dans un journal")
            }
            Self::Forbidden(refused) => write!(formatter, "{refused}"),
            Self::TerminalState { state } => write!(
                formatter,
                "une tâche « {} » ne se confie plus à personne",
                state.as_str()
            ),
        }
    }
}

impl std::error::Error for TaskError {}
