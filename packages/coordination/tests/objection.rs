//! Test de sortie de W15.d — **les trois garanties de l'item.**
//!
//! 1. Une décision de coordination offre ses quatre cibles : décision, déclencheur, politique,
//!    périmètre.
//! 2. Aucune fonction ne convertit une objection de coordination en `ObjectionTarget` ni l'inverse.
//! 3. Aucun trait générique ne factorise les deux familles.
//!
//! Les garanties 2 et 3 ne se testent pas d'ici, et c'est le sujet : **elles ne peuvent pas
//! s'écrire ici**. La sixième frontière interdit à ce crate d'importer `packages/graph`, donc ce
//! fichier ne pourrait pas nommer `ObjectionTarget` même pour affirmer qu'il ne le convertit pas.
//! Un test qui dirait « la conversion n'existe pas » sans pouvoir voir l'autre famille
//! n'affirmerait rien.
//!
//! Elles sont tenues par la **septième frontière** — « aucun fichier ne voit les deux familles
//! d'objection à la fois » — vérifiée par `npm run check:boundaries`, avec la fixture
//! `tests/boundaries/fixtures/imports/objection-families-converted` qui écrit la conversion
//! exprès et exige que la garde la trouve. C'est la seule place d'où l'absence est vérifiable :
//! la règle 6 empêchant chaque crate de voir l'autre, une conversion ne peut naître que dans un
//! troisième fichier, et c'est celui-là qu'il faut regarder.
//!
//! Ce que ce fichier tient, c'est que la famille de coordination **existe séparément** et dit ce
//! qu'elle doit dire.

use locus_coordination::{Contestable, ObjectedTo, Objection, ObjectionError, Remedy};
use locus_protocol::{Id, IdKind, Timestamp, id::provisional::Decision as DecisionKind};

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

/// Une décision de recomposition d'équipe, déclenchée par un désaccord de revue (§14.5).
fn decision() -> Contestable {
    Contestable::declare(
        id::<DecisionKind>(1),
        "review_disagreement",
        &["agt_alice", "agt_bob"],
    )
    .expect("la décision dit son déclencheur et son périmètre")
}

// ---------------------------------------------------------------------------------------------
// 1. Quatre cibles, et elles ne disent pas la même chose
// ---------------------------------------------------------------------------------------------

#[test]
fn a_decision_offers_its_four_targets() {
    let targets = decision().targets();
    assert_eq!(targets.len(), 4);
    assert!(targets.contains(&ObjectedTo::Decision));
    assert!(targets.contains(&ObjectedTo::Policy));
    assert!(targets.contains(&ObjectedTo::Perimeter));
    assert!(targets.contains(&ObjectedTo::Trigger {
        trigger: "review_disagreement".to_owned()
    }));
}

/// Ce que §7.6 établit dans l'autre domaine, et qui vaut ici mot pour mot : fondre les cibles
/// ferait perdre **ce qu'il faut corriger**.
///
/// Objecter au déclencheur demande d'établir un fait ; objecter à la politique demande de la
/// reprendre, le fait étant admis ; objecter au périmètre demande de le restreindre, la politique
/// étant admise. Trois corrections qui n'ont rien à voir. Une seule « objection à la décision »
/// rendrait la réponse indéterminée.
#[test]
fn each_target_asks_for_a_different_correction() {
    let contestable = decision();
    let remedies = [
        (ObjectedTo::Decision, Remedy::ReopenTheDecision),
        (
            ObjectedTo::Trigger {
                trigger: "review_disagreement".to_owned(),
            },
            Remedy::EstablishTheTrigger,
        ),
        (ObjectedTo::Policy, Remedy::RevisitThePolicy),
        (ObjectedTo::Perimeter, Remedy::NarrowThePerimeter),
    ];

    let mut seen = Vec::new();
    for (target, expected) in remedies {
        let objection = Objection::raise(&contestable, target, "le fait n'a pas eu lieu", "alice")
            .expect("objection valide");
        assert_eq!(objection.remedy(), expected);
        seen.push(objection.remedy());
    }
    seen.dedup();
    assert_eq!(
        seen.len(),
        4,
        "quatre cibles, quatre corrections distinctes"
    );
}

/// Le déclencheur est **nommé**, pas générique.
///
/// « Le déclencheur est faux » sans dire lequel obligerait à retrouver dans le dossier ce qui avait
/// été invoqué, et personne ne le fait. Objecter au déclencheur d'une autre décision est refusé
/// plutôt que consigné : le dossier porterait une contestation sans objet.
#[test]
fn objecting_to_a_trigger_that_was_never_invoked_is_refused() {
    let error = Objection::raise(
        &decision(),
        ObjectedTo::Trigger {
            trigger: "budget_exceeded".to_owned(),
        },
        "ce déclencheur n'a jamais été invoqué",
        "alice",
    )
    .expect_err("la décision a été déclenchée par un désaccord de revue");
    assert!(matches!(error, ObjectionError::NoSuchTarget { .. }));
}

// ---------------------------------------------------------------------------------------------
// Ce qu'une objection doit porter
// ---------------------------------------------------------------------------------------------

/// Une objection sans motif ne se répond pas ; une objection anonyme ne se discute avec personne.
#[test]
fn an_objection_says_why_and_who() {
    for (because, raised_by) in [("", "alice"), ("  ", "alice"), ("le fait est faux", " ")] {
        let error = Objection::raise(&decision(), ObjectedTo::Policy, because, raised_by)
            .expect_err("motif ou auteur vide");
        assert!(matches!(error, ObjectionError::EmptyField { .. }));
    }
}

/// Une décision qui ne dit pas sur qui elle porte rend l'objection de périmètre sans cible.
#[test]
fn a_decision_says_what_it_bears_on() {
    assert!(matches!(
        Contestable::declare(id::<DecisionKind>(1), "review_disagreement", &[])
            .expect_err("périmètre vide"),
        ObjectionError::EmptyPerimeter
    ));
    assert!(matches!(
        Contestable::declare(id::<DecisionKind>(1), "  ", &["agt_alice"])
            .expect_err("déclencheur vide"),
        ObjectionError::EmptyField { field: "trigger" }
    ));
}

/// Une objection reste attachée à la décision qu'elle vise.
///
/// C'est ce qui fait de l'histoire de l'organisation un dossier plutôt qu'une suite de remarques :
/// l'objection se relit à côté de la décision, comme une objection épistémique se relit à côté de
/// l'inférence.
#[test]
fn an_objection_stays_attached_to_its_decision() {
    let contestable = decision();
    let objection = Objection::raise(
        &contestable,
        ObjectedTo::Perimeter,
        "bob n'était pas concerné",
        "alice",
    )
    .expect("objection valide");
    assert_eq!(objection.decision(), contestable.decision());
    assert_eq!(objection.raised_by(), "alice");
    assert_eq!(objection.because(), "bob n'était pas concerné");
}

/// Les noms des cibles sont ceux de ce domaine, pas ceux de l'autre.
///
/// `premise`, `rule`, `scope` et `inference` sont le vocabulaire du graphe épistémique. Les
/// réemployer ici ferait croire à une famille commune — et c'est par le vocabulaire qu'une
/// unification recommence.
#[test]
fn the_names_belong_to_this_domain() {
    let slugs: Vec<&str> = decision().targets().iter().map(ObjectedTo::slug).collect();
    assert_eq!(slugs, ["decision", "trigger", "policy", "perimeter"]);
    for borrowed in ["premise", "rule", "scope", "inference"] {
        assert!(
            !slugs.contains(&borrowed),
            "« {borrowed} » est le vocabulaire de l'autre domaine"
        );
    }
}
