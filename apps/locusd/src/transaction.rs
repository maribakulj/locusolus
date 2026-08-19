//! La transaction — le seul chemin par lequel une mutation atteint le journal.
//!
//! # Ce que « le seul chemin » veut dire, exactement
//!
//! [`Transaction`] **possède** le journal, dans un champ privé, et n'expose aucun accesseur qui le
//! rende. Un appelant de `locusd` qui tient une transaction ne peut donc rien écrire d'autre que ce
//! qu'un [`Decide`] a rendu ; un appelant qui ne tient pas de transaction n'a pas de journal du
//! tout.
//!
//! La garantie a une frontière, et elle mérite d'être dite plutôt que suggérée : `locus-event-store`
//! est un crate public, dont `EventStore::append` est une méthode publique. Rien n'empêche un
//! **autre** crate de s'en procurer un et d'écrire. Ce que `W20.b` rend opposable est la règle *dans
//! `locusd`* — et c'est là qu'elle compte, puisque c'est `locusd` qui détient l'autorité
//! transactionnelle (`SPEC_V1.md` §4.1). Le test de sortie le vérifie par l'absence, sur les
//! fichiers de ce crate.
//!
//! # L'échec ne laisse rien
//!
//! Un refus survient **avant** l'`append` : la transaction demande sa décision au handler, et
//! n'écrit que si elle en obtient une. Il n'existe pas d'état où une partie du lot est écrite et le
//! reste refusé, parce qu'il n'existe pas de chemin qui écrive deux fois pour une commande.

use locus_event_store::{Append, AppendError, Draft as EventDraft, EventStore, StreamId};
use locus_protocol::Timestamp;

use crate::command::CommandEnvelope;
use crate::error::{CommandError, Conflict, ResourceRef, Revision};
use crate::handler::{Batch, Decide, Ledger, Submission, expected_from};
use crate::outcome::{Accepted, Outcome};

/// L'autorité transactionnelle de §4.1 : elle tient le journal, et elle est seule à le tenir.
///
/// # Pourquoi le registre d'idempotence vit ici et non dans le journal
///
/// Le journal porte déjà une idempotence, par `command_id` (§10.2) : deux `append` sous le même
/// identifiant de commande rendent le premier résultat. Ce n'est pas la même chose que celle de
/// §22.5, qui porte sur l'`idempotency_key` **choisie par le client** — un client qui retente après
/// une coupure réémet sa clé, mais rien ne l'oblige à réémettre le même `command_id`, qu'il n'a
/// peut-être jamais vu.
///
/// Les deux sont nécessaires et ne se remplacent pas : celle du journal protège l'écriture, celle-ci
/// protège le client.
#[derive(Debug)]
pub struct Transaction<S> {
    store: S,
    ledger: Ledger,
}

impl<S: EventStore> Transaction<S> {
    /// Une transaction sur ce journal, qu'elle prend et ne rend plus.
    pub fn new(store: S) -> Self {
        Self {
            store,
            ledger: Ledger::default(),
        }
    }

    /// Soumettre une commande, et rendre son verdict.
    ///
    /// # La resoumission
    ///
    /// Une clé déjà vue **dans la même portée** rend le résultat d'origine sans rien réécrire. Une
    /// clé identique dans une autre portée est une autre soumission, et elle s'exécute : c'est ce
    /// que `W20.b` demande, et c'est ce qui empêche le succès d'un client de répondre à un autre.
    pub fn submit<D: Decide>(
        &mut self,
        handler: &D,
        command: &CommandEnvelope,
        state: &D::State,
        now: Timestamp,
    ) -> Outcome {
        let submission = Submission::of(command);
        if let Some(revision) = self.ledger.recall(&submission) {
            return Outcome::Accepted(Accepted { revision });
        }

        let events = match handler.decide(command, state) {
            Ok(events) => events,
            Err(refusal) => return Outcome::Refused(refusal),
        };

        match self.write(command, events, now) {
            Ok(accepted) => {
                self.ledger.remember(submission, accepted.revision);
                Outcome::Accepted(accepted)
            }
            Err(refusal) => Outcome::Refused(refusal),
        }
    }

    /// Soumettre un lot, selon ce qu'il **a déclaré** être.
    ///
    /// # Deux comportements, parce que le lot en a déclaré un
    ///
    /// - [`Batch::Atomic`] : une seule écriture. Toutes les décisions sont prises d'abord ; si l'une
    ///   refuse, **rien** n'est écrit et le verdict du lot est ce refus. Les commandes doivent viser
    ///   un seul stream, faute de quoi le lot est refusé avant toute décision — promettre une
    ///   atomicité inter-streams que le journal ne peut pas tenir serait pire que la refuser.
    /// - [`Batch::Sequential`] : une écriture par commande, dans l'ordre, jusqu'au premier refus.
    ///   Ce qui précède reste écrit. C'est ce que « non atomique » veut dire, et le rendre visible
    ///   est l'objet de la déclaration.
    ///
    /// Le vecteur rendu a **un verdict par commande exécutée**, et il est donc plus court que le lot
    /// quand un refus l'a arrêté. Un vecteur de même longueur, complété par des refus fabriqués,
    /// laisserait croire que les commandes suivantes ont été tentées.
    pub fn submit_batch<D: Decide>(
        &mut self,
        handler: &D,
        batch: &Batch,
        state: &D::State,
        now: Timestamp,
    ) -> Vec<Outcome> {
        match batch {
            Batch::Sequential(commands) => {
                let mut verdicts = Vec::with_capacity(commands.len());
                for command in commands {
                    let verdict = self.submit(handler, command, state, now);
                    let refused = verdict.refused().is_some();
                    verdicts.push(verdict);
                    if refused {
                        break;
                    }
                }
                verdicts
            }
            Batch::Atomic(commands) => self.submit_atomic(handler, commands, state, now),
        }
    }

    /// Le journal, en lecture seule.
    ///
    /// Il n'existe pas d'accesseur mutable, et c'est tout l'objet du type : une projection ou un
    /// diagnostic a besoin de lire, personne n'a besoin d'écrire hors d'ici.
    pub const fn store(&self) -> &S {
        &self.store
    }

    fn submit_atomic<D: Decide>(
        &mut self,
        handler: &D,
        commands: &[CommandEnvelope],
        state: &D::State,
        now: Timestamp,
    ) -> Vec<Outcome> {
        let Some(first) = commands.first() else {
            return Vec::new();
        };

        // Toutes les décisions d'abord. Une seule écriture ensuite — c'est ce qui rend l'échec sans
        // trace, plutôt qu'une suite d'écritures qu'il faudrait défaire.
        let mut events = Vec::new();
        for command in commands {
            if command.expected_revision() != first.expected_revision() {
                return vec![Outcome::Refused(CommandError::Validation {
                    field: "expected_revision".to_owned(),
                    detail: "un lot atomique est une écriture : ses commandes annoncent la même révision".to_owned(),
                })];
            }
            match handler.decide(command, state) {
                Ok(mut decided) => events.append(&mut decided),
                Err(refusal) => return vec![Outcome::Refused(refusal)],
            }
        }

        if let Some(refusal) = single_stream(&events) {
            return vec![Outcome::Refused(refusal)];
        }

        match self.write(first, events, now) {
            Ok(accepted) => {
                for command in commands {
                    self.ledger
                        .remember(Submission::of(command), accepted.revision);
                }
                commands
                    .iter()
                    .map(|_| Outcome::Accepted(accepted.clone()))
                    .collect()
            }
            Err(refusal) => vec![Outcome::Refused(refusal)],
        }
    }

    fn write(
        &mut self,
        command: &CommandEnvelope,
        events: Vec<EventDraft>,
        now: Timestamp,
    ) -> Result<Accepted, CommandError> {
        let Some(stream_id) = events.first().map(|event| event.stream_id.clone()) else {
            return Err(CommandError::Internal {
                detail:
                    "le handler n'a décidé aucun événement : une commande acceptée produit un fait"
                        .to_owned(),
            });
        };

        let append = Append {
            stream_id: stream_id.clone(),
            expected: expected_from(command.expected_revision()),
            command_id: *command.command_id(),
            events,
        };

        match self.store.append(append, now) {
            Ok(appended) => Ok(Accepted {
                revision: Revision::new(appended.revision),
            }),
            Err(error) => Err(refusal_for(&error, command, &stream_id)),
        }
    }
}

/// Le refus que le client doit lire, à partir de ce que le journal a répondu.
///
/// Chaque cas est traduit sous sa famille de §22.5, et le `match` est exhaustif : une variante
/// nouvelle d'[`AppendError`] ne compilera pas tant que personne n'aura dit comment un client doit y
/// réagir. C'est la question à laquelle un fourre-tout `_ =>` répondrait « comme à une panne », ce
/// qui est faux pour trois des quatre cas.
fn refusal_for(
    error: &AppendError,
    command: &CommandEnvelope,
    stream_id: &StreamId,
) -> CommandError {
    match error {
        AppendError::Conflict { actual, .. } => ResourceRef::new(stream_id.clone()).map_or_else(
            |_| CommandError::Internal {
                detail: "conflit sur un stream sans identifiant".to_owned(),
            },
            |resource| {
                CommandError::Conflict(Conflict {
                    expected: command.expected_revision(),
                    current: Revision::new(*actual),
                    resource,
                })
            },
        ),
        // Les trois suivants sont des défauts du handler, pas de l'appelant : le client n'a rien à
        // corriger, et lui rendre `validation` l'enverrait chercher une faute dans sa requête.
        //
        // `EmptyBatch` est **inatteignable par construction** : `write` refuse avant d'appeler
        // `append` quand la décision est vide, puisqu'il n'aurait aucun stream où écrire. Le bras
        // reste parce que le `match` est exhaustif, et l'exhaustivité est ce qui empêchera une
        // variante future d'`AppendError` d'être avalée sans que personne ait dit comment un client
        // doit y réagir. Un mutation testing le confirme : le mutant qui change ce bras survit, et
        // celui qui supprime la garde de `write` meurt — c'est bien la garde qui répond.
        AppendError::EmptyBatch => CommandError::Internal {
            detail: "le handler a rendu un lot vide".to_owned(),
        },
        AppendError::StreamMismatch { expected, found } => CommandError::Internal {
            detail: format!(
                "le handler a décidé un événement pour `{found}` dans une écriture sur `{expected}`"
            ),
        },
        AppendError::CommandReused { command_id } => CommandError::Internal {
            detail: format!("`{command_id}` a déjà écrit un lot différent"),
        },
    }
}

/// Un lot atomique vise un seul stream, ou il est refusé avant d'écrire.
fn single_stream(events: &[EventDraft]) -> Option<CommandError> {
    let first = events.first()?;
    let divergent = events
        .iter()
        .find(|event| event.stream_id != first.stream_id)?;
    Some(CommandError::Validation {
        field: "batch".to_owned(),
        detail: format!(
            "un lot atomique vise un seul stream : `{}` et `{}` n'en font pas un",
            first.stream_id, divergent.stream_id
        ),
    })
}
