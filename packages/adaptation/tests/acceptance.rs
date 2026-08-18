//! Test de sortie de W18.e — **les trois garanties de l'item.**
//!
//! 1. Le taux ne compte que des annulations **humaines**, et une annulation par le système ne le
//!    fait pas monter.
//! 2. Une adaptation que personne n'a regardée est déclarée **hors mesure**, jamais comptée comme
//!    acceptée — le silence n'est pas un accord.
//! 3. Une adaptation d'auteur humain n'entre pas dans la mesure.

use locus_adaptation::acceptance::{CancellationRate, Fate, Loop, Record, reviewers};
use locus_coordination::Author;
use locus_protocol::{Id, IdKind, Timestamp, id::Agent};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn robot(seed: u8) -> Author {
    Author::Agent(id::<Agent>(seed))
}

fn cancelled(by: &str) -> Fate {
    Fate::CancelledByHuman { by: by.to_owned() }
}

fn kept(by: &str) -> Fate {
    Fate::ReviewedAndKept { by: by.to_owned() }
}

fn expired() -> Fate {
    Fate::CancelledBySystem {
        reason: "fenêtre expirée".to_owned(),
    }
}

fn agentic(fate: Fate) -> Record {
    Record::of(robot(1), Loop::Slow, fate)
}

// ---------------------------------------------------------------------------------------------
// 1. Seules les annulations humaines comptent
// ---------------------------------------------------------------------------------------------

/// Les quatre issues sont nommées, et une seule fait monter le taux.
#[test]
fn only_one_of_the_four_fates_raises_the_rate() {
    assert_eq!(
        Fate::NAMES.to_vec(),
        vec![
            "cancelled_by_human",
            "reviewed_and_kept",
            "unreviewed",
            "cancelled_by_system",
        ]
    );
    let sample = [
        cancelled("usr-marie"),
        kept("usr-marie"),
        Fate::Unreviewed,
        expired(),
    ];
    let named: Vec<&str> = sample.iter().map(Fate::slug).collect();
    assert_eq!(named, Fate::NAMES.to_vec());

    // Deux sont un jugement humain, deux ne le sont pas.
    let judgements: Vec<bool> = sample.iter().map(Fate::is_a_human_judgement).collect();
    assert_eq!(judgements, vec![true, true, false, false]);
}

/// Le taux compte les annulations humaines sur les prononcés humains.
#[test]
fn the_rate_is_human_cancellations_over_human_judgements() {
    let records = vec![
        agentic(cancelled("usr-marie")),
        agentic(kept("usr-marie")),
        agentic(kept("usr-jean")),
        agentic(kept("usr-jean")),
    ];
    let rate = CancellationRate::over(&records);
    let ratio = rate.ratio().expect("quatre personnes se sont prononcées");
    assert_eq!(ratio.cancelled(), 1);
    assert_eq!(ratio.measured(), 4);
    assert!((ratio.value() - 0.25).abs() < f64::EPSILON);
    assert_eq!(ratio.to_string(), "1/4");
}

/// **La garantie centrale.** Une annulation par le système ne fait pas monter le taux.
///
/// La machine n'a pas de préférence : compter son annulation ferait monter le taux sans qu'aucun
/// jugement ait eu lieu, et on chercherait un désaccord humain là où une fenêtre a expiré.
#[test]
fn a_system_cancellation_does_not_raise_the_rate() {
    let without = vec![agentic(cancelled("usr-marie")), agentic(kept("usr-jean"))];
    let with_system = vec![
        agentic(cancelled("usr-marie")),
        agentic(kept("usr-jean")),
        agentic(expired()),
        agentic(expired()),
        agentic(expired()),
    ];

    let before = CancellationRate::over(&without)
        .ratio()
        .expect("deux prononcés");
    let after = CancellationRate::over(&with_system)
        .ratio()
        .expect("toujours deux prononcés");
    assert_eq!(before, after, "trois annulations système n'ont rien changé");

    // Mais elles sont comptées, à part.
    let rate = CancellationRate::over(&with_system);
    assert_eq!(rate.cancelled_by_system(), 3);
    assert_eq!(rate.out_of_measure(), 3);
}

// ---------------------------------------------------------------------------------------------
// 2. Le silence n'est pas un accord
// ---------------------------------------------------------------------------------------------

/// Une adaptation non regardée est hors mesure, ni au numérateur ni au dénominateur.
#[test]
fn an_unreviewed_adaptation_is_out_of_measure() {
    let records = vec![
        agentic(cancelled("usr-marie")),
        agentic(kept("usr-jean")),
        agentic(Fate::Unreviewed),
        agentic(Fate::Unreviewed),
        agentic(Fate::Unreviewed),
    ];
    let rate = CancellationRate::over(&records);
    let ratio = rate.ratio().expect("deux prononcés");
    assert_eq!(ratio.cancelled(), 1);
    assert_eq!(
        ratio.measured(),
        2,
        "les trois non regardées ne sont pas au dénominateur"
    );
    assert_eq!(rate.unreviewed(), 3);
    assert_eq!(rate.out_of_measure(), 3);
}

/// **La faute que la forme prévient.** Sans aucun prononcé, il n'y a pas de taux — pas un taux nul.
///
/// Rendre `0.0` se lirait « aucun humain n'a jamais annulé », donc une acceptation parfaite, tirée de
/// zéro observation — et au moment précis où le déploiement a perdu son seuil humain. « Pas
/// vérifié » n'est jamais « réussi ».
#[test]
fn no_human_judgement_means_no_rate_at_all() {
    let records = vec![
        agentic(Fate::Unreviewed),
        agentic(Fate::Unreviewed),
        agentic(expired()),
    ];
    let rate = CancellationRate::over(&records);
    assert_eq!(rate.ratio(), None);
    assert_eq!(rate.measured(), 0);
    assert_eq!(rate.out_of_measure(), 3);
}

/// Un ensemble vide ne rend pas non plus de taux.
#[test]
fn an_empty_history_yields_no_rate() {
    let rate = CancellationRate::over(&[]);
    assert_eq!(rate.ratio(), None);
    assert_eq!(rate.out_of_measure(), 0);
    assert_eq!(rate, CancellationRate::default());
}

// ---------------------------------------------------------------------------------------------
// 3. Les adaptations humaines n'entrent pas dans la mesure
// ---------------------------------------------------------------------------------------------

/// Ce qu'un opérateur écrit lui-même ne fait pas varier le score du système.
#[test]
fn a_human_authored_adaptation_is_not_measured() {
    let human = Author::Human("usr-marie".to_owned());
    let records = vec![
        agentic(cancelled("usr-jean")),
        agentic(kept("usr-jean")),
        Record::of(human.clone(), Loop::Slow, cancelled("usr-jean")),
        Record::of(human.clone(), Loop::Fast, kept("usr-jean")),
        Record::of(human, Loop::Slow, Fate::Unreviewed),
    ];
    let rate = CancellationRate::over(&records);
    let ratio = rate.ratio().expect("deux prononcés agentiques");
    assert_eq!(ratio.cancelled(), 1);
    assert_eq!(ratio.measured(), 2);
    assert_eq!(rate.human_authored(), 3);
    // Elles ne sont pas non plus « hors mesure » : elles ne sont pas dans la mesure du tout.
    assert_eq!(rate.out_of_measure(), 0);
}

/// `is_agentic` tranche sur l'auteur, jamais sur le sort.
#[test]
fn agentic_is_decided_by_the_author() {
    assert!(agentic(Fate::Unreviewed).is_agentic());
    assert!(
        !Record::of(
            Author::Human("usr-marie".to_owned()),
            Loop::Fast,
            cancelled("usr-jean"),
        )
        .is_agentic()
    );
}

// ---------------------------------------------------------------------------------------------
// Les deux boucles, et qui a jugé
// ---------------------------------------------------------------------------------------------

/// Les deux boucles se mesurent séparément.
///
/// Les additionner ferait disparaître le signal de la boucle lente dans celui de la rapide, qui est
/// bien plus nombreuse.
#[test]
fn the_two_loops_are_measured_separately() {
    let records = vec![
        Record::of(robot(1), Loop::Fast, kept("usr-jean")),
        Record::of(robot(1), Loop::Fast, kept("usr-jean")),
        Record::of(robot(1), Loop::Fast, kept("usr-jean")),
        Record::of(robot(2), Loop::Slow, cancelled("usr-marie")),
    ];

    let fast = CancellationRate::over_loop(&records, Loop::Fast)
        .ratio()
        .expect("trois prononcés");
    assert_eq!((fast.cancelled(), fast.measured()), (0, 3));

    let slow = CancellationRate::over_loop(&records, Loop::Slow)
        .ratio()
        .expect("un prononcé");
    assert_eq!((slow.cancelled(), slow.measured()), (1, 1));

    // Ensemble, la boucle lente disparaît presque.
    let both = CancellationRate::over(&records).ratio().expect("quatre");
    assert_eq!((both.cancelled(), both.measured()), (1, 4));
    assert_eq!(Loop::ALL.len(), 2);
}

/// Le taux dit aussi **combien de personnes** le portent.
///
/// Cent adaptations toutes jugées par la même personne ne sont pas cent observations. La liste sert
/// à le savoir, pas à noter qui que ce soit — §14.6 : la réputation « ne doit pas devenir un score
/// social unique ».
#[test]
fn the_rate_says_how_many_people_carry_it() {
    let records = vec![
        agentic(cancelled("usr-marie")),
        agentic(kept("usr-marie")),
        agentic(kept("usr-jean")),
        agentic(Fate::Unreviewed),
        agentic(expired()),
    ];
    let people = reviewers(&records);
    assert_eq!(people.len(), 2);
    assert!(people.contains("usr-marie"));
    assert!(people.contains("usr-jean"));
}

/// Un taux ne se stocke pas en flottant : `1/2` et `500/1000` ne sont pas la même preuve.
#[test]
fn a_ratio_keeps_both_of_its_numbers() {
    let thin = vec![agentic(cancelled("usr-marie")), agentic(kept("usr-jean"))];
    let mut thick = Vec::new();
    for index in 0..500 {
        thick.push(agentic(cancelled(&format!("usr-{index}"))));
        thick.push(agentic(kept(&format!("usr-{index}"))));
    }

    let thin = CancellationRate::over(&thin).ratio().expect("deux");
    let thick = CancellationRate::over(&thick).ratio().expect("mille");
    assert!((thin.value() - thick.value()).abs() < f64::EPSILON);
    assert_ne!(thin, thick, "le même nombre, pas la même preuve");
    assert_eq!(thin.to_string(), "1/2");
    assert_eq!(thick.to_string(), "500/1000");
}
