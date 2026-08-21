//! Test de sortie de `W21.e` — **`rollback_rate`**, par cohorte. ADR 0024.
//!
//! 1. Le type ne se construit pas sans ses deux moitiés — l'ensemble et la fenêtre.
//! 2. Une cohorte ouverte ne rend **aucun** flottant, seulement le compte des observables.
//! 3. Une annulation hors fenêtre n'est pas une annulation de cette cohorte.
//! 4. Deux cohortes de fenêtres différentes ne se comparent pas, et l'API ne l'offre pas.
//! 5. Le biais que tout ceci supprime : un taux instantané baisse quand on accélère.

use locus_coordination::{Accepted, Cohort, CohortError, Rollbacks};

// ---------------------------------------------------------------------------------------------
// 1. Les deux moitiés sont obligatoires
// ---------------------------------------------------------------------------------------------

/// **Une fenêtre nulle est refusée.**
///
/// Observer pendant zéro opération n'observe rien : toute cohorte serait close avec zéro annulation,
/// et ce zéro n'aurait rien constaté. C'est le pire des nombres — un fait apparent tiré d'une
/// absence de mesure.
#[test]
fn une_fenetre_nulle_est_refusee() {
    let refus = Cohort::over(0, 100, [Accepted::holding(1)]).expect_err("fenêtre nulle");

    assert_eq!(refus, CohortError::EmptyWindow);
}

/// **Une annulation antérieure à son acceptation est refusée.**
#[test]
fn une_annulation_avant_son_acceptation_est_refusee() {
    let refus = Accepted::reverted(10, 4).expect_err("ordre impossible");

    assert_eq!(
        refus,
        CohortError::RevertedBeforeAccepted {
            at: 10,
            reverted_at: 4
        }
    );
}

/// **Une acceptation postérieure à ce qui a été lu est refusée.**
///
/// La cohorte porterait un fait que son propre journal ne contient pas.
#[test]
fn une_acceptation_au_dela_de_la_lecture_est_refusee() {
    let refus = Cohort::over(5, 20, [Accepted::holding(21)]).expect_err("au-delà de l'observation");

    assert_eq!(
        refus,
        CohortError::AcceptedBeyondObservation {
            at: 21,
            observed_through: 20
        }
    );
}

// ---------------------------------------------------------------------------------------------
// 2. Une cohorte ouverte ne rend pas de taux
// ---------------------------------------------------------------------------------------------

/// **Tant qu'une fenêtre n'est pas finie, aucun flottant ne sort.**
///
/// Un taux provisoire serait affiché à côté de taux définitifs, comparé à eux, et personne ne
/// saurait lequel des deux nombres a fini de bouger.
#[test]
fn une_cohorte_ouverte_ne_rend_aucun_taux() {
    // Acceptée en 18, fenêtre de 10 : sa fenêtre finit en 28, et le journal n'est lu que jusqu'à 20.
    let cohorte = Cohort::over(
        10,
        20,
        [
            Accepted::reverted(1, 3).expect("annulée dans sa fenêtre"),
            Accepted::holding(2),
            Accepted::holding(18),
        ],
    )
    .expect("la cohorte est licite");

    let rollbacks = cohorte.rollbacks();

    assert!(!rollbacks.is_closed());
    assert_eq!(
        rollbacks.rate(),
        None,
        "un taux provisoire serait comparé à un taux définitif"
    );
    assert_eq!(
        rollbacks,
        Rollbacks::Open {
            still_observable: 1,
            reverted_so_far: 1,
            accepted: 3
        }
    );
}

/// **Une cohorte close rend son taux.**
#[test]
fn une_cohorte_close_rend_son_taux() {
    let cohorte = Cohort::over(
        10,
        100,
        [
            Accepted::reverted(1, 5).expect("dans la fenêtre"),
            Accepted::holding(2),
            Accepted::holding(3),
            Accepted::holding(4),
        ],
    )
    .expect("la cohorte est licite");

    let rollbacks = cohorte.rollbacks();

    assert!(rollbacks.is_closed());
    assert_eq!(
        rollbacks,
        Rollbacks::Closed {
            reverted: 1,
            accepted: 4
        }
    );
    assert_eq!(rollbacks.rate(), Some(0.25));
}

/// **Une cohorte close et vide ne rend pas zéro.**
///
/// Diviser par zéro acceptation ne rend pas zéro, cela ne rend rien — et « aucune annulation » se
/// lirait comme un constat alors que rien n'a été observé.
#[test]
fn une_cohorte_vide_ne_rend_pas_zero() {
    let cohorte = Cohort::over(10, 100, []).expect("une cohorte vide est licite");
    let rollbacks = cohorte.rollbacks();

    assert!(cohorte.is_empty());
    assert!(rollbacks.is_closed());
    assert_eq!(rollbacks.rate(), None);
}

// ---------------------------------------------------------------------------------------------
// 3. La fenêtre borne ce qui compte
// ---------------------------------------------------------------------------------------------

/// **Une annulation hors fenêtre n'est pas une annulation de cette cohorte.**
///
/// Elle a eu lieu, elle est vraie, et elle appartient à une autre question. La compter ferait
/// dépendre le résultat de tout ce qui s'est passé après la fenêtre — exactement le biais que la
/// cohorte supprime.
///
/// Les deux acceptations sont identiques à la position de l'annulation près : l'une à la limite de
/// la fenêtre, l'autre juste après.
#[test]
fn une_annulation_hors_fenetre_ne_compte_pas() {
    let dans = Cohort::over(
        10,
        200,
        [Accepted::reverted(5, 15).expect("exactement à la limite")],
    )
    .expect("licite");
    let hors =
        Cohort::over(10, 200, [Accepted::reverted(5, 16).expect("une de trop")]).expect("licite");

    assert_eq!(
        dans.rollbacks().rate(),
        Some(1.0),
        "15 = 5 + 10, dans la fenêtre"
    );
    assert_eq!(
        hors.rollbacks().rate(),
        Some(0.0),
        "16 > 5 + 10, hors fenêtre"
    );
}

/// **Une acceptation annulée hors fenêtre a un sort connu, donc n'ouvre pas la cohorte.**
///
/// Son sort *dans cette cohorte* est « a tenu pendant la fenêtre », et il est établi sans attendre.
/// La compter comme encore observable ferait attendre une information qu'on a déjà.
#[test]
fn une_annulation_hors_fenetre_n_ouvre_pas_la_cohorte() {
    let cohorte = Cohort::over(
        5,
        12,
        [Accepted::reverted(1, 30).expect("bien après sa fenêtre")],
    )
    .expect("licite");

    assert!(
        cohorte.rollbacks().is_closed(),
        "son sort dans cette fenêtre est connu : elle a tenu"
    );
    assert_eq!(cohorte.rollbacks().rate(), Some(0.0));
}

// ---------------------------------------------------------------------------------------------
// 4. Aucune comparaison entre cohortes
// ---------------------------------------------------------------------------------------------

/// **Aucun type de ce module ne dérive un ordre.**
///
/// Une part annulée « dans les dix opérations » et une part annulée « dans les mille » ne mesurent
/// pas la même chose, et la seconde est mécaniquement plus grande. Un ordre dérivé laisserait
/// `a < b` compiler entre deux cohortes incomparables.
///
/// **Le motif lit les attributs `derive`, jamais le texte.** La première rédaction cherchait la
/// chaîne dans toute la source et a mordu sur la phrase de documentation qui *explique* l'absence —
/// septième occurrence de cette faute dans ce dépôt, et la plus nette, puisqu'elle est survenue
/// dans le test dont le commentaire disait déjà de viser des formes de code. Une garde qui doit
/// décider, à chaque relecture, si une occurrence est un usage ou une explication est une garde
/// qu'on finit par assouplir.
#[test]
fn aucun_type_ne_derive_un_ordre() {
    let source = include_str!("../src/cohort.rs");

    let derives: Vec<&str> = source
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("#[derive("))
        .collect();

    assert!(
        !derives.is_empty(),
        "aucun attribut `derive` trouvé : le motif ne lit plus ce qu'il croit lire"
    );
    for derive in derives {
        assert!(
            !derive.contains("Ord"),
            "« {derive} » laisse comparer deux cohortes de fenêtres différentes"
        );
    }
}

/// **Aucune signature n'offre de comparaison ni de jugement.**
///
/// Les motifs visent des signatures, pour la raison ci-dessus.
#[test]
fn la_source_n_offre_ni_comparaison_ni_verdict() {
    let source = include_str!("../src/cohort.rs");

    for interdit in [
        "fn compare",
        "fn better_than",
        "fn worse_than",
        "const MIN",
        "const MAX",
        "fn is_healthy",
        "fn score",
        "enum Verdict",
        "fn verdict",
    ] {
        assert!(
            !source.contains(interdit),
            "« {interdit} » ferait de cette mesure un jugement"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 5. Le biais que la cohorte supprime
// ---------------------------------------------------------------------------------------------

/// **Le biais, montré : accélérer ferait baisser un taux instantané, et ne bouge pas la cohorte.**
///
/// Deux lots identiques par leur sort — une annulation sur deux — mais le second contient en plus
/// des acceptations trop récentes pour avoir pu être annulées. Un taux instantané les compterait au
/// dénominateur et tomberait à 1/4 ; la cohorte, elle, refuse de conclure et **dit** qu'elle attend.
///
/// C'est la démonstration exécutable de la décision 5 de l'ADR 0024 : « on annule de moins en moins »
/// se produit tout seul dès qu'on accélère.
#[test]
fn accelerer_ne_fait_pas_baisser_le_taux_de_la_cohorte() {
    let etabli = [
        Accepted::reverted(1, 4).expect("dans sa fenêtre"),
        Accepted::holding(2),
    ];

    let posee = Cohort::over(10, 100, etabli).expect("licite");
    assert_eq!(posee.rollbacks().rate(), Some(0.5));

    // Les mêmes, plus deux acceptations toutes fraîches que rien n'a eu le temps d'annuler.
    let mut accelere = etabli.to_vec();
    accelere.extend([Accepted::holding(99), Accepted::holding(100)]);
    let rapide = Cohort::over(10, 100, accelere).expect("licite");

    assert_eq!(
        rapide.rollbacks().rate(),
        None,
        "un taux instantané aurait rendu 0.25 et fait croire à une amélioration"
    );
    assert_eq!(
        rapide.rollbacks(),
        Rollbacks::Open {
            still_observable: 2,
            reverted_so_far: 1,
            accepted: 4
        }
    );
}
