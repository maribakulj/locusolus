//! Test de sortie de W12.b — **un compteur non relevé n'est pas un seuil atteint.**
//!
//! §29.6 fixe neuf exigences : huit en liste, plus les sept jours de la phrase qui les introduit.
//! Une campagne de six jours qui aurait atteint les huit puces n'est pas celle que §29.6 demande, et
//! faire vivre la durée ailleurs que dans la liste reviendrait à l'oublier.
//!
//! Les trois façons de ne pas tenir restent séparées parce qu'elles n'appellent pas le même geste :
//! prolonger la campagne, l'instrumenter, ou corriger le produit.

use locus_evaluation::{Campaign, Endurance, EnduranceError, Measure, Requirement};

fn tenue() -> Campaign {
    let mut campagne = Campaign::new();
    for requirement in Requirement::ALL {
        campagne = match requirement.minimum() {
            Some(minimum) => campagne
                .counted(requirement, minimum)
                .expect("exigence comptée"),
            None => campagne
                .held(requirement, true)
                .expect("invariant constaté"),
        };
    }
    campagne
}

// ---------------------------------------------------------------------------------------------
// Les neuf exigences de §29.6
// ---------------------------------------------------------------------------------------------

#[test]
fn les_seuils_sont_ceux_que_29_6_chiffre() {
    assert_eq!(Requirement::DurationDays.minimum(), Some(7));
    assert_eq!(Requirement::ConcurrentWorkstreams.minimum(), Some(10));
    assert_eq!(Requirement::Branches.minimum(), Some(30));
    assert_eq!(Requirement::AgentInstances.minimum(), Some(100));
    assert_eq!(Requirement::Tasks.minimum(), Some(5_000));
    assert_eq!(Requirement::Events.minimum(), Some(250_000));
    // §29.6 veut des redémarrages et des pertes « réguliers » sans chiffrer : zéro est la seule
    // valeur dont on soit sûr qu'elle ne les exerce pas.
    assert_eq!(Requirement::Restarts.minimum(), Some(1));
    assert_eq!(Requirement::WorkerLosses.minimum(), Some(1));
    // Et la reprise ne se compte pas.
    assert_eq!(Requirement::RecoveryIntact.minimum(), None);
    assert_eq!(Requirement::ALL.len(), 9);
}

#[test]
fn une_campagne_qui_atteint_tout_est_endurante() {
    assert_eq!(tenue().endurance(), Endurance::Held);
    assert_eq!(tenue().endurance().to_string(), "endurante");
}

/// Chaque seuil, manqué d'une unité et lui seul, fait tomber la campagne — et le verdict le
/// **nomme** avec ce qui a été atteint. Les éprouver un par un est ce qui empêche qu'un seuil
/// devienne décoratif sans que personne ne s'en aperçoive.
#[test]
fn chaque_seuil_manque_d_une_unite_fait_tomber_la_campagne() {
    for requirement in Requirement::ALL {
        let Some(minimum) = requirement.minimum() else {
            continue;
        };
        let campagne = tenue()
            .counted(requirement, minimum - 1)
            .expect("exigence comptée");

        let Endurance::Fell { short, .. } = campagne.endurance() else {
            panic!(
                "{requirement} à {} et la campagne se dit endurante",
                minimum - 1
            );
        };
        assert_eq!(short.len(), 1, "{requirement}");
        assert_eq!(short[0].requirement, requirement);
        assert_eq!(short[0].reached, minimum - 1);
        assert_eq!(short[0].minimum, minimum);
        assert!(
            campagne
                .endurance()
                .to_string()
                .contains(requirement.slug())
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Trois causes, trois gestes
// ---------------------------------------------------------------------------------------------

/// Le cœur de W12.b. Un seuil non relevé demande d'**instrumenter** ; un seuil sous la barre demande
/// de **prolonger**. Les fondre en un seul « échec » ferait tourner la campagne plus longtemps quand
/// c'est la mesure qui manquait, donc pour rien.
#[test]
fn non_releve_et_sous_la_barre_ne_se_confondent_pas() {
    let mut sans_mesure = Campaign::new();
    for requirement in Requirement::ALL {
        if requirement == Requirement::Events {
            continue;
        }
        sans_mesure = match requirement.minimum() {
            Some(minimum) => sans_mesure.counted(requirement, minimum).expect("comptée"),
            None => sans_mesure.held(requirement, true).expect("constatée"),
        };
    }

    let Endurance::Fell {
        short, unmeasured, ..
    } = sans_mesure.endurance()
    else {
        panic!("un seuil non relevé et la campagne se dit endurante");
    };
    assert!(
        short.is_empty(),
        "rien n'est sous la barre : rien n'a été mesuré"
    );
    assert_eq!(unmeasured, vec![Requirement::Events]);
    assert!(
        sans_mesure
            .endurance()
            .to_string()
            .contains("events : non relevé")
    );
}

/// Et un invariant violé demande de **corriger le produit** : tourner plus longtemps n'y changera
/// rien, et le ranger avec les seuils ferait croire le contraire.
#[test]
fn un_invariant_viole_ne_se_range_pas_avec_les_seuils() {
    let campagne = tenue()
        .held(Requirement::RecoveryIntact, false)
        .expect("invariant constaté");

    let Endurance::Fell {
        short,
        unmeasured,
        violated,
    } = campagne.endurance()
    else {
        panic!("reprise cassée et la campagne se dit endurante");
    };
    assert!(short.is_empty());
    assert!(unmeasured.is_empty());
    assert_eq!(violated, vec![Requirement::RecoveryIntact]);
    assert!(
        campagne
            .endurance()
            .to_string()
            .contains("recovery-intact : violé")
    );
}

/// Une campagne neuve n'a rien relevé : les neuf sont non mesurées, aucune n'est « sous la barre ».
/// Compter zéro pour ce que personne n'a compté ferait passer une absence d'instrumentation pour
/// une campagne ratée — et on chercherait la panne au mauvais endroit.
#[test]
fn une_campagne_neuve_n_a_rien_releve_et_ne_rate_rien() {
    let Endurance::Fell {
        short, unmeasured, ..
    } = Campaign::new().endurance()
    else {
        panic!("une campagne vide se dit endurante");
    };
    assert!(short.is_empty());
    assert_eq!(unmeasured.len(), 9);
}

// ---------------------------------------------------------------------------------------------
// Ce qu'un relevé refuse d'être
// ---------------------------------------------------------------------------------------------

/// « La reprise s'est bien passée 4 fois » ne dit rien de la cinquième, et c'est exactement la
/// question que §29.6 pose.
#[test]
fn la_reprise_ne_se_compte_pas() {
    assert_eq!(
        Campaign::new().counted(Requirement::RecoveryIntact, 4),
        Err(EnduranceError::NotCounted {
            requirement: Requirement::RecoveryIntact
        })
    );
}

/// Et l'inverse : répondre « oui » à « avez-vous eu 5 000 tâches ? » ne dit pas combien.
#[test]
fn un_seuil_ne_se_constate_pas_par_oui_ou_non() {
    assert_eq!(
        Campaign::new().held(Requirement::Tasks, true),
        Err(EnduranceError::NotAnInvariant {
            requirement: Requirement::Tasks
        })
    );
}

#[test]
fn un_releve_se_relit() {
    let campagne = tenue();
    assert_eq!(
        campagne.measure(Requirement::Events),
        Some(Measure::Counted(250_000))
    );
    assert_eq!(
        campagne.measure(Requirement::RecoveryIntact),
        Some(Measure::Held(true))
    );
    assert_eq!(Campaign::new().measure(Requirement::Tasks), None);
}
