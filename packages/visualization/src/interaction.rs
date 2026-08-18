//! L'interaction cockpit ↔ viewer — `docs/SPEC_V1.md` §23 et `docs/07`.
//!
//! # Ce que le texte accorde, et ce qu'il retire
//!
//! « IDs stables. Emacs peut envoyer `focus`, `filter`, `select` ; le viewer renvoie
//! `node_selected`, `artifact_opened`, etc. **Toute mutation passe ensuite par command API et
//! confirmation appropriée.** »
//!
//! Trois commandes vers le viewer, deux événements en retour, et une phrase qui ferme le canal :
//! rien de ce qui revient d'un viewer n'écrit quoi que ce soit.
//!
//! # Un événement porte une identité, jamais un contenu
//!
//! C'est par là que « la vue devient éditable en place » reviendrait si on la chassait de la vue :
//! non pas en modifiant la projection, mais en laissant le viewer **dire** au control plane ce
//! qu'un nœud vaut désormais. Un `node_selected` qui porterait un label remplacerait une lecture
//! par une écriture sans jamais toucher au graphe directement.
//!
//! [`ViewerEvent`] ne porte donc que des identités. Ce n'est pas une convention de sérialisation :
//! il n'y a aucun champ où mettre autre chose, et un consommateur qui voudrait faire écrire un
//! viewer devrait d'abord changer le type ici — c'est-à-dire dans un fichier qui explique pourquoi
//! il ne faut pas.
//!
//! # Une vue dérivée dit toujours d'où elle vient
//!
//! `focus` et `filter` produisent une **autre** vue, plus petite. Le danger n'est pas qu'elle
//! existe, c'est qu'on la prenne pour la projection : un lecteur qui compte les objections d'un
//! claim dans une vue filtrée en compte moins, et rien à l'écran ne le lui dit. La forme canonique
//! d'une vue dérivée porte donc le condensat de son parent, sans exception — y compris quand le
//! filtre ne retire rien, parce que les exceptions sont précisément là où la confusion se loge.

use std::collections::BTreeSet;
use std::fmt;

use crate::{Digest, View, ViewError};

/// Ce que le cockpit envoie au viewer — les trois de §23.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewerCommand {
    /// Centrer sur un nœud, avec son voisinage à `depth` sauts.
    Focus {
        /// Le nœud visé.
        node: String,
        /// Combien de sauts autour de lui.
        depth: usize,
    },
    /// Ne garder que certaines sortes de nœuds.
    Filter {
        /// Les sortes gardées.
        node_kinds: Vec<String>,
    },
    /// Surligner une sélection.
    ///
    /// Ne change pas ce que la vue contient : sélectionner n'est pas filtrer, et confondre les deux
    /// ferait disparaître de l'écran ce qu'on voulait seulement désigner.
    Select {
        /// Les nœuds désignés.
        nodes: Vec<String>,
    },
}

impl ViewerCommand {
    /// Son nom sur le fil.
    #[must_use]
    pub const fn slug(&self) -> &'static str {
        match self {
            Self::Focus { .. } => "focus",
            Self::Filter { .. } => "filter",
            Self::Select { .. } => "select",
        }
    }
}

/// Ce que le viewer renvoie — les deux que `docs/07` nomme.
///
/// Deux, et pas « etc. » : une sorte d'événement entre dans cette énumération quand un
/// consommateur exécutable et testé existe. Une variante que personne ne produit est une promesse
/// que le code ne tient pas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewerEvent {
    /// Un nœud a été sélectionné.
    NodeSelected {
        /// Lequel — son identité, et rien d'autre.
        node: String,
    },
    /// Un artefact a été ouvert.
    ArtifactOpened {
        /// Lequel — son identité, et rien d'autre.
        artifact: String,
    },
}

impl ViewerEvent {
    /// Relire un événement.
    ///
    /// # Errors
    ///
    /// [`InteractionError::UnknownEvent`] pour un nom que `docs/07` ne donne pas, et
    /// [`InteractionError::EmptyField`] pour une identité vide. Un événement sans sujet ne dit rien
    /// et encombrerait un journal de sélections vides.
    pub fn from_wire(name: &str, subject: &str) -> Result<Self, InteractionError> {
        if subject.trim().is_empty() {
            return Err(InteractionError::EmptyField { field: "subject" });
        }
        match name {
            "node_selected" => Ok(Self::NodeSelected {
                node: subject.to_owned(),
            }),
            "artifact_opened" => Ok(Self::ArtifactOpened {
                artifact: subject.to_owned(),
            }),
            other => Err(InteractionError::UnknownEvent {
                name: other.to_owned(),
            }),
        }
    }

    /// Son nom sur le fil.
    #[must_use]
    pub const fn slug(&self) -> &'static str {
        match self {
            Self::NodeSelected { .. } => "node_selected",
            Self::ArtifactOpened { .. } => "artifact_opened",
        }
    }

    /// Ce que l'événement désigne — une identité.
    ///
    /// Il n'existe pas d'accesseur au « contenu » d'un événement, parce qu'il n'y a pas de contenu.
    /// C'est ainsi que « toute mutation passe par command API » tient ici : ce canal ne transporte
    /// rien qui puisse être écrit quelque part.
    #[must_use]
    pub fn subject(&self) -> &str {
        match self {
            Self::NodeSelected { node } => node,
            Self::ArtifactOpened { artifact } => artifact,
        }
    }
}

impl View {
    /// La vue centrée sur `node`, avec son voisinage à `depth` sauts.
    ///
    /// Rend une **autre** vue, qui déclare son parent : une vue centrée montre moins, et ce qui
    /// manque doit être imputable au cadrage, pas au graphe.
    ///
    /// # Errors
    ///
    /// [`ViewError::DanglingEdge`] ne peut pas se produire — les arêtes dont une extrémité sort du
    /// cadrage sont retirées, jamais laissées à pointer vers rien. Les autres refus de
    /// [`View::render`] restent possibles.
    pub fn focused(
        &self,
        node: &str,
        depth: usize,
        digest: &dyn Digest,
    ) -> Result<Self, ViewError> {
        let mut kept: BTreeSet<&str> = BTreeSet::new();
        if self.nodes().iter().any(|candidate| candidate.id == node) {
            kept.insert(node);
        }
        for _ in 0..depth {
            let voisins: Vec<&str> = self
                .edges()
                .iter()
                .flat_map(|edge| {
                    let mut trouves = Vec::new();
                    if kept.contains(edge.from.as_str()) {
                        trouves.push(edge.to.as_str());
                    }
                    if kept.contains(edge.to.as_str()) {
                        trouves.push(edge.from.as_str());
                    }
                    trouves
                })
                .collect();
            kept.extend(voisins);
        }
        self.restricted(&kept, digest)
    }

    /// La vue réduite aux nœuds dont la sorte est dans `node_kinds`.
    ///
    /// # Errors
    ///
    /// Ce que [`View::render`] refuse.
    pub fn filtered(&self, node_kinds: &[String], digest: &dyn Digest) -> Result<Self, ViewError> {
        let kept: BTreeSet<&str> = self
            .nodes()
            .iter()
            .filter(|node| node_kinds.contains(&node.kind))
            .map(|node| node.id.as_str())
            .collect();
        self.restricted(&kept, digest)
    }

    fn restricted(&self, kept: &BTreeSet<&str>, digest: &dyn Digest) -> Result<Self, ViewError> {
        let nodes = self
            .nodes()
            .iter()
            .filter(|node| kept.contains(node.id.as_str()))
            .cloned()
            .collect();
        // Une arête dont une extrémité est sortie du cadrage est **retirée**. La garder ferait
        // supposer un nœud absent — et §23 dit qu'un viewer montre, il n'infère pas.
        let edges = self
            .edges()
            .iter()
            .filter(|edge| kept.contains(edge.from.as_str()) && kept.contains(edge.to.as_str()))
            .cloned()
            .collect();
        Self::render_derived(self, nodes, edges, digest)
    }
}

/// Ce qui empêche une interaction d'être lue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteractionError {
    /// Un champ obligatoire vide.
    EmptyField {
        /// Lequel.
        field: &'static str,
    },
    /// Un événement que `docs/07` ne nomme pas.
    UnknownEvent {
        /// Le nom reçu.
        name: String,
    },
}

impl fmt::Display for InteractionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(formatter, "« {field} » est vide"),
            Self::UnknownEvent { name } => write!(
                formatter,
                "« {name} » n'est pas un événement de viewer : une sorte n'entre dans \
                 l'énumération que lorsqu'un consommateur testé existe"
            ),
        }
    }
}

impl std::error::Error for InteractionError {}
