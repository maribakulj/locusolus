//! La projection « graphe d'exécution » — `docs/SPEC_V1.md` §9.3, W13.f.
//!
//! # Ce que W13.b avait établi, et que celle-ci consomme
//!
//! Le pli de `tests/graph/fold.ts` a répondu à la question posée avant d'écrire cette projection :
//! le graphe d'exécution est **dérivable de `lep/1.0` tel quel**. Aucun champ n'a été ajouté au
//! protocole, et cette projection n'en demande pas non plus. Elle lit le journal, pas des documents
//! LEP — mais les faits sont les mêmes, portés par `payload`.
//!
//! # Ce qu'elle ne contient pas, et ne peut pas contenir
//!
//! Aucun nœud d'agent. Le pli l'avait déjà montré : rien dans l'événement ne dit **quel agent** a
//! agi. C'est W13.g qui joindra `assigned_agent_id` — un fait porté par les événements
//! d'assignation de W13.d — au graphe d'exécution pour obtenir le graphe organisationnel réalisé.
//! Fabriquer ici un nœud d'agent depuis un `worker_id` confondrait la machine et le rôle.

use std::collections::{BTreeMap, BTreeSet};

use locus_event_store::Envelope;

use crate::projection::{Projection, ProjectionError, Watermark};

/// Ce qu'un nœud du graphe d'exécution peut être.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeKind {
    /// Une tâche.
    Task,
    /// Une tentative d'exécution.
    Attempt,
    /// Un worker.
    Worker,
    /// Un artefact.
    Artifact,
    /// Un run consigné.
    Run,
}

impl NodeKind {
    /// Son nom, qui sert aussi de préfixe d'identifiant.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Task => "task",
            Self::Attempt => "attempt",
            Self::Worker => "worker",
            Self::Artifact => "artifact",
            Self::Run => "run",
        }
    }
}

/// Les arêtes, orientées de la partie vers ce dont elle dépend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EdgeKind {
    /// Un attempt appartient à une tâche.
    BelongsTo,
    /// Un attempt a été exécuté par un worker.
    ExecutedBy,
    /// Un artefact a été produit par un attempt.
    ProducedBy,
    /// Un run consigne un attempt.
    RecordedFor,
}

impl EdgeKind {
    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::BelongsTo => "belongs_to",
            Self::ExecutedBy => "executed_by",
            Self::ProducedBy => "produced_by",
            Self::RecordedFor => "recorded_for",
        }
    }
}

/// Une arête du graphe.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Edge {
    /// D'où.
    pub from: String,
    /// Vers où.
    pub to: String,
    /// De quelle sorte.
    pub kind: EdgeKind,
}

/// Le graphe d'exécution, reconstruit depuis le journal.
///
/// # Les nœuds sont préfixés par leur sorte
///
/// Un `task_id` et un `run_id` peuvent porter la même chaîne sans désigner la même chose. Sans
/// préfixe, deux nœuds distincts fusionneraient en silence et le graphe aurait l'air plus connexe
/// qu'il ne l'est — le pli de W13.b avait déjà pris cette décision, pour la même raison.
///
/// # Une arête est un fait, pas une occurrence
///
/// « Cet attempt appartient à cette tâche » est écrit par chaque événement de l'attempt. Les
/// empiler ferait d'un graphe de dépendances un histogramme de mentions, et le premier calcul de
/// degré s'en trouverait faux. Le `BTreeSet` le dit une fois.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ExecutionGraph {
    nodes: BTreeMap<String, NodeKind>,
    edges: BTreeSet<Edge>,
    watermark: Watermark,
}

impl ExecutionGraph {
    /// Un graphe vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// L'identifiant d'un nœud, préfixé par sa sorte.
    #[must_use]
    pub fn node_id(kind: NodeKind, key: &str) -> String {
        format!("{}:{key}", kind.slug())
    }

    /// L'identifiant d'un attempt, qui n'existe que dans sa tâche.
    #[must_use]
    pub fn attempt_id(task_id: &str, attempt: u64) -> String {
        Self::node_id(NodeKind::Attempt, &format!("{task_id}#{attempt}"))
    }

    /// Tous les nœuds, avec leur sorte.
    #[must_use]
    pub const fn nodes(&self) -> &BTreeMap<String, NodeKind> {
        &self.nodes
    }

    /// Toutes les arêtes.
    #[must_use]
    pub const fn edges(&self) -> &BTreeSet<Edge> {
        &self.edges
    }

    /// Les nœuds d'une sorte donnée.
    #[must_use]
    pub fn of_kind(&self, kind: NodeKind) -> Vec<&String> {
        self.nodes
            .iter()
            .filter(|(_, node)| **node == kind)
            .map(|(id, _)| id)
            .collect()
    }

    /// Les arêtes dont une extrémité ne désigne aucun nœud.
    ///
    /// **Vide, toujours.** C'est la garantie de la projection, et elle tient par construction :
    /// aucune arête n'est posée sans que ses deux nœuds aient été créés. Le test l'exige quand
    /// même — une garantie par construction cesse d'en être une à la première ligne ajoutée, et
    /// une arête qui pointe dans le vide se parcourt sans erreur en mentant à chaque parcours.
    #[must_use]
    pub fn orphan_edges(&self) -> Vec<&Edge> {
        self.edges
            .iter()
            .filter(|edge| {
                !self.nodes.contains_key(&edge.from) || !self.nodes.contains_key(&edge.to)
            })
            .collect()
    }

    fn put(&mut self, kind: NodeKind, key: &str) -> String {
        let id = Self::node_id(kind, key);
        self.nodes.insert(id.clone(), kind);
        id
    }

    fn link(&mut self, from: String, to: String, kind: EdgeKind) {
        self.edges.insert(Edge { from, to, kind });
    }

    /// L'attempt nommé par un événement, quand il en nomme un — avec sa tâche.
    fn attempt_of(
        &mut self,
        payload: &serde_json::Map<String, serde_json::Value>,
    ) -> Option<String> {
        let task_id = text(payload, "task_id")?;
        let attempt = payload.get("attempt")?.as_u64()?;
        let task = self.put(NodeKind::Task, task_id);
        let node = self.put(NodeKind::Attempt, &format!("{task_id}#{attempt}"));
        self.link(node.clone(), task, EdgeKind::BelongsTo);
        Some(node)
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

impl Projection for ExecutionGraph {
    fn name(&self) -> &'static str {
        "execution_graph"
    }

    fn apply(&mut self, position: u64, event: &Envelope) -> Result<(), ProjectionError> {
        self.watermark = position;
        let namespace = event.event_type.namespace();
        if !matches!(namespace, "task" | "lease" | "artifact" | "run") {
            return Ok(());
        }
        let payload = event.payload.as_object().ok_or_else(|| ProjectionError {
            position,
            reason: format!("charge « {namespace} » non objet"),
        })?;

        let attempt = self.attempt_of(payload);

        if let Some(worker_id) = text(payload, "worker_id") {
            let worker = self.put(NodeKind::Worker, worker_id);
            if let Some(attempt) = attempt.clone() {
                self.link(attempt, worker, EdgeKind::ExecutedBy);
            }
        }

        if namespace == "artifact" {
            let artifact_id = text(payload, "artifact_id").ok_or_else(|| ProjectionError {
                position,
                reason: "`artifact_id` absent : un artefact sans identité n'est pas suivable"
                    .to_owned(),
            })?;
            let node = self.put(NodeKind::Artifact, artifact_id);
            if let Some(attempt) = attempt.clone() {
                self.link(node, attempt, EdgeKind::ProducedBy);
            }
        }

        if namespace == "run" {
            let run_id = text(payload, "run_id").ok_or_else(|| ProjectionError {
                position,
                reason: "`run_id` absent : un run sans identité ne se rejoue pas".to_owned(),
            })?;
            let node = self.put(NodeKind::Run, run_id);
            if let Some(attempt) = attempt {
                self.link(node, attempt, EdgeKind::RecordedFor);
            }
        }

        Ok(())
    }

    fn watermark(&self) -> Watermark {
        self.watermark
    }

    fn reset(&mut self) {
        self.nodes.clear();
        self.edges.clear();
        self.watermark = 0;
    }

    fn checksum(&self) -> String {
        // Les nœuds et les arêtes sont dans des collections **ordonnées**, donc le résumé ne
        // dépend pas de l'ordre d'insertion. Sans cela, une reconstruction depuis le journal
        // rendrait un résumé différent de l'état courant alors que les deux portent le même
        // graphe, et le test de W1.d échouerait sur un artefact de `HashMap`.
        let nodes: Vec<String> = self
            .nodes
            .iter()
            .map(|(id, kind)| format!("{id}={}", kind.slug()))
            .collect();
        let edges: Vec<String> = self
            .edges
            .iter()
            .map(|edge| format!("{}-{}->{}", edge.from, edge.kind.slug(), edge.to))
            .collect();
        format!(
            "nodes={};edges={};n={};e={}",
            nodes.join(","),
            edges.join(","),
            self.nodes.len(),
            self.edges.len()
        )
    }
}
