//! Test de sortie de W6.d — **un niveau de reproductibilité se calcule ; un niveau déclaré
//! au-dessus de ce que le manifeste soutient est refusé, et le refus nomme ce qui manque.**
//!
//! Même forme que l'attestation de sandbox (W4.d.2) et que le digest de build (W5.e) : ce qui
//! atteste vient de ce qui est consigné, jamais de ce qui est demandé. Un champ qui s'auto-atteste
//! n'atteste rien.

use locus_artifacts::{Caveat, Level, Missing, RunError, RunManifest};
use locus_lep::RunManifest as WireRun;
use serde_json::Value;
use std::{fs, path::PathBuf};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

const REPRODUCIBLE: &str = "run-manifest-reproducible.json";
const NARRATION: &str = "run-manifest-narration-only.json";

fn body(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/examples")
        .join(name);
    let raw = fs::read_to_string(path).expect("fixture lisible");
    let mut value: Value = serde_json::from_str(&raw).expect("fixture en JSON valide");
    value
        .as_object_mut()
        .expect("une fixture est un objet")
        .remove("_fixture");
    value
}

fn wire(name: &str) -> WireRun {
    serde_json::from_value(body(name)).expect("la fixture se décode dans le type généré")
}

// ---------------------------------------------------------------------------------------------
// Le niveau se calcule
// ---------------------------------------------------------------------------------------------

#[test]
fn un_run_verrouille_soutient_r2() {
    let run = RunManifest::from_wire(&wire(REPRODUCIBLE)).expect("fixture conforme");
    let assessment = run.assessment();

    assert_eq!(assessment.attained, Level::R2);
    assert!(assessment.supports(Level::R1));
    assert!(assessment.supports(Level::R2));
    assert!(
        !assessment.supports(Level::R3),
        "R3 se constate en rejouant, pas en lisant"
    );
    assert!(
        assessment.caveats.is_empty(),
        "les seeds sont consignés : rien ne reste en suspens"
    );
}

#[test]
fn un_run_sans_inputs_et_sale_ne_depasse_pas_la_narration() {
    let run = RunManifest::from_wire(&wire(NARRATION)).expect("fixture conforme");
    let assessment = run.assessment();

    assert_eq!(assessment.attained, Level::R0);
    assert!(assessment.missing.contains(&Missing::Inputs));
    assert!(assessment.missing.contains(&Missing::DirtyTree));
    assert!(
        assessment.caveats.contains(&Caveat::NoSeeds),
        "aucun seed consigné : la question doit rester posée, pas disparaître"
    );
}

/// Chaque condition prise séparément fait tomber le run à R0. Le test les défait une à une plutôt
/// que d'affirmer le seuil sur un seul cas : un calcul qui ne regarderait qu'un des trois champs
/// donnerait la même réponse sur la fixture complète.
#[test]
fn chaque_condition_manquante_ramene_a_la_narration() {
    let complete = wire(REPRODUCIBLE);
    assert_eq!(
        RunManifest::from_wire(&complete)
            .expect("conforme")
            .assessment()
            .attained,
        Level::R2
    );

    let mut without_inputs = complete.clone();
    without_inputs.inputs.clear();
    without_inputs.reproducibility_level = None;
    assert_eq!(attained(&without_inputs), Level::R0);

    let mut without_revision = complete.clone();
    without_revision.code_revision = None;
    without_revision.reproducibility_level = None;
    assert_eq!(attained(&without_revision), Level::R0);

    let mut without_commit = complete.clone();
    without_commit
        .code_revision
        .as_mut()
        .expect("une révision")
        .commit = None;
    without_commit.reproducibility_level = None;
    assert_eq!(
        attained(&without_commit),
        Level::R0,
        "une révision sans commit n'identifie pas le code"
    );

    let mut dirty = complete;
    dirty.code_revision.as_mut().expect("une révision").dirty = Some(true);
    dirty.reproducibility_level = None;
    assert_eq!(attained(&dirty), Level::R0);
}

fn attained(document: &WireRun) -> Level {
    RunManifest::from_wire(document)
        .expect("conforme")
        .assessment()
        .attained
}

// ---------------------------------------------------------------------------------------------
// Un niveau déclaré au-dessus est refusé
// ---------------------------------------------------------------------------------------------

#[test]
fn declarer_plus_haut_que_ce_qu_on_soutient_est_refuse() {
    let mut document = wire(NARRATION);
    document.reproducibility_level = Some("R2".to_owned());

    match RunManifest::from_wire(&document) {
        Err(RunError::LevelNotSupported {
            claimed,
            attained,
            missing,
        }) => {
            assert_eq!(claimed, Level::R2);
            assert_eq!(attained, Level::R0);
            assert!(missing.contains(&Missing::Inputs));
            assert!(missing.contains(&Missing::DirtyTree));
        }
        other => panic!("un niveau non soutenu doit être refusé, et nommer pourquoi : {other:?}"),
    }
}

/// Le refus qui porte la décision du sprint. R3 est « reproduction automatisée sur backend
/// compatible » et R4 « reproduction indépendante sur worker distinct » : ce sont des
/// **événements**, pas des propriétés d'un document. Un manifeste qui les déclare décrit quelque
/// chose dont il n'a pas la trace, et aucun champ à remplir ne remplace un rejeu.
#[test]
fn aucun_manifeste_seul_ne_declare_r3_ni_r4() {
    for level in ["R3", "R4"] {
        let mut document = wire(REPRODUCIBLE);
        document.reproducibility_level = Some(level.to_owned());
        match RunManifest::from_wire(&document) {
            Err(RunError::LevelNotSupported {
                attained, missing, ..
            }) => {
                assert_eq!(attained, Level::FROM_A_MANIFEST_ALONE);
                assert!(
                    missing.contains(&Missing::ReproductionNotEvidenced),
                    "le refus doit dire que c'est la reproduction qui manque, pas un champ"
                );
            }
            other => panic!("« {level} » ne se lit pas dans un manifeste : {other:?}"),
        }
    }
}

/// Même un run parfaitement verrouillé garde `ReproductionNotEvidenced` dans ce qui lui manque :
/// R2 est le plafond d'un document, et ce qui sépare de R3 doit rester visible plutôt que de
/// disparaître parce que le reste est en ordre.
#[test]
fn le_plafond_d_un_document_reste_nomme_meme_au_mieux() {
    let run = RunManifest::from_wire(&wire(REPRODUCIBLE)).expect("fixture conforme");
    assert_eq!(
        run.assessment().missing,
        vec![Missing::ReproductionNotEvidenced]
    );
    assert_eq!(Level::FROM_A_MANIFEST_ALONE, Level::R2);
}

#[test]
fn un_niveau_inconnu_ne_devient_pas_un_niveau_par_defaut() {
    let mut document = wire(REPRODUCIBLE);
    document.reproducibility_level = Some("R5".to_owned());
    assert_eq!(
        RunManifest::from_wire(&document),
        Err(RunError::UnknownLevel {
            value: "R5".to_owned()
        })
    );

    assert_eq!(Level::parse("R4"), Some(Level::R4));
    assert_eq!(Level::parse("r2"), None, "la casse n'est pas normalisée");
}

#[test]
fn les_niveaux_sont_ordonnes() {
    assert!(Level::R0 < Level::R1);
    assert!(Level::R2 < Level::R3);
    assert_eq!(
        Level::ALL.into_iter().map(Level::slug).collect::<Vec<_>>(),
        vec!["R0", "R1", "R2", "R3", "R4"]
    );
}

// ---------------------------------------------------------------------------------------------
// Ce qu'un run ne peut pas être
// ---------------------------------------------------------------------------------------------

#[test]
fn un_run_sans_commande_ne_consigne_rien_a_rejouer() {
    let mut document = wire(REPRODUCIBLE);
    document.commands.clear();
    assert_eq!(RunManifest::from_wire(&document), Err(RunError::NoCommands));

    let mut empty_argv = wire(REPRODUCIBLE);
    empty_argv.commands[0].argv.clear();
    assert_eq!(
        RunManifest::from_wire(&empty_argv),
        Err(RunError::EmptyArgv)
    );
}

#[test]
fn un_run_ne_finit_pas_avant_de_commencer() {
    let mut document = wire(REPRODUCIBLE);
    document.completed_at = Some("2026-08-17T11:18:00.000Z".to_owned());
    assert_eq!(
        RunManifest::from_wire(&document),
        Err(RunError::EndsBeforeItStarts)
    );
}

#[test]
fn un_horodatage_non_canonique_est_refuse() {
    let mut document = wire(REPRODUCIBLE);
    document.started_at = "2026-08-17T11:18:55Z".to_owned();
    assert_eq!(
        RunManifest::from_wire(&document),
        Err(RunError::MalformedTimestamp {
            value: "2026-08-17T11:18:55Z".to_owned()
        })
    );
}

#[test]
fn un_hash_d_input_mal_forme_est_refuse() {
    let mut document = wire(REPRODUCIBLE);
    document.inputs[0].content_hash = "md5:0123456789abcdef".to_owned();
    assert!(matches!(
        RunManifest::from_wire(&document),
        Err(RunError::MalformedHash { .. })
    ));
}

#[test]
fn un_attempt_zero_ne_designe_aucune_execution() {
    let mut document = wire(REPRODUCIBLE);
    document.attempt = 0;
    assert_eq!(
        RunManifest::from_wire(&document),
        Err(RunError::ImpossibleAttempt { value: 0 })
    );
}

// ---------------------------------------------------------------------------------------------
// Le document traverse
// ---------------------------------------------------------------------------------------------

/// # Pourquoi la comparaison porte sur les documents décodés
///
/// Pas sur leur écriture. `cpu` est un `number` au schéma — pour les cœurs fractionnaires — donc
/// un `4` lu ressort en `4.0`, et JSON ne distingue pas les deux : aucun lecteur conforme ne
/// rapporte lequel a été écrit. `packages/lep/tests/round_trip.rs` a rencontré le même fait, et en
/// a tiré la conséquence qui compte : les octets à hasher viennent d'un canonicaliseur, jamais de
/// la sortie d'un sérialiseur. Comparer ici les textes reviendrait à tester le sérialiseur.
#[test]
fn un_run_relu_se_reecrit_a_l_identique() {
    let run = RunManifest::from_wire(&wire(REPRODUCIBLE)).expect("fixture conforme");
    assert_eq!(
        run.to_wire(),
        &wire(REPRODUCIBLE),
        "un lecteur validant ne modélise pas à côté du schéma : il tient le document"
    );
    let rewritten: WireRun =
        serde_json::from_value(serde_json::to_value(run.to_wire()).expect("ré-encodage"))
            .expect("ce qui sort se relit");
    assert_eq!(
        &rewritten,
        run.to_wire(),
        "aucun champ ne se perd à l'aller-retour"
    );
    assert_eq!(run.run_id(), "run-2026-08-17-0007");
    assert_eq!(
        run.image_digest(),
        "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "un run dont l'image n'est pas identifiée par digest ne se rejoue pas"
    );
    assert!(run.completed_at() > Some(run.started_at()));
}
