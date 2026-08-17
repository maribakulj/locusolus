//! Test de sortie de W6.a — ADR 0005, `docs/SPEC_V1.md` §19.1, §19.2.
//!
//! **Le hash est déclaré avant l'upload et confronté à l'arrivée ; la promotion ne se saute pas ;
//! l'histoire d'un artefact ne s'efface pas.**
//!
//! Les trois disent la même chose sous trois angles : un artefact ne vaut que par ce qu'on peut
//! vérifier de lui. Un manifeste écrit après coup à partir du contenu reçu dit seulement que ce qui
//! est arrivé est ce qui est arrivé.

use locus_artifacts::{
    ArtifactManifest, ArtifactState, Derivation, DerivationRelation, ManifestError, ProducedBy,
    transition,
};
use locus_domain::{Confidentiality, ContentHash};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

const PROMISED: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const OTHER: &str = "sha256:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

fn hash(value: &str) -> ContentHash {
    ContentHash::parse(value).expect("hash bien formé")
}

fn declared() -> ArtifactManifest {
    ArtifactManifest::declare(
        "artifact-0001",
        hash(PROMISED),
        "image/jp2",
        4_194_304,
        ProducedBy::new("task-0007", 2),
        Confidentiality::Public,
    )
    .expect("manifeste valide")
}

// ---------------------------------------------------------------------------------------------
// Le hash déclaré avant l'upload
// ---------------------------------------------------------------------------------------------

/// La garantie de ADR 0005, dans un test. La déclaration fait du hash une **promesse** ; l'arrivée
/// la confronte. Sans cet ordre, le manifeste ne dirait que ce que le contenu reçu contenait.
#[test]
fn un_contenu_qui_n_est_pas_celui_promis_est_refuse() {
    match declared().uploaded(&hash(OTHER)) {
        Err(ManifestError::HashMismatch { declared, observed }) => {
            assert_eq!(declared.to_string(), PROMISED);
            assert_eq!(observed.to_string(), OTHER);
        }
        other => panic!("le hash déclaré doit être confronté, pas remplacé : {other:?}"),
    }
}

#[test]
fn le_contenu_promis_fait_avancer_l_artefact() {
    let uploaded = declared()
        .uploaded(&hash(PROMISED))
        .expect("le contenu est celui qui avait été déclaré");
    assert_eq!(uploaded.state(), ArtifactState::Uploaded);
    assert_eq!(uploaded.declared_hash().to_string(), PROMISED);
}

#[test]
fn un_hash_mal_forme_n_entre_pas_dans_un_manifeste() {
    // Le type vient du domaine depuis W6.b : ce crate en avait écrit un deuxième, plus permissif
    // que celui de §7.7 — il acceptait l'hexadécimal majuscule, donc deux écritures d'un même
    // hash, donc deux formes canoniques d'un même contenu.
    for rejected in [
        "",
        "sha256:court",
        "md5:0123456789abcdef",
        PROMISED.trim_end_matches('f'),
        &PROMISED.to_uppercase(),
    ] {
        assert!(
            ContentHash::parse(rejected).is_err(),
            "« {rejected} » ne désigne pas un contenu"
        );
    }
}

#[test]
fn un_manifeste_sans_identite_ni_taille_est_refuse() {
    let missing = ArtifactManifest::declare(
        "  ",
        hash(PROMISED),
        "image/jp2",
        1,
        ProducedBy::new("task-0007", 1),
        Confidentiality::Public,
    );
    assert_eq!(
        missing,
        Err(ManifestError::EmptyField {
            field: "artifact_id"
        })
    );

    let empty = ArtifactManifest::declare(
        "artifact-0002",
        hash(PROMISED),
        "image/jp2",
        0,
        ProducedBy::new("task-0007", 1),
        Confidentiality::Public,
    );
    assert_eq!(
        empty,
        Err(ManifestError::ZeroSize),
        "la taille annoncée sert à borner l'upload avant de l'accepter"
    );
}

// ---------------------------------------------------------------------------------------------
// La promotion ne se saute pas
// ---------------------------------------------------------------------------------------------

#[test]
fn declared_ne_mene_pas_directement_a_promoted() {
    let refused = declared().moved_to(ArtifactState::Promoted);
    assert!(
        matches!(refused, Err(ManifestError::Forbidden(_))),
        "ADR 0005 dit « quarantaine puis promotion » : {refused:?}"
    );
}

#[test]
fn un_contenu_arrive_n_est_pas_un_contenu_verifie() {
    let uploaded = declared()
        .uploaded(&hash(PROMISED))
        .expect("contenu conforme");
    assert!(matches!(
        uploaded.moved_to(ArtifactState::Promoted),
        Err(ManifestError::Forbidden(_))
    ));
}

#[test]
fn le_chemin_complet_mene_a_la_promotion() {
    let promoted = declared()
        .uploaded(&hash(PROMISED))
        .expect("contenu conforme")
        .moved_to(ArtifactState::Quarantined)
        .expect("un contenu non fiable est retenu")
        .moved_to(ArtifactState::Verified)
        .expect("la vérification conclut")
        .moved_to(ArtifactState::Promoted)
        .expect("la promotion suit la vérification");

    assert!(promoted.is_servable());
    assert_eq!(
        promoted.history(),
        [
            ArtifactState::Declared,
            ArtifactState::Uploaded,
            ArtifactState::Quarantined,
            ArtifactState::Verified,
            ArtifactState::Promoted,
        ],
        "un artefact promu après quarantaine n'a pas la même histoire qu'un artefact promu droit"
    );
}

#[test]
fn la_quarantine_peut_etre_evitee_mais_pas_la_verification() {
    let promoted = declared()
        .uploaded(&hash(PROMISED))
        .expect("contenu conforme")
        .moved_to(ArtifactState::Verified)
        .expect("un contenu de source fiable se vérifie sans quarantaine")
        .moved_to(ArtifactState::Promoted)
        .expect("la promotion suit la vérification");
    assert_eq!(promoted.history().len(), 4);
}

#[test]
fn un_seul_etat_autorise_a_servir_le_contenu() {
    let servable: Vec<&str> = ArtifactState::ALL
        .into_iter()
        .filter(|state| state.is_servable())
        .map(ArtifactState::slug)
        .collect();
    assert_eq!(
        servable,
        vec!["promoted"],
        "écrire « state != rejected » quelque part reviendrait à servir cinq états sur six"
    );
}

// ---------------------------------------------------------------------------------------------
// Les états terminaux
// ---------------------------------------------------------------------------------------------

#[test]
fn le_refus_est_atteignable_depuis_partout_sauf_la_fin() {
    for state in ArtifactState::ALL {
        let reachable = transition(state, ArtifactState::Rejected).is_ok();
        assert_eq!(
            reachable,
            !state.is_terminal(),
            "« {state} » : un refus doit rester possible tant que quelque chose peut arriver"
        );
    }
}

#[test]
fn promoted_et_rejected_sont_terminaux() {
    let terminal: Vec<&str> = ArtifactState::ALL
        .into_iter()
        .filter(|state| state.is_terminal())
        .map(ArtifactState::slug)
        .collect();
    assert_eq!(terminal, vec!["promoted", "rejected"]);
}

/// Retirer un artefact promu n'est pas une transition d'état — ce serait effacer qu'il a été cité.
/// L'invariant 12 vaut ici comme ailleurs, et le refus le rend visible.
#[test]
fn un_artefact_promu_ne_se_deprome_pas() {
    for target in ArtifactState::ALL {
        assert!(
            transition(ArtifactState::Promoted, target).is_err(),
            "« promoted » ne mène nulle part, pas même à « {target} »"
        );
    }
}

#[test]
fn le_refus_nomme_ce_qui_etait_possible() {
    let Err(refused) = transition(ArtifactState::Declared, ArtifactState::Verified) else {
        panic!("un contenu qui n'est pas arrivé ne se vérifie pas")
    };
    let message = refused.to_string();
    assert!(message.contains("uploaded"), "{message}");
    assert!(message.contains("rejected"), "{message}");
}

// ---------------------------------------------------------------------------------------------
// La dérivation
// ---------------------------------------------------------------------------------------------

/// §19.2 : par hash et non par nom. Un chemin change, un contenu non — nommer un parent par son
/// chemin ferait pointer la provenance vers ce qui se trouve aujourd'hui à cet endroit.
#[test]
fn la_derivation_se_declare_par_hash() {
    let parent = Derivation::new(
        "artifact-source",
        DerivationRelation::DerivedFrom,
        Some(hash(OTHER)),
    )
    .expect("un parent identifié");
    let derived = declared().with_derivations(vec![parent]);
    assert_eq!(derived.derivations().len(), 1);
    assert_eq!(
        derived.derivations()[0]
            .content_hash()
            .map(ToString::to_string)
            .as_deref(),
        Some(OTHER)
    );
}

#[test]
fn une_derivation_sans_parent_identifie_ne_designe_rien() {
    assert!(Derivation::new("  ", DerivationRelation::Reproduces, None).is_err());
}

#[test]
fn un_etat_inconnu_ne_devient_pas_un_etat_par_defaut() {
    assert_eq!(ArtifactState::parse("published"), None);
    assert_eq!(
        ArtifactState::parse("promoted"),
        Some(ArtifactState::Promoted)
    );
}
