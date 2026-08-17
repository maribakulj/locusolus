//! Test de sortie de W6.e — **rejouer un `RunManifest` et retrouver les mêmes hashes établit
//! `R3`/`R4` ; une divergence est un résultat rendu, pas une erreur avalée.**
//!
//! W6.d refuse `R3` et `R4` à tout manifeste seul, parce que ce sont des événements. Ce test est
//! l'événement.

use locus_artifacts::{
    Comparison, Divergence, Independence, Level, Mismatch, RunManifest, Verdict, compare,
};
use locus_lep::RunManifest as WireRun;
use serde_json::Value;
use std::{fs, path::PathBuf};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

const ORIGINAL: &str = "run-manifest-reproducible.json";
const FIGURE: &str = "artifact-figure-3";

fn wire() -> WireRun {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/examples")
        .join(ORIGINAL);
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

/// Le rejeu du même run : un autre identifiant, un autre moment, tout le reste identique.
fn replay_of(original: &WireRun) -> WireRun {
    let mut replay = original.clone();
    replay.run_id = String::from("run-2026-08-18-0001");
    replay.started_at = String::from("2026-08-18T08:00:00.000Z");
    replay.completed_at = Some("2026-08-18T08:00:51.000Z".to_owned());
    replay
}

fn distinct_workers() -> Independence {
    Independence::DistinctWorker {
        original: "canterel-vm-linux-01".to_owned(),
        replay: "canterel-vm-linux-02".to_owned(),
    }
}

// ---------------------------------------------------------------------------------------------
// Ce qu'une reproduction établit
// ---------------------------------------------------------------------------------------------

#[test]
fn un_rejeu_identique_sur_un_worker_distinct_etablit_r4() {
    let original = wire();
    let comparison = compare(
        &run(&original),
        &run(&replay_of(&original)),
        distinct_workers(),
    )
    .expect("le rejeu exécute bien le même run");

    assert!(comparison.is_reproduced());
    assert_eq!(comparison.attained, Level::R4);
}

#[test]
fn un_rejeu_identique_sur_le_meme_worker_etablit_r3() {
    let original = wire();
    let comparison = compare(
        &run(&original),
        &run(&replay_of(&original)),
        Independence::SameWorker,
    )
    .expect("même run");
    assert_eq!(comparison.attained, Level::R3);
}

/// L'absence de preuve n'est pas une preuve : rien dans un `RunManifest` ne nomme le worker, donc
/// un plan de contrôle qui ne le dit pas laisse la reproduction à `R3`. Monter à `R4` faute
/// d'information reviendrait à conclure d'un silence.
#[test]
fn une_independance_inconnue_plafonne_a_r3() {
    let original = wire();
    let comparison = compare(
        &run(&original),
        &run(&replay_of(&original)),
        Independence::Unknown,
    )
    .expect("même run");
    assert_eq!(comparison.attained, Level::R3);
    assert_eq!(comparison.independence, Independence::Unknown);
}

// ---------------------------------------------------------------------------------------------
// Une divergence est un résultat
// ---------------------------------------------------------------------------------------------

/// Le cœur du sprint. Un rejeu qui ne retrouve pas les mêmes sorties est une **information
/// scientifique** — souvent la plus intéressante des deux. `compare` rend donc un verdict, pas une
/// erreur : la traiter en panne la ferait remonter comme un incident technique, c'est-à-dire
/// disparaître. Invariant 12.
#[test]
fn un_contenu_different_est_rendu_comme_verdict_et_non_comme_erreur() {
    let original = wire();
    let mut diverging = replay_of(&original);
    diverging.outputs.as_mut().expect("des sorties")[0].content_hash =
        "sha256:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210".to_owned();

    let comparison = compare(&run(&original), &run(&diverging), distinct_workers())
        .expect("un rejeu divergent reste un rejeu du même run");

    assert!(!comparison.is_reproduced());
    let Verdict::Diverged { divergences } = &comparison.verdict else {
        panic!("une divergence doit être une valeur : {comparison:?}")
    };
    assert_eq!(divergences.len(), 1);
    assert!(matches!(
        &divergences[0],
        Divergence::ContentChanged { artifact_id, .. } if artifact_id == FIGURE
    ));
    assert!(
        comparison.to_string().contains(FIGURE),
        "le verdict nomme ce qui a divergé : {comparison}"
    );
}

/// Le niveau ne descend pas et ne monte pas. La reproduction a eu lieu — elle n'a rien établi de
/// plus que ce que le manifeste soutenait, et un rejeu raté ne défait pas un environnement
/// verrouillé.
#[test]
fn une_divergence_laisse_le_niveau_ou_le_manifeste_l_avait_laisse() {
    let original = wire();
    let mut diverging = replay_of(&original);
    diverging.outputs.as_mut().expect("des sorties").clear();

    let comparison =
        compare(&run(&original), &run(&diverging), distinct_workers()).expect("même run");
    assert_eq!(comparison.attained, Level::R2);
    assert_eq!(Level::R2, Level::FROM_A_MANIFEST_ALONE);
}

#[test]
fn une_sortie_manquante_et_une_sortie_en_trop_sont_deux_divergences() {
    let original = wire();
    let mut diverging = replay_of(&original);
    let outputs = diverging.outputs.as_mut().expect("des sorties");
    outputs[0].artifact_id = String::from("artifact-figure-3-bis");

    let comparison =
        compare(&run(&original), &run(&diverging), Independence::Unknown).expect("même run");
    let Verdict::Diverged { divergences } = &comparison.verdict else {
        panic!("divergence attendue")
    };

    assert!(divergences.contains(&Divergence::Missing {
        artifact_id: FIGURE.to_owned()
    }));
    assert!(
        divergences.contains(&Divergence::Unexpected {
            artifact_id: "artifact-figure-3-bis".to_owned()
        }),
        "une sortie de plus est une divergence : l'ignorer laisserait passer un run qui produit \
         silencieusement autre chose en plus"
    );
}

// ---------------------------------------------------------------------------------------------
// Ce qui n'est pas une reproduction
// ---------------------------------------------------------------------------------------------

/// La distinction que le sprint tient : « les sorties diffèrent » ne dit rien sur la
/// reproductibilité du premier run si le second n'exécutait pas la même chose. Là, il n'y a pas de
/// verdict à rendre — dans un sens comme dans l'autre — et c'est une erreur.
#[test]
fn un_rejeu_sur_une_autre_image_n_est_pas_une_reproduction() {
    let original = wire();
    let mut elsewhere = replay_of(&original);
    elsewhere.environment.image_digest =
        "sha256:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210".to_owned();

    let refused = compare(&run(&original), &run(&elsewhere), distinct_workers())
        .expect_err("une autre image n'est pas le même run");
    assert!(matches!(
        refused.differences.as_slice(),
        [Mismatch::ImageDigest { .. }]
    ));
}

#[test]
fn un_rejeu_sur_d_autres_inputs_ou_d_autres_commandes_n_est_pas_une_reproduction() {
    let original = wire();

    // Le niveau déclaré tombe avec les inputs : W6.d refuse un `R2` qu'un run sans input ne
    // soutient plus, et un rejeu qui garderait la déclaration de l'original serait invalide bien
    // avant d'être comparé. Les deux sprints se tiennent, et le test doit rester honnête.
    let mut other_inputs = replay_of(&original);
    other_inputs.inputs.clear();
    other_inputs.reproducibility_level = None;
    let refused = compare(&run(&original), &run(&other_inputs), Independence::Unknown)
        .expect_err("d'autres inputs");
    assert!(refused.differences.contains(&Mismatch::Inputs));

    let mut other_commands = replay_of(&original);
    other_commands.commands[0].argv = vec!["python".to_owned(), "-c".to_owned(), "pass".to_owned()];
    let refused = compare(
        &run(&original),
        &run(&other_commands),
        Independence::Unknown,
    )
    .expect_err("d'autres commandes");
    assert!(refused.differences.contains(&Mismatch::Commands));
}

/// L'ordre des inputs n'en fait pas d'autres inputs — deux runs qui consomment les mêmes contenus
/// consomment les mêmes contenus. L'ordre des **commandes**, lui, compte : `train` puis `evaluate`
/// n'est pas `evaluate` puis `train`.
#[test]
fn l_ordre_compte_pour_les_commandes_et_pas_pour_les_inputs() {
    let mut original = wire();
    let second_input = locus_lep::RunManifestInputsItem {
        artifact_id: Some("artifact-config".to_owned()),
        content_hash: "sha512:abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".to_owned(),
        role: Some("config".to_owned()),
    };
    original.inputs.push(second_input);

    let mut reordered = replay_of(&original);
    reordered.inputs.reverse();
    let comparison =
        compare(&run(&original), &run(&reordered), Independence::Unknown).expect("mêmes contenus");
    assert!(comparison.is_reproduced());

    let mut second_command = original.commands[0].clone();
    second_command.argv = vec![
        "python".to_owned(),
        "-m".to_owned(),
        "analysis.check".to_owned(),
    ];
    original.commands.push(second_command);

    let mut swapped = replay_of(&original);
    swapped.commands.reverse();
    let refused =
        compare(&run(&original), &run(&swapped), Independence::Unknown).expect_err("autre ordre");
    assert!(refused.differences.contains(&Mismatch::Commands));
}

#[test]
fn le_refus_nomme_ce_qui_differe() {
    let original = wire();
    let mut everything = replay_of(&original);
    everything.environment.image_digest =
        "sha256:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210".to_owned();
    everything.inputs.clear();
    everything.reproducibility_level = None;

    let refused = compare(&run(&original), &run(&everything), Independence::Unknown)
        .expect_err("deux écarts");
    assert_eq!(refused.differences.len(), 2);
    let message = refused.to_string();
    assert!(message.contains("l'image diffère"), "{message}");
    assert!(message.contains("inputs"), "{message}");
}

// ---------------------------------------------------------------------------------------------
// Le verdict est une donnée
// ---------------------------------------------------------------------------------------------

/// Un `Comparison` se transporte, se compare et s'écrit. C'est ce qui permet à l'étape
/// `record_reproduction_verdict` du `ReproductionWorkflow` de le consigner tel quel plutôt que de
/// le reconstituer — et à un rejeu divergent d'exister dans le graphe au lieu d'y manquer.
#[test]
fn un_verdict_se_transporte_tel_quel() {
    let original = wire();
    let comparison: Comparison = compare(
        &run(&original),
        &run(&replay_of(&original)),
        distinct_workers(),
    )
    .expect("même run");

    let carried = comparison.clone();
    assert_eq!(carried, comparison);
    assert_eq!(carried.to_string(), "reproduit, R4");
}
