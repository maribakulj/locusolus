//! Le consensus circulaire — `docs/SPEC_V1.md` §16.6, quatrième forme, et `docs/10` item `R1`.
//!
//! # Ce que W7.b couvre déjà, et ce qui manquait
//!
//! `packages/review/src/contamination.rs` détecte le consensus circulaire **dans un contexte qu'on
//! s'apprête à livrer** : la question y est « ce dossier peut-il partir vers ce destinataire ». Elle
//! porte sur des `ContextItem`, dont chacun **déclare** s'il est une source externe.
//!
//! Ce module pose la même question du graphe lui-même : *quelles parties de ce qui est cru ne
//! tiennent que sur elles-mêmes ?* C'est une propriété permanente du dossier institutionnel, qu'on
//! interroge sans destinataire et sans livraison en cours. Trois différences en découlent, et
//! aucune n'est cosmétique.
//!
//! **L'ancrage est dérivé, pas déclaré.** Ici il n'y a pas de booléen `is_external_source` : il y a
//! des arêtes `AnchoredIn`, et l'ancrage se lit dans le graphe. Un drapeau qu'un appelant pose est
//! un drapeau qu'un appelant peut oublier de poser — ou poser à tort sur ce qu'il vient d'écrire.
//!
//! **Le résultat nomme le groupe, pas chacun de ses membres.** Un cycle de cinq révisions est
//! **un** problème, et le rapporter cinq fois donne cinq fois la même chose à corriger. La
//! détection passe donc par les composantes fortement connexes du sous-graphe `Cites`.
//!
//! **Un ancrage interne n'est pas un ancrage.** Si les membres d'un cycle s'ancrent les uns dans
//! les autres, le groupe ne s'appuie sur rien de plus qu'avant. C'est exactement la règle de W15.c
//! sur « un chemin passant hors de la région », transposée : ce qui compte est ce qui sort.
//!
//! # Ce que ce module ne fait pas
//!
//! Il ne supprime rien, ne marque rien, ne fait descendre aucun niveau de validation. L'invariant 12
//! interdit d'effacer un conflit pour rendre le graphe propre, et un consensus circulaire **est** un
//! constat, pas une faute prouvée : deux résultats indépendants peuvent se citer en rond sans qu'on
//! ait pensé à écrire les ancrages. Le module rend une liste ; ce qu'on en fait est une décision.

use std::collections::{BTreeMap, BTreeSet};

use locus_domain::RevisionId;

use crate::graph::Graph;
use crate::relation::RelationKind;

/// Un groupe de révisions qui se citent en rond sans rien tenir de l'extérieur.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CircularConsensus {
    members: BTreeSet<RevisionId>,
    internal_anchors: BTreeSet<RevisionId>,
}

impl CircularConsensus {
    /// Les révisions prises dans le cycle.
    #[must_use]
    pub const fn members(&self) -> &BTreeSet<RevisionId> {
        &self.members
    }

    /// Combien elles sont.
    #[must_use]
    pub fn size(&self) -> usize {
        self.members.len()
    }

    /// Les révisions du groupe vers lesquelles un `AnchoredIn` du groupe pointe.
    ///
    /// Vide la plupart du temps. Quand elle ne l'est pas, elle répond à l'objection que le constat
    /// appelle — « mais nous **avons** des ancrages » — en montrant qu'ils pointent tous dedans. Un
    /// constat qui laisserait chercher cette réponse à la main serait lu comme une erreur de
    /// détection.
    #[must_use]
    pub const fn internal_anchors(&self) -> &BTreeSet<RevisionId> {
        &self.internal_anchors
    }
}

/// Les cycles de citation d'un graphe, ancrés ou non.
///
/// Une composante fortement connexe du sous-graphe `Cites`, de taille au moins deux — ou de taille
/// un lorsqu'une révision se cite **elle-même**, ce qui est le plus petit cycle possible et le plus
/// facile à écrire par accident.
///
/// Rendus séparément du constat de §16.6 parce qu'un cycle **ancré** est parfaitement légitime :
/// deux travaux qui se citent mutuellement et tiennent tous deux à une source extérieure
/// s'appuient sur quelque chose. Les confondre ferait signaler la moitié d'une bibliographie.
#[must_use]
pub fn citation_cycles(graph: &Graph) -> Vec<BTreeSet<RevisionId>> {
    let cites = adjacency(graph, RelationKind::Cites);
    strongly_connected(&cites)
        .into_iter()
        .filter(|component| {
            component.len() > 1
                || component.iter().any(|revision| {
                    cites
                        .get(revision)
                        .is_some_and(|targets| targets.contains(revision))
                })
        })
        .collect()
}

/// Les consensus circulaires : les cycles de citation qu'aucun `AnchoredIn` ne fait sortir.
#[must_use]
pub fn circular_consensus(graph: &Graph) -> Vec<CircularConsensus> {
    let anchors = adjacency(graph, RelationKind::AnchoredIn);
    citation_cycles(graph)
        .into_iter()
        .filter_map(|members| {
            let (inside, outside) = split_anchors(&anchors, &members);
            outside.is_empty().then_some(CircularConsensus {
                members,
                internal_anchors: inside,
            })
        })
        .collect()
}

/// Les ancres d'un groupe, réparties selon qu'elles sortent ou non.
fn split_anchors(
    anchors: &BTreeMap<RevisionId, BTreeSet<RevisionId>>,
    members: &BTreeSet<RevisionId>,
) -> (BTreeSet<RevisionId>, BTreeSet<RevisionId>) {
    let mut inside = BTreeSet::new();
    let mut outside = BTreeSet::new();
    for member in members {
        for target in anchors.get(member).into_iter().flatten() {
            if members.contains(target) {
                inside.insert(*target);
            } else {
                outside.insert(*target);
            }
        }
    }
    (inside, outside)
}

/// Les arêtes d'une sorte, en liste d'adjacence.
fn adjacency(graph: &Graph, kind: RelationKind) -> BTreeMap<RevisionId, BTreeSet<RevisionId>> {
    let mut edges: BTreeMap<RevisionId, BTreeSet<RevisionId>> = BTreeMap::new();
    for relation in graph.relations_of_kind(kind) {
        edges.entry(relation.from).or_default().insert(relation.to);
    }
    edges
}

/// Les composantes fortement connexes, par l'algorithme de Tarjan.
///
/// Itératif plutôt que récursif : la profondeur de la pile suivrait la profondeur du graphe, et un
/// dossier institutionnel n'a pas de raison d'être plat. Une détection de consensus circulaire qui
/// déborderait la pile sur un grand graphe serait absente exactement quand elle sert.
fn strongly_connected(
    edges: &BTreeMap<RevisionId, BTreeSet<RevisionId>>,
) -> Vec<BTreeSet<RevisionId>> {
    let nodes: BTreeSet<RevisionId> = edges
        .iter()
        .flat_map(|(from, targets)| std::iter::once(*from).chain(targets.iter().copied()))
        .collect();

    let mut index = BTreeMap::new();
    let mut lowlink = BTreeMap::new();
    let mut on_stack = BTreeSet::new();
    let mut stack: Vec<RevisionId> = Vec::new();
    let mut next = 0_usize;
    let mut components = Vec::new();

    for root in &nodes {
        if index.contains_key(root) {
            continue;
        }
        // Chaque entrée est un nœud et la position du prochain successeur à examiner.
        let mut work: Vec<(RevisionId, usize)> = vec![(*root, 0)];
        while let Some((node, offset)) = work.pop() {
            if offset == 0 {
                index.insert(node, next);
                lowlink.insert(node, next);
                next += 1;
                stack.push(node);
                on_stack.insert(node);
            }
            let successors: Vec<RevisionId> = edges
                .get(&node)
                .map(|targets| targets.iter().copied().collect())
                .unwrap_or_default();

            if let Some(successor) = successors.get(offset).copied() {
                work.push((node, offset + 1));
                if index.contains_key(&successor) {
                    if on_stack.contains(&successor) {
                        let seen = index[&successor];
                        let current = lowlink[&node];
                        lowlink.insert(node, current.min(seen));
                    }
                } else {
                    work.push((successor, 0));
                }
                continue;
            }

            // Tous les successeurs sont traités : remonter le `lowlink` vers le parent.
            if let Some((parent, _)) = work.last().copied() {
                let child = lowlink[&node];
                let current = lowlink[&parent];
                lowlink.insert(parent, current.min(child));
            }
            if lowlink[&node] == index[&node] {
                let mut component = BTreeSet::new();
                while let Some(member) = stack.pop() {
                    on_stack.remove(&member);
                    component.insert(member);
                    if member == node {
                        break;
                    }
                }
                components.push(component);
            }
        }
    }
    components
}
