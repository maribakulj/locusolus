//! Test de sortie de W17.d, seconde moitié — **la compaction de §16.5.**
//!
//! 1. Une compaction **signale ce qu'elle a omis**.
//! 2. Elle ne transforme **jamais** un objet non validé en connaissance établie.
//! 3. Elle est toujours une projection : « peut être régénérée » n'est pas optionnel.

use locus_domain::{RevisionId, ValidationLevel, ids::RevisionKind};
use locus_memory::{Compaction, CompactionError, Kept, Kind, Substance};
use locus_protocol::{Id, Timestamp};

fn revision(seed: u8) -> RevisionId {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::<RevisionKind>::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn kept(seed: u8, kind: Kind, level: ValidationLevel) -> Kept {
    Kept {
        revision: revision(seed),
        kind,
        level,
    }
}

// ---------------------------------------------------------------------------------------------
// 1. Ce qui a été omis est nommé
// ---------------------------------------------------------------------------------------------

/// §16.5 l'exige, et c'est la moitié de ce qu'une compaction dit.
///
/// Un résumé qui ne signale pas ses omissions se lit comme complet, et personne ne va chercher ce
/// qu'il ignore avoir perdu.
#[test]
fn a_compaction_names_what_it_left_out() {
    let compaction = Compaction::of(
        "synthèse du 2026-08-18",
        42,
        vec![kept(1, Kind::Fact, ValidationLevel::Reproduced)],
        vec![revision(2), revision(3)],
    )
    .expect("compaction valide");

    assert_eq!(compaction.omitted(), [revision(2), revision(3)]);
    assert_eq!(compaction.kept().len(), 1);
    assert_eq!(compaction.provenance(), "synthèse du 2026-08-18");
    assert_eq!(compaction.watermark(), 42);
}

/// Les identifiants sont **conservés**, pas résumés : « conserve les identifiants et pointeurs de
/// preuve ». Sans eux, la compaction ne renvoie à rien et devient sa propre source.
#[test]
fn a_compaction_keeps_the_identifiers_it_summarises() {
    let compaction = Compaction::of(
        "p",
        1,
        vec![
            kept(1, Kind::Fact, ValidationLevel::Reproduced),
            kept(2, Kind::Question, ValidationLevel::Unassessed),
        ],
        Vec::new(),
    )
    .expect("compaction valide");

    let revisions: Vec<RevisionId> = compaction
        .kept()
        .iter()
        .map(|entry| entry.revision)
        .collect();
    assert_eq!(revisions, [revision(1), revision(2)]);
}

/// Les quatre sortes de §16.5 se distinguent, et se relisent séparément.
///
/// Un résumé qui les aplatirait en « points » rendrait une liste où plus rien ne distingue ce qui
/// est établi de ce qui est demandé.
#[test]
fn the_four_kinds_of_the_section_stay_distinct() {
    assert_eq!(
        Kind::ALL.iter().map(|kind| kind.slug()).collect::<Vec<_>>(),
        ["fact", "hypothesis", "decision", "question"]
    );

    let compaction = Compaction::of(
        "p",
        1,
        vec![
            kept(1, Kind::Fact, ValidationLevel::Reproduced),
            kept(2, Kind::Hypothesis, ValidationLevel::Traceable),
            kept(3, Kind::Decision, ValidationLevel::InstitutionallyAccepted),
            kept(4, Kind::Question, ValidationLevel::Unassessed),
        ],
        Vec::new(),
    )
    .expect("compaction valide");

    for kind in Kind::ALL {
        assert_eq!(
            compaction.of_kind(kind).count(),
            1,
            "chaque sorte se relit séparément : {kind}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 2. Rien n'est promu en connaissance établie
// ---------------------------------------------------------------------------------------------

/// **Le test qui porte la dernière exigence de §16.5.**
///
/// Un objet que personne n'a évalué ne peut pas être consigné comme fait : c'est exactement
/// « transformer un objet non validé en connaissance établie ».
#[test]
fn an_unassessed_object_is_never_recorded_as_a_fact() {
    let error = Compaction::of(
        "p",
        1,
        vec![kept(7, Kind::Fact, ValidationLevel::Unassessed)],
        Vec::new(),
    )
    .expect_err("personne ne l'a évalué");

    assert_eq!(
        error,
        CompactionError::UnvalidatedPresentedAsFact {
            revision: revision(7)
        }
    );
}

/// Le même objet non évalué entre sans difficulté sous une **autre** sorte.
///
/// C'est le point : le refus porte sur la promotion, pas sur l'objet. Interdire à un objet non
/// évalué d'apparaître du tout ferait disparaître les questions ouvertes d'un résumé, ce qui est
/// l'inverse du but.
#[test]
fn the_same_object_enters_freely_under_another_kind() {
    for kind in [Kind::Hypothesis, Kind::Decision, Kind::Question] {
        assert!(
            Compaction::of(
                "p",
                1,
                vec![kept(7, kind, ValidationLevel::Unassessed)],
                Vec::new(),
            )
            .is_ok(),
            "{kind} n'est pas une prétention de connaissance établie"
        );
    }
}

/// Le niveau voyage **à côté** de la sorte, jamais fondu dedans.
///
/// C'est ce qui permet de constater après coup qu'une compaction n'a rien promu — sans lui, il
/// faudrait remonter à la source pour vérifier, et personne ne le fait.
#[test]
fn the_validation_level_travels_beside_the_kind() {
    let compaction = Compaction::of(
        "p",
        1,
        vec![kept(1, Kind::Fact, ValidationLevel::Traceable)],
        Vec::new(),
    )
    .expect("évalué, donc admissible comme fait");
    assert_eq!(compaction.kept()[0].level, ValidationLevel::Traceable);
    assert_eq!(compaction.kept()[0].kind, Kind::Fact);
}

#[test]
fn a_compaction_without_provenance_is_refused() {
    assert_eq!(
        Compaction::of("  ", 1, Vec::new(), Vec::new()).expect_err("sans provenance"),
        CompactionError::EmptyProvenance
    );
}

// ---------------------------------------------------------------------------------------------
// 3. Toujours une projection
// ---------------------------------------------------------------------------------------------

/// « Peut être régénérée » n'est pas une faculté optionnelle : c'est ce qui range la compaction du
/// côté des projections de §16.1.
///
/// Il n'existe aucun chemin par lequel elle se déclare canonique. Elle deviendrait la source, et
/// l'invariant 2 tomberait sans qu'aucune ligne ne l'annonce.
#[test]
fn a_compaction_is_always_a_projection() {
    let compaction = Compaction::of(
        "p",
        1,
        vec![kept(1, Kind::Fact, ValidationLevel::Reproduced)],
        vec![revision(2)],
    )
    .expect("compaction valide");

    assert_eq!(compaction.substance(), Substance::Projection);
    assert!(compaction.substance().is_regenerable());
    assert_ne!(compaction.substance(), Substance::Canonical);
}
