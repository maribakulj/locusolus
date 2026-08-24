//! Un `EpistemicCommit` entre au journal, et l'institution l'intègre — `W20.r`, §2.3, §7.4, §8.1.
//!
//! # Ce qui manquait, et le défaut que l'item a mis au jour
//!
//! `W2.15` plafonne le worker à `staged` côté client, `packages/validation` porte la propagation, et
//! **rien dans `apps/locusd` ne connaissait `EpistemicCommit`** — seule la traduction de type
//! d'événement le nommait. La clause de `W12.d` « un `EpistemicCommit` est mis en scène puis
//! intégré » n'avait donc pas de sujet côté serveur.
//!
//! En le lui donnant, une sonde a trouvé pire qu'une absence. Un `epistemic_commit.submitted`
//! remonté par §15.6 recevait `202 Accepted`, et le fait écrit portait la sérialisation de
//! l'événement LEP — sans `status`, sans `validation_level`. La projection « état de validation » de
//! §9.3 refuse à juste titre un tel fait, elle passait en **quarantaine**, et `main.rs` refuse
//! d'ouvrir le port avec une projection en quarantaine : **un worker qui soumettait un commit
//! empêchait le daemon de redémarrer**, après lui avoir répondu que tout allait bien.
//!
//! Ce module rend cette faute inexprimable : un `epistemic_commit.*` ne traverse plus la traduction
//! générique, il a son chemin, et ce chemin écrit les deux champs que §9.3 exige.
//!
//! # Le worker met en scène ; il n'intègre pas
//!
//! §2.3 : « Canterel NE DOIT PAS promouvoir un claim au-delà de `staged` ». La règle vit des deux
//! côtés du fil, et [`locus_domain::Status::is_worker_proposable`] la dit déjà — ce module
//! l'**appelle** plutôt que de réécrire la liste. Un worker qui annonce `validated` reçoit un refus
//! qui nomme le champ, et rien n'est écrit.
//!
//! L'intégration est une commande **distincte**, sous une [`Authority`] et non sous une créance de
//! worker. Ce n'est pas une précaution de style : une seule fonction qui aurait pris « le statut
//! demandé » et « qui le demande » aurait fait dépendre l'invariant 3 d'un `if` au lieu d'une
//! frontière. Ici, [`Integrate`] n'est atteignable par aucun chemin de worker, et un test d'absence
//! le tient sur le source.
//!
//! # Le niveau de validation n'est **jamais** déduit du statut
//!
//! §7.4 : « `validation_level` décrit la force épistémique et ne doit pas être déduit du seul
//! statut ». `packages/domain` n'offre aucune conversion, et la projection n'en calcule aucune.
//!
//! Ce module n'en fait pas une non plus, et la distinction mérite d'être écrite parce qu'elle est
//! fine : un commit mis en scène est enregistré en [`ValidationLevel::Unassessed`] — `L0`, « objet
//! enregistré, non évalué » — non pas parce que `staged` impliquerait `L0`, mais parce que
//! **personne ne l'a évalué**. C'est un constat sur ce qui a eu lieu, pas une déduction depuis le
//! statut. À l'intégration, l'institution nomme les deux, et le type l'y oblige : ni [`Status`] ni
//! [`ValidationLevel`] n'y sont facultatifs.
//!
//! # Invariant 12 : un commit qui en contredit un autre entre par la même porte
//!
//! Aucun chemin ne compare un commit entrant à ce qui est déjà intégré, et c'est délibéré. « Les
//! résultats négatifs et conflits ne sont jamais supprimés pour rendre le graphe propre » : un
//! serveur qui refuserait une contradiction la ferait disparaître au moment exact où elle a le plus
//! de valeur. Le conflit est un **fait**, et `packages/projections` a un registre pour le lire.
//!
//! # Sur quel stream, et ce que ce choix coûte
//!
//! Sur celui de la tâche. La transaction verrouille **par stream** : écrire l'objet épistémique
//! ailleurs que là où arrivent les faits de la tâche rendrait un lot mixte inatomique, ce que
//! `W20.k` refuse déjà pour la même raison.
//!
//! Conséquence, nommée plutôt que tue : dans la projection, plusieurs tentatives d'une même tâche
//! partagent l'état courant de « l'objet épistémique de cette tâche ». **Le journal, lui, garde
//! chaque fait** — l'invariant 12 porte sur les faits, pas sur les index qui les servent. Les
//! distinguer par tentative est une décision de clé de projection, qui appartient à qui en aura
//! besoin.

use locus_domain::{Status, ValidationLevel};
use locus_event_store::{Actor, ActorKind, Draft as EventDraft, EventStore, EventType};
use locus_protocol::Timestamp;

use crate::command::CommandEnvelope;
use crate::composition::Runtime;
use crate::error::CommandError;
use crate::handler::Decide;
use crate::lep::{LepContext, Submitted, stream_of_task};
use crate::mission::Authority;

/// Le préfixe des types d'événement de §15.7 sur le fil LEP.
///
/// Écrit ici en clair, et non composé : c'est la moitié serveur d'un contrat, et `canterel` émet
/// `epistemic_commit.submitted` sous ce nom exact depuis `W2.15`.
pub const LEP_COMMIT_PREFIX: &str = "epistemic_commit.";

/// Le type de fait qu'un statut produit — `epistemic_object.<statut>`.
///
/// Le verbe **est** le statut, et c'est ce qui rend le journal lisible sans table de correspondance :
/// un fait dit dans quel état l'objet se trouve, pas ce qu'on a essayé de lui faire. C'est déjà le
/// vocabulaire que `packages/projections` rejoue dans ses tests (`epistemic_object.staged`).
#[must_use]
pub fn fact_type(status: Status) -> String {
    format!("epistemic_object.{}", status.as_str())
}

/// La charge d'un fait épistémique, telle que la projection de §9.3 l'exige.
///
/// Les deux champs sont **toujours** présents. La projection refuse un fait qui en manque un, et
/// elle a raison : « une projection qui compléterait les manquants par un défaut inventerait un état
/// de validation ». C'est la sonde de `W20.r` qui l'a établi en pratique — un fait sans ces deux
/// champs met la projection en quarantaine, et un daemon en quarantaine ne redémarre pas.
/// Les champs de contexte d'un fait, sous la forme que [`payload`] attend.
///
/// Une `Map` et non une `Value` : un contexte qui ne serait pas un objet n'a pas de sens ici, et le
/// type l'interdit plutôt qu'une branche de repli. Une branche qu'aucun appelant ne peut atteindre
/// ne se teste pas, ne se mute pas, et vieillit sans que rien ne le dise — c'est la faute que
/// `W20.n` a corrigée en retirant `Rejection::WrongEndpoint`.
#[must_use]
pub fn fields(
    pairs: impl IntoIterator<Item = (&'static str, serde_json::Value)>,
) -> serde_json::Map<String, serde_json::Value> {
    pairs
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect()
}

/// La charge d'un fait épistémique, telle que la projection de §9.3 l'exige.
///
/// Les deux champs sont **toujours** présents, et c'est **le serveur** qui les fixe : le contexte
/// est écrit d'abord, les deux champs propres ensuite, donc ils gagnent par construction et non par
/// une garde qu'il faudrait se souvenir d'écrire.
///
/// La projection refuse un fait qui en manque un, et elle a raison : « une projection qui
/// compléterait les manquants par un défaut inventerait un état de validation ». C'est la sonde de
/// `W20.r` qui l'a établi en pratique — un fait sans ces deux champs met la projection en
/// quarantaine, et un daemon en quarantaine ne redémarre pas.
#[must_use]
pub fn payload(
    status: Status,
    level: ValidationLevel,
    mut extra: serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    extra.insert(
        "status".to_owned(),
        serde_json::Value::String(status.as_str().to_owned()),
    );
    extra.insert(
        "validation_level".to_owned(),
        serde_json::to_value(level).unwrap_or(serde_json::Value::Null),
    );
    serde_json::Value::Object(extra)
}

/// La mise en scène : ce qu'un worker propose entre au journal, et **jamais au-delà de `staged`**.
pub struct Stage {
    /// Le rang de ce fait dans son lot.
    ///
    /// Deux faits écrits par la même commande ne peuvent pas porter la même identité, et la réserve
    /// est **fournie** — ce crate ne fabrique pas d'identifiants. Un `Stage` qui prendrait toujours
    /// le rang `0` collisionnerait avec le premier événement d'un lot mixte, et le journal porterait
    /// deux faits différents sous la même identité.
    pub rank: usize,
    /// La tâche à laquelle ce commit appartient.
    pub task_id: String,
    /// Le statut que le worker annonce.
    pub announced: String,
    /// Ce que le worker a mis dans la charge de son événement — transporté, jamais interprété.
    pub summary: serde_json::Value,
    /// Le worker qui parle, tel que **la créance** l'identifie.
    pub worker_id: String,
}

impl Stage {
    /// Le statut accepté, ou le refus qui nomme le champ.
    ///
    /// # Errors
    ///
    /// [`CommandError::Authorization`] quand le statut annoncé sort de ce que §2.3 permet à un
    /// worker, [`CommandError::Validation`] quand il n'est pas un statut de §7.4 du tout.
    ///
    /// Les deux ne se confondent pas : « ce mot n'existe pas » envoie relire le protocole,
    /// « ce mot ne t'appartient pas » envoie relire qui décide. Les fondre ferait chercher une
    /// faute de frappe là où il y a une usurpation d'autorité.
    pub fn accepted(&self) -> Result<Status, CommandError> {
        let status = Status::ALL
            .into_iter()
            .find(|candidate| candidate.as_str() == self.announced)
            .ok_or_else(|| CommandError::Validation {
                field: "payload.status".to_owned(),
                detail: format!(
                    "« {} » n'est pas un statut de §7.4 — les dix sont {}",
                    self.announced,
                    Status::ALL.map(Status::as_str).join(", ")
                ),
            })?;
        if !status.is_worker_proposable() {
            return Err(CommandError::Authorization {
                action: format!(
                    "annoncer le statut « {} » depuis un worker : §2.3 plafonne à « {} », et « {} » \
                     est un verdict que l'institution prononce",
                    status.as_str(),
                    Status::Staged.as_str(),
                    status.as_str()
                ),
            });
        }
        Ok(status)
    }
}

impl Decide for Stage {
    type State = LepContext;

    fn decide(
        &self,
        command: &CommandEnvelope,
        context: &Self::State,
    ) -> Result<Vec<EventDraft>, CommandError> {
        let status = self.accepted()?;
        if self.task_id.is_empty() {
            return Err(CommandError::Validation {
                field: "task_id".to_owned(),
                detail: "sans tâche, un commit n'a pas de stream où atterrir".to_owned(),
            });
        }
        Ok(vec![fact(
            command,
            context,
            self.rank,
            &fact_type(status),
            &stream_of_task(&self.task_id),
            payload(
                status,
                // `L0` parce que **personne n'a évalué**, et non parce que `staged` impliquerait
                // `L0` : §7.4 interdit de déduire le niveau du statut, et la nuance vaut d'être
                // écrite ici plutôt que devinée à la relecture.
                ValidationLevel::Unassessed,
                fields([
                    ("task_id", serde_json::json!(self.task_id)),
                    ("worker_id", serde_json::json!(self.worker_id)),
                    ("commit", self.summary.clone()),
                ]),
            ),
        )?])
    }
}

/// Reconnaître un commit de §15.7 dans un événement de fil, et le décideur qui lui convient.
///
/// Rend `None` pour tout le reste — c'est-à-dire pour les événements de progression de §15.6, qui
/// gardent le chemin générique. La reconnaissance se fait sur le **préfixe de type**, celui que
/// `canterel` émet depuis `W2.15`, et non sur la présence d'un champ : un événement de progression
/// qui porterait par hasard un `status` dans sa charge ne doit pas devenir un objet épistémique.
///
/// Le statut lu est celui de la **charge** du worker, et il est vérifié par [`Stage::accepted`].
/// Absent, il vaut `staged` : c'est ce que `W2.15` met en scène avant de signer, et un commit qui
/// arriverait sans statut serait mis en scène — jamais promu. Le seul cas que ce défaut pourrait
/// masquer est celui d'un worker qui **voulait** annoncer `validated` et a oublié le champ, et lui
/// accorder `staged` est précisément ce que §2.3 lui accorde.
#[must_use]
pub fn staging(event: &locus_lep::Event, task_id: &str, worker_id: &str) -> Option<Stage> {
    if !event.event_type.starts_with(LEP_COMMIT_PREFIX) {
        return None;
    }
    let summary = event.payload.clone().unwrap_or(serde_json::Value::Null);
    let announced = summary
        .get("status")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(Status::Staged.as_str())
        .to_owned();
    Some(Stage {
        rank: 0,
        task_id: task_id.to_owned(),
        announced,
        summary,
        worker_id: worker_id.to_owned(),
    })
}

/// L'intégration : l'institution prononce, et elle nomme les deux.
///
/// # Ce que ce décideur ne fait pas, et qui est le point
///
/// Il ne déduit rien. Ni le niveau depuis le statut — §7.4 —, ni le statut depuis ce qui précède.
/// Aucun champ n'est facultatif, donc une intégration qui « oublierait » le niveau ne compile pas.
/// C'est la forme que `W20.k` a donnée à `Complete::worker_id` pour la même raison : rendre la faute
/// inexprimable vaut mieux que la chercher.
pub struct Integrate {
    /// La tâche dont l'objet épistémique est intégré.
    pub task_id: String,
    /// Le statut que l'institution prononce — §7.4.
    pub status: Status,
    /// La force épistémique constatée — §8.1, et jamais déduite du statut.
    pub level: ValidationLevel,
    /// Pourquoi, en clair. Une intégration sans motif se relit dans dix ans sans qu'on sache.
    pub rationale: String,
}

impl Decide for Integrate {
    type State = LepContext;

    fn decide(
        &self,
        command: &CommandEnvelope,
        context: &Self::State,
    ) -> Result<Vec<EventDraft>, CommandError> {
        if self.rationale.trim().is_empty() {
            return Err(CommandError::Validation {
                field: "rationale".to_owned(),
                detail: "une intégration sans motif ne se relit pas : §8.4 refuse qu'une décision \
                         épistémique s'appuie sur rien qu'on puisse citer"
                    .to_owned(),
            });
        }
        Ok(vec![fact(
            command,
            context,
            0,
            &fact_type(self.status),
            &stream_of_task(&self.task_id),
            payload(
                self.status,
                self.level,
                fields([
                    ("task_id", serde_json::json!(self.task_id)),
                    ("rationale", serde_json::json!(self.rationale)),
                ]),
            ),
        )?])
    }
}

fn fact(
    command: &CommandEnvelope,
    context: &LepContext,
    rank: usize,
    event_type: &str,
    stream_id: &str,
    body: serde_json::Value,
) -> Result<EventDraft, CommandError> {
    Ok(EventDraft {
        event_id: context.identity(rank)?,
        event_type: EventType::parse(event_type).unwrap_or_else(|_| {
            unreachable!(
                "« {event_type} » sort de `fact_type`, et `epistemic_object` est un namespace de §10.3"
            )
        }),
        schema_version: 1,
        stream_id: stream_id.to_owned(),
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
        payload: body,
        payload_hash: context.payload_hash.clone(),
    })
}

impl<S: EventStore> Runtime<S> {
    /// Intégrer l'objet épistémique d'une tâche — §7.4, sous l'autorité de l'institution.
    ///
    /// # Aucune créance de worker n'atteint cette fonction
    ///
    /// Elle prend une [`Authority`], que seul un appelant d'administration porte : un worker n'en a
    /// pas, et la surface §15.2 n'en construit jamais. C'est la même séparation que `W20.o` a posée
    /// entre [`Authority`] et [`Submitted`], et pour la même raison — un worker qui annoncerait son
    /// workspace écrirait dans n'importe lequel.
    ///
    /// # Errors
    ///
    /// [`CommandError`] — ce que le décideur ou la transaction refusent.
    pub fn lep_integrate(
        &self,
        integrate: &Integrate,
        authority: Authority,
        submitted: &Submitted,
        now: Timestamp,
    ) -> Result<(), CommandError> {
        let identities = self.lep().identities();
        let context = LepContext {
            project_id: submitted.project_id,
            event_ids: identities.events(1)?,
            occurred_at: submitted.occurred_at,
            payload_hash: String::new(),
        };
        let stream = stream_of_task(&integrate.task_id);
        let command = CommandEnvelope::mutating(
            identities.command()?,
            "epistemic_object.integrate",
            authority.workspace_id,
            authority.principal_id,
            submitted.idempotency_key.clone(),
            crate::error::Revision::new(self.revision_of_stream(&stream)),
        )?;
        match self.commit(integrate, &command, &context, now) {
            crate::outcome::Outcome::Accepted(_) => Ok(()),
            crate::outcome::Outcome::Refused(error) => Err(error),
        }
    }
}
