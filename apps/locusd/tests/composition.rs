//! Le test de sortie de `W20.d` — le composition root câble, et ne décide rien.

use locus_event_store::{
    Actor, ActorKind, Draft as EventDraft, EventStore, EventType, MemoryEventStore,
};
use locus_policy::{Facts, Outcome as PolicyOutcome, Policy, Rule, Verb};
use locus_projections::NodeKind;
use locus_protocol::id::{Agent, Command, Event, Project, Workspace};
use locus_protocol::{Id, IdKind, Timestamp};
use locusd::composition::{Readiness, Runtime, Wired};
use locusd::{CommandEnvelope, CommandError, Decide, Revision};

const NOW: Timestamp = Timestamp::from_millis(1_700_000_000_000);

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

/// Un décideur qui produit un fait d'exécution, celui que `execution_graph` consomme.
struct Executer;

impl Decide for Executer {
    type State = ();

    fn decide(
        &self,
        _: &CommandEnvelope,
        (): &Self::State,
    ) -> Result<Vec<EventDraft>, CommandError> {
        Ok(vec![evenement("task/tsk_01")])
    }
}

/// Un fait d'exécution bien formé, que `execution_graph` consomme sans faute.
fn evenement(stream: &str) -> EventDraft {
    EventDraft {
        event_id: id::<Event>(9),
        event_type: EventType::parse("task.started").expect("type valide"),
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
        payload: serde_json::json!({ "attempt_id": "att_01", "worker_id": "wrk_01" }),
        payload_hash: format!("sha256:{}", "ab".repeat(32)),
    }
}

fn commande() -> CommandEnvelope {
    CommandEnvelope::mutating(
        id::<Command>(1),
        "task.start",
        id::<Workspace>(2),
        id::<Agent>(3),
        "idem-1",
        Revision::INITIAL,
    )
    .expect("commande bien formée")
}

// ---------------------------------------------------------------------------------------------
// 1. Le binaire démarre : l'assemblage produit les projections et le moteur de politique
// ---------------------------------------------------------------------------------------------

/// **Les projections de §9.5 sont câblées**, sous les noms qu'elles se donnent.
///
/// Les nommer une par une plutôt que compter : un test qui vérifierait `len() == 5` resterait vert
/// si une projection était remplacée par une autre, ou par la même deux fois. Le nom du test ne
/// porte plus de nombre non plus — il en portait un, et `W20.u` l'a démenti.
#[test]
fn l_assemblage_cable_les_projections_de_9_5() {
    let runtime = Runtime::in_memory();
    let readiness = runtime.catch_up();

    let noms: Vec<&str> = readiness.projections.iter().map(|w| w.name).collect();
    assert_eq!(
        noms,
        vec![
            "execution_graph",
            "organisation_graph",
            "conflict_registry",
            "validation_state",
            "epistemic_graph"
        ]
    );
    assert!(
        readiness.is_ready(),
        "un journal vide ne met rien en quarantaine"
    );
    assert!(readiness.quarantined().is_empty());
}

/// **Le moteur de politique est joignable depuis l'assemblage, et il décide.**
///
/// Compter les règles n'aurait rien prouvé : un compteur est vrai d'un moteur débranché. Ce qui
/// atteste le câblage est qu'une évaluation traverse le composition root et rend un verdict.
#[test]
fn le_moteur_de_politique_est_cable_et_decide() {
    let regle = Rule::declare(
        "sandbox-obligatoire",
        1,
        10,
        &[("execution", "untrusted")],
        Verb::Deny,
    )
    .expect("règle bien formée");
    let policy = Policy::new().with(regle).expect("règle unique");
    let runtime = Runtime::assemble(MemoryEventStore::new(), policy);

    let refuse = runtime.simulate(&Facts::new().with("execution", "untrusted"));
    assert_eq!(
        refuse.would_decide(),
        &PolicyOutcome::Decided {
            verb: Verb::Deny,
            by: "sandbox-obligatoire".to_owned(),
        }
    );

    // Le silence n'est pas une autorisation — §20.2, et c'est le moteur qui le dit, pas ce test.
    let neutre = runtime.simulate(&Facts::new().with("execution", "trusted"));
    assert_eq!(neutre.would_decide(), &PolicyOutcome::NoRule);
}

// ---------------------------------------------------------------------------------------------
// 2. Le sens de lecture : on écrit par la transaction, les projections lisent
// ---------------------------------------------------------------------------------------------

/// **Une écriture passe par la transaction, et les projections la voient au rattrapage.**
///
/// C'est le câblage complet exercé de bout en bout : commande → décideur → transaction → journal →
/// projection. Aucun raccourci n'existe, et `W20.b` en donne la raison — la seule poignée que le
/// composition root puisse passer aux projections est immuable.
#[test]
fn une_ecriture_traverse_la_transaction_puis_les_projections() {
    let runtime = Runtime::in_memory();
    assert!(
        runtime.with_execution_graph(|graph| graph.of_kind(NodeKind::Worker).is_empty()),
        "avant l'écriture, le graphe est vide"
    );

    let verdict = runtime
        .transaction()
        .submit(&Executer, &commande(), &(), NOW);
    assert!(verdict.accepted().is_some(), "{verdict:?}");

    let readiness = runtime.catch_up();
    assert!(readiness.is_ready(), "{:?}", readiness.quarantined());

    // L'état de la projection, et non seulement l'absence de panne. Se contenter de `is_ready()`
    // aurait laissé passer un rattrapage qui lit un journal vide : il est « prêt » lui aussi. Un
    // mutant l'a montré.
    assert_eq!(
        runtime.with_execution_graph(|graph| graph.of_kind(NodeKind::Worker).len()),
        1,
        "le fait écrit a atteint le graphe d'exécution"
    );
}

/// **Une projection qui fautent passe en quarantaine, et le rapport le dit.**
///
/// La traduction de `Health` vers le rapport n'était vérifiée que sur un `Readiness` construit à la
/// main — donc pas vérifiée du tout. Il fallait une faute réelle, traversant l'assemblage.
#[test]
fn une_faute_reelle_met_en_quarantaine_et_se_lit_dans_le_rapport() {
    /// Un artefact sans `artifact_id` : `execution_graph` refuse, parce qu'un artefact sans identité
    /// n'est pas suivable.
    struct Anonyme;
    impl Decide for Anonyme {
        type State = ();
        fn decide(
            &self,
            _: &CommandEnvelope,
            (): &Self::State,
        ) -> Result<Vec<EventDraft>, CommandError> {
            let mut draft = evenement("artifact/art_01");
            draft.event_type = EventType::parse("artifact.declared").expect("type valide");
            draft.payload = serde_json::json!({ "attempt_id": "att_01" });
            Ok(vec![draft])
        }
    }

    let runtime = Runtime::in_memory();
    runtime
        .transaction()
        .submit(&Anonyme, &commande(), &(), NOW)
        .accepted()
        .expect("le journal accepte : c'est la projection qui refusera");

    let readiness = runtime.catch_up();
    assert!(!readiness.is_ready(), "une projection a fauté");
    // **Les deux** projections qui lisent les faits d'artefact refusent, et pour la même raison :
    // un artefact sans identité n'est rattachable à rien. Que la seconde ait été ajoutée par
    // `W20.u` sans que ce test soit relâché est le point — une liste qui aurait dit « au moins
    // execution_graph » aurait laissé passer une projection devenue muette.
    assert_eq!(
        readiness.quarantined(),
        vec!["execution_graph", "epistemic_graph"]
    );
    assert!(readiness.to_string().contains("EN QUARANTAINE"));
}

/// Le journal n'est joignable qu'en lecture depuis l'extérieur de la transaction.
#[test]
fn le_journal_ne_sort_qu_en_lecture() {
    let runtime = Runtime::in_memory();
    runtime
        .transaction()
        .submit(&Executer, &commande(), &(), NOW)
        .accepted()
        .expect("écrite");

    let evenements = runtime.transaction().store().read_stream("task/tsk_01", 0);
    assert_eq!(evenements.len(), 1);
}

// ---------------------------------------------------------------------------------------------
// 3. « Prêt » ne ment pas
// ---------------------------------------------------------------------------------------------

/// **Une projection en quarantaine rend l'assemblage non prêt, et se nomme.**
///
/// Une projection en quarantaine sert des lectures périmées. Un daemon qui se dirait prêt dans cet
/// état ferait exactement la promesse qu'il ne tient pas — d'où le code de sortie `1` du binaire.
#[test]
fn une_quarantaine_empeche_de_se_dire_pret() {
    let malade = Readiness {
        projections: vec![
            Wired {
                name: "execution_graph",
                healthy: true,
            },
            Wired {
                name: "conflict_registry",
                healthy: false,
            },
        ],
    };

    assert!(!malade.is_ready());
    assert_eq!(malade.quarantined(), vec!["conflict_registry"]);
    assert!(malade.to_string().contains("EN QUARANTAINE"));
}

/// **Un assemblage qui n'a jamais rattrapé n'est pas prêt.**
///
/// `all()` sur un itérateur vide rend `true` : sans la garde, un `Runtime` construit et jamais
/// rattrapé se serait déclaré disponible **avec zéro projection câblée**. C'est le même mensonge que
/// la quarantaine, obtenu par un chemin plus discret — et il compte double depuis `W20.g`, où
/// `is_ready()` décide si le port s'ouvre et ce que `GET /projections/status` annonce.
///
/// Le défaut a été corrigé dans le code avant que ce test n'existe ; un mutant l'a signalé, et
/// c'est lui qui a rappelé qu'une correction sans test est une correction qu'on peut défaire sans
/// s'en apercevoir.
#[test]
fn un_assemblage_qui_n_a_jamais_rattrape_n_est_pas_pret() {
    let vide = Readiness {
        projections: Vec::new(),
    };
    assert!(
        !vide.is_ready(),
        "zéro projection câblée n'est pas une disponibilité"
    );
    assert!(vide.quarantined().is_empty());

    // Et le runtime neuf, avant tout rattrapage, dit la même chose.
    let neuf = Runtime::in_memory();
    assert!(
        !neuf.readiness().is_ready(),
        "un assemblage qui n'a rien lu ne peut pas se dire prêt"
    );
}

/// **Un assemblage qui n'a jamais rattrapé n'est pas prêt.**
///
/// `all()` sur un itérateur vide rend `true` : sans la garde, un rapport à zéro projection se serait
/// déclaré disponible. C'est le même mensonge que la quarantaine, obtenu par un chemin plus discret
/// — et un mutant a montré que rien ne le tenait.
#[test]
fn un_rapport_sans_projection_n_est_pas_pret() {
    let vide = Readiness {
        projections: Vec::new(),
    };
    assert!(
        !vide.is_ready(),
        "zéro projection câblée n'est pas une disponibilité"
    );
    assert!(
        vide.quarantined().is_empty(),
        "et rien n'est en quarantaine non plus : les deux faits sont distincts"
    );
}

// ---------------------------------------------------------------------------------------------
// 4. Ce que le composition root ne contient pas
// ---------------------------------------------------------------------------------------------

/// **Aucune règle métier dans le composition root**, et aucune surface de transport.
///
/// Un composition root finit toujours par recevoir « juste une petite décision », parce qu'il est le
/// seul endroit qui voit tout — donc le pire endroit où cacher une règle. Le test lit la source,
/// faute de pouvoir lire une intention.
#[test]
fn le_composition_root_ne_decide_rien_et_n_ecoute_rien() {
    let source = include_str!("../src/composition.rs");
    let code: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    // Les crates de transport se cherchent dans les **déclarations d'import**, et non dans le
    // fichier entier : le rapport de disponibilité nomme `axum` en toutes lettres pour dire qu'il
    // n'est *pas* branché, et un test qui refuserait le mot interdirait de documenter l'absence.
    // La propriété est « ce fichier ne dépend d'aucun transport », pas « ce mot n'y figure pas ».
    let imports: Vec<&str> = code
        .lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with("use "))
        .collect();
    for transport in ["axum", "tokio", "hyper"] {
        assert!(
            !imports.iter().any(|line| line.contains(transport)),
            "« {transport} » importé : `W20.d` est sans surface HTTP, et l'ADR 0018 autorise sans introduire"
        );
    }

    // Ceux-ci n'apparaissent pas en prose : les chercher dans tout le code est exact.
    for ecoute in ["TcpListener", "bind(", "async fn"] {
        assert!(
            !code.contains(ecoute),
            "« {ecoute} » : le composition root n'écoute rien"
        );
    }

    // `Run::live` accorde le droit d'agir. Le composition root ne l'appelle pas : il expose la
    // simulation, et laisse la décision d'agir à qui a une commande en main.
    assert!(
        !code.contains("Run::live"),
        "le composition root accorderait un droit d'agir, ce qui est une décision"
    );
}

/// **Le binaire rend compte avant de servir, et ne sert pas s'il n'est pas prêt.**
///
/// Ce test a changé avec `W20.g`, et le changement est délibéré : jusqu'à `W20.f` il vérifiait que
/// `main.rs` ne contenait ni boucle ni `await`, parce que `W20.d` livrait un composition root
/// **sans surface HTTP** et qu'un serveur qui n'écoute rien se distingue mal d'un serveur en panne.
/// `W20.g` donne la surface ; la propriété d'alors n'a plus lieu d'être, et la remplacer en silence
/// aurait laissé croire qu'elle tient encore.
///
/// Ce qui reste vrai, et que ce test tient désormais : le compte rendu **précède** l'écoute, et une
/// quarantaine empêche d'ouvrir le port. Un daemon qui servirait des lectures périmées à des clients
/// qui n'ont aucun moyen de le savoir est pire qu'un daemon qui refuse.
#[test]
fn le_binaire_rend_compte_avant_de_servir_et_refuse_si_non_pret() {
    let source = include_str!("../src/main.rs");

    let compte_rendu = source
        .find("println!(\"{readiness}\")")
        .expect("il rend compte");
    let refus = source
        .find("!readiness.is_ready()")
        .expect("il vérifie sa disponibilité");
    let ecoute = source.find("TcpListener::bind").expect("il écoute");

    assert!(compte_rendu < refus, "le compte rendu précède la décision");
    assert!(
        refus < ecoute,
        "le port ne s'ouvre qu'après avoir vérifié la disponibilité"
    );
    assert!(
        source.contains("ExitCode::FAILURE"),
        "la quarantaine se voit du dehors"
    );
}
