//! Test de sortie de `R2` — **le crédit structurel.**
//!
//! 1. Le hasard d'échantillonnage est une **issue nommée**, jamais un reste.
//! 2. Deux facteurs changés n'attribuent rien, et le refus les nomme.
//! 3. La bande de bruit se **mesure** par rejeu ; il n'existe ni bande par défaut ni seuil constant.
//! 4. Une régression s'attribue comme une amélioration — invariant 12.

use std::collections::BTreeSet;

use locus_evaluation::{Arm, Baseline, Credit, CreditError, Factor, attribute};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn arm(relation: &str, role: &str, budget: u64) -> Arm {
    Arm::new(relation, role, budget)
}

fn base() -> Arm {
    arm("pair-review", "prover", 1_000)
}

/// Une bande de bruit de 1.0, mesurée sur trois rejeux.
fn baseline() -> Baseline {
    Baseline::from_replays(&[10.0, 10.5, 11.0]).expect("trois rejeux")
}

// ---------------------------------------------------------------------------------------------
// 1. Le hasard est une issue nommée
// ---------------------------------------------------------------------------------------------

/// **La garantie centrale.** Un écart qui tient dans la bande n'est attribué à personne.
///
/// Une attribution qui rend toujours l'un des trois facteurs donne une histoire à chaque
/// fluctuation : on a changé quelque chose, la mesure a bougé, donc le changement a marché.
#[test]
fn a_gain_inside_the_band_is_credited_to_nobody() {
    let credit = attribute(
        &base(),
        10.0,
        &arm("independent-pool", "prover", 1_000),
        10.8,
        baseline(),
    )
    .expect("un seul facteur diffère");

    // La comparaison se fait à la tolérance près : l'écart est **calculé**, et l'égalité exacte de
    // deux flottants issus d'une soustraction est une assertion sur la représentation, pas sur le
    // verdict.
    let Credit::SamplingNoise { gain, band } = credit else {
        panic!("un écart dans la bande n'est attribué à personne");
    };
    assert!((gain - 0.8).abs() < 1e-9, "{gain}");
    assert!((band - 1.0).abs() < f64::EPSILON, "{band}");
    assert_eq!(credit.factor(), None);
    assert!(!credit.is_improvement());
}

/// Et le constat porte la bande, pas seulement le verdict.
///
/// « On ne sait pas » n'aide personne ; « voici de combien la même configuration varie toute seule,
/// et votre écart est dedans » dit quoi faire — mesurer plus, ou changer davantage.
#[test]
fn the_noise_verdict_carries_the_band_it_fell_into() {
    let credit = attribute(
        &base(),
        10.0,
        &arm("independent-pool", "prover", 1_000),
        10.2,
        baseline(),
    )
    .expect("un seul facteur diffère");
    let said = credit.to_string();
    assert!(said.contains("1.0000"), "{said}");
    assert!(said.contains("rien n'est attribué"), "{said}");
}

/// Un écart qui sort de la bande est attribué au facteur qui a changé.
#[test]
fn a_gain_beyond_the_band_is_credited_to_the_single_changed_factor() {
    for (after, factor) in [
        (arm("independent-pool", "prover", 1_000), Factor::Relation),
        (arm("pair-review", "critic", 1_000), Factor::Role),
        (arm("pair-review", "prover", 4_000), Factor::Budget),
    ] {
        let credit =
            attribute(&base(), 10.0, &after, 14.0, baseline()).expect("un seul facteur diffère");
        assert_eq!(credit, Credit::Attributed { factor, gain: 4.0 });
        assert_eq!(credit.factor(), Some(factor));
        assert!(credit.is_improvement());
    }
}

/// La frontière est stricte : un écart **égal** à la bande reste du bruit.
///
/// Trancher dans l'autre sens ferait d'une mesure qui n'a jamais dépassé ce que la configuration
/// fait toute seule une preuve d'amélioration.
#[test]
fn a_gain_exactly_equal_to_the_band_is_still_noise() {
    let credit = attribute(
        &base(),
        10.0,
        &arm("independent-pool", "prover", 1_000),
        11.0,
        baseline(),
    )
    .expect("un seul facteur diffère");
    assert!(matches!(credit, Credit::SamplingNoise { .. }));
}

// ---------------------------------------------------------------------------------------------
// 2. Deux facteurs changés n'attribuent rien
// ---------------------------------------------------------------------------------------------

/// Un écart entre deux bras qui diffèrent par deux facteurs n'est attribuable à ni l'un ni l'autre.
#[test]
fn two_changed_factors_credit_nothing() {
    let refused = attribute(
        &base(),
        10.0,
        &arm("independent-pool", "critic", 1_000),
        20.0,
        baseline(),
    )
    .expect_err("deux facteurs diffèrent");
    assert_eq!(
        refused,
        CreditError::Confounded {
            factors: [Factor::Relation, Factor::Role].into_iter().collect(),
        }
    );
}

/// Le refus les **nomme** : la suite est de mesurer chacun séparément.
#[test]
fn the_confounded_refusal_names_the_factors() {
    let refused = attribute(
        &base(),
        10.0,
        &arm("independent-pool", "critic", 4_000),
        20.0,
        baseline(),
    )
    .expect_err("les trois facteurs diffèrent");
    let CreditError::Confounded { factors } = &refused else {
        panic!("le refus doit nommer les facteurs");
    };
    assert_eq!(factors, &Factor::ALL.into_iter().collect::<BTreeSet<_>>());

    let said = refused.to_string();
    for named in ["relation", "role", "budget"] {
        assert!(said.contains(named), "{said}");
    }
}

/// Deux bras identiques ne sont pas du bruit : il n'y a rien eu à éprouver.
///
/// Rendre `SamplingNoise` ferait croire qu'un facteur a été mis à l'épreuve et n'a rien donné, alors
/// qu'aucun ne l'a été. C'est la même distinction qu'entre « non regardé » et « regardé et gardé ».
#[test]
fn identical_arms_are_not_noise() {
    let refused = attribute(&base(), 10.0, &base(), 14.0, baseline()).expect_err("rien n'a changé");
    assert_eq!(refused, CreditError::Unchanged);
    assert!(refused.to_string().contains("pas de facteur éprouvé"));
}

/// Et le confondu est refusé **avant** de regarder l'écart : un gros écart ne l'excuse pas.
#[test]
fn a_large_gain_does_not_excuse_confounding() {
    let refused = attribute(
        &base(),
        0.0,
        &arm("independent-pool", "critic", 1_000),
        1_000.0,
        baseline(),
    )
    .expect_err("deux facteurs diffèrent, quel que soit l'écart");
    assert!(matches!(refused, CreditError::Confounded { .. }));
}

// ---------------------------------------------------------------------------------------------
// 3. La bande se mesure
// ---------------------------------------------------------------------------------------------

/// Un seul rejeu ne mesure pas une variation ; zéro encore moins.
#[test]
fn fewer_than_two_replays_measure_no_band() {
    for replays in [Vec::new(), vec![10.0]] {
        let refused = Baseline::from_replays(&replays).expect_err("il en faut au moins deux");
        assert_eq!(
            refused,
            CreditError::TooFewReplays {
                given: replays.len()
            }
        );
    }
}

/// La bande est l'**étendue** des rejeux, et elle porte leur nombre.
///
/// Une bande tirée de deux rejeux et une bande tirée de deux cents ne se lisent pas pareil.
#[test]
fn the_band_is_the_range_and_carries_its_count() {
    let measured = Baseline::from_replays(&[7.0, 12.0, 9.0, 11.5]).expect("quatre rejeux");
    assert!((measured.band() - 5.0).abs() < f64::EPSILON);
    assert_eq!(measured.replays(), 4);
}

/// Une configuration parfaitement stable a une bande nulle, et tout écart lui échappe.
///
/// Zéro est une bande mesurée, pas une absence de bande : il dit « cette configuration ne varie pas
/// », ce qui est une observation forte et non un défaut de mesure.
#[test]
fn a_perfectly_stable_configuration_has_a_zero_band() {
    let stable = Baseline::from_replays(&[10.0, 10.0, 10.0]).expect("trois rejeux identiques");
    assert!((stable.band() - 0.0).abs() < f64::EPSILON);

    let credit = attribute(
        &base(),
        10.0,
        &arm("independent-pool", "prover", 1_000),
        10.000_1,
        stable,
    )
    .expect("un seul facteur diffère");
    assert_eq!(credit.factor(), Some(Factor::Relation));
}

/// Une valeur non finie n'est pas une mesure, ni dans les rejeux ni dans les bras.
#[test]
fn a_non_finite_value_is_not_a_measure() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(matches!(
            Baseline::from_replays(&[10.0, bad]),
            Err(CreditError::NotAMeasure { .. })
        ));
        assert!(matches!(
            attribute(
                &base(),
                bad,
                &arm("independent-pool", "prover", 1_000),
                14.0,
                baseline(),
            ),
            Err(CreditError::NotAMeasure { .. })
        ));
    }
}

/// Il n'existe ni bande par défaut ni seuil constant dans le module.
///
/// Le test lit le **code**, commentaires retirés : expliquer pourquoi un seuil est absent le fait
/// apparaître dans la prose, et un test qui confondrait les deux interdirait d'écrire la raison.
#[test]
fn there_is_no_default_band_and_no_constant_threshold() {
    let source = include_str!("../src/credit.rs")
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "impl Default for Baseline",
        "const BAND",
        "const THRESHOLD",
        "const EPSILON",
        "fn assumed",
    ] {
        assert!(
            !source.contains(forbidden),
            "« {forbidden} » ferait passer pour du bruit ce qu'aucun rejeu n'a mesuré"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 4. Une régression s'attribue comme une amélioration
// ---------------------------------------------------------------------------------------------

/// L'invariant 12 : les résultats négatifs ne sont jamais supprimés.
///
/// C'est aussi le seul moyen de **défaire** un changement inutile plutôt que de l'oublier.
#[test]
fn a_regression_is_credited_just_like_an_improvement() {
    let credit = attribute(
        &base(),
        14.0,
        &arm("independent-pool", "prover", 1_000),
        10.0,
        baseline(),
    )
    .expect("un seul facteur diffère");

    assert_eq!(
        credit,
        Credit::Attributed {
            factor: Factor::Relation,
            gain: -4.0,
        }
    );
    assert_eq!(credit.factor(), Some(Factor::Relation));
    assert!(
        !credit.is_improvement(),
        "attribuée, et pourtant pas une amélioration"
    );
}

/// `factor` et `is_improvement` répondent à deux questions distinctes.
///
/// Les réunir dans un seul accesseur ferait disparaître les régressions attribuées : elles ont un
/// facteur et ne sont pas des améliorations.
#[test]
fn attribution_and_improvement_are_two_questions() {
    let regression = attribute(
        &base(),
        14.0,
        &arm("pair-review", "critic", 1_000),
        10.0,
        baseline(),
    )
    .expect("un seul facteur diffère");
    let noise = attribute(
        &base(),
        10.0,
        &arm("pair-review", "critic", 1_000),
        10.5,
        baseline(),
    )
    .expect("un seul facteur diffère");

    assert!(regression.factor().is_some() && !regression.is_improvement());
    assert!(noise.factor().is_none() && !noise.is_improvement());
}

/// Les trois facteurs sont une liste close, sous leur nom.
///
/// Le hasard d'échantillonnage n'y est pas : on ne le change pas, on le mesure. Le ranger là
/// donnerait un quatrième bouton à tourner, qui n'existe pas.
#[test]
fn the_three_factors_are_a_closed_list() {
    assert_eq!(
        Factor::ALL.iter().map(|f| f.slug()).collect::<Vec<_>>(),
        vec!["relation", "role", "budget"]
    );
    let source = include_str!("../src/credit.rs");
    let start = source
        .find("pub enum Factor {")
        .expect("l'énumération existe");
    let end = source[start..].find('}').expect("elle se referme") + start;
    for absent in ["Noise", "Sampling", "Random", "Unknown", "Other"] {
        assert!(
            !source[start..end].contains(absent),
            "« {absent} » n'est pas un bouton qu'on tourne"
        );
    }
}
