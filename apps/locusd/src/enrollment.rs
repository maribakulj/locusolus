//! L'enrôlement de §7.2, côté serveur — `W20.n`, ADR 0031.
//!
//! # Ce que `W2.4` a livré, et ce que personne n'écoutait
//!
//! `canterel` génère une identité Ed25519, signe une demande liant `worker_id`, endpoint et nonce,
//! et garde la créance obtenue. La moitié serveur n'existait pas : `W20.k` a dû livrer un
//! `WorkerRegistry` que seul un test remplit, donc un worker réel ne pouvait pas obtenir de créance,
//! donc les trois chemins de §15.2 étaient injoignables en pratique.
//!
//! # Le token ne devient jamais le secret permanent
//!
//! §7.2 l'écrit, et ce module le rend vrai **par construction** : la créance émise est une valeur
//! distincte, tirée de la source d'identifiants de `W20.k`, et le token est consommé au premier
//! usage. Un serveur qui renverrait le token comme créance passerait tous les tests fonctionnels et
//! donnerait à un secret court-terme la durée de vie d'un secret permanent — un test le refuse
//! explicitement, parce que c'est la faute qu'aucun symptôme ne signale.
//!
//! # Le nonce lie la demande à **son** serveur
//!
//! Le client signe `worker_id\nendpoint\nnonce`. Sans l'endpoint, une demande capturée se resservirait
//! vers un autre serveur ; sans le nonce, vers le même. `W2.4` a écrit la première moitié dans son
//! propre commentaire ; celui-ci tient la seconde.

use std::collections::HashSet;
use std::sync::{Mutex, PoisonError};

use ed25519_dalek::pkcs8::DecodePublicKey;
use ed25519_dalek::{Signature, VerifyingKey};
use locus_event_store::{Actor, ActorKind, Draft as EventDraft, EventStore, EventType};
use locus_protocol::{Id, Timestamp};

use crate::command::CommandEnvelope;
use crate::composition::Runtime;
use crate::error::CommandError;
use crate::handler::Decide;
use crate::lep::{Enrolling, LepContext, Submitted, WorkerIdentity};

/// La demande signée qu'un worker envoie — la forme de `W2.4`, champ pour champ.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EnrollmentRequest {
    /// L'identité que le worker s'est donnée.
    pub worker_id: String,
    /// Sa nature. `canterel` aujourd'hui ; un worker Lean en aura une autre.
    pub worker_kind: String,
    /// Clé publique Ed25519, SPKI DER en base64.
    pub public_key: String,
    /// L'empreinte de la machine — §7.1.
    pub runtime: String,
    /// L'aléa qui empêche le rejeu.
    pub nonce: String,
    /// La signature de `worker_id\nendpoint\nnonce`, en base64.
    pub signature: String,
    /// Le token, transporté une fois et jamais persisté.
    pub enrollment_token: String,
}

/// Ce que le serveur rend en échange — §7.2.
///
/// `credential` n'est **pas** le token : c'est une valeur neuve, et le test qui le vérifie existe
/// parce que la confusion serait indétectable autrement.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Credential {
    /// Le worker.
    pub worker_id: String,
    /// La créance renouvelable.
    pub credential: String,
    /// Quand elle a été émise.
    pub issued_at: String,
    /// Quand elle expire — `null` tant que personne ne le fait respecter (ADR 0031 décision 5).
    pub expires_at: Option<String>,
    /// Le scope imposé par le token.
    pub scope: Vec<String>,
    /// Les labels que l'enrôlement impose.
    pub labels: Vec<String>,
}

/// Ce qu'un token d'enrôlement autorise, quand il est encore valable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    /// Le scope de §7.2.
    pub scope: Vec<String>,
    /// Les labels imposés.
    pub labels: Vec<String>,
    /// Le workspace dans lequel les faits de ce worker s'écriront.
    pub workspace_id: Id<locus_protocol::id::Workspace>,
    /// Le projet auquel ses faits appartiendront — `W20.w`.
    ///
    /// # Pourquoi il vit ici et non dans la demande du worker
    ///
    /// Il vivait dans `WorkerBody`, donc dans ce que le **worker envoie**, et c'est la mauvaise
    /// moitié : un worker qui choisit son propre projet écrit dans un projet que personne ne lui a
    /// assigné. `Grant` porte déjà `workspace_id` et `principal_id` pour exactement cette raison —
    /// ce sont des choses que l'institution décide **de** lui, pas des choses qu'il déclare.
    ///
    /// La distinction n'est pas théorique : le projet est l'endroit où ses faits atterrissent, donc
    /// ce que les projections de §9.5 rangeront sous ce nom, donc ce qu'un lecteur croira avoir été
    /// produit là.
    pub project_id: Id<locus_protocol::id::Project>,
    /// Le principal sous lequel il agira.
    pub principal_id: Id<locus_protocol::id::Agent>,
}

/// D'où viennent les tokens d'enrôlement, et ce qui se souvient qu'ils ont servi.
///
/// # Un port, parce que rien n'en émet encore
///
/// §7.2 veut un token court-terme, à usage unique, portant un scope. Aucune commande de §22.3 n'en
/// émet. Reporter l'item reviendrait à dire « aucun appelant », ce que l'ADR 0022 décision 0 refuse ;
/// inventer un émetteur en passant serait bâtir une fonctionnalité pour justifier une surface.
pub trait EnrollmentTokens: Send + Sync {
    /// **Consommer** ce token, s'il est valable. Un token consommé ne l'est plus.
    ///
    /// Le verbe compte : `redeem` et non `check`. Une méthode qui vérifierait sans consommer
    /// laisserait au serveur le soin de consommer ensuite, donc la possibilité de l'oublier — le
    /// « check-then-act » que l'ADR 0029 a rendu inexprimable pour les écritures.
    fn redeem(&self, token: &str) -> Option<Grant>;

    /// Vrai si ce nonce a **déjà** servi. L'enregistre au passage, pour la même raison.
    fn nonce_seen(&self, nonce: &str) -> bool;
}

/// L'implémentation de référence — en mémoire, remplie par qui la détient.
#[derive(Debug, Default)]
pub struct MemoryTokens {
    grants: Mutex<Vec<(String, Grant)>>,
    nonces: Mutex<HashSet<String>>,
}

impl MemoryTokens {
    /// Un émetteur vide — donc un daemon qui n'enrôle personne.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Déposer un token et ce qu'il autorise.
    pub fn issue(&self, token: &str, grant: Grant) {
        self.grants
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((token.to_owned(), grant));
    }
}

impl EnrollmentTokens for MemoryTokens {
    fn redeem(&self, token: &str) -> Option<Grant> {
        let mut grants = self.grants.lock().unwrap_or_else(PoisonError::into_inner);
        let position = grants.iter().position(|(known, _)| known == token)?;
        Some(grants.remove(position).1)
    }

    fn nonce_seen(&self, nonce: &str) -> bool {
        !self
            .nonces
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(nonce.to_owned())
    }
}

/// Pourquoi un enrôlement est refusé — §7.2, et chaque variante envoie chercher ailleurs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    /// La clé publique n'est pas une clé Ed25519 SPKI lisible.
    UnreadableKey,
    /// La signature ne correspond pas à ce qui est signé.
    BadSignature,
    /// Ce nonce a déjà servi.
    ReplayedNonce,
    /// Le token est inconnu, ou déjà consommé.
    UnknownToken,
}

impl std::fmt::Display for Rejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnreadableKey => formatter.write_str(
                "clé publique illisible : §7.2 attend une clé Ed25519 au format SPKI, encodée en base64",
            ),
            Self::BadSignature => formatter.write_str(
                "signature invalide : la demande n'a pas été signée par la clé qu'elle annonce",
            ),
            Self::ReplayedNonce => formatter.write_str(
                "nonce déjà vu : une demande capturée ne se ressert pas contre le même serveur",
            ),
            Self::UnknownToken => formatter.write_str(
                "token d'enrôlement inconnu ou déjà consommé : §7.2 le veut à usage unique",
            ),
        }
    }
}

/// Ce que le client signe — `W2.4`, `identity.ts`, et il ne faut pas s'en écarter d'un octet.
#[must_use]
pub fn signed_payload(worker_id: &str, endpoint: &str, nonce: &str) -> String {
    format!("{worker_id}\n{endpoint}\n{nonce}")
}

/// Vérifier la demande, sans rien écrire ni consommer.
///
/// # Pourquoi il n'y a **pas** de refus « signée pour un autre serveur »
///
/// Une première rédaction en portait un. Il était **inatteignable** : la charge est reconstruite
/// avec *notre* endpoint, donc une demande signée pour un autre serveur échoue à la vérification et
/// se lit `BadSignature`. Le distinguer demanderait que le client renvoie l'endpoint qu'il a signé,
/// c'est-à-dire qu'on le croie sur parole — et cela ne servirait qu'à lui dire lequel des deux a
/// échoué, ce qui transforme la surface en oracle.
///
/// La variante a donc été retirée plutôt que gardée « au cas où » : `CLAUDE.md` refuse une valeur
/// d'énumération qui annonce un effet dont personne n'est le consommateur. Ce que la garde protège
/// reste vrai — l'endpoint est bien dans la signature, donc une demande capturée ne se ressert pas
/// ailleurs. C'est le refus qui n'a pas de nom propre, pas la protection.
///
/// # Errors
///
/// [`Rejection::UnreadableKey`] ou [`Rejection::BadSignature`]. Le nonce et le token ne sont **pas**
/// vérifiés ici : les consulter consomme, et une fonction qui vérifie ne doit pas avoir d'effet.
pub fn verify(request: &EnrollmentRequest, endpoint: &str) -> Result<(), Rejection> {
    let der = base64_decode(&request.public_key).ok_or(Rejection::UnreadableKey)?;
    let key = VerifyingKey::from_public_key_der(&der).map_err(|_| Rejection::UnreadableKey)?;

    let raw = base64_decode(&request.signature).ok_or(Rejection::BadSignature)?;
    let bytes: [u8; 64] = raw.try_into().map_err(|_| Rejection::BadSignature)?;
    let signature = Signature::from_bytes(&bytes);

    // La charge est reconstruite avec **notre** endpoint : une demande signée pour un autre serveur
    // ne peut pas la reproduire, et échoue donc ici. C'est ce qui rend une demande capturée
    // inutilisable ailleurs — la moitié serveur de ce que `W2.4` a écrit côté client.
    let payload = signed_payload(&request.worker_id, endpoint, &request.nonce);
    key.verify_strict(payload.as_bytes(), &signature)
        .map_err(|_| Rejection::BadSignature)?;
    Ok(())
}

/// Décoder du base64 standard, ou rien.
///
/// Écrit ici plutôt qu'apporté par un crate : c'est vingt lignes, et `dependencies.json` refuse à
/// juste titre ce qui n'a pas d'ADR. Une dépendance pour décoder du base64 serait un paquet de plus
/// dans tous les profils de §27.1 — voir le raisonnement de l'ADR 0031 sur `ring`.
fn base64_decode(text: &str) -> Option<Vec<u8>> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let body = text.trim_end_matches('=');
    let mut bits: u32 = 0;
    let mut held = 0_u32;
    let mut out = Vec::with_capacity(body.len() * 3 / 4);
    for byte in body.bytes() {
        let value = ALPHABET.iter().position(|candidate| *candidate == byte)?;
        bits = (bits << 6) | u32::try_from(value).ok()?;
        held += 6;
        if held >= 8 {
            held -= 8;
            out.push(u8::try_from((bits >> held) & 0xFF).ok()?);
        }
    }
    Some(out)
}

/// Le stream d'un worker. Un seul écrivain par worker, donc un seul verrou — `W20.h`.
#[must_use]
pub fn stream_of_worker(worker_id: &str) -> String {
    format!("worker/{worker_id}")
}

/// L'enrôlement écrit un fait — sans quoi rien n'atteste qu'il a eu lieu.
pub struct Enroll {
    /// La demande, déjà vérifiée.
    pub request: EnrollmentRequest,
    /// Le workspace et le principal que le token a liés.
    pub grant: Grant,
}

impl Decide for Enroll {
    type State = LepContext;

    fn decide(
        &self,
        command: &CommandEnvelope,
        context: &Self::State,
    ) -> Result<Vec<EventDraft>, CommandError> {
        Ok(vec![fact(
            command,
            context,
            "worker.registered",
            &stream_of_worker(&self.request.worker_id),
            serde_json::json!({
                "worker_id": self.request.worker_id,
                "worker_kind": self.request.worker_kind,
                "runtime": self.request.runtime,
                "scope": self.grant.scope,
                "labels": self.grant.labels,
                // La clé **publique** est un fait du registre ; ni le token ni la créance n'entrent
                // au journal. `CLAUDE.md` interdit de journaliser un secret, et un journal est ce
                // qu'on relit le plus longtemps.
                "public_key": self.request.public_key,
            }),
        )?])
    }
}

/// La révocation écrit un fait de plus — jamais une ligne supprimée (invariant 12).
pub struct Revoke {
    /// Le worker révoqué.
    pub worker_id: String,
    /// Pourquoi, en clair.
    pub reason: String,
}

impl Decide for Revoke {
    type State = LepContext;

    fn decide(
        &self,
        command: &CommandEnvelope,
        context: &Self::State,
    ) -> Result<Vec<EventDraft>, CommandError> {
        Ok(vec![fact(
            command,
            context,
            "worker.revoked",
            &stream_of_worker(&self.worker_id),
            serde_json::json!({ "worker_id": self.worker_id, "reason": self.reason }),
        )?])
    }
}

fn fact(
    command: &CommandEnvelope,
    context: &LepContext,
    event_type: &str,
    stream_id: &str,
    payload: serde_json::Value,
) -> Result<EventDraft, CommandError> {
    Ok(EventDraft {
        event_id: context.identity(0)?,
        event_type: EventType::parse(event_type).unwrap_or_else(|_| {
            unreachable!("« {event_type} » est un littéral de ce module, et `worker` est un namespace de §10.3")
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
        // `W20.j` : **jamais** renseignée ici. La clé d'idempotence est l'affaire de la
        // transaction, qui l'appose à l'écriture — un producteur qui la choisirait ferait
        // dépendre l'idempotence du client de ce que chaque handler se trouve écrire.
        idempotency_key: None,
        correlation_id: command.correlation_id().copied(),
        trace_id: None,
        payload,
        payload_hash: context.payload_hash.clone(),
    })
}

impl<S: EventStore> Runtime<S> {
    /// `POST /lep/v1/enroll` — §7.2.
    ///
    /// # L'ordre des vérifications, et ce qu'il protège
    ///
    /// Signature d'abord, **puis** nonce, **puis** token. Une demande mal signée ne doit consommer
    /// ni nonce ni token : sinon n'importe qui épuiserait les tokens d'un worker en envoyant du
    /// bruit signé n'importe comment.
    ///
    /// # Errors
    ///
    /// [`CommandError::Authorization`] portant la raison de §7.2, ou ce que la transaction refuse.
    pub fn lep_enroll(
        &self,
        request: &EnrollmentRequest,
        endpoint: &str,
        enrolling: &Enrolling,
        now: Timestamp,
    ) -> Result<Credential, CommandError> {
        verify(request, endpoint).map_err(|rejection| refusal(&rejection))?;

        if self.enrollment().nonce_seen(&request.nonce) {
            return Err(refusal(&Rejection::ReplayedNonce));
        }
        let grant = self
            .enrollment()
            .redeem(&request.enrollment_token)
            .ok_or_else(|| refusal(&Rejection::UnknownToken))?;

        // Le projet vient du **grant**, jamais du corps — `W20.w`. Un worker qui en propose un
        // autre est refusé plutôt qu'ignoré : l'ignorer en silence le laisserait croire qu'il écrit
        // dans le projet qu'il a nommé, et découvrir le contraire des mois plus tard en lisant une
        // projection.
        if let Some(propose) = enrolling.proposed_project
            && propose != grant.project_id
        {
            return Err(CommandError::Validation {
                field: "project_id".to_owned(),
                detail: format!(
                    "« {propose} » n'est pas le projet de ce token d'enrôlement : le projet est \
                     assigné par le grant, pas choisi par le worker"
                ),
            });
        }
        // La clé d'idempotence d'un enrôlement est son **nonce** — `W20.x`.
        //
        // Pas un champ de plus à remplir : le nonce est déjà ce qui rend une demande unique, il est
        // **signé**, et le daemon en tient le registre (`nonce_seen`). Exiger en plus une clé
        // choisie par le worker demanderait deux valeurs pour une garantie, et la moins sûre des
        // deux — un worker peut réutiliser sa clé, il ne peut pas réutiliser son nonce.
        //
        // Trouvé en enrôlant un worker réel : `canterel` n'envoie pas d'`idempotency_key`, et
        // `CommandEnvelope::mutating` la refuse vide. Le worker n'avait pas tort : il avait déjà
        // envoyé ce qu'il fallait, sous un autre nom.
        let submitted = &Submitted {
            idempotency_key: request.nonce.clone(),
            project_id: grant.project_id,
            occurred_at: enrolling.occurred_at,
        };

        // La créance est une valeur **neuve** — jamais le token. §7.2 : « un token ne devient pas le
        // secret permanent du worker ».
        let credential = self.lep().identities().command()?.to_string();

        let enroll = Enroll {
            request: request.clone(),
            grant: grant.clone(),
        };
        let stream = stream_of_worker(&request.worker_id);
        self.write_enrollment_fact(&grant, submitted, &stream, &enroll, now)?;

        self.lep().registry().admit_enrolled(
            &credential,
            WorkerIdentity {
                worker_id: request.worker_id.clone(),
                workspace_id: grant.workspace_id,
                principal_id: grant.principal_id,
                // `W20.z` : la troisième coordonnée entre ici, où les deux autres entraient déjà.
                // C'est le seul endroit qui la connaît de source sûre — le grant, redeemé une fois
                // et à usage unique.
                project_id: grant.project_id,
            },
        );

        Ok(Credential {
            worker_id: request.worker_id.clone(),
            credential,
            issued_at: now.to_string(),
            // `null`, et non une date que rien n'honore — ADR 0031 décision 5.
            expires_at: None,
            scope: grant.scope,
            labels: grant.labels,
        })
    }

    /// Révoquer un worker — §7.4, et le fait entre au journal.
    ///
    /// # Errors
    ///
    /// Ce que la transaction refuse.
    pub fn lep_revoke(
        &self,
        worker_id: &str,
        reason: &str,
        grant: &Grant,
        submitted: &Submitted,
        now: Timestamp,
    ) -> Result<(), CommandError> {
        let revoke = Revoke {
            worker_id: worker_id.to_owned(),
            reason: reason.to_owned(),
        };
        let stream = stream_of_worker(worker_id);
        self.write_enrollment_fact(grant, submitted, &stream, &revoke, now)?;
        self.lep().registry().revoke(worker_id);
        Ok(())
    }

    fn write_enrollment_fact<D: Decide<State = LepContext>>(
        &self,
        grant: &Grant,
        submitted: &Submitted,
        stream: &str,
        decider: &D,
        now: Timestamp,
    ) -> Result<(), CommandError> {
        let identities = self.lep().identities();
        let context = LepContext {
            project_id: submitted.project_id,
            event_ids: identities.events(1)?,
            occurred_at: submitted.occurred_at,
            payload_hash: String::new(),
        };
        let command = CommandEnvelope::mutating(
            identities.command()?,
            "worker.enroll",
            grant.workspace_id,
            grant.principal_id,
            submitted.idempotency_key.clone(),
            crate::error::Revision::new(self.revision_of_stream(stream)),
        )?;
        match self.commit(decider, &command, &context, now) {
            crate::outcome::Outcome::Accepted(_) => Ok(()),
            crate::outcome::Outcome::Refused(error) => Err(error),
        }
    }
}

/// Un refus de §7.2, traduit sous la famille de §22.5 qui envoie chercher au bon endroit.
///
/// `Authorization` et non `Validation` : la requête est bien formée, et c'est le **droit** d'enrôler
/// qui manque. Lui rendre `validation` enverrait relire une requête où il n'y a rien à corriger.
fn refusal(rejection: &Rejection) -> CommandError {
    CommandError::Authorization {
        action: rejection.to_string(),
    }
}
