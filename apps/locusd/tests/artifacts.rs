//! Le test de sortie de `W20.t` — la chaîne d'artefacts de §19.1 atteint le daemon.
//!
//! # Ce qui manquait, et comment on l'a su
//!
//! `apps/locusd/Cargo.toml` ne dépendait **pas** de `locus-artifacts`. `W2.14` avait livré la
//! déclaration-avant-upload côté worker, `packages/artifacts` portait le manifeste et la
//! confrontation du hash, et rien côté serveur ne les recevait — la clause « les artefacts sont
//! hashés » de `e2e/minimal_science` n'avait donc aucun sujet institutionnel. Vérifié au code en
//! tentant `W12.d`, pas déduit du texte.
//!
//! # Les trois clauses, et ce que chacune refuse
//!
//! 1. **Le hash est vérifié, jamais cru.** Le daemon le recalcule sur les octets reçus. Un
//!    manifeste dont le hash ne correspond pas est refusé en nommant `content_hash`.
//! 2. **La déclaration précède l'upload.** Un dépôt sous un artefact que personne n'a déclaré est
//!    refusé : il n'y a pas de promesse à confronter, donc pas de vérification possible.
//! 3. **L'invariant 4 est tenu.** Le fait porte la provenance, et un manifeste sans elle n'entre
//!    pas — `ArtifactManifest::declare` refuse une `task_id` vide, et c'est la seule porte.

use std::fmt::Write as _;
use std::sync::Arc;

use locus_domain::ContentHash;
use locus_protocol::id::{Agent, Event as EventId, Project, Workspace};
use locus_protocol::{Id, IdKind, Timestamp};
use locusd::CommandError;
use locusd::artifacts::{
    Declared, MemoryBlobs, UPLOAD_WINDOW_SECONDS, Uploaded, stream_of_artifact, upload_path,
};
use locusd::http::{CONTENT_PATH, DECLARE_PATH, router};
use locusd::lep::{Desk, Identities, MemoryQueue, MemoryRegistry, WorkerIdentity};
use locusd::{MissionQueue, Runtime};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const WORKER_CREANCE: &str = "creance-de-worker";
const WORKER: &str = "canterel-vm-linux-01";
const ARTEFACT: &str = "artifact-figure-3";
/// Le contenu réel de l'artefact. Ce que le daemon hashe, et ce à quoi la promesse est confrontée.
const CONTENU: &[u8] = b"<svg xmlns=\"http://www.w3.org/2000/svg\"><title>recall</title></svg>";

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

    fn command(&self) -> Result<Id<locus_protocol::id::Command>, CommandError> {
        Ok(id::<locus_protocol::id::Command>(
            self.prochain
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        ))
    }

    fn lease(&self) -> Result<Id<locus_protocol::id::Command>, CommandError> {
        self.command()
    }
}

/// Un daemon avec un worker enrôlé et un stockage d'objets câblé.
fn daemon() -> (
    Runtime<locus_event_store::MemoryEventStore>,
    Arc<MemoryBlobs>,
) {
    let registre = Arc::new(MemoryRegistry::new());
    registre.admit(
        WORKER_CREANCE,
        WorkerIdentity {
            worker_id: WORKER.to_owned(),
            workspace_id: id::<Workspace>(2),
            principal_id: id::<Agent>(3),
        },
    );
    let octets = Arc::new(MemoryBlobs::new());
    let runtime = Runtime::in_memory().with_lep(
        Desk::new(
            Arc::new(MemoryQueue::new()) as Arc<dyn MissionQueue>,
            registre,
            Arc::new(Identites::default()),
        )
        .storing(Arc::clone(&octets) as Arc<dyn locusd::artifacts::Blobs>),
    );
    (runtime, octets)
}

/// Le même, sans stockage câblé — le défaut.
fn daemon_sans_stockage() -> Runtime<locus_event_store::MemoryEventStore> {
    let registre = Arc::new(MemoryRegistry::new());
    registre.admit(
        WORKER_CREANCE,
        WorkerIdentity {
            worker_id: WORKER.to_owned(),
            workspace_id: id::<Workspace>(2),
            principal_id: id::<Agent>(3),
        },
    );
    Runtime::in_memory().with_lep(Desk::new(
        Arc::new(MemoryQueue::new()) as Arc<dyn MissionQueue>,
        registre,
        Arc::new(Identites::default()),
    ))
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

async fn envoyer(requete: &str, corps: &[u8], adresse: &str) -> String {
    let mut flux = TcpStream::connect(adresse).await.expect("le daemon écoute");
    let mut octets = requete.as_bytes().to_vec();
    octets.extend_from_slice(corps);
    flux.write_all(&octets).await.expect("la requête part");
    let mut reponse = Vec::new();
    flux.read_to_end(&mut reponse)
        .await
        .expect("la réponse revient");
    String::from_utf8_lossy(&reponse).into_owned()
}

async fn declarer(adresse: &str, creance: Option<&str>, manifeste: &str) -> String {
    let corps = format!(
        "{{\"idempotency_key\":\"idem-declare\",\"project_id\":\"{}\",\"manifest\":{manifeste}}}",
        id::<Project>(4)
    );
    let mut requete = format!(
        "POST {DECLARE_PATH} HTTP/1.1\r\nHost: {adresse}\r\nConnection: close\r\n\
         content-type: application/json\r\ncontent-length: {}\r\n",
        corps.len()
    );
    if let Some(creance) = creance {
        let _ = write!(requete, "authorization: Bearer {creance}\r\n");
    }
    requete.push_str("\r\n");
    envoyer(&requete, corps.as_bytes(), adresse).await
}

async fn deposer(adresse: &str, artefact: &str, creance: Option<&str>, octets: &[u8]) -> String {
    let mut requete = format!(
        "PUT {} HTTP/1.1\r\nHost: {adresse}\r\nConnection: close\r\n\
         content-type: application/octet-stream\r\ncontent-length: {}\r\n\
         locus-idempotency-key: idem-upload\r\nlocus-project-id: {}\r\n",
        upload_path(artefact),
        octets.len(),
        id::<Project>(4),
    );
    if let Some(creance) = creance {
        let _ = write!(requete, "authorization: Bearer {creance}\r\n");
    }
    requete.push_str("\r\n");
    envoyer(&requete, octets, adresse).await
}

/// Le manifeste que déclare le worker — **le hash est celui du contenu réel**.
///
/// Il est écrit champ par champ plutôt que repris d'une fixture parce que la fixture du dépôt est
/// `promoted` et porte un hash arbitraire : elle prouve la traversée du fil, pas la confrontation.
/// Ici c'est la confrontation qui est en jeu, donc le hash doit être celui d'octets qui existent.
fn manifeste_de(hash: &str, taille: usize, tache: &str) -> String {
    serde_json::json!({
        "artifact_id": ARTEFACT,
        "content_hash": hash,
        "media_type": "image/svg+xml",
        "size_bytes": taille,
        "produced_by": { "task_id": tache, "attempt": 2 },
        "classification": "internal",
        "state": "declared",
    })
    .to_string()
}

fn manifeste_nominal() -> String {
    manifeste_de(
        &ContentHash::of(CONTENU).to_string(),
        CONTENU.len(),
        "task-plot-recall",
    )
}

/// Faire décider un décideur, et rendre le fait unique qu'il compose.
///
/// Les faits de §19.1 se testent ici plutôt que par le journal : c'est le décideur qui compose la
/// charge, et l'interroger directement fait porter l'assertion sur ce qui est écrit plutôt que sur
/// ce qu'une relecture veut bien en rendre.
fn decide<D: locusd::Decide<State = locusd::lep::LepContext>>(
    decideur: &D,
) -> locus_event_store::Draft {
    let contexte = locusd::lep::LepContext {
        project_id: id::<Project>(4),
        event_ids: vec![id::<EventId>(1)],
        occurred_at: Timestamp::from_millis(1_700_000_000_000),
        payload_hash: String::new(),
    };
    let commande = locusd::CommandEnvelope::mutating(
        id::<locus_protocol::id::Command>(13),
        "worker.report",
        id::<Workspace>(2),
        id::<Agent>(3),
        "idem".to_owned(),
        locusd::Revision::new(0),
    )
    .expect("enveloppe valide");
    let mut faits =
        locusd::Decide::decide(decideur, &commande, &contexte).expect("le décideur compose");
    assert_eq!(faits.len(), 1, "un fait par acte de §19.1");
    faits.remove(0)
}

fn corps_de(reponse: &str) -> serde_json::Value {
    let corps = reponse
        .split_once("\r\n\r\n")
        .map_or("", |(_, corps)| corps);
    serde_json::from_str(corps).unwrap_or_else(|_| panic!("réponse JSON lisible :\n{reponse}"))
}

async fn faits(adresse: &str) -> Vec<(String, String)> {
    let mut flux = TcpStream::connect(adresse).await.expect("le daemon écoute");
    let requete =
        format!("GET /timeline?limit=100 HTTP/1.1\r\nHost: {adresse}\r\nConnection: close\r\n\r\n");
    flux.write_all(requete.as_bytes())
        .await
        .expect("la requête part");
    let mut reponse = Vec::new();
    flux.read_to_end(&mut reponse)
        .await
        .expect("la réponse revient");
    let reponse = String::from_utf8_lossy(&reponse).into_owned();
    let valeur = corps_de(&reponse);
    valeur["items"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some((
                        item["event_type"].as_str()?.to_owned(),
                        item["stream_id"].as_str()?.to_owned(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn types(faits: &[(String, String)]) -> Vec<&str> {
    faits.iter().map(|(kind, _)| kind.as_str()).collect()
}

// ---------------------------------------------------------------------------------------------
// 1. La chaîne complète : déclarer, déposer, vérifier.
// ---------------------------------------------------------------------------------------------

/// **Déclarer puis déposer écrit les deux faits, et le hash est confronté.**
///
/// C'est le test de sortie. Ce qu'il tient, et qu'aucune de ses moitiés ne tiendrait seule : le
/// condensat que le daemon renvoie est celui qu'il a **calculé** sur les octets reçus, et le fait
/// `artifact.uploaded` porte celui-là — pas le déclaré recopié. Un daemon qui recopierait la
/// déclaration passerait ce test si l'on ne comparait que les deux entre eux, puisqu'ils sont
/// égaux quand tout va bien ; c'est pourquoi le test suivant fait diverger le contenu.
#[tokio::test]
async fn declarer_puis_deposer_ecrit_les_deux_faits() {
    let (runtime, octets) = daemon();
    let adresse = servir(runtime).await;

    let declaration = declarer(&adresse, Some(WORKER_CREANCE), &manifeste_nominal()).await;
    assert!(
        declaration.starts_with("HTTP/1.1 202"),
        "la déclaration est acceptée :\n{declaration}"
    );
    let ticket = corps_de(&declaration);
    assert_eq!(ticket["artifact_id"], ARTEFACT);
    assert_eq!(ticket["upload_path"], upload_path(ARTEFACT));
    assert!(
        ticket["expires_at"].is_string(),
        "la déclaration ouvre une fenêtre datée : {ticket}"
    );
    assert_eq!(
        octets.object_count(),
        0,
        "déclarer ne range aucun octet : le contenu n'est pas encore arrivé, et c'est le point"
    );

    let depot = deposer(&adresse, ARTEFACT, Some(WORKER_CREANCE), CONTENU).await;
    assert!(
        depot.starts_with("HTTP/1.1 201"),
        "le dépôt aboutit :\n{depot}"
    );
    let recu = corps_de(&depot);
    assert_eq!(
        recu["received_hash"],
        ContentHash::of(CONTENU).to_string(),
        "le condensat rendu est celui du contenu reçu"
    );
    assert_eq!(recu["size_bytes"], CONTENU.len());
    assert_eq!(octets.object_count(), 1);

    assert_eq!(
        types(&faits(&adresse).await),
        vec!["artifact.declared", "artifact.uploaded"],
        "les deux faits de §19.1, dans l'ordre"
    );
}

/// **Les deux faits sont écrits dans le stream de l'artefact, pas dans celui de la tâche.**
///
/// Deux workers déclarant deux artefacts de la même tâche entreraient sinon en conflit d'écriture
/// sans rien avoir de commun : la révision d'un stream avance à chaque fait, et deux artefacts
/// indépendants se bloqueraient l'un l'autre.
///
/// Le nom attendu est écrit **en toutes lettres** ici, et pas obtenu de `stream_of_artifact`. Une
/// première rédaction comparait la valeur à elle-même : un passage de mutation qui rangeait les
/// faits sous `task/…` la laissait verte, puisque les deux côtés de l'égalité bougeaient ensemble.
#[tokio::test]
async fn les_faits_vont_dans_le_stream_de_l_artefact() {
    let (runtime, _) = daemon();
    let adresse = servir(runtime).await;
    declarer(&adresse, Some(WORKER_CREANCE), &manifeste_nominal()).await;
    deposer(&adresse, ARTEFACT, Some(WORKER_CREANCE), CONTENU).await;

    let ecrits = faits(&adresse).await;
    assert_eq!(ecrits.len(), 2);
    for (kind, ou) in &ecrits {
        assert_eq!(
            ou,
            &format!("artifact/{ARTEFACT}"),
            "« {kind} » va dans le stream de l'artefact"
        );
        assert!(
            !ou.starts_with("task/"),
            "et surtout pas dans celui d'une tâche : « {ou} »"
        );
    }
    assert_eq!(
        stream_of_artifact(ARTEFACT),
        format!("artifact/{ARTEFACT}"),
        "le nom que le code construit est celui que ce test attend"
    );
}

/// **Le fait d'arrivée porte le condensat *observé*, pas le déclaré recopié.**
///
/// Le décideur est interrogé directement : c'est lui qui compose le fait, et c'est donc là que la
/// question se tranche. Par le fil, les deux valeurs sont égales quand tout va bien — un daemon qui
/// recopierait la déclaration passerait tous les tests de bout en bout. Ici le condensat observé
/// est délibérément **différent** de tout ce que le manifeste porte, et le fait doit dire celui-là.
///
/// Un fait qui ne peut pas différer de la déclaration ne prouve rien : que les deux coïncident est
/// le résultat de la vérification, pas sa définition.
#[test]
fn le_fait_d_arrivee_porte_le_condensat_observe() {
    let observe = ContentHash::of(b"ce que le daemon a reellement recu");
    let arrivee = Uploaded {
        artifact_id: ARTEFACT.to_owned(),
        observed: observe.clone(),
        size_bytes: 42,
        worker_id: WORKER.to_owned(),
    };

    let charge = &decide(&arrivee).payload;
    assert_eq!(charge["observed_hash"], observe.to_string());
    assert_eq!(charge["state"], "uploaded");
    assert_eq!(charge["worker_id"], WORKER);
    assert_eq!(charge["size_bytes"], 42);
}

// ---------------------------------------------------------------------------------------------
// 2. Le hash est vérifié, jamais cru.
// ---------------------------------------------------------------------------------------------

/// **Un contenu qui n'est pas celui qui avait été promis est refusé, en nommant le champ.**
///
/// La déclaration est parfaitement bien formée ; ce sont les octets qui divergent. C'est le seul
/// endroit où l'ordre de §19.1 paie : si le manifeste avait été écrit à partir du contenu reçu, ce
/// test ne pourrait pas exister — la vérification comparerait le contenu à lui-même.
#[tokio::test]
async fn un_contenu_qui_diverge_est_refuse_en_nommant_le_hash() {
    let (runtime, octets) = daemon();
    let adresse = servir(runtime).await;
    declarer(&adresse, Some(WORKER_CREANCE), &manifeste_nominal()).await;

    let depot = deposer(
        &adresse,
        ARTEFACT,
        Some(WORKER_CREANCE),
        b"<svg><title>autre chose entierement, meme longueur !!</title></svg>",
    )
    .await;

    assert!(depot.starts_with("HTTP/1.1 400"), "{depot}");
    assert!(
        depot.contains("content_hash"),
        "le refus nomme le champ :\n{depot}"
    );
    assert_eq!(
        octets.object_count(),
        0,
        "un contenu refusé ne laisse rien derrière lui — ni sous le hash déclaré, ni sous le sien"
    );
    assert_eq!(
        types(&faits(&adresse).await),
        vec!["artifact.declared"],
        "aucun `artifact.uploaded` : la vérification précède le fait, sans quoi le fait ne \
         prouverait rien"
    );
}

/// **Un contenu plus long que la taille déclarée est refusé pendant l'écriture.**
///
/// Pas après : `packages/artifacts` borne l'écriture au fragment qui dépasse, et c'est ce qui
/// permet de refuser un contenu de plusieurs gigaoctets sans l'avoir tenu en mémoire.
#[tokio::test]
async fn un_contenu_plus_long_que_la_taille_declaree_est_refuse() {
    let (runtime, octets) = daemon();
    let adresse = servir(runtime).await;
    declarer(&adresse, Some(WORKER_CREANCE), &manifeste_nominal()).await;

    let mut trop = CONTENU.to_vec();
    trop.extend_from_slice(b"et encore ceci");
    let depot = deposer(&adresse, ARTEFACT, Some(WORKER_CREANCE), &trop).await;

    assert!(depot.starts_with("HTTP/1.1 400"), "{depot}");
    assert!(
        depot.contains("size_bytes"),
        "le refus nomme le champ :\n{depot}"
    );
    assert_eq!(octets.object_count(), 0);
}

// ---------------------------------------------------------------------------------------------
// 3. La déclaration précède l'upload.
// ---------------------------------------------------------------------------------------------

/// **Déposer sous un artefact que personne n'a déclaré est refusé.**
///
/// Et le refus dit *pourquoi* : sans promesse, il n'y a rien à confronter. Un daemon qui
/// accepterait ici fabriquerait le manifeste à partir du contenu reçu — c'est-à-dire une
/// vérification qui ne peut pas échouer.
#[tokio::test]
async fn deposer_sans_declaration_est_refuse() {
    let (runtime, octets) = daemon();
    let adresse = servir(runtime).await;

    let depot = deposer(
        &adresse,
        "artifact-jamais-declare",
        Some(WORKER_CREANCE),
        CONTENU,
    )
    .await;

    assert!(depot.starts_with("HTTP/1.1 400"), "{depot}");
    assert!(
        depot.contains("artifact_id"),
        "le refus nomme le champ :\n{depot}"
    );
    assert_eq!(octets.object_count(), 0);
    assert!(
        faits(&adresse).await.is_empty(),
        "rien n'est écrit : ni déclaration inventée, ni arrivée"
    );
}

/// **Un stream qui porte autre chose qu'une déclaration reste « non déclaré ».**
///
/// Ce test vient d'un survivant de mutation. La relecture cherchait « le premier fait qui porte une
/// clé `manifest` », et se rabattre sur le premier fait venu ne cassait rien — parce qu'aucun test
/// ne plaçait dans un stream d'artefact un fait qui ne soit pas une déclaration.
///
/// Ce qu'un tel repli produirait : un `500` là où le client attend un `400`. Le daemon dirait « je
/// suis cassé » à un worker dont la seule faute est de n'avoir pas déclaré, et un `500` s'accompagne
/// de « réessaie » — donc il réessaierait, à l'identique, indéfiniment. La relecture cherche
/// maintenant le **nom du fait**, qui ne se trompe pas de sujet.
#[test]
fn un_stream_sans_declaration_reste_non_declare() {
    let (runtime, octets) = daemon();
    let soumis = locusd::lep::Submitted {
        idempotency_key: "idem".to_owned(),
        project_id: id::<Project>(4),
        occurred_at: Timestamp::from_millis(1_700_000_000_000),
    };

    // Un `artifact.uploaded` sans déclaration préalable : le stream existe, il ne déclare rien.
    runtime
        .commit(
            &Uploaded {
                artifact_id: ARTEFACT.to_owned(),
                observed: ContentHash::of(CONTENU),
                size_bytes: CONTENU.len() as u64,
                worker_id: WORKER.to_owned(),
            },
            &locusd::CommandEnvelope::mutating(
                id::<locus_protocol::id::Command>(13),
                "worker.report",
                id::<Workspace>(2),
                id::<Agent>(3),
                "idem-hors-chemin".to_owned(),
                locusd::Revision::new(0),
            )
            .expect("enveloppe valide"),
            &locusd::lep::LepContext {
                project_id: id::<Project>(4),
                event_ids: vec![id::<EventId>(9)],
                occurred_at: Timestamp::from_millis(1_700_000_000_000),
                payload_hash: String::new(),
            },
            Timestamp::from_millis(1_700_000_000_000),
        )
        .accepted()
        .expect("le fait hors chemin est écrit");

    let refus = runtime
        .lep_upload_artifact(
            WORKER_CREANCE,
            ARTEFACT,
            CONTENU,
            &soumis,
            Timestamp::from_millis(1_700_000_000_000),
        )
        .expect_err("rien n'a été déclaré");

    match refus {
        CommandError::Validation { field, .. } => assert_eq!(field, "artifact_id"),
        autre => {
            panic!("le client doit déclarer, pas retenter contre un défaut interne : {autre:?}")
        }
    }
    assert_eq!(octets.object_count(), 0);
}

/// **La fenêtre de dépôt se ferme, et le refus dit laquelle.**
///
/// §19.1 dit « URL temporaire ». Une échéance que rien ne vérifie serait un champ décoratif — un
/// effet annoncé qui n'a pas lieu, ce que l'ADR 0022 nomme une promesse et refuse. Le test appelle
/// le daemon directement plutôt que par le fil : l'horloge de la route est celle du système, et un
/// test qui attendrait quinze minutes ne serait pas un test.
#[test]
fn une_declaration_expiree_n_ouvre_plus_le_depot() {
    let (runtime, octets) = daemon();
    let declaration = Timestamp::from_millis(1_700_000_000_000);
    let apres = Timestamp::from_millis(1_700_000_000_000 + (UPLOAD_WINDOW_SECONDS + 1) * 1_000);
    let soumis = locusd::lep::Submitted {
        idempotency_key: "idem".to_owned(),
        project_id: id::<Project>(4),
        occurred_at: declaration,
    };

    let document = serde_json::from_str(&manifeste_nominal()).expect("manifeste lisible");
    runtime
        .lep_declare_artifact(WORKER_CREANCE, &document, &soumis, declaration)
        .expect("la déclaration aboutit");

    let refus = runtime
        .lep_upload_artifact(WORKER_CREANCE, ARTEFACT, CONTENU, &soumis, apres)
        .expect_err("la fenêtre est fermée");

    match refus {
        CommandError::Validation { field, detail } => {
            assert_eq!(field, "artifact_id");
            assert!(
                detail.contains(&UPLOAD_WINDOW_SECONDS.to_string()),
                "le refus dit quelle fenêtre : {detail}"
            );
        }
        autre => panic!("un dépôt hors délai est une faute du client : {autre:?}"),
    }
    assert_eq!(octets.object_count(), 0);
}

/// **Et à l'intérieur de la fenêtre, le dépôt a lieu.**
///
/// L'autre moitié, et elle n'est pas facultative : une garde qui refuserait *aussi* ce qui est
/// juste passerait le test précédent sans qu'on s'en aperçoive, et se ferait désactiver au premier
/// worker qui se plaint.
#[test]
fn a_l_interieur_de_la_fenetre_le_depot_a_lieu() {
    let (runtime, octets) = daemon();
    let declaration = Timestamp::from_millis(1_700_000_000_000);
    let juste_avant =
        Timestamp::from_millis(1_700_000_000_000 + (UPLOAD_WINDOW_SECONDS - 1) * 1_000);
    let soumis = locusd::lep::Submitted {
        idempotency_key: "idem".to_owned(),
        project_id: id::<Project>(4),
        occurred_at: declaration,
    };

    let document = serde_json::from_str(&manifeste_nominal()).expect("manifeste lisible");
    runtime
        .lep_declare_artifact(WORKER_CREANCE, &document, &soumis, declaration)
        .expect("la déclaration aboutit");
    runtime
        .lep_upload_artifact(WORKER_CREANCE, ARTEFACT, CONTENU, &soumis, juste_avant)
        .expect("la fenêtre est encore ouverte");

    assert_eq!(octets.object_count(), 1);
}

// ---------------------------------------------------------------------------------------------
// 4. L'invariant 4 : la provenance, sans quoi rien n'entre.
// ---------------------------------------------------------------------------------------------

/// **Un manifeste sans provenance n'entre pas, et le refus nomme le champ.**
///
/// Invariant 4 : « tout résultat scientifique majeur est artifact-first et provenance-first ». Un
/// artefact sans tâche productrice est un fichier — il ne peut être rattaché à aucune exécution,
/// donc ni reproduit, ni contesté, ni cité.
#[tokio::test]
async fn un_manifeste_sans_provenance_n_entre_pas() {
    let (runtime, _) = daemon();
    let adresse = servir(runtime).await;

    let muet = manifeste_de(&ContentHash::of(CONTENU).to_string(), CONTENU.len(), "   ");
    let refus = declarer(&adresse, Some(WORKER_CREANCE), &muet).await;

    assert!(refus.starts_with("HTTP/1.1 400"), "{refus}");
    assert!(
        refus.contains("produced_by.task_id"),
        "le refus nomme le champ :\n{refus}"
    );
    assert!(faits(&adresse).await.is_empty());
}

/// **Le fait de déclaration porte la provenance en clair, et le manifeste entier.**
///
/// La provenance est déjà dans le manifeste que le fait embarque. L'écrire à côté fait qu'une
/// projection qui ne relit pas le manifeste la voit quand même — et une projection qui devrait
/// désérialiser un manifeste entier pour savoir de quelle tâche vient un artefact finira par ne
/// pas le faire.
///
/// Le manifeste entier y est aussi, et pour la raison de `W20.s` : le dépôt du contenu doit relire
/// **ce qui a été déclaré**. Le faire renvoyer par son déposant laisserait déclarer un hash et en
/// téléverser un autre sous le même identifiant.
#[test]
fn le_fait_de_declaration_porte_la_provenance() {
    let document: locus_lep::ArtifactManifest =
        serde_json::from_str(&manifeste_nominal()).expect("manifeste lisible");
    let declaration = Declared {
        manifest: locus_artifacts::ArtifactManifest::from_wire(&document)
            .expect("le domaine accepte ce manifeste"),
        worker_id: WORKER.to_owned(),
        expires_at: Timestamp::from_millis(1_700_000_900_000),
    };

    let charge = &decide(&declaration).payload;
    assert_eq!(charge["produced_by"]["task_id"], "task-plot-recall");
    assert_eq!(charge["produced_by"]["attempt"], 2);
    assert_eq!(
        charge["declared_hash"],
        ContentHash::of(CONTENU).to_string()
    );
    assert_eq!(charge["state"], "declared");
    assert_eq!(
        charge["manifest"]["content_hash"],
        ContentHash::of(CONTENU).to_string(),
        "le manifeste entier voyage dans le fait"
    );
}

/// **Un hash mal formé est refusé en le nommant, avant que rien ne soit écrit.**
///
/// Un digest tronqué ressemble en tout point à un digest valide tant que personne ne compte, et
/// c'est la forme que prend une intégrité cassée. Il est refusé à la traduction, là où un document
/// venu d'ailleurs devient un manifeste dont les invariants tiennent.
#[tokio::test]
async fn un_hash_malforme_est_refuse_a_la_traduction() {
    let (runtime, _) = daemon();
    let adresse = servir(runtime).await;

    let tronque = manifeste_de("sha256:abc", CONTENU.len(), "task-plot-recall");
    let refus = declarer(&adresse, Some(WORKER_CREANCE), &tronque).await;

    assert!(refus.starts_with("HTTP/1.1 400"), "{refus}");
    assert!(refus.contains("content_hash"), "{refus}");
    assert!(faits(&adresse).await.is_empty());
}

// ---------------------------------------------------------------------------------------------
// 5. Les créances, et le stockage absent.
// ---------------------------------------------------------------------------------------------

/// **Sans créance reconnue, ni déclaration ni dépôt.**
#[tokio::test]
async fn sans_creance_la_chaine_est_fermee() {
    let (runtime, octets) = daemon();
    let adresse = servir(runtime).await;

    for reponse in [
        declarer(&adresse, None, &manifeste_nominal()).await,
        deposer(&adresse, ARTEFACT, None, CONTENU).await,
    ] {
        assert!(reponse.starts_with("HTTP/1.1 401"), "{reponse}");
    }
    let inconnue = declarer(&adresse, Some("creance-inventee"), &manifeste_nominal()).await;
    assert!(inconnue.starts_with("HTTP/1.1 403"), "{inconnue}");

    assert_eq!(octets.object_count(), 0);
    assert!(faits(&adresse).await.is_empty());
}

/// **Un daemon sans stockage câblé refuse le dépôt en le disant, et n'écrit pas le fait.**
///
/// Il ne l'accepte pas en jetant les octets : un `201` sur un contenu que personne n'a rangé ferait
/// écrire `artifact.uploaded` pour un artefact introuvable, et l'invariant 4 tomberait sans qu'un
/// seul test rougisse. La famille est `unavailable` et non `validation` — c'est l'exploitant qui
/// n'a rien câblé, pas le client qui s'est trompé, et lui faire corriger une requête juste
/// l'enverrait chercher au mauvais endroit.
#[tokio::test]
async fn sans_stockage_cable_le_depot_est_refuse_en_le_disant() {
    let runtime = daemon_sans_stockage();
    let adresse = servir(runtime).await;
    declarer(&adresse, Some(WORKER_CREANCE), &manifeste_nominal()).await;

    let depot = deposer(&adresse, ARTEFACT, Some(WORKER_CREANCE), CONTENU).await;

    assert!(depot.starts_with("HTTP/1.1 503"), "{depot}");
    assert!(
        depot.contains("stockage"),
        "le refus dit ce qui manque :\n{depot}"
    );
    assert_eq!(
        types(&faits(&adresse).await),
        vec!["artifact.declared"],
        "la déclaration a eu lieu, l'arrivée non"
    );
}

// ---------------------------------------------------------------------------------------------
// 6. Les chemins.
// ---------------------------------------------------------------------------------------------

/// **Le chemin qu'annonce le ticket est celui que le routeur sert.**
///
/// Deux endroits construisent ce chemin : [`upload_path`], qui le met dans le ticket, et
/// [`CONTENT_PATH`], qui est le motif d'`axum`. Deux endroits qui construisent le même chemin
/// finissent par en construire deux, et le worker suivrait alors une adresse que personne ne sert
/// — un `404` que rien dans les deux fichiers ne laisserait prévoir.
#[test]
fn le_chemin_annonce_est_celui_qui_est_route() {
    assert_eq!(
        CONTENT_PATH.replace("{artifact_id}", ARTEFACT),
        upload_path(ARTEFACT)
    );
}

/// **Les deux chemins de §19.1 sont sous `/lep/`.**
///
/// Contrairement aux deux de §22.3 : c'est un worker qui déclare ce qu'il a produit, sous sa
/// créance de worker. Les ranger sous `/commands/` en ferait de l'administration, et un exploitant
/// se mettrait à déclarer des artefacts qu'aucune exécution n'a produits.
#[test]
fn les_chemins_de_19_1_sont_le_protocole_des_workers() {
    for chemin in [DECLARE_PATH, CONTENT_PATH] {
        assert!(
            chemin.starts_with("/lep/v1/"),
            "« {chemin} » appartient au protocole des workers"
        );
    }
}
