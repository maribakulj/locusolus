//! L'implémentation de référence, en mémoire.

use std::collections::HashMap;
use std::sync::{PoisonError, RwLock};

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
///
/// # La concurrence est **à l'intérieur** — ADR 0029 décision 1
///
/// [`EventStore::append`] prend `&self` : chaque backend possède sa propre concurrence, et le trait
/// l'exige plutôt que de l'espérer. Celui-ci se protège par un verrou interne — il **est**
/// globalement sérialisé, et il le dit.
///
/// Le verrou vit ici et non au-dessus parce que `read_stream`, `revision` et `feed` prennent déjà
/// `&self` : un verrou placé au-dessus du journal bloquerait **toutes** les lectures pendant une
/// écriture. En mémoire cela ne se verrait pas ; avec un driver distant, une requête de lecture
/// attendrait qu'une écriture d'entrée/sortie finisse. Ce serait créer un goulot que le stockage n'a
/// pas — `Expected` est par stream, et deux streams n'ont aucune raison de s'attendre.
#[derive(Debug, Default)]
pub struct MemoryEventStore {
    journal: RwLock<Journal>,
}

/// L'état protégé.
///
/// Séparé du type public pour qu'aucun champ ne soit atteignable sans passer par le verrou : c'est
/// la même forme que `Transaction`, qui possède le journal dans un champ privé sans accesseur.
#[derive(Debug, Default)]
struct Journal {
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
        self.read().streams.len()
    }

    /// Le journal, en lecture.
    ///
    /// # L'empoisonnement se récupère, et c'est sûr **parce que** la section critique ne panique pas
    ///
    /// Un verrou empoisonné signale qu'un porteur a paniqué. Refuser de servir ensuite ferait d'une
    /// panique isolée un arrêt du journal entier. On ne peut le récupérer sans danger que si aucune
    /// écriture ne peut laisser un état partiel — et c'est le cas : [`EventStore::append`] scelle
    /// tous ses événements dans des variables locales, puis les remet au journal en un seul bloc qui
    /// ne peut pas paniquer. Voir le commentaire qui y est.
    fn read(&self) -> std::sync::RwLockReadGuard<'_, Journal> {
        self.journal.read().unwrap_or_else(PoisonError::into_inner)
    }

    /// Le journal, en écriture.
    fn write(&self) -> std::sync::RwLockWriteGuard<'_, Journal> {
        self.journal.write().unwrap_or_else(PoisonError::into_inner)
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
}

impl Journal {
    fn revision_of(&self, stream_id: &str) -> Option<u64> {
        self.streams
            .get(stream_id)
            .map(|events| u64::try_from(events.len()).unwrap_or(u64::MAX))
    }

    /// Confronter une écriture à l'état courant.
    ///
    /// # `&mut self` alors que rien n'est modifié, et c'est le point
    ///
    /// Le `&mut` n'est pas là pour muter : il est un **marqueur de capacité**. Il rend
    /// `check`-puis-écrire sous deux verrous différents — le défaut « check-then-act », celui qui
    /// laisse deux écritures passer la même vérification — **non compilable**, puisqu'un verrou de
    /// lecture ne donne qu'un `&Journal`.
    ///
    /// La raison de ce choix est mesurée. Un test de concurrence a d'abord été écrit pour attraper
    /// cette course : il l'a manquée entièrement, puis, avec un rendez-vous, ne l'a attrapée que
    /// deux fois sur trois, puis quatre fois sur dix en augmentant la contention. Une garde qui
    /// n'attrape qu'à moitié n'est pas une garde, et la détection dépendait de l'ordonnanceur, donc
    /// de la machine. Rendre la faute **inexprimable** vaut mieux que la chercher : c'est la même
    /// règle que le dépôt applique partout ailleurs sous le nom de « tenu par l'absence ».
    fn check(&mut self, command: &Append) -> Result<(), AppendError> {
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
        let current = self.revision_of(&command.stream_id);
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
    fn append(&self, command: Append, recorded_at: Timestamp) -> Result<Appended, AppendError> {
        let mut journal = self.write();
        // L'idempotence d'abord — §10.2. Une commande déjà appliquée rend son résultat d'origine,
        // et surtout pas un conflit : le stream a avancé *à cause d'elle*, et lui opposer sa propre
        // écriture serait le comble de la concurrence optimiste.
        let key = command.command_id.to_string();
        if let Some(previous) = journal.applied.get(&key) {
            if MemoryEventStore::fingerprint_of(previous)
                != MemoryEventStore::fingerprint(&command.events)
            {
                return Err(AppendError::CommandReused {
                    command_id: command.command_id,
                });
            }
            return Ok(Appended {
                replayed: true,
                ..previous.clone()
            });
        }

        journal.check(&command)?;

        // Tout est scellé dans des variables locales **avant** que le journal soit touché. Ce n'est
        // pas une élégance : c'est ce qui rend la récupération d'un verrou empoisonné sûre, en
        // rendant inexprimable un état où une partie du lot serait écrite. C'est la règle
        // « l'échec ne laisse rien » de la transaction, appliquée un étage plus bas.
        let existing = journal.revision_of(&command.stream_id).unwrap_or(0);
        let mut written = Vec::with_capacity(command.events.len());
        let mut revision = existing;
        for draft in command.events {
            revision += 1;
            written.push(draft.seal(revision, recorded_at));
        }

        let stream = journal
            .streams
            .entry(command.stream_id.clone())
            .or_default();
        stream.extend(written.iter().cloned());
        for sealed in &written {
            journal
                .order
                .push((command.stream_id.clone(), sealed.stream_revision));
        }

        let result = Appended {
            events: written,
            revision,
            replayed: false,
        };
        journal.applied.insert(key, result.clone());
        Ok(result)
    }

    fn read_stream(&self, stream_id: &str, from: u64) -> Vec<Envelope> {
        self.read()
            .streams
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
        self.read().revision_of(stream_id)
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
        let journal = self.read();
        journal
            .order
            .iter()
            .filter_map(|(stream_id, revision)| {
                journal.streams.get(stream_id).and_then(|events| {
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
        self.read().applied.contains_key(&command_id.to_string())
    }
}
