//! Test de sortie de W14.b — **deux principals, jamais un ; et cinq motifs de refus, jamais un.**
//!
//! §20.4 : « les actions d'un agent sont attribuées **au principal agentique et à la délégation
//! humaine ou institutionnelle qui les autorise**. »
//!
//! Un journal qui ne retiendrait que l'agent ferait porter à un programme une décision qu'un humain
//! a autorisée. Un journal qui ne retiendrait que le délégant effacerait qui a agi. Les deux erreurs
//! sont symétriques et toutes deux invisibles à la relecture.

use locus_policy::{Authorisation, Delegation, DelegationError, Refusal, Request};

fn delegation() -> Delegation {
    Delegation::grant(
        "human:pi-marie",
        "agent:prover-7",
        &["run-proof", "read-corpus"],
        "programme/lemme-3",
        1_000,
        2,
        100,
        200,
        true,
    )
    .expect("délégation valide")
}

fn demande() -> Request {
    Request {
        agent: "agent:prover-7".to_owned(),
        action: "run-proof".to_owned(),
        scope: "programme/lemme-3".to_owned(),
        budget: 500,
        confidentiality: 1,
        at: 150,
    }
}

// ---------------------------------------------------------------------------------------------
// Deux principals
// ---------------------------------------------------------------------------------------------

/// Le cœur de W14.b. L'attribution porte les deux, et rien ne les résume — c'est la même règle que
/// les deux verdicts de §19, pour la même raison : chacun des deux raccourcis efface une moitié de
/// la responsabilité, et les deux moitiés servent à des questions différentes.
#[test]
fn une_action_autorisee_est_attribuee_aux_deux_principals() {
    let Authorisation::Granted(attribution) = delegation().authorises(&demande()) else {
        panic!("une demande conforme a été refusée");
    };
    assert_eq!(attribution.agent, "agent:prover-7");
    assert_eq!(attribution.authorised_by, "human:pi-marie");
}

/// C'est le **délégant** qui autorise, pas le délégataire. Attribuer au délégataire ferait signer
/// l'agent par lui-même, ce qui est précisément ce qu'une délégation existe pour éviter.
#[test]
fn c_est_le_delegant_qui_autorise_pas_le_delegataire() {
    let Authorisation::Granted(attribution) = delegation().authorises(&demande()) else {
        panic!("une demande conforme a été refusée");
    };
    assert_ne!(attribution.authorised_by, attribution.agent);
    assert_ne!(attribution.authorised_by, "agent:prover-7");
}

/// L'agent attribué est celui qui a demandé, pas le délégataire nommé dans la délégation : un
/// sous-agent qui agit sous une délégation garde son identité, et confondre les deux ferait porter
/// ses actes à un autre.
#[test]
fn l_agent_attribue_est_celui_qui_a_demande() {
    let mut sous_agent = demande();
    sous_agent.agent = "agent:prover-7/child-2".to_owned();
    let Authorisation::Granted(attribution) = delegation().authorises(&sous_agent) else {
        panic!("une demande conforme a été refusée");
    };
    assert_eq!(attribution.agent, "agent:prover-7/child-2");
    assert_eq!(attribution.authorised_by, "human:pi-marie");
}

// ---------------------------------------------------------------------------------------------
// Cinq bornes, cinq motifs
// ---------------------------------------------------------------------------------------------

/// Chaque borne franchie seule refuse, et **nomme** son motif. Les fondre en « non autorisé » ferait
/// chercher au mauvais endroit dans quatre cas sur cinq : demander une autre portée, faire étendre
/// la délégation, réduire la demande, ou la renouveler.
#[test]
fn chaque_borne_franchie_seule_refuse_avec_son_motif() {
    let hors_portee = Request {
        scope: "programme/autre".to_owned(),
        ..demande()
    };
    assert_eq!(
        delegation().authorises(&hors_portee),
        Authorisation::Refused(Refusal::OutOfScope {
            requested: "programme/autre".to_owned(),
            granted: "programme/lemme-3".to_owned()
        })
    );

    let action_absente = Request {
        action: "publish".to_owned(),
        ..demande()
    };
    assert_eq!(
        delegation().authorises(&action_absente),
        Authorisation::Refused(Refusal::ActionNotGranted {
            action: "publish".to_owned()
        })
    );

    let trop_cher = Request {
        budget: 1_001,
        ..demande()
    };
    assert_eq!(
        delegation().authorises(&trop_cher),
        Authorisation::Refused(Refusal::OverBudget {
            requested: 1_001,
            ceiling: 1_000
        })
    );

    let trop_confidentiel = Request {
        confidentiality: 3,
        ..demande()
    };
    assert_eq!(
        delegation().authorises(&trop_confidentiel),
        Authorisation::Refused(Refusal::OverConfidentiality {
            requested: 3,
            ceiling: 2
        })
    );

    let trop_tard = Request {
        at: 200,
        ..demande()
    };
    assert!(matches!(
        delegation().authorises(&trop_tard),
        Authorisation::Refused(Refusal::Expired { .. })
    ));
}

/// Les plafonds sont des plafonds : la valeur exacte passe, la suivante non. Une inégalité stricte
/// de trop rendrait inutilisable la dernière unité de budget accordée.
#[test]
fn un_plafond_atteint_exactement_est_autorise() {
    let au_plafond = Request {
        budget: 1_000,
        confidentiality: 2,
        ..demande()
    };
    assert!(delegation().authorises(&au_plafond).is_granted());
}

/// La fenêtre est fermée à droite, ouverte à gauche : `valid_from` autorise, `expires_at` non. Une
/// borne d'expiration qui autoriserait encore serait une délégation qui dure un instant de trop, et
/// cet instant est exactement celui où quelqu'un croit qu'elle a cessé.
#[test]
fn la_fenetre_de_validite_inclut_son_debut_et_exclut_sa_fin() {
    let au_debut = Request {
        at: 100,
        ..demande()
    };
    assert!(delegation().authorises(&au_debut).is_granted());

    let juste_avant = Request {
        at: 99,
        ..demande()
    };
    assert!(!delegation().authorises(&juste_avant).is_granted());

    let a_la_fin = Request {
        at: 200,
        ..demande()
    };
    assert!(!delegation().authorises(&a_la_fin).is_granted());

    let juste_avant_la_fin = Request {
        at: 199,
        ..demande()
    };
    assert!(delegation().authorises(&juste_avant_la_fin).is_granted());
}

// ---------------------------------------------------------------------------------------------
// La révocation
// ---------------------------------------------------------------------------------------------

#[test]
fn une_delegation_revoquee_n_autorise_plus_rien() {
    let revoquee = delegation().revoke().expect("révocable");
    assert!(revoquee.is_revoked());
    assert_eq!(
        revoquee.authorises(&demande()),
        Authorisation::Refused(Refusal::Revoked)
    );
}

/// La révocation prime sur tout le reste : une demande par ailleurs parfaitement conforme est
/// refusée pour révocation, pas pour autre chose. Un motif secondaire ferait croire qu'en corrigeant
/// la demande on retrouverait l'autorisation.
#[test]
fn la_revocation_prime_sur_les_autres_motifs() {
    let revoquee = delegation().revoke().expect("révocable");
    let par_ailleurs_fautive = Request {
        budget: 99_999,
        scope: "programme/autre".to_owned(),
        at: 1,
        ..demande()
    };
    assert_eq!(
        revoquee.authorises(&par_ailleurs_fautive),
        Authorisation::Refused(Refusal::Revoked)
    );
}

/// `revocable` est un champ du texte, donc il décide de quelque chose. Accepter la révocation en
/// apparence et continuer d'autoriser serait la pire des deux réponses : le délégant croirait avoir
/// agi.
#[test]
fn une_delegation_irrevocable_refuse_la_revocation() {
    let irrevocable = Delegation::grant(
        "human:pi-marie",
        "agent:archive",
        &["read-corpus"],
        "programme/lemme-3",
        10,
        0,
        0,
        1_000,
        false,
    )
    .expect("délégation valide");

    assert_eq!(
        irrevocable.clone().revoke(),
        Err(DelegationError::NotRevocable)
    );
    // Et elle continue d'autoriser, ce qui est cohérent avec le refus : rien n'a changé.
    let demande = Request {
        agent: "agent:archive".to_owned(),
        action: "read-corpus".to_owned(),
        scope: "programme/lemme-3".to_owned(),
        budget: 1,
        confidentiality: 0,
        at: 5,
    };
    assert!(irrevocable.authorises(&demande).is_granted());
}

// ---------------------------------------------------------------------------------------------
// Ce qu'une délégation refuse d'être
// ---------------------------------------------------------------------------------------------

/// Une délégation sans action passerait pour une autorisation alors qu'elle n'en est pas une — et
/// c'est le genre de ligne qu'on écrit en croyant préparer le terrain.
#[test]
fn une_delegation_sans_action_est_refusee() {
    assert_eq!(
        Delegation::grant("h", "a", &[], "s", 1, 0, 0, 1, true),
        Err(DelegationError::NoAction)
    );
}

/// Une fenêtre vide autoriserait pendant zéro instant tout en ayant l'air d'une délégation valide.
#[test]
fn une_fenetre_vide_ou_inversee_est_refusee() {
    assert_eq!(
        Delegation::grant("h", "a", &["x"], "s", 1, 0, 100, 100, true),
        Err(DelegationError::EmptyWindow {
            valid_from: 100,
            expires_at: 100
        })
    );
    assert_eq!(
        Delegation::grant("h", "a", &["x"], "s", 1, 0, 200, 100, true),
        Err(DelegationError::EmptyWindow {
            valid_from: 200,
            expires_at: 100
        })
    );
}

#[test]
fn un_delegant_un_delegataire_ou_une_portee_vide_sont_refuses() {
    for (champ, args) in [
        ("delegator", ("  ", "a", "s")),
        ("delegate", ("h", " ", "s")),
        ("scope", ("h", "a", "   ")),
    ] {
        assert_eq!(
            Delegation::grant(args.0, args.1, &["x"], args.2, 1, 0, 0, 1, true),
            Err(DelegationError::EmptyField { field: champ })
        );
    }
}
