//! Test de sortie de `W21.l` — **`communication_tokens`**, ADR 0024.
//!
//! 1. Les non classées sont comptées **à part** et n'entrent dans aucun des deux termes.
//! 2. Une campagne entièrement non classée rend une **absence**, pas un zéro.
//! 3. « Dépensé » est ce que le registre appelle dépensé : trois mouvements, deux signes.
//! 4. Un journal incohérent est **refusé**, pas saturé à zéro.
//! 5. Rien ne juge, et rien ne se déduit du texte libre.

use locus_budget::{Amounts, BudgetAccount, Classification, Dimension, Limits, Reservation, Spend};
use locus_coordination::{Communication, CommunicationError, Share};
use locus_protocol::{
    Id, IdKind, Timestamp,
    id::provisional::{BudgetAccount as AccountKind, Reservation as ReservationKind},
};

/// Le code d'un fichier, c'est sa source moins ses commentaires — voir `W21.j`.
fn code_seul(source: &str) -> String {
    source
        .lines()
        .filter(|ligne| !ligne.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn jetons(combien: u64) -> Amounts {
    [(Dimension::Tokens, combien)].into_iter().collect()
}

fn compte() -> BudgetAccount {
    let mut compte = BudgetAccount::open(
        id::<AccountKind>(1),
        Limits::bounding([(Dimension::Tokens, 10_000_000)]).expect("une borne suffit"),
    );
    compte
        .allocate(&jetons(1_000_000), "dotation")
        .expect("dotation licite");
    compte
}

/// Retenir puis consommer `combien` jetons, classés ou non.
fn depenser(compte: &mut BudgetAccount, seed: u8, combien: u64, objet: Option<Spend>) {
    let tenue = retenir(compte, seed, combien, objet);
    compte
        .consume(tenue, &jetons(combien), "constat")
        .expect("constat licite");
}

fn retenir(
    compte: &mut BudgetAccount,
    seed: u8,
    combien: u64,
    objet: Option<Spend>,
) -> Reservation {
    let retenue = id::<ReservationKind>(seed);
    match objet {
        Some(objet) => compte.reserve_for(retenue, &jetons(combien), "retenue", objet),
        None => compte.reserve(retenue, &jetons(combien), "retenue"),
    }
    .expect("retenue licite")
}

// ---------------------------------------------------------------------------------------------
// 1. Les non classées ne sont dans aucun des deux termes
// ---------------------------------------------------------------------------------------------

/// **Le dénominateur ne contient que ce que quelqu'un a déclaré.**
///
/// Le test qui porte l'item. Mettre les non classées au dénominateur ferait **baisser** la part de
/// coordination à chaque écriture que personne n'a classée, ce qui se lirait comme un progrès ; les
/// mettre au numérateur ferait l'inverse. Elles sont donc comptées à côté — traitement des indécises
/// de `W21.d`.
#[test]
fn les_non_classees_n_entrent_dans_aucun_des_deux_termes() {
    let mut compte = compte();
    depenser(&mut compte, 10, 200, Some(Spend::Coordination));
    depenser(&mut compte, 11, 800, Some(Spend::Work));
    depenser(&mut compte, 12, 5_000, None);

    let releve = Communication::over(compte.entries()).expect("journal cohérent");

    assert_eq!(releve.coordination(), 200);
    assert_eq!(releve.work(), 800);
    assert_eq!(releve.unclassified(), 5_000, "comptées, et à part");
    assert_eq!(
        releve.declared(),
        1_000,
        "l'assiette est la somme des deux tas déclarés, et rien d'autre"
    );
    assert_eq!(
        releve.share(),
        Share::Measured(0.2),
        "cinq mille jetons non classés ne diluent pas la part"
    );
}

/// **Retirer les non classées ne change pas la part.**
///
/// La garde précédente lit une valeur ; celle-ci compare deux journaux. Si les non classées entraient
/// dans un terme quelconque, les deux parts différeraient — et c'est une égalité stricte qui le dit,
/// pas une lecture de phrase.
#[test]
fn la_part_ne_bouge_pas_quand_on_retire_les_non_classees() {
    let mut avec = compte();
    depenser(&mut avec, 10, 200, Some(Spend::Coordination));
    depenser(&mut avec, 11, 800, Some(Spend::Work));
    depenser(&mut avec, 12, 5_000, None);

    let mut sans = compte();
    depenser(&mut sans, 10, 200, Some(Spend::Coordination));
    depenser(&mut sans, 11, 800, Some(Spend::Work));

    let large = Communication::over(avec.entries()).expect("journal cohérent");
    let etroit = Communication::over(sans.entries()).expect("journal cohérent");

    assert_eq!(large.share(), etroit.share());
    assert_eq!(large.declared(), etroit.declared());
    assert_ne!(
        large.unclassified(),
        etroit.unclassified(),
        "les deux journaux diffèrent bien, sinon ce test ne compare rien"
    );
}

// ---------------------------------------------------------------------------------------------
// 2. Une absence n'est pas un zéro
// ---------------------------------------------------------------------------------------------

/// **Rien de déclaré rend une absence ; zéro coordination rend zéro.**
///
/// Zéro voudrait dire « aucune coordination », ce qui est une bonne nouvelle ; ne rien savoir n'en
/// est pas une. Même distinction qu'entre « aucune panne » et « aucune reprise » en `W21.k`, posée
/// au même endroit : sur l'accesseur.
#[test]
fn rien_de_declare_n_est_pas_zero_coordination() {
    let mut aveugle = compte();
    depenser(&mut aveugle, 10, 4_000, None);

    let mut sans_coordination = compte();
    depenser(&mut sans_coordination, 10, 4_000, Some(Spend::Work));

    let aveugle = Communication::over(aveugle.entries()).expect("journal cohérent");
    let sans_coordination =
        Communication::over(sans_coordination.entries()).expect("journal cohérent");

    assert_eq!(aveugle.share(), Share::NothingDeclared);
    assert_eq!(aveugle.share().value(), None);
    assert_eq!(sans_coordination.share(), Share::Measured(0.0));
    assert_eq!(sans_coordination.share().value(), Some(0.0));
    assert_ne!(aveugle.share(), sans_coordination.share());
}

/// **Un journal vide aussi rend une absence.**
#[test]
fn un_journal_sans_depense_rend_une_absence() {
    let vide = Communication::over(compte().entries()).expect("journal cohérent");

    assert_eq!(vide.share(), Share::NothingDeclared);
    assert_eq!(vide.declared(), 0);
    assert_eq!(vide.unclassified(), 0);
}

/// **Tout en coordination rend un, et rien d'autre ne le rend.**
#[test]
fn tout_en_coordination_rend_un() {
    let mut compte = compte();
    depenser(&mut compte, 10, 1_234, Some(Spend::Coordination));

    let releve = Communication::over(compte.entries()).expect("journal cohérent");

    assert_eq!(releve.share(), Share::Measured(1.0));
    assert_eq!(releve.work(), 0);
}

// ---------------------------------------------------------------------------------------------
// 3. « Dépensé » est ce que le registre appelle dépensé
// ---------------------------------------------------------------------------------------------

/// **Allouer, retenir et rendre ne sont pas des dépenses.**
///
/// Une allocation de coordination généreuse jamais consommée ne dit rien de ce que la coordination a
/// coûté. La compter ferait dire à la métrique le contraire de ce qu'elle mesure : plus on prévoit
/// de se coordonner, plus on paraîtrait le faire.
#[test]
fn le_provisionnement_n_est_pas_une_depense() {
    let mut compte = compte();
    compte
        .allocate_for(&jetons(500_000), "dotation", Spend::Coordination)
        .expect("dotation licite");
    let tenue = retenir(&mut compte, 10, 400_000, Some(Spend::Coordination));
    compte.release(tenue, "inutilisée").expect("rendu licite");
    depenser(&mut compte, 11, 100, Some(Spend::Coordination));
    depenser(&mut compte, 12, 900, Some(Spend::Work));

    let releve = Communication::over(compte.entries()).expect("journal cohérent");

    assert_eq!(
        releve.coordination(),
        100,
        "une allocation et une retenue rendue, toutes deux de coordination, n'ont rien coûté"
    );
    assert_eq!(releve.share(), Share::Measured(0.1));
}

/// **Un ajustement ajoute, un remboursement retire.**
///
/// Ce sont les trois mouvements et les deux signes que `BudgetAccount::spent` emploie. En réinventer
/// d'autres ici produirait une seconde arithmétique, et c'est toujours la seconde qui ment.
#[test]
fn ajustement_et_remboursement_portent_leurs_signes() {
    let mut compte = compte();
    let retenue = id::<ReservationKind>(10);
    let tenue = retenir(&mut compte, 10, 400, Some(Spend::Coordination));
    compte
        .consume(tenue, &jetons(400), "constat")
        .expect("constat licite");
    depenser(&mut compte, 11, 600, Some(Spend::Work));

    let avant = Communication::over(compte.entries()).expect("journal cohérent");
    assert_eq!(avant.coordination(), 400);

    compte
        .reconcile(&retenue, &jetons(700), "le worker a mesuré plus")
        .expect("rapprochement licite");
    let apres_hausse = Communication::over(compte.entries()).expect("journal cohérent");
    assert_eq!(
        apres_hausse.coordination(),
        700,
        "l'ajustement a ajouté 300"
    );

    compte
        .reconcile(&retenue, &jetons(100), "puis moins")
        .expect("rapprochement licite");
    let apres_baisse = Communication::over(compte.entries()).expect("journal cohérent");
    assert_eq!(
        apres_baisse.coordination(),
        400,
        "le remboursement de 300 a retiré ce que l'ajustement avait ajouté"
    );
    assert_eq!(apres_baisse.work(), 600, "l'autre tas n'a pas bougé");
}

/// **Seuls les jetons comptent : une autre dimension ne déplace rien.**
#[test]
fn une_autre_dimension_ne_deplace_pas_la_part() {
    let mut bornes = compte();
    depenser(&mut bornes, 10, 200, Some(Spend::Coordination));
    depenser(&mut bornes, 11, 800, Some(Spend::Work));
    let avant = Communication::over(bornes.entries()).expect("journal cohérent");

    let mut avec_appels = BudgetAccount::open(
        id::<AccountKind>(1),
        Limits::bounding([
            (Dimension::Tokens, 10_000_000),
            (Dimension::ModelCalls, 500),
        ])
        .expect("deux bornes"),
    );
    avec_appels
        .allocate(&jetons(1_000_000), "dotation")
        .expect("dotation licite");
    avec_appels
        .allocate(
            &[(Dimension::ModelCalls, 400)].into_iter().collect(),
            "dotation d'appels",
        )
        .expect("dotation licite");
    depenser(&mut avec_appels, 10, 200, Some(Spend::Coordination));
    depenser(&mut avec_appels, 11, 800, Some(Spend::Work));
    let retenue = id::<ReservationKind>(12);
    let appels: Amounts = [(Dimension::ModelCalls, 300)].into_iter().collect();
    let tenue = avec_appels
        .reserve_for(retenue, &appels, "des appels", Spend::Coordination)
        .expect("retenue licite");
    avec_appels
        .consume(tenue, &appels, "constat")
        .expect("constat licite");

    let apres = Communication::over(avec_appels.entries()).expect("journal cohérent");

    assert_eq!(avant.share(), apres.share());
    assert_eq!(avant.coordination(), apres.coordination());
}

// ---------------------------------------------------------------------------------------------
// 4. Un journal incohérent est refusé
// ---------------------------------------------------------------------------------------------

/// **Rembourser au-delà de la dépense est refusé, pas saturé à zéro.**
///
/// Le cas est **atteignable** : chaque rapprochement se compare à la consommation enregistrée, pas
/// au cumul des corrections, donc trois rapprochements à zéro remboursent trois fois le même écart.
/// Saturer rendrait un nombre d'apparence normale sur un journal incohérent, ce qui est exactement
/// ce qu'un registre existe pour ne pas faire.
#[test]
fn un_remboursement_au_dela_de_la_depense_est_refuse() {
    let mut compte = compte();
    let retenue = id::<ReservationKind>(10);
    let tenue = retenir(&mut compte, 10, 300, Some(Spend::Coordination));
    compte
        .consume(tenue, &jetons(300), "constat")
        .expect("constat licite");
    for tour in 0..3 {
        compte
            .reconcile(
                &retenue,
                &jetons(0),
                "le worker n'a finalement rien dépensé",
            )
            .unwrap_or_else(|erreur| panic!("rapprochement {tour} licite : {erreur}"));
    }

    let refus = Communication::over(compte.entries()).expect_err("journal incohérent");

    assert_eq!(
        refus,
        CommunicationError::RefundedMoreThanSpent {
            spend: Classification::Classified(Spend::Coordination),
            net: -600,
        }
    );
    assert!(
        refus.to_string().contains("coordination"),
        "le refus nomme le tas : {refus}"
    );
}

// ---------------------------------------------------------------------------------------------
// 5. Rien ne juge, rien ne se déduit du texte libre
// ---------------------------------------------------------------------------------------------

/// **La source ne juge pas et ne lit pas `reason`.**
///
/// Décision 9 de l'ADR 0024 : aucune métrique de cette famille ne juge. Une équipe qui ne se parle
/// jamais dépense zéro en coordination et se trompe ensemble ; une part élevée peut décrire une
/// négociation coûteuse ou une négociation nécessaire.
///
/// Les motifs lisent le **code seul** — la source privée de ses commentaires — pour la raison
/// établie en `W21.j`. Et le motif visé est `.reason()`, la **lecture** du champ, pas le mot :
/// `reason` seul mord sur `#[expect(reason = …)]`, que la configuration de lints du workspace
/// impose. C'est le cas que `W21.j` avait entrevu sans le rencontrer — nettoyer la botte de foin
/// traite la prose, mais un mot qui est aussi du code dans un autre sens demande la **forme
/// d'appel**.
#[test]
fn la_source_ne_juge_pas_et_ne_lit_pas_le_motif() {
    let code = code_seul(include_str!("../src/communication.rs"));
    assert!(
        code.contains("pub fn"),
        "le nettoyage a trop enlevé : ce test ne lit plus ce qu'il croit lire"
    );

    for interdit in [
        ".reason()",
        "to_lowercase",
        "starts_with",
        "const MIN",
        "const MAX",
        "fn is_healthy",
        "fn score",
        "enum Verdict",
        "fn acceptable",
        "fn too_much",
    ] {
        assert!(
            !code.contains(interdit),
            "« {interdit} » ferait de cette part un jugement, ou la ferait dépendre du texte libre"
        );
    }
}

/// **Un motif trompeur ne déplace pas la part.**
///
/// La garde précédente lit la source ; celle-ci exerce le comportement, comme en `W21.m`.
#[test]
fn un_motif_trompeur_ne_deplace_pas_la_part() {
    let mut compte = compte();
    let menteuse = id::<ReservationKind>(10);
    let tenue = compte
        .reserve_for(
            menteuse,
            &jetons(500),
            "coordination handoff négociation",
            Spend::Work,
        )
        .expect("retenue licite");
    compte
        .consume(tenue, &jetons(500), "coordination coordination")
        .expect("constat licite");
    depenser(&mut compte, 11, 500, Some(Spend::Coordination));

    let releve = Communication::over(compte.entries()).expect("journal cohérent");

    assert_eq!(
        releve.share(),
        Share::Measured(0.5),
        "le motif dit le contraire de la déclaration, et c'est la déclaration qui compte"
    );
    assert_eq!(releve.work(), 500);
}
