//! Second test de sortie de W7.f — **une stratégie qui optimise l'indicateur sans produire de
//! connaissance ne paie pas**, et **une branche jamais criblée n'a pas de valeur.**
//!
//! # Ce que ce fichier doit éviter de faire
//!
//! Poser à la main des indicateurs plus bas pour la branche qui triche. Ce serait supposer la
//! conclusion. Ici, les deux branches passent par la **même** règle d'indicateur — celle que la
//! manœuvre vise — et le test constate d'abord que **la manœuvre marche** : la branche qui triche a
//! un meilleur `V(b)` brut. C'est seulement la pénalité de §13.6 qui renverse l'ordre.

use locus_portfolio::{
    BranchActivity, ClaimRecord, Indicators, LexicalSimilarity, Screening, Thresholds, ValueError,
    Weights, screen, value,
};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn count(number: usize) -> f64 {
    f64::from(u32::try_from(number).expect("les fixtures restent petites"))
}

fn claim(statement: &str, evidence_count: usize) -> ClaimRecord {
    ClaimRecord {
        statement: statement.to_owned(),
        evidence_count,
        declared_confidence: 50,
        held_up: None,
    }
}

/// L'indicateur **naïf** — celui qu'une branche a intérêt à optimiser.
///
/// « Gain d'information » compté au nombre de revendications produites. C'est une mesure paresseuse
/// et c'est exactement pour cela qu'elle sert ici : §13.6 existe parce que les indicateurs faciles
/// à mesurer sont faciles à fabriquer.
fn naive(activity: &BranchActivity) -> Indicators {
    Indicators {
        calibrated_progress: 0.5,
        impact: 2.0,
        information_gain: count(activity.claims.len()),
        reusability: 1.0,
        diversity: 1.0,
        negative_value: 0.0,
        marginal_cost: 1.0,
        portfolio_similarity: 0.0,
        error_correlation: 0.0,
        dependency_fragility: 0.0,
    }
}

fn crible(activity: &BranchActivity) -> Screening {
    screen(activity, &LexicalSimilarity, Thresholds::default())
}

/// Quatre revendications, toutes étayées.
fn honest() -> BranchActivity {
    BranchActivity {
        claims: vec![
            claim("le lemme 2 borne la variance", 3),
            claim("la borne est atteinte au point critique", 2),
            claim("le contre-exemple de Weil ne s'applique pas ici", 4),
            claim("la méthode échoue en dimension impaire", 3),
        ],
        ..BranchActivity::default()
    }
}

/// Trente revendications, aucune étayée.
fn gamed() -> BranchActivity {
    BranchActivity {
        claims: (0..30)
            .map(|index| claim(&format!("observation numéro {index}"), 0))
            .collect(),
        ..BranchActivity::default()
    }
}

// ---------------------------------------------------------------------------------------------
// La manœuvre marche sur l'indicateur, et ne paie pas
// ---------------------------------------------------------------------------------------------

/// Le test qui porte le sprint. Il se lit en trois temps, et les trois comptent : la manœuvre est
/// **efficace** sur l'indicateur, elle est **efficace** sur `V(b)` brut, et elle est **perdante**
/// une fois §13.6 appliqué. Retirer le deuxième temps rendrait le test creux — on ne saurait plus si
/// la pénalité a renversé quelque chose ou si la manœuvre était mauvaise dès le départ.
#[test]
fn une_strategie_qui_optimise_l_indicateur_sans_produire_de_connaissance_ne_paie_pas() {
    let honest_activity = honest();
    let gamed_activity = gamed();

    let honest_indicators = naive(&honest_activity);
    let gamed_indicators = naive(&gamed_activity);

    assert!(
        gamed_indicators.information_gain > honest_indicators.information_gain,
        "premier temps : la manœuvre gonfle bien l'indicateur visé"
    );

    let honest_value = value(
        &honest_indicators,
        &Weights::default(),
        &crible(&honest_activity),
    )
    .expect("valorisation valide");
    let gamed_value = value(
        &gamed_indicators,
        &Weights::default(),
        &crible(&gamed_activity),
    )
    .expect("valorisation valide");

    assert!(
        gamed_value.gross() > honest_value.gross(),
        "deuxième temps : sans §13.6, tricher paie — brut {} contre {}",
        gamed_value.gross(),
        honest_value.gross()
    );
    assert!(
        gamed_value.value() < honest_value.value(),
        "troisième temps : avec §13.6, tricher coûte — {} contre {}",
        gamed_value.value(),
        honest_value.value()
    );
}

/// La branche honnête ne paie **rien** : la pénalité n'est pas une taxe générale, sinon elle
/// déplacerait toutes les valeurs sans rien départager.
#[test]
fn une_branche_criblee_propre_ne_paie_aucune_penalite() {
    let activity = honest();
    let valuation = value(&naive(&activity), &Weights::default(), &crible(&activity))
        .expect("valorisation valide");

    assert_eq!(valuation.pressure(), 0);
    assert!(valuation.penalty().abs() < f64::EPSILON);
    assert!((valuation.value() - valuation.gross()).abs() < f64::EPSILON);
}

/// Le piège qu'une pénalité multiplicative sur la valeur nette aurait creusé : une branche de valeur
/// **négative** se rapprocherait de zéro, donc s'améliorerait, et tricher paierait précisément sur
/// les branches qu'il faut abandonner. La pénalité porte sur les termes positifs, donc elle ne peut
/// jamais remonter une valeur.
#[test]
fn la_penalite_ne_remonte_jamais_une_branche() {
    let activity = gamed();
    let mut indicators = naive(&activity);
    indicators.marginal_cost = 500.0; // une branche ruineuse

    let valuation =
        value(&indicators, &Weights::default(), &crible(&activity)).expect("valorisation valide");

    assert!(valuation.gross() < 0.0, "la branche est déjà mauvaise");
    assert!(
        valuation.value() <= valuation.gross(),
        "et le criblage ne l'améliore pas : {} contre {}",
        valuation.value(),
        valuation.gross()
    );
}

// ---------------------------------------------------------------------------------------------
// Une branche jamais criblée n'a pas de valeur
// ---------------------------------------------------------------------------------------------

/// L'ordre de `docs/10` sous sa forme durable. Un merge écrasé ne garde pas l'ordre des commits ;
/// le type le garde : `value` exige un `Screening`, et `Screening` n'a pas d'autre constructeur que
/// `screen`. Ce test dit la conséquence observable — toute valorisation transporte les seuils du
/// criblage qui l'a précédée, donc aucune n'a pu se passer de lui.
#[test]
fn toute_valorisation_porte_la_trace_du_criblage_qui_l_a_precedee() {
    let activity = honest();
    let severe = Thresholds {
        max_unsupported_percent: 0,
        ..Thresholds::default()
    };

    let indulgent = screen(&activity, &LexicalSimilarity, Thresholds::default());
    let strict = screen(&activity, &LexicalSimilarity, severe);

    assert_eq!(indulgent.thresholds().max_unsupported_percent, 50);
    assert_eq!(strict.thresholds().max_unsupported_percent, 0);

    // Et le même jeu d'indicateurs valorisé sous deux criblages ne donne pas le même nombre : le
    // criblage n'est pas décoratif, il entre dans le résultat.
    let indicators = naive(&activity);
    let first = value(&indicators, &Weights::default(), &indulgent).expect("valorisation valide");
    let second = value(&indicators, &Weights::default(), &strict).expect("valorisation valide");
    assert!(first.value() >= second.value());
}

// ---------------------------------------------------------------------------------------------
// §13.4 : « tous ses paramètres et entrées sont enregistrés »
// ---------------------------------------------------------------------------------------------

/// Une valeur dont on ne peut plus retrouver les paramètres est un chiffre, pas une décision — et
/// elle serait incontestable au mauvais sens du mot.
#[test]
fn une_valorisation_transporte_ses_parametres_et_ses_entrees() {
    let activity = honest();
    let weights = Weights {
        lambda: 3.0,
        ..Weights::default()
    };
    let indicators = naive(&activity);

    let valuation = value(&indicators, &weights, &crible(&activity)).expect("valorisation valide");

    assert_eq!(valuation.weights(), &weights);
    assert_eq!(valuation.indicators(), &indicators);
}

/// §13.4 donne la forme de la formule, pas ses nombres. Le défaut neutre dit « aucune pondération
/// n'a été décidée », ce qui est vrai — des coefficients réglés inventés ici passeraient pour la
/// spec parce qu'ils seraient écrits en Rust.
#[test]
fn les_coefficients_par_defaut_sont_neutres_et_ne_pretendent_a_rien() {
    let weights = Weights::default();
    for coefficient in [
        weights.lambda,
        weights.mu,
        weights.nu,
        weights.xi,
        weights.alpha,
        weights.beta,
        weights.gamma,
        weights.delta,
    ] {
        assert!((coefficient - 1.0).abs() < f64::EPSILON);
    }
}

/// Un poids change le résultat — sans quoi « politique par défaut » ne voudrait rien dire.
#[test]
fn un_coefficient_change_la_valeur() {
    let activity = honest();
    let indicators = naive(&activity);
    let screening = crible(&activity);

    let neutral = value(&indicators, &Weights::default(), &screening).expect("valorisation valide");
    let weighted = value(
        &indicators,
        &Weights {
            lambda: 10.0,
            ..Weights::default()
        },
        &screening,
    )
    .expect("valorisation valide");

    assert!(weighted.value() > neutral.value());
}

// ---------------------------------------------------------------------------------------------
// Ce qui n'est pas un nombre
// ---------------------------------------------------------------------------------------------

/// Un `NaN` ne se compare à rien, pas même à lui-même : une branche qui en porte un ne serait ni
/// meilleure ni pire que les autres, donc invisible au tri — et rien ne le dirait. W7.g trie sur ce
/// nombre, ce qui rend le refus nécessaire ici plutôt que là-bas.
#[test]
fn une_entree_non_finie_est_refusee() {
    let activity = honest();
    let mut indicators = naive(&activity);
    indicators.impact = f64::NAN;

    let refused = value(&indicators, &Weights::default(), &crible(&activity))
        .expect_err("un NaN ne se valorise pas");
    assert!(matches!(refused, ValueError::NotFinite { .. }));

    let mut infinite = naive(&activity);
    infinite.information_gain = f64::INFINITY;
    assert!(matches!(
        value(&infinite, &Weights::default(), &crible(&activity)),
        Err(ValueError::NotFinite { .. })
    ));
}

/// Un coefficient non fini est refusé au même titre : la politique n'est pas plus fiable que les
/// entrées.
#[test]
fn un_coefficient_non_fini_est_refuse() {
    let activity = honest();
    let weights = Weights {
        alpha: f64::NAN,
        ..Weights::default()
    };
    assert!(matches!(
        value(&naive(&activity), &weights, &crible(&activity)),
        Err(ValueError::NotFinite { .. })
    ));
}

/// §13.4 dit `p_s` « calibrée ». Hors de [0, 1], elle multiplie l'impact au lieu de le pondérer, et
/// une branche à `p_s = 3` triplerait son impact sans rien prouver.
#[test]
fn une_probabilite_hors_de_zero_un_est_refusee() {
    let activity = honest();
    let mut indicators = naive(&activity);
    indicators.calibrated_progress = 3.0;

    assert!(matches!(
        value(&indicators, &Weights::default(), &crible(&activity)),
        Err(ValueError::NotAProbability { .. })
    ));

    indicators.calibrated_progress = -0.1;
    assert!(matches!(
        value(&indicators, &Weights::default(), &crible(&activity)),
        Err(ValueError::NotAProbability { .. })
    ));

    // Les bornes, elles, sont admises : une certitude et une impossibilité sont des calibrations.
    indicators.calibrated_progress = 0.0;
    assert!(value(&indicators, &Weights::default(), &crible(&activity)).is_ok());
    indicators.calibrated_progress = 1.0;
    assert!(value(&indicators, &Weights::default(), &crible(&activity)).is_ok());
}

/// Deux valorisations des mêmes entrées donnent le **même** nombre, au bit près. L'ordre des
/// additions est fixé dans le code pour cela : W7.g triera sur ce nombre, et un tri qui dépend de
/// l'ordre d'évaluation ne serait pas reproductible.
#[test]
fn deux_valorisations_des_memes_entrees_donnent_le_meme_nombre() {
    let activity = honest();
    let indicators = naive(&activity);

    let first =
        value(&indicators, &Weights::default(), &crible(&activity)).expect("valorisation valide");
    let second =
        value(&indicators, &Weights::default(), &crible(&activity)).expect("valorisation valide");

    assert_eq!(first.value().to_bits(), second.value().to_bits());
    assert_eq!(first, second);
}
