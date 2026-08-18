//! La version canonique immuable et le jeu d'opérations — `docs/13` §3, ADR 0016 décisions 4 et 5.
//!
//! # Deux hashes, et c'est le sujet du module
//!
//! Une version porte **un hash de contenu** et **un hash de version**. Le premier ne dépend que de
//! ce que la version contient : deux organisations identiques le partagent, quelle que soit
//! l'histoire qui y a mené. Le second ajoute le parent, et c'est lui l'identité.
//!
//! La séparation n'est pas décorative, elle rend testable la phrase de l'ADR 0016 décision 5 :
//! « une annulation est le commit d'un changement inverse […] aucune version antérieure n'est
//! supprimée ». Défaire une opération rend **le même contenu** et **une autre version** — l'état
//! revient, l'histoire non. Avec un seul hash il aurait fallu choisir : ou bien défaire ramène
//! littéralement à la version d'avant, et l'histoire est fausse ; ou bien défaire produit un état
//! qu'on ne peut pas reconnaître comme celui d'avant, et personne ne peut vérifier qu'une
//! annulation a bien annulé.
//!
//! # Un IR déclaratif, jamais un script
//!
//! `docs/10` : « la représentation détermine ce qui est vérifiable ». Une opération énonce son
//! résultat ; elle ne le calcule pas. La conséquence visible est le refus de la **cascade** :
//! retirer un nœud qui porte encore des arêtes est refusé au lieu d'emporter les arêtes avec lui.
//! Une cascade est un script — elle fait au commit des choses que le diff ne montrait pas, et
//! l'approbation aurait porté sur autre chose que ce qui s'applique.
//!
//! # Sept opérations, et quatre qui attendent leur lecteur
//!
//! `docs/13` nomme onze opérations cibles. La règle « aucune sémantique inerte » (ADR 0016,
//! décision 4) vaut pour une opération comme pour une sorte de relation, et elle trace ici une
//! frontière nette : une opération **structurelle** — nœuds et arêtes — a son effet entièrement
//! défini par l'état que ce crate détient, donc un consommateur exécutable et testé, qui est
//! [`Version::apply`]. Une opération **attributaire** écrit sur un nœud un champ dont le lecteur
//! vit ailleurs ; l'écrire ici produirait un attribut que le système sait versionner, différencier,
//! approuver et afficher, et que rien n'honore.
//!
//! Les quatre absentes, et ce que chacune attend :
//!
//! - `SET_ROLE` : les `extraInstructions` additives de l'overlay du worker (décision 4) ;
//! - `SET_VISIBILITY` : la construction de `ContextView` (décision 11) ;
//! - `SET_VALIDATOR` : qu'un validateur soit un nœud — `docs/13` le range dans les nœuds « plus
//!   tard », et il n'y en a pas aujourd'hui ;
//! - `SET_EXECUTION_ORDER` : qu'une chose ordonne des attempts **entre instances d'agent**. La
//!   décision 4 a déjà fait cette vérification en instruisant `dependency` : `steps` ordonne à
//!   l'intérieur d'un workflow, et le scheduler de §12 place sans ordonner.
//!
//! # Fusionner se compense, se défait pas
//!
//! Six des sept opérations ont un inverse exact. La fusion n'en a pas, et pour une raison qui se
//! lit dans sa définition : elle perd la partition. Deux arêtes `X → premier` et `X → second`
//! deviennent une seule `X → fusionné`, et rien dans le résultat ne dit qu'elles étaient deux. La
//! scission, elle, énonce sa partition, donc sa fusion inverse la restitue.
//!
//! [`Undo::Compensating`] nomme cette asymétrie plutôt que de la cacher derrière une fonction qui
//! rendrait une scission approximative. ADR 0016, décision 5 : « une modification non inversible ne
//! peut être que compensée, et elle le déclare à la proposition ».

use std::collections::BTreeSet;
use std::fmt;

use locus_domain::ContentHash;
use locus_protocol::{Id, id::Agent};

use crate::proposal::Relation;

/// La ligne d'en-tête de la forme canonique d'un contenu.
const CONTENT_MAGIC: &str = "coordination-content/1";

/// La ligne d'en-tête de la forme canonique d'une identité de version.
const VERSION_MAGIC: &str = "coordination-version/1";

/// Ce qui calcule le condensat d'une forme canonique — un port.
///
/// Ce crate ne choisit aucun algorithme, pour la raison que `locus_domain::ContentHash` énonce
/// déjà : ce serait une décision d'infrastructure. Ce qu'il garantit est plus fort et plus utile —
/// deux contenus égaux produisent la **même** forme canonique, octet pour octet, et un test la fige
/// en fixture.
pub trait Digest {
    /// Le condensat de `canonical`.
    fn digest(&self, canonical: &str) -> ContentHash;
}

/// L'identité d'une version : son contenu **et** d'où elle vient.
///
/// Deux versions de même contenu et de parents différents sont deux versions. C'est ce qui empêche
/// une annulation de se faire passer pour un retour en arrière.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VersionId(ContentHash);

impl VersionId {
    /// Le hash sous-jacent.
    #[must_use]
    pub const fn hash(&self) -> &ContentHash {
        &self.0
    }
}

impl fmt::Display for VersionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// Les sept opérations structurelles de `docs/13` qui ont un consommateur exécutable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    /// `ADD_NODE` — une instance d'agent entre.
    AddNode(Id<Agent>),
    /// `REMOVE_NODE` — elle sort. **Sans cascade** : ses arêtes se retirent avant, une par une.
    RemoveNode(Id<Agent>),
    /// `REPLACE_NODE` — une identité en remplace une autre, arêtes comprises.
    ReplaceNode {
        /// Celle qui sort.
        from: Id<Agent>,
        /// Celle qui entre.
        to: Id<Agent>,
    },
    /// `ADD_EDGE` — une relation de coordination.
    AddEdge(Relation),
    /// `REMOVE_EDGE` — elle disparaît.
    RemoveEdge(Relation),
    /// `SPLIT_NODE` — un nœud devient deux, et la partition de ses arêtes est **énoncée**.
    SplitNode {
        /// Celui qui disparaît.
        node: Id<Agent>,
        /// Les deux qui le remplacent, dans cet ordre.
        into: (Id<Agent>, Id<Agent>),
        /// Les arêtes incidentes qui suivent le premier ; les autres suivent le second.
        follows_first: BTreeSet<Relation>,
    },
    /// `MERGE_NODES` — deux nœuds deviennent un, et la partition est perdue.
    MergeNodes {
        /// Le premier.
        first: Id<Agent>,
        /// Le second.
        second: Id<Agent>,
        /// L'identité produite, neuve.
        into: Id<Agent>,
    },
}

impl Operation {
    /// Les sept noms, ceux de `docs/13`.
    ///
    /// Les quatre absents — `SET_ROLE`, `SET_VISIBILITY`, `SET_VALIDATOR`, `SET_EXECUTION_ORDER` —
    /// le sont parce qu'aucun lecteur n'existe pour l'attribut qu'ils écriraient, et un test le
    /// tient par l'absence. La liste est ici pour que ce test ait quelque chose à lire.
    pub const NAMES: [&'static str; 7] = [
        "ADD_NODE",
        "REMOVE_NODE",
        "REPLACE_NODE",
        "ADD_EDGE",
        "REMOVE_EDGE",
        "SPLIT_NODE",
        "MERGE_NODES",
    ];

    /// Son nom, celui de `docs/13`.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::AddNode(_) => "ADD_NODE",
            Self::RemoveNode(_) => "REMOVE_NODE",
            Self::ReplaceNode { .. } => "REPLACE_NODE",
            Self::AddEdge(_) => "ADD_EDGE",
            Self::RemoveEdge(_) => "REMOVE_EDGE",
            Self::SplitNode { .. } => "SPLIT_NODE",
            Self::MergeNodes { .. } => "MERGE_NODES",
        }
    }

    /// Ce qui défait cette opération, quand quelque chose la défait.
    ///
    /// « Défaire » veut dire : appliqué à la version que celle-ci a produite, rendre le **contenu**
    /// d'avant. Pas la version d'avant — l'histoire ne se défait pas, et c'est [`Version`] qui le
    /// tient.
    ///
    /// La fusion rend [`Undo::Compensating`] parce qu'elle perd la partition : deux arêtes
    /// `X → premier` et `X → second` deviennent une seule, et aucune scission ne saurait dire
    /// laquelle était laquelle. Rendre ici une scission plausible ferait passer une compensation
    /// pour une annulation.
    #[must_use]
    pub fn undo(&self) -> Undo {
        match self {
            Self::AddNode(node) => Undo::Exact(Self::RemoveNode(*node)),
            Self::RemoveNode(node) => Undo::Exact(Self::AddNode(*node)),
            Self::ReplaceNode { from, to } => Undo::Exact(Self::ReplaceNode {
                from: *to,
                to: *from,
            }),
            Self::AddEdge(relation) => Undo::Exact(Self::RemoveEdge(*relation)),
            Self::RemoveEdge(relation) => Undo::Exact(Self::AddEdge(*relation)),
            Self::SplitNode { node, into, .. } => Undo::Exact(Self::MergeNodes {
                first: into.0,
                second: into.1,
                into: *node,
            }),
            Self::MergeNodes { .. } => Undo::Compensating,
        }
    }
}

impl fmt::Display for Operation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

/// Ce qui défait une opération, ou le fait qu'aucune ne la défasse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Undo {
    /// L'opération qui, appliquée ensuite, rend le contenu d'avant.
    Exact(Operation),
    /// Rien ne la défait : elle se **compense**, et la proposition le déclare.
    ///
    /// ADR 0016, décision 5. Ce n'est pas un manque : c'est le refus de rendre une opération
    /// plausible là où l'information nécessaire n'existe plus.
    Compensating,
}

impl Undo {
    /// L'opération inverse, quand il y en a une.
    ///
    /// Rend `None` pour une compensation, et c'est **le point** : il n'existe dans ce module aucune
    /// fonction qui rende une opération pour une fusion.
    #[must_use]
    pub const fn exact(&self) -> Option<&Operation> {
        match self {
            Self::Exact(operation) => Some(operation),
            Self::Compensating => None,
        }
    }
}

/// Une version canonique immuable.
///
/// Elle ne s'édite pas : [`Version::apply`] en rend une autre. Ce n'est pas une discipline d'appel,
/// il n'y a aucun accesseur mutable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    id: VersionId,
    parent: Option<VersionId>,
    members: BTreeSet<Id<Agent>>,
    relations: BTreeSet<Relation>,
    content: ContentHash,
}

impl Version {
    /// La première version, sans parent.
    ///
    /// # Errors
    ///
    /// [`VersionError::DanglingEdge`] pour une relation dont une extrémité n'est pas membre, et
    /// [`VersionError::SelfRelation`] pour une relation d'un agent vers lui-même : sous la seule
    /// sorte qui existe, ce serait un agent qui se relit, ce que §14.4 et l'invariant 11 refusent.
    pub fn root(
        members: &[Id<Agent>],
        relations: &[Relation],
        digest: &impl Digest,
    ) -> Result<Self, VersionError> {
        let members: BTreeSet<Id<Agent>> = members.iter().copied().collect();
        let relations: BTreeSet<Relation> = relations.iter().copied().collect();
        for relation in &relations {
            check_relation(relation, &members)?;
        }
        Ok(Self::seal(None, members, relations, digest))
    }

    /// Son identité.
    #[must_use]
    pub const fn id(&self) -> &VersionId {
        &self.id
    }

    /// La version dont elle descend.
    #[must_use]
    pub const fn parent(&self) -> Option<&VersionId> {
        self.parent.as_ref()
    }

    /// Le hash de ce qu'elle contient, indépendant de l'histoire.
    #[must_use]
    pub const fn content_hash(&self) -> &ContentHash {
        &self.content
    }

    /// Les instances d'agent.
    #[must_use]
    pub const fn members(&self) -> &BTreeSet<Id<Agent>> {
        &self.members
    }

    /// Les relations de coordination.
    #[must_use]
    pub const fn relations(&self) -> &BTreeSet<Relation> {
        &self.relations
    }

    /// La forme canonique du contenu.
    ///
    /// Les lignes sont triées **en tant que chaînes**, pas dans l'ordre d'un conteneur : deux
    /// producteurs qui rempliraient leurs collections différemment doivent produire les mêmes
    /// octets, sinon le hash cesse de dire ce qu'il prétend dire.
    #[must_use]
    pub fn canonical(&self) -> String {
        content_canonical(&self.members, &self.relations)
    }

    /// Appliquer une opération, produisant la version suivante.
    ///
    /// # Errors
    ///
    /// Toutes les variantes de [`VersionError`]. Aucune n'est un détail : chacune nomme une chose
    /// que l'appelant doit écrire explicitement dans son diff plutôt que de la laisser arriver.
    pub fn apply(&self, operation: &Operation, digest: &impl Digest) -> Result<Self, VersionError> {
        let mut members = self.members.clone();
        let mut relations = self.relations.clone();
        match operation {
            Operation::AddNode(node) => add_node(&mut members, *node)?,
            Operation::RemoveNode(node) => remove_node(&mut members, &relations, *node)?,
            Operation::ReplaceNode { from, to } => {
                replace_node(&mut members, &mut relations, *from, *to)?;
            }
            Operation::AddEdge(relation) => add_edge(&members, &mut relations, *relation)?,
            Operation::RemoveEdge(relation) => remove_edge(&mut relations, *relation)?,
            Operation::SplitNode {
                node,
                into,
                follows_first,
            } => split_node(&mut members, &mut relations, *node, *into, follows_first)?,
            Operation::MergeNodes {
                first,
                second,
                into,
            } => merge_nodes(&mut members, &mut relations, *first, *second, *into)?,
        }
        Ok(Self::seal(
            Some(self.id.clone()),
            members,
            relations,
            digest,
        ))
    }

    fn seal(
        parent: Option<VersionId>,
        members: BTreeSet<Id<Agent>>,
        relations: BTreeSet<Relation>,
        digest: &impl Digest,
    ) -> Self {
        let content = digest.digest(&content_canonical(&members, &relations));
        let id = VersionId(digest.digest(&identity_canonical(parent.as_ref(), &content)));
        Self {
            id,
            parent,
            members,
            relations,
            content,
        }
    }
}

fn add_node(members: &mut BTreeSet<Id<Agent>>, node: Id<Agent>) -> Result<(), VersionError> {
    if members.insert(node) {
        return Ok(());
    }
    Err(VersionError::NodeAlreadyPresent {
        node: node.to_string(),
    })
}

/// Retirer un nœud, **sans cascade**.
///
/// Un nœud qui porte encore des arêtes n'est pas retiré : l'appelant les retire d'abord, une par
/// une, dans le diff. Emporter les arêtes avec le nœud ferait au commit ce que le diff ne montrait
/// pas, et l'approbation aurait porté sur autre chose que ce qui s'applique.
fn remove_node(
    members: &mut BTreeSet<Id<Agent>>,
    relations: &BTreeSet<Relation>,
    node: Id<Agent>,
) -> Result<(), VersionError> {
    if !members.contains(&node) {
        return Err(VersionError::NoSuchNode {
            node: node.to_string(),
        });
    }
    let attached = incident(relations, &node).count();
    if attached > 0 {
        return Err(VersionError::NodeStillConnected {
            node: node.to_string(),
            edges: attached,
        });
    }
    members.remove(&node);
    Ok(())
}

fn replace_node(
    members: &mut BTreeSet<Id<Agent>>,
    relations: &mut BTreeSet<Relation>,
    from: Id<Agent>,
    to: Id<Agent>,
) -> Result<(), VersionError> {
    if from == to {
        return Err(VersionError::SameNode {
            node: from.to_string(),
        });
    }
    if !members.contains(&from) {
        return Err(VersionError::NoSuchNode {
            node: from.to_string(),
        });
    }
    if members.contains(&to) {
        return Err(VersionError::NodeAlreadyPresent {
            node: to.to_string(),
        });
    }
    members.remove(&from);
    members.insert(to);
    *relations = substitute(relations, &[(from, to)]);
    Ok(())
}

fn add_edge(
    members: &BTreeSet<Id<Agent>>,
    relations: &mut BTreeSet<Relation>,
    relation: Relation,
) -> Result<(), VersionError> {
    check_relation(&relation, members)?;
    if relations.insert(relation) {
        return Ok(());
    }
    Err(VersionError::EdgeAlreadyPresent {
        edge: render(&relation),
    })
}

fn remove_edge(relations: &mut BTreeSet<Relation>, relation: Relation) -> Result<(), VersionError> {
    if relations.remove(&relation) {
        return Ok(());
    }
    Err(VersionError::NoSuchEdge {
        edge: render(&relation),
    })
}

/// Scinder un nœud en deux, selon la partition que l'opération **énonce**.
///
/// Les deux identités produites sont neuves. Laisser l'une d'elles reprendre celle du nœud scindé
/// ferait dire à la version qu'un agent a été scindé et qu'il a survécu inchangé, et rien en aval
/// ne saurait dire laquelle des deux moitiés est l'originale.
fn split_node(
    members: &mut BTreeSet<Id<Agent>>,
    relations: &mut BTreeSet<Relation>,
    node: Id<Agent>,
    into: (Id<Agent>, Id<Agent>),
    follows_first: &BTreeSet<Relation>,
) -> Result<(), VersionError> {
    let (first, second) = into;
    if first == second {
        return Err(VersionError::SameNode {
            node: first.to_string(),
        });
    }
    if !members.contains(&node) {
        return Err(VersionError::NoSuchNode {
            node: node.to_string(),
        });
    }
    for produced in [first, second] {
        if members.contains(&produced) {
            return Err(VersionError::NodeAlreadyPresent {
                node: produced.to_string(),
            });
        }
    }
    for declared in follows_first {
        if declared.from != node && declared.to != node {
            return Err(VersionError::NotIncident {
                edge: render(declared),
                node: node.to_string(),
            });
        }
        if !relations.contains(declared) {
            return Err(VersionError::NoSuchEdge {
                edge: render(declared),
            });
        }
    }
    members.remove(&node);
    members.insert(first);
    members.insert(second);
    *relations = relations
        .iter()
        .map(|relation| {
            if relation.from != node && relation.to != node {
                return *relation;
            }
            let side = if follows_first.contains(relation) {
                first
            } else {
                second
            };
            rewrite(relation, &[(node, side)])
        })
        .collect();
    Ok(())
}

/// Fusionner deux nœuds en une identité neuve.
///
/// `into` doit être absent, symétriquement à la scission : si le fusionné reprenait l'identité de
/// l'un des deux, l'histoire ne distinguerait plus « ils ont fusionné » de « l'autre a été retiré ».
fn merge_nodes(
    members: &mut BTreeSet<Id<Agent>>,
    relations: &mut BTreeSet<Relation>,
    first: Id<Agent>,
    second: Id<Agent>,
    into: Id<Agent>,
) -> Result<(), VersionError> {
    if first == second {
        return Err(VersionError::SameNode {
            node: first.to_string(),
        });
    }
    for absorbed in [first, second] {
        if !members.contains(&absorbed) {
            return Err(VersionError::NoSuchNode {
                node: absorbed.to_string(),
            });
        }
    }
    if members.contains(&into) {
        return Err(VersionError::NodeAlreadyPresent {
            node: into.to_string(),
        });
    }
    // Une relation qui joignait les deux fusionnés deviendrait une relation d'un agent vers
    // lui-même. Sous « review », ce serait un agent qui se relit, obtenu sans qu'aucune opération
    // ne l'ait demandé : la fusion est refusée, et l'appelant retire l'arête d'abord — dans le
    // diff, où l'approbateur la voit.
    let joins = |relation: &Relation, left: Id<Agent>, right: Id<Agent>| {
        relation.from == left && relation.to == right
    };
    for relation in relations.iter() {
        if joins(relation, first, second) || joins(relation, second, first) {
            return Err(VersionError::SelfRelation {
                node: into.to_string(),
            });
        }
    }
    members.remove(&first);
    members.remove(&second);
    members.insert(into);
    *relations = substitute(relations, &[(first, into), (second, into)]);
    Ok(())
}

/// La forme canonique d'un contenu — voir [`Version::canonical`].
fn content_canonical(members: &BTreeSet<Id<Agent>>, relations: &BTreeSet<Relation>) -> String {
    let mut lines: Vec<String> =
        members
            .iter()
            .map(|member| format!("n\t{member}"))
            .chain(relations.iter().map(|relation| {
                format!("e\t{}\t{}\t{}", relation.from, relation.kind, relation.to)
            }))
            .collect();
    lines.sort_unstable();
    let mut canonical = String::from(CONTENT_MAGIC);
    for line in lines {
        canonical.push('\n');
        canonical.push_str(&line);
    }
    canonical.push('\n');
    canonical
}

/// La forme canonique de l'identité : le contenu **et** le parent.
fn identity_canonical(parent: Option<&VersionId>, content: &ContentHash) -> String {
    let parent = parent.map_or_else(|| "-".to_owned(), ToString::to_string);
    format!("{VERSION_MAGIC}\nparent\t{parent}\ncontent\t{content}\n")
}

fn render(relation: &Relation) -> String {
    format!("{} -{}-> {}", relation.from, relation.kind, relation.to)
}

fn incident<'a>(
    relations: &'a BTreeSet<Relation>,
    node: &'a Id<Agent>,
) -> impl Iterator<Item = &'a Relation> {
    relations
        .iter()
        .filter(move |relation| relation.from == *node || relation.to == *node)
}

fn check_relation(relation: &Relation, members: &BTreeSet<Id<Agent>>) -> Result<(), VersionError> {
    if relation.from == relation.to {
        return Err(VersionError::SelfRelation {
            node: relation.from.to_string(),
        });
    }
    for endpoint in [relation.from, relation.to] {
        if !members.contains(&endpoint) {
            return Err(VersionError::DanglingEdge {
                edge: render(relation),
                endpoint: endpoint.to_string(),
            });
        }
    }
    Ok(())
}

fn rewrite(relation: &Relation, substitutions: &[(Id<Agent>, Id<Agent>)]) -> Relation {
    let map = |node: Id<Agent>| {
        substitutions
            .iter()
            .find(|(from, _)| *from == node)
            .map_or(node, |(_, to)| *to)
    };
    Relation {
        from: map(relation.from),
        to: map(relation.to),
        kind: relation.kind,
    }
}

/// Réécrire toutes les arêtes. Le résultat est un ensemble : deux arêtes qui deviennent égales
/// n'en font qu'une, et c'est exactement l'information qu'une fusion perd.
fn substitute(
    relations: &BTreeSet<Relation>,
    substitutions: &[(Id<Agent>, Id<Agent>)],
) -> BTreeSet<Relation> {
    relations
        .iter()
        .map(|relation| rewrite(relation, substitutions))
        .collect()
}

/// Ce qui empêche une opération de s'appliquer.
///
/// Chaque variante nomme une chose que l'appelant doit écrire dans son diff. Un refus générique
/// laisserait croire que l'opération était mal formée alors qu'elle était incomplète.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionError {
    /// Ce nœud est déjà là.
    NodeAlreadyPresent {
        /// Lequel.
        node: String,
    },
    /// Ce nœud n'y est pas.
    NoSuchNode {
        /// Lequel.
        node: String,
    },
    /// Ce nœud porte encore des arêtes. **Aucune cascade** : elles se retirent avant.
    NodeStillConnected {
        /// Lequel.
        node: String,
        /// Combien d'arêtes.
        edges: usize,
    },
    /// Cette arête est déjà là.
    EdgeAlreadyPresent {
        /// Laquelle.
        edge: String,
    },
    /// Cette arête n'y est pas.
    NoSuchEdge {
        /// Laquelle.
        edge: String,
    },
    /// Une arête dont une extrémité n'est pas membre.
    DanglingEdge {
        /// Laquelle.
        edge: String,
        /// L'extrémité absente.
        endpoint: String,
    },
    /// Une relation d'un agent vers lui-même.
    SelfRelation {
        /// Lequel.
        node: String,
    },
    /// Une arête déclarée dans une scission sans toucher le nœud scindé.
    NotIncident {
        /// Laquelle.
        edge: String,
        /// Le nœud scindé.
        node: String,
    },
    /// Deux identités qui devaient différer.
    SameNode {
        /// Laquelle.
        node: String,
    },
}

impl fmt::Display for VersionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NodeAlreadyPresent { node } => write!(formatter, "{node} est déjà membre"),
            Self::NoSuchNode { node } => write!(formatter, "{node} n'est pas membre"),
            Self::NodeStillConnected { node, edges } => write!(
                formatter,
                "{node} porte encore {edges} arête(s) : les retirer d'abord, une par une — une \
                 cascade ferait au commit ce que le diff ne montrait pas"
            ),
            Self::EdgeAlreadyPresent { edge } => write!(formatter, "« {edge} » existe déjà"),
            Self::NoSuchEdge { edge } => write!(formatter, "« {edge} » n'existe pas"),
            Self::DanglingEdge { edge, endpoint } => {
                write!(formatter, "« {edge} » pend : {endpoint} n'est pas membre")
            }
            Self::SelfRelation { node } => write!(
                formatter,
                "{node} serait en relation avec lui-même : sous « review », un agent qui se relit, \
                 ce que §14.4 et l'invariant 11 refusent"
            ),
            Self::NotIncident { edge, node } => write!(
                formatter,
                "« {edge} » ne touche pas {node} : une scission ne partage que ses propres arêtes"
            ),
            Self::SameNode { node } => {
                write!(formatter, "{node} est donné deux fois pour deux identités")
            }
        }
    }
}

impl std::error::Error for VersionError {}
