//! Test de sortie de `R6` — **l'évolution inter-exécutions.**
//!
//! 1. **Récurrente** : une victoire unique ne propose rien.
//! 2. **Gagnante en validation appariée** : seul un `Credit::Attributed` positif compte ; le bruit
//!    ne compte pas.
//! 3. **Propose** : rien n'applique, et aucun chemin de type ne le permet.
//! 4. Des gains et des régressions ne se moyennent pas — invariant 12.

use locus_evaluation::{
    Credit, Evolution, EvolutionError, Factor, Improvement, Occurrence, consider,
};

/// Le source sans ses commentaires — ce que le module **fait**, pas ce qu'il explique.
fn code_of(source: &str) -> String {
    source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn won(run: &str) -> Occurrence {
    Occurrence::in_run(
        run,
        Credit::Attributed {
            factor: Factor::Relation,
            gain: 4.0,
        },
    )
    .expect("une exécution nommée")
}

fn lost(run: &str) -> Occurrence {
    Occurrence::in_run(
        run,
        Credit::Attributed {
            factor: Factor::Relation,
            gain: -4.0,
        },
    )
    .expect("une exécution nommée")
}

fn noise(run: &str) -> Occurrence {
    Occurrence::in_run(
        run,
        Credit::SamplingNoise {
            gain: 0.3,
            band: 1.0,
        },
    )
    .expect("une exécution nommée")
}

// ---------------------------------------------------------------------------------------------
// 1. Récurrente
// ---------------------------------------------------------------------------------------------

/// Trois exécutions gagnantes proposent une amélioration, et elle **nomme** les exécutions.
///
/// Nommées, pas comptées : une proposition de template se conteste, et la contester demande de
/// pouvoir aller relire les exécutions citées.
#[test]
fn three_winning_runs_propose_an_improvement() {
    let observed = [won("run-a"), won("run-b"), won("run-c")];
    let Evolution::Proposed(improvement) =
        consider(Factor::Relation, &observed, 3).expect("un seuil licite")
    else {
        panic!("trois victoires distinctes proposent");
    };
    assert_eq!(improvement.factor(), Factor::Relation);
    assert_eq!(improvement.recurrence(), 3);
    assert!(improvement.runs().contains("run-a"));
    assert!(improvement.runs().contains("run-c"));
}

/// Une victoire unique ne propose rien, et le refus dit combien il en fallait.
#[test]
fn a_single_win_proposes_nothing() {
    let observed = [won("run-a")];
    assert_eq!(
        consider(Factor::Relation, &observed, 3).expect("un seuil licite"),
        Evolution::NotRecurrent {
            runs: 1,
            required: 3,
        }
    );
}

/// La même exécution deux fois n'est qu'une exécution.
///
/// « Récurrente » se compte en exécutions **distinctes** ; compter deux fois la même ferait
/// promouvoir le tirage d'une seule.
#[test]
fn the_same_run_twice_is_one_run() {
    let observed = [won("run-a"), won("run-a"), won("run-a")];
    assert_eq!(
        consider(Factor::Relation, &observed, 3).expect("un seuil licite"),
        Evolution::NotRecurrent {
            runs: 1,
            required: 3,
        }
    );
}

/// Un seuil de récurrence inférieur à deux est refusé.
#[test]
fn a_recurrence_threshold_below_two_is_refused() {
    for required in [0_usize, 1] {
        assert_eq!(
            consider(Factor::Relation, &[won("run-a")], required)
                .expect_err("un seuil de moins de deux ne constate aucune récurrence"),
            EvolutionError::NoRecurrenceRequired { required }
        );
    }
}

/// Une exécution sans nom ne se distingue pas d'une autre.
#[test]
fn an_unnamed_run_is_refused() {
    assert_eq!(
        Occurrence::in_run(
            "   ",
            Credit::Attributed {
                factor: Factor::Relation,
                gain: 4.0,
            },
        )
        .expect_err("une exécution anonyme"),
        EvolutionError::UnnamedRun
    );
}

// ---------------------------------------------------------------------------------------------
// 2. Gagnante en validation appariée
// ---------------------------------------------------------------------------------------------

/// **Le bruit ne compte pas.** Trois exécutions où `R2` n'attribue rien ne proposent rien.
///
/// C'est la moitié de l'item : « gagnante en **validation appariée** ». Compter le bruit ferait
/// promouvoir un template sur des écarts que la même configuration produit toute seule.
#[test]
fn sampling_noise_proposes_nothing() {
    let observed = [noise("run-a"), noise("run-b"), noise("run-c")];
    assert_eq!(
        consider(Factor::Relation, &observed, 3).expect("un seuil licite"),
        Evolution::NothingAttributed { examined: 0 }
    );
}

/// Un autre facteur n'est ni au numérateur ni au dénominateur.
///
/// Si une exécution crédite la relation et une autre le budget, ce ne sont pas deux occurrences
/// d'une même adaptation : ce sont deux adaptations vues une fois chacune.
#[test]
fn another_factor_is_neither_counted_nor_examined() {
    let budget = Occurrence::in_run(
        "run-b",
        Credit::Attributed {
            factor: Factor::Budget,
            gain: 9.0,
        },
    )
    .expect("une exécution nommée");
    let observed = [won("run-a"), budget];

    assert_eq!(
        consider(Factor::Relation, &observed, 2).expect("un seuil licite"),
        Evolution::NotRecurrent {
            runs: 1,
            required: 2,
        }
    );
    // Et le budget, lui, n'a qu'une exécution.
    assert_eq!(
        consider(Factor::Budget, &observed, 2).expect("un seuil licite"),
        Evolution::NotRecurrent {
            runs: 1,
            required: 2,
        }
    );
}

/// « Rien d'attribué » et « pas assez d'exécutions » sont deux constats distincts.
///
/// Là, le facteur gagnait sans assez d'exécutions ; ici il n'a jamais rien gagné. Les confondre
/// ferait attendre d'autres exécutions d'un facteur que personne n'a vu marcher.
#[test]
fn nothing_attributed_is_not_not_recurrent() {
    let never = consider(Factor::Relation, &[noise("a"), noise("b")], 2).expect("seuil licite");
    let seldom = consider(Factor::Relation, &[won("a")], 2).expect("seuil licite");
    assert!(matches!(never, Evolution::NothingAttributed { .. }));
    assert!(matches!(seldom, Evolution::NotRecurrent { .. }));
    assert_ne!(never, seldom);
}

/// Un ensemble vide n'a rien examiné, et le dit.
#[test]
fn an_empty_history_examines_nothing() {
    assert_eq!(
        consider(Factor::Role, &[], 2).expect("un seuil licite"),
        Evolution::NothingAttributed { examined: 0 }
    );
}

/// Ce module ne rejuge rien : il compte des verdicts déjà rendus.
///
/// Refaire l'attribution ici serait une seconde attribution, avec sa propre bande de bruit, qui
/// divergerait de la première.
#[test]
fn the_module_recomputes_no_attribution() {
    let source = code_of(include_str!("../src/evolution.rs"));
    for absent in ["Baseline", "fn attribute", "band", "utility"] {
        assert!(
            !source.contains(absent),
            "« {absent} » ferait de ce module une seconde attribution"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 3. Propose, n'applique pas
// ---------------------------------------------------------------------------------------------

/// Rien dans le module n'applique une amélioration.
///
/// Même forme que la boucle lente de W18.b : une adaptation de structure est une proposition qui
/// suit son chemin entier, jamais une écriture.
///
/// Le test lit le **code**, commentaires retirés : expliquer pourquoi un terme est absent le fait
/// apparaître dans la prose, et un test qui confondrait les deux interdirait d'écrire la raison.
#[test]
fn nothing_applies_an_improvement() {
    let source = code_of(include_str!("../src/evolution.rs"));
    for forbidden in [
        "fn apply",
        "fn commit",
        "fn write",
        "AgentTemplate",
        "fn update_template",
        "Improvement::new",
    ] {
        assert!(
            !source.contains(forbidden),
            "« {forbidden} » ferait de ce module un chemin d'écriture de template"
        );
    }
    // Un seul site construit la structure, et il est dans `consider`. La déclaration, le bloc
    // `impl` et l'implémentation de `Display` portent la même sous-chaîne ; ils sont retirés par
    // leur préfixe plutôt qu'en ajustant un compte, qu'un ajout ultérieur ferait « corriger ».
    let constructions = source
        .match_indices("Improvement {")
        .filter(|(offset, _)| {
            let before = &source[..*offset];
            !before.ends_with("pub struct ")
                && !before.ends_with("impl ")
                && !before.ends_with("for ")
        })
        .count();
    assert_eq!(constructions, 1);
    assert!(source.contains("Evolution::Proposed(Improvement {"));
}

/// Et l'`Improvement` ne porte que ce qui se conteste : un facteur et des exécutions.
#[test]
fn an_improvement_carries_only_what_can_be_contested() {
    let observed = [won("run-a"), won("run-b")];
    let Evolution::Proposed(improvement) =
        consider(Factor::Relation, &observed, 2).expect("un seuil licite")
    else {
        panic!("deux victoires distinctes proposent");
    };
    let shown: Improvement = improvement;
    assert!(shown.to_string().contains("proposée"), "{shown}");
}

// ---------------------------------------------------------------------------------------------
// 4. Rien ne se moyenne
// ---------------------------------------------------------------------------------------------

/// **La garantie de l'invariant 12.** Deux gains et une régression ne font pas « globalement
/// positif ».
///
/// Moyenner reviendrait à supprimer un résultat négatif pour rendre le dossier lisible — et à
/// promouvoir un template dont on sait qu'il a déjà nui une fois, sans savoir pourquoi.
#[test]
fn wins_and_regressions_do_not_average_out() {
    let observed = [won("run-a"), won("run-b"), lost("run-c")];
    let Evolution::Contradictory { wins, regressions } =
        consider(Factor::Relation, &observed, 2).expect("un seuil licite")
    else {
        panic!("un gain et une régression ne se moyennent pas");
    };
    assert_eq!(wins.len(), 2);
    assert_eq!(regressions.len(), 1);
    assert!(regressions.contains("run-c"));
}

/// La contradiction l'emporte même quand les gains dépassent largement le seuil.
///
/// Un seuil atteint n'est pas une raison d'oublier la régression : c'est même le cas où l'oubli
/// serait le plus tentant.
#[test]
fn the_contradiction_wins_even_over_a_met_threshold() {
    let observed = [
        won("run-a"),
        won("run-b"),
        won("run-c"),
        won("run-d"),
        lost("run-e"),
    ];
    assert!(matches!(
        consider(Factor::Relation, &observed, 2).expect("un seuil licite"),
        Evolution::Contradictory { .. }
    ));
}

/// Et une régression seule ne propose rien non plus — sans être une contradiction.
#[test]
fn regressions_alone_are_not_a_contradiction() {
    let observed = [lost("run-a"), lost("run-b")];
    assert_eq!(
        consider(Factor::Relation, &observed, 2).expect("un seuil licite"),
        Evolution::NothingAttributed { examined: 2 },
    );
}
