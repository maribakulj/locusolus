//! Test de sortie de `R1` — **le consensus circulaire de §16.6, lu sur le graphe.**
//!
//! 1. Un cycle de `Cites` dont aucun membre n'a d'`AnchoredIn` sortant est un consensus circulaire.
//! 2. Un cycle **ancré** n'en est pas un, et le reste quel que soit le nombre de citations.
//! 3. Un ancrage **interne** au groupe n'est pas un ancrage, et le constat le dit.
//! 4. Le constat nomme le groupe, une fois — pas chacun de ses membres.

use std::collections::BTreeSet;

use locus_domain::RevisionId;
use locus_graph::{
    CircularConsensus, Graph, Relation, RelationKind, Strength, circular_consensus, citation_cycles,
};
use locus_protocol::Timestamp;

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

/// Une révision de fixture, déterministe pour une graine donnée.
fn revision(seed: u8) -> RevisionId {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    RevisionId::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn edge(from: u8, to: u8, kind: RelationKind) -> Relation {
    Relation {
        id: format!("rel_{from}_{to}_{}", kind.as_str()),
        from: revision(from),
        to: revision(to),
        kind,
        author: "agent_1".to_owned(),
        scope: "global".to_owned(),
        strength: Strength::new(0.8).expect("force bornée"),
        justification: "fixture".to_owned(),
        evidence_refs: Vec::new(),
        revision: 1,
    }
}

fn graph_of(edges: Vec<Relation>) -> Graph {
    let mut graph = Graph::new();
    for relation in edges {
        graph.add_relation(relation);
    }
    graph
}

fn members(seeds: &[u8]) -> BTreeSet<RevisionId> {
    seeds.iter().map(|seed| revision(*seed)).collect()
}

/// `1 → 2 → 3 → 1`, en citations.
fn ring() -> Vec<Relation> {
    vec![
        edge(1, 2, RelationKind::Cites),
        edge(2, 3, RelationKind::Cites),
        edge(3, 1, RelationKind::Cites),
    ]
}

// ---------------------------------------------------------------------------------------------
// 1. Un cycle sans ancre est un consensus circulaire
// ---------------------------------------------------------------------------------------------

#[test]
fn a_citation_ring_with_no_anchor_is_a_circular_consensus() {
    let graph = graph_of(ring());
    let found = circular_consensus(&graph);
    assert_eq!(found.len(), 1, "un groupe, pas trois constats");
    assert_eq!(found[0].members(), &members(&[1, 2, 3]));
    assert_eq!(found[0].size(), 3);
    assert!(found[0].internal_anchors().is_empty());
}

/// La plus petite forme : une révision qui se cite elle-même.
///
/// C'est le cycle le plus facile à écrire par accident, et une détection qui ne regarderait que les
/// composantes de taille deux ou plus le manquerait entièrement.
#[test]
fn a_self_citation_is_the_smallest_circular_consensus() {
    let graph = graph_of(vec![edge(1, 1, RelationKind::Cites)]);
    let found = circular_consensus(&graph);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].members(), &members(&[1]));
}

/// Une chaîne de citations sans retour n'est pas un cycle.
#[test]
fn a_citation_chain_is_not_a_cycle() {
    let graph = graph_of(vec![
        edge(1, 2, RelationKind::Cites),
        edge(2, 3, RelationKind::Cites),
    ]);
    assert!(citation_cycles(&graph).is_empty());
    assert!(circular_consensus(&graph).is_empty());
}

/// Un cycle d'une **autre** sorte n'est pas un cycle de citations.
///
/// §7.5 donne vingt-huit relations et elles ne sont pas interchangeables. Deux thèses qui se
/// soutiennent mutuellement posent une autre question que deux thèses qui se citent ; les mélanger
/// ferait signaler l'une pour l'autre.
#[test]
fn a_ring_of_another_kind_is_not_a_citation_cycle() {
    let graph = graph_of(vec![
        edge(1, 2, RelationKind::Supports),
        edge(2, 3, RelationKind::Supports),
        edge(3, 1, RelationKind::Supports),
    ]);
    assert!(citation_cycles(&graph).is_empty());
    assert!(circular_consensus(&graph).is_empty());
}

// ---------------------------------------------------------------------------------------------
// 2. Un cycle ancré n'en est pas un
// ---------------------------------------------------------------------------------------------

/// **La distinction qui porte l'item.** Un cycle ancré à l'extérieur reste un cycle, sans être un
/// consensus circulaire.
///
/// Deux travaux qui se citent mutuellement et tiennent tous deux à une source extérieure s'appuient
/// sur quelque chose. Les confondre ferait signaler la moitié d'une bibliographie.
#[test]
fn an_anchored_ring_is_a_cycle_but_not_a_circular_consensus() {
    let mut edges = ring();
    edges.push(edge(2, 9, RelationKind::AnchoredIn));
    let graph = graph_of(edges);

    assert_eq!(citation_cycles(&graph).len(), 1, "le cycle existe toujours");
    assert!(
        circular_consensus(&graph).is_empty(),
        "un seul ancrage sortant suffit à sortir le groupe du constat"
    );
}

/// Un seul membre ancré suffit pour tout le groupe.
///
/// Exiger que **chacun** s'ancre ferait du constat une exigence de forme bibliographique, alors que
/// §16.6 vise l'absence de fondation — et un groupe dont un membre tient à l'extérieur en a une.
#[test]
fn one_anchored_member_is_enough_for_the_whole_group() {
    for anchored in [1_u8, 2, 3] {
        let mut edges = ring();
        edges.push(edge(anchored, 9, RelationKind::AnchoredIn));
        assert!(
            circular_consensus(&graph_of(edges)).is_empty(),
            "l'ancrage de {anchored} tient le groupe entier"
        );
    }
}

/// Deux cycles séparés se rapportent séparément, et l'ancrage de l'un ne couvre pas l'autre.
#[test]
fn two_rings_are_judged_apart() {
    let mut edges = ring();
    edges.extend([
        edge(4, 5, RelationKind::Cites),
        edge(5, 4, RelationKind::Cites),
        edge(4, 9, RelationKind::AnchoredIn),
    ]);
    let graph = graph_of(edges);

    assert_eq!(citation_cycles(&graph).len(), 2);
    let found = circular_consensus(&graph);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].members(), &members(&[1, 2, 3]));
}

// ---------------------------------------------------------------------------------------------
// 3. Un ancrage interne n'est pas un ancrage
// ---------------------------------------------------------------------------------------------

/// **La faute que la forme prévient.** S'ancrer les uns dans les autres n'ancre rien.
///
/// C'est la règle de W15.c sur « un chemin passant hors de la région », transposée : ce qui compte
/// est ce qui **sort**. Un compte d'`AnchoredIn` — « ce groupe a trois ancrages » — laisserait
/// exactement ce cas passer.
#[test]
fn anchoring_inside_the_group_anchors_nothing() {
    let mut edges = ring();
    edges.extend([
        edge(1, 2, RelationKind::AnchoredIn),
        edge(2, 3, RelationKind::AnchoredIn),
        edge(3, 1, RelationKind::AnchoredIn),
    ]);
    let graph = graph_of(edges);

    let found = circular_consensus(&graph);
    assert_eq!(found.len(), 1, "trois ancrages internes n'ancrent rien");
    assert_eq!(found[0].internal_anchors(), &members(&[1, 2, 3]));
}

/// Et le constat dit **lesquels** sont internes, pour répondre à l'objection qu'il appelle.
#[test]
fn the_finding_names_the_anchors_that_do_not_count() {
    let mut edges = ring();
    edges.push(edge(1, 3, RelationKind::AnchoredIn));
    let graph = graph_of(edges);

    let found: Vec<CircularConsensus> = circular_consensus(&graph);
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].internal_anchors(),
        &members(&[3]),
        "« mais nous avons un ancrage » — il pointe dedans"
    );
}

/// Un ancrage interne **et** un externe : l'externe l'emporte, et il n'y a plus de constat.
#[test]
fn one_outward_anchor_beats_any_number_of_inward_ones() {
    let mut edges = ring();
    edges.extend([
        edge(1, 2, RelationKind::AnchoredIn),
        edge(2, 3, RelationKind::AnchoredIn),
        edge(3, 9, RelationKind::AnchoredIn),
    ]);
    assert!(circular_consensus(&graph_of(edges)).is_empty());
}

// ---------------------------------------------------------------------------------------------
// 4. Le constat nomme le groupe, une fois
// ---------------------------------------------------------------------------------------------

/// Un cycle de cinq est **un** problème, pas cinq.
///
/// Le rapporter par membre donnerait cinq fois la même chose à corriger, et ferait paraître un
/// petit graphe malade cinq fois plus qu'il ne l'est.
#[test]
fn a_ring_of_five_is_one_finding_not_five() {
    let graph = graph_of(vec![
        edge(1, 2, RelationKind::Cites),
        edge(2, 3, RelationKind::Cites),
        edge(3, 4, RelationKind::Cites),
        edge(4, 5, RelationKind::Cites),
        edge(5, 1, RelationKind::Cites),
    ]);
    let found = circular_consensus(&graph);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].size(), 5);
}

/// Deux cycles qui partagent un membre n'en font qu'un : c'est une seule composante.
#[test]
fn overlapping_rings_are_one_component() {
    let mut edges = ring();
    edges.extend([
        edge(1, 4, RelationKind::Cites),
        edge(4, 2, RelationKind::Cites),
    ]);
    let found = circular_consensus(&graph_of(edges));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].members(), &members(&[1, 2, 3, 4]));
}

/// Un cycle qui cite un **autre** cycle déjà clos ne fusionne pas avec lui.
///
/// C'est le cas que la détection rate le plus discrètement. Une arête vers une révision déjà
/// visitée peut être de deux sortes : un retour dans le groupe qu'on est en train de fermer, ou un
/// renvoi vers un groupe déjà fermé. Confondre les deux ne signale pas d'erreur — cela fait
/// simplement **disparaître** le second groupe du rapport, et un consensus circulaire non rapporté
/// se lit comme un graphe sain.
#[test]
fn a_ring_citing_a_finished_ring_is_not_absorbed_by_it() {
    let graph = graph_of(vec![
        edge(1, 2, RelationKind::Cites),
        edge(2, 1, RelationKind::Cites),
        edge(3, 4, RelationKind::Cites),
        edge(4, 3, RelationKind::Cites),
        // Le second groupe cite le premier, sans retour.
        edge(3, 1, RelationKind::Cites),
    ]);

    let cycles = citation_cycles(&graph);
    assert_eq!(cycles.len(), 2, "deux groupes, pas un : {cycles:?}");

    let found = circular_consensus(&graph);
    assert_eq!(found.len(), 2);
    let groups: Vec<&BTreeSet<RevisionId>> = found.iter().map(CircularConsensus::members).collect();
    assert!(groups.contains(&&members(&[1, 2])));
    assert!(groups.contains(&&members(&[3, 4])));
}

/// Une révision qui cite le cycle sans en être citée n'en fait pas partie.
///
/// Elle s'appuie sur un groupe auto-porteur, ce qui est une autre question que d'en être. Les
/// mélanger ferait grossir le constat à chaque nouveau lecteur du cycle.
#[test]
fn a_citer_outside_the_ring_stays_outside() {
    let mut edges = ring();
    edges.push(edge(7, 1, RelationKind::Cites));
    let found = circular_consensus(&graph_of(edges));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].members(), &members(&[1, 2, 3]));
}

/// Un graphe vide ne rend rien, et un graphe sans citation non plus.
#[test]
fn an_empty_graph_yields_nothing() {
    assert!(circular_consensus(&Graph::new()).is_empty());
    assert!(citation_cycles(&Graph::new()).is_empty());

    let graph = graph_of(vec![edge(1, 9, RelationKind::AnchoredIn)]);
    assert!(circular_consensus(&graph).is_empty());
}

/// Le résultat ne dépend pas de l'ordre dans lequel les arêtes ont été ajoutées.
///
/// Un constat qui changerait avec l'ordre d'insertion ferait douter du précédent chaque fois qu'un
/// import se rejoue.
#[test]
fn the_finding_does_not_depend_on_insertion_order() {
    let forwards = ring();
    let mut backwards = ring();
    backwards.reverse();

    let one = circular_consensus(&graph_of(forwards));
    let other = circular_consensus(&graph_of(backwards));
    assert_eq!(one, other);
}

/// Une longue chaîne ne fait pas déborder la pile.
///
/// La détection est itérative : une profondeur de récursion suivant la profondeur du graphe ferait
/// disparaître le constat exactement quand il sert, sur un grand dossier.
#[test]
fn a_long_chain_does_not_blow_the_stack() {
    let mut edges = Vec::new();
    for step in 0_u8..200 {
        edges.push(edge(step, step + 1, RelationKind::Cites));
    }
    // Refermer la chaîne en un seul très long cycle.
    edges.push(edge(200, 0, RelationKind::Cites));
    let found = circular_consensus(&graph_of(edges));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].size(), 201);
}
