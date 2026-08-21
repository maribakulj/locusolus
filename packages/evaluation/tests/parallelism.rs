//! Test de sortie de `W21.h` — **`average_parallelism`**, ADR 0024.
//!
//! 1. Ce n'est pas le nombre d'agents, ni la largeur maximale — une fixture où les trois diffèrent.
//! 2. Ce n'est pas le plafond `Dimension::Parallelism` de §7.2, et le paquet ne peut pas le voir.
//! 3. Un chemin critique nul est un cas **rendu**, jamais une division par zéro.
//! 4. Un cycle n'a pas de parallélisme : le refus se propage.

use locus_evaluation::{AverageParallelism, CriticalPathError, Dependencies};

/// Un éventail : `a` puis `largeur` feuilles indépendantes.
fn eventail(largeur: usize) -> Dependencies {
    Dependencies::between((0..largeur).map(|i| ("a".to_owned(), format!("feuille{i}"))))
}

// ---------------------------------------------------------------------------------------------
// 1. Une moyenne, et rien d'autre
// ---------------------------------------------------------------------------------------------

/// **Une chaîne pure vaut un.**
///
/// Trois nœuds, trois étapes : rien n'avance de front, et la mesure le dit.
#[test]
fn une_chaine_pure_vaut_un() {
    let suite = Dependencies::between([("a", "b"), ("b", "c")]);

    assert_eq!(
        suite.average_parallelism().expect("acyclique"),
        AverageParallelism::Measured(1.0)
    );
}

/// **La moyenne n'est pas la largeur maximale.**
///
/// Le test qui porte l'item. Onze nœuds sur deux étapes : le niveau le plus large en porte **dix**,
/// et la moyenne vaut **5,5**. Les deux répondent à deux questions — « combien pourrait avancer de
/// front au mieux » et « combien avance de front en moyenne » — et lire l'une pour l'autre ferait
/// dimensionner une flotte sur un pic qui ne dure qu'une étape.
#[test]
fn la_moyenne_n_est_pas_la_largeur_maximale() {
    let large = eventail(10);

    let largeur_maximale = 10;
    let moyenne = large
        .average_parallelism()
        .expect("acyclique")
        .value()
        .expect("mesurable");

    assert_eq!(large.nodes().len(), 11, "les dix feuilles, plus la racine");
    assert_eq!(large.critical_path().expect("acyclique").steps(), 2);
    assert!(
        (moyenne - 5.5).abs() < f64::EPSILON,
        "onze nœuds sur deux étapes valent 5,5, pas {moyenne}"
    );
    assert!(
        moyenne < f64::from(largeur_maximale),
        "la moyenne ({moyenne}) doit rester en deçà du pic ({largeur_maximale})"
    );
}

/// **Élargir augmente la moyenne ; allonger la diminue.**
///
/// Les deux sens, parce qu'un seul laisserait passer une mesure qui ne bouge que dans une
/// direction.
#[test]
fn la_moyenne_suit_la_forme_dans_les_deux_sens() {
    let etroit = eventail(2);
    let large = eventail(8);
    let long = Dependencies::between([("a", "b"), ("b", "c"), ("c", "d"), ("d", "e")]);

    let etroit = etroit.average_parallelism().expect("acyclique").value();
    let large = large.average_parallelism().expect("acyclique").value();
    let long = long.average_parallelism().expect("acyclique").value();

    assert!(large > etroit, "élargir doit augmenter la moyenne");
    assert!(long < etroit, "allonger doit la diminuer");
}

/// **La mesure ne peut pas voir d'exécutant : elle n'en reçoit aucun.**
///
/// « Ce n'est pas le nombre d'agents qui tournaient » se tient ici par la **signature** :
/// `average_parallelism` ne prend aucun argument, et `Dependencies` ne porte que des identifiants de
/// travail. Une organisation à un seul agent obtiendrait la même valeur — ce qui est correct, parce
/// que la mesure décrit le graphe et non son exécution.
#[test]
fn la_mesure_ne_recoit_aucun_executant() {
    let source = include_str!("../src/critical_path.rs");

    for interdit in [
        "fn agents",
        "agent_id",
        "Agent",
        "worker",
        "Worker",
        "fn assigned",
    ] {
        assert!(
            !source.contains(interdit),
            "« {interdit} » ferait de cette mesure une observation de l'exécution"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 2. Le plafond de §7.2, que ce paquet ne peut pas voir
// ---------------------------------------------------------------------------------------------

/// **Le paquet ne peut pas importer le budget, donc la borne et le constat ne se confondent pas.**
///
/// `Dimension::Parallelism` de §7.2 est une **limite qu'on fixe** ; ceci est un **constat**. « Le
/// parallélisme vaut 4 » ne dirait plus lequel des deux — la confusion que le renommage de la
/// décision 2 de l'ADR 0024 évite.
///
/// L'item demandait un test d'absence sur les imports du module. La forme retenue est plus forte :
/// `locus-evaluation` n'a **aucune** dépendance, donc l'import ne compilerait pas. Le test lit le
/// manifeste, et vérifie qu'il a bien trouvé la section — sans quoi un manifeste remanié le ferait
/// passer en silence.
#[test]
fn le_paquet_ne_peut_pas_voir_le_plafond_de_budget() {
    let manifeste = include_str!("../Cargo.toml");

    assert!(
        manifeste.contains("[dependencies]"),
        "le manifeste a changé de forme : ce test ne lit plus ce qu'il croit lire"
    );
    for interdit in ["locus-budget", "locus-coordination", "locus-domain"] {
        assert!(
            !manifeste.contains(interdit),
            "« {interdit} » laisserait confondre la borne et le constat"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 3 et 4. Les deux cas où il n'y a pas de valeur
// ---------------------------------------------------------------------------------------------

/// **Aucun nœud : `NoWork`, jamais une division par zéro.**
///
/// `0 / 0` produirait un `NaN`, qui ne se compare à rien — pas même à lui-même — et se propagerait
/// sans qu'aucune assertion ne le retienne.
#[test]
fn aucun_travail_est_un_cas_rendu() {
    let rien = Dependencies::default();

    let mesure = rien.average_parallelism().expect("acyclique");

    assert_eq!(mesure, AverageParallelism::NoWork);
    assert_eq!(mesure.value(), None);
    assert!(
        mesure.value().is_none_or(f64::is_finite),
        "aucune valeur non finie ne doit sortir"
    );
}

/// **Un cycle n'a pas de parallélisme : le refus se propage, il ne se rattrape pas.**
///
/// Sans chemin critique il n'y a rien à diviser. Rendre une valeur quand même obligerait à inventer
/// un dénominateur, et ce nombre serait lu comme une mesure.
#[test]
fn un_cycle_n_a_pas_de_parallelisme() {
    let boucle = Dependencies::between([("a", "b"), ("b", "a")]);

    let refus = boucle
        .average_parallelism()
        .expect_err("le cycle doit se propager");
    let CriticalPathError::Cycle { members } = refus;

    assert_eq!(members, vec!["a", "b"]);
}

/// **Rien ne juge.**
#[test]
fn la_source_ne_porte_aucun_jugement() {
    let source = include_str!("../src/critical_path.rs");

    for interdit in [
        "const MIN",
        "const MAX",
        "fn is_healthy",
        "fn score",
        "enum Verdict",
        "fn is_efficient",
    ] {
        assert!(
            !source.contains(interdit),
            "« {interdit} » ferait de cette moyenne un jugement"
        );
    }
}
