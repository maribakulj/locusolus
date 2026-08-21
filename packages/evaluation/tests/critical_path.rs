//! Test de sortie de `W21.g` — **`critical_path_length`**, ADR 0024.
//!
//! 1. La plus longue chaîne se compte en **étapes**, et la parallélisation ne la raccourcit pas.
//! 2. Un cycle est refusé **en le nommant**, jamais parcouru.
//! 3. La mesure ne parle pas de temps, et ne peut pas se calculer sur le graphe de coordination.

use locus_evaluation::{CriticalPathError, Dependencies};

// ---------------------------------------------------------------------------------------------
// 1. Le compte d'étapes
// ---------------------------------------------------------------------------------------------

/// **Une chaîne de trois vaut trois étapes.**
#[test]
fn une_chaine_vaut_sa_longueur() {
    let suite = Dependencies::between([("a", "b"), ("b", "c")]);

    let chemin = suite.critical_path().expect("acyclique");

    assert_eq!(chemin.steps(), 3);
}

/// **Élargir en parallèle n'allonge pas le chemin critique.**
///
/// C'est le sens même de la mesure : ce qu'aucune parallélisation ne raccourcit, et que rien
/// n'allonge non plus tant qu'on ajoute **à côté**. Dix tâches indépendantes après `a` valent deux
/// étapes, pas onze.
#[test]
fn la_largeur_n_allonge_pas_le_chemin() {
    let etroit = Dependencies::between([("a", "b"), ("b", "c")]);
    let large = Dependencies::between(
        std::iter::once(("a".to_owned(), "b".to_owned()))
            .chain(std::iter::once(("b".to_owned(), "c".to_owned())))
            .chain((0..10).map(|i| ("a".to_owned(), format!("feuille{i}")))),
    );

    assert_eq!(etroit.critical_path().expect("acyclique").steps(), 3);
    assert_eq!(
        large.critical_path().expect("acyclique").steps(),
        3,
        "dix tâches ajoutées en parallèle ont allongé un chemin qui ne dépend que de sa plus longue \
         chaîne"
    );
}

/// **Le chemin critique suit la branche la plus longue, pas la première rencontrée.**
///
/// Deux branches partent de `a` : l'une de deux étapes, l'autre de quatre. La mesure doit rendre la
/// seconde quel que soit l'ordre dans lequel les couples sont fournis.
#[test]
fn la_plus_longue_branche_l_emporte() {
    let courte_d_abord = Dependencies::between([
        ("a", "x1"),
        ("x1", "fin"),
        ("a", "y1"),
        ("y1", "y2"),
        ("y2", "y3"),
    ]);
    let longue_d_abord = Dependencies::between([
        ("a", "y1"),
        ("y1", "y2"),
        ("y2", "y3"),
        ("a", "x1"),
        ("x1", "fin"),
    ]);

    assert_eq!(
        courte_d_abord.critical_path().expect("acyclique").steps(),
        4
    );
    assert_eq!(
        longue_d_abord.critical_path().expect("acyclique").steps(),
        4
    );
}

/// **Quand deux branches convergent, la plus longue commande — quel que soit l'ordre de traitement.**
///
/// Trouvé par un mutant survivant. Le test précédent a deux branches, mais elles ne **convergent**
/// nulle part : aucun nœud n'y a deux prédécesseurs, donc le maximum n'était jamais exercé, et
/// remplacer `max(actuel, atteint + 1)` par `atteint + 1` passait toute la suite.
///
/// C'est le défaut classique du plus long chemin : un nœud atteint par plusieurs prédécesseurs
/// prendrait la profondeur du **dernier traité**, et la valeur dépendrait alors de l'ordre
/// d'itération — une métrique qui change de valeur selon la façon dont on la lit.
///
/// Ici `fin` est atteint par une branche de quatre et par une branche de deux ; la réponse est cinq,
/// et les deux ordres de déclaration la rendent.
#[test]
fn deux_branches_qui_convergent_prennent_la_plus_longue() {
    let longue_en_dernier = Dependencies::between([
        ("a", "court"),
        ("court", "fin"),
        ("a", "l1"),
        ("l1", "l2"),
        ("l2", "l3"),
        ("l3", "fin"),
    ]);
    let longue_en_premier = Dependencies::between([
        ("a", "l1"),
        ("l1", "l2"),
        ("l2", "l3"),
        ("l3", "fin"),
        ("a", "court"),
        ("court", "fin"),
    ]);

    assert_eq!(
        longue_en_dernier
            .critical_path()
            .expect("acyclique")
            .steps(),
        5,
        "la branche courte a écrasé la longue à la convergence"
    );
    assert_eq!(
        longue_en_premier
            .critical_path()
            .expect("acyclique")
            .steps(),
        5
    );
}

/// **Un nœud sans dépendance vaut une étape ; aucun nœud vaut zéro.**
///
/// Les deux se lisent « il n'y a rien à faire », et l'un des deux est faux : une tâche seule doit
/// quand même être faite.
#[test]
fn le_plancher_distingue_une_tache_de_pas_de_tache() {
    let rien = Dependencies::default();
    let une = Dependencies::between([("seule", "suivante")]);

    assert!(rien.is_empty());
    assert_eq!(rien.critical_path().expect("acyclique").steps(), 0);
    assert_eq!(une.critical_path().expect("acyclique").steps(), 2);
}

// ---------------------------------------------------------------------------------------------
// 2. Le cycle, refusé en le nommant
// ---------------------------------------------------------------------------------------------

/// **Un cycle réel est refusé, et le refus liste ses membres.**
///
/// `R3` a montré qu'une version de coordination peut être cyclique, et une métrique qui ne termine
/// pas emporte son appelant. Un refus muet obligerait en outre à chercher le cycle à la main dans
/// un graphe dont on vient d'apprendre qu'il en contient un — au pire moment.
#[test]
fn un_cycle_est_refuse_en_nommant_ses_membres() {
    let boucle = Dependencies::between([("a", "b"), ("b", "c"), ("c", "a"), ("depart", "a")]);

    let refus = boucle
        .critical_path()
        .expect_err("le cycle doit être refusé");
    let CriticalPathError::Cycle { members } = refus;

    assert_eq!(
        members,
        vec!["a", "b", "c"],
        "les trois du cycle, et eux seuls"
    );
    assert!(
        !members.contains(&"depart".to_owned()),
        "un nœud qui mène au cycle sans en faire partie n'y est pas"
    );
}

/// **Une dépendance d'un nœud vers lui-même est un cycle.**
///
/// Le plus court possible, et celui qu'une implémentation naïve laisse passer en ne regardant que
/// les couples distincts.
#[test]
fn une_dependance_reflexive_est_un_cycle() {
    let sur_soi = Dependencies::between([("a", "a")]);

    let refus = sur_soi.critical_path().expect_err("refusé");
    let CriticalPathError::Cycle { members } = refus;

    assert_eq!(members, vec!["a"]);
}

/// **Le refus se lit sans avoir à ouvrir le graphe.**
#[test]
fn le_refus_nomme_le_cycle_dans_son_message() {
    let boucle = Dependencies::between([("x", "y"), ("y", "x")]);

    let rendu = boucle.critical_path().expect_err("refusé").to_string();

    assert!(rendu.contains('x') && rendu.contains('y'), "{rendu}");
}

// ---------------------------------------------------------------------------------------------
// 3. Ce que la mesure ne dit pas, et où elle ne peut pas aller
// ---------------------------------------------------------------------------------------------

/// **Aucune signature ne parle de temps.**
///
/// La valeur est un compte d'**étapes** : une durée dépendrait de ce que chaque étape coûte, que ce
/// module ne connaît pas. Les motifs visent des signatures.
#[test]
fn la_source_ne_parle_jamais_de_duree() {
    let source = include_str!("../src/critical_path.rs");

    for interdit in [
        "fn duration",
        "fn elapsed",
        "fn seconds",
        "fn millis",
        "Duration",
        "Instant",
        "const MIN",
        "const MAX",
        "fn is_healthy",
        "fn score",
        "enum Verdict",
    ] {
        assert!(
            !source.contains(interdit),
            "« {interdit} » ferait lire une durée là où il y a un compte d'étapes"
        );
    }
}

/// **La mesure ne peut pas se calculer sur le graphe de coordination — tenu par le graphe de paquets.**
///
/// L'item annonçait un test d'absence sur les motifs de la source. La forme retenue est plus forte :
/// `locus-evaluation` **n'a aucune dépendance**, donc importer `locus-coordination` ne compilerait
/// pas. Le test lit le `Cargo.toml` plutôt qu'une source, parce que c'est là que la frontière vit.
///
/// Il vérifie aussi qu'il a bien lu quelque chose : un test qui ne trouverait plus la section
/// passerait sinon en silence, ce qui est la faute du compteur qui n'a rien lu.
#[test]
fn le_paquet_ne_peut_pas_importer_la_coordination() {
    let manifeste = include_str!("../Cargo.toml");

    assert!(
        manifeste.contains("[dependencies]"),
        "le manifeste a changé de forme : ce test ne lit plus ce qu'il croit lire"
    );
    for interdit in ["locus-coordination", "locus-graph", "locus-domain"] {
        assert!(
            !manifeste.contains(interdit),
            "« {interdit} » ferait de cette mesure une mesure du graphe de coordination, dont les \
             arêtes n'ont pas la même sémantique"
        );
    }
}
