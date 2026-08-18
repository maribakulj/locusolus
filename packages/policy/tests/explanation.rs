//! Test de sortie de W14.c — **une alternative rejetée sans motif n'en est pas une, et un override
//! n'efface pas ce que le moteur avait conclu.**
//!
//! §20.5 énumère huit facettes. Sept se remplissent naturellement en construisant la décision ; les
//! **alternatives rejetées** sont la seule qu'il faut décider de garder, parce qu'elles n'existent
//! nulle part une fois la décision prise. C'est aussi celle qui rend une décision contestable :
//! savoir qu'un moteur a choisi A ne dit rien tant qu'on ignore s'il a même envisagé B.

use locus_policy::{
    Explanation, ExplanationError, Facet, Facts, Outcome, Override, Policy, Rejected, Rule, Verb,
};

fn faits() -> Facts {
    Facts::new()
        .with("object_type", "Claim")
        .with("impact", "high")
}

fn politique() -> Policy {
    Policy::new()
        .with(
            Rule::declare(
                "p_major_claim_review",
                3,
                10,
                &[("impact", "high")],
                Verb::RequireApproval {
                    approver_role: "logical-reviewer".to_owned(),
                },
            )
            .expect("règle déclarée"),
        )
        .expect("règle ajoutée")
}

fn expose() -> Explanation {
    Explanation::of(&faits(), &politique().evaluate(&faits()))
}

// ---------------------------------------------------------------------------------------------
// Les huit facettes de §20.5
// ---------------------------------------------------------------------------------------------

#[test]
fn les_huit_facettes_existent_sous_leur_nom() {
    let slugs: Vec<&str> = Facet::ALL
        .iter()
        .map(|facet: &Facet| facet.slug())
        .collect();
    assert_eq!(
        slugs,
        vec![
            "policy-and-version",
            "inputs",
            "fired-rules",
            "scores-and-uncertainty",
            "rejected-alternatives",
            "approvals",
            "overrides",
            "produced-events"
        ]
    );
}

/// L'exposé porte ce que §20.5 demande : les faits, la règle et sa version, la décision, les
/// approbations, les événements produits.
#[test]
fn un_expose_porte_les_facettes_qu_il_a() {
    let expose = expose().approved_by("human:pi-marie").producing("evt-0001");

    assert_eq!(expose.facts(), &faits());
    assert_eq!(expose.evaluation().trace()[0].rule, "p_major_claim_review");
    assert_eq!(expose.evaluation().trace()[0].version, 3);
    assert_eq!(expose.approvals(), &["human:pi-marie".to_owned()]);
    assert_eq!(expose.events(), &["evt-0001".to_owned()]);
}

/// Deux facettes manquantes sont toujours un manquement : sans **données d'entrée** la décision ne
/// se rejoue pas, sans **règle déclenchée** elle ne s'explique par rien. Les autres peuvent être
/// légitimement vides — crier au manquement sur un exposé complet apprend à ignorer l'alarme.
#[test]
fn seules_deux_facettes_vides_sont_toujours_un_manquement() {
    let complet = expose();
    assert!(
        complet.gaps().is_empty(),
        "un exposé sans override ni alternative reste complet : {:?}",
        complet.gaps()
    );

    let sans_faits = Explanation::of(&Facts::new(), &politique().evaluate(&Facts::new()));
    let manques = sans_faits.gaps();
    assert!(manques.contains(&Facet::Inputs));
    assert!(manques.contains(&Facet::FiredRules));
}

// ---------------------------------------------------------------------------------------------
// Les alternatives rejetées
// ---------------------------------------------------------------------------------------------

/// Le cœur de W14.c. « Nous avons envisagé B » sans dire pourquoi ne se conteste pas : il n'y a rien
/// à objecter. Et une case cochée dans un rapport d'explicabilité est pire que son absence, parce
/// qu'elle donne l'apparence d'un examen qui n'a pas eu lieu.
#[test]
fn une_alternative_rejetee_sans_motif_est_refusee() {
    assert_eq!(
        Rejected::considered("allow-sans-revue", "   "),
        Err(ExplanationError::EmptyField {
            field: "rejected.because"
        })
    );
    assert_eq!(
        Rejected::considered("  ", "hors budget"),
        Err(ExplanationError::EmptyField {
            field: "rejected.option"
        })
    );
}

#[test]
fn les_alternatives_rejetees_gardent_leur_motif() {
    let expose = expose()
        .rejecting(
            Rejected::considered("allow", "l'impact est haut et §20.3 exige deux relecteurs")
                .expect("motivée"),
        )
        .rejecting(
            Rejected::considered(
                "deny",
                "aucune règle ne l'exige, et refuser bloquerait le programme",
            )
            .expect("motivée"),
        );

    assert_eq!(expose.rejected().len(), 2);
    assert_eq!(expose.rejected()[0].option(), "allow");
    assert!(expose.rejected()[0].because().contains("§20.3"));
    assert!(expose.rejected()[1].because().contains("bloquerait"));
}

// ---------------------------------------------------------------------------------------------
// Les overrides
// ---------------------------------------------------------------------------------------------

/// §20.2 exige de « conserver les overrides humains ». **Conserver** veut dire que la décision
/// automatique reste lisible à côté : les fondre effacerait ce que le moteur avait conclu, et
/// personne ne pourrait plus distinguer une erreur corrigée d'une garde contournée.
#[test]
fn un_override_n_efface_pas_ce_que_le_moteur_avait_conclu() {
    let machine = Outcome::Decided {
        verb: Verb::RequireApproval {
            approver_role: "logical-reviewer".to_owned(),
        },
        by: "p_major_claim_review".to_owned(),
    };

    let expose = expose().overridden_by(
        Override::recorded(
            "human:pi-marie",
            Outcome::Decided {
                verb: Verb::Allow,
                by: "override".to_owned(),
            },
            "le relecteur logique est en congé, la revue est reportée et tracée",
        )
        .expect("motivé"),
    );

    // Ce que le moteur avait dit reste lisible…
    assert_eq!(*expose.machine_outcome(), machine);
    // …et ce qui s'applique est l'override.
    assert_eq!(
        *expose.effective_outcome(),
        Outcome::Decided {
            verb: Verb::Allow,
            by: "override".to_owned()
        }
    );
    assert_ne!(expose.machine_outcome(), expose.effective_outcome());
}

/// Sans override, les deux coïncident : `effective_outcome` n'invente rien quand personne n'a
/// contredit le moteur.
#[test]
fn sans_override_les_deux_verdicts_coincident() {
    let expose = expose();
    assert_eq!(expose.machine_outcome(), expose.effective_outcome());
    assert!(expose.overridden().is_none());
}

/// Un override anonyme ou muet est indiscernable d'un défaut du moteur — et c'est précisément ce
/// qu'il ne faut pas confondre.
#[test]
fn un_override_anonyme_ou_muet_est_refuse() {
    assert_eq!(
        Override::recorded("  ", Outcome::NoRule, "parce que"),
        Err(ExplanationError::EmptyField {
            field: "override.by"
        })
    );
    assert_eq!(
        Override::recorded("human:x", Outcome::NoRule, ""),
        Err(ExplanationError::EmptyField {
            field: "override.because"
        })
    );
}

#[test]
fn l_override_garde_son_auteur_et_son_motif() {
    let expose = expose().overridden_by(
        Override::recorded("human:pi-marie", Outcome::NoRule, "revue reportée").expect("motivé"),
    );
    let overridden = expose.overridden().expect("consigné");
    assert_eq!(overridden.by(), "human:pi-marie");
    assert_eq!(overridden.because(), "revue reportée");
}

/// Un override sur un **conflit** est le cas où la conservation compte le plus : c'est le moment où
/// un humain tranche ce que le moteur a refusé de trancher, et où il faut pouvoir relire ce refus.
#[test]
fn un_override_sur_un_conflit_conserve_le_conflit() {
    let contradictoire = Policy::new()
        .with(Rule::declare("oui", 1, 7, &[("impact", "high")], Verb::Allow).expect("déclarée"))
        .expect("ajoutée")
        .with(Rule::declare("non", 1, 7, &[("impact", "high")], Verb::Deny).expect("déclarée"))
        .expect("ajoutée");

    let evaluation = contradictoire.evaluate(&faits());
    let expose = Explanation::of(&faits(), &evaluation).overridden_by(
        Override::recorded(
            "human:pi-marie",
            Outcome::Decided {
                verb: Verb::Deny,
                by: "override".to_owned(),
            },
            "en cas de doute sur un claim à fort impact, on refuse",
        )
        .expect("motivé"),
    );

    assert!(matches!(expose.machine_outcome(), Outcome::Conflict { .. }));
    assert_eq!(expose.evaluation().trace().len(), 2);
}
