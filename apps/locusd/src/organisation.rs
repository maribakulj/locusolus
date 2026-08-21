//! Le commit d'une version de coordination — `W17.i`, le producteur qui manquait.
//!
//! # Ce que cet item ferme
//!
//! `packages/coordination` savait produire une `Version` et la faire évoluer par `apply`, et
//! **rien n'écrivait le résultat**. Un commit rendait une valeur que l'appelant gardait pour lui :
//! le journal n'en portait aucune trace, donc aucun résolveur ne pouvait relire une `VersionId`, et
//! `/branches/{id}/diff` n'avait pas de quoi répondre. `W17.j` en dépend directement.
//!
//! # L'opération voyage sous sa forme canonique
//!
//! Et ce n'est pas une économie. Les octets écrits dans le journal sont **exactement** ceux sur
//! lesquels le condensat a été calculé : un lecteur relit ce qui a été signé, et non une seconde
//! représentation dont il faudrait prouver qu'elle dit la même chose. `Operation::parse` est
//! l'inverse exact de `Operation::canonical`, et un test tient les dix.
//!
//! Cette forme n'était pas analysable il y a peu : un rôle portant une tabulation forgeait un champ,
//! et un rôle nommé `-` était indistinguable d'une absence. Le durcissement contre l'injection l'a
//! rendue non ambiguë, et ce module en est le bénéficiaire direct — la correction d'un défaut a
//! ouvert la voie qu'elle ne cherchait pas.
//!
//! # Aucun magasin, et la révision vient du journal
//!
//! ADR 0016 décision 5 : « aucun compteur, aucun magasin, aucun bus n'est créé ». La révision d'une
//! version **est** la révision de stream que le journal attribue, et la concurrence optimistique est
//! celle de `Expected` — un commit sur une base périmée est refusé **avant** d'écrire, par le même
//! mécanisme que toute autre commande. Ce module ne retient rien.

use locus_coordination::version::{
    Digest, Operation, ParseOperationError, Version, VersionError, VersionId,
};
use locus_coordination::{CoordinationMode, Relation, RelationKind};
use locus_event_store::{Actor, ActorKind, Draft as EventDraft, EventType};
use locus_protocol::id::{Branch, Event, Project};
use locus_protocol::{Id, Timestamp};

use crate::command::CommandEnvelope;
use crate::error::CommandError;
use crate::handler::Decide;

/// Le type d'événement d'un commit de coordination.
///
/// `team` est la famille de §10.3, et `team.modify` la commande de §22.3 : l'événement est ce que la
/// commande produit, au passé, comme `branch.validated` l'est de `branch.merge.apply`.
const MODIFIED: &str = "team.modified";

/// Le type d'événement qui **fonde** un stream d'organisation.
///
/// `team.create` est la commande de §22.3 ; ceci en est le fait. Sans lui, `W17.i` devait recevoir
/// sa racine de l'appelant, et une route HTTP — qui n'a qu'un identifiant de branche dans son
/// chemin — n'aurait eu nulle part où la prendre. Le mettre dans le journal est ce qui rend le
/// résolveur possible **sans magasin**.
const CREATED: &str = "team.created";

/// Le stream d'une organisation.
///
/// **Une organisation par branche**, et l'argument est la route qui a motivé toute la chaîne :
/// §22.4 sert `/branches/{id}/diff`, donc le graphe de coordination qu'on y compare est celui d'une
/// branche. Le ranger ailleurs obligerait cette route à une jointure pour retrouver ce qu'elle a
/// déjà dans son chemin.
#[must_use]
pub fn stream_of(branch: Id<Branch>) -> String {
    format!("organisation/{branch}")
}

/// Ce qu'un commit a besoin de savoir et que le domaine ne porte pas.
///
/// Même lacune que `BranchContext` et `MessageContext`, nommée de la même façon : `EventDraft` exige
/// un `event_id` et un `project_id`, et rien dans le domaine ne fabrique d'identifiants.
pub struct OrganisationContext {
    /// La branche dont c'est l'organisation.
    pub branch_id: Id<Branch>,
    /// Le projet auquel l'événement appartient.
    pub project_id: Id<Project>,
    /// L'identité de l'événement à écrire.
    pub event_id: Id<Event>,
    /// Quand l'acte a eu lieu — distinct de l'instant d'écriture (§10.1).
    pub occurred_at: Timestamp,
    /// Le hash de la charge canonicalisée.
    pub payload_hash: String,
}

/// Fonder l'organisation d'une branche.
///
/// # Pourquoi la racine est un fait et non une convention
///
/// `W17.i` laissait la racine à l'appelant, en nommant la lacune plutôt qu'en choisissant un mode de
/// coordination par défaut que §14.3 ne donne pas. La lacune se comble ici, et par le seul moyen qui
/// n'invente rien : **quelqu'un déclare** la composition initiale, et cette déclaration est un fait.
/// Le mode vient de qui fonde l'organisation, pas d'un défaut de ce module.
pub struct Create<D: Digest> {
    /// La composition initiale.
    pub root: Version,
    /// De quoi vérifier qu'elle est bien scellée par le même condensat.
    pub digest: D,
}

impl<D: Digest> Decide for Create<D> {
    type State = OrganisationContext;

    fn decide(
        &self,
        command: &CommandEnvelope,
        context: &Self::State,
    ) -> Result<Vec<EventDraft>, CommandError> {
        Ok(vec![created_event(command, context, &self.root)])
    }
}

/// Commiter une opération sur la version courante.
pub struct Commit<D: Digest> {
    /// La version sur laquelle l'opération s'applique.
    pub base: Version,
    /// Ce qui est commité.
    pub operation: Operation,
    /// De quoi sceller la version produite.
    pub digest: D,
}

impl<D: Digest> Decide for Commit<D> {
    type State = OrganisationContext;

    fn decide(
        &self,
        command: &CommandEnvelope,
        context: &Self::State,
    ) -> Result<Vec<EventDraft>, CommandError> {
        let produced = self
            .base
            .apply(&self.operation, &self.digest)
            .map_err(|refusal| refusal_of(&refusal))?;
        Ok(vec![modified_event(
            command,
            context,
            &self.operation,
            &produced,
        )])
    }
}

/// Le refus d'une opération, sous sa famille de §22.5.
///
/// Une opération inapplicable est une **politique** du domaine qui s'y oppose — retirer un nœud qui
/// porte encore une arête, poser un rôle sur un absent —, pas une requête mal écrite. Lui rendre
/// `validation` enverrait le client relire sa requête, où il ne trouverait rien. Même arbitrage que
/// pour les transitions de branche.
fn refusal_of(error: &VersionError) -> CommandError {
    CommandError::Policy {
        policy: "coordination.operation".to_owned(),
        detail: error.to_string(),
    }
}

/// Le fait qu'un commit produit.
///
/// La charge porte **l'opération** et **ce qu'elle a produit** : un lecteur du journal reconstruit
/// la version en rejouant, et vérifie qu'il est arrivé au bon endroit en comparant le condensat. Les
/// deux sont nécessaires — l'opération seule ne se vérifie pas, le résultat seul ne se rejoue pas.
fn modified_event(
    command: &CommandEnvelope,
    context: &OrganisationContext,
    operation: &Operation,
    produced: &Version,
) -> EventDraft {
    EventDraft {
        event_id: context.event_id,
        event_type: EventType::parse(MODIFIED).unwrap_or_else(|_| {
            unreachable!(
                "« {MODIFIED} » est un littéral de ce module, et `team` est un namespace de §10.3"
            )
        }),
        schema_version: 1,
        stream_id: stream_of(context.branch_id),
        workspace_id: *command.workspace_id(),
        project_id: context.project_id,
        program_id: None,
        branch_id: Some(context.branch_id),
        actor: Actor {
            principal_id: *command.actor_principal_id(),
            kind: ActorKind::Agent,
            delegation_id: command.delegation_id().copied(),
        },
        occurred_at: context.occurred_at,
        causation_id: *command.command_id(),
        correlation_id: command.correlation_id().copied(),
        trace_id: None,
        payload: serde_json::json!({
            "operation": operation.canonical(),
            "version": produced.id().to_string(),
            "content": produced.content_hash().to_string(),
        }),
        payload_hash: context.payload_hash.clone(),
    }
}

/// Le fait qui fonde une organisation.
///
/// La charge porte la composition **en clair** — membres, relations, mode, coordinateur — et non la
/// forme canonique du contenu. Le choix diffère de celui des opérations, et il se défend : la forme
/// canonique d'un contenu est un texte à lignes **trié**, dont la relecture demanderait un second
/// analyseur pour un seul usage. Les opérations, elles, se relisent par milliers.
///
/// Le condensat accompagne la composition pour que le lecteur vérifie qu'il a reconstruit la bonne
/// racine, plutôt que de le croire.
fn created_event(
    command: &CommandEnvelope,
    context: &OrganisationContext,
    root: &Version,
) -> EventDraft {
    EventDraft {
        event_id: context.event_id,
        event_type: EventType::parse(CREATED).unwrap_or_else(|_| {
            unreachable!(
                "« {CREATED} » est un littéral de ce module, et `team` est un namespace de §10.3"
            )
        }),
        schema_version: 1,
        stream_id: stream_of(context.branch_id),
        workspace_id: *command.workspace_id(),
        project_id: context.project_id,
        program_id: None,
        branch_id: Some(context.branch_id),
        actor: Actor {
            principal_id: *command.actor_principal_id(),
            kind: ActorKind::Agent,
            delegation_id: command.delegation_id().copied(),
        },
        occurred_at: context.occurred_at,
        causation_id: *command.command_id(),
        correlation_id: command.correlation_id().copied(),
        trace_id: None,
        payload: serde_json::json!({
            "members": root.members().iter().map(ToString::to_string).collect::<Vec<_>>(),
            "relations": root.relations().iter().map(edge_on_the_wire).collect::<Vec<_>>(),
            "mode": root.mode().slug(),
            "coordinator": root.coordinator().map(ToString::to_string),
            "version": root.id().to_string(),
            "content": root.content_hash().to_string(),
        }),
        payload_hash: context.payload_hash.clone(),
    }
}

/// Une arête sur le fil — la même forme que dans une opération, pour qu'il n'y ait qu'une grammaire.
fn edge_on_the_wire(relation: &Relation) -> String {
    format!("{}>{}>{}", relation.from, relation.kind, relation.to)
}

/// Reconstruire la racine depuis le fait qui l'a fondée.
///
/// # Errors
///
/// [`ReplayError::Unreadable`] quand un champ manque ou ne se lit pas, et quand le condensat
/// reconstruit **ne correspond pas** à celui que le fait annonce — reconstruire un contenu qu'on ne
/// peut pas reconnaître ne prouve rien, et c'est ce que `W17.j` demande explicitement.
fn root_from(payload: &serde_json::Value, digest: &impl Digest) -> Result<Version, ReplayError> {
    let unreadable = |detail: &str| ReplayError::Unreadable {
        position: 0,
        detail: detail.to_owned(),
    };

    let members = payload
        .get("members")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| unreadable("aucun champ « members »"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .and_then(|text| Id::parse(text).ok())
                .ok_or_else(|| unreadable("membre illisible"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let relations = payload
        .get("relations")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| unreadable("aucun champ « relations »"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| unreadable("arête illisible"))
                .and_then(|text| {
                    relation_from_wire(text).ok_or_else(|| unreadable("arête illisible"))
                })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mode = payload
        .get("mode")
        .and_then(serde_json::Value::as_str)
        .and_then(CoordinationMode::parse)
        .ok_or_else(|| unreadable("mode de coordination illisible"))?;

    let coordinator = match payload.get("coordinator") {
        None | Some(serde_json::Value::Null) => None,
        Some(value) => Some(
            value
                .as_str()
                .and_then(|text| Id::parse(text).ok())
                .ok_or_else(|| unreadable("coordinateur illisible"))?,
        ),
    };

    let root = Version::root(&members, &relations, mode, coordinator, digest)
        .map_err(|because| unreadable(&because.to_string()))?;

    // Vérifié, pas supposé : le fait annonce une identité, et une racine reconstruite qui n'est pas
    // celle-là est une racine qu'on ne peut pas reconnaître.
    let annonce = payload
        .get("version")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| unreadable("aucun champ « version »"))?;
    if root.id().to_string() != annonce {
        return Err(unreadable(&format!(
            "la racine reconstruite est {} et le fait annonce {annonce}",
            root.id()
        )));
    }
    Ok(root)
}

fn relation_from_wire(text: &str) -> Option<Relation> {
    let parts: Vec<&str> = text.split('>').collect();
    let [from, kind, to] = parts.as_slice() else {
        return None;
    };
    Some(Relation {
        from: Id::parse(from).ok()?,
        to: Id::parse(to).ok()?,
        kind: RelationKind::parse(kind)?,
    })
}

/// Résoudre l'organisation courante d'une branche depuis son stream **entier**.
///
/// Le premier événement fonde, les suivants modifient. C'est ce qui rend `W17.j` possible sans
/// magasin : une route HTTP n'a qu'un identifiant de branche, et il lui suffit.
///
/// # Errors
///
/// [`ReplayError::Empty`] quand le stream n'a aucun fait — une branche sans organisation n'est pas
/// une organisation vide, et rendre une racine plausible serait exactement ce que `W17.j` interdit ;
/// les autres variantes comme [`replay`].
pub fn resolve(
    payloads: &[serde_json::Value],
    digest: &impl Digest,
) -> Result<Version, ReplayError> {
    let (first, rest) = payloads.split_first().ok_or(ReplayError::Empty)?;
    let root = root_from(first, digest)?;
    replay(&root, rest, digest)
}

/// Résoudre **une version nommée** dans le stream d'une branche.
///
/// Le rejeu s'arrête sur la version demandée plutôt que d'aller au bout : une `VersionId` désigne un
/// point de l'histoire, et rendre l'état final pour une version intermédiaire serait répondre à une
/// autre question que celle posée.
///
/// # Errors
///
/// [`ReplayError::UnknownVersion`] quand aucune version du stream ne porte cette identité. **Jamais
/// une racine plausible** : `W17.j` l'interdit nommément, et le mode d'échec est celui des cursors
/// de `W20.e` — une réponse plausible prise au mauvais endroit, que rien dans la réponse ne signale.
pub fn resolve_at(
    payloads: &[serde_json::Value],
    target: &VersionId,
    digest: &impl Digest,
) -> Result<Version, ReplayError> {
    let (first, rest) = payloads.split_first().ok_or(ReplayError::Empty)?;
    let mut current = root_from(first, digest)?;
    if current.id() == target {
        return Ok(current);
    }
    for (position, payload) in rest.iter().enumerate() {
        current = apply_from(&current, payload, position, digest)?;
        if current.id() == target {
            return Ok(current);
        }
    }
    Err(ReplayError::UnknownVersion {
        version: target.to_string(),
    })
}

/// Une étape de rejeu — lire l'opération d'une charge et l'appliquer.
///
/// Écrite une fois et appelée par les trois lecteurs : sans quoi `replay`, `resolve` et
/// `resolve_at` auraient chacun leur lecture de charge, et la troisième aurait fini par diverger.
fn apply_from(
    current: &Version,
    payload: &serde_json::Value,
    position: usize,
    digest: &impl Digest,
) -> Result<Version, ReplayError> {
    let canonical = payload
        .get("operation")
        .and_then(serde_json::Value::as_str)
        .ok_or(ReplayError::Unreadable {
            position,
            detail: "aucun champ « operation »".to_owned(),
        })?;
    let operation = Operation::parse(canonical).map_err(|because: ParseOperationError| {
        ReplayError::Unreadable {
            position,
            detail: because.to_string(),
        }
    })?;
    current
        .apply(&operation, digest)
        .map_err(|because| ReplayError::Inapplicable {
            position,
            operation: canonical.to_owned(),
            detail: because.to_string(),
        })
}

/// Rejouer un stream d'organisation depuis une racine, et rendre la version courante.
///
/// # Pourquoi la racine est fournie
///
/// Une lacune nommée plutôt que comblée, comme `BranchContext` l'a fait pour les identifiants. La
/// racine d'une organisation est ce que produit `team.create` — une commande de §22.3 qui n'est pas
/// cet item —, et l'inventer ici demanderait de choisir un mode de coordination par défaut. §14.3
/// n'en donne aucun : les cinq modes sont obligatoires et aucun n'est le repli des autres. Un défaut
/// choisi en passant se lirait comme une décision de §14.3 alors qu'il ne serait qu'une commodité de
/// ce module, et fausserait la comparaison que §14.3 annonce entre campagnes.
///
/// # Errors
///
/// [`ReplayError::Unreadable`] quand la charge d'un événement n'a pas d'opération lisible, en
/// nommant la position — un stream illisible dont on ne sait pas *où* il l'est ne se répare pas ;
/// [`ReplayError::Inapplicable`] quand une opération ne s'applique pas sur l'état où le rejeu la
/// mène, ce qui **ne devrait pas arriver** et n'est donc pas supposé : un stream qui ne se rejoue
/// pas est un journal qui ment, et il vaut mieux l'apprendre par une erreur que par un condensat
/// qui diverge.
pub fn replay(
    root: &Version,
    payloads: &[serde_json::Value],
    digest: &impl Digest,
) -> Result<Version, ReplayError> {
    let mut current = root.clone();
    for (position, payload) in payloads.iter().enumerate() {
        current = apply_from(&current, payload, position, digest)?;
    }
    Ok(current)
}

/// Pourquoi un stream d'organisation ne se rejoue pas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayError {
    /// Le stream est vide — aucune organisation n'a été fondée sur cette branche.
    ///
    /// Distincte des autres, et le rester : une organisation **absente** et une organisation
    /// **vide** sont deux choses. Rendre une racine sans membre laisserait croire à la seconde, et
    /// un client afficherait une équipe vide là où il n'y a pas d'équipe.
    Empty,
    /// Aucune version du stream ne porte cette identité.
    ///
    /// Séparée d'[`ReplayError::Unreadable`] parce que la suite n'est pas la même : un stream
    /// illisible se répare, une version inconnue se corrige côté client. Et surtout, elle ne se
    /// résout **jamais** en une racine plausible.
    UnknownVersion {
        /// Celle qui a été demandée.
        version: String,
    },
    /// La charge d'un événement ne porte pas d'opération lisible.
    Unreadable {
        /// Sa position dans le stream.
        position: usize,
        /// Ce qui manque ou ne se lit pas.
        detail: String,
    },
    /// Une opération lisible qui ne s'applique pas là où le rejeu la mène.
    Inapplicable {
        /// Sa position dans le stream.
        position: usize,
        /// Sa forme canonique.
        operation: String,
        /// Ce que le domaine a répondu.
        detail: String,
    },
}

impl std::fmt::Display for ReplayError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => formatter.write_str(
                "aucune organisation n'a été fondée sur cette branche — une organisation absente \
                 n'est pas une organisation vide",
            ),
            Self::UnknownVersion { version } => write!(
                formatter,
                "aucune version « {version} » dans ce stream — une version inconnue ne se résout \
                 pas en une racine plausible"
            ),
            Self::Unreadable { position, detail } => write!(
                formatter,
                "événement {position} illisible : {detail} — un stream d'organisation qui ne se \
                 rejoue pas est un journal qui ment"
            ),
            Self::Inapplicable {
                position,
                operation,
                detail,
            } => write!(
                formatter,
                "événement {position} « {operation} » ne s'applique pas : {detail}"
            ),
        }
    }
}

impl std::error::Error for ReplayError {}
