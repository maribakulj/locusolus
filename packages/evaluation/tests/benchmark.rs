//! Test de sortie de W12.c — **une mesure absente n'est pas une mesure nulle.**
//!
//! §29.7 compare six architectures, dont la dernière est celle qu'on construit. C'est ce qui rend la
//! tentation de la faire gagner permanente, et les trous de mesure dangereux : une configuration
//! dont on n'a pas relevé les faux positifs les aurait à zéro dans un classement naïf, donc
//! gagnerait — et le classement aurait l'air parfaitement sain.

use locus_evaluation::{
    BenchmarkError, Comparison, Configuration, Coverage, Direction, Metric, Ranking,
};

fn complete() -> Comparison {
    let mut comparaison = Comparison::new();
    for (rang, configuration) in Configuration::ALL.into_iter().enumerate() {
        for metric in Metric::ALL {
            #[expect(clippy::cast_precision_loss, reason = "six configurations")]
            let valeur = rang as f64;
            comparaison = comparaison
                .measured(configuration, metric, valeur)
                .expect("lecture finie");
        }
    }
    comparaison
}

// ---------------------------------------------------------------------------------------------
// Les six configurations et les onze mesures
// ---------------------------------------------------------------------------------------------

#[test]
fn les_six_configurations_de_29_7_existent_sous_leur_nom() {
    let slugs: Vec<&str> = Configuration::ALL.iter().map(|c| c.slug()).collect();
    assert_eq!(
        slugs,
        vec![
            "single-agent",
            "parallel-without-shared-memory",
            "simple-hierarchy",
            "canterel-alone",
            "locus-without-portfolio",
            "locus-complete"
        ]
    );
}

#[test]
fn les_onze_mesures_de_29_7_existent_sous_leur_nom() {
    assert_eq!(Metric::ALL.len(), 11);
    let mut slugs: Vec<&str> = Metric::ALL.iter().map(|m| m.slug()).collect();
    let total = slugs.len();
    slugs.sort_unstable();
    slugs.dedup();
    assert_eq!(slugs.len(), total, "onze mesures distinctes");
}

/// Plus n'est pas toujours mieux. Se tromper de sens ferait élire la configuration la plus chère,
/// et le classement aurait l'air parfaitement sain — d'où l'énumération explicite des quatre qui se
/// lisent à l'envers.
#[test]
fn quatre_mesures_se_lisent_a_l_envers_des_autres() {
    let a_minimiser: Vec<&str> = Metric::ALL
        .iter()
        .filter(|metric| metric.direction() == Direction::LowerIsBetter)
        .map(|metric| metric.slug())
        .collect();
    assert_eq!(
        a_minimiser,
        vec![
            "false-positives",
            "cost",
            "review-rejection-rate",
            "time-to-validation"
        ]
    );
}

// ---------------------------------------------------------------------------------------------
// Une mesure absente n'est pas une mesure nulle
// ---------------------------------------------------------------------------------------------

/// Le cœur de W12.c. La configuration à qui il manque la lecture serait la meilleure sur une mesure
/// à minimiser si l'absence valait zéro — et c'est la configuration qu'on construit qui aurait le
/// plus de raisons d'avoir un trou de mesure.
#[test]
fn une_lecture_manquante_empeche_de_trancher_au_lieu_de_valoir_zero() {
    let mut trouee = Comparison::new();
    for configuration in Configuration::ALL {
        if configuration == Configuration::LocusComplete {
            continue;
        }
        trouee = trouee
            .measured(configuration, Metric::FalsePositives, 5.0)
            .expect("lecture finie");
    }

    let verdict = trouee.best_on(Metric::FalsePositives);
    assert_eq!(
        verdict,
        Ranking::Incomparable {
            missing: vec![Configuration::LocusComplete]
        }
    );
    assert!(verdict.to_string().contains("locus-complete"));
}

/// Chacune des six, laissée sans lecture, empêche de trancher. Les éprouver une par une est ce qui
/// empêche qu'une configuration devienne facultative sans qu'on le voie.
#[test]
fn chaque_configuration_sans_lecture_empeche_de_trancher() {
    for absente in Configuration::ALL {
        let mut comparaison = Comparison::new();
        for configuration in Configuration::ALL {
            if configuration == absente {
                continue;
            }
            comparaison = comparaison
                .measured(configuration, Metric::Accuracy, 1.0)
                .expect("lecture finie");
        }
        assert_eq!(
            comparaison.best_on(Metric::Accuracy),
            Ranking::Incomparable {
                missing: vec![absente]
            },
            "{absente}"
        );
    }
}

#[test]
fn une_comparaison_complete_tranche() {
    // `complete()` donne au rang le plus élevé la plus grande valeur.
    assert_eq!(
        complete().best_on(Metric::Accuracy),
        Ranking::Best {
            configuration: Configuration::LocusComplete,
            value: 5.0
        }
    );
    // Et sur une mesure à minimiser, c'est l'inverse qui gagne.
    assert_eq!(
        complete().best_on(Metric::Cost),
        Ranking::Best {
            configuration: Configuration::SingleAgent,
            value: 0.0
        }
    );
}

/// La direction s'applique à chaque mesure, pas seulement à celles qu'on a pensé à tester : les
/// onze sont parcourues, et le gagnant attendu dépend du sens.
#[test]
fn le_sens_de_chaque_mesure_decide_du_gagnant() {
    let comparaison = complete();
    for metric in Metric::ALL {
        let attendu = match metric.direction() {
            Direction::HigherIsBetter => Configuration::LocusComplete,
            Direction::LowerIsBetter => Configuration::SingleAgent,
        };
        let Ranking::Best { configuration, .. } = comparaison.best_on(metric) else {
            panic!("{metric} incomparable dans une comparaison complète");
        };
        assert_eq!(configuration, attendu, "{metric}");
    }
}

// ---------------------------------------------------------------------------------------------
// La couverture
// ---------------------------------------------------------------------------------------------

#[test]
fn une_comparaison_complete_couvre_les_soixante_six_lectures() {
    assert_eq!(complete().coverage(), Coverage::Complete);
}

#[test]
fn une_comparaison_vide_nomme_tout_ce_qui_manque() {
    let Coverage::Partial { missing } = Comparison::new().coverage() else {
        panic!("une comparaison vide se dit complète");
    };
    assert_eq!(missing.len(), 6 * 11);
}

/// Une seule lecture manquante suffit à rendre la comparaison partielle, et elle est nommée par son
/// couple : dire « il manque une lecture » sans dire laquelle oblige à tout reprendre.
#[test]
fn une_seule_lecture_manquante_rend_la_comparaison_partielle() {
    let mut presque = Comparison::new();
    for configuration in Configuration::ALL {
        for metric in Metric::ALL {
            if configuration == Configuration::CanterelAlone && metric == Metric::Novelty {
                continue;
            }
            presque = presque
                .measured(configuration, metric, 1.0)
                .expect("lecture finie");
        }
    }

    let Coverage::Partial { missing } = presque.coverage() else {
        panic!("une lecture manque et la comparaison se dit complète");
    };
    assert_eq!(
        missing,
        vec![(Configuration::CanterelAlone, Metric::Novelty)]
    );
}

// ---------------------------------------------------------------------------------------------
// Ce qu'une lecture refuse d'être
// ---------------------------------------------------------------------------------------------

/// `NaN` ne se compare à rien : il rendrait le classement **muet** plutôt que faux, ce qui est pire
/// — un classement faux se remarque, un classement qui élit toujours le premier venu non.
#[test]
fn une_lecture_qui_n_est_pas_un_nombre_fini_est_refusee() {
    for valeur in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            Comparison::new().measured(Configuration::SingleAgent, Metric::Cost, valeur),
            Err(BenchmarkError::NotFinite {
                configuration: Configuration::SingleAgent,
                metric: Metric::Cost
            })
        );
    }
}

#[test]
fn une_lecture_se_relit() {
    let comparaison = Comparison::new()
        .measured(Configuration::CanterelAlone, Metric::Diversity, 0.42)
        .expect("lecture finie");
    assert_eq!(
        comparaison.reading(Configuration::CanterelAlone, Metric::Diversity),
        Some(0.42)
    );
    assert_eq!(
        comparaison.reading(Configuration::CanterelAlone, Metric::Cost),
        None
    );
}
