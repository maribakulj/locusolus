//! Ce que la sorte `visibility` décide — ADR 0016 décision 11, `docs/SPEC_V1.md` §16.2 et §16.3.
//!
//! # La phrase de la décision 11
//!
//! « Recâbler une relation change **qui peut lire quoi**, donc le graphe de coordination est le
//! graphe de circulation de la mémoire. » C'est ce module qui rend la phrase exécutable : il lit
//! une [`Version`] et répond à la seule question que la construction d'une `ContextView` a besoin
//! de poser — *ce destinataire peut-il voir ce que cet agent a produit ?*
//!
//! # Elle retire, elle n'ajoute jamais
//!
//! §16.3 exige que les embeddings « ne contournent pas les ACL ». Une relation de coordination ne
//! saurait pas davantage les contourner, et la garantie est plus forte si elle est **structurelle**
//! plutôt que promise : ce module ne rend qu'un `bool` que l'appelant compose avec ses propres
//! refus. Il n'existe aucun chemin par lequel une relation de visibilité fasse entrer quelque chose
//! qu'un autre filtre écarte — parce qu'il n'y a rien à faire entrer, seulement quelque chose à
//! laisser sortir.
//!
//! # Un agent voit toujours ce qu'il a produit
//!
//! Et il le voit **sans arête**, parce que [`crate::version`] refuse les auto-relations. Ce n'est
//! pas une exception glissée ici pour arranger un cas : c'est la conséquence directe d'une règle
//! posée en W15.a, et l'oublier ferait disparaître d'une vue le travail de celui qui la reçoit.
//!
//! # Ce qui n'est pas déclaré n'est pas vu
//!
//! Un élément produit par un agent vers lequel le destinataire n'a aucune relation de visibilité
//! est retiré. Le défaut permissif — « tout est visible sauf mention contraire » — ferait qu'ajouter
//! un agent à une organisation lui donnerait accès à tout, et il faudrait penser à l'en priver.
//! Personne n'y pense.
//!
//! Un élément qu'**aucun agent** n'a produit — une source externe, une saisie humaine — n'est pas
//! concerné : la visibilité est une relation entre agents, elle ne sait rien dire de ce qui n'en
//! vient pas. Le taire reviendrait à couper une vue de ses sources sous couvert d'organisation.

use std::collections::BTreeSet;

use locus_protocol::{Id, id::Agent};

use crate::proposal::RelationKind;
use crate::version::Version;

/// Qui voit le travail de qui, sous une version donnée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Visibility {
    edges: BTreeSet<(Id<Agent>, Id<Agent>)>,
}

impl Visibility {
    /// Lire les relations `visibility` d'une version.
    ///
    /// Les relations d'une autre sorte sont ignorées : une relation de revue dit qui relit qui,
    /// pas qui voit quoi. Les confondre donnerait à tout relecteur la vue de son relu, ce que §12.4
    /// et l'invariant 11 refusent précisément.
    #[must_use]
    pub fn of(version: &Version) -> Self {
        Self {
            edges: version
                .relations()
                .iter()
                .filter(|relation| relation.kind == RelationKind::Visibility)
                .map(|relation| (relation.from, relation.to))
                .collect(),
        }
    }

    /// `viewer` peut-il voir ce que `producer` a produit ?
    #[must_use]
    pub fn sees(&self, viewer: Id<Agent>, producer: Id<Agent>) -> bool {
        viewer == producer || self.edges.contains(&(viewer, producer))
    }

    /// Les agents dont `viewer` voit le travail, lui-même excepté.
    #[must_use]
    pub fn seen_by(&self, viewer: Id<Agent>) -> BTreeSet<Id<Agent>> {
        self.edges
            .iter()
            .filter(|(from, _)| *from == viewer)
            .map(|(_, to)| *to)
            .collect()
    }

    /// Le nombre de relations de visibilité déclarées.
    #[must_use]
    pub fn len(&self) -> usize {
        self.edges.len()
    }

    /// Vrai quand aucune n'est déclarée — donc quand un destinataire ne voit que son propre travail.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }
}
