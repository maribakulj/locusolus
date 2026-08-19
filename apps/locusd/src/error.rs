//! Les huit familles d'erreur de `SPEC_V1.md` §22.5, et ce qu'un refus doit rendre.
//!
//! # Huit familles, et pas dix-sept
//!
//! `locus_protocol::error::Category` porte **dix-sept** catégories, celles de la spec Canterel §26.
//! Ce sont deux vocabulaires pour deux lecteurs : `Category` dit *où* une erreur de worker est née
//! — sandbox, modèle, outil, lease — et s'adresse à qui exploite une exécution. Les huit familles
//! ci-dessous disent *comment un client d'API doit réagir* à un refus de commande, et s'adressent à
//! qui écrit un client.
//!
//! Les fondre serait tentant : quatre noms sont communs. Ce serait faux dans les deux sens. Une
//! commande refusée pour `policy` n'est née dans aucune sandbox, et une erreur de modèle n'a aucune
//! réaction de client d'API — elle n'arrive jamais en réponse à une commande. §22 nomme ses huit
//! familles, `CLAUDE.md` dit que les objets de §22 entrent « sous leur nom », et c'est ce qui est
//! fait ici.
//!
//! # Ce qu'un conflit doit rendre
//!
//! §22.5 : « un conflit retourne l'**état courant** et un code de conflit structuré ». Les deux
//! moitiés comptent. Un client qui reçoit un conflit doit relire avant de retenter ; lui rendre un
//! entier nu — `409`, ou même la révision attendue — l'oblige à deviner *quoi* relire. [`Conflict`]
//! porte donc la révision courante **et** la ressource à relire, sous une forme qu'un client peut
//! suivre sans connaître la topologie du serveur.

use std::fmt;

use serde::{Deserialize, Serialize};

/// La révision d'une ressource — le compteur qu'`expected_revision` compare.
///
/// Un type à part plutôt qu'un `u64` nu : les deux révisions d'un conflit se ressemblent trop pour
/// être distinguées par leur position dans une signature, et une inversion produirait un message
/// exactement faux — « attendu 7, courant 4 » là où c'est l'inverse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Revision(u64);

impl Revision {
    /// La révision d'une ressource qui vient d'être créée.
    pub const INITIAL: Self = Self(0);

    /// Une révision lue.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Sa valeur.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for Revision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// Ce qu'un client doit relire avant de retenter — §22.5, §22.4.
///
/// Une chaîne de chemin plutôt qu'un identifiant nu, parce que c'est ce que le client va demander :
/// §22.4 énumère `GET /branches/:id`, `GET /tasks/:id`. Lui rendre `br_01…` l'obligerait à savoir
/// quelle collection interroger, c'est-à-dire à reconstruire la table que le serveur possède déjà.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ResourceRef(String);

impl ResourceRef {
    /// La ressource, telle qu'un client la relira.
    ///
    /// # Errors
    ///
    /// [`EmptyResourceRef`] pour une référence vide : un conflit qui ne dit pas quoi relire n'a pas
    /// rendu l'état courant, il a seulement dit non.
    pub fn new(path: impl Into<String>) -> Result<Self, EmptyResourceRef> {
        let path = path.into();
        if path.trim().is_empty() {
            return Err(EmptyResourceRef);
        }
        Ok(Self(path))
    }

    /// Le chemin.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.0
    }
}

/// Une référence de ressource vide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyResourceRef;

impl fmt::Display for EmptyResourceRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("un conflit sans ressource à relire ne rend pas l'état courant")
    }
}

impl std::error::Error for EmptyResourceRef {}

/// Un conflit de concurrence optimiste — §22.5.
///
/// Il porte les **deux** révisions et la ressource. Un client qui reçoit ce type sait ce qu'il
/// croyait, ce qui est, et où aller lire ; les trois lui manqueraient s'il ne recevait qu'un code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conflict {
    /// Ce que la commande déclarait attendre.
    pub expected: Revision,
    /// Ce que la ressource porte réellement.
    pub current: Revision,
    /// Ce qu'il faut relire pour retenter.
    pub resource: ResourceRef,
}

/// Les huit familles d'erreur de §22.5.
///
/// Liste **close** : une neuvième n'existe pas, et un test le tient par l'absence. Une famille de
/// plus voudrait dire qu'un client doit apprendre une réaction nouvelle, ce qui est une rupture de
/// contrat et pas un détail d'implémentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    /// La commande est mal formée : un champ manque, ou ne respecte pas sa forme.
    Validation,
    /// L'appelant est identifié et n'a pas le droit.
    Authorization,
    /// La ressource a bougé sous la commande — voir [`Conflict`].
    Conflict,
    /// Le service ne peut pas répondre maintenant. Distinct de `internal` : celle-ci passe.
    Unavailable,
    /// Un budget est dépassé, ou la réservation excède ce qui reste.
    Budget,
    /// Une politique refuse. Distinct de `authorization` : le droit existe, la règle s'y oppose.
    Policy,
    /// Une garantie de sécurité est en cause. Jamais réessayée aveuglément.
    Security,
    /// Rien de ce qui précède. Le seul aveu, et il ne se sous-divise pas.
    Internal,
}

impl Family {
    /// Les huit, sous les noms de §22.5.
    ///
    /// Écrits en toutes lettres pour qu'un test d'absence ait quelque chose à lire, et pour que
    /// l'échec dise **laquelle** est entrée ou sortie.
    pub const NAMES: [&'static str; 8] = [
        "validation",
        "authorization",
        "conflict",
        "unavailable",
        "budget",
        "policy",
        "security",
        "internal",
    ];

    /// Les huit, dans l'ordre de §22.5.
    pub const ALL: [Self; 8] = [
        Self::Validation,
        Self::Authorization,
        Self::Conflict,
        Self::Unavailable,
        Self::Budget,
        Self::Policy,
        Self::Security,
        Self::Internal,
    ];

    /// Son nom sur le fil.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Validation => "validation",
            Self::Authorization => "authorization",
            Self::Conflict => "conflict",
            Self::Unavailable => "unavailable",
            Self::Budget => "budget",
            Self::Policy => "policy",
            Self::Security => "security",
            Self::Internal => "internal",
        }
    }
}

impl fmt::Display for Family {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

/// Pourquoi une commande est refusée.
///
/// Chaque variante porte ce qui manque à l'appelant pour agir. Un refus générique lui laisserait
/// relire la documentation ; celui-ci lui dit quel champ, quelle révision, quelle politique.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "family", rename_all = "snake_case")]
pub enum CommandError {
    /// Un champ manque ou ne respecte pas sa forme — le champ est **nommé**.
    Validation {
        /// Lequel.
        field: String,
        /// Ce qui ne va pas.
        detail: String,
    },
    /// L'appelant n'a pas le droit.
    Authorization {
        /// L'action refusée.
        action: String,
    },
    /// La ressource a bougé.
    Conflict(Conflict),
    /// Le service ne peut pas répondre maintenant.
    Unavailable {
        /// Ce qui ne répond pas.
        detail: String,
    },
    /// Un budget est dépassé.
    Budget {
        /// Lequel.
        budget: String,
        /// Ce qui a été demandé au-delà.
        detail: String,
    },
    /// Une politique s'y oppose.
    Policy {
        /// Laquelle.
        policy: String,
        /// Pourquoi.
        detail: String,
    },
    /// Une garantie de sécurité est en cause.
    Security {
        /// Laquelle.
        detail: String,
    },
    /// Un défaut interne.
    Internal {
        /// Ce qu'on peut en dire à un client.
        detail: String,
    },
}

impl CommandError {
    /// Sa famille.
    ///
    /// Exhaustif, sans branche fourre-tout : une variante nouvelle sans famille ne compile pas.
    #[must_use]
    pub const fn family(&self) -> Family {
        match self {
            Self::Validation { .. } => Family::Validation,
            Self::Authorization { .. } => Family::Authorization,
            Self::Conflict(_) => Family::Conflict,
            Self::Unavailable { .. } => Family::Unavailable,
            Self::Budget { .. } => Family::Budget,
            Self::Policy { .. } => Family::Policy,
            Self::Security { .. } => Family::Security,
            Self::Internal { .. } => Family::Internal,
        }
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation { field, detail } => {
                write!(formatter, "« {field} » : {detail}")
            }
            Self::Authorization { action } => write!(formatter, "« {action} » n'est pas permis"),
            Self::Conflict(conflict) => write!(
                formatter,
                "révision attendue {}, courante {} — relire {}",
                conflict.expected,
                conflict.current,
                conflict.resource.path()
            ),
            Self::Unavailable { detail } => write!(formatter, "indisponible : {detail}"),
            Self::Budget { budget, detail } => write!(formatter, "budget « {budget} » : {detail}"),
            Self::Policy { policy, detail } => {
                write!(formatter, "politique « {policy} » : {detail}")
            }
            Self::Security { detail } => write!(formatter, "sécurité : {detail}"),
            Self::Internal { detail } => write!(formatter, "défaut interne : {detail}"),
        }
    }
}

impl std::error::Error for CommandError {}
