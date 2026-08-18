//! Les métriques structurelles d'une version de coordination — `docs/10_V1_ROADMAP.md`, item `R3`.
//!
//! # Ce qu'une métrique doit mesurer pour valoir quelque chose
//!
//! Une métrique de graphe qui mesure une propriété **déjà garantie** ne dit rien : elle rend la même
//! valeur sur tout ce que le système accepte, et son passage au vert n'a jamais été en jeu. Les cinq
//! métriques d'ici ont donc été choisies pour ce qu'aucun invariant ne force.
//!
//! La réciprocité de revue a bien failli être écartée pour cette raison — `A relit B` et `B relit A`
//! est un cycle de longueur deux, et [`crate::region`] veto déjà `ReviewAcyclicity`. Vérification
//! faite, elle est le contraire d'une métrique morte : le veto s'applique à un **diff**, et
//! [`Version::root`] ne refuse que les arêtes pendantes et les auto-relations. Une version racine,
//! ou une version arrivée par un autre chemin, peut donc parfaitement porter l'aller-retour. Le
//! veto garde les transitions ; la métrique lit les états, et le premier état n'a jamais été gardé.
//!
//! C'est aussi la métrique la plus intéressante des cinq : la revue mutuelle est la forme à deux du
//! consensus circulaire de §16.6, transposée de l'épistémique à la coordination.
//!
//! # Ce que ce module ne fait pas
//!
//! Il ne juge pas. Aucune des cinq n'a de seuil, et aucun `Metrics` ne rend « bon » ou « mauvais ».
//! Un seuil inventé ici deviendrait la définition d'une bonne organisation, alors que c'est une
//! question de politique et de portefeuille (§13, §20) — et qu'un chiffre écrit en Rust a l'air
//! d'une décision prise.

use std::collections::{BTreeMap, BTreeSet};

use locus_protocol::{Id, id::Agent};

use crate::proposal::RelationKind;
use crate::version::Version;

/// Les cinq métriques d'une version.
///
/// Toutes rendues ensemble, en un passage. Les calculer à la demande ferait lire la même version
/// cinq fois et permettrait d'en rapporter quatre, ce qui est la façon la plus discrète de choisir
/// ce qu'on montre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Metrics {
    members: usize,
    reviewed_members: usize,
    review_depth: usize,
    busiest_reviewer_load: usize,
    review_edges: usize,
    mutual_review_pairs: usize,
    blind_members: usize,
}

impl Metrics {
    /// Mesurer une version.
    #[must_use]
    pub fn of(version: &Version) -> Self {
        let members = version.members();
        let reviews = edges(version, RelationKind::Review);
        let sees = edges(version, RelationKind::Visibility);

        let reviewed: BTreeSet<Id<Agent>> = reviews.values().flatten().copied().collect();
        let review_edges = reviews.values().map(BTreeSet::len).sum();
        let busiest = reviews.values().map(BTreeSet::len).max().unwrap_or(0);
        let blind = members
            .iter()
            .filter(|member| sees.get(member).is_none_or(BTreeSet::is_empty))
            .count();

        let mutual = reviews
            .iter()
            .flat_map(|(from, targets)| targets.iter().map(move |to| (*from, *to)))
            .filter(|(from, to)| {
                // Comptée une fois par paire, pas deux : `A ↔ B` est **un** aller-retour.
                from < to && reviews.get(to).is_some_and(|back| back.contains(from))
            })
            .count();

        Self {
            members: members.len(),
            reviewed_members: members
                .iter()
                .filter(|member| reviewed.contains(member))
                .count(),
            review_depth: longest_chain(&reviews),
            busiest_reviewer_load: busiest,
            review_edges,
            mutual_review_pairs: mutual,
            blind_members: blind,
        }
    }

    /// Combien de membres l'organisation compte.
    #[must_use]
    pub const fn members(self) -> usize {
        self.members
    }

    /// **Couverture de revue** : les membres que quelqu'un relit.
    ///
    /// Rien ne l'impose : une version parfaitement valide peut ne relire personne. C'est ce que
    /// l'invariant 11 et §14.4 supposent acquis, et ce qu'aucun type ne vérifie.
    #[must_use]
    pub const fn reviewed_members(self) -> usize {
        self.reviewed_members
    }

    /// **Profondeur de revue** : la plus longue chaîne `A relit B relit C…`.
    ///
    /// Comptée en arêtes, donc zéro quand personne ne relit personne. Elle termine même sur une
    /// version qui porterait un cycle — et il en existe, voir [`Metrics::mutual_review_pairs`] —
    /// parce que [`longest_chain`] borne sa profondeur par le nombre de nœuds.
    #[must_use]
    pub const fn review_depth(self) -> usize {
        self.review_depth
    }

    /// **Concentration de revue** : le plus grand nombre de relus par un même relecteur.
    ///
    /// §13.3 demande « une limite de concentration par famille de modèle et méthode ». Ici la
    /// concentration se lit sur qui relit : un relecteur qui relit tout le monde fait de son
    /// jugement le seul jugement, et l'indépendance de §14.4 n'y survit pas.
    #[must_use]
    pub const fn busiest_reviewer_load(self) -> usize {
        self.busiest_reviewer_load
    }

    /// Le nombre d'arêtes de revue.
    #[must_use]
    pub const fn review_edges(self) -> usize {
        self.review_edges
    }

    /// **Revue mutuelle** : les paires `A relit B` et `B relit A`.
    ///
    /// La forme à deux du consensus circulaire de §16.6, transposée de l'épistémique à la
    /// coordination : chacun est relu, le couple ne l'est par personne. Le veto `ReviewAcyclicity`
    /// l'interdit sur un **diff**, mais [`Version::root`] ne le refuse pas — une version racine, ou
    /// arrivée par un autre chemin, peut la porter. Le veto garde les transitions ; ceci lit l'état.
    ///
    /// Comptée par **paire**, jamais par arête : `A ↔ B` est un aller-retour, pas deux.
    #[must_use]
    pub const fn mutual_review_pairs(self) -> usize {
        self.mutual_review_pairs
    }

    /// **Isolement de visibilité** : les membres qui ne voient le travail de personne.
    ///
    /// W15.e : la visibilité **restreint**, elle n'élargit jamais. Un membre sans aucune relation
    /// sortante ne voit que ce qu'il a produit lui-même — ce qui est licite, et vaut d'être compté.
    #[must_use]
    pub const fn blind_members(self) -> usize {
        self.blind_members
    }
}

/// Les arêtes d'une sorte, en liste d'adjacence.
fn edges(version: &Version, kind: RelationKind) -> BTreeMap<Id<Agent>, BTreeSet<Id<Agent>>> {
    let mut adjacency: BTreeMap<Id<Agent>, BTreeSet<Id<Agent>>> = BTreeMap::new();
    for relation in version.relations() {
        if relation.kind == kind {
            adjacency
                .entry(relation.from)
                .or_default()
                .insert(relation.to);
        }
    }
    adjacency
}

/// La plus longue chaîne, en arêtes.
///
/// Le parcours **borne** sa profondeur par le nombre de nœuds. Ce n'est pas une précaution
/// théorique : `Version::root` n'interdit pas les cycles de revue — seul le veto de `region` le fait,
/// et seulement sur un diff. Une métrique qui ne termine pas est pire qu'une métrique absente : elle
/// emporte l'appelant avec elle.
fn longest_chain(edges: &BTreeMap<Id<Agent>, BTreeSet<Id<Agent>>>) -> usize {
    let nodes: BTreeSet<Id<Agent>> = edges
        .iter()
        .flat_map(|(from, targets)| std::iter::once(*from).chain(targets.iter().copied()))
        .collect();
    let ceiling = nodes.len();

    let mut best = 0;
    for start in &nodes {
        // Parcours en largeur borné : la distance ne peut pas dépasser le nombre de nœuds.
        let mut frontier: BTreeSet<Id<Agent>> = [*start].into_iter().collect();
        let mut distance = 0;
        while !frontier.is_empty() && distance < ceiling {
            let next: BTreeSet<Id<Agent>> = frontier
                .iter()
                .filter_map(|node| edges.get(node))
                .flatten()
                .copied()
                .collect();
            if next.is_empty() {
                break;
            }
            distance += 1;
            frontier = next;
        }
        best = best.max(distance);
    }
    best
}
