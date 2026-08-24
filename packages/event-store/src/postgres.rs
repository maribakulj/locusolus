//! Le driver `PostgreSQL` du journal — `W20.i`, ADR 0030.
//!
//! # Ce que ce module doit prouver, et qui en décide
//!
//! Pas sa documentation : la suite de contract tests de `W1.c`. Elle définit ce que « journal » veut
//! dire ici, elle s'exécute **deux fois** — une par backend — et c'est elle qui tranche.
//!
//! # Les garanties sont dans les contraintes, pas dans le code
//!
//! `unique (stream_id, stream_revision)` **est** la concurrence optimiste : deux écrivains qui
//! visent la même révision ne peuvent pas gagner tous les deux, quoi que fasse le code au-dessus.
//! `command_applied.command_id` en clé primaire **est** l'idempotence de §10.2. Une garantie portée
//! par une contrainte survit à une réécriture de la requête ; une garantie portée par un `if` non.
//!
//! # L'ordre global se paie — ADR 0030 décision 2
//!
//! `Sequenced::position` est « le rang dans l'ordre d'écriture global, à partir de 1 », et les
//! projections s'en servent comme filigrane. Une séquence `bigserial` ne le donne pas : les numéros
//! sont attribués **avant** le commit, et les transactions valident dans un ordre quelconque. Un
//! lecteur verrait 1, 2, 4, avancerait son filigrane à 4, et ne verrait **jamais** 3 quand sa
//! transaction valide ensuite — un événement écrit mais invisible aux projections, ce qui est pire
//! qu'une écriture refusée.
//!
//! La position vient donc d'un compteur à **une seule ligne**, incrémenté par `UPDATE … RETURNING`
//! dans la transaction d'écriture. Le verrou de ligne est tenu jusqu'au commit, donc les positions
//! sont attribuées dans l'ordre des commits, sans trou.
//!
//! Cela rend ce driver plus sérialisé que l'ADR 0029 ne le prévoyait — sa documentation annonçait
//! qu'« un driver relationnel s'en remettra au verrouillage de ligne, qui laisse deux streams
//! distincts avancer ensemble ». C'est vrai de la concurrence **optimiste**, et faux de l'**ordre
//! global**. L'ADR 0030 amende la prévision plutôt que de laisser le code la démentir en silence.
//!
//! # Immutabilité logique, tenue par l'absence
//!
//! §10.2. Aucune requête de ce module n'écrit `update`, `delete`, `truncate` ou `drop` sur `event` —
//! le seul `update` porte sur le compteur de position. Un test le vérifie en lisant ce fichier,
//! comme le backend mémoire fait vérifier l'absence de méthode mutante.

use std::sync::{Mutex, PoisonError};

use locus_protocol::{Id, Timestamp, id::Command};
use postgres::{Client, NoTls, Transaction};

use crate::envelope::{Actor, ActorKind, Draft, Envelope, EventType};
use crate::store::{Append, AppendError, Appended, EventStore, Expected, Sequenced};

/// Le schéma, créé s'il manque.
///
/// `if not exists` plutôt qu'un système de migrations : ce driver n'est câblé nulle part, donc il
/// n'existe aucune donnée à faire évoluer. Le jour où il y en aura, la migration sera le sujet de
/// l'item qui le câblera — l'écrire maintenant produirait un mécanisme que rien n'éprouve.
const SCHEMA: &str = "
create table if not exists event (
    position            bigint       not null,
    stream_id           text         not null,
    stream_revision     bigint       not null,
    event_id            text         not null,
    event_type          text         not null,
    schema_version      bigint       not null,
    workspace_id        text         not null,
    project_id          text         not null,
    program_id          text,
    branch_id           text,
    actor_principal_id  text         not null,
    actor_kind          text         not null,
    actor_delegation_id text,
    occurred_at         bigint       not null,
    recorded_at         bigint       not null,
    causation_id        text         not null,
    correlation_id      text,
    trace_id            text,
    payload             jsonb        not null,
    payload_hash        text         not null,
    constraint event_position_unique unique (position),
    constraint event_stream_revision_unique unique (stream_id, stream_revision)
);

create table if not exists command_applied (
    command_id     text   primary key,
    stream_id      text   not null,
    first_revision bigint not null,
    revision       bigint not null,
    fingerprint    text   not null
);

create table if not exists journal_position (
    only_row boolean primary key default true,
    next     bigint  not null,
    constraint journal_position_single check (only_row)
);

insert into journal_position (only_row, next) values (true, 1) on conflict do nothing;
";

/// Ce qui a mal tourné avant même de pouvoir écrire.
///
/// Distinct d'[`AppendError`] exprès : celui-ci dit « le journal a refusé », celui-là « je n'ai pas
/// pu lui parler ». Les deux envoient chercher à des endroits opposés, et un driver qui les
/// fondrait ferait relire une commande à qui doit vérifier un réseau.
#[derive(Debug)]
pub enum ConnectError {
    /// La chaîne de connexion, la base, ou le réseau.
    Unreachable(String),
    /// Le schéma n'a pas pu être posé.
    Schema(String),
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreachable(detail) => write!(formatter, "base injoignable : {detail}"),
            Self::Schema(detail) => write!(formatter, "schéma non posé : {detail}"),
        }
    }
}

impl std::error::Error for ConnectError {}

/// Un journal sur `PostgreSQL`.
///
/// # Un client sous `Mutex`, et pourquoi ce n'est pas le verrou global du backend mémoire
///
/// `postgres::Client` n'est pas partageable entre fils, et le port prend `&self`. Le `Mutex`
/// sérialise donc l'**usage de la connexion**, pas les écritures : deux instances de ce type sur la
/// même base écrivent en parallèle, et c'est la base qui arbitre — par ses contraintes.
///
/// Une réserve de connexions serait l'étape suivante, et elle n'est pas ici : elle demanderait un
/// paquet de plus, elle ne change rien à ce que la suite de contract tests éprouve, et rien ne l'a
/// mesurée nécessaire. `CLAUDE.md` : simplicité avant abstraction spéculative.
pub struct PostgresEventStore {
    client: Mutex<Client>,
}

/// `postgres::Client` n'implémente pas `Debug` — une connexion porte des identifiants, et son
/// auteur a choisi de ne pas les rendre imprimables par accident. C'est le bon choix, et ce dépôt
/// l'aurait fait aussi : `CLAUDE.md` interdit de journaliser une créance. La forme écrite ici ne
/// révèle donc rien de la connexion.
impl std::fmt::Debug for PostgresEventStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PostgresEventStore")
            .finish_non_exhaustive()
    }
}

impl PostgresEventStore {
    /// Se connecter et poser le schéma.
    ///
    /// # Errors
    ///
    /// [`ConnectError::Unreachable`] si la base ne répond pas, [`ConnectError::Schema`] si le schéma
    /// ne se pose pas.
    pub fn connect(url: &str) -> Result<Self, ConnectError> {
        let mut client = Client::connect(url, NoTls)
            .map_err(|error| ConnectError::Unreachable(error.to_string()))?;
        client
            .batch_execute(SCHEMA)
            .map_err(|error| ConnectError::Schema(error.to_string()))?;
        Ok(Self {
            client: Mutex::new(client),
        })
    }

    /// Effacer le journal — **réservé aux tests**, et le nom le dit.
    ///
    /// Ce n'est pas une brèche dans l'immutabilité de §10.2 : la garantie porte sur ce qu'un
    /// **appelant du port** peut faire, et `EventStore` n'offre rien de tel. Une suite de contract
    /// tests a besoin d'un journal vide entre deux cas, et la seule alternative — une base par test —
    /// coûterait plus qu'elle ne protège.
    ///
    /// # Errors
    ///
    /// [`ConnectError::Unreachable`] si la base ne répond pas.
    pub fn truncate_for_tests(&self) -> Result<(), ConnectError> {
        self.locked()
            .batch_execute(
                "truncate table event; truncate table command_applied; update journal_position set next = 1;",
            )
            .map_err(|error| ConnectError::Unreachable(error.to_string()))
    }

    fn locked(&self) -> std::sync::MutexGuard<'_, Client> {
        self.client.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// L'empreinte d'un lot — la même règle que le backend mémoire.
    ///
    /// Sur les brouillons, pas sur les enveloppes scellées : `stream_revision` et `recorded_at` sont
    /// attribués par le journal, donc différents à chaque tentative. Les inclure ferait passer tout
    /// rejeu pour une réutilisation d'identifiant.
    fn fingerprint(events: &[Draft]) -> String {
        events
            .iter()
            .map(|draft| format!("{}|{}", draft.event_id, draft.payload_hash))
            .collect::<Vec<_>>()
            .join(";")
    }
}

/// Une panne de base pendant une écriture.
///
/// Elle ne peut pas devenir un [`AppendError`] — aucune de ses variantes ne veut dire « la base est
/// tombée », et en choisir une ferait relire sa commande à un appelant dont la requête était juste.
/// Elle panique donc, comme le backend mémoire panique sur un verrou irrécupérable : c'est une
/// panne d'infrastructure, pas un refus.
fn fatal<T>(context: &str, error: &postgres::Error) -> T {
    panic!("journal PostgreSQL — {context} : {error}");
}

impl EventStore for PostgresEventStore {
    fn append(&self, command: Append, recorded_at: Timestamp) -> Result<Appended, AppendError> {
        let mut client = self.locked();
        let mut transaction = client
            .transaction()
            .unwrap_or_else(|error| fatal("ouverture de transaction", &error));

        // L'idempotence d'abord — §10.2, et le même ordre que le backend mémoire. Une commande déjà
        // appliquée rend son résultat d'origine, et surtout pas un conflit : le stream a avancé *à
        // cause d'elle*, et lui opposer sa propre écriture serait le comble de la concurrence
        // optimiste.
        let key = command.command_id.to_string();
        let empreinte = PostgresEventStore::fingerprint(&command.events);
        if let Some(precedente) = applied(&mut transaction, &key) {
            if precedente.fingerprint != empreinte {
                return Err(AppendError::CommandReused {
                    command_id: command.command_id,
                });
            }
            // La **plage de révisions**, enregistrée au moment de l'écriture, et non une
            // reconstruction. Une première rédaction relisait le stream et filtrait par
            // `causation_id` : elle a rendu un lot **vide** au rejeu, et surtout elle demandait au
            // journal de redécouvrir ce qu'il savait déjà. Ce qu'une commande a produit est un fait
            // à enregistrer, pas à déduire.
            let events = stream_events(
                &mut transaction,
                &precedente.stream_id,
                precedente.first_revision.saturating_sub(1),
            )
            .into_iter()
            .filter(|event| event.stream_revision <= precedente.revision)
            .collect();
            return Ok(Appended {
                events,
                revision: precedente.revision,
                replayed: true,
            });
        }

        check(&mut transaction, &command)?;

        let existing = revision_of(&mut transaction, &command.stream_id).unwrap_or(0);

        // La position est réservée **maintenant**, sous le verrou de ligne du compteur, et tenue
        // jusqu'au commit : c'est ce qui rend l'ordre global sans trou (ADR 0030 décision 2).
        let combien = u64::try_from(command.events.len()).unwrap_or(0);
        let premiere = reserve_positions(&mut transaction, combien);

        let mut written = Vec::with_capacity(command.events.len());
        let mut revision = existing;
        for (rang, draft) in command.events.into_iter().enumerate() {
            revision += 1;
            let sealed = draft.seal(revision, recorded_at);
            let position = premiere + u64::try_from(rang).unwrap_or(0);
            insert(&mut transaction, position, &sealed);
            written.push(sealed);
        }

        transaction
            .execute(
                "insert into command_applied (command_id, stream_id, first_revision, revision, fingerprint) \
                 values ($1, $2, $3, $4, $5)",
                &[
                    &key,
                    &command.stream_id,
                    &i64::try_from(existing + 1).unwrap_or(i64::MAX),
                    &i64::try_from(revision).unwrap_or(i64::MAX),
                    &empreinte,
                ],
            )
            .unwrap_or_else(|error| fatal("enregistrement de la commande", &error));

        transaction
            .commit()
            .unwrap_or_else(|error| fatal("commit", &error));

        Ok(Appended {
            events: written,
            revision,
            replayed: false,
        })
    }

    fn read_stream(&self, stream_id: &str, from: u64) -> Vec<Envelope> {
        let mut client = self.locked();
        let mut transaction = client
            .transaction()
            .unwrap_or_else(|error| fatal("lecture de stream", &error));
        stream_events(&mut transaction, stream_id, from)
    }

    fn revision(&self, stream_id: &str) -> Option<u64> {
        let mut client = self.locked();
        let mut transaction = client
            .transaction()
            .unwrap_or_else(|error| fatal("lecture de révision", &error));
        revision_of(&mut transaction, stream_id)
    }

    fn feed(&self, from: u64) -> Vec<Sequenced> {
        let mut client = self.locked();
        let rows = client
            .query(
                "select position, stream_id, stream_revision, event_id, event_type, schema_version, \
                 workspace_id, project_id, program_id, branch_id, actor_principal_id, actor_kind, \
                 actor_delegation_id, occurred_at, recorded_at, causation_id, correlation_id, \
                 trace_id, payload, payload_hash from event where position > $1 order by position",
                &[&i64::try_from(from).unwrap_or(i64::MAX)],
            )
            .unwrap_or_else(|error| fatal("lecture du flux", &error));
        rows.iter()
            .map(|row| Sequenced {
                position: u64::try_from(row.get::<_, i64>(0)).unwrap_or(0),
                event: envelope_of(row),
            })
            .collect()
    }

    fn export(&self) -> Vec<Envelope> {
        self.feed(0)
            .into_iter()
            .map(|sequenced| sequenced.event)
            .collect()
    }
}

/// Ce qu'une commande déjà appliquée a produit — la plage exacte, pas un indice.
#[derive(Debug)]
struct Appliquee {
    stream_id: String,
    first_revision: u64,
    revision: u64,
    fingerprint: String,
}

fn applied(transaction: &mut Transaction<'_>, key: &str) -> Option<Appliquee> {
    let rows = transaction
        .query(
            "select stream_id, first_revision, revision, fingerprint from command_applied \
             where command_id = $1",
            &[&key],
        )
        .unwrap_or_else(|error| fatal("lecture d'idempotence", &error));
    rows.first().map(|row| Appliquee {
        stream_id: row.get::<_, String>(0),
        first_revision: u64::try_from(row.get::<_, i64>(1)).unwrap_or(0),
        revision: u64::try_from(row.get::<_, i64>(2)).unwrap_or(0),
        fingerprint: row.get::<_, String>(3),
    })
}

/// Confronter une écriture à l'état courant — les trois refus de §10.2, dans l'ordre du port.
fn check(transaction: &mut Transaction<'_>, command: &Append) -> Result<(), AppendError> {
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
    let current = revision_of(transaction, &command.stream_id);
    match (command.expected, current) {
        (Expected::NoStream, None) => Ok(()),
        (Expected::Exact(expected), Some(actual)) if expected == actual => Ok(()),
        (expected, actual) => Err(AppendError::Conflict {
            expected,
            actual: actual.unwrap_or(0),
        }),
    }
}

fn revision_of(transaction: &mut Transaction<'_>, stream_id: &str) -> Option<u64> {
    let rows = transaction
        .query(
            "select max(stream_revision) from event where stream_id = $1",
            &[&stream_id],
        )
        .unwrap_or_else(|error| fatal("lecture de révision", &error));
    rows.first()
        .and_then(|row| row.get::<_, Option<i64>>(0))
        .map(|revision| u64::try_from(revision).unwrap_or(0))
}

/// Réserver `combien` positions consécutives, et tenir le verrou jusqu'au commit.
fn reserve_positions(transaction: &mut Transaction<'_>, combien: u64) -> u64 {
    let rows = transaction
        .query(
            "update journal_position set next = next + $1 where only_row returning next - $1",
            &[&i64::try_from(combien).unwrap_or(0)],
        )
        .unwrap_or_else(|error| fatal("réservation de position", &error));
    rows.first()
        .map_or(1, |row| u64::try_from(row.get::<_, i64>(0)).unwrap_or(1))
}

fn insert(transaction: &mut Transaction<'_>, position: u64, event: &Envelope) {
    transaction
        .execute(
            "insert into event (position, stream_id, stream_revision, event_id, event_type, \
             schema_version, workspace_id, project_id, program_id, branch_id, actor_principal_id, \
             actor_kind, actor_delegation_id, occurred_at, recorded_at, causation_id, \
             correlation_id, trace_id, payload, payload_hash) values \
             ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20)",
            &[
                &i64::try_from(position).unwrap_or(i64::MAX),
                &event.stream_id,
                &i64::try_from(event.stream_revision).unwrap_or(i64::MAX),
                &event.event_id.to_string(),
                &event.event_type.to_string(),
                &i64::from(event.schema_version),
                &event.workspace_id.to_string(),
                &event.project_id.to_string(),
                &event.program_id.map(|id| id.to_string()),
                &event.branch_id.map(|id| id.to_string()),
                &event.actor.principal_id.to_string(),
                &kind_name(event.actor.kind),
                &event.actor.delegation_id.map(|id| id.to_string()),
                &event.occurred_at.millis(),
                &event.recorded_at.millis(),
                &event.causation_id.to_string(),
                &event.correlation_id.map(|id| id.to_string()),
                &event.trace_id,
                &event.payload,
                &event.payload_hash,
            ],
        )
        .unwrap_or_else(|error| fatal("insertion d'événement", &error));
}

fn stream_events(transaction: &mut Transaction<'_>, stream_id: &str, from: u64) -> Vec<Envelope> {
    let rows = transaction
        .query(
            "select position, stream_id, stream_revision, event_id, event_type, schema_version, \
             workspace_id, project_id, program_id, branch_id, actor_principal_id, actor_kind, \
             actor_delegation_id, occurred_at, recorded_at, causation_id, correlation_id, \
             trace_id, payload, payload_hash from event where stream_id = $1 and stream_revision > $2 \
             order by stream_revision",
            &[&stream_id, &i64::try_from(from).unwrap_or(i64::MAX)],
        )
        .unwrap_or_else(|error| fatal("lecture de stream", &error));
    rows.iter().map(envelope_of).collect()
}

fn kind_name(kind: ActorKind) -> String {
    match kind {
        ActorKind::Human => "human",
        ActorKind::Agent => "agent",
        ActorKind::System => "system",
    }
    .to_owned()
}

/// Relire une nature d'acteur.
///
/// Une valeur inconnue **panique** plutôt que de retomber sur `System` : un acteur mal relu ferait
/// lire « le système l'a fait » sur l'acte d'un humain, et l'invariant de traçabilité de §10.1
/// vaudrait alors moins que rien. La colonne n'est écrite que par [`kind_name`], donc ce chemin
/// suppose une base modifiée à la main.
fn kind_of(name: &str) -> ActorKind {
    match name {
        "human" => ActorKind::Human,
        "agent" => ActorKind::Agent,
        "system" => ActorKind::System,
        other => panic!(
            "journal PostgreSQL — nature d'acteur « {other} » inconnue : la relire en `system` \
             ferait lire « le système l'a fait » sur l'acte d'un humain"
        ),
    }
}

fn parsed<K: locus_protocol::IdKind>(text: &str) -> Id<K> {
    Id::parse(text).unwrap_or_else(|error| {
        panic!("journal PostgreSQL — identifiant « {text} » illisible : {error}")
    })
}

fn envelope_of(row: &postgres::Row) -> Envelope {
    Envelope {
        event_id: parsed(&row.get::<_, String>(3)),
        event_type: EventType::parse(&row.get::<_, String>(4)).unwrap_or_else(|error| {
            panic!("journal PostgreSQL — type d'événement illisible : {error}")
        }),
        schema_version: u32::try_from(row.get::<_, i64>(5)).unwrap_or(1),
        stream_id: row.get::<_, String>(1),
        stream_revision: u64::try_from(row.get::<_, i64>(2)).unwrap_or(0),
        workspace_id: parsed(&row.get::<_, String>(6)),
        project_id: parsed(&row.get::<_, String>(7)),
        program_id: row.get::<_, Option<String>>(8).map(|id| parsed(&id)),
        branch_id: row.get::<_, Option<String>>(9).map(|id| parsed(&id)),
        actor: Actor {
            principal_id: parsed(&row.get::<_, String>(10)),
            kind: kind_of(&row.get::<_, String>(11)),
            delegation_id: row.get::<_, Option<String>>(12).map(|id| parsed(&id)),
        },
        occurred_at: Timestamp::from_millis(row.get::<_, i64>(13)),
        recorded_at: Timestamp::from_millis(row.get::<_, i64>(14)),
        causation_id: parsed::<Command>(&row.get::<_, String>(15)),
        correlation_id: row.get::<_, Option<String>>(16).map(|id| parsed(&id)),
        trace_id: row.get::<_, Option<String>>(17),
        payload: row.get::<_, serde_json::Value>(18),
        payload_hash: row.get::<_, String>(19),
    }
}
