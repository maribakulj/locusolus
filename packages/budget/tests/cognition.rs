//! Test de sortie de `W25.b` — **le plafond de cognition.**
//!
//! Trois clauses, celles de la roadmap :
//!
//! 1. le plafond est une dimension au sens de §7.2, et son dépassement refuse en **nommant la
//!    dimension** ;
//! 2. il se combine avec la classification de `W21.m` — une dépense de coordination et une dépense
//!    de travail ne s'imputent pas au même plafond ;
//! 3. une dépense **non classée** n'entre dans aucun plafond, exactement comme `W21.l` la traite, et
//!    ne devient pas « travail » par défaut.

use locus_budget::{Charge, CognitionLimits, Dimension, Limits, Spend, Verdict};
use locus_domain::CognitionClass;

/// Un plafond sur une seule dimension.
fn borne(dimension: Dimension, valeur: u64) -> Limits {
    Limits::bounding([(dimension, valeur)]).expect("au moins une dimension bornée")
}

/// Le plafond de référence : la frontière serrée, l'économie large — le levier de l'ADR 0026.
fn plafonds() -> CognitionLimits {
    CognitionLimits::none()
        .bounding(
            Charge {
                class: CognitionClass::Frontier,
                spend: Spend::Work,
            },
            borne(Dimension::ModelCalls, 10),
        )
        .bounding(
            Charge {
                class: CognitionClass::Economy,
                spend: Spend::Work,
            },
            borne(Dimension::ModelCalls, 10_000),
        )
}

// ---------------------------------------------------------------------------------------------
// 1. Le refus nomme la dimension
// ---------------------------------------------------------------------------------------------

/// Un dépassement **nomme la dimension**, ce qui était permis et ce qui a été demandé.
#[test]
fn un_depassement_nomme_la_dimension() {
    let verdict = plafonds().admits(
        CognitionClass::Frontier,
        Spend::Work.into(),
        Dimension::ModelCalls,
        11,
    );

    match verdict {
        Verdict::Over {
            charge,
            dimension,
            ceiling,
            requested,
        } => {
            assert_eq!(charge.class, CognitionClass::Frontier);
            assert_eq!(dimension, Dimension::ModelCalls);
            assert_eq!(ceiling, 10);
            assert_eq!(requested, 11);
        }
        autre => panic!("un dépassement, et il nomme sa dimension : {autre:?}"),
    }
    assert_eq!(verdict.dimension(), Some(Dimension::ModelCalls));
}

/// Une dimension **non bornée** dans un plafond existant se distingue d'un dépassement.
///
/// Les deux refusent, et un exploitant n'a pas la même chose à faire : dans un cas il relève une
/// borne, dans l'autre il en pose une. Un booléen les confondrait.
#[test]
fn une_dimension_non_bornee_se_distingue_d_un_depassement() {
    let verdict = plafonds().admits(
        CognitionClass::Frontier,
        Spend::Work.into(),
        Dimension::Tokens,
        1,
    );
    assert!(matches!(verdict, Verdict::Unbounded { .. }), "{verdict:?}");
    assert_eq!(verdict.dimension(), Some(Dimension::Tokens));
    assert!(!verdict.is_admitted());
}

/// Sous la borne, et **exactement à** la borne, la dépense passe.
///
/// Les deux bouts : une garde qui ne dirait que « refusé » serait exacte et inutile, et l'égalité
/// est le cas où une erreur d'inégalité stricte se cache.
#[test]
fn sous_la_borne_et_exactement_a_la_borne_passent() {
    for montant in [0_u64, 1, 9, 10] {
        let verdict = plafonds().admits(
            CognitionClass::Frontier,
            Spend::Work.into(),
            Dimension::ModelCalls,
            montant,
        );
        assert!(verdict.is_admitted(), "{montant} : {verdict:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// 2. Coordination et travail ne s'imputent pas au même plafond
// ---------------------------------------------------------------------------------------------

/// **Le même montant, la même classe, la même dimension — et deux verdicts.**
///
/// C'est la clause 2 réduite à ce qui la distingue : seule la classification change. Le plafond de
/// référence ne borne que le travail, donc la coordination est hors budget — et non « admise parce
/// qu'aucun plafond ne la refuse ».
#[test]
fn coordination_et_travail_ne_s_imputent_pas_au_meme_plafond() {
    let plafonds = plafonds();
    let travail = plafonds.admits(
        CognitionClass::Frontier,
        Spend::Work.into(),
        Dimension::ModelCalls,
        5,
    );
    let coordination = plafonds.admits(
        CognitionClass::Frontier,
        Spend::Coordination.into(),
        Dimension::ModelCalls,
        5,
    );

    assert!(travail.is_admitted());
    assert!(
        matches!(coordination, Verdict::OutsideBudget { .. }),
        "{coordination:?}"
    );
}

/// **Le levier de l'ADR 0026** : la frontière est serrée, l'économie est large.
///
/// Même dépense, même classification, même dimension — et c'est la **classe** qui décide. Sans cette
/// distinction, « frontière pour planifier, bon marché pour exécuter » ne serait pas exprimable.
#[test]
fn la_classe_decide_a_dimension_et_montant_egaux() {
    let plafonds = plafonds();
    let frontiere = plafonds.admits(
        CognitionClass::Frontier,
        Spend::Work.into(),
        Dimension::ModelCalls,
        100,
    );
    let economie = plafonds.admits(
        CognitionClass::Economy,
        Spend::Work.into(),
        Dimension::ModelCalls,
        100,
    );

    assert!(matches!(frontiere, Verdict::Over { .. }), "{frontiere:?}");
    assert!(economie.is_admitted(), "{economie:?}");
}

/// **Les quatre couples sont énumérables**, et c'est ce que le typage de la classe achète.
///
/// Un plafond indexé par slug ne permettrait pas de répondre à « quelles clés sont couvertes ? ».
/// Ici la question est décidable, et ce test la pose : deux couples bornés, deux hors budget.
#[test]
fn les_quatre_couples_sont_enumerables_et_leur_couverture_est_decidable() {
    let plafonds = plafonds();
    let tous = Charge::all();
    assert_eq!(tous.len(), 4, "deux classes × deux dépenses");

    let couverts: Vec<Charge> = tous
        .iter()
        .copied()
        .filter(|charge| plafonds.ceiling(*charge).is_some())
        .collect();
    assert_eq!(couverts.len(), 2);
    assert!(couverts.iter().all(|charge| charge.spend == Spend::Work));
    assert_eq!(plafonds.charges().count(), 2);
}

// ---------------------------------------------------------------------------------------------
// 3. Une dépense non classée n'entre dans aucun plafond
// ---------------------------------------------------------------------------------------------

/// **Non classée n'est pas « travail »**, et ce n'est pas non plus une permission.
///
/// `W21.l` et `W21.m` l'ont posé, et le rejouer autrement ici ferait deux règles pour une. Une
/// dépense qu'on ne sait pas classer ne peut pas être autorisée, puisqu'on ne sait pas contre quoi la
/// compter.
#[test]
fn une_depense_non_classee_n_entre_dans_aucun_plafond() {
    let plafonds = plafonds();
    let verdict = plafonds.admits(
        CognitionClass::Economy,
        locus_budget::Classification::Unclassified,
        Dimension::ModelCalls,
        1,
    );

    assert_eq!(verdict, Verdict::Unclassified);
    assert!(!verdict.is_admitted(), "non classée n'est pas admise");
    assert_eq!(
        verdict.dimension(),
        None,
        "la dimension n'a pas été atteinte : chercher un plafond de dimension serait chercher là où \
         il n'y a rien"
    );

    // Et la même dépense, **classée**, passe : le refus vient de l'ignorance, pas du montant.
    assert!(
        plafonds
            .admits(
                CognitionClass::Economy,
                Spend::Work.into(),
                Dimension::ModelCalls,
                1
            )
            .is_admitted()
    );
}

/// **Aucun plafond n'est « illimité »**, et l'absence de plafond ne laisse rien passer.
///
/// `Limits` le pose déjà pour ses dimensions ; le plafond de cognition en hérite. L'inverse ferait
/// d'un oubli de configuration une autorisation de dépenser — le silence lu comme un accord.
#[test]
fn aucun_plafond_ne_laisse_rien_passer() {
    let vides = CognitionLimits::none();
    assert_eq!(vides.charges().count(), 0);

    for charge in Charge::all() {
        let verdict = vides.admits(charge.class, charge.spend.into(), Dimension::ModelCalls, 0);
        assert!(
            matches!(verdict, Verdict::OutsideBudget { .. }),
            "{charge:?} : {verdict:?}"
        );
    }
}

/// Reposer un couple **remplace** son plafond.
///
/// Deux valeurs simultanées pour une même clé rendraient la borne dépendante de l'ordre
/// d'insertion, et deux configurations identiques cesseraient de l'être.
#[test]
fn reposer_un_couple_remplace_son_plafond() {
    let charge = Charge {
        class: CognitionClass::Frontier,
        spend: Spend::Work,
    };
    let resserre = plafonds().bounding(charge, borne(Dimension::ModelCalls, 2));

    assert_eq!(resserre.charges().count(), 2, "toujours deux couples");
    assert!(matches!(
        resserre.admits(charge.class, charge.spend.into(), Dimension::ModelCalls, 5),
        Verdict::Over { ceiling: 2, .. }
    ));
}
