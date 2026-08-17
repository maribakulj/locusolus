//! L'implémentation de référence, en mémoire.

use std::collections::HashMap;

use locus_protocol::{Id, Timestamp, id::Command};

use crate::envelope::{Draft, Envelope};
use crate::store::{Append, AppendError, Appended, EventStore, Expected, Sequenced};

/// Un journal en mémoire.
///
/// # Ce qu'il est
///
/// L'implémentation **de référence** des garanties de §10.2, et le sujet de la suite de contract
/// tests. Le driver `PostgreSQL` passera la même suite ; c'est elle qui décidera s'il est conforme,
/// pas sa documentation.
///
/// # Ce qu'il n'est pas
///
/// Un journal durable. Il ne survit pas au processus, et c'est assumé : `CLAUDE.md` demande des
/// ports purs avant tout branchement, et une implémentation qui ouvrirait un fichier ici ferait
/// entrer une décision d'infrastructure dans le paquet qui définit le contrat.
///
/// # Immutabilité logique
///
/// §10.2 : « immutabilité logique ». Il n'existe dans ce type **aucune** méthode qui modifie ou
/// supprime un événement écrit — ni `update`, ni `delete`, ni `truncate`, ni `compact`. Un test le
/// vérifie par l'absence, parce que c'est la seule façon de garder vraie une garantie qui se
/// violerait d'une ligne.
#[derive(Debug, Default)]
pub struct MemoryEventStore {
    /// Les streams, par identifiant, dans l'ordre des révisions.
    streams: HashMap<String, Vec<Envelope>>,
    /// L'ordre global d'écriture — l'export brut de §10.2.
    order: Vec<(String, u64)>,
    /// Ce que chaque commande a déjà produit — l'idempotence de §10.2.
    ///
    /// La clé est l'identifiant de commande, la valeur ce qui a été écrit. Garder le résultat
    /// plutôt qu'un simple drapeau permet de **rendre la même réponse** au rejeu, ce qui est ce
    /// qu'attend un appelant qui retente après une coupure.
    applied: HashMap<String, Appended>,
}

impl MemoryEventStore {
    /// Un journal vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Le nombre de streams. Lecture, pour les diagnostics et les tests.
    #[must_use]
    pub fn stream_count(&self) -> usize {
        self.streams.len()
    }

    /// L'empreinte d'un lot, pour distinguer un rejeu d'une réutilisation d'identifiant.
    ///
    /// Sur les brouillons, pas sur les enveloppes scellées : `stream_revision` et `recorded_at`
    /// sont attribués par le journal, donc différents à chaque tentative. Les inclure ferait passer
    /// tout rejeu pour une réutilisation.
    fn fingerprint(events: &[Draft]) -> String {
        events
            .iter()
            .map(|draft| format!("{}|{}", draft.event_id, draft.payload_hash))
            .collect::<Vec<_>>()
            .join(";")
    }

    fn fingerprint_of(applied: &Appended) -> String {
        applied
            .events
            .iter()
            .map(|event| format!("{}|{}", event.event_id, event.payload_hash))
            .collect::<Vec<_>>()
            .join(";")
    }

    fn check(&self, command: &Append) -> Result<(), AppendError> {
        if command.events.is_empty() {
            return Err(AppendError::EmptyBatch);
        }
        for draft in &command.events {
            if draft.stream_id != command.stream_id {
                return Err(AppendError::StreamMismatch {
                    expected: command.stream_id.clone(),
                    found: draft.stream_id.clone(),
                });
            }
        }
        let current = self.revision(&command.stream_id);
        match (command.expected, current) {
            (Expected::NoStream, None) => Ok(()),
            (Expected::Exact(expected), Some(actual)) if expected == actual => Ok(()),
            (expected, actual) => Err(AppendError::Conflict {
                expected,
                actual: actual.unwrap_or(0),
            }),
        }
    }
}

impl EventStore for MemoryEventStore {
    fn append(&mut self, command: Append, recorded_at: Timestamp) -> Result<Appended, AppendError> {
        // L'idempotence d'abord — §10.2. Une commande déjà appliquée rend son résultat d'origine,
        // et surtout pas un conflit : le stream a avancé *à cause d'elle*, et lui opposer sa propre
        // écriture serait le comble de la concurrence optimiste.
        let key = command.command_id.to_string();
        if let Some(previous) = self.applied.get(&key) {
            if Self::fingerprint_of(previous) != Self::fingerprint(&command.events) {
                return Err(AppendError::CommandReused {
                    command_id: command.command_id,
                });
            }
            return Ok(Appended {
                replayed: true,
                ..previous.clone()
            });
        }

        self.check(&command)?;

        let stream = self.streams.entry(command.stream_id.clone()).or_default();
        let mut revision = u64::try_from(stream.len()).unwrap_or(u64::MAX);
        let mut written = Vec::with_capacity(command.events.len());
        for draft in command.events {
            revision += 1;
            let sealed = draft.seal(revision, recorded_at);
            stream.push(sealed.clone());
            self.order.push((command.stream_id.clone(), revision));
            written.push(sealed);
        }

        let result = Appended {
            events: written,
            revision,
            replayed: false,
        };
        self.applied.insert(key, result.clone());
        Ok(result)
    }

    fn read_stream(&self, stream_id: &str, from: u64) -> Vec<Envelope> {
        self.streams
            .get(stream_id)
            .map(|events| {
                events
                    .iter()
                    .filter(|event| event.stream_revision > from)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    fn revision(&self, stream_id: &str) -> Option<u64> {
        self.streams
            .get(stream_id)
            .map(|events| u64::try_from(events.len()).unwrap_or(u64::MAX))
    }

    fn feed(&self, from: u64) -> Vec<Sequenced> {
        self.export()
            .into_iter()
            .enumerate()
            .filter_map(|(index, event)| {
                let position = u64::try_from(index).unwrap_or(u64::MAX) + 1;
                (position > from).then_some(Sequenced { position, event })
            })
            .collect()
    }

    fn export(&self) -> Vec<Envelope> {
        self.order
            .iter()
            .filter_map(|(stream_id, revision)| {
                self.streams.get(stream_id).and_then(|events| {
                    events
                        .iter()
                        .find(|event| event.stream_revision == *revision)
                        .cloned()
                })
            })
            .collect()
    }
}

/// Les commandes déjà appliquées, pour un diagnostic.
///
/// Séparée du trait : c'est une introspection de l'implémentation de référence, pas une garantie
/// que tout journal doit offrir.
impl MemoryEventStore {
    /// Vrai quand cette commande a déjà été appliquée.
    #[must_use]
    pub fn has_applied(&self, command_id: Id<Command>) -> bool {
        self.applied.contains_key(&command_id.to_string())
    }
}
