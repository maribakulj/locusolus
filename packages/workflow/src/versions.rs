//! Les versions supportées et leur couverture de replay — `docs/SPEC_V1.md` §11.3.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::definition::WorkflowVersion;
use crate::kind::WorkflowKind;

/// Quelles versions de quels workflows le déploiement déclare savoir exécuter.
///
/// Le registre n'exécute rien : il porte une **affirmation**, et c'est ce qui rend les deux
/// dernières règles de §11.3 vérifiables. « Tests de replay pour les versions supportées » n'a de
/// sens que si la liste des versions supportées existe quelque part d'autre que dans une intention.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VersionRegistry {
    supported: BTreeMap<WorkflowKind, BTreeSet<WorkflowVersion>>,
}

impl VersionRegistry {
    /// Un registre vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Déclarer une version supportée.
    #[must_use]
    pub fn support(mut self, kind: WorkflowKind, version: WorkflowVersion) -> Self {
        self.supported.entry(kind).or_default().insert(version);
        self
    }

    /// Les versions supportées pour un workflow, de la plus ancienne à la plus récente.
    #[must_use]
    pub fn supported(&self, kind: WorkflowKind) -> Vec<WorkflowVersion> {
        self.supported
            .get(&kind)
            .map(|versions| versions.iter().copied().collect())
            .unwrap_or_default()
    }

    /// La version courante d'un workflow — la plus récente que le registre déclare.
    #[must_use]
    pub fn current(&self, kind: WorkflowKind) -> Option<WorkflowVersion> {
        self.supported.get(&kind)?.iter().next_back().copied()
    }

    /// Les workflows de §11.2 pour lesquels le registre ne déclare aucune version.
    ///
    /// Un déploiement qui n'en supporte que dix n'est pas neuf dixièmes conforme : le onzième
    /// manquant est un workflow que la spécification exige et que rien n'exécute. La liste le dit
    /// plutôt que de laisser l'absence se confondre avec le silence.
    #[must_use]
    pub fn unsupported_kinds(&self) -> Vec<WorkflowKind> {
        WorkflowKind::ALL
            .into_iter()
            .filter(|kind| self.supported(*kind).is_empty())
            .collect()
    }

    /// Retirer une version du support.
    ///
    /// §11.3, dernière règle : « migrations contrôlées des workflows longue durée ». Deux retraits
    /// sont refusés ici — celui de la version courante, et celui de la dernière restante. Les deux
    /// laisseraient des exécutions en cours pointer vers une forme que plus personne ne revendique,
    /// et une exécution longue durée ne se replie pas parce qu'on a retiré sa version.
    ///
    /// Ce refus est **structurel, pas informé** : le registre ne sait pas ce qui tourne. W3.b, qui
    /// aura un moteur, saura le demander ; jusque-là, refuser les deux retraits certainement
    /// dangereux vaut mieux que de tous les autoriser.
    ///
    /// # Errors
    ///
    /// [`RetirementError`] selon le cas.
    pub fn retire(
        &mut self,
        kind: WorkflowKind,
        version: WorkflowVersion,
    ) -> Result<(), RetirementError> {
        let versions = self
            .supported
            .get_mut(&kind)
            .ok_or(RetirementError::NotSupported { kind, version })?;
        if !versions.contains(&version) {
            return Err(RetirementError::NotSupported { kind, version });
        }
        if versions.len() == 1 {
            return Err(RetirementError::LastRemaining { kind, version });
        }
        if versions.iter().next_back() == Some(&version) {
            return Err(RetirementError::Current { kind, version });
        }
        versions.remove(&version);
        Ok(())
    }
}

/// Pourquoi un retrait de version est refusé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetirementError {
    /// La version n'était pas supportée.
    NotSupported {
        /// Le workflow.
        kind: WorkflowKind,
        /// La version.
        version: WorkflowVersion,
    },
    /// C'est la version courante : la retirer laisserait les exécutions en cours sans forme.
    Current {
        /// Le workflow.
        kind: WorkflowKind,
        /// La version.
        version: WorkflowVersion,
    },
    /// C'est la dernière : le workflow ne serait plus supporté du tout.
    LastRemaining {
        /// Le workflow.
        kind: WorkflowKind,
        /// La version.
        version: WorkflowVersion,
    },
}

impl fmt::Display for RetirementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSupported { kind, version } => {
                write!(formatter, "{kind} {version} n'était pas supportée")
            }
            Self::Current { kind, version } => write!(
                formatter,
                "{kind} {version} est la version courante : la retirer laisserait les exécutions en cours sans forme déclarée"
            ),
            Self::LastRemaining { kind, version } => write!(
                formatter,
                "{kind} {version} est la dernière supportée : la retirer retirerait le workflow"
            ),
        }
    }
}

impl std::error::Error for RetirementError {}

/// Ce qui manque, ou ce qui est de trop, dans les tests de replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverageFinding {
    /// Une version déclarée supportée que rejoue aucun test.
    Untested {
        /// Le workflow.
        kind: WorkflowKind,
        /// La version.
        version: WorkflowVersion,
    },
    /// Un test de replay pour une version que plus personne ne supporte.
    ///
    /// Ce n'est pas une faute, mais ce n'est pas rien : le test passe, il compte dans le total, et
    /// il rejoue une forme que le déploiement ne revendique plus. Un décompte qui l'ignorerait
    /// laisserait croire à une couverture qu'il n'a pas mesurée.
    Stray {
        /// Le workflow.
        kind: WorkflowKind,
        /// La version.
        version: WorkflowVersion,
    },
}

impl fmt::Display for CoverageFinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Untested { kind, version } => write!(
                formatter,
                "{kind} {version} est supportée mais aucun test ne la rejoue"
            ),
            Self::Stray { kind, version } => write!(
                formatter,
                "un test rejoue {kind} {version}, que le registre ne supporte plus"
            ),
        }
    }
}

/// Confronter les versions supportées aux versions réellement rejouées.
///
/// §11.3 : « tests de replay pour les versions supportées ». La règle porte sur une correspondance,
/// pas sur un nombre : elle est tenue quand les deux listes coïncident, et un total flatteur ne
/// dit rien de celle qui manque.
#[must_use]
pub fn replay_coverage(
    registry: &VersionRegistry,
    tested: &[(WorkflowKind, WorkflowVersion)],
) -> Vec<CoverageFinding> {
    let replayed: BTreeSet<(WorkflowKind, WorkflowVersion)> = tested.iter().copied().collect();
    let mut findings = Vec::new();

    for kind in WorkflowKind::ALL {
        for version in registry.supported(kind) {
            if !replayed.contains(&(kind, version)) {
                findings.push(CoverageFinding::Untested { kind, version });
            }
        }
    }
    for (kind, version) in replayed {
        if !registry.supported(kind).contains(&version) {
            findings.push(CoverageFinding::Stray { kind, version });
        }
    }
    findings
}
