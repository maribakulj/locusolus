//! L'assignation d'une tâche atteint le journal — `W20.ad`, le producteur qui manquait.
//!
//! # Ce que cet item ferme
//!
//! `packages/projections/src/organisation_graph.rs`, livré par `W13.g` et marqué **fait**, lit des
//! événements `task.assigned` et en tire l'`agent_id` — avec, dans son en-tête, cette phrase :
//! « cette projection joint donc `assigned_agent_id` au graphe d'exécution — **et c'est la seule
//! source** ». Aucun handler ne les écrivait, et ses tests fabriquaient eux-mêmes les événements
//! qu'elle relit. C'est un **lecteur sans producteur**, la cinquième occurrence de cette forme
//! dans le chantier après `NoIdentities`, `NoAdministrators`, `NothingProven` et le convoyeur
//! d'attestations, et la première où c'est une projection qui attend.
//!
//! Le fait, lui, existe depuis `W13.d` : `packages/coordination` porte `Assignment` avec son
//! triplet — agent, worker, instant — et `Task::assigned` qui l'ajoute. Journaliser un fait de
//! domaine déjà là n'est **pas** « inventer un fait pour avoir quoi compter », ce que `W21.g` a
//! refusé sous ce nom : ce qui manquait n'était pas le fait, c'était son producteur.
//!
//! # Les deux identités, et pourquoi aucune ne suffit
//!
//! Un worker est une machine, un agent est un rôle situé. Deux agents peuvent tourner sur le même
//! worker, et un agent peut passer d'un worker à un autre. La charge porte donc les deux, comme
//! §7.1 porte `assigned_agent_id` **et** `assigned_worker_id` : n'en écrire qu'un rendrait
//! indécidable l'une des deux questions que la projection existe pour trancher — « qui a fait ce
//! travail » et « où a-t-il tourné ».
//!
//! C'est aussi ce qui sépare cet item de `W20.k`. Le cycle de bail — `task.leased`, `run.completed`
//! — est journalisé depuis cet item-là, et il ne joint rien : **un bail nomme un worker**. Le lien
//! instance × tâche passe par `agent_id`, et par lui seul.
//!
//! # L'acteur est le système, et c'est le premier du dépôt
//!
//! La projection ignore en silence tout `task.assigned` dont l'acteur n'est pas
//! [`ActorKind::System`], et sa raison est l'invariant 3 : « le plan de contrôle décide des
//! assignations ; un agent qui en annoncerait une décrirait sa propre affectation, et un graphe qui
//! le croirait serait un graphe que les workers écrivent ».
//!
//! Or **aucun producteur du dépôt n'écrivait `System`** — ni `lep::fact`, ni `mission::fact`, qui
//! posent tous deux `Agent` et documentent pourquoi : « un worker agit comme agent, jamais comme
//! système ». Les deux moitiés étaient cohérentes séparément, et la garde de la projection n'avait
//! jusqu'ici aucun cas à laisser passer. Ce module est le premier, et il n'emprunte donc pas les
//! deux aides existantes : leur acteur est précisément ce qu'il ne faut pas ici.
//!
//! `principal_id` reste celui de la commande. Les deux champs ne disent pas la même chose : `kind`
//! dit **qui a décidé** — le plan de contrôle —, `principal_id` dit **sous quelle autorité**. Les
//! confondre obligerait à inventer un principal système, que rien n'énrôle.
//!
//! # Ce que la charge ne porte pas
//!
//! Pas de champ `state`. `Task::assigned` le dit dans sa propre docstring : « **n'est pas une
//! transition** — une tâche `running` réassignée reste `running` ». Tous les autres faits de la
//! famille `task` portent l'état qu'ils viennent d'atteindre ; celui-ci n'en atteint aucun, et lui
//! en donner un ferait de `task.assigned` le seul événement de la famille dont le champ `state` ne
//! rapporte pas de changement. Un lecteur qui balaie la famille pour reconstruire une machine à
//! états compterait alors une transition qui n'a pas eu lieu.
//!
//! Pas de champ `at` non plus, et pour la raison symétrique : l'instant de l'acte a déjà un
//! domicile, `occurred_at` de §10.1. Il est pris **de l'assignation**, pas du contexte — la valeur
//! de domaine est la seule qui sache quand l'acte a eu lieu, et l'écrire aux deux endroits laisserait
//! les deux diverger sans que rien ne le dise.

use locus_coordination::task::Assignment;
use locus_domain::TaskState;
use locus_event_store::{Actor, ActorKind, Draft as EventDraft, EventType};

use crate::command::CommandEnvelope;
use crate::error::CommandError;
use crate::handler::Decide;
use crate::lep::{LepContext, stream_of_task};

/// Le type d'événement d'une assignation.
///
/// `task` est la famille de §10.3, et le verbe est celui que la projection de `W13.g` attend
/// littéralement — c'est un contrat entre deux moitiés, pas un choix de ce module.
const ASSIGNED: &str = "task.assigned";

/// Confier une tâche à un agent, sur un worker.
pub struct Assign {
    /// La tâche confiée.
    pub task_id: String,
    /// Son état, tel que l'appelant l'a lu.
    ///
    /// Comme pour [`crate::mission::Queue`] : le décideur ne va pas le chercher, il le reçoit et
    /// le confronte au domaine.
    pub from: TaskState,
    /// À qui, où, et quand.
    pub assignment: Assignment,
}

impl Decide for Assign {
    type State = LepContext;

    /// # Le refus vient du domaine, il n'y est pas transcrit
    ///
    /// `Task::assigned` refuse sur un état terminal, et le motif qu'il en donne est le bon :
    /// « confier un travail achevé n'assigne rien, et laisserait croire dans un journal que
    /// quelqu'un s'y est mis ». Ce décideur pose la même question à `TaskState::is_terminal`, qui
    /// la dérive elle-même de la table de §7.1. Écrire ici la liste des états finis en ferait une
    /// seconde copie, et les deux divergeraient au premier état ajouté.
    fn decide(
        &self,
        command: &CommandEnvelope,
        context: &Self::State,
    ) -> Result<Vec<EventDraft>, CommandError> {
        if self.from.is_terminal() {
            return Err(CommandError::Policy {
                policy: "task.assignment".to_owned(),
                detail: format!(
                    "§7.1 ne laisse rien sortir de « {} » : confier une tâche finie n'assigne \
                     rien, et le journal dirait que quelqu'un s'y est mis",
                    self.from
                ),
            });
        }
        Ok(vec![assigned_event(command, context, self)?])
    }
}

/// Le fait qu'une assignation produit.
fn assigned_event(
    command: &CommandEnvelope,
    context: &LepContext,
    assign: &Assign,
) -> Result<EventDraft, CommandError> {
    Ok(EventDraft {
        event_id: context.identity(0)?,
        event_type: EventType::parse(ASSIGNED).unwrap_or_else(|_| {
            unreachable!(
                "« {ASSIGNED} » est un littéral de ce module, et `task` est un namespace de §10.3"
            )
        }),
        schema_version: 1,
        stream_id: stream_of_task(&assign.task_id),
        workspace_id: *command.workspace_id(),
        project_id: context.project_id,
        program_id: None,
        branch_id: None,
        actor: Actor {
            principal_id: *command.actor_principal_id(),
            // Voir l'en-tête : la projection ignore tout autre acteur, et l'invariant 3 dit
            // pourquoi. C'est le seul `System` écrit par le dépôt.
            kind: ActorKind::System,
            delegation_id: command.delegation_id().copied(),
        },
        // De l'assignation, jamais du contexte : la valeur de domaine sait quand l'acte a eu lieu.
        occurred_at: assign.assignment.at(),
        causation_id: *command.command_id(),
        // `W20.j` : **jamais** renseignée ici. La clé d'idempotence est l'affaire de la
        // transaction, qui l'appose à l'écriture — un producteur qui la choisirait ferait
        // dépendre l'idempotence du client de ce que chaque handler se trouve écrire.
        idempotency_key: None,
        correlation_id: command.correlation_id().copied(),
        trace_id: None,
        payload: serde_json::json!({
            "task_id": assign.task_id,
            "agent_id": assign.assignment.agent_id().to_string(),
            "worker_id": assign.assignment.worker_id(),
        }),
        payload_hash: context.payload_hash.clone(),
    })
}
