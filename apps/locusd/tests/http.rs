//! Le test de sortie de `W20.g` — la liaison HTTP, vérifiée sur des réponses **réelles**.
//!
//! # Pourquoi une requête écrite à la main plutôt qu'un client HTTP
//!
//! Le test de sortie demande « vérifié sur une réponse réelle ». Un appel de service en mémoire —
//! `tower::ServiceExt::oneshot` — court-circuite le serveur, le parsing de la requête et l'écriture
//! de la réponse : il vérifie un handler, pas une liaison. Et il coûterait une dépendance de plus.
//!
//! Ces tests ouvrent donc un vrai socket, écrivent une requête HTTP/1.1 à la main et lisent les
//! octets qui reviennent. Une requête minimale tient en trois lignes, et ce qu'on lit ensuite est
//! exactement ce qu'un client verrait.

use std::fmt::Write as _;
use std::sync::Arc;

use locus_event_store::{Actor, ActorKind, Draft as EventDraft, EventType};
use locus_protocol::id::{Agent, Command, Event, Project, Workspace};
use locus_protocol::{Id, IdKind, Timestamp};
use locusd::cursor::{Collection, Cursor};
use locusd::http::{DEFAULT_BIND, router, served};
use locusd::{CommandEnvelope, CommandError, Decide, Revision, Runtime};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const NOW: Timestamp = Timestamp::from_millis(1_700_000_000_000);

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

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
                correlation_id: None,
                trace_id: None,
                payload: serde_json::json!({ "worker_id": "wrk_01", "attempt_id": "att_01" }),
                payload_hash: format!("sha256:{}", "ab".repeat(32)),
            })
            .collect())
    }
}

fn commande() -> CommandEnvelope {
    CommandEnvelope::mutating(
        id::<Command>(1),
        "task.start",
        id::<Workspace>(2),
        id::<Agent>(3),
        "idem-1",
        Revision::INITIAL,
    )
    .expect("commande bien formée")
}

/// Un daemon qui écoute réellement, sur un port que le système choisit.
///
/// Le port `0` laisse le noyau attribuer : deux tests qui se partageraient un port fixe
/// échoueraient l'un l'autre au hasard de l'ordonnancement, et le diagnostic porterait sur la
/// mauvaise chose.
async fn serveur(types: &'static [&'static str]) -> String {
    let mut runtime = Runtime::in_memory();
    runtime
        .transaction()
        .submit(&Ecrit(types), &commande(), &(), NOW)
        .accepted()
        .expect("l'écriture passe");
    runtime.catch_up();

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("la boucle locale accepte un port libre");
    let adresse = listener
        .local_addr()
        .expect("l'adresse est connue")
        .to_string();
    let app = router(Arc::new(runtime));

    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    adresse
}

/// Une requête HTTP/1.1 écrite à la main, et la réponse brute.
async fn demander(adresse: &str, cible: &str, entetes: &[(&str, &str)]) -> String {
    let mut flux = TcpStream::connect(adresse).await.expect("le daemon écoute");
    let mut requete = format!("GET {cible} HTTP/1.1\r\nHost: {adresse}\r\nConnection: close\r\n");
    for (nom, valeur) in entetes {
        let _ = write!(requete, "{nom}: {valeur}\r\n");
    }
    requete.push_str("\r\n");
    flux.write_all(requete.as_bytes())
        .await
        .expect("la requête part");

    let mut reponse = Vec::new();
    flux.read_to_end(&mut reponse)
        .await
        .expect("la réponse arrive");
    String::from_utf8_lossy(&reponse).into_owned()
}

// ---------------------------------------------------------------------------------------------
// 1. Une route rend `text/event-stream`, et le cadre porte le cursor en `id:`
// ---------------------------------------------------------------------------------------------

/// **Vérifié sur une réponse réelle** : les octets qui reviennent du socket, pas un handler appelé.
#[tokio::test]
async fn le_fil_rend_un_event_stream_dont_le_cadre_porte_le_cursor() {
    let adresse = serveur(&["task.started", "task.completed"]).await;
    let reponse = demander(&adresse, "/events", &[]).await;

    assert!(reponse.starts_with("HTTP/1.1 200 OK"), "{reponse}");
    assert!(
        reponse
            .to_lowercase()
            .contains("content-type: text/event-stream"),
        "{reponse}"
    );

    // Le cursor du premier événement, tel que le serveur a dû l'émettre.
    let attendu = Cursor::issue(Collection::Events, 1);
    assert!(reponse.contains(&format!("id: {attendu}")), "{reponse}");
    assert!(reponse.contains("event: task.started"), "{reponse}");
    assert!(reponse.contains("event: task.completed"), "{reponse}");
}

/// **La reprise passe par `Last-Event-ID`**, l'en-tête qu'un navigateur renvoie tout seul.
///
/// C'est ce qui rend la reconnexion automatique : le client n'écrit pas une ligne pour cela.
#[tokio::test]
async fn le_fil_reprend_depuis_last_event_id() {
    let adresse = serveur(&["task.started", "artifact.declared", "task.completed"]).await;

    let apres_le_premier = Cursor::issue(Collection::Events, 1);
    let reponse = demander(
        &adresse,
        "/events",
        &[("Last-Event-ID", apres_le_premier.as_str())],
    )
    .await;

    assert!(
        !reponse.contains("event: task.started"),
        "déjà reçu : {reponse}"
    );
    assert!(reponse.contains("event: artifact.declared"), "{reponse}");
    assert!(reponse.contains("event: task.completed"), "{reponse}");
}

/// **L'en-tête gagne sur le paramètre**, quand les deux sont là.
///
/// La documentation du module l'affirme ; rien ne le vérifiait, et un mutant qui inversait la
/// priorité survivait. L'ordre compte pour de vrai : après une reconnexion automatique, c'est
/// `Last-Event-ID` que le protocole a mis à jour, tandis que le `?cursor=` de l'URL est celui,
/// périmé, avec lequel le client s'était connecté la première fois. Prendre le paramètre ferait
/// donc **rejouer** au client tout ce qu'il a déjà reçu, à chaque reconnexion.
#[tokio::test]
async fn l_en_tete_gagne_sur_le_parametre_quand_les_deux_sont_la() {
    let adresse = serveur(&["task.started", "artifact.declared", "task.completed"]).await;

    let reponse = demander(
        &adresse,
        &format!("/events?cursor={}", Cursor::issue(Collection::Events, 0)),
        &[(
            "Last-Event-ID",
            Cursor::issue(Collection::Events, 2).as_str(),
        )],
    )
    .await;

    assert!(
        !reponse.contains("event: task.started"),
        "l'en-tête dit que les deux premiers sont reçus : {reponse}"
    );
    assert!(
        !reponse.contains("event: artifact.declared"),
        "l'en-tête dit que les deux premiers sont reçus : {reponse}"
    );
    assert!(reponse.contains("event: task.completed"), "{reponse}");
}

// ---------------------------------------------------------------------------------------------
// 2. Les queries de §22.4 se servent par HTTP
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn les_queries_se_servent_par_http() {
    let adresse = serveur(&["task.started"]).await;

    let timeline = demander(&adresse, "/timeline", &[]).await;
    assert!(timeline.starts_with("HTTP/1.1 200 OK"), "{timeline}");
    assert!(timeline.contains("\"items\":["), "{timeline}");
    assert!(timeline.contains("task.started"), "{timeline}");

    let workers = demander(&adresse, "/workers", &[]).await;
    assert!(workers.contains("wrk_01"), "{workers}");

    let statut = demander(&adresse, "/projections/status", &[]).await;
    assert!(statut.contains("\"ready\":true"), "{statut}");
    assert!(statut.contains("execution_graph"), "{statut}");
}

/// **L'en-tête gagne sur le paramètre**, et les deux sont présents pour le vérifier.
///
/// La documentation du module l'affirmait sans que rien ne le tienne : un mutant qui inversait la
/// priorité survivait. Or l'ordre compte après une reconnexion automatique — c'est le navigateur qui
/// met `Last-Event-ID` à jour, pas l'URL, et une URL périmée rejouerait un retard déjà consommé.
#[tokio::test]
async fn last_event_id_gagne_sur_le_parametre_cursor() {
    let adresse = serveur(&["task.started", "artifact.declared", "task.completed"]).await;

    let depuis_le_debut = Cursor::issue(Collection::Events, 0);
    let apres_le_second = Cursor::issue(Collection::Events, 2);

    let reponse = demander(
        &adresse,
        &format!("/events?cursor={depuis_le_debut}"),
        &[("Last-Event-ID", apres_le_second.as_str())],
    )
    .await;

    assert!(
        !reponse.contains("event: task.started"),
        "l'en-tête a été ignoré au profit du paramètre : {reponse}"
    );
    assert!(reponse.contains("event: task.completed"), "{reponse}");
}

/// **`?limit=` est honoré, et le `next` de la page ramène la suite.**
///
/// Deux mutants survivaient ici : l'un ignorait la limite, l'autre rendait toujours `next: null`.
/// Les deux passaient parce qu'aucun test HTTP ne paginait vraiment — il en lisait la première page
/// et s'arrêtait, ce qui est exactement ce qu'un client ne fait pas.
#[tokio::test]
async fn la_pagination_http_honore_la_limite_et_rend_un_cursor_de_suite() {
    let adresse = serveur(&["task.started", "artifact.declared", "task.completed"]).await;

    let premiere = demander(&adresse, "/timeline?limit=1", &[]).await;
    let corps = premiere
        .split("\r\n\r\n")
        .nth(1)
        .expect("la réponse a un corps");
    assert_eq!(
        corps.matches("\"position\"").count(),
        1,
        "la limite n'est pas honorée : {corps}"
    );
    assert!(
        !corps.contains("\"next\":null"),
        "il reste des pages, le cursor de suite doit voyager : {corps}"
    );

    // Le cursor de suite, tel que le client le relirait dans le JSON.
    let suite = corps
        .split("\"next\":\"")
        .nth(1)
        .and_then(|reste| reste.split('"').next())
        .expect("le cursor de suite est là");
    let seconde = demander(&adresse, &format!("/timeline?limit=1&cursor={suite}"), &[]).await;
    assert!(seconde.starts_with("HTTP/1.1 200 OK"), "{seconde}");
    assert!(
        !seconde.contains("task.started"),
        "la seconde page rend la suite, pas le début : {seconde}"
    );
}

/// **La limite demandée est honorée, et le cursor de suite voyage.**
///
/// Deux propriétés que la liaison doit transmettre et que rien ne vérifiait : un mutant qui
/// ignorait `?limit=` survivait, et un autre qui rendait `"next": null` en permanence aussi. La
/// seconde est la plus grave des deux — un client qui ne reçoit jamais de cursor croit avoir tout
/// lu, et s'arrête à la première page sans que rien ne le signale.
#[tokio::test]
async fn la_limite_est_honoree_et_le_cursor_de_suite_voyage() {
    let adresse = serveur(&["task.started", "artifact.declared", "task.completed"]).await;

    let page = demander(&adresse, "/timeline?limit=1", &[]).await;
    let corps = page.split("\r\n\r\n").nth(1).unwrap_or_default().to_owned();

    assert!(corps.contains("task.started"), "{corps}");
    assert!(
        !corps.contains("artifact.declared"),
        "« limit=1 » n'a pas été honoré : {corps}"
    );
    assert!(
        !corps.contains("\"next\":null"),
        "il reste des éléments : le cursor de suite doit voyager. {corps}"
    );

    // Et ce cursor reprend bien où la page s'est arrêtée.
    let suite_cursor = corps
        .split("\"next\":\"")
        .nth(1)
        .and_then(|reste| reste.split('"').next())
        .expect("le corps porte un cursor de suite")
        .to_owned();
    let suite = demander(&adresse, &format!("/timeline?cursor={suite_cursor}"), &[]).await;
    assert!(
        !suite.contains("task.started"),
        "la reprise ne répète pas : {suite}"
    );
    assert!(suite.contains("artifact.declared"), "{suite}");
}

// ---------------------------------------------------------------------------------------------
// 3. Un cursor étranger rend un refus typé, pas un 500
// ---------------------------------------------------------------------------------------------

/// **`400` et la famille `validation` de §22.5**, jamais `500`.
///
/// Un `500` invite au retry — c'est ce qu'il veut dire — et le client retenterait indéfiniment avec
/// le même cursor. Le refus doit lui dire que c'est **sa** requête qu'il faut changer.
#[tokio::test]
async fn un_cursor_etranger_rend_un_refus_type_et_non_un_500() {
    let adresse = serveur(&["task.started"]).await;
    let etranger = Cursor::issue(Collection::Workers, 1);

    let reponse = demander(&adresse, &format!("/timeline?cursor={etranger}"), &[]).await;

    assert!(reponse.starts_with("HTTP/1.1 400 Bad Request"), "{reponse}");
    assert!(!reponse.contains("500"), "{reponse}");
    assert!(reponse.contains("\"family\":\"validation\""), "{reponse}");
    assert!(reponse.contains("\"field\":\"cursor\""), "{reponse}");
    // Le refus nomme les deux collections : un client qui mélange deux paginations doit savoir
    // laquelle il a présentée où.
    assert!(
        reponse.contains("workers") && reponse.contains("timeline"),
        "{reponse}"
    );
}

/// Un cursor illisible aussi — le serveur n'a pas de raison de tomber sur une chaîne quelconque.
#[tokio::test]
async fn un_cursor_illisible_rend_un_refus_type() {
    let adresse = serveur(&["task.started"]).await;
    let reponse = demander(&adresse, "/timeline?cursor=zzzz", &[]).await;

    assert!(reponse.starts_with("HTTP/1.1 400 Bad Request"), "{reponse}");
    assert!(reponse.contains("\"family\":\"validation\""), "{reponse}");
}

// ---------------------------------------------------------------------------------------------
// 4. Ce que la liaison n'expose pas
// ---------------------------------------------------------------------------------------------

/// **Aucune commande de §22.3 n'est servie**, et la raison est dans les types.
///
/// `Transaction::submit` prend `&mut self` ; la couche HTTP ne tient qu'un `&Runtime`. Elle ne peut
/// donc rien muter — non par discipline, mais parce qu'elle n'a pas de quoi.
#[test]
fn la_liaison_http_ne_peut_rien_muter() {
    let source = include_str!("../src/http.rs");
    let code: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    for mutation in ["submit(", "&mut Runtime", "transaction()"] {
        assert!(
            !code.contains(mutation),
            "« {mutation} » : la liaison de W20.g est en lecture seule, et les commandes de §22.3 auront leur item"
        );
    }
}

/// La liste des collections servies est celle du routeur, pas une seconde liste.
#[test]
fn les_collections_servies_sont_celles_du_routeur() {
    let source = include_str!("../src/http.rs");
    for collection in served() {
        assert!(
            source.contains(&format!("/{collection}\"")),
            "« {collection} » est annoncée servie mais n'a pas de route"
        );
    }
}

/// L'écoute par défaut est la boucle locale.
#[test]
fn l_ecoute_par_defaut_ne_sort_pas_de_la_machine() {
    assert!(
        DEFAULT_BIND.starts_with("127.0.0.1:"),
        "« {DEFAULT_BIND} » : un défaut qui expose est un défaut qu'on découvre trop tard, et §22 demande une authentification qui n'existe pas encore"
    );
}
