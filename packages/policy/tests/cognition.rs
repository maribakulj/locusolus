//! Test de sortie de `W25.a` — **la classe de cognition dans la mission.**
//!
//! Trois clauses, celles de la roadmap :
//!
//! 1. une mission déclare une **classe**, jamais un modèle, et un test d'absence refuse tout
//!    identifiant de modèle dans le domaine ;
//! 2. l'affectation classe → modèle est une valeur de politique **versionnée**, visible dans la
//!    trace d'évaluation de §20.5 ;
//! 3. **changer l'affectation ne change aucun type**, et c'est le test qui porte l'item.
//!
//! # Pourquoi la troisième porte l'item
//!
//! L'ADR 0026 appelle cette mesure « la plus actionnable du dossier » — un facteur 7,9 sur le coût
//! total, 22 sur la flotte de workers seule, à qualité identique vérifiée par test caché. Le levier
//! n'est pas le modèle, c'est **l'affectation**, et une affectation qu'on ne peut pas changer sans
//! recompiler n'est pas un levier.

use locus_policy::cognition::{Assignment, AssignmentError};

/// Une affectation versionnée, avec les deux classes que l'ADR nomme.
fn affectation(version: u32, frontier: &str, economy: &str) -> Assignment {
    Assignment::versioned(
        version,
        [
            ("frontier".to_owned(), frontier.to_owned()),
            ("economy".to_owned(), economy.to_owned()),
        ],
    )
    .expect("la fixture est cohérente")
}

// ---------------------------------------------------------------------------------------------
// 2. Versionnée et visible
// ---------------------------------------------------------------------------------------------

/// Une résolution porte la **version** de l'affectation qui a répondu.
///
/// §20.5 demande « politique et version ». Sans elle, deux exploitations lisant la même trace ne
/// sauraient pas si elles parlent de la même affectation — et c'est précisément quand une affectation
/// vient de changer qu'on lit une trace.
#[test]
fn une_resolution_porte_la_version_qui_a_repondu() {
    let resolue = affectation(7, "modele-haut", "modele-bas")
        .resolve("frontier")
        .expect("la classe est affectée");

    assert_eq!(resolue.class, "frontier");
    assert_eq!(resolue.model, "modele-haut");
    assert_eq!(resolue.version, 7);
    assert!(
        resolue.to_string().contains("v7"),
        "la forme lisible porte la version : {resolue}"
    );
}

/// Une classe **non affectée** ne rend pas de modèle par défaut.
///
/// Un défaut ferait tourner une mission sur un modèle que personne n'a choisi, et le silence serait
/// lu comme une décision. `Outcome::NoRule` fait déjà cette distinction pour les règles, et pour la
/// même raison.
#[test]
fn une_classe_non_affectee_ne_rend_pas_de_modele_par_defaut() {
    let partielle = Assignment::versioned(1, [("frontier".to_owned(), "modele-haut".to_owned())])
        .expect("une entrée suffit");
    assert!(partielle.resolve("frontier").is_some());
    assert!(
        partielle.resolve("economy").is_none(),
        "rien n'a été choisi pour cette classe, et le dire est le résultat"
    );
}

/// Ce qui se lirait dans une trace comme une résolution aboutie est **refusé**.
#[test]
fn une_affectation_incomplete_est_refusee() {
    assert_eq!(
        Assignment::versioned(1, [(" ".to_owned(), "modele".to_owned())]),
        Err(AssignmentError::EmptyClass)
    );
    assert_eq!(
        Assignment::versioned(1, [("frontier".to_owned(), "  ".to_owned())]),
        Err(AssignmentError::EmptyModel {
            class: "frontier".to_owned()
        })
    );
    assert_eq!(
        Assignment::versioned(
            1,
            [
                ("frontier".to_owned(), "un".to_owned()),
                ("frontier".to_owned(), "deux".to_owned()),
            ]
        ),
        Err(AssignmentError::DuplicateClass {
            class: "frontier".to_owned()
        })
    );
}

// ---------------------------------------------------------------------------------------------
// 3. Changer l'affectation ne change aucun type
// ---------------------------------------------------------------------------------------------

/// **Permuter les deux modèles ne touche rien d'autre que la table.**
///
/// C'est la clause qui porte l'item, et elle se vérifie par ce qui **ne bouge pas** : la même classe
/// demandée, le même appel, la même signature — seul le modèle rendu diffère. Aucun type n'a changé,
/// aucun `match` n'a été étendu, rien n'a été recompilé de plus que la donnée.
#[test]
fn permuter_l_affectation_ne_change_que_le_modele_rendu() {
    let avant = affectation(1, "modele-haut", "modele-bas");
    let apres = affectation(2, "modele-bas", "modele-haut");

    for classe in ["frontier", "economy"] {
        let une = avant.resolve(classe).expect("affectée");
        let autre = apres.resolve(classe).expect("affectée");
        assert_eq!(une.class, autre.class, "la classe demandée est la même");
        assert_ne!(une.model, autre.model, "seul le modèle a changé");
        assert_ne!(une.version, autre.version, "et la version le dit");
    }
}

/// Une affectation peut nommer une classe que le domaine **ne connaît pas encore**.
///
/// C'est la conséquence directe d'indexer par slug : la politique n'a pas d'opinion sur
/// l'énumération du domaine. Un troisième barreau s'affecte le jour où il existe, sans que ce crate
/// change d'une ligne — et une classe retirée du domaine laisse une entrée morte plutôt qu'une
/// erreur de compilation dans la politique.
#[test]
fn la_politique_n_a_pas_d_opinion_sur_l_enumeration_du_domaine() {
    let large = Assignment::versioned(
        3,
        [
            ("frontier".to_owned(), "haut".to_owned()),
            ("un-barreau-a-venir".to_owned(), "milieu".to_owned()),
        ],
    )
    .expect("un slug est un slug");

    assert!(large.resolve("un-barreau-a-venir").is_some());
    assert_eq!(large.classes().count(), 2);
}

/// **Le crate de politique ne dépend de rien**, et cet item ne l'a pas changé.
///
/// C'est la forme la plus dure de la clause 3 : si la politique connaissait `CognitionClass`,
/// ajouter un barreau au domaine deviendrait un changement de type traversant. Prendre un `&str` est
/// moins « typé » et c'est exactement ce qui achète la propriété.
#[test]
fn le_crate_de_politique_ne_depend_d_aucun_autre() {
    let manifeste = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
        .expect("le crate lit son propre manifeste");

    // **Les tables de dépendances seules.** Une première rédaction cherchait « locus- » dans tout le
    // manifeste et rougissait sur `name = "locus-policy"` — le crate s'y nomme lui-même. Troisième
    // faux positif de cet idiome aujourd'hui, après `Default` qui contient « fault » et `->` qui
    // contient « > ». La règle qui s'en dégage : une recherche par sous-chaîne se **restreint** à ce
    // qu'elle doit lire, elle ne se relâche pas.
    let mut dans_les_dependances = false;
    let mut declarees: Vec<&str> = Vec::new();
    for ligne in manifeste.lines() {
        let taillee = ligne.trim();
        if taillee.starts_with('[') {
            dans_les_dependances = taillee.contains("dependencies");
            continue;
        }
        if dans_les_dependances && !taillee.is_empty() && !taillee.starts_with('#') {
            declarees.push(taillee);
        }
    }

    let internes: Vec<&&str> = declarees
        .iter()
        .filter(|ligne| ligne.contains("locus-"))
        .collect();
    assert!(
        internes.is_empty(),
        "la politique n'a d'opinion sur l'énumération de personne : {internes:?}"
    );
}
