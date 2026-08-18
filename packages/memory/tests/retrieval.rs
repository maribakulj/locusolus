//! Test de sortie de W17.b — **les trois garanties de l'item.**
//!
//! 1. Un résultat porte la contribution de **chacun** des signaux qui l'ont produit.
//! 2. Un ranking sans facteurs exposés est refusé.
//! 3. Un élément qu'une ACL refuse reste absent **quel que soit son score vectoriel**, et le test
//!    l'exerce avec `f64::MAX`.

use locus_domain::Confidentiality;
use locus_memory::{Candidate, Excluded, Ranking, RetrievalError, Signal, retrieve};

fn ranking(contributions: &[(Signal, f64)]) -> Ranking {
    Ranking::of(contributions).expect("le score expose ses facteurs")
}

fn candidate(key: &str, classification: Confidentiality, total: f64) -> Candidate {
    Candidate {
        key: key.to_owned(),
        classification,
        is_negative: false,
        ranking: ranking(&[(Signal::Lexical, total)]),
    }
}

// ---------------------------------------------------------------------------------------------
// 1. Les dix signaux, et la contribution de chacun
// ---------------------------------------------------------------------------------------------

#[test]
fn the_ten_signals_of_the_section_are_there_under_their_names() {
    let slugs: Vec<&str> = Signal::ALL.iter().map(|signal| signal.slug()).collect();
    assert_eq!(
        slugs,
        [
            "graph-traversal",
            "lexical",
            "vector",
            "exact-identifiers",
            "temporality",
            "validation-level",
            "branch-and-confidentiality",
            "source-diversity",
            "negative-results",
            "context-budget",
        ]
    );
}

/// « Le ranking DOIT exposer ses facteurs. »
///
/// Chaque contribution se relit séparément, et le total n'existe qu'à côté d'elles. Un score qui ne
/// rendrait que son total se comparerait, se trierait et se citerait sans que personne puisse dire
/// pourquoi il vaut ce qu'il vaut.
#[test]
fn a_score_carries_the_contribution_of_each_signal_that_made_it() {
    let score = ranking(&[
        (Signal::GraphTraversal, 0.5),
        (Signal::Vector, 0.25),
        (Signal::NegativeResults, 0.25),
    ]);

    assert_eq!(score.contribution(Signal::GraphTraversal), Some(0.5));
    assert_eq!(score.contribution(Signal::Vector), Some(0.25));
    assert_eq!(score.contribution(Signal::NegativeResults), Some(0.25));
    assert_eq!(
        score.contribution(Signal::Lexical),
        None,
        "un signal qui n'a pas contribué ne se déclare pas à zéro : il n'a rien dit"
    );
    assert!((score.total() - 1.0).abs() < f64::EPSILON);
    assert_eq!(score.factors().count(), 3);
}

// ---------------------------------------------------------------------------------------------
// 2. Un ranking sans facteurs est refusé
// ---------------------------------------------------------------------------------------------

#[test]
fn a_score_without_factors_is_refused() {
    assert_eq!(
        Ranking::of(&[]).expect_err("aucun facteur"),
        RetrievalError::NoFactorsExposed
    );
}

/// `NaN` se propagerait dans le tri en le rendant **muet plutôt que faux**, ce qui est pire : un
/// classement silencieusement arbitraire se cite comme un classement.
#[test]
fn a_contribution_that_is_not_a_finite_number_is_refused() {
    for absurd in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            Ranking::of(&[(Signal::Vector, absurd)]).expect_err("pas fini"),
            RetrievalError::NotFinite {
                signal: Signal::Vector
            }
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 3. Les embeddings ne contournent pas les ACL
// ---------------------------------------------------------------------------------------------

/// **Le test qui porte la seconde obligation de §16.3.**
///
/// Le document restreint a le score vectoriel maximal représentable. Il n'est pas dans le résultat,
/// et il n'y est pas parce que l'habilitation écarte **avant** le classement : un score maximal n'a
/// rien à contourner, il n'est pas dans la course.
#[test]
fn a_maximal_vector_score_does_not_defeat_an_acl() {
    let secret = Candidate {
        key: "restreint".to_owned(),
        classification: Confidentiality::Restricted,
        is_negative: false,
        ranking: ranking(&[(Signal::Vector, f64::MAX)]),
    };
    let ordinary = candidate("interne", Confidentiality::Internal, 0.1);

    let results = retrieve(&[secret, ordinary], Confidentiality::Internal, 10);

    assert_eq!(results.included().len(), 1);
    assert_eq!(results.included()[0].key, "interne");
    assert_eq!(
        results.excluded(),
        [Excluded::BeyondClearance {
            key: "restreint".to_owned(),
            classification: Confidentiality::Restricted,
            clearance: Confidentiality::Internal,
        }],
        "et l'exclusion est nommée : une exclusion silencieuse se lit « il n'y avait rien »"
    );
}

/// L'habilitation couvre ce qui lui est inférieur ou égal, jamais ce qui lui est supérieur.
#[test]
fn clearance_covers_what_is_at_or_below_it() {
    let all = [
        candidate("public", Confidentiality::Public, 1.0),
        candidate("interne", Confidentiality::Internal, 1.0),
        candidate("confidentiel", Confidentiality::Confidential, 1.0),
        candidate("restreint", Confidentiality::Restricted, 1.0),
    ];
    for (clearance, expected) in [
        (Confidentiality::Public, 1),
        (Confidentiality::Internal, 2),
        (Confidentiality::Confidential, 3),
        (Confidentiality::Restricted, 4),
    ] {
        let results = retrieve(&all, clearance, 10);
        assert_eq!(results.included().len(), expected, "{clearance:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// Le classement, le budget, et les résultats négatifs
// ---------------------------------------------------------------------------------------------

/// Le tri est déterministe : total décroissant, puis clé.
///
/// Un résultat qui changerait d'ordre à contenu égal ferait douter de la mémoire plutôt que du tri.
#[test]
fn the_order_is_deterministic_down_to_ties() {
    let candidates = [
        candidate("b", Confidentiality::Public, 1.0),
        candidate("a", Confidentiality::Public, 1.0),
        candidate("c", Confidentiality::Public, 2.0),
    ];
    let results = retrieve(&candidates, Confidentiality::Public, 10);
    let keys: Vec<&str> = results
        .included()
        .iter()
        .map(|found| found.key.as_str())
        .collect();
    assert_eq!(keys, ["c", "a", "b"]);
}

/// Le budget tronque, et **nomme** ce qui tombe.
///
/// Une troncature silencieuse se lit comme « il n'y avait que cela », et le chercheur ne saura pas
/// qu'il doit élargir.
#[test]
fn the_context_budget_truncates_and_says_what_it_dropped() {
    let candidates = [
        candidate("a", Confidentiality::Public, 3.0),
        candidate("b", Confidentiality::Public, 2.0),
        candidate("c", Confidentiality::Public, 1.0),
    ];
    let results = retrieve(&candidates, Confidentiality::Public, 2);

    assert_eq!(results.included().len(), 2);
    assert_eq!(
        results.excluded(),
        [Excluded::BeyondBudget {
            key: "c".to_owned(),
            rank: 3,
        }]
    );
}

/// Un résultat négatif n'est **pas** écarté.
///
/// L'invariant 12 refuse qu'on supprime les résultats négatifs pour rendre le graphe propre ; les
/// taire au retrieval reviendrait au même, en moins visible. §16.3 en fait d'ailleurs un **signal**,
/// pas un filtre.
#[test]
fn a_negative_result_is_retrieved_like_any_other() {
    let negative = Candidate {
        key: "réfutation".to_owned(),
        classification: Confidentiality::Internal,
        is_negative: true,
        ranking: ranking(&[(Signal::NegativeResults, 1.0)]),
    };
    let results = retrieve(&[negative], Confidentiality::Internal, 10);

    assert_eq!(results.included().len(), 1);
    assert!(results.included()[0].is_negative);
    assert!(results.excluded().is_empty());
}

/// Rien n'est écarté sans être nommé : les deux listes couvrent toujours l'entrée.
#[test]
fn nothing_disappears_between_the_two_lists() {
    let candidates = [
        candidate("a", Confidentiality::Public, 3.0),
        candidate("b", Confidentiality::Restricted, 2.0),
        candidate("c", Confidentiality::Public, 1.0),
    ];
    let results = retrieve(&candidates, Confidentiality::Public, 1);
    assert_eq!(
        results.included().len() + results.excluded().len(),
        candidates.len(),
        "un candidat qui n'est ni rendu ni écarté aurait disparu sans que personne le sache"
    );
}
