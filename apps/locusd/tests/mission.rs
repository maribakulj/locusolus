//! Le test de sortie de `W20.o` — une mission naît d'une question.
//!
//! # La clause « valide contre son schéma » a été rétrécie, et le motif est ici
//!
//! Elle disait : « la `MissionEnvelope` produite valide contre son schéma de `W0.5`, vérifié par le
//! registre de schémas et non par relecture ». Le registre de schémas est **en TypeScript** —
//! `tooling/schemas/check-schemas.ts`, sur `ajv` — et il valide les fixtures du corpus, pas une
//! valeur produite en Rust. Valider ici demanderait un validateur Draft 7 côté Rust : `jsonschema`
//! coûte **88 paquets**, plus que le driver `PostgreSQL` entier, pour une propriété de test.
//!
//! Ce qui la remplace est plus étroit et **réellement vérifié** : la liste `required` est lue du
//! fichier de schéma au moment du test, et chaque champ y est cherché dans la mission sérialisée.
//! C'est la partie qu'un constructeur se trompe effectivement — un champ obligatoire oublié —, et
//! elle est tenue contre la source de vérité plutôt que contre une liste recopiée.
//!
//! Ce qui n'est **pas** vérifié : les contraintes de valeur du schéma — `minLength`, formats,
//! `additionalProperties`. Le dire évite de lire « valide contre son schéma » là où il faut lire
//! « porte tous ses champs obligatoires ».

use std::sync::Arc;

use locus_domain::task::TaskState;
use locus_lep::{MissionEnvelopeBudget, NetworkMode, ResourceSpec, SandboxLevel};
use locus_protocol::id::{Agent, Command as CommandId, Event as EventId, Project, Workspace};
use locus_protocol::{Id, IdKind, Timestamp};
use locusd::context_view::Requested;
use locusd::lep::{Desk, Identities, MemoryQueue, MemoryRegistry, MissionQueue, Submitted};
use locusd::mission::{Authority, Proposal, claimable};
use locusd::{CommandError, Runtime};

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

/// L'empreinte qu'emploient les tests **sans daemon**.
///
/// Ceux-là construisent une `MissionEnvelope` depuis une `Proposal` et ne proposent rien : aucune
/// liaison n'est traversée, donc aucune vue n'a à exister. La leur faire déposer une vue ferait
/// croire que la construction d'une enveloppe en dépend.
const EMPREINTE_DE_FIXTURE: &str =
    "sha256:abababababababababababababababababababababababababababababababab";

/// Déposer la vue que les propositions de ce fichier nomment, et rendre **son** empreinte.
///
/// Depuis `W20.ac`, `lep_propose` refuse une proposition qui nomme une vue que personne n'a bâtie,
/// ou qui en annonce une empreinte qui n'est pas la sienne. Les tests d'ici passent donc par la
/// commande réelle plutôt que d'inventer un condensat — ce qui est le point de l'item.
///
/// La demande est lue d'un document JSON pour deux raisons : elle exerce les défauts de `serde`, et
/// elle n'oblige pas ce fichier à énumérer les seize champs facultatifs de §16.2.
fn depose_la_vue(runtime: &Runtime<locus_event_store::MemoryEventStore>) -> String {
    let demande: Requested = serde_json::from_value(serde_json::json!({
        "id": "ctx_1",
        "query": "tenue du catalyseur A",
        "source_event_watermark": 0,
        "recipient": {
            "agent_id": id::<Agent>(9).to_string(),
            "worker_id": "canterel-vm-linux-01",
            "clearance": "internal"
        },
        "candidates": []
    }))
    .expect("la demande de vue est bien formée");
    runtime
        .build_context_view(
            &demande,
            autorite(),
            &soumis("idem-vue"),
            Timestamp::from_millis(1_700_000_000_000),
        )
        .expect("la vue est bâtie")
        .content_hash
}

fn proposition(context_view_hash: &str) -> Proposal {
    Proposal {
        context_view_hash: context_view_hash.to_owned(),
        cognition: locus_domain::CognitionClass::Economy,
        statement: "Le catalyseur A tient-il au-delà de 300 °C ?".to_owned(),
        success_conditions: vec!["une mesure reproductible à trois essais".to_owned()],
        task_id: "tsk_catalyseur".to_owned(),
        attempt_id: "att_1".to_owned(),
        attempt: 1,
        branch_id: "br_principal".to_owned(),
        context_view_id: "ctx_1".to_owned(),
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
        output_contract: "epistemic-commit/1".to_owned(),
    }
}

fn autorite() -> Authority {
    Authority {
        workspace_id: id::<Workspace>(2),
        principal_id: id::<Agent>(3),
    }
}

fn soumis(cle: &str) -> Submitted {
    Submitted {
        idempotency_key: cle.to_owned(),
        project_id: id::<Project>(4),
        occurred_at: Timestamp::from_millis(1_700_000_000_000),
    }
}

fn daemon() -> (
    Runtime<locus_event_store::MemoryEventStore>,
    Arc<MemoryQueue>,
) {
    let file = Arc::new(MemoryQueue::new());
    let registre = Arc::new(MemoryRegistry::new());
    // Une créance reconnue : `lep_events` identifie le worker avant d'écrire, et un daemon qui n'en
    // connaîtrait aucun refuserait la remontée avant d'arriver au refus qu'on veut éprouver.
    registre.admit(
        CREANCE,
        locusd::lep::WorkerIdentity {
            worker_id: WORKER.to_owned(),
            workspace_id: id::<Workspace>(2),
            principal_id: id::<Agent>(3),
            project_id: id::<Project>(4),
        },
    );
    let desk = Desk::new(
        Arc::clone(&file) as Arc<dyn MissionQueue>,
        registre,
        Arc::new(Identites::default()),
    );
    (Runtime::in_memory().with_lep(desk), file)
}

/// La créance du worker de ces tests, et le worker qu'elle désigne.
const CREANCE: &str = "creance-de-test";
const WORKER: &str = "canterel-de-test";

// ---------------------------------------------------------------------------------------------
// 1. Une question produit une mission, et le fait atteint le journal.
// ---------------------------------------------------------------------------------------------

/// **Proposer écrit un fait, et ne met rien en file.**
///
/// Une tâche `proposed` n'est pas réclamable : §7.1 exige qu'elle passe par `queued`. Enfiler dès la
/// proposition confierait à un worker une mission que personne n'a mise en file — et le journal
/// porterait un `task.leased` qu'aucun `task.queued` n'a précédé.
#[test]
fn proposer_ecrit_un_fait_et_ne_met_rien_en_file() {
    let (runtime, file) = daemon();
    let empreinte = depose_la_vue(&runtime);
    runtime
        .lep_propose(
            &proposition(&empreinte),
            autorite(),
            &soumis("idem-propose"),
            Timestamp::from_millis(1_700_000_000_000),
        )
        .expect("la proposition aboutit");

    let faits: Vec<String> = runtime
        .timeline(None, None)
        .expect("timeline")
        .items
        .iter()
        .map(|entry| entry.event_type.clone())
        .collect();
    // `context_view.built` précède, et c'est `W20.ac` : une mission ne nomme plus un contexte que
    // personne n'a déposé, donc le dépôt de la vue est un fait du journal comme les autres.
    assert_eq!(
        faits,
        vec!["context_view.built".to_owned(), "task.proposed".to_owned()]
    );
    assert!(
        file.is_empty(),
        "rien n'est mis en file par une proposition"
    );
}

/// **Mettre en file écrit un fait *et* dépose la mission.**
///
/// C'est le test de sortie : avant `W20.o`, la file n'était garnie que par un test. Elle l'est
/// maintenant par le daemon, et la mission déposée est celle que la proposition décrit.
#[test]
fn mettre_en_file_depose_la_mission_que_la_question_decrit() {
    let (runtime, file) = daemon();
    let proposal = proposition(&depose_la_vue(&runtime));
    let maintenant = Timestamp::from_millis(1_700_000_000_000);

    runtime
        .lep_propose(&proposal, autorite(), &soumis("idem-propose"), maintenant)
        .expect("proposition");
    runtime
        .lep_queue(
            &proposal.task_id,
            autorite(),
            &soumis("idem-queue"),
            maintenant,
        )
        .expect("mise en file");

    let faits: Vec<String> = runtime
        .timeline(None, None)
        .expect("timeline")
        .items
        .iter()
        .map(|entry| entry.event_type.clone())
        .collect();
    assert_eq!(
        faits,
        vec![
            "context_view.built".to_owned(),
            "task.proposed".to_owned(),
            "task.queued".to_owned()
        ]
    );

    let en_file = file.take("peu-importe").expect("une mission attend");
    assert_eq!(en_file.mission.task_id, proposal.task_id);
    assert_eq!(en_file.mission.objective.statement, proposal.statement);
    assert_eq!(
        en_file.attempt, proposal.attempt,
        "le rang vient de la proposition — §12.3 veut qu'une réattribution le conserve"
    );
    // **Aucun bail** dans la file — `W20.v`. Un bail autorise un worker, et aucun n'est choisi ici.
    // Le type le rend inexprimable : `Queued` ne porte pas le champ.
}

/// **Une transition que §7.1 refuse ne s'écrit pas, et ne dépose pas une seconde mission.**
///
/// La table est interrogée, jamais recopiée : `queued → queued` n'existe pas, et le refus le dit
/// sous la famille `policy` — la requête est bien formée, c'est l'état qui s'y oppose.
///
/// Depuis `W20.s` l'appelant ne déclare plus l'état de départ, donc ce test ne peut plus l'inventer
/// non plus : il **amène** la tâche en `queued` par le chemin normal, puis redemande la mise en
/// file. C'est précisément ce que l'ancienne signature laissait contourner — il suffisait
/// d'annoncer `proposed` une seconde fois pour que la garde valide un état que le journal
/// démentait.
#[test]
fn une_transition_interdite_ne_met_rien_en_file() {
    let (runtime, file) = daemon();
    let proposal = proposition(&depose_la_vue(&runtime));
    let maintenant = Timestamp::from_millis(1_700_000_000_000);

    runtime
        .lep_propose(&proposal, autorite(), &soumis("idem-propose"), maintenant)
        .expect("proposition");
    runtime
        .lep_queue(
            &proposal.task_id,
            autorite(),
            &soumis("idem-queue"),
            maintenant,
        )
        .expect("première mise en file");
    file.take("peu-importe").expect("la première a déposé");

    let refus = runtime
        .lep_queue(
            &proposal.task_id,
            autorite(),
            &soumis("idem-requeue"),
            maintenant,
        )
        .expect_err("§7.1 ne permet pas queued → queued");

    assert!(matches!(refus, CommandError::Policy { .. }), "{refus:?}");
    assert!(
        file.is_empty(),
        "un refus n'écrit rien et ne dépose rien : c'est la transaction qui écrit"
    );
}

/// **Mettre en file une tâche que personne n'a proposée est refusé en nommant le champ.**
///
/// L'état ne se lit plus de l'appelant, donc il faut qu'il existe. Un stream vide n'est pas un
/// `proposed` implicite : c'est une tâche qui n'existe pas, et le refus doit le dire au client
/// plutôt que d'échouer plus loin sur une proposition absente.
#[test]
fn mettre_en_file_une_tache_inconnue_nomme_le_champ() {
    let (runtime, file) = daemon();

    let refus = runtime
        .lep_queue(
            "task-jamais-proposee",
            autorite(),
            &soumis("idem-queue"),
            Timestamp::from_millis(1_700_000_000_000),
        )
        .expect_err("rien n'a été proposé sous cet identifiant");

    match refus {
        CommandError::Validation { field, .. } => assert_eq!(field, "task_id"),
        autre => panic!("le refus doit nommer le champ, pas échouer plus loin : {autre:?}"),
    }
    assert!(file.is_empty());
}

/// **Une question vide est refusée en nommant le champ.**
///
/// « Une mission qui omet un [de ces champs] ne peut pas être jugée — elle serait acceptée par
/// défaut, ce qui est exactement le contraire de l'admission », dit `MissionEnvelope`. Une question
/// vide passerait la construction et arriverait chez un worker qui ne saurait pas quoi en faire.
#[test]
fn une_question_vide_est_refusee_en_nommant_le_champ() {
    let (runtime, _) = daemon();
    // La vue existe : ce test veut voir le refus de la **question**, et un contexte introuvable en
    // produirait un autre, plus tôt. Une assertion ne vaut que si elle observe le défaut qu'elle
    // nomme.
    let empreinte = depose_la_vue(&runtime);
    let mut muette = proposition(&empreinte);
    muette.statement = "   ".to_owned();

    let refus = runtime
        .lep_propose(
            &muette,
            autorite(),
            &soumis("idem"),
            Timestamp::from_millis(1_700_000_000_000),
        )
        .expect_err("une mission sans question ne se propose pas");
    assert!(
        matches!(&refus, CommandError::Validation { field, .. } if field == "objective.statement"),
        "{refus:?}"
    );

    let mut sans_critere = proposition(&empreinte);
    sans_critere.success_conditions.clear();
    let refus = runtime
        .lep_propose(
            &sans_critere,
            autorite(),
            &soumis("idem-2"),
            Timestamp::from_millis(1_700_000_000_000),
        )
        .expect_err("sans condition de succès, rien ne dit quand c'est traité");
    assert!(
        matches!(&refus, CommandError::Validation { field, .. } if field == "objective.success_conditions"),
        "{refus:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// 2. La mission porte tous les champs que son schéma exige.
// ---------------------------------------------------------------------------------------------

/// **Les champs `required` sont lus du schéma, pas d'une liste recopiée.**
///
/// Voir l'en-tête de ce fichier pour ce que cette vérification remplace et ce qu'elle ne couvre
/// pas. Ce qu'elle tient : un champ obligatoire ajouté au schéma fera **rougir ce test** tant que le
/// constructeur ne l'émettra pas — ce qu'une liste écrite ici ne ferait jamais.
#[test]
fn la_mission_porte_tous_les_champs_que_le_schema_exige() {
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../schemas/lep/1.0/mission-envelope.schema.json"),
        )
        .expect("le schéma de W0.5 est lisible"),
    )
    .expect("schéma en JSON valide");

    let exiges: Vec<&str> = schema["required"]
        .as_array()
        .expect("un schéma d'objet nomme ses champs obligatoires")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    assert!(
        exiges.len() >= 10,
        "le schéma doit exiger quelque chose — {exiges:?} ne prouverait rien"
    );

    let mission =
        serde_json::to_value(proposition(EMPREINTE_DE_FIXTURE).envelope()).expect("sérialisable");
    let objet = mission.as_object().expect("une mission est un objet");
    for champ in exiges {
        assert!(
            objet.contains_key(champ),
            "la mission produite n'émet pas « {champ} », que le schéma de `lep/1.0` exige"
        );
    }
}

/// **Chaque champ de la mission vient de la proposition, et aucun n'est inventé.**
///
/// Le test précédent vérifie que les champs obligatoires sont **présents** ; celui-ci qu'ils portent
/// ce qu'on a demandé. La distinction n'est pas théorique : une passe de mutation a vidé
/// `output_contract` en `String::new()` et le test de présence est resté vert — la clé était là, et
/// la mission demandait à un worker de ne rien rendre.
///
/// Un contrat de sortie vide est le cas le plus net, mais la faute est générique : un constructeur
/// qui perd une valeur produit une mission bien formée et vide de sens. C'est donc l'**égalité**
/// qui est vérifiée, champ par champ, et non la forme.
#[test]
fn chaque_champ_de_la_mission_vient_de_la_proposition() {
    let proposal = proposition(EMPREINTE_DE_FIXTURE);
    let mission = proposal.envelope();

    assert_eq!(mission.task_id, proposal.task_id);
    assert_eq!(mission.attempt_id, proposal.attempt_id);
    assert_eq!(mission.branch_id, proposal.branch_id);
    assert_eq!(mission.objective.statement, proposal.statement);
    assert_eq!(
        mission.objective.success_conditions,
        proposal.success_conditions
    );
    assert_eq!(mission.context_view.id, proposal.context_view_id);
    assert_eq!(mission.context_view.hash, proposal.context_view_hash);
    assert_eq!(mission.environment.environment_id, proposal.environment_id);
    assert_eq!(mission.sandbox.minimum_level, proposal.sandbox_level);
    assert_eq!(mission.sandbox.network, proposal.network);
    assert_eq!(mission.resources, proposal.resources);
    assert_eq!(mission.budget, proposal.budget);
    assert_eq!(mission.output_contract, proposal.output_contract);

    // Et aucun de ces champs n'est vide : une mission bien formée dont l'objectif ou le contrat de
    // sortie est vide ne peut pas être jugée, ce que `MissionEnvelope` refuse dans sa propre
    // documentation.
    assert!(!mission.output_contract.is_empty());
    assert!(!mission.objective.statement.is_empty());
    assert!(!mission.objective.success_conditions.is_empty());
}

/// **Et elle annonce la version que ce daemon sert.**
#[test]
fn la_mission_annonce_le_protocole_du_daemon() {
    assert_eq!(
        proposition(EMPREINTE_DE_FIXTURE).envelope().protocol,
        "lep/1.0"
    );
}

/// **Les champs optionnels restent absents.**
///
/// « Absent ne se remplit pas d'un défaut » — un document `1.0` qui ne parle pas de `role` ne le
/// demande pas, et lui en donner un ferait choisir au serveur ce que l'appelant n'a pas demandé.
/// `offline_allowed` est le cas le plus net : une **dispense** accordée sans qu'on la demande.
#[test]
fn les_champs_optionnels_restent_absents() {
    let mission =
        serde_json::to_value(proposition(EMPREINTE_DE_FIXTURE).envelope()).expect("sérialisable");
    let objet = mission.as_object().expect("un objet");
    for champ in [
        "role",
        "review_policy",
        "offline_allowed",
        "offline_budget_ms",
        "required_capabilities",
        "confidentiality_ceiling",
        "deadline",
    ] {
        assert!(
            !objet.contains_key(champ),
            "« {champ} » est absent du schéma d'entrée : le serveur ne le remplit pas d'un défaut"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 3. Réclamable se lit de §7.1, et le placement reste ailleurs.
// ---------------------------------------------------------------------------------------------

/// **Réclamable veut dire « `Leased` est atteignable », et rien d'autre.**
///
/// La règle n'est pas écrite dans `mission.rs` : elle est **déduite** du tableau de §7.1. Ce test le
/// vérifie sur les quinze états, en recalculant l'attendu depuis `allowed()` — donc sans recopier
/// la table ici non plus. Ce qu'il éprouve n'est pas la liste mais l'**équivalence**.
#[test]
fn reclamable_se_lit_de_la_machine_a_etats() {
    for state in TaskState::ALL {
        assert_eq!(
            claimable(state),
            state.allowed().contains(&TaskState::Leased),
            "« {state} » : réclamable doit se lire du tableau, jamais d'une seconde liste"
        );
    }
    // Et les trois cas qui comptent, nommés — un test qui ne ferait que la boucle ci-dessus
    // passerait aussi si `claimable` rendait toujours `false`.
    assert!(claimable(TaskState::Queued), "une tâche en file se réclame");
    assert!(
        !claimable(TaskState::Proposed),
        "une tâche proposée doit d'abord être mise en file"
    );
    assert!(
        !claimable(TaskState::Cancelled),
        "une tâche annulée ne se réclame plus"
    );
}

/// **Créer une mission ne choisit aucun hôte.**
///
/// §4 et `W4.g` : `place` décide de l'hôte, et il vit chez `locus-execd`. Un `locusd` qui placerait
/// en créant reprendrait une décision que l'ADR 0004 a séparée — et la reprendrait au seul endroit
/// où personne ne penserait à la chercher.
#[test]
fn creer_une_mission_ne_choisit_aucun_hote() {
    let source = include_str!("../src/mission.rs");
    for interdit in ["place(", "host", "hote_choisi", "Placement"] {
        assert!(
            !source.contains(interdit),
            "« {interdit} » dans la création de mission : le placement est celui de `W4.g`, chez \
             `locus-execd`"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 4. Un refus de worker remet la mission en file — `W19.c`, ADR 0037.
// ---------------------------------------------------------------------------------------------

/// Un `task.refused` tel que le worker l'émettra, une fois `refusal-events` négociée.
fn refus(task_id: &str, code: &str) -> locus_lep::Event {
    locus_lep::Event {
        protocol: "lep/1.1".to_owned(),
        event_type: "task.refused".to_owned(),
        sequence: 1,
        occurred_at: "2026-09-01T10:00:00Z".to_owned(),
        idempotency_key: "idem-refus".to_owned(),
        task_id: Some(task_id.to_owned()),
        attempt: Some(1),
        lease_id: None,
        worker_id: None,
        correlation_id: None,
        causation_id: None,
        payload: Some(serde_json::json!({ "code": code, "details": {} })),
        payload_hash: None,
    }
}

/// **Le refus d'un worker remet la mission en file, et le fait est écrit.**
///
/// C'est le test de sortie de `W19.c`. Avant lui, `runLoop` refusait et rendait la main : la mission
/// restait sous bail jusqu'à expiration, et « le worker a refusé » se confondait avec « le worker est
/// mort ».
///
/// **La conséquence est ce qui compte, pas la trace.** Le chemin générique de `Report` écrit déjà un
/// fait pour n'importe quel type d'événement ; un `task.refused` qui n'aurait fait que cela serait
/// une valeur d'énumération sans effet, ce que l'ADR 0022 décision 0 refuse. Le test exige donc les
/// deux, et la mission remise est **comparée** à celle qui avait été mise en file — remettre une
/// autre mission serait pire que n'en remettre aucune.
#[test]
fn un_refus_de_worker_remet_la_mission_en_file() {
    let (runtime, file) = daemon();
    let proposal = proposition(&depose_la_vue(&runtime));
    let maintenant = Timestamp::from_millis(1_700_000_000_000);

    runtime
        .lep_propose(&proposal, autorite(), &soumis("idem-propose"), maintenant)
        .expect("proposition");
    runtime
        .lep_queue(
            &proposal.task_id,
            autorite(),
            &soumis("idem-queue"),
            maintenant,
        )
        .expect("mise en file");

    // La réclamation, modélisée par ce qu'elle fait à la file : la mission en sort. Passer par
    // `lep_claim` demanderait un broker, et ce qui est éprouvé ici est l'état « la mission n'est plus
    // réclamable », quelle que soit la façon dont elle y est arrivée.
    let servie = file
        .take(WORKER)
        .expect("la mission mise en file est réclamable");
    assert!(file.is_empty(), "une mission servie quitte la file");

    runtime
        .lep_events(
            CREANCE,
            vec![refus(&proposal.task_id, "model_unavailable")],
            &locusd::lep::WorkerSubmission {
                idempotency_key: "idem-refus".to_owned(),
                proposed_project: None,
                occurred_at: maintenant,
            },
            maintenant,
        )
        .expect("la remontée d'un refus aboutit");

    let revenue = file
        .take(WORKER)
        .expect("le refus remet la mission en file : c'est toute la conséquence de W19.c");
    assert_eq!(
        revenue.mission.task_id, servie.mission.task_id,
        "la mission remise est celle qui avait été servie"
    );
    assert_eq!(
        revenue.attempt, servie.attempt,
        "le rang vient de la proposition, pas du bail perdu : aucune tentative n'a eu lieu"
    );

    let faits: Vec<String> = runtime
        .timeline(None, None)
        .expect("timeline")
        .items
        .iter()
        .map(|entry| entry.event_type.clone())
        .collect();
    assert!(
        faits.iter().any(|fait| fait.contains("task.refused")),
        "le refus laisse aussi une trace, sans quoi la mission reviendrait sans qu'on sache pourquoi : {faits:?}"
    );
}

/// **Un événement ordinaire ne remet rien en file.**
///
/// Le symétrique, et il n'est pas décoratif : une remise en file déclenchée par n'importe quelle
/// remontée ferait réclamer deux fois une mission qui s'exécute. La reconnaissance se fait sur le
/// type, et ce test est ce qui l'exige.
#[test]
fn un_evenement_ordinaire_ne_remet_rien_en_file() {
    let (runtime, file) = daemon();
    let proposal = proposition(&depose_la_vue(&runtime));
    let maintenant = Timestamp::from_millis(1_700_000_000_000);

    runtime
        .lep_propose(&proposal, autorite(), &soumis("idem-propose"), maintenant)
        .expect("proposition");
    runtime
        .lep_queue(
            &proposal.task_id,
            autorite(),
            &soumis("idem-queue"),
            maintenant,
        )
        .expect("mise en file");
    file.take(WORKER).expect("la mission est servie");

    let mut progres = refus(&proposal.task_id, "model_unavailable");
    progres.event_type = "progress".to_owned();
    runtime
        .lep_events(
            CREANCE,
            vec![progres],
            &locusd::lep::WorkerSubmission {
                idempotency_key: "idem-progres".to_owned(),
                proposed_project: None,
                occurred_at: maintenant,
            },
            maintenant,
        )
        .expect("la remontée aboutit");

    assert!(
        file.is_empty(),
        "seul un refus remet en file : un progrès laisse la mission là où elle est"
    );
}
