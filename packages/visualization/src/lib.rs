//! Le service de projection de visualisation — `docs/SPEC_V1.md` §23.3.
//!
//! # La phrase qui décide de la forme du crate
//!
//! « Le graphe canonique n'est **jamais** envoyé brut à un viewer. Le service produit des
//! projections versionnées et hashées. »
//!
//! Une vue n'est donc pas un raccourci vers le graphe, c'est un instantané : elle porte le
//! watermark auquel elle a été prise, une forme canonique qui la détermine entièrement, et rien qui
//! permette de remonter. Il n'existe dans ce paquet aucun chemin d'écriture — pas parce qu'on a
//! choisi de ne pas en écrire, mais parce qu'une [`View`] ne détient rien d'autre que des données.
//!
//! # « Jamais une copie mutable du graphe »
//!
//! `docs/10` : « si une vue devient éditable en place, l'invariant *aucun frontend n'écrit
//! directement dans le graphe* est perdu. » Le crate le tient d'une façon qui se teste : une vue
//! dont on a changé un nœud n'a plus la même forme canonique, donc plus le même condensat, donc
//! elle ne peut pas être présentée comme la projection dont elle vient. Éditer une vue est
//! possible — c'est de la donnée — mais le résultat cesse d'être cette vue, et c'est exactement ce
//! qu'on veut : le frontend peut travailler, il ne peut pas se faire passer pour la source.
//!
//! # Le condensat est un port
//!
//! Le crate ne calcule aucun hash. Il produit la **forme canonique** — l'ordre y est fixé, pas
//! hérité de l'ordre d'insertion — et confie le condensat à [`Digest`]. Deux viewers qui affichent
//! « la même vue » doivent le prouver en comparant deux chaînes construites de la même façon ; s'ils
//! comparaient l'ordre dans lequel un producteur a rempli un vecteur, ils différeraient sans que
//! rien n'ait changé.

pub mod registry;

pub use registry::{ArtifactViewerRegistry, Choice, RegistryError, Viewer, ViewerRequest};

use std::collections::BTreeSet;
use std::fmt;
use std::fmt::Write as _;

use locus_domain::ContentHash;
use locus_projections::Watermark;

/// Les huit projections que §23.3 nomme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ViewKind {
    /// Le graphe 2D.
    Graph2d,
    /// La carte d'argumentation.
    ArgumentMap,
    /// La provenance.
    Provenance,
    /// Les dépendances.
    Dependencies,
    /// Les désaccords.
    Disagreements,
    /// L'espace sémantique.
    SemanticSpace,
    /// Le paysage de branches.
    BranchLandscape,
    /// La société d'agents.
    AgentSociety,
}

impl ViewKind {
    /// Les huit de §23.3, dans l'ordre où le texte les nomme.
    pub const ALL: [Self; 8] = [
        Self::Graph2d,
        Self::ArgumentMap,
        Self::Provenance,
        Self::Dependencies,
        Self::Disagreements,
        Self::SemanticSpace,
        Self::BranchLandscape,
        Self::AgentSociety,
    ];

    /// Son nom sur le fil.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Graph2d => "graph_2d",
            Self::ArgumentMap => "argument_map",
            Self::Provenance => "provenance",
            Self::Dependencies => "dependencies",
            Self::Disagreements => "disagreements",
            Self::SemanticSpace => "semantic_space",
            Self::BranchLandscape => "branch_landscape",
            Self::AgentSociety => "agent_society",
        }
    }
}

impl fmt::Display for ViewKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Ce qui calcule le condensat d'une forme canonique — un port.
///
/// Le crate ne dépend d'aucune implémentation de hachage : ce qu'il garantit est que deux contenus
/// égaux produisent la **même** forme canonique. Ce qu'on en fait ensuite appartient à l'appelant,
/// et le jour où l'algorithme change, rien ici ne bouge.
pub trait Digest {
    /// Le condensat de `canonical`.
    fn digest(&self, canonical: &str) -> ContentHash;
}

/// Un nœud de la vue.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ViewNode {
    /// L'identité stable — §23 : « IDs stables », parce qu'une sélection doit désigner la même
    /// chose d'un rendu à l'autre.
    pub id: String,
    /// Ce que le nœud est.
    pub kind: String,
    /// Ce qu'on en affiche.
    pub label: String,
}

/// Une arête de la vue.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ViewEdge {
    /// D'où.
    pub from: String,
    /// Vers où.
    pub to: String,
    /// Le type de relation.
    pub kind: String,
}

/// Une projection de visualisation, versionnée et hashée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    kind: ViewKind,
    watermark: Watermark,
    nodes: Vec<ViewNode>,
    edges: Vec<ViewEdge>,
    canonical: String,
    digest: ContentHash,
}

impl View {
    /// Rendre une vue de `kind` à `watermark`.
    ///
    /// L'ordre des entrées n'a aucune importance : la forme canonique trie. C'est ce qui permet à
    /// deux producteurs — un rebuild complet, un rattrapage incrémental — de prouver qu'ils
    /// montrent la même chose.
    ///
    /// # Errors
    ///
    /// [`ViewError::EmptyField`] pour une identité vide, [`ViewError::DuplicateNode`] pour deux
    /// nœuds de même identité — une sélection ne saurait plus lequel elle désigne — et
    /// [`ViewError::DanglingEdge`] pour une arête dont une extrémité n'est pas dans la vue : une
    /// arête qui pointe vers rien invite le lecteur à supposer un nœud que le graphe n'a pas.
    pub fn render(
        kind: ViewKind,
        watermark: Watermark,
        nodes: Vec<ViewNode>,
        edges: Vec<ViewEdge>,
        digest: &dyn Digest,
    ) -> Result<Self, ViewError> {
        let mut nodes = nodes;
        let mut edges = edges;
        for node in &nodes {
            if node.id.trim().is_empty() {
                return Err(ViewError::EmptyField { field: "node.id" });
            }
        }
        nodes.sort();
        let identities: BTreeSet<&str> = nodes.iter().map(|node| node.id.as_str()).collect();
        if identities.len() != nodes.len() {
            return Err(ViewError::DuplicateNode);
        }
        for edge in &edges {
            for end in [&edge.from, &edge.to] {
                if !identities.contains(end.as_str()) {
                    return Err(ViewError::DanglingEdge {
                        endpoint: end.clone(),
                    });
                }
            }
        }
        edges.sort();
        edges.dedup();

        let canonical = canonicalise(kind, watermark, &nodes, &edges);
        let digest = digest.digest(&canonical);
        Ok(Self {
            kind,
            watermark,
            nodes,
            edges,
            canonical,
            digest,
        })
    }

    /// Ce que la vue montre.
    #[must_use]
    pub const fn kind(&self) -> ViewKind {
        self.kind
    }

    /// Le point du journal auquel elle a été prise.
    #[must_use]
    pub const fn watermark(&self) -> Watermark {
        self.watermark
    }

    /// Ses nœuds, dans l'ordre canonique.
    #[must_use]
    pub fn nodes(&self) -> &[ViewNode] {
        &self.nodes
    }

    /// Ses arêtes, dans l'ordre canonique.
    #[must_use]
    pub fn edges(&self) -> &[ViewEdge] {
        &self.edges
    }

    /// La forme canonique — ce qui a été condensé.
    ///
    /// Exposée parce qu'une vue qui prétend être hashée doit pouvoir dire **quoi** : sans cela,
    /// deux implémentations qui divergent ne peuvent que constater que leurs condensats diffèrent.
    #[must_use]
    pub fn canonical(&self) -> &str {
        &self.canonical
    }

    /// Son condensat.
    #[must_use]
    pub const fn digest(&self) -> &ContentHash {
        &self.digest
    }

    /// Où en est cette vue par rapport au journal en `now`.
    ///
    /// Ne rend jamais [`Freshness::Current`] par défaut : une vue en retard qui se dirait à jour
    /// est exactement la panne qu'un lecteur ne peut pas voir.
    #[must_use]
    pub const fn freshness(&self, now: Watermark) -> Freshness {
        if now == self.watermark {
            Freshness::Current
        } else if now > self.watermark {
            Freshness::Behind {
                by: now - self.watermark,
            }
        } else {
            // Le journal ne recule pas : c'est le point de comparaison qui est périmé. Le dire est
            // le seul moyen que l'appelant s'en aperçoive — répondre `Current` ferait passer sa
            // méprise pour un accord.
            Freshness::Inconsistent
        }
    }
}

fn canonicalise(
    kind: ViewKind,
    watermark: Watermark,
    nodes: &[ViewNode],
    edges: &[ViewEdge],
) -> String {
    let mut canonical = format!("view/1\n{kind}\n{watermark}\n");
    for node in nodes {
        // `write!` sur une `String` ne peut pas échouer ; l'ignorer explicitement est ce que la
        // signature de `fmt::Write` impose, et clippy refuse le `push_str(&format!(..))` qui
        // rallouerait à chaque ligne.
        let _ = writeln!(canonical, "n\t{}\t{}\t{}", node.id, node.kind, node.label);
    }
    for edge in edges {
        let _ = writeln!(canonical, "e\t{}\t{}\t{}", edge.from, edge.to, edge.kind);
    }
    canonical
}

/// Où en est une vue par rapport au journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    /// Elle est au point demandé.
    Current,
    /// Le journal a avancé depuis.
    Behind {
        /// De combien de positions.
        by: u64,
    },
    /// Le point de comparaison est antérieur à la vue : quelqu'un compare à un état périmé.
    Inconsistent,
}

/// Ce qui empêche une vue d'exister.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewError {
    /// Un champ obligatoire vide.
    EmptyField {
        /// Lequel.
        field: &'static str,
    },
    /// Deux nœuds portent la même identité.
    DuplicateNode,
    /// Une arête pointe vers un nœud qui n'est pas dans la vue.
    DanglingEdge {
        /// L'extrémité introuvable.
        endpoint: String,
    },
}

impl fmt::Display for ViewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(formatter, "« {field} » est vide"),
            Self::DuplicateNode => formatter.write_str(
                "deux nœuds de même identité : une sélection ne saurait plus lequel elle désigne",
            ),
            Self::DanglingEdge { endpoint } => write!(
                formatter,
                "l'arête mène à « {endpoint} », qui n'est pas dans la vue — le lecteur en \
                 déduirait un nœud que le graphe n'a pas"
            ),
        }
    }
}

impl std::error::Error for ViewError {}
