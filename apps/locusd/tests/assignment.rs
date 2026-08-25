//! Test de sortie de `W20.ad` — **l'assignation d'une tâche atteint le journal.**
//!
//! # Ce que ce fichier doit prouver, et pourquoi la troisième propriété est la seule qui compte
//!
//! `packages/projections` sait lire `task.assigned` depuis `W13.g`, et **aucun handler ne l'écrivait**
//! : ses propres tests fabriquaient les enveloppes qu'elle relit. Un producteur écrit ici ne vaut
//! donc rien tant qu'on ne l'a pas confronté au lecteur réel — écrire un second test qui fabrique
//! la même enveloppe des deux côtés reproduirait exactement le défaut qu'on répare.
//!
//! Trois propriétés, et aucune ne se déduit des autres :
//!
//! 1. une assignation passe par un `Decide` et écrit **un** fait, portant les trois champs ;
//! 2. une tâche dans un état terminal est refusée par une **politique**, et rien n'est écrit ;
//! 3. la projection de `W13.g`, **sans être modifiée**, lit ce que le journal porte et en tire le
//!    lien agent × tâche × worker.
//!
//! La troisième est le test de sortie proprement dit. Les deux premières la rendent lisible : sans
//! elles, un échec de la troisième ne dirait pas de quel côté du fil il vient.

use locus_coordination::task::Assignment;
use locus_domain::TaskState;
use locus_event_store::{ActorKind, EventStore, MemoryEventStore};
use locus_projections::organisation_graph::OrganisationGraph;
use locus_projections::projection::Projection;
use locus_protocol::id::{Agent, Command, Event, Project, Workspace};
use locus_protocol::{Id, IdKind, Timestamp};
use locusd::{
    Assign, CommandEnvelope, CommandError, LepContext, Outcome, Revision, Transaction,
    stream_of_task,
};

const NOW: Timestamp = Timestamp::from_millis(1_700_000_000_000);
const TACHE: &str = "01J0000000000000000000TASK";
const WORKER: &str = "worker-a";

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(NOW, entropy).expect("l'instant de fixture tient sur 48 bits")
}

fn agent(seed: u8) -> Id<Agent> {
    id::<Agent>(seed)
}

fn contexte(seed: u8) -> LepContext {
    LepContext {
        project_id: id::<Project>(4),
        event_ids: vec![id::<Event>(seed)],
        occurred_at: NOW,
        payload_hash: format!("sha256:{}", "ab".repeat(32)),
    }
}

fn commande(seed: u8, revision: u64) -> CommandEnvelope {
    CommandEnvelope::mutating(
        id::<Command>(seed),
        "task.assign",
        id::<Workspace>(2),
        agent(3),
        format!("cle-{seed}"),
        Revision::new(revision),
    )
    .expect("commande bien formée")
}

fn assignation(agent_seed: u8, worker: &str) -> Assignment {
    Assignment::new(agent(agent_seed), worker, NOW).expect("un worker nommé")
}

/// Assigner, et rendre la transaction pour que l'appelant lise ce qu'elle a écrit.
fn assigner(
    transaction: &Transaction<MemoryEventStore>,
    seed: u8,
    revision: u64,
    from: TaskState,
    agent_seed: u8,
    worker: &str,
) -> Outcome {
    transaction.submit(
        &Assign {
            task_id: TACHE.to_owned(),
            from,
            assignment: assignation(agent_seed, worker),
        },
        &commande(seed, revision),
        &contexte(seed),
        NOW,
    )
}

// ---------------------------------------------------------------------------------------------
// 1. Le fait est écrit, et il porte les trois champs
// ---------------------------------------------------------------------------------------------

/// **Les deux identités, pas une.**
///
/// §7.1 porte `assigned_agent_id` et `assigned_worker_id`, et la projection existe pour joindre les
/// deux. Un fait qui n'en porterait qu'un rendrait indécidable l'une des deux questions — « qui a
/// fait ce travail » et « où a-t-il tourné » —, et c'est exactement ce que le cycle de bail de
/// `W20.k` ne sait pas dire : un bail nomme un worker.
#[test]
fn une_assignation_ecrit_un_fait_portant_l_agent_et_le_worker() {
    let transaction = Transaction::new(MemoryEventStore::new());
    let verdict = assigner(&transaction, 1, 0, TaskState::Queued, 7, WORKER);
    assert!(matches!(verdict, Outcome::Accepted(_)), "{verdict:?}");

    let ecrits = transaction.store().read_stream(&stream_of_task(TACHE), 0);
    assert_eq!(ecrits.len(), 1, "une assignation écrit un fait, pas deux");

    let ecrit = &ecrits[0];
    assert_eq!(ecrit.event_type.to_string(), "task.assigned");
    assert_eq!(ecrit.payload["task_id"], TACHE);
    assert_eq!(ecrit.payload["agent_id"], agent(7).to_string());
    assert_eq!(ecrit.payload["worker_id"], WORKER);
}

/// L'acteur est le **système**, et c'est le seul du dépôt.
///
/// La projection ignore en silence tout autre acteur — invariant 3 : l'assignation est une décision
/// du plan de contrôle, et un graphe qui croirait un agent qui annonce sa propre affectation serait
/// un graphe que les workers écrivent. Les deux aides existantes, `lep::fact` et `mission::fact`,
/// posent `Agent` et documentent pourquoi ; les emprunter ici aurait produit un fait que le seul
/// lecteur laisse tomber sans rien dire.
#[test]
fn l_acteur_est_le_systeme_et_le_principal_reste_celui_de_la_commande() {
    let transaction = Transaction::new(MemoryEventStore::new());
    assert!(matches!(
        assigner(&transaction, 1, 0, TaskState::Queued, 7, WORKER),
        Outcome::Accepted(_)
    ));

    let ecrits = transaction.store().read_stream(&stream_of_task(TACHE), 0);
    assert_eq!(ecrits[0].actor.kind, ActorKind::System);
    // `kind` dit qui a décidé, `principal_id` sous quelle autorité. Les confondre obligerait à
    // inventer un principal système, que rien n'enrôle.
    assert_eq!(ecrits[0].actor.principal_id, agent(3));
}

/// La charge **ne porte pas** d'état, et l'absence est la propriété.
///
/// `Task::assigned` le dit : « n'est pas une transition — une tâche `running` réassignée reste
/// `running` ». Tous les autres faits de la famille `task` portent l'état atteint ; celui-ci
/// n'en atteint aucun. Un champ `state` ferait de `task.assigned` le seul événement de la famille
/// dont l'état ne rapporte pas de changement, et un lecteur qui balaie la famille pour reconstruire
/// la machine de §7.1 compterait une transition qui n'a pas eu lieu.
#[test]
fn la_charge_ne_porte_pas_d_etat_car_assigner_n_est_pas_une_transition() {
    let transaction = Transaction::new(MemoryEventStore::new());
    assert!(matches!(
        assigner(&transaction, 1, 0, TaskState::Running, 7, WORKER),
        Outcome::Accepted(_)
    ));

    let ecrits = transaction.store().read_stream(&stream_of_task(TACHE), 0);
    assert!(
        ecrits[0].payload.get("state").is_none(),
        "assigner ne fait pas changer d'état : {}",
        ecrits[0].payload
    );
}

/// L'instant vient de l'**assignation**, pas du contexte.
///
/// §10.1 sépare `occurred_at` de l'instant d'écriture, et la valeur de domaine est la seule qui
/// sache quand l'acte a eu lieu. L'écrire aux deux endroits laisserait les deux diverger sans que
/// rien ne le dise.
#[test]
fn l_instant_de_l_acte_vient_de_l_assignation() {
    let transaction = Transaction::new(MemoryEventStore::new());
    let jadis = Timestamp::from_millis(1_600_000_000_000);
    let verdict = transaction.submit(
        &Assign {
            task_id: TACHE.to_owned(),
            from: TaskState::Queued,
            assignment: Assignment::new(agent(7), WORKER, jadis).expect("un worker nommé"),
        },
        &commande(1, 0),
        // Le contexte, lui, dit `NOW`.
        &contexte(1),
        NOW,
    );
    assert!(matches!(verdict, Outcome::Accepted(_)), "{verdict:?}");

    let ecrits = transaction.store().read_stream(&stream_of_task(TACHE), 0);
    assert_eq!(ecrits[0].occurred_at, jadis);
}

// ---------------------------------------------------------------------------------------------
// 2. Ce que le domaine refuse n'entre pas dans le journal
// ---------------------------------------------------------------------------------------------

/// Une tâche finie ne se confie pas, et le refus est une **politique**.
///
/// Le client a envoyé une requête bien écrite ; c'est l'état de la tâche qui s'y oppose. Lui rendre
/// `validation` l'enverrait relire sa requête, où il ne trouverait rien. Même arbitrage que pour
/// les transitions de branche et les opérations de coordination.
#[test]
fn une_tache_terminale_est_refusee_par_une_politique_et_n_ecrit_rien() {
    let transaction = Transaction::new(MemoryEventStore::new());
    let verdict = assigner(&transaction, 1, 0, TaskState::Accepted, 7, WORKER);

    match verdict {
        Outcome::Refused(CommandError::Policy { policy, detail }) => {
            assert_eq!(policy, "task.assignment");
            assert!(
                detail.contains("accepted"),
                "le refus nomme l'état qui s'y oppose : {detail}"
            );
        }
        autre => panic!("une tâche acceptée ne se confie pas, et par une politique : {autre:?}"),
    }
    assert!(
        transaction
            .store()
            .read_stream(&stream_of_task(TACHE), 0)
            .is_empty(),
        "un refus n'écrit rien"
    );
}

/// Les **six** états terminaux de §7.1 sont refusés, pas seulement celui qui a servi d'exemple.
///
/// La garde interroge `TaskState::is_terminal`, qui dérive la réponse de la table de §7.1 ; ce test
/// vérifie que la dérivation est bien celle qu'on croit, en balayant les quinze états. Un test sur
/// un seul état terminal passerait avec un `if state == Accepted` écrit à la main, c'est-à-dire
/// avec la seconde copie de la table que ce module refuse d'avoir.
#[test]
fn tous_les_etats_terminaux_sont_refuses_et_aucun_autre() {
    for etat in TaskState::ALL {
        let transaction = Transaction::new(MemoryEventStore::new());
        let _ = assigner(&transaction, 1, 0, etat, 7, WORKER);
        let ecrit = !transaction
            .store()
            .read_stream(&stream_of_task(TACHE), 0)
            .is_empty();
        assert_eq!(
            ecrit,
            !etat.is_terminal(),
            "« {etat} » : terminal={}, écrit={ecrit}",
            etat.is_terminal()
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 3. Le test de sortie : la projection de W13.g lit ce que le producteur écrit
// ---------------------------------------------------------------------------------------------

/// **Le lecteur n'est pas modifié**, et c'est la façon de vérifier que le producteur parle sa
/// langue.
///
/// `OrganisationGraph` est pris tel que `W13.g` l'a livré. Ce qu'on lui donne est ce que le journal
/// porte — pas une enveloppe fabriquée pour l'occasion, ce qui est précisément l'artifice par lequel
/// un lecteur sans producteur peut rester vert pendant des mois.
#[test]
fn la_projection_de_w13g_lit_l_evenement_sans_etre_modifiee() {
    let transaction = Transaction::new(MemoryEventStore::new());
    assert!(matches!(
        assigner(&transaction, 1, 0, TaskState::Queued, 7, WORKER),
        Outcome::Accepted(_)
    ));
    // La même tâche change de main : l'histoire, pas l'état courant (invariant 12).
    assert!(matches!(
        assigner(&transaction, 2, 1, TaskState::Running, 8, "worker-b"),
        Outcome::Accepted(_)
    ));

    let mut graphe = OrganisationGraph::new();
    for (rang, ecrit) in transaction.store().feed(0).iter().enumerate() {
        let position = u64::try_from(rang).expect("deux faits tiennent dans un u64") + 1;
        graphe
            .apply(position, &ecrit.event)
            .expect("la projection accepte ce que le producteur écrit");
    }

    assert_eq!(graphe.tasks().len(), 1, "une seule tâche a été confiée");
    assert_eq!(
        graphe.current_agent(TACHE),
        Some(agent(8).to_string().as_str()),
        "le dernier à l'avoir est celui de la seconde assignation"
    );
    assert_eq!(
        graphe.tasks_of(&agent(7).to_string()).len(),
        1,
        "la première assignation n'est pas effacée par la seconde"
    );
    assert_eq!(
        graphe.workers_of(&agent(8).to_string()),
        [("worker-b")].into_iter().collect(),
        "l'agent de la seconde assignation a tourné sur le worker de la seconde"
    );
}
