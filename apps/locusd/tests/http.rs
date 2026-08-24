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
///
/// **Ce test est passé au vert alors que la liste avait dérivé**, et c'est ce qu'il faut retenir de
/// lui : `history` a reçu sa route sans entrer dans `served()`, et la boucle ne vérifiait qu'un
/// sens — « toute collection annoncée a une route », jamais « toute route est annoncée ». Une
/// vérification à sens unique sur deux listes ne les tient pas ensemble, elle en tient une.
///
/// La correction n'est pas ici mais dans `served()`, qui **déstructure** désormais `Collection::ALL`
/// au lieu de la recopier : la liste ne peut plus s'écarter de l'énumération sans que le compilateur
/// le dise, et cette boucle-ci redevient ce qu'elle prétendait être — la vérification que chaque
/// collection de l'énumération a bien sa route.
#[test]
fn les_collections_servies_sont_celles_du_routeur() {
    let source = include_str!("../src/http.rs");
    for collection in served() {
        let litteral = collection.strip_prefix('/').unwrap_or(collection);
        assert!(
            source.contains(&format!("/{litteral}\""))
                || source.contains(&format!(".route({})", nom_de_constante(collection))),
            "« {collection} » est annoncée servie mais n'a pas de route"
        );
    }
}

/// Le nom de la constante qui porte ce chemin, ou une chaîne qui ne peut apparaître nulle part.
///
/// Les trois chemins de §15.2 sont montés depuis leurs constantes plutôt que depuis un littéral :
/// chercher `"/lep/v1/claim"` dans le routeur ne les trouverait pas.
fn nom_de_constante(chemin: &str) -> &'static str {
    match chemin {
        "/lep/v1/claim" => "CLAIM_PATH",
        "/lep/v1/events" => "EVENTS_PATH",
        "/lep/v1/result" => "RESULT_PATH",
        _ => "\u{0}",
    }
}

/// **Et l'autre sens** : toute route montée est annoncée.
///
/// Le test ci-dessus ne vérifiait que « toute annonce a une route », et c'est ce qui a laissé passer
/// `history`, puis `diff`. Le déstructurage de `Collection::ALL` a rattrapé la première et pas la
/// seconde : `diff` ne pagine rien, donc n'est pas une `Collection`, donc échappait au compilateur —
/// servie sans être annoncée depuis `W17.h`, découvert en écrivant `W20.k`.
///
/// Le compte des `.route(` est ce qui tient les deux listes ensemble quelle que soit l'origine du
/// chemin. Il est grossier — il ne dit pas *laquelle* manque — et c'est acceptable : il ne peut pas
/// être vert à tort, et le test précédent nomme celles qui existent.
#[test]
fn toute_route_montee_est_annoncee() {
    let source = include_str!("../src/http.rs");
    // **Les lignes dont le premier caractère non blanc est `.route(`**, et non les occurrences de
    // la chaîne. La première rédaction comptait les secondes, et elle en a trouvé onze pour dix
    // routes : la onzième était dans la documentation de `served()`, qui explique ce test. C'est la
    // douzième fois dans ce chantier qu'une garde mord sur la prose qui la justifie — assez pour
    // que ce soit une règle et non une malchance : un motif qui vise du code se contraint à la
    // **forme** du code, jamais à ses caractères.
    let montees = source
        .lines()
        .filter(|ligne| ligne.trim_start().starts_with(".route("))
        .count();
    assert_eq!(
        montees,
        served().len(),
        "{montees} route(s) montée(s) pour {} annoncée(s) : une liste écrite à la main a encore dérivé",
        served().len()
    );
}

/// L'écoute par défaut est la boucle locale.
#[test]
fn l_ecoute_par_defaut_ne_sort_pas_de_la_machine() {
    assert!(
        DEFAULT_BIND.starts_with("127.0.0.1:"),
        "« {DEFAULT_BIND} » : un défaut qui expose est un défaut qu'on découvre trop tard, et §22 demande une authentification qui n'existe pas encore"
    );
}

// ---------------------------------------------------------------------------------------------
// 8. L'histoire d'une branche se sert par HTTP — et elle est la seule des quatre lectures
// ---------------------------------------------------------------------------------------------

/// Le même écrivain, mais dans le stream d'une branche.
struct EcritDansLaBranche(&'static [&'static str]);

impl Decide for EcritDansLaBranche {
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
                stream_id: "branch/br_01".to_owned(),
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
                payload: serde_json::json!({}),
                payload_hash: format!("sha256:{}", "ab".repeat(32)),
            })
            .collect())
    }
}

async fn serveur_de_branche(types: &'static [&'static str]) -> String {
    let mut runtime = Runtime::in_memory();
    runtime
        .transaction()
        .submit(&EcritDansLaBranche(types), &commande(), &(), NOW)
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

/// **Vérifié sur une réponse réelle**, et sur la **révision de stream**, pas la position globale.
///
/// C'est ce qui distingue cette route de `/timeline` : deux streams ont tous deux une révision 1, et
/// une histoire indexée par la position globale désignerait un tout autre événement. Le test lit donc
/// `"revision":1` — un nombre qu'une timeline ne rendrait pas ici.
#[tokio::test]
async fn l_histoire_d_une_branche_se_sert_par_http() {
    let adresse = serveur_de_branche(&["branch.opened", "branch.validated"]).await;
    let reponse = demander(&adresse, "/branches/br_01/history", &[]).await;

    assert!(reponse.starts_with("HTTP/1.1 200 OK"), "{reponse}");
    assert!(reponse.contains("application/json"), "{reponse}");
    assert!(reponse.contains("\"revision\":1"), "{reponse}");
    assert!(reponse.contains("\"revision\":2"), "{reponse}");
    assert!(reponse.contains("branch.opened"), "{reponse}");
    assert!(reponse.contains("branch.validated"), "{reponse}");
}

/// La pagination est celle de `W20.e`, et le cursor rendu est **un cursor d'histoire**.
///
/// Le représenter à `/timeline` doit être refusé : une position `1` a un sens dans les deux
/// collections, et la lire dans la mauvaise rendrait une page plausible prise au mauvais endroit.
#[tokio::test]
async fn le_cursor_d_histoire_est_refuse_ailleurs() {
    let adresse = serveur_de_branche(&["branch.opened", "branch.validated"]).await;
    let premiere = demander(&adresse, "/branches/br_01/history?limit=1", &[]).await;
    assert!(premiere.contains("\"revision\":1"), "{premiere}");
    assert!(!premiere.contains("\"revision\":2"), "{premiere}");

    let cursor = premiere
        .rsplit_once("\"next\":\"")
        .and_then(|(_, reste)| reste.split_once('"'))
        .map(|(cursor, _)| cursor.to_owned())
        .expect("une page tronquée rend un cursor de suite");

    let suite = demander(
        &adresse,
        &format!("/branches/br_01/history?cursor={cursor}"),
        &[],
    )
    .await;
    assert!(suite.contains("\"revision\":2"), "{suite}");
    // **Et surtout : la suite ne recommence pas.** Un mutant qui ignorait le cursor rendait les deux
    // révisions et passait l'assertion précédente — « contient 2 » est vrai d'une page qui contient
    // aussi 1. Reprendre ne veut rien dire si le test ne regarde pas ce qui a été laissé derrière.
    assert!(!suite.contains("\"revision\":1"), "{suite}");

    // Le même cursor, présenté à la timeline : refusé, et par un 400 typé — pas un 500.
    let ailleurs = demander(&adresse, &format!("/timeline?cursor={cursor}"), &[]).await;
    assert!(ailleurs.starts_with("HTTP/1.1 400"), "{ailleurs}");
    assert!(ailleurs.contains("\"family\":\"validation\""), "{ailleurs}");
}

/// **L'histoire est celle de la branche demandée**, pas d'une branche codée en dur.
///
/// Un mutant qui remplaçait le stream calculé par `branch/br_01` a survécu à la première version de
/// ces tests : ils ne demandaient que `br_01`. Une route paramétrée dont aucun test ne change le
/// paramètre n'est pas une route paramétrée, c'est une constante avec une jolie signature.
#[tokio::test]
async fn l_histoire_ne_sert_que_la_branche_demandee() {
    let adresse = serveur_de_branche(&["branch.opened", "branch.validated"]).await;

    let sienne = demander(&adresse, "/branches/br_01/history", &[]).await;
    assert!(sienne.contains("branch.opened"), "{sienne}");

    // Une autre branche n'a pas d'histoire ici, et la réponse est une page **vide**, pas celle du
    // voisin. Rendre l'histoire d'une autre branche serait le mode d'échec silencieux de §22.6,
    // transposé du cursor au chemin.
    let autre = demander(&adresse, "/branches/br_99/history", &[]).await;
    assert!(autre.starts_with("HTTP/1.1 200 OK"), "{autre}");
    assert!(autre.contains("\"items\":[]"), "{autre}");
    assert!(!autre.contains("branch.opened"), "{autre}");
}

/// **Un cursor illisible sur cette route rend `400`, pas `500`.**
///
/// La règle de `W20.g` vaut route par route et ne s'hérite pas : un mutant qui rendait `500` ici a
/// survécu tant qu'aucun test ne présentait de cursor cassé à `/branches/:id/history`. Un `500`
/// inviterait le client à retenter à l'identique — c'est ce que `500` veut dire — et il retenterait
/// indéfiniment avec le même cursor.
#[tokio::test]
async fn un_cursor_casse_sur_l_histoire_rend_un_refus_type() {
    let adresse = serveur_de_branche(&["branch.opened"]).await;
    let reponse = demander(&adresse, "/branches/br_01/history?cursor=zz", &[]).await;

    assert!(reponse.starts_with("HTTP/1.1 400"), "{reponse}");
    assert!(reponse.contains("\"family\":\"validation\""), "{reponse}");

    // Et un cursor d'une autre collection est refusé en nommant les deux.
    let etranger = Cursor::issue(Collection::Timeline, 1);
    let refus = demander(
        &adresse,
        &format!("/branches/br_01/history?cursor={etranger}"),
        &[],
    )
    .await;
    assert!(refus.starts_with("HTTP/1.1 400"), "{refus}");
    assert!(refus.contains("timeline"), "{refus}");
    assert!(refus.contains("history"), "{refus}");
}

/// **Deux des quatre lectures sont joignables**, et le test tient encore les deux autres par
/// l'absence.
///
/// # Ce que ce test disait, et pourquoi il ne le dit plus
///
/// `W17.g` l'avait écrit pour refuser les **trois** routes restantes, en notant : « il tombera le
/// jour où le résolveur existera, et c'est exactement ce qu'on lui demande ». Ce jour est `W17.j` :
/// `resolve_at` rend une `Version` depuis une `VersionId`, donc `/diff` a de quoi répondre et la
/// propriété d'origine ne s'applique plus à lui.
///
/// Elle s'applique toujours aux deux autres, et pour les raisons qui n'ont pas bougé : la preview
/// demande les `Barriers` en vigueur, que rien ne matérialise depuis le journal ; l'ombre demande un
/// plan et un environnement enregistré, qu'un `GET` ne porte pas. Les câbler maintenant reviendrait
/// à inventer en passant ce qui leur manque.
#[test]
fn les_deux_lectures_de_branche_restantes_ne_sont_pas_cablees() {
    let source = include_str!("../src/http.rs");
    for (route, raison) in [
        (
            "/branches/{id}/preview",
            "la preview demande les `Barriers` en vigueur, que rien ne matérialise depuis le journal",
        ),
        (
            "/branches/{id}/shadow",
            "l'ombre demande un plan et un environnement enregistré, qu'un `GET` ne porte pas",
        ),
    ] {
        assert!(!source.contains(route), "« {route} » : {raison}");
    }
    // Les deux qui le sont, nommées : un test d'absence qui ne dirait pas ce qui est présent
    // passerait aussi sur un routeur vide.
    assert!(source.contains("/branches/{id}/history"));
    assert!(source.contains("/branches/{id}/diff"));
}

// ---------------------------------------------------------------------------------------------
// 6. `/branches/{id}/diff` — W17.j, et c'est la route qui a motivé toute la chaîne
// ---------------------------------------------------------------------------------------------

/// Un daemon dont une branche a une organisation fondée puis modifiée.
///
/// Rend l'adresse et les deux `VersionId` — la racine et l'état final — parce qu'un test qui
/// fabriquerait ses bornes ne prouverait pas qu'elles viennent du journal.
async fn serveur_avec_organisation() -> (String, String, String) {
    use locus_coordination::CoordinationMode;
    use locus_coordination::version::{ContentDigest, Operation, Version};
    use locusd::{Commit, Create, OrganisationContext};

    let branche = id::<locus_protocol::id::Branch>(9);
    let contexte = |seed: u8| OrganisationContext {
        branch_id: branche,
        project_id: id::<Project>(4),
        event_id: id::<Event>(seed),
        occurred_at: NOW,
        payload_hash: format!("sha256:{}", "ab".repeat(32)),
    };
    let commande_org = |seed: u8, revision: u64| {
        CommandEnvelope::mutating(
            id::<Command>(seed),
            "team.modify",
            id::<Workspace>(2),
            id::<Agent>(3),
            format!("org-{seed}"),
            Revision::new(revision),
        )
        .expect("commande bien formée")
    };

    let racine = Version::root(
        &[id::<Agent>(1), id::<Agent>(2)],
        &[],
        CoordinationMode::Blackboard,
        None,
        &ContentDigest,
    )
    .expect("fixture cohérente");
    let apres = racine
        .apply(&Operation::AddNode(id::<Agent>(3)), &ContentDigest)
        .expect("licite");

    let mut runtime = Runtime::in_memory();
    runtime
        .transaction()
        .submit(
            &Create {
                root: racine.clone(),
                digest: ContentDigest,
            },
            &commande_org(1, 0),
            &contexte(1),
            NOW,
        )
        .accepted()
        .expect("la fondation passe");
    runtime
        .transaction()
        .submit(
            &Commit {
                base: racine.clone(),
                operation: Operation::AddNode(id::<Agent>(3)),
                digest: ContentDigest,
            },
            &commande_org(2, 1),
            &contexte(2),
            NOW,
        )
        .accepted()
        .expect("le commit passe");
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

    (adresse, racine.id().to_string(), apres.id().to_string())
}

/// **Le diff se sert par HTTP, et il porte la nature des opérations.**
///
/// C'est la propriété que deux mutants de `W17.f` avaient traversée : rendre « 47 changements »
/// sans dire lesquels laisse un approbateur signer au lieu d'approuver. Le test lit donc la forme
/// canonique de l'opération dans la réponse, et pas seulement un compte.
#[tokio::test]
async fn le_diff_d_une_branche_se_sert_par_http_avec_la_nature_des_operations() {
    let (adresse, racine, apres) = serveur_avec_organisation().await;
    let branche = id::<locus_protocol::id::Branch>(9);

    let reponse = demander(
        &adresse,
        &format!("/branches/{branche}/diff?from={racine}&to={apres}"),
        &[],
    )
    .await;

    assert!(reponse.starts_with("HTTP/1.1 200 OK"), "{reponse}");
    assert!(reponse.contains(&racine), "la borne de départ est nommée");
    assert!(reponse.contains(&apres), "la borne d'arrivée est nommée");
    assert!(
        reponse.contains("ADD_NODE"),
        "la **nature** de l'opération, pas seulement un compte : {reponse}"
    );
}

/// **Une version inconnue rend `404`, jamais une racine plausible.**
///
/// `W17.j` l'interdit nommément, et le mode d'échec est celui des cursors de `W20.e` : une réponse
/// plausible prise au mauvais endroit, que rien dans la réponse ne signale.
#[tokio::test]
async fn une_version_inconnue_rend_404_et_non_une_racine_plausible() {
    let (adresse, racine, _) = serveur_avec_organisation().await;
    let branche = id::<locus_protocol::id::Branch>(9);
    let inconnue = format!("sha256:{}", "cd".repeat(32));

    let reponse = demander(
        &adresse,
        &format!("/branches/{branche}/diff?from={racine}&to={inconnue}"),
        &[],
    )
    .await;

    assert!(reponse.starts_with("HTTP/1.1 404"), "{reponse}");
    assert!(reponse.contains("not_found"), "{reponse}");
    // Et surtout : aucune opération n'est rendue. Un diff vide serait la réponse plausible.
    assert!(
        !reponse.contains("operations"),
        "une version inconnue ne rend pas un diff : {reponse}"
    );
}

/// **Une comparaison sans borne n'est pas une comparaison** — `400`, et non un diff « depuis le
/// début ».
#[tokio::test]
async fn un_diff_sans_bornes_est_refuse() {
    let (adresse, racine, _) = serveur_avec_organisation().await;
    let branche = id::<locus_protocol::id::Branch>(9);

    for cible in [
        format!("/branches/{branche}/diff"),
        format!("/branches/{branche}/diff?from={racine}"),
    ] {
        let reponse = demander(&adresse, &cible, &[]).await;
        assert!(
            reponse.starts_with("HTTP/1.1 400"),
            "« {cible} » : {reponse}"
        );
        assert!(reponse.contains("validation"), "{reponse}");
    }
}
