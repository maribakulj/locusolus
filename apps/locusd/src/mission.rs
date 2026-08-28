//! Une mission naît d'une question — `W20.o`.
//!
//! # Ce qui manquait, et pourquoi c'était la dernière condition de `W12.d`
//!
//! `W20.k` a livré `MissionQueue` comme port avec son implémentation de référence. Ce qui manquait
//! est ce qui la **remplit** : aucune commande de §22.3 ne créait de tâche, donc rien ne produisait
//! de `MissionEnvelope`, donc « une question produit une mission » n'avait pas de sujet et la file
//! n'était garnie que par un test.
//!
//! # Ce que « claimable » veut dire, et d'où ça se lit
//!
//! **De la machine à états de §7.1, et de nulle part ailleurs.** Une tâche est réclamable
//! exactement quand `Leased` est un état où elle peut aller — c'est la définition, pas une
//! coïncidence. L'écrire comme une liste (`Queued` et rien d'autre) créerait une seconde
//! énumération qui divergerait de `TaskState::allowed` au premier ajout, et c'est la dérive que
//! `served()` a connue quatre fois dans ce chantier.
//!
//! Conséquence gratuite : une tâche `proposed` n'est pas réclamable — elle doit d'abord être mise en
//! file —, et une tâche terminale ne l'est plus. Aucune de ces deux règles n'est écrite ici ; les
//! deux se lisent du tableau.
//!
//! # Le placement reste chez `locus-execd`
//!
//! §4 et `W4.g` : `place` décide de l'hôte, et il vit dans l'autre binaire. Créer une mission ne
//! choisit **aucun** hôte, et un test d'absence le vérifie sur le source — la même forme que la
//! règle 4 de `boundaries.json` pour les sockets de runtime.

use locus_domain::CognitionClass;
use locus_domain::task::TaskState;
use locus_event_store::{Actor, ActorKind, Draft as EventDraft, EventStore, EventType};
use locus_lep::{
    MissionEnvelope, MissionEnvelopeBudget, MissionEnvelopeContextView, MissionEnvelopeEnvironment,
    MissionEnvelopeObjective, NetworkMode, ResourceSpec, SandboxLevel, SandboxSpec,
};
use locus_protocol::Timestamp;

use crate::command::CommandEnvelope;
use crate::composition::Runtime;
use crate::error::CommandError;
use crate::handler::Decide;
use crate::lep::{LepContext, Queued, Submitted, stream_of_task};

/// La version de protocole que ce daemon parle — `lep/1.0`, gelée depuis `W0.5`.
///
/// Écrite ici et non demandée à l'appelant : un client n'a pas à choisir la version que le serveur
/// émet, et le laisser faire produirait des missions annonçant une version que ce daemon ne sert
/// pas.
pub const PROTOCOL: &str = "lep/1.0";

/// Sous quelle autorité une commande d'administration est écrite.
///
/// Séparée de [`Submitted`] parce que les deux ne viennent pas du même endroit : pour un worker,
/// workspace et principal sont **lus de la créance** (`W20.k`), et le corps de la requête n'y peut
/// rien. Pour une commande d'administration, il n'y a pas de créance de worker — c'est l'appelant
/// qui les porte. Les fondre dans un seul type ferait croire qu'un worker peut les choisir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Authority {
    /// Le workspace visé.
    pub workspace_id: locus_protocol::Id<locus_protocol::id::Workspace>,
    /// Le principal qui agit.
    pub principal_id: locus_protocol::Id<locus_protocol::id::Agent>,
}

/// Qui a le droit de commander — §22.3, `W20.s`.
///
/// # Un **second** registre, et c'est toute la garantie
///
/// `WorkerRegistry` résout une créance vers un [`crate::lep::WorkerIdentity`] ; celui-ci résout une
/// créance vers une [`Authority`]. Les deux ne se croisent jamais, et c'est ce qui rend inexprimable
/// qu'un worker se crée du travail : sa créance n'est pas dans ce registre-ci, donc elle n'y résout
/// rien, donc il est refusé — sans qu'aucun `if` n'ait à s'en souvenir.
///
/// Un registre unique porteur d'un rôle aurait tenu la même règle par une **comparaison**, et une
/// comparaison se déplace, s'inverse, ou se perd dans un refactor. `W20.o` avait déjà séparé
/// [`Authority`] de [`crate::lep::Submitted`] pour cette raison ; ceci en est la moitié manquante.
///
/// # Le défaut n'admet personne
///
/// Comme [`crate::lep::NoIdentities`] et comme un `MemoryRegistry` vide : un daemon dont personne
/// n'a câblé l'administration refuse toutes les commandes de §22.3. C'est exact, et c'est ce qu'un
/// daemon sans exploitant doit faire — l'inverse ferait de l'absence de configuration une
/// autorisation.
pub trait Administrators: Send + Sync {
    /// L'autorité que porte cette créance, ou `None` si elle n'en porte aucune.
    fn authority(&self, credential: &str) -> Option<Authority>;
}

/// Le registre par défaut : personne n'administre.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoAdministrators;

impl Administrators for NoAdministrators {
    fn authority(&self, _credential: &str) -> Option<Authority> {
        None
    }
}

/// Le registre de référence — en mémoire, rempli par qui le détient.
///
/// Ce qui le remplira en production est une commande d'administration de §22.3, nommée et non
/// simulée — la même forme que `EnrollmentTokens` a prise en `W20.n`.
#[derive(Debug, Default)]
pub struct MemoryAdministrators {
    known: std::sync::RwLock<Vec<(String, Authority)>>,
}

impl MemoryAdministrators {
    /// Un registre vide — donc un daemon qui refuse toutes les commandes de §22.3.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reconnaître une créance comme portant cette autorité.
    pub fn admit(&self, credential: &str, authority: Authority) {
        self.known
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((credential.to_owned(), authority));
    }
}

impl Administrators for MemoryAdministrators {
    fn authority(&self, credential: &str) -> Option<Authority> {
        self.known
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .find(|(known, _)| known == credential)
            .map(|(_, authority)| *authority)
    }
}

/// Vrai quand une tâche dans cet état peut être confiée à un worker.
///
/// **Lu du tableau de §7.1**, jamais recopié : réclamable veut dire « `Leased` est atteignable
/// depuis ici », et c'est exactement ce que `TaskState::allowed` répond. Un jour où §7.1 gagnera une
/// autre entrée vers `Leased`, cette fonction la connaîtra sans être modifiée ; une liste écrite à
/// la main l'ignorerait.
#[must_use]
pub fn claimable(state: TaskState) -> bool {
    state.allowed().contains(&TaskState::Leased)
}

/// La question à laquelle une mission répond, et les bornes sous lesquelles y répondre.
///
/// # Pourquoi rien n'a de défaut
///
/// `MissionEnvelope` documente que « rien ici n'est optionnel par commodité : objectif, contexte,
/// sandbox, ressources, budget et contrat de sortie sont ce qui rend une mission admissible ou
/// refusable ». Une proposition qui laisserait le serveur inventer un budget produirait une mission
/// que personne n'a bornée — l'invariant 6 pris à l'envers.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Proposal {
    /// La **classe de cognition** que la mission déclare — `W25.a`, ADR 0026 décision 6.
    ///
    /// Une classe, **jamais un modèle** : quel modèle la sert est une valeur de politique
    /// versionnée, `locus_policy::Assignment`, et le domaine n'a aucun moyen de l'apprendre. C'est
    /// ce qui rend le changement d'affectation gratuit — il ne traverse aucun type.
    pub cognition: CognitionClass,
    /// La question, en clair — l'`objective.statement` de §15.4.
    pub statement: String,
    /// À quoi on reconnaîtra qu'elle est traitée.
    pub success_conditions: Vec<String>,
    /// La tâche que cette proposition ouvre.
    pub task_id: String,
    /// La première tentative, sous l'identité que §11.1 lui donne.
    pub attempt_id: String,
    /// Son **rang** — §12.3, et distinct de son identité (§11.1).
    ///
    /// Fixé par la proposition et non compté par le daemon : « une tâche réattribuée conserve son
    /// numéro d'attempt », donc un compteur de réclamations donnerait un rang neuf à une reprise
    /// après panne — c'est-à-dire le doublon que §15.5 existe pour empêcher.
    pub attempt: i64,
    /// La branche sur laquelle elle s'inscrit.
    pub branch_id: String,
    /// La vue de contexte, par référence — la mission ne porte jamais son contenu.
    pub context_view_id: String,
    /// Le hash de cette vue. Sans lui, « ce que l'agent pouvait connaître » n'est plus vérifiable.
    pub context_view_hash: String,
    /// L'environnement d'exécution déclaré.
    pub environment_id: String,
    /// Le plancher de confinement exigé.
    pub sandbox_level: SandboxLevel,
    /// Le mode réseau imposé.
    pub network: NetworkMode,
    /// Les ressources réservées — invariant 6.
    pub resources: ResourceSpec,
    /// Les trois bornes de modèle, toutes obligatoires.
    pub budget: MissionEnvelopeBudget,
    /// Ce que l'attempt doit rendre.
    pub output_contract: String,
}

impl Proposal {
    /// La mission que cette proposition décrit.
    ///
    /// # Rien n'est inventé ici
    ///
    /// Chaque champ vient de la proposition, sauf `protocol` — qui est la version que ce daemon
    /// parle, et qu'un appelant n'a pas à choisir. Les champs optionnels de §15.4 restent
    /// **absents** : `role`, `review_policy`, `offline_allowed` ne se remplissent pas d'un défaut,
    /// et un document `1.0` qui n'en parle pas ne les demande pas.
    #[must_use]
    pub fn envelope(&self) -> MissionEnvelope {
        MissionEnvelope {
            protocol: PROTOCOL.to_owned(),
            task_id: self.task_id.clone(),
            attempt_id: self.attempt_id.clone(),
            branch_id: self.branch_id.clone(),
            objective: MissionEnvelopeObjective {
                statement: self.statement.clone(),
                success_conditions: self.success_conditions.clone(),
                failure_conditions: None,
            },
            context_view: MissionEnvelopeContextView {
                id: self.context_view_id.clone(),
                hash: self.context_view_hash.clone(),
            },
            environment: MissionEnvelopeEnvironment {
                environment_id: self.environment_id.clone(),
                image_digest: None,
                toolchains: None,
            },
            sandbox: SandboxSpec {
                minimum_level: self.sandbox_level,
                network: self.network,
                network_allowlist: None,
                profile: None,
                attestation_required: None,
            },
            resources: self.resources.clone(),
            budget: self.budget.clone(),
            required_capabilities: None,
            confidentiality_ceiling: None,
            review_policy: None,
            role: None,
            offline_allowed: None,
            offline_budget_ms: None,
            output_contract: self.output_contract.clone(),
            deadline: None,
        }
    }
}

/// La création d'une tâche — §22.3, et le premier fait de son histoire.
pub struct Propose {
    /// Ce qui est proposé.
    pub proposal: Proposal,
}

impl Decide for Propose {
    type State = LepContext;

    fn decide(
        &self,
        command: &CommandEnvelope,
        context: &Self::State,
    ) -> Result<Vec<EventDraft>, CommandError> {
        if self.proposal.statement.trim().is_empty() {
            return Err(CommandError::Validation {
                field: "objective.statement".to_owned(),
                detail: "une mission sans question ne peut pas être jugée : elle serait acceptée \
                         par défaut, ce qui est le contraire de l'admission"
                    .to_owned(),
            });
        }
        if self.proposal.success_conditions.is_empty() {
            return Err(CommandError::Validation {
                field: "objective.success_conditions".to_owned(),
                detail: "sans condition de succès, rien ne dit quand la mission est traitée"
                    .to_owned(),
            });
        }
        Ok(vec![fact(
            command,
            context,
            "task.proposed",
            &stream_of_task(&self.proposal.task_id),
            serde_json::json!({
                "task_id": self.proposal.task_id,
                "state": TaskState::Proposed.to_string(),
                "statement": self.proposal.statement,
                "branch_id": self.proposal.branch_id,
                // `W25.a` : la **classe**, sous son slug. Jamais le modèle — le journal ne doit pas
                // porter une affectation qui aura changé quand on le relira.
                "cognition": self.proposal.cognition.to_string(),
                // La proposition **entière** — `W20.s`. Sans elle, la mise en file devrait faire
                // renvoyer la proposition par son appelant, et rien n'empêcherait de proposer une
                // question et d'en mettre une autre en file sous le même identifiant de tâche : le
                // fait écrit ne porterait pas de quoi constater la divergence. L'invariant 2 dit que
                // le journal est la vérité institutionnelle ; il faut donc qu'il porte de quoi
                // reconstruire ce qu'on y a proposé.
                "proposal": self.proposal,
            }),
        )?])
    }
}

/// La mise en file — `proposed → queued`, la seule sortie que §7.1 lui donne.
pub struct Queue {
    /// La tâche mise en file.
    pub task_id: String,
    /// L'état d'où elle part, tel que l'appelant l'a lu.
    pub from: TaskState,
}

impl Decide for Queue {
    type State = LepContext;

    fn decide(
        &self,
        command: &CommandEnvelope,
        context: &Self::State,
    ) -> Result<Vec<EventDraft>, CommandError> {
        // La transition est demandée à §7.1, jamais décidée ici. Un `if from == Proposed` écrirait
        // une seconde fois ce que le tableau dit déjà, et les deux divergeraient.
        if !self.from.allowed().contains(&TaskState::Queued) {
            return Err(CommandError::Policy {
                policy: "task.transition".to_owned(),
                detail: format!(
                    "§7.1 ne permet pas « {} → queued » : une tâche ne se met pas en file depuis \
                     n'importe où",
                    self.from
                ),
            });
        }
        Ok(vec![fact(
            command,
            context,
            "task.queued",
            &stream_of_task(&self.task_id),
            serde_json::json!({
                "task_id": self.task_id,
                "state": TaskState::Queued.to_string(),
            }),
        )?])
    }
}

pub(crate) fn fact(
    command: &CommandEnvelope,
    context: &LepContext,
    event_type: &str,
    stream_id: &str,
    payload: serde_json::Value,
) -> Result<EventDraft, CommandError> {
    Ok(EventDraft {
        event_id: context.identity(0)?,
        event_type: EventType::parse(event_type).unwrap_or_else(|_| {
            unreachable!(
                "« {event_type} » est un littéral de ce module, et `task` est un namespace de §10.3"
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
    /// Proposer une tâche — §22.3. Le fait entre au journal, et **rien n'est mis en file**.
    ///
    /// Une tâche `proposed` n'est pas réclamable : §7.1 exige qu'elle passe par `queued`, et
    /// [`claimable`] le lit du tableau plutôt que de le redire. Enfiler ici confierait à un worker
    /// une mission que personne n'a mise en file.
    ///
    /// # Errors
    ///
    /// [`CommandError`] — ce que le décideur ou la transaction refusent.
    pub fn lep_propose(
        &self,
        proposal: &Proposal,
        authority: Authority,
        submitted: &Submitted,
        now: Timestamp,
    ) -> Result<(), CommandError> {
        self.bind_context_view(proposal)?;
        let propose = Propose {
            proposal: proposal.clone(),
        };
        let stream = stream_of_task(&proposal.task_id);
        self.write_mission_fact(authority, submitted, &stream, &propose, now)
    }

    /// Mettre une tâche en file, et **y déposer sa mission**.
    ///
    /// L'ordre compte : le fait est écrit d'abord, la file garnie ensuite. Une mission déposée avant
    /// que le fait soit écrit pourrait être réclamée par un worker plus rapide que l'écriture, et
    /// l'institution lirait un `task.leased` sur une tâche qu'aucun `task.queued` n'a précédée.
    ///
    /// # Aucun bail ici — `W20.v`
    ///
    /// Cette fonction en a exigé un pendant deux items, et c'était faux : un bail autorise **un
    /// worker**, et aucun n'est choisi à la mise en file. Le lui faire nommer d'avance rendait la
    /// question de placement de `W20.q` purement décorative — le broker ne pouvait que confirmer le
    /// worker que le bail avait déjà désigné. Le bail est frappé à la réclamation, par
    /// [`Runtime::lep_claim`].
    ///
    /// # La proposition et l'état viennent du **journal**, jamais de l'appelant — `W20.s`
    ///
    /// Cette fonction a pris une `&Proposal` et un `from: TaskState` de son appelant, et les deux
    /// étaient des trous. Faire renvoyer la proposition permettait de proposer une question et d'en
    /// mettre une autre en file sous le même identifiant de tâche — le fait écrit ne porte que
    /// l'identifiant, donc rien n'aurait montré la divergence. Faire annoncer l'état laissait un
    /// appelant déclarer celui qui l'arrange, et la garde de §7.1 aurait validé le mensonge.
    ///
    /// Les deux se lisent maintenant du stream de la tâche. C'est l'invariant 2 appliqué : « le
    /// journal est la vérité institutionnelle », y compris pour décider de la commande suivante.
    ///
    /// # Errors
    ///
    /// [`CommandError::NotFound`] si aucune tâche n'a été proposée sous cet identifiant,
    /// [`CommandError::Internal`] si le fait de proposition ne porte pas de proposition relisible,
    /// `Policy` si §7.1 ne permet pas la transition depuis l'état courant.
    pub fn lep_queue(
        &self,
        task_id: &str,
        authority: Authority,
        submitted: &Submitted,
        now: Timestamp,
    ) -> Result<(), CommandError> {
        let stream = stream_of_task(task_id);
        let (proposal, from) = self.proposed(&stream, task_id)?;
        let queue = Queue {
            task_id: task_id.to_owned(),
            from,
        };
        self.write_mission_fact(authority, submitted, &stream, &queue, now)?;
        self.lep().queue().enqueue(Queued {
            mission: proposal.envelope(),
            attempt: proposal.attempt,
        });
        Ok(())
    }

    /// La vue que cette proposition nomme existe-t-elle, et est-ce bien celle-là ? — `W20.ac`
    ///
    /// # Ce que ce contrôle empêche, et pourquoi il est ici et pas plus tard
    ///
    /// `MissionEnvelope` porte `context_view : {id, hash}`, et rien n'attachait ces deux valeurs à
    /// un document. Une proposition pouvait donc nommer une vue qui n'existe pas, ou annoncer d'une
    /// vue réelle une empreinte qui n'est pas la sienne. Dans les deux cas, le premier à s'en
    /// apercevoir aurait été le **worker**, après réclamation, au moment où §12.3 lui demande de
    /// vérifier l'empreinte avant de démarrer : une mission confiée, un bail frappé, un attempt
    /// ouvert, pour un contexte introuvable.
    ///
    /// Le contrôle est donc à la proposition, où il coûte un refus et rien d'autre.
    ///
    /// # Errors
    ///
    /// [`CommandError::Validation`] nommant `context_view.id` quand aucune vue n'a été bâtie sous
    /// cet identifiant, `context_view.hash` quand l'empreinte annoncée n'est pas celle du document
    /// conservé.
    fn bind_context_view(&self, proposal: &Proposal) -> Result<(), CommandError> {
        let vue = self
            .context_view(&proposal.context_view_id)?
            .ok_or_else(|| CommandError::Validation {
                field: "context_view.id".to_owned(),
                detail: format!(
                    "aucune vue « {} » n'a été bâtie : une mission ne nomme pas un contexte que \
                     personne n'a déposé (§16.2)",
                    proposal.context_view_id
                ),
            })?;
        if vue.content_hash == proposal.context_view_hash {
            return Ok(());
        }
        Err(CommandError::Validation {
            field: "context_view.hash".to_owned(),
            detail: format!(
                "la vue « {} » porte {}, la proposition annonce {} : sans cet accord, « ce que \
                 l'agent pouvait connaître » n'est plus vérifiable (§16.2)",
                proposal.context_view_id, vue.content_hash, proposal.context_view_hash
            ),
        })
    }

    /// Ce que le journal dit de cette tâche : ce qui a été proposé, et où elle en est.
    ///
    /// # L'état courant est celui du **dernier** fait qui en porte un
    ///
    /// Chaque fait de tâche écrit son `state` — `Propose`, `Queue` et `Claim` le font tous les
    /// trois. Le dernier gagne, et c'est exact par construction : le journal est ordonné par
    /// révision, et une transition n'est écrite que si §7.1 l'a permise.
    ///
    /// # Errors
    ///
    /// [`CommandError::NotFound`] quand rien n'a été proposé, [`CommandError::Internal`] quand le
    /// fait de proposition ne porte pas de proposition relisible — ce qui ne peut venir que d'un
    /// journal écrit par une version antérieure, et se répare par migration, pas par le client.
    fn proposed(&self, stream: &str, task_id: &str) -> Result<(Proposal, TaskState), CommandError> {
        let faits = self.transaction_store().read_stream(stream, 0);
        let brut = faits
            .iter()
            .find_map(|fait| fait.payload.get("proposal"))
            .ok_or_else(|| CommandError::Validation {
                field: "task_id".to_owned(),
                detail: format!(
                    "aucune tâche « {task_id} » n'a été proposée : §7.1 veut qu'une tâche passe par \
                     `proposed` avant d'entrer en file, et le journal n'en garde pas trace"
                ),
            })?;
        let proposal = serde_json::from_value::<Proposal>(brut.clone()).map_err(|erreur| {
            CommandError::Internal {
                detail: format!(
                    "le fait de proposition de « {task_id} » ne se relit pas comme une proposition : \
                     {erreur}. Un journal écrit avant `W20.s` n'en porte pas, et cela se répare par \
                     migration — pas en corrigeant la requête"
                ),
            }
        })?;
        let etat = faits
            .iter()
            .rev()
            .find_map(|fait| {
                fait.payload
                    .get("state")
                    .and_then(serde_json::Value::as_str)
            })
            .and_then(|nom| {
                TaskState::ALL
                    .into_iter()
                    .find(|etat| etat.to_string() == nom)
            })
            .ok_or_else(|| CommandError::Internal {
                detail: format!(
                    "aucun fait de « {task_id} » ne porte d'état lisible : le stream existe, donc \
                     quelque chose y a été écrit sans dire où la tâche en était"
                ),
            })?;
        Ok((proposal, etat))
    }

    fn write_mission_fact<D: Decide<State = LepContext>>(
        &self,
        authority: Authority,
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
            "task.propose",
            authority.workspace_id,
            authority.principal_id,
            submitted.idempotency_key.clone(),
            crate::error::Revision::new(self.revision_of_stream(stream)),
        )?;
        match self.commit(decider, &command, &context, now) {
            crate::outcome::Outcome::Accepted(_) => Ok(()),
            crate::outcome::Outcome::Refused(error) => Err(error),
        }
    }
}
