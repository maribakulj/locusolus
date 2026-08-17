//! Les inférences et leurs hyperarêtes — `docs/SPEC_V1.md` §7.6.

use serde::{Deserialize, Serialize};

use locus_domain::RevisionId;

/// Où en est la formalisation d'une inférence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormalizationStatus {
    /// Énoncée en langue naturelle.
    Informal,
    /// En cours de formalisation.
    InProgress,
    /// Formalisée, non vérifiée.
    Formalized,
    /// Formalisée et vérifiée mécaniquement.
    Verified,
}

/// Ce qu'une objection peut viser — §7.6, dernière phrase.
///
/// « Une objection peut cibler l'inférence, une prémisse, la règle ou son domaine de validité. »
///
/// Quatre cibles, et elles ne disent pas la même chose. Objecter à une **prémisse**, c'est dire
/// qu'un fait est faux ; objecter à la **règle**, c'est dire que le raisonnement ne tient pas même
/// si tous les faits sont vrais ; objecter au **scope**, c'est dire que la règle vaut, mais pas
/// ici. Les fondre en une seule « objection à l'inférence » ferait perdre ce qu'il faut corriger.
///
/// C'est aussi la raison pour laquelle une inférence doit être un nœud : sur trois arêtes
/// indépendantes, « la règle est fausse » n'a **aucun endroit où s'accrocher**.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "target", rename_all = "snake_case")]
pub enum ObjectionTarget {
    /// L'inférence dans son ensemble.
    Inference,
    /// Une prémisse en particulier.
    Premise {
        /// La révision visée.
        revision_id: RevisionId,
    },
    /// La règle d'inférence elle-même.
    Rule,
    /// Le domaine de validité.
    Scope,
}

/// Une inférence — le nœud explicite de §7.6.
///
/// # Pourquoi ce n'est pas un ensemble d'arêtes
///
/// §7.6 : « le système **NE DOIT PAS** réduire un raisonnement multi-prémisses à plusieurs arêtes
/// indépendantes. »
///
/// Trois prémisses qui, **ensemble**, soutiennent une conclusion ne sont pas trois soutiens
/// séparés. La différence n'est pas de présentation :
///
/// - réfuter une prémisse sur trois casse l'inférence entière ; sur trois arêtes indépendantes,
///   il en resterait deux, et la conclusion paraîtrait encore soutenue « aux deux tiers » ;
/// - une objection à la règle n'a pas d'endroit où se poser sur des arêtes indépendantes, puisque
///   la règle n'y existe pas ;
/// - « quelles sont les prémisses minimales de ce claim » (§9.4) rendrait trois réponses d'une
///   prémisse au lieu d'une réponse de trois.
///
/// D'où la forme : les prémisses vivent dans **un** champ de ce nœud, et il n'existe dans ce crate
/// aucune fonction qui les décompose en arêtes. Un test le vérifie par l'absence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inference {
    /// L'identité de l'inférence.
    pub id: String,
    /// Le genre d'inférence — déduction, abduction, induction, analogie…
    ///
    /// Ouvert exprès : §7.6 ne donne pas de liste fermée, et les packs disciplinaires en ajoutent.
    pub inference_kind: String,
    /// Les prémisses. **Ensemble**, pas une par une.
    pub premise_ids: Vec<RevisionId>,
    /// Les conclusions.
    pub conclusion_ids: Vec<RevisionId>,
    /// Les hypothèses admises sans preuve, distinctes des prémisses.
    ///
    /// §7.6 les sépare, et la séparation porte : une prémisse est affirmée, une hypothèse est
    /// admise. Les confondre ferait passer pour établi ce qui a seulement été supposé.
    #[serde(default)]
    pub assumption_ids: Vec<RevisionId>,
    /// La règle appliquée.
    pub rule: String,
    /// Le domaine de validité de la règle.
    pub scope: String,
    /// Où en est la formalisation.
    pub formalization_status: FormalizationStatus,
    /// Sur quoi l'inférence s'appuie.
    #[serde(default)]
    pub evidence_refs: Vec<RevisionId>,
    /// Qui l'a posée.
    pub author: String,
    /// Où en est la revue.
    pub review_status: String,
}

/// Ce qui manque à une inférence pour en être une.
///
/// Rend des constats plutôt que de lever : une inférence se corrige mieux avec la liste complète.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferenceFindings(pub Vec<String>);

impl Inference {
    /// Le nombre de prémisses. Trois prémisses font **une** inférence, jamais trois.
    #[must_use]
    pub fn arity(&self) -> usize {
        self.premise_ids.len()
    }

    /// Vrai quand l'inférence porte plus d'une prémisse — le cas que §7.6 protège.
    #[must_use]
    pub fn is_multi_premise(&self) -> bool {
        self.arity() > 1
    }

    /// Vrai si cette révision est l'une des prémisses.
    #[must_use]
    pub fn has_premise(&self, revision_id: &RevisionId) -> bool {
        self.premise_ids.contains(revision_id)
    }

    /// Vrai si cette révision est l'une des conclusions.
    #[must_use]
    pub fn concludes(&self, revision_id: &RevisionId) -> bool {
        self.conclusion_ids.contains(revision_id)
    }

    /// Ce qui cloche.
    ///
    /// Une inférence sans prémisse est une conclusion déguisée en raisonnement ; sans conclusion,
    /// c'est un raisonnement qui ne conclut rien ; sans règle, c'est une juxtaposition.
    #[must_use]
    pub fn findings(&self) -> InferenceFindings {
        let mut findings = Vec::new();
        if self.premise_ids.is_empty() {
            findings.push("aucune prémisse : une conclusion déguisée en raisonnement".to_owned());
        }
        if self.conclusion_ids.is_empty() {
            findings.push("aucune conclusion : un raisonnement qui ne conclut rien".to_owned());
        }
        if self.rule.trim().is_empty() {
            findings.push("aucune règle : une juxtaposition, pas une inférence".to_owned());
        }
        if self.scope.trim().is_empty() {
            // Sans domaine de validité, l'objection de §7.6 « la règle ne vaut pas ici » n'a pas
            // de cible.
            findings.push(
                "aucun domaine de validité : une objection de scope n'aurait pas de cible"
                    .to_owned(),
            );
        }
        let mut seen = self.premise_ids.clone();
        seen.sort_unstable();
        seen.dedup();
        if seen.len() != self.premise_ids.len() {
            findings.push("une prémisse est répétée : elle ne compte pas deux fois".to_owned());
        }
        InferenceFindings(findings)
    }

    /// Les cibles d'objection que cette inférence offre — §7.6.
    ///
    /// Une par prémisse, plus l'inférence, la règle et le scope. C'est cette liste qui n'existe pas
    /// sur un graphe d'arêtes indépendantes : `Rule` et `Scope` n'y auraient aucun support.
    #[must_use]
    pub fn objection_targets(&self) -> Vec<ObjectionTarget> {
        let mut targets = vec![
            ObjectionTarget::Inference,
            ObjectionTarget::Rule,
            ObjectionTarget::Scope,
        ];
        targets.extend(
            self.premise_ids
                .iter()
                .map(|revision_id| ObjectionTarget::Premise {
                    revision_id: *revision_id,
                }),
        );
        targets
    }
}
