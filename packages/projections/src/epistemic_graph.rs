//! La projection « graphe épistémique » — `docs/SPEC_V1.md` §9.3, §9.4, §7.6, `W20.u`.
//!
//! # Ce qu'elle reconstruit, et d'où
//!
//! Du **journal seul**. Aucun instantané reçu d'un worker n'entre ici : un worker qui enverrait
//! « voici mon graphe » ferait de son transcript la vérité institutionnelle, ce que l'invariant 2
//! réserve au journal. `W13.g` tient déjà cette règle pour le graphe organisationnel ; celle-ci en
//! est l'application au graphe épistémique.
//!
//! Les sources sont trois familles de faits, et rien d'autre :
//!
//! - `epistemic_object.*` — la charge porte le `EpistemicCommit` de §15.7, avec ses claims, ses
//!   inférences et ses objections ;
//! - `artifact.*` — ce qu'une tâche a produit (`W20.t`) ;
//! - `budget.*` — ce qu'elle a coûté (§7.2).
//!
//! # §7.6 est tenu par le type, pas par la discipline
//!
//! Les prémisses d'une inférence sont un **ensemble**, jamais des soutiens séparés :
//! [`locus_graph::Graph`] n'offre aucun chemin de l'hyperarête vers des arêtes, et c'est lui qui
//! range les inférences ici. Une projection qui aurait tenu ses propres `Vec<Vec<String>>` aurait
//! offert la même garantie **par la vigilance de qui la relit**, ce qui n'est pas la même chose.
//!
//! Conséquence directe : une inférence dont **une** référence ne se relit pas n'entre pas
//! amputée. Elle n'entre pas du tout, et se lit comme illisible — voir [`Unreadable`]. Y entrer
//! avec deux prémisses sur trois ferait paraître la conclusion soutenue par un raisonnement que
//! personne n'a posé, ce qui est précisément la faute que §7.6 nomme.
//!
//! # Trois absences, et elles ne se confondent pas
//!
//! - **aucune inférence ne conclut cette révision** : la conclusion existe et n'est soutenue par
//!   rien. Elle se lit, avec zéro ensemble de prémisses — elle ne « manque » pas ;
//! - **une inférence existe mais ses références sont illisibles** : quelque chose a été posé, et ce
//!   quelque chose n'est pas interrogeable ;
//! - **personne n'a relevé le coût** : `None`, jamais `0`. Un zéro dirait « cette recherche n'a
//!   rien coûté », ce qui est une affirmation ; l'absence dit « personne n'a compté ».

use std::collections::{BTreeMap, BTreeSet};

use locus_domain::RevisionId;
use locus_event_store::Envelope;
use locus_graph::{Graph, Inference, Support};

use crate::projection::{Projection, ProjectionError, Watermark};

/// Une inférence que le graphe n'a pas pu accueillir, et pourquoi.
///
/// Elle n'est **pas** perdue : l'invariant 12 vaut ici comme ailleurs, et une inférence tue serait
/// un raisonnement effacé pour rendre le graphe propre. Elle est rangée à part parce qu'une
/// inférence dont les références ne se relisent pas ne peut pas être interrogée — pas parce qu'elle
/// serait fausse.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Unreadable {
    /// La règle qu'elle appliquait, telle qu'elle a été écrite.
    pub rule: String,
    /// Les références qui n'ont pas pu être relues comme identifiants de révision.
    pub refs: Vec<String>,
    /// La tâche d'où elle vient.
    pub task_id: String,
}

/// Une objection, telle que le commit l'a portée — §15.7, invariant 12.
///
/// Les cibles restent des **chaînes brutes**, et c'est délibéré : une objection qui vise une
/// référence illisible reste une objection. La refuser pour cette raison reviendrait à taire une
/// contestation parce qu'elle est mal adressée, ce que l'invariant 12 interdit plus fermement que
/// n'importe quelle règle de forme.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Objection {
    /// Ce qui est objecté.
    pub statement: String,
    /// Ce que l'objection vise.
    pub targets: Vec<String>,
    /// La tâche d'où elle vient.
    pub task_id: String,
}

/// Ce qu'une tâche a produit — `W20.t`, §19.1.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ArtifactRecord {
    /// L'artefact.
    pub artifact_id: String,
    /// Son état, tel que le dernier fait le dit.
    pub state: String,
}

/// Ce qu'une tâche a coûté — §7.2.
///
/// # Pourquoi une somme par dimension et non un total
///
/// §7.2 borne séparément l'argent, les appels de modèle, les tokens, les secondes de calcul et le
/// temps réel. Les additionner produirait un nombre qui n'a pas d'unité, et qu'aucune limite ne
/// peut contredire. La table garde donc chaque dimension sous son nom.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Cost {
    /// Ce qui a été **consommé**, par dimension.
    pub consumed: BTreeMap<String, u64>,
    /// Le nombre d'écritures qui l'ont composé — §7.2 : « le budget est un registre ».
    pub entries: usize,
}

/// L'expérience d'où vient un raisonnement : la tâche et sa tentative.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub struct Experiment {
    /// La tâche.
    pub task_id: String,
    /// Le rang de la tentative — §12.3 veut qu'une réattribution le conserve.
    pub attempt: i64,
}

/// Ce que l'institution sait d'une conclusion — les six termes de §9.4.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Dossier {
    /// La révision interrogée.
    pub conclusion: RevisionId,
    /// Les prémisses, **un ensemble par inférence**. Vide veut dire « rien ne la soutient », et
    /// c'est une réponse.
    pub premise_sets: Vec<Vec<RevisionId>>,
    /// Les inférences posées sur cette conclusion que le graphe n'a pas pu accueillir.
    pub unreadable: Vec<Unreadable>,
    /// Les expériences d'où viennent les inférences qui la concluent.
    pub experiments: Vec<Experiment>,
    /// Ce que ces expériences ont produit.
    pub artifacts: Vec<ArtifactRecord>,
    /// Ce qui la conteste — invariant 12.
    pub objections: Vec<Objection>,
    /// Ce qu'elle a coûté, **ou rien du tout si personne ne l'a relevé**.
    pub cost: Option<Cost>,
}

/// Le graphe épistémique reconstruit du journal.
#[derive(Debug, Default)]
pub struct EpistemicGraph {
    graph: Graph,
    /// La tâche d'où vient chaque inférence, par identifiant d'inférence.
    origins: BTreeMap<String, Experiment>,
    unreadable: Vec<Unreadable>,
    objections: Vec<Objection>,
    /// Les artefacts, par tâche.
    artifacts: BTreeMap<String, BTreeMap<String, ArtifactRecord>>,
    /// Le coût, par tâche.
    costs: BTreeMap<String, Cost>,
    watermark: Watermark,
}

impl EpistemicGraph {
    /// Le nom de la projection.
    pub const NAME: &'static str = "epistemic_graph";

    /// Un graphe vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Le nombre d'inférences accueillies.
    #[must_use]
    pub fn inference_count(&self) -> usize {
        self.graph.inference_count()
    }

    /// Les inférences que le graphe n'a pas pu accueillir.
    #[must_use]
    pub fn unreadable(&self) -> &[Unreadable] {
        &self.unreadable
    }

    /// Toutes les objections, y compris celles qui ne visent rien de connu — invariant 12.
    #[must_use]
    pub fn objections(&self) -> &[Objection] {
        &self.objections
    }

    /// Ce que le journal dit de cette conclusion — les six termes de §9.4.
    ///
    /// Rend toujours un dossier, **même vide** : « rien ne soutient cette conclusion » est une
    /// réponse, et c'est celle que §9.4 attend. Un `None` ferait passer une conclusion isolée pour
    /// une conclusion inconnue, et un lecteur relancerait sa requête au lieu de lire ce qu'elle
    /// dit.
    #[must_use]
    pub fn dossier(&self, conclusion: &RevisionId) -> Dossier {
        let premise_sets = self.graph.minimal_premise_sets(conclusion);
        let concluding: Vec<&Inference> = self
            .graph
            .supports_of(conclusion)
            .into_iter()
            .filter_map(|support| match support {
                Support::Inference(inference) => Some(inference),
                Support::Relation(_) => None,
            })
            .collect();

        let mut experiments: Vec<Experiment> = concluding
            .iter()
            .filter_map(|inference| self.origins.get(&inference.id).cloned())
            .collect();
        experiments.sort();
        experiments.dedup();

        let mut artifacts: Vec<ArtifactRecord> = experiments
            .iter()
            .filter_map(|experiment| self.artifacts.get(&experiment.task_id))
            .flat_map(|par_tache| par_tache.values().cloned())
            .collect();
        artifacts.sort_by(|left, right| left.artifact_id.cmp(&right.artifact_id));
        artifacts.dedup();

        // Une objection vise la conclusion elle-même, ou l'une des inférences qui la concluent.
        // Les deux comptent : « la règle est fausse » ne vise pas la conclusion et la conteste
        // pourtant — c'est exactement pourquoi §7.6 fait de l'inférence un nœud.
        let vises: BTreeSet<String> = std::iter::once(conclusion.to_string())
            .chain(concluding.iter().map(|inference| inference.id.clone()))
            .collect();
        let objections: Vec<Objection> = self
            .objections
            .iter()
            .filter(|objection| objection.targets.iter().any(|cible| vises.contains(cible)))
            .cloned()
            .collect();

        let unreadable: Vec<Unreadable> = self
            .unreadable
            .iter()
            .filter(|inference| inference.refs.contains(&conclusion.to_string()))
            .cloned()
            .collect();

        Dossier {
            conclusion: *conclusion,
            premise_sets,
            unreadable,
            cost: cost_of(&self.costs, &experiments),
            experiments,
            artifacts,
            objections,
        }
    }

    /// Absorber un commit épistémique.
    fn absorb_commit(
        &mut self,
        position: u64,
        payload: &serde_json::Value,
    ) -> Result<(), ProjectionError> {
        let Some(commit) = payload.get("commit") else {
            // Un fait épistémique sans commit n'est pas une faute : `W20.r` en écrit pour des actes
            // d'autorité qui ne portent pas de raisonnement. Il n'apporte simplement rien au graphe.
            return Ok(());
        };
        let task_id = text(payload, "task_id").unwrap_or_default();
        let attempt = commit
            .get("attempt")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or_default();

        for (rang, brut) in tableau(commit, "inferences").iter().enumerate() {
            let rule = text(brut, "rule").unwrap_or_default();
            let premises = refs(brut, "premise_refs");
            let conclusions = refs(brut, "conclusion_refs");
            let assumptions = refs(brut, "assumption_refs");

            let Some((premise_ids, conclusion_ids, assumption_ids)) =
                relire(&premises, &conclusions, &assumptions)
            else {
                self.unreadable.push(Unreadable {
                    rule,
                    refs: premises.into_iter().chain(conclusions).collect(),
                    task_id: task_id.clone(),
                });
                continue;
            };

            // L'identité vient de la **position dans le journal** et du rang dans le commit. Elle
            // est donc stable à la reconstruction — deux passages sur le même journal la rendent à
            // l'identique — sans qu'aucune entropie soit inventée ici.
            let id = format!("inference/{position}/{rang}");
            self.origins.insert(
                id.clone(),
                Experiment {
                    task_id: task_id.clone(),
                    attempt,
                },
            );
            self.graph.add_inference(Inference {
                id,
                inference_kind: text(brut, "inference_kind").unwrap_or_default(),
                premise_ids,
                conclusion_ids,
                assumption_ids,
                rule,
                scope: text(brut, "scope").unwrap_or_default(),
                formalization_status: locus_graph::FormalizationStatus::Informal,
                evidence_refs: Vec::new(),
                author: task_id.clone(),
                review_status: text(payload, "status").unwrap_or_default(),
            });
        }

        for brut in tableau(commit, "objections") {
            let Some(statement) = text(&brut, "statement") else {
                return Err(ProjectionError {
                    position,
                    reason: "objection sans énoncé : l'invariant 12 veut qu'elle soit du contenu \
                             de premier plan, et un énoncé vide n'en est pas"
                        .to_owned(),
                });
            };
            self.objections.push(Objection {
                statement,
                targets: refs(&brut, "targets"),
                task_id: task_id.clone(),
            });
        }
        Ok(())
    }

    /// Absorber un fait d'artefact — `W20.t`.
    fn absorb_artifact(
        &mut self,
        position: u64,
        payload: &serde_json::Value,
    ) -> Result<(), ProjectionError> {
        let artifact_id = text(payload, "artifact_id").ok_or_else(|| ProjectionError {
            position,
            reason: "`artifact_id` absent : un artefact sans identité n'est rattachable à rien"
                .to_owned(),
        })?;
        let task_id = payload
            .get("produced_by")
            .and_then(|par| text(par, "task_id"))
            .unwrap_or_default();
        self.artifacts.entry(task_id).or_default().insert(
            artifact_id.clone(),
            ArtifactRecord {
                artifact_id,
                state: text(payload, "state").unwrap_or_default(),
            },
        );
        Ok(())
    }

    /// Absorber une écriture de budget — §7.2.
    ///
    /// **Seule la consommation compte** dans un coût. Une réservation est de l'argent tenu, pas
    /// dépensé, et l'additionner ferait payer deux fois ce qui sera consommé ensuite ; une
    /// allocation est un plafond. §7.2 énumère six sortes d'écritures, et les confondre rendrait un
    /// nombre qui ne veut rien dire.
    fn absorb_budget(&mut self, position: u64, event: &Envelope) -> Result<(), ProjectionError> {
        if event.event_type.verb() != "consumed" {
            return Ok(());
        }
        let payload = &event.payload;
        let task_id = text(payload, "task_id").ok_or_else(|| ProjectionError {
            position,
            reason: "consommation sans tâche : elle ne peut être imputée à aucune expérience"
                .to_owned(),
        })?;
        let amounts = payload
            .get("amounts")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| ProjectionError {
                position,
                reason: "consommation sans montants : un coût sans dimension n'est pas un coût"
                    .to_owned(),
            })?;

        let cost = self.costs.entry(task_id).or_default();
        cost.entries += 1;
        for (dimension, valeur) in amounts {
            let Some(montant) = valeur.as_u64() else {
                return Err(ProjectionError {
                    position,
                    reason: format!(
                        "montant « {dimension} » non entier positif : §7.2 fait du budget un \
                         registre, et une correction s'écrit comme un ajustement compensatoire — \
                         jamais comme une consommation négative"
                    ),
                });
            };
            *cost.entry_for(dimension) += montant;
        }
        Ok(())
    }
}

impl Cost {
    /// La somme d'une dimension, créée à zéro **à la première écriture qui la nomme**.
    ///
    /// Ce zéro-là est licite : il est le point de départ d'une somme dont une écriture existe. Le
    /// zéro que ce module refuse est celui d'une dimension que personne n'a relevée, et il ne peut
    /// pas naître ici — il faudrait qu'une écriture le nomme d'abord.
    fn entry_for(&mut self, dimension: &str) -> &mut u64 {
        self.consumed.entry(dimension.to_owned()).or_default()
    }
}

impl Projection for EpistemicGraph {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&mut self, position: u64, event: &Envelope) -> Result<(), ProjectionError> {
        self.watermark = position;
        match event.event_type.namespace() {
            "epistemic_object" => self.absorb_commit(position, &event.payload),
            "artifact" => self.absorb_artifact(position, &event.payload),
            "budget" => self.absorb_budget(position, event),
            _ => Ok(()),
        }
    }

    fn watermark(&self) -> Watermark {
        self.watermark
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    fn checksum(&self) -> String {
        format!(
            "{}:{}:{}:{}:{}:{}",
            self.graph.inference_count(),
            self.graph.relation_count(),
            self.unreadable.len(),
            self.objections.len(),
            self.artifacts.values().map(BTreeMap::len).sum::<usize>(),
            self.costs.len(),
        )
    }
}

/// Le coût des expériences citées, ou **rien** si aucune n'a été relevée.
///
/// La distinction est la clause du test de sortie : « le coût est absent tant que personne ne l'a
/// relevé, jamais nul ». Un `Cost::default()` rendu ici dirait « ces expériences ont coûté zéro »,
/// ce qui est une affirmation — et fausse, puisqu'une exécution coûte toujours quelque chose. Ce
/// que le journal permet de dire est « personne n'a compté ».
fn cost_of(costs: &BTreeMap<String, Cost>, experiments: &[Experiment]) -> Option<Cost> {
    let releves: Vec<&Cost> = experiments
        .iter()
        .filter_map(|experiment| costs.get(&experiment.task_id))
        .collect();
    if releves.is_empty() {
        return None;
    }
    let mut total = Cost::default();
    for cost in releves {
        total.entries += cost.entries;
        for (dimension, montant) in &cost.consumed {
            *total.entry_for(dimension) += montant;
        }
    }
    Some(total)
}

/// Relire les trois listes de références d'une inférence, **ou aucune**.
///
/// Tout ou rien, et c'est §7.6 : une inférence à trois prémisses dont une référence ne se relit pas
/// n'entre pas avec deux. Elle ferait paraître la conclusion soutenue par un raisonnement que
/// personne n'a posé, et réfuter les deux prémisses restantes laisserait croire la troisième
/// encore debout.
fn relire(
    premises: &[String],
    conclusions: &[String],
    assumptions: &[String],
) -> Option<(Vec<RevisionId>, Vec<RevisionId>, Vec<RevisionId>)> {
    Some((
        ids(premises)?,
        ids(conclusions)?,
        ids(assumptions).unwrap_or_default(),
    ))
}

fn ids(refs: &[String]) -> Option<Vec<RevisionId>> {
    refs.iter()
        .map(|brut| RevisionId::parse(brut).ok())
        .collect()
}

fn text(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

fn tableau(value: &serde_json::Value, key: &str) -> Vec<serde_json::Value> {
    value
        .get(key)
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn refs(value: &serde_json::Value, key: &str) -> Vec<String> {
    tableau(value, key)
        .iter()
        .filter_map(|item| item.as_str().map(str::to_owned))
        .collect()
}
