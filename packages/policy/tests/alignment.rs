//! Test de sortie de `W14.e` — **l'alignement d'ontologies comme proposition.**
//!
//! Quatre propriétés, celles du tableau de `docs/10` :
//!
//! 1. un alignement proposé est une proposition soumise à politique et approbation ;
//! 2. **aucun chemin n'écrit une équivalence sans décision**, tenu par l'absence ;
//! 3. le refus nomme la **contrainte structurelle** non satisfaite plutôt que de rendre un score ;
//! 4. deux propositions contradictoires sur la même paire ne committent pas toutes deux.

use locus_policy::{AlignmentError, AlignmentProposal, Alignments, Category, Equivalence, approve};

fn proposition(gauche: &str, droite: &str, base: u64) -> AlignmentProposal {
    AlignmentProposal::propose(gauche, droite, Equivalence::ExactMatch, "usr-marie", base)
        .expect("une paire distincte et nommée")
}

// ---------------------------------------------------------------------------------------------
// 1 et 2 — la décision est le seul chemin
// ---------------------------------------------------------------------------------------------

/// **Le registre n'accepte qu'un alignement approuvé**, et l'approbation ne se fabrique pas.
///
/// `ApprovedAlignment` n'a aucun champ public et aucun constructeur : `approve` est la seule porte.
/// C'est ce qui rend « aucun chemin n'écrit une équivalence sans décision » vrai **par signature**
/// plutôt que par vigilance.
#[test]
fn une_equivalence_ne_s_ecrit_qu_apres_approbation() {
    let mut registre = Alignments::new();
    assert_eq!(registre.revision(), 0);
    assert!(registre.partner("cidoc:E22").is_none());

    let approuve = approve(proposition("cidoc:E22", "bf:Work", 0), "usr-gov").expect("distinct");
    assert_eq!(registre.commit(&approuve), Ok(1));

    // L'appariement est **symétrique** : les deux termes se retrouvent l'un par l'autre.
    assert_eq!(
        registre.partner("cidoc:E22"),
        Some(("bf:Work", Equivalence::ExactMatch))
    );
    assert_eq!(
        registre.partner("bf:Work"),
        Some(("cidoc:E22", Equivalence::ExactMatch))
    );
}

/// Un proposeur n'approuve pas sa propre proposition — la borne de `coordination::approve`.
#[test]
fn un_proposeur_n_approuve_pas_son_propre_alignement() {
    let refus = approve(proposition("a", "b", 0), "usr-marie").expect_err("même personne");
    assert_eq!(
        refus,
        AlignmentError::SelfApproval {
            author: "usr-marie".to_owned()
        }
    );
}

/// **Aucun chemin ne décide par similarité**, tenu par l'absence.
///
/// Les motifs visent des **signatures et des champs**, pas des mots : la documentation du module
/// emploie « similarité » pour dire précisément ce qu'il ne fait pas, et une garde qui se déclenche
/// sur sa propre justification est une garde qu'on finit par assouplir. C'est la sixième fois de
/// cette série que la distinction compte.
#[test]
fn aucun_chemin_ne_decide_par_similarite() {
    let source = include_str!("../src/alignment.rs");
    for interdit in [
        "score: f64",
        "confidence: f64",
        "threshold",
        "fn auto_align",
        "fn infer_alignment",
        "impl From<f64>",
        "fn commit_unapproved",
    ] {
        assert!(
            !source.contains(interdit),
            "« {interdit} » dans alignment.rs : un matcher propose, il ne décide jamais que deux \
             choses sont la même"
        );
    }

    // Et la proposition elle-même ne porte aucun score : en porter un inviterait à la trancher en
    // le comparant à un seuil, c'est-à-dire à décider par similarité.
    let proposee = proposition("a", "b", 0);
    let rendu = format!("{proposee:?}");
    assert!(!rendu.contains("score"), "{rendu}");
    assert!(!rendu.contains("confidence"), "{rendu}");
}

// ---------------------------------------------------------------------------------------------
// 3 — le refus nomme la contrainte, jamais un score
// ---------------------------------------------------------------------------------------------

/// **La contrainte structurelle est nommée, et le partenaire existant avec elle.**
///
/// C'est ce que l'ablation de l'ADR 0023 mesure : retirer l'appariement un-à-un fait chuter le F1
/// de 0,829 à 0,728, quand cinq pondérations de similarité s'écartent de 0,0033. Un refus qui
/// rendrait « 0,62 » enverrait son lecteur chercher un seuil, alors que le problème n'en est pas
/// un — aucune confiance supplémentaire ne libérerait un terme déjà apparié.
#[test]
fn le_refus_nomme_la_contrainte_structurelle_et_non_un_score() {
    let mut registre = Alignments::new();
    let premier = approve(proposition("cidoc:E22", "bf:Work", 0), "usr-gov").expect("distinct");
    registre.commit(&premier).expect("le premier passe");

    // Un second alignement sur un terme déjà apparié.
    let second = approve(
        proposition("cidoc:E22", "schema:CreativeWork", 1),
        "usr-gov",
    )
    .expect("distinct");
    let refus = registre.commit(&second).expect_err("E22 est déjà apparié");

    assert_eq!(
        refus,
        AlignmentError::AlreadyMatched {
            term: "cidoc:E22".to_owned(),
            partner: "bf:Work".to_owned(),
            relation: Equivalence::ExactMatch,
        }
    );

    let dit = refus.to_string();
    assert!(dit.contains("un-à-un"), "{dit}");
    assert!(
        dit.contains("bf:Work"),
        "la paire existante est nommée : {dit}"
    );
    assert!(
        !dit.contains("0,") && !dit.contains("0."),
        "aucun score dans le refus : {dit}"
    );

    // Et le refus n'a **rien écrit** : la révision n'a pas bougé.
    assert_eq!(registre.revision(), 1);
    assert!(registre.partner("schema:CreativeWork").is_none());
}

/// La contrainte vaut des **deux** côtés : le terme de droite aussi est protégé.
#[test]
fn la_contrainte_protege_les_deux_termes_de_la_paire() {
    let mut registre = Alignments::new();
    registre
        .commit(&approve(proposition("a", "b", 0), "usr-gov").expect("distinct"))
        .expect("le premier passe");

    // « c » est libre, mais « b » ne l'est pas.
    let refus = registre
        .commit(&approve(proposition("c", "b", 1), "usr-gov").expect("distinct"))
        .expect_err("b est déjà apparié");
    assert!(matches!(refus, AlignmentError::AlreadyMatched { term, .. } if term == "b"));
}

/// Un terme aligné sur lui-même n'aligne rien, et occuperait un appariement.
#[test]
fn un_alignement_reflexif_est_refuse() {
    let refus = AlignmentProposal::propose(
        "cidoc:E22",
        "cidoc:E22",
        Equivalence::SameAs,
        "usr-marie",
        0,
    )
    .expect_err("la même chose des deux côtés");
    assert_eq!(
        refus,
        AlignmentError::Reflexive {
            term: "cidoc:E22".to_owned()
        }
    );
}

// ---------------------------------------------------------------------------------------------
// 4 — deux propositions contradictoires ne committent pas toutes deux
// ---------------------------------------------------------------------------------------------

/// **Le CAS d'abord, la contrainte ensuite.**
///
/// Deux propositions écrites sur la même base s'opposent : la première commite, la seconde a été
/// écrite contre un monde qui n'existe plus. Le refus est `Stale` et **dit qu'il faut rebaser** —
/// et non `AlreadyMatched`, qui enverrait son auteur croire que sa paire est mauvaise alors qu'elle
/// est seulement en retard.
#[test]
fn deux_propositions_concurrentes_ne_committent_pas_toutes_deux() {
    let mut registre = Alignments::new();

    let une = approve(proposition("a", "b", 0), "usr-gov").expect("distinct");
    let autre = approve(proposition("c", "d", 0), "usr-gov").expect("distinct");

    assert_eq!(registre.commit(&une), Ok(1));

    let refus = registre
        .commit(&autre)
        .expect_err("écrite sur la révision 0");
    assert_eq!(
        refus,
        AlignmentError::Stale {
            expected: 0,
            actual: 1
        }
    );
    assert!(refus.to_string().contains("rebaser"), "{refus}");

    // Rebasée, la même paire passe : le refus portait sur la base, pas sur le contenu.
    let rebasee = approve(proposition("c", "d", 1), "usr-gov").expect("distinct");
    assert_eq!(registre.commit(&rebasee), Ok(2));
}

// ---------------------------------------------------------------------------------------------
// La catégorie de politique
// ---------------------------------------------------------------------------------------------

/// **`Alignment` est la dix-septième catégorie, signalée comme ajout local.**
///
/// §20.1 en énumère seize. La fondre dans la liste normative ferait passer un ajout pour une lecture
/// de la spec — la même discipline que les namespaces `projection` et `migration` de l'event store.
#[test]
fn la_categorie_d_alignement_est_un_ajout_local_assume() {
    assert_eq!(Category::ALL.len(), 17);
    assert_eq!(Category::from_slug("alignment"), Some(Category::Alignment));
    assert_eq!(Category::Alignment.slug(), "alignment");

    // Les seize de §20.1 n'ont pas bougé — ni leur nombre, ni leur ordre, ni leurs noms.
    let seize = [
        "spawn",
        "model-routing",
        "team-coordination",
        "information-sharing",
        "budget",
        "scheduling",
        "sandbox-and-network",
        "secrets",
        "review",
        "validation",
        "branch-and-termination",
        "publication",
        "retention",
        "federation",
        "disciplinary-compliance",
        "human-escalation",
    ];
    for (categorie, nom) in Category::ALL.into_iter().zip(seize) {
        assert_eq!(categorie.slug(), nom, "§20.1 ne se réécrit pas");
    }
}

/// Les trois sortes d'équivalence ne sont pas interchangeables.
///
/// `SameAs` porte sur des individus et se propage par transitivité, `EquivalentClass` sur des
/// classes, `ExactMatch` est une correspondance de vocabulaire qui ne promet **aucune** inférence.
/// Les confondre ferait tirer d'un rapprochement de thésaurus des conclusions logiques que personne
/// n'a autorisées.
#[test]
fn les_trois_equivalences_se_lisent_sous_leur_nom() {
    let noms = ["owl:equivalentClass", "skos:exactMatch", "owl:sameAs"];
    assert_eq!(Equivalence::ALL.len(), 3);
    for (equivalence, nom) in Equivalence::ALL.into_iter().zip(noms) {
        assert_eq!(equivalence.slug(), nom);
    }
}
