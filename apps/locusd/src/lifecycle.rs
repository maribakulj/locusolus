//! Les transitions de cycle de vie d'instance atteignent le journal — `W20.ae`.
//!
//! # L'affirmation que cet item corrige
//!
//! L'ADR 0026, décision 2, écrivait : « `coordination::lifecycle` journalise les transitions ;
//! `W21.j` mesure déjà la durée de vie d'une instance à partir d'elles ». C'était **faux**, et
//! `W0.20` l'a vérifié trois fois : le module rend un [`Outcome`] et n'émet aucun événement ; aucun
//! crate hors de `packages/coordination` ne l'importe ; `W21.j` reçoit ses instants **en données**.
//!
//! Le module est une machine à états de domaine, correcte et éprouvée, **dont les décisions ne
//! sortaient jamais**. C'est la même forme que `W20.ad` — un fait de domaine sans producteur — et
//! non « inventer un fait pour avoir quoi compter » : les quatre commandes, leurs refus et leurs
//! comptes existent depuis `W13`.
//!
//! # `agent` n'était écrit par personne
//!
//! Le namespace existe dans `EVENT_NAMESPACES` de §10.3 depuis `W1`, entre `task` et `team`. En
//! énumérant tous les types d'événement que `apps/locusd/src` écrit, `W20.ad` a trouvé `artifact`,
//! `branch`, `message`, `resource`, `run`, `task`, `team`, `worker` — et **aucun `agent`**. Le
//! journal ne portait donc aucun fait d'existence ni d'état d'`AgentInstance`, ce qui est ce qui
//! bloquait `W23.b` : `nominal` compterait des identités que le journal ne connaît pas, et zéro est
//! la valeur qu'un compteur vide rend quand il fonctionne.
//!
//! # Le verbe est **dérivé** de la commande, pas transcrit à côté d'elle
//!
//! `agent.spawned`, `agent.suspended`, `agent.drained`, `agent.killed` : chacun est
//! `Command::slug()` suivi de `ed`, calculé et non écrit. Un `match` sur [`Command`] aurait produit
//! quatre littéraux, c'est-à-dire un second vocabulaire — celui-là même que `CLAUDE.md` interdit
//! (« aucun vocabulaire parallèle ») et que l'en-tête de `coordination::lifecycle` refuse pour les
//! verbes de version. Un cinquième verbe qui ne se régulariserait pas ainsi ne compilerait pas en
//! silence : il produirait un type d'événement, et le test qui balaie [`Command::ALL`] le lirait.
//!
//! # Le fait nomme la commande appliquée, pas l'état atteint
//!
//! `agent.drained` dit « un drain a été appliqué à cette instance », ce qui est vrai des deux
//! issues que `drain` connaît. Le nommer d'après l'état atteint aurait forcé un choix entre deux
//! mensonges : un drain sur un nœud occupé **ne termine pas** l'instance — [`Outcome::Draining`]
//! laisse l'état inchangé, et le dire *est* le résultat —, donc `agent.completed` aurait menti une
//! fois sur deux, et deux types d'événement pour une commande auraient fait dépendre le vocabulaire
//! du journal de la charge d'un nœud.
//!
//! La charge porte donc l'issue sous son nom, avec son compte :
//!
//! | Issue                | `outcome`    | Compte                                   |
//! | -------------------- | ------------ | ---------------------------------------- |
//! | [`Outcome::Settled`] | `settled`    | aucun                                    |
//! | [`Outcome::Draining`]| `draining`   | `remaining` — ce qu'il reste à finir     |
//! | [`Outcome::Killed`]  | `killed`     | `abandoned` — **même quand il vaut zéro** |
//!
//! Le compte de `killed` est écrit même nul, et c'est la propriété que le type prend soin de porter
//! : sans lui, un exploitant ne distinguerait pas un arrêt propre d'un arrêt coûteux.
//!
//! # L'état résultant vient du domaine, jamais de ce module
//!
//! [`Lifecycle::command`] range lui-même l'état produit, et le fait le relit. Le recalculer ici —
//! fût-ce par un `match` de trois lignes — ferait une seconde machine à états, exactement ce que
//! `coordination::lifecycle` dit ne pas être.

use locus_coordination::agent::InstanceState;
use locus_coordination::lifecycle::{Command, Lifecycle, LifecycleError, Outcome, Quiescence};
use locus_event_store::{Actor, ActorKind, Draft as EventDraft, EventType};
use locus_protocol::Id;
use locus_protocol::id::Agent;

use crate::command::CommandEnvelope;
use crate::error::CommandError;
use crate::handler::Decide;
use crate::lep::LepContext;

/// Le stream d'une instance d'agent.
///
/// **Une par instance**, donc un seul écrivain et un seul verrou — le même arbitrage que
/// `stream_of_task` de `W20.h`. C'est aussi ce qui rend `nominal` calculable sans jointure : un
/// stream `agent/…` qui existe est une identité que le journal connaît.
#[must_use]
pub fn stream_of_instance(node: Id<Agent>) -> String {
    format!("agent/{node}")
}

/// Le type d'événement d'une commande de cycle de vie.
///
/// Dérivé, jamais transcrit — voir l'en-tête. Les quatre slugs de §7.1 sont réguliers, et le test
/// de sortie balaie [`Command::ALL`] pour que le jour où l'un ne le serait plus se voie.
#[must_use]
pub fn event_type_of(command: Command) -> String {
    format!("agent.{}ed", command.slug())
}

/// Adresser une commande de cycle de vie à une instance, et l'écrire.
pub struct Apply {
    /// L'instance visée.
    pub node: Id<Agent>,
    /// Les instances pilotées, telles que l'appelant les a reconstituées depuis le journal.
    ///
    /// `Lifecycle::knowing` existe pour ça, et sa docstring le dit : « pour reconstituer un
    /// scheduler depuis le journal ». Ce décideur ne tient donc aucun état — ADR 0016 décision 5,
    /// « aucun compteur, aucun magasin, aucun bus n'est créé ».
    pub lifecycle: Lifecycle,
    /// Ce qui est demandé.
    pub command: Command,
    /// Ce que le nœud a en vol, constaté.
    ///
    /// Une **lecture**, jamais une attente : rien n'oblige un nœud à devenir quiescent, et le
    /// module de domaine refuse par construction d'avoir un `wait_for_quiescence`.
    pub quiescence: Quiescence,
}

impl Decide for Apply {
    type State = LepContext;

    fn decide(
        &self,
        command: &CommandEnvelope,
        context: &Self::State,
    ) -> Result<Vec<EventDraft>, CommandError> {
        // Une copie : un décideur ne mute rien, et l'état qui compte est celui que le journal
        // portera. `Lifecycle` est une carte d'identités vers des états, pas une ressource.
        let mut piloted = self.lifecycle.clone();
        let outcome = piloted
            .command(self.node, self.command, self.quiescence)
            .map_err(|refusal| refusal_of(&refusal))?;
        // Du domaine, jamais recalculé ici.
        let state = piloted.state(self.node).ok_or_else(|| CommandError::Internal {
            detail: format!(
                "`Lifecycle::command` a accepté « {} » sur {} sans ranger d'état : le domaine et \
                 ce module ne diraient plus la même chose",
                self.command, self.node
            ),
        })?;
        Ok(vec![applied_event(command, context, self, outcome, state)?])
    }
}

/// Le refus d'une commande de cycle de vie, sous sa famille de §22.5.
///
/// Les cinq refus du domaine sont des **politiques** : ranimer une instance terminée effacerait la
/// trace de sa fin (§14.2), suspendre une instance seulement provisionnée laisserait croire qu'on a
/// arrêté quelque chose, `spawn` sur une instance vivante en créerait une seconde sous la même
/// identité. Dans les cinq cas la requête est bien écrite et c'est l'état qui s'y oppose ; rendre
/// `validation` enverrait l'appelant relire sa requête, où il ne trouverait rien.
///
/// §22.5 n'a pas de famille « absent » — les huit sont closes, et un test le tient par l'absence —,
/// donc `NoSuchInstance` ne fait pas exception plutôt que d'inventer la neuvième.
fn refusal_of(error: &LifecycleError) -> CommandError {
    CommandError::Policy {
        policy: "agent.lifecycle".to_owned(),
        detail: error.to_string(),
    }
}

/// Le fait qu'une commande de cycle de vie produit.
fn applied_event(
    command: &CommandEnvelope,
    context: &LepContext,
    apply: &Apply,
    outcome: Outcome,
    state: InstanceState,
) -> Result<EventDraft, CommandError> {
    let event_type = event_type_of(apply.command);
    Ok(EventDraft {
        event_id: context.identity(0)?,
        event_type: EventType::parse(&event_type).map_err(|_| CommandError::Internal {
            detail: format!(
                "« {event_type} » n'est pas un type d'événement : le slug de « {} » ne se \
                 régularise pas, et le journal recevrait un verbe que personne ne relira",
                apply.command
            ),
        })?,
        schema_version: 1,
        stream_id: stream_of_instance(apply.node),
        workspace_id: *command.workspace_id(),
        project_id: context.project_id,
        program_id: None,
        branch_id: None,
        actor: Actor {
            principal_id: *command.actor_principal_id(),
            // Comme `task.assigned` de `W20.ad`, et pour la même raison : piloter une instance est
            // une décision du **plan de contrôle**. `kind` dit qui a décidé, `principal_id` sous
            // quelle autorité.
            kind: ActorKind::System,
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
        payload: payload_of(apply.node, outcome, state),
        payload_hash: context.payload_hash.clone(),
    })
}

/// La charge : l'issue sous son nom, avec son compte, et l'état où l'instance se retrouve.
fn payload_of(node: Id<Agent>, outcome: Outcome, state: InstanceState) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "agent_id": node.to_string(),
        "state": state.to_string(),
    });
    let fields = payload
        .as_object_mut()
        .unwrap_or_else(|| unreachable!("`json!` d'un objet littéral rend un objet"));
    match outcome {
        Outcome::Settled(_) => {
            fields.insert("outcome".to_owned(), "settled".into());
        }
        Outcome::Draining { remaining } => {
            fields.insert("outcome".to_owned(), "draining".into());
            fields.insert("remaining".to_owned(), remaining.into());
        }
        Outcome::Killed { abandoned } => {
            fields.insert("outcome".to_owned(), "killed".into());
            // Écrit **même nul** : c'est ce qui sépare un arrêt propre d'un arrêt coûteux, et le
            // type de domaine prend déjà soin de le porter dans les deux cas.
            fields.insert("abandoned".to_owned(), abandoned.into());
        }
    }
    payload
}
