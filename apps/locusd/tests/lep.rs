//! Le test de sortie de `W20.k` — la surface §15.2, vérifiée sur des réponses **réelles**.
//!
//! # Ce que ces tests éprouvent, et pourquoi c'est le bon bout
//!
//! La clause d'origine de l'item disait « le harnais de conformance de `W0.9` tourne contre le
//! daemon réel ». Elle a été écrite en marquant `W2.21` et n'a pas survécu à la lecture du harnais,
//! une heure plus tard : `packages/testing` **joue le serveur** — « il n'y a personne pour
//! compenser » — donc le faire tourner contre le daemon opposait deux serveurs et ne voulait rien
//! dire.
//!
//! Ce qui la remplace tient la même propriété par un autre bout. Les corps qui traversent sont les
//! types **générés** de `packages/lep`, décodés depuis les fixtures de `W0.7` — les mêmes que
//! `canterel` consomme en TypeScript. Les deux moitiés du fil viennent d'un seul schéma, donc un
//! changement de schéma casse les deux côtés à la compilation au lieu de les laisser diverger en
//! silence.
//!
//! # Un vrai socket, comme `W20.g`
//!
//! Requêtes HTTP/1.1 écrites à la main, réponses lues en octets. Un appel de service en mémoire
//! court-circuiterait le parsing de la requête et l'écriture de la réponse : il vérifierait un
//! handler, pas une liaison.

use std::fmt::Write as _;
use std::sync::Arc;

use locus_broker::port::{BrokerError, BrokerPort, Placement};
use locus_broker::protocol::Verdict;
use locus_lep::{CapabilityManifest, Event, ResourceSpec, SandboxSpec};
use locus_protocol::id::{Agent, Command as CommandId, Event as EventId, Project, Workspace};
use locus_protocol::{Id, IdKind, Timestamp};
use locusd::http::{CLAIM_PATH, EVENTS_PATH, RESULT_PATH, router};
use locusd::lep::{
    Desk, HEARTBEAT_INTERVAL_SECONDS, Identities, LEASE_TTL_SECONDS, MemoryQueue, MemoryRegistry,
    NoIdentities, Offer, Queued, WorkerIdentity, stream_of_task,
};
use locusd::{CommandError, Runtime};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const CREANCE: &str = "creance-de-worker";
const WORKER: &str = "canterel-vm-linux-01";

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

/// Une source d'identifiants **de test**, et son nom le dit.
///
/// Elle tire des identités par un compteur, ce que `NoIdentities` refuse de faire en production
/// pour une raison qui vaut ici aussi : au redémarrage, elle réattribuerait les mêmes. Dans un test
/// il n'y a pas de redémarrage, et le déterminisme est un avantage — un fait écrit porte une
/// identité qu'on peut prédire.
#[derive(Debug, Default)]
struct IdentitesDeTest {
    prochain: std::sync::atomic::AtomicU8,
}

impl Identities for IdentitesDeTest {
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

fn fixture<T: serde::de::DeserializeOwned>(nom: &str) -> T {
    let chemin = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/examples")
        .join(nom);
    let brut = std::fs::read_to_string(&chemin).expect("fixture lisible");
    let mut valeur: serde_json::Value =
        serde_json::from_str(&brut).expect("fixture en JSON valide");
    valeur
        .as_object_mut()
        .expect("une fixture est un objet")
        .remove("_fixture");
    serde_json::from_value(valeur).expect("la fixture se décode dans le type généré")
}

/// Ce qu'une mise en file dépose — `W20.v` : la mission, et le rang que la proposition a fixé.
///
/// **Aucun bail.** Il n'a pas d'objet avant qu'un worker soit choisi, et c'est le daemon qui le
/// frappe à la réclamation. Le type le rend inexprimable : `Queued` ne porte pas le champ.
/// Le rang d'attempt de ces tests.
///
/// **Trois, et non un.** Un premier jeu portait `1`, et une passe de mutation a montré ce que ça
/// vaut : remplacer `queued.attempt` par la constante `1` survivait, parce que l'attendu et le
/// fourni étaient le même chiffre. C'est le défaut que `SOCKET_MODE` a eu, puis les chemins de
/// §15.2, puis `served()` — une valeur comparée à elle-même ne vérifie rien.
///
/// Trois dit en plus quelque chose de vrai : §12.3 veut qu'une tâche **réattribuée** conserve son
/// numéro, donc un rang supérieur à un est le cas normal d'une reprise, pas une curiosité.
const RANG: i64 = 3;

fn en_file() -> Queued {
    Queued {
        mission: fixture("mission-envelope-nominal.json"),
        attempt: RANG,
    }
}

/// Ce que le worker annonce — la fixture Linux de `W0.7`, dont le `worker_id` **est** [`WORKER`].
///
/// Prise telle quelle, sans un champ retouché : `W0.7` a écrit un corpus pour que les tests
/// n'inventent pas leurs propres documents, et un manifeste bricolé ici ne prouverait rien de ce
/// qu'un worker réel envoie.
fn manifeste() -> CapabilityManifest {
    let manifeste: CapabilityManifest = fixture("capability-manifest-vm-linux.json");
    assert_eq!(
        manifeste.worker_id, WORKER,
        "la fixture Linux de W0.7 désigne le worker de ces tests ; si elle change, ce n'est plus \
         elle qu'on éprouve"
    );
    manifeste
}

/// Le manifeste d'un worker nommé — le même document, sous un autre `worker_id`.
///
/// Retouché sur ce seul champ, et pour une raison précise : le manifeste doit s'accorder à la
/// créance, sans quoi la réclamation est refusée avant tout placement (`W20.q`). Ce qu'on éprouve
/// ici est le bail, pas le manifeste.
fn manifeste_de(worker: &str) -> String {
    let mut manifeste = manifeste();
    worker.clone_into(&mut manifeste.worker_id);
    serde_json::to_string(&manifeste).expect("le manifeste se sérialise")
}

/// L'offre lue du corps d'une réponse HTTP.
fn offre_de(reponse: &str) -> Offer {
    let corps = reponse
        .split_once("\r\n\r\n")
        .map_or("", |(_, corps)| corps);
    serde_json::from_str(corps)
        .unwrap_or_else(|erreur| panic!("offre lisible ({erreur}) :\n{reponse}"))
}

/// Un broker de test, qui **retient ce qu'on lui a demandé**.
///
/// Un `Loopback` suffirait à rendre un verdict ; il ne dirait rien de ce que `locusd` lui a passé.
/// Or c'est la moitié de l'item : « la réclamation passe le `CapabilityManifest` du worker au
/// broker ». Un daemon qui poserait la question avec le manifeste d'un autre, ou avec une exigence
/// inventée, obtiendrait le même verdict et passerait tous les tests d'un port muet.
struct BrokerDeTest {
    verdict: Result<Verdict, BrokerError>,
    vu: std::sync::Mutex<Vec<(CapabilityManifest, SandboxSpec, ResourceSpec)>>,
}

impl BrokerDeTest {
    fn rendant(verdict: Verdict) -> Self {
        Self {
            verdict: Ok(verdict),
            vu: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn injoignable() -> Self {
        Self {
            verdict: Err(BrokerError::Unreachable {
                endpoint: "/tmp/pas-de-broker.sock".to_owned(),
                why: "aucun processus n'écoute".to_owned(),
            }),
            vu: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn placant() -> Self {
        Self::rendant(Verdict::Placed {
            worker: WORKER.to_owned(),
            level: locus_lep::SandboxLevel::S3,
        })
    }

    fn refusant() -> Self {
        Self::rendant(Verdict::NotPlaced {
            shortfalls: vec![locus_broker::protocol::Shortfall {
                worker: WORKER.to_owned(),
                reasons: vec![locus_lep::Reason::LevelUnavailable {
                    required: locus_lep::SandboxLevel::S3,
                    best: locus_lep::SandboxLevel::S2,
                }],
            }],
        })
    }

    fn questions(&self) -> Vec<(CapabilityManifest, SandboxSpec, ResourceSpec)> {
        self.vu
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl BrokerPort for BrokerDeTest {
    fn endpoint(&self) -> String {
        "broker-de-test".to_owned()
    }

    fn readiness(&self) -> Result<Verdict, BrokerError> {
        self.verdict.clone()
    }

    fn place(
        &self,
        manifest: &CapabilityManifest,
        sandbox: &SandboxSpec,
        resources: &ResourceSpec,
    ) -> Result<Placement, BrokerError> {
        self.vu
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((manifest.clone(), sandbox.clone(), resources.clone()));
        locus_broker::port::as_placement(self.verdict.clone()?)
    }
}

fn identite() -> WorkerIdentity {
    WorkerIdentity {
        worker_id: WORKER.to_owned(),
        workspace_id: id::<Workspace>(2),
        principal_id: id::<Agent>(3),
        project_id: id::<Project>(4),
    }
}

/// Un daemon prêt à parler §15.2, et la file qu'on lui a remplie.
fn daemon(missions: Vec<Queued>) -> Runtime<locus_event_store::MemoryEventStore> {
    daemon_brokere(missions, Arc::new(BrokerDeTest::placant())).0
}

/// Le même, avec le broker qu'on lui donne — et ce broker rendu à l'appelant.
fn daemon_brokere(
    missions: Vec<Queued>,
    broker: Arc<BrokerDeTest>,
) -> (
    Runtime<locus_event_store::MemoryEventStore>,
    Arc<BrokerDeTest>,
) {
    let file = Arc::new(MemoryQueue::new());
    for mission in missions {
        file.push(mission);
    }
    let registre = Arc::new(MemoryRegistry::new());
    registre.admit(CREANCE, identite());
    let runtime = Runtime::in_memory().with_lep(
        Desk::new(file, registre, Arc::new(IdentitesDeTest::default()))
            .placing(Arc::clone(&broker) as Arc<dyn BrokerPort + Send + Sync>),
    );
    (runtime, broker)
}

/// Le même, avec un puits de diagnostic qu'on peut relire — `W20.aa`.
fn daemon_observe(
    missions: Vec<Queued>,
    broker: Arc<BrokerDeTest>,
) -> (
    Runtime<locus_event_store::MemoryEventStore>,
    Arc<locusd::observations::MemoryObservations>,
) {
    let file = Arc::new(MemoryQueue::new());
    for mission in missions {
        file.push(mission);
    }
    let registre = Arc::new(MemoryRegistry::new());
    registre.admit(CREANCE, identite());
    let puits = Arc::new(locusd::observations::MemoryObservations::new());
    let runtime = Runtime::in_memory().with_lep(
        Desk::new(file, registre, Arc::new(IdentitesDeTest::default()))
            .placing(broker as Arc<dyn BrokerPort + Send + Sync>)
            .observing(Arc::clone(&puits) as Arc<dyn locusd::observations::Observations>),
    );
    (runtime, puits)
}

async fn servir(runtime: Runtime<locus_event_store::MemoryEventStore>) -> String {
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

/// Un `POST` écrit à la main, et la réponse brute.
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

/// Un `GET` écrit à la main.
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

/// Les types d'événements écrits sur un stream, **relus par la surface publique**.
///
/// Par `/timeline` plutôt que par un accesseur de test sur le journal, et c'est délibéré : le
/// journal sort de `locusd` en lecture seule par les queries de §22.4, et lui ajouter une porte
/// « pour les tests » créerait le chemin que `W20.b` a fermé. Ce qui est vérifié devient du même
/// coup plus fort — non seulement le fait est écrit, mais **un client le voit**.
async fn faits_sur(adresse: &str, stream: &str) -> Vec<String> {
    let reponse = demander(adresse, "/timeline?limit=100").await;
    let corps = reponse
        .split_once("\r\n\r\n")
        .map_or("", |(_, corps)| corps);
    let valeur: serde_json::Value =
        serde_json::from_str(corps).unwrap_or_else(|_| panic!("timeline lisible :\n{reponse}"));
    valeur["items"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter(|item| item["stream_id"] == stream)
                .filter_map(|item| item["event_type"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// Un corps de worker, sous une clé d'idempotence **nommée**.
///
/// Une première rédaction câblait `idem-1` dans tous les corps. Quatre tests sont tombés, et le
/// diagnostic vaut d'être gardé : le registre faisait exactement son travail. La clé est scopée par
/// `(workspace, principal)` — pas par commande —, donc réclamer puis rendre sous la même clé est
/// **une resoumission**, et la seconde a rendu le verdict de la première sans rien écrire.
///
/// C'est le comportement que §22.5 décrit et il est juste : la clé appartient au client, à lui de ne
/// pas la resservir pour autre chose. Ce qui était faux était la fixture, pas le code — et un test
/// qui aurait été « corrigé » en relâchant l'assertion aurait caché la seule chose que ces quatre
/// échecs avaient à dire.
fn corps(cle: &str, extra: &str) -> String {
    format!(
        "{{\"idempotency_key\":\"{cle}\",\"project_id\":\"{}\"{extra}}}",
        id::<Project>(4)
    )
}

/// Le corps d'une réclamation, sous une clé qui n'est employée nulle part ailleurs.
///
/// Il porte le manifeste depuis `W20.q` : une réclamation sans manifeste est refusée, et c'est
/// éprouvé à part — ici, ce qu'on veut est une réclamation ordinaire.
fn corps_minimal(extra: &str) -> String {
    let annonce = serde_json::to_string(&manifeste()).expect("le manifeste se sérialise");
    corps("idem-claim", &format!(",\"manifest\":{annonce}{extra}"))
}

// ---------------------------------------------------------------------------------------------
// 1. « Rien pour toi » n'est pas « je n'ai pas pu demander » — des DEUX côtés du fil.
// ---------------------------------------------------------------------------------------------

/// **`204` sur une file vide, et non une erreur.**
///
/// C'est la séparation de l'ADR 0028 décision 4, que `W2.21` tient déjà côté client : un `204` y
/// devient un tour `idle`, une panne de transport y lève. Répondre `404` ou `503` à une file vide
/// enverrait le worker chercher un lien cassé là où il n'y a que du calme — et un worker qui
/// cherche un lien cassé s'arrête, alors qu'un worker au calme revient.
#[tokio::test]
async fn une_file_vide_rend_204_et_non_une_erreur() {
    let adresse = servir(daemon(Vec::new())).await;
    let reponse = poster(&adresse, CLAIM_PATH, Some(CREANCE), &corps_minimal("")).await;

    assert!(
        reponse.starts_with("HTTP/1.1 204"),
        "une file vide doit répondre 204 :\n{reponse}"
    );
}

/// **Et une créance inconnue reçoit un refus, pas du calme.**
///
/// Le pendant du test précédent, et celui qui l'empêche de passer pour de mauvaises raisons : un
/// daemon qui répondrait `204` à tout le monde passerait le premier test sans rien servir.
#[tokio::test]
async fn une_creance_inconnue_est_refusee_et_non_servie() {
    let adresse = servir(daemon(vec![en_file()])).await;

    let sans = poster(&adresse, CLAIM_PATH, None, &corps_minimal("")).await;
    assert!(
        sans.starts_with("HTTP/1.1 401"),
        "sans porteur, 401 :\n{sans}"
    );

    let fausse = poster(
        &adresse,
        CLAIM_PATH,
        Some("pas-la-bonne"),
        &corps_minimal(""),
    )
    .await;
    assert!(
        fausse.starts_with("HTTP/1.1 403"),
        "une créance inconnue est une faute d'autorisation, pas un défaut interne :\n{fausse}"
    );
    // Et la créance refusée ne se relit nulle part dans la réponse : `CLAUDE.md` interdit de
    // journaliser un token, et un message d'erreur qui la citerait la ferait fuir dans le premier
    // rapport de bug venu.
    assert!(
        !fausse.contains("pas-la-bonne"),
        "une créance refusée ne se cite pas :\n{fausse}"
    );
}

/// **La file n'est pas entamée par une requête non authentifiée.**
///
/// Retirer l'offre avant de savoir qui parle la perdrait au profit de personne — et c'est le genre
/// de faute qu'aucun journal ne montre, puisqu'il ne se passe rien.
#[tokio::test]
async fn une_requete_refusee_ne_consomme_pas_la_file() {
    let file = Arc::new(MemoryQueue::new());
    file.push(en_file());
    let registre = Arc::new(MemoryRegistry::new());
    registre.admit(CREANCE, identite());
    let runtime = Runtime::in_memory().with_lep(
        Desk::new(
            Arc::clone(&file) as Arc<dyn locusd::MissionQueue>,
            registre,
            Arc::new(IdentitesDeTest::default()),
        )
        .placing(Arc::new(BrokerDeTest::placant())),
    );
    let adresse = servir(runtime).await;

    let _ = poster(
        &adresse,
        CLAIM_PATH,
        Some("pas-la-bonne"),
        &corps_minimal(""),
    )
    .await;

    assert_eq!(
        file.len(),
        1,
        "une réclamation refusée ne doit rien retirer de la file"
    );
}

// ---------------------------------------------------------------------------------------------
// 2. Le fait atteint le journal — ce que `W23.b` compte, et que rien ne faisait exister.
// ---------------------------------------------------------------------------------------------

/// **Une réclamation servie écrit `task.leased` dans le journal.**
///
/// C'est le test de sortie au sens strict. `W23.b` compte `generating`, un fait qu'aucun journal
/// n'écrivait ; son marqueur a visé `W2.20` puis `W2.21`, deux jalons voisins, parce qu'il n'y
/// avait pas d'item à viser. Celui-ci écrit le fait, et le test le **relit du journal** plutôt que
/// de croire la réponse HTTP.
#[tokio::test]
async fn une_reclamation_servie_atteint_le_journal() {
    let adresse = servir(daemon(vec![en_file()])).await;

    let reponse = poster(&adresse, CLAIM_PATH, Some(CREANCE), &corps_minimal("")).await;
    assert!(
        reponse.starts_with("HTTP/1.1 200"),
        "une offre disponible se sert :\n{reponse}"
    );
    // La mission part bien sur le fil, sous les noms que `canterel` lit.
    assert!(
        reponse.contains("\"mission\""),
        "la mission voyage :\n{reponse}"
    );
    assert!(reponse.contains("\"lease\""), "le bail voyage :\n{reponse}");

    let ecrits = faits_sur(&adresse, "task/task-nominal").await;
    assert_eq!(
        ecrits.len(),
        1,
        "un fait, et un seul : la réclamation a été confiée une fois"
    );
    assert_eq!(
        ecrits[0], "task.leased",
        "le fait écrit nomme ce qui a eu lieu : la tâche est confiée sous bail (§7.1)"
    );

    // **Dès cette écriture-ci**, et non à la suivante — `W20.l`. Une passe de mutation a montré
    // qu'un rattrapage placé *avant* la soumission passait tous les tests : ils faisaient deux
    // écritures, et la seconde rendait visible le fait de la première. Une seule écriture est donc
    // le seul cas qui distingue « rattraper après » de « rattraper avant ».
    let workers = demander(&adresse, "/workers").await;
    assert!(
        workers.contains(WORKER),
        "le fait de cette écriture doit être visible sans en attendre une autre :\n{workers}"
    );
}

/// **Un résultat rendu écrit l'achèvement de la tentative.**
///
/// `task.leased` ouvre, `run.completed` referme — les deux bornes de ce que `W23.b` compte.
///
/// Ce fait ne dit **pas** que la tâche a réussi, et ce n'est pas un oubli : le corps que `W2.21`
/// envoie ne porte aucune issue. En déduire un succès parce qu'un résultat est arrivé serait
/// affirmer ce que personne n'a dit — et §7.1 fait de `succeeded` le contrat technique rempli, que
/// l'institution lit ensuite pour décider d'accepter.
#[tokio::test]
async fn un_resultat_rendu_acheve_la_tentative_sans_la_declarer_reussie() {
    let en_file = en_file();
    let tache = en_file.mission.task_id.clone();
    let adresse = servir(daemon(vec![en_file])).await;

    let _ = poster(&adresse, CLAIM_PATH, Some(CREANCE), &corps_minimal("")).await;
    let rendu = corps(
        "idem-result",
        &format!(
            ",\"task_id\":\"{tache}\",\"attempt_id\":\"attempt-nominal\",\"session_id\":\"ses_01\",\"output\":{{\"resume\":\"fait\"}}"
        ),
    );
    let reponse = poster(&adresse, RESULT_PATH, Some(CREANCE), &rendu).await;

    assert!(
        reponse.starts_with("HTTP/1.1 202"),
        "un résultat rendu est accepté :\n{reponse}"
    );
    let ecrits = faits_sur(&adresse, "task/task-nominal").await;
    assert_eq!(ecrits, vec!["task.leased", "run.completed"]);
    assert!(
        !ecrits.iter().any(|kind| kind.contains("succeeded")),
        "aucun fait ne déclare un succès que personne n'a annoncé : {ecrits:?}"
    );
}

/// **Les événements de §15.6 atteignent le journal, traduits dans les namespaces de §10.3.**
///
/// §15.6 nomme `attempt.started` et `tool.completed` — une taxonomie de **protocole**. §10.3 nomme
/// les namespaces du **journal**, où `attempt` et `tool` n'existent pas. Les faire passer tels
/// quels écrirait dans un namespace que personne ne relit ; la traduction est explicite, et ce test
/// la lit sur le journal.
#[tokio::test]
async fn les_evenements_du_worker_atteignent_le_journal_sous_les_namespaces_de_10_3() {
    let en_file = en_file();
    let tache = en_file.mission.task_id.clone();
    let adresse = servir(daemon(vec![en_file])).await;
    let _ = poster(&adresse, CLAIM_PATH, Some(CREANCE), &corps_minimal("")).await;

    let mut demarrage: Event = fixture("event-reconnection-1-started.json");
    demarrage.task_id = Some(tache.clone());
    demarrage.worker_id = Some(WORKER.to_owned());
    let mut outil: Event = fixture("event-reconnection-3-tool-completed.json");
    outil.task_id = Some(tache.clone());
    outil.worker_id = Some(WORKER.to_owned());

    let evenements = serde_json::to_string(&vec![demarrage, outil]).expect("sérialisable");
    let reponse = poster(
        &adresse,
        EVENTS_PATH,
        Some(CREANCE),
        &corps("idem-events", &format!(",\"events\":{evenements}")),
    )
    .await;

    assert!(
        reponse.starts_with("HTTP/1.1 202"),
        "une remontée est acceptée :\n{reponse}"
    );
    let ecrits = faits_sur(&adresse, "task/task-nominal").await;
    assert_eq!(
        ecrits,
        vec!["task.leased", "run.started", "run.tool_completed"],
        "`attempt.*` devient `run.*`, et `tool.completed` ne se confond pas avec le démarrage de la \
         tentative elle-même"
    );
}

/// **Un worker ne parle pas au nom d'un autre.**
///
/// La créance dit qui parle ; le corps de la requête n'est qu'une déclaration. Un événement qui
/// prétend venir d'un autre worker est refusé plutôt qu'écrit sous le nom qu'il annonce — c'est ce
/// que §7 existe pour empêcher, et le refus est **typé**.
#[tokio::test]
async fn un_evenement_au_nom_d_un_autre_worker_est_refuse() {
    let en_file = en_file();
    let tache = en_file.mission.task_id.clone();
    let adresse = servir(daemon(vec![en_file])).await;
    let _ = poster(&adresse, CLAIM_PATH, Some(CREANCE), &corps_minimal("")).await;

    let mut usurpe: Event = fixture("event-reconnection-1-started.json");
    usurpe.task_id = Some(tache.clone());
    usurpe.worker_id = Some("un-autre-worker".to_owned());
    let evenements = serde_json::to_string(&vec![usurpe]).expect("sérialisable");

    let reponse = poster(
        &adresse,
        EVENTS_PATH,
        Some(CREANCE),
        &corps("idem-usurpation", &format!(",\"events\":{evenements}")),
    )
    .await;

    assert!(
        reponse.starts_with("HTTP/1.1 403"),
        "une usurpation est une faute d'autorisation :\n{reponse}"
    );
    assert_eq!(
        faits_sur(&adresse, "task/task-nominal").await,
        vec!["task.leased"],
        "un refus n'écrit rien : c'est la transaction qui écrit, et elle n'écrit qu'un `Ok`"
    );
}

// ---------------------------------------------------------------------------------------------
// 3. Ce que le daemon refuse de faire à la place de quelqu'un d'autre.
// ---------------------------------------------------------------------------------------------

/// **Sans source d'identifiants, le daemon refuse — il n'invente pas.**
///
/// `NoIdentities` est le défaut, et il ne fabrique rien. Un défaut qui rendrait des identifiants
/// séquentiels aurait marché en test, marché au premier démarrage, et réattribué les mêmes
/// identités au redémarrage suivant : un journal dont deux faits différents portent la même
/// identité, découvert des mois plus tard.
///
/// Le refus est `503` — le service ne peut pas répondre **maintenant**, ce qui est exact et se
/// répare par configuration — et non `500`, qui enverrait chercher un défaut dans le code.
#[tokio::test]
async fn sans_source_d_identifiants_le_daemon_refuse_plutot_que_d_inventer() {
    let file = Arc::new(MemoryQueue::new());
    file.push(en_file());
    let registre = Arc::new(MemoryRegistry::new());
    registre.admit(CREANCE, identite());
    // Le broker **place** : sans cela, le `503` viendrait du lien absent et non de la source
    // d'identifiants, et ce test passerait en n'éprouvant pas ce que son nom annonce.
    let runtime = Runtime::in_memory().with_lep(
        Desk::new(file, registre, Arc::new(NoIdentities))
            .placing(Arc::new(BrokerDeTest::placant())),
    );
    let adresse = servir(runtime).await;

    let reponse = poster(&adresse, CLAIM_PATH, Some(CREANCE), &corps_minimal("")).await;

    assert!(
        reponse.starts_with("HTTP/1.1 503"),
        "sans source d'identifiants, le service est indisponible, pas en panne :\n{reponse}"
    );
    assert!(
        reponse.contains("identifiant"),
        "le refus doit nommer la source d'identifiants, et non le broker :\n{reponse}"
    );
}

/// **Un worker n'a pas à dire son projet : son grant le dit — `W20.z`.**
///
/// Ce test disait exactement le contraire, et le contraire était le défaut. Il exigeait `400
/// project_id` sur une réclamation sans projet, et c'est ce refus qu'un worker `canterel` réel a
/// reçu à sa toute première réclamation contre un `locusd` réel.
///
/// Le worker n'avait pas tort. `W20.w` a tranché la même question pour l'enrôlement — « c'est
/// l'institution qui décide où un worker écrit » —, et la trancher pour l'enrôlement seul laissait
/// la surface §15.2 redemander à chaque acte ce que la créance savait déjà. Le workspace et le
/// principal venaient du registre depuis `W20.k` ; le projet est la coordonnée qui manquait.
#[tokio::test]
async fn un_corps_sans_projet_est_servi_depuis_le_grant() {
    let adresse = servir(daemon(vec![en_file()])).await;
    let annonce = serde_json::to_string(&manifeste()).expect("le manifeste se sérialise");

    let reponse = poster(
        &adresse,
        CLAIM_PATH,
        Some(CREANCE),
        &format!("{{\"idempotency_key\":\"idem-sans-projet\",\"manifest\":{annonce}}}"),
    )
    .await;

    assert!(
        reponse.starts_with("HTTP/1.1 200"),
        "la réclamation aboutit sans que le worker nomme son projet :\n{reponse}"
    );
}

/// **Un projet qui n'est pas celui du grant est refusé, pas ignoré — `W20.z`.**
///
/// Le pendant du test précédent, et il porte la moitié qui compte. Sans lui, rendre le champ
/// facultatif reviendrait à le rendre **décoratif** : un worker qui croit écrire dans un projet
/// verrait ses faits atterrir ailleurs sans que rien ne le lui dise, et le découvrirait en relisant
/// le journal — ou ne le découvrirait jamais.
///
/// C'est la règle que `W20.w` tient déjà pour l'enrôlement, appliquée à l'acte.
#[tokio::test]
async fn un_projet_qui_n_est_pas_celui_du_grant_est_refuse() {
    let adresse = servir(daemon(vec![en_file()])).await;
    let annonce = serde_json::to_string(&manifeste()).expect("le manifeste se sérialise");

    let reponse = poster(
        &adresse,
        CLAIM_PATH,
        Some(CREANCE),
        &format!(
            "{{\"idempotency_key\":\"idem-autre-projet\",\"project_id\":\"{}\",\"manifest\":{annonce}}}",
            id::<Project>(9)
        ),
    )
    .await;

    assert!(
        reponse.starts_with("HTTP/1.1 400"),
        "écrire ailleurs que dans son projet est une faute du client :\n{reponse}"
    );
    assert!(
        reponse.contains("project_id"),
        "le refus nomme le champ :\n{reponse}"
    );
}

/// **Un projet illisible est refusé pour ce qu'il est, et non pour ne pas être le bon.**
///
/// Deux refus qui se ressembleraient enverraient chercher au mauvais endroit : « ce n'est pas ton
/// projet » fait relire un grant, « ce n'est pas un identifiant » fait relire une chaîne. Le champ
/// reste donc **relu** même lorsqu'il ne décide plus rien.
#[tokio::test]
async fn un_projet_illisible_est_refuse_avant_d_etre_compare() {
    let adresse = servir(daemon(vec![en_file()])).await;

    let reponse = poster(
        &adresse,
        CLAIM_PATH,
        Some(CREANCE),
        "{\"idempotency_key\":\"idem-illisible\",\"project_id\":\"pas-un-identifiant\"}",
    )
    .await;

    assert!(reponse.starts_with("HTTP/1.1 400"), "{reponse}");
    assert!(
        !reponse.contains("son grant dit"),
        "un identifiant illisible n'est pas une divergence de projet :\n{reponse}"
    );
}

/// **Une resoumission sous la même clé n'écrit pas deux fois** — §15.5, §22.5.
///
/// La clé est **scopée** par `(workspace, principal)`, tous deux lus du registre : deux workers qui
/// choisissent `idem-1` ne se répondent pas l'un à l'autre. Ce que ce test tient est l'autre moitié
/// — le même worker qui retente ne produit pas le doublon que §15.5 existe pour empêcher.
///
/// La **durabilité** de ce registre reste `W20.j` : il vit en mémoire vive, et un redémarrage le
/// perd. Les deux ne se confondent pas, et le dire ici évite de croire l'un livré avec l'autre.
#[tokio::test]
async fn une_resoumission_sous_la_meme_cle_n_ecrit_pas_deux_fois() {
    let en_file = en_file();
    let tache = en_file.mission.task_id.clone();
    let adresse = servir(daemon(vec![en_file])).await;
    let _ = poster(&adresse, CLAIM_PATH, Some(CREANCE), &corps_minimal("")).await;

    // La **même** clé, deux fois, sur la **même** commande : c'est ce que §15.5 appelle une
    // resoumission, et c'est cela qu'on éprouve ici.
    let rendu = corps(
        "idem-result",
        &format!(
            ",\"task_id\":\"{tache}\",\"attempt_id\":\"attempt-nominal\",\"session_id\":\"ses_01\",\"output\":{{}}"
        ),
    );
    let premier = poster(&adresse, RESULT_PATH, Some(CREANCE), &rendu).await;
    let second = poster(&adresse, RESULT_PATH, Some(CREANCE), &rendu).await;

    assert!(premier.starts_with("HTTP/1.1 202"), "{premier}");
    assert!(
        second.starts_with("HTTP/1.1 202"),
        "une resoumission rend le résultat d'origine, elle n'échoue pas :\n{second}"
    );
    assert_eq!(
        faits_sur(&adresse, "task/task-nominal").await,
        vec!["task.leased", "run.completed"],
        "deux envois de la même clé produisent un fait, pas deux"
    );
}

// ---------------------------------------------------------------------------------------------
// 4. La règle 4 de `boundaries.json` n'a pas bougé.
// ---------------------------------------------------------------------------------------------

/// **Servir un worker n'ouvre aucun socket de runtime.**
///
/// La règle 4 est vérifiée par `check:boundaries` sur tout le crate ; ce test la tient sur le
/// fichier qui aurait le plus de raisons d'y contrevenir — celui qui parle aux workers. Deux
/// vérifications indépendantes valent mieux qu'une, et celle-ci est lisible ici, à côté du code
/// qu'elle protège.
///
/// `locusd` reçoit un compte rendu d'exécution ; il n'exécute pas. C'est ce que l'ADR 0004 sépare,
/// et la tentation de parler à Podman « juste pour le profil local » est exactement ce que la
/// séparation empêche.
#[test]
fn la_surface_worker_ne_touche_aucun_socket_de_runtime() {
    let source = include_str!("../src/lep.rs");
    for interdit in ["bollard", "podman", "docker", "UnixStream", "os::unix::net"] {
        assert!(
            !source.contains(interdit),
            "« {interdit} » dans la surface §15.2 : `locusd` ne détient jamais de socket de runtime, \
             c'est le rôle de `locus-execd` (ADR 0004, règle 4)"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 5. Ce qu'une passe de mutation a trouvé, et que rien ne tenait.
// ---------------------------------------------------------------------------------------------

/// **Les trois chemins de §15.2, littéralement.**
///
/// Une première rédaction n'employait que les constantes — donc la même valeur des deux côtés de
/// l'égalité, donc rien. Une passe de mutation l'a montré en remplaçant `/lep/v1/claim` par
/// `/lep/v1/claimx` sans faire rougir un seul test. C'est la deuxième fois de la journée que ce
/// défaut apparaît : `worker-client.ts` de `canterel` le portait ce matin, contre la même
/// constante, de l'autre côté du même fil.
#[test]
fn les_chemins_de_15_2_sont_ceux_que_le_protocole_nomme() {
    assert_eq!(CLAIM_PATH, "/lep/v1/claim");
    assert_eq!(EVENTS_PATH, "/lep/v1/events");
    assert_eq!(RESULT_PATH, "/lep/v1/result");
}

/// **Le stream d'une tâche porte le préfixe que le journal attend.**
///
/// Même défaut, même remède : les tests de journal comparaient à `stream_of_task(&tache)`, donc à
/// eux-mêmes. Renommer le préfixe en `tache/` passait. Un stream renommé silencieusement rendrait
/// invisible tout l'historique déjà écrit — les faits resteraient, et plus personne ne les lirait.
#[test]
fn le_stream_d_une_tache_porte_son_prefixe() {
    assert_eq!(stream_of_task("task-nominal"), "task/task-nominal");
}

/// **Un en-tête d'autorisation sans `Bearer ` n'est pas une créance.**
///
/// La mutation qui remplaçait `strip_prefix("Bearer ")` par `trim_start_matches` survivait : tous
/// les tests envoyaient soit un porteur bien formé, soit rien du tout. Un jeton nu accepté serait
/// une créance lue d'un en-tête que §15.2 ne définit pas, donc une porte que personne n'a décidée.
#[tokio::test]
async fn un_entete_sans_bearer_n_est_pas_une_creance() {
    let adresse = servir(daemon(vec![en_file()])).await;

    let mut flux = TcpStream::connect(&adresse)
        .await
        .expect("le daemon écoute");
    let corps = corps_minimal("");
    let requete = format!(
        "POST {CLAIM_PATH} HTTP/1.1\r\nHost: {adresse}\r\nConnection: close\r\ncontent-type: application/json\r\ncontent-length: {}\r\nauthorization: {CREANCE}\r\n\r\n{corps}",
        corps.len()
    );
    flux.write_all(requete.as_bytes())
        .await
        .expect("la requête part");
    let mut reponse = Vec::new();
    flux.read_to_end(&mut reponse)
        .await
        .expect("la réponse revient");
    let reponse = String::from_utf8_lossy(&reponse);

    assert!(
        reponse.starts_with("HTTP/1.1 401"),
        "un jeton nu, sans `Bearer `, n'est pas une créance :\n{reponse}"
    );
}

/// **Le bail servi nomme le worker qui a réclamé, et la tâche qu'il accompagne.**
///
/// Ces deux propriétés étaient tenues par des **gardes** jusqu'à `W20.v` : la file portait des
/// paires déjà formées, un bail y arrivait avec un `worker_id` que personne n'avait confronté au
/// réclamant, et deux tests vérifiaient qu'une paire dépareillée était refusée.
///
/// Elles sont désormais vraies **par construction** — le bail est frappé depuis le worker admis et
/// depuis la mission retirée —, et les gardes ont disparu parce qu'elles ne pouvaient plus se
/// déclencher : c'est ce que `W20.n` a fait à `Rejection::WrongEndpoint`.
///
/// Ce test est ce qui remplace les deux : il ne vérifie plus qu'un refus arrive, il vérifie que
/// **le bail servi est le bon**. Une garantie de construction qui n'est éprouvée nulle part est une
/// garantie qu'on croit tenir.
#[tokio::test]
async fn le_bail_servi_nomme_son_worker_et_sa_tache() {
    let adresse = servir(daemon(vec![en_file()])).await;

    let reponse = poster(&adresse, CLAIM_PATH, Some(CREANCE), &corps_minimal("")).await;
    assert!(reponse.starts_with("HTTP/1.1 200"), "{reponse}");

    let offre = offre_de(&reponse);
    assert_eq!(
        offre.lease.worker_id, WORKER,
        "le bail autorise le worker qui a réclamé, et personne d'autre"
    );
    assert_eq!(
        offre.lease.task_id, offre.mission.task_id,
        "et il désigne la mission qu'il accompagne — §11.1, aucune identité substituée à une autre"
    );
    assert_eq!(
        offre.lease.attempt, RANG,
        "le rang vient de la mise en file, pas d'un compteur de réclamations (§12.3)"
    );
}

/// **Deux workers qui réclament la même file reçoivent deux baux distincts.**
///
/// C'est ce qui rend le placement de `W20.q` non décoratif. Tant que la file portait des paires, un
/// bail y nommait un worker d'avance : la question posée au broker ne pouvait que **confirmer** ce
/// choix, jamais le faire. Deux missions, deux workers, deux baux — et chacun le sien.
#[tokio::test]
async fn deux_workers_recoivent_chacun_leur_bail() {
    const AUTRE: &str = "creance-de-l-autre-worker";
    const AUTRE_WORKER: &str = "canterel-vm-linux-02";

    let file = Arc::new(MemoryQueue::new());
    file.push(en_file());
    file.push(en_file());
    let registre = Arc::new(MemoryRegistry::new());
    registre.admit(CREANCE, identite());
    registre.admit(
        AUTRE,
        WorkerIdentity {
            worker_id: AUTRE_WORKER.to_owned(),
            workspace_id: id::<Workspace>(2),
            principal_id: id::<Agent>(3),
            project_id: id::<Project>(4),
        },
    );
    let runtime = Runtime::in_memory().with_lep(
        Desk::new(file, registre, Arc::new(IdentitesDeTest::default()))
            .placing(Arc::new(BrokerDeTest::placant())),
    );
    let adresse = servir(runtime).await;

    let premier = offre_de(&poster(&adresse, CLAIM_PATH, Some(CREANCE), &corps_minimal("")).await);
    let second = offre_de(
        &poster(
            &adresse,
            CLAIM_PATH,
            Some(AUTRE),
            &corps(
                "idem-autre",
                &format!(",\"manifest\":{}", manifeste_de(AUTRE_WORKER)),
            ),
        )
        .await,
    );

    assert_eq!(premier.lease.worker_id, WORKER);
    assert_eq!(second.lease.worker_id, AUTRE_WORKER);
    assert_ne!(
        premier.lease.lease_id, second.lease.lease_id,
        "deux baux distincts portent deux identités : les confondre rendrait indistinguables les \
         deux droits d'exécution"
    );
}

/// **Le bail servi porte les bornes de §12.3, et elles tiennent la relation.**
///
/// La relation elle-même — le battement sous le tiers du TTL — est tenue **à la compilation** par
/// un `const` de `lep.rs` : un réglage fautif ne compile pas. Ce test tient l'autre moitié, qui ne
/// se déduit pas de la première : que le bail réellement servi **porte** ces bornes, et non des
/// valeurs qu'un chemin aurait recomposées en route.
#[tokio::test]
async fn le_bail_servi_porte_les_bornes_de_12_3() {
    let adresse = servir(daemon(vec![en_file()])).await;

    let offre = offre_de(&poster(&adresse, CLAIM_PATH, Some(CREANCE), &corps_minimal("")).await);

    assert_eq!(offre.lease.ttl_seconds, LEASE_TTL_SECONDS);
    assert_eq!(
        offre.lease.heartbeat_interval_seconds,
        HEARTBEAT_INTERVAL_SECONDS
    );
    assert!(
        offre.lease.heartbeat_interval_seconds * 3 <= offre.lease.ttl_seconds,
        "§12.3 sur le document servi : {} s de battement pour {} s de bail",
        offre.lease.heartbeat_interval_seconds,
        offre.lease.ttl_seconds
    );
    // Et l'échéance est **postérieure** à l'émission : un bail né expiré se lit comme un bail perdu
    // (`W2.9`), et le worker rendrait aussitôt une tâche qu'on vient de lui confier.
    assert!(
        offre.lease.expires_at > offre.lease.issued_at,
        "émis {} , expire {}",
        offre.lease.issued_at,
        offre.lease.expires_at
    );
}

/// **Une réserve d'identités trop courte refuse en disant combien il en manquait.**
///
/// Ce crate ne fabrique pas d'identifiants ; ce qu'il peut faire est refuser d'en manquer. La
/// mutation qui comblait le trou par un identifiant fabriqué survivait, faute de test — et c'est le
/// scénario le plus coûteux qui soit : deux faits différents portant la même identité, dans un
/// journal qui est la vérité institutionnelle.
#[tokio::test]
async fn une_reserve_d_identites_trop_courte_refuse_en_la_nommant() {
    /// Une source qui n'a qu'une identité à donner, quoi qu'on lui demande.
    #[derive(Debug)]
    struct Avare;

    impl Identities for Avare {
        fn events(&self, _count: usize) -> Result<Vec<Id<EventId>>, CommandError> {
            Ok(vec![id::<EventId>(1)])
        }

        fn command(&self) -> Result<Id<CommandId>, CommandError> {
            Ok(id::<CommandId>(2))
        }

        fn lease(&self) -> Result<Id<CommandId>, CommandError> {
            Ok(id::<CommandId>(3))
        }
    }

    let en_file = en_file();
    let tache = en_file.mission.task_id.clone();
    let file = Arc::new(MemoryQueue::new());
    file.push(en_file);
    let registre = Arc::new(MemoryRegistry::new());
    registre.admit(CREANCE, identite());
    let adresse = servir(Runtime::in_memory().with_lep(
        Desk::new(file, registre, Arc::new(Avare)).placing(Arc::new(BrokerDeTest::placant())),
    ))
    .await;
    let _ = poster(&adresse, CLAIM_PATH, Some(CREANCE), &corps_minimal("")).await;

    // Deux événements, une seule identité disponible.
    let mut premier: Event = fixture("event-reconnection-1-started.json");
    premier.task_id = Some(tache.clone());
    premier.worker_id = Some(WORKER.to_owned());
    let mut second: Event = fixture("event-reconnection-3-tool-completed.json");
    second.task_id = Some(tache);
    second.worker_id = Some(WORKER.to_owned());
    let evenements = serde_json::to_string(&vec![premier, second]).expect("sérialisable");

    let reponse = poster(
        &adresse,
        EVENTS_PATH,
        Some(CREANCE),
        &corps("idem-avare", &format!(",\"events\":{evenements}")),
    )
    .await;

    assert!(
        reponse.starts_with("HTTP/1.1 400"),
        "une réserve trop courte se dit, elle ne se comble pas :\n{reponse}"
    );
    assert!(
        reponse.contains("context.event_ids"),
        "le refus nomme ce qui manque :\n{reponse}"
    );
    assert_eq!(
        faits_sur(&adresse, "task/task-nominal").await,
        vec!["task.leased"],
        "et rien n'est écrit : un lot s'écrit d'un bloc ou pas du tout"
    );
}

/// **`NoIdentities` refuse les deux, indépendamment.**
///
/// Une passe de mutation a laissé vivant le remplacement de son refus d'identités d'événement par
/// une liste vide : sur le chemin d'écriture, l'autre méthode refusait juste après, et le statut
/// rendu ne changeait pas. Le mutant était donc masqué, pas inoffensif — un appelant qui n'aurait
/// besoin que d'identités d'événement recevrait un `Ok` vide, c'est-à-dire un silence.
///
/// Le contrat du port se vérifie donc **méthode par méthode**, sans passer par un chemin qui les
/// appelle toutes les deux.
#[test]
fn la_source_par_defaut_refuse_les_deux_sortes_d_identifiants() {
    let refus = NoIdentities
        .events(1)
        .expect_err("aucune identité n'est fabriquée");
    assert!(
        matches!(refus, CommandError::Unavailable { .. }),
        "le service ne peut pas répondre maintenant — ce n'est pas un défaut du code : {refus:?}"
    );
    assert!(
        NoIdentities.events(0).is_err(),
        "même pour zéro : une source absente ne devient pas présente parce qu'on ne lui demande rien"
    );
    assert!(matches!(
        NoIdentities
            .command()
            .expect_err("aucune identité de commande non plus"),
        CommandError::Unavailable { .. }
    ),);
}

/// **Ce que `W20.k` rendait observable, `W20.l` le corrige : les projections voient les écritures.**
///
/// Ce test était écrit à l'envers il y a un sprint, et **c'était son objet**. Il attestait que
/// `/workers` restait vide alors qu'un worker avait réclamé et rendu — `catch_up` prenait
/// `&mut self`, la liaison HTTP ne tient qu'un `&Runtime` —, et il disait en toutes lettres qu'il
/// rougirait le jour où `W20.l` serait livré. Il a rougi. Une limite tue se redécouvre en
/// production ; une limite testée se signale d'elle-même au moment où elle cesse d'exister.
///
/// Ce qu'il vérifie maintenant est la propriété que l'item promet : **sans redémarrage**, le graphe
/// d'exécution nomme le worker qui a réclamé.
#[tokio::test]
async fn les_projections_voient_ce_que_la_surface_ecrit() {
    let en_file = en_file();
    let tache = en_file.mission.task_id.clone();
    let adresse = servir(daemon(vec![en_file])).await;
    let _ = poster(&adresse, CLAIM_PATH, Some(CREANCE), &corps_minimal("")).await;
    let rendu = corps(
        "idem-result",
        &format!(
            ",\"task_id\":\"{tache}\",\"attempt_id\":\"a\",\"session_id\":\"s\",\"output\":{{}}"
        ),
    );
    let _ = poster(&adresse, RESULT_PATH, Some(CREANCE), &rendu).await;

    // Le journal a les deux faits.
    assert_eq!(
        faits_sur(&adresse, "task/task-nominal").await,
        vec!["task.leased", "run.completed"]
    );

    // Et le graphe d'exécution les a vus — c'est le test de sortie de `W20.l`.
    let reponse = demander(&adresse, "/workers").await;
    assert!(
        reponse.contains(WORKER),
        "le worker de la créance doit être visible sans redémarrage :\n{reponse}"
    );
}

/// **Et c'est bien la créance qui nomme le worker, pas le corps de la requête.**
///
/// Le seizième mutant de la passe de `W20.k` était rapporté vivant faute de chemin public exposant
/// la charge d'un fait : `/timeline` et `/events` n'en portent pas, et `/workers` — qui la lit par
/// le graphe d'exécution — était inerte. `W20.l` l'a rendue lisible, et le mutant meurt ici.
///
/// La propriété est celle de §7 : un worker ne parle pas au nom d'un autre. Le corps annonce ce
/// qu'il veut ; ce qui atteint le journal est ce que la créance identifie.
#[tokio::test]
async fn le_worker_du_journal_est_celui_de_la_creance() {
    let en_file = en_file();
    let tache = en_file.mission.task_id.clone();
    let adresse = servir(daemon(vec![en_file])).await;
    let _ = poster(&adresse, CLAIM_PATH, Some(CREANCE), &corps_minimal("")).await;

    // Le corps annonce un autre worker sur un champ que le daemon ne lit pas — et ce nom-là ne doit
    // apparaître nulle part dans le graphe.
    let rendu = corps(
        "idem-result",
        &format!(
            ",\"task_id\":\"{tache}\",\"attempt_id\":\"a\",\"session_id\":\"s\",\"worker_id\":\"usurpateur\",\"output\":{{}}"
        ),
    );
    let _ = poster(&adresse, RESULT_PATH, Some(CREANCE), &rendu).await;

    let reponse = demander(&adresse, "/workers").await;
    assert!(
        reponse.contains(WORKER),
        "la créance nomme le worker :\n{reponse}"
    );
    assert!(
        !reponse.contains("usurpateur"),
        "ce que le corps annonce ne fait pas foi :\n{reponse}"
    );
}

/// **Une projection en quarantaine ne bloque pas l'écriture canonique** — §9.5, promesse de `W1.d`.
///
/// C'est la clause que `W20.l` avait le plus de chances de trahir : le rattrapage vit désormais
/// dans le chemin d'écriture, donc c'est de là qu'une faute de projection pourrait remonter jusqu'à
/// faire échouer une commande. `catch_up` ne rend jamais d'erreur, et ce test le vérifie du dehors
/// plutôt que de le croire.
///
/// La quarantaine est provoquée par un fait **réel** que le graphe d'exécution refuse — un
/// `artifact.declared` sans `artifact_id` —, et non par une projection d'épreuve : une fixture
/// fautive éprouverait le harnais, pas la promesse.
#[tokio::test]
async fn une_projection_en_quarantaine_ne_bloque_pas_l_ecriture() {
    let en_file = en_file();
    let tache = en_file.mission.task_id.clone();
    let adresse = servir(daemon(vec![en_file])).await;
    let _ = poster(&adresse, CLAIM_PATH, Some(CREANCE), &corps_minimal("")).await;

    // `artifact.declared` sans `artifact_id` : le graphe d'exécution met en quarantaine.
    let mut fautif: Event = fixture("event-reconnection-1-started.json");
    fautif.event_type = "artifact.declared".to_owned();
    fautif.task_id = Some(tache.clone());
    fautif.worker_id = Some(WORKER.to_owned());
    fautif.payload = Some(serde_json::json!({ "task_id": tache }));
    let evenements = serde_json::to_string(&vec![fautif]).expect("sérialisable");
    let refus = poster(
        &adresse,
        EVENTS_PATH,
        Some(CREANCE),
        &corps("idem-fautif", &format!(",\"events\":{evenements}")),
    )
    .await;
    assert!(
        refus.starts_with("HTTP/1.1 202"),
        "l'écriture aboutit : c'est la projection qui a un problème, pas le journal :\n{refus}"
    );

    // Et l'écriture suivante aboutit encore.
    let rendu = corps(
        "idem-result",
        &format!(
            ",\"task_id\":\"{tache}\",\"attempt_id\":\"a\",\"session_id\":\"s\",\"output\":{{}}"
        ),
    );
    let apres = poster(&adresse, RESULT_PATH, Some(CREANCE), &rendu).await;
    assert!(
        apres.starts_with("HTTP/1.1 202"),
        "une projection en quarantaine ne bloque pas l'écriture canonique :\n{apres}"
    );

    // Le journal a tout, et le rapport de disponibilité dit laquelle va mal.
    assert_eq!(
        faits_sur(&adresse, "task/task-nominal").await,
        vec!["task.leased", "artifact.declared", "run.completed"]
    );
    // La quarantaine est **lue**, pas supposée : sans cette assertion, le test passerait aussi bien
    // si le fait fautif n'avait jamais atteint la projection — et il ne prouverait alors rien de la
    // promesse de `W1.d`. Un compteur qui n'a rien lu ne vaut pas zéro.
    let sante = demander(&adresse, "/projections/status").await;
    assert!(
        sante.contains("{\"name\":\"execution_graph\",\"healthy\":false}"),
        "le graphe d'exécution doit être en quarantaine — sinon rien n'a été éprouvé :\n{sante}"
    );
    assert!(
        sante.contains("\"ready\":false"),
        "un daemon dont une projection est en quarantaine ne se dit pas prêt :\n{sante}"
    );
}

/// **Une lecture ne fait pas avancer les projections.**
///
/// `readiness()` le refusait déjà, et cela doit le rester : une query qui rattraperait rendrait le
/// résultat dépendant de qui a lu en dernier — deux clients identiques verraient deux états, et le
/// second ne saurait pas pourquoi.
///
/// `W20.l` déplace le rattrapage dans le chemin d'**écriture** précisément pour cela. Ce test lit
/// deux fois de suite après une écriture faite hors de ce chemin, et exige que les deux lectures
/// rendent la même chose.
#[tokio::test]
async fn une_lecture_ne_fait_pas_avancer_les_projections() {
    let adresse = servir(daemon(Vec::new())).await;

    let premiere = demander(&adresse, "/workers").await;
    let seconde = demander(&adresse, "/workers").await;

    let corps_de = |reponse: &str| {
        reponse
            .split_once("\r\n\r\n")
            .map_or(String::new(), |(_, corps)| corps.to_owned())
    };
    assert_eq!(
        corps_de(&premiere),
        corps_de(&seconde),
        "deux lectures consécutives rendent le même état"
    );
}

// ---------------------------------------------------------------------------------------------
// 6. Le placement est **demandé**, jamais décidé ici — `W20.q`.
// ---------------------------------------------------------------------------------------------

/// **La réclamation passe au broker le manifeste du worker et l'exigence de la mission.**
///
/// C'est la moitié de l'item qu'un verdict seul ne prouverait pas : un daemon qui poserait la
/// question avec un manifeste bricolé, ou avec une exigence inventée, obtiendrait le même `Placed`
/// et passerait tous les tests d'un port muet. Ce test lit **ce qui a été demandé**, pas ce qui a
/// été répondu.
#[tokio::test]
async fn la_reclamation_soumet_le_manifeste_du_worker_et_l_exigence_de_la_mission() {
    let en_file = en_file();
    let exigence = en_file.mission.sandbox.clone();
    let reservation = en_file.mission.resources.clone();
    let (runtime, broker) = daemon_brokere(vec![en_file], Arc::new(BrokerDeTest::placant()));
    let adresse = servir(runtime).await;

    let reponse = poster(&adresse, CLAIM_PATH, Some(CREANCE), &corps_minimal("")).await;
    assert!(reponse.starts_with("HTTP/1.1 200"), "{reponse}");

    let questions = broker.questions();
    assert_eq!(
        questions.len(),
        1,
        "une réclamation pose une question de placement, et une seule"
    );
    assert_eq!(
        questions[0].0,
        manifeste(),
        "le manifeste soumis est celui que le worker a annoncé, champ pour champ"
    );
    assert_eq!(
        questions[0].1, exigence,
        "l'exigence soumise est celle de la mission — un plancher, pas un souhait recomposé"
    );
    assert_eq!(
        questions[0].2, reservation,
        "la réservation soumise est celle de la mission (invariant 6)"
    );
}

/// **Un placement refusé rend `204`, et la mission reste en file.**
///
/// Les deux moitiés comptent. `204` parce que « rien pour toi » est exact : il y a du travail, mais
/// pas pour cet hôte-là. Et la mission **revient** parce que `take` retire : sans remise, un worker
/// macOS qui sonde une file portant une mission `S3` la ferait disparaître, et le worker Linux qui
/// pouvait la porter ne la verrait jamais. Aucun journal ne montrerait cette perte-là.
#[tokio::test]
async fn un_placement_refuse_rend_204_et_laisse_la_mission_en_file() {
    let file = Arc::new(MemoryQueue::new());
    file.push(en_file());
    let registre = Arc::new(MemoryRegistry::new());
    registre.admit(CREANCE, identite());
    let runtime = Runtime::in_memory().with_lep(
        Desk::new(
            Arc::clone(&file) as Arc<dyn locusd::MissionQueue>,
            registre,
            Arc::new(IdentitesDeTest::default()),
        )
        .placing(Arc::new(BrokerDeTest::refusant())),
    );
    let adresse = servir(runtime).await;

    let reponse = poster(&adresse, CLAIM_PATH, Some(CREANCE), &corps_minimal("")).await;

    assert!(
        reponse.starts_with("HTTP/1.1 204"),
        "« pas pour cet hôte » est du calme, pas une panne :\n{reponse}"
    );
    assert_eq!(
        file.len(),
        1,
        "une mission qu'on n'a pas confiée retourne dans la file : la perdre la retirerait à qui \
         pouvait la porter"
    );
    assert!(
        faits_sur(&adresse, "task/task-nominal").await.is_empty(),
        "rien n'a été confié, donc rien n'est écrit"
    );
}

/// **Un broker injoignable rend `unavailable`, et non `204`.**
///
/// ADR 0028 décision 4, tenue jusque dans le code de statut : « je n'ai pas pu demander » envoie
/// démarrer un service ou vérifier un chemin de socket ; « rien pour toi » dit d'attendre. Un
/// worker qui recevrait `204` sur un lien coupé attendrait en silence un ordonnanceur qui avait du
/// travail — et personne ne saurait pourquoi rien n'avance.
#[tokio::test]
async fn un_broker_injoignable_rend_unavailable_et_non_204() {
    let file = Arc::new(MemoryQueue::new());
    file.push(en_file());
    let registre = Arc::new(MemoryRegistry::new());
    registre.admit(CREANCE, identite());
    let runtime = Runtime::in_memory().with_lep(
        Desk::new(
            Arc::clone(&file) as Arc<dyn locusd::MissionQueue>,
            registre,
            Arc::new(IdentitesDeTest::default()),
        )
        .placing(Arc::new(BrokerDeTest::injoignable())),
    );
    let adresse = servir(runtime).await;

    let reponse = poster(&adresse, CLAIM_PATH, Some(CREANCE), &corps_minimal("")).await;

    assert!(
        reponse.starts_with("HTTP/1.1 503"),
        "un broker injoignable est une indisponibilité, pas du calme :\n{reponse}"
    );
    assert!(
        reponse.contains("injoignable"),
        "le refus dit où regarder :\n{reponse}"
    );
    assert_eq!(
        file.len(),
        1,
        "on n'a pas pu demander : la mission reste en file, et le worker retentera"
    );
}

/// **Une réclamation sans manifeste est refusée, et n'entame pas la file.**
///
/// §15.3 : un worker annonce ce qu'il sait faire avant qu'on lui confie quoi que ce soit. Servir la
/// première mission venue à qui n'annonce rien est **exactement** ce que `W20.q` corrige — la file
/// de `W20.k` le faisait, et sa propre documentation le disait.
#[tokio::test]
async fn une_reclamation_sans_manifeste_est_refusee_et_ne_consomme_rien() {
    let file = Arc::new(MemoryQueue::new());
    file.push(en_file());
    let registre = Arc::new(MemoryRegistry::new());
    registre.admit(CREANCE, identite());
    let runtime = Runtime::in_memory().with_lep(
        Desk::new(
            Arc::clone(&file) as Arc<dyn locusd::MissionQueue>,
            registre,
            Arc::new(IdentitesDeTest::default()),
        )
        .placing(Arc::new(BrokerDeTest::placant())),
    );
    let adresse = servir(runtime).await;

    let reponse = poster(
        &adresse,
        CLAIM_PATH,
        Some(CREANCE),
        &corps("idem-sans-manifeste", ""),
    )
    .await;

    assert!(
        reponse.starts_with("HTTP/1.1 400"),
        "un manifeste manquant est une requête à corriger :\n{reponse}"
    );
    assert!(
        reponse.contains("manifest"),
        "le refus nomme le champ, il ne dit pas « requête invalide » :\n{reponse}"
    );
    assert_eq!(
        file.len(),
        1,
        "une réclamation refusée avant tout placement ne retire rien"
    );
}

/// **Un manifeste au nom d'un autre worker ne réclame rien.**
///
/// La créance dit qui parle ; le manifeste dit ce que la machine sait faire. Les laisser diverger
/// ferait placer sur les capacités d'une machine et exécuter sur une autre — un downgrade
/// silencieux au sens de §21.6, obtenu sans jamais toucher au niveau demandé.
///
/// C'est la règle que [`Report`] applique déjà à un événement, et le refus est de la même famille.
#[tokio::test]
async fn un_manifeste_au_nom_d_un_autre_worker_est_refuse() {
    let file = Arc::new(MemoryQueue::new());
    file.push(en_file());
    let registre = Arc::new(MemoryRegistry::new());
    registre.admit(CREANCE, identite());
    let runtime = Runtime::in_memory().with_lep(
        Desk::new(
            Arc::clone(&file) as Arc<dyn locusd::MissionQueue>,
            registre,
            Arc::new(IdentitesDeTest::default()),
        )
        .placing(Arc::new(BrokerDeTest::placant())),
    );
    let adresse = servir(runtime).await;

    let mut usurpe = manifeste();
    "un-autre-worker".clone_into(&mut usurpe.worker_id);
    let annonce = serde_json::to_string(&usurpe).expect("sérialisable");
    let reponse = poster(
        &adresse,
        CLAIM_PATH,
        Some(CREANCE),
        &corps("idem-usurpe", &format!(",\"manifest\":{annonce}")),
    )
    .await;

    assert!(
        reponse.starts_with("HTTP/1.1 403"),
        "annoncer les capacités d'un autre est une faute d'autorisation :\n{reponse}"
    );
    assert_eq!(file.len(), 1, "un refus n'entame pas la file");
}

/// **`locusd` ne décide toujours d'aucun hôte.**
///
/// La même forme d'absence que `W20.o` tient pour la création de mission, appliquée au chemin qui
/// aurait maintenant le plus de raisons d'y contrevenir : celui qui **demande** un placement. Ce
/// module transmet un manifeste et lit un verdict ; il ne compare aucun niveau, ne trie aucun
/// candidat, et n'importe rien de `locus-execd`.
///
/// `Placement` figure dans la liste des permis — c'est le type de la **réponse**, pas de la
/// décision. Ce qui est interdit est la décision : `place(`, `Candidate`, `shortfall`, `admit(`.
#[test]
fn reclamer_ne_choisit_aucun_hote() {
    let source = include_str!("../src/lep.rs");
    for interdit in [
        "Candidate",
        "shortfall",
        "Admission",
        "RefusalReason",
        "locus_execd",
        "HostCapabilities",
        "proven_level",
    ] {
        assert!(
            !source.contains(interdit),
            "« {interdit} » dans la surface §15.2 : le placement est celui de `W4.g`, chez \
             `locus-execd`, et `locusd` se contente de le demander"
        );
    }

    // Et l'absence seule ne suffirait pas : un module qui ne déciderait rien **et** ne demanderait
    // rien la tiendrait aussi. Le seul `place` de ce fichier est donc un appel au port, et il n'y
    // en a qu'un — deux chemins de placement seraient deux politiques.
    assert_eq!(
        source.matches(".place(").count(),
        1,
        "un seul chemin demande un placement"
    );
    assert!(
        source.contains("broker.place("),
        "et il passe par le port du broker, jamais par un calcul local"
    );
}

// ---------------------------------------------------------------------------------------------
// 7. Le bail frappé à la réclamation — `W20.v`, et ce qu'une passe de mutation a trouvé.
// ---------------------------------------------------------------------------------------------

/// **Sans identité de bail, la réclamation est refusée — et la mission reste en file.**
///
/// Deux propriétés, et une passe de mutation les a trouvées toutes deux sans test.
///
/// La première : le `lease_id` vient de [`Identities::lease`], **pas** de
/// [`Identities::command`]. §11.1 refuse qu'une identité soit substituée à une autre, et le port
/// porte deux méthodes pour cela. Emprunter l'une pour l'autre passait inaperçu tant qu'aucune
/// source ne les distinguait.
///
/// La seconde : quand l'identité manque, la mission **retourne dans la file**. Elle n'a pas été
/// confiée ; la perdre ici la retirerait à qui pouvait la porter, et rien ne le dirait — c'est la
/// règle que `W20.q` a posée pour un refus de placement, et elle vaut sur tous les chemins qui
/// renoncent après avoir retiré.
#[tokio::test]
async fn sans_identite_de_bail_la_mission_reste_en_file() {
    /// Une source qui sait tout donner **sauf** un bail.
    #[derive(Debug, Default)]
    struct SansBail {
        prochain: std::sync::atomic::AtomicU8,
    }

    impl Identities for SansBail {
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
            Err(CommandError::Unavailable {
                detail: "aucune source d'identifiant de bail n'est câblée".to_owned(),
            })
        }
    }

    let file = Arc::new(MemoryQueue::new());
    file.push(en_file());
    let registre = Arc::new(MemoryRegistry::new());
    registre.admit(CREANCE, identite());
    let runtime = Runtime::in_memory().with_lep(
        Desk::new(
            Arc::clone(&file) as Arc<dyn locusd::MissionQueue>,
            registre,
            Arc::new(SansBail::default()),
        )
        .placing(Arc::new(BrokerDeTest::placant())),
    );
    let adresse = servir(runtime).await;

    let reponse = poster(&adresse, CLAIM_PATH, Some(CREANCE), &corps_minimal("")).await;

    assert!(
        reponse.starts_with("HTTP/1.1 503"),
        "sans identité, le service ne peut pas répondre maintenant :\n{reponse}"
    );
    assert!(
        reponse.contains("bail"),
        "le refus nomme ce qui manque — et « bail » n'est pas « commande » :\n{reponse}"
    );
    assert_eq!(
        file.len(),
        1,
        "la mission n'a pas été confiée : elle retourne dans la file"
    );
    assert!(
        faits_sur(&adresse, "task/task-nominal").await.is_empty(),
        "et rien n'est écrit"
    );
}

/// **Une échéance qui déborderait est refusée, jamais repliée.**
///
/// Aucune horloge réelle n'en approche, et c'est précisément pourquoi la garde avait besoin d'un
/// test : une passe de mutation a remplacé l'addition vérifiée par une addition qui se replie, sans
/// faire rougir quoi que ce soit.
///
/// Ce qu'un repli produirait n'est pas une valeur bizarre, c'est un **bail né expiré** — que `W2.9`
/// traite comme un bail perdu. Le worker rendrait aussitôt la tâche qu'on vient de lui confier, et
/// la boucle recommencerait sans que rien ne dise pourquoi.
#[test]
fn une_echeance_qui_deborde_est_refusee() {
    let deborde = locusd::expiration(Timestamp::from_millis(i64::MAX), LEASE_TTL_SECONDS)
        .expect_err("l'échéance déborde");
    assert_eq!(deborde.family(), locusd::Family::Internal);

    // Et le cas ordinaire, sans quoi le test précédent passerait pour une fonction qui refuse tout.
    let ordinaire =
        locusd::expiration(Timestamp::from_millis(1_700_000_000_000), LEASE_TTL_SECONDS)
            .expect("une échéance ordinaire se calcule");
    assert_eq!(
        ordinaire.millis(),
        1_700_000_000_000 + LEASE_TTL_SECONDS * 1_000,
        "l'échéance est l'instant plus le TTL, en millisecondes"
    );
}

// ---------------------------------------------------------------------------------------------
// `W20.aa` — le `204` cesse d'être ambigu.
// ---------------------------------------------------------------------------------------------

/// **Un placement refusé le dit, en nommant la tâche et ce qui manquait.**
///
/// Le `204` reste vide — ADR 0028 décision 4, et le détail des manques d'un hôte n'est rien qu'une
/// créance de worker donne le droit de connaître. Ce qui change est qu'un **exploitant** peut
/// désormais savoir pourquoi sa chaîne ne place rien : `Runtime::placed` jetait les `shortfalls`,
/// et une sonde de session s'est arrêtée là, faute de pouvoir distinguer ce refus d'une file vide.
#[tokio::test]
async fn un_placement_refuse_dit_ce_qui_manquait() {
    let (runtime, puits) = daemon_observe(vec![en_file()], Arc::new(BrokerDeTest::refusant()));
    let adresse = servir(runtime).await;

    let reponse = poster(&adresse, CLAIM_PATH, Some(CREANCE), &corps_minimal("")).await;

    assert!(
        reponse.starts_with("HTTP/1.1 204"),
        "« rien pour toi » reste un 204 :\n{reponse}"
    );
    let notes = puits.notes();
    assert_eq!(notes.len(), 1, "une note, et une seule : {notes:?}");
    // La tâche est celle de la fixture, lue d'elle plutôt que recopiée : une constante écrite à la
    // main ici passerait encore si la fixture changeait de tâche.
    assert!(
        notes[0].contains(&en_file().mission.task_id),
        "{}",
        notes[0]
    );
    assert!(notes[0].contains("changer de machine"), "{}", notes[0]);
    // Le corps du 204 ne porte rien : la note est pour l'exploitant, pas pour le worker.
    assert!(
        !reponse.contains("changer de machine"),
        "le détail des manques ne part pas vers le worker :\n{reponse}"
    );
}

/// **Une file vide ne dit rien — et ce silence est le renseignement.**
///
/// L'asymétrie est la propriété entière. Un worker sonde en boucle ; écrire une ligne par sondage
/// remplirait n'importe quel journal et rendrait les vraies notes illisibles. Comme seul le refus
/// parle, l'**absence** de note veut dire « la file n'avait rien » — ce qui lève l'ambiguïté du
/// `204` sans coûter une ligne par tour.
#[tokio::test]
async fn une_file_vide_ne_dit_rien() {
    let (runtime, puits) = daemon_observe(Vec::new(), Arc::new(BrokerDeTest::placant()));
    let adresse = servir(runtime).await;

    let reponse = poster(&adresse, CLAIM_PATH, Some(CREANCE), &corps_minimal("")).await;

    assert!(reponse.starts_with("HTTP/1.1 204"), "{reponse}");
    assert_eq!(
        puits.notes(),
        Vec::<String>::new(),
        "un sondage sur file vide n'écrit rien"
    );
}
