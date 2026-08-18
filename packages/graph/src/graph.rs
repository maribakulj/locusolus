//! Le graphe épistémique — `docs/SPEC_V1.md` §7.5, §7.6 et §9.4.

use std::collections::BTreeMap;

use locus_domain::RevisionId;

use crate::inference::Inference;
use crate::relation::{Direction, Relation, RelationKind};

/// Ce qui étaye une conclusion : une inférence entière, ou une relation binaire.
///
/// L'énumération est le cœur de W1.e. Une conclusion soutenue par une inférence à trois prémisses
/// rend **un** `Support::Inference` portant les trois — jamais trois `Support::Relation`. Le type
/// interdit de confondre les deux, et un appelant qui ne traite qu'un cas ne compile pas.
#[derive(Debug, Clone, PartialEq)]
pub enum Support<'graph> {
    /// Une inférence, avec toutes ses prémisses.
    Inference(&'graph Inference),
    /// Une relation binaire — `supports`, `implies`, et le reste de §7.5.
    Relation(&'graph Relation),
}

/// Le graphe épistémique.
///
/// # Deux sortes d'arêtes, et elles ne se convertissent pas
///
/// Les relations de §7.5 sont binaires : une source, une cible. Les inférences de §7.6 sont des
/// **hyperarêtes** : n prémisses, m conclusions, une règle, un scope.
///
/// §7.6 : « le système NE DOIT PAS réduire un raisonnement multi-prémisses à plusieurs arêtes
/// indépendantes. » Ce graphe range donc les deux séparément et n'offre **aucun** chemin de l'une
/// vers l'autre : ni `flatten`, ni `decompose`, ni `as_edges`. Un test le vérifie par l'absence,
/// parce que c'est une fonction de commodité que quelqu'un finira par vouloir écrire.
#[derive(Debug, Default, Clone)]
pub struct Graph {
    relations: BTreeMap<String, Relation>,
    inferences: BTreeMap<String, Inference>,
}

impl Graph {
    /// Un graphe vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Ajouter une relation.
    pub fn add_relation(&mut self, relation: Relation) {
        self.relations.insert(relation.id.clone(), relation);
    }

    /// Ajouter une inférence.
    pub fn add_inference(&mut self, inference: Inference) {
        self.inferences.insert(inference.id.clone(), inference);
    }

    /// Le nombre de relations binaires.
    #[must_use]
    pub fn relation_count(&self) -> usize {
        self.relations.len()
    }

    /// Le nombre d'inférences.
    #[must_use]
    pub fn inference_count(&self) -> usize {
        self.inferences.len()
    }

    /// Vrai quand le graphe ne porte rien.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.relations.is_empty() && self.inferences.is_empty()
    }

    /// Ce qui étaye une révision — inférences **et** relations, sans les confondre.
    #[must_use]
    pub fn supports_of(&self, conclusion: &RevisionId) -> Vec<Support<'_>> {
        let mut supports: Vec<Support<'_>> = self
            .inferences
            .values()
            .filter(|inference| inference.concludes(conclusion))
            .map(Support::Inference)
            .collect();
        supports.extend(
            self.relations
                .values()
                .filter(|relation| {
                    relation.to == *conclusion
                        && matches!(
                            relation.kind,
                            RelationKind::Supports | RelationKind::Implies
                        )
                })
                .map(Support::Relation),
        );
        supports
    }

    /// Les prémisses minimales d'une conclusion — la requête de §9.4.
    ///
    /// Rend **un ensemble par inférence**, jamais un ensemble aplati. Une inférence à trois
    /// prémisses donne un ensemble de trois ; trois inférences à une prémisse donnent trois
    /// ensembles d'une. La différence est exactement ce que §7.6 protège, et c'est aussi la
    /// différence entre « il faut ces trois faits » et « il suffit d'un des trois ».
    #[must_use]
    pub fn minimal_premise_sets(&self, conclusion: &RevisionId) -> Vec<Vec<RevisionId>> {
        self.inferences
            .values()
            .filter(|inference| inference.concludes(conclusion))
            .map(|inference| inference.premise_ids.clone())
            .collect()
    }

    /// Les inférences qu'une révision fait tomber si elle est réfutée.
    ///
    /// Une prémisse sur trois suffit : c'est la différence entre une hyperarête et trois arêtes.
    /// Sur trois arêtes indépendantes, réfuter l'une en laisserait deux, et la conclusion
    /// paraîtrait « encore soutenue aux deux tiers ».
    #[must_use]
    pub fn inferences_broken_by(&self, refuted: &RevisionId) -> Vec<&Inference> {
        self.inferences
            .values()
            .filter(|inference| inference.has_premise(refuted))
            .collect()
    }

    /// Les relations partant d'une révision.
    #[must_use]
    pub fn outgoing(&self, from: &RevisionId) -> Vec<&Relation> {
        self.relations
            .values()
            .filter(|relation| relation.from == *from)
            .collect()
    }

    /// Les relations arrivant sur une révision.
    ///
    /// Une **lecture** des arêtes entrantes, jamais une déduction : le graphe ne retourne aucune
    /// relation. Voir [`Graph::traversable_backwards`].
    #[must_use]
    pub fn incoming(&self, to: &RevisionId) -> Vec<&Relation> {
        self.relations
            .values()
            .filter(|relation| relation.to == *to)
            .collect()
    }

    /// Les relations d'une sorte, dans un ordre stable.
    ///
    /// Une lecture, comme [`Graph::outgoing`] et [`Graph::incoming`] — et une lecture d'**une seule
    /// sorte à la fois** : les vingt-huit relations de §7.5 ne sont pas interchangeables, et une
    /// analyse qui les mélangerait lirait un `cites` comme un `supports`. L'ordre suit l'identité de
    /// la relation, donc deux parcours du même graphe rendent la même chose.
    pub fn relations_of_kind(&self, kind: RelationKind) -> impl Iterator<Item = &Relation> {
        self.relations
            .values()
            .filter(move |relation| relation.kind == kind)
    }

    /// Ce qu'on a le droit d'affirmer en sens inverse d'une relation — §7.5.
    ///
    /// Rend la relation réciproque quand elle existe, et `None` sinon. `None` ne veut pas dire
    /// « la réciproque est fausse » : il veut dire qu'elle **n'est pas déductible**, et qu'affirmer
    /// quoi que ce soit dans ce sens demanderait de l'écrire comme une relation à part entière.
    ///
    /// C'est la seule voie du crate vers un parcours à rebours affirmatif, et elle refuse vingt-deux
    /// relations sur vingt-huit.
    #[must_use]
    pub fn traversable_backwards(relation: &Relation) -> Option<RelationKind> {
        match relation.kind.direction() {
            Direction::Symmetric => Some(relation.kind),
            Direction::Converse(other) => Some(other),
            Direction::OneWay => None,
        }
    }
}
