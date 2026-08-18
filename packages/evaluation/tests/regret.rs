//! Test de sortie de `R3`, première moitié — **le regret structurel.**
//!
//! `R_s = U(meilleur candidat disponible) − U(graphe choisi)`.
//!
//! 1. Le regret se mesure contre le meilleur **disponible**, et le choisi doit être du menu.
//! 2. « Sur fixtures identiques » est une **condition** : un lot mélangé est refusé, en nommant.
//! 3. Un regret plus petit que la bande de bruit de `R2` n'est pas un regret.
//! 4. Deux mesures d'une même structure sont des rejeux, pas deux candidats.

use locus_evaluation::{Baseline, Candidate, RegretError, regret};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn on_alpha(structure: &str, utility: f64) -> Candidate {
    Candidate::measured(structure, "fixture-alpha", utility).expect("une mesure nommée et finie")
}

fn menu() -> Vec<Candidate> {
    vec![
        on_alpha("pair-review", 10.0),
        on_alpha("independent-pool", 14.0),
        on_alpha("blackboard", 11.0),
    ]
}

// ---------------------------------------------------------------------------------------------
// 1. Le meilleur disponible, et le choisi du menu
// ---------------------------------------------------------------------------------------------

/// Le regret est l'écart au meilleur du menu, et il nomme celui qui faisait mieux.
#[test]
fn the_regret_is_the_gap_to_the_best_available() {
    let measured = regret(&menu(), "pair-review").expect("le choisi est du menu");
    assert!((measured.value() - 4.0).abs() < f64::EPSILON);
    assert_eq!(measured.best(), "independent-pool");
    assert_eq!(measured.chosen(), "pair-review");
    assert_eq!(measured.fixture(), "fixture-alpha");
    assert_eq!(measured.candidates(), 3);
    assert!(!measured.is_none());
}

/// Choisir le meilleur ne laisse aucun regret, et le regret n'est jamais négatif.
#[test]
fn choosing_the_best_leaves_no_regret() {
    let measured = regret(&menu(), "independent-pool").expect("le choisi est du menu");
    assert!((measured.value() - 0.0).abs() < f64::EPSILON);
    assert!(measured.is_none());
    assert_eq!(measured.best(), "independent-pool");
    assert!(measured.to_string().contains("aucun regret"), "{measured}");
}

/// **La borne qui porte l'item.** Le choisi doit être du menu.
///
/// Comparer à un idéal donnerait un nombre qu'aucune décision n'aurait pu améliorer, et qui
/// grandirait à mesure qu'on imagine mieux. Un regret contre un menu dont on n'a rien pris ne veut
/// rien dire — et rendrait quand même un nombre.
#[test]
fn the_chosen_must_be_on_the_menu() {
    let refused = regret(&menu(), "hypothetical-optimum").expect_err("il n'est pas du menu");
    assert_eq!(
        refused,
        RegretError::NotAmongCandidates {
            chosen: "hypothetical-optimum".to_owned(),
        }
    );
    assert!(refused.to_string().contains("ne veut rien dire"));
}

/// Un menu vide n'a rien laissé sur la table.
#[test]
fn an_empty_menu_leaves_nothing_on_the_table() {
    let refused = regret(&[], "pair-review").expect_err("il n'y a pas de menu");
    assert_eq!(refused, RegretError::NoCandidates);
}

/// Un menu d'un seul candidat rend un regret nul — et le dit sur un seul candidat.
///
/// Un regret nul sur un candidat et un regret nul sur cinquante ne disent pas la même chose ; le
/// nombre voyage avec la valeur.
#[test]
fn a_menu_of_one_yields_no_regret_and_says_on_how_many() {
    let measured = regret(&[on_alpha("pair-review", 10.0)], "pair-review").expect("un candidat");
    assert!(measured.is_none());
    assert_eq!(measured.candidates(), 1);
}

// ---------------------------------------------------------------------------------------------
// 2. Fixtures identiques, ou rien
// ---------------------------------------------------------------------------------------------

/// **La seconde moitié de l'item.** Deux fixtures différentes comparent les fixtures.
#[test]
fn a_mixed_fixture_batch_is_refused_by_name() {
    let mixed = vec![
        on_alpha("pair-review", 10.0),
        Candidate::measured("independent-pool", "fixture-beta", 14.0).expect("mesure nommée"),
    ];
    let refused = regret(&mixed, "pair-review").expect_err("deux fixtures");
    let RegretError::DifferentFixtures { fixtures } = &refused else {
        panic!("le refus doit nommer les fixtures");
    };
    assert_eq!(fixtures.len(), 2);
    assert!(fixtures.contains("fixture-alpha"));
    assert!(fixtures.contains("fixture-beta"));
    assert!(refused.to_string().contains("comparant les fixtures"));
}

/// Le refus tombe **avant** le calcul : un écart énorme ne l'excuse pas.
#[test]
fn a_huge_gap_does_not_excuse_mixed_fixtures() {
    let mixed = vec![
        on_alpha("pair-review", 0.0),
        Candidate::measured("independent-pool", "fixture-beta", 1_000.0).expect("mesure nommée"),
    ];
    assert!(matches!(
        regret(&mixed, "pair-review"),
        Err(RegretError::DifferentFixtures { .. })
    ));
}

/// Une structure ou une fixture sans nom n'est pas une mesure.
#[test]
fn an_unnamed_measure_is_refused_by_field() {
    assert_eq!(
        Candidate::measured("  ", "fixture-alpha", 10.0).expect_err("structure vide"),
        RegretError::Unnamed { field: "structure" }
    );
    assert_eq!(
        Candidate::measured("pair-review", "", 10.0).expect_err("fixture vide"),
        RegretError::Unnamed { field: "fixture" }
    );
}

/// Une utilité non finie n'est pas une mesure, `NaN` compris.
#[test]
fn a_non_finite_utility_is_not_a_measure() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(matches!(
            Candidate::measured("pair-review", "fixture-alpha", bad),
            Err(RegretError::NotAMeasure { .. })
        ));
    }
}

// ---------------------------------------------------------------------------------------------
// 3. Un regret sous le bruit n'est pas un regret
// ---------------------------------------------------------------------------------------------

/// Les deux items de recherche se tiennent : `R3` confronte son écart à la bande de `R2`.
///
/// Sans cette confrontation, un système poursuivrait des écarts que la même structure produit toute
/// seule d'une graine à l'autre, et changerait d'organisation pour rien.
#[test]
fn a_regret_inside_the_noise_band_is_not_a_regret() {
    let thin = vec![
        on_alpha("pair-review", 10.0),
        on_alpha("independent-pool", 10.5),
    ];
    let measured = regret(&thin, "pair-review").expect("le choisi est du menu");
    assert!(!measured.is_none(), "l'écart existe");

    // Une bande de 1.0, mesurée sur trois rejeux de la même structure.
    let band = Baseline::from_replays(&[10.0, 10.5, 11.0]).expect("trois rejeux");
    assert!(
        !measured.exceeds(band),
        "0.5 tient dans une bande de 1.0 : rien à poursuivre"
    );

    // Le même écart, contre une structure qui ne varie pas.
    let stable = Baseline::from_replays(&[10.0, 10.0]).expect("deux rejeux identiques");
    assert!(measured.exceeds(stable));
}

/// Un regret nul ne dépasse aucune bande, pas même la bande nulle.
#[test]
fn no_regret_exceeds_no_band() {
    let measured = regret(&menu(), "independent-pool").expect("le meilleur");
    let stable = Baseline::from_replays(&[10.0, 10.0]).expect("deux rejeux identiques");
    assert!(!measured.exceeds(stable));
}

// ---------------------------------------------------------------------------------------------
// 4. Deux mesures d'une même structure sont des rejeux
// ---------------------------------------------------------------------------------------------

/// Le même nom deux fois n'est pas deux candidats.
///
/// C'est une [`Baseline`] qu'elles font — la bande de bruit de `R2` — et les compter comme deux
/// options ferait battre une structure par elle-même.
#[test]
fn the_same_structure_twice_is_replays_not_two_candidates() {
    let duplicated = vec![
        on_alpha("pair-review", 10.0),
        on_alpha("pair-review", 14.0),
        on_alpha("blackboard", 11.0),
    ];
    let refused = regret(&duplicated, "pair-review").expect_err("une structure mesurée deux fois");
    assert_eq!(
        refused,
        RegretError::DuplicateCandidate {
            structure: "pair-review".to_owned(),
        }
    );
    assert!(refused.to_string().contains("rejeux"));
}
