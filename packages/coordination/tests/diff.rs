//! Test de sortie de W15.b — **les trois garanties de l'item.**
//!
//! 1. Un diff se rejoue sur sa base et rend exactement la cible ; deux rejeux du même diff sur la
//!    même base rendent la **même version**, identité comprise.
//! 2. Rejoué sur une autre base il est refusé, et le refus dit s'il faut rebaser.
//! 3. Le diff d'une version vers elle-même est **vide**, jamais absent.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use locus_coordination::{
    CoordinationMode, Diff, DiffError, Digest, Operation, Relation, RelationKind, Version,
};
use locus_domain::ContentHash;
use locus_protocol::{Id, IdKind, Timestamp, id::Agent};

// ---------------------------------------------------------------------------------------------
// Fixtures — les mêmes que celles de `version.rs`, pour que les deux tests parlent du même monde.
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

fn reviews(from: Id<Agent>, to: Id<Agent>) -> Relation {
    Relation {
        from,
        to,
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

/// Trois agents, deux relations de revue : 1 relit 2, 3 relit 2.
fn three() -> Version {
    Version::root(
        &[agent(1), agent(2), agent(3)],
        &[reviews(agent(1), agent(2)), reviews(agent(3), agent(2))],
        CoordinationMode::Blackboard,
        None,
        &Fnv,
    )
    .expect("la fixture est cohérente")
}

// ---------------------------------------------------------------------------------------------
// 1. Un diff rend exactement sa cible, et deux rejeux s'accordent
// ---------------------------------------------------------------------------------------------

#[test]
fn a_diff_replays_to_exactly_its_target() {
    let from = three();
    let to = from
        .apply(&Operation::AddNode(agent(4)), &Fnv)
        .expect("un nœud neuf")
        .apply(&Operation::AddEdge(reviews(agent(4), agent(2))), &Fnv)
        .expect("une arête neuve")
        .apply(&Operation::RemoveEdge(reviews(agent(3), agent(2))), &Fnv)
        .expect("une arête retirée");

    let diff = Diff::between(&from, &to);
    let replayed = diff
        .replay(&from, &Fnv)
        .expect("le diff s'applique sur sa base");

    assert_eq!(
        replayed.content_hash(),
        to.content_hash(),
        "le rejeu produit exactement le contenu visé"
    );
    assert_eq!(replayed.members(), to.members());
    assert_eq!(replayed.relations(), to.relations());
}

/// La garantie de `docs/10` W17 : « diff calculé une fois côté serveur, donc identique dans Emacs
/// et dans le web ». Deux clients qui rejouent le même diff sur la même base obtiennent la **même
/// version**, identité comprise — sinon l'approbation porterait sur ce que chacun a cru voir.
#[test]
fn two_replays_of_one_diff_agree_down_to_the_version_id() {
    let from = three();
    let to = from
        .apply(&Operation::AddNode(agent(4)), &Fnv)
        .expect("un nœud neuf");
    let diff = Diff::between(&from, &to);

    let emacs = diff.replay(&from, &Fnv).expect("rejeu");
    let web = diff.replay(&from, &Fnv).expect("rejeu");
    assert_eq!(emacs.id(), web.id());
    assert_eq!(emacs, web);
}

/// L'ordre du diff n'est pas décoratif : le refus de la cascade impose de retirer les arêtes avant
/// les nœuds, et d'ajouter les nœuds avant les arêtes. Un diff qui retire un agent et ses deux
/// relations ne s'applique que dans cet ordre.
#[test]
fn a_diff_removes_before_it_adds() {
    let from = three();
    let to = Version::root(
        &[agent(1), agent(3), agent(4)],
        &[],
        CoordinationMode::Blackboard,
        None,
        &Fnv,
    )
    .expect("cible cohérente");

    let diff = Diff::between(&from, &to);
    let replayed = diff
        .replay(&from, &Fnv)
        .expect("l'ordre du diff s'applique");
    assert_eq!(replayed.content_hash(), to.content_hash());

    let names: Vec<&str> = diff
        .operations()
        .iter()
        .map(locus_coordination::Operation::name)
        .collect();
    let last_removal = names
        .iter()
        .rposition(|name| name.starts_with("REMOVE"))
        .expect("le diff retire quelque chose");
    let first_addition = names
        .iter()
        .position(|name| name.starts_with("ADD"))
        .expect("le diff ajoute quelque chose");
    assert!(
        last_removal < first_addition,
        "tout ce qui se retire précède tout ce qui s'ajoute : {names:?}"
    );
}

/// Un diff ne devine pas d'intention.
///
/// `REPLACE_NODE` et « retirer puis ajouter » mènent au même état ; les distinguer demanderait de
/// lire dans les pensées du proposeur. Le diff décrit l'écart, la proposition porte l'intention —
/// et c'est `declaring` qui la transporte.
#[test]
fn a_diff_between_two_states_never_infers_a_replacement() {
    let from = three();
    let to = from
        .apply(
            &Operation::ReplaceNode {
                from: agent(3),
                to: agent(9),
            },
            &Fnv,
        )
        .expect("le remplaçant est neuf");

    let diff = Diff::between(&from, &to);
    assert!(
        !diff
            .operations()
            .iter()
            .any(|operation| operation.name() == "REPLACE_NODE"),
        "deviner un remplacement ferait lire une intention que personne n'a écrite"
    );
    assert_eq!(
        diff.replay(&from, &Fnv).expect("rejeu").content_hash(),
        to.content_hash(),
        "l'écart reste exactement décrit"
    );
}

/// L'autre chemin : ce que le proposeur a écrit lui survit jusqu'à l'approbateur.
#[test]
fn declaring_keeps_the_operations_the_proposer_wrote() {
    let base = three();
    let split = Operation::SplitNode {
        node: agent(2),
        into: (agent(8), agent(9)),
        follows_first: [reviews(agent(1), agent(2))].into_iter().collect(),
    };
    let diff = Diff::declaring(&base, vec![split.clone()], &Fnv).expect("scission valide");

    assert_eq!(diff.operations(), [split]);
    let replayed = diff.replay(&base, &Fnv).expect("rejeu");
    assert!(replayed.relations().contains(&reviews(agent(1), agent(8))));
    assert!(replayed.relations().contains(&reviews(agent(3), agent(9))));
}

// ---------------------------------------------------------------------------------------------
// 2. Rejoué ailleurs, refusé — et le refus dit quoi faire
// ---------------------------------------------------------------------------------------------

#[test]
fn a_diff_replayed_on_another_base_is_refused_and_says_to_rebase() {
    let from = three();
    let to = from
        .apply(&Operation::AddNode(agent(4)), &Fnv)
        .expect("un nœud neuf");
    let diff = Diff::between(&from, &to);

    let moved_on = from
        .apply(&Operation::AddNode(agent(7)), &Fnv)
        .expect("quelqu'un d'autre a commité");

    let error = diff
        .replay(&moved_on, &Fnv)
        .expect_err("la base a bougé sous le diff");
    assert_eq!(
        error,
        DiffError::Stale {
            expected: from.id().clone(),
            actual: moved_on.id().clone(),
        }
    );
    assert!(
        error.needs_rebase(),
        "un « conflit » sans consigne fait retenter à l'identique"
    );
}

/// Deux versions de contenu identique et d'histoires différentes ne sont pas la même base.
///
/// C'est la conséquence directe des deux hashes de W15.a : si le diff se contentait de comparer les
/// contenus, il s'appliquerait sur une organisation qui a entre-temps été défaite et refaite, et
/// l'approbation aurait porté sur une autre histoire.
#[test]
fn a_base_is_a_version_not_a_content() {
    let from = three();
    let diff = Diff::between(
        &from,
        &from
            .apply(&Operation::AddNode(agent(4)), &Fnv)
            .expect("un nœud neuf"),
    );

    let same_content_other_history = from
        .apply(&Operation::AddNode(agent(7)), &Fnv)
        .expect("ajouté")
        .apply(&Operation::RemoveNode(agent(7)), &Fnv)
        .expect("puis retiré");
    assert_eq!(
        same_content_other_history.content_hash(),
        from.content_hash(),
        "le contenu est revenu"
    );

    let error = diff
        .replay(&same_content_other_history, &Fnv)
        .expect_err("l'histoire, elle, n'est pas revenue");
    assert!(error.needs_rebase());
}

/// Un refus nomme **laquelle** des opérations a échoué, et sous sa forme complète.
///
/// La position seule fait recompter ; le nom seul — `ADD_NODE` — fait deviner de quel nœud il
/// s'agit dans une suite qui en ajoute plusieurs. Les deux ensemble se corrigent sans relire.
#[test]
fn an_inapplicable_operation_names_its_position_and_itself() {
    let base = three();
    let culprit = Operation::AddNode(agent(4));
    let error = Diff::declaring(
        &base,
        vec![
            culprit.clone(),
            Operation::AddNode(agent(5)),
            // `agent(4)` vient d'entrer : le refaire entrer ne s'applique pas.
            culprit.clone(),
        ],
        &Fnv,
    )
    .expect_err("la troisième opération répète la première");
    let DiffError::Inapplicable {
        position,
        operation,
        ..
    } = error
    else {
        panic!("le refus doit nommer l'opération fautive, pas la suite");
    };
    assert_eq!(
        position, 2,
        "une suite qui échoue sans dire où oblige à tout relire"
    );
    assert_eq!(
        operation,
        culprit.canonical(),
        "« ADD_NODE » seul fait deviner lequel des deux nœuds ajoutés a échoué"
    );
}

/// Ce qui prouve n'est pas ce qui est annoncé.
///
/// Un diff venu du fil peut déclarer n'importe quelle cible. Le rejeu confronte ce qu'il **produit**
/// à ce que le document promet, et refuse plutôt que de croire sur parole.
#[test]
fn a_wire_diff_that_announces_what_it_does_not_produce_is_refused() {
    let base = three();
    let honest = Diff::between(
        &base,
        &base
            .apply(&Operation::AddNode(agent(4)), &Fnv)
            .expect("un nœud neuf"),
    );
    let flattering = Diff::from_wire(
        honest.base().clone(),
        base.content_hash().clone(),
        honest.operations().to_vec(),
    );

    let error = flattering
        .replay(&base, &Fnv)
        .expect_err("la cible annoncée n'est pas celle produite");
    assert!(matches!(error, DiffError::ContentMismatch { .. }));
}

// ---------------------------------------------------------------------------------------------
// 3. Vide, jamais absent
// ---------------------------------------------------------------------------------------------

/// Un diff d'une version vers elle-même **existe** et ne change rien.
///
/// Rendre `None` obligerait chaque appelant à un cas particulier, et surtout un approbateur ne
/// verrait rien du tout au lieu de lire que la proposition ne change rien — ce qui est une
/// information, et souvent une surprise.
#[test]
fn a_diff_from_a_version_to_itself_is_empty_never_absent() {
    let version = three();
    let diff = Diff::between(&version, &version);

    assert!(diff.is_empty());
    assert!(diff.operations().is_empty());
    assert_eq!(diff.base(), version.id());
    assert_eq!(diff.target_content(), version.content_hash());

    let replayed = diff.replay(&version, &Fnv).expect("rejeu du vide");
    assert_eq!(
        replayed.id(),
        version.id(),
        "rejouer rien ne doit pas inscrire qu'il s'est passé quelque chose"
    );
}

// ---------------------------------------------------------------------------------------------
// La forme canonique : ce que deux clients comparent
// ---------------------------------------------------------------------------------------------

#[test]
fn canonical_form_is_frozen() {
    let base = three();
    let diff = Diff::declaring(
        &base,
        vec![
            Operation::RemoveEdge(reviews(agent(3), agent(2))),
            Operation::AddNode(agent(4)),
        ],
        &Fnv,
    )
    .expect("suite valide");

    let expected = format!(
        "coordination-diff/1\nbase\t{}\ntarget\t{}\nop\t0\tREMOVE_EDGE\t{}>review>{}\nop\t1\tADD_NODE\t{}\n",
        base.id(),
        diff.target_content(),
        agent(3),
        agent(2),
        agent(4),
    );
    assert_eq!(diff.canonical(), expected);
}

/// Un diff est une **suite**, pas un ensemble.
///
/// Deux diffs portant les mêmes opérations dans deux ordres différents ne décrivent pas la même
/// chose — l'un s'applique, l'autre non. Une forme canonique triée les confondrait, et deux clients
/// signeraient un document qui ne décrit pas ce qui sera commité.
#[test]
fn the_canonical_form_of_a_diff_is_not_sorted() {
    let base = three();
    let removal = Operation::RemoveEdge(reviews(agent(1), agent(2)));
    let addition = Operation::AddNode(agent(4));

    let applicable = Diff::declaring(&base, vec![addition.clone(), removal.clone()], &Fnv)
        .expect("les deux ordres s'appliquent ici");
    let other_order =
        Diff::declaring(&base, vec![removal, addition], &Fnv).expect("l'autre ordre aussi");

    assert_ne!(
        applicable.canonical(),
        other_order.canonical(),
        "l'ordre fait partie de ce que le diff dit"
    );
    assert_eq!(
        applicable.target_content(),
        other_order.target_content(),
        "ici les deux mènent au même état — ce qui est justement pourquoi trier serait tentant"
    );
}

/// La partition d'une scission entre dans la forme canonique.
///
/// Deux scissions du même nœud vers les deux mêmes identités, de partitions opposées, produisent des
/// organisations différentes. Une forme qui n'écrirait que les identités les rendrait
/// indistinguables, et l'approbation aurait porté sur celle qu'on n'applique pas.
#[test]
fn two_splits_that_differ_only_by_their_partition_are_two_diffs() {
    let base = three();
    let split = |follows_first: BTreeSet<Relation>| {
        Diff::declaring(
            &base,
            vec![Operation::SplitNode {
                node: agent(2),
                into: (agent(8), agent(9)),
                follows_first,
            }],
            &Fnv,
        )
        .expect("scission valide")
    };

    let one = split([reviews(agent(1), agent(2))].into_iter().collect());
    let other = split([reviews(agent(3), agent(2))].into_iter().collect());

    assert_ne!(one.canonical(), other.canonical());
    assert_ne!(one.target_content(), other.target_content());
}
