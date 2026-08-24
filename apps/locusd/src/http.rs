//! La liaison HTTP — `W20.g`. Ce qui traduit, et rien qui décide.
//!
//! # Une surface qui a cessé d'être en lecture seule, et ce que sa raison est devenue
//!
//! `W20.g` n'exposait que §22.4 et §22.1, et le disait ainsi : « `Transaction::submit` prend
//! `&mut self`, alors qu'un handler `axum` partage son état entre des requêtes concurrentes ».
//! C'était vrai, et `W20.h` l'a levé — `submit` prend `&self` depuis l'ADR 0029, et la
//! sérialisation se fait par stream sous [`crate::writes::StreamLocks`].
//!
//! **La phrase a survécu six sprints à la condition qu'elle décrivait**, et elle est corrigée ici
//! plutôt que laissée : un commentaire qui invoque un obstacle levé est une affirmation fausse sur
//! l'état du système, ce que l'ADR 0025 rend coûteux. C'est la deuxième fois dans ce fichier qu'un
//! raisonnement se périme sans que personne le relise — [`branch_history`] porte l'autre.
//!
//! Les trois routes de §15.2 (`W20.k`) écrivent donc, et elles écrivent **par la transaction** :
//! la couche HTTP ne décide toujours rien, elle traduit.
//!
//! # Un refus de cursor est **typé**, jamais un 500
//!
//! Un cursor illisible est une faute du client, pas une panne du serveur. Lui rendre `500` le
//! ferait retenter à l'identique — un `500` invite au retry, c'est même ce qu'il veut dire — et il
//! retenterait indéfiniment avec le même cursor. La réponse est donc `400` avec la famille
//! `validation` de §22.5, qui dit au client que c'est **sa** requête qu'il doit changer.
//!
//! # Ce que cette liaison ne fait pas encore
//!
//! Le fil SSE rend ce que le journal a **au moment de la requête**, puis ferme. Ce n'est pas un flux
//! poussé : un client reçoit son retard, la connexion se termine, et il revient — SSE prescrivant la
//! reconnexion automatique avec `Last-Event-ID`, la reprise fonctionne de bout en bout sans que le
//! client écrive une ligne pour cela. Ce qui manque est le maintien de la connexion et la poussée à
//! l'écriture ; c'est une amélioration de latence, pas une correction de sémantique, et elle
//! demandera un canal de notification que `W20.f` a délibérément laissé de côté.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use locus_coordination::version::VersionId;
use locus_event_store::EventStore;
use locus_protocol::{Id, Timestamp};
use serde::Deserialize;

use crate::composition::Runtime;
use crate::cursor::{Collection, Cursor, CursorError};
use crate::enrollment::EnrollmentRequest;
use crate::error::{CommandError, Family};
use crate::lep::{Rendered, Submitted};
use crate::organisation::ReplayError;
use crate::query::Page;
use crate::stream::Frame;

/// Les paramètres de pagination, tels qu'un client les envoie.
#[derive(Debug, Deserialize)]
pub struct Paging {
    /// Le cursor rendu par la page précédente, tel quel.
    cursor: Option<String>,
    /// Le nombre d'éléments souhaité. Ramené dans ses bornes, jamais refusé.
    limit: Option<usize>,
}

impl Paging {
    fn cursor(&self) -> Option<Cursor> {
        self.cursor
            .as_ref()
            .map(|text| Cursor::from_wire(text.clone()))
    }
}

/// Les routes de lecture de §22.4 et §22.1.
///
/// La table est courte et explicite. Un routeur qui monterait des sous-routeurs par convention
/// rendrait impossible de lire, ici, ce que le daemon expose — et c'est la première question qu'on
/// pose à un service.
pub fn router<S>(runtime: Arc<Runtime<S>>) -> Router
where
    S: EventStore + Send + Sync + 'static,
{
    Router::new()
        .route("/timeline", get(timeline::<S>))
        .route("/workers", get(workers::<S>))
        .route("/conflicts", get(conflicts::<S>))
        .route("/events", get(events::<S>))
        .route("/branches/{id}/history", get(branch_history::<S>))
        .route("/branches/{id}/diff", get(branch_diff::<S>))
        .route("/projections/status", get(projections_status::<S>))
        .route(CLAIM_PATH, post(claim::<S>))
        .route(EVENTS_PATH, post(worker_events::<S>))
        .route(RESULT_PATH, post(result::<S>))
        .route(ENROLL_PATH, post(enroll::<S>))
        .with_state(runtime)
}

async fn timeline<S: EventStore + Send + Sync + 'static>(
    State(runtime): State<Arc<Runtime<S>>>,
    Query(paging): Query<Paging>,
) -> Response {
    match runtime.timeline(paging.cursor().as_ref(), paging.limit) {
        Ok(page) => json_page(&page, |entry| {
            format!(
                "{{\"position\":{},\"event_type\":\"{}\",\"stream_id\":\"{}\"}}",
                entry.position, entry.event_type, entry.stream_id
            )
        }),
        Err(error) => refusal(error),
    }
}

async fn workers<S: EventStore + Send + Sync + 'static>(
    State(runtime): State<Arc<Runtime<S>>>,
    Query(paging): Query<Paging>,
) -> Response {
    match runtime.workers(paging.cursor().as_ref(), paging.limit) {
        Ok(page) => json_page(&page, |worker| format!("\"{worker}\"")),
        Err(error) => refusal(error),
    }
}

async fn conflicts<S: EventStore + Send + Sync + 'static>(
    State(runtime): State<Arc<Runtime<S>>>,
    Query(paging): Query<Paging>,
) -> Response {
    match runtime.open_conflicts(paging.cursor().as_ref(), paging.limit) {
        Ok(page) => json_page(&page, |entry| {
            format!(
                "{{\"stream_id\":\"{}\",\"declared_at\":{}}}",
                entry.stream_id, entry.declared_at
            )
        }),
        Err(error) => refusal(error),
    }
}

/// `GET /events` — le fil de §22.1, en `text/event-stream`.
///
/// Le cursor arrive par `Last-Event-ID` **ou** par `?cursor=`. Les deux, parce qu'un navigateur
/// envoie le premier sans qu'on le lui demande, tandis qu'un client en ligne de commande ou un test
/// trouve le second plus simple. L'en-tête gagne quand les deux sont là : c'est celui que le
/// protocole gère tout seul, donc celui qui est à jour après une reconnexion automatique.
async fn events<S: EventStore + Send + Sync + 'static>(
    State(runtime): State<Arc<Runtime<S>>>,
    headers: axum::http::HeaderMap,
    Query(paging): Query<Paging>,
) -> Response {
    let last_event_id = headers
        .get("last-event-id")
        .and_then(|value| value.to_str().ok())
        .map(|text| Cursor::from_wire(text.to_owned()));
    let cursor = last_event_id.or_else(|| paging.cursor());

    match runtime.events_since(cursor.as_ref()) {
        Ok(delivery) => {
            let mut body = String::new();
            for event in &delivery.events {
                body.push_str(&Frame::event(event));
            }
            if body.is_empty() {
                body.push_str(&Frame::keep_alive());
            }
            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, Frame::CONTENT_TYPE),
                    (header::CACHE_CONTROL, "no-store"),
                ],
                body,
            )
                .into_response()
        }
        Err(error) => refusal(error),
    }
}

/// `GET /projections/status` — §22.4, la santé des projections.
/// `GET /branches/{id}/history` — la navigation dans le temps de `W17.f`, §22.4.
///
/// # Pourquoi celle-ci et pas les trois autres
///
/// `W17.f` a livré six capacités et le ledger annonçait « la liaison HTTP des quatre lectures ».
/// La vérification a démenti le compte : **une seule** des quatre est joignable par un `GET`
/// aujourd'hui, et le dire vaut mieux que d'en câbler trois qui mentiraient.
///
/// - **le diff** et **la preview** prennent deux `&Version`, pas deux identifiants. Rien dans le
///   dépôt ne sait rendre une `Version` depuis une `VersionId` — vérifié, pas supposé : aucune
///   projection n'en tient, et `Version` ne se reconstruit que par `apply` depuis sa racine. Ce
///   qu'il faudrait est un **résolveur de versions**, donc une projection de plus, donc un item ;
///   l'écrire en passant dans une liaison HTTP serait exactement le débordement que `W20.c` a
///   refusé pour le transport. La preview demande en outre les `Barriers` en vigueur, que le
///   composition root ne câble pas.
/// - **l'ombre** n'est pas une lecture : elle prend un plan et un environnement enregistré. Un
///   `GET` ne les porte pas, et lui faire porter un corps de requête en ferait une commande sous un
///   verbe qui promet le contraire.
///
/// L'histoire, elle, ne demande que le stream et un cursor — les deux que le client a déjà.
async fn branch_history<S: EventStore + Send + Sync + 'static>(
    State(runtime): State<Arc<Runtime<S>>>,
    Path(id): Path<String>,
    Query(paging): Query<Paging>,
) -> Response {
    let stream = format!("branch/{id}");
    match runtime.branch_history(&stream, paging.cursor().as_ref(), paging.limit) {
        Ok(page) => json_page(&page, |entry| {
            format!(
                "{{\"revision\":{},\"event_type\":\"{}\",\"recorded_at\":\"{}\"}}",
                entry.revision, entry.event_type, entry.recorded_at
            )
        }),
        Err(error) => refusal(error),
    }
}

/// Les deux bornes d'un diff — §22.4, `GET /branches/{id}/diff?from=&to=`.
///
/// **Les deux sont obligatoires**, et c'est la propriété que `W17.f` a posée : une comparaison sans
/// borne n'est pas une comparaison. Un défaut « depuis le début » aurait rendu un diff plausible à
/// qui a oublié un paramètre.
#[derive(Debug, Deserialize)]
struct Bornes {
    from: Option<String>,
    to: Option<String>,
}

async fn branch_diff<S: EventStore + Send + Sync + 'static>(
    State(runtime): State<Arc<Runtime<S>>>,
    Path(id): Path<String>,
    Query(bornes): Query<Bornes>,
) -> Response {
    let (Some(from), Some(to)) = (bornes.from.as_deref(), bornes.to.as_deref()) else {
        return probleme(
            StatusCode::BAD_REQUEST,
            "validation",
            "« from » et « to » sont requis : une comparaison sans borne n'est pas une comparaison",
        );
    };
    let (Ok(branche), Some(depart), Some(arrivee)) =
        (Id::parse(&id), VersionId::parse(from), VersionId::parse(to))
    else {
        return probleme(
            StatusCode::BAD_REQUEST,
            "validation",
            "identifiant de branche ou de version illisible",
        );
    };

    match runtime.organisation_diff(branche, &depart, &arrivee) {
        Ok(view) => {
            let operations: Vec<String> = view
                .operations
                .iter()
                .map(|operation| format!("\"{}\"", operation.replace('\t', "\\t")))
                .collect();
            json(
                StatusCode::OK,
                format!(
                    "{{\"from\":\"{}\",\"to\":\"{}\",\"operations\":[{}]}}",
                    view.from,
                    view.to,
                    operations.join(",")
                ),
            )
        }
        // Une version inconnue est un `404` : la ressource demandée n'existe pas. Un `400` dirait
        // que la requête est mal écrite, et enverrait le client relire une syntaxe correcte.
        Err(ReplayError::UnknownVersion { version }) => probleme(
            StatusCode::NOT_FOUND,
            "not_found",
            &format!("aucune version « {version} » dans cette branche"),
        ),
        Err(ReplayError::Empty) => probleme(
            StatusCode::NOT_FOUND,
            "not_found",
            "aucune organisation n'a été fondée sur cette branche",
        ),
        // Un stream illisible est une faute du serveur, pas du client : le dire autrement enverrait
        // le client corriger ce qu'il n'a pas écrit.
        Err(autre) => probleme(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            &autre.to_string(),
        ),
    }
}

async fn projections_status<S: EventStore + Send + Sync + 'static>(
    State(runtime): State<Arc<Runtime<S>>>,
) -> Response {
    let readiness = runtime.readiness();
    let lignes: Vec<String> = readiness
        .projections
        .iter()
        .map(|wired| {
            format!(
                "{{\"name\":\"{}\",\"healthy\":{}}}",
                wired.name, wired.healthy
            )
        })
        .collect();
    json(
        StatusCode::OK,
        format!(
            "{{\"ready\":{},\"projections\":[{}]}}",
            readiness.is_ready(),
            lignes.join(",")
        ),
    )
}

/// Une page, avec son cursor de suite.
fn json_page<T>(page: &Page<T>, render: impl Fn(&T) -> String) -> Response {
    let items: Vec<String> = page.items.iter().map(render).collect();
    let next = page
        .next
        .as_ref()
        .map_or_else(|| "null".to_owned(), |cursor| format!("\"{cursor}\""));
    json(
        StatusCode::OK,
        format!("{{\"items\":[{}],\"next\":{next}}}", items.join(",")),
    )
}

/// Un refus de cursor, **typé** et sous sa famille de §22.5.
///
/// `400` et non `500` : un cursor illisible est une faute du client. Un `500` l'inviterait à
/// retenter à l'identique — c'est ce que `500` veut dire — et il retenterait indéfiniment avec le
/// même cursor.
fn refusal(error: CursorError) -> Response {
    let refus = CommandError::Validation {
        field: "cursor".to_owned(),
        detail: error.to_string(),
    };
    debug_assert_eq!(refus.family(), Family::Validation);
    json(
        StatusCode::BAD_REQUEST,
        format!(
            "{{\"family\":\"{}\",\"field\":\"cursor\",\"detail\":\"{}\"}}",
            refus.family(),
            error.to_string().replace('"', "'")
        ),
    )
}

/// Les trois chemins de §15.2, mode pull — les mêmes littéraux que le client de `W2.21`.
///
/// Écrits en clair des deux côtés du fil, et c'est voulu : ce sont la moitié serveur d'un contrat,
/// pas un détail d'implémentation. Les changer casse un worker qu'on ne redéploie pas en même temps,
/// et un diff doit le montrer.
pub const CLAIM_PATH: &str = "/lep/v1/claim";
/// Voir [`CLAIM_PATH`].
pub const EVENTS_PATH: &str = "/lep/v1/events";
/// Voir [`CLAIM_PATH`].
pub const RESULT_PATH: &str = "/lep/v1/result";
/// L'enrôlement de §7.2 — `W20.n`. Le seul chemin qui se parle **sans** créance, par construction :
/// c'est celui par lequel on en obtient une.
pub const ENROLL_PATH: &str = "/lep/v1/enroll";

/// La créance portée par `Authorization: Bearer …`, si elle y est.
///
/// Jamais journalisée, jamais renvoyée dans un refus : `CLAUDE.md` interdit de logger un token, et
/// un message d'erreur qui citerait la créance refusée la ferait fuir dans le premier rapport de bug
/// venu.
fn bearer(headers: &axum::http::HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|credential| !credential.is_empty())
}

/// Ce qu'un worker envoie sur les trois chemins — la part que le daemon ne décide pas à sa place.
#[derive(Debug, Deserialize)]
struct WorkerBody {
    /// La clé d'idempotence de §15.2. Absente, une chaîne vide : le worker n'annonce alors aucune
    /// resoumission, et deux envois identiques produisent deux faits — ce qui est ce qu'il a demandé.
    #[serde(default)]
    idempotency_key: String,
    /// Le projet. Absent, celui-ci est refusé plutôt que deviné.
    #[serde(default)]
    project_id: Option<String>,
    /// Les événements, sur `/lep/v1/events`.
    #[serde(default)]
    events: Vec<locus_lep::Event>,
    /// Sur `/lep/v1/result`.
    #[serde(default)]
    task_id: String,
    #[serde(default)]
    attempt_id: String,
    #[serde(default)]
    session_id: String,
    #[serde(default)]
    output: serde_json::Value,
}

impl WorkerBody {
    /// Ce que le worker a soumis, ou le refus qui nomme le champ manquant.
    fn submitted(&self, now: Timestamp) -> Result<Submitted, CommandError> {
        let project = self
            .project_id
            .as_deref()
            .ok_or_else(|| CommandError::Validation {
                field: "project_id".to_owned(),
                detail: "sans projet, un fait n'a pas d'endroit où appartenir : le deviner \
                         écrirait dans un projet que personne n'a choisi"
                    .to_owned(),
            })?;
        let project_id = Id::parse(project).map_err(|error| CommandError::Validation {
            field: "project_id".to_owned(),
            detail: error.to_string(),
        })?;
        Ok(Submitted {
            idempotency_key: self.idempotency_key.clone(),
            project_id,
            // L'instant de l'acte est celui de la réception : le worker date ses **événements**
            // (§15.6 `occurred_at`), pas son enveloppe, et prendre une date qu'il n'a pas envoyée
            // serait l'inventer. §10.1 garde les deux distincts par `recorded_at`, que le journal
            // pose lui-même.
            occurred_at: now,
        })
    }
}

/// `POST /lep/v1/claim` — §15.2, mode pull. `W20.k`.
///
/// # `204` n'est pas une erreur
///
/// Un ordonnanceur sans travail répond `204 No Content`, et le client de `W2.21` en fait un tour
/// `idle`. Répondre `404` ou `503` l'enverrait chercher un lien cassé là où il n'y a que du calme —
/// la séparation de l'ADR 0028 décision 4, tenue ici du côté serveur.
async fn claim<S: EventStore + Send + Sync + 'static>(
    State(runtime): State<Arc<Runtime<S>>>,
    headers: axum::http::HeaderMap,
    body: String,
) -> Response {
    let Some(credential) = bearer(&headers) else {
        return sans_creance();
    };
    let body = match lu(&body) {
        Ok(body) => body,
        Err(error) => return commande_refusee(&error),
    };
    let now = maintenant();
    match body
        .submitted(now)
        .and_then(|submitted| runtime.lep_claim(credential, &submitted, now))
    {
        Ok(None) => StatusCode::NO_CONTENT.into_response(),
        Ok(Some(offer)) => match serde_json::to_string(&offer) {
            Ok(body) => json(StatusCode::OK, body),
            Err(error) => probleme(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                &error.to_string(),
            ),
        },
        Err(error) => commande_refusee(&error),
    }
}

/// `POST /lep/v1/events` — §15.6, les événements que le worker fait remonter.
async fn worker_events<S: EventStore + Send + Sync + 'static>(
    State(runtime): State<Arc<Runtime<S>>>,
    headers: axum::http::HeaderMap,
    body: String,
) -> Response {
    let Some(credential) = bearer(&headers) else {
        return sans_creance();
    };
    let body = match lu(&body) {
        Ok(body) => body,
        Err(error) => return commande_refusee(&error),
    };
    let now = maintenant();
    match body
        .submitted(now)
        .and_then(|submitted| runtime.lep_events(credential, body.events.clone(), &submitted, now))
    {
        Ok(()) => StatusCode::ACCEPTED.into_response(),
        Err(error) => commande_refusee(&error),
    }
}

/// `POST /lep/v1/result` — l'achèvement d'une tentative, et le fait que `W23.b` compte.
async fn result<S: EventStore + Send + Sync + 'static>(
    State(runtime): State<Arc<Runtime<S>>>,
    headers: axum::http::HeaderMap,
    body: String,
) -> Response {
    let Some(credential) = bearer(&headers) else {
        return sans_creance();
    };
    let body = match lu(&body) {
        Ok(body) => body,
        Err(error) => return commande_refusee(&error),
    };
    let now = maintenant();
    // Aucun `worker_id` : le type n'en porte pas. Ce que le corps annoncerait ne ferait de toute
    // façon pas foi, et l'inexprimable vaut mieux que l'écrasé.
    let rendered = Rendered {
        task_id: body.task_id.clone(),
        attempt_id: body.attempt_id.clone(),
        session_id: body.session_id.clone(),
        output: body.output.clone(),
    };
    match body
        .submitted(now)
        .and_then(|submitted| runtime.lep_result(credential, rendered, &submitted, now))
    {
        Ok(()) => StatusCode::ACCEPTED.into_response(),
        Err(error) => commande_refusee(&error),
    }
}

/// `POST /lep/v1/enroll` — §7.2, `W20.n`.
///
/// # Le seul chemin sans porteur, et ce n'est pas un trou
///
/// Les trois autres exigent `Authorization: Bearer`. Celui-ci ne peut pas : c'est par lui qu'on
/// obtient la créance. Ce qui le protège est la **signature** — la demande est signée par la clé
/// privée du worker, liée à son `worker_id`, à **cet** endpoint et à un nonce à usage unique — et le
/// token d'enrôlement, court-terme et consommé au premier usage.
async fn enroll<S: EventStore + Send + Sync + 'static>(
    State(runtime): State<Arc<Runtime<S>>>,
    headers: axum::http::HeaderMap,
    body: String,
) -> Response {
    let requete: EnrollmentBody = match serde_json::from_str(&body) {
        Ok(lu) => lu,
        Err(error) => {
            return commande_refusee(&CommandError::Validation {
                field: "body".to_owned(),
                detail: format!("corps illisible : {error}"),
            });
        }
    };
    let submitted = match requete.worker.submitted(maintenant()) {
        Ok(submitted) => submitted,
        Err(error) => return commande_refusee(&error),
    };

    // L'endpoint que le worker a signé doit être **celui-ci**. Le lire de l'en-tête `Host` plutôt
    // que d'une configuration : c'est l'adresse à laquelle il a réellement parlé, et une valeur
    // configurée pourrait diverger de celle qui sert.
    let hote = headers
        .get(axum::http::header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let endpoint = format!("http://{hote}");

    match runtime.lep_enroll(&requete.request, &endpoint, &submitted, maintenant()) {
        Ok(credential) => match serde_json::to_string(&credential) {
            Ok(rendu) => json(StatusCode::OK, rendu),
            Err(error) => probleme(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                &error.to_string(),
            ),
        },
        Err(error) => commande_refusee(&error),
    }
}

/// Le corps d'un enrôlement : la demande signée, et ce que tout worker envoie par ailleurs.
#[derive(Debug, Deserialize)]
struct EnrollmentBody {
    #[serde(flatten)]
    worker: WorkerBody,
    #[serde(flatten)]
    request: EnrollmentRequest,
}

/// Lire un corps de worker, ou rendre le refus qui dit pourquoi il n'est pas lisible.
///
/// Écrit à la main plutôt qu'avec l'extracteur `Json` d'`axum` : `dependencies.json` écarte sa
/// feature `json` sous l'ADR 0018 — « `serde_json` suffit » — et l'activer pour trois routes
/// reviendrait sur une décision motivée, en tirant une dépendance de plus pour ce que quatre lignes
/// font ici.
/// Rend le refus **sous sa forme d'erreur de commande**, et non déjà en `Response` : clippy relève
/// à juste titre qu'une `Response` d'axum pèse trop pour voyager dans un `Err`, et la traduction en
/// statut appartient de toute façon à [`commande_refusee`], qui la fait pour les huit familles.
fn lu(body: &str) -> Result<WorkerBody, CommandError> {
    serde_json::from_str::<WorkerBody>(body).map_err(|error| CommandError::Validation {
        field: "body".to_owned(),
        detail: format!("corps illisible : {error}"),
    })
}

/// L'instant courant, et la **première** fois qu'une horloge entre dans `locusd`.
///
/// Jusqu'ici l'instant venait toujours d'un appelant — `Transaction::submit` le prend en paramètre,
/// et `locus-protocol` « ne lit pas l'heure ». Une route n'a pas d'appelant à qui le demander, comme
/// elle n'en avait pas pour les identifiants ; à la différence de ceux-là, l'heure ne coûte aucune
/// dépendance, `std::time` la portant.
///
/// Une horloge qui reculerait avant 1970 rend `0`. Le cas est irréel et la conséquence serait pire
/// que le cas : `expect` ferait tomber le daemon entier pour une horloge mal réglée.
fn maintenant() -> Timestamp {
    Timestamp::from_millis(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| i64::try_from(since.as_millis()).unwrap_or(0)),
    )
}

/// Le refus d'une requête sans porteur — `401`, et sans citer quoi que ce soit.
fn sans_creance() -> Response {
    probleme(
        StatusCode::UNAUTHORIZED,
        "authorization",
        "aucune créance : §15.2 se parle sous `Authorization: Bearer`",
    )
}

/// Un refus de commande, traduit en statut selon sa famille de §22.5.
///
/// # Pourquoi la famille décide du statut, et pas l'inverse
///
/// `W20.a` a rangé les refus en huit familles précisément pour qu'un client sache **quoi faire**.
/// Les aplatir en `400` pour tout ferait retenter à l'identique une saturation qui aurait abouti
/// plus tard, et relire sa requête à qui n'a rien à y corriger.
fn commande_refusee(error: &CommandError) -> Response {
    let status = match error.family() {
        Family::Validation => StatusCode::BAD_REQUEST,
        Family::Authorization => StatusCode::FORBIDDEN,
        Family::Conflict => StatusCode::CONFLICT,
        Family::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        Family::Budget | Family::Policy | Family::Security => StatusCode::UNPROCESSABLE_ENTITY,
        Family::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    };
    probleme(status, error.family().name(), &error.to_string())
}

/// Un refus qui n'est pas celui d'un cursor — même forme, autre famille et autre statut.
///
/// Écrit à côté de [`refusal`] plutôt qu'en le généralisant : `refusal` porte un raisonnement qui
/// lui est propre — pourquoi `400` et jamais `500` pour un cursor — et le fondre dans une fonction
/// à trois paramètres aurait effacé ce raisonnement au profit d'un appelant qui choisit.
fn probleme(status: StatusCode, family: &str, detail: &str) -> Response {
    json(
        status,
        format!(
            "{{\"family\":\"{family}\",\"detail\":\"{}\"}}",
            detail.replace('"', "'")
        ),
    )
}

fn json(status: StatusCode, body: String) -> Response {
    (status, [(header::CONTENT_TYPE, "application/json")], body).into_response()
}

/// L'adresse d'écoute par défaut — la boucle locale, et rien d'autre.
///
/// `127.0.0.1` et non `0.0.0.0` : le profil `personal-local` de `docs/05` n'a aucune raison d'être
/// joignable depuis le réseau, et un défaut qui expose est un défaut qu'on découvre trop tard.
/// Ouvrir au-delà sera une décision explicite, avec l'authentification que §22 demande — laquelle
/// n'existe pas encore.
pub const DEFAULT_BIND: &str = "127.0.0.1:8787";

/// Le collecteur des routes servies, pour un diagnostic.
///
/// Il vit ici, à côté du routeur, parce que c'est le routeur qui décide ce qui est servi : deux
/// listes à deux endroits divergeraient au premier ajout de route.
///
/// # Le déstructurage n'est pas une coquetterie — et il ne suffisait pas
///
/// La liste était écrite à la main, et elle a dérivé au premier ajout : `history` a été servie sans
/// être annoncée, et le test qui lit cette liste est passé au vert — il ne vérifiait qu'un sens,
/// « toute collection annoncée a une route », jamais l'inverse.
///
/// Le déstructurage de `Collection::ALL` a été la réponse, et il rattache **à la compilation** la
/// moitié de la liste qui vient d'une énumération. `W20.k` a montré qu'il n'en tenait que la moitié :
/// `diff` était servie et non annoncée depuis `W17.h`, parce que `diff` n'est pas une `Collection` —
/// elle ne pagine rien. La même dérive, au même endroit, sous la protection d'un correctif qui ne
/// la couvrait pas. C'est la quatrième fois qu'une liste écrite à la main se désynchronise dans ce
/// chantier, après `Family::rang`, `Collection::ALL` et `history`.
///
/// Ce qui la tient maintenant est le **compte**, vérifié par un test qui lit les `.route(` de ce
/// fichier : ajouter une route sans l'annoncer fait rougir, qu'elle soit une `Collection` ou non.
/// Le déstructurage reste, pour ce qu'il couvre — un nom, pas seulement un nombre.
#[must_use]
pub fn served() -> [&'static str; 11] {
    let [timeline, workers, conflicts, events, history] = Collection::ALL.map(Collection::name);
    [
        timeline,
        workers,
        conflicts,
        events,
        history,
        "branches/{id}/diff",
        "projections/status",
        // Les trois de §15.2, lues de leurs constantes : renommer un chemin suit ici tout seul.
        // Ce qu'aucune constante ne tient est l'**oubli** d'une quatrième route, et c'est ce que
        // le compte vérifie.
        CLAIM_PATH,
        EVENTS_PATH,
        RESULT_PATH,
        ENROLL_PATH,
    ]
}
