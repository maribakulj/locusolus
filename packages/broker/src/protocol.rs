//! Ce qui traverse le tube — ADR 0028 décision 5.
//!
//! # Une seule question aujourd'hui, et ce n'est pas un jalon partiel
//!
//! Le lien porte [`Request::Readiness`] : *broker, sais-tu confiner, et sinon que te manque-t-il ?*
//! C'est la question que `locusd` doit poser avant toute autre — sans elle, il placerait une mission
//! sur un hôte dont il ne sait rien.
//!
//! Au sens de l'ADR 0022 décision 0, c'est une **capacité finie** : le tube existe, il est éprouvé
//! de bout en bout, et l'admission puis le cycle de vie des sandboxes s'ajouteront comme des
//! variantes de requête sur un tube qui marche. Ce qui serait une promesse serait l'inverse —
//! déclarer ici des variantes que personne ne sait honorer.
//!
//! # Le vocabulaire de fil ne duplique rien
//!
//! Les six niveaux de confinement s'écrivent avec [`locus_lep::SandboxLevel`], qui existe déjà. Une
//! troisième orthographe de `S0`–`S5` serait le « vocabulaire parallèle » que `CLAUDE.md` interdit,
//! et deux orthographes qui divergent d'un cran sont pires qu'une seule mal choisie.
//!
//! # Pourquoi `Missing` est recopié ici plutôt qu'importé
//!
//! `locus_execd::linux::Missing` porte `what: &'static str` : il se sérialise, il ne se
//! **désérialise** pas — une chaîne lue sur un fil n'est pas statique. La traduction est donc
//! nécessaire, et elle vit du côté qui répond, comme `wire.rs` traduit déjà les refus d'admission
//! vers `packages/lep`. L'importer aurait par ailleurs fait dépendre `locusd` du crate qui contient
//! la seule fonction du dépôt exécutant `podman`, ce qui aurait tenu la règle 4 contre son objet.

use std::fmt;

use locus_lep::SandboxLevel;
use serde::{Deserialize, Serialize};

/// La version du protocole de lien, portée par chaque requête et chaque réponse.
///
/// Elle n'est pas celle de LEP : LEP est le protocole des **workers**, figé en `lep/1.0` et amendé
/// par un mineur. Ce lien-ci est interne au control plane, et confondre les deux ferait qu'un
/// changement de l'un obligerait à versionner l'autre.
pub const PROTOCOL: &str = "broker/1.0";

/// Ce que `locusd` demande.
///
/// Le champ `protocol` est en premier et n'a pas de valeur par défaut : une requête d'une autre
/// version se lit et se refuse, au lieu d'être interprétée avec le vocabulaire courant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    /// La version du protocole de l'appelant.
    pub protocol: String,
    /// Ce qui est demandé.
    pub ask: Ask,
}

impl Request {
    /// La requête de disponibilité, dans la version courante.
    #[must_use]
    pub fn readiness() -> Self {
        Self {
            protocol: PROTOCOL.to_owned(),
            ask: Ask::Readiness,
        }
    }
}

/// La question posée, sous une forme close.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "ask", rename_all = "snake_case")]
pub enum Ask {
    /// Sais-tu confiner, et sinon que te manque-t-il ?
    Readiness,
}

/// Ce que `locus-execd` répond.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Response {
    /// La version du protocole du répondant.
    pub protocol: String,
    /// Le verdict.
    pub verdict: Verdict,
}

impl Response {
    /// Une réponse dans la version courante.
    #[must_use]
    pub fn new(verdict: Verdict) -> Self {
        Self {
            protocol: PROTOCOL.to_owned(),
            verdict,
        }
    }
}

/// Ce que le broker rend.
///
/// # Trois issues, et le type ne permet pas de les confondre
///
/// `Refused` n'est **pas** une erreur de transport : le broker a parlé, et il a dit non. Une panne
/// de lien ne s'exprime pas ici du tout — elle vit dans [`crate::BrokerError`], du côté de
/// l'appelant, parce qu'un broker qui ne répond pas ne peut par définition rien mettre sur le fil.
/// C'est la décision 4 de l'ADR 0028 rendue structurelle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum Verdict {
    /// L'hôte prouve ce qu'il faut, et voici jusqu'où il plafonne.
    Provable {
        /// Le niveau le plus fort que cet hôte sait tenir.
        ceiling: SandboxLevel,
    },
    /// L'hôte ne prouve pas assez, et voici **tout** ce qui manque.
    HostShort {
        /// Le niveau le plus fort que cet hôte sait tenir malgré tout.
        ceiling: SandboxLevel,
        /// Les manques, dans l'ordre où ils ont été constatés.
        missing: Vec<Missing>,
    },
    /// Le broker refuse de répondre à cet appelant.
    ///
    /// Il porte son propre nom sur le fil parce que, sans lui, la première mise en service se
    /// passerait à chercher un problème de réseau qui n'existe pas — ADR 0028 décision 2.
    Refused {
        /// Pourquoi, en clair.
        why: String,
    },
}

/// Un manque, sous une forme qui traverse un fil.
///
/// Les deux variantes ne se fondent pas : « l'hôte ne l'offre pas » envoie changer de machine,
/// « on n'a pas pu l'établir » envoie regarder pourquoi la lecture a échoué. C'est la règle de
/// `W5.h`, et elle vaut ici comme là-bas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "missing", rename_all = "snake_case")]
pub enum Missing {
    /// L'hôte ne l'offre pas.
    Unavailable {
        /// La capacité concernée.
        what: String,
        /// Ce qui le dit.
        reason: String,
    },
    /// On n'a pas pu l'établir.
    Undetermined {
        /// La capacité concernée.
        what: String,
        /// Ce qui a empêché de savoir.
        reason: String,
    },
}

impl fmt::Display for Missing {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable { what, reason } => write!(formatter, "{what} : {reason}"),
            Self::Undetermined { what, reason } => {
                write!(formatter, "{what} : indéterminé — {reason}")
            }
        }
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provable { ceiling } => {
                write!(formatter, "prouvé jusqu'à {ceiling:?}")
            }
            Self::HostShort { ceiling, missing } => {
                write!(
                    formatter,
                    "hôte insuffisant — plafond {ceiling:?}, {} manque(s)",
                    missing.len()
                )
            }
            Self::Refused { why } => write!(formatter, "refusé — {why}"),
        }
    }
}
