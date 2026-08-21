//! Test de sortie de `W6.g` — **formats d'artefact 3D et suggestions de viewer.**
//!
//! Trois propriétés, celles du tableau de `docs/10` :
//!
//! 1. un maillage entre comme artefact avec son `ArtifactManifest`, son `Integrity` et son
//!    `ProducedBy` ;
//! 2. il **suggère** un viewer par `ViewerHints` sans l'imposer, et l'invariant 10 est exercé par un
//!    client qui ignore la suggestion ;
//! 3. un artefact 3D dont l'`Assessment` porte un `Missing` **ne se promeut pas**, et le refus nomme
//!    ce qui manque.

use locus_artifacts::{
    ArtifactManifest, ArtifactState, Assessment, Integrity, Level, Missing, ProducedBy,
    PromotionError, ViewerHints, promote,
};
use locus_domain::{Confidentiality, ContentHash};
use locus_protocol::Timestamp;

const NOW: Timestamp = Timestamp::from_millis(1_700_000_000_000);

fn hash(seed: &str) -> ContentHash {
    ContentHash::of(seed.as_bytes())
}

/// Les types MIME que les formats 3D emploient réellement.
///
/// `model/gltf-binary` et `model/stl` sont enregistrés à l'IANA sous l'arbre `model/` ; PLY et OBJ
/// circulent sous des types applicatifs. Les quatre sont ici pour la même raison : ce sont ceux
/// qu'un pipeline de photogrammétrie produit.
const FORMATS_3D: [&str; 4] = [
    "model/gltf-binary",
    "model/stl",
    "application/x-ply",
    "text/plain",
];

/// Un maillage produit par un run.
fn maillage(media_type: &str, run: Option<&str>) -> ArtifactManifest {
    let mut par = ProducedBy::new("tsk-photogrammetrie", 1);
    par.run_id = run.map(ToOwned::to_owned);
    ArtifactManifest::declare(
        "art-maillage-1",
        hash("maillage"),
        media_type,
        4_096,
        par,
        Confidentiality::Internal,
    )
    .expect("un maillage est un artefact ordinaire")
}

/// L'amener jusqu'à `Verified`, d'où la promotion est permise.
fn verifie(manifeste: ArtifactManifest) -> ArtifactManifest {
    manifeste
        .uploaded(&hash("maillage"))
        .expect("le hash observé est celui qui avait été déclaré")
        .moved_to(ArtifactState::Verified)
        .expect("un artefact uploadé se vérifie")
}

// ---------------------------------------------------------------------------------------------
// 1 — un maillage est un artefact, sans machinerie neuve
// ---------------------------------------------------------------------------------------------

/// **Les quatre formats entrent**, et le manifeste porte les trois choses que §19.1 exige.
///
/// Rien de neuf n'était nécessaire pour cela, et c'est le point : un maillage est un artefact comme
/// un autre. L'item n'invente pas un chemin 3D — il vérifie qu'il n'y en a pas besoin.
#[test]
fn un_maillage_entre_comme_artefact_avec_sa_provenance() {
    for format in FORMATS_3D {
        let declare = maillage(format, Some("run-1"));
        assert_eq!(declare.media_type(), format);
        assert_eq!(declare.state(), ArtifactState::Declared);
        assert_eq!(declare.produced_by().task_id, "tsk-photogrammetrie");
        assert_eq!(declare.produced_by().attempt, 1);

        // Le hash est déclaré **avant** l'upload, et l'upload le confronte (ADR 0005).
        let charge = declare
            .uploaded(&hash("maillage"))
            .expect("le hash observé correspond");
        assert_eq!(charge.state(), ArtifactState::Uploaded);

        // Et l'intégrité s'y consigne, comme pour n'importe quel artefact.
        let inspecte = charge.with_integrity(Integrity {
            verified_at: Some(NOW),
            verified_hash_matches: Some(true),
            scanner: Some("clamav/1.4".to_owned()),
        });
        assert_eq!(
            inspecte.integrity().and_then(|i| i.verified_hash_matches),
            Some(true)
        );
    }
}

/// Un maillage dont le contenu ne correspond pas au hash déclaré est refusé, comme les autres.
#[test]
fn un_maillage_dont_le_contenu_ment_est_refuse() {
    let declare = maillage("model/gltf-binary", Some("run-1"));
    assert!(declare.uploaded(&hash("autre chose")).is_err());
}

// ---------------------------------------------------------------------------------------------
// 2 — la suggestion n'est pas une imposition
// ---------------------------------------------------------------------------------------------

/// **`ViewerHints` suggère ; l'invariant 10 tient parce qu'un client peut l'ignorer.**
///
/// « xiiif n'est pas requis par les agents » — et, plus largement, aucun viewer ne l'est. Le test
/// l'exerce des deux côtés : un client qui lit la suggestion, et un client qui ne la regarde pas et
/// obtient malgré tout ce dont il a besoin — l'identité, le type MIME, le hash.
#[test]
fn le_viewer_est_suggere_et_jamais_impose() {
    let sans = maillage("model/gltf-binary", Some("run-1"));
    assert!(
        sans.viewer_hints().is_none(),
        "un artefact sans suggestion reste un artefact valide"
    );

    let avec = maillage("model/gltf-binary", Some("run-1")).with_viewer_hints(ViewerHints {
        kind: Some("three-js".to_owned()),
        iiif_manifest_url: None,
        preview_artifact_id: Some("art-apercu-1".to_owned()),
    });
    assert_eq!(
        avec.viewer_hints().and_then(|h| h.kind.as_deref()),
        Some("three-js")
    );

    // **Le client qui ignore la suggestion.** Il n'appelle pas `viewer_hints`, et rien ne lui
    // manque : il a de quoi récupérer et vérifier le contenu. C'est cela, « n'est pas requis ».
    let client_indifferent = |manifeste: &ArtifactManifest| {
        (
            manifeste.artifact_id().to_owned(),
            manifeste.media_type().to_owned(),
            manifeste.declared_hash().clone(),
        )
    };
    assert_eq!(client_indifferent(&avec), client_indifferent(&sans));

    // Et la suggestion ne change **rien** à l'état : elle n'ouvre ni ne ferme aucune transition.
    assert_eq!(avec.state(), sans.state());
}

// ---------------------------------------------------------------------------------------------
// 3 — la promotion se mérite
// ---------------------------------------------------------------------------------------------

/// **Un artefact issu d'un run et non reproductible ne se promeut pas, et le refus nomme tout.**
///
/// « Un artefact promu peut être cité, servi, dérivé » — et ce qu'on cite doit pouvoir être refait.
/// Le refus rend **tous** les manques : un appelant qui corrigerait le premier pour buter sur le
/// suivant ferait autant d'allers-retours qu'il y a de causes.
#[test]
fn un_maillage_non_reproductible_ne_se_promeut_pas() {
    let verifie = verifie(maillage("model/gltf-binary", Some("run-1")));

    let lacunaire = Assessment {
        attained: Level::R0,
        missing: vec![Missing::Inputs, Missing::CodeRevision],
        caveats: Vec::new(),
    };

    let refus = promote(&verifie, &lacunaire).expect_err("un run sans inputs ni révision de code");
    assert_eq!(
        refus,
        PromotionError::NotReproducible {
            artifact_id: "art-maillage-1".to_owned(),
            missing: vec![Missing::Inputs, Missing::CodeRevision],
        }
    );

    let dit = refus.to_string();
    assert!(dit.contains("art-maillage-1"), "{dit}");
    assert!(dit.contains("refait"), "{dit}");

    // Le même artefact, une fois l'évaluation sans manque, se promeut.
    let complet = Assessment {
        attained: Level::R2,
        missing: Vec::new(),
        caveats: Vec::new(),
    };
    assert_eq!(
        promote(&verifie, &complet),
        Ok(ArtifactState::Promoted),
        "rien ne manque : la promotion est méritée"
    );
}

/// **Le gate ne s'applique pas à ce qui n'a jamais prétendu venir d'un run.**
///
/// Un artefact déposé par un humain n'a aucun run à reproduire ; lui reprocher `Missing::Inputs`
/// serait lui reprocher de ne pas être ce qu'il n'a jamais prétendu être. La frontière se lit dans
/// le type — `ProducedBy::run_id` — et non dans une convention de nommage.
#[test]
fn un_artefact_sans_run_se_promeut_malgre_les_manques() {
    let depose = verifie(maillage("model/stl", None));
    let lacunaire = Assessment {
        attained: Level::R0,
        missing: vec![Missing::Inputs, Missing::ReproductionNotEvidenced],
        caveats: Vec::new(),
    };

    assert_eq!(promote(&depose, &lacunaire), Ok(ArtifactState::Promoted));
}

/// **La machine à états parle en premier**, et son refus n'est pas maquillé en reproductibilité.
///
/// Un artefact en quarantaine n'a pas à être évalué pour être refusé ; lui rendre un motif de
/// reproductibilité masquerait la vraie raison et enverrait son auteur corriger ce qui n'est pas en
/// cause.
#[test]
fn un_artefact_en_quarantaine_est_refuse_par_la_machine_a_etats() {
    let quarantaine = maillage("model/gltf-binary", Some("run-1"))
        .uploaded(&hash("maillage"))
        .expect("hash correspondant")
        .moved_to(ArtifactState::Quarantined)
        .expect("un artefact uploadé se met en quarantaine");

    let complet = Assessment {
        attained: Level::R4,
        missing: Vec::new(),
        caveats: Vec::new(),
    };

    let refus = promote(&quarantaine, &complet).expect_err("la quarantaine ne promeut pas");
    assert!(
        matches!(refus, PromotionError::Forbidden(_)),
        "le motif doit être la transition, pas la reproductibilité : {refus}"
    );
}
