//! La chaîne d'artefacts, côté institution — `docs/SPEC_V1.md` §19.1, `W20.t`.
//!
//! # Ce que cet item corrige
//!
//! `W2.14` a livré la déclaration-avant-upload **côté worker**, et `packages/artifacts` porte le
//! manifeste, la confrontation du hash, la quarantaine et la promotion. Entre les deux, rien :
//! `apps/locusd` ne dépendait pas de `locus-artifacts`, aucune route ne recevait de manifeste, et
//! la clause « les artefacts sont hashés » de `e2e/minimal_science` n'avait donc **aucun sujet
//! institutionnel** — exactement la situation où `EpistemicCommit` se trouvait avant `W20.r`.
//!
//! # L'ordre, qui est toute la garantie
//!
//! §19.1 : `artifact.declared` → adresse de dépôt → upload → vérification → `artifact.uploaded`.
//!
//! Déclarer d'abord fait du hash une **promesse**, faite quand personne ne sait encore ce qui
//! arrivera à l'autre bout ; la vérification confronte cette promesse à ce qui est reçu. Inverser
//! les deux — hasher le contenu reçu puis écrire le manifeste — produirait une vérification qui ne
//! peut pas échouer, donc pas une vérification. C'est la même dissymétrie que l'attestation de
//! `W4.d.2`, qui vient de l'observation et non de la demande.
//!
//! Ce module ne réimplémente pas cet ordre : [`locus_artifacts::ingest`] **est** l'ordre, et
//! [`Blobs`] existe pour que le daemon l'appelle plutôt que de le refaire.
//!
//! # Ce que le daemon ne croit pas
//!
//! Ni le hash — il le recalcule sur les octets reçus —, ni l'état — il le lit du journal —, ni la
//! taille — le store borne l'écriture pendant qu'elle a lieu. Un worker qui déclare `size_bytes:
//! 10` puis envoie un mégaoctet est refusé au fragment qui dépasse, pas après.

use std::sync::{Arc, Mutex, PoisonError};

use locus_artifacts::{
    ArtifactManifest, IngestError, ManifestError, MemoryObjectStore, Sha256Digest, StoreError,
    WireError, ingest,
};
use locus_domain::ContentHash;
use locus_event_store::{Draft as EventDraft, EventStore};
use locus_lep::ArtifactManifest as WireManifest;
use locus_protocol::Timestamp;
use serde::Serialize;

use crate::command::CommandEnvelope;
use crate::composition::Runtime;
use crate::error::CommandError;
use crate::handler::Decide;
use crate::lep::{LepContext, Submitted};

/// Le préfixe de stream d'un artefact.
///
/// Distinct de celui d'une tâche : un artefact a sa propre histoire — déclaré, arrivé, mis en
/// quarantaine, promu — et la ranger dans le stream de la tâche qui l'a produit ferait dépendre la
/// révision de l'un des écritures de l'autre. Deux workers qui déclarent deux artefacts de la même
/// tâche entreraient alors en conflit d'écriture sans avoir rien de commun.
#[must_use]
pub fn stream_of_artifact(artifact_id: &str) -> String {
    format!("artifact/{artifact_id}")
}

/// Les deux faits de §19.1, sous leur nom de §10.3.
///
/// # Pourquoi la relecture cherche **ce nom** et pas un champ
///
/// Une première rédaction cherchait « le premier fait qui porte une clé `manifest` ». Un passage de
/// mutation l'a démentie : la clé est un *indice* de la déclaration, pas la déclaration. Le jour où
/// un autre fait d'artefact — une promotion, une mise en quarantaine — porterait un manifeste, la
/// relecture prendrait le premier venu, et un stream qui n'a jamais rien déclaré rendrait un défaut
/// interne au lieu de dire au client de déclarer d'abord. Chercher le nom du fait ne se trompe pas
/// de sujet.
pub const DECLARED_FACT: &str = "artifact.declared";
/// Voir [`DECLARED_FACT`].
pub const UPLOADED_FACT: &str = "artifact.uploaded";

/// Le délai pendant lequel une déclaration ouvre le dépôt de son contenu — §19.1, « URL
/// temporaire ».
///
/// # Pourquoi une valeur enforcée et non un champ décoratif
///
/// §19.1 dit **temporaire**. Rendre une échéance que rien ne vérifie serait annoncer un effet qui
/// n'a pas lieu — ce que l'ADR 0022 refuse sous le nom de promesse. Elle est donc comparée à
/// l'arrivée du contenu, et un dépôt hors délai est refusé en disant lequel.
///
/// Quinze minutes : la fenêtre borne le moment où le contenu **commence** à arriver, pas la durée
/// du transfert. Un worker qui a déclaré et qui téléverse dans la foulée n'en voit jamais le bord ;
/// une déclaration oubliée depuis une heure n'ouvre plus rien. Une garde qui crie sur ce qui est
/// juste se fait désactiver, et c'est pourquoi elle est large.
pub const UPLOAD_WINDOW_SECONDS: i64 = 900;

/// Le stockage des octets, tel que le daemon le voit.
///
/// # Une seule méthode, et c'est délibéré
///
/// [`locus_artifacts::store::ObjectStore`] en a cinq — `begin`, `write`, `commit`, `abort`, et la
/// lecture —, et **l'ordre dans lequel on les appelle est la garantie**. Exposer les cinq au
/// daemon lui laisserait refaire cet ordre, donc le refaire de travers un jour : confronter après
/// avoir rangé revient à faire entrer le contenu puis à espérer pouvoir l'oublier.
///
/// Ce port porte donc l'appel que `ingest` fait déjà correctement, et rien d'autre. Un driver sur
/// système de fichiers ou sur S3 l'implémentera de la même façon : en appelant `ingest`.
pub trait Blobs: Send + Sync {
    /// Recevoir le contenu d'un artefact **déjà déclaré**, et confronter son hash.
    ///
    /// Rend le manifeste avancé en `uploaded`. Sur refus, rien n'est lisible : ni sous le hash
    /// déclaré, ni sous celui du contenu écrit.
    ///
    /// # Errors
    ///
    /// [`IngestError`] : ce que le store refuse, ou un contenu qui n'est pas celui qui avait été
    /// promis.
    fn receive(
        &self,
        manifest: ArtifactManifest,
        bytes: &[u8],
    ) -> Result<ArtifactManifest, IngestError>;
}

/// Le stockage par défaut : aucun.
///
/// Comme [`crate::lep::NoIdentities`] et comme le broker absent de `W20.q` : un daemon dont
/// personne n'a câblé le stockage **refuse** les dépôts, en disant que rien n'est câblé. Il ne les
/// accepte pas en jetant les octets — un `202` sur un contenu que personne n'a rangé ferait écrire
/// `artifact.uploaded` pour un artefact introuvable, et l'invariant 4 tomberait sans qu'aucun test
/// ne rougisse.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoBlobs;

impl Blobs for NoBlobs {
    fn receive(
        &self,
        _manifest: ArtifactManifest,
        _bytes: &[u8],
    ) -> Result<ArtifactManifest, IngestError> {
        Err(IngestError::Store(StoreError::NotDeclared {
            state: "aucun stockage d'objets n'est câblé sur ce daemon",
        }))
    }
}

/// Le stockage de référence — en mémoire, celui de `packages/artifacts`.
///
/// Le `Mutex` est ici et pas dans le port : [`locus_artifacts::store::ObjectStore`] prend
/// `&mut self` parce qu'un store **est** mutable, et c'est juste. Ce qui est propre au daemon,
/// c'est qu'il le partage entre des requêtes concurrentes ; la sérialisation est donc sa
/// responsabilité, pas celle du contrat.
#[derive(Debug, Default)]
pub struct MemoryBlobs {
    store: Mutex<MemoryObjectStore>,
}

impl MemoryBlobs {
    /// Un stockage vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Le nombre d'objets rangés. Lecture, pour les diagnostics et les tests.
    #[must_use]
    pub fn object_count(&self) -> usize {
        self.store
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .object_count()
    }
}

impl Blobs for MemoryBlobs {
    fn receive(
        &self,
        manifest: ArtifactManifest,
        bytes: &[u8],
    ) -> Result<ArtifactManifest, IngestError> {
        let mut store = self.store.lock().unwrap_or_else(PoisonError::into_inner);
        let mut digest = Sha256Digest::new();
        ingest(&mut *store, &mut digest, manifest, &[bytes])
    }
}

/// L'adresse où déposer le contenu d'un artefact déclaré, et jusqu'à quand.
///
/// # Un chemin, et non une URL absolue
///
/// §19.1 parle d'une « URL temporaire ». Une URL absolue exige que le daemon connaisse son propre
/// nom public — celui que voit le worker, derrière le proxy éventuel —, et il ne le connaît pas.
/// Le deviner depuis l'en-tête `Host` d'une requête reviendrait à laisser un client choisir l'hôte
/// vers lequel les artefacts partent ; le coder en dur reviendrait à supposer une machine de
/// développeur, ce que `CLAUDE.md` interdit explicitement.
///
/// Le champ s'appelle donc `upload_path` et non `url` : il **est** un chemin, et le client le
/// résout contre l'endpoint qu'il vient d'appeler — qu'il connaît nécessairement, puisqu'il y a
/// posté sa déclaration. Un champ nommé `url` portant un chemin serait le mensonge que ce nom
/// évite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ticket {
    /// L'artefact concerné.
    pub artifact_id: String,
    /// Où déposer le contenu, relativement à l'endpoint du daemon.
    pub upload_path: String,
    /// Jusqu'à quand cette déclaration ouvre le dépôt — voir [`UPLOAD_WINDOW_SECONDS`].
    pub expires_at: String,
}

/// Ce que le daemon dit avoir reçu.
///
/// `received_hash` est **calculé**, jamais recopié de la déclaration : le renvoyer depuis le
/// manifeste ferait dire au daemon « j'ai reçu ce que tu as annoncé » quoi qu'il ait reçu, et le
/// client qui le compare à sa propre valeur comparerait sa déclaration à elle-même.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Receipt {
    /// Le condensat du contenu reçu.
    pub received_hash: String,
    /// Sa taille.
    pub size_bytes: u64,
}

/// Le fait de §19.1 : un artefact est déclaré, son contenu n'est pas encore là.
#[derive(Debug, Clone)]
pub struct Declared {
    /// Le manifeste, tel que le domaine l'a accepté.
    pub manifest: ArtifactManifest,
    /// Qui l'a déclaré.
    pub worker_id: String,
    /// Quand la fenêtre de dépôt se ferme.
    pub expires_at: Timestamp,
}

impl Decide for Declared {
    type State = LepContext;

    fn decide(
        &self,
        command: &CommandEnvelope,
        context: &Self::State,
    ) -> Result<Vec<EventDraft>, CommandError> {
        Ok(vec![crate::mission::fact(
            command,
            context,
            DECLARED_FACT,
            &stream_of_artifact(self.manifest.artifact_id()),
            serde_json::json!({
                "artifact_id": self.manifest.artifact_id(),
                "worker_id": self.worker_id,
                // **La provenance, dans le fait** — invariant 4. Elle est déjà dans le manifeste ;
                // l'écrire en clair à côté fait qu'une projection qui ne relit pas le manifeste la
                // voit quand même. Un artefact sans elle n'entre pas : `ArtifactManifest::declare`
                // refuse une `task_id` vide, et c'est la seule porte d'entrée.
                "produced_by": {
                    "task_id": self.manifest.produced_by().task_id,
                    "attempt": self.manifest.produced_by().attempt,
                },
                "declared_hash": self.manifest.declared_hash().to_string(),
                "size_bytes": self.manifest.size_bytes(),
                "state": self.manifest.state().slug(),
                "expires_at": self.expires_at.to_string(),
                // Le manifeste **entier**, sous la forme du schéma. Même raison qu'en `W20.s` pour
                // la proposition : le dépôt du contenu doit relire ce qui a été déclaré, et le
                // faire renvoyer par son déposant laisserait déclarer un hash et en téléverser un
                // autre sous le même identifiant.
                "manifest": self.manifest.to_wire(),
            }),
        )?])
    }
}

/// Le fait de §19.1 : le contenu est arrivé, et son hash est celui qui avait été promis.
#[derive(Debug, Clone)]
pub struct Uploaded {
    /// L'artefact.
    pub artifact_id: String,
    /// Le condensat **observé**, celui que le daemon a calculé.
    pub observed: ContentHash,
    /// La taille reçue.
    pub size_bytes: u64,
    /// Qui a déposé.
    pub worker_id: String,
}

impl Decide for Uploaded {
    type State = LepContext;

    fn decide(
        &self,
        command: &CommandEnvelope,
        context: &Self::State,
    ) -> Result<Vec<EventDraft>, CommandError> {
        Ok(vec![crate::mission::fact(
            command,
            context,
            UPLOADED_FACT,
            &stream_of_artifact(&self.artifact_id),
            serde_json::json!({
                "artifact_id": self.artifact_id,
                "worker_id": self.worker_id,
                // Le hash **observé**. Réécrire le hash déclaré ici ferait du journal le témoin de
                // la promesse et non de sa tenue, et un fait qui ne peut pas différer de la
                // déclaration ne prouve rien. Qu'il soit égal est le résultat de la vérification,
                // pas sa définition.
                "observed_hash": self.observed.to_string(),
                "size_bytes": self.size_bytes,
                "state": locus_artifacts::ArtifactState::Uploaded.slug(),
            }),
        )?])
    }
}

impl<S: EventStore> Runtime<S> {
    /// Déclarer un artefact — la première étape de §19.1.
    ///
    /// Rend l'adresse où en déposer le contenu, et jusqu'à quand.
    ///
    /// # Errors
    ///
    /// [`CommandError::Authorization`] pour une créance que le registre des workers ne connaît pas,
    /// [`CommandError::Validation`] pour un document que le domaine refuse — hash mal formé, type
    /// MIME que le schéma n'accepterait pas, taille nulle ou négative, provenance vide —, et le
    /// refus **nomme le champ**.
    pub fn lep_declare_artifact(
        &self,
        credential: &str,
        document: &WireManifest,
        submitted: &Submitted,
        now: Timestamp,
    ) -> Result<Ticket, CommandError> {
        let identity = self.identify(credential)?;
        let manifest =
            ArtifactManifest::from_wire(document).map_err(|erreur| wire_refuse(&erreur))?;
        let expires_at = crate::lep::expiration(now, UPLOAD_WINDOW_SECONDS)?;
        let declared = Declared {
            manifest,
            worker_id: identity.worker_id.clone(),
            expires_at,
        };
        let stream = stream_of_artifact(declared.manifest.artifact_id());
        self.write_worker_fact(&identity, submitted, &stream, 1, &declared, now)?;
        Ok(Ticket {
            artifact_id: declared.manifest.artifact_id().to_owned(),
            upload_path: upload_path(declared.manifest.artifact_id()),
            expires_at: expires_at.to_string(),
        })
    }

    /// Déposer le contenu d'un artefact déclaré — la seconde étape de §19.1.
    ///
    /// # Ce qui se lit du journal, et pourquoi
    ///
    /// Le manifeste **entier**. Le faire renvoyer par son déposant laisserait déclarer un hash et
    /// en téléverser un autre sous le même identifiant : la confrontation comparerait alors le
    /// contenu à une promesse faite après coup, c'est-à-dire à lui-même. C'est la leçon de `W20.s`
    /// appliquée au second couple de commandes du dépôt.
    ///
    /// # Errors
    ///
    /// [`CommandError::Authorization`] pour une créance inconnue ; [`CommandError::Validation`]
    /// quand aucun artefact n'a été déclaré sous cet identifiant — « la déclaration précède
    /// l'upload » —, quand le contenu reçu n'est pas celui qui avait été promis, ou quand la
    /// fenêtre de dépôt est fermée ; [`CommandError::Unavailable`] quand aucun stockage n'est
    /// câblé.
    pub fn lep_upload_artifact(
        &self,
        credential: &str,
        artifact_id: &str,
        bytes: &[u8],
        submitted: &Submitted,
        now: Timestamp,
    ) -> Result<Receipt, CommandError> {
        let identity = self.identify(credential)?;
        let stream = stream_of_artifact(artifact_id);
        let (manifest, expires_at) = self.declared(&stream, artifact_id)?;
        if now.millis() > expires_at.millis() {
            return Err(CommandError::Validation {
                field: "artifact_id".to_owned(),
                detail: format!(
                    "la déclaration de « {artifact_id} » a expiré : §19.1 ouvre le dépôt pour \
                     {UPLOAD_WINDOW_SECONDS} secondes, et il faut redéclarer plutôt que déposer \
                     sous une promesse que plus personne ne tient"
                ),
            });
        }
        let uploaded = self
            .lep()
            .blobs()
            .receive(manifest, bytes)
            .map_err(ingest_refuse)?;
        let observed = uploaded.declared_hash().clone();
        let fait = Uploaded {
            artifact_id: artifact_id.to_owned(),
            observed: observed.clone(),
            size_bytes: uploaded.size_bytes(),
            worker_id: identity.worker_id.clone(),
        };
        self.write_worker_fact(&identity, submitted, &stream, 1, &fait, now)?;
        Ok(Receipt {
            received_hash: observed.to_string(),
            size_bytes: uploaded.size_bytes(),
        })
    }

    /// Ce que le journal dit de cet artefact : ce qui a été déclaré, et jusqu'à quand.
    ///
    /// # Errors
    ///
    /// [`CommandError::Validation`] quand rien n'a été déclaré sous cet identifiant — c'est une
    /// faute du client, qui dépose sous un artefact qui n'existe pas —, et
    /// [`CommandError::Internal`] quand le fait de déclaration ne se relit pas, ce qui ne peut
    /// venir que d'un journal écrit par une version antérieure et se répare par migration.
    fn declared(
        &self,
        stream: &str,
        artifact_id: &str,
    ) -> Result<(ArtifactManifest, Timestamp), CommandError> {
        let faits = self.transaction_store().read_stream(stream, 0);
        let declaration = faits
            .iter()
            .find(|fait| fait.event_type.to_string() == DECLARED_FACT)
            .ok_or_else(|| CommandError::Validation {
                field: "artifact_id".to_owned(),
                detail: format!(
                    "aucun artefact « {artifact_id} » n'a été déclaré : §19.1 veut que le hash soit \
                     promis **avant** que le contenu arrive, et un contenu reçu sans promesse ne \
                     peut être confronté à rien"
                ),
            })?;
        let document = declaration
            .payload
            .get("manifest")
            .cloned()
            .and_then(|brut| serde_json::from_value::<WireManifest>(brut).ok())
            .ok_or_else(|| CommandError::Internal {
                detail: format!(
                    "le fait de déclaration de « {artifact_id} » ne se relit pas comme un \
                     manifeste : un journal écrit avant `W20.t` n'en porte pas, et cela se répare \
                     par migration — pas en corrigeant la requête"
                ),
            })?;
        let manifest =
            ArtifactManifest::from_wire(&document).map_err(|erreur| wire_refuse(&erreur))?;
        let expires_at = declaration
            .payload
            .get("expires_at")
            .and_then(serde_json::Value::as_str)
            .map(Timestamp::parse)
            .transpose()
            .map_err(|erreur| CommandError::Internal {
                detail: format!("échéance de dépôt illisible pour « {artifact_id} » : {erreur}"),
            })?
            .ok_or_else(|| CommandError::Internal {
                detail: format!(
                    "le fait de déclaration de « {artifact_id} » ne porte pas d'échéance de dépôt"
                ),
            })?;
        Ok((manifest, expires_at))
    }
}

/// Où déposer le contenu de cet artefact.
///
/// Écrit ici et pas dans `http.rs` : c'est [`Ticket`] qui l'annonce, et deux endroits qui
/// construisent le même chemin finissent par en construire deux.
#[must_use]
pub fn upload_path(artifact_id: &str) -> String {
    format!("/lep/v1/artifacts/{artifact_id}/content")
}

/// Ce qu'un document refusé par le domaine devient sur le fil.
///
/// **En nommant le champ.** Un `WireError` dit précisément ce qui ne va pas ; le traduire en
/// « document invalide » perdrait le seul renseignement dont le client a besoin. La famille est
/// `validation` — la requête est mal formée, elle ne le sera pas moins en la retentant.
fn wire_refuse(error: &WireError) -> CommandError {
    let field = match error {
        WireError::UnknownState { .. } => "state",
        WireError::UnknownRelation { .. } => "derived_from.relation",
        WireError::NegativeSize { .. } => "size_bytes",
        WireError::ImpossibleAttempt { .. } => "produced_by.attempt",
        WireError::MalformedHash { .. } => "content_hash",
        WireError::MalformedTimestamp { .. } => "declared_at",
        WireError::Manifest { error } => manifest_field(error),
    };
    CommandError::Validation {
        field: field.to_owned(),
        detail: error.to_string(),
    }
}

/// Le champ que met en cause un manifeste refusé.
fn manifest_field(error: &ManifestError) -> &'static str {
    match error {
        ManifestError::EmptyField { field } => field,
        ManifestError::ZeroSize | ManifestError::SizeBeyondWire { .. } => "size_bytes",
        ManifestError::ZeroAttempt => "produced_by.attempt",
        ManifestError::MalformedMediaType { .. } => "media_type",
        ManifestError::MalformedHash(_) | ManifestError::HashMismatch { .. } => "content_hash",
        ManifestError::Derivation(_) => "derived_from",
        ManifestError::Forbidden(_) => "state",
    }
}

/// Ce qu'un dépôt refusé devient sur le fil.
///
/// # Les trois familles, distinguées
///
/// Un hash qui ne correspond pas est une **faute du client** : il a envoyé autre chose que ce qu'il
/// avait promis, et le refus nomme `content_hash`. Une taille qui déborde l'est aussi. Un stockage
/// absent ne l'est pas — c'est l'exploitant qui n'a rien câblé —, et le rendre `validation` ferait
/// corriger sa requête à un client dont la requête était juste.
fn ingest_refuse(error: IngestError) -> CommandError {
    match error {
        IngestError::Manifest(erreur) => CommandError::Validation {
            field: manifest_field(&erreur).to_owned(),
            detail: erreur.to_string(),
        },
        IngestError::Store(StoreError::NotDeclared { state }) => CommandError::Unavailable {
            detail: format!("le contenu ne peut pas être reçu : {state}"),
        },
        IngestError::Store(erreur) => CommandError::Validation {
            field: "size_bytes".to_owned(),
            detail: erreur.to_string(),
        },
    }
}

/// Le stockage, tel que le composition root le câble.
pub(crate) type SharedBlobs = Arc<dyn Blobs>;
