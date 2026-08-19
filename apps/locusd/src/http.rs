//! La liaison HTTP — `W20.g`. Ce qui traduit, et rien qui décide.
//!
//! # Une surface **en lecture seule**, et c'est délibéré
//!
//! Les routes servies sont celles de §22.4 (queries) et §22.1 (fil d'événements). **Aucune commande
//! de §22.3 n'est exposée**, et la raison est dans les types plutôt que dans une intention :
//! `Transaction::submit` prend `&mut self`, alors qu'un handler `axum` partage son état entre des
//! requêtes concurrentes. Servir une commande demanderait donc de décider comment sérialiser les
//! écritures — un verrou, une file, un acteur — et ce choix mérite son item, pas un coin de
//! celui-ci.
//!
//! Le résultat est agréable et n'est pas un hasard : la couche HTTP ne peut rien muter, parce
//! qu'elle ne tient qu'un `&Runtime`.
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
use axum::extract::{Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use locus_event_store::EventStore;
use serde::Deserialize;

use crate::composition::Runtime;
use crate::cursor::{Collection, Cursor, CursorError};
use crate::error::{CommandError, Family};
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
        .route("/projections/status", get(projections_status::<S>))
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

/// Le collecteur des collections servies, pour un diagnostic.
///
/// Il vit ici, à côté du routeur, parce que c'est le routeur qui décide ce qui est servi : deux
/// listes à deux endroits divergeraient au premier ajout de route.
#[must_use]
pub fn served() -> [&'static str; 5] {
    [
        Collection::Timeline.name(),
        Collection::Workers.name(),
        Collection::Conflicts.name(),
        Collection::Events.name(),
        "projections/status",
    ]
}
