//! Le test de sortie de `W20.n` — l'enrôlement de §7.2, servi.
//!
//! # Ce qui est éprouvé, et depuis quel bout
//!
//! Un vrai socket, des requêtes HTTP/1.1 écrites à la main, et une **vraie** signature Ed25519
//! produite par la même bibliothèque que `canterel` emploie côté client — pas un double. Ce qui
//! traverse est la forme de `W2.4`, champ pour champ.

use std::fmt::Write as _;
use std::sync::Arc;

use ed25519_dalek::pkcs8::EncodePublicKey;
use ed25519_dalek::{Signer, SigningKey};
use locus_protocol::id::{Agent, Command as CommandId, Event as EventId, Project, Workspace};
use locus_protocol::{Id, IdKind, Timestamp};
use locusd::enrollment::{
    Credential, Enroll, EnrollmentRequest, Grant, MemoryTokens, Rejection, signed_payload,
    stream_of_worker, verify,
};
use locusd::http::{CLAIM_PATH, ENROLL_PATH, router};
use locusd::lep::{
    Desk, Identities, LepContext, MemoryQueue, MemoryRegistry, Submitted, WorkerRegistry,
};
use locusd::{CommandEnvelope, CommandError, Decide, Revision, Runtime};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const TOKEN: &str = "token-d-enrolement";
const WORKER: &str = "canterel-vm-linux-01";

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

/// Une source d'identifiants déterministe — la créance émise devient prévisible, donc lisible.
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

/// Le base64 standard, pour écrire ce que le client écrit.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for morceau in bytes.chunks(3) {
        let mut bloc = [0_u8; 3];
        bloc[..morceau.len()].copy_from_slice(morceau);
        let valeur = (u32::from(bloc[0]) << 16) | (u32::from(bloc[1]) << 8) | u32::from(bloc[2]);
        for rang in 0..4 {
            if rang <= morceau.len() {
                let index = (valeur >> (18 - rang * 6)) & 0x3F;
                out.push(char::from(ALPHABET[index as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Une clé déterministe : un test qui tirerait au sort serait un test qu'on ne peut pas rejouer.
fn cle() -> SigningKey {
    SigningKey::from_bytes(&[7_u8; 32])
}

fn demande(endpoint: &str, nonce: &str) -> EnrollmentRequest {
    let signing = cle();
    let public = signing
        .verifying_key()
        .to_public_key_der()
        .expect("SPKI encodable");
    let signature = signing.sign(signed_payload(WORKER, endpoint, nonce).as_bytes());
    EnrollmentRequest {
        worker_id: WORKER.to_owned(),
        worker_kind: "canterel".to_owned(),
        public_key: base64(public.as_bytes()),
        runtime: "linux-x86_64".to_owned(),
        nonce: nonce.to_owned(),
        signature: base64(&signature.to_bytes()),
        enrollment_token: TOKEN.to_owned(),
    }
}

/// Ce qu'un worker envoie en s'enrôlant — sans projet, puisqu'il ne le connaît pas encore.
fn enrolling() -> locusd::lep::Enrolling {
    locusd::lep::Enrolling {
        idempotency_key: "idem-enrol".to_owned(),
        proposed_project: None,
        occurred_at: Timestamp::from_millis(1_700_000_000_000),
    }
}

fn grant() -> Grant {
    Grant {
        scope: vec!["worker".to_owned()],
        labels: Vec::new(),
        workspace_id: id::<Workspace>(2),
        principal_id: id::<Agent>(3),
        // `W20.w` : le projet vient du grant, jamais de la demande du worker.
        project_id: id::<Project>(4),
    }
}

fn daemon() -> (
    Runtime<locus_event_store::MemoryEventStore>,
    Arc<MemoryRegistry>,
) {
    let tokens = Arc::new(MemoryTokens::new());
    tokens.issue(TOKEN, grant());
    let registre = Arc::new(MemoryRegistry::new());
    let desk = Desk::new(
        Arc::new(MemoryQueue::new()),
        Arc::clone(&registre) as Arc<dyn WorkerRegistry>,
        Arc::new(Identites::default()),
    )
    .enrolling(tokens);
    (Runtime::in_memory().with_lep(desk), registre)
}

/// Ce que le worker annonce — la fixture Linux de `W0.7`, dont le `worker_id` est [`WORKER`].
fn manifeste() -> locus_lep::CapabilityManifest {
    let chemin =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas/examples");
    let brut = std::fs::read_to_string(chemin.join("capability-manifest-vm-linux.json"))
        .expect("fixture lisible");
    let mut valeur: serde_json::Value = serde_json::from_str(&brut).expect("JSON valide");
    valeur.as_object_mut().expect("un objet").remove("_fixture");
    let manifeste: locus_lep::CapabilityManifest =
        serde_json::from_value(valeur).expect("manifeste");
    assert_eq!(manifeste.worker_id, WORKER);
    manifeste
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
    flux.write_all(requete.as_bytes()).await.expect("requête");
    let mut reponse = Vec::new();
    flux.read_to_end(&mut reponse).await.expect("réponse");
    String::from_utf8_lossy(&reponse).into_owned()
}

fn corps(demande: &EnrollmentRequest) -> String {
    let mut valeur = serde_json::to_value(demande).expect("sérialisable");
    let objet = valeur.as_object_mut().expect("un objet");
    objet.insert(
        "project_id".to_owned(),
        serde_json::Value::String(id::<Project>(4).to_string()),
    );
    objet.insert(
        "idempotency_key".to_owned(),
        serde_json::Value::String(format!("idem-{}", demande.nonce)),
    );
    valeur.to_string()
}

fn credential_de(reponse: &str) -> Credential {
    let corps = reponse.split_once("\r\n\r\n").map_or("", |(_, c)| c);
    serde_json::from_str(corps).unwrap_or_else(|_| panic!("créance lisible :\n{reponse}"))
}

// ---------------------------------------------------------------------------------------------
// 1. Une demande signée obtient une créance, et le token n'en est pas une.
// ---------------------------------------------------------------------------------------------

/// **Un worker s'enrôle, et il peut ensuite réclamer.**
///
/// C'est le test de sortie : avant `W20.n`, le `WorkerRegistry` de `W20.k` n'était rempli que par un
/// test, donc aucun worker réel ne pouvait obtenir de créance. La chaîne complète est ici — enrôler,
/// puis parler §15.2 avec ce qu'on a reçu.
#[tokio::test]
async fn un_worker_signe_s_enrole_et_peut_ensuite_reclamer() {
    let (runtime, _) = daemon();
    let adresse = servir(runtime).await;
    let endpoint = format!("http://{adresse}");

    let reponse = poster(
        &adresse,
        ENROLL_PATH,
        None,
        &corps(&demande(&endpoint, "n-1")),
    )
    .await;
    assert!(
        reponse.starts_with("HTTP/1.1 200"),
        "une demande signée s'enrôle :\n{reponse}"
    );
    let creance = credential_de(&reponse);
    assert_eq!(creance.worker_id, WORKER);
    assert_eq!(creance.scope, vec!["worker".to_owned()]);

    // Et la créance obtenue ouvre §15.2. Une file vide rend `204` — ce qui prouve que la créance
    // est **reconnue** : une créance inconnue rendrait `403`.
    //
    // Le manifeste est exigé même sur une file vide — `W20.q`. C'est délibéré : sans lui, un worker
    // mal configuré recevrait des `204` indéfiniment et lirait sa panne comme du calme. Le refus
    // arrive donc au premier appel, avant qu'il y ait quoi que ce soit à confier.
    let claim = poster(
        &adresse,
        CLAIM_PATH,
        Some(&creance.credential),
        &format!(
            "{{\"project_id\":\"{}\",\"manifest\":{}}}",
            id::<Project>(4),
            serde_json::to_string(&manifeste()).expect("le manifeste se sérialise")
        ),
    )
    .await;
    assert!(
        claim.starts_with("HTTP/1.1 204"),
        "la créance émise doit être reconnue par la surface §15.2 :\n{claim}"
    );
}

/// **Le token ne devient jamais le secret permanent** — §7.2, mot pour mot.
///
/// Un serveur qui renverrait le token comme créance passerait tous les tests fonctionnels : le
/// worker s'enrôle, réclame, tout marche. Il aurait seulement donné à un secret court-terme et à
/// usage unique la durée de vie d'un secret permanent. C'est la faute qu'aucun symptôme ne signale,
/// donc celle qui a besoin d'un test à elle.
#[tokio::test]
async fn la_creance_emise_n_est_jamais_le_token() {
    let (runtime, _) = daemon();
    let adresse = servir(runtime).await;
    let endpoint = format!("http://{adresse}");

    let reponse = poster(
        &adresse,
        ENROLL_PATH,
        None,
        &corps(&demande(&endpoint, "n-1")),
    )
    .await;
    let creance = credential_de(&reponse);

    assert_ne!(creance.credential, TOKEN);
    assert!(
        !reponse.contains(TOKEN),
        "le token ne repart pas :\n{reponse}"
    );
}

/// **Un token ne sert qu'une fois** — §7.2.
#[tokio::test]
async fn un_token_ne_sert_qu_une_fois() {
    let (runtime, _) = daemon();
    let adresse = servir(runtime).await;
    let endpoint = format!("http://{adresse}");

    let premier = poster(
        &adresse,
        ENROLL_PATH,
        None,
        &corps(&demande(&endpoint, "n-1")),
    )
    .await;
    assert!(premier.starts_with("HTTP/1.1 200"), "{premier}");

    // Un **autre** nonce, pour que ce soit le token qui refuse et non le rejeu.
    let second = poster(
        &adresse,
        ENROLL_PATH,
        None,
        &corps(&demande(&endpoint, "n-2")),
    )
    .await;
    assert!(
        second.starts_with("HTTP/1.1 403"),
        "un token consommé ne resert pas :\n{second}"
    );
}

/// **Un nonce rejoué est refusé, et c'est bien le nonce qui refuse.**
///
/// Une première rédaction envoyait deux fois la même demande et concluait « le rejeu est refusé ».
/// Il l'était — par le **token**, consommé au premier passage. La garde de nonce n'était éprouvée
/// nulle part, et une passe de mutation l'a montrée en la neutralisant sans qu'un test bouge.
///
/// Un **second** token est donc émis avant le rejeu : le token ne peut plus expliquer le refus, et
/// il ne reste que le nonce. Sans cette garde, une demande capturée se resservirait contre le même
/// serveur.
#[tokio::test]
async fn un_nonce_rejoue_est_refuse() {
    let tokens = Arc::new(MemoryTokens::new());
    tokens.issue(TOKEN, grant());
    let registre = Arc::new(MemoryRegistry::new());
    let desk = Desk::new(
        Arc::new(MemoryQueue::new()),
        Arc::clone(&registre) as Arc<dyn WorkerRegistry>,
        Arc::new(Identites::default()),
    )
    .enrolling(Arc::clone(&tokens) as Arc<dyn locusd::enrollment::EnrollmentTokens>);
    let adresse = servir(Runtime::in_memory().with_lep(desk)).await;
    let endpoint = format!("http://{adresse}");
    let demande = demande(&endpoint, "n-unique");

    let premier = poster(&adresse, ENROLL_PATH, None, &corps(&demande)).await;
    assert!(premier.starts_with("HTTP/1.1 200"), "{premier}");

    // Le token d'origine est consommé : on en remet un, **sous le même nom**, pour que le refus ne
    // puisse pas venir de lui.
    tokens.issue(TOKEN, grant());
    let second = poster(&adresse, ENROLL_PATH, None, &corps(&demande)).await;
    assert!(
        second.starts_with("HTTP/1.1 403"),
        "un nonce déjà vu est refusé, même quand le token est encore bon :\n{second}"
    );
}

/// **Une demande mal signée ne consomme ni nonce ni token.**
///
/// Sinon n'importe qui épuiserait les tokens d'un worker en envoyant du bruit signé n'importe
/// comment — un déni de service qui ne demande aucune clé. L'ordre des vérifications est donc :
/// signature, **puis** nonce, **puis** token, et ce test le tient en enrôlant *après* l'attaque.
#[tokio::test]
async fn une_demande_mal_signee_ne_consomme_rien() {
    let (runtime, _) = daemon();
    let adresse = servir(runtime).await;
    let endpoint = format!("http://{adresse}");

    let mut fausse = demande(&endpoint, "n-1");
    fausse.signature = base64(&[0_u8; 64]);
    let refus = poster(&adresse, ENROLL_PATH, None, &corps(&fausse)).await;
    assert!(
        refus.starts_with("HTTP/1.1 403"),
        "une signature invalide est refusée :\n{refus}"
    );

    // Le même nonce et le même token restent utilisables : l'attaque n'a rien coûté.
    let vraie = poster(
        &adresse,
        ENROLL_PATH,
        None,
        &corps(&demande(&endpoint, "n-1")),
    )
    .await;
    assert!(
        vraie.starts_with("HTTP/1.1 200"),
        "l'attaque ne doit avoir consommé ni le nonce ni le token :\n{vraie}"
    );
}

/// **Une demande signée pour un autre serveur ne passe pas ici.**
///
/// C'est la moitié serveur de ce que `W2.4` a écrit côté client : « une demande capturée ne peut pas
/// être rejouée vers un autre serveur, ni resservie au même ». La charge est reconstruite avec
/// **notre** endpoint, donc une signature faite pour un autre ne peut pas la reproduire.
#[tokio::test]
async fn une_demande_signee_pour_un_autre_serveur_ne_passe_pas() {
    let (runtime, _) = daemon();
    let adresse = servir(runtime).await;

    let ailleurs = demande("http://un-autre-serveur.example", "n-1");
    let refus = poster(&adresse, ENROLL_PATH, None, &corps(&ailleurs)).await;
    assert!(
        refus.starts_with("HTTP/1.1 403"),
        "l'endpoint est dans la signature :\n{refus}"
    );
}

// ---------------------------------------------------------------------------------------------
// 2. La vérification, éprouvée directement.
// ---------------------------------------------------------------------------------------------

/// **Les deux refus de `verify`, et le fait qu'il n'y en ait que deux.**
///
/// Une première rédaction portait une troisième variante — « signée pour un autre serveur » — et
/// elle était **inatteignable** : la charge est reconstruite avec notre endpoint, donc ce cas se
/// lit `BadSignature`. `CLAUDE.md` refuse une valeur d'énumération qui annonce un effet dont
/// personne n'est le consommateur ; elle a été retirée plutôt que gardée « au cas où ».
#[test]
fn la_verification_a_deux_refus_et_pas_trois() {
    let endpoint = "http://locus.example";
    assert_eq!(verify(&demande(endpoint, "n"), endpoint), Ok(()));

    let mut illisible = demande(endpoint, "n");
    illisible.public_key = "pas du base64 !".to_owned();
    assert_eq!(
        verify(&illisible, endpoint),
        Err(Rejection::UnreadableKey),
        "une clé illisible se dit, elle ne se lit pas comme une signature fausse"
    );

    let mut fausse = demande(endpoint, "n");
    fausse.signature = base64(&[0_u8; 64]);
    assert_eq!(verify(&fausse, endpoint), Err(Rejection::BadSignature));

    // Et l'endpoint étranger tombe bien dans `BadSignature`, ce qui est la raison de n'avoir que
    // deux variantes.
    assert_eq!(
        verify(&demande("http://ailleurs", "n"), endpoint),
        Err(Rejection::BadSignature)
    );
}

// ---------------------------------------------------------------------------------------------
// 3. La révocation est un fait, et elle ferme les trois chemins.
// ---------------------------------------------------------------------------------------------

/// **Un worker révoqué perd les trois chemins de §15.2, et le journal garde son histoire.**
///
/// Invariant 12 : rien n'est supprimé. Le registre cesse de reconnaître la créance, `worker.revoked`
/// entre au journal, et l'enrôlement d'origine y reste lisible — c'est ce qui distingue une
/// révocation d'un effacement.
#[test]
fn un_worker_revoque_perd_ses_chemins_et_garde_son_histoire() {
    let (runtime, registre) = daemon();
    let endpoint = "http://locus.example";
    let creance = runtime
        .lep_enroll(
            &demande(endpoint, "n-1"),
            endpoint,
            &enrolling(),
            Timestamp::from_millis(1_700_000_000_000),
        )
        .expect("l'enrôlement aboutit");

    assert!(
        registre.identify(&creance.credential).is_some(),
        "avant la révocation, la créance est reconnue"
    );

    runtime
        .lep_revoke(
            WORKER,
            "clé compromise",
            &grant(),
            // La révocation, elle, est un acte d'un worker **déjà enrôlé** : elle connaît son
            // projet et le dit. C'est la distinction que `W20.w` a rendue visible en séparant les
            // deux types.
            &Submitted {
                idempotency_key: "idem-revoc".to_owned(),
                project_id: id::<Project>(4),
                occurred_at: Timestamp::from_millis(1_700_000_000_000),
            },
            Timestamp::from_millis(1_700_000_100_000),
        )
        .expect("la révocation aboutit");

    assert!(
        registre.identify(&creance.credential).is_none(),
        "après la révocation, la créance n'ouvre plus rien — §7.4"
    );

    // Et l'histoire est **plus longue**, pas plus courte : deux faits sur le stream du worker.
    let histoire = runtime
        .branch_history(&stream_of_worker(WORKER), None, None)
        .expect("stream lisible");
    let types: Vec<String> = histoire
        .items
        .iter()
        .map(|entry| entry.event_type.clone())
        .collect();
    assert_eq!(
        types,
        vec!["worker.registered".to_owned(), "worker.revoked".to_owned()],
        "une révocation ajoute un fait ; elle n'en retire aucun (invariant 12)"
    );
}

/// **Le fait d'enrôlement ne porte ni le token ni une créance — lu dans sa charge.**
///
/// `CLAUDE.md` interdit de journaliser un secret, et un journal est ce qu'on relit le plus
/// longtemps : c'est l'endroit dont un secret sort le plus difficilement une fois entré.
///
/// Une première rédaction lisait `/timeline`, qui ne porte **pas** de charge — elle ne rend que
/// position, type et stream. Le test s'appelait « le journal ne porte aucun secret » et ne
/// regardait rien de ce qui pourrait en contenir ; une passe de mutation l'a montré en écrivant le
/// token à la place de la clé publique sans qu'il bronche.
///
/// La charge se lit donc **au décideur**, seul endroit où elle existe avant d'être scellée. Aucune
/// query de §22.4 ne l'expose, et lui ajouter une porte « pour les tests » ouvrirait le chemin que
/// `W20.b` a fermé.
#[test]
fn le_fait_d_enrolement_ne_porte_aucun_secret() {
    let endpoint = "http://locus.example";
    let enroll = Enroll {
        request: demande(endpoint, "n-1"),
        grant: grant(),
    };
    let commande = CommandEnvelope::mutating(
        id::<CommandId>(9),
        "worker.enroll",
        id::<Workspace>(2),
        id::<Agent>(3),
        "idem".to_owned(),
        Revision::INITIAL,
    )
    .expect("enveloppe constructible");
    let contexte = LepContext {
        project_id: id::<Project>(4),
        event_ids: vec![id::<EventId>(1)],
        occurred_at: Timestamp::from_millis(1_700_000_000_000),
        payload_hash: String::new(),
    };

    let faits = enroll.decide(&commande, &contexte).expect("décision");
    let charge = faits.first().expect("un fait").payload.to_string();

    assert!(
        !charge.contains(TOKEN),
        "le token n'entre pas au journal : {charge}"
    );
    // La clé **publique**, elle, y est : c'est un fait du registre, et pas un secret.
    assert!(charge.contains(&demande(endpoint, "n-1").public_key));
    assert!(charge.contains(WORKER));
}

// ---------------------------------------------------------------------------------------------
// `W20.w` — le projet vient du grant, jamais de la demande.
// ---------------------------------------------------------------------------------------------

/// **Un worker qui propose un autre projet que celui de son grant est refusé.**
///
/// Refusé, et non ignoré. L'ignorer en silence le laisserait croire qu'il écrit dans le projet
/// qu'il a nommé, et découvrir le contraire des mois plus tard en lisant une projection qui range
/// ses faits ailleurs. Le refus nomme le champ et dit **qui** décide.
#[test]
fn un_projet_propose_qui_diverge_du_grant_est_refuse() {
    let (runtime, _) = daemon();
    let endpoint = "http://locus.example";

    let refus = runtime
        .lep_enroll(
            &demande(endpoint, "n-divergent"),
            endpoint,
            &locusd::lep::Enrolling {
                proposed_project: Some(id::<Project>(99)),
                ..enrolling()
            },
            Timestamp::from_millis(1_700_000_000_000),
        )
        .expect_err("le projet proposé n'est pas celui du grant");

    match refus {
        locusd::error::CommandError::Validation { field, detail } => {
            assert_eq!(field, "project_id");
            assert!(detail.contains("assigné par le grant"), "{detail}");
        }
        autre => panic!("attendu un refus de validation, reçu {autre:?}"),
    }
}

/// **Le même projet que le grant passe — la garde ne crie pas sur ce qui est juste.**
///
/// Le pendant du test précédent. Un worker peut légitimement répéter le projet qu'on lui a donné,
/// et une garde qui refuserait aussi ce cas se ferait désactiver à la première mise en service.
#[test]
fn un_projet_propose_identique_au_grant_passe() {
    let (runtime, _) = daemon();
    let endpoint = "http://locus.example";

    runtime
        .lep_enroll(
            &demande(endpoint, "n-identique"),
            endpoint,
            &locusd::lep::Enrolling {
                proposed_project: Some(id::<Project>(4)),
                ..enrolling()
            },
            Timestamp::from_millis(1_700_000_000_000),
        )
        .expect("répéter le projet de son grant n'est pas une faute");
}

/// **Sans projet proposé, l'enrôlement aboutit et le fait atterrit dans le projet du grant.**
///
/// C'est le cas nominal — celui du worker `canterel` réel, qui n'envoie pas de `project_id` parce
/// qu'il ne le connaît pas encore. Avant `W20.w`, il recevait « sans projet, un fait n'a pas
/// d'endroit où appartenir » et ne pouvait pas s'enrôler du tout.
#[test]
fn sans_projet_propose_l_enrolement_aboutit() {
    let (runtime, registre) = daemon();
    let endpoint = "http://locus.example";

    let creance = runtime
        .lep_enroll(
            &demande(endpoint, "n-nominal"),
            endpoint,
            &enrolling(),
            Timestamp::from_millis(1_700_000_000_000),
        )
        .expect("l'enrôlement aboutit sans que le worker nomme un projet");

    assert!(registre.identify(&creance.credential).is_some());
}
