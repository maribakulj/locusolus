//! Le test de sortie de `W20.ac` — une route rend la vue que la mission nomme.
//!
//! # Les trois clauses, et pourquoi la troisième décide
//!
//! 1. Une route rend la vue que la mission nomme.
//! 2. L'empreinte servie est celle que le worker recalculera.
//! 3. Une vue **échangée** est refusée.
//!
//! Sans la troisième, la route serait un tuyau : elle rendrait un document, et rien ne dirait que
//! c'est *celui-là*. Le refus se joue à la proposition, où il coûte un refus HTTP — et non chez le
//! worker après réclamation, bail frappé et attempt ouvert.
//!
//! # Ce que ce fichier ne vérifie pas, et où ça l'est
//!
//! Que le **worker** recalcule bien cette empreinte-là : c'est la moitié `canterel`. Ce qui est
//! vérifié ici est que l'empreinte servie est celle de la définition partagée — le hash du document
//! privé du champ qui le porte —, et `packages/lep/tests/canonical_corpus.rs` tient l'accord entre
//! les deux canonicalisations.

use std::fmt::Write as _;
use std::sync::Arc;

use locus_domain::canonical_hash;
use locus_lep::{MissionEnvelopeBudget, NetworkMode, ResourceSpec, SandboxLevel};
use locus_protocol::id::{Agent, Command as CommandId, Event as EventId, Project, Workspace};
use locus_protocol::{Id, IdKind, Timestamp};
use locusd::context_view::{seal, stream_of_view};
use locusd::http::{BUILD_VIEW_PATH, PROPOSE_PATH, QUEUE_PATH, router};
use locusd::lep::{Desk, Identities, MemoryQueue, MemoryRegistry};
use locusd::mission::{Authority, MemoryAdministrators, Proposal};
use locusd::{CommandError, MissionQueue, Runtime};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const ADMIN: &str = "creance-d-administration";
const WORKER: &str = "canterel-vm-linux-01";
const VUE: &str = "ctx_catalyseur";

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

#[derive(Debug, Default)]
struct Identites {
    prochain: std::sync::atomic::AtomicU8,
}

impl Identities for Identites {
    fn events(&self, count: usize) -> Result<Vec<Id<EventId>>, CommandError> {
        Ok((0..count)
            .map(|_| {
                id::<EventId>(
                    self.prochain
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                )
            })
            .collect())
    }

    fn command(&self) -> Result<Id<CommandId>, CommandError> {
        Ok(id::<CommandId>(
            self.prochain
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        ))
    }

    fn lease(&self) -> Result<Id<CommandId>, CommandError> {
        Ok(id::<CommandId>(
            self.prochain
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        ))
    }
}

fn autorite() -> Authority {
    Authority {
        workspace_id: id::<Workspace>(2),
        principal_id: id::<Agent>(7),
    }
}

fn daemon() -> (
    Runtime<locus_event_store::MemoryEventStore>,
    Arc<MemoryQueue>,
) {
    let file = Arc::new(MemoryQueue::new());
    let exploitants = Arc::new(MemoryAdministrators::new());
    exploitants.admit(ADMIN, autorite());
    let runtime = Runtime::in_memory().with_lep(
        Desk::new(
            Arc::clone(&file) as Arc<dyn MissionQueue>,
            Arc::new(MemoryRegistry::new()),
            Arc::new(Identites::default()),
        )
        .administering(exploitants),
    );
    (runtime, file)
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

async fn poster(adresse: &str, cible: &str, creance: Option<&str>, corps: &str) -> String {
    let mut flux = TcpStream::connect(adresse).await.expect("le daemon écoute");
    let mut requete = format!(
        "POST {cible} HTTP/1.1\r\nHost: {adresse}\r\nConnection: close\r\ncontent-type: application/json\r\ncontent-length: {}\r\n",
        corps.len()
    );
    if let Some(creance) = creance {
        let _ = write!(requete, "authorization: Bearer {creance}\r\n");
    }
    requete.push_str("\r\n");
    requete.push_str(corps);
    flux.write_all(requete.as_bytes())
        .await
        .expect("la requête part");
    let mut reponse = Vec::new();
    flux.read_to_end(&mut reponse)
        .await
        .expect("la réponse revient");
    String::from_utf8_lossy(&reponse).into_owned()
}

async fn demander(adresse: &str, cible: &str) -> String {
    let mut flux = TcpStream::connect(adresse).await.expect("le daemon écoute");
    let requete = format!("GET {cible} HTTP/1.1\r\nHost: {adresse}\r\nConnection: close\r\n\r\n");
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
    serde_json::from_str(corps).unwrap_or_else(|_| panic!("réponse lisible :\n{reponse}"))
}

/// La demande de vue, avec deux candidats dont **un** que le plafond écarte.
///
/// Le plafond est `internal` et l'un des deux candidats est `restricted` : la vue servie doit donc
/// porter une rédaction, et non pas simplement omettre l'élément. Une exclusion silencieuse rendrait
/// deux vues indiscernables — celle qui n'avait rien à écarter et celle qui a tout écarté.
fn demande(watermark: u64) -> String {
    serde_json::to_string(&serde_json::json!({
        "id": VUE,
        "query": "tenue du catalyseur A au-delà de 300 °C",
        "root_ids": ["rev_racine"],
        "max_depth": 3,
        "branch_scope": ["br_principal"],
        "negative_result_policy": "include",
        "source_event_watermark": watermark,
        "recipient": {
            "agent_id": id::<Agent>(9).to_string(),
            "worker_id": WORKER,
            "clearance": "internal"
        },
        "candidates": [
            {
                "revision_id": id::<locus_domain::RevisionKind>(11).to_string(),
                "position": 1,
                "classification": "internal"
            },
            {
                "revision_id": id::<locus_domain::RevisionKind>(12).to_string(),
                "position": 2,
                "classification": "restricted"
            }
        ]
    }))
    .expect("la demande se sérialise")
}

fn corps_vue(cle: &str, watermark: u64) -> String {
    format!(
        "{{\"idempotency_key\":\"{cle}\",\"project_id\":\"{}\",\"view\":{}}}",
        id::<Project>(4),
        demande(watermark)
    )
}

fn proposition(empreinte: &str, vue: &str) -> Proposal {
    Proposal {
        cognition: locus_domain::CognitionClass::Economy,
        statement: "Le catalyseur A tient-il au-delà de 300 °C ?".to_owned(),
        success_conditions: vec!["une mesure reproductible à trois essais".to_owned()],
        task_id: "tsk_catalyseur".to_owned(),
        attempt_id: "att_1".to_owned(),
        attempt: 1,
        branch_id: "br_principal".to_owned(),
        context_view_id: vue.to_owned(),
        context_view_hash: empreinte.to_owned(),
        environment_id: "env_linux".to_owned(),
        sandbox_level: SandboxLevel::S0,
        network: NetworkMode::Deny,
        resources: ResourceSpec {
            cpu: 1.0,
            memory_mb: 512,
            disk_mb: 512,
            wall_time_seconds: 60,
            accelerator: None,
        },
        budget: MissionEnvelopeBudget {
            max_model_calls: 4,
            max_input_tokens: 1000,
            max_output_tokens: 1000,
            max_cost_micros: None,
        },
        output_contract: "epistemic-commit/1".to_owned(),
    }
}

fn corps_propose(cle: &str, empreinte: &str, vue: &str) -> String {
    format!(
        "{{\"idempotency_key\":\"{cle}\",\"project_id\":\"{}\",\"proposal\":{}}}",
        id::<Project>(4),
        serde_json::to_string(&proposition(empreinte, vue)).expect("sérialisable")
    )
}

// ---------------------------------------------------------------------------------------------
// 1. Une route rend la vue que la mission nomme.
// ---------------------------------------------------------------------------------------------

/// **La mission nomme une vue, et cette vue se récupère — identifiant et empreinte compris.**
///
/// La comparaison porte sur les **deux** champs. Vérifier le seul identifiant laisserait passer une
/// mission qui nomme la bonne vue sous une empreinte étrangère, ce qui est exactement l'échange que
/// §12.3 demande au worker de détecter.
#[tokio::test]
async fn la_route_rend_la_vue_que_la_mission_nomme() {
    let (runtime, file) = daemon();
    let adresse = servir(runtime).await;

    let batie = poster(
        &adresse,
        BUILD_VIEW_PATH,
        Some(ADMIN),
        &corps_vue("idem-v", 10),
    )
    .await;
    assert!(batie.starts_with("HTTP/1.1 201"), "{batie}");
    let empreinte = corps_de(&batie)["content_hash"]
        .as_str()
        .expect("une vue scellée porte son empreinte")
        .to_owned();

    let propose = poster(
        &adresse,
        PROPOSE_PATH,
        Some(ADMIN),
        &corps_propose("idem-p", &empreinte, VUE),
    )
    .await;
    assert!(propose.starts_with("HTTP/1.1 202"), "{propose}");
    let queue = poster(
        &adresse,
        QUEUE_PATH,
        Some(ADMIN),
        &format!(
            "{{\"idempotency_key\":\"idem-q\",\"project_id\":\"{}\",\"task_id\":\"tsk_catalyseur\"}}",
            id::<Project>(4)
        ),
    )
    .await;
    assert!(queue.starts_with("HTTP/1.1 202"), "{queue}");

    let mission = file.take(WORKER).expect("une mission est en file").mission;
    let servie = demander(&adresse, &format!("/context-views/{VUE}")).await;
    assert!(servie.starts_with("HTTP/1.1 200"), "{servie}");
    let vue = corps_de(&servie);

    assert_eq!(mission.context_view.id, vue["id"].as_str().unwrap_or(""));
    assert_eq!(
        mission.context_view.hash,
        vue["content_hash"].as_str().unwrap_or("")
    );
}

/// **L'empreinte servie est celle que le worker recalculera.**
///
/// La définition est celle de `viewContentHash` : le hash du document **privé du champ qui le
/// porte**. Ce test la rejoue sur les octets réellement servis plutôt que d'appeler la fonction qui
/// a scellé — sans quoi il vérifierait qu'une fonction est égale à elle-même.
#[tokio::test]
async fn l_empreinte_servie_est_celle_du_document_prive_de_son_empreinte() {
    let (runtime, _) = daemon();
    let adresse = servir(runtime).await;
    let _ = poster(
        &adresse,
        BUILD_VIEW_PATH,
        Some(ADMIN),
        &corps_vue("idem-v", 10),
    )
    .await;

    let servie = demander(&adresse, &format!("/context-views/{VUE}")).await;
    let mut document = corps_de(&servie);
    let annoncee = document["content_hash"]
        .as_str()
        .expect("le document porte son empreinte")
        .to_owned();
    document
        .as_object_mut()
        .expect("un objet")
        .remove("content_hash");

    assert_eq!(
        annoncee,
        canonical_hash(&document)
            .expect("le document servi a une forme canonique")
            .to_string()
    );
}

/// **Ce que le filtre écarte est servi comme rédaction, pas comme absence.**
///
/// Un candidat `restricted` sous un plafond `internal` sort de la vue. Le document doit le **dire**
/// : sans rédaction, un contexte amputé et un contexte complet se ressemblent, et le raisonnement
/// mené sur le premier ne sait pas qu'il est aveugle.
#[tokio::test]
async fn ce_que_le_filtre_ecarte_est_nomme_dans_la_vue_servie() {
    let (runtime, _) = daemon();
    let adresse = servir(runtime).await;
    let _ = poster(
        &adresse,
        BUILD_VIEW_PATH,
        Some(ADMIN),
        &corps_vue("idem-v", 10),
    )
    .await;

    let vue = corps_de(&demander(&adresse, &format!("/context-views/{VUE}")).await);
    let redactions = vue["redactions"]
        .as_array()
        .expect("une vue porte ses rédactions, même vides");
    assert_eq!(
        redactions.len(),
        1,
        "un candidat sur deux dépasse le plafond : {vue}"
    );
    assert_eq!(
        redactions[0]["target"].as_str().unwrap_or(""),
        id::<locus_domain::RevisionKind>(12).to_string()
    );
    assert!(
        !redactions[0]["reason"].as_str().unwrap_or("").is_empty(),
        "une exclusion sans raison est indistinguable d'un oubli"
    );
    assert_eq!(vue["confidentiality_ceiling"].as_str(), Some("internal"));
}

// ---------------------------------------------------------------------------------------------
// 2. Une vue échangée est refusée.
// ---------------------------------------------------------------------------------------------

/// **Une proposition qui annonce une autre empreinte est refusée, en nommant le champ.**
#[tokio::test]
async fn une_vue_echangee_est_refusee() {
    let (runtime, file) = daemon();
    let adresse = servir(runtime).await;
    let _ = poster(
        &adresse,
        BUILD_VIEW_PATH,
        Some(ADMIN),
        &corps_vue("idem-v", 10),
    )
    .await;

    let etrangere = "sha256:".to_owned() + &"cd".repeat(32);
    let refus = poster(
        &adresse,
        PROPOSE_PATH,
        Some(ADMIN),
        &corps_propose("idem-p", &etrangere, VUE),
    )
    .await;

    assert!(
        refus.starts_with("HTTP/1.1 400"),
        "une empreinte étrangère est refusée :\n{refus}"
    );
    assert!(
        refus.contains("context_view.hash"),
        "le refus nomme le champ :\n{refus}"
    );
    assert!(file.is_empty(), "rien n'entre en file sur un refus");
}

/// **Une proposition qui nomme une vue que personne n'a bâtie est refusée.**
///
/// C'est le trou que `W20.ac` a trouvé : rien ne rattachait `context_view.id` à un document.
#[tokio::test]
async fn une_vue_inexistante_est_refusee() {
    let (runtime, _) = daemon();
    let adresse = servir(runtime).await;

    let refus = poster(
        &adresse,
        PROPOSE_PATH,
        Some(ADMIN),
        &corps_propose(
            "idem-p",
            &("sha256:".to_owned() + &"ab".repeat(32)),
            "ctx_absente",
        ),
    )
    .await;

    assert!(refus.starts_with("HTTP/1.1 400"), "{refus}");
    assert!(
        refus.contains("context_view.id"),
        "le refus nomme le champ :\n{refus}"
    );
}

/// **Une vue est immuable : son identifiant ne se réemploie pas.**
#[tokio::test]
async fn reecrire_une_vue_sous_le_meme_identifiant_est_refuse() {
    let (runtime, _) = daemon();
    let adresse = servir(runtime).await;
    let premiere = poster(
        &adresse,
        BUILD_VIEW_PATH,
        Some(ADMIN),
        &corps_vue("idem-v1", 10),
    )
    .await;
    assert!(premiere.starts_with("HTTP/1.1 201"), "{premiere}");

    let seconde = poster(
        &adresse,
        BUILD_VIEW_PATH,
        Some(ADMIN),
        &corps_vue("idem-v2", 20),
    )
    .await;

    assert!(
        seconde.starts_with("HTTP/1.1 422"),
        "réécrire une vue est un refus de politique :\n{seconde}"
    );
    assert!(seconde.contains("context_view.immutable"), "{seconde}");
}

/// **Une resoumission sous la même clé rend la vue déjà bâtie, sans en écrire une seconde.**
///
/// §22.5 : « les clients peuvent resoumettre sans dupliquer l'effet ». Sans cette distinction, une
/// commande rejouée — un client qui n'a pas vu passer la réponse, une chaîne de bout en bout relancée
/// sur un journal durable — se heurterait au refus d'immuabilité, qui est fait pour une **autre**
/// faute : réécrire une vue sous un identifiant déjà pris.
#[tokio::test]
async fn une_resoumission_sous_la_meme_cle_rend_la_meme_vue() {
    let (runtime, _) = daemon();
    let adresse = servir(runtime).await;

    let premiere = poster(
        &adresse,
        BUILD_VIEW_PATH,
        Some(ADMIN),
        &corps_vue("idem-v", 10),
    )
    .await;
    let seconde = poster(
        &adresse,
        BUILD_VIEW_PATH,
        Some(ADMIN),
        &corps_vue("idem-v", 10),
    )
    .await;

    assert!(premiere.starts_with("HTTP/1.1 201"), "{premiere}");
    assert!(
        seconde.starts_with("HTTP/1.1 201"),
        "une resoumission rend le verdict d'origine, elle n'échoue pas :\n{seconde}"
    );
    assert_eq!(corps_de(&premiere), corps_de(&seconde));

    // Et **un** fait, pas deux : c'est ce que « sans dupliquer l'effet » veut dire.
    let journal = corps_de(&demander(&adresse, "/timeline?limit=100").await);
    let batis = journal["items"]
        .as_array()
        .expect("une timeline porte ses items")
        .iter()
        .filter(|item| item["event_type"].as_str() == Some("context_view.built"))
        .count();
    assert_eq!(batis, 1, "{journal}");
}

/// **Un identifiant inconnu rend `404`, et non un document fabriqué.**
#[tokio::test]
async fn une_vue_inconnue_rend_404() {
    let (runtime, _) = daemon();
    let adresse = servir(runtime).await;

    let reponse = demander(&adresse, "/context-views/ctx_absente").await;

    assert!(reponse.starts_with("HTTP/1.1 404"), "{reponse}");
}

/// **Un candidat au-delà du watermark est refusé, pas silencieusement écarté.**
///
/// Une vue qui contiendrait l'avenir ne dirait plus ce qu'on savait, et l'écarter en silence
/// laisserait croire qu'il n'existait pas.
#[tokio::test]
async fn un_candidat_au_dela_du_watermark_refuse_la_construction() {
    let (runtime, _) = daemon();
    let adresse = servir(runtime).await;

    let refus = poster(
        &adresse,
        BUILD_VIEW_PATH,
        Some(ADMIN),
        // Watermark à 1 : le second candidat est en position 2.
        &corps_vue("idem-v", 1),
    )
    .await;

    assert!(refus.starts_with("HTTP/1.1 400"), "{refus}");
    assert!(
        refus.contains("candidates"),
        "le refus nomme le champ :\n{refus}"
    );
}

// ---------------------------------------------------------------------------------------------
// 3. Ce que « sceller » veut dire.
// ---------------------------------------------------------------------------------------------

/// **La valeur d'entrée de `content_hash` n'entre pas dans le résultat.**
///
/// C'est ce qui fait que sceller n'est pas « faire confiance à ce qu'on nous a annoncé ». Le champ
/// est retiré du document avant la canonicalisation ; deux brouillons qui ne diffèrent que par lui
/// scellent donc à l'identique. Sans ce test, le commentaire qui l'affirme serait une promesse.
#[test]
fn sceller_ignore_l_empreinte_annoncee() {
    let brouillon = |annoncee: &str| locus_lep::ContextView {
        id: VUE.to_owned(),
        query: Some("tenue du catalyseur".to_owned()),
        root_ids: None,
        included_types: None,
        included_relations: None,
        max_depth: None,
        time_range: None,
        branch_scope: None,
        validation_levels: None,
        confidentiality_ceiling: locus_lep::DataClass::Internal,
        artifact_policy: None,
        negative_result_policy: None,
        diversity_policy: None,
        token_budget: None,
        redactions: Some(Vec::new()),
        source_event_watermark: 10,
        content_hash: annoncee.to_owned(),
        generated_at: "2026-08-28T10:00:00.000Z".to_owned(),
    };

    let gauche = seal(brouillon(&("sha256:".to_owned() + &"ab".repeat(32))))
        .expect("un document canonicalisable");
    let droite = seal(brouillon(&("sha256:".to_owned() + &"cd".repeat(32))))
        .expect("un document canonicalisable");

    assert_eq!(gauche.content_hash, droite.content_hash);
    assert_eq!(gauche, droite);
}

/// **Le stream d'une vue porte son identifiant, et le namespace de §10.3.**
#[test]
fn le_stream_d_une_vue_est_dans_sa_famille() {
    assert_eq!(stream_of_view("ctx_1"), "context_view/ctx_1");
}
