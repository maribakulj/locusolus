//! Le test de sortie de `W20.u` — les six termes de §9.4, servis.
//!
//! # Ce qui manquait, et comment on l'a su
//!
//! Vérifié au code en tentant `W12.d` : `served()` listait sept routes et **aucune n'était le
//! graphe** ; `apps/locusd/Cargo.toml` ne dépendait pas de `packages/graph` ; et **aucune
//! projection ne portait le coût** — `grep` sur `packages/projections/src` rendait zéro fichier.
//! La clause la plus longue du test de sortie de `W12.d` — « le graphe rend la conclusion, ses
//! prémisses, son expérience, ses artefacts, ses objections et son coût » — n'avait donc de sujet
//! pour aucun de ses six termes.
//!
//! # Les quatre clauses
//!
//! 1. les six se lisent par une query de §22.4, **depuis le journal seul**, sans instantané reçu
//!    d'un worker ;
//! 2. une conclusion sans prémisse se lit **comme telle** plutôt que de manquer ;
//! 3. les objections y sont — invariant 12 ;
//! 4. le coût est **absent** tant que personne ne l'a relevé, jamais nul.

use std::fmt::Write as _;
use std::sync::Arc;

use locus_domain::RevisionId;
use locus_domain::RevisionKind;
use locus_event_store::{Actor, ActorKind, Draft as EventDraft, EventType};
use locus_protocol::id::{Agent, Command, Event as EventId, Project, Workspace};
use locus_protocol::{Id, IdKind, Timestamp};
use locusd::http::{GRAPH_PATH, router};
use locusd::{CommandEnvelope, Decide, Revision, Runtime};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const NOW: Timestamp = Timestamp::from_millis(1_700_000_000_000);
const TACHE: &str = "tsk_catalyseur";

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(NOW, entropy).expect("l'instant de fixture tient sur 48 bits")
}

/// Une révision de fixture, par graine.
fn revision(seed: u8) -> RevisionId {
    id::<RevisionKind>(seed)
}

/// Le décideur d'épreuve : il écrit exactement le fait qu'on lui donne.
///
/// Écrire **par la transaction** et non en poussant dans le journal : `W20.b` en a fait le seul
/// chemin d'écriture, et un test qui le contournerait éprouverait une projection sur des faits que
/// le daemon ne saurait pas produire.
struct Ecrire(EventDraft);

impl Decide for Ecrire {
    type State = ();

    fn decide(
        &self,
        _: &CommandEnvelope,
        (): &Self::State,
    ) -> Result<Vec<EventDraft>, locusd::CommandError> {
        Ok(vec![self.0.clone()])
    }
}

fn fait(seed: u8, stream: &str, event_type: &str, payload: serde_json::Value) -> EventDraft {
    EventDraft {
        event_id: id::<EventId>(seed),
        event_type: EventType::parse(event_type).expect("type de §10.3"),
        schema_version: 1,
        stream_id: stream.to_owned(),
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
        causation_id: id::<Command>(1),
        correlation_id: None,
        trace_id: None,
        payload,
        payload_hash: String::new(),
    }
}

/// Écrire un fait dans le journal du daemon, par le chemin ordinaire.
fn ecrire(runtime: &Runtime<locus_event_store::MemoryEventStore>, seed: u8, draft: EventDraft) {
    let commande = CommandEnvelope::mutating(
        id::<Command>(seed),
        "worker.report",
        id::<Workspace>(2),
        id::<Agent>(3),
        format!("idem-{seed}"),
        Revision::new(
            locus_event_store::EventStore::revision(
                runtime.transaction().store(),
                &draft.stream_id,
            )
            .unwrap_or(0),
        ),
    )
    .expect("enveloppe valide");
    runtime
        .commit(&Ecrire(draft), &commande, &(), NOW)
        .accepted()
        .expect("le fait est écrit");
}

/// Un commit épistémique, tel que `W20.r` l'écrit — inférences et objections comprises.
fn commit(inferences: &serde_json::Value, objections: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "task_id": TACHE,
        "worker_id": "canterel-vm-linux-01",
        "status": "staged",
        "validation_state": { "ready": true },
        "commit": {
            "protocol": "lep/1.0",
            "task_id": TACHE,
            "attempt": 2,
            "status": "staged",
            "produced_at": "2026-08-24T12:00:00.000Z",
            "inferences": inferences,
            "objections": objections,
        },
    })
}

fn inference_nominale() -> serde_json::Value {
    serde_json::json!([{
        "rule": "si la cinétique tient à trois températures, le catalyseur est actif",
        "inference_kind": "induction",
        "premise_refs": [revision(11).to_string(), revision(12).to_string(), revision(13).to_string()],
        "conclusion_refs": [revision(20).to_string()],
    }])
}

fn daemon() -> Runtime<locus_event_store::MemoryEventStore> {
    Runtime::in_memory()
}

async fn servir(runtime: Runtime<locus_event_store::MemoryEventStore>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("la boucle locale accepte un port libre");
    let adresse = listener.local_addr().expect("adresse connue").to_string();
    let app = router(Arc::new(runtime));
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    adresse
}

async fn demander(adresse: &str, cible: &str) -> String {
    let mut flux = TcpStream::connect(adresse).await.expect("le daemon écoute");
    let mut requete = String::new();
    let _ = write!(
        requete,
        "GET {cible} HTTP/1.1\r\nHost: {adresse}\r\nConnection: close\r\n\r\n"
    );
    flux.write_all(requete.as_bytes())
        .await
        .expect("la requête part");
    let mut reponse = Vec::new();
    flux.read_to_end(&mut reponse)
        .await
        .expect("la réponse revient");
    String::from_utf8_lossy(&reponse).into_owned()
}

fn corps_de(reponse: &str) -> serde_json::Value {
    let corps = reponse
        .split_once("\r\n\r\n")
        .map_or("", |(_, corps)| corps);
    serde_json::from_str(corps).unwrap_or_else(|_| panic!("réponse JSON lisible :\n{reponse}"))
}

async fn dossier(adresse: &str, conclusion: &RevisionId) -> serde_json::Value {
    let reponse = demander(adresse, &format!("/graph/{conclusion}")).await;
    assert!(reponse.starts_with("HTTP/1.1 200"), "{reponse}");
    corps_de(&reponse)
}

// ---------------------------------------------------------------------------------------------
// 1. Les six termes, sur le fil, depuis le journal seul.
// ---------------------------------------------------------------------------------------------

/// **Les six termes de §9.4 se lisent par une query, et ils viennent du journal.**
///
/// C'est le test de sortie. Rien n'est injecté dans la projection : trois faits sont écrits par la
/// transaction — un commit épistémique, un artefact, une consommation de budget —, et les six
/// termes en sortent. Un instantané reçu d'un worker ferait de son transcript la vérité
/// institutionnelle, ce que l'invariant 2 réserve au journal ; il n'y a ici aucun chemin pour en
/// fournir un.
#[tokio::test]
async fn les_six_termes_se_lisent_du_journal() {
    let runtime = daemon();
    ecrire(
        &runtime,
        10,
        fait(
            10,
            &format!("task/{TACHE}"),
            "epistemic_object.staged",
            commit(
                &inference_nominale(),
                &serde_json::json!([{
                    "statement": "la troisième température sort du domaine de validité",
                    "targets": [revision(20).to_string()],
                }]),
            ),
        ),
    );
    ecrire(
        &runtime,
        11,
        fait(
            11,
            "artifact/artifact-figure-3",
            "artifact.uploaded",
            serde_json::json!({
                "artifact_id": "artifact-figure-3",
                "state": "uploaded",
                "produced_by": { "task_id": TACHE, "attempt": 2 },
            }),
        ),
    );
    ecrire(
        &runtime,
        12,
        fait(
            12,
            &format!("budget/{TACHE}"),
            "budget.consumed",
            serde_json::json!({
                "task_id": TACHE,
                "amounts": { "tokens": 4_200, "model_calls": 7 },
            }),
        ),
    );

    let adresse = servir(runtime).await;
    let vu = dossier(&adresse, &revision(20)).await;

    // 1. la conclusion
    assert_eq!(vu["conclusion"], revision(20).to_string());
    // 2. ses prémisses — **un ensemble**, pas trois soutiens
    assert_eq!(
        vu["premise_sets"],
        serde_json::json!([[
            revision(11).to_string(),
            revision(12).to_string(),
            revision(13).to_string()
        ]]),
        "trois prémisses font une inférence, jamais trois"
    );
    // 3. son expérience
    assert_eq!(
        vu["experiments"],
        serde_json::json!([{ "task_id": TACHE, "attempt": 2 }])
    );
    // 4. ses artefacts
    assert_eq!(
        vu["artifacts"],
        serde_json::json!([{ "artifact_id": "artifact-figure-3", "state": "uploaded" }])
    );
    // 5. ses objections
    assert_eq!(
        vu["objections"][0]["statement"],
        "la troisième température sort du domaine de validité"
    );
    // 6. son coût
    assert_eq!(vu["cost"]["consumed"]["tokens"], 4_200);
    assert_eq!(vu["cost"]["consumed"]["model_calls"], 7);
    assert_eq!(vu["cost"]["entries"], 1);
}

/// **Trois prémisses font un ensemble de trois, pas trois ensembles d'une.**
///
/// La différence est celle entre « il faut ces trois faits » et « il suffit d'un des trois », et
/// elle décide de ce qu'il advient quand on en réfute un. Elle est tenue par
/// `locus_graph::Graph`, qui n'offre aucun chemin de l'hyperarête vers des arêtes — pas par la
/// vigilance de qui relit cette projection.
#[tokio::test]
async fn deux_inferences_font_deux_ensembles_et_pas_une_liste_plate() {
    let runtime = daemon();
    ecrire(
        &runtime,
        10,
        fait(
            10,
            &format!("task/{TACHE}"),
            "epistemic_object.staged",
            commit(
                &serde_json::json!([
                    {
                        "rule": "voie cinétique",
                        "premise_refs": [revision(11).to_string(), revision(12).to_string()],
                        "conclusion_refs": [revision(20).to_string()],
                    },
                    {
                        "rule": "voie spectroscopique",
                        "premise_refs": [revision(13).to_string()],
                        "conclusion_refs": [revision(20).to_string()],
                    }
                ]),
                &serde_json::json!([]),
            ),
        ),
    );

    let adresse = servir(runtime).await;
    let vu = dossier(&adresse, &revision(20)).await;

    let ensembles = vu["premise_sets"].as_array().expect("des ensembles");
    assert_eq!(ensembles.len(), 2, "deux inférences, deux ensembles");
    let tailles: Vec<usize> = ensembles
        .iter()
        .map(|set| set.as_array().expect("un ensemble").len())
        .collect();
    assert_eq!(
        tailles,
        vec![2, 1],
        "les deux voies gardent leur arité : aplaties, elles auraient rendu un ensemble de trois, \
         c'est-à-dire « il faut ces trois faits » là où le journal dit « l'une ou l'autre suffit »"
    );
}

// ---------------------------------------------------------------------------------------------
// 2. Une conclusion sans prémisse se lit comme telle.
// ---------------------------------------------------------------------------------------------

/// **Une conclusion que rien ne soutient rend `200` et une liste vide, jamais `404`.**
///
/// Un `404` dirait « je ne connais pas cette conclusion » là où le journal dit « rien ne la
/// soutient » — deux choses différentes, et un client qui reçoit un `404` relance sa requête au
/// lieu de lire ce qu'elle lui a appris.
#[tokio::test]
async fn une_conclusion_sans_premisse_se_lit_comme_telle() {
    let adresse = servir(daemon()).await;

    let vu = dossier(&adresse, &revision(99)).await;

    assert_eq!(vu["conclusion"], revision(99).to_string());
    assert_eq!(vu["premise_sets"], serde_json::json!([]));
    assert_eq!(vu["objections"], serde_json::json!([]));
    assert_eq!(vu["experiments"], serde_json::json!([]));
}

/// **Un identifiant qui n'est pas une révision est le seul refus de cette route.**
#[tokio::test]
async fn un_identifiant_illisible_est_refuse_en_nommant_le_champ() {
    let adresse = servir(daemon()).await;

    let reponse = demander(&adresse, "/graph/pas-une-revision").await;

    assert!(reponse.starts_with("HTTP/1.1 400"), "{reponse}");
    assert!(reponse.contains("revision_id"), "{reponse}");
}

/// **Une inférence dont une référence est illisible n'entre pas amputée — elle se lit comme
/// illisible.**
///
/// Entrer avec deux prémisses sur trois ferait paraître la conclusion soutenue par un raisonnement
/// que personne n'a posé ; réfuter les deux qui restent laisserait croire la troisième encore
/// debout. La jeter, à l'inverse, effacerait un raisonnement pour rendre le graphe propre —
/// invariant 12. Elle est donc rangée à part, sous son nom.
#[tokio::test]
async fn une_inference_a_reference_illisible_ne_soutient_ni_ne_disparait() {
    let runtime = daemon();
    ecrire(
        &runtime,
        10,
        fait(
            10,
            &format!("task/{TACHE}"),
            "epistemic_object.staged",
            commit(
                &serde_json::json!([{
                    "rule": "voie cinétique",
                    "premise_refs": [revision(11).to_string(), "prémisse-écrite-à-la-main"],
                    "conclusion_refs": [revision(20).to_string()],
                }]),
                &serde_json::json!([]),
            ),
        ),
    );

    let adresse = servir(runtime).await;
    let vu = dossier(&adresse, &revision(20)).await;

    assert_eq!(
        vu["premise_sets"],
        serde_json::json!([]),
        "elle ne soutient rien : une prémisse sur deux n'est pas l'inférence qui a été posée"
    );
    assert_eq!(
        vu["unreadable"][0]["rule"], "voie cinétique",
        "et elle n'a pas disparu :\n{vu}"
    );
    assert!(
        vu["unreadable"][0]["refs"]
            .as_array()
            .expect("les références brutes")
            .contains(&serde_json::json!("prémisse-écrite-à-la-main")),
        "la référence fautive est citée telle qu'elle a été écrite :\n{vu}"
    );
}

// ---------------------------------------------------------------------------------------------
// 3. Les objections y sont — invariant 12.
// ---------------------------------------------------------------------------------------------

/// **Une objection qui vise l'inférence, et non la conclusion, est rendue quand même.**
///
/// « La règle est fausse » ne vise pas la conclusion et la conteste pourtant — c'est exactement
/// pourquoi §7.6 fait de l'inférence un nœud. Sur trois arêtes indépendantes, cette objection
/// n'aurait aucun endroit où se poser ; ici elle en a un, et le dossier de la conclusion doit la
/// voir. Un dossier qui ne montrerait que les objections visant la conclusion elle-même serait
/// « propre » au sens que l'invariant 12 refuse.
#[tokio::test]
async fn une_objection_a_l_inference_apparait_au_dossier_de_la_conclusion() {
    let runtime = daemon();
    ecrire(
        &runtime,
        10,
        fait(
            10,
            &format!("task/{TACHE}"),
            "epistemic_object.staged",
            commit(&inference_nominale(), &serde_json::json!([])),
        ),
    );
    // L'identité de l'inférence vient de sa position au journal et de son rang dans le commit :
    // elle est donc stable à la reconstruction, et une objection peut la viser.
    let cible = runtime
        .with_epistemic_graph(|graph| graph.dossier(&revision(20)).premise_sets.len().to_string());
    assert_eq!(cible, "1", "l'inférence est bien entrée");

    ecrire(
        &runtime,
        11,
        fait(
            11,
            &format!("task/{TACHE}"),
            "epistemic_object.staged",
            commit(
                &serde_json::json!([]),
                &serde_json::json!([{
                    "statement": "l'induction ne vaut pas hors du domaine mesuré",
                    "targets": ["inference/1/0"],
                }]),
            ),
        ),
    );

    let adresse = servir(runtime).await;
    let vu = dossier(&adresse, &revision(20)).await;

    assert_eq!(
        vu["objections"][0]["statement"], "l'induction ne vaut pas hors du domaine mesuré",
        "une objection à la règle vise l'inférence, pas la conclusion — et le dossier la voit :\n{vu}"
    );
}

/// **Une objection dont la cible ne se relit pas reste une objection.**
///
/// La refuser reviendrait à taire une contestation parce qu'elle est mal adressée. L'invariant 12
/// interdit cela plus fermement que n'importe quelle règle de forme : elle n'apparaît pas au
/// dossier de la conclusion — elle ne la vise pas —, mais elle est dans le graphe et se compte.
#[tokio::test]
async fn une_objection_mal_adressee_n_est_pas_supprimee() {
    let runtime = daemon();
    ecrire(
        &runtime,
        10,
        fait(
            10,
            &format!("task/{TACHE}"),
            "epistemic_object.staged",
            commit(
                &serde_json::json!([]),
                &serde_json::json!([{
                    "statement": "je conteste, mais je ne sais pas quoi nommer",
                    "targets": ["quelque-chose-qui-n-existe-pas"],
                }]),
            ),
        ),
    );

    assert_eq!(
        runtime.with_epistemic_graph(|graph| graph.objections().len()),
        1,
        "l'objection est gardée"
    );
    let adresse = servir(runtime).await;
    assert_eq!(
        dossier(&adresse, &revision(20)).await["objections"],
        serde_json::json!([]),
        "et elle ne se rattache pas à une conclusion qu'elle ne vise pas"
    );
}

// ---------------------------------------------------------------------------------------------
// 4. Le coût est absent, jamais nul.
// ---------------------------------------------------------------------------------------------

/// **Sans écriture de consommation, le coût est `null` — jamais zéro.**
///
/// Un zéro dirait « cette recherche n'a rien coûté », ce qui est une affirmation, et fausse : une
/// exécution coûte toujours quelque chose. Ce que le journal permet de dire est « personne n'a
/// compté ».
#[tokio::test]
async fn sans_releve_le_cout_est_absent_et_non_nul() {
    let runtime = daemon();
    ecrire(
        &runtime,
        10,
        fait(
            10,
            &format!("task/{TACHE}"),
            "epistemic_object.staged",
            commit(&inference_nominale(), &serde_json::json!([])),
        ),
    );

    let adresse = servir(runtime).await;
    let vu = dossier(&adresse, &revision(20)).await;

    assert!(
        vu["cost"].is_null(),
        "absent, et non un total de zéro :\n{vu}"
    );
    assert!(
        !vu["premise_sets"]
            .as_array()
            .expect("des ensembles")
            .is_empty(),
        "le reste du dossier est bien là — c'est le coût seul qui manque"
    );
}

/// **Une réservation n'est pas une consommation, et ne coûte rien.**
///
/// §7.2 énumère six sortes d'écritures. Une réservation est de l'argent **tenu**, pas dépensé ;
/// l'additionner ferait payer deux fois ce qui sera consommé ensuite. Le coût reste donc absent
/// tant qu'aucune consommation n'a été écrite.
#[tokio::test]
async fn une_reservation_ne_fait_pas_un_cout() {
    let runtime = daemon();
    ecrire(
        &runtime,
        10,
        fait(
            10,
            &format!("task/{TACHE}"),
            "epistemic_object.staged",
            commit(&inference_nominale(), &serde_json::json!([])),
        ),
    );
    ecrire(
        &runtime,
        11,
        fait(
            11,
            &format!("budget/{TACHE}"),
            "budget.reserved",
            serde_json::json!({
                "task_id": TACHE,
                "amounts": { "tokens": 100_000 },
            }),
        ),
    );

    let adresse = servir(runtime).await;
    assert!(
        dossier(&adresse, &revision(20)).await["cost"].is_null(),
        "réserver n'est pas dépenser"
    );
}

/// **Deux consommations s'additionnent par dimension, et le registre les compte.**
///
/// §7.2 : « le budget est un registre, pas un compteur mutable isolé ». Le nombre d'écritures est
/// donc rendu avec les sommes — un total sans son nombre d'écritures ne se rapproche d'aucun
/// relevé.
#[tokio::test]
async fn deux_consommations_s_additionnent_par_dimension() {
    let runtime = daemon();
    ecrire(
        &runtime,
        10,
        fait(
            10,
            &format!("task/{TACHE}"),
            "epistemic_object.staged",
            commit(&inference_nominale(), &serde_json::json!([])),
        ),
    );
    for (seed, tokens) in [(11_u8, 1_000_u64), (12, 250)] {
        ecrire(
            &runtime,
            seed,
            fait(
                seed,
                &format!("budget/{TACHE}"),
                "budget.consumed",
                serde_json::json!({ "task_id": TACHE, "amounts": { "tokens": tokens } }),
            ),
        );
    }

    let adresse = servir(runtime).await;
    let vu = dossier(&adresse, &revision(20)).await;

    assert_eq!(vu["cost"]["consumed"]["tokens"], 1_250);
    assert_eq!(vu["cost"]["entries"], 2, "le registre compte ses écritures");
}

// ---------------------------------------------------------------------------------------------
// 5. La projection est reconstructible — §9.5.
// ---------------------------------------------------------------------------------------------

/// **Reconstruite du journal, la projection rend le même résumé.**
///
/// §9.1 : une projection qu'on ne saurait pas reconstruire serait une seconde source de vérité. La
/// propriété est ce qui rend cette projection **secondaire**, et l'identité de ses inférences en
/// dépend : elle vient de la position au journal et du rang dans le commit, donc deux passages sur
/// le même journal la rendent à l'identique. Une identité tirée d'un compteur d'instance aurait
/// rendu deux graphes différents à partir du même journal.
#[test]
fn la_projection_se_reconstruit_a_l_identique() {
    use locus_projections::{EpistemicGraph, Projection as _, ProjectionRunner};

    let runtime = daemon();
    ecrire(
        &runtime,
        10,
        fait(
            10,
            &format!("task/{TACHE}"),
            "epistemic_object.staged",
            commit(
                &inference_nominale(),
                &serde_json::json!([{ "statement": "une objection", "targets": [] }]),
            ),
        ),
    );

    let courant = runtime.with_epistemic_graph(locus_projections::Projection::checksum);
    let mut reconstruite = ProjectionRunner::new(EpistemicGraph::new());
    reconstruite.catch_up(runtime.transaction().store());

    assert_eq!(reconstruite.projection().checksum(), courant);
    assert_ne!(
        courant, "0:0:0:0:0:0",
        "le résumé décrit quelque chose : un test qui comparerait deux vides passerait toujours"
    );
}

/// **La route du graphe est annoncée par `served()`.**
#[test]
fn la_route_du_graphe_est_annoncee() {
    assert!(
        locusd::http::served().contains(&GRAPH_PATH),
        "une route montée sans être annoncée est une route que personne ne trouve"
    );
    assert!(
        !GRAPH_PATH.starts_with("/lep/") && !GRAPH_PATH.starts_with("/commands/"),
        "une query de §22.4 n'est ni le protocole des workers, ni une commande : {GRAPH_PATH}"
    );
}
