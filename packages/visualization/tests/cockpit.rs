//! Test de sortie de W17.e — **les deux garanties de l'item.**
//!
//! 1. Une sélection dans une vue désigne le même agent dans les trois autres.
//! 2. Un geste de canvas rend une commande que **rien n'applique sur place**, et aucun chemin de
//!    type ne permet à une vue d'écrire.

use locus_protocol::{Id, Timestamp, id::Agent};
use locus_visualization::{Cockpit, CockpitError, Pane, gesture};

/// Le source sans ses commentaires — ce que le module **fait**, pas ce qu'il explique.
fn code_of(source: &str) -> String {
    source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn agent(seed: u8) -> Id<Agent> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::<Agent>::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

// ---------------------------------------------------------------------------------------------
// 1. Quatre vues, une seule sélection
// ---------------------------------------------------------------------------------------------

#[test]
fn the_four_panes_of_the_cockpit_are_there_under_their_names() {
    assert_eq!(
        Pane::ALL.iter().map(|pane| pane.slug()).collect::<Vec<_>>(),
        ["plan", "live", "trace", "epistemic"]
    );
}

/// **Le test qui porte la synchronisation.**
///
/// Sélectionner depuis n'importe laquelle des quatre vues désigne le même agent dans les quatre.
/// Ce n'est pas un mécanisme de notification : le cockpit ne détient qu'un champ, donc il n'existe
/// aucun chemin par lequel deux vues divergent.
#[test]
fn selecting_in_one_pane_designates_the_same_agent_in_the_other_three() {
    for origin in Pane::ALL {
        let mut cockpit = Cockpit::new();
        cockpit.select(origin, agent(7));

        for pane in Pane::ALL {
            let seen = cockpit
                .selection_in(pane)
                .unwrap_or_else(|| panic!("{pane} doit voir la sélection faite depuis {origin}"));
            assert_eq!(
                seen.agent(),
                agent(7),
                "sélection depuis {origin}, lue dans {pane}"
            );
        }
    }
}

/// L'origine se conserve — elle aide à relire une session — mais **elle ne change pas** ce que les
/// autres vues montrent.
#[test]
fn the_origin_is_recorded_without_changing_what_the_others_show() {
    let mut cockpit = Cockpit::new();
    let selection = cockpit.select(Pane::Trace, agent(3));

    assert_eq!(selection.origin(), Pane::Trace);
    assert_eq!(
        cockpit
            .selection_in(Pane::Plan)
            .expect("le plan voit la même")
            .origin(),
        Pane::Trace,
        "la vue qui lit n'est pas celle qui a désigné, et le journal doit pouvoir le dire"
    );
}

/// Une seconde sélection remplace la première **partout à la fois**.
///
/// Avec quatre états, c'est exactement ici que la dérive s'installe : une vue garderait l'ancienne
/// sélection, resterait cohérente avec elle-même, et personne ne le verrait.
#[test]
fn a_second_selection_replaces_the_first_everywhere_at_once() {
    let mut cockpit = Cockpit::new();
    cockpit.select(Pane::Plan, agent(1));
    cockpit.select(Pane::Epistemic, agent(2));

    for pane in Pane::ALL {
        assert_eq!(
            cockpit.selection_in(pane).expect("une sélection").agent(),
            agent(2),
            "{pane} montre encore l'ancienne"
        );
    }
}

#[test]
fn a_cockpit_without_a_selection_shows_none_in_all_four() {
    let mut cockpit = Cockpit::new();
    for pane in Pane::ALL {
        assert!(cockpit.selection_in(pane).is_none());
    }
    cockpit.select(Pane::Live, agent(1));
    cockpit.clear();
    for pane in Pane::ALL {
        assert!(cockpit.selection_in(pane).is_none(), "{pane}");
    }
}

// ---------------------------------------------------------------------------------------------
// 2. Le canvas produit une commande, jamais une écriture
// ---------------------------------------------------------------------------------------------

/// Un geste rend une **demande** : un verbe, un sujet, une origine, et rien qui l'applique.
#[test]
fn a_canvas_gesture_yields_a_request_that_carries_no_way_to_apply_itself() {
    let requested = gesture(Pane::Live, "team.modify", agent(5)).expect("un geste qui demande");

    assert_eq!(requested.verb(), "team.modify");
    assert_eq!(requested.subject(), agent(5));
    assert_eq!(requested.origin(), Pane::Live);
}

#[test]
fn a_gesture_that_asks_for_nothing_is_refused() {
    assert_eq!(
        gesture(Pane::Plan, "  ", agent(1)).expect_err("aucun verbe"),
        CockpitError::EmptyVerb
    );
}

/// **Vérification par l'absence, sur le source du module.**
///
/// Un geste qui écrirait ferait du canvas un chemin de mutation parallèle à la command API — sans
/// approbation, sans trace et sans `expected_revision`. Le test nomme les mots qu'on serait tenté
/// d'employer, pour que l'échec dise lequel est apparu.
#[test]
fn nothing_in_the_cockpit_applies_commits_or_writes() {
    let source = code_of(include_str!("../src/cockpit.rs"));
    for forbidden in [
        "fn apply",
        "fn commit",
        "fn write",
        "fn mutate",
        "fn save",
        "fn execute",
    ] {
        assert!(
            !source.contains(forbidden),
            "« {forbidden} » ferait du canvas un chemin de mutation parallèle à la command API"
        );
    }
}

/// Et le cockpit ne connaît aucun type d'écriture.
///
/// Il ne dépend ni de l'event store, ni du domaine de coordination : une vue qui saurait nommer une
/// commande de mutation finirait par en composer une.
///
/// Le test lit le **code**, commentaires retirés : expliquer pourquoi un terme est absent le fait
/// apparaître dans la prose, et un test qui confondrait les deux interdirait d'écrire la raison.
#[test]
fn the_cockpit_names_no_writing_type() {
    let source = code_of(include_str!("../src/cockpit.rs"));
    for absent in [
        "locus_event_store",
        "locus_coordination",
        "CommandEnvelope",
        "expected_revision",
    ] {
        assert!(
            !source.contains(absent),
            "« {absent} » n'a rien à faire dans une vue"
        );
    }
}
