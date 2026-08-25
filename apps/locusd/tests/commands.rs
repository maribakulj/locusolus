//! Le test de sortie de `W20.s` — les deux commandes de §22.3, servies.
//!
//! # Ce qui manquait, et comment on l'a su
//!
//! `W20.o` a livré `lep_propose` et `lep_queue`, et **aucune route ne les appelait** : zéro
//! occurrence dans `http.rs`. La tentative de `W12.d` l'a constaté avant d'écrire une ligne, et la
//! conséquence était nette — « une question produit une mission » n'était déclenchable que depuis
//! l'intérieur du processus, donc un test de bout en bout ne pouvait pas **commencer**.
//!
//! # Ce que l'écriture de ces routes a trouvé au passage
//!
//! Deux trous dans `lep_queue`, corrigés ici parce qu'une route les aurait rendus publics. Elle
//! prenait la proposition de son appelant — donc on pouvait proposer une question et en mettre une
//! autre en file sous le même identifiant de tâche, sans qu'aucun fait ne montre la divergence — et
//! elle prenait l'état courant de son appelant, donc la garde de §7.1 validait ce qu'on lui
//! annonçait. Les deux se lisent maintenant du journal.

use std::fmt::Write as _;
use std::sync::Arc;

use locus_broker::port::{BrokerPort, Loopback};
use locus_broker::protocol::Verdict;
use locus_domain::task::TaskState;
use locus_lep::{MissionEnvelopeBudget, NetworkMode, ResourceSpec, SandboxLevel};
use locus_protocol::id::{Agent, Command as CommandId, Event as EventId, Project, Workspace};
use locus_protocol::{Id, IdKind, Timestamp};
use locusd::http::{PROPOSE_PATH, QUEUE_PATH, router};
use locusd::lep::{Desk, Identities, MemoryQueue, MemoryRegistry, WorkerIdentity};
use locusd::mission::{Authority, MemoryAdministrators, Proposal};
use locusd::{CommandError, MissionQueue, Runtime};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// La créance d'un exploitant — celle qui ouvre §22.3.
const ADMIN: &str = "creance-d-administration";
/// La créance d'un **worker** — parfaitement valide sur `/lep/`, et qui ne doit rien ouvrir ici.
const WORKER_CREANCE: &str = "creance-de-worker";
const WORKER: &str = "canterel-vm-linux-01";
const TACHE: &str = "tsk_catalyseur";

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

fn proposition() -> Proposal {
    Proposal {
        cognition: locus_domain::CognitionClass::Economy,
        statement: "Le catalyseur A tient-il au-delà de 300 °C ?".to_owned(),
        success_conditions: vec!["une mesure reproductible à trois essais".to_owned()],
        task_id: TACHE.to_owned(),
        attempt_id: "att_1".to_owned(),
        attempt: 3,
        branch_id: "br_principal".to_owned(),
        context_view_id: "ctx_1".to_owned(),
        context_view_hash: "sha256:".to_owned() + &"ab".repeat(32),
        environment_id: "env_linux".to_owned(),
        sandbox_level: SandboxLevel::S2,
        network: NetworkMode::Deny,
        resources: ResourceSpec {
            cpu: 2.0,
            memory_mb: 4096,
            disk_mb: 8192,
            wall_time_seconds: 900,
            accelerator: None,
        },
        budget: MissionEnvelopeBudget {
            max_model_calls: 40,
            max_input_tokens: 200_000,
            max_output_tokens: 40_000,
            max_cost_micros: None,
        },
        output_contract: "un rapport et ses mesures".to_owned(),
    }
}

/// Un daemon qui reconnaît **un** exploitant et **un** worker, dans deux registres distincts.
fn daemon() -> (
    Runtime<locus_event_store::MemoryEventStore>,
    Arc<MemoryQueue>,
) {
    let file = Arc::new(MemoryQueue::new());
    let registre = Arc::new(MemoryRegistry::new());
    registre.admit(
        WORKER_CREANCE,
        WorkerIdentity {
            worker_id: WORKER.to_owned(),
            workspace_id: id::<Workspace>(2),
            principal_id: id::<Agent>(3),
            project_id: id::<Project>(4),
        },
    );
    let exploitants = Arc::new(MemoryAdministrators::new());
    exploitants.admit(ADMIN, autorite());
    let broker: Arc<dyn BrokerPort + Send + Sync> =
        Arc::new(Loopback::answering(Verdict::Placed {
            worker: WORKER.to_owned(),
            level: SandboxLevel::S3,
        }));
    let runtime = Runtime::in_memory().with_lep(
        Desk::new(
            Arc::clone(&file) as Arc<dyn MissionQueue>,
            registre,
            Arc::new(Identites::default()),
        )
        .placing(broker)
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

fn corps_propose(cle: &str) -> String {
    format!(
        "{{\"idempotency_key\":\"{cle}\",\"project_id\":\"{}\",\"proposal\":{}}}",
        id::<Project>(4),
        serde_json::to_string(&proposition()).expect("la proposition se sérialise")
    )
}

fn corps_queue(cle: &str, tache: &str) -> String {
    format!(
        "{{\"idempotency_key\":\"{cle}\",\"project_id\":\"{}\",\"task_id\":\"{tache}\"}}",
        id::<Project>(4)
    )
}

async fn faits(adresse: &str) -> Vec<String> {
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
                .filter_map(|item| item["event_type"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------------------------
// 1. Une question produit une mission — de bout en bout, sur le fil.
// ---------------------------------------------------------------------------------------------

/// **Proposer puis mettre en file dépose la mission que la question décrit.**
///
/// C'est la première clause de `W12.d`, et jusqu'ici elle n'était atteignable que depuis l'intérieur
/// du processus. La comparaison est **champ pour champ**, pas par présence : une mission qui
/// arriverait avec le bon `task_id` et un objectif vide passerait une assertion d'existence.
#[tokio::test]
async fn une_question_posee_sur_le_fil_produit_une_mission() {
    let (runtime, file) = daemon();
    let adresse = servir(runtime).await;

    let propose = poster(
        &adresse,
        PROPOSE_PATH,
        Some(ADMIN),
        &corps_propose("idem-p"),
    )
    .await;
    assert!(
        propose.starts_with("HTTP/1.1 202"),
        "une proposition est acceptée :\n{propose}"
    );
    assert!(
        file.is_empty(),
        "proposer ne met **rien** en file : §7.1 exige le passage par `queued`"
    );

    let queue = poster(
        &adresse,
        QUEUE_PATH,
        Some(ADMIN),
        &corps_queue("idem-q", TACHE),
    )
    .await;
    assert!(
        queue.starts_with("HTTP/1.1 202"),
        "une mise en file est acceptée :\n{queue}"
    );

    assert_eq!(
        faits(&adresse).await,
        vec!["task.proposed".to_owned(), "task.queued".to_owned()]
    );

    let en_file = file.take("peu-importe").expect("une mission attend");
    let attendue = proposition();
    assert_eq!(en_file.mission.task_id, attendue.task_id);
    assert_eq!(en_file.mission.objective.statement, attendue.statement);
    assert_eq!(
        en_file.mission.objective.success_conditions,
        attendue.success_conditions
    );
    assert_eq!(
        en_file.mission.sandbox.minimum_level,
        attendue.sandbox_level
    );
    assert_eq!(en_file.mission.resources, attendue.resources);
    assert_eq!(en_file.mission.budget, attendue.budget);
    assert_eq!(en_file.mission.output_contract, attendue.output_contract);
    assert_eq!(
        en_file.attempt, attendue.attempt,
        "le rang de §12.3 traverse la commande sans être recompté"
    );
}

// ---------------------------------------------------------------------------------------------
// 2. Une créance de worker n'ouvre pas §22.3.
// ---------------------------------------------------------------------------------------------

/// **Un worker ne se crée pas de travail.**
///
/// Sa créance est **valide** — le registre des workers la reconnaît, et elle ouvre les trois chemins
/// de §15.2. Elle ne figure simplement pas dans le registre d'administration, donc elle n'y résout
/// rien. La règle ne tient pas par une comparaison de rôle, qui se déplace ou s'inverse : elle tient
/// parce que ce sont **deux registres**.
#[tokio::test]
async fn une_creance_de_worker_n_ouvre_pas_les_commandes_de_22_3() {
    let (runtime, file) = daemon();
    let adresse = servir(runtime).await;

    for (chemin, corps) in [
        (PROPOSE_PATH, corps_propose("idem-usurpe")),
        (QUEUE_PATH, corps_queue("idem-usurpe", TACHE)),
    ] {
        let reponse = poster(&adresse, chemin, Some(WORKER_CREANCE), &corps).await;
        assert!(
            reponse.starts_with("HTTP/1.1 403"),
            "{chemin} sous une créance de worker est une faute d'autorisation :\n{reponse}"
        );
        assert!(
            !reponse.contains(WORKER_CREANCE),
            "une créance refusée ne se cite pas :\n{reponse}"
        );
    }

    assert!(file.is_empty(), "rien n'a été mis en file");
    assert!(faits(&adresse).await.is_empty(), "et rien n'a été écrit");
}

/// **Et sans porteur du tout, c'est `401`.**
///
/// Le pendant qui empêche le test précédent de passer pour de mauvaises raisons : un daemon qui
/// refuserait tout le monde le passerait aussi.
#[tokio::test]
async fn sans_creance_les_commandes_sont_refusees() {
    let (runtime, _) = daemon();
    let adresse = servir(runtime).await;

    let reponse = poster(&adresse, PROPOSE_PATH, None, &corps_propose("idem-nu")).await;

    assert!(reponse.starts_with("HTTP/1.1 401"), "{reponse}");
}

// ---------------------------------------------------------------------------------------------
// 3. §7.1 refuse toujours, et le refus le dit.
// ---------------------------------------------------------------------------------------------

/// **Mettre en file deux fois est refusé — et l'état vient du journal, pas de l'appelant.**
///
/// §7.1 ne connaît pas `queued → queued`. Le refus est de la famille `policy` : la requête est bien
/// formée, c'est l'état qui s'y oppose.
///
/// La propriété qui compte est **d'où vient l'état**. Tant que `lep_queue` le prenait de son
/// appelant, un client pouvait annoncer `proposed` une seconde fois et la garde de §7.1 aurait
/// validé le mensonge. Il se lit maintenant du dernier fait qui en porte un.
#[tokio::test]
async fn mettre_en_file_deux_fois_est_refuse_par_7_1() {
    let (runtime, file) = daemon();
    let adresse = servir(runtime).await;

    let _ = poster(
        &adresse,
        PROPOSE_PATH,
        Some(ADMIN),
        &corps_propose("idem-p"),
    )
    .await;
    let premier = poster(
        &adresse,
        QUEUE_PATH,
        Some(ADMIN),
        &corps_queue("idem-q1", TACHE),
    )
    .await;
    assert!(premier.starts_with("HTTP/1.1 202"), "{premier}");

    let second = poster(
        &adresse,
        QUEUE_PATH,
        Some(ADMIN),
        &corps_queue("idem-q2", TACHE),
    )
    .await;

    assert!(
        second.starts_with("HTTP/1.1 409") || second.starts_with("HTTP/1.1 422"),
        "une transition que §7.1 refuse n'est ni un défaut du client ni une panne :\n{second}"
    );
    assert!(
        second.contains("queued"),
        "le refus nomme la transition qu'il refuse :\n{second}"
    );
    assert_eq!(
        file.len(),
        1,
        "une seule mise en file a réussi, donc une seule mission attend"
    );
}

/// **Mettre en file une tâche qui n'a pas été proposée est refusé en nommant le champ.**
///
/// Le journal ne porte rien sous cet identifiant : il n'y a pas de proposition à relire, donc rien
/// à mettre en file. C'est une faute du client — il a nommé une tâche qui n'existe pas.
#[tokio::test]
async fn mettre_en_file_une_tache_inconnue_est_refuse() {
    let (runtime, file) = daemon();
    let adresse = servir(runtime).await;

    let reponse = poster(
        &adresse,
        QUEUE_PATH,
        Some(ADMIN),
        &corps_queue("idem-fantome", "tsk_qui_n_existe_pas"),
    )
    .await;

    assert!(reponse.starts_with("HTTP/1.1 400"), "{reponse}");
    assert!(
        reponse.contains("task_id"),
        "le refus nomme le champ :\n{reponse}"
    );
    assert!(file.is_empty());
}

/// **Un corps sans le champ qu'exige la commande est refusé par le type, en le nommant.**
///
/// Ce test vient d'un survivant de mutation. `task_id` était un `Option` que le handler dépliait à
/// la main : remplacer ce dépliage par une chaîne vide ne cassait rien, parce que la tâche « » était
/// ensuite cherchée dans le journal, absente, et refusée — sous un message qui parlait d'une tâche
/// inexistante alors que le client, lui, avait simplement omis le champ.
///
/// La garde a donc été **supprimée** plutôt que testée : les deux corps sont maintenant deux types,
/// et le champ y est obligatoire. Ce n'est plus un `if` que deux handlers doivent tenir, c'est
/// serde qui refuse, et il nomme le champ lui-même. Ce test tient les deux chemins à la fois, parce
/// qu'un seul type rendu obligatoire aurait laissé l'autre revenir en arrière sans bruit.
#[tokio::test]
async fn un_corps_ampute_est_refuse_en_nommant_le_champ() {
    let (runtime, file) = daemon();
    let adresse = servir(runtime).await;
    let projet = id::<Project>(4);

    for (cible, absent, corps) in [
        (
            QUEUE_PATH,
            "task_id",
            format!("{{\"idempotency_key\":\"i\",\"project_id\":\"{projet}\"}}"),
        ),
        (
            PROPOSE_PATH,
            "proposal",
            format!("{{\"idempotency_key\":\"i\",\"project_id\":\"{projet}\"}}"),
        ),
    ] {
        let reponse = poster(&adresse, cible, Some(ADMIN), &corps).await;
        assert!(reponse.starts_with("HTTP/1.1 400"), "{cible} :\n{reponse}");
        assert!(
            reponse.contains(absent),
            "le refus de {cible} nomme « {absent} » :\n{reponse}"
        );
        // Le champ que `CommandError::Validation` nomme est **`body`**, et c'est là le point : le
        // corps a été refusé à la lecture. Un défaut par champ le ferait nommer `task_id`, après
        // avoir cherché dans le journal une tâche « » que personne n'a proposée — même statut,
        // même mot dans le message, autre chose dite. C'est ce que le survivant exploitait.
        assert!(
            reponse.contains("« body »"),
            "le refus de {cible} vient de la lecture du corps, pas d'une recherche au \
             journal :\n{reponse}"
        );
    }
    assert!(
        file.is_empty(),
        "un corps amputé n'atteint jamais le daemon, donc ne dépose rien"
    );
    assert!(
        faits(&adresse).await.is_empty(),
        "et n'écrit aucun fait non plus"
    );
}

// ---------------------------------------------------------------------------------------------
// 4. L'idempotence de §22.5.
// ---------------------------------------------------------------------------------------------

/// **Deux propositions identiques produisent une tâche, pas deux.**
///
/// La clé est celle du client — c'est lui qui sait qu'il retente —, et elle est scopée par
/// `(workspace, principal)`, tous deux lus du registre d'administration.
#[tokio::test]
async fn deux_propositions_sous_la_meme_cle_produisent_une_tache() {
    let (runtime, _) = daemon();
    let adresse = servir(runtime).await;

    let premier = poster(
        &adresse,
        PROPOSE_PATH,
        Some(ADMIN),
        &corps_propose("idem-p"),
    )
    .await;
    let second = poster(
        &adresse,
        PROPOSE_PATH,
        Some(ADMIN),
        &corps_propose("idem-p"),
    )
    .await;

    assert!(premier.starts_with("HTTP/1.1 202"), "{premier}");
    assert!(
        second.starts_with("HTTP/1.1 202"),
        "une resoumission rend le verdict d'origine, elle n'échoue pas :\n{second}"
    );
    assert_eq!(
        faits(&adresse).await,
        vec!["task.proposed".to_owned()],
        "deux envois de la même clé produisent un fait, pas deux"
    );
}

// ---------------------------------------------------------------------------------------------
// 5. La proposition mise en file est celle du journal, pas celle qu'on redemande.
// ---------------------------------------------------------------------------------------------

/// **La mise en file ne prend aucune proposition dans son corps.**
///
/// C'est ce qui ferme le trou : tant que `lep_queue` prenait la proposition de son appelant, on
/// pouvait proposer une question et en mettre une autre en file sous le même identifiant de tâche.
/// Le fait `task.queued` ne porte que l'identifiant, donc **rien n'aurait montré la divergence**.
///
/// Le test le tient par la surface : un corps qui porte une proposition différente est ignoré, et la
/// mission déposée reste celle du journal.
#[tokio::test]
async fn la_mise_en_file_ignore_une_proposition_glissee_dans_le_corps() {
    let (runtime, file) = daemon();
    let adresse = servir(runtime).await;

    let _ = poster(
        &adresse,
        PROPOSE_PATH,
        Some(ADMIN),
        &corps_propose("idem-p"),
    )
    .await;

    let mut autre = proposition();
    "Le catalyseur B est-il inerte ?".clone_into(&mut autre.statement);
    let corps = format!(
        "{{\"idempotency_key\":\"idem-q\",\"project_id\":\"{}\",\"task_id\":\"{TACHE}\",\"proposal\":{}}}",
        id::<Project>(4),
        serde_json::to_string(&autre).expect("sérialisable")
    );
    let reponse = poster(&adresse, QUEUE_PATH, Some(ADMIN), &corps).await;
    assert!(reponse.starts_with("HTTP/1.1 202"), "{reponse}");

    let en_file = file.take("peu-importe").expect("une mission attend");
    assert_eq!(
        en_file.mission.objective.statement,
        proposition().statement,
        "la mission déposée est celle qui a été **proposée**, pas celle qu'on a glissée dans la \
         requête de mise en file"
    );
}

// ---------------------------------------------------------------------------------------------
// 6. Les commandes ne sont pas sous `/lep/`.
// ---------------------------------------------------------------------------------------------

/// **Les deux chemins de §22.3, littéralement, et hors du protocole des workers.**
///
/// Écrits en clair et non composés depuis les constantes : la même valeur des deux côtés d'une
/// égalité ne vérifie rien, et ce chantier l'a appris quatre fois.
///
/// Le préfixe compte autant que le reste. Loger ces deux commandes sous `/lep/` aurait suggéré
/// qu'une créance de worker les ouvre — ce que le test d'autorisation ci-dessus dément, mais qu'un
/// lecteur de routeur aurait cru.
#[test]
fn les_commandes_de_22_3_ne_sont_pas_sous_lep() {
    assert_eq!(PROPOSE_PATH, "/commands/task/propose");
    assert_eq!(QUEUE_PATH, "/commands/task/queue");
    assert!(!PROPOSE_PATH.starts_with("/lep/"));
    assert!(!QUEUE_PATH.starts_with("/lep/"));
}

/// **`claimable` se lit toujours de §7.1, et la mise en file n'y change rien.**
///
/// Le garde-fou de `W20.o` : une tâche `proposed` n'est pas réclamable, une tâche `queued` l'est, et
/// aucune des deux règles n'est écrite ailleurs que dans le tableau de §7.1.
#[test]
fn la_mise_en_file_ne_redefinit_pas_ce_qui_est_reclamable() {
    assert!(!locusd::mission::claimable(TaskState::Proposed));
    assert!(locusd::mission::claimable(TaskState::Queued));
}
