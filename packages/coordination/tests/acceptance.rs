//! Test de sortie de `W21.d` — **`accepted_mutation_rate`**, ADR 0024.
//!
//! 1. Une proposition indécise n'entre pas au dénominateur, et le taux ne bouge pas quand on en
//!    ajoute.
//! 2. Les indécises sont rendues **à part**, jamais fondues dans le taux.
//! 3. Une révocation ne défait pas une acceptation.
//! 4. Aucune décision terminale rend `None`, jamais zéro.
//! 5. Rien ne juge.

use locus_coordination::{DecisionState, MutationAcceptance};

// ---------------------------------------------------------------------------------------------
// 1 et 2. L'indécis ne compte pas, et se compte à part
// ---------------------------------------------------------------------------------------------

/// **Ajouter des propositions indécises ne bouge pas le taux.**
///
/// Le test qui porte l'item. Sans cette règle, la lenteur des décideurs se lirait « les agents
/// proposent n'importe quoi » — et on irait corriger les agents.
#[test]
fn ajouter_des_indecises_ne_bouge_pas_le_taux() {
    let decidees = [
        DecisionState::Approved,
        DecisionState::Approved,
        DecisionState::Approved,
        DecisionState::Rejected,
    ];
    let sans = MutationAcceptance::over(&decidees);

    let mut avec_attente = decidees.to_vec();
    avec_attente.extend([DecisionState::Proposed; 20]);
    let avec = MutationAcceptance::over(&avec_attente);

    assert_eq!(sans.rate(), Some(0.75));
    assert_eq!(
        avec.rate(),
        sans.rate(),
        "vingt propositions en attente ont fait bouger un taux qui ne parle que des décisions"
    );
    assert_eq!(
        avec.decided(),
        4,
        "le dénominateur ne compte que les décidées"
    );
}

/// **Les indécises sont rendues à part, avec le taux.**
///
/// Un taux dont on ignore combien de propositions attendent encore ne se lit pas : 3/4 sur quatre
/// propositions et 3/4 sur quatre-vingt-quatre ne disent pas la même chose de la gouvernance.
#[test]
fn les_indecises_sont_comptees_a_part() {
    let etats = [
        DecisionState::Approved,
        DecisionState::Rejected,
        DecisionState::Proposed,
        DecisionState::Proposed,
        DecisionState::Proposed,
    ];
    let mesure = MutationAcceptance::over(&etats);

    assert_eq!(mesure.accepted(), 1);
    assert_eq!(mesure.refused(), 1);
    assert_eq!(mesure.pending(), 3);
    assert_eq!(mesure.decided(), 2);
    assert_eq!(mesure.rate(), Some(0.5));

    let rendu = mesure.to_string();
    assert!(
        rendu.contains("3 en attente"),
        "le compte des indécises accompagne le taux : {rendu}"
    );
}

// ---------------------------------------------------------------------------------------------
// 3. Une révocation ne défait pas une acceptation
// ---------------------------------------------------------------------------------------------

/// **Révoquer une décision approuvée ne change pas le taux d'acceptation.**
///
/// `revoked` désigne une décision qui **a été** approuvée puis annulée. L'exclure du numérateur
/// ferait baisser le taux rétroactivement à chaque révocation, ce qui fondrait deux questions en un
/// seul nombre : « les propositions passent-elles ? » et « celles qui passent tiennent-elles ? ».
///
/// La seconde est celle de `W21.e`, et un taux d'acceptation qui bougerait en même temps rendrait
/// les deux illisibles.
#[test]
fn une_revocation_ne_defait_pas_une_acceptation() {
    let avant = MutationAcceptance::over(&[
        DecisionState::Approved,
        DecisionState::Approved,
        DecisionState::Rejected,
    ]);
    let apres = MutationAcceptance::over(&[
        DecisionState::Approved,
        DecisionState::Revoked,
        DecisionState::Rejected,
    ]);

    assert_eq!(avant.rate(), apres.rate());
    assert_eq!(avant.accepted(), apres.accepted());
    assert_eq!(avant.decided(), apres.decided());
}

/// **Une révoquée reste au dénominateur.**
///
/// La retirer des deux termes changerait le taux dans l'autre sens, et le rendrait sensible aux
/// révocations tout en ayant l'air stable sur un petit échantillon.
#[test]
fn une_revoquee_reste_une_decision() {
    let mesure = MutationAcceptance::over(&[DecisionState::Revoked, DecisionState::Rejected]);

    assert_eq!(mesure.decided(), 2);
    assert_eq!(mesure.accepted(), 1);
    assert_eq!(mesure.pending(), 0);
    assert_eq!(mesure.rate(), Some(0.5));
}

// ---------------------------------------------------------------------------------------------
// 4. Aucune décision n'est pas zéro
// ---------------------------------------------------------------------------------------------

/// **Sans décision terminale, le taux n'existe pas — il ne vaut pas zéro.**
///
/// Zéro signifie « tout ce qui a été décidé a été refusé », ce qui est un fait. `None` signifie
/// « rien n'a été décidé », qui est l'absence de fait. Les deux appellent des suites opposées :
/// regarder les propositions, ou attendre les décideurs.
#[test]
fn sans_decision_le_taux_est_absent_et_non_nul() {
    let rien = MutationAcceptance::over(&[]);
    let en_attente = MutationAcceptance::over(&[DecisionState::Proposed; 5]);
    let tout_refuse = MutationAcceptance::over(&[DecisionState::Rejected, DecisionState::Rejected]);

    assert_eq!(rien.rate(), None);
    assert_eq!(
        en_attente.rate(),
        None,
        "cinq en attente ne font pas un taux"
    );
    assert_eq!(en_attente.pending(), 5);
    assert_eq!(
        tout_refuse.rate(),
        Some(0.0),
        "tout refuser est un fait, et il se distingue de l'absence de décision"
    );
}

/// **Tout approuver rend un, et tout refuser rend zéro.**
#[test]
fn les_deux_bornes_sont_atteintes() {
    let tout = MutationAcceptance::over(&[DecisionState::Approved, DecisionState::Revoked]);
    let rien = MutationAcceptance::over(&[DecisionState::Rejected]);

    assert_eq!(tout.rate(), Some(1.0));
    assert_eq!(rien.rate(), Some(0.0));
}

// ---------------------------------------------------------------------------------------------
// 5. Rien ne juge
// ---------------------------------------------------------------------------------------------

/// **Aucun seuil, aucune note, aucun verdict.**
///
/// Un taux d'acceptation bas n'est pas une faute — une gouvernance exigeante en produit, et c'est ce
/// qu'on lui demande. Un taux haut n'est pas une réussite : il peut signifier que personne ne
/// regarde. Les motifs visent des **signatures**, pas des mots.
#[test]
fn la_source_ne_porte_aucun_jugement() {
    let source = include_str!("../src/acceptance.rs");

    for interdit in [
        "const MIN",
        "const MAX",
        "fn is_healthy",
        "fn score",
        "enum Verdict",
        "fn verdict",
        "fn is_good",
    ] {
        assert!(
            !source.contains(interdit),
            "« {interdit} » ferait de ce taux un jugement, alors que le seuil est une question de \
             politique"
        );
    }
}
