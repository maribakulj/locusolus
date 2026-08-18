//! Test de sortie de W14.a — **une décision sans trace n'existe pas, et la priorité est déclarée.**
//!
//! §20.2 : « séparer faits d'entrée et décision ; produire une trace d'évaluation ; détecter les
//! conflits de politiques ; définir une priorité explicite ; être déterministe à entrées
//! identiques. »
//!
//! Trois de ces exigences se tiennent l'une l'autre. Les faits séparés rendent le déterminisme
//! vrai ; le déterminisme rend la décision rejouable ; la trace rend le rejeu compréhensible. Un
//! moteur qui perdrait l'une des trois garderait l'air de marcher.

use locus_policy::{Facts, Outcome, Policy, PolicyError, Rule, Verb};

fn regle(id: &str, priority: u32, when: &[(&str, &str)], verb: Verb) -> Rule {
    Rule::declare(id, 3, priority, when, verb).expect("règle déclarée")
}

fn faits() -> Facts {
    Facts::new()
        .with("object_type", "Claim")
        .with("impact", "high")
}

// ---------------------------------------------------------------------------------------------
// Les cinq verbes de §20.2
// ---------------------------------------------------------------------------------------------

#[test]
fn les_cinq_verbes_existent_sous_leur_nom() {
    let verbes = [
        Verb::Allow,
        Verb::Deny,
        Verb::Modify {
            constraint: "sandbox=strict".to_owned(),
        },
        Verb::RequireApproval {
            approver_role: "logical-reviewer".to_owned(),
        },
        Verb::RequireTasks {
            tasks: vec!["reproduce".to_owned()],
        },
    ];
    let slugs: Vec<&str> = verbes.iter().map(Verb::slug).collect();
    assert_eq!(
        slugs,
        vec![
            "allow",
            "deny",
            "modify",
            "require_approval",
            "require_tasks"
        ]
    );
}

/// `modify` laisse passer **autre chose** que ce qui a été demandé. Le confondre avec `allow` ferait
/// croire qu'une contrainte imposée est une permission simple, et l'appelant appliquerait la demande
/// d'origine.
#[test]
fn seul_allow_laisse_passer_la_demande_telle_quelle() {
    assert!(Verb::Allow.permits_as_requested());
    for autre in [
        Verb::Deny,
        Verb::Modify {
            constraint: "x".to_owned(),
        },
        Verb::RequireApproval {
            approver_role: "x".to_owned(),
        },
        Verb::RequireTasks { tasks: vec![] },
    ] {
        assert!(!autre.permits_as_requested(), "{autre}");
    }
}

// ---------------------------------------------------------------------------------------------
// Déterminisme et séparation des faits
// ---------------------------------------------------------------------------------------------

/// §20.2 : « déterministe à entrées identiques ». Sans lui, rejouer une décision pour la comprendre
/// en produit une autre — et la décision cesse d'être contestable.
#[test]
fn deux_evaluations_des_memes_faits_rendent_la_meme_chose() {
    let politique = Policy::new()
        .with(regle("p1", 10, &[("impact", "high")], Verb::Deny))
        .expect("règle ajoutée")
        .with(regle("p2", 5, &[("object_type", "Claim")], Verb::Allow))
        .expect("règle ajoutée");

    let premiere = politique.evaluate(&faits());
    let seconde = politique.evaluate(&faits());
    assert_eq!(premiere, seconde);
    assert_eq!(premiere.trace(), seconde.trace());
}

/// L'ordre dans lequel les faits ont été posés n'entre pas dans la décision : ce sont les mêmes
/// faits. Deux appelants qui construisent le même contexte différemment doivent obtenir le même
/// verdict, sans quoi la politique dépend de la façon d'appeler.
#[test]
fn l_ordre_des_faits_ne_change_rien() {
    let politique = Policy::new()
        .with(regle(
            "p1",
            10,
            &[("impact", "high"), ("object_type", "Claim")],
            Verb::Deny,
        ))
        .expect("règle ajoutée");

    let un_sens = Facts::new()
        .with("object_type", "Claim")
        .with("impact", "high");
    let autre_sens = Facts::new()
        .with("impact", "high")
        .with("object_type", "Claim");

    assert_eq!(un_sens, autre_sens);
    assert_eq!(
        politique.evaluate(&un_sens),
        politique.evaluate(&autre_sens)
    );
}

// ---------------------------------------------------------------------------------------------
// La priorité est déclarée, jamais héritée de l'ordre
// ---------------------------------------------------------------------------------------------

/// Le cœur de W14.a. Trancher par l'ordre de déclaration ferait d'un réordonnancement de fichier un
/// changement de comportement — et personne ne relit un diff de réordonnancement comme tel.
#[test]
fn l_ordre_de_declaration_ne_decide_rien() {
    let permissive = regle("permissive", 1, &[("impact", "high")], Verb::Allow);
    let stricte = regle("stricte", 9, &[("impact", "high")], Verb::Deny);

    let dans_un_sens = Policy::new()
        .with(permissive.clone())
        .expect("ajoutée")
        .with(stricte.clone())
        .expect("ajoutée");
    let dans_l_autre = Policy::new()
        .with(stricte)
        .expect("ajoutée")
        .with(permissive)
        .expect("ajoutée");

    let attendu = Outcome::Decided {
        verb: Verb::Deny,
        by: "stricte".to_owned(),
    };
    assert_eq!(*dans_un_sens.evaluate(&faits()).outcome(), attendu);
    assert_eq!(*dans_l_autre.evaluate(&faits()).outcome(), attendu);
}

/// À priorité égale et verbes contraires, le conflit est **rendu**, pas résolu. Un moteur qui
/// choisirait tout de même déciderait à la place de qui a écrit les règles, et le ferait en silence.
#[test]
fn deux_regles_contradictoires_a_priorite_egale_sont_un_conflit() {
    let politique = Policy::new()
        .with(regle("oui", 7, &[("impact", "high")], Verb::Allow))
        .expect("ajoutée")
        .with(regle("non", 7, &[("impact", "high")], Verb::Deny))
        .expect("ajoutée");

    let Outcome::Conflict { priority, rules } = politique.evaluate(&faits()).outcome().clone()
    else {
        panic!("deux verbes contraires à priorité égale et le moteur a tranché");
    };
    assert_eq!(priority, 7);
    assert_eq!(rules, vec!["non".to_owned(), "oui".to_owned()]);
}

/// Deux règles de même priorité qui disent la **même** chose ne sont pas un conflit : elles sont
/// d'accord, et refuser de décider parce qu'elles sont deux serait un faux positif qui pousserait à
/// supprimer une règle utile.
#[test]
fn deux_regles_d_accord_a_priorite_egale_ne_sont_pas_un_conflit() {
    let politique = Policy::new()
        .with(regle("a", 7, &[("impact", "high")], Verb::Deny))
        .expect("ajoutée")
        .with(regle("b", 7, &[("object_type", "Claim")], Verb::Deny))
        .expect("ajoutée");

    assert_eq!(
        *politique.evaluate(&faits()).outcome(),
        Outcome::Decided {
            verb: Verb::Deny,
            by: "a".to_owned()
        }
    );
}

/// Un conflit à priorité basse ne masque pas une décision à priorité haute : c'est le rang le plus
/// élevé qui tranche, et lui seul est examiné pour le désaccord.
#[test]
fn un_conflit_de_rang_inferieur_ne_bloque_pas_une_decision_superieure() {
    let politique = Policy::new()
        .with(regle("haute", 9, &[("impact", "high")], Verb::Deny))
        .expect("ajoutée")
        .with(regle("basse-oui", 2, &[("impact", "high")], Verb::Allow))
        .expect("ajoutée")
        .with(regle("basse-non", 2, &[("impact", "high")], Verb::Deny))
        .expect("ajoutée");

    assert_eq!(
        *politique.evaluate(&faits()).outcome(),
        Outcome::Decided {
            verb: Verb::Deny,
            by: "haute".to_owned()
        }
    );
}

// ---------------------------------------------------------------------------------------------
// La trace
// ---------------------------------------------------------------------------------------------

/// §20.5 demande « les règles déclenchées ». La trace porte **toutes** celles qui ont matché, pas
/// seulement celle qui a tranché : savoir ce qui a failli s'appliquer est la moitié de ce qui rend
/// une décision contestable.
#[test]
fn la_trace_porte_toutes_les_regles_declenchees_pas_seulement_la_gagnante() {
    let politique = Policy::new()
        .with(regle("gagnante", 9, &[("impact", "high")], Verb::Deny))
        .expect("ajoutée")
        .with(regle(
            "perdante",
            3,
            &[("object_type", "Claim")],
            Verb::Allow,
        ))
        .expect("ajoutée")
        .with(regle("hors-sujet", 9, &[("impact", "low")], Verb::Allow))
        .expect("ajoutée");

    let evaluation = politique.evaluate(&faits());
    let noms: Vec<&str> = evaluation
        .trace()
        .iter()
        .map(|fired| fired.rule.as_str())
        .collect();

    assert_eq!(noms, vec!["gagnante", "perdante"]);
    assert!(!noms.contains(&"hors-sujet"), "elle n'a pas matché");
}

/// §20.5 demande « politique et version ». Une trace qui dirait seulement le nom laisserait relire
/// une règle qui a changé depuis, donc reconstituer une décision qui n'a pas eu lieu.
#[test]
fn la_trace_porte_la_version_de_chaque_regle() {
    let politique = Policy::new()
        .with(Rule::declare("p", 42, 5, &[("impact", "high")], Verb::Deny).expect("déclarée"))
        .expect("ajoutée");

    let evaluation = politique.evaluate(&faits());
    assert_eq!(evaluation.trace()[0].version, 42);
    assert_eq!(evaluation.trace()[0].priority, 5);
}

/// Même un conflit a sa trace : c'est là qu'elle sert le plus, puisqu'il faut savoir quelles règles
/// se contredisent pour en corriger une.
#[test]
fn un_conflit_porte_sa_trace() {
    let politique = Policy::new()
        .with(regle("oui", 7, &[("impact", "high")], Verb::Allow))
        .expect("ajoutée")
        .with(regle("non", 7, &[("impact", "high")], Verb::Deny))
        .expect("ajoutée");

    assert_eq!(politique.evaluate(&faits()).trace().len(), 2);
}

/// Et l'absence de règle aussi : une trace vide est un fait, pas un trou. Elle dit que rien n'a
/// matché, ce qui se corrige en écrivant une règle.
#[test]
fn aucune_regle_applicable_se_dit_et_ne_vaut_pas_allow() {
    let politique = Policy::new()
        .with(regle("p", 5, &[("impact", "low")], Verb::Allow))
        .expect("ajoutée");

    let evaluation = politique.evaluate(&faits());
    assert_eq!(*evaluation.outcome(), Outcome::NoRule);
    assert!(evaluation.trace().is_empty());
    // `NoRule` n'est pas `allow` : personne n'a autorisé quoi que ce soit, et c'est à l'appelant de
    // décider ce qu'il fait d'un silence.
    assert_ne!(
        *evaluation.outcome(),
        Outcome::Decided {
            verb: Verb::Allow,
            by: "p".to_owned()
        }
    );
}

// ---------------------------------------------------------------------------------------------
// Ce qu'une règle refuse d'être
// ---------------------------------------------------------------------------------------------

/// Une règle sans condition s'applique à tout. Ce n'est presque jamais voulu, et ça ne se voit pas
/// en relisant — d'où le refus à la déclaration.
#[test]
fn une_regle_sans_condition_est_refusee() {
    assert_eq!(
        Rule::declare("p", 1, 1, &[], Verb::Allow),
        Err(PolicyError::EmptyField { field: "rule.when" })
    );
}

#[test]
fn une_regle_sans_identifiant_est_refusee() {
    assert_eq!(
        Rule::declare("  ", 1, 1, &[("a", "b")], Verb::Allow),
        Err(PolicyError::EmptyField { field: "rule.id" })
    );
}

/// Deux règles du même nom rendraient la trace ambiguë — et c'est la trace qui sert à contester.
#[test]
fn deux_regles_du_meme_identifiant_sont_refusees() {
    assert_eq!(
        Policy::new()
            .with(regle("p", 1, &[("a", "b")], Verb::Allow))
            .expect("ajoutée")
            .with(regle("p", 2, &[("c", "d")], Verb::Deny)),
        Err(PolicyError::DuplicateRule { id: "p".to_owned() })
    );
}

/// Une règle ne se déclenche que si **tous** ses `when` sont posés. Un « ou » implicite ferait
/// s'appliquer une politique de haute gravité à un objet qui n'en relève pas.
#[test]
fn une_regle_exige_toutes_ses_conditions() {
    let politique = Policy::new()
        .with(regle(
            "p",
            5,
            &[("impact", "high"), ("object_type", "Theorem")],
            Verb::Deny,
        ))
        .expect("ajoutée");

    assert_eq!(*politique.evaluate(&faits()).outcome(), Outcome::NoRule);
}
