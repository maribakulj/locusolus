//! Test de sortie de W16.b — **les trois garanties de l'item.**
//!
//! 1. Une reconfiguration ne barre que les nœuds dont elle menace un invariant, et le refus nomme
//!    **l'invariant**, pas le lieu.
//! 2. Deux reconfigurations qui ne menacent pas le même invariant ne se bloquent pas l'une l'autre.
//! 3. Une barrière posée sans invariant menacé est refusée.

use std::fmt::Write as _;

use locus_coordination::{
    Barrier, BarrierError, Barriers, Diff, Digest, Invariant, Operation, Passage, Relation,
    RelationKind, Version, threatened_by,
};
use locus_domain::ContentHash;
use locus_protocol::{Id, IdKind, Timestamp, id::Agent};

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn agent(seed: u8) -> Id<Agent> {
    id::<Agent>(seed)
}

fn relation(from: Id<Agent>, to: Id<Agent>, kind: RelationKind) -> Relation {
    Relation { from, to, kind }
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

/// Six agents isolés : de quoi opérer au même endroit ou à deux endroits différents.
fn base() -> Version {
    Version::root(
        &[agent(1), agent(2), agent(3), agent(4), agent(5), agent(6)],
        &[],
        &Fnv,
    )
    .expect("organisation cohérente")
}

fn diff_of(base: &Version, operations: Vec<Operation>) -> Diff {
    Diff::declaring(base, operations, &Fnv).expect("les opérations s'appliquent")
}

/// Une reconfiguration qui menace l'acyclicité de revue.
fn threatening(from: Id<Agent>, to: Id<Agent>) -> Diff {
    diff_of(
        &base(),
        vec![Operation::AddEdge(relation(from, to, RelationKind::Review))],
    )
}

/// Une reconfiguration qui n'en menace aucun — depuis W15.e, la visibilité ne ferme aucun cycle.
fn harmless(from: Id<Agent>, to: Id<Agent>) -> Diff {
    diff_of(
        &base(),
        vec![Operation::AddEdge(relation(
            from,
            to,
            RelationKind::Visibility,
        ))],
    )
}

// ---------------------------------------------------------------------------------------------
// 1. Le refus nomme l'invariant, jamais le lieu
// ---------------------------------------------------------------------------------------------

#[test]
fn a_held_barrier_names_the_invariant_and_who_holds_it() {
    let mut barriers = Barriers::new();
    barriers
        .raise(&threatening(agent(1), agent(2)), "alice")
        .expect("le lot menace l'acyclicité");

    let passage = barriers.admits(&threatening(agent(3), agent(4)));
    assert_eq!(
        passage,
        Passage::Held {
            invariant: Invariant::ReviewAcyclicity,
            by: "alice".to_owned(),
        },
        "« cette équipe est gelée » n'apprend rien ; l'invariant tenu dit quoi attendre"
    );
}

/// **Là où une barrière par lieu bloque trop peu.**
///
/// Les deux reconfigurations opèrent sur des nœuds entièrement disjoints — `1 → 2` et `5 → 6` — et
/// menacent pourtant le même invariant. Une barrière par lieu les aurait laissées passer toutes les
/// deux, et l'acyclicité serait tombée entre elles.
#[test]
fn two_reconfigurations_in_different_places_still_serialise() {
    let mut barriers = Barriers::new();
    barriers
        .raise(&threatening(agent(1), agent(2)), "alice")
        .expect("le lot menace l'acyclicité");

    let elsewhere = threatening(agent(5), agent(6));
    assert!(
        !barriers.admits(&elsewhere).is_clear(),
        "des lieux disjoints ne mettent pas l'invariant à l'abri"
    );
    assert!(matches!(
        barriers.raise(&elsewhere, "bob").expect_err("déjà tenu"),
        BarrierError::AlreadyHeld { .. }
    ));
}

// ---------------------------------------------------------------------------------------------
// 2. Ce qui ne menace pas le même invariant ne se bloque pas
// ---------------------------------------------------------------------------------------------

/// **Là où une barrière par lieu bloque trop.**
///
/// Les deux reconfigurations touchent **les mêmes nœuds** — `1 → 2` dans les deux cas — mais l'une
/// ajoute une revue et l'autre une visibilité. Depuis W15.e, la seconde ne peut fermer aucun cycle
/// de revue : elle ne menace rien, donc rien ne la retient. Une barrière par lieu les aurait
/// sérialisées sans qu'elles puissent se casser quoi que ce soit.
#[test]
fn two_reconfigurations_in_the_same_place_do_not_block_each_other() {
    let mut barriers = Barriers::new();
    barriers
        .raise(&threatening(agent(1), agent(2)), "alice")
        .expect("le lot menace l'acyclicité");

    assert!(
        barriers.admits(&harmless(agent(1), agent(2))).is_clear(),
        "au même endroit, et pourtant rien à protéger l'un de l'autre"
    );
}

#[test]
fn releasing_lets_the_next_one_through() {
    let mut barriers = Barriers::new();
    let raised: Vec<Barrier> = barriers
        .raise(&threatening(agent(1), agent(2)), "alice")
        .expect("menace l'acyclicité");
    assert_eq!(raised.len(), 1);

    let waiting = threatening(agent(3), agent(4));
    assert!(!barriers.admits(&waiting).is_clear());

    barriers.release(&raised[0]).expect("alice la tenait");
    assert!(barriers.held().is_empty());
    assert!(barriers.admits(&waiting).is_clear());
}

#[test]
fn releasing_what_is_not_held_is_refused() {
    let mut barriers = Barriers::new();
    let raised = barriers
        .raise(&threatening(agent(1), agent(2)), "alice")
        .expect("menace l'acyclicité");
    barriers.release(&raised[0]).expect("tenue");
    assert!(matches!(
        barriers.release(&raised[0]).expect_err("plus tenue"),
        BarrierError::NotHeld { .. }
    ));
}

// ---------------------------------------------------------------------------------------------
// 3. Une barrière sans invariant menacé est refusée
// ---------------------------------------------------------------------------------------------

/// Sans invariant à nommer, elle ne pourrait barrer que par lieu — donc être exactement ce que
/// `docs/13` écarte. Et le lot n'en a de toute façon pas besoin.
#[test]
fn a_barrier_with_nothing_to_protect_is_refused() {
    let mut barriers = Barriers::new();
    assert_eq!(
        barriers
            .raise(&harmless(agent(1), agent(2)), "alice")
            .expect_err("rien à protéger"),
        BarrierError::NothingThreatened
    );
    assert!(barriers.held().is_empty());
    assert!(
        barriers.admits(&harmless(agent(1), agent(2))).is_clear(),
        "et il passe sans barrière"
    );
}

#[test]
fn a_barrier_nobody_holds_is_refused() {
    let mut barriers = Barriers::new();
    assert_eq!(
        barriers
            .raise(&threatening(agent(1), agent(2)), "  ")
            .expect_err("anonyme"),
        BarrierError::EmptyHolder
    );
}

// ---------------------------------------------------------------------------------------------
// La portée est dérivée, jamais déclarée
// ---------------------------------------------------------------------------------------------

/// Ce qu'un lot met en jeu est **calculé**, par le même `threatens` que le plafond de risque de
/// W15.c. Deux calculs auraient produit deux vérités sur la même question.
#[test]
fn what_a_batch_threatens_is_computed_not_declared() {
    assert_eq!(
        threatened_by(&threatening(agent(1), agent(2)))
            .into_iter()
            .collect::<Vec<_>>(),
        vec![Invariant::ReviewAcyclicity]
    );
    assert!(threatened_by(&harmless(agent(1), agent(2))).is_empty());

    // Un lot mixte met en jeu ce que sa partie la plus menaçante met en jeu.
    let mixed = diff_of(
        &base(),
        vec![
            Operation::AddEdge(relation(agent(1), agent(2), RelationKind::Visibility)),
            Operation::AddEdge(relation(agent(3), agent(4), RelationKind::Review)),
        ],
    );
    assert_eq!(
        threatened_by(&mixed).into_iter().collect::<Vec<_>>(),
        vec![Invariant::ReviewAcyclicity]
    );
}

/// Une barrière ne nomme **aucun lieu**.
///
/// Vérification par l'absence : elle porte un invariant et celui qui la tient, rien d'autre. Un
/// accesseur qui rendrait des identités ferait écrire, un jour, « barrer aussi ceux-là » — et la
/// barrière par lieu serait revenue sous un autre nom.
#[test]
fn a_barrier_names_no_place() {
    let mut barriers = Barriers::new();
    let raised = barriers
        .raise(&threatening(agent(1), agent(2)), "alice")
        .expect("menace l'acyclicité");
    let barrier = &raised[0];

    assert_eq!(barrier.invariant(), Invariant::ReviewAcyclicity);
    assert_eq!(barrier.held_by(), "alice");
    assert!(
        !format!("{barrier:?}").contains(&agent(1).to_string()),
        "une barrière qui saurait nommer un lieu finirait par en barrer un"
    );
}
