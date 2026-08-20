//! L'enveloppe normative d'un événement — `docs/SPEC_V1.md` §10.1.

use std::fmt;

use serde::{Deserialize, Serialize};

use locus_protocol::{
    Id, Timestamp,
    id::{Agent, Branch, Command, Delegation, Event, Program, Project, Workflow, Workspace},
};

/// L'identifiant d'un événement.
pub type EventId = Id<Event>;

/// L'identifiant du stream auquel appartient l'événement.
///
/// §10.1 montre `"stream_id": "claim_01..."` : un stream est un objet, et son identifiant porte le
/// préfixe de sa nature. Le type reste une chaîne validée plutôt qu'un `Id<K>` typé, parce que la
/// nature varie d'un stream à l'autre — un journal qui n'accepterait qu'un seul préfixe ne
/// pourrait pas porter les branches à côté des claims.
pub type StreamId = String;

/// Qui a agi — §10.1, champ `actor`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Actor {
    /// Le principal, humain ou agent.
    pub principal_id: Id<Agent>,
    /// Sa nature.
    pub kind: ActorKind,
    /// La délégation sous laquelle il agit, quand il en a une.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delegation_id: Option<Id<Delegation>>,
}

/// La nature d'un acteur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorKind {
    /// Un être humain.
    Human,
    /// Un agent.
    Agent,
    /// Le système lui-même — une migration, une projection, une tâche périodique.
    System,
}

/// L'enveloppe de §10.1, champ pour champ.
///
/// # Les deux horodatages
///
/// `occurred_at` est le moment de l'acte, `recorded_at` celui de l'écriture. §10.1 les montre
/// distincts à la milliseconde près dans son propre exemple, et la distinction n'est pas
/// décorative : un worker hors ligne (§24.3) produit des événements dont l'acte précède l'écriture
/// de plusieurs heures. Les confondre ferait dater tout un travail offline de son moment de
/// synchronisation, et l'ordre des faits en serait faux.
///
/// # `stream_revision` est attribué par le journal
///
/// Le champ existe dans l'enveloppe, mais aucun producteur ne le remplit : c'est
/// [`crate::store::EventStore`] qui le pose à l'append. Le laisser à l'appelant rendrait possibles
/// deux événements de même rang dans un stream, ce qui est exactement ce que « ordre total par
/// stream » interdit. D'où [`Draft`], qui est l'enveloppe **moins** ce que le journal attribue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Envelope {
    /// L'identité de l'événement.
    pub event_id: EventId,
    /// Le type, dans la taxonomie de §10.3.
    pub event_type: EventType,
    /// La version du schéma de `payload`.
    pub schema_version: u32,
    /// Le stream auquel l'événement appartient.
    pub stream_id: StreamId,
    /// Le rang dans le stream, à partir de 1. **Attribué par le journal.**
    pub stream_revision: u64,
    /// Le workspace.
    pub workspace_id: Id<Workspace>,
    /// Le projet.
    pub project_id: Id<Project>,
    /// Le programme.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program_id: Option<Id<Program>>,
    /// La branche.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_id: Option<Id<Branch>>,
    /// Qui a agi.
    pub actor: Actor,
    /// Quand l'acte a eu lieu.
    pub occurred_at: Timestamp,
    /// Quand le journal l'a écrit.
    pub recorded_at: Timestamp,
    /// La commande qui a causé cet événement. C'est elle qui porte l'idempotence (§10.2).
    pub causation_id: Id<Command>,
    /// Le workflow qui corrèle plusieurs événements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<Id<Workflow>>,
    /// La trace d'observabilité — §25.1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    /// La charge, opaque pour l'enveloppe.
    pub payload: serde_json::Value,
    /// Le hash de la charge canonicalisée.
    pub payload_hash: String,
}

/// Un événement **avant** son écriture : tout, sauf ce que le journal attribue.
///
/// Deux champs manquent — `stream_revision` et `recorded_at` — et c'est délibéré. Le premier est
/// le rang dans le stream, le second l'instant de l'écriture : les deux sont des faits du journal,
/// pas du producteur. Les demander à l'appelant reviendrait à lui faire promettre ce qu'il ne peut
/// pas savoir, et à rendre représentable un journal où deux événements portent le même rang.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Draft {
    /// L'identité de l'événement.
    pub event_id: EventId,
    /// Le type, dans la taxonomie de §10.3.
    pub event_type: EventType,
    /// La version du schéma de `payload`.
    pub schema_version: u32,
    /// Le stream visé.
    pub stream_id: StreamId,
    /// Le workspace.
    pub workspace_id: Id<Workspace>,
    /// Le projet.
    pub project_id: Id<Project>,
    /// Le programme.
    pub program_id: Option<Id<Program>>,
    /// La branche.
    pub branch_id: Option<Id<Branch>>,
    /// Qui a agi.
    pub actor: Actor,
    /// Quand l'acte a eu lieu.
    pub occurred_at: Timestamp,
    /// La commande causale.
    pub causation_id: Id<Command>,
    /// Le workflow corrélant.
    pub correlation_id: Option<Id<Workflow>>,
    /// La trace d'observabilité.
    pub trace_id: Option<String>,
    /// La charge.
    pub payload: serde_json::Value,
    /// Le hash de la charge canonicalisée.
    pub payload_hash: String,
}

impl Draft {
    /// Sceller le brouillon avec ce que le journal attribue.
    ///
    /// Le seul chemin vers une [`Envelope`], et il passe par le journal : c'est ce qui fait que
    /// « ordre total par stream » n'a pas besoin d'être vérifié après coup.
    #[must_use]
    pub fn seal(self, stream_revision: u64, recorded_at: Timestamp) -> Envelope {
        Envelope {
            event_id: self.event_id,
            event_type: self.event_type,
            schema_version: self.schema_version,
            stream_id: self.stream_id,
            stream_revision,
            workspace_id: self.workspace_id,
            project_id: self.project_id,
            program_id: self.program_id,
            branch_id: self.branch_id,
            actor: self.actor,
            occurred_at: self.occurred_at,
            recorded_at,
            causation_id: self.causation_id,
            correlation_id: self.correlation_id,
            trace_id: self.trace_id,
            payload: self.payload,
            payload_hash: self.payload_hash,
        }
    }
}

/// Les préfixes de la taxonomie de §10.3, dans l'ordre du texte.
pub const EVENT_NAMESPACES: [&str; 31] = [
    "workspace",
    "project",
    "program",
    "workstream",
    "branch",
    "task",
    "agent",
    "team",
    "worker",
    "lease",
    "budget",
    "policy",
    "approval",
    "decision",
    "epistemic_object",
    "relation",
    "inference",
    "conflict",
    "artifact",
    "run",
    "reproduction",
    "review",
    "rebuttal",
    "memory",
    "context_view",
    "workflow",
    "federation",
    "security",
    // Trois ajouts locaux : `docs/10` place ces familles dans W1 et W3, et §10.3 les cite ailleurs
    // dans le texte sans les lister ici. Signalés plutôt que fondus dans la liste normative.
    "projection",
    "migration",
    // `message` est le troisième, et il n'est pas du même genre : §10.3 ne le cite **nulle part**.
    // Il entre par l'ADR 0019, qui décide qu'un message inter-agents est un événement plutôt qu'un
    // transport parallèle — donc qu'il a une famille, comme tout fait du journal. Le fondre dans la
    // liste sans le dire ferait passer un ajout pour une lecture de la spec.
    //
    // Il entre **avec son consommateur** — `locusd::messaging` — et pas avant : une famille inscrite
    // sans lecteur est ce que `CLAUDE.md` refuse pour les relations de coordination, et la raison
    // vaut à l'identique ici.
    "message",
];

/// Un type d'événement — `namespace.verbe`, dans la taxonomie de §10.3.
///
/// Le namespace est **vérifié**, le verbe ne l'est pas. §10.3 donne les familles avec un `*` et
/// n'énumère aucun verbe : fermer la liste des verbes ici interdirait un événement que la spec
/// autorise. Fermer celle des namespaces, en revanche, attrape la faute qui compte — un événement
/// rangé dans une famille qui n'existe pas est un événement qu'aucune projection n'ira chercher.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EventType {
    namespace: String,
    verb: String,
}

impl EventType {
    /// Lit un type d'événement `namespace.verbe`.
    ///
    /// # Errors
    ///
    /// Rend [`ParseEventTypeError`] si la forme n'est pas `namespace.verbe`, si le namespace n'est
    /// pas l'un de ceux de §10.3, ou si l'une des deux moitiés est vide.
    pub fn parse(text: &str) -> Result<Self, ParseEventTypeError> {
        let Some((namespace, verb)) = text.split_once('.') else {
            return Err(ParseEventTypeError::NotNamespaced);
        };
        if namespace.is_empty() || verb.is_empty() {
            return Err(ParseEventTypeError::Empty);
        }
        if !EVENT_NAMESPACES.contains(&namespace) {
            return Err(ParseEventTypeError::UnknownNamespace);
        }
        Ok(Self {
            namespace: namespace.to_owned(),
            verb: verb.to_owned(),
        })
    }

    /// La famille — §10.3.
    #[must_use]
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Le verbe.
    #[must_use]
    pub fn verb(&self) -> &str {
        &self.verb
    }
}

impl fmt::Display for EventType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}", self.namespace, self.verb)
    }
}

impl Serialize for EventType {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for EventType {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        let text = <&str>::deserialize(deserializer)?;
        Self::parse(text).map_err(D::Error::custom)
    }
}

/// Ce qui peut empêcher de lire un type d'événement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseEventTypeError {
    /// Pas de `.` : le type ne dit pas de quelle famille il relève.
    NotNamespaced,
    /// Une famille absente de §10.3.
    UnknownNamespace,
    /// Namespace ou verbe vide.
    Empty,
}

impl fmt::Display for ParseEventTypeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NotNamespaced => "type d'événement sans famille — la forme est `famille.verbe`",
            Self::UnknownNamespace => "famille absente de la taxonomie de §10.3",
            Self::Empty => "famille ou verbe vide",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ParseEventTypeError {}
