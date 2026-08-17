//! Test de sortie de W13.g — **le graphe se reconstruit depuis le journal seul ; aucun instantané
//! n'est reçu du worker.**
//!
//! C'est la jointure que W13.b avait déclarée impossible sur `lep/1.0` tel quel, et que W13.d a
//! rendue possible en faisant de l'assignation un événement. Elle clôt W13.

use locus_event_store::{
    Actor, ActorKind, Append, Draft, EventStore, EventType, Expected, MemoryEventStore,
};
use locus_projections::{OrganisationGraph, Projection, ProjectionRunner, verify};
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

fn push(
    store: &mut MemoryEventStore,
    seed: u8,
    stream: &str,
    kind: &str,
    actor: ActorKind,
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
                events: vec![Draft {
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
                        kind: actor,
                        delegation_id: None,
                    },
                    occurred_at: Timestamp::from_millis(1_700_000_000_000),
                    causation_id: id::<Command>(seed),
                    correlation_id: None,
                    trace_id: None,
                    payload,
                    payload_hash: format!("sha256:{}", "ab".repeat(32)),
                }],
            },
            Timestamp::from_millis(1_700_000_100_000),
        )
        .expect("écriture permise");
}

fn assign(
    store: &mut MemoryEventStore,
    seed: u8,
    task: &str,
    agent: &str,
    worker: &str,
    actor: ActorKind,
) {
    push(
        store,
        seed,
        task,
        "task.assigned",
        actor,
        serde_json::json!({ "task_id": task, "agent_id": agent, "worker_id": worker }),
    );
}

/// Une tâche qui change de main, une autre qui reste, et du bruit qui n'est pas une assignation.
fn populate(store: &mut MemoryEventStore) {
    assign(
        store,
        1,
        "task-0001",
        "agent-alpha",
        "vm-01",
        ActorKind::System,
    );
    push(
        store,
        2,
        "task-0001",
        "task.started",
        ActorKind::Agent,
        serde_json::json!({ "task_id": "task-0001", "attempt": 1 }),
    );
    // Le lease se perd : la tâche repart chez un autre agent, sur une autre machine.
    assign(
        store,
        3,
        "task-0001",
        "agent-beta",
        "vm-02",
        ActorKind::System,
    );
    assign(
        store,
        4,
        "task-0002",
        "agent-alpha",
        "vm-02",
        ActorKind::System,
    );
}

fn live() -> (MemoryEventStore, ProjectionRunner<OrganisationGraph>) {
    let mut store = MemoryEventStore::new();
    populate(&mut store);
    let mut runner = ProjectionRunner::new(OrganisationGraph::new());
    runner.catch_up(&store);
    (store, runner)
}

// ---------------------------------------------------------------------------------------------
// Le graphe se reconstruit depuis le journal seul
// ---------------------------------------------------------------------------------------------

#[test]
fn la_reconstruction_depuis_zero_rend_l_etat_courant() {
    let (store, runner) = live();
    let report = verify(&runner, OrganisationGraph::new, &store);
    assert!(
        report.agrees(),
        "reconstruction et état courant divergent : {report:#?}"
    );
}

/// Reconstruire un runner **déjà peuplé**, ce que `verify` ne fait pas.
///
/// Trouvé par mutation : un `reset` qui garderait les assignations laissait la suite verte, parce
/// que `verify` reconstruit une projection **neuve** — `reset` y est appelé sur un état déjà vide,
/// où oublier de le vider ne se voit pas. La reconstruction en place est pourtant le cas réel :
/// c'est ainsi qu'une projection sort de quarantaine.
#[test]
fn reconstruire_une_projection_peuplee_ne_double_pas_les_faits() {
    let (store, mut runner) = live();
    let before = runner.projection().assignments().len();
    assert_eq!(before, 3);

    runner.rebuild(&store);
    assert_eq!(
        runner.projection().assignments().len(),
        before,
        "reconstruire n'ajoute pas aux faits déjà là : `reset` doit vider"
    );
    assert_eq!(runner.projection().agents().len(), 2);
}

/// La jointure que W13 existait pour rendre possible : **qui** a fait **quoi**, et **où**.
#[test]
fn la_jointure_rend_qui_a_fait_quoi_et_ou() {
    let (_, runner) = live();
    let graph = runner.projection();

    assert_eq!(graph.agents().len(), 2);
    assert_eq!(graph.workers().len(), 2);
    assert_eq!(graph.tasks().len(), 2);

    assert_eq!(
        graph.tasks_of("agent-alpha"),
        ["task-0001", "task-0002"].into_iter().collect(),
        "alpha a porté les deux tâches, même celle qu'il n'a plus"
    );
    assert_eq!(
        graph.workers_of("agent-alpha"),
        ["vm-01", "vm-02"].into_iter().collect(),
        "un agent peut avoir tourné sur plusieurs machines"
    );
}

/// Réalisé, et non prévu. Une tâche qui a changé de main porte **deux** assignations, et la
/// seconde n'efface pas la première — invariant 12, et c'est ce qui distingue ce graphe d'un
/// organigramme.
#[test]
fn une_tache_qui_change_de_main_garde_les_deux_faits() {
    let (_, runner) = live();
    let graph = runner.projection();

    let history: Vec<&str> = graph
        .assignments()
        .iter()
        .filter(|record| record.task_id == "task-0001")
        .map(|record| record.agent_id.as_str())
        .collect();
    assert_eq!(history, vec!["agent-alpha", "agent-beta"]);

    assert_eq!(
        graph.current_agent("task-0001"),
        Some("agent-beta"),
        "« qui la fait » est la dernière assignation"
    );
    assert!(
        graph.tasks_of("agent-alpha").contains("task-0001"),
        "« qui l'a faite » les garde toutes : c'est la question qu'un graphe réalisé doit trancher"
    );
}

#[test]
fn l_ordre_des_assignations_fait_partie_de_l_etat() {
    let mut forward = MemoryEventStore::new();
    assign(&mut forward, 1, "t", "a", "w", ActorKind::System);
    assign(&mut forward, 2, "t", "b", "w", ActorKind::System);

    let mut backward = MemoryEventStore::new();
    assign(&mut backward, 1, "t", "b", "w", ActorKind::System);
    assign(&mut backward, 2, "t", "a", "w", ActorKind::System);

    let mut one = ProjectionRunner::new(OrganisationGraph::new());
    one.catch_up(&forward);
    let mut other = ProjectionRunner::new(OrganisationGraph::new());
    other.catch_up(&backward);

    assert_ne!(
        one.projection().checksum(),
        other.projection().checksum(),
        "« A puis B » n'est pas « B puis A » : un résumé qui les confond ne détecte pas une \
         inversion à la reconstruction"
    );
    assert_eq!(one.projection().current_agent("t"), Some("b"));
    assert_eq!(other.projection().current_agent("t"), Some("a"));
}

// ---------------------------------------------------------------------------------------------
// Aucun instantané n'est reçu du worker
// ---------------------------------------------------------------------------------------------

/// Invariant 3 : « un worker ne modifie jamais directement la base canonique. »
///
/// L'assignation est une décision du plan de contrôle. Un agent qui en annoncerait une décrirait
/// **sa propre affectation**, et un graphe qui le croirait serait un graphe que les workers
/// écrivent. L'événement n'est pas une erreur — il est journalisé, et c'est bien ainsi — il n'est
/// simplement pas une source.
#[test]
fn une_assignation_annoncee_par_un_agent_n_entre_pas_dans_le_graphe() {
    let mut store = MemoryEventStore::new();
    assign(
        &mut store,
        1,
        "task-0001",
        "agent-intrus",
        "vm-01",
        ActorKind::Agent,
    );
    let mut runner = ProjectionRunner::new(OrganisationGraph::new());
    runner.catch_up(&store);

    assert!(
        runner.projection().assignments().is_empty(),
        "un worker qui s'auto-assigne écrirait le graphe organisationnel"
    );
    assert!(
        runner.projection().agents().is_empty(),
        "et il n'y entre pas non plus comme agent connu"
    );
    assert_eq!(
        store.export().len(),
        1,
        "l'événement reste journalisé : refuser de le croire n'est pas l'effacer"
    );
}

/// La distinction n'est pas « agent contre humain » : c'est « le plan de contrôle décide ».
#[test]
fn seul_le_systeme_est_source_d_assignation() {
    for actor in [ActorKind::Agent, ActorKind::Human] {
        let mut store = MemoryEventStore::new();
        assign(&mut store, 1, "t", "a", "w", actor);
        let mut runner = ProjectionRunner::new(OrganisationGraph::new());
        runner.catch_up(&store);
        assert!(
            runner.projection().assignments().is_empty(),
            "{actor:?} n'est pas le plan de contrôle"
        );
    }

    let mut store = MemoryEventStore::new();
    assign(&mut store, 1, "t", "a", "w", ActorKind::System);
    let mut runner = ProjectionRunner::new(OrganisationGraph::new());
    runner.catch_up(&store);
    assert_eq!(runner.projection().assignments().len(), 1);
}

// ---------------------------------------------------------------------------------------------
// La quarantaine — ADR 0013
// ---------------------------------------------------------------------------------------------

/// Une assignation sans agent est exactement ce que cette projection existe pour joindre : la
/// laisser passer rendrait un graphe silencieusement incomplet, ce qui est pire qu'un graphe
/// arrêté.
#[test]
fn une_assignation_sans_agent_met_la_projection_en_quarantaine() {
    let mut store = MemoryEventStore::new();
    push(
        &mut store,
        1,
        "task-0001",
        "task.assigned",
        ActorKind::System,
        serde_json::json!({ "task_id": "task-0001", "worker_id": "vm-01" }),
    );
    let mut runner = ProjectionRunner::new(OrganisationGraph::new());
    runner.catch_up(&store);

    assert!(
        matches!(
            runner.health(),
            locus_projections::Health::Quarantined { .. }
        ),
        "{:?}",
        runner.health()
    );
}

#[test]
fn le_bruit_qui_n_est_pas_une_assignation_ne_derange_pas() {
    let (_, runner) = live();
    assert!(matches!(
        runner.health(),
        locus_projections::Health::Healthy
    ));
    assert_eq!(
        runner.projection().assignments().len(),
        3,
        "trois assignations, et l'événement `task.started` n'en est pas une"
    );
}
