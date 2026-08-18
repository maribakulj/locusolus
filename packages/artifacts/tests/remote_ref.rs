//! Test de sortie de W6.f — **le snapshot prouve, la ressource live ne prouve rien, et une
//! divergence entre les deux ne rend jamais la preuve historique douteuse.**
//!
//! `xiiif/SPEC_V1.md` §19 : « une ressource distante modifiée après le run ne doit **jamais** faire
//! croire que la preuve historique a changé. Le snapshot/hash reste la référence de reproduction ;
//! la ressource live sert à constater l'évolution. »
//!
//! Deux verdicts, donc, et le test les tient séparés de bout en bout : c'est la confusion qui est
//! la faute, pas l'un ou l'autre des constats.

use locus_artifacts::{Drift, Locator, Observed, RemoteArtifactRef, RemoteRefError, Standing};
use locus_domain::ContentHash;

fn hash(byte: &str) -> ContentHash {
    ContentHash::parse(&format!("sha256:{}", byte.repeat(32))).expect("hash bien formé")
}

fn observed(live_at_run: Option<&str>) -> Observed {
    Observed {
        snapshot_hash: hash("ab"),
        live_hash_at_run: live_at_run.map(hash),
        captured_at: None,
    }
}

fn reference(observed: Observed) -> RemoteArtifactRef {
    RemoteArtifactRef::new(
        "art-0001",
        "image/jpeg",
        observed,
        Locator::ManifestUrl("https://gallica.example/iiif/manifest".to_owned()),
    )
    .expect("référence valide")
}

// ---------------------------------------------------------------------------------------------
// Le snapshot prouve
// ---------------------------------------------------------------------------------------------

#[test]
fn le_snapshot_est_ce_qui_prouve_la_reproduction() {
    let reference = reference(observed(None));
    assert_eq!(reference.proof_standing(&hash("ab")), Standing::Holds);
    assert_eq!(reference.proof_standing(&hash("cd")), Standing::Broken);
}

/// Le cœur de §19. La source a bougé — c'est un fait — et la preuve tient quand même, parce que ce
/// qui la porte est l'instantané. Confondre les deux ferait douter d'un travail correct chaque fois
/// qu'une bibliothèque remanie son site.
#[test]
fn une_source_qui_a_bouge_ne_casse_pas_la_preuve() {
    let reference = reference(observed(Some("11")));

    assert_eq!(
        reference.live_drift(&hash("99")),
        Drift::Moved,
        "la ressource live a changé depuis le run"
    );
    assert_eq!(
        reference.proof_standing(&hash("ab")),
        Standing::Holds,
        "et la preuve historique tient : c'est le snapshot qui la porte"
    );
}

/// Et l'inverse, qui est le cas grave et rare : la source n'a pas bougé, mais la reproduction ne
/// retrouve pas l'instantané. Là, c'est la preuve qui est en cause.
#[test]
fn une_source_intacte_ne_sauve_pas_une_preuve_cassee() {
    let reference = reference(observed(Some("11")));

    assert_eq!(reference.live_drift(&hash("11")), Drift::Unchanged);
    assert_eq!(reference.proof_standing(&hash("cd")), Standing::Broken);
}

/// Les deux verdicts n'ont aucun accesseur qui les résumerait. Un « intégrité : divergente » unique
/// laisserait croire que le résultat scientifique est en cause quand c'est la source qui a bougé —
/// et, dans l'autre sens, tairait la divergence d'une source qu'on continuerait de citer.
#[test]
fn les_deux_verdicts_ne_se_resument_pas_en_un_seul() {
    let reference = reference(observed(Some("11")));
    let quatre_cas = [
        (hash("ab"), hash("11"), Standing::Holds, Drift::Unchanged),
        (hash("ab"), hash("99"), Standing::Holds, Drift::Moved),
        (hash("cd"), hash("11"), Standing::Broken, Drift::Unchanged),
        (hash("cd"), hash("99"), Standing::Broken, Drift::Moved),
    ];

    for (replayed, live, standing, drift) in quatre_cas {
        assert_eq!(reference.proof_standing(&replayed), standing);
        assert_eq!(reference.live_drift(&live), drift);
    }
}

/// « L'absence de preuve n'est pas une preuve », appliqué à la dérive : rien n'a été relevé au run,
/// donc on ne peut rien dire. Répondre `Unchanged` ferait passer une ignorance pour un constat.
#[test]
fn sans_releve_au_run_la_derive_est_inconnue() {
    let reference = reference(observed(None));
    assert_eq!(reference.live_drift(&hash("99")), Drift::Unknown);
    assert_eq!(reference.live_drift(&hash("ab")), Drift::Unknown);
}

// ---------------------------------------------------------------------------------------------
// Un seul locator
// ---------------------------------------------------------------------------------------------

/// §19 nomme cinq locators et n'en autorise qu'un. Le type le rend indéfaisable : une énumération
/// ne porte qu'une variante, là où une structure à cinq champs facultatifs en accepterait deux — et
/// laisserait au viewer le soin de choisir, donc de choisir différemment d'une fois sur l'autre.
#[test]
fn les_cinq_locators_de_19_existent_et_s_excluent() {
    let slugs: Vec<&str> = [
        Locator::ManifestUrl("x".to_owned()),
        Locator::CanvasId("x".to_owned()),
        Locator::ContentState("x".to_owned()),
        Locator::AnnotationTarget("x".to_owned()),
        Locator::LocalSnapshot("x".to_owned()),
    ]
    .iter()
    .map(Locator::slug)
    .collect();

    assert_eq!(
        slugs,
        vec![
            "manifest_url",
            "canvas_id",
            "content_state",
            "annotation_target",
            "local_snapshot"
        ]
    );
}

/// Un instantané local se relit hors ligne : c'est ce qui permet à une preuve de rester consultable
/// quand la source ne l'est plus — et c'est la seule des cinq façons d'y arriver.
#[test]
fn seul_l_instantane_local_se_lit_sans_reseau() {
    assert!(!Locator::LocalSnapshot("/var/locus/snap".to_owned()).needs_network());
    for distant in [
        Locator::ManifestUrl("https://x".to_owned()),
        Locator::CanvasId("x".to_owned()),
        Locator::ContentState("x".to_owned()),
        Locator::AnnotationTarget("x".to_owned()),
    ] {
        assert!(
            distant.needs_network(),
            "{} demande le réseau",
            distant.slug()
        );
    }
}

#[test]
fn un_locator_vide_est_refuse() {
    assert_eq!(
        RemoteArtifactRef::new(
            "art-1",
            "image/jpeg",
            observed(None),
            Locator::CanvasId("  ".to_owned())
        ),
        Err(RemoteRefError::EmptyField { field: "locator" })
    );
}

// ---------------------------------------------------------------------------------------------
// L'identité et le type
// ---------------------------------------------------------------------------------------------

#[test]
fn une_identite_vide_est_refusee() {
    assert_eq!(
        RemoteArtifactRef::new(
            "  ",
            "image/jpeg",
            observed(None),
            Locator::CanvasId("c1".to_owned())
        ),
        Err(RemoteRefError::EmptyField {
            field: "artifact_id"
        })
    );
}

#[test]
fn un_media_type_qui_n_en_est_pas_un_est_refuse() {
    for mauvais in ["image", "/jpeg", "image/"] {
        assert!(
            matches!(
                RemoteArtifactRef::new(
                    "art-1",
                    mauvais,
                    observed(None),
                    Locator::CanvasId("c1".to_owned())
                ),
                Err(RemoteRefError::MalformedMediaType { .. })
            ),
            "« {mauvais} » devrait être refusé"
        );
    }
}

/// Invariant 10 : xiiif n'est pas requis par les agents. La suggestion de viewer est donc
/// facultative, et son absence n'empêche rien.
#[test]
fn la_suggestion_de_viewer_est_facultative() {
    let sans = reference(observed(None));
    assert_eq!(sans.viewer_hint(), None);
    assert_eq!(sans.hinting("iiif").viewer_hint(), Some("iiif"));
}

// ---------------------------------------------------------------------------------------------
// Le lecteur validant — ce que le type engendré ne peut pas dire
// ---------------------------------------------------------------------------------------------

/// Le schéma porte `maxProperties: 1` sur `locator` ; Rust ne sait pas l'exprimer, donc le type
/// engendré offre **cinq champs facultatifs**. Un document à deux locators le traverse sans bruit,
/// et l'exclusivité ne serait tenue que par le validateur JSON — c'est-à-dire nulle part, dès qu'un
/// producteur construit la valeur en mémoire.
///
/// C'est la faute que W6.a avait laissée passer pour le manifeste et que W6.b a corrigée.
#[test]
fn un_document_a_deux_locators_est_refuse_par_le_domaine() {
    let mut wire = wire_reference();
    wire.locator.canvas_id = Some("https://exemple/canvas/p1".to_owned());

    assert_eq!(
        RemoteArtifactRef::from_wire(&wire),
        Err(locus_artifacts::RemoteRefError::LocatorCount { found: 2 })
    );
}

#[test]
fn un_document_sans_locator_est_refuse_aussi() {
    let mut wire = wire_reference();
    wire.locator.manifest_url = None;

    assert_eq!(
        RemoteArtifactRef::from_wire(&wire),
        Err(locus_artifacts::RemoteRefError::LocatorCount { found: 0 })
    );
}

#[test]
fn un_document_bien_forme_se_relit() {
    let reference = RemoteArtifactRef::from_wire(&wire_reference()).expect("document valide");
    assert_eq!(reference.artifact_id(), "art-0001");
    assert_eq!(reference.media_type(), "image/jpeg");
    assert!(matches!(reference.locator(), Locator::ManifestUrl(_)));
    assert_eq!(reference.observed().snapshot_hash, hash("ab"));
}

fn wire_reference() -> locus_lep::RemoteArtifactRef {
    locus_lep::RemoteArtifactRef {
        artifact_id: "art-0001".to_owned(),
        media_type: "image/jpeg".to_owned(),
        expected: locus_lep::RemoteArtifactRefExpected {
            snapshot_hash: locus_lep::Hash::try_from(format!("sha256:{}", "ab".repeat(32)))
                .expect("hash bien formé"),
            live_hash_at_run: None,
            captured_at: None,
        },
        locator: locus_lep::RemoteArtifactRefLocator {
            manifest_url: Some("https://exemple/iiif/manifest.json".to_owned()),
            canvas_id: None,
            content_state: None,
            annotation_target: None,
            local_snapshot: None,
        },
        viewer_hint: None,
    }
}
