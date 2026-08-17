//! Test de sortie de W6.b — **le manifeste dit ce que le schéma dit, et rien ne se perd à la
//! traversée.**
//!
//! Un manifeste qui traverse un service qui n'en connaît que le noyau ressortirait amputé de sa
//! licence, de ses hints et de ses dérivations typées, sans qu'aucune erreur ne le signale. C'est
//! la forme de perte qu'on ne remarque qu'en cherchant une licence six mois plus tard.
//!
//! L'aller-retour se fait sur la fixture **la plus complète**, pas sur un cas minimal : un
//! manifeste minimal ne prouverait que le noyau, et c'est justement le reste qui disparaît.

use locus_artifacts::{
    ArtifactManifest, ArtifactState, DerivationRelation, ManifestError, ProducedBy, WireError,
};
use locus_domain::{Confidentiality, ContentHash};
use locus_lep::ArtifactManifest as WireManifest;
use serde_json::Value;
use std::{fs, path::PathBuf};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

const COMPLETE: &str = "artifact-manifest-promoted.json";
const MINIMAL: &str = "artifact-manifest-quarantined.json";

/// Le document, sans le bloc `_fixture` : métadonnée de test, jamais un champ du schéma.
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

fn wire(name: &str) -> WireManifest {
    serde_json::from_value(body(name)).expect("la fixture se décode dans le type généré")
}

fn hash(value: &str) -> ContentHash {
    ContentHash::parse(value).expect("hash bien formé")
}

// ---------------------------------------------------------------------------------------------
// Rien ne se perd
// ---------------------------------------------------------------------------------------------

#[test]
fn un_manifeste_complet_fait_un_aller_retour_exact() {
    let original = body(COMPLETE);
    let manifest = ArtifactManifest::from_wire(&wire(COMPLETE)).expect("fixture conforme");
    let rewritten = serde_json::to_value(manifest.to_wire()).expect("ré-encodage");
    assert_eq!(
        rewritten, original,
        "un champ que le domaine ne modélise pas disparaîtrait ici, et nulle part ailleurs"
    );
}

#[test]
fn un_champ_absent_ne_reapparait_pas() {
    let original = body(MINIMAL);
    assert!(original.get("derived_from").is_none());
    assert!(original.get("rights").is_none());
    let manifest = ArtifactManifest::from_wire(&wire(MINIMAL)).expect("fixture conforme");
    let rewritten = serde_json::to_value(manifest.to_wire()).expect("ré-encodage");
    assert_eq!(
        rewritten, original,
        "réécrire `derived_from: []` là où l'entrée n'avait rien ferait diverger deux hashes de \
         document sur une donnée que personne n'a écrite"
    );
}

#[test]
fn la_derivation_garde_sa_relation_et_son_identite() {
    let manifest = ArtifactManifest::from_wire(&wire(COMPLETE)).expect("fixture conforme");
    let parents = manifest.derivations();
    assert_eq!(parents.len(), 2);

    assert_eq!(parents[0].artifact_id(), "artifact-measurements");
    assert_eq!(parents[0].relation(), DerivationRelation::DerivedFrom);
    assert_eq!(
        parents[0]
            .content_hash()
            .map(ToString::to_string)
            .as_deref(),
        Some("blake3:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"),
        "le vocabulaire LEP accepte blake3 : le domaine doit le lire aussi"
    );

    assert_eq!(parents[1].relation(), DerivationRelation::Supersedes);
    assert!(
        parents[1].content_hash().is_none(),
        "le hash d'un parent est facultatif au schéma ; l'inventer attesterait de ce qu'on ignore"
    );
}

/// `reproduces` est ce qu'inscrit une reproduction indépendante (§19.7, R4) et `supersedes` ce
/// qu'inscrit une correction. Une liste de hashes nus — ce que portait W6.a — les rend
/// indistinguables, et un graphe qui ne sait plus qui reproduit qui ne peut plus dire ce qui est
/// reproduit.
#[test]
fn les_cinq_relations_sont_distinctes_et_completes() {
    let slugs: Vec<&str> = DerivationRelation::ALL
        .into_iter()
        .map(DerivationRelation::slug)
        .collect();
    assert_eq!(
        slugs,
        vec![
            "derived_from",
            "produced_by",
            "consumes",
            "supersedes",
            "reproduces"
        ]
    );
    assert_eq!(
        DerivationRelation::parse("reproduces"),
        Some(DerivationRelation::Reproduces)
    );
    assert_eq!(
        DerivationRelation::parse("inspired_by"),
        None,
        "une relation inconnue avalée par `derived_from` serait une provenance devinée"
    );
}

// ---------------------------------------------------------------------------------------------
// Ce que le fil laisse passer et que le domaine refuse
// ---------------------------------------------------------------------------------------------

#[test]
fn un_etat_hors_enumeration_est_refuse_par_son_nom() {
    let mut document = wire(COMPLETE);
    document.state = "published".to_owned();
    assert_eq!(
        ArtifactManifest::from_wire(&document),
        Err(WireError::UnknownState {
            value: "published".to_owned()
        }),
        "le type généré porte `state: String` ; c'est ici, et seulement ici, que l'énumération tient"
    );
}

#[test]
fn une_relation_hors_enumeration_est_refusee_par_son_nom() {
    let mut document = wire(COMPLETE);
    document.derived_from.as_mut().expect("des parents")[0].relation = "inspired_by".to_owned();
    assert_eq!(
        ArtifactManifest::from_wire(&document),
        Err(WireError::UnknownRelation {
            value: "inspired_by".to_owned()
        })
    );
}

#[test]
fn une_taille_negative_n_existe_pas() {
    let mut document = wire(COMPLETE);
    document.size_bytes = -1;
    assert_eq!(
        ArtifactManifest::from_wire(&document),
        Err(WireError::NegativeSize { value: -1 }),
        "le schéma dit `minimum: 0`, le type généré dit `i64` : l'écart se referme à la traduction"
    );
}

#[test]
fn un_horodatage_non_canonique_est_refuse() {
    let mut document = wire(COMPLETE);
    document.declared_at = Some("2026-08-17T11:19:58Z".to_owned());
    assert_eq!(
        ArtifactManifest::from_wire(&document),
        Err(WireError::MalformedTimestamp {
            value: "2026-08-17T11:19:58Z".to_owned()
        }),
        "§7.7 : exactement trois décimales — deux écritures d'un même instant, deux signatures"
    );
}

#[test]
fn un_attempt_zero_ne_designe_aucune_execution() {
    let mut document = wire(COMPLETE);
    document.produced_by.attempt = 0;
    assert_eq!(
        ArtifactManifest::from_wire(&document),
        Err(WireError::Manifest {
            error: ManifestError::ZeroAttempt
        })
    );
}

// ---------------------------------------------------------------------------------------------
// L'histoire ne traverse pas, et c'est dit
// ---------------------------------------------------------------------------------------------

/// Le manifeste porte l'**état**, pas le chemin. L'histoire des transitions vit dans l'event
/// store, qui est la vérité institutionnelle (invariant 2) ; un manifeste relu depuis le fil ne
/// peut pas savoir ce qu'il a traversé, et rejouer les transitions depuis `declared` inventerait
/// une histoire que personne n'a vue.
#[test]
fn un_manifeste_relu_ne_pretend_pas_connaitre_son_passe() {
    let manifest = ArtifactManifest::from_wire(&wire(COMPLETE)).expect("fixture conforme");
    assert_eq!(manifest.state(), ArtifactState::Promoted);
    assert_eq!(
        manifest.history(),
        [ArtifactState::Promoted],
        "quatre états inventés seraient indiscernables de quatre états observés"
    );
    assert!(manifest.is_servable());
}

/// Et l'état relu est bien le point de départ des transitions suivantes : un artefact promu relu
/// depuis le fil reste terminal.
#[test]
fn l_etat_relu_gouverne_la_suite() {
    let promoted = ArtifactManifest::from_wire(&wire(COMPLETE)).expect("fixture conforme");
    assert!(matches!(
        promoted.moved_to(ArtifactState::Rejected),
        Err(ManifestError::Forbidden(_))
    ));

    let quarantined = ArtifactManifest::from_wire(&wire(MINIMAL)).expect("fixture conforme");
    assert!(
        quarantined.moved_to(ArtifactState::Verified).is_ok(),
        "la quarantaine mène à la vérification, depuis le fil comme depuis la mémoire"
    );
}

// ---------------------------------------------------------------------------------------------
// Le domaine ne construit rien que le schéma refuserait
// ---------------------------------------------------------------------------------------------

#[test]
fn un_type_mime_hors_forme_n_entre_pas_dans_un_manifeste() {
    for refused in ["", "image", "IMAGE/JP2", "image/", "image/jp 2"] {
        let built = ArtifactManifest::declare(
            "artifact-0001",
            hash("sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"),
            refused,
            1,
            ProducedBy::new("task-0007", 1),
            Confidentiality::Public,
        );
        assert!(
            matches!(built, Err(ManifestError::MalformedMediaType { .. })),
            "« {refused} » ne passerait pas le patron du schéma : {built:?}"
        );
    }
}

#[test]
fn une_taille_qui_ne_tient_pas_sur_le_fil_est_refusee_a_la_construction() {
    let built = ArtifactManifest::declare(
        "artifact-0001",
        hash("sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"),
        "image/jp2",
        u64::MAX,
        ProducedBy::new("task-0007", 1),
        Confidentiality::Public,
    );
    assert_eq!(
        built,
        Err(ManifestError::SizeBeyondWire { value: u64::MAX }),
        "refuser ici est la seule façon que `to_wire` soit total sans mentir sur la taille"
    );
}

#[test]
fn les_quatre_classifications_traversent_dans_les_deux_sens() {
    for classification in [
        Confidentiality::Public,
        Confidentiality::Internal,
        Confidentiality::Confidential,
        Confidentiality::Restricted,
    ] {
        let manifest = ArtifactManifest::declare(
            "artifact-0001",
            hash("sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"),
            "image/jp2",
            1024,
            ProducedBy::new("task-0007", 1),
            classification,
        )
        .expect("manifeste valide");
        let returned = ArtifactManifest::from_wire(&manifest.to_wire()).expect("aller-retour");
        assert_eq!(
            returned.classification(),
            classification,
            "une classification qui glisse d'un cran est une fuite ou une entrave"
        );
    }
}
