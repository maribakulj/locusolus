//! La `ContextView` bâtie, conservée et servie — §16.2, §12.3, `W20.ac`.
//!
//! # Ce qui manquait
//!
//! `MissionEnvelope` porte `context_view : {id, hash}` — un identifiant et une empreinte, **jamais
//! le document**. C'est le dessin voulu : la mission dit *laquelle*, le worker la récupère et
//! vérifie qu'on ne la lui a pas échangée. Or aucune route de ce daemon n'en rendait une, et rien ne
//! rattachait l'empreinte annoncée par une proposition à un document existant. Une mission pouvait
//! donc nommer n'importe quelle vue sous n'importe quelle empreinte, et personne ne l'aurait su.
//!
//! # Les deux `ContextView` ne sont pas des doublons
//!
//! Celle de `packages/review` porte le **contenu** filtré — ce qui entre, ce qui est écarté et
//! pourquoi. Celle de `schemas/lep/1.0/context-view.schema.json` porte la **description** de §16.2
//! et l'intégrité. Elles se recoupent exactement sur ce qui décide de la vérification :
//! `redactions`, `confidentiality_ceiling`, `source_event_watermark`, `content_hash`. Ce module est
//! la jonction entre les deux, et rien d'autre : il ne réimplémente aucun filtre.
//!
//! # L'ordre des trois moments, et pourquoi il ne peut pas être un autre
//!
//! 1. **Filtrer** — `packages/review` dit ce qui entre et ce qu'il écarte. Les cas adverses de
//!    `contamination.rs` existaient avant la vue, donc ils n'ont pas pu être écrits pour qu'elle
//!    passe.
//! 2. **Écrire le document**, avec les rédactions que le filtre vient de nommer. Il ne peut pas
//!    l'être avant : il les porte.
//! 3. **Sceller** — l'empreinte porte sur ce document privé du champ qui la porte, exactement comme
//!    `viewContentHash` la recalcule côté worker. Elle ne peut pas être calculée avant que le
//!    document existe.
//!
//! # Ce que ce module **ne sait pas** faire, et ce qui le lèverait
//!
//! Un candidat ne peut pas porter de **dévoilement** (`Disclosure`, ADR 0027) : la commande le
//! transporte en JSON, et un dévoilement se construit par `Disclosure::granting`, qui écrit un fait.
//! Conséquence, nommée plutôt que tue : un élément dont l'exposition serait légitimée par un
//! dévoilement est **écarté** au lieu d'être inclus. C'est la direction sûre — le filtre retire, il
//! n'ajoute jamais —, et le registre de dévoilements de `W26.d` atteignant cette commande est ce qui
//! la lèverait.

use locus_domain::{Confidentiality, ContentHash, RevisionId, canonical_hash};
use locus_event_store::{Draft as EventDraft, EventStore};
use locus_lep::{ContextView, ContextViewRedactionsItem, ContextViewTimeRange, DataClass};
use locus_protocol::{Id, Timestamp, id::Agent};
use locus_review::{ContextItem, ContextView as Filter, Recipient};

use crate::command::CommandEnvelope;
use crate::composition::Runtime;
use crate::error::CommandError;
use crate::handler::Decide;
use crate::lep::{LepContext, Submitted};
use crate::mission::{Authority, fact};

/// Le stream d'une vue de contexte — un par vue, parce qu'une vue est immuable.
#[must_use]
pub fn stream_of_view(view_id: &str) -> String {
    format!("context_view/{view_id}")
}

/// Ce que le destinataire d'un contexte est autorisé à voir, dans la forme que la commande porte.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct For {
    /// L'agent destinataire.
    pub agent_id: String,
    /// Le worker où il tourne.
    pub worker_id: String,
    /// Vrai quand la politique le rend aveugle au raisonnement du générateur — invariant 11.
    #[serde(default)]
    pub blind_to_generator: bool,
    /// Le plafond de confidentialité que son worker est habilité à recevoir.
    pub clearance: DataClass,
}

/// Un élément candidat, dans la forme que la commande porte.
///
/// Les champs sont ceux de `locus_review::ContextItem`, **sauf** le dévoilement — voir l'en-tête du
/// module. Ils portent tous un défaut sûr : un candidat qui ne dit rien de lui-même est un candidat
/// ordinaire, et c'est le filtre qui décide, pas l'absence de déclaration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Candidate {
    /// La révision citée.
    pub revision_id: String,
    /// Sa position dans le journal — c'est elle que le watermark borne.
    pub position: u64,
    /// La classification de la donnée.
    pub classification: DataClass,
    /// Vrai quand cet élément est le raisonnement privé du générateur.
    #[serde(default)]
    pub is_generator_reasoning: bool,
    /// Vrai quand la revendication portée a été réfutée.
    #[serde(default)]
    pub is_refuted: bool,
    /// Ce que cet élément cite — pour détecter un consensus circulaire.
    #[serde(default)]
    pub cites: Vec<String>,
    /// Vrai quand la source est extérieure au laboratoire.
    #[serde(default)]
    pub is_external_source: bool,
    /// L'agent qui l'a produit, s'il y en a un.
    #[serde(default)]
    pub produced_by: Option<String>,
}

/// La description de §16.2, plus de quoi la bâtir : le destinataire et les candidats.
///
/// # Pourquoi la description entre entière et n'est pas devinée
///
/// §16.2 en fait le contenu d'une vue. Un champ que le daemon inventerait — une profondeur par
/// défaut, une politique de résultats négatifs supposée — dirait à qui relit la vue que quelqu'un
/// l'a voulu ainsi, alors que personne ne l'aurait choisi. `negative_result_policy` est le cas le
/// plus net : l'invariant 12 interdit d'effacer un résultat négatif, et une valeur par défaut posée
/// ici passerait pour une décision.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Requested {
    /// L'identifiant sous lequel la vue sera conservée et servie.
    pub id: String,
    /// La question à laquelle la vue répond.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// Les racines de la traversée.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_ids: Option<Vec<String>>,
    /// Les types d'objets retenus.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub included_types: Option<Vec<String>>,
    /// Les relations suivies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub included_relations: Option<Vec<String>>,
    /// La profondeur de traversée.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_depth: Option<i64>,
    /// La fenêtre temporelle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_range: Option<ContextViewTimeRange>,
    /// Les branches que la vue peut atteindre — invariant 11.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_scope: Option<Vec<String>>,
    /// Les niveaux de validation retenus.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_levels: Option<Vec<String>>,
    /// La politique d'artefacts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_policy: Option<String>,
    /// La politique de résultats négatifs — invariant 12.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negative_result_policy: Option<String>,
    /// La politique de diversité des sources.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diversity_policy: Option<String>,
    /// Le budget de contexte.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_budget: Option<i64>,
    /// La position du journal jusqu'à laquelle la vue est arrêtée.
    pub source_event_watermark: u64,
    /// Pour qui la vue est bâtie.
    pub recipient: For,
    /// Les éléments soumis au filtre.
    pub candidates: Vec<Candidate>,
}

/// Bâtir la vue : filtrer, écrire le document, sceller.
///
/// # Errors
///
/// [`CommandError::Validation`] pour un identifiant qui n'en est pas un, une position au-delà du
/// watermark, ou un document que le type du fil ne relit pas ; [`CommandError::Internal`] si la
/// canonicalisation refuse — voir [`locus_domain::CanonicalError`].
pub fn build(requested: &Requested, generated_at: Timestamp) -> Result<ContextView, CommandError> {
    let destinataire = recipient(&requested.recipient)?;
    let candidats = candidates(&requested.candidates)?;
    let filtre = Filter::filter(
        &candidats,
        &destinataire,
        requested.source_event_watermark,
        generated_at,
    )
    .map_err(|erreur| CommandError::Validation {
        field: "candidates".to_owned(),
        detail: erreur.to_string(),
    })?;

    // Le plafond du **document** est celui que le filtre a appliqué, pas celui que la demande a
    // annoncé. Les deux coïncident aujourd'hui — le filtre le lit du destinataire — et les écrire
    // séparément laisserait un jour la description dire autre chose que ce qui a été fait.
    let vue = ContextView {
        id: requested.id.clone(),
        query: requested.query.clone(),
        root_ids: requested.root_ids.clone(),
        included_types: requested.included_types.clone(),
        included_relations: requested.included_relations.clone(),
        max_depth: requested.max_depth,
        time_range: requested.time_range.clone(),
        branch_scope: requested.branch_scope.clone(),
        validation_levels: requested.validation_levels.clone(),
        confidentiality_ceiling: data_class(filtre.confidentiality_ceiling()),
        artifact_policy: requested.artifact_policy.clone(),
        negative_result_policy: requested.negative_result_policy.clone(),
        diversity_policy: requested.diversity_policy.clone(),
        token_budget: requested.token_budget,
        redactions: Some(
            filtre
                .redactions()
                .iter()
                .map(|redaction| ContextViewRedactionsItem {
                    target: redaction.revision.to_string(),
                    reason: redaction.reason.clone(),
                })
                .collect(),
        ),
        source_event_watermark: i64::try_from(filtre.source_event_watermark()).map_err(|_| {
            CommandError::Validation {
                field: "source_event_watermark".to_owned(),
                detail: "au-delà de ce qu'un entier du fil représente".to_owned(),
            }
        })?,
        // L'empreinte du vide, et elle **ne survit pas** à `seal` : le champ est retiré du document
        // avant la canonicalisation, donc sa valeur d'entrée n'entre pas dans le résultat. Un test
        // le vérifie plutôt que ce commentaire ne le promette.
        content_hash: ContentHash::of(&[]).to_string(),
        generated_at: generated_at.to_string(),
    };
    seal(vue)
}

/// Sceller une vue : `content_hash` prend la valeur que le reste du document impose.
///
/// # Ce que « le reste » veut dire, et pourquoi ça ne peut pas être autre chose
///
/// Le champ est **retiré** avant le calcul. L'y laisser rendrait l'empreinte invérifiable — il
/// faudrait connaître le résultat pour le recalculer. C'est la définition qu'applique
/// `viewContentHash` dans `context-materializer.ts`, et l'accord entre les deux est ce qui fait que
/// le worker reconnaît une vue au lieu de la refuser.
///
/// # Errors
///
/// [`CommandError::Internal`] si le document ne se canonicalise pas.
pub fn seal(view: ContextView) -> Result<ContextView, CommandError> {
    let mut document = serde_json::to_value(&view).map_err(|erreur| CommandError::Internal {
        detail: format!("une vue ne se sérialise pas : {erreur}"),
    })?;
    document
        .as_object_mut()
        .ok_or_else(|| CommandError::Internal {
            detail: "une vue sérialisée n'est pas un objet".to_owned(),
        })?
        .remove("content_hash");
    let empreinte = canonical_hash(&document).map_err(|erreur| CommandError::Internal {
        detail: format!("cette vue n'a pas de forme canonique : {erreur}"),
    })?;
    Ok(ContextView {
        content_hash: empreinte.to_string(),
        ..view
    })
}

fn recipient(demande: &For) -> Result<Recipient, CommandError> {
    Ok(Recipient {
        agent_id: parse_id::<Agent>(&demande.agent_id, "recipient.agent_id")?,
        worker_id: demande.worker_id.clone(),
        blind_to_generator: demande.blind_to_generator,
        clearance: confidentiality(demande.clearance),
    })
}

fn candidates(demandes: &[Candidate]) -> Result<Vec<(ContextItem, u64)>, CommandError> {
    demandes
        .iter()
        .map(|candidat| {
            let cites = candidat
                .cites
                .iter()
                .map(|cite| parse_id::<locus_domain::RevisionKind>(cite, "candidates[].cites"))
                .collect::<Result<Vec<RevisionId>, CommandError>>()?;
            let produced_by = candidat
                .produced_by
                .as_deref()
                .map(|agent| parse_id::<Agent>(agent, "candidates[].produced_by"))
                .transpose()?;
            Ok((
                ContextItem {
                    revision: parse_id::<locus_domain::RevisionKind>(
                        &candidat.revision_id,
                        "candidates[].revision_id",
                    )?,
                    is_generator_reasoning: candidat.is_generator_reasoning,
                    is_refuted: candidat.is_refuted,
                    classification: confidentiality(candidat.classification),
                    cites,
                    is_external_source: candidat.is_external_source,
                    produced_by,
                    disclosed: None,
                },
                candidat.position,
            ))
        })
        .collect()
}

fn parse_id<K: locus_protocol::id::IdKind>(
    value: &str,
    field: &str,
) -> Result<Id<K>, CommandError> {
    Id::parse(value).map_err(|erreur| CommandError::Validation {
        field: field.to_owned(),
        detail: erreur.to_string(),
    })
}

/// Les deux sens du même vocabulaire — §21.9 côté domaine, `data_class` côté fil.
///
/// `packages/artifacts/src/wire.rs` en tient une paire identique, et ce n'est pas une omission :
/// le foyer commun serait un crate que `locus-artifacts` et `locusd` voient tous deux et qui
/// traduit le vocabulaire du domaine vers celui du fil ; il n'existe pas, et l'inventer pour quatre
/// bras coûterait plus qu'il ne rapporte. La dérive, elle, est tenue par le compilateur : les deux
/// `match` sont exhaustifs, donc un cinquième niveau de sensibilité les casse toutes les deux.
const fn confidentiality(class: DataClass) -> Confidentiality {
    match class {
        DataClass::Public => Confidentiality::Public,
        DataClass::Internal => Confidentiality::Internal,
        DataClass::Confidential => Confidentiality::Confidential,
        DataClass::Restricted => Confidentiality::Restricted,
    }
}

/// Voir [`confidentiality`].
const fn data_class(confidentiality: Confidentiality) -> DataClass {
    match confidentiality {
        Confidentiality::Public => DataClass::Public,
        Confidentiality::Internal => DataClass::Internal,
        Confidentiality::Confidential => DataClass::Confidential,
        Confidentiality::Restricted => DataClass::Restricted,
    }
}

/// La construction d'une vue — le fait `context_view.built` de §10.3.
pub struct Built {
    /// La vue scellée.
    pub view: ContextView,
}

impl Decide for Built {
    type State = LepContext;

    fn decide(
        &self,
        command: &CommandEnvelope,
        context: &Self::State,
    ) -> Result<Vec<EventDraft>, CommandError> {
        Ok(vec![fact(
            command,
            context,
            "context_view.built",
            &stream_of_view(&self.view.id),
            serde_json::json!({
                "view_id": self.view.id,
                "content_hash": self.view.content_hash,
                // Le **document entier** — c'est lui que la route servira. L'invariant 2 dit que le
                // journal est la vérité institutionnelle ; une vue conservée ailleurs que dans le
                // journal ne survivrait pas au redémarrage que `W12.d.5` a rendu vérifiable.
                "view": self.view,
            }),
        )?])
    }
}

impl<S: EventStore> Runtime<S> {
    /// Bâtir une vue, l'écrire au journal, et rendre ce qui la nomme.
    ///
    /// # Une vue est immuable, donc son identifiant ne se réemploie pas
    ///
    /// §16.2 : « une `ContextView` est immuable, adressée par hash ». Réécrire sous le même
    /// identifiant ferait qu'une mission déjà proposée nommerait, après coup, un autre document que
    /// celui qu'elle avait nommé — et son empreinte le trahirait au moment le plus coûteux, chez le
    /// worker, sans que rien ici n'ait signalé quoi que ce soit. Le refus est donc ici.
    ///
    /// # Errors
    ///
    /// [`CommandError`] — ce que la construction, le décideur ou la transaction refusent, et
    /// `Policy` si une vue existe déjà sous cet identifiant.
    pub fn build_context_view(
        &self,
        requested: &Requested,
        authority: Authority,
        submitted: &Submitted,
        now: Timestamp,
    ) -> Result<ContextView, CommandError> {
        let stream = stream_of_view(&requested.id);
        if let Some((existante, cle)) = self.recorded_view(&requested.id)? {
            // §22.5 : « les clients peuvent resoumettre sans dupliquer l'effet ». Une resoumission
            // sous **la même clé** rend donc la vue déjà bâtie, sans écrire un second fait. La
            // distinguer d'une réécriture demande de lire la clé du fait, et non de comparer les
            // documents : `generated_at` change d'un envoi à l'autre, donc deux demandes identiques
            // produisent deux documents différents — une comparaison de contenu appellerait
            // « réécriture » ce qui n'est qu'un renvoi.
            if cle.as_deref() == Some(submitted.idempotency_key.as_str()) {
                return Ok(existante);
            }
            // `Policy` et non `Conflict` : dans ce daemon, `Conflict` porte une révision attendue et
            // ce qu'il faut relire pour retenter. Ici rien ne se retente — la règle est que
            // l'identifiant d'une vue ne se réemploie pas.
            return Err(CommandError::Policy {
                policy: "context_view.immutable".to_owned(),
                detail: format!(
                    "une vue « {} » existe déjà : §16.2 la veut immuable, et réécrire sous le même \
                     identifiant changerait ce qu'une mission déjà proposée avait nommé",
                    requested.id
                ),
            });
        }
        let vue = build(requested, submitted.occurred_at)?;
        let built = Built { view: vue.clone() };
        self.write_view_fact(authority, submitted, &stream, &built, now)?;
        Ok(vue)
    }

    /// La vue conservée sous cet identifiant, si elle existe.
    ///
    /// # Trois réponses, jamais deux
    ///
    /// `Ok(None)` dit **aucune vue sous ce nom** ; `Err` dit **un fait est là et ne se relit pas**.
    /// Les fondre ferait passer un journal corrompu pour un identifiant inconnu, et la route rendrait
    /// `404` à qui a raison de demander.
    ///
    /// # Errors
    ///
    /// [`CommandError::Internal`] quand le fait ne porte pas de vue relisible — ce qui ne peut venir
    /// que d'un journal écrit par une autre version, et se répare par migration.
    pub fn context_view(&self, view_id: &str) -> Result<Option<ContextView>, CommandError> {
        Ok(self.recorded_view(view_id)?.map(|(vue, _)| vue))
    }

    /// La vue conservée **et la clé d'idempotence sous laquelle elle a été bâtie**.
    ///
    /// La clé est ce qui distingue une resoumission d'une réécriture ; elle n'a pas d'autre usage,
    /// et c'est pourquoi [`Runtime::context_view`] ne la rend pas — une lecture publique qui
    /// exposerait la clé d'un client la ferait voyager sans raison.
    fn recorded_view(
        &self,
        view_id: &str,
    ) -> Result<Option<(ContextView, Option<String>)>, CommandError> {
        let faits = self
            .transaction_store()
            .read_stream(&stream_of_view(view_id), 0);
        // Le **premier** fait, jamais le dernier : une vue est immuable, donc c'est celui qui l'a
        // bâtie qui fait foi.
        let Some(fait) = faits.iter().find(|fait| fait.payload.get("view").is_some()) else {
            return Ok(None);
        };
        let brut = fait
            .payload
            .get("view")
            .unwrap_or(&serde_json::Value::Null)
            .clone();
        serde_json::from_value::<ContextView>(brut)
            .map(|vue| Some((vue, fait.idempotency_key.clone())))
            .map_err(|erreur| CommandError::Internal {
                detail: format!(
                    "le fait de construction de « {view_id} » ne se relit pas comme une vue : \
                     {erreur}"
                ),
            })
    }

    fn write_view_fact<D: Decide<State = LepContext>>(
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
            "context_view.build",
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
