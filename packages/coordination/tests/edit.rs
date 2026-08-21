//! Test de sortie de `W21.c` — **`applied_edit_length`**, ADR 0024.
//!
//! 1. La longueur mesurée n'est **pas** minimale, et le test l'exhibe plutôt que de le déduire.
//! 2. L'écart avec `W21.a` est le détour, calculé et non décrit.
//! 3. Le détour n'existe que lorsque les deux mesures partagent un vocabulaire.
//! 4. Aucune signature ne porte le nom de la distance de graphe.

use std::fmt::Write as _;

use locus_coordination::{
    AppliedEdit, CoordinationMode, Digest, Mutations, Operation, Relation, RelationKind, Version,
};
use locus_domain::ContentHash;
use locus_protocol::{Id, IdKind, Timestamp, id::Agent};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn agent(seed: u8) -> Id<Agent> {
    id::<Agent>(seed)
}

fn reviews(from: u8, to: u8) -> Relation {
    Relation {
        from: agent(from),
        to: agent(to),
        kind: RelationKind::Review,
    }
}

const PRIME: u64 = 0x0000_0100_0000_01b3;
const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

struct Fnv;

impl Digest for Fnv {
    fn digest(&self, canonical: &str) -> ContentHash {
        let mut digest = String::with_capacity(64);
        for salt in 0_u64..4 {
            let mut hash = OFFSET ^ salt.wrapping_mul(PRIME);
            for byte in canonical.bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(PRIME);
            }
            write!(digest, "{hash:016x}").expect("écrire dans une String n'échoue pas");
        }
        ContentHash::parse(&format!("sha256:{digest}")).expect("64 hexadécimaux minuscules")
    }
}

fn base() -> Version {
    Version::root(
        &[agent(1), agent(2), agent(3)],
        &[reviews(1, 2)],
        CoordinationMode::Blackboard,
        None,
        &Fnv,
    )
    .expect("la fixture est licite")
}

// ---------------------------------------------------------------------------------------------
// 1. La longueur n'est pas minimale, et voici le contre-exemple
// ---------------------------------------------------------------------------------------------

/// **Un remplacement coûte une opération au chemin et quatre au diff.**
///
/// Le contre-exemple qui rend concrète la borne supérieure. `Diff::between` n'émet que quatre
/// sortes d'opérations et n'infère jamais un `REPLACE_NODE` : au niveau des états, un remplacement
/// est indiscernable d'un retrait suivi d'un ajout, et deviner ferait lire à un approbateur une
/// intention que personne n'a écrite.
///
/// La longueur mesurée est donc **plus grande** que le nombre minimal d'opérations qui mènent d'une
/// version à l'autre — un `REPLACE_NODE` suffit, et le diff en écrit quatre. Publier cela sous le
/// nom de « distance d'édition » aurait été faux, et c'est ce que la décision 2 de l'ADR 0024
/// évite.
#[test]
fn un_remplacement_se_relit_en_quatre_operations() {
    let base = base();
    let remplacement = [Operation::ReplaceNode {
        from: agent(1),
        to: agent(5),
    }];

    let chemin = Mutations::replay(&base, &remplacement, &Fnv).expect("le remplacement s'applique");
    let edit = AppliedEdit::between(&base, chemin.landed());

    assert_eq!(
        chemin.mutations().total(),
        1,
        "le chemin réel : une opération"
    );
    assert_eq!(edit.length(), 4, "le diff : quatre");

    let sortes: Vec<&str> = edit.operations().iter().map(Operation::name).collect();
    assert_eq!(
        sortes,
        ["REMOVE_EDGE", "REMOVE_NODE", "ADD_NODE", "ADD_EDGE"],
        "le diff décrit l'écart d'états, il n'invente pas l'intention"
    );
    assert!(
        !sortes.contains(&"REPLACE_NODE"),
        "un diff qui inférerait le remplacement ferait lire une intention que personne n'a écrite"
    );

    // Les **sortes** ne disent pas le sens : un diff pris à l'envers produit exactement la même
    // séquence. Un mutant qui inversait les deux versions a donc survécu aux assertions ci-dessus.
    // Seuls les opérandes distinguent « on retire 1 et on ajoute 5 » de son contraire.
    assert!(
        edit.operations().contains(&Operation::RemoveNode(agent(1))),
        "le diff doit retirer l'identité sortante, pas l'entrante"
    );
    assert!(
        edit.operations().contains(&Operation::AddNode(agent(5))),
        "le diff doit ajouter l'identité entrante, pas la sortante"
    );
    assert!(!edit.operations().contains(&Operation::RemoveNode(agent(5))));
    assert!(!edit.operations().contains(&Operation::AddNode(agent(1))));
}

// ---------------------------------------------------------------------------------------------
// 2 et 3. Le détour, et quand il n'a pas de sens
// ---------------------------------------------------------------------------------------------

/// **Un aller-retour coûte deux au chemin, zéro au diff : le détour vaut deux.**
///
/// Le détour est **calculé**, pas décrit — décision 3 de l'ADR 0024. C'est le travail de
/// coordination qui n'a laissé aucune trace dans la structure finale.
#[test]
fn un_aller_retour_donne_un_detour_de_deux() {
    let base = base();
    let aller_retour = [
        Operation::AddEdge(reviews(2, 3)),
        Operation::RemoveEdge(reviews(2, 3)),
    ];

    let chemin = Mutations::replay(&base, &aller_retour, &Fnv).expect("l'aller-retour s'applique");
    let edit = AppliedEdit::between(&base, chemin.landed());

    assert_eq!(chemin.mutations().total(), 2);
    assert_eq!(edit.length(), 0);
    assert!(edit.is_empty());
    assert_eq!(edit.detour_from(chemin.mutations().total()), Some(2));
}

/// **Un chemin sans détour rend zéro, et non l'absence de détour.**
///
/// `Some(0)` et `None` disent deux choses différentes : « il n'y a pas eu de détour » et « ces deux
/// mesures ne se comparent pas ici ». Les confondre ferait lire la seconde comme la première.
#[test]
fn un_chemin_direct_donne_un_detour_nul() {
    let base = base();
    let simple = [Operation::AddEdge(reviews(2, 3))];

    let chemin = Mutations::replay(&base, &simple, &Fnv).expect("l'ajout s'applique");
    let edit = AppliedEdit::between(&base, chemin.landed());

    assert_eq!(chemin.mutations().total(), 1);
    assert_eq!(edit.length(), 1);
    assert_eq!(edit.detour_from(chemin.mutations().total()), Some(0));
    // Exercé à la longueur **un**, le seul endroit où `is_empty` peut mentir sans qu'on le voie :
    // aucun test ne l'y touchait, et un mutant lisant `len() <= 1` a survécu.
    assert!(
        !edit.is_empty(),
        "une opération d'écart n'est pas une absence d'écart"
    );
}

/// **Quand le chemin est plus court que le diff, le détour n'existe pas — et se dit `None`.**
///
/// Trouvé en mesurant plutôt qu'en supposant : la décision 3 de l'ADR énonçait le détour comme un
/// écart, ce qui suppose que le chemin est toujours au moins aussi long que le diff. C'est faux dès
/// que le chemin emploie une opération que le diff ne sait pas exprimer.
///
/// Une soustraction signée aurait rendu `-3`, qui serait affiché et lu comme « moins que rien »
/// alors qu'il signifie « ces deux mesures ne comptent pas dans le même vocabulaire ».
#[test]
fn un_chemin_plus_court_que_le_diff_n_a_pas_de_detour() {
    let base = base();
    let remplacement = [Operation::ReplaceNode {
        from: agent(1),
        to: agent(5),
    }];

    let chemin = Mutations::replay(&base, &remplacement, &Fnv).expect("le remplacement s'applique");
    let edit = AppliedEdit::between(&base, chemin.landed());

    assert!(chemin.mutations().total() < edit.length());
    assert_eq!(
        edit.detour_from(chemin.mutations().total()),
        None,
        "un détour négatif serait affiché, et lu comme une quantité"
    );
}

/// **Deux versions identiques n'ont aucun écart.**
#[test]
fn deux_versions_identiques_ont_une_longueur_nulle() {
    let edit = AppliedEdit::between(&base(), &base());

    assert_eq!(edit.length(), 0);
    assert!(edit.is_empty());
    assert!(edit.operations().is_empty());
    assert_eq!(edit.detour_from(0), Some(0));
}

// ---------------------------------------------------------------------------------------------
// 4. Le nom que ce module ne porte pas
// ---------------------------------------------------------------------------------------------

/// **Aucune signature ne s'appelle du nom de la distance de graphe, et rien ne juge.**
///
/// Les motifs visent des **signatures**, pas des mots : la documentation du module explique
/// longuement pourquoi ce n'est pas une distance, et un test qui refuserait le mot mordrait sur son
/// propre motif.
#[test]
fn la_source_ne_promet_aucune_distance_ni_verdict() {
    let source = include_str!("../src/edit.rs");

    for interdit in [
        "fn graph_edit_distance",
        "fn distance",
        "fn ged",
        "fn minimal",
        "const MIN",
        "const MAX",
        "fn is_healthy",
        "fn score",
        "enum Verdict",
        "fn verdict",
    ] {
        assert!(
            !source.contains(interdit),
            "« {interdit} » affirmerait une minimalité qu'aucun code ici ne calcule"
        );
    }
}
