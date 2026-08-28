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
use axum::routing::{get, post, put};
use locus_coordination::version::VersionId;
use locus_event_store::EventStore;
use locus_protocol::{Id, Timestamp};
use serde::{Deserialize, Serialize};

use crate::composition::Runtime;
use crate::cursor::{Collection, Cursor, CursorError};
use crate::enrollment::EnrollmentRequest;
use crate::error::{CommandError, Family};
use crate::lep::{Enrolling, Rendered, Submitted, WorkerSubmission};
use crate::offload::Offload;
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
        .route(GRAPH_PATH, get(epistemic_graph::<S>))
        .route(CLAIM_PATH, post(claim::<S>))
        .route(EVENTS_PATH, post(worker_events::<S>))
        .route(RESULT_PATH, post(result::<S>))
        .route(ENROLL_PATH, post(enroll::<S>))
        .route(PROPOSE_PATH, post(propose::<S>))
        .route(QUEUE_PATH, post(queue::<S>))
        .route(BUILD_VIEW_PATH, post(build_view::<S>))
        .route(VIEW_PATH, get(context_view::<S>))
        .route(DECLARE_PATH, post(declare::<S>))
        .route(CONTENT_PATH, put(upload::<S>))
        .with_state(Offload::new(runtime))
}

/// Le passage obligé vers le daemon — `W20.p`.
///
/// # Pourquoi un passe-plat plutôt qu'un appel direct à `Offload::run`
///
/// Pour que la garde de source ait quelque chose à vérifier. `hors_du_fil` est **le seul** endroit
/// de ce fichier qui touche un [`Runtime`], et un test lit le source pour l'exiger : un handler qui
/// reprendrait l'habitude d'appeler le daemon sur le fil du runtime ferait rougir la CI, au lieu de
/// réintroduire en silence la famine que cet item corrige.
///
/// # Errors
///
/// [`CommandError::Unavailable`] quand la borne d'appels bloquants est franchie — le refus la nomme.
async fn hors_du_fil<S, T, F>(desk: &Offload<S>, work: F) -> Result<T, CommandError>
where
    S: EventStore + Send + Sync + 'static,
    F: FnOnce(&Runtime<S>) -> T + Send + 'static,
    T: Send + 'static,
{
    desk.run(work).await
}

async fn timeline<S: EventStore + Send + Sync + 'static>(
    State(desk): State<Offload<S>>,
    Query(paging): Query<Paging>,
) -> Response {
    let cursor = paging.cursor();
    let limit = paging.limit;
    match hors_du_fil(&desk, move |runtime| {
        runtime.timeline(cursor.as_ref(), limit)
    })
    .await
    {
        Err(sature) => commande_refusee(&sature),
        Ok(Ok(page)) => json_page(&page, |entry| {
            format!(
                "{{\"position\":{},\"event_type\":\"{}\",\"stream_id\":\"{}\"}}",
                entry.position, entry.event_type, entry.stream_id
            )
        }),
        Ok(Err(error)) => refusal(error),
    }
}

async fn workers<S: EventStore + Send + Sync + 'static>(
    State(desk): State<Offload<S>>,
    Query(paging): Query<Paging>,
) -> Response {
    let cursor = paging.cursor();
    let limit = paging.limit;
    match hors_du_fil(&desk, move |runtime| {
        runtime.workers(cursor.as_ref(), limit)
    })
    .await
    {
        Err(sature) => commande_refusee(&sature),
        Ok(Ok(page)) => json_page(&page, |worker| format!("\"{worker}\"")),
        Ok(Err(error)) => refusal(error),
    }
}

async fn conflicts<S: EventStore + Send + Sync + 'static>(
    State(desk): State<Offload<S>>,
    Query(paging): Query<Paging>,
) -> Response {
    let cursor = paging.cursor();
    let limit = paging.limit;
    match hors_du_fil(&desk, move |runtime| {
        runtime.open_conflicts(cursor.as_ref(), limit)
    })
    .await
    {
        Err(sature) => commande_refusee(&sature),
        Ok(Ok(page)) => json_page(&page, |entry| {
            format!(
                "{{\"stream_id\":\"{}\",\"declared_at\":{}}}",
                entry.stream_id, entry.declared_at
            )
        }),
        Ok(Err(error)) => refusal(error),
    }
}

/// `GET /events` — le fil de §22.1, en `text/event-stream`.
///
/// Le cursor arrive par `Last-Event-ID` **ou** par `?cursor=`. Les deux, parce qu'un navigateur
/// envoie le premier sans qu'on le lui demande, tandis qu'un client en ligne de commande ou un test
/// trouve le second plus simple. L'en-tête gagne quand les deux sont là : c'est celui que le
/// protocole gère tout seul, donc celui qui est à jour après une reconnexion automatique.
async fn events<S: EventStore + Send + Sync + 'static>(
    State(desk): State<Offload<S>>,
    headers: axum::http::HeaderMap,
    Query(paging): Query<Paging>,
) -> Response {
    let last_event_id = headers
        .get("last-event-id")
        .and_then(|value| value.to_str().ok())
        .map(|text| Cursor::from_wire(text.to_owned()));
    let cursor = last_event_id.or_else(|| paging.cursor());

    match hors_du_fil(&desk, move |runtime| runtime.events_since(cursor.as_ref())).await {
        Err(sature) => commande_refusee(&sature),
        Ok(Ok(delivery)) => {
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
        Ok(Err(error)) => refusal(error),
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
    State(desk): State<Offload<S>>,
    Path(id): Path<String>,
    Query(paging): Query<Paging>,
) -> Response {
    let stream = format!("branch/{id}");
    let cursor = paging.cursor();
    let limit = paging.limit;
    match hors_du_fil(&desk, move |runtime| {
        runtime.branch_history(&stream, cursor.as_ref(), limit)
    })
    .await
    {
        Err(sature) => commande_refusee(&sature),
        Ok(Ok(page)) => json_page(&page, |entry| {
            format!(
                "{{\"revision\":{},\"event_type\":\"{}\",\"recorded_at\":\"{}\"}}",
                entry.revision, entry.event_type, entry.recorded_at
            )
        }),
        Ok(Err(error)) => refusal(error),
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
    State(desk): State<Offload<S>>,
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

    match hors_du_fil(&desk, move |runtime| {
        runtime.organisation_diff(branche, &depart, &arrivee)
    })
    .await
    {
        Err(sature) => commande_refusee(&sature),
        Ok(Ok(view)) => {
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
        Ok(Err(ReplayError::UnknownVersion { version })) => probleme(
            StatusCode::NOT_FOUND,
            "not_found",
            &format!("aucune version « {version} » dans cette branche"),
        ),
        Ok(Err(ReplayError::Empty)) => probleme(
            StatusCode::NOT_FOUND,
            "not_found",
            "aucune organisation n'a été fondée sur cette branche",
        ),
        // Un stream illisible est une faute du serveur, pas du client : le dire autrement enverrait
        // le client corriger ce qu'il n'a pas écrit.
        Ok(Err(autre)) => probleme(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            &autre.to_string(),
        ),
    }
}

async fn projections_status<S: EventStore + Send + Sync + 'static>(
    State(desk): State<Offload<S>>,
) -> Response {
    let readiness = match hors_du_fil(&desk, Runtime::readiness).await {
        Ok(readiness) => readiness,
        Err(sature) => return commande_refusee(&sature),
    };
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

/// Les deux commandes de §22.3 — `W20.s`. **Hors** de `/lep/`, et ce n'est pas cosmétique.
///
/// `/lep/` est le protocole des **workers** : un worker y réclame, y remonte et y rend. Proposer une
/// tâche et la mettre en file sont des commandes d'**administration**, sous une autorité que la
/// créance d'un worker ne porte pas. Les loger sous le même préfixe aurait suggéré qu'une même
/// créance ouvre les deux, ce qui est exactement ce que `W20.s` rend inexprimable.
pub const PROPOSE_PATH: &str = "/commands/task/propose";
/// Voir [`PROPOSE_PATH`].
pub const QUEUE_PATH: &str = "/commands/task/queue";

/// `POST` — la déclaration d'un artefact, §19.1, `W20.t`.
///
/// **Sous `/lep/`**, contrairement aux deux de §22.3 : c'est un worker qui déclare ce qu'il a
/// produit, sous sa créance de worker, et non un exploitant qui administre.
pub const DECLARE_PATH: &str = "/lep/v1/artifacts";
/// `PUT` — les octets de l'artefact déclaré. Voir [`DECLARE_PATH`].
///
/// Le chemin est construit par [`locusd::artifacts::upload_path`] pour un artefact donné ; ce
/// littéral est le motif qu'`axum` route, et un test vérifie que les deux s'accordent — deux
/// endroits qui construisent le même chemin finissent par en construire deux.
pub const CONTENT_PATH: &str = "/lep/v1/artifacts/{artifact_id}/content";

/// `POST` — la construction d'une `ContextView`, §16.2, `W20.ac`.
///
/// **Sous `/commands/`**, comme les deux de §22.3 : bâtir une vue est un acte d'administration. Un
/// worker qui pourrait s'en construire une choisirait ce qu'il a le droit de savoir, ce qui est
/// exactement l'inverse de l'invariant 11.
pub const BUILD_VIEW_PATH: &str = "/commands/context-view/build";

/// `GET` — la `ContextView` que la mission nomme, §16.2 et §12.3, `W20.ac`.
///
/// # §22.4 ne la liste pas, et cette route existe quand même
///
/// La liste des queries essentielles est courte et ne prétend pas être close — `/events`,
/// `/conflicts` et les quatre chemins `lep/v1` n'y sont pas davantage. Celle-ci est **présupposée**
/// par §12.3, qui exige du worker qu'il vérifie l'empreinte de la vue avant de démarrer : la mission
/// ne porte que `{id, hash}`, donc sans surface qui rende le document, la vérification n'a pas
/// d'objet et le matérialisateur du worker n'a pas de source.
///
/// Hors de `/lep/` : c'est une lecture, comme `/graph/{revision_id}`.
pub const VIEW_PATH: &str = "/context-views/{id}";

/// `GET` — le dossier épistémique d'une conclusion, §9.4, `W20.u`.
///
/// Une lecture, donc hors de `/lep/` **et** hors de `/commands/` : ce n'est ni le protocole des
/// workers, ni une commande d'administration. C'est une query de §22.4, comme `/timeline` et
/// `/conflicts`.
pub const GRAPH_PATH: &str = "/graph/{revision_id}";

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
#[derive(Debug, Default, Deserialize)]
struct WorkerBody {
    /// La clé d'idempotence de §15.2. Absente, une chaîne vide : le worker n'annonce alors aucune
    /// resoumission, et deux envois identiques produisent deux faits — ce qui est ce qu'il a demandé.
    #[serde(default)]
    idempotency_key: String,
    /// Le projet **proposé**, s'il en propose un — `W20.z`.
    ///
    /// Facultatif, et l'était déjà syntaxiquement ; ce qui change est qu'il ne devient plus
    /// obligatoire à la lecture. Le projet d'un fait de worker vient de son **grant** :
    /// `WorkerIdentity` le porte depuis l'enrôlement, et `project_of` confronte ce champ-ci
    /// lorsqu'il est présent, pour refuser une divergence plutôt que de l'ignorer.
    ///
    /// Avant, un worker qui n'en envoyait pas recevait `400`. Il n'avait pas tort de ne pas en
    /// envoyer — c'est l'institution qui décide où un worker écrit (`W20.w`).
    #[serde(default)]
    project_id: Option<String>,
    /// Les événements, sur `/lep/v1/events`.
    #[serde(default)]
    events: Vec<locus_lep::Event>,
    /// Ce que le worker annonce, sur `/lep/v1/claim` — §15.3, `W20.q`.
    ///
    /// Encadré : une variante de `WorkerBody` ne doit pas peser dix fois les autres pour un champ
    /// qui n'est lu que sur un chemin. Absent, la réclamation est refusée en nommant le champ —
    /// jamais servie « au mieux », parce que confier une mission à un hôte dont on ne sait rien est
    /// exactement ce que cet item existe pour empêcher.
    #[serde(default)]
    manifest: Option<Box<locus_lep::CapabilityManifest>>,
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
    /// Ce que le worker envoie **en s'enrôlant** — `W20.w`.
    ///
    /// Le projet n'est pas exigé : il vient du grant, que seul `lep_enroll` peut redeemer. S'il est
    /// tout de même fourni, il est lu pour pouvoir être **confronté** au grant, jamais pour être
    /// utilisé — une proposition qui diverge est refusée plutôt qu'ignorée.
    fn enrolling(&self, now: Timestamp) -> Result<Enrolling, CommandError> {
        let proposed_project = match self.project_id.as_deref() {
            None => None,
            Some(brut) => Some(Id::parse(brut).map_err(|error| CommandError::Validation {
                field: "project_id".to_owned(),
                detail: error.to_string(),
            })?),
        };
        Ok(Enrolling {
            idempotency_key: self.idempotency_key.clone(),
            proposed_project,
            occurred_at: now,
        })
    }

    fn submission(&self, now: Timestamp) -> Result<WorkerSubmission, CommandError> {
        // Un projet **proposé**, jamais exigé — `W20.z`. Celui qui décide est le grant, que
        // `WorkerIdentity` porte depuis l'enrôlement. Ce champ n'existe plus que pour être
        // confronté : un worker qui en enverrait un autre est refusé plutôt qu'ignoré.
        //
        // Il reste **relu** ici même quand il ne décide rien : une chaîne illisible est une erreur
        // de client, et la laisser passer en silence pour la comparer plus loin ferait dire au
        // refus « ce n'est pas ton projet » là où il fallait dire « ce n'est pas un identifiant ».
        let proposed_project = match self.project_id.as_deref() {
            None => None,
            Some(brut) => Some(Id::parse(brut).map_err(|error| CommandError::Validation {
                field: "project_id".to_owned(),
                detail: error.to_string(),
            })?),
        };
        Ok(WorkerSubmission {
            idempotency_key: self.idempotency_key.clone(),
            proposed_project,
            // L'instant de l'acte est celui de la réception : le worker date ses **événements**
            // (§15.6 `occurred_at`), pas son enveloppe, et prendre une date qu'il n'a pas envoyée
            // serait l'inventer. §10.1 garde les deux distincts par `recorded_at`, que le journal
            // pose lui-même.
            occurred_at: now,
        })
    }
}

/// `POST /lep/v1/claim` — §15.2, mode pull. `W20.k`, `W20.q`.
///
/// # `204` n'est pas une erreur, et `503` non plus n'est pas `204`
///
/// Un ordonnanceur sans travail répond `204 No Content`, et le client de `W2.21` en fait un tour
/// `idle`. Répondre `404` ou `503` l'enverrait chercher un lien cassé là où il n'y a que du calme —
/// la séparation de l'ADR 0028 décision 4, tenue ici du côté serveur.
///
/// Depuis `W20.q`, l'autre moitié de cette séparation est servie aussi : un broker injoignable rend
/// `503` — la famille `unavailable` de §22.5 — et **jamais** `204`. « Je n'ai pas pu demander sur
/// quoi ça tournerait » n'est pas « rien pour toi », et un worker qui recevrait `204` attendrait en
/// silence un ordonnanceur qui, lui, avait du travail.
async fn claim<S: EventStore + Send + Sync + 'static>(
    State(desk): State<Offload<S>>,
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
    let credential = credential.to_owned();
    match hors_du_fil(&desk, move |runtime| {
        body.submission(now).and_then(|submitted| {
            runtime.lep_claim(&credential, body.manifest.as_deref(), &submitted, now)
        })
    })
    .await
    {
        Err(sature) => commande_refusee(&sature),
        Ok(Ok(None)) => StatusCode::NO_CONTENT.into_response(),
        Ok(Ok(Some(offer))) => match serde_json::to_string(&offer) {
            Ok(body) => json(StatusCode::OK, body),
            Err(error) => probleme(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                &error.to_string(),
            ),
        },
        Ok(Err(error)) => commande_refusee(&error),
    }
}

/// `POST /lep/v1/events` — §15.6, les événements que le worker fait remonter.
async fn worker_events<S: EventStore + Send + Sync + 'static>(
    State(desk): State<Offload<S>>,
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
    let credential = credential.to_owned();
    match hors_du_fil(&desk, move |runtime| {
        body.submission(now).and_then(|submitted| {
            runtime.lep_events(&credential, body.events.clone(), &submitted, now)
        })
    })
    .await
    {
        Err(sature) => commande_refusee(&sature),
        Ok(Ok(())) => StatusCode::ACCEPTED.into_response(),
        Ok(Err(error)) => commande_refusee(&error),
    }
}

/// `POST /lep/v1/result` — l'achèvement d'une tentative, et le fait que `W23.b` compte.
async fn result<S: EventStore + Send + Sync + 'static>(
    State(desk): State<Offload<S>>,
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
    let credential = credential.to_owned();
    match hors_du_fil(&desk, move |runtime| {
        body.submission(now)
            .and_then(|submitted| runtime.lep_result(&credential, rendered, &submitted, now))
    })
    .await
    {
        Err(sature) => commande_refusee(&sature),
        Ok(Ok(())) => StatusCode::ACCEPTED.into_response(),
        Ok(Err(error)) => commande_refusee(&error),
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
    State(desk): State<Offload<S>>,
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
    let enrolling = match requete.worker.enrolling(maintenant()) {
        Ok(enrolling) => enrolling,
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

    let maintenant = maintenant();
    match hors_du_fil(&desk, move |runtime| {
        runtime.lep_enroll(&requete.request, &endpoint, &enrolling, maintenant)
    })
    .await
    {
        Err(sature) => commande_refusee(&sature),
        Ok(Ok(credential)) => match serde_json::to_string(&credential) {
            Ok(rendu) => json(StatusCode::OK, rendu),
            Err(error) => probleme(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                &error.to_string(),
            ),
        },
        Ok(Err(error)) => commande_refusee(&error),
    }
}

/// Ce qu'une commande porte et que le daemon ne décide pas à sa place — `W20.s`.
///
/// Partagée par les deux commandes de §22.3 et par le dépôt d'octets de §19.1, qui la lit de ses
/// en-têtes plutôt que de son corps — voir [`upload`].
///
/// Elle ne porte **aucune** autorité : ni workspace, ni principal. Les deux viennent du registre
/// d'administration, résolus depuis la créance — un appelant qui les annoncerait écrirait dans le
/// workspace de son choix, au nom de qui il veut. C'est la même règle que `W20.k` applique aux
/// workers, et elle vaut d'autant plus ici que ces commandes **créent** du travail.
#[derive(Debug, Deserialize)]
struct CommandBody {
    /// La clé d'idempotence de §22.5.
    #[serde(default)]
    idempotency_key: String,
    /// Le projet auquel les faits appartiennent.
    #[serde(default)]
    project_id: Option<String>,
}

/// Ce que porte `POST /commands/task/propose`, et rien d'autre.
///
/// # `proposal` n'est **pas** un `Option`
///
/// Une proposition absente n'est pas un cas à gérer : c'est un corps qui n'est pas celui de cette
/// commande. Le rendre obligatoire fait refuser serde, qui nomme le champ manquant lui-même ; le
/// rendre optionnel obligeait chaque handler à s'en souvenir dans un `ok_or_else` — et un `if` que
/// deux handlers doivent tenir se perd. Un passage de mutation l'a montré : effacer la garde de
/// `task_id` ne cassait aucun test, parce que la chaîne vide qui la remplaçait finissait quand même
/// par un refus — plus loin, après une lecture du journal, sous un message qui parlait d'une tâche
/// inexistante au lieu d'un champ absent.
#[derive(Debug, Deserialize)]
struct ProposeBody {
    #[serde(flatten)]
    command: CommandBody,
    /// Encadrée : `Proposal` porte une mission entière, et clippy relève à juste titre qu'une
    /// variante ne doit pas peser dix fois les autres.
    proposal: Box<crate::mission::Proposal>,
}

/// Ce que porte `POST /commands/task/queue`, et rien d'autre.
///
/// **Aucune proposition, aucun état de départ** : les deux se lisent du journal. Les faire renvoyer
/// laisserait mettre en file autre chose que ce qui a été proposé, et déclarer l'état qui arrange —
/// voir [`Runtime::lep_queue`]. Ici ce n'est pas une garde mais une absence de champ : une
/// proposition glissée dans ce corps n'est pas rejetée, elle n'est *lue* nulle part.
#[derive(Debug, Deserialize)]
struct QueueBody {
    #[serde(flatten)]
    command: CommandBody,
    task_id: String,
}

/// Ce que devient un corps que serde refuse — y compris un champ obligatoire absent.
///
/// Le message de serde est repris tel quel plutôt que reformulé : c'est lui qui sait **quel** champ
/// manque, et le paraphraser en « corps invalide » perdrait le seul renseignement dont le client a
/// besoin pour corriger sa requête.
fn corps_illisible(error: &serde_json::Error) -> CommandError {
    CommandError::Validation {
        field: "body".to_owned(),
        detail: format!("corps illisible : {error}"),
    }
}

impl CommandBody {
    fn submitted(&self, now: Timestamp) -> Result<Submitted, CommandError> {
        let project = self
            .project_id
            .as_deref()
            .ok_or_else(|| CommandError::Validation {
                field: "project_id".to_owned(),
                detail: "sans projet, un fait n'a pas d'endroit où appartenir".to_owned(),
            })?;
        Ok(Submitted {
            idempotency_key: self.idempotency_key.clone(),
            project_id: Id::parse(project).map_err(|error| CommandError::Validation {
                field: "project_id".to_owned(),
                detail: error.to_string(),
            })?,
            occurred_at: now,
        })
    }
}

/// `POST /commands/task/propose` — §22.3, `W20.s`.
///
/// # Une créance de worker n'ouvre pas ce chemin
///
/// L'autorité est résolue par [`crate::mission::Administrators`], un registre **distinct** de celui
/// des workers. Une créance de worker n'y figure pas, donc elle n'y résout rien, donc elle est
/// refusée — sans qu'aucune comparaison de rôle n'ait à s'en souvenir. Un worker qui pourrait se
/// créer du travail choisirait le sien.
async fn propose<S: EventStore + Send + Sync + 'static>(
    State(desk): State<Offload<S>>,
    headers: axum::http::HeaderMap,
    body: String,
) -> Response {
    let Some(credential) = bearer(&headers) else {
        return sans_creance();
    };
    let corps: ProposeBody = match serde_json::from_str(&body) {
        Ok(lu) => lu,
        Err(error) => return commande_refusee(&corps_illisible(&error)),
    };
    let credential = credential.to_owned();
    let now = maintenant();
    match hors_du_fil(&desk, move |runtime| {
        let authority = authorite(runtime, &credential)?;
        runtime.lep_propose(
            &corps.proposal,
            authority,
            &corps.command.submitted(now)?,
            now,
        )
    })
    .await
    {
        Err(sature) => commande_refusee(&sature),
        Ok(Ok(())) => StatusCode::ACCEPTED.into_response(),
        Ok(Err(error)) => commande_refusee(&error),
    }
}

/// `POST /commands/task/queue` — §22.3, `W20.s`.
async fn queue<S: EventStore + Send + Sync + 'static>(
    State(desk): State<Offload<S>>,
    headers: axum::http::HeaderMap,
    body: String,
) -> Response {
    let Some(credential) = bearer(&headers) else {
        return sans_creance();
    };
    let corps: QueueBody = match serde_json::from_str(&body) {
        Ok(lu) => lu,
        Err(error) => return commande_refusee(&corps_illisible(&error)),
    };
    let credential = credential.to_owned();
    let now = maintenant();
    match hors_du_fil(&desk, move |runtime| {
        let authority = authorite(runtime, &credential)?;
        runtime.lep_queue(
            &corps.task_id,
            authority,
            &corps.command.submitted(now)?,
            now,
        )
    })
    .await
    {
        Err(sature) => commande_refusee(&sature),
        Ok(Ok(())) => StatusCode::ACCEPTED.into_response(),
        Ok(Err(error)) => commande_refusee(&error),
    }
}

/// Ce que porte `POST /commands/context-view/build`, et rien d'autre.
///
/// `view` y est obligatoire, pour la raison écrite dans [`ProposeBody`] : une demande sans
/// description ne demande rien, et c'est serde qui nomme le champ absent.
#[derive(Debug, Deserialize)]
struct BuildViewBody {
    #[serde(flatten)]
    command: CommandBody,
    /// Encadrée : la demande porte la description de §16.2 **et** ses candidats.
    view: Box<crate::context_view::Requested>,
}

/// `POST /commands/context-view/build` — §16.2, `W20.ac`.
///
/// Rend la vue **scellée**, avec l'empreinte que le worker recalculera : c'est cette valeur-là que
/// la proposition devra annoncer, et la rendre ici évite que l'appelant la devine.
async fn build_view<S: EventStore + Send + Sync + 'static>(
    State(desk): State<Offload<S>>,
    headers: axum::http::HeaderMap,
    body: String,
) -> Response {
    let Some(credential) = bearer(&headers) else {
        return sans_creance();
    };
    let corps: BuildViewBody = match serde_json::from_str(&body) {
        Ok(lu) => lu,
        Err(error) => return commande_refusee(&corps_illisible(&error)),
    };
    let credential = credential.to_owned();
    let now = maintenant();
    match hors_du_fil(&desk, move |runtime| {
        let authority = authorite(runtime, &credential)?;
        runtime.build_context_view(&corps.view, authority, &corps.command.submitted(now)?, now)
    })
    .await
    {
        Err(sature) => commande_refusee(&sature),
        Ok(Ok(vue)) => rendu(StatusCode::CREATED, &vue),
        Ok(Err(error)) => commande_refusee(&error),
    }
}

/// `GET /context-views/{id}` — la vue que la mission nomme, `W20.ac`.
///
/// # Pourquoi `404` ici, alors que `/graph/{revision_id}` rend `200` pour une conclusion vide
///
/// Parce que les deux questions ne sont pas la même. Là-bas, « rien ne soutient cette conclusion »
/// **est** une réponse. Ici, une vue absente n'est pas une vue vide : le worker a reçu un
/// identifiant dans sa mission, et lui rendre `200` avec un document fabriqué lui ferait vérifier
/// une empreinte contre quelque chose que personne n'a bâti. Un `404` dit ce qui est vrai — cet
/// identifiant ne désigne rien ici.
///
/// Aucune créance n'est exigée pour l'instant, comme pour `/graph/{revision_id}` et le fil : §22
/// demande une authentification que ce daemon n'a pas encore, et la simuler sur une seule route
/// donnerait l'illusion d'une garantie. Ce que la route ne fait **pas** est de choisir ce qu'elle
/// montre en fonction de qui demande — la vue est déjà filtrée pour son destinataire au moment où
/// elle est bâtie, et c'est là que l'invariant 11 se joue.
async fn context_view<S: EventStore + Send + Sync + 'static>(
    State(desk): State<Offload<S>>,
    Path(id): Path<String>,
) -> Response {
    match hors_du_fil(&desk, move |runtime| runtime.context_view(&id)).await {
        Err(sature) => commande_refusee(&sature),
        Ok(Ok(Some(vue))) => rendu(StatusCode::OK, &vue),
        Ok(Ok(None)) => probleme(
            StatusCode::NOT_FOUND,
            "validation",
            "aucune vue de contexte sous cet identifiant",
        ),
        Ok(Err(error)) => commande_refusee(&error),
    }
}

/// `GET /graph/{revision_id}` — les six termes de §9.4, `W20.u`.
///
/// # Pourquoi `200` pour une conclusion que rien ne soutient
///
/// Parce que c'est une **réponse**. Un `404` dirait « je ne connais pas cette conclusion » là où le
/// journal dit « rien ne la soutient, personne ne l'a contestée, aucune expérience ne la porte » —
/// et un client qui reçoit un `404` relance sa requête au lieu de lire ce qu'elle lui a appris. Le
/// seul refus de cette route est un identifiant qui n'est pas une révision.
async fn epistemic_graph<S: EventStore + Send + Sync + 'static>(
    State(desk): State<Offload<S>>,
    Path(revision_id): Path<String>,
) -> Response {
    match hors_du_fil(&desk, move |runtime| {
        runtime.epistemic_dossier(&revision_id)
    })
    .await
    {
        Err(sature) => commande_refusee(&sature),
        Ok(Ok(dossier)) => rendu(StatusCode::OK, &dossier),
        Ok(Err(error)) => commande_refusee(&error),
    }
}

/// `POST /lep/v1/artifacts` — §19.1, la déclaration d'un artefact, `W20.t`.
///
/// Rend l'adresse où en déposer le contenu, et jusqu'à quand. Le hash n'est **pas** vérifié ici :
/// il ne peut pas l'être, puisque le contenu n'est pas encore arrivé — c'est tout l'intérêt de
/// déclarer d'abord.
async fn declare<S: EventStore + Send + Sync + 'static>(
    State(desk): State<Offload<S>>,
    headers: axum::http::HeaderMap,
    body: String,
) -> Response {
    let Some(credential) = bearer(&headers) else {
        return sans_creance();
    };
    let corps: DeclareBody = match serde_json::from_str(&body) {
        Ok(lu) => lu,
        Err(error) => return commande_refusee(&corps_illisible(&error)),
    };
    let credential = credential.to_owned();
    let now = maintenant();
    match hors_du_fil(&desk, move |runtime| {
        runtime.lep_declare_artifact(
            &credential,
            &corps.manifest,
            &corps.worker.submission(now)?,
            now,
        )
    })
    .await
    {
        Err(sature) => commande_refusee(&sature),
        Ok(Ok(ticket)) => rendu(StatusCode::ACCEPTED, &ticket),
        Ok(Err(error)) => commande_refusee(&error),
    }
}

/// `PUT /lep/v1/artifacts/{artifact_id}/content` — §19.1, les octets, `W20.t`.
///
/// # Pourquoi les métadonnées voyagent en en-têtes
///
/// Le corps **est** l'artefact. Y glisser un objet JSON qui l'enveloppe obligerait à encoder des
/// octets arbitraires en base64 — un tiers de volume en plus sur des contenus qui se comptent en
/// gigaoctets —, et à tenir le tout en mémoire pour le décoder avant de savoir s'il est
/// acceptable, ce que `packages/artifacts` refuse par construction.
///
/// La clé d'idempotence et le projet passent donc par `Locus-Idempotency-Key` et
/// `Locus-Project-Id`. Ils ne portent aucune autorité — celle-ci vient de la créance, comme
/// partout ailleurs depuis `W20.k`.
async fn upload<S: EventStore + Send + Sync + 'static>(
    State(desk): State<Offload<S>>,
    Path(artifact_id): Path<String>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let Some(credential) = bearer(&headers) else {
        return sans_creance();
    };
    // `W20.z` : les mêmes deux en-têtes, lues comme le corps des autres chemins de worker — une
    // clé d'idempotence, et un projet **proposé** que le grant tranchera.
    let submitted = WorkerBody {
        idempotency_key: entete(&headers, "locus-idempotency-key").unwrap_or_default(),
        project_id: entete(&headers, "locus-project-id"),
        ..WorkerBody::default()
    };
    let credential = credential.to_owned();
    let now = maintenant();
    match hors_du_fil(&desk, move |runtime| {
        runtime.lep_upload_artifact(
            &credential,
            &artifact_id,
            &body,
            &submitted.submission(now)?,
            now,
        )
    })
    .await
    {
        Err(sature) => commande_refusee(&sature),
        Ok(Ok(receipt)) => rendu(StatusCode::CREATED, &receipt),
        Ok(Err(error)) => commande_refusee(&error),
    }
}

/// Un en-tête, s'il est là et lisible.
fn entete(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

/// Rendre une valeur en JSON, ou dire que la sérialisation a échoué.
///
/// Un `500` et non un corps tronqué : une réponse partielle serait lue comme une réponse.
fn rendu<T: Serialize>(status: StatusCode, value: &T) -> Response {
    match serde_json::to_string(value) {
        Ok(body) => json(status, body),
        Err(error) => probleme(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            &error.to_string(),
        ),
    }
}

/// Le corps d'une déclaration d'artefact — `W20.t`.
///
/// Le manifeste y est **obligatoire**, comme la proposition l'est dans [`ProposeBody`] : une
/// déclaration sans manifeste ne déclare rien, et c'est serde qui le refuse en nommant le champ.
#[derive(Debug, Deserialize)]
struct DeclareBody {
    #[serde(flatten)]
    worker: WorkerBody,
    /// Encadré : un `ArtifactManifest` porte licence, dérivations et viewer hints.
    manifest: Box<locus_lep::ArtifactManifest>,
}

/// L'autorité que porte cette créance, ou le refus **typé** qui dit qu'elle n'en porte aucune.
///
/// Jamais une trace : une créance sans autorité est une faute d'autorisation, et lui rendre `500`
/// la ferait retenter à l'identique. La créance refusée n'est pas citée — `CLAUDE.md` interdit de
/// journaliser un jeton, et un message d'erreur la ferait fuir dans le premier rapport de bug venu.
fn authorite<S: EventStore>(
    runtime: &Runtime<S>,
    credential: &str,
) -> Result<crate::mission::Authority, CommandError> {
    runtime
        .lep()
        .administrators()
        .authority(credential)
        .ok_or_else(|| CommandError::Authorization {
            action: "commander §22.3 sans autorité d'administration reconnue".to_owned(),
        })
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
pub fn served() -> [&'static str; 18] {
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
        // Les deux de §22.3 — `W20.s`.
        PROPOSE_PATH,
        QUEUE_PATH,
        // Les deux de §19.1 — `W20.t`.
        DECLARE_PATH,
        CONTENT_PATH,
        // Le graphe épistémique de §9.4 — `W20.u`.
        GRAPH_PATH,
        // La `ContextView` de §16.2, bâtie puis servie — `W20.ac`.
        BUILD_VIEW_PATH,
        VIEW_PATH,
    ]
}
