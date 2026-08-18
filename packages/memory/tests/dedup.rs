//! Test de sortie de W17.d, première moitié — **la déduplication de §16.4.**
//!
//! 1. Un duplicata exact par hash est détecté.
//! 2. Un candidat **sémantique** n'est jamais fusionné automatiquement, et sa résolution porte
//!    confiance et provenance.
//! 3. Une fusion se défait par une **nouvelle** décision.

use locus_domain::ContentHash;
use locus_memory::{DedupError, DuplicateCandidate, Entity, Resolution, Verdict, exact_duplicates};

fn hash(byte: &str) -> ContentHash {
    ContentHash::parse(&format!("sha256:{}", byte.repeat(64))).expect("64 hexadécimaux minuscules")
}

fn entity(key: &str, content: &str) -> Entity {
    Entity::new(key, hash(content)).expect("une entité nommée")
}

// ---------------------------------------------------------------------------------------------
// 1. Le duplicata exact est un constat
// ---------------------------------------------------------------------------------------------

/// Deux contenus de même hash **sont** le même contenu. Le dire n'engage personne, et c'est la
/// seule forme de doublon qu'on puisse affirmer sans juger.
#[test]
fn exact_duplicates_are_found_by_hash() {
    // L'ordre d'insertion est **délibérément** différent de l'ordre trié : un groupe rendu dans
    // l'ordre où les entités sont arrivées changerait à chaque relecture du corpus, et deux
    // opérateurs ne verraient pas le même doublon.
    let found = exact_duplicates(&[
        entity("d", "a"),
        entity("b", "a"),
        entity("c", "b"),
        entity("a", "a"),
    ]);

    assert_eq!(
        found.len(),
        1,
        "un seul groupe : les trois qui partagent `a`"
    );
    assert_eq!(
        found[0].keys,
        ["a", "b", "d"],
        "et il est trié, pas dans l'ordre d'arrivée"
    );
    assert_eq!(found[0].hash, hash("a"));
}

#[test]
fn an_entity_alone_with_its_hash_is_not_a_duplicate() {
    assert!(exact_duplicates(&[entity("a", "a"), entity("b", "b")]).is_empty());
}

/// Alias et identifiants externes voyagent avec l'entité — §16.4 les nomme, et sans eux une
/// résolution ne peut pas dire pourquoi deux noms désignaient la même chose.
#[test]
fn an_entity_carries_its_aliases_and_external_identifiers() {
    let entity = entity("claim-1", "a")
        .also_known_as("le résultat de mars")
        .identified_elsewhere_as("doi:10.1234/abcd");
    assert_eq!(entity.aliases(), ["le résultat de mars"]);
    assert_eq!(entity.external_ids(), ["doi:10.1234/abcd"]);
}

#[test]
fn an_entity_without_a_key_is_refused() {
    assert_eq!(
        Entity::new("  ", hash("a")).expect_err("sans clé"),
        DedupError::EmptyKey
    );
}

// ---------------------------------------------------------------------------------------------
// 2. Un candidat sémantique ne fusionne jamais tout seul
// ---------------------------------------------------------------------------------------------

/// **Le test qui porte l'exigence de §16.4.**
///
/// Un `Candidate` n'expose aucune méthode qui fusionne. Le seul chemin est `Resolution::decide`, et
/// il exige une confiance, une provenance et un décideur. Ce n'est pas une discipline d'appel : il
/// n'y a pas de `merge()` à ne pas appeler.
#[test]
fn a_semantic_candidate_carries_no_way_to_merge_itself() {
    let candidate = DuplicateCandidate::between("claim-1", "claim-2").expect("deux entités");
    assert_eq!(candidate.pair(), ("claim-1", "claim-2"));

    let resolution = Resolution::decide(
        candidate,
        Verdict::Same,
        0.9,
        "revue manuelle du 2026-08-18",
        "alice",
    )
    .expect("résolution explicite");

    assert_eq!(resolution.verdict(), Verdict::Same);
    assert!((resolution.confidence() - 0.9).abs() < f64::EPSILON);
    assert_eq!(resolution.provenance(), "revue manuelle du 2026-08-18");
    assert_eq!(resolution.decided_by(), "alice");
}

/// L'ordre des deux clés ne change pas le candidat : c'est une paire, pas une flèche.
#[test]
fn a_candidate_is_a_pair_not_an_arrow() {
    let one = DuplicateCandidate::between("b", "a").expect("deux entités");
    let other = DuplicateCandidate::between("a", "b").expect("deux entités");
    assert_eq!(one, other);
    assert_eq!(one.pair(), ("a", "b"));
}

#[test]
fn an_entity_does_not_resemble_itself() {
    assert_eq!(
        DuplicateCandidate::between("a", "a").expect_err("la même"),
        DedupError::SameEntity {
            key: "a".to_owned()
        }
    );
}

/// « Distinct » est une **réponse**.
///
/// Sans elle, un candidat non fusionné serait indiscernable d'un candidat jamais examiné, et
/// quelqu'un le réexaminerait — jusqu'à ce que l'un d'eux tranche dans l'autre sens. C'est la
/// « possibilité de *mêmes mots, concepts différents* » que §16.4 nomme en dernière ligne.
#[test]
fn concluding_that_two_things_differ_is_a_result_not_an_absence() {
    let resolution = Resolution::decide(
        DuplicateCandidate::between("bank-1", "bank-2").expect("deux entités"),
        Verdict::Distinct,
        1.0,
        "les deux « banques » ne sont pas le même concept",
        "alice",
    )
    .expect("résolution explicite");
    assert_eq!(resolution.verdict(), Verdict::Distinct);
    assert_ne!(Verdict::Distinct, Verdict::Same);
}

/// Une confiance hors bornes n'est pas une confiance faible : c'est un chiffre dont personne ne
/// sait ce qu'il mesure.
#[test]
fn a_confidence_outside_its_bounds_is_refused() {
    for absurd in [-0.1, 1.1, f64::NAN, f64::INFINITY] {
        let error = Resolution::decide(
            DuplicateCandidate::between("a", "b").expect("deux entités"),
            Verdict::Same,
            absurd,
            "p",
            "alice",
        )
        .expect_err("hors bornes");
        assert!(matches!(error, DedupError::ConfidenceOutOfRange { .. }));
    }
}

/// Une fusion anonyme ne se conteste auprès de personne ; sans provenance, elle ne se rejoue pas.
#[test]
fn a_resolution_says_where_it_comes_from_and_who_decided() {
    for (provenance, decided_by) in [("", "alice"), ("p", "  ")] {
        let error = Resolution::decide(
            DuplicateCandidate::between("a", "b").expect("deux entités"),
            Verdict::Same,
            1.0,
            provenance,
            decided_by,
        )
        .expect_err("champ vide");
        assert!(matches!(error, DedupError::EmptyField { .. }));
    }
}

// ---------------------------------------------------------------------------------------------
// 3. Une fusion se défait par une nouvelle décision
// ---------------------------------------------------------------------------------------------

/// « Fusion réversible par nouvelle décision » — pas par suppression.
///
/// La résolution d'origine reste et se relit **à travers** celle qui la renverse. La retirer
/// rendrait l'histoire fausse : on ne pourrait plus dire que des travaux ont été menés sous une
/// identification qui, désormais, n'aurait jamais existé.
#[test]
fn a_merge_is_undone_by_a_new_decision_that_cites_the_old_one() {
    let merged = Resolution::decide(
        DuplicateCandidate::between("a", "b").expect("deux entités"),
        Verdict::Same,
        0.8,
        "heuristique de mars",
        "alice",
    )
    .expect("résolution");

    let split = merged
        .clone()
        .reversed_by(Verdict::Distinct, 1.0, "relecture humaine", "bob")
        .expect("un renversement");

    assert_eq!(split.verdict(), Verdict::Distinct);
    let original = split.reverses().expect("l'originale est citée");
    assert_eq!(original.verdict(), Verdict::Same);
    assert_eq!(original.decided_by(), "alice");
    assert_eq!(original, &merged, "et elle est conservée telle quelle");
}

/// Un « renversement » qui conclut comme l'original ne renverse rien.
///
/// Le consigner comme un renversement ferait croire à un changement, et le dossier porterait une
/// décision là où il n'y a qu'une répétition.
#[test]
fn a_reversal_that_concludes_the_same_thing_is_refused() {
    let merged = Resolution::decide(
        DuplicateCandidate::between("a", "b").expect("deux entités"),
        Verdict::Same,
        0.8,
        "p",
        "alice",
    )
    .expect("résolution");

    assert_eq!(
        merged
            .reversed_by(Verdict::Same, 1.0, "p", "bob")
            .expect_err("ne renverse rien"),
        DedupError::SameVerdict {
            verdict: Verdict::Same
        }
    );
}
