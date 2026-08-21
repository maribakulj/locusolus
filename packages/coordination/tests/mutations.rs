//! Test de sortie de `W21.a` — **`mutations_per_run`**, ADR 0024.
//!
//! 1. Le compte se rejoue à l'identique : un préfixe puis le reste valent la suite entière.
//! 2. Il compte les opérations **appliquées**, jamais les proposées — et c'est le rejeu qui le
//!    tient, pas une convention d'appel.
//! 3. Les dix sortes d'[`Operation::NAMES`] sont toujours présentes, à zéro s'il le faut.
//! 4. Aucun seuil, aucune note, aucun verdict.

use std::fmt::Write as _;

use locus_coordination::{
    CoordinationMode, Digest, Mutations, MutationsError, Operation, Relation, RelationKind,
    Version, VersionError,
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

/// Une racine à trois membres et une arête de revue.
fn root() -> Version {
    Version::root(
        &[agent(1), agent(2), agent(3)],
        &[reviews(1, 2)],
        CoordinationMode::Blackboard,
        None,
        &Fnv,
    )
    .expect("la fixture est licite")
}

/// Une suite qui applique cinq opérations de quatre sortes.
fn suite() -> Vec<Operation> {
    vec![
        Operation::AddNode(agent(4)),
        Operation::AddEdge(reviews(3, 4)),
        Operation::AddEdge(reviews(4, 1)),
        Operation::RemoveEdge(reviews(1, 2)),
        Operation::SetRole {
            node: agent(2),
            from: None,
            to: Some("relecteur".to_owned()),
        },
    ]
}

// ---------------------------------------------------------------------------------------------
// 1. Le rejeu est reproductible, et se découpe
// ---------------------------------------------------------------------------------------------

/// **Deux calculs sur le même préfixe rendent la même valeur.**
///
/// La propriété que la matrice exige de toute la famille — « calculées depuis le seul journal » n'a
/// de sens que si deux lectures du même journal s'accordent.
#[test]
fn deux_rejeux_du_meme_prefixe_rendent_la_meme_valeur() {
    let (root, suite) = (root(), suite());
    let une = Mutations::replay(&root, &suite, &Fnv).expect("la suite s'applique");
    let deux = Mutations::replay(&root, &suite, &Fnv).expect("la suite s'applique");

    assert_eq!(une.mutations(), deux.mutations());
    assert_eq!(une.landed(), deux.landed());
}

/// **Un préfixe puis le reste valent la suite entière**, compte comme version atteinte.
///
/// C'est la forme forte de la reproductibilité, et la seule qui ait un contenu : rappeler deux fois
/// la même fonction pure ne prouve que sa pureté. Découper la suite prouve que le compte est une
/// fonction de l'**histoire**, et non de la façon dont on la lit.
#[test]
fn un_prefixe_puis_le_reste_valent_la_suite_entiere() {
    let (root, suite) = (root(), suite());
    let entier = Mutations::replay(&root, &suite, &Fnv).expect("la suite s'applique");

    let (debut, fin) = suite.split_at(2);
    let premier = Mutations::replay(&root, debut, &Fnv).expect("le préfixe s'applique");
    let second = Mutations::replay(premier.landed(), fin, &Fnv).expect("le reste s'applique");

    assert_eq!(
        second.landed(),
        entier.landed(),
        "le rejeu découpé atterrit ailleurs que le rejeu entier"
    );
    for sort in Operation::NAMES {
        let attendu = entier.mutations().of_sort(sort).expect("sorte connue");
        let obtenu = premier.mutations().of_sort(sort).expect("sorte connue")
            + second.mutations().of_sort(sort).expect("sorte connue");
        assert_eq!(
            obtenu, attendu,
            "les comptes de « {sort} » ne s'additionnent pas"
        );
    }
    assert_eq!(
        premier.mutations().total() + second.mutations().total(),
        entier.mutations().total()
    );
}

// ---------------------------------------------------------------------------------------------
// 2. Appliquée, jamais proposée
// ---------------------------------------------------------------------------------------------

/// **Une opération que la version refuse fait échouer le rejeu, et n'est jamais comptée.**
///
/// C'est ici que « appliquée » cesse d'être une promesse d'appelant. Le refus vient de
/// `Version::apply`, que ce test n'a pas eu à simuler : il propose de retirer un nœud absent, et
/// c'est la version qui tranche.
#[test]
fn une_operation_refusee_arrete_le_rejeu_en_nommant_son_rang() {
    let proposees = vec![
        Operation::AddNode(agent(4)),
        // Le nœud 9 n'a jamais été membre. Une proposition, pas une opération appliquée.
        Operation::RemoveNode(agent(9)),
        Operation::AddEdge(reviews(3, 4)),
    ];

    let refus =
        Mutations::replay(&root(), &proposees, &Fnv).expect_err("le rang 1 ne s'applique pas");
    let MutationsError::NotApplicable { index, sort, cause } = refus;

    assert_eq!(
        index, 1,
        "le refus doit nommer le rang, sinon on cherche partout"
    );
    assert_eq!(sort, "REMOVE_NODE");
    assert!(
        matches!(cause, VersionError::NoSuchNode { .. }),
        "la cause doit venir de la version, pas d'une vérification refaite ici : {cause}"
    );
}

/// **Le rejeu ne saute pas l'opération fautive pour compter les suivantes.**
///
/// Sauter produirait un compte pour une histoire qui n'a pas eu lieu — un nombre plausible, que
/// rien dans son apparence ne distinguerait d'un nombre juste. Il n'existe donc aucun chemin qui
/// rende un `Mutations` depuis une suite dont un élément a été refusé, et ce test le tient par
/// l'absence : le seul constructeur est `replay`, et il rend `Err`.
#[test]
fn aucun_compte_ne_sort_d_une_suite_partiellement_applicable() {
    let proposees = vec![
        Operation::AddNode(agent(4)),
        Operation::RemoveNode(agent(9)),
        Operation::AddNode(agent(5)),
    ];

    assert!(
        Mutations::replay(&root(), &proposees, &Fnv).is_err(),
        "une suite dont un élément est refusé ne doit produire aucun compte"
    );
}

/// **Le chemin parcouru n'est pas la destination atteinte.**
///
/// Ajouter une arête puis la retirer compte **deux**, et la version revient exactement d'où elle
/// venait. C'est correct, et c'est ce qui distingue `W21.a` de `W21.c` : leur écart est le travail
/// de coordination qui n'a laissé aucune trace.
#[test]
fn un_aller_retour_compte_deux_et_ne_change_rien() {
    let root = root();
    let aller_retour = vec![
        Operation::AddEdge(reviews(2, 3)),
        Operation::RemoveEdge(reviews(2, 3)),
    ];

    let rejeu = Mutations::replay(&root, &aller_retour, &Fnv).expect("l'aller-retour s'applique");

    assert_eq!(rejeu.mutations().total(), 2);
    assert_eq!(rejeu.mutations().of_sort("ADD_EDGE"), Some(1));
    assert_eq!(rejeu.mutations().of_sort("REMOVE_EDGE"), Some(1));
    assert_eq!(
        rejeu.landed().relations(),
        root.relations(),
        "la destination est la même, seul le chemin a coûté"
    );
}

// ---------------------------------------------------------------------------------------------
// 3. Les dix sortes, toujours
// ---------------------------------------------------------------------------------------------

/// **Les clés du compteur sont exactement `Operation::NAMES`.**
///
/// Le test d'exhaustivité que l'item demande. Une onzième sorte qui entrerait dans l'énumération
/// sans entrer dans `NAMES` ferait apparaître une clé inconnue au premier rejeu qui la rencontre,
/// et l'égalité échouerait — le compteur aurait une lacune que personne n'a décidée.
#[test]
fn les_cles_sont_exactement_les_dix_sortes() {
    let rejeu = Mutations::replay(&root(), &suite(), &Fnv).expect("la suite s'applique");

    let mut attendues: Vec<&str> = Operation::NAMES.to_vec();
    attendues.sort_unstable();
    let obtenues: Vec<&str> = rejeu.mutations().by_sort().keys().copied().collect();

    assert_eq!(obtenues, attendues);
}

/// **Une sorte qui n'est pas survenue vaut zéro, et zéro est un fait.**
///
/// Y compris sur un rejeu vide : les dix clés sont là avant qu'aucune opération n'ait eu lieu.
#[test]
fn un_rejeu_vide_porte_les_dix_sortes_a_zero() {
    let rejeu = Mutations::replay(&root(), &[], &Fnv).expect("une suite vide s'applique");

    assert_eq!(rejeu.mutations().total(), 0);
    assert_eq!(rejeu.mutations().by_sort().len(), Operation::NAMES.len());
    for sort in Operation::NAMES {
        assert_eq!(
            rejeu.mutations().of_sort(sort),
            Some(0),
            "« {sort} » manque"
        );
    }
    assert_eq!(rejeu.landed(), &root(), "un rejeu vide ne déplace rien");
}

/// **Un nom que le compteur ne connaît pas rend `None`, jamais zéro.**
///
/// Les deux se lisent « il n'y en a pas eu », et l'un des deux est faux. Un appelant qui interroge
/// une sorte mal orthographiée doit l'apprendre plutôt que de lire une absence rassurante.
#[test]
fn une_sorte_inconnue_rend_none_et_non_zero() {
    let rejeu = Mutations::replay(&root(), &suite(), &Fnv).expect("la suite s'applique");

    assert_eq!(rejeu.mutations().of_sort("ADD_NOD"), None);
    assert_eq!(rejeu.mutations().of_sort("add_node"), None);
    assert_eq!(rejeu.mutations().of_sort("SPLIT_NODE"), Some(0));
}

// ---------------------------------------------------------------------------------------------
// 4. Rien ne juge
// ---------------------------------------------------------------------------------------------

/// **Aucun seuil, aucune note, aucun verdict dans la source.**
///
/// Décision 9 de l'ADR 0024. Les motifs visent des **formes de code** — une déclaration de
/// constante, une signature, une déclaration de type — et non des mots : un test qui refuserait le
/// mot « seuil » mordrait sur la phrase qui explique pourquoi il n'y en a pas, ce qui est arrivé
/// six fois dans ce dépôt et a chaque fois été réparé dans le même sens.
#[test]
fn la_source_ne_porte_aucun_jugement() {
    let source = include_str!("../src/mutations.rs");

    for interdit in [
        "const MIN",
        "const MAX",
        "fn is_healthy",
        "fn score",
        "enum Verdict",
        "fn verdict",
    ] {
        assert!(
            !source.contains(interdit),
            "« {interdit} » ferait de ce compteur un juge : un seuil écrit en Rust a l'apparence \
             d'un fait mesuré alors que c'est une décision de politique"
        );
    }
}
