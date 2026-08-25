//! La projection du graphe épistémique, éprouvée au grain du port — `W20.u`, §9.5.
//!
//! `apps/locusd/tests/epistemic_graph.rs` tient les six termes de §9.4 **sur le fil**. Ce
//! fichier-ci tient ce que le fil ne montre pas : la destruction, la reconstruction, et les refus
//! qui mettent en quarantaine. §9.5 en fait des propriétés du port, pas de la route.

use locus_domain::RevisionKind;
use locus_event_store::{
    Actor, ActorKind, Append, Draft, Envelope, EventStore, EventType, Expected, MemoryEventStore,
};
use locus_projections::{EpistemicGraph, Projection};
use locus_protocol::{
    Id, IdKind, Timestamp,
    id::{Agent, Command, Event, Project, Workspace},
};

const NOW: Timestamp = Timestamp::from_millis(1_700_000_000_000);
const TACHE: &str = "tsk_catalyseur";

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(NOW, entropy).expect("l'instant de fixture tient sur 48 bits")
}

fn revision(seed: u8) -> Id<RevisionKind> {
    id::<RevisionKind>(seed)
}

/// Une enveloppe telle que le journal la rend — écrite puis relue.
///
/// Fabriquer une `Envelope` à la main serait plus court et éprouverait la projection sur une forme
/// que le journal ne produit pas. Elle passe donc par un `MemoryEventStore`, comme les faits réels.
fn enveloppe(event_type: &str, payload: serde_json::Value) -> Envelope {
    let stream = format!("task/{TACHE}");
    let store = MemoryEventStore::new();
    store
        .append(
            Append {
                stream_id: stream.clone(),
                expected: Expected::NoStream,
                command_id: id::<Command>(1),
                events: vec![Draft {
                    event_id: id::<Event>(1),
                    event_type: EventType::parse(event_type).expect("type de §10.3"),
                    schema_version: 1,
                    stream_id: stream.clone(),
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
                    idempotency_key: None,
                    correlation_id: None,
                    trace_id: None,
                    payload,
                    payload_hash: format!("sha256:{}", "ab".repeat(32)),
                }],
            },
            NOW,
        )
        .expect("écriture permise");
    store
        .read_stream(&stream, 0)
        .into_iter()
        .next()
        .expect("le fait vient d'être écrit")
}

fn commit_nominal() -> serde_json::Value {
    serde_json::json!({
        "task_id": TACHE,
        "commit": {
            "attempt": 2,
            "inferences": [{
                "rule": "voie cinétique",
                "premise_refs": [revision(11).to_string()],
                "conclusion_refs": [revision(20).to_string()],
            }],
            "objections": [{ "statement": "je conteste", "targets": [] }],
        },
    })
}

/// **Détruire une projection la ramène à zéro — vraiment.**
///
/// §9.5 : « une projection peut être détruite et reconstruite ». Ce n'est pas une facilité
/// d'opérateur, c'est la propriété qui la rend **secondaire** : une projection qu'on ne saurait pas
/// détruire serait une seconde source de vérité. Un `reset` qui ne remettrait que le watermark
/// laisserait un état que le journal ne produit plus, et la reconstruction le doublerait — chaque
/// inférence deux fois, chaque objection deux fois.
#[test]
fn detruire_ramene_vraiment_a_zero() {
    let mut graph = EpistemicGraph::new();
    graph
        .apply(7, &enveloppe("epistemic_object.staged", commit_nominal()))
        .expect("le commit entre");
    assert_eq!(graph.inference_count(), 1);
    assert_eq!(graph.objections().len(), 1);
    let plein = graph.checksum();

    graph.reset();

    assert_eq!(graph.watermark(), 0);
    assert_eq!(graph.inference_count(), 0, "les inférences sont parties");
    assert_eq!(graph.objections().len(), 0, "les objections aussi");
    assert_ne!(
        graph.checksum(),
        plein,
        "le résumé distingue le plein du vide : sans quoi `verify` comparerait deux chaînes égales \
         et déclarerait conforme une projection qui a tout gardé"
    );
    assert_eq!(graph.checksum(), EpistemicGraph::new().checksum());
}

/// **Le résumé distingue chacune des six choses qu'il compte.**
///
/// Un checksum qui figerait l'un de ses termes laisserait `verify` déclarer conforme une
/// reconstruction qui a perdu exactement ce terme-là — et c'est le genre de perte qu'on ne
/// découvre qu'en interrogeant le graphe six mois plus tard.
#[test]
fn le_resume_distingue_ce_qu_il_compte() {
    let mut avec_inference = EpistemicGraph::new();
    avec_inference
        .apply(
            1,
            &enveloppe(
                "epistemic_object.staged",
                serde_json::json!({
                    "task_id": TACHE,
                    "commit": {
                        "attempt": 1,
                        "inferences": [{
                            "rule": "r",
                            "premise_refs": [revision(11).to_string()],
                            "conclusion_refs": [revision(20).to_string()],
                        }],
                    },
                }),
            ),
        )
        .expect("entre");

    let mut avec_illisible = EpistemicGraph::new();
    avec_illisible
        .apply(
            1,
            &enveloppe(
                "epistemic_object.staged",
                serde_json::json!({
                    "task_id": TACHE,
                    "commit": {
                        "attempt": 1,
                        "inferences": [{
                            "rule": "r",
                            "premise_refs": ["écrit-à-la-main"],
                            "conclusion_refs": [revision(20).to_string()],
                        }],
                    },
                }),
            ),
        )
        .expect("entre");

    let mut avec_objection = EpistemicGraph::new();
    avec_objection
        .apply(
            1,
            &enveloppe(
                "epistemic_object.staged",
                serde_json::json!({
                    "task_id": TACHE,
                    "commit": { "attempt": 1, "objections": [{ "statement": "s" }] },
                }),
            ),
        )
        .expect("entre");

    let mut avec_artefact = EpistemicGraph::new();
    avec_artefact
        .apply(
            1,
            &enveloppe(
                "artifact.declared",
                serde_json::json!({
                    "artifact_id": "a",
                    "produced_by": { "task_id": TACHE },
                }),
            ),
        )
        .expect("entre");

    let mut avec_cout = EpistemicGraph::new();
    avec_cout
        .apply(
            1,
            &enveloppe(
                "budget.consumed",
                serde_json::json!({ "task_id": TACHE, "amounts": { "tokens": 1 } }),
            ),
        )
        .expect("entre");

    let resumes = [
        EpistemicGraph::new().checksum(),
        avec_inference.checksum(),
        avec_illisible.checksum(),
        avec_objection.checksum(),
        avec_artefact.checksum(),
        avec_cout.checksum(),
    ];
    let mut uniques = resumes.to_vec();
    uniques.sort_unstable();
    uniques.dedup();
    assert_eq!(
        uniques.len(),
        resumes.len(),
        "six états distincts, six résumés distincts : {resumes:?}"
    );
}

/// **Une objection sans énoncé met la projection en quarantaine.**
///
/// L'invariant 12 veut qu'une objection soit du contenu de premier plan. Une objection vide n'en
/// est pas un : elle occuperait une ligne dans le dossier d'une conclusion sans rien dire, et un
/// lecteur croirait la conclusion contestée sans pouvoir savoir de quoi.
#[test]
fn une_objection_sans_enonce_est_refusee() {
    let mut graph = EpistemicGraph::new();
    let refus = graph
        .apply(
            3,
            &enveloppe(
                "epistemic_object.staged",
                serde_json::json!({
                    "task_id": TACHE,
                    "commit": { "attempt": 1, "objections": [{ "targets": ["x"] }] },
                }),
            ),
        )
        .expect_err("une objection muette n'est pas une objection");
    assert_eq!(refus.position, 3, "le refus dit où reprendre");
}

/// **Une consommation sans montants est refusée, et une consommation sans tâche aussi.**
///
/// Un coût sans dimension n'est pas un coût ; une consommation qu'on ne peut imputer à aucune
/// expérience ne se rattache à aucune conclusion. Les accepter en silence produirait un `entries`
/// qui augmente sans qu'aucune somme bouge — un registre qui compte des écritures vides.
#[test]
fn une_consommation_incomplete_est_refusee() {
    for charge in [
        serde_json::json!({ "task_id": TACHE }),
        serde_json::json!({ "amounts": { "tokens": 1 } }),
    ] {
        let mut graph = EpistemicGraph::new();
        graph
            .apply(5, &enveloppe("budget.consumed", charge))
            .expect_err("une consommation incomplète n'entre pas");
    }
}

/// **Une consommation négative est refusée plutôt que tronquée.**
///
/// §7.2 : « une correction ne réécrit pas une écriture antérieure ; elle crée un ajustement
/// compensatoire ». Une consommation négative est donc une écriture qui n'existe pas dans le
/// vocabulaire du budget, et l'accepter en la lisant comme zéro ferait disparaître une correction
/// que quelqu'un a voulu écrire.
#[test]
fn une_consommation_negative_est_refusee() {
    let mut graph = EpistemicGraph::new();
    graph
        .apply(
            5,
            &enveloppe(
                "budget.consumed",
                serde_json::json!({ "task_id": TACHE, "amounts": { "tokens": -3 } }),
            ),
        )
        .expect_err("le registre ne consomme pas négativement");
}

/// **Un fait épistémique sans commit n'est pas une faute — il n'apporte simplement rien.**
///
/// `W20.r` en écrit pour des actes d'autorité qui ne portent aucun raisonnement. Les refuser
/// mettrait la projection en quarantaine sur un fait parfaitement légitime, et un daemon en
/// quarantaine ne redémarre pas — c'est la leçon que `W20.r` a payée.
#[test]
fn un_fait_sans_commit_traverse_sans_bruit() {
    let mut graph = EpistemicGraph::new();
    graph
        .apply(
            9,
            &enveloppe(
                "epistemic_object.validated",
                serde_json::json!({ "task_id": TACHE, "status": "validated" }),
            ),
        )
        .expect("un acte d'autorité n'est pas un raisonnement mal formé");
    assert_eq!(graph.inference_count(), 0);
    assert_eq!(graph.watermark(), 9, "il a tout de même été vu");
}
