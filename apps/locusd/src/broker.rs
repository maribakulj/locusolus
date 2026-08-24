//! Ce que `locusd` sait de l'Execution Fabric — `W4.h`, ADR 0028 décision 4.
//!
//! # Le daemon démarre, et le dit
//!
//! Refuser de démarrer sans broker punirait quinze fonctions — lire le graphe, consulter la
//! mémoire, servir une revue — pour l'absence d'une seule. `locusd` démarre donc, **déclare l'état
//! du lien au démarrage et bruyamment**, et refuse ensuite uniquement ce qui en dépend.
//!
//! Ce que cette forme interdit : un `locusd` qui aurait l'air d'aller bien et qui échouerait à la
//! première mission réelle. C'est le même motif que la quarantaine de projection, qui refuse
//! d'ouvrir le port en le disant plutôt qu'en servant des lectures périmées.
//!
//! # Quatre états, et aucun ne se déduit d'un autre
//!
//! [`Standing`] ne fond pas « je n'ai pas pu demander » avec « on m'a dit non », ni « l'hôte est
//! incomplet » avec « le broker refuse de me parler ». Les quatre envoient chercher à quatre
//! endroits différents, et c'est la seule raison pour laquelle ils sont quatre.
//!
//! # La quatrième frontière tient par le graphe de paquets
//!
//! Ce module parle au broker et **n'importe rien** qui touche à un runtime de containers : il
//! dépend de `packages/broker`, qui ne dépend que de `packages/lep`. `apps/locusd` ne dépend pas de
//! `apps/locus-execd`, qui contient la seule fonction du dépôt exécutant `podman`. La règle 4 de
//! `boundaries.json` est ainsi tenue par le graphe plutôt que par une recherche de texte — comme
//! `W21.g` l'avait fait pour la sixième frontière.

use std::fmt;

use locus_broker::port::{BrokerError, BrokerPort};
use locus_broker::protocol::{Missing, Verdict};
use locus_lep::SandboxLevel;

/// L'état du lien vers le broker, tel que `locusd` l'a constaté.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Standing {
    /// Le broker répond et son hôte prouve ce qu'il faut.
    Ready {
        /// Le niveau le plus fort que cet hôte sait tenir.
        ceiling: SandboxLevel,
    },
    /// Le broker répond, et son hôte est insuffisant — voici **tout** ce qui manque.
    HostShort {
        /// Le niveau le plus fort que cet hôte sait tenir malgré tout.
        ceiling: SandboxLevel,
        /// Les manques, dans l'ordre où le broker les a constatés.
        missing: Vec<Missing>,
    },
    /// Le broker répond et refuse de nous parler.
    ///
    /// Distinct de [`Standing::Unreachable`] : ici il y a quelqu'un au bout du fil, et ce qu'il faut
    /// corriger est une identité ou des permissions, pas un service éteint.
    Refused {
        /// Ce que le broker en a dit.
        why: String,
    },
    /// On n'a pas pu demander.
    Unreachable {
        /// Où on a essayé.
        endpoint: String,
        /// Ce qui a empêché.
        why: String,
    },
}

impl Standing {
    /// Interroger le broker et rendre ce qu'on en a appris.
    ///
    /// Ne rend jamais d'erreur : l'échec **est** l'un des états, parce qu'un `locusd` qui ne saurait
    /// pas dire pourquoi il ne sait rien serait exactement le daemon silencieux que la décision 4
    /// refuse.
    pub fn probe(port: &dyn BrokerPort) -> Self {
        match port.readiness() {
            Ok(Verdict::Provable { ceiling }) => Self::Ready { ceiling },
            Ok(Verdict::HostShort { ceiling, missing }) => Self::HostShort { ceiling, missing },
            Ok(Verdict::Refused { why }) => Self::Refused { why },
            // Une réponse de **placement** à une question de disponibilité est un désaccord, pas un
            // état de l'hôte. La lire comme un `Ready` ferait annoncer au démarrage un plafond que
            // personne n'a mesuré ; l'ignorer laisserait le daemon sans nouvelle. Elle se dit donc,
            // sous le nom qui envoie regarder la version des deux binaires.
            Ok(answer @ (Verdict::Placed { .. } | Verdict::NotPlaced { .. })) => Self::Refused {
                why: format!(
                    "on a demandé la disponibilité de l'hôte et le broker a répondu sur un \
                     placement ({answer}) : une réponse hors sujet se dit, elle ne s'interprète pas"
                ),
            },
            Err(BrokerError::Unreachable { endpoint, why }) => Self::Unreachable { endpoint, why },
            // Un broker qui parle un vocabulaire qu'on ne lit pas est **joignable** : le dire
            // injoignable enverrait démarrer un service qui tourne déjà. La cause voyage dans la
            // phrase, et l'`endpoint` reste celui qu'on a interrogé.
            Err(error @ (BrokerError::Malformed { .. } | BrokerError::TooLong { .. })) => {
                Self::Refused {
                    why: error.to_string(),
                }
            }
        }
    }

    /// Vrai quand une exécution confinée peut être demandée.
    ///
    /// Un hôte insuffisant ne permet **pas** l'exécution : admettre une mission sur un hôte qui ne
    /// prouve pas son niveau serait le downgrade silencieux que §21.6 interdit, pris au moment où
    /// personne ne regarde.
    #[must_use]
    pub const fn permits_execution(&self) -> bool {
        matches!(self, Self::Ready { .. })
    }

    /// Ce qu'il faut répondre à qui demande une exécution que ce lien ne permet pas.
    ///
    /// Rend `None` quand l'exécution est permise. La phrase nomme **ce qui manque et où**, parce
    /// qu'un refus qui ne dit pas où regarder coûte une soirée à qui le reçoit.
    #[must_use]
    pub fn refusal(&self) -> Option<String> {
        match self {
            Self::Ready { .. } => None,
            _ => Some(format!("aucune exécution confinée n'est possible : {self}")),
        }
    }
}

impl fmt::Display for Standing {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready { ceiling } => {
                write!(formatter, "broker prêt — confinement jusqu'à {ceiling:?}")
            }
            Self::HostShort { ceiling, missing } => {
                write!(
                    formatter,
                    "broker joignable, hôte insuffisant — plafond {ceiling:?}, {} manque(s) : {}",
                    missing.len(),
                    missing
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(" ; ")
                )
            }
            Self::Refused { why } => write!(formatter, "broker joignable et refusant — {why}"),
            Self::Unreachable { endpoint, why } => {
                write!(formatter, "broker injoignable sur {endpoint} — {why}")
            }
        }
    }
}
