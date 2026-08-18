//! Test de sortie de `R4` — **le substitut d'environnement et les trajectoires contrefactuelles.**
//!
//! 1. **Unilatéral en rejet** : une seule issue conclut, et elle est négative.
//! 2. **Jamais un juge, jamais une preuve** : aucun chemin ne rend une confirmation.
//! 3. Graine et préfixe identiques sont une **condition**, et les deux refus sont distincts.
//! 4. La fidélité sur les environnements du domaine est **inconnue**, et rien ne sait dire autre
//!    chose.

use locus_evaluation::{
    CompareError, DomainEnvironment, Outcome, Trajectory, Unmeasured, compare, fidelity,
};

/// Le source sans ses commentaires — ce que le module **fait**, pas ce qu'il explique.
fn code_of(source: &str) -> String {
    source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn run(seed: u64, tail: &[&str]) -> Trajectory {
    Trajectory::observed(seed, &["boot", "fetch-manifest"], tail)
}

// ---------------------------------------------------------------------------------------------
// 1. Une seule issue conclut, et elle est négative
// ---------------------------------------------------------------------------------------------

/// Une divergence réfute, et dit **où**.
#[test]
fn a_divergence_refutes_and_says_where() {
    let outcome = compare(
        &run(7, &["read-alto", "extract-region", "commit"]),
        &run(7, &["read-alto", "extract-page", "commit"]),
    )
    .expect("même graine, même préfixe");

    assert_eq!(
        outcome,
        Outcome::Refuted {
            step: 1,
            actual: "extract-region".to_owned(),
            counterfactual: "extract-page".to_owned(),
        }
    );
    assert!(outcome.refutes());
}

/// **La garantie centrale.** L'absence de divergence n'est pas une confirmation.
///
/// Les deux suites peuvent diverger au pas suivant, et rien dans la comparaison ne dit le contraire.
#[test]
fn agreement_is_not_confirmation() {
    let outcome = compare(&run(7, &["a", "b", "c"]), &run(7, &["a", "b", "c"]))
        .expect("même graine, même préfixe");

    assert_eq!(outcome, Outcome::NotRefuted { compared: 3 });
    assert!(!outcome.refutes());
    assert!(outcome.to_string().contains("ne prouve rien"), "{outcome}");
}

/// Le constat porte le nombre de pas comparés.
///
/// « Non réfuté sur trois pas » et « non réfuté sur trois mille » ne sont pas la même chose, et un
/// verdict qui tairait la différence les rendrait interchangeables.
#[test]
fn the_non_refutation_says_over_how_many_steps() {
    let short = compare(&run(7, &["a"]), &run(7, &["a"])).expect("comparable");
    let long: Vec<&str> = std::iter::repeat_n("a", 3_000).collect();
    let deep = compare(&run(7, &long), &run(7, &long)).expect("comparable");

    assert_eq!(short, Outcome::NotRefuted { compared: 1 });
    assert_eq!(deep, Outcome::NotRefuted { compared: 3_000 });
    assert_ne!(short, deep, "le même verdict, pas la même preuve");
}

/// Deux suites de longueurs différentes se comparent sur ce qu'elles ont en commun.
///
/// Ce qui dépasse n'est pas comparé, donc ne réfute rien — et le compte le dit.
#[test]
fn only_the_common_length_is_compared() {
    let outcome = compare(&run(7, &["a", "b"]), &run(7, &["a", "b", "c", "d"]))
        .expect("même graine, même préfixe");
    assert_eq!(outcome, Outcome::NotRefuted { compared: 2 });
}

/// Deux suites vides ne réfutent rien, sur zéro pas.
///
/// Zéro n'est pas une absence de comparaison : c'est une comparaison qui n'a rien pu regarder, et le
/// dire évite qu'un rapport la confonde avec un accord.
#[test]
fn two_empty_tails_refute_nothing_over_zero_steps() {
    let outcome = compare(&run(7, &[]), &run(7, &[])).expect("même graine, même préfixe");
    assert_eq!(outcome, Outcome::NotRefuted { compared: 0 });
    assert!(!outcome.refutes());
}

// ---------------------------------------------------------------------------------------------
// 2. Jamais un juge, jamais une preuve
// ---------------------------------------------------------------------------------------------

/// Aucun chemin ne rend une confirmation.
///
/// Le test lit le **code**, commentaires retirés : expliquer pourquoi un terme est absent le fait
/// apparaître dans la prose, et un test qui confondrait les deux interdirait d'écrire la raison.
#[test]
fn nothing_here_confirms_judges_or_proves() {
    let source = code_of(include_str!("../src/counterfactual.rs"));
    for forbidden in [
        "Confirmed",
        "fn is_confirmed",
        "Validated",
        "Proven",
        "fn proves",
        "fn judge",
        "struct Proof",
        "fn accept",
    ] {
        assert!(
            !source.contains(forbidden),
            "« {forbidden} » ferait de l'absence de contre-exemple une preuve"
        );
    }
}

/// `Outcome` a exactement deux variantes, et une seule conclut.
#[test]
fn the_outcome_has_exactly_two_variants() {
    let source = code_of(include_str!("../src/counterfactual.rs"));
    let start = source
        .find("pub enum Outcome {")
        .expect("l'énumération existe");
    let end = source[start..].find("\n}").expect("elle se referme") + start;
    let body = &source[start..end];
    assert!(body.contains("Refuted {"));
    assert!(body.contains("NotRefuted {"));
    // Deux variantes : deux accolades ouvrantes de variante, plus celle de l'énumération.
    assert_eq!(body.matches(" {").count(), 3, "{body}");
}

// ---------------------------------------------------------------------------------------------
// 3. Graine et préfixe identiques, ou rien
// ---------------------------------------------------------------------------------------------

/// Deux graines différentes ne se comparent pas : la divergence s'expliquerait par le tirage.
#[test]
fn two_seeds_do_not_compare() {
    let refused = compare(&run(7, &["a"]), &run(8, &["b"])).expect_err("graines différentes");
    assert_eq!(
        refused,
        CompareError::DifferentSeed {
            actual: 7,
            counterfactual: 8,
        }
    );
    assert!(refused.to_string().contains("tirage"));
}

/// Deux préfixes différents non plus, et le refus dit **où** ils divergent.
#[test]
fn two_prefixes_do_not_compare() {
    let refused = compare(
        &Trajectory::observed(7, &["boot", "fetch-manifest"], &["a"]),
        &Trajectory::observed(7, &["boot", "fetch-collection"], &["a"]),
    )
    .expect_err("préfixes différents");
    assert_eq!(refused, CompareError::DifferentPrefix { step: 1 });
}

/// Un préfixe qui est le **début** de l'autre est refusé aussi, au pas où le plus court s'arrête.
///
/// « L'un continue » n'est pas « les deux partent du même endroit » : la trajectoire longue a fait un
/// pas de plus avant qu'on commence à comparer, et ce pas peut expliquer toute la suite.
#[test]
fn a_prefix_that_is_a_beginning_of_the_other_is_still_refused() {
    let refused = compare(
        &Trajectory::observed(7, &["boot"], &["a"]),
        &Trajectory::observed(7, &["boot", "fetch-manifest"], &["a"]),
    )
    .expect_err("un préfixe est le début de l'autre");
    assert_eq!(refused, CompareError::DifferentPrefix { step: 1 });
}

/// Les deux refus sont distincts : ils se réparent différemment.
#[test]
fn the_two_refusals_are_distinct() {
    let seed = compare(&run(7, &["a"]), &run(8, &["a"])).expect_err("graines");
    let prefix = compare(
        &Trajectory::observed(7, &["boot"], &["a"]),
        &Trajectory::observed(7, &["other"], &["a"]),
    )
    .expect_err("préfixes");
    assert_ne!(seed, prefix);
    assert_ne!(seed.to_string(), prefix.to_string());
}

/// La graine est vérifiée **avant** le préfixe : refixer la graine vient d'abord.
#[test]
fn the_seed_is_checked_before_the_prefix() {
    let refused = compare(
        &Trajectory::observed(7, &["boot"], &["a"]),
        &Trajectory::observed(8, &["other"], &["a"]),
    )
    .expect_err("les deux diffèrent");
    assert!(matches!(refused, CompareError::DifferentSeed { .. }));
}

// ---------------------------------------------------------------------------------------------
// 4. La fidélité est inconnue
// ---------------------------------------------------------------------------------------------

/// Les cinq environnements du domaine, sous leur nom.
#[test]
fn the_five_domain_environments_read_under_their_name() {
    assert_eq!(
        DomainEnvironment::ALL
            .iter()
            .map(|environment| environment.slug())
            .collect::<Vec<_>>(),
        vec!["iiif", "sparql", "alto_page", "notebook", "prover"]
    );
}

/// **La garantie de la seconde moitié.** Aucun des cinq n'a de fidélité mesurée.
///
/// La roadmap est explicite : « la fidélité sur les environnements du domaine est inconnue ». Il
/// n'existe donc aucun moyen d'exprimer une fidélité mesurée — le jour où quelqu'un mesure, le type
/// change et **tous** les appelants sont forcés de regarder.
#[test]
fn no_domain_environment_has_a_measured_fidelity() {
    for environment in DomainEnvironment::ALL {
        let known: Unmeasured = fidelity(environment);
        assert_eq!(known.environment(), environment);
        assert!(known.to_string().contains("n'a pas été mesurée"));
    }
}

/// Et le module ne sait pas exprimer un nombre de fidélité.
///
/// Une énumération à deux variantes dont l'une serait vide, ou un `f64` par défaut, dispenserait les
/// appelants de regarder le jour où la mesure arriverait.
#[test]
fn the_module_cannot_express_a_measured_fidelity() {
    let source = code_of(include_str!("../src/counterfactual.rs"));
    for forbidden in [
        "Measured {",
        "fidelity: f64",
        "enum Fidelity",
        "fn as_f64",
        "const DEFAULT_FIDELITY",
    ] {
        assert!(
            !source.contains(forbidden),
            "« {forbidden} » ferait croire à une mesure que personne n'a faite"
        );
    }
}
