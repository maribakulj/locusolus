//! Test de sortie de `W17.l` — **le retrieval est un plan.**
//!
//! Six propriétés, celles du tableau de `docs/10` :
//!
//! 1. les six intentions se lisent sous leur nom ;
//! 2. trois intentions distinctes produisent trois **ordres de canaux** différents sur la même
//!    question — le test compare les **ordres**, pas les résultats ;
//! 3. un plan sans critère d'arrêt n'est pas constructible ;
//! 4. le plan porte l'**identité de la fonction de classement**, et une identité vide est refusée ;
//! 5. une escalade est enregistrée, et un résultat post-escalade se distingue **par son type** ;
//! 6. `Plan::compatible` reproduit exactement le comportement d'avant l'item, réserve nulle
//!    comprise.

use locus_domain::Confidentiality;
use locus_memory::{
    Candidate, Channel, Escalation, Excluded, Genre, Intent, Plan, PlanError, Provenance, Ranking,
    RankingIdentity, Signal, Stop, retrieve,
};

fn score(total: f64) -> Ranking {
    Ranking::of(&[(Signal::Lexical, total)]).expect("un score à un facteur est un score")
}

fn candidat(key: &str, genre: Genre, total: f64) -> Candidate {
    Candidate::new(key, Confidentiality::Internal, genre, score(total))
        .expect("aucun de ces genres n'est formel")
}

// ---------------------------------------------------------------------------------------------
// 1 et 2 — les six intentions, et les ordres qu'elles produisent
// ---------------------------------------------------------------------------------------------

#[test]
fn les_six_intentions_se_lisent_sous_leur_nom() {
    let noms = [
        "explanatory",
        "episodic",
        "formal",
        "bibliographic",
        "structural",
        "global",
    ];
    assert_eq!(Intent::ALL.len(), 6);
    for (intent, nom) in Intent::ALL.into_iter().zip(noms) {
        assert_eq!(intent.slug(), nom);
        assert_eq!(Intent::parse(nom), Some(intent));
    }
    assert_eq!(Intent::parse("exploratoire"), None);
}

/// **Trois intentions, trois ordres — et le test compare les ordres.**
///
/// Comparer les résultats ne prouverait rien : sur un corpus de fixtures, trois routages différents
/// peuvent rendre le même ensemble. Ce qui distingue une intention d'une autre est **par où elle
/// passe d'abord**, et c'est là que se paie ou s'économise le coût.
#[test]
fn trois_intentions_produisent_trois_ordres_de_canaux_differents() {
    let explicative = Intent::Explanatory.channels();
    let bibliographique = Intent::Bibliographic.channels();
    let globale = Intent::Global.channels();

    assert_ne!(explicative, bibliographique);
    assert_ne!(bibliographique, globale);
    assert_ne!(explicative, globale);

    // Et les premiers pas disent pourquoi : une explication part du graphe, une bibliographie de
    // l'identité exacte, une question globale d'un balayage large.
    assert_eq!(explicative.first(), Some(&Channel::GraphTraversal));
    assert_eq!(bibliographique.first(), Some(&Channel::ExactIdentifiers));
    assert_eq!(globale.first(), Some(&Channel::Lexical));

    // Aucune intention n'a d'ordre vide : un plan qui n'interroge rien n'est pas un plan.
    for intent in Intent::ALL {
        assert!(!intent.channels().is_empty(), "{intent}");
    }
}

/// **Une intention formelle ne passe jamais par le vectoriel.**
///
/// Ce n'est pas une préférence de routage mais la décision 2 de l'ADR 0022 : l'autorité d'un objet
/// formel est un vérificateur, et un score de proximité n'a aucune relation avec elle. Le refus
/// existe déjà à la construction du candidat ; ici il se lit **aussi** dans l'ordre des canaux, ce
/// qui évite de payer une route dont on refusera le résultat.
#[test]
fn l_intention_formelle_n_emprunte_pas_le_canal_vectoriel() {
    assert!(!Intent::Formal.channels().contains(&Channel::Vector));
}

// ---------------------------------------------------------------------------------------------
// 3 et 4 — ce qu'un plan ne peut pas taire
// ---------------------------------------------------------------------------------------------

/// **Le critère d'arrêt et l'identité de classement sont des paramètres, pas des options.**
///
/// Il n'existe aucun constructeur qui les omette : `Plan::new` les exige tous les deux, donc un plan
/// muet sur l'un des deux n'est pas représentable. C'est plus fort qu'une vérification, qui pourrait
/// être contournée par un second chemin.
#[test]
fn un_plan_declare_son_arret_et_sa_fonction_de_classement() {
    let plan = Plan::new(
        Intent::Episodic,
        10,
        0,
        Stop::ChannelsTried { after: 2 },
        RankingIdentity::named("bm25/1.4").expect("nom non vide"),
    )
    .expect("plan licite");

    assert_eq!(plan.stop(), Stop::ChannelsTried { after: 2 });
    assert_eq!(plan.ranking().as_str(), "bm25/1.4");
    assert!(plan.ranking().is_replayable());
    assert_eq!(plan.channels(), Intent::Episodic.channels());
}

/// Une identité vide est refusée : « non nommée » et « nommée par le vide » se liraient pareil.
#[test]
fn une_fonction_de_classement_sans_nom_est_refusee() {
    assert_eq!(RankingIdentity::named(""), Err(PlanError::UnnamedRanking));
    assert_eq!(
        RankingIdentity::named("   "),
        Err(PlanError::UnnamedRanking)
    );
}

/// **`caller-supplied` déclare son absence au lieu de la taire.**
///
/// C'est l'état réel du dépôt : `Ranking::of` reçoit des flottants calculés par l'appelant. Un reçu
/// peut donc dire « le rejeu n'est pas garanti » plutôt que de le promettre — la même discipline que
/// `None` contre `Some(0.0)` pour une couverture.
#[test]
fn une_identite_fournie_par_l_appelant_dit_qu_elle_ne_rejoue_pas() {
    let anonyme = RankingIdentity::caller_supplied();
    assert!(!anonyme.is_replayable());

    let nommee = RankingIdentity::named("bm25/1.4").expect("nom non vide");
    assert!(nommee.is_replayable());
    assert_ne!(anonyme, nommee);
}

/// Un budget nul et une réserve plus large que le budget sont refusés, chacun en le disant.
#[test]
fn un_plan_incoherent_est_refuse_en_nommant_ce_qui_cloche() {
    assert_eq!(Plan::compatible(0), Err(PlanError::EmptyBudget));

    let trop = Plan::new(
        Intent::Global,
        3,
        4,
        Stop::BudgetFilled,
        RankingIdentity::caller_supplied(),
    )
    .expect_err("une réserve de 4 dans un budget de 3");
    assert_eq!(
        trop,
        PlanError::ReserveExceedsBudget {
            reserve: 4,
            budget: 3,
        }
    );
}

// ---------------------------------------------------------------------------------------------
// 5 — l'escalade se lit dans le type
// ---------------------------------------------------------------------------------------------

/// **Un résultat post-escalade se distingue par son type, pas par une convention.**
///
/// Un préfixe de clé ou un drapeau se perdrait à la première sérialisation. Et la distinction
/// compte : un résultat trouvé après élargissement du périmètre n'a pas été obtenu sous les mêmes
/// contraintes d'isolation, dont §12.4 dépend.
#[test]
fn un_resultat_post_escalade_se_distingue_par_son_type() {
    let direct = candidat("c1", Genre::Semantic, 0.9);
    assert_eq!(direct.provenance(), &Provenance::Direct);
    assert!(direct.provenance().escalation().is_none());

    let escalade = Escalation::BroaderScope {
        requested: "branche voisine".to_owned(),
        granted_by: "usr-gov".to_owned(),
    };
    let apres = candidat("c2", Genre::Semantic, 0.8).obtained_after(escalade.clone());
    assert_eq!(
        apres.provenance(),
        &Provenance::AfterEscalation(escalade.clone())
    );
    assert_eq!(apres.provenance().escalation(), Some(&escalade));

    // La provenance survit au retrieval : un résultat rendu porte encore d'où il vient.
    let plan = Plan::compatible(10).expect("budget licite");
    let resultats = retrieve(&plan, &[direct, apres], Confidentiality::Internal);
    let provenances: Vec<&Provenance> = resultats
        .included()
        .iter()
        .map(Candidate::provenance)
        .collect();
    assert!(provenances.contains(&&Provenance::Direct));
    assert!(provenances.iter().any(|p| p.escalation().is_some()));
}

/// Les trois sortes d'escalade existent sous leur nom, et une escalade de périmètre **nomme qui l'a
/// accordée** — sans quoi ce serait un contournement plutôt qu'une escalade.
#[test]
fn les_trois_sortes_d_escalade_existent_sous_leur_nom() {
    let profondeur = Escalation::DeeperGraph {
        from_depth: 2,
        to_depth: 5,
    };
    let perimetre = Escalation::BroaderScope {
        requested: "programme".to_owned(),
        granted_by: "usr-gov".to_owned(),
    };
    let coprocesseur = Escalation::Coprocessor {
        capability_id: "cap-01H".to_owned(),
    };

    assert_ne!(profondeur, perimetre);
    assert_ne!(perimetre, coprocesseur);
    assert_ne!(profondeur, coprocesseur);
}

// ---------------------------------------------------------------------------------------------
// 6 — l'item est additif, et c'est un test
// ---------------------------------------------------------------------------------------------

/// **`Plan::compatible` reproduit le comportement d'avant `W17.l`.**
///
/// Le comportement d'avant : filtrer par habilitation, trier par score décroissant puis par clé
/// croissante, couper au budget, et **ne jamais lire `is_negative`**. C'est ce qui rend l'item
/// additif plutôt qu'un changement de comportement déguisé — et c'est pour cela que la réserve de
/// négatifs vit dans le plan et non dans le genre.
#[test]
fn le_plan_compatible_reproduit_le_comportement_d_avant() {
    let corpus = [
        candidat("b", Genre::Semantic, 0.5),
        candidat("a", Genre::Semantic, 0.5),
        candidat("c", Genre::Negative, 0.1),
        candidat("d", Genre::Semantic, 0.9),
    ];

    let plan = Plan::compatible(2).expect("budget licite");
    assert_eq!(plan.negative_reserve(), 0, "aucune réserve par défaut");

    let resultats = retrieve(&plan, &corpus, Confidentiality::Internal);
    let retenus: Vec<&str> = resultats.included().iter().map(Candidate::key).collect();

    // Score décroissant, puis clé croissante à score égal — et le négatif tombe, faute de réserve.
    assert_eq!(retenus, vec!["d", "a"]);
    assert_eq!(resultats.excluded().len(), 2);
    assert!(
        resultats
            .excluded()
            .iter()
            .any(|exclu| matches!(exclu, Excluded::BeyondBudget { key, .. } if key == "c")),
        "sans réserve, un négatif mal classé est exclu comme les autres"
    );
}

/// **Avec une réserve, le budget saturé exclut d'abord ailleurs.**
///
/// C'est la moitié qui change le comportement, et elle ne s'applique **que** sous un plan qui la
/// déclare. Les deux moitiés sont testées : sans réserve ci-dessus, avec réserve ici.
#[test]
fn sous_reserve_un_budget_sature_n_exclut_pas_un_negatif() {
    let corpus = [
        candidat("b", Genre::Semantic, 0.5),
        candidat("a", Genre::Semantic, 0.5),
        candidat("c", Genre::Negative, 0.1),
        candidat("d", Genre::Semantic, 0.9),
    ];

    let plan = Plan::new(
        Intent::Global,
        2,
        1,
        Stop::BudgetFilled,
        RankingIdentity::caller_supplied(),
    )
    .expect("plan licite");

    let resultats = retrieve(&plan, &corpus, Confidentiality::Internal);
    let retenus: Vec<&str> = resultats.included().iter().map(Candidate::key).collect();

    assert!(
        retenus.contains(&"c"),
        "le négatif tient sa place réservée : {retenus:?}"
    );
    assert_eq!(retenus.len(), 2, "le budget reste tenu : {retenus:?}");
    assert!(
        retenus.contains(&"d"),
        "et le mieux classé la garde : {retenus:?}"
    );
    // L'exclusion est tombée ailleurs — sur un candidat d'un autre genre.
    assert!(resultats.excluded().iter().any(
        |exclu| matches!(exclu, Excluded::BeyondBudget { key, .. } if key == "a" || key == "b")
    ));
}

/// Les dix `Signal` de §16.3 sont **inchangés** — l'item ajoute un axe, il n'en modifie pas un.
#[test]
fn les_dix_signaux_de_la_spec_sont_inchanges() {
    let noms = [
        "graph-traversal",
        "lexical",
        "vector",
        "exact-identifiers",
        "temporality",
        "validation-level",
        "branch-and-confidentiality",
        "source-diversity",
        "negative-results",
        "context-budget",
    ];
    assert_eq!(Signal::ALL.len(), 10);
    for (signal, nom) in Signal::ALL.into_iter().zip(noms) {
        assert_eq!(signal.slug(), nom, "§16.3 ne se réécrit pas");
    }
}
