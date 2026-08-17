//! Second volet du test de sortie de W6.e — **le `ReproductionWorkflow` de §11.2 porte le verdict
//! jusqu'au bout, divergence comprise.**
//!
//! `packages/artifacts/tests/reproduction.rs` prouve que la comparaison est juste. Celui-ci prouve
//! qu'un rejeu **divergent** traverse le workflow comme n'importe quel autre : l'exécution se
//! termine, le verdict est consigné, et rien n'est remonté comme incident. C'est la forme
//! exécutable de l'invariant 12 — un résultat négatif qui ferait échouer son propre workflow
//! disparaîtrait du graphe en ressemblant à une panne.
//!
//! Le test vit ici et non dans `packages/artifacts` parce que la dépendance va de l'extérieur vers
//! l'intérieur : un moteur de workflow peut connaître les artefacts, l'inverse serait une
//! infrastructure entrée dans le vocabulaire.

use locus_artifacts::{Comparison, Independence, Level, RunManifest, Verdict, compare};
use locus_domain::StableId;
use locus_lep::RunManifest as WireRun;
use locus_protocol::Timestamp;
use locus_workflow::{
    CATALOG_VERSION, Step, WorkflowBackend, WorkflowDefinition, WorkflowKind, WorkflowState,
    catalog,
};
use locus_workflow_backends::{DeterministicBackend, HistoryEvent, block_on, replay};
use serde_json::Value;
use std::{fs, path::PathBuf};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn subject() -> Vec<StableId> {
    let mut entropy = [0_u8; 10];
    entropy[9] = 6;
    vec![
        StableId::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
            .expect("l'instant de fixture tient sur 48 bits"),
    ]
}

fn wire() -> WireRun {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/examples/run-manifest-reproducible.json");
    let raw = fs::read_to_string(path).expect("fixture lisible");
    let mut value: Value = serde_json::from_str(&raw).expect("fixture en JSON valide");
    value
        .as_object_mut()
        .expect("une fixture est un objet")
        .remove("_fixture");
    serde_json::from_value(value).expect("la fixture se décode dans le type généré")
}

fn run(document: &WireRun) -> RunManifest {
    RunManifest::from_wire(document).expect("run conforme")
}

/// Un rejeu du même run, dont une sortie ne retrouve pas le même contenu.
fn diverging_replay(original: &WireRun) -> WireRun {
    let mut replay = original.clone();
    replay.run_id = String::from("run-2026-08-18-0001");
    replay.started_at = String::from("2026-08-18T08:00:00.000Z");
    replay.completed_at = Some("2026-08-18T08:00:51.000Z".to_owned());
    replay.outputs.as_mut().expect("des sorties")[0].content_hash =
        String::from("sha256:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210");
    replay
}

fn definition() -> WorkflowDefinition {
    catalog::definition(WorkflowKind::Reproduction, CATALOG_VERSION, subject())
        .expect("le catalogue définit la reproduction")
}

/// Un moteur où chaque activity a un exécutant, et où le verdict est **le vrai verdict**.
///
/// C'est ce qui distingue ce test d'une mise en scène : le résultat consigné par
/// `record_reproduction_verdict` n'est pas une chaîne choisie pour le test, c'est ce que
/// `compare` a rendu.
fn staffed_with(verdict: &Comparison) -> DeterministicBackend {
    let mut backend = DeterministicBackend::new();
    for activity in definition().activities() {
        let result = if activity.name() == "record_reproduction_verdict" {
            verdict.to_string()
        } else {
            format!("result:{}", activity.name())
        };
        backend.register_activity(activity.name(), &result);
    }
    backend
}

// ---------------------------------------------------------------------------------------------
// Une divergence traverse le workflow
// ---------------------------------------------------------------------------------------------

#[test]
fn un_rejeu_divergent_termine_le_workflow_et_consigne_son_verdict() {
    let original = wire();
    let verdict = compare(
        &run(&original),
        &run(&diverging_replay(&original)),
        Independence::DistinctWorker {
            original: "canterel-vm-linux-01".to_owned(),
            replay: "canterel-vm-linux-02".to_owned(),
        },
    )
    .expect("un rejeu divergent reste un rejeu du même run");
    assert!(matches!(verdict.verdict, Verdict::Diverged { .. }));

    let definition = definition();
    let mut backend = staffed_with(&verdict);
    let handle = block_on(backend.start(&definition)).expect("le workflow démarre");
    backend.run(&handle.id).expect("le workflow se déroule");

    assert_eq!(
        block_on(backend.inspect(&handle.id)).expect("un état"),
        WorkflowState::Completed,
        "une divergence n'est pas une panne : le workflow va jusqu'au bout"
    );

    let history = backend.history(&handle.id).expect("une histoire");
    let recorded = history
        .iter()
        .find_map(|event| match event {
            HistoryEvent::ActivityCompleted { name, result, .. }
                if name == "record_reproduction_verdict" =>
            {
                Some(result.clone())
            }
            _ => None,
        })
        .expect("le verdict est consigné");

    assert!(recorded.starts_with("divergent"), "{recorded}");
    assert!(
        recorded.contains("artifact-figure-3"),
        "le verdict consigné nomme ce qui a divergé : {recorded}"
    );
}

#[test]
fn le_verdict_consigne_survit_au_rejeu_de_l_historique() {
    let original = wire();
    let verdict = compare(
        &run(&original),
        &run(&diverging_replay(&original)),
        Independence::Unknown,
    )
    .expect("même run");

    let definition = definition();
    let mut backend = staffed_with(&verdict);
    let handle = block_on(backend.start(&definition)).expect("le workflow démarre");
    backend.run(&handle.id).expect("le workflow se déroule");
    let history = backend.history(&handle.id).expect("une histoire").to_vec();

    let replayed = replay(&definition, &history).expect("l'historique se rejoue");
    assert!(
        replayed
            .activity_results
            .iter()
            .any(|(_, result)| result.starts_with("divergent")),
        "le rejeu doit retrouver le verdict, pas le recalculer : {:?}",
        replayed.activity_results
    );
}

// ---------------------------------------------------------------------------------------------
// Le workflow reste celui de §11.2
// ---------------------------------------------------------------------------------------------

/// La comparaison est **déterministe** : elle ne lit ni horloge, ni réseau, ni disque. C'est ce qui
/// permet qu'elle vive au pas `compare_outputs`, qui n'a le droit à aucun effet. Un jour où elle
/// irait rechercher un artefact pour le rehasher, ce pas devrait devenir une activity — et ce test
/// est ce qui obligerait à s'en apercevoir.
#[test]
fn la_comparaison_vit_a_un_pas_sans_effet() {
    let definition = definition();
    let step = definition
        .steps()
        .iter()
        .find(|step| step.name() == "compare_outputs")
        .expect("§11.2 : la reproduction compare ses sorties");
    assert!(
        matches!(step, Step::Deterministic { .. }),
        "comparer deux manifestes ne produit aucun effet : ce pas est déterministe"
    );
}

#[test]
fn un_rejeu_conforme_consigne_le_niveau_qu_il_etablit() {
    let original = wire();
    let mut replayed = original.clone();
    replayed.run_id = String::from("run-2026-08-18-0002");
    replayed.started_at = String::from("2026-08-18T09:00:00.000Z");
    replayed.completed_at = Some("2026-08-18T09:00:44.000Z".to_owned());

    let verdict = compare(
        &run(&original),
        &run(&replayed),
        Independence::DistinctWorker {
            original: "canterel-vm-linux-01".to_owned(),
            replay: "canterel-vm-linux-02".to_owned(),
        },
    )
    .expect("même run");
    assert_eq!(verdict.attained, Level::R4);

    let definition = definition();
    let mut backend = staffed_with(&verdict);
    let handle = block_on(backend.start(&definition)).expect("le workflow démarre");
    backend.run(&handle.id).expect("le workflow se déroule");
    let history = backend.history(&handle.id).expect("une histoire");

    assert!(
        history.iter().any(|event| matches!(
            event,
            HistoryEvent::ActivityCompleted { result, .. } if result == "reproduit, R4"
        )),
        "R4 n'existe que consigné par une reproduction : {history:#?}"
    );
}
