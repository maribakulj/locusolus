//! Test de sortie de `W21.b` — **`edge_churn`**, ADR 0024.
//!
//! 1. Deux arêtes qui entrent et deux qui sortent rendent **quatre**, pas zéro.
//! 2. Le churn ne se déduit pas du compte d'arêtes.
//! 3. Il voit les arêtes que les opérations de **nœud** changent, que `W21.a` ne compte pas.
//! 4. Aucun solde n'est rendu, et aucun jugement.

use std::fmt::Write as _;

use locus_coordination::{
    CoordinationMode, Digest, EdgeChurn, Mutations, Operation, Relation, RelationKind, Version,
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

fn version(members: &[u8], relations: &[Relation]) -> Version {
    let members: Vec<Id<Agent>> = members.iter().map(|seed| agent(*seed)).collect();
    Version::root(
        &members,
        relations,
        CoordinationMode::Blackboard,
        None,
        &Fnv,
    )
    .expect("la fixture est licite")
}

// ---------------------------------------------------------------------------------------------
// 1 et 2. Le solde n'est pas le churn
// ---------------------------------------------------------------------------------------------

/// **Deux entrent, deux sortent : quatre, pas zéro.**
///
/// Le test qui porte l'item. Les deux versions ont **le même nombre d'arêtes** et **aucune arête en
/// commun** : le compte d'arêtes ne bouge pas, et le churn est maximal. Une lecture par le solde
/// conclurait « rien n'a changé » sur une organisation entièrement recomposée.
#[test]
fn deux_arretes_qui_entrent_et_deux_qui_sortent_rendent_quatre() {
    let avant = version(&[1, 2, 3, 4], &[reviews(1, 2), reviews(3, 4)]);
    let apres = version(&[1, 2, 3, 4], &[reviews(2, 1), reviews(4, 3)]);

    assert_eq!(
        avant.relations().len(),
        apres.relations().len(),
        "la fixture doit avoir la même cardinalité des deux côtés, sinon elle ne prouve rien"
    );

    let churn = EdgeChurn::between(&avant, &apres);

    assert_eq!(churn.total(), 4);
    assert_eq!(churn.entered().len(), 2);
    assert_eq!(churn.left().len(), 2);
    assert!(!churn.is_still());
}

/// **Le churn ne se déduit pas du compte d'arêtes.**
///
/// Deux paires de versions ont la même variation de cardinalité — nulle — et des churns de zéro et
/// de quatre. Aucune fonction du seul compte d'arêtes ne peut donc rendre le churn, et c'est ce qui
/// justifie que cette métrique existe séparément.
#[test]
fn le_churn_ne_se_deduit_pas_du_compte_d_aretes() {
    let stable = version(&[1, 2, 3, 4], &[reviews(1, 2), reviews(3, 4)]);
    let recompose = version(&[1, 2, 3, 4], &[reviews(2, 1), reviews(4, 3)]);

    let immobile = EdgeChurn::between(&stable, &stable);
    let agite = EdgeChurn::between(&stable, &recompose);

    assert_eq!(stable.relations().len(), recompose.relations().len());
    assert_eq!(immobile.total(), 0);
    assert_eq!(agite.total(), 4);
    assert!(immobile.is_still());
}

/// **Un churn nul est une identité d'ensembles, pas un solde nul.**
#[test]
fn un_churn_nul_veut_dire_les_memes_aretes() {
    let une = version(&[1, 2], &[reviews(1, 2)]);
    let autre = version(&[1, 2], &[reviews(1, 2)]);

    let churn = EdgeChurn::between(&une, &autre);

    assert!(churn.is_still());
    assert_eq!(churn.total(), 0);
    assert!(churn.entered().is_empty() && churn.left().is_empty());
}

/// **Une organisation qui ne fait que perdre des arêtes n'est pas immobile — ni l'inverse.**
///
/// Trouvé par un mutant survivant : `is_still` réduit à « aucune entrée » passait toute la suite,
/// parce qu'aucun test n'exerçait un churn **unilatéral**. Une organisation qui se dépeuple de ses
/// arêtes sans en gagner aurait été déclarée immobile, ce qui est le pire des deux sens : c'est
/// exactement le moment où on veut regarder.
///
/// Les deux directions sont tenues, parce qu'une seule laisserait passer l'autre moitié — la
/// leçon que ce dépôt réapprend à chaque passe de mutants.
#[test]
fn un_churn_unilateral_n_est_pas_une_immobilite() {
    let deux_aretes = version(&[1, 2, 3], &[reviews(1, 2), reviews(2, 3)]);
    let une_arete = version(&[1, 2, 3], &[reviews(1, 2)]);

    let perte = EdgeChurn::between(&deux_aretes, &une_arete);
    assert!(perte.entered().is_empty(), "rien n'entre");
    assert_eq!(perte.left().len(), 1);
    assert_eq!(perte.total(), 1);
    assert!(
        !perte.is_still(),
        "perdre une arête sans en gagner n'est pas rester immobile"
    );

    let gain = EdgeChurn::between(&une_arete, &deux_aretes);
    assert!(gain.left().is_empty(), "rien ne sort");
    assert_eq!(gain.entered().len(), 1);
    assert_eq!(gain.total(), 1);
    assert!(!gain.is_still());
}

/// **Inverser les versions échange entrées et sorties, sans changer le total.**
#[test]
fn inverser_l_ordre_echange_les_deux_ensembles() {
    let avant = version(&[1, 2, 3], &[reviews(1, 2)]);
    let apres = version(&[1, 2, 3], &[reviews(2, 3)]);

    let aller = EdgeChurn::between(&avant, &apres);
    let retour = EdgeChurn::between(&apres, &avant);

    assert_eq!(aller.entered(), retour.left());
    assert_eq!(aller.left(), retour.entered());
    assert_eq!(aller.total(), retour.total());
}

// ---------------------------------------------------------------------------------------------
// 3. Ce que les opérations d'arête ne disent pas
// ---------------------------------------------------------------------------------------------

/// **Un remplacement de nœud réécrit des arêtes sans qu'aucune opération d'arête n'ait lieu.**
///
/// C'est la raison pour laquelle le churn se lit entre deux versions et non sur une suite
/// d'opérations. `REPLACE_NODE` emporte les arêtes de l'identité remplacée : le churn vaut deux,
/// tandis que le compteur de `W21.a` rend **zéro** `ADD_EDGE` et **zéro** `REMOVE_EDGE`.
///
/// Un churn tiré des seules opérations d'arête aurait donc rendu zéro sur une version dont tout le
/// voisinage d'un nœud a changé.
#[test]
fn le_churn_voit_ce_que_les_operations_d_arete_ne_disent_pas() {
    let avant = version(&[1, 2, 3], &[reviews(1, 2)]);
    let remplacement = [Operation::ReplaceNode {
        from: agent(1),
        to: agent(5),
    }];

    let rejeu = Mutations::replay(&avant, &remplacement, &Fnv).expect("le remplacement s'applique");
    let churn = EdgeChurn::between(&avant, rejeu.landed());

    assert_eq!(
        rejeu.mutations().of_sort("ADD_EDGE"),
        Some(0),
        "aucune arête n'a été ajoutée par une opération d'arête"
    );
    assert_eq!(rejeu.mutations().of_sort("REMOVE_EDGE"), Some(0));
    assert_eq!(rejeu.mutations().of_sort("REPLACE_NODE"), Some(1));

    assert_eq!(
        churn.total(),
        2,
        "l'arête 1→2 est sortie et l'arête 5→2 est entrée : le churn les voit"
    );
    assert!(churn.left().contains(&reviews(1, 2)));
    assert!(churn.entered().contains(&reviews(5, 2)));
}

// ---------------------------------------------------------------------------------------------
// 4. Aucun solde, aucun jugement
// ---------------------------------------------------------------------------------------------

/// **Aucun accesseur ne rend de solde, et aucun ne juge.**
///
/// Un solde serait à un caractère de distance de ce qu'il faut lire, porterait un nom tout aussi
/// plausible, et rendrait un nombre plus petit — donc plus rassurant. Les motifs visent des
/// **signatures**, pas des mots : la source explique longuement pourquoi le solde n'est pas le
/// churn, et un test qui refuserait le mot mordrait sur son propre motif.
#[test]
fn la_source_ne_rend_ni_solde_ni_verdict() {
    let source = include_str!("../src/churn.rs");

    for interdit in [
        "fn net",
        "fn balance",
        "fn solde",
        "fn delta",
        "const MIN",
        "const MAX",
        "fn is_healthy",
        "fn score",
        "enum Verdict",
        "fn verdict",
    ] {
        assert!(
            !source.contains(interdit),
            "« {interdit} » : un solde ou un jugement rendus ici seraient lus à la place du churn"
        );
    }
}
