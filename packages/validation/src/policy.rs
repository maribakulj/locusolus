//! La validation par type — `docs/SPEC_V1.md` §8.2.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use locus_domain::ValidationLevel;

/// Un événement qui invalide les dépendants — §8.2, dernière puce.
///
/// §8.3 en nomme trois : « réfuté, retiré ou révisé ». Ils ne se valent pas — une révision peut
/// laisser une conclusion debout, un retrait rarement — mais tous trois déclenchent la même
/// question, et c'est cette question que la propagation pose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvalidatingEvent {
    /// L'objet a été réfuté.
    Refuted,
    /// L'objet a été retiré par son auteur.
    Withdrawn,
    /// L'objet a été révisé — son contenu a changé sous ses dépendants.
    Revised,
}

impl InvalidatingEvent {
    /// Les trois événements de §8.3, dans l'ordre du texte.
    pub const ALL: [Self; 3] = [Self::Refuted, Self::Withdrawn, Self::Revised];
}

/// Ce qu'un schéma disciplinaire **DOIT** déclarer — §8.2, les six puces.
///
/// # Pourquoi tous les champs sont obligatoires
///
/// §8.2 dit « DOIT déclarer », et les six puces sont sur le même plan. Un champ facultatif ici
/// produirait une politique à moitié écrite qui aurait l'air complète : un pack qui oublierait
/// `inapplicable_levels` laisserait croire que tous les niveaux sont atteignables dans sa
/// discipline, alors que §8.1 dit explicitement que « ces niveaux ne forment pas toujours une
/// chaîne totale ».
///
/// Une liste **vide** reste possible et se distingue d'un champ absent : « aucune revue
/// obligatoire » est une décision, « je n'ai pas rempli ce champ » n'en est pas une. Le type ne
/// peut pas séparer les deux, mais [`TypePolicy::findings`] le peut, et il le fait pour
/// `minimal_evidence` — une discipline sans aucune preuve minimale est une discipline qui
/// n'exige rien.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypePolicy {
    /// Le type d'objet auquel la politique s'applique.
    pub object_type: String,
    /// La discipline qui la déclare.
    pub discipline: String,
    /// Les preuves minimales.
    pub minimal_evidence: Vec<String>,
    /// Les revues obligatoires.
    pub mandatory_reviews: Vec<String>,
    /// Les contrôles automatisables.
    pub automatable_checks: Vec<String>,
    /// Les niveaux que cette discipline déclare inapplicables — §8.1.
    pub inapplicable_levels: BTreeSet<String>,
    /// Les conditions de promotion, par niveau visé.
    pub promotion_conditions: Vec<Condition>,
    /// Les conditions de rétrogradation.
    pub demotion_conditions: Vec<Condition>,
    /// Les événements qui invalident les dépendants.
    pub invalidating_events: BTreeSet<InvalidatingEvent>,
}

/// Une condition de promotion ou de rétrogradation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Condition {
    /// Le niveau visé.
    pub level: String,
    /// Ce qui doit être vrai.
    pub requirement: String,
}

impl TypePolicy {
    /// Ce qui manque à une politique pour en être une.
    #[must_use]
    pub fn findings(&self) -> Vec<String> {
        let mut findings = Vec::new();
        if self.minimal_evidence.is_empty() {
            findings.push(
                "aucune preuve minimale : une discipline qui n'exige rien ne valide rien"
                    .to_owned(),
            );
        }
        if self.invalidating_events.is_empty() {
            // Sans événement invalidant, rien ne déclenche jamais la propagation de §8.3, et les
            // dépendants d'une prémisse réfutée resteraient tels quels sans que personne ne le
            // décide.
            findings.push(
                "aucun événement invalidant : la propagation de §8.3 ne se déclencherait jamais"
                    .to_owned(),
            );
        }
        for condition in &self.promotion_conditions {
            if self.inapplicable_levels.contains(&condition.level) {
                findings.push(format!(
                    "le niveau `{}` est déclaré inapplicable et porte pourtant une condition de promotion",
                    condition.level
                ));
            }
        }
        findings
    }

    /// Ce niveau est-il atteignable dans cette discipline — §8.1, §8.2.
    ///
    /// « Ces niveaux ne forment pas toujours une chaîne totale. Une interprétation historique peut
    /// atteindre L3 et L6 sans être *reproduite* au sens expérimental. » Une discipline non
    /// expérimentale déclare donc `Reproduced` inapplicable, et rien ne doit exiger d'y passer.
    #[must_use]
    pub fn is_applicable(&self, level: ValidationLevel) -> bool {
        !self.inapplicable_levels.contains(level.as_str())
    }

    /// Cet événement invalide-t-il les dépendants, selon cette discipline ?
    #[must_use]
    pub fn invalidates(&self, event: InvalidatingEvent) -> bool {
        self.invalidating_events.contains(&event)
    }
}
