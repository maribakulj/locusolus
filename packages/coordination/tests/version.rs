//! Test de sortie de W15.a — **les quatre garanties de l'item.**
//!
//! 1. Le hash de contenu ne dépend que du contenu, et la forme canonique se reproduit octet pour
//!    octet sur une fixture figée.
//! 2. Appliquer une opération puis son défaire rend le **même contenu** et une **autre version** :
//!    l'état revient, l'histoire non.
//! 3. Une opération dont le défaire perdrait de l'information se déclare compensable, et aucune
//!    fonction ne rend d'opération qui prétende la défaire.
//! 4. Les quatre opérations attributaires de `docs/13` sont absentes, et le test le tient par
//!    l'absence.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use locus_coordination::{Digest, Operation, Relation, RelationKind, Undo, Version, VersionError};
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

fn reviews(from: Id<Agent>, to: Id<Agent>) -> Relation {
    Relation {
        from,
        to,
        kind: RelationKind::Review,
    }
}

/// Un condensat de test : déterministe, et différent pour deux entrées différentes.
///
/// Le crate ne choisit pas d'algorithme, donc le test en fournit un. Ce qui est vérifié ici n'est
/// pas la qualité du hachage — c'est que la **forme canonique** est stable, et un condensat jouet
/// suffit à le montrer tant qu'il est une fonction de ses octets et de rien d'autre.
struct Fnv;

/// Le nombre premier de FNV-1a sur 64 bits.
const PRIME: u64 = 0x0000_0100_0000_01b3;

/// Son décalage initial.
const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

impl Digest for Fnv {
    fn digest(&self, canonical: &str) -> ContentHash {
        let mut digest = String::with_capacity(64);
        for salt in 0_u64..4 {
            let mut hash = OFFSET ^ salt.wrapping_mul(PRIME);
            for byte in canonical.bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(PRIME);
            }
            write!(digest, "{hash:016x}").expect("écrire dans une String ne échoue pas");
        }
        ContentHash::parse(&format!("sha256:{digest}")).expect("64 hexadécimaux minuscules")
    }
}

/// Trois agents, deux relations de revue : a relit b, c relit b.
fn three() -> Version {
    Version::root(
        &[agent(1), agent(2), agent(3)],
        &[reviews(agent(1), agent(2)), reviews(agent(3), agent(2))],
        &Fnv,
    )
    .expect("la fixture est cohérente")
}

// ---------------------------------------------------------------------------------------------
// 1. Le hash de contenu ne dépend que du contenu
// ---------------------------------------------------------------------------------------------

#[test]
fn canonical_form_is_frozen() {
    let version = Version::root(&[agent(1), agent(2)], &[reviews(agent(1), agent(2))], &Fnv)
        .expect("fixture cohérente");
    let first = agent(1);
    let second = agent(2);
    let expected =
        format!("coordination-content/1\ne\t{first}\treview\t{second}\nn\t{first}\nn\t{second}\n");
    assert_eq!(
        version.canonical(),
        expected,
        "la forme canonique est le contrat : elle se lit, elle ne se devine pas"
    );
}

#[test]
fn content_hash_ignores_the_order_a_producer_used() {
    let ordered = Version::root(
        &[agent(1), agent(2), agent(3)],
        &[reviews(agent(1), agent(2)), reviews(agent(3), agent(2))],
        &Fnv,
    )
    .expect("fixture cohérente");
    let shuffled = Version::root(
        &[agent(3), agent(1), agent(2)],
        &[reviews(agent(3), agent(2)), reviews(agent(1), agent(2))],
        &Fnv,
    )
    .expect("fixture cohérente");
    assert_eq!(
        ordered.content_hash(),
        shuffled.content_hash(),
        "deux producteurs qui remplissent leurs collections différemment décrivent la même chose"
    );
    assert_eq!(
        ordered.id(),
        shuffled.id(),
        "même contenu, même absence de parent"
    );
}

#[test]
fn content_hash_moves_with_the_content() {
    let before = three();
    let after = before
        .apply(&Operation::AddNode(agent(4)), &Fnv)
        .expect("un nœud neuf entre");
    assert_ne!(before.content_hash(), after.content_hash());
    assert_eq!(after.parent(), Some(before.id()));
}

/// L'identité tient aux **deux** bouts : le parent et le contenu.
///
/// Deux opérations différentes menées depuis la même base ont le même parent. Si l'identité ne
/// retenait que le parent, elles porteraient le même identifiant et une histoire se réécrirait en
/// silence — deux organisations distinctes se citant sous le même nom.
#[test]
fn two_operations_from_one_base_are_two_versions() {
    let base = three();
    let one = base
        .apply(&Operation::AddNode(agent(8)), &Fnv)
        .expect("un nœud neuf entre");
    let other = base
        .apply(&Operation::AddNode(agent(9)), &Fnv)
        .expect("un autre nœud neuf entre");
    assert_eq!(one.parent(), other.parent(), "même base");
    assert_ne!(one.content_hash(), other.content_hash());
    assert_ne!(
        one.id(),
        other.id(),
        "même parent et contenus différents font deux versions, pas une"
    );
}

// ---------------------------------------------------------------------------------------------
// 2. Défaire rend le même contenu et une autre version
// ---------------------------------------------------------------------------------------------

/// Le cœur de l'item, sur les six opérations qui ont un inverse exact.
///
/// Deux affirmations à chaque fois, et l'une sans l'autre serait fausse : le **contenu** revient,
/// la **version** non. Si les deux revenaient, l'histoire dirait qu'une organisation n'a jamais été
/// modifiée ; si aucune ne revenait, personne ne pourrait vérifier qu'une annulation a annulé.
#[test]
fn undoing_restores_the_content_and_never_the_history() {
    let split_edges: BTreeSet<Relation> = [reviews(agent(1), agent(2))].into_iter().collect();
    let cases = [
        Operation::AddNode(agent(9)),
        Operation::RemoveNode(agent(4)),
        Operation::ReplaceNode {
            from: agent(3),
            to: agent(9),
        },
        Operation::AddEdge(reviews(agent(4), agent(2))),
        Operation::RemoveEdge(reviews(agent(3), agent(2))),
        Operation::SplitNode {
            node: agent(2),
            into: (agent(8), agent(9)),
            follows_first: split_edges,
        },
    ];

    // `agent(4)` est isolé exprès : c'est ce qui rend `REMOVE_NODE` applicable sans cascade.
    let base = three()
        .apply(&Operation::AddNode(agent(4)), &Fnv)
        .expect("un nœud isolé entre");

    for operation in cases {
        let applied = base
            .apply(&operation, &Fnv)
            .unwrap_or_else(|error| panic!("{operation} devait s'appliquer : {error}"));
        let Undo::Exact(inverse) = operation.undo() else {
            panic!("{operation} devait avoir un inverse exact");
        };
        let undone = applied
            .apply(&inverse, &Fnv)
            .unwrap_or_else(|error| panic!("{inverse} devait s'appliquer : {error}"));

        assert_eq!(
            undone.content_hash(),
            base.content_hash(),
            "{operation} défaite doit rendre le contenu d'avant"
        );
        assert_ne!(
            undone.id(),
            base.id(),
            "{operation} défaite ne doit pas rendre la version d'avant : l'histoire ne se défait pas"
        );
        assert_eq!(
            undone.parent(),
            Some(applied.id()),
            "la version qui défait descend de celle qui avait fait"
        );
    }
}

#[test]
fn a_replaced_node_keeps_its_edges() {
    let replaced = three()
        .apply(
            &Operation::ReplaceNode {
                from: agent(2),
                to: agent(9),
            },
            &Fnv,
        )
        .expect("le remplaçant est neuf");
    assert!(replaced.relations().contains(&reviews(agent(1), agent(9))));
    assert!(replaced.relations().contains(&reviews(agent(3), agent(9))));
    assert!(!replaced.members().contains(&agent(2)));
}

#[test]
fn a_split_shares_the_edges_it_declared() {
    let split = three()
        .apply(
            &Operation::SplitNode {
                node: agent(2),
                into: (agent(8), agent(9)),
                follows_first: [reviews(agent(1), agent(2))].into_iter().collect(),
            },
            &Fnv,
        )
        .expect("les deux identités sont neuves");
    assert!(split.relations().contains(&reviews(agent(1), agent(8))));
    assert!(split.relations().contains(&reviews(agent(3), agent(9))));
    assert_eq!(
        split.relations().len(),
        2,
        "aucune arête n'est perdue ni dupliquée"
    );
}

// ---------------------------------------------------------------------------------------------
// 3. Fusionner se compense
// ---------------------------------------------------------------------------------------------

#[test]
fn merging_is_compensating_and_nothing_pretends_otherwise() {
    let merge = Operation::MergeNodes {
        first: agent(1),
        second: agent(3),
        into: agent(9),
    };
    assert_eq!(merge.undo(), Undo::Compensating);
    assert!(
        merge.undo().exact().is_none(),
        "aucune fonction ne doit rendre une scission plausible : l'information n'existe plus"
    );
}

/// Ce que la fusion perd, montré plutôt qu'affirmé.
///
/// Deux relations distinctes entrent, une seule sort. C'est la raison exacte pour laquelle
/// `MERGE_NODES` n'a pas d'inverse : une scission devrait dire laquelle des deux était laquelle.
#[test]
fn merging_collapses_two_edges_into_one() {
    let merged = three()
        .apply(
            &Operation::MergeNodes {
                first: agent(1),
                second: agent(3),
                into: agent(9),
            },
            &Fnv,
        )
        .expect("l'identité produite est neuve");
    assert_eq!(merged.relations().len(), 1);
    assert!(merged.relations().contains(&reviews(agent(9), agent(2))));
}

/// La fusion ne distingue pas ses deux absorbés, et c'est ce qui rend l'inverse d'une scission
/// insensible à l'ordre de ses moitiés.
///
/// Le test est écrit parce que la propriété n'est pas gratuite : elle tient tant que la
/// substitution envoie les deux vers `into` et que le refus de l'auto-relation examine les deux
/// sens. Quelque chose qui privilégierait `first` la casserait sans casser rien d'autre.
#[test]
fn merging_does_not_distinguish_its_two_nodes() {
    let merge = |first, second| {
        three()
            .apply(
                &Operation::MergeNodes {
                    first,
                    second,
                    into: agent(9),
                },
                &Fnv,
            )
            .expect("l'identité produite est neuve")
    };
    assert_eq!(
        merge(agent(1), agent(3)).content_hash(),
        merge(agent(3), agent(1)).content_hash()
    );
}

/// L'inverse d'une scission est une fusion, et elle est exacte **parce que** la scission a énoncé
/// sa partition. La réciproque ne tient pas, et les deux tests ensemble disent pourquoi.
#[test]
fn splitting_then_merging_recovers_every_edge() {
    let base = three();
    let split = Operation::SplitNode {
        node: agent(2),
        into: (agent(8), agent(9)),
        follows_first: [reviews(agent(1), agent(2))].into_iter().collect(),
    };
    let applied = base.apply(&split, &Fnv).expect("scission valide");
    let Undo::Exact(merge) = split.undo() else {
        panic!("une scission se défait");
    };
    let back = applied.apply(&merge, &Fnv).expect("fusion valide");
    assert_eq!(back.relations(), base.relations());
    assert_eq!(back.content_hash(), base.content_hash());
}

/// Une scission ne touche que ses propres arêtes.
///
/// L'arête `4 → 1` ne concerne pas `agent(2)` et doit traverser la scission intacte. Une
/// implémentation qui ne garderait que les arêtes incidentes déplacerait silencieusement une partie
/// de l'organisation, et le diff aurait montré une scission.
#[test]
fn a_split_leaves_the_rest_of_the_organisation_alone() {
    let base = three()
        .apply(&Operation::AddNode(agent(4)), &Fnv)
        .expect("un nœud neuf entre")
        .apply(&Operation::AddEdge(reviews(agent(4), agent(1))), &Fnv)
        .expect("une arête étrangère à la scission");
    let split = base
        .apply(
            &Operation::SplitNode {
                node: agent(2),
                into: (agent(8), agent(9)),
                follows_first: [reviews(agent(1), agent(2))].into_iter().collect(),
            },
            &Fnv,
        )
        .expect("scission valide");
    assert!(
        split.relations().contains(&reviews(agent(4), agent(1))),
        "une arête qui ne touche pas le nœud scindé traverse la scission inchangée"
    );
    assert_eq!(split.relations().len(), 3);
}

// ---------------------------------------------------------------------------------------------
// 4. Les quatre opérations attributaires sont absentes
// ---------------------------------------------------------------------------------------------

/// Vérification par l'absence — ADR 0016, décision 4.
///
/// Le test échouera le jour où quelqu'un ajoutera une variante attributaire sans lui donner de
/// lecteur, et c'est ce qu'on lui demande. Les quatre noms sont écrits en toutes lettres pour que
/// l'échec dise **laquelle** est entrée sans son consommateur.
#[test]
fn the_four_attribute_operations_await_their_reader() {
    for absent in [
        "SET_ROLE",
        "SET_VISIBILITY",
        "SET_VALIDATOR",
        "SET_EXECUTION_ORDER",
    ] {
        assert!(
            !Operation::NAMES.contains(&absent),
            "{absent} écrirait un attribut qu'aucun lecteur ne lit"
        );
    }
    assert_eq!(
        Operation::NAMES.len(),
        7,
        "sept structurelles, et pas une de plus sans consommateur exécutable"
    );
}

// ---------------------------------------------------------------------------------------------
// Les refus : chacun nomme une chose à écrire dans le diff
// ---------------------------------------------------------------------------------------------

#[test]
fn removing_a_connected_node_is_refused_rather_than_cascaded() {
    let error = three()
        .apply(&Operation::RemoveNode(agent(2)), &Fnv)
        .expect_err("agent(2) porte deux arêtes");
    assert_eq!(
        error,
        VersionError::NodeStillConnected {
            node: agent(2).to_string(),
            edges: 2,
        },
        "une cascade ferait au commit ce que le diff ne montrait pas"
    );
}

#[test]
fn an_edge_needs_both_of_its_endpoints() {
    let error = three()
        .apply(&Operation::AddEdge(reviews(agent(1), agent(7))), &Fnv)
        .expect_err("agent(7) n'est pas membre");
    assert!(matches!(error, VersionError::DanglingEdge { .. }));
}

#[test]
fn no_agent_reviews_itself() {
    let error = three()
        .apply(&Operation::AddEdge(reviews(agent(1), agent(1))), &Fnv)
        .expect_err("§14.4 et l'invariant 11");
    assert_eq!(
        error,
        VersionError::SelfRelation {
            node: agent(1).to_string()
        }
    );
}

/// Le refus qui n'était pas évident : fusionner un relecteur avec son relu.
///
/// La substitution en ferait une relation d'un agent vers lui-même — un agent qui se relit, obtenu
/// sans qu'aucune opération ne l'ait demandé. Refuser oblige à retirer l'arête d'abord, dans le
/// diff, où l'approbateur la voit.
/// Les **deux** ordres sont examinés : l'arête va de `agent(1)` vers `agent(2)`, et la fusion
/// refuse qu'on la nomme dans un sens comme dans l'autre. Ne regarder que `premier → second`
/// laisserait passer exactement la même faute écrite à l'envers.
#[test]
fn merging_a_reviewer_with_its_reviewee_is_refused() {
    for (first, second) in [(agent(1), agent(2)), (agent(2), agent(1))] {
        let error = three()
            .apply(
                &Operation::MergeNodes {
                    first,
                    second,
                    into: agent(9),
                },
                &Fnv,
            )
            .expect_err("agent(1) relit agent(2)");
        assert_eq!(
            error,
            VersionError::SelfRelation {
                node: agent(9).to_string()
            },
            "fusionner {first} et {second}"
        );
    }
}

/// Une identité produite doit être neuve, des deux côtés.
///
/// Réutiliser une identité déjà membre ferait absorber un tiers par la scission ou par la fusion,
/// sans qu'aucune opération ne l'ait demandé — et le diff aurait montré une scission.
#[test]
fn a_produced_identity_is_never_one_already_taken() {
    let split = three()
        .apply(
            &Operation::SplitNode {
                node: agent(2),
                into: (agent(1), agent(9)),
                follows_first: BTreeSet::new(),
            },
            &Fnv,
        )
        .expect_err("agent(1) est déjà membre");
    assert_eq!(
        split,
        VersionError::NodeAlreadyPresent {
            node: agent(1).to_string()
        }
    );

    let merged = three()
        .apply(
            &Operation::MergeNodes {
                first: agent(1),
                second: agent(3),
                into: agent(2),
            },
            &Fnv,
        )
        .expect_err("agent(2) est déjà membre");
    assert_eq!(
        merged,
        VersionError::NodeAlreadyPresent {
            node: agent(2).to_string()
        }
    );
}

#[test]
fn a_split_only_shares_its_own_edges() {
    let all_to_first = three()
        .apply(
            &Operation::SplitNode {
                node: agent(2),
                into: (agent(8), agent(9)),
                follows_first: [reviews(agent(1), agent(2)), reviews(agent(3), agent(2))]
                    .into_iter()
                    .collect::<BTreeSet<_>>(),
            },
            &Fnv,
        )
        .expect("les deux arêtes touchent bien agent(2)");
    assert_eq!(all_to_first.relations().len(), 2);
    assert!(
        all_to_first.members().contains(&agent(9)),
        "un côté peut rester isolé"
    );

    let refused = three()
        .apply(
            &Operation::SplitNode {
                node: agent(1),
                into: (agent(8), agent(9)),
                follows_first: [reviews(agent(3), agent(2))].into_iter().collect(),
            },
            &Fnv,
        )
        .expect_err("cette arête ne touche pas agent(1)");
    assert!(matches!(refused, VersionError::NotIncident { .. }));
}

#[test]
fn a_root_refuses_a_dangling_edge() {
    let error = Version::root(&[agent(1)], &[reviews(agent(1), agent(2))], &Fnv)
        .expect_err("agent(2) n'est pas membre");
    assert!(matches!(error, VersionError::DanglingEdge { .. }));
}
