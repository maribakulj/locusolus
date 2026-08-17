//! Le reroutage : replacer une mission dont l'hôte est tombé — §12.3, §12.2, ADR 0004.
//!
//! # Ce que §12.3 impose, mot pour mot
//!
//! « L'expiration produit `task.orphaned` ; **une tâche réattribuée conserve le numéro
//! d'attempt** ; un résultat tardif est stocké en quarantaine et ne peut committer sans
//! arbitrage. »
//!
//! La deuxième clause est la moins intuitive et la plus structurante. Un reroutage n'est pas une
//! nouvelle tentative : c'est la **même** tentative, déplacée. Incrémenter le numéro ferait croire
//! au budget qu'une seconde exécution a été demandée, ferait compter deux échecs là où il y en a
//! un, et casserait l'idempotence que `attempt.schema.json` construit autour de ce numéro.
//!
//! # Ce qui doit être exclu, et pourquoi c'est par identité
//!
//! Un hôte dont la lease a expiré reste candidat pour le placement — il annonce toujours, il a
//! toujours ses preuves. Rien dans ses capacités ne dit qu'il vient de tomber. Sans exclusion
//! explicite, le reroutage le rechoisirait, et la mission tournerait en rond sur la même machine
//! morte jusqu'à épuisement du budget.
//!
//! # Ce que ce module ne fait pas
//!
//! La quarantaine des résultats tardifs. C'est le troisième membre de §12.3, et il appartient au
//! control plane : c'est `locusd` qui décide qu'un résultat arrivé après réattribution ne committe
//! pas sans arbitrage. Le broker, lui, ne voit que le placement.

use std::collections::BTreeSet;
use std::fmt;

use locus_execution::{SandboxLevel, SandboxSpec};

use crate::admission::RefusalReason;
use crate::placement::{Candidate, Placement, place};

/// L'historique d'une tentative : son numéro, et les hôtes qui l'ont perdue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attempt {
    number: u32,
    lost: BTreeSet<String>,
}

impl Attempt {
    /// Ouvrir une tentative.
    ///
    /// # Errors
    ///
    /// [`RerouteError::ZeroAttempt`] pour le numéro zéro : `attempt.schema.json` exige un entier
    /// minimum 1, et un numéro hors du domaine du protocole ne se rattraperait qu'à la sérialisation.
    pub fn new(number: u32) -> Result<Self, RerouteError> {
        if number == 0 {
            return Err(RerouteError::ZeroAttempt);
        }
        Ok(Self {
            number,
            lost: BTreeSet::new(),
        })
    }

    /// Le numéro, invariant par reroutage.
    #[must_use]
    pub const fn number(&self) -> u32 {
        self.number
    }

    /// Constater qu'un hôte a perdu cette tentative.
    ///
    /// Idempotent : la même perte constatée deux fois — un `task.orphaned` rejoué, un heartbeat
    /// tardif — ne change rien.
    #[must_use]
    pub fn lost_on(mut self, worker: &str) -> Self {
        self.lost.insert(worker.to_owned());
        self
    }

    /// Les hôtes qui ont déjà perdu cette tentative.
    #[must_use]
    pub const fn lost(&self) -> &BTreeSet<String> {
        &self.lost
    }
}

/// Le verdict du reroutage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rerouting {
    /// La tentative repart, sur un autre hôte, **sous le même numéro**.
    Rerouted {
        /// L'hôte retenu.
        worker: String,
        /// Le niveau appliqué.
        level: SandboxLevel,
        /// Le numéro d'attempt, inchangé.
        attempt: u32,
    },
    /// Plus aucun hôte ne peut la reprendre.
    ///
    /// Les deux listes disent deux choses différentes, et les fondre ferait perdre la question à
    /// poser : `already_lost` nomme les hôtes qui ont **essayé et perdu**, `shortfalls` nomme ceux
    /// qui restaient et ce qui leur manquait. Un épuisement où la première liste est pleine et la
    /// seconde vide est une panne d'infrastructure ; l'inverse est une mission mal dimensionnée.
    Exhausted {
        /// Le numéro d'attempt, toujours inchangé.
        attempt: u32,
        /// Les hôtes qui ont déjà perdu cette tentative.
        already_lost: Vec<String>,
        /// Ce qui manquait à chacun des hôtes restants.
        shortfalls: Vec<(String, Vec<RefusalReason>)>,
    },
}

impl Rerouting {
    /// Le numéro d'attempt, quel que soit le verdict.
    ///
    /// Il est le même des deux côtés, et l'exposer par une seule fonction rend l'invariant
    /// difficile à casser sans le voir.
    #[must_use]
    pub const fn attempt(&self) -> u32 {
        match self {
            Self::Rerouted { attempt, .. } | Self::Exhausted { attempt, .. } => *attempt,
        }
    }
}

/// Replacer une tentative parmi les candidats qui ne l'ont pas déjà perdue.
///
/// # Pourquoi l'exclusion précède le placement
///
/// Écarter d'abord, choisir ensuite. L'ordre inverse — choisir puis vérifier que ce n'est pas un
/// hôte perdu — rendrait le verdict dépendant de l'ordre des candidats : un hôte perdu mais
/// « meilleur » aurait été choisi, puis rejeté, et le suivant n'aurait pas été essayé.
#[must_use]
pub fn reroute(spec: &SandboxSpec, candidates: &[Candidate], attempt: &Attempt) -> Rerouting {
    let remaining: Vec<Candidate> = candidates
        .iter()
        .filter(|candidate| !attempt.lost.contains(candidate.worker()))
        .cloned()
        .collect();

    match place(spec, &remaining) {
        Placement::Placed { worker, level } => Rerouting::Rerouted {
            worker,
            level,
            attempt: attempt.number,
        },
        Placement::Refused { shortfalls } => Rerouting::Exhausted {
            attempt: attempt.number,
            already_lost: attempt.lost.iter().cloned().collect(),
            shortfalls,
        },
    }
}

/// Ce qui empêche de suivre une tentative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RerouteError {
    /// Le numéro zéro, que le protocole n'admet pas.
    ZeroAttempt,
}

impl fmt::Display for RerouteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroAttempt => {
                formatter.write_str("un numéro d'attempt vaut au moins 1 (`attempt.schema.json`)")
            }
        }
    }
}

impl std::error::Error for RerouteError {}
