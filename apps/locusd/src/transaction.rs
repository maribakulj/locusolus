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
use crate::writes::{Admitted, StreamLocks};

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
///
/// # `&self` sur `submit` — ADR 0029 décision 2
///
/// La couche HTTP ne tient qu'un `&Runtime`, partagé entre plusieurs fils. Une transaction qui
/// exigeait `&mut self` rendait §22.3 inservable, et c'est ce que `main.rs` nommait depuis `W20.g`.
///
/// Ce qui s'exclut est le couple `(consultation du registre, écriture)` **par stream**, tenu par
/// [`StreamLocks`] ; `Decide::decide` est pure et reste dehors. Voir `crate::writes` pour le motif
/// complet.
#[derive(Debug)]
pub struct Transaction<S> {
    store: S,
    ledger: std::sync::Mutex<Ledger>,
    locks: StreamLocks,
}

impl<S: EventStore> Transaction<S> {
    /// Une transaction sur ce journal, qu'elle prend et ne rend plus.
    ///
    /// # Le registre d'idempotence se **reconstruit** ici — `W20.j`
    ///
    /// `Ledger::default()` était un registre vide, et il l'était **à chaque démarrage**. Or un
    /// redémarrage est précisément ce qui coupe les connexions et déclenche les retentes : la
    /// garantie de §22.5 était donc fausse au moment exact où elle sert.
    ///
    /// La reconstruction lit le flux global, comme les quatre projections de §9.5, et pour la même
    /// raison — l'invariant 2 fait du journal la vérité institutionnelle, et un registre qui vivrait
    /// ailleurs en serait un second stockage durable, ce que l'ADR 0019 a déjà refusé.
    pub fn new(store: S) -> Self {
        let ledger = Ledger::rebuild(store.feed(0).iter().map(|entry| &entry.event));
        Self {
            store,
            ledger: std::sync::Mutex::new(ledger),
            locks: StreamLocks::new(),
        }
    }

    /// Une transaction dont la borne d'admission est choisie.
    ///
    /// La borne est une valeur du service — ADR 0029 décision 6 — et non une constante cachée : un
    /// profil de déploiement peut la fixer, et un test peut l'exercer sans fabriquer mille
    /// écritures concurrentes.
    pub fn bounded(store: S, limit: usize) -> Self {
        // La reconstruction est **ici aussi**, et l'oublier serait le genre de défaut que ce dépôt
        // trouve trois mois plus tard : deux constructeurs dont un seul tient la garantie, et le
        // second est celui que le binaire emploie quand un exploitant borne les écritures.
        let ledger = Ledger::rebuild(store.feed(0).iter().map(|entry| &entry.event));
        Self {
            store,
            ledger: std::sync::Mutex::new(ledger),
            locks: StreamLocks::with_limit(limit),
        }
    }

    /// Les verrous d'écriture, pour un diagnostic ou un test.
    ///
    /// En lecture seule : personne n'a besoin d'acquérir un verrou hors de [`Transaction::submit`],
    /// et l'offrir ouvrirait un chemin où une écriture se ferait sans passer par le handler.
    pub const fn locks(&self) -> &StreamLocks {
        &self.locks
    }

    /// Soumettre une commande, et rendre son verdict.
    ///
    /// # La resoumission
    ///
    /// Une clé déjà vue **dans la même portée** rend le résultat d'origine sans rien réécrire. Une
    /// clé identique dans une autre portée est une autre soumission, et elle s'exécute : c'est ce
    /// que `W20.b` demande, et c'est ce qui empêche le succès d'un client de répondre à un autre.
    pub fn submit<D: Decide>(
        &self,
        handler: &D,
        command: &CommandEnvelope,
        state: &D::State,
        now: Timestamp,
    ) -> Outcome {
        let submission = Submission::of(command);
        if let Some(revision) = self.recall(&submission) {
            return Outcome::Accepted(Accepted { revision });
        }

        // La décision se prend **hors** de toute exclusion : elle est pure, et c'est ce qui permet à
        // une commande lente sur un stream de ne pas retarder une commande sur un autre.
        let events = match handler.decide(command, state) {
            Ok(events) => events,
            Err(refusal) => return Outcome::Refused(refusal),
        };
        let Some(stream_id) = events.first().map(|event| event.stream_id.clone()) else {
            return Outcome::Refused(no_events());
        };

        self.serialised(&stream_id, || match self.write(command, events, now) {
            Ok(accepted) => {
                self.remember(submission, accepted.revision);
                Outcome::Accepted(accepted)
            }
            Err(refusal) => Outcome::Refused(refusal),
        })
    }

    /// Exécuter une écriture sous le verrou de son stream, ou refuser si la borne est franchie.
    ///
    /// Le refus est un `unavailable` de §22.5 qui **nomme la borne** : sans elle, un exploitant ne
    /// distingue pas une saturation d'une lenteur, et un client ne sait pas qu'il peut retenter.
    fn serialised(&self, stream_id: &str, work: impl FnOnce() -> Outcome) -> Outcome {
        match self.locks.with(stream_id, work) {
            Admitted::Done(outcome) => outcome,
            Admitted::Saturated { limit } => Outcome::Refused(CommandError::Unavailable {
                detail: format!(
                    "{limit} écritures sont déjà admises : le service est saturé, pas en panne — \
                     retenter plus tard aboutira"
                ),
            }),
        }
    }

    fn recall(&self, submission: &Submission) -> Option<Revision> {
        self.ledger
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .recall(submission)
    }

    fn remember(&self, submission: Submission, revision: Revision) {
        self.ledger
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remember(submission, revision);
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
        &self,
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

    /// Rendre le journal, et abandonner tout ce qui vivait en mémoire — `W20.j`.
    ///
    /// Ce que fait un redémarrage, exactement : le journal survit, la mémoire vive non. Le type
    /// existe pour qu'un test puisse jouer ce moment sans mentir dessus — reconstruire une
    /// transaction sur un journal neuf ne prouverait rien, et partager le journal entre deux
    /// transactions vivantes prouverait autre chose.
    pub fn into_store(self) -> S {
        self.store
    }

    /// Le journal, en lecture seule.
    ///
    /// Il n'existe pas d'accesseur mutable, et c'est tout l'objet du type : une projection ou un
    /// diagnostic a besoin de lire, personne n'a besoin d'écrire hors d'ici.
    pub const fn store(&self) -> &S {
        &self.store
    }

    fn submit_atomic<D: Decide>(
        &self,
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

        let Some(stream_id) = events.first().map(|event| event.stream_id.clone()) else {
            return vec![Outcome::Refused(no_events())];
        };

        let verdict = self.serialised(&stream_id, || match self.write(first, events, now) {
            Ok(accepted) => {
                for command in commands {
                    self.remember(Submission::of(command), accepted.revision);
                }
                Outcome::Accepted(accepted)
            }
            Err(refusal) => Outcome::Refused(refusal),
        });

        match verdict {
            Outcome::Accepted(accepted) => commands
                .iter()
                .map(|_| Outcome::Accepted(accepted.clone()))
                .collect(),
            refused @ Outcome::Refused(_) => vec![refused],
        }
    }

    fn write(
        &self,
        command: &CommandEnvelope,
        events: Vec<EventDraft>,
        now: Timestamp,
    ) -> Result<Accepted, CommandError> {
        // Les appelants ont déjà lu le stream pour prendre le verrou, donc ce cas ne peut plus se
        // produire par ce chemin. La garde reste : elle est ce qui rend `AppendError::EmptyBatch`
        // inatteignable, et une passe de mutants l'a confirmé — supprimer cette garde tue, changer
        // le bras d'`EmptyBatch` survit.
        let Some(stream_id) = events.first().map(|event| event.stream_id.clone()) else {
            return Err(no_events());
        };

        // `W20.j` : la clé du client est **apposée ici**, et nulle part ailleurs. Aucun producteur ne
        // la choisit — un handler qui la renseignerait ferait dépendre l'idempotence du client de ce
        // que chaque décideur se trouve écrire, et la portée serait alors invérifiable.
        //
        // Elle est apposée sur **chaque** événement du lot, et non sur le premier seul : la
        // reconstruction lit le journal événement par événement, sans savoir lesquels formaient une
        // écriture. Ne marquer que le premier ferait dépendre le registre reconstruit de l'ordre de
        // lecture.
        let events = events
            .into_iter()
            .map(|mut event| {
                event.idempotency_key = Some(command.idempotency_key().to_owned());
                event
            })
            .collect();

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
/// Le refus d'un handler qui n'a décidé aucun événement.
///
/// Écrit une fois : `submit` et `submit_atomic` doivent connaître le stream **avant** de prendre le
/// verrou, donc les deux rencontrent ce cas au même endroit, et deux formulations divergeraient.
fn no_events() -> CommandError {
    CommandError::Internal {
        detail: "le handler n'a décidé aucun événement : une commande acceptée produit un fait"
            .to_owned(),
    }
}

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
