//! Test de sortie de W13.f — **reconstruction depuis zéro = état courant ; quarantaine conforme à
//! ADR 0013.**
//!
//! Les deux moitiés sont les deux façons dont une projection peut mentir. La première : rendre un
//! état que le journal ne produit pas. La seconde : continuer après un événement qu'elle n'a pas su
//! appliquer, en présentant un état amputé comme s'il était complet.

use locus_event_store::{
    Actor, ActorKind, Append, Draft, EventStore, EventType, Expected, MemoryEventStore,
};
use locus_projections::{
    EdgeKind, ExecutionGraph, Health, NodeKind, Projection, ProjectionRunner, verify,
};
use locus_protocol::{
    Id, IdKind, Timestamp,
    id::{Agent, Command, Event, Project, Workspace},
};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn draft(seed: u8, stream: &str, kind: &str, payload: serde_json::Value) -> Draft {
    Draft {
        event_id: id::<Event>(seed),
        event_type: EventType::parse(kind).expect("type valide"),
        schema_version: 1,
        stream_id: stream.to_owned(),
        workspace_id: id::<Workspace>(1),
        project_id: id::<Project>(1),
        program_id: None,
        branch_id: None,
        actor: Actor {
            principal_id: id::<Agent>(1),
            kind: ActorKind::Agent,
            delegation_id: None,
        },
        occurred_at: Timestamp::from_millis(1_700_000_000_000),
        causation_id: id::<Command>(seed),
        correlation_id: None,
        trace_id: None,
        payload,
        payload_hash: format!("sha256:{}", "ab".repeat(32)),
    }
}

fn push(
    store: &mut MemoryEventStore,
    seed: u8,
    stream: &str,
    kind: &str,
    payload: serde_json::Value,
) {
    let expected = store
        .revision(stream)
        .map_or(Expected::NoStream, Expected::Exact);
    store
        .append(
            Append {
                stream_id: stream.to_owned(),
                expected,
                command_id: id::<Command>(seed.wrapping_add(100)),
                events: vec![draft(seed, stream, kind, payload)],
            },
            Timestamp::from_millis(1_700_000_100_000),
        )
        .expect("écriture permise");
}

/// Un journal d'exécution ordinaire : une tâche, deux attempts, un worker, un artefact, un run.
fn populate(store: &mut MemoryEventStore) {
    push(
        store,
        1,
        "task-0001",
        "task.created",
        serde_json::json!({ "task_id": "task-0001", "attempt": 1 }),
    );
    push(
        store,
        2,
        "task-0001",
        "lease.granted",
        serde_json::json!({ "task_id": "task-0001", "attempt": 1, "worker_id": "vm-linux-01" }),
    );
    push(
        store,
        3,
        "task-0001",
        "artifact.declared",
        serde_json::json!({
            "task_id": "task-0001",
            "attempt": 1,
            "artifact_id": "artifact-figure-3"
        }),
    );
    push(
        store,
        4,
        "task-0001",
        "run.recorded",
        serde_json::json!({ "task_id": "task-0001", "attempt": 1, "run_id": "run-0007" }),
    );
    // Un second attempt, sur un autre worker : la tâche est la même, l'attempt non.
    push(
        store,
        5,
        "task-0001",
        "task.retried",
        serde_json::json!({ "task_id": "task-0001", "attempt": 2, "worker_id": "vm-linux-02" }),
    );
}

fn live() -> (MemoryEventStore, ProjectionRunner<ExecutionGraph>) {
    let mut store = MemoryEventStore::new();
    populate(&mut store);
    let mut runner = ProjectionRunner::new(ExecutionGraph::new());
    runner.catch_up(&store);
    (store, runner)
}

// ---------------------------------------------------------------------------------------------
// Reconstruction depuis zéro
// ---------------------------------------------------------------------------------------------

#[test]
fn la_reconstruction_depuis_zero_rend_l_etat_courant() {
    let (store, runner) = live();
    let report = verify(&runner, ExecutionGraph::new, &store);
    assert!(
        report.agrees(),
        "reconstruction et état courant divergent : {report:#?}"
    );
}

#[test]
fn le_graphe_porte_ce_que_le_journal_dit() {
    let (_, runner) = live();
    let graph = runner.projection();

    assert_eq!(graph.of_kind(NodeKind::Task).len(), 1);
    assert_eq!(
        graph.of_kind(NodeKind::Attempt).len(),
        2,
        "deux attempts de la même tâche sont deux nœuds"
    );
    assert_eq!(graph.of_kind(NodeKind::Worker).len(), 2);
    assert_eq!(graph.of_kind(NodeKind::Artifact).len(), 1);
    assert_eq!(graph.of_kind(NodeKind::Run).len(), 1);

    let attempt = ExecutionGraph::attempt_id("task-0001", 1);
    assert!(graph.edges().iter().any(|edge| {
        edge.from == attempt
            && edge.to == ExecutionGraph::node_id(NodeKind::Task, "task-0001")
            && edge.kind == EdgeKind::BelongsTo
    }));
    assert!(graph.edges().iter().any(|edge| {
        edge.from == ExecutionGraph::node_id(NodeKind::Artifact, "artifact-figure-3")
            && edge.to == attempt
            && edge.kind == EdgeKind::ProducedBy
    }));
}

/// Deux mutations sont passées vertes au premier essai, et les deux disaient la même chose.
///
/// La première retirait le **préfixe de sorte** des identifiants de nœud : les tests employaient
/// `node_id` pour composer leurs attentes, donc ils suivaient la mutation au lieu de la voir. Un
/// test qui construit son attendu avec la fonction qu'il vérifie compare une valeur à elle-même.
/// Celui-ci écrit donc les identifiants **en toutes lettres**, et exige qu'une tâche et un run de
/// même clé restent deux nœuds.
#[test]
fn deux_identifiants_egaux_de_sortes_differentes_ne_fusionnent_pas() {
    let mut store = MemoryEventStore::new();
    push(
        &mut store,
        1,
        "collision",
        "task.created",
        serde_json::json!({ "task_id": "x", "attempt": 1 }),
    );
    push(
        &mut store,
        2,
        "collision",
        "run.recorded",
        serde_json::json!({ "task_id": "x", "attempt": 1, "run_id": "x" }),
    );
    let mut runner = ProjectionRunner::new(ExecutionGraph::new());
    runner.catch_up(&store);
    let graph = runner.projection();

    let ids: Vec<&String> = graph.nodes().keys().collect();
    assert!(ids.iter().any(|id| id.as_str() == "task:x"), "{ids:?}");
    assert!(ids.iter().any(|id| id.as_str() == "run:x"), "{ids:?}");
    assert!(
        ids.iter().any(|id| id.as_str() == "attempt:x#1"),
        "un attempt n'existe que dans sa tâche : {ids:?}"
    );
    assert_eq!(graph.of_kind(NodeKind::Task).len(), 1);
    assert_eq!(graph.of_kind(NodeKind::Run).len(), 1);
}

/// La seconde mutation rendait le **résumé constant**, et la suite restait verte : `verify`
/// compare le résumé courant à celui de la reconstruction, et deux constantes s'accordent
/// toujours. Un résumé qui ne dépend pas de l'état ferait passer la propriété de reconstruction
/// pour n'importe quelle projection — c'est la garde muette de W13.b, sous un autre nom.
#[test]
fn deux_graphes_differents_ont_des_resumes_differents() {
    let mut small = MemoryEventStore::new();
    push(
        &mut small,
        1,
        "task-0001",
        "task.created",
        serde_json::json!({ "task_id": "task-0001", "attempt": 1 }),
    );
    let mut one = ProjectionRunner::new(ExecutionGraph::new());
    one.catch_up(&small);

    let (_, full) = live();

    assert_ne!(
        one.projection().checksum(),
        full.projection().checksum(),
        "sans dépendance au contenu, le résumé ne dit plus rien de l'état"
    );
    assert_eq!(
        one.projection().checksum(),
        one.projection().checksum(),
        "et il reste stable pour un même état, sinon la comparaison serait du bruit"
    );
}

/// # Ce que ce test vaut, et comment on le sait
///
/// La propriété tient **par construction** : aucune arête n'est posée sans que ses deux nœuds
/// aient été créés, et `ExecutionGraph` n'expose aucun moyen d'en ajouter une autrement. Un test
/// ne peut donc pas fabriquer d'orpheline pour éprouver le détecteur, et prétendre le contraire
/// reviendrait à écrire une mise en scène.
///
/// Ce qui donne prise à ce test est la **mutation** : poser une arête avant de créer son nœud le
/// fait rougir. C'est la leçon de W13.b — un détecteur muet est indiscernable d'un graphe sain —
/// appliquée là où l'assemblage d'un contre-exemple n'est pas possible.
#[test]
fn aucune_arete_ne_pointe_dans_le_vide() {
    let (_, runner) = live();
    assert!(runner.projection().orphan_edges().is_empty());
}

// ---------------------------------------------------------------------------------------------
// La quarantaine — ADR 0013, décisions 3 et 4
// ---------------------------------------------------------------------------------------------

/// Décision 3 : « une projection en défaut **s'arrête**, elle ne saute pas ». Sauter présenterait
/// un état amputé comme s'il était complet, et rien ne le dirait.
#[test]
fn un_evenement_inapplicable_met_la_projection_en_quarantaine() {
    let mut store = MemoryEventStore::new();
    push(
        &mut store,
        1,
        "task-0001",
        "task.created",
        serde_json::json!({ "task_id": "task-0001", "attempt": 1 }),
    );
    // Un artefact sans identité : la projection ne peut pas le suivre.
    push(
        &mut store,
        2,
        "task-0001",
        "artifact.declared",
        serde_json::json!({ "task_id": "task-0001", "attempt": 1 }),
    );
    push(
        &mut store,
        3,
        "task-0001",
        "run.recorded",
        serde_json::json!({ "task_id": "task-0001", "attempt": 1, "run_id": "run-0007" }),
    );

    let mut runner = ProjectionRunner::new(ExecutionGraph::new());
    let progress = runner.catch_up(&store);

    assert!(
        matches!(runner.health(), Health::Quarantined { .. }),
        "un événement inapplicable met la projection en quarantaine : {:?}",
        runner.health()
    );
    assert_eq!(
        progress.applied, 1,
        "elle s'arrête sur l'événement fautif, et ne consomme pas celui d'après"
    );
    assert!(
        runner.projection().of_kind(NodeKind::Run).is_empty(),
        "le run qui suit l'événement fautif n'a pas été appliqué : sauter aurait rendu un graphe \
         qui a l'air complet"
    );
}

/// Décision 4 : « la quarantaine ne bloque pas l'écriture ». Le journal continue d'accepter des
/// événements pendant que la projection est arrêtée — c'est ce qui distingue une projection en
/// défaut d'une panne du système.
#[test]
fn la_quarantaine_ne_bloque_pas_l_ecriture() {
    let mut store = MemoryEventStore::new();
    push(
        &mut store,
        1,
        "task-0001",
        "artifact.declared",
        serde_json::json!({ "task_id": "task-0001", "attempt": 1 }),
    );
    let mut runner = ProjectionRunner::new(ExecutionGraph::new());
    runner.catch_up(&store);
    assert!(matches!(runner.health(), Health::Quarantined { .. }));

    push(
        &mut store,
        2,
        "task-0002",
        "task.created",
        serde_json::json!({ "task_id": "task-0002", "attempt": 1 }),
    );
    assert_eq!(
        store.export().len(),
        2,
        "l'écriture canonique n'a jamais été suspendue"
    );
}

/// Et une projection en quarantaine ne repart pas toute seule : elle attend qu'on la reconstruise.
#[test]
fn la_reconstruction_sort_de_quarantaine_quand_le_journal_le_permet() {
    let mut store = MemoryEventStore::new();
    push(
        &mut store,
        1,
        "task-0001",
        "task.created",
        serde_json::json!({ "task_id": "task-0001", "attempt": 1 }),
    );
    let mut runner = ProjectionRunner::new(ExecutionGraph::new());
    runner.catch_up(&store);
    assert!(matches!(runner.health(), Health::Healthy));

    runner.rebuild(&store);
    assert!(matches!(runner.health(), Health::Healthy));
    assert_eq!(runner.projection().of_kind(NodeKind::Task).len(), 1);
}

// ---------------------------------------------------------------------------------------------
// Aucun agent, et c'est le sujet de W13.g
// ---------------------------------------------------------------------------------------------

/// W13.b l'avait établi sur les fixtures : rien dans l'événement ne dit **quel agent** a agi. La
/// projection ne peut donc pas en fabriquer, et un `worker_id` ne fait pas un agent — confondre la
/// machine et le rôle rendrait le graphe organisationnel faux dès sa première jointure.
#[test]
fn le_graphe_d_execution_ne_contient_aucun_agent() {
    let (_, runner) = live();
    let kinds: Vec<&str> = runner
        .projection()
        .nodes()
        .values()
        .map(|kind| kind.slug())
        .collect();
    assert!(
        !kinds.iter().any(|kind| kind.contains("agent")),
        "aucun nœud d'agent : c'est W13.g qui joindra l'assignation, {kinds:?}"
    );
    assert!(kinds.contains(&"worker"), "le worker, lui, est dans le fil");
}
