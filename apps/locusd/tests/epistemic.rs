//! Le test de sortie de `W20.r` — un `EpistemicCommit` entre au journal, et l'institution l'intègre.
//!
//! # Ce que cet item a trouvé en se donnant un sujet
//!
//! `W2.15` plafonne le worker à `staged` côté client et `packages/validation` porte la propagation,
//! mais **rien dans `apps/locusd` ne connaissait `EpistemicCommit`**. En lui donnant un chemin, une
//! sonde a trouvé pire qu'une absence : un `epistemic_commit.submitted` remonté par §15.6 recevait
//! `202 Accepted`, et le fait écrit ne portait ni `status` ni `validation_level`. La projection de
//! §9.3 passait en **quarantaine**, et `main.rs` refuse d'ouvrir le port avec une projection en
//! quarantaine — donc un worker qui soumettait un commit empêchait le daemon de redémarrer.
//!
//! `un_commit_soumis_ne_met_aucune_projection_en_quarantaine` est le test qui aurait rougi avant
//! cet item, et c'est celui à ne jamais supprimer.

use std::fmt::Write as _;
use std::sync::Arc;

use locus_broker::port::{BrokerPort, Loopback};
use locus_broker::protocol::Verdict;
use locus_domain::{Status, ValidationLevel};
use locus_lep::{Event, MissionEnvelope};
use locus_protocol::id::{Agent, Command as CommandId, Event as EventId, Project, Workspace};
use locus_protocol::{Id, IdKind, Timestamp};
use locusd::epistemic::{Integrate, Stage, fact_type, staging};
use locusd::http::{EVENTS_PATH, router};
use locusd::lep::{
    Desk, Identities, MemoryQueue, MemoryRegistry, Queued, Submitted, WorkerIdentity,
};
use locusd::mission::Authority;
use locusd::{CommandError, Runtime};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const CREANCE: &str = "creance-de-worker";
const WORKER: &str = "canterel-vm-linux-01";
const TACHE: &str = "task-nominal";

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

fn identite() -> WorkerIdentity {
    WorkerIdentity {
        worker_id: WORKER.to_owned(),
        workspace_id: id::<Workspace>(2),
        principal_id: id::<Agent>(3),
    }
}

fn autorite() -> Authority {
    Authority {
        workspace_id: id::<Workspace>(2),
        principal_id: id::<Agent>(7),
    }
}

fn soumission(cle: &str) -> Submitted {
    Submitted {
        idempotency_key: cle.to_owned(),
        project_id: id::<Project>(4),
        occurred_at: Timestamp::from_millis(1_700_000_000_000),
    }
}

/// Ce qu'une mise en file dépose — `W20.v` : la mission, et le rang que la proposition a fixé.
///
/// **Aucun bail** : il est frappé à la réclamation, pour le worker que le placement admet.
fn en_file() -> Queued {
    let mission: MissionEnvelope = fixture("mission-envelope-nominal.json");
    Queued {
        mission,
        attempt: 1,
    }
}

fn daemon() -> Runtime<locus_event_store::MemoryEventStore> {
    let file = Arc::new(MemoryQueue::new());
    file.push(en_file());
    let registre = Arc::new(MemoryRegistry::new());
    registre.admit(CREANCE, identite());
    let broker: Arc<dyn BrokerPort + Send + Sync> =
        Arc::new(Loopback::answering(Verdict::Placed {
            worker: WORKER.to_owned(),
            level: locus_lep::SandboxLevel::S3,
        }));
    Runtime::in_memory()
        .with_lep(Desk::new(file, registre, Arc::new(Identites::default())).placing(broker))
}

/// Un événement `epistemic_commit.submitted` porteur du statut annoncé.
///
/// La charge est celle que `commitSubmittedPayload` de `W2.15` produit — `commit_hash`, `signature`,
/// `status`, et les trois comptes. Écrite ici sous les mêmes noms parce que c'est la moitié serveur
/// d'un contrat, et qu'un miroir qui divergerait ne le dirait pas.
fn commit(statut: &str) -> Event {
    let mut event: Event = fixture("event-reconnection-1-started.json");
    event.task_id = Some(TACHE.to_owned());
    event.worker_id = Some(WORKER.to_owned());
    "epistemic_commit.submitted".clone_into(&mut event.event_type);
    event.payload = Some(serde_json::json!({
        "commit_hash": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "signature": "signature-de-worker",
        "status": statut,
        "claims": 1,
        "objections": 0,
        "negative_results": 0,
    }));
    event
}

async fn servir(runtime: Arc<Runtime<locus_event_store::MemoryEventStore>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("la boucle locale accepte un port libre");
    let adresse = listener.local_addr().expect("adresse connue").to_string();
    let app = router(runtime);
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

fn corps_evenements(cle: &str, events: &[Event]) -> String {
    format!(
        "{{\"idempotency_key\":\"{cle}\",\"project_id\":\"{}\",\"events\":{}}}",
        id::<Project>(4),
        serde_json::to_string(events).expect("sérialisable")
    )
}

/// Les faits d'un stream, relus par la surface publique de §22.4.
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

// ---------------------------------------------------------------------------------------------
// 1. Le défaut que cet item a mis au jour, et qu'il ferme.
// ---------------------------------------------------------------------------------------------

/// **Un commit soumis ne met aucune projection en quarantaine.**
///
/// C'est le test qui aurait rougi avant `W20.r`, et le plus important du fichier. Avant, le fait
/// écrit portait la sérialisation de l'événement LEP — donc ni `status` ni `validation_level` — et
/// la projection « état de validation » de §9.3 le refusait à juste titre. Elle passait en
/// quarantaine, `ready` tombait à `false`, et `main.rs` refuse d'ouvrir le port dans cet état.
///
/// Le worker, lui, recevait `202 Accepted`. C'est ce qui rendait la faute invisible : rien, du côté
/// qui agissait, ne disait que quelque chose venait de casser.
#[tokio::test]
async fn un_commit_soumis_ne_met_aucune_projection_en_quarantaine() {
    let adresse = servir(Arc::new(daemon())).await;

    let reponse = poster(
        &adresse,
        EVENTS_PATH,
        Some(CREANCE),
        &corps_evenements("idem-commit", &[commit("staged")]),
    )
    .await;
    assert!(
        reponse.starts_with("HTTP/1.1 202"),
        "un commit mis en scène est accepté :\n{reponse}"
    );

    let sante = demander(&adresse, "/projections/status").await;
    assert!(
        sante.contains("\"ready\":true"),
        "un commit soumis ne doit pas empêcher le daemon de se dire prêt :\n{sante}"
    );
    assert!(
        !sante.contains("\"healthy\":false"),
        "aucune projection en quarantaine :\n{sante}"
    );
}

/// **Et le fait écrit est bien celui d'un objet épistémique mis en scène.**
///
/// Le pendant du test précédent : une projection reste saine si on ne lui écrit **rien**, donc
/// « pas de quarantaine » ne prouve pas à lui seul que le commit est arrivé quelque part.
#[tokio::test]
async fn un_commit_soumis_ecrit_un_fait_epistemique() {
    let adresse = servir(Arc::new(daemon())).await;

    let _ = poster(
        &adresse,
        EVENTS_PATH,
        Some(CREANCE),
        &corps_evenements("idem-commit", &[commit("staged")]),
    )
    .await;

    assert_eq!(
        faits_sur(&adresse, &format!("task/{TACHE}")).await,
        vec!["epistemic_object.staged"],
        "le fait dit dans quel état l'objet se trouve, sous le namespace de §10.3"
    );
}

// ---------------------------------------------------------------------------------------------
// 2. §2.3 — jamais au-delà de `staged`, et le refus nomme le champ.
// ---------------------------------------------------------------------------------------------

/// **Un worker qui annonce `validated` est refusé, et rien n'est écrit.**
///
/// C'est l'invariant 3 pris à sa racine : « un worker ne modifie jamais directement la base
/// canonique ». Le refus est une **autorisation** et non une validation — la requête est bien
/// formée, c'est le droit de prononcer ce verdict qui manque, et lui rendre `400` enverrait relire
/// une requête où il n'y a rien à corriger.
///
/// Le statut refusé est celui de `invalid-commit-self-validated.json`, la fixture que `W0.7` a
/// écrite pour que la garantie soit vérifiée **comme une donnée**.
#[tokio::test]
async fn un_worker_qui_annonce_validated_est_refuse_et_rien_n_est_ecrit() {
    let attendu: serde_json::Value = fixture("invalid-commit-self-validated.json");
    let statut = attendu["status"]
        .as_str()
        .expect("la fixture porte un statut");
    assert_eq!(
        statut, "validated",
        "la fixture de W0.7 est celle du worker qui s'auto-valide ; si elle change, ce n'est plus \
         elle qu'on éprouve"
    );

    let adresse = servir(Arc::new(daemon())).await;
    let reponse = poster(
        &adresse,
        EVENTS_PATH,
        Some(CREANCE),
        &corps_evenements("idem-usurpe", &[commit(statut)]),
    )
    .await;

    assert!(
        reponse.starts_with("HTTP/1.1 403"),
        "s'auto-valider est une faute d'autorisation, pas une requête à corriger :\n{reponse}"
    );
    assert!(
        reponse.contains("validated") && reponse.contains("staged"),
        "le refus nomme ce qui a été annoncé et ce qui est permis :\n{reponse}"
    );
    assert!(
        faits_sur(&adresse, &format!("task/{TACHE}"))
            .await
            .is_empty(),
        "un refus n'écrit rien : c'est la transaction qui écrit, et elle n'écrit qu'un `Ok`"
    );
}

/// **Les huit statuts que §2.3 refuse à un worker sont refusés, un par un.**
///
/// `validated` seul laisserait croire que la garantie porte sur un mot. Elle porte sur la liste que
/// [`Status::is_worker_proposable`] tient, et ce test l'éprouve **entière** — sans la recopier :
/// il lit `Status::ALL` et interroge le domaine, donc un statut ajouté à §7.4 y entre tout seul.
#[test]
fn les_statuts_hors_de_2_3_sont_tous_refuses() {
    let mut proposables = 0;
    for statut in Status::ALL {
        let mise = Stage {
            rank: 0,
            task_id: TACHE.to_owned(),
            announced: statut.as_str().to_owned(),
            summary: serde_json::Value::Null,
            worker_id: WORKER.to_owned(),
        };
        match mise.accepted() {
            Ok(accepte) => {
                assert_eq!(accepte, statut);
                assert!(
                    statut.is_worker_proposable(),
                    "« {statut} » n'est pas proposable par un worker et a pourtant été accepté"
                );
                proposables += 1;
            }
            Err(erreur) => {
                assert!(
                    !statut.is_worker_proposable(),
                    "« {statut} » est proposable et a pourtant été refusé : {erreur}"
                );
                assert!(
                    matches!(erreur, CommandError::Authorization { .. }),
                    "un statut réservé à l'institution est une faute d'autorisation : {erreur:?}"
                );
            }
        }
    }
    assert_eq!(
        proposables, 2,
        "§2.3 en laisse deux — `draft` et `staged` ; si ce compte change, c'est la règle qui a \
         changé, et cela se décide ailleurs qu'ici"
    );
}

/// **Un statut que §7.4 ne nomme pas est une faute de forme, pas d'autorisation.**
///
/// « Ce mot n'existe pas » envoie relire le protocole ; « ce mot ne t'appartient pas » envoie
/// relire qui décide. Les fondre ferait chercher une faute de frappe là où il y a une usurpation,
/// et l'inverse.
#[test]
fn un_statut_hors_de_7_4_est_une_faute_de_forme() {
    let mise = Stage {
        rank: 0,
        task_id: TACHE.to_owned(),
        announced: "presque-valide".to_owned(),
        summary: serde_json::Value::Null,
        worker_id: WORKER.to_owned(),
    };

    let erreur = mise.accepted().expect_err("ce mot n'est pas un statut");
    let CommandError::Validation { field, detail } = &erreur else {
        panic!("un mot inconnu est une faute de forme : {erreur:?}");
    };
    assert_eq!(field, "payload.status");
    assert!(
        detail.contains("presque-valide") && detail.contains("under_review"),
        "le refus cite ce qui a été annoncé et énumère les dix de §7.4 : {detail}"
    );
}

// ---------------------------------------------------------------------------------------------
// 3. L'intégration est une commande distincte, sous une autorité distincte.
// ---------------------------------------------------------------------------------------------

/// **L'institution intègre, et la projection suit — statut et niveau, tous deux nommés.**
#[tokio::test]
async fn l_institution_integre_et_la_projection_suit() {
    let runtime = Arc::new(daemon());
    let adresse = servir(Arc::clone(&runtime)).await;

    let _ = poster(
        &adresse,
        EVENTS_PATH,
        Some(CREANCE),
        &corps_evenements("idem-commit", &[commit("staged")]),
    )
    .await;

    runtime
        .lep_integrate(
            &Integrate {
                task_id: TACHE.to_owned(),
                status: Status::Validated,
                level: ValidationLevel::IndependentlyReviewed,
                rationale: "revue indépendante de C-184 satisfaite".to_owned(),
            },
            autorite(),
            &soumission("idem-integration"),
            Timestamp::from_millis(1_700_000_001_000),
        )
        .expect("l'intégration aboutit");

    assert_eq!(
        faits_sur(&adresse, &format!("task/{TACHE}")).await,
        vec!["epistemic_object.staged", "epistemic_object.validated"],
        "les deux faits restent, dans l'ordre : le journal ne remplace pas, il ajoute"
    );

    // Et l'état lu porte les **deux** champs, sans que l'un soit déduit de l'autre.
    runtime.with_validation_state(|etat| {
        let objet = etat
            .get(&format!("task/{TACHE}"))
            .expect("la projection a vu l'objet");
        assert_eq!(objet.status, "validated");
        assert_eq!(
            objet.validation_level, "independently_reviewed",
            "le niveau est celui que l'institution a nommé — `validated` n'implique aucun niveau \
             (§7.4)"
        );
    });
}

/// **Un objet `validated` peut l'être à `L0`, et rien ne s'y oppose.**
///
/// §7.4 : « `validation_level` décrit la force épistémique et ne doit pas être déduit du seul
/// statut ». Un objet peut avoir traversé le processus sans qu'aucune preuve indépendante ait été
/// produite. Un serveur qui refuserait cette combinaison — ou qui relèverait le niveau « puisque
/// c'est validé » — transformerait une décision de procédure en constat scientifique.
///
/// Ce test est l'inverse d'une garde : il vérifie qu'**il n'y a pas** de garde.
#[tokio::test]
async fn valide_au_niveau_zero_est_representable() {
    let runtime = Arc::new(daemon());
    let adresse = servir(Arc::clone(&runtime)).await;

    runtime
        .lep_integrate(
            &Integrate {
                task_id: TACHE.to_owned(),
                status: Status::Validated,
                level: ValidationLevel::Unassessed,
                rationale: "processus traversé ; aucune preuve indépendante produite".to_owned(),
            },
            autorite(),
            &soumission("idem-l0"),
            Timestamp::from_millis(1_700_000_001_000),
        )
        .expect("rien ne s'oppose à `validated` avec `L0`");

    let _ = adresse;
    runtime.with_validation_state(|etat| {
        let objet = etat.get(&format!("task/{TACHE}")).expect("objet vu");
        assert_eq!(
            (objet.status.as_str(), objet.validation_level.as_str()),
            ("validated", "unassessed")
        );
    });
}

/// **Une intégration sans motif est refusée.**
///
/// Une décision épistémique qui ne cite rien se relit dans dix ans sans qu'on sache sur quoi elle
/// reposait — et §8.4 refuse qu'une telle décision s'appuie sur rien de citable.
#[test]
fn une_integration_sans_motif_est_refusee() {
    let runtime = daemon();
    let erreur = runtime
        .lep_integrate(
            &Integrate {
                task_id: TACHE.to_owned(),
                status: Status::Validated,
                level: ValidationLevel::Reproduced,
                rationale: "   ".to_owned(),
            },
            autorite(),
            &soumission("idem-muet"),
            Timestamp::from_millis(1_700_000_001_000),
        )
        .expect_err("une intégration muette ne passe pas");

    let CommandError::Validation { field, .. } = &erreur else {
        panic!("un motif manquant est une faute de forme : {erreur:?}");
    };
    assert_eq!(field, "rationale");
}

/// **Aucun chemin de worker n'atteint l'intégration.**
///
/// Tenu par le source, comme la règle 4 de `boundaries.json` l'est pour les sockets de runtime.
/// `lep.rs` sert les workers ; s'il nommait `Integrate`, l'invariant 3 dépendrait d'un `if` au lieu
/// d'une frontière, et un `if` se déplace.
///
/// L'autre moitié est structurelle et ne se teste pas : [`Runtime::lep_integrate`] prend une
/// [`Authority`], que la surface §15.2 ne construit jamais — elle n'a qu'une créance, et une créance
/// ne se convertit pas en autorité.
#[test]
fn aucun_chemin_de_worker_n_atteint_l_integration() {
    for (nom, source) in [
        ("lep.rs", include_str!("../src/lep.rs")),
        ("enrollment.rs", include_str!("../src/enrollment.rs")),
    ] {
        for interdit in ["Integrate", "lep_integrate", "ValidationLevel"] {
            assert!(
                !source.contains(interdit),
                "« {interdit} » dans {nom} : l'intégration est une commande distincte, sous une \
                 autorité distincte, et aucun chemin de worker ne doit la nommer"
            );
        }
    }

    // Et la surface HTTP ne l'expose sur aucun chemin — il n'y a pas de route d'intégration.
    let http = include_str!("../src/http.rs");
    assert!(
        !http.contains("lep_integrate"),
        "aucune route ne prononce un verdict épistémique : ce que §22.3 en fera est une commande \
         d'administration, pas une extension de §15.2"
    );
}

// ---------------------------------------------------------------------------------------------
// 4. Invariant 12 — un commit qui en contredit un autre entre au même titre.
// ---------------------------------------------------------------------------------------------

/// **Un commit qui contredit un commit déjà intégré est écrit, et ne remplace rien.**
///
/// « Les résultats négatifs et conflits ne sont jamais supprimés pour rendre le graphe propre. » Un
/// serveur qui refuserait la contradiction la ferait disparaître au moment exact où elle a le plus
/// de valeur — et un serveur qui écraserait le fait précédent ferait pire : il rendrait le
/// désaccord invisible tout en ayant l'air d'avoir tout gardé.
///
/// Les **trois** faits doivent être lisibles ensuite, dans l'ordre où ils ont eu lieu.
#[tokio::test]
async fn un_commit_qui_contredit_un_commit_integre_entre_au_meme_titre() {
    let runtime = Arc::new(daemon());
    let adresse = servir(Arc::clone(&runtime)).await;

    let _ = poster(
        &adresse,
        EVENTS_PATH,
        Some(CREANCE),
        &corps_evenements("idem-premier", &[commit("staged")]),
    )
    .await;
    runtime
        .lep_integrate(
            &Integrate {
                task_id: TACHE.to_owned(),
                status: Status::Validated,
                level: ValidationLevel::IndependentlyReviewed,
                rationale: "C-184 est réfutée : revue satisfaite".to_owned(),
            },
            autorite(),
            &soumission("idem-integration"),
            Timestamp::from_millis(1_700_000_001_000),
        )
        .expect("la première intégration aboutit");

    // Le second commit dit le contraire du premier, et porte un résultat négatif.
    let mut contradiction = commit("staged");
    contradiction.payload = Some(serde_json::json!({
        "commit_hash": "sha256:1111111111111111111111111111111111111111111111111111111111111111",
        "signature": "signature-de-worker",
        "status": "staged",
        "claims": 1,
        "objections": 1,
        "negative_results": 1,
    }));
    let reponse = poster(
        &adresse,
        EVENTS_PATH,
        Some(CREANCE),
        &corps_evenements("idem-contradiction", &[contradiction]),
    )
    .await;

    assert!(
        reponse.starts_with("HTTP/1.1 202"),
        "une contradiction entre au même titre qu'un accord :\n{reponse}"
    );
    assert_eq!(
        faits_sur(&adresse, &format!("task/{TACHE}")).await,
        vec![
            "epistemic_object.staged",
            "epistemic_object.validated",
            "epistemic_object.staged",
        ],
        "les trois faits restent : le journal ajoute, il ne réconcilie pas"
    );
}

// ---------------------------------------------------------------------------------------------
// 5. Ce que la reconnaissance d'un commit ne fait pas.
// ---------------------------------------------------------------------------------------------

/// **Un événement de progression n'est pas un commit, même s'il porte un `status`.**
///
/// La reconnaissance se fait sur le **préfixe de type**, celui que `canterel` émet depuis `W2.15`,
/// et non sur la présence d'un champ. Reconnaître par la charge ferait d'un `progress` portant un
/// `status` un objet épistémique — et un objet épistémique qui apparaît sans que personne l'ait
/// proposé est exactement ce que l'invariant 2 refuse.
#[test]
fn un_evenement_de_progression_portant_un_statut_n_est_pas_un_commit() {
    let mut progression: Event = fixture("event-reconnection-2-progress.json");
    progression.payload = Some(serde_json::json!({ "status": "validated" }));

    assert!(
        staging(&progression, TACHE, WORKER).is_none(),
        "un `progress` n'est pas un commit, quoi qu'il porte dans sa charge"
    );
    assert!(
        staging(&commit("staged"), TACHE, WORKER).is_some(),
        "et un `epistemic_commit.*` en est un — sans quoi le test précédent ne prouverait rien"
    );
}

/// **Un commit sans statut est mis en scène, jamais promu.**
///
/// `W2.15` met en scène avant de signer, donc le champ est là en pratique. Le défaut compte quand
/// même : le seul cas qu'il pourrait masquer est celui d'un worker qui **voulait** annoncer
/// `validated` et a oublié le champ — et `staged` est précisément ce que §2.3 lui accorde.
#[test]
fn un_commit_sans_statut_est_mis_en_scene() {
    let mut muet = commit("staged");
    muet.payload = Some(serde_json::json!({ "commit_hash": "sha256:00" }));

    let mise = staging(&muet, TACHE, WORKER).expect("c'est un commit");
    assert_eq!(mise.accepted().expect("statut par défaut"), Status::Staged);
}

/// **Le type de fait est celui du statut, et il vit dans le namespace de §10.3.**
///
/// Écrit en clair et non recomposé : `packages/projections` rejoue `epistemic_object.staged` dans
/// ses propres tests, et les deux doivent désigner la même chose. Une passe de mutation a montré
/// ailleurs qu'une constante comparée à elle-même ne vérifie rien.
#[test]
fn le_type_de_fait_est_celui_du_statut() {
    assert_eq!(fact_type(Status::Staged), "epistemic_object.staged");
    assert_eq!(fact_type(Status::Validated), "epistemic_object.validated");
    assert_eq!(fact_type(Status::Contested), "epistemic_object.contested");
}

// ---------------------------------------------------------------------------------------------
// 6. Ce qu'une passe de mutation a trouvé, et que rien ne tenait.
// ---------------------------------------------------------------------------------------------

/// **Un commit mis en scène est enregistré à `L0`, et pas à un niveau qu'on lui prêterait.**
///
/// C'est la propriété la plus coûteuse à perdre de ce module : une mise en scène enregistrée à `L6`
/// ferait lire une soumission de worker comme un résultat institutionnellement accepté — l'invariant
/// 3 défait par un champ, sans qu'aucun statut soit franchi.
///
/// `L0` n'est pas déduit de `staged` : c'est le constat que **personne n'a évalué**. La nuance est
/// écrite dans le module ; ce test est ce qui la rend vérifiable, et une passe de mutation a montré
/// que rien ne l'éprouvait — remonter le niveau à `institutionally_accepted` survivait.
#[tokio::test]
async fn un_commit_mis_en_scene_est_enregistre_non_evalue() {
    let runtime = Arc::new(daemon());
    let adresse = servir(Arc::clone(&runtime)).await;

    let _ = poster(
        &adresse,
        EVENTS_PATH,
        Some(CREANCE),
        &corps_evenements("idem-commit", &[commit("staged")]),
    )
    .await;

    runtime.with_validation_state(|etat| {
        let objet = etat
            .get(&format!("task/{TACHE}"))
            .expect("la projection a vu l'objet");
        assert_eq!(objet.status, "staged");
        assert_eq!(
            objet.validation_level, "unassessed",
            "un commit que personne n'a évalué est enregistré `L0` : lui prêter un niveau ferait \
             lire une soumission de worker comme un verdict"
        );
    });
}

/// **Les deux champs de §9.3 sont posés par le serveur, et rien ne les redéfinit.**
///
/// Le contexte est écrit d'abord, les deux champs propres ensuite — donc ils gagnent par
/// construction. Ce test le vérifie en soumettant un contexte qui les porte tous les deux, avec des
/// valeurs qu'il ne faut surtout pas retenir.
///
/// La forme précédente était une garde qui refusait d'écraser ; une passe de mutation a montré
/// qu'elle était **inatteignable**, parce que le contexte est bâti par le module et ne porte jamais
/// ces clés. Elle a donc été supprimée, et la propriété rendue vraie par l'ordre d'écriture — qui,
/// lui, s'éprouve.
#[test]
fn le_serveur_fixe_les_deux_champs_quoi_que_porte_le_contexte() {
    let empoisonne = locusd::fields([
        ("status", serde_json::json!("validated")),
        (
            "validation_level",
            serde_json::json!("institutionally_accepted"),
        ),
        ("task_id", serde_json::json!(TACHE)),
    ]);

    let corps = locusd::payload(Status::Staged, ValidationLevel::Unassessed, empoisonne);

    assert_eq!(corps["status"], "staged");
    assert_eq!(corps["validation_level"], "unassessed");
    assert_eq!(
        corps["task_id"], TACHE,
        "le reste du contexte, lui, traverse : ce qui est fixé est ce que §9.3 exige, pas tout"
    );
}

/// **Dans un lot mixte, deux faits ne portent pas la même identité.**
///
/// La réserve d'identités est **fournie** — ce crate n'en fabrique pas —, et chaque fait en prend
/// une au rang qui lui revient. Un commit qui prendrait toujours le rang `0` collisionnerait avec le
/// premier événement du lot, et le journal porterait deux faits différents sous le même `event_id`.
///
/// La faute serait silencieuse : les deux faits s'écrivent, la réponse est `202`, et rien ne le dit
/// avant qu'on essaie de relire l'histoire par identité. Une passe de mutation l'a établi en fixant
/// le rang à `0` sans faire rougir quoi que ce soit.
#[test]
fn un_lot_mixte_ne_reutilise_pas_une_identite() {
    let mut progression: Event = fixture("event-reconnection-2-progress.json");
    progression.task_id = Some(TACHE.to_owned());
    progression.worker_id = Some(WORKER.to_owned());

    let rapport = locusd::lep::Report {
        events: vec![progression, commit("staged")],
        worker_id: WORKER.to_owned(),
    };
    let contexte = locusd::lep::LepContext {
        project_id: id::<Project>(4),
        event_ids: vec![id::<EventId>(11), id::<EventId>(12)],
        occurred_at: Timestamp::from_millis(1_700_000_000_000),
        payload_hash: String::new(),
    };
    let commande = locusd::CommandEnvelope::mutating(
        id::<CommandId>(13),
        "worker.report",
        id::<Workspace>(2),
        id::<Agent>(3),
        "idem-mixte".to_owned(),
        locusd::Revision::new(0),
    )
    .expect("enveloppe valide");

    let faits = locusd::Decide::decide(&rapport, &commande, &contexte).expect("le lot passe");

    assert_eq!(faits.len(), 2, "un fait par événement");
    assert_ne!(
        faits[0].event_id, faits[1].event_id,
        "deux faits d'un même lot ne peuvent pas porter la même identité — la réserve en fournit \
         une par rang, et ce crate n'en fabrique pas"
    );
    assert_eq!(
        faits[1].event_type.to_string(),
        "epistemic_object.staged",
        "et le second est bien le commit, pas une seconde progression"
    );
}

/// **Un commit dont la tâche est une chaîne vide est refusé.**
///
/// Le chemin est réel : `Report` exige qu'un événement porte un `task_id`, et `Some("")` satisfait
/// cette exigence. Sans cette garde, le fait partirait sur le stream `task/` — un stream que
/// personne ne relit jamais, et où les commits de toutes les tâches sans nom se mêleraient.
///
/// Une passe de mutation l'a trouvée non éprouvée : la neutraliser survivait.
#[tokio::test]
async fn un_commit_sans_tache_nommee_est_refuse() {
    let adresse = servir(Arc::new(daemon())).await;
    let mut anonyme = commit("staged");
    anonyme.task_id = Some(String::new());

    let reponse = poster(
        &adresse,
        EVENTS_PATH,
        Some(CREANCE),
        &corps_evenements("idem-anonyme", &[anonyme]),
    )
    .await;

    assert!(
        reponse.starts_with("HTTP/1.1 400"),
        "une tâche vide est une requête à corriger :\n{reponse}"
    );
    assert!(
        reponse.contains("task_id"),
        "le refus nomme le champ :\n{reponse}"
    );
    assert!(
        faits_sur(&adresse, "task/").await.is_empty(),
        "et rien n'atterrit sur le stream sans nom"
    );
}
