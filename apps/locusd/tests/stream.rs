//! Le test de sortie de `W20.f` — le fil d'événements clients de §22.1.

use locus_event_store::{
    Actor, ActorKind, Draft as EventDraft, EventStore, EventType, MemoryEventStore,
};
use locus_protocol::id::{Agent, Command, Event, Project, Workspace};
use locus_protocol::{Id, IdKind, Timestamp};
use locusd::cursor::{Collection, Cursor, CursorError};
use locusd::stream::{COALESCIBLE, DELIVERY, Frame, is_coalescible};
use locusd::{CommandEnvelope, CommandError, Decide, Revision, Runtime};

const NOW: Timestamp = Timestamp::from_millis(1_700_000_000_000);

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

/// Un décideur qui écrit exactement les types qu'on lui donne, dans l'ordre.
struct Ecrit(&'static [&'static str]);

impl Decide for Ecrit {
    type State = ();

    fn decide(
        &self,
        command: &CommandEnvelope,
        (): &Self::State,
    ) -> Result<Vec<EventDraft>, CommandError> {
        Ok(self
            .0
            .iter()
            .enumerate()
            .map(|(index, kind)| EventDraft {
                event_id: id::<Event>(u8::try_from(index).unwrap_or(0)),
                event_type: EventType::parse(kind).expect("type valide"),
                schema_version: 1,
                stream_id: "task/tsk_01".to_owned(),
                workspace_id: id::<Workspace>(2),
                project_id: id::<Project>(4),
                program_id: None,
                branch_id: None,
                actor: Actor {
                    principal_id: id::<Agent>(3),
                    kind: ActorKind::Agent,
                    delegation_id: None,
                },
                occurred_at: NOW,
                causation_id: *command.command_id(),
                idempotency_key: None,
                correlation_id: None,
                trace_id: None,
                payload: serde_json::json!({ "n": index }),
                payload_hash: format!("sha256:{}", "ab".repeat(32)),
            })
            .collect())
    }
}

fn commande(seed: u8, revision: u64) -> CommandEnvelope {
    CommandEnvelope::mutating(
        id::<Command>(seed),
        "task.start",
        id::<Workspace>(2),
        id::<Agent>(3),
        format!("idem-{seed}"),
        Revision::new(revision),
    )
    .expect("commande bien formée")
}

fn runtime_avec(types: &'static [&'static str]) -> Runtime<MemoryEventStore> {
    let runtime = Runtime::in_memory();
    runtime
        .transaction()
        .submit(&Ecrit(types), &commande(1, 0), &(), NOW)
        .accepted()
        .expect("l'écriture passe");
    runtime
}

// ---------------------------------------------------------------------------------------------
// 1. Un client qui se reconnecte reprend depuis sa séquence et ne perd rien
// ---------------------------------------------------------------------------------------------

/// **La reprise ne saute ni ne répète**, et c'est vérifié sur l'ensemble parcouru.
///
/// Le scénario est celui d'une déconnexion : le client a reçu deux événements, garde le cursor du
/// dernier, revient. Ce qu'il reçoit ensuite doit être exactement le reste.
#[test]
fn un_client_qui_se_reconnecte_reprend_sans_rien_perdre() {
    let runtime = runtime_avec(&[
        "task.started",
        "artifact.declared",
        "run.recorded",
        "task.completed",
    ]);

    let tout: Vec<u64> = runtime
        .events_since(None)
        .expect("sans cursor")
        .events
        .iter()
        .map(|event| event.position)
        .collect();
    assert_eq!(tout.len(), 4);

    // Le client s'interrompt après le deuxième, et garde le cursor de celui-là.
    let cursor = Cursor::issue(Collection::Events, tout[1]);
    let suite: Vec<u64> = runtime
        .events_since(Some(&cursor))
        .expect("reprise")
        .events
        .iter()
        .map(|event| event.position)
        .collect();

    assert_eq!(suite, tout[2..], "exactement le reste, dans l'ordre");
    assert!(
        suite.iter().all(|position| !tout[..2].contains(position)),
        "rien de déjà reçu ne revient"
    );
}

/// Un cursor d'une autre collection est refusé sur le fil aussi.
#[test]
fn le_fil_refuse_un_cursor_d_une_autre_collection() {
    let runtime = runtime_avec(&["task.started"]);
    let etranger = Cursor::issue(Collection::Timeline, 1);

    assert_eq!(
        runtime.events_since(Some(&etranger)).err(),
        Some(CursorError::WrongCollection {
            expected: Collection::Events,
            found: Collection::Timeline,
        }),
        "la timeline et le fil suivent la même position : les confondre serait silencieux"
    );
}

// ---------------------------------------------------------------------------------------------
// 2. Un client lent ne fait rien perdre au journal, et relit ce qu'il n'a pas reçu
// ---------------------------------------------------------------------------------------------

/// **Le journal garde tout, quoi que fasse le client.**
///
/// C'est ce qui remplace le spool par client : un abonné qui ne revient jamais ne coûte rien, et un
/// abonné qui revient trouve tout. Le test le vérifie en n'appelant simplement pas le fil pendant
/// que des événements s'écrivent — ce qu'un client lent fait, du point de vue du serveur.
#[test]
fn un_client_lent_ne_fait_rien_perdre_et_relit() {
    let runtime = runtime_avec(&["task.started", "artifact.declared"]);

    let premier = runtime.events_since(None).expect("premier passage");
    let cursor = premier.next.expect("il y a eu des événements");
    assert_eq!(premier.events.len(), 2);

    // Le client ne revient pas. Le serveur, lui, continue d'écrire.
    let revision = runtime
        .transaction()
        .store()
        .revision("task/tsk_01")
        .expect("le stream existe");
    runtime
        .transaction()
        .submit(&Ecrit(&["run.recorded"]), &commande(2, revision), &(), NOW)
        .accepted()
        .expect("l'écriture pendant l'absence passe");

    // Il revient avec son ancien cursor : rien n'a été perdu.
    let reprise = runtime.events_since(Some(&cursor)).expect("reprise");
    assert_eq!(reprise.events.len(), 1);
    assert_eq!(reprise.events[0].event_type, "run.recorded");
}

/// Un décideur qui écrit `count` faits non coalescibles, pour dépasser la borne.
struct Beaucoup(usize);

impl Decide for Beaucoup {
    type State = ();

    fn decide(
        &self,
        command: &CommandEnvelope,
        (): &Self::State,
    ) -> Result<Vec<EventDraft>, CommandError> {
        Ok((0..self.0)
            .map(|index| EventDraft {
                event_id: id::<Event>(u8::try_from(index % 250).unwrap_or(0)),
                event_type: EventType::parse("task.started").expect("type valide"),
                schema_version: 1,
                stream_id: "task/tsk_01".to_owned(),
                workspace_id: id::<Workspace>(2),
                project_id: id::<Project>(4),
                program_id: None,
                branch_id: None,
                actor: Actor {
                    principal_id: id::<Agent>(3),
                    kind: ActorKind::Agent,
                    delegation_id: None,
                },
                occurred_at: NOW,
                causation_id: *command.command_id(),
                idempotency_key: None,
                correlation_id: None,
                trace_id: None,
                payload: serde_json::json!({ "n": index }),
                payload_hash: format!("sha256:{}", "ab".repeat(32)),
            })
            .collect())
    }
}

/// **Le drapeau `more` dit au client de rappeler tout de suite** — et les deux cas sont exercés.
///
/// Sans lui, un client qui rattrape un long retard s'arrêterait à la première borne en se croyant à
/// jour, et resterait en retard jusqu'à ce que le hasard produise un événement de plus.
///
/// Vérifier le seul cas court aurait laissé le drapeau bloqué à `false` sans que rien ne bronche :
/// c'est le cas long qui porte la propriété, et c'est le seul qu'un test nommé « plus long que la
/// borne » a le droit d'omettre le moins.
#[test]
fn un_retard_plus_long_que_la_borne_se_signale() {
    let court = runtime_avec(&["task.started", "task.completed"]);
    assert!(
        !court.events_since(None).expect("passage").more,
        "tout tenait dans la borne"
    );

    let long = Runtime::in_memory();
    long.transaction()
        .submit(&Beaucoup(DELIVERY + 5), &commande(1, 0), &(), NOW)
        .accepted()
        .expect("l'écriture longue passe");

    let premier = long.events_since(None).expect("premier passage");
    assert_eq!(premier.events.len(), DELIVERY, "la borne est tenue");
    assert!(premier.more, "il en reste, et le client doit le savoir");

    let reste = long
        .events_since(premier.next.as_ref())
        .expect("second passage");
    assert_eq!(reste.events.len(), 5, "exactement le reste");
    assert!(!reste.more, "et cette fois c'est fini");
}

// ---------------------------------------------------------------------------------------------
// 3. La coalescence de §18.3 vaut pour le fil client
// ---------------------------------------------------------------------------------------------

/// **Deny-by-default**, et les deux listes de §18.3 ne se recoupent pas.
#[test]
fn seuls_les_types_de_la_premiere_liste_sont_coalescibles() {
    for coalescible in COALESCIBLE {
        assert!(is_coalescible(&format!("task.{coalescible}")));
    }
    for jamais in [
        "task.started",
        "attempt.completed",
        "tool.completed",
        "artifact.declared",
        "budget.exceeded",
        "review.requested",
    ] {
        assert!(
            !is_coalescible(jamais),
            "« {jamais} » ne peut jamais être fusionné : un coût ou une alerte perdus dans une fusion sont perdus pour de bon"
        );
    }
}

/// Une rafale coalescible fusionne, et le compte voyage.
#[test]
fn une_rafale_coalescible_fusionne_en_disant_combien() {
    let runtime = runtime_avec(&[
        "task.progress",
        "task.progress",
        "task.progress",
        "task.completed",
    ]);

    let delivery = runtime.events_since(None).expect("passage");
    assert_eq!(delivery.events.len(), 2, "{:?}", delivery.events);
    assert_eq!(delivery.events[0].event_type, "task.progress");
    assert_eq!(
        delivery.events[0].coalesced, 3,
        "le client doit savoir qu'il a reçu un résumé"
    );
    assert_eq!(delivery.events[1].event_type, "task.completed");
    assert_eq!(delivery.events[1].coalesced, 1);
}

/// **Un événement non coalescible coupe la rafale.**
///
/// Sans la coupure, le `progress` postérieur à l'artefact remonterait avant lui, et le client
/// verrait une progression annoncée avant le fait qu'elle décrit.
#[test]
fn un_evenement_non_coalescible_coupe_la_rafale() {
    let runtime = runtime_avec(&[
        "task.progress",
        "task.progress",
        "artifact.declared",
        "task.progress",
    ]);

    let delivery = runtime.events_since(None).expect("passage");
    let types: Vec<&str> = delivery
        .events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect();
    assert_eq!(
        types,
        vec!["task.progress", "artifact.declared", "task.progress"],
        "la rafale ne franchit pas l'artefact"
    );
    assert_eq!(delivery.events[0].coalesced, 2);
    assert_eq!(delivery.events[2].coalesced, 1);
}

/// Deux types coalescibles **différents** ne fusionnent pas entre eux.
#[test]
fn deux_types_coalescibles_differents_ne_fusionnent_pas() {
    let runtime = runtime_avec(&["task.progress", "task.log", "task.progress"]);

    let delivery = runtime.events_since(None).expect("passage");
    assert_eq!(delivery.events.len(), 3);
    assert!(delivery.events.iter().all(|event| event.coalesced == 1));
}

// ---------------------------------------------------------------------------------------------
// 4. Le cadrage SSE
// ---------------------------------------------------------------------------------------------

/// **Le cadre porte le cursor en `id:`**, ce qui fait de la reprise une propriété du protocole.
#[test]
fn le_cadre_sse_porte_le_cursor_en_id() {
    let runtime = runtime_avec(&["task.started"]);
    let delivery = runtime.events_since(None).expect("passage");
    let event = &delivery.events[0];

    let cadre = Frame::event(event);
    let attendu = Cursor::issue(Collection::Events, event.position);

    assert!(cadre.starts_with(&format!("id: {attendu}\n")), "{cadre}");
    assert!(cadre.contains("event: task.started\n"));
    assert!(cadre.contains("data: {"));
    assert!(
        cadre.ends_with("\n\n"),
        "un cadre SSE se termine par une ligne vide"
    );
    assert_eq!(Frame::CONTENT_TYPE, "text/event-stream");
}

/// **Le keep-alive ne porte pas d'`id`.**
///
/// Un maintien de connexion qui avancerait la reprise ferait perdre des événements à chaque
/// silence — c'est-à-dire précisément quand rien ne permet de s'en apercevoir.
#[test]
fn le_keep_alive_ne_deplace_pas_la_reprise() {
    let cadre = Frame::keep_alive();
    assert!(
        cadre.starts_with(':'),
        "un commentaire SSE commence par « : »"
    );
    assert!(!cadre.contains("id:"), "{cadre}");
    assert!(cadre.ends_with("\n\n"));
}
