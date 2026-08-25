//! L'émission d'un message — ADR 0019, seconde moitié de `W16.e`.
//!
//! # Où passe la frontière
//!
//! `locus_coordination::messaging` tient ce qu'un journal ne sait pas décider seul : sous quel epoch
//! un émetteur a agi, et ce qu'un destinataire conclut quand ce n'est pas le sien. Ce module-ci tient
//! l'autre moitié — **écrire le fait**, et par le seul chemin que `W20.b` autorise : un [`Decide`],
//! exécuté par la transaction.
//!
//! La séparation n'est pas décorative. Le domaine de coordination ne connaît pas `locus-event-store`
//! et n'a donc aucun moyen d'écrire ; c'est ce qui rend « toute mutation passe par un command
//! handler transactionnel » opposable par la signature plutôt que par la vigilance.
//!
//! # Un événement, et exactement un
//!
//! ADR 0019, condition 1 : aucun second stockage durable. Émettre un message rend **un**
//! `EventDraft` du namespace `message`, et rien d'autre — aucune file, aucun tampon local, aucune
//! table à part. Un test le vérifie sur le journal réel après la transaction, et pas seulement sur
//! le retour de [`Decide::decide`] : compter ce qu'une fonction rend ne dit rien de ce qu'elle a
//! écrit ailleurs en chemin.
//!
//! Ce test lit cette source et refuse les noms des structures qu'il interdit — d'où la périphrase
//! ci-dessus plutôt que la liste littérale. Même discipline que `version.rs` : une garde qui doit
//! décider, à chaque relecture, si une occurrence est un usage ou une explication est une garde
//! qu'on finit par assouplir.
//!
//! # Le stream est celui du destinataire
//!
//! Choix, et il se défend. §10.2 donne l'ordre total **par stream** ; ranger les messages sous le
//! destinataire donne donc gratuitement ce qu'une messagerie garantit péniblement — deux messages
//! adressés au même agent arrivent dans un ordre, et cet ordre est le même pour tous les lecteurs.
//! Les ranger sous l'émetteur aurait éparpillé la boîte de réception d'un agent sur autant de
//! streams qu'il a de correspondants, et lire « ce qui m'a été dit » serait devenu une jointure.

use locus_coordination::messaging::Message;
use locus_event_store::{Actor, ActorKind, Draft as EventDraft, EventType};
use locus_protocol::id::{Event, Project};
use locus_protocol::{Id, Timestamp};

use crate::command::CommandEnvelope;
use crate::error::CommandError;
use crate::handler::Decide;

/// Le type d'événement d'un message émis.
///
/// Un seul verbe pour l'instant, et c'est la règle des énumérations de `CLAUDE.md` : `message.read`,
/// `message.acknowledged` ou `message.expired` entreraient quand un consommateur exécutable et testé
/// les lira. Les écrire d'avance produirait des faits que rien n'honore.
const SENT: &str = "message.sent";

/// Ce qu'un message a besoin de savoir et que le domaine ne porte pas.
///
/// Même lacune que `BranchContext`, et nommée de la même façon plutôt que comblée à la sauvette :
/// `EventDraft` exige un `event_id` et un `project_id`, et rien dans le domaine ne fabrique
/// d'identifiants — le faire ici demanderait de l'entropie, donc un crate, donc un ADR.
pub struct MessageContext {
    /// Le projet auquel l'événement appartient.
    pub project_id: Id<Project>,
    /// L'identité de l'événement à écrire.
    pub event_id: Id<Event>,
    /// Quand l'acte a eu lieu — distinct de l'instant d'écriture (§10.1).
    pub occurred_at: Timestamp,
    /// Le hash de la charge canonicalisée.
    pub payload_hash: String,
}

/// Émettre un message — la commande.
pub struct Send {
    /// Ce qui est émis.
    pub message: Message,
}

impl Decide for Send {
    type State = MessageContext;

    fn decide(
        &self,
        command: &CommandEnvelope,
        context: &Self::State,
    ) -> Result<Vec<EventDraft>, CommandError> {
        Ok(vec![sent_event(command, context, &self.message)])
    }
}

/// Le fait qu'une émission produit.
///
/// La charge porte l'epoch **de l'émetteur**, sous sa forme textuelle. Un lecteur du journal doit
/// pouvoir rendre le verdict de réception sans relire le code qui a écrit l'événement — c'est la
/// même exigence que pour l'état d'arrivée d'une transition de branche.
fn sent_event(
    command: &CommandEnvelope,
    context: &MessageContext,
    message: &Message,
) -> EventDraft {
    EventDraft {
        event_id: context.event_id,
        event_type: EventType::parse(SENT).unwrap_or_else(|_| {
            unreachable!("« {SENT} » est un littéral de ce module, et `message` est dans EVENT_NAMESPACES depuis l'ADR 0019")
        }),
        schema_version: 1,
        stream_id: format!("agent/{}", message.to()),
        workspace_id: *command.workspace_id(),
        project_id: context.project_id,
        program_id: None,
        branch_id: None,
        actor: Actor {
            principal_id: *command.actor_principal_id(),
            kind: ActorKind::Agent,
            delegation_id: command.delegation_id().copied(),
        },
        occurred_at: context.occurred_at,
        causation_id: *command.command_id(),
        // `W20.j` : **jamais** renseignée ici. La clé d'idempotence est l'affaire de la
        // transaction, qui l'appose à l'écriture — un producteur qui la choisirait ferait
        // dépendre l'idempotence du client de ce que chaque handler se trouve écrire.
        idempotency_key: None,
        correlation_id: command.correlation_id().copied(),
        trace_id: None,
        payload: serde_json::json!({
            "from": message.from().to_string(),
            "to": message.to().to_string(),
            "epoch": message.epoch().to_string(),
            "subject": message.subject(),
        }),
        payload_hash: context.payload_hash.clone(),
    }
}
