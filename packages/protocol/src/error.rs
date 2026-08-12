//! L'enveloppe d'erreur structurée.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

use crate::id::Id;
use crate::id::provisional::{Error as ErrorKind, Mission};
use crate::time::Timestamp;

/// Ce que l'enveloppe remplace quand l'erreur est marquée sensible.
const REDACTED: &str = "<expurgé>";

/// La catégorie d'une erreur.
///
/// Les dix-sept catégories minimales de la spec Canterel §26, sans ajout. Elles décrivent *où*
/// l'erreur est née, ce qui n'est pas la même chose que son `code`, qui décrit *quoi*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// Violation du protocole lui-même.
    Protocol,
    /// L'appelant n'a pas pu être identifié.
    Authentication,
    /// L'appelant est identifié mais n'a pas le droit.
    Authorization,
    /// La mission a été refusée à l'admission.
    Admission,
    /// Une capability annoncée manque ou ne suffit pas.
    Capability,
    /// Le fournisseur de modèle a échoué.
    Model,
    /// Un outil a échoué.
    Tool,
    /// La sandbox a refusé ou échoué.
    Sandbox,
    /// Le réseau a échoué ou était interdit.
    Network,
    /// Un secret manque, est invalide ou a été refusé.
    Secret,
    /// Un budget est dépassé.
    Budget,
    /// Un artefact est invalide, absent ou incohérent.
    Artifact,
    /// La matérialisation du contexte a échoué.
    Context,
    /// La session locale du worker a échoué.
    Session,
    /// La lease est perdue, expirée ou refusée.
    Lease,
    /// Une garantie de sécurité est en cause.
    Security,
    /// Rien de ce qui précède : un défaut interne.
    Internal,
}

/// La politique de nouvelle tentative attachée à une erreur.
///
/// La spec Canterel §26 pose deux règles : « une erreur `retryable` doit préciser les conditions
/// de retry » et « les erreurs de politique ou sécurité ne sont jamais réessayées aveuglément ».
/// Ce type les rend indéfaisables plutôt que documentées — il n'existe aucune façon de
/// construire une erreur réessayable sans énoncer sa condition, en Rust comme sur le fil, où un
/// `"retryable": true` nu est refusé au décodage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Retry {
    /// Ne pas réessayer.
    Never,
    /// Réessayable, une fois la condition satisfaite.
    When(RetryCondition),
}

impl Retry {
    /// La condition à satisfaire, s'il y en a une.
    #[must_use]
    pub const fn condition(&self) -> Option<&RetryCondition> {
        match self {
            Self::Never => None,
            Self::When(condition) => Some(condition),
        }
    }
}

/// La condition sous laquelle une nouvelle tentative a un sens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryCondition {
    /// Ce qui doit changer pour qu'une nouvelle tentative ait un sens. Jamais vide.
    condition: String,
    /// Pas avant cet instant, lorsque l'attente est bornée.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    not_before: Option<Timestamp>,
}

impl RetryCondition {
    /// Énonce une condition de nouvelle tentative.
    ///
    /// # Errors
    ///
    /// Rend [`EmptyRetryCondition`] si la condition est vide ou blanche : « réessayable » sans
    /// dire à quelle condition est précisément ce que la spec interdit.
    pub fn new(condition: impl Into<String>) -> Result<Self, EmptyRetryCondition> {
        let condition = condition.into();
        if condition.trim().is_empty() {
            return Err(EmptyRetryCondition);
        }
        Ok(Self {
            condition,
            not_before: None,
        })
    }

    /// Borne l'attente : pas de nouvelle tentative avant cet instant.
    #[must_use]
    pub fn not_before(mut self, instant: Timestamp) -> Self {
        self.not_before = Some(instant);
        self
    }

    /// Ce qui doit changer.
    #[must_use]
    pub fn condition(&self) -> &str {
        &self.condition
    }

    /// L'instant avant lequel réessayer est inutile, s'il est connu.
    #[must_use]
    pub const fn earliest(&self) -> Option<Timestamp> {
        self.not_before
    }
}

/// Une condition de nouvelle tentative vide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyRetryCondition;

impl fmt::Display for EmptyRetryCondition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("une erreur réessayable doit préciser sa condition de retry")
    }
}

impl std::error::Error for EmptyRetryCondition {}

impl Serialize for Retry {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Never => serializer.serialize_bool(false),
            Self::When(condition) => condition.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for Retry {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Wire {
            Flag(bool),
            Condition(RetryCondition),
        }
        match Wire::deserialize(deserializer)? {
            Wire::Flag(false) => Ok(Self::Never),
            Wire::Flag(true) => Err(D::Error::custom(EmptyRetryCondition)),
            Wire::Condition(condition) if condition.condition.trim().is_empty() => {
                Err(D::Error::custom(EmptyRetryCondition))
            }
            Wire::Condition(condition) => Ok(Self::When(condition)),
        }
    }
}

/// Une erreur structurée, telle qu'elle traverse LEP.
///
/// Les champs sont ceux de la spec Canterel §26, dans son ordre.
///
/// # Ce que `security_sensitive` garantit
///
/// Une erreur marquée sensible ne laisse **pas** filtrer son message ni ses détails par
/// [`Display`], qui est le chemin par lequel une erreur finit dans un log. `CLAUDE.md` interdit
/// de journaliser jeton, clé, cookie ou contenu classifié ; l'interdiction ne tient que si le
/// chemin par défaut est le chemin sûr. Les champs restent lisibles pour qui les traite
/// légitimement — c'est l'écriture accidentelle qui est fermée, pas l'accès.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuredError {
    /// L'identifiant de cette occurrence.
    pub error_id: Id<ErrorKind>,
    /// Le code stable, lisible par une machine.
    pub code: String,
    /// D'où l'erreur vient.
    pub category: Category,
    /// Si et sous quelle condition réessayer.
    pub retryable: Retry,
    /// La mission concernée, s'il y en a une.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mission_id: Option<Id<Mission>>,
    /// Le rang de la tentative concernée, s'il y en a une.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt: Option<u32>,
    /// Le composant qui a produit l'erreur.
    pub component: String,
    /// Le message destiné à un humain.
    pub message: String,
    /// Les détails, ordonnés pour que la canonicalisation soit stable.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub details: BTreeMap<String, String>,
    /// L'erreur qui a causé celle-ci.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caused_by: Option<Box<StructuredError>>,
    /// Le message et les détails ne doivent pas être journalisés.
    pub security_sensitive: bool,
    /// Quand l'erreur est survenue.
    pub occurred_at: Timestamp,
}

impl StructuredError {
    /// Le message, ou une marque d'expurgation si l'erreur est sensible.
    #[must_use]
    pub fn public_message(&self) -> &str {
        if self.security_sensitive {
            REDACTED
        } else {
            &self.message
        }
    }

    /// L'erreur peut-elle être réessayée, et sous quelle condition ?
    #[must_use]
    pub const fn retry_condition(&self) -> Option<&RetryCondition> {
        self.retryable.condition()
    }

    /// La chaîne des causes, celle-ci comprise.
    pub fn chain(&self) -> impl Iterator<Item = &Self> {
        std::iter::successors(Some(self), |error| error.caused_by.as_deref())
    }
}

impl fmt::Display for StructuredError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "[{code}] {component}: {message}",
            code = self.code,
            component = self.component,
            message = self.public_message()
        )?;
        if !self.security_sensitive && !self.details.is_empty() {
            let rendered: Vec<_> = self
                .details
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect();
            write!(formatter, " ({})", rendered.join(", "))?;
        }
        if let Some(cause) = &self.caused_by {
            write!(formatter, " <- {cause}")?;
        }
        Ok(())
    }
}

impl std::error::Error for StructuredError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.caused_by
            .as_deref()
            .map(|error| error as &(dyn std::error::Error + 'static))
    }
}
