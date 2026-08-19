//! Les six capacités de branche de `W17.f`, joignables depuis `locusd`.
//!
//! # Une façade, et rien d'autre
//!
//! La ligne de roadmap disait « la logique des six est écrite, il ne leur manque qu'une façade ».
//! Vérifié avant d'écrire, et c'était exact :
//!
//! | Capacité | Où elle vit déjà |
//! | --- | --- |
//! | diff | `locus_coordination::diff::Diff::between(from, to)` |
//! | preview | `locus_coordination::barrier::Barriers::admits(diff)` |
//! | ombre | `locus_coordination::simulation::run(…, Fidelity::Shadow, …)` |
//! | approbation | `locus_domain::branch::Branch::validate(witness)` |
//! | rollback | `locus_domain::branch::Branch::reopen(into)` |
//! | navigation dans le temps | `locus_event_store` — `read_stream(stream, from)` |
//!
//! Ce module ne réécrit aucune des six. Il les **nomme**, les compose, et les range du bon côté de
//! la frontière que `W20.g` a posée. Une façade qui recalculerait serait une seconde implémentation,
//! et deux implémentations d'une même règle divergent le jour où l'une est corrigée.
//!
//! # Quatre lectures, deux commandes — et la frontière n'est pas décorative
//!
//! `W20.g` a fait de la surface HTTP une surface **en lecture seule** : elle ne tient qu'un
//! `&Runtime`, et `Transaction::submit` demande `&mut self`.
//!
//! Les six se répartissent donc d'eux-mêmes. Le diff, la preview, l'ombre et la navigation
//! **répondent** — ce sont des lectures, et elles passent par HTTP. L'approbation et le rollback
//! **décident** — ce sont des commandes, et elles sont des [`Decide`] que la transaction exécute.
//!
//! Ce n'est pas une commodité de découpage : c'est ce qui rend la clause suivante vraie par
//! construction.
//!
//! # « L'ombre et la preview ne produisent aucun événement »
//!
//! Prévisualiser ne doit pas agir, et la faute serait silencieuse : personne ne relit le journal
//! après une preview. Ici la garantie ne tient pas à la vigilance — [`Runtime::branch_preview`] et
//! [`Runtime::branch_shadow`] prennent `&self`, donc n'ont aucun chemin vers l'écriture. Un test le
//! vérifie quand même par le journal, parce que deux vérifications indépendantes valent mieux qu'une.

use locus_coordination::barrier::{Barriers, Passage};
use locus_coordination::diff::Diff;
use locus_coordination::simulation::{Fidelity, Outcome as ShadowOutcome, Recorded, run};
use locus_coordination::version::Version;
use locus_domain::branch::{Branch, BranchState, TransitionError, ValidationWitness};
use locus_event_store::{Actor, ActorKind, Draft as EventDraft, EventStore, EventType};
use locus_protocol::id::provisional::Decision as DecisionKind;
use locus_protocol::id::{Event, Project};
use locus_protocol::{Id, Timestamp};

use crate::command::CommandEnvelope;
use crate::composition::Runtime;
use crate::cursor::{Collection, Cursor, CursorError};
use crate::error::CommandError;
use crate::handler::Decide;
use crate::query::Page;

/// Ce qu'un diff rend à un lecteur — §22.4, `GET /branches/:id/diff`.
///
/// Le nombre d'opérations **et** leur nature : un approbateur qui lirait « 47 changements » sans
/// savoir lesquels n'approuverait rien, il signerait.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffView {
    /// La version de départ, nommée.
    pub from: String,
    /// La version d'arrivée, nommée.
    pub to: String,
    /// Les opérations, dans l'ordre imposé par le refus de la cascade.
    pub operations: Vec<String>,
}

impl DiffView {
    /// Vrai quand les deux versions ne diffèrent en rien.
    ///
    /// Un diff vide est **rendu**, jamais absent : c'est la règle que `Diff::between` pose déjà, et
    /// la façade ne la défait pas. Un approbateur doit voir que rien ne change, pas ne rien voir.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }
}

impl<S: EventStore> Runtime<S> {
    /// `GET /branches/:id/diff` — **entre deux révisions nommées**.
    ///
    /// # Pourquoi il n'existe pas de diff « depuis le début »
    ///
    /// Une comparaison sans borne n'est pas une comparaison : elle rend l'état, pas l'écart. Un
    /// approbateur à qui l'on montrerait « tout ce qui existe » croirait relire un changement alors
    /// qu'il relit un monde, et approuverait des faits que personne ne lui proposait.
    ///
    /// Les deux bornes sont donc **exigées par la signature**, comme `expected_revision` l'est pour
    /// une commande mutante (`W20.a`).
    #[must_use]
    pub fn branch_diff(&self, from: &Version, to: &Version) -> DiffView {
        let diff = Diff::between(from, to);
        DiffView {
            from: from.id().to_string(),
            to: to.id().to_string(),
            operations: diff
                .operations()
                .iter()
                .map(|operation| format!("{operation:?}"))
                .collect(),
        }
    }

    /// La **preview** : ce diff passerait-il maintenant ?
    ///
    /// Prend `&self`. Elle ne peut donc rien écrire — non par discipline, mais parce qu'elle n'a
    /// pas de quoi. C'est la même garantie que `W20.b` donne aux décideurs, appliquée à l'envers :
    /// là, le décideur n'a pas le journal ; ici, la preview n'a pas la transaction.
    #[must_use]
    pub fn branch_preview(&self, barriers: &Barriers, from: &Version, to: &Version) -> Passage {
        barriers.admits(&Diff::between(from, to))
    }

    /// L'**ombre** — §W16, `Fidelity::Shadow` : exécuter sans effet institutionnel.
    ///
    /// Le degré réellement atteint voyage dans le résultat, et ce n'est pas un détail : un rejeu ne
    /// dit pas ce qu'un canari dirait, et lire un verdict d'ombre comme un verdict de production est
    /// exactement la confusion que `simulation` existe pour empêcher.
    #[must_use]
    pub fn branch_shadow(
        &self,
        proposal: Id<DecisionKind>,
        plan: &[&str],
        environment: &Recorded,
    ) -> ShadowOutcome {
        run(proposal, Fidelity::Shadow, plan, environment)
    }

    /// La **navigation dans le temps** : l'état d'un stream tel qu'il était à une révision.
    ///
    /// # Ce que « naviguer » veut dire ici
    ///
    /// Pas un instantané reconstruit et stocké, mais la relecture du journal jusqu'à un rang. C'est
    /// la garantie de §10.2 — ordre total par stream — qui rend l'opération possible, et c'est
    /// pourquoi elle n'a besoin d'aucun stockage supplémentaire.
    ///
    /// # Errors
    ///
    /// [`CursorError`] si le cursor n'a pas été émis ici, ou vient d'une autre collection.
    pub fn branch_history(
        &self,
        stream: &str,
        after: Option<&Cursor>,
        limit: Option<usize>,
    ) -> Result<Page<HistoryEntry>, CursorError> {
        let from = after.map_or(Ok(0), |cursor| cursor.read(Collection::History))?;
        let limit = limit.unwrap_or(50).clamp(1, 500);

        let events = self.transaction_store().read_stream(stream, from);
        let items: Vec<HistoryEntry> = events
            .iter()
            .take(limit)
            .map(|envelope| HistoryEntry {
                revision: envelope.stream_revision,
                event_type: envelope.event_type.to_string(),
                recorded_at: envelope.recorded_at.to_string(),
            })
            .collect();
        let next = (events.len() > limit)
            .then(|| {
                items
                    .last()
                    .map(|last| Cursor::issue(Collection::History, last.revision))
            })
            .flatten();

        Ok(Page { items, next })
    }
}

/// Un pas de l'histoire d'un stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    /// Le rang dans le stream, à partir de 1.
    pub revision: u64,
    /// Le type d'événement, tel que §10.3 le nomme.
    pub event_type: String,
    /// Quand le journal l'a écrit.
    pub recorded_at: String,
}

/// L'**approbation** d'une branche — §8, `Branch::validate`.
///
/// # Le témoin voyage, et le refus nomme ce qui manque
///
/// `ValidationWitness` porte la politique lue **et** ses conditions. Une approbation qui ne dirait
/// que « refusé » obligerait le demandeur à deviner ; celle-ci rend les conditions non satisfaites,
/// une par une, parce que c'est la seule information qui permet d'agir.
pub struct Approve {
    /// La branche visée, dans son état actuel.
    pub branch: Branch,
    /// Ce que la politique a constaté.
    pub witness: ValidationWitness,
}

impl Decide for Approve {
    type State = BranchContext;

    fn decide(
        &self,
        command: &CommandEnvelope,
        context: &Self::State,
    ) -> Result<Vec<EventDraft>, CommandError> {
        match self.branch.validate(&self.witness) {
            Ok(validated) => Ok(vec![transition_event(
                command,
                context,
                "branch.validated",
                &validated,
            )]),
            Err(refusal) => Err(refusal_of(&refusal)),
        }
    }
}

/// Le **rollback** — `Branch::reopen`.
///
/// # Une commande, pas une suppression
///
/// Rouvrir une branche fusionnée **écrit un fait de plus**. Le journal est plus long après qu'avant,
/// et c'est l'invariant 12 vu du bon côté : ce qui a eu lieu ne cesse pas d'avoir eu lieu parce
/// qu'on est revenu dessus. Un rollback qui effacerait rendrait l'histoire cohérente et fausse.
pub struct Rollback {
    /// La branche à rouvrir.
    pub branch: Branch,
    /// L'état dans lequel elle repart.
    pub into: BranchState,
}

impl Decide for Rollback {
    type State = BranchContext;

    fn decide(
        &self,
        command: &CommandEnvelope,
        context: &Self::State,
    ) -> Result<Vec<EventDraft>, CommandError> {
        let reopened = self.branch.reopen(self.into);
        Ok(vec![transition_event(
            command,
            context,
            "branch.reopened",
            &reopened,
        )])
    }
}

/// Ce qu'une commande de branche a besoin de savoir, et que la branche ne porte pas.
///
/// # Pourquoi l'identifiant d'événement vient du dehors
///
/// `EventDraft` exige un `event_id` et un `project_id` ; `Branch` n'a ni l'un ni l'autre — elle
/// connaît son workstream, pas le projet, et rien dans le domaine ne fabrique d'identifiants.
///
/// Les fabriquer ici demanderait de l'entropie, donc un crate, donc un ADR et une entrée dans
/// `dependencies.json`. Un item de façade n'est pas le lieu où l'on prend cette décision, et
/// l'inventer en passant serait exactement le débordement que `W20.c` a évité pour le transport.
/// Le contexte est donc **fourni**, et la lacune est nommée plutôt que comblée à la sauvette.
pub struct BranchContext {
    /// Le projet auquel l'événement appartient.
    pub project_id: Id<Project>,
    /// L'identité de l'événement à écrire.
    pub event_id: Id<Event>,
    /// Quand l'acte a eu lieu — distinct de l'instant d'écriture (§10.1).
    pub occurred_at: Timestamp,
    /// Le hash de la charge canonicalisée.
    pub payload_hash: String,
}

/// Le refus d'une transition, traduit sous sa famille de §22.5.
///
/// Une transition refusée est une **politique** qui s'y oppose, pas une requête mal formée : le
/// client a demandé quelque chose de bien écrit, et l'état de la branche l'interdit. Lui rendre
/// `validation` l'enverrait relire sa requête, où il ne trouverait rien.
fn refusal_of(error: &TransitionError) -> CommandError {
    CommandError::Policy {
        policy: "branch.transition".to_owned(),
        detail: error.to_string(),
    }
}

/// Le fait qu'une transition produit.
///
/// Le stream est celui de la branche, et la charge porte l'état **d'arrivée** : un lecteur du
/// journal doit pouvoir reconstituer l'état sans relire le code qui l'a produit.
fn transition_event(
    command: &CommandEnvelope,
    context: &BranchContext,
    event_type: &str,
    branch: &Branch,
) -> EventDraft {
    EventDraft {
        event_id: context.event_id,
        event_type: EventType::parse(event_type).unwrap_or_else(|_| {
            unreachable!("« {event_type} » est un littéral de ce module, et `branch` est un namespace de §10.3")
        }),
        schema_version: 1,
        stream_id: format!("branch/{}", branch.id),
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
        correlation_id: command.correlation_id().copied(),
        trace_id: None,
        payload: serde_json::json!({ "state": format!("{:?}", branch.state), "revision": branch.revision }),
        payload_hash: context.payload_hash.clone(),
    }
}
