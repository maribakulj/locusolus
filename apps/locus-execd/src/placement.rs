//! Le placement : choisir un hôte, ou dire pourquoi aucun ne convient — §12.2, ADR 0004.
//!
//! # Ce que ce module ajoute à l'admission
//!
//! [`crate::admission::admit`] répond à « **cet** hôte peut-il ? ». Le placement répond à « lequel,
//! parmi ceux-ci ? », et les deux questions n'ont pas la même réponse quand aucun ne convient : un
//! refus d'admission nomme ce qui manque à un hôte, un refus de placement doit nommer ce qui
//! manquait à **chacun**. Sans cela, un opérateur corrige un hôte, réessaie, et découvre que le
//! suivant manquait d'autre chose — un aller-retour par candidat.
//!
//! # La confiance ne se déclare pas, elle se prouve
//!
//! §12.2 place parmi les critères « sandbox disponible **et attestée** ». Le mot « attestée » était
//! resté sans consommateur : `HostCapabilities` annonçait un niveau, et rien ne demandait à l'hôte
//! de l'avoir tenu. W4.d.3 a produit le juge — la suite de self-tests rend un [`Standing`] — et ce
//! module le branche : **un candidat sans `Trusted` au niveau exigé n'est pas placé**, quoi qu'il
//! annonce.
//!
//! Le cas limite est celui qui compte. Un hôte qui n'a jamais passé la suite n'est pas un hôte dont
//! on ignore la valeur : c'est un hôte dont on n'a **aucune preuve**, et `denies_trust` a déjà
//! tranché que l'absence de preuve n'est pas une preuve. Ce module ne fait que refuser de placer
//! dessus.

use std::fmt;

use locus_execution::{SandboxLevel, SandboxSpec, Standing};

use crate::admission::{Admission, HostCapabilities, RefusalReason, admit};

/// Un hôte candidat, avec ce qu'il annonce et ce qu'il a prouvé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    worker: String,
    capabilities: HostCapabilities,
    attested: Vec<Standing>,
}

impl Candidate {
    /// Déclarer un candidat qui n'a rien prouvé.
    ///
    /// C'est l'état d'un worker qui vient de s'enrôler : il annonce, la suite n'a pas encore
    /// tourné. Il ne recevra rien au-dessus de `S0` avant d'avoir passé la suite, et c'est le
    /// comportement voulu — pas une lacune à combler par un défaut permissif.
    #[must_use]
    pub fn new(worker: &str, capabilities: HostCapabilities) -> Self {
        Self {
            worker: worker.to_owned(),
            capabilities,
            attested: Vec::new(),
        }
    }

    /// Joindre le verdict d'une campagne de self-tests.
    #[must_use]
    pub fn attested(mut self, standing: Standing) -> Self {
        self.attested.push(standing);
        self
    }

    /// Son identifiant.
    #[must_use]
    pub fn worker(&self) -> &str {
        &self.worker
    }

    /// Ce qu'il annonce.
    #[must_use]
    pub const fn capabilities(&self) -> &HostCapabilities {
        &self.capabilities
    }

    /// Le niveau le plus élevé auquel il a été jugé `Trusted`.
    ///
    /// `None` quand aucune campagne n'a conclu — ce qui n'est pas la même chose qu'une campagne
    /// ayant conclu `S0`.
    #[must_use]
    pub fn proven_level(&self) -> Option<SandboxLevel> {
        self.attested
            .iter()
            .filter_map(|standing| match standing {
                Standing::Trusted { level } => Some(*level),
                Standing::NotTrusted { .. } => None,
            })
            .max()
    }

    /// Ce qui manque à ce candidat pour cette mission.
    ///
    /// Vide quand il convient. L'attestation est vérifiée **en plus** de l'admission, jamais à sa
    /// place : un hôte peut avoir prouvé `S3` et manquer de mémoire.
    #[must_use]
    pub fn shortfall(&self, spec: &SandboxSpec) -> Vec<RefusalReason> {
        let mut reasons = match admit(spec, &self.capabilities) {
            Admission::Admitted { .. } => Vec::new(),
            Admission::Refused { reasons } => reasons,
        };
        let required = spec.minimum_level();
        if required > SandboxLevel::S0 && !self.proves(required) {
            reasons.push(RefusalReason::LevelNotAttested {
                required,
                proven: self.proven_level(),
            });
        }
        reasons
    }

    fn proves(&self, required: SandboxLevel) -> bool {
        self.proven_level()
            .is_some_and(|proven| proven.satisfies(required))
    }
}

/// Le verdict du placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placement {
    /// La mission ira sur cet hôte, à ce niveau.
    Placed {
        /// L'hôte retenu.
        worker: String,
        /// Le niveau qui sera appliqué — celui qu'exige la mission.
        level: SandboxLevel,
    },
    /// Aucun candidat ne convient, et voici ce qui manquait à chacun.
    ///
    /// La liste porte **tous** les candidats examinés, y compris ceux qui ne manquaient que d'une
    /// chose. Ne garder que « le plus proche » ferait corriger un hôte pour découvrir ensuite que
    /// les autres manquaient d'autre chose.
    Refused {
        /// Un couple par candidat, dans l'ordre où ils ont été proposés.
        shortfalls: Vec<(String, Vec<RefusalReason>)>,
    },
}

/// Placer une mission parmi des candidats.
///
/// # La règle de choix, et pourquoi celle-là
///
/// Parmi les candidats qui conviennent, le retenu est celui dont le **plafond prouvé est le plus
/// bas**. Un `S3` consommé par une mission `S1` est un `S3` indisponible pour la mission qui en
/// avait besoin, et c'est le genre de gaspillage qui ne se voit qu'au moment où il coûte. À plafond
/// égal, l'ordre est celui de l'identifiant : le placement doit être **reproductible**, sans quoi
/// deux rejeux du même journal placeraient différemment et la trace ne dirait plus ce qui s'est
/// passé.
#[must_use]
pub fn place(spec: &SandboxSpec, candidates: &[Candidate]) -> Placement {
    let mut shortfalls = Vec::new();
    let mut eligible: Vec<&Candidate> = Vec::new();

    for candidate in candidates {
        let missing = candidate.shortfall(spec);
        if missing.is_empty() {
            eligible.push(candidate);
        } else {
            shortfalls.push((candidate.worker.clone(), missing));
        }
    }

    eligible.sort_by(|left, right| {
        left.proven_level()
            .cmp(&right.proven_level())
            .then_with(|| left.worker.cmp(&right.worker))
    });

    eligible
        .first()
        .map_or(Placement::Refused { shortfalls }, |chosen| {
            Placement::Placed {
                worker: chosen.worker.clone(),
                level: spec.minimum_level(),
            }
        })
}

impl fmt::Display for Placement {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Placed { worker, level } => {
                write!(formatter, "« {worker} » en {}", level.code())
            }
            Self::Refused { shortfalls } if shortfalls.is_empty() => {
                formatter.write_str("aucun candidat n'a été proposé")
            }
            Self::Refused { shortfalls } => {
                write!(formatter, "aucun des {} candidats", shortfalls.len())
            }
        }
    }
}
