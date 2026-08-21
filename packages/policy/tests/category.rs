//! Test de sortie de W14.d — **le dry-run est le même calcul, et la liste des seize est close.**
//!
//! §20.2 : « supporter dry-run et simulation ». La faute que cette exigence prévient est courante :
//! un chemin « simulation » écrit à part, qui diverge du chemin réel le jour où l'un des deux est
//! corrigé. La simulation ne dirait alors plus ce que fera le run — la seule chose qu'on lui
//! demande.
//!
//! §20.1 : seize catégories. La liste est close, et c'est ce qui permet de dire qu'un déploiement
//! n'a **aucune** politique de secrets. Le risque n'est pas d'écrire une mauvaise politique de
//! secrets, c'est de n'y pas penser.

use locus_policy::{Category, Coverage, Facts, Outcome, Policy, Rule, Run, Verb};

fn faits() -> Facts {
    Facts::new()
        .with("object_type", "Claim")
        .with("impact", "high")
}

fn politique() -> Policy {
    Policy::new()
        .with(Rule::declare("p_deny", 2, 10, &[("impact", "high")], Verb::Deny).expect("déclarée"))
        .expect("ajoutée")
        .with(
            Rule::declare("p_allow", 1, 3, &[("object_type", "Claim")], Verb::Allow)
                .expect("déclarée"),
        )
        .expect("ajoutée")
}

// ---------------------------------------------------------------------------------------------
// Les seize catégories de §20.1, et l'ajout local qui les suit
// ---------------------------------------------------------------------------------------------

/// **Les seize de §20.1 sont intactes ; `W14.e` en a ajouté une dix-septième.**
///
/// Ce test comptait seize et a échoué en rouge quand la catégorie `Alignment` est entrée — c'est
/// exactement ce qu'on lui demandait de faire. Une liste normative ne s'allonge pas sans que
/// quelqu'un le lise, et le mettre à jour est un acte délibéré, pas un ajustement de confort.
///
/// Il tient désormais **deux** choses au lieu d'une : que les seize premières n'ont bougé ni en
/// nombre, ni en ordre, ni en nom — c'est §20.1, elle ne se réécrit pas — et que ce qui suit est un
/// ajout local assumé. Sans la première moitié, une réécriture de la spec passerait pour un ajout.
#[test]
fn les_seize_categories_existent_sous_leur_nom() {
    const NORMATIVES: usize = 16;
    assert_eq!(
        Category::ALL.len(),
        NORMATIVES + 1,
        "seize de §20.1, plus `alignment`"
    );

    let slugs: Vec<&str> = Category::ALL.iter().map(|c: &Category| c.slug()).collect();
    assert_eq!(slugs[0], "spawn");
    assert_eq!(slugs[7], "secrets");
    assert_eq!(slugs[15], "human-escalation");
    assert_eq!(
        slugs[16], "alignment",
        "l'ajout local vient après, jamais au milieu"
    );

    let mut uniques = slugs.clone();
    uniques.sort_unstable();
    uniques.dedup();
    assert_eq!(uniques.len(), NORMATIVES + 1, "des noms distincts");

    for category in Category::ALL {
        assert_eq!(Category::from_slug(category.slug()), Some(category));
    }
    assert_eq!(Category::from_slug("logging"), None);
}

/// Le résultat qui compte est la liste des **absentes**. Le risque n'est pas d'écrire une mauvaise
/// politique de secrets, c'est de n'y pas penser — et une liste ouverte ne saurait pas le dire.
#[test]
fn la_couverture_nomme_ce_qu_aucune_politique_ne_couvre() {
    let partielle = Coverage::of(&[Category::Budget, Category::Review]);
    assert!(!partielle.is_complete());
    // Quinze depuis `W14.e` : les dix-sept moins les deux couvertes. Le compte suit la liste, et
    // c'est voulu — une couverture qui ignorerait l'ajout local dirait « complète » alors qu'aucune
    // politique d'alignement n'existe.
    assert_eq!(partielle.uncovered().len(), 15);
    assert!(partielle.uncovered().contains(&Category::Alignment));
    assert!(partielle.uncovered().contains(&Category::Secrets));
    assert!(partielle.to_string().contains("secrets"));
}

/// Chacune des seize, absente seule, est signalée. Les éprouver une par une empêche qu'une catégorie
/// devienne facultative sans que personne ne s'en aperçoive.
#[test]
fn chaque_categorie_absente_seule_est_signalee() {
    for absente in Category::ALL {
        let couvertes: Vec<Category> = Category::ALL
            .into_iter()
            .filter(|category| *category != absente)
            .collect();
        let couverture = Coverage::of(&couvertes);
        assert_eq!(couverture.uncovered(), &[absente], "{absente}");
        assert!(couverture.to_string().contains(absente.slug()));
    }
}

#[test]
fn les_seize_couvertes_font_une_couverture_complete() {
    let complete = Coverage::of(&Category::ALL);
    assert!(complete.is_complete());
    assert!(complete.uncovered().is_empty());
    assert!(complete.to_string().contains("§20.1"));
}

// ---------------------------------------------------------------------------------------------
// Le dry-run est le même calcul
// ---------------------------------------------------------------------------------------------

/// Le cœur de W14.d. Un chemin « simulation » écrit à part divergerait du chemin réel le jour où
/// l'un des deux est corrigé, et la simulation ne dirait plus ce que fera le run.
#[test]
fn le_dry_run_rend_exactement_ce_que_rendrait_le_run_reel() {
    let simule = Run::dry(&politique(), &faits());
    let reel = Run::live(&politique(), &faits());

    assert_eq!(simule.would_decide(), reel.outcome());
    assert_eq!(simule.evaluation(), reel.evaluation());
    assert_eq!(*simule.explanation(), reel.explanation());
}

/// Et cela vaut sur tous les cas de figure du moteur, pas seulement celui qui décide : un conflit et
/// une absence de règle sont exactement les états qu'un chemin de simulation bâclé simplifierait.
#[test]
fn le_dry_run_reproduit_aussi_les_conflits_et_les_silences() {
    let contradictoire = Policy::new()
        .with(Rule::declare("oui", 1, 7, &[("impact", "high")], Verb::Allow).expect("déclarée"))
        .expect("ajoutée")
        .with(Rule::declare("non", 1, 7, &[("impact", "high")], Verb::Deny).expect("déclarée"))
        .expect("ajoutée");

    assert!(matches!(
        Run::dry(&contradictoire, &faits()).would_decide(),
        Outcome::Conflict { .. }
    ));
    assert_eq!(
        Run::dry(&contradictoire, &faits()).would_decide(),
        Run::live(&contradictoire, &faits()).outcome()
    );

    let muette = Policy::new()
        .with(Rule::declare("p", 1, 1, &[("impact", "low")], Verb::Allow).expect("déclarée"))
        .expect("ajoutée");
    assert_eq!(*Run::dry(&muette, &faits()).would_decide(), Outcome::NoRule);
    assert_eq!(
        Run::dry(&muette, &faits()).would_decide(),
        Run::live(&muette, &faits()).outcome()
    );
}

/// La simulation porte la trace entière — c'est ce qui la rend utile : on simule pour comprendre ce
/// qui *va* se déclencher, pas seulement ce qui sera décidé.
#[test]
fn la_simulation_porte_la_trace_entiere() {
    let simule = Run::dry(&politique(), &faits());
    let noms: Vec<&str> = simule
        .evaluation()
        .trace()
        .iter()
        .map(|fired| fired.rule.as_str())
        .collect();
    assert_eq!(noms, vec!["p_deny", "p_allow"]);
}

/// Simuler deux fois rend la même chose : le dry-run hérite du déterminisme du moteur, il ne
/// l'affaiblit pas.
#[test]
fn simuler_deux_fois_rend_la_meme_chose() {
    assert_eq!(
        Run::dry(&politique(), &faits()),
        Run::dry(&politique(), &faits())
    );
}

/// Un dry-run n'expose aucun moyen de produire un effet. La garantie n'est pas une discipline
/// d'appel — il n'y a pas de méthode à ne pas appeler : `Simulation` ne rend pas l'`Explanation` par
/// valeur, donc rien ne peut y rattacher d'événements.
#[test]
fn une_simulation_ne_donne_pas_l_expose_a_completer() {
    let simule = Run::dry(&politique(), &faits());
    // On peut le lire…
    assert!(simule.explanation().events().is_empty());
    // …et le seul chemin qui rend l'exposé par valeur, pour y rattacher des événements, part d'un
    // run réel.
    let expose = Run::live(&politique(), &faits())
        .explanation()
        .producing("evt-0001");
    assert_eq!(expose.events(), &["evt-0001".to_owned()]);
}

/// L'exposé d'un run réel porte les faits qui l'ont produit : c'est ce que §20.5 appelle « données
/// d'entrée », et c'est ce qui permet de rejouer la décision.
#[test]
fn l_expose_d_un_run_porte_les_faits_qui_l_ont_produit() {
    let expose = Run::live(&politique(), &faits()).explanation();
    assert_eq!(expose.facts(), &faits());
    assert!(expose.gaps().is_empty());
}
