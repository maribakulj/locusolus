//! La surface §15.2 — par où un worker réclame, remonte et rend. `W20.k`.
//!
//! # Comment cet item a été trouvé
//!
//! En marquant `W2.21`. `canterel` savait parler les trois chemins de §15.2 depuis un sprint, et
//! **personne ne les servait** : `http.rs` portait sept routes, toutes en lecture, aucune sous
//! `/lep/`. Trois lignes de roadmap désignaient ce trou sans le nommer — deux marqueurs successifs
//! de `W23.b` qui visaient des jalons voisins, et une liste de dépendances de `W12.d` à laquelle il
//! en manquait une. Aucune n'avait d'item à viser, parce qu'il n'y en avait pas.
//!
//! C'est le cinquième maillon de la fermeture verticale, et il est resté invisible plus longtemps
//! que les quatre autres : avant qu'un client existe, « personne ne parle §15.2 » se lisait comme
//! l'inertie du worker, et non comme un trou du daemon.
//!
//! # Les types du fil sont ceux de `packages/lep`, jamais des miroirs
//!
//! [`MissionEnvelope`], [`Lease`] et [`Event`] sont **générés** depuis les JSON Schemas de `W0.5`,
//! les mêmes qui génèrent le `lep/generated.ts` que `canterel` consomme. Les deux moitiés du fil
//! viennent donc d'un seul schéma : un changement de schéma casse les deux côtés à la compilation.
//!
//! Les redéclarer ici aurait produit un miroir, et un miroir ne diverge pas bruyamment — il diverge
//! le jour où l'un des deux est corrigé, et rien ne le dit.
//!
//! # « Rien pour toi » n'est pas « je n'ai pas pu demander »
//!
//! Une réclamation sans mission assignable rend `204`, et **non** une erreur. Les deux envoient
//! chercher à des endroits opposés — un ordonnanceur qui n'a rien à donner, ou un lien cassé. C'est
//! la séparation que l'ADR 0028 décision 4 tient pour le broker, et `W2.21` la tient déjà de l'autre
//! côté du fil : ce module la rend vraie des deux.
//!
//! # Deux ports, et leurs implémentations de référence
//!
//! Rien dans ce dépôt ne **crée** encore de mission, et rien n'enrôle un worker côté serveur. Deux
//! réponses étaient possibles, et une seule est admise ici.
//!
//! Reporter l'item aurait été dire « aucun appelant ne l'utilise encore », ce que l'ADR 0022
//! décision 0 refuse comme motif. Fabriquer un ordonnanceur en passant aurait été bâtir une
//! fonctionnalité pour justifier une surface — ce que `W21.g` a refusé sous ce nom.
//!
//! Ce module livre donc [`MissionQueue`] et [`WorkerRegistry`] comme **ports**, avec leur
//! implémentation de référence en mémoire. C'est la forme de `packages/event-store`, construit
//! avant tout écrivain, et celle que l'ADR 0026 décision 0 reconnaît comme une capacité finie. Ce
//! qui les remplira en production est nommé — `W23.c` pour l'ordonnancement — et non simulé.
//!
//! # Ce que cet item rend observable et ne corrige pas — `W20.l`
//!
//! `Runtime::catch_up` prend `&mut self`, et la liaison HTTP ne tient qu'un `&Runtime`. Tant que la
//! surface était en lecture seule, cela n'avait aucune conséquence : rien n'était écrit pendant que
//! le daemon servait, donc rien ne pouvait devenir périmé. Ces trois routes écrivent, et **les
//! quatre projections de §9.5 ne voient jamais ce qu'elles écrivent** : `/workers` reste vide alors
//! qu'un worker a réclamé et rendu.
//!
//! Ce n'est pas un défaut introduit ici — c'est un défaut que cet item **rend visible**, et le
//! corriger demande de décider comment une écriture fait avancer une projection, ce qui est le
//! frère de la décision que l'ADR 0029 a prise pour la sérialisation des écritures. Il a donc son
//! item plutôt qu'un coin de celui-ci, et un test de `tests/lep.rs` atteste l'état actuel de façon
//! à rougir le jour où il change.
//!
//! # Ce que ce module n'a pas
//!
//! Il ne fabrique aucun identifiant : ni `event_id`, ni `command_id`. Cela demanderait de
//! l'entropie, donc un crate, donc un ADR — la lacune que [`crate::branch::BranchContext`] a
//! nommée plutôt que comblée en passant, et [`LepContext`] la nomme de la même façon.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError, RwLock};

use locus_broker::port::{BrokerError, BrokerPort, Loopback, Placement};
use locus_domain::task::TaskState;
use locus_event_store::{Actor, ActorKind, Draft as EventDraft, EventStore, EventType};
use locus_lep::{CapabilityManifest, Event, Lease, MissionEnvelope};
use locus_protocol::Id;
use locus_protocol::Timestamp;
use locus_protocol::id::{Agent, Command as CommandId, Event as EventId, Project, Workspace};

use crate::command::CommandEnvelope;
use crate::composition::Runtime;
use crate::error::{CommandError, Revision};
use crate::handler::Decide;
use crate::outcome::Outcome;

/// Ce qu'une réclamation rend quand il y a du travail — la paire de §15.4 et §15.5.
///
/// La mission **et** son bail, jamais l'une sans l'autre : une mission sans bail n'autorise rien, et
/// un bail sans mission n'a pas d'objet. `canterel` les lit sous ce nom (`Offer`), et les séparer en
/// deux appels aurait ouvert la fenêtre où un worker tient l'un et pas l'autre.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Offer {
    /// La mission, telle que §15.4 la décrit.
    pub mission: MissionEnvelope,
    /// Le bail qui l'autorise, tel que §15.5 le décrit.
    pub lease: Lease,
}

/// Une mission en attente, et le rang d'attempt que la proposition lui a fixé — `W20.v`.
///
/// # Pourquoi le rang voyage à côté et non dans la mission
///
/// `MissionEnvelope` est un type **généré** depuis les JSON Schemas de `W0.5` : il porte
/// `attempt_id`, l'identité de la tentative, et pas son **rang**. Les deux sont distincts par
/// construction — §11.1 : « aucune de ces identités ne doit être substituée aux autres » — et
/// ajouter un champ au schéma pour cela toucherait les deux moitiés du fil, donc `W0.5`, donc
/// `canterel`.
///
/// Le rang est de toute façon une donnée de **file**, pas de mission : c'est ce que
/// [`crate::mission::Proposal`] a fixé, et ce que le bail reprendra. Il vit donc ici.
#[derive(Debug, Clone, PartialEq)]
pub struct Queued {
    /// Ce que la mission demande — §15.4.
    pub mission: MissionEnvelope,
    /// Le rang de la tentative, tel que §12.3 le veut : **conservé** par une réattribution.
    pub attempt: i64,
}

/// D'où viennent les missions assignables.
///
/// # Un port, parce que ce qui les produira n'existe pas
///
/// L'ordonnancement d'instances est `W23.c` ; la création de missions viendra avec la chaîne
/// complète. En attendant, ce trait dit **exactement** ce dont la surface a besoin — prendre la
/// prochaine mission s'il y en a une — et rien de plus. Un port plus large aurait anticipé un
/// ordonnanceur que personne n'a écrit.
///
/// # La file porte des **missions**, jamais des paires — `W20.v`
///
/// Elle a porté des [`Offer`] pendant deux items, et c'était faux. Un bail n'a pas d'objet avant
/// qu'un worker soit choisi : §12.3 en fait « le droit, borné dans le temps, d'exécuter un attempt »
/// **pour un worker**. Une file de paires ne peut donc contenir que des missions **déjà
/// attribuées** — et [`Claim`] refuse un bail dont le `worker_id` n'est pas le réclamant, ce qu'un
/// test tient depuis `W20.k`.
///
/// La conséquence était plus grave que la maladresse : la question de placement que `W20.q` pose au
/// broker ne pouvait jamais que **confirmer** le worker que le bail avait déjà choisi. Le placement
/// était décoratif, et rien ne le disait.
pub trait MissionQueue: Send + Sync {
    /// Déposer une mission — `W20.o`.
    ///
    /// Sur le trait et non sur l'implémentation de référence : la file cesse d'être garnie par un
    /// test, donc c'est le **daemon** qui dépose, donc tout port doit savoir le faire. La laisser
    /// hors du trait ferait de `MemoryQueue` la seule file remplissable.
    fn enqueue(&self, queued: Queued);

    /// Retirer la prochaine mission destinée à ce worker, s'il y en a une.
    ///
    /// `None` veut dire « rien à donner », jamais « je n'ai pas pu regarder ». Une file qui ne sait
    /// pas répondre doit le dire par un autre chemin que celui du calme : c'est la règle de
    /// `W22.e` — une ignorance n'est pas une absence — et la file de référence, elle, sait toujours.
    fn take(&self, worker_id: &str) -> Option<Queued>;
}

/// Ce qu'une créance reconnue désigne — §7.2, ce que l'enrôlement lie.
///
/// # Les trois vont ensemble, et le worker n'en choisit aucun
///
/// Un worker qui annoncerait son workspace pourrait écrire dans n'importe lequel. Un worker qui
/// annoncerait son principal signerait au nom de qui il veut. Les deux viennent donc du **registre**,
/// c'est-à-dire de ce que l'enrôlement a lié, et jamais du corps de la requête.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerIdentity {
    /// Le worker, sous le nom que LEP lui donne.
    pub worker_id: String,
    /// Le workspace dans lequel ses faits s'écrivent.
    pub workspace_id: Id<Workspace>,
    /// Le principal sous lequel il agit.
    pub principal_id: Id<Agent>,
}

/// Qui a le droit de parler à cette surface.
///
/// # Pourquoi la créance ne rend pas un booléen
///
/// Un booléen aurait obligé l'appelant à croire sur parole le `worker_id` que le corps de la requête
/// annonce, alors que c'est la **créance** qui dit qui parle. Un worker qui réclamerait au nom d'un
/// autre est précisément ce que §7 existe pour empêcher, et un port qui rend `bool` le rend
/// inexprimable côté appelant.
pub trait WorkerRegistry: Send + Sync {
    /// L'identité que porte cette créance, ou `None` si elle n'en porte aucune.
    ///
    /// Un worker **révoqué** n'en porte plus : §7.4. `None` couvre donc « inconnu » et « révoqué »,
    /// et c'est voulu — les distinguer dans la réponse dirait à qui interroge lequel des deux, ce
    /// qui transforme la surface en oracle d'existence.
    fn identify(&self, credential: &str) -> Option<WorkerIdentity>;

    /// Reconnaître une créance émise par l'enrôlement de §7.2 — `W20.n`.
    ///
    /// Sur le trait et non sur l'implémentation de référence : un registre réel devra l'implémenter
    /// aussi, et le laisser hors du port ferait de `MemoryRegistry` le seul enrôlable.
    fn admit_enrolled(&self, credential: &str, identity: WorkerIdentity);

    /// Cesser de reconnaître ce worker — §7.4.
    ///
    /// **Rien n'est effacé du journal** : `worker.revoked` y est écrit par ailleurs, et ce registre
    /// n'est qu'un cache de ce que le journal dit. L'invariant 12 porte sur les faits, pas sur les
    /// index qui les servent.
    fn revoke(&self, worker_id: &str);
}

/// D'où viennent les identifiants que ce crate n'a pas le droit de fabriquer.
///
/// # Pourquoi un port, et pourquoi son défaut **refuse**
///
/// `EventDraft` exige un `event_id` et `CommandEnvelope` un `command_id`. Les fabriquer demanderait
/// de l'entropie, donc un crate, donc un ADR et une entrée dans `dependencies.json` — la lacune que
/// [`crate::branch::BranchContext`] a nommée plutôt que comblée en passant.
///
/// Jusqu'ici la lacune se contournait : `branch.rs` **exige** un contexte, et c'est l'appelant qui
/// fournit. Une route HTTP n'a pas d'appelant à qui demander. Le port entre donc ici, et son
/// implémentation par défaut ne fabrique rien : elle **refuse**, en nommant ce qui manque.
///
/// Un défaut qui rendrait des identifiants séquentiels aurait été pire que le refus. Il aurait
/// marché en test, marché au premier démarrage, et réattribué les mêmes identités au redémarrage
/// suivant — un journal dont deux faits différents portent la même identité, découvert des mois
/// plus tard. Ce qui n'est pas vérifié n'est jamais réussi, et une identité qu'on n'a pas su tirer
/// ne vaut pas la précédente plus un.
pub trait Identities: Send + Sync {
    /// `count` identités d'événement, ou le refus qui dit ce qui manque.
    ///
    /// # Errors
    ///
    /// [`CommandError::Unavailable`] quand aucune source n'est câblée : le service ne peut pas
    /// répondre **maintenant**, ce qui est exact et se répare par configuration — au contraire d'un
    /// `Internal`, qui enverrait chercher un défaut dans le code.
    fn events(&self, count: usize) -> Result<Vec<Id<EventId>>, CommandError>;

    /// Une identité de commande, ou le même refus.
    ///
    /// # Errors
    ///
    /// [`CommandError::Unavailable`], comme ci-dessus.
    fn command(&self) -> Result<Id<CommandId>, CommandError>;

    /// Une identité de bail — `W20.v`.
    ///
    /// Une méthode de plus plutôt qu'un réemploi de [`Identities::command`] : un bail n'est pas une
    /// commande, et emprunter l'identité de l'une pour l'autre ferait qu'un `lease_id` et un
    /// `command_id` désigneraient la même valeur. §11.1 : « aucune de ces identités ne doit être
    /// substituée aux autres ».
    ///
    /// # Errors
    ///
    /// [`CommandError::Unavailable`], comme ci-dessus.
    fn lease(&self) -> Result<Id<CommandId>, CommandError>;
}

/// La source par défaut : aucune. Elle refuse, et dit pourquoi.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoIdentities;

impl NoIdentities {
    fn refusal(what: &str) -> CommandError {
        CommandError::Unavailable {
            detail: format!(
                "aucune source d'identifiants n'est câblée : `locusd` ne tire pas d'entropie — cela \
                 demande un crate, donc un ADR et une entrée dans `dependencies.json` — et il refuse \
                 d'inventer un {what} plutôt que d'en réattribuer un au redémarrage suivant"
            ),
        }
    }
}

impl Identities for NoIdentities {
    fn events(&self, _count: usize) -> Result<Vec<Id<EventId>>, CommandError> {
        Err(Self::refusal("identifiant d'événement"))
    }

    fn command(&self) -> Result<Id<CommandId>, CommandError> {
        Err(Self::refusal("identifiant de commande"))
    }

    fn lease(&self) -> Result<Id<CommandId>, CommandError> {
        Err(Self::refusal("identifiant de bail"))
    }
}

/// La file de référence — en mémoire, alimentée par qui la détient.
///
/// Elle n'ordonnance rien : elle rend dans l'ordre où on l'a remplie. Un tri par priorité serait
/// une décision d'ordonnancement, donc `W23.c`, et l'écrire ici la rendrait invisible.
#[derive(Debug, Default)]
pub struct MemoryQueue {
    missions: Mutex<VecDeque<Queued>>,
}

impl MemoryQueue {
    /// Une file vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Déposer une mission.
    pub fn push(&self, queued: Queued) {
        self.missions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push_back(queued);
    }

    /// Ce qui reste à distribuer.
    #[must_use]
    pub fn len(&self) -> usize {
        self.missions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// Vrai quand plus rien n'attend.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl MissionQueue for MemoryQueue {
    fn enqueue(&self, queued: Queued) {
        self.push(queued);
    }

    /// Le `worker_id` est reçu et **délibérément ignoré** par cette implémentation.
    ///
    /// Le port le porte parce qu'un ordonnanceur réel en aura besoin — placer selon ce qu'un hôte a
    /// prouvé est `W4.g`. La file de référence, elle, ne place rien, et le dire ici vaut mieux que
    /// laisser croire qu'elle filtre.
    fn take(&self, _worker_id: &str) -> Option<Queued> {
        self.missions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
    }
}

/// Le registre de référence — en mémoire, rempli par qui le détient.
///
/// L'enrôlement de §7.2 côté serveur n'existe pas ; `W2.4` en a livré la moitié cliente. Ce registre
/// est ce dont la surface a besoin pour refuser, et il ne prétend pas être un enrôlement.
#[derive(Debug, Default)]
pub struct MemoryRegistry {
    known: RwLock<Vec<(String, WorkerIdentity)>>,
    /// Les workers révoqués — §7.4. Une **liste de plus**, jamais un retrait de `known` : le
    /// registre doit pouvoir dire « je connais ce worker et il est révoqué », ce qu'un effacement
    /// rendrait indistinguable de « je ne l'ai jamais vu ».
    revoked: RwLock<Vec<String>>,
}

impl MemoryRegistry {
    /// Un registre vide — donc un daemon qui refuse tout le monde.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reconnaître une créance comme étant celle de ce worker.
    pub fn admit(&self, credential: &str, identity: WorkerIdentity) {
        self.known
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .push((credential.to_owned(), identity));
    }
}

impl WorkerRegistry for MemoryRegistry {
    fn identify(&self, credential: &str) -> Option<WorkerIdentity> {
        let revoked = self.revoked.read().unwrap_or_else(PoisonError::into_inner);
        self.known
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .find(|(known, _)| known == credential)
            .filter(|(_, identity)| !revoked.contains(&identity.worker_id))
            .map(|(_, identity)| identity.clone())
    }

    fn admit_enrolled(&self, credential: &str, identity: WorkerIdentity) {
        self.admit(credential, identity);
    }

    fn revoke(&self, worker_id: &str) {
        self.revoked
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .push(worker_id.to_owned());
    }
}

/// Les deux ports, réunis pour que le composition root n'en câble qu'un champ.
///
/// Les `Arc<dyn …>` plutôt que des paramètres de type sur `Runtime` : substituer une file réelle ne
/// doit pas obliger à toucher `composition.rs`, et c'est exactement ce que le paramètre `S` du
/// journal garantit déjà pour l'event store.
#[derive(Clone)]
pub struct Desk {
    queue: Arc<dyn MissionQueue>,
    registry: Arc<dyn WorkerRegistry>,
    identities: Arc<dyn Identities>,
    enrollment: Arc<dyn crate::enrollment::EnrollmentTokens>,
    broker: Arc<dyn BrokerPort + Send + Sync>,
    administrators: Arc<dyn crate::mission::Administrators>,
}

impl std::fmt::Debug for Desk {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Desk").finish_non_exhaustive()
    }
}

/// Ce que dit un daemon dont personne n'a câblé le broker.
///
/// Pas une panne feinte : c'est **exactement** la situation, dite avec la phrase qui envoie au bon
/// endroit. La rendre `Placed` par défaut aurait confié des missions sans que rien ne les place ;
/// la rendre `NotPlaced` aurait dit « ce worker ne convient pas » alors que personne ne l'a
/// regardé — l'ignorance rangée comme une absence, ce que `W22.e` refuse partout ailleurs.
fn broker_absent() -> Loopback {
    Loopback::unreachable(
        "aucun broker n'est câblé",
        "`Desk::default()` n'en porte pas : un placement demande `locus-execd`, et ce daemon ne \
         décide d'aucun hôte lui-même",
    )
}

impl Default for Desk {
    /// Une file vide, un registre vide, et aucun broker : un daemon qui répond `204` à tout le monde
    /// et refuse toute créance. C'est exact, et c'est ce qu'un daemon sans ordonnanceur doit faire.
    fn default() -> Self {
        Self {
            queue: Arc::new(MemoryQueue::new()),
            registry: Arc::new(MemoryRegistry::new()),
            identities: Arc::new(NoIdentities),
            enrollment: Arc::new(crate::enrollment::MemoryTokens::new()),
            broker: Arc::new(broker_absent()),
            administrators: Arc::new(crate::mission::NoAdministrators),
        }
    }
}

impl Desk {
    /// Câbler une file, un registre et une source d'identifiants.
    #[must_use]
    pub fn new(
        queue: Arc<dyn MissionQueue>,
        registry: Arc<dyn WorkerRegistry>,
        identities: Arc<dyn Identities>,
    ) -> Self {
        Self {
            queue,
            registry,
            identities,
            enrollment: Arc::new(crate::enrollment::MemoryTokens::new()),
            broker: Arc::new(broker_absent()),
            administrators: Arc::new(crate::mission::NoAdministrators),
        }
    }

    /// Câbler l'émetteur de tokens d'enrôlement — `W20.n`.
    #[must_use]
    pub fn enrolling(mut self, tokens: Arc<dyn crate::enrollment::EnrollmentTokens>) -> Self {
        self.enrollment = tokens;
        self
    }

    /// Câbler le registre d'administration — `W20.s`.
    #[must_use]
    pub fn administering(
        mut self,
        administrators: Arc<dyn crate::mission::Administrators>,
    ) -> Self {
        self.administrators = administrators;
        self
    }

    /// Le registre d'administration, en lecture.
    #[must_use]
    pub fn administrators(&self) -> &dyn crate::mission::Administrators {
        self.administrators.as_ref()
    }

    /// Câbler le broker qui décide du placement — `W20.q`.
    #[must_use]
    pub fn placing(mut self, broker: Arc<dyn BrokerPort + Send + Sync>) -> Self {
        self.broker = broker;
        self
    }

    /// Le broker, en lecture.
    #[must_use]
    pub fn broker(&self) -> &(dyn BrokerPort + Send + Sync) {
        self.broker.as_ref()
    }

    /// L'émetteur de tokens, en lecture.
    #[must_use]
    pub fn tokens(&self) -> &dyn crate::enrollment::EnrollmentTokens {
        self.enrollment.as_ref()
    }

    /// La file, en lecture.
    #[must_use]
    pub fn queue(&self) -> &dyn MissionQueue {
        self.queue.as_ref()
    }

    /// Le registre, en lecture.
    #[must_use]
    pub fn registry(&self) -> &dyn WorkerRegistry {
        self.registry.as_ref()
    }

    /// La source d'identifiants, en lecture.
    #[must_use]
    pub fn identities(&self) -> &dyn Identities {
        self.identities.as_ref()
    }
}

/// Ce qu'un fait de §15.2 a besoin de savoir et que LEP ne porte pas.
///
/// Même motif que [`crate::branch::BranchContext`] : `EventDraft` exige un `event_id` et un
/// `project_id`, et rien dans ce crate ne fabrique d'identifiants — cela demanderait de l'entropie,
/// donc un crate, donc un ADR. Le contexte est **fourni**, et la lacune nommée plutôt que comblée à
/// la sauvette.
///
/// # Pourquoi une réserve d'identités, et pas une seule
///
/// Une remontée d'événements écrit **un fait par événement**, et deux faits ne peuvent pas porter
/// la même identité. Une première rédaction décalait une identité unique par le rang du fait —
/// `event_id.offset(rank)`. Cette méthode n'existe pas, et c'est heureux : elle aurait fabriqué des
/// identifiants par arithmétique, c'est-à-dire exactement l'entropie que ce crate n'a pas le droit
/// d'inventer, sous couvert de déterminisme.
///
/// La réserve est donc **fournie**, et un décideur à qui il en manque **refuse en disant combien** —
/// il n'en fabrique pas. Un compteur qui n'a rien lu ne vaut pas zéro, et une identité qu'on n'a pas
/// reçue ne se déduit pas de celle d'à côté.
#[derive(Debug, Clone)]
pub struct LepContext {
    /// Le projet auquel les faits appartiennent.
    pub project_id: Id<Project>,
    /// Les identités disponibles, une par fait à écrire.
    pub event_ids: Vec<Id<EventId>>,
    /// Quand l'acte a eu lieu — distinct de l'instant d'écriture (§10.1).
    pub occurred_at: Timestamp,
    /// Le hash de la charge canonicalisée.
    pub payload_hash: String,
}

impl LepContext {
    /// L'identité de rang `rank`, ou le refus qui dit ce qui manque.
    ///
    /// # Errors
    ///
    /// [`CommandError::Validation`] nommant le champ et le compte : un exploitant doit lire
    /// « il en fallait 3, j'en ai reçu 1 », pas « erreur interne ».
    pub fn identity(&self, rank: usize) -> Result<Id<EventId>, CommandError> {
        self.event_ids
            .get(rank)
            .copied()
            .ok_or_else(|| CommandError::Validation {
                field: "context.event_ids".to_owned(),
                detail: format!(
                    "{} identité(s) fournie(s) pour au moins {} fait(s) à écrire : ce crate ne \
                     fabrique pas d'identifiants, il refuse d'en manquer",
                    self.event_ids.len(),
                    rank.saturating_add(1)
                ),
            })
    }
}

/// Combien de temps un bail vaut, en secondes — §12.3, « courte et renouvelable ».
///
/// # Une valeur de politique, pas une propriété du protocole
///
/// Le schéma de `Lease` le dit lui-même : « aucune borne haute ici : c'est une politique de
/// scheduler ». Elle vit donc dans le daemon, sous un nom, et non dans le fil.
///
/// Cinq minutes : assez pour qu'un worker qui redémarre son processus ne perde pas sa tâche, assez
/// court pour qu'une machine morte la rende avant qu'on s'en aperçoive à la main.
pub const LEASE_TTL_SECONDS: i64 = 300;

/// À quel intervalle un worker doit donner signe de vie — §12.3.
///
/// La contrainte de §12.3 est une **relation** : « le worker envoie un heartbeat à intervalle
/// inférieur au tiers du TTL ». Draft 7 ne sait pas énoncer une relation entre deux champs, et le
/// schéma le dit plutôt que de laisser croire qu'il la couvre. Elle est donc tenue ici par un test.
///
/// Le tiers, et non la moitié : il faut que **deux** heartbeats puissent se perdre sans que le bail
/// expire, sinon un réseau qui hoquette rend un worker vivant indistinguable d'un worker mort — et
/// c'est exactement la distinction que le bail existe pour faire.
pub const HEARTBEAT_INTERVAL_SECONDS: i64 = 30;

/// La relation de §12.3, tenue **à la compilation**.
///
/// « Le worker envoie un heartbeat à intervalle inférieur au tiers du TTL. » Le schéma de `Lease`
/// dit lui-même qu'il ne peut pas l'énoncer — c'est une relation entre deux champs, et Draft 7 n'en
/// exprime pas — et qu'elle est donc vérifiée ailleurs. « Ailleurs » n'existait nulle part côté
/// serveur.
///
/// Elle est ici, et sous la forme la plus forte disponible : un `const` qui ne compile pas si la
/// relation cesse d'être vraie. Un test aurait dit la même chose plus tard et moins fort ; ce qui
/// suit rend le réglage fautif **inexprimable**, ce que ce dépôt préfère partout où c'est possible.
const _: () = assert!(
    HEARTBEAT_INTERVAL_SECONDS * 3 <= LEASE_TTL_SECONDS,
    "§12.3 : l'intervalle de heartbeat doit tenir sous le tiers du TTL — deux battements perdus ne \
     doivent pas suffire à faire expirer le bail d'un worker vivant"
);

/// L'instant où un bail cesse de valoir.
///
/// Publique pour être **éprouvable** : le débordement ci-dessous n'est atteignable par aucune
/// horloge réelle, et une garde qu'aucun test ne peut atteindre est une garde qu'on croit tenir.
///
/// # Un débordement se dit, il ne se replie pas
///
/// `Timestamp` porte des millisecondes signées. Une addition qui déborderait et se replierait
/// produirait une expiration **dans le passé** — c'est-à-dire un bail né expiré, que `W2.9` traite
/// comme un bail perdu. Le refus est donc explicite, même si aucune valeur réelle n'en approche :
/// ce qui n'est pas vérifié n'est jamais réussi.
///
/// # Errors
///
/// [`CommandError::Internal`] sur débordement — ce n'est pas la faute du client, et rien dans sa
/// requête ne le corrigerait.
pub fn expiration(now: Timestamp, ttl_seconds: i64) -> Result<Timestamp, CommandError> {
    ttl_seconds
        .checked_mul(1_000)
        .and_then(|millis| now.millis().checked_add(millis))
        .map(Timestamp::from_millis)
        .ok_or_else(|| CommandError::Internal {
            detail: format!(
                "l'échéance d'un bail de {ttl_seconds} s déborde depuis {} : un bail qui repartirait \
                 dans le passé naîtrait expiré",
                now.millis()
            ),
        })
}

/// Le stream d'une tâche. Un seul écrivain par tâche, donc un seul verrou — `W20.h`.
#[must_use]
pub fn stream_of_task(task_id: &str) -> String {
    format!("task/{task_id}")
}

/// La réclamation : une offre devient un fait.
///
/// # Pourquoi c'est une commande et non une lecture
///
/// Réclamer **change l'état** : la tâche passe de `queued` à `leased`, et c'est ce fait que
/// `W23.b` compte. Une réclamation servie comme une lecture rendrait une mission sans que rien
/// n'atteste qu'elle a été confiée — et deux workers pourraient recevoir la même.
pub struct Claim {
    /// L'offre confiée : la mission retirée de la file, et le bail frappé pour ce worker.
    ///
    /// Privée au module, comme [`Complete::worker_id`] et pour la même raison — `W20.v` a rendu
    /// **inexprimable** ce que deux gardes vérifiaient jusqu'ici.
    offer: Offer,
    /// Le worker qui réclame, tel que **la créance** l'identifie.
    worker_id: String,
}

impl Decide for Claim {
    type State = LepContext;

    /// # Les deux gardes d'appariement ont disparu, et ce n'est pas un relâchement
    ///
    /// Jusqu'à `W20.v`, ce décideur vérifiait deux choses : que le bail désignait le réclamant, et
    /// qu'il désignait la mission qu'il accompagne. Les deux étaient nécessaires, parce que la file
    /// portait des paires **déjà formées** — un bail y arrivait avec un `worker_id` que personne
    /// n'avait confronté au réclamant.
    ///
    /// Depuis `W20.v`, le bail est frappé par [`Runtime::lep_claim`] **depuis** le worker admis et
    /// **depuis** la mission retirée. Les deux appariements sont donc vrais par construction, et les
    /// gardes ne pouvaient plus se déclencher : c'est ce que `W20.n` a fait à
    /// `Rejection::WrongEndpoint`, et la règle est la même — une garde qu'aucun appelant ne peut
    /// déclencher se supprime, elle ne se conserve pas « au cas où ».
    ///
    /// Ce qu'elles protégeaient n'est pas perdu, il a changé de nature : les champs sont privés,
    /// donc personne hors de ce module ne peut composer une paire dépareillée, et deux tests
    /// vérifient sur l'offre **servie** que le bail nomme bien son worker et sa tâche.
    fn decide(
        &self,
        command: &CommandEnvelope,
        context: &Self::State,
    ) -> Result<Vec<EventDraft>, CommandError> {
        Ok(vec![fact(
            command,
            context,
            0,
            "task.leased",
            &stream_of_task(&self.offer.mission.task_id),
            serde_json::json!({
                "task_id": self.offer.mission.task_id,
                "attempt": self.offer.lease.attempt,
                "lease_id": self.offer.lease.lease_id,
                "worker_id": self.worker_id,
                "state": TaskState::Leased.to_string(),
            }),
        )?])
    }
}

/// Les événements que le worker fait remonter — §15.6.
///
/// # Un lot, et un seul `append`
///
/// Le port [`Decide`] rend un `Vec` parce qu'un lot s'écrit d'un bloc : c'est ce qui rend l'échec
/// sans trace. Faire remonter les événements un par un laisserait un journal à moitié rempli quand
/// la deuxième écriture échoue.
pub struct Report {
    /// Les événements, dans l'ordre où le worker les a produits.
    pub events: Vec<Event>,
    /// Le worker qui parle, tel que **la créance** l'identifie.
    pub worker_id: String,
}

impl Decide for Report {
    type State = LepContext;

    fn decide(
        &self,
        command: &CommandEnvelope,
        context: &Self::State,
    ) -> Result<Vec<EventDraft>, CommandError> {
        if self.events.is_empty() {
            return Err(CommandError::Validation {
                field: "events".to_owned(),
                detail: "aucun événement : une remontée vide n'est pas un fait à écrire".to_owned(),
            });
        }

        let mut drafts = Vec::with_capacity(self.events.len());
        for (rank, event) in self.events.iter().enumerate() {
            // Un événement qui prétend venir d'un autre worker ne passe pas. La créance dit qui
            // parle ; le corps de la requête n'est qu'une déclaration.
            if let Some(claimed) = event.worker_id.as_deref()
                && claimed != self.worker_id
            {
                return Err(CommandError::Authorization {
                    action: format!(
                        "faire remonter un événement au nom de « {claimed} » sous la créance de « {} »",
                        self.worker_id
                    ),
                });
            }
            let Some(task_id) = event.task_id.as_deref() else {
                return Err(CommandError::Validation {
                    field: format!("events[{rank}].task_id"),
                    detail: "sans tâche, un événement n'a pas de stream où atterrir".to_owned(),
                });
            };
            // Tous les événements d'un lot visent la même tâche : la transaction verrouille **par
            // stream**, et un lot qui en viserait deux ne pourrait pas s'écrire d'un bloc.
            if task_id != first_task(&self.events) {
                return Err(CommandError::Validation {
                    field: format!("events[{rank}].task_id"),
                    detail: "un lot vise une seule tâche : l'atomicité inter-streams n'est pas \
                             tenable, et la promettre serait pire que la refuser"
                        .to_owned(),
                });
            }

            // Un commit de §15.7 n'est pas un événement de progression de §15.6, et le traiter comme
            // tel a coûté cher : le fait écrit portait la sérialisation de l'événement LEP, donc ni
            // `status` ni `validation_level`, donc la projection de §9.3 passait en quarantaine —
            // et un daemon dont une projection est en quarantaine ne redémarre pas. Le worker, lui,
            // recevait `202`. `W20.r` lui donne son chemin, et celui-ci écrit les deux champs.
            if let Some(mut commit) = crate::epistemic::staging(event, task_id, &self.worker_id) {
                commit.rank = rank;
                drafts.extend(commit.decide(command, context)?);
                continue;
            }

            drafts.push(fact(
                command,
                context,
                rank,
                &lep_event_type(&event.event_type),
                &stream_of_task(task_id),
                serde_json::to_value(event).unwrap_or(serde_json::Value::Null),
            )?);
        }
        Ok(drafts)
    }
}

/// Le résultat rendu : la tentative s'achève, et **le fait atteint le journal**.
///
/// C'est le fait exact que `W23.b` compte — `task.leased` l'ouvre, celui-ci le referme — et rien
/// d'autre dans ce dépôt ne le faisait exister.
///
/// # Ce que ce fait ne dit **pas**, et pourquoi
///
/// Il ne transitionne pas la tâche vers `succeeded` ou `failed`. Le corps que `W2.21` envoie ne
/// porte aucune issue : `{task_id, attempt_id, session_id, output}`, et rien de plus. En déduire un
/// succès parce qu'un résultat est arrivé serait affirmer ce que personne n'a dit — le motif de
/// l'ADR 0025, et le plus coûteux ici, puisque §7.1 fait de `succeeded` le contrat **technique**
/// rempli, que l'institution lit ensuite pour décider d'accepter.
///
/// Le fait écrit est donc l'achèvement de la **tentative**, qui a réellement eu lieu. Faire porter
/// l'issue au fil est un item à part : il touche le schéma LEP, donc les deux moitiés, donc `W0.5`.
pub struct Complete {
    /// Ce que le worker a rendu.
    pub rendered: Rendered,
    /// Le worker qui rend, tel que **la créance** l'identifie.
    ///
    /// Privé au module : [`Runtime::lep_result`] le pose, et personne d'autre ne peut. Une première
    /// rédaction le laissait public et le faisait écraser par l'appelant ; une passe de mutation a
    /// montré que la route pouvait y écrire n'importe quoi sans qu'aucun test bouge — vrai, puisque
    /// la valeur était toujours remplacée. Un champ qu'on écrase est un champ qu'on peut oublier
    /// d'écraser : celui-ci est devenu **inécrivable** de l'extérieur, ce qui vaut mieux que de
    /// chercher la faute.
    worker_id: String,
}

/// Ce qu'un worker rend, et rien de plus — **pas** son identité.
///
/// Elle vient de la créance, jamais du corps. Ne pas la mettre ici est ce qui rend l'usurpation
/// inexprimable sur ce chemin, plutôt que rattrapée par un écrasement qu'un refactor pourrait
/// perdre.
#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    /// La tâche.
    pub task_id: String,
    /// La tentative, sous l'identité que §11.1 lui donne — jamais réinventée par le worker (§15.5).
    pub attempt_id: String,
    /// La session amont qui l'a produite.
    pub session_id: String,
    /// Ce que la tentative a produit, opaque ici : le daemon le transporte, il ne l'interprète pas.
    pub output: serde_json::Value,
}

impl Decide for Complete {
    type State = LepContext;

    fn decide(
        &self,
        command: &CommandEnvelope,
        context: &Self::State,
    ) -> Result<Vec<EventDraft>, CommandError> {
        if self.rendered.task_id.is_empty() {
            return Err(CommandError::Validation {
                field: "task_id".to_owned(),
                detail: "sans tâche, un résultat n'a pas de stream où atterrir".to_owned(),
            });
        }
        Ok(vec![fact(
            command,
            context,
            0,
            "run.completed",
            &stream_of_task(&self.rendered.task_id),
            serde_json::json!({
                "task_id": self.rendered.task_id,
                "attempt_id": self.rendered.attempt_id,
                "session_id": self.rendered.session_id,
                "worker_id": self.worker_id,
                "output": self.rendered.output,
            }),
        )?])
    }
}

/// La tâche que vise le premier événement d'un lot, ou la chaîne vide.
fn first_task(events: &[Event]) -> &str {
    events
        .first()
        .and_then(|event| event.task_id.as_deref())
        .unwrap_or_default()
}

/// Le type d'événement du journal pour un type LEP de §15.6.
///
/// # Pourquoi une traduction, et pourquoi elle ne peut pas mentir
///
/// §15.6 nomme `attempt.started`, `heartbeat`, `progress` — une taxonomie de **protocole**. §10.3
/// nomme les namespaces du **journal**, dont `attempt` ne fait pas partie et `run` oui. Les deux ne
/// se recouvrent pas, et faire passer l'une pour l'autre écrirait des faits dans un namespace que
/// personne ne relit.
///
/// La traduction est donc explicite, et son cas par défaut range dans `run` — le namespace de
/// l'exécution — plutôt que d'inventer un namespace ou d'échouer. Un événement de protocole que
/// §10.3 ne prévoit pas reste **écrit** : l'invariant 12 refuse qu'on perde un fait pour faire
/// propre.
///
/// `attempt.*` n'a **pas** de bras à lui, et ce n'est pas un oubli : une tentative *est* une
/// exécution, donc `run` est sa place et non un repli. Lui écrire un bras identique au défaut aurait
/// dit la même chose deux fois — clippy l'a relevé sur une première rédaction, et il avait raison :
/// la phrase appartient à cette documentation, pas au `match`.
fn lep_event_type(lep: &str) -> String {
    match lep.split_once('.') {
        Some(("task", verb)) => format!("task.{verb}"),
        Some(("worker", verb)) => format!("worker.{verb}"),
        Some(("artifact", verb)) => format!("artifact.{verb}"),
        // `tool.started` et `tool.completed` : §10.3 n'a pas de namespace `tool`, et le verbe seul
        // (`run.started`) serait indistinguable du démarrage de la tentative elle-même.
        Some(("tool", verb)) => format!("run.tool_{verb}"),
        Some(("epistemic_commit", verb)) => format!("epistemic_object.{verb}"),
        Some((_, verb)) => format!("run.{verb}"),
        None => format!("run.{lep}"),
    }
}

/// Un fait du journal, tel que §10.1 le veut.
///
/// `rank` décale l'identité d'événement dans un lot : deux faits écrits par la même commande ne
/// peuvent pas porter le même `event_id`, et le contexte n'en fournit qu'un — la lacune d'entropie
/// nommée en tête de module. Le décalage est déterministe, donc rejouable, ce qu'un identifiant
/// tiré au hasard ne serait pas.
fn fact(
    command: &CommandEnvelope,
    context: &LepContext,
    rank: usize,
    event_type: &str,
    stream_id: &str,
    payload: serde_json::Value,
) -> Result<EventDraft, CommandError> {
    Ok(EventDraft {
        event_id: context.identity(rank)?,
        event_type: EventType::parse(event_type).unwrap_or_else(|_| {
            unreachable!("« {event_type} » sort de `lep_event_type`, dont tous les cas sont des namespaces de §10.3")
        }),
        schema_version: 1,
        stream_id: stream_id.to_owned(),
        workspace_id: *command.workspace_id(),
        project_id: context.project_id,
        program_id: None,
        branch_id: None,
        actor: Actor {
            principal_id: *command.actor_principal_id(),
            // Un worker agit comme agent, jamais comme système : `System` est réservé aux
            // migrations et aux projections, et l'y ranger rendrait indistinguable ce qu'une
            // machine a fait d'elle-même de ce qu'un worker a fait sur ordre.
            kind: ActorKind::Agent,
            delegation_id: command.delegation_id().copied(),
        },
        occurred_at: context.occurred_at,
        causation_id: *command.command_id(),
        correlation_id: command.correlation_id().copied(),
        trace_id: None,
        payload,
        payload_hash: context.payload_hash.clone(),
    })
}

/// Ce que le worker annonce, ou le refus qui dit ce qui manque — `W20.q`.
///
/// # Le manifeste ne dit pas **qui** parle
///
/// La créance le dit, et elle seule. Un manifeste au nom d'un autre worker est donc refusé plutôt
/// qu'ignoré : le laisser passer ferait placer sur les capacités d'une machine et exécuter sur une
/// autre — c'est la même règle que [`Report`] applique déjà à un événement, et que [`Complete`]
/// rend inexprimable en ne portant pas le champ du tout.
///
/// Une fonction libre, et non une méthode : elle ne consulte **rien** du daemon. La rendre méthode
/// laisserait croire qu'elle regarde un état, alors que tout ce qu'elle compare lui est passé.
fn announced<'a>(
    identity: &WorkerIdentity,
    manifest: Option<&'a CapabilityManifest>,
) -> Result<&'a CapabilityManifest, CommandError> {
    let manifest = manifest.ok_or_else(|| CommandError::Validation {
        field: "manifest".to_owned(),
        detail: "§15.3 : un worker annonce ce qu'il sait faire avant qu'on lui confie quoi que ce \
                 soit. Sans manifeste il n'y a rien à placer, et servir la première mission venue \
                 reviendrait à l'exécuter sur un hôte dont personne ne sait rien"
            .to_owned(),
    })?;
    if manifest.worker_id != identity.worker_id {
        return Err(CommandError::Authorization {
            action: format!(
                "réclamer avec le manifeste de « {} » sous la créance de « {} »",
                manifest.worker_id, identity.worker_id
            ),
        });
    }
    Ok(manifest)
}

/// Ce qu'un verdict devient pour un appelant qui ne sait qu'échouer ou continuer.
///
/// Le verdict reste un [`Outcome`] partout ailleurs — `W20.a` tient à ce qu'un refus ne ressemble
/// pas à un succès, et un `Result` le dirait moins bien dans le journal des commandes. Ici, en
/// revanche, l'appelant est une route HTTP : elle traduit un refus en statut, et n'a rien à faire
/// de la révision d'un succès.
fn verdict(outcome: Outcome) -> Result<(), CommandError> {
    match outcome {
        Outcome::Accepted(_) => Ok(()),
        Outcome::Refused(error) => Err(error),
    }
}

/// Ce qu'un worker envoie et que le daemon ne décide pas à sa place.
///
/// La clé d'idempotence vient du worker : c'est lui qui sait qu'il retente. Elle est **scopée** par
/// `(workspace, principal)` — `IdempotencyScope`, `W20.b` — et les deux viennent du registre, donc
/// deux workers qui choisissent `retry-1` ne se répondent pas l'un à l'autre.
#[derive(Debug, Clone)]
pub struct Submitted {
    /// La clé d'idempotence de §15.2, telle que le worker l'a choisie.
    pub idempotency_key: String,
    /// Le projet auquel les faits appartiennent.
    pub project_id: Id<Project>,
    /// Quand l'acte a eu lieu, tel que le worker l'a daté — distinct de l'écriture (§10.1).
    pub occurred_at: Timestamp,
}

impl<S: EventStore> Runtime<S> {
    /// Réclamer du travail pour la créance donnée — `POST /lep/v1/claim`.
    ///
    /// # Quatre issues, et elles ne se confondent pas
    ///
    /// - `Err(Authorization)` : la créance n'identifie personne. Le worker doit s'enrôler.
    /// - `Err(Validation)` : le worker n'a pas annoncé ce qu'il sait faire. Rien n'est retiré.
    /// - `Err(Unavailable)` : on n'a **pas pu demander** au broker. Le worker retentera.
    /// - `Ok(None)` : personne n'a de travail **pour lui**. Le worker attend.
    /// - `Ok(Some(offer))` : une mission est confiée, **et le fait est écrit**.
    ///
    /// Les fondre enverrait chercher un réglage manquant là où il n'y a que du calme, ou l'inverse.
    /// C'est la séparation que `W2.21` tient de l'autre côté du fil ; celle-ci la rend vraie ici.
    ///
    /// # Le placement est demandé, jamais décidé ici — `W20.q`
    ///
    /// `locusd` ne choisit aucun hôte : il transmet au broker le `CapabilityManifest` du worker et
    /// l'exigence de la mission, et il obéit. C'est la règle 4 de `boundaries.json` prise à sa
    /// racine — le binaire qui parle au monde ne décide pas de ce qui s'exécute où — et un test
    /// d'absence le tient sur le source, comme `W20.o` le fait déjà pour la création de mission.
    ///
    /// # Une mission qui ne convient pas **retourne dans la file**
    ///
    /// [`MissionQueue::take`] retire. Si le placement échoue ensuite, la mission serait perdue au
    /// profit de personne : un worker macOS qui sonde une file portant une mission `S3` la ferait
    /// disparaître, et le worker Linux qui pouvait la porter ne la verrait jamais. Elle est donc
    /// **remise**, dans tous les cas où elle n'est pas confiée — refus de placement, broker
    /// injoignable, broker qui refuse.
    ///
    /// Une seule offre est examinée par réclamation, et c'est délibéré : parcourir la file jusqu'à
    /// en trouver une qui convienne serait de l'ordonnancement, donc `W23.c`, et l'écrire ici le
    /// rendrait invisible.
    ///
    /// # Errors
    ///
    /// [`CommandError`] : `Authorization` sur créance inconnue ou manifeste usurpé, `Validation`
    /// sans manifeste, `Unavailable` sans source d'identifiants ou sans broker joignable, ou ce que
    /// la transaction refuse.
    pub fn lep_claim(
        &self,
        credential: &str,
        manifest: Option<&CapabilityManifest>,
        submitted: &Submitted,
        now: Timestamp,
    ) -> Result<Option<Offer>, CommandError> {
        let identity = self.identify(credential)?;
        // Vérifié **avant** de retirer quoi que ce soit : une réclamation qu'on va refuser ne doit
        // pas coûter une mission à la file.
        let manifest = announced(&identity, manifest)?;
        // La file est consultée **après** l'authentification : un daemon qui retirerait une mission
        // avant de savoir qui parle la perdrait au profit de personne.
        let Some(queued) = self.lep().queue().take(&identity.worker_id) else {
            return Ok(None);
        };
        match self.placed(manifest, &queued.mission) {
            Ok(true) => {}
            Ok(false) => {
                self.lep().queue().enqueue(queued);
                return Ok(None);
            }
            Err(error) => {
                self.lep().queue().enqueue(queued);
                return Err(error);
            }
        }
        // Le bail est frappé **ici** — `W20.v` —, après que le placement a admis ce worker-là, et
        // pour lui. Le frapper à la mise en file aurait obligé à nommer le worker avant de savoir
        // lequel convient, ce qui rendait la question posée au broker purement décorative.
        //
        // Si l'identité manque, la mission **retourne dans la file** : elle n'a pas été confiée, et
        // la perdre ici la retirerait à qui pouvait la porter — la même règle que pour un refus de
        // placement.
        let lease = match self.lease_for(&identity.worker_id, &queued, now) {
            Ok(lease) => lease,
            Err(error) => {
                self.lep().queue().enqueue(queued);
                return Err(error);
            }
        };
        let offer = Offer {
            mission: queued.mission,
            lease,
        };
        let stream = stream_of_task(&offer.mission.task_id);
        let claim = Claim {
            offer: offer.clone(),
            worker_id: identity.worker_id.clone(),
        };
        debug_assert_eq!(
            claim.offer.lease.worker_id, claim.worker_id,
            "le bail est frappé depuis le réclamant : l'inégalité serait un défaut de ce module"
        );
        self.write_worker_fact(&identity, submitted, &stream, 1, &claim, now)?;
        Ok(Some(offer))
    }

    /// Frapper le bail de §15.5 qui autorise **ce** worker sur **cette** mission — `W20.v`.
    ///
    /// # Le rang d'attempt vient de la mission, jamais d'un compteur
    ///
    /// §12.3 : « une tâche réattribuée conserve son numéro d'attempt ». Le dériver d'un compteur de
    /// réclamations ferait qu'une reprise après panne recevrait un rang neuf — c'est-à-dire
    /// exactement le doublon que §15.5 existe pour empêcher, et que le jumeau de `W12.d` doit
    /// démontrer impossible.
    ///
    /// # Errors
    ///
    /// [`CommandError::Unavailable`] quand aucune source d'identifiants n'est câblée : un bail sans
    /// identité ne s'oppose à rien, et deux baux qui porteraient la même ne se distingueraient plus.
    fn lease_for(
        &self,
        worker_id: &str,
        queued: &Queued,
        now: Timestamp,
    ) -> Result<Lease, CommandError> {
        let lease_id = self.lep().identities().lease()?;
        Ok(Lease {
            protocol: queued.mission.protocol.clone(),
            lease_id: lease_id.to_string(),
            task_id: queued.mission.task_id.clone(),
            attempt: queued.attempt,
            worker_id: worker_id.to_owned(),
            issued_at: now.to_string(),
            expires_at: expiration(now, LEASE_TTL_SECONDS)?.to_string(),
            ttl_seconds: LEASE_TTL_SECONDS,
            heartbeat_interval_seconds: HEARTBEAT_INTERVAL_SECONDS,
            renewal_count: None,
            // La clé d'idempotence des effets de bord est **indépendante** de l'attempt (§12.3) :
            // rejouer un attempt ne doit pas rejouer ses effets. Ce daemon n'en attribue aucune, et
            // l'absence est exacte — l'inventer ici ferait croire qu'un effet est protégé.
            idempotency_key: None,
        })
    }

    /// Demander au broker si cette offre peut aller sur ce worker — `W20.q`.
    ///
    /// Rend `false` quand le broker a répondu **non**, et une erreur quand il n'a pas répondu. Les
    /// deux sont l'ADR 0028 décision 4 : « rien pour toi » se traduit en `204`, « je n'ai pas pu
    /// demander » en `unavailable`. Les fondre ferait attendre en silence un worker dont le lien est
    /// coupé.
    fn placed(
        &self,
        manifest: &CapabilityManifest,
        mission: &MissionEnvelope,
    ) -> Result<bool, CommandError> {
        let broker = self.lep().broker();
        match broker.place(manifest, &mission.sandbox, &mission.resources) {
            Ok(Placement::Placed { .. }) => Ok(true),
            Ok(Placement::NotPlaced { .. }) => Ok(false),
            // Un broker qui refuse de nous parler n'a pas dit que le worker ne convient pas : il a
            // dit qu'il ne répondra pas. Le ranger en `204` ferait attendre indéfiniment un worker
            // capable, pour un problème d'identité entre deux services.
            Ok(Placement::Refused { why }) => Err(CommandError::Unavailable {
                detail: format!(
                    "le broker {} refuse de placer : {why}. Aucune mission n'est confiée tant que \
                     personne ne peut dire sur quoi elle s'exécuterait",
                    broker.endpoint()
                ),
            }),
            Err(
                error @ (BrokerError::Unreachable { .. }
                | BrokerError::Malformed { .. }
                | BrokerError::TooLong { .. }),
            ) => Err(CommandError::Unavailable {
                detail: format!(
                    "{error} — « je n'ai pas pu demander » n'est pas « rien pour toi » : la mission \
                     reste en file, et le worker retentera"
                ),
            }),
        }
    }

    /// Faire remonter des événements — `POST /lep/v1/events`.
    ///
    /// # Errors
    ///
    /// [`CommandError`], comme ci-dessus.
    pub fn lep_events(
        &self,
        credential: &str,
        events: Vec<Event>,
        submitted: &Submitted,
        now: Timestamp,
    ) -> Result<(), CommandError> {
        let identity = self.identify(credential)?;
        let count = events.len();
        let stream = stream_of_task(first_task(&events));
        let report = Report {
            events,
            worker_id: identity.worker_id.clone(),
        };
        self.write_worker_fact(&identity, submitted, &stream, count, &report, now)
    }

    /// Rendre un résultat — `POST /lep/v1/result`.
    ///
    /// # Errors
    ///
    /// [`CommandError`], comme ci-dessus.
    pub fn lep_result(
        &self,
        credential: &str,
        rendered: Rendered,
        submitted: &Submitted,
        now: Timestamp,
    ) -> Result<(), CommandError> {
        let identity = self.identify(credential)?;
        let stream = stream_of_task(&rendered.task_id);
        let result = Complete {
            rendered,
            worker_id: identity.worker_id.clone(),
        };
        self.write_worker_fact(&identity, submitted, &stream, 1, &result, now)
    }

    /// Le chemin d'écriture commun aux trois : assembler, puis **passer par la transaction**.
    ///
    /// # Pourquoi la révision se lit ici et non dans la requête
    ///
    /// §22.5 exige `expected_revision` sur toute commande mutante. Un worker ne connaît pas la
    /// révision du stream d'une tâche, et la lui faire annoncer l'obligerait à lire l'état du
    /// daemon — ou, plus vraisemblablement, à envoyer n'importe quoi.
    ///
    /// Le daemon la lit donc lui-même, **hors** du verrou d'écriture, et c'est sûr pour la raison
    /// même pour laquelle `Expected` existe : une lecture périmée produit un `Conflict` au moment
    /// de l'écriture, jamais un écrasement. Ce n'est pas le « lire puis agir » que l'ADR 0029 a
    /// rendu inexprimable dans le journal — là, rien ne protégeait ; ici, le journal refuse.
    fn write_worker_fact<D: Decide<State = LepContext>>(
        &self,
        identity: &WorkerIdentity,
        submitted: &Submitted,
        stream: &str,
        facts: usize,
        decider: &D,
        now: Timestamp,
    ) -> Result<(), CommandError> {
        let identities = self.lep().identities();
        let context = LepContext {
            project_id: submitted.project_id,
            event_ids: identities.events(facts)?,
            occurred_at: submitted.occurred_at,
            payload_hash: String::new(),
        };
        let command = CommandEnvelope::mutating(
            identities.command()?,
            "worker.report",
            identity.workspace_id,
            identity.principal_id,
            submitted.idempotency_key.clone(),
            Revision::new(self.revision_of(stream)),
        )?;
        // `commit` et non `transaction().submit` : depuis `W20.l`, c'est le chemin d'écriture qui
        // fait avancer les projections. Passer par la transaction directement écrirait un fait que
        // les quatre projections de §9.5 ne verraient jamais.
        verdict(self.commit(decider, &command, &context, now))
    }

    /// La révision courante d'un stream, ou `0` — « il n'existe pas encore ».
    fn revision_of(&self, stream: &str) -> u64 {
        self.revision_of_stream(stream)
    }

    /// La même, ouverte au crate : `enrollment.rs` en a besoin pour la même raison.
    pub(crate) fn revision_of_stream(&self, stream: &str) -> u64 {
        self.transaction_store().revision(stream).unwrap_or(0)
    }

    /// Les tokens d'enrôlement, en lecture — `W20.n`.
    pub(crate) fn enrollment(&self) -> &dyn crate::enrollment::EnrollmentTokens {
        self.lep().tokens()
    }

    /// Qui parle, ou un refus **typé** — jamais une trace.
    ///
    /// Une créance inconnue est une faute d'autorisation, pas un défaut interne : lui rendre un
    /// `500` la ferait retenter à l'identique, un `500` voulant dire « réessaie ».
    fn identify(&self, credential: &str) -> Result<WorkerIdentity, CommandError> {
        self.lep()
            .registry()
            .identify(credential)
            .ok_or_else(|| CommandError::Authorization {
                action: "parler à la surface §15.2 sans créance reconnue".to_owned(),
            })
    }
}
