//! Lignée et supersession — `docs/SPEC_V1.md` §7.7.

use serde::{Deserialize, Serialize};

use crate::ids::RevisionId;

/// D'où vient une révision.
///
/// §7.7 pose deux phrases qui se contredisent en apparence :
///
/// - « une révision possède **au plus un** prédécesseur direct dans sa lignée » ;
/// - « un merge peut créer une révision avec **plusieurs parents déclarés** ».
///
/// Elles ne parlent pas de la même chose. La lignée est une chaîne : c'est elle qui donne un sens
/// à « la version précédente de cet objet ». Les parents déclarés d'un merge sont une information
/// de provenance : ce que la fusion a incorporé. Modéliser les deux par un même `Vec<RevisionId>`
/// perdrait la distinction, et « la version précédente » deviendrait une question sans réponse dès
/// le premier merge.
///
/// D'où cette énumération : le prédécesseur de lignée est unique **par construction**, y compris
/// dans le cas du merge, et les parents supplémentaires vivent à côté sans pouvoir s'y substituer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Lineage {
    /// La première révision de cet objet. Aucun prédécesseur.
    Root,
    /// Une révision ordinaire : exactement un prédécesseur.
    Successor {
        /// La révision remplacée — le `supersedes_revision_id` de §7.4.
        supersedes: RevisionId,
    },
    /// Une fusion : un prédécesseur de lignée, et des parents incorporés.
    Merge {
        /// Le prédécesseur de lignée. Unique, comme partout ailleurs.
        supersedes: RevisionId,
        /// Les autres parents déclarés. Peut être vide — un merge sans apport reste un merge.
        incorporates: Vec<RevisionId>,
    },
}

impl Lineage {
    /// Le prédécesseur direct de lignée, s'il existe. **Au plus un**, par construction.
    #[must_use]
    pub const fn supersedes(&self) -> Option<&RevisionId> {
        match self {
            Self::Root => None,
            Self::Successor { supersedes } | Self::Merge { supersedes, .. } => Some(supersedes),
        }
    }

    /// Les parents incorporés par une fusion. Vide hors du cas `Merge`.
    #[must_use]
    pub fn incorporates(&self) -> &[RevisionId] {
        match self {
            Self::Root | Self::Successor { .. } => &[],
            Self::Merge { incorporates, .. } => incorporates,
        }
    }

    /// Tous les parents déclarés, lignée comprise, sans doublon d'ordre.
    ///
    /// Existe pour la provenance et pour rien d'autre : c'est une lecture, pas une identité. Ce
    /// qui répond à « quelle était la version précédente » reste [`Lineage::supersedes`].
    #[must_use]
    pub fn declared_parents(&self) -> Vec<RevisionId> {
        let mut parents = Vec::new();
        if let Some(direct) = self.supersedes() {
            parents.push(*direct);
        }
        parents.extend(self.incorporates().iter().copied());
        parents
    }
}
