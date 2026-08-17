//! Le cycle de vie d'un objet épistémique — `docs/SPEC_V1.md` §7.4.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Les dix statuts de §7.4, dans l'ordre du texte.
///
/// `status` décrit **le cycle de vie**, et rien d'autre. La phrase qui suit la liste dans la spec
/// est la contrainte de conception la plus forte de ce module : « `validation_level` décrit la
/// force épistémique et ne doit pas être déduit du seul statut ». Il n'existe donc dans ce crate
/// **aucune** conversion d'un statut vers un niveau, et un test vérifie qu'elle n'existe pas.
///
/// La raison est simple à énoncer et coûteuse à oublier : un objet peut être `validated` au sens
/// du processus — il a traversé les étapes — sans qu'aucune preuve indépendante n'ait été
/// produite. Déduire `L3` de `validated` transformerait une décision de procédure en constat
/// scientifique.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Brouillon local, non soumis.
    Draft,
    /// Soumis, en attente de traitement institutionnel. C'est le plafond d'un worker (Canterel §2.3).
    Staged,
    /// En cours de revue.
    UnderReview,
    /// Contesté : une objection est ouverte et non résolue.
    Contested,
    /// Validé au sens du processus. **Pas** au sens de la force épistémique.
    Validated,
    /// Réfuté.
    Refuted,
    /// Remplacé par une révision ultérieure.
    Superseded,
    /// Retiré par son auteur.
    Withdrawn,
    /// Mis de côté, avec raison, sans être supprimé (§19.5 de la spec Canterel, §24.5 ici).
    Quarantined,
    /// Archivé.
    Archived,
}

impl Status {
    /// Tous les statuts, dans l'ordre du texte.
    pub const ALL: [Self; 10] = [
        Self::Draft,
        Self::Staged,
        Self::UnderReview,
        Self::Contested,
        Self::Validated,
        Self::Refuted,
        Self::Superseded,
        Self::Withdrawn,
        Self::Quarantined,
        Self::Archived,
    ];

    /// La forme textuelle canonique, celle du fil.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Staged => "staged",
            Self::UnderReview => "under_review",
            Self::Contested => "contested",
            Self::Validated => "validated",
            Self::Refuted => "refuted",
            Self::Superseded => "superseded",
            Self::Withdrawn => "withdrawn",
            Self::Quarantined => "quarantined",
            Self::Archived => "archived",
        }
    }

    /// Ce qu'un worker a le droit d'écrire — Canterel §2.3.
    ///
    /// « Canterel NE DOIT PAS promouvoir un claim au-delà de `staged`. » La règle vit des deux
    /// côtés du fil : le worker refuse d'écrire au-delà, et le domaine sait lesquels sont
    /// proposables. La redondance est voulue — un domaine qui accepterait `validated` d'un worker
    /// ferait dépendre l'invariant 3 de la bonne foi du client.
    #[must_use]
    pub const fn is_worker_proposable(self) -> bool {
        matches!(self, Self::Draft | Self::Staged)
    }
}

impl fmt::Display for Status {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
