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

use crate::protocol::Verdict;

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

/// Ce que `locusd` sait demander au broker.
///
/// Une seule question aujourd'hui, et l'ADR 0028 décision 5 dit pourquoi ce n'est pas un jalon
/// partiel : le tube est complet, et les opérations suivantes sont des variantes de requête sur un
/// tube qui marche.
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
}
