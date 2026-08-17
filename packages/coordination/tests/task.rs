//! Test de sortie de W13.d — **l'assignation est un événement ; la machine à états de `task.rs`
//! est inchangée et ses tests passent.**
//!
//! Un état dit **où en est** le travail ; une assignation dit **qui le fait**. Les deux changent
//! indépendamment, et les confondre obligerait à croiser quinze états avec autant d'agents.

use locus_coordination::{Assignment, Task, TaskError};
use locus_domain::{ForbiddenTransition, TaskState, transition};
use locus_protocol::{
    Id, IdKind, Timestamp,
    id::{Agent, Branch, provisional::Task as TaskKind},
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

fn instant(millis: i64) -> Timestamp {
    Timestamp::from_millis(1_700_000_000_000 + millis)
}

fn proposed() -> Task {
    Task::propose(
        id::<TaskKind>(1),
        id::<Branch>(1),
        "formalisation",
        "formaliser le lemme 3",
        "idem-task-0001",
    )
    .expect("tâche valide")
}

fn running() -> Task {
    proposed()
        .moved_to(TaskState::Queued)
        .expect("en file")
        .moved_to(TaskState::Leased)
        .expect("attribuée")
        .moved_to(TaskState::Running)
        .expect("en cours")
}

fn assignment(agent: u8, worker: &str, millis: i64) -> Assignment {
    Assignment::new(id::<Agent>(agent), worker, instant(millis)).expect("assignation valide")
}

// ---------------------------------------------------------------------------------------------
// L'assignation ne touche pas à l'état
// ---------------------------------------------------------------------------------------------

/// La propriété que le sprint tient. Assigner ne fait pas avancer, et avancer ne réassigne pas.
#[test]
fn assigner_ne_change_pas_l_etat() {
    let before = running();
    let after = before
        .clone()
        .assigned(assignment(1, "canterel-vm-linux-01", 0))
        .expect("tâche vivante");

    assert_eq!(after.state(), before.state());
    assert_eq!(after.state(), TaskState::Running);
    assert_eq!(after.assigned_agent_id(), Some(id::<Agent>(1)));
    assert_eq!(after.assigned_worker_id(), Some("canterel-vm-linux-01"));
}

#[test]
fn avancer_ne_reassigne_pas() {
    let assigned = proposed()
        .moved_to(TaskState::Queued)
        .expect("en file")
        .moved_to(TaskState::Leased)
        .expect("attribuée")
        .assigned(assignment(1, "canterel-vm-linux-01", 0))
        .expect("tâche vivante");

    let advanced = assigned
        .clone()
        .moved_to(TaskState::Running)
        .expect("en cours");

    assert_eq!(advanced.assigned_agent_id(), assigned.assigned_agent_id());
    assert_eq!(advanced.assignments().len(), 1);
}

/// Une réassignation en cours d'exécution — le cas qui rend la table impossible si l'assignation
/// était une transition. La tâche ne quitte pas `running`, et les deux assignations restent.
#[test]
fn une_reassignation_garde_l_etat_et_l_histoire() {
    let task = running()
        .assigned(assignment(1, "canterel-vm-linux-01", 0))
        .expect("première")
        .assigned(assignment(2, "canterel-vm-linux-02", 60_000))
        .expect("seconde");

    assert_eq!(task.state(), TaskState::Running);
    assert_eq!(task.assignments().len(), 2);
    assert_eq!(task.assigned_agent_id(), Some(id::<Agent>(2)));
    assert_eq!(
        task.assignments()[0].agent_id(),
        id::<Agent>(1),
        "la première assignation reste : une tâche qui a changé de main a deux faits à consigner, \
         et le second n'efface pas le premier"
    );
    assert_eq!(task.assignments()[0].at(), instant(0));
}

/// W13.g dérive le graphe organisationnel **réalisé** de cette suite. Une seule valeur courante ne
/// dirait pas qui a travaillé, seulement qui travaille.
#[test]
fn l_histoire_des_assignations_est_ordonnee_et_complete() {
    let task = running()
        .assigned(assignment(1, "w1", 0))
        .expect("première")
        .assigned(assignment(2, "w2", 1_000))
        .expect("deuxième")
        .assigned(assignment(3, "w3", 2_000))
        .expect("troisième");

    let agents: Vec<Id<Agent>> = task
        .assignments()
        .iter()
        .map(Assignment::agent_id)
        .collect();
    assert_eq!(agents, vec![id::<Agent>(1), id::<Agent>(2), id::<Agent>(3)]);

    let workers: Vec<&str> = task
        .assignments()
        .iter()
        .map(Assignment::worker_id)
        .collect();
    assert_eq!(workers, vec!["w1", "w2", "w3"]);
}

/// Les deux identités, pas une : deux agents peuvent tourner sur le même worker, et un agent peut
/// être réassigné d'un worker à un autre. N'en garder qu'une rendrait indécidable l'une des deux
/// questions de W13.g.
#[test]
fn agent_et_worker_sont_deux_identites_distinctes() {
    let same_worker = running()
        .assigned(assignment(1, "canterel-vm-linux-01", 0))
        .expect("premier agent")
        .assigned(assignment(2, "canterel-vm-linux-01", 1_000))
        .expect("second agent, même machine");
    assert_eq!(same_worker.assignments().len(), 2);
    assert_eq!(
        same_worker.assignments()[0].worker_id(),
        same_worker.assignments()[1].worker_id()
    );
    assert_ne!(
        same_worker.assignments()[0].agent_id(),
        same_worker.assignments()[1].agent_id()
    );
}

#[test]
fn une_tache_finie_ne_se_confie_plus() {
    let done = running().moved_to(TaskState::Failed).expect("échouée");
    assert_eq!(
        done.assigned(assignment(1, "w1", 0)),
        Err(TaskError::TerminalState {
            state: TaskState::Failed
        }),
        "confier un travail achevé laisserait croire dans un journal que quelqu'un s'y est mis"
    );
}

#[test]
fn un_worker_sans_identite_ne_s_assigne_pas() {
    assert_eq!(
        Assignment::new(id::<Agent>(1), "  ", instant(0)),
        Err(TaskError::EmptyWorker)
    );
}

// ---------------------------------------------------------------------------------------------
// La machine à états du domaine est inchangée
// ---------------------------------------------------------------------------------------------

/// Ce module n'a pas de table à lui : il délègue. Une transition refusée par le domaine l'est ici
/// aussi, avec la **même** erreur — donc une divergence entre les deux tables est impossible, elle
/// n'a pas d'endroit où s'écrire.
#[test]
fn la_table_du_domaine_est_la_seule_qui_decide() {
    let task = proposed();
    let refused = task.clone().moved_to(TaskState::Running);
    assert_eq!(
        refused,
        Err(TaskError::Forbidden(
            transition(TaskState::Proposed, TaskState::Running)
                .expect_err("le domaine refuse ce saut")
        )),
        "l'erreur vient du domaine, mot pour mot"
    );

    // Et le chemin que le domaine autorise, ce module l'autorise.
    assert!(task.moved_to(TaskState::Queued).is_ok());
}

/// Les arêtes que §7.1 dessine, épinglées une par une.
///
/// Le test ne parcourt pas la table pour se comparer à elle-même : il énumère les transitions du
/// diagramme de la spec et exige que chacune soit permise, puis quelques-unes qu'elle ne dessine
/// pas et exige qu'elles soient refusées. C'est ce qui rend le test capable de voir un changement
/// de table, alors qu'un parcours ne verrait que sa propre cohérence.
#[test]
fn le_diagramme_de_7_1_est_celui_que_la_table_porte() {
    for (from, to) in [
        (TaskState::Proposed, TaskState::Queued),
        (TaskState::Queued, TaskState::Leased),
        (TaskState::Leased, TaskState::Running),
        (TaskState::Running, TaskState::WaitingForTool),
        (TaskState::Running, TaskState::WaitingForHuman),
        (TaskState::Running, TaskState::WaitingForReview),
        (TaskState::Running, TaskState::Succeeded),
        (TaskState::Running, TaskState::Failed),
        (TaskState::Running, TaskState::Cancelled),
        (TaskState::Running, TaskState::TimedOut),
        (TaskState::Leased, TaskState::Orphaned),
        (TaskState::Running, TaskState::Orphaned),
        (TaskState::Orphaned, TaskState::Queued),
        (TaskState::Succeeded, TaskState::Accepted),
        (TaskState::Succeeded, TaskState::Rejected),
        (TaskState::Succeeded, TaskState::Superseded),
    ] {
        assert!(
            transition(from, to).is_ok(),
            "§7.1 dessine {} → {}",
            from.as_str(),
            to.as_str()
        );
    }

    for (from, to) in [
        (TaskState::Proposed, TaskState::Leased),
        (TaskState::WaitingForTool, TaskState::Succeeded),
        (TaskState::Failed, TaskState::Running),
        (TaskState::Accepted, TaskState::Rejected),
    ] {
        assert!(
            matches!(transition(from, to), Err(ForbiddenTransition { .. })),
            "§7.1 ne dessine pas {} → {}",
            from.as_str(),
            to.as_str()
        );
    }
}

#[test]
fn les_quinze_etats_de_7_1_sont_toujours_la() {
    assert_eq!(TaskState::ALL.len(), 15);
    let terminal: Vec<&str> = TaskState::ALL
        .into_iter()
        .filter(|state| state.is_terminal())
        .map(TaskState::as_str)
        .collect();
    assert_eq!(
        terminal,
        vec![
            "failed",
            "cancelled",
            "timed_out",
            "accepted",
            "rejected",
            "superseded"
        ],
        "ajouter ou retirer un état terminal changerait ce qu'une tâche finie veut dire"
    );
}

// ---------------------------------------------------------------------------------------------
// Le reste de l'agrégat
// ---------------------------------------------------------------------------------------------

#[test]
fn une_tache_nait_proposee_et_sans_assignation() {
    let task = proposed();
    assert_eq!(task.state(), TaskState::Proposed);
    assert_eq!(task.attempt(), 0);
    assert!(task.assignments().is_empty(), "proposer n'est pas confier");
    assert_eq!(task.assigned_agent_id(), None);
    assert_eq!(task.assigned_worker_id(), None);
}

#[test]
fn une_tache_sans_cle_d_idempotence_est_refusee() {
    // La clé est exigée dès la proposition : c'est elle qui empêche qu'une reprise après incident
    // crée une seconde tâche pour le même travail, et une clé attribuée plus tard arriverait après
    // le doublon.
    assert_eq!(
        Task::propose(
            id::<TaskKind>(1),
            id::<Branch>(1),
            "formalisation",
            "formaliser le lemme 3",
            "  "
        ),
        Err(TaskError::EmptyField {
            field: "idempotency_key"
        })
    );
}

#[test]
fn le_numero_d_attempt_ne_redescend_jamais() {
    let reborn = running()
        .moved_to(TaskState::Orphaned)
        .expect("lease perdu")
        .next_attempt()
        .moved_to(TaskState::Queued)
        .expect("elle repart");
    assert_eq!(reborn.attempt(), 1);
    assert_eq!(reborn.state(), TaskState::Queued);

    let again = reborn.next_attempt();
    assert_eq!(
        again.attempt(),
        2,
        "réutiliser un numéro rendrait deux exécutions indiscernables dans le journal"
    );
}

#[test]
fn chaque_changement_incremente_la_revision() {
    let task = proposed();
    assert_eq!(task.revision(), 1);
    let queued = task.moved_to(TaskState::Queued).expect("en file");
    assert_eq!(queued.revision(), 2);
    let leased = queued.moved_to(TaskState::Leased).expect("attribuée");
    let assigned = leased.assigned(assignment(1, "w1", 0)).expect("confiée");
    assert_eq!(
        assigned.revision(),
        4,
        "l'assignation est un changement : le CAS de W13.e s'appuie dessus"
    );
}
