//! Le port, et les trois issues qu'il ne permet pas de confondre — ADR 0028 décisions 1 et 4.
//!
//! # Pourquoi un port avant un backend
//!
//! C'est l'ordre que ce dépôt suit partout : le backend de workflow déterministe avant Temporal
//! (ADR 0003), le port transactionnel avant le transport (`W20.b`), l'event store en mémoire avant
//! tout driver. Un appelant écrit contre le trait ; le jour où un second backend arrive — le lien
//! distant de l'ADR 0028 décision 6 — aucun appelant ne change.
//!
//! # « Je n'ai pas pu demander » n'est pas « on m'a dit non »
//!
//! [`BrokerError::Unreachable`] et un [`Verdict::Refused`] envoient chercher à des endroits opposés :
//! l'un dit de démarrer le service ou de vérifier le chemin de la socket, l'autre dit de corriger
//! une identité ou des permissions. Les fondre est la faute que `W22` a passé une phase entière à
//! corriger ailleurs, et que `W5.h` avait déjà nommée pour les sondes d'hôte — une absence de
//! réponse n'est pas une réponse négative.
//!
//! La séparation est **structurelle** et non conventionnelle : un broker qui ne répond pas ne peut
//! par définition rien mettre sur le fil, donc l'injoignabilité ne peut pas être une variante de
//! [`Verdict`]. Elle vit du côté de l'appelant, où elle est constatée.

use std::fmt;

use locus_lep::{CapabilityManifest, ResourceSpec, SandboxLevel, SandboxSpec};

use crate::protocol::{Shortfall, Verdict};

/// Ce qui empêche d'obtenir un verdict.
///
/// Aucune de ces variantes n'est un verdict : elles disent toutes que la question n'a pas abouti,
/// et chacune dit à quel endroit regarder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrokerError {
    /// On n'a pas pu parler au broker du tout.
    Unreachable {
        /// Où on a essayé.
        endpoint: String,
        /// Ce que le système en a dit.
        why: String,
    },
    /// Le broker a parlé, mais dans un vocabulaire qu'on ne lit pas.
    ///
    /// C'est en général un écart de version entre les deux binaires, et le dire ainsi évite de
    /// chercher une panne de service là où il y a un désaccord de protocole.
    Malformed {
        /// Ce que le lecteur en a dit.
        why: String,
    },
    /// Le broker a parlé, et ce qu'il a dit dépasse ce que le protocole permet.
    ///
    /// Distinct de [`BrokerError::Malformed`] : ici la forme est peut-être juste, c'est la borne de
    /// l'ADR 0028 décision 7 qui est franchie.
    TooLong {
        /// Ce qui a été lu avant d'abandonner.
        read: usize,
    },
}

impl fmt::Display for BrokerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreachable { endpoint, why } => {
                write!(formatter, "broker injoignable sur {endpoint} — {why}")
            }
            Self::Malformed { why } => write!(formatter, "réponse illisible du broker — {why}"),
            Self::TooLong { read } => write!(
                formatter,
                "réponse du broker trop longue : {read} octets sans fin de ligne"
            ),
        }
    }
}

impl std::error::Error for BrokerError {}

/// Ce qu'une question de placement rend — `W20.q`.
///
/// # Pourquoi un type à part, et pas [`Verdict`] tel quel
///
/// [`Verdict`] porte les réponses aux **deux** questions, parce qu'il n'y a qu'une [`crate::protocol::Response`].
/// Un appelant qui demande un placement, lui, n'a que trois issues possibles, et les deux variantes
/// de disponibilité n'en sont pas : les lui rendre l'obligerait à écrire une branche
/// « ça ne devrait pas arriver », c'est-à-dire l'endroit exact où l'on finit par supposer.
///
/// Le hors-sujet devient donc un [`BrokerError::Malformed`] — le broker a parlé, mais pas de ce
/// qu'on lui demandait, et c'est un désaccord de protocole comme un autre.
#[derive(Debug, Clone, PartialEq)]
pub enum Placement {
    /// La mission peut aller sur ce worker, à ce niveau.
    Placed {
        /// Le worker retenu.
        worker: String,
        /// Le niveau qui sera appliqué.
        level: SandboxLevel,
    },
    /// Aucun worker soumis ne convient, et voici ce qui manquait à chacun.
    NotPlaced {
        /// Un manque par worker examiné.
        shortfalls: Vec<Shortfall>,
    },
    /// Le broker refuse de répondre à cet appelant.
    ///
    /// Il reste une **réponse**, jamais une erreur : `port.rs` tient cette séparation pour la
    /// disponibilité depuis `W4.h`, et la relâcher ici enverrait chercher un service éteint là où
    /// il y a une identité à corriger.
    Refused {
        /// Ce que le broker en a dit.
        why: String,
    },
}

/// Ce que `locusd` sait demander au broker.
///
/// Deux questions, et l'ADR 0028 décision 5 avait annoncé la seconde : « l'admission […]
/// s'ajouter[a] comme des variantes de requête sur un tube qui marche ». C'est ce qui s'est passé —
/// le transport n'a pas bougé.
pub trait BrokerPort {
    /// Où ce port parle, pour que les messages d'erreur nomment un endroit réel.
    fn endpoint(&self) -> String;

    /// Demander au broker ce qu'il sait confiner.
    ///
    /// # Errors
    ///
    /// [`BrokerError`] quand la question n'aboutit pas. Un broker qui répond « je refuse de te
    /// parler » **n'est pas** une erreur : c'est un [`Verdict::Refused`], parce qu'il a parlé.
    fn readiness(&self) -> Result<Verdict, BrokerError>;

    /// Demander au broker si ce worker, tel qu'il s'annonce, peut porter cette mission.
    ///
    /// # Errors
    ///
    /// [`BrokerError`] quand la question n'aboutit pas — y compris [`BrokerError::Malformed`] quand
    /// le broker répond à l'**autre** question.
    fn place(
        &self,
        manifest: &CapabilityManifest,
        sandbox: &SandboxSpec,
        resources: &ResourceSpec,
    ) -> Result<Placement, BrokerError>;
}

/// Lire un verdict comme la réponse à une question de placement, ou dire qu'il n'en est pas une.
///
/// # Errors
///
/// [`BrokerError::Malformed`] quand le verdict répond à la question de disponibilité.
pub fn as_placement(verdict: Verdict) -> Result<Placement, BrokerError> {
    match verdict {
        Verdict::Placed { worker, level } => Ok(Placement::Placed { worker, level }),
        Verdict::NotPlaced { shortfalls } => Ok(Placement::NotPlaced { shortfalls }),
        Verdict::Refused { why } => Ok(Placement::Refused { why }),
        other @ (Verdict::Provable { .. } | Verdict::HostShort { .. }) => {
            Err(BrokerError::Malformed {
                why: format!(
                    "on a demandé un placement et le broker a répondu sur la disponibilité de son \
                     hôte ({other}) : une réponse hors sujet se dit, elle ne s'interprète pas"
                ),
            })
        }
    }
}

/// Un broker en mémoire, pour les appelants qui n'en ont pas de vrai.
///
/// # Ce qu'il est, et ce qu'il n'est pas
///
/// C'est l'implémentation de référence du port, au sens où `packages/event-store` en a une : elle
/// existe pour que le contrat soit exerçable sans hôte, et les tests de contrat passent contre elle
/// **et** contre la socket. Ce n'est pas un simulacre de broker — elle ne prétend rien savoir de
/// l'hôte, elle rend ce qu'on lui a donné à rendre.
#[derive(Debug, Clone)]
pub struct Loopback {
    verdict: Result<Verdict, BrokerError>,
}

impl Loopback {
    /// Un broker qui rend ce verdict.
    #[must_use]
    pub const fn answering(verdict: Verdict) -> Self {
        Self {
            verdict: Ok(verdict),
        }
    }

    /// Un broker qu'on n'atteint pas.
    ///
    /// Il existe parce que l'injoignabilité doit être **exerçable** par les appelants : sans elle,
    /// le chemin le plus important de la décision 4 ne serait éprouvé que là où une vraie socket est
    /// disponible.
    #[must_use]
    pub fn unreachable(endpoint: &str, why: &str) -> Self {
        Self {
            verdict: Err(BrokerError::Unreachable {
                endpoint: endpoint.to_owned(),
                why: why.to_owned(),
            }),
        }
    }
}

impl BrokerPort for Loopback {
    fn endpoint(&self) -> String {
        match &self.verdict {
            Err(BrokerError::Unreachable { endpoint, .. }) => endpoint.clone(),
            _ => "mémoire".to_owned(),
        }
    }

    fn readiness(&self) -> Result<Verdict, BrokerError> {
        self.verdict.clone()
    }

    /// Le **même** verdict, quelle que soit la question.
    ///
    /// C'est ce qui fait de ce type une implémentation de référence et non un simulacre : il rend ce
    /// qu'on lui a donné à rendre, sans savoir de quoi on parle. Conséquence voulue : un `Loopback`
    /// monté avec un verdict de disponibilité fait échouer une demande de placement en
    /// [`BrokerError::Malformed`], exactement comme le ferait un vrai broker d'une autre version.
    fn place(
        &self,
        _manifest: &CapabilityManifest,
        _sandbox: &SandboxSpec,
        _resources: &ResourceSpec,
    ) -> Result<Placement, BrokerError> {
        as_placement(self.verdict.clone()?)
    }
}
