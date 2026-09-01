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
use crate::announced::Attested;
use crate::mechanism::{Employment, employment};

/// Un hôte candidat, avec ce qu'il annonce et ce qu'il a prouvé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    worker: String,
    capabilities: HostCapabilities,
    attested: Vec<Attested>,
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

    /// Joindre le verdict d'une campagne de self-tests, **avec le mécanisme sous lequel elle a
    /// conclu**.
    ///
    /// Le mécanisme n'est pas un ornement : sans lui, la décision 3 de l'ADR 0035 n'a rien à
    /// comparer. C'est [`Attested`] qui les tient ensemble, précisément pour qu'un appelant ne
    /// puisse pas joindre l'un en oubliant l'autre.
    #[must_use]
    pub fn attested(mut self, attested: Attested) -> Self {
        self.attested.push(attested);
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

    /// Le niveau le plus élevé auquel il a été jugé `Trusted` **sous un mécanisme qu'il emploie**.
    ///
    /// `None` quand aucune campagne n'a conclu — ce qui n'est pas la même chose qu'une campagne
    /// ayant conclu `S0`.
    ///
    /// La restriction au mécanisme employé est l'ADR 0035 décision 3, et elle change ce que cette
    /// méthode veut dire : une preuve portant sur un mécanisme que ce worker n'emploie pas n'est
    /// pas un niveau prouvé **pour lui**. Sans elle, un worker sans `bwrap` hériterait du `S2` que
    /// podman a prouvé sur la même machine, et le placement enverrait une mission dans un
    /// confinement que personne n'appliquerait.
    #[must_use]
    pub fn proven_level(&self) -> Option<SandboxLevel> {
        self.reconciled().employed
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
        if required > SandboxLevel::S0 {
            reasons.extend(self.reconciled().shortfall(required, &self.capabilities));
        }
        reasons
    }

    /// Ce que les attestations de ce candidat deviennent une fois rapprochées de son mécanisme.
    fn reconciled(&self) -> Reconciled {
        let employs = self.capabilities.mechanism();
        let mut reconciled = Reconciled::default();
        for attested in &self.attested {
            // Seuls les verdicts `Trusted` entrent dans le rapprochement. Un `NotTrusted` ne prouve
            // rien sous aucun mécanisme, et le compter parmi les preuves écartées ferait dire
            // « prouvé, mais ailleurs » d'une campagne qui a conclu que non.
            let Standing::Trusted { level } = attested.standing else {
                continue;
            };
            match employment(&attested.backend, employs) {
                Employment::Employed => {
                    reconciled.employed = reconciled.employed.max(Some(level));
                }
                Employment::Foreign => {
                    reconciled.foreign.push(attested.backend.clone());
                    reconciled.discarded = reconciled.discarded.max(Some(level));
                }
                Employment::Unresolved { unregistered } => {
                    reconciled.unresolved = true;
                    reconciled.unregistered.extend(unregistered);
                    reconciled.discarded = reconciled.discarded.max(Some(level));
                }
            }
        }
        reconciled.foreign.sort_unstable();
        reconciled.foreign.dedup();
        reconciled.unregistered.sort_unstable();
        reconciled.unregistered.dedup();
        reconciled
    }
}

/// Ce que les attestations d'un candidat deviennent une fois confrontées à son mécanisme.
///
/// Une seule passe produit tout ce dont le refus a besoin. En deux passes, le jour où l'une change,
/// l'autre reste — et le refus dirait une chose que le placement ne fait plus.
#[derive(Debug, Default)]
struct Reconciled {
    /// Le meilleur niveau prouvé sous un mécanisme que ce worker emploie.
    employed: Option<SandboxLevel>,
    /// Le meilleur niveau prouvé sous un mécanisme **écarté**, quelle qu'en soit la raison.
    discarded: Option<SandboxLevel>,
    /// Les mécanismes attestés, connus du registre, que ce worker n'emploie pas.
    foreign: Vec<String>,
    /// Un rapprochement au moins n'a pas pu se faire.
    ///
    /// Distinct de `!unregistered.is_empty()` : le rapprochement échoue aussi quand le manifeste ne
    /// nomme aucun mécanisme, et il n'y a alors **aucun** nom hors registre à montrer.
    unresolved: bool,
    /// Les noms hors registre rencontrés, triés et sans doublon.
    unregistered: Vec<String>,
}

impl Reconciled {
    /// Ce qui manque à ce candidat du côté de l'attestation, une fois le mécanisme pris en compte.
    fn shortfall(
        &self,
        required: SandboxLevel,
        capabilities: &HostCapabilities,
    ) -> Vec<RefusalReason> {
        if self
            .employed
            .is_some_and(|proven| proven.satisfies(required))
        {
            return Vec::new();
        }
        let mut reasons = Vec::new();
        // `foreign` non vide implique un mécanisme annoncé : sans annonce, aucun rapprochement ne
        // conclut `Foreign`. Le `if let` le dit au compilateur plutôt qu'à un lecteur.
        if let Some(employs) = capabilities.mechanism()
            && !self.foreign.is_empty()
        {
            reasons.push(RefusalReason::MechanismNotEmployed {
                required,
                employs: employs.to_owned(),
                attested: self.foreign.clone(),
            });
        }
        if self.unresolved {
            reasons.push(RefusalReason::MechanismUnresolved {
                required,
                employs: capabilities.mechanism().map(str::to_owned),
                unregistered: self.unregistered.clone(),
            });
        }
        // `level_not_attested` dit « l'hôte annonce ce niveau et ne l'a jamais prouvé ». Quand une
        // preuve écartée l'atteignait, c'est faux : le niveau **a** été prouvé, sous un mécanisme
        // qui n'est pas celui-ci, et les motifs ci-dessus le disent déjà. L'ajouter enverrait
        // relancer une campagne qui conclut déjà — exactement la confusion que ce découpage défait.
        if !self
            .discarded
            .is_some_and(|proven| proven.satisfies(required))
        {
            reasons.push(RefusalReason::LevelNotAttested {
                required,
                proven: self.employed,
            });
        }
        reasons
    }
}

/// Le verdict du placement.
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
