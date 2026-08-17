//! Les résultats négatifs — `docs/SPEC_V1.md` §18.7.

use serde::{Deserialize, Serialize};

use crate::ids::RevisionId;

/// La puissance statistique ou formelle d'un résultat négatif.
///
/// # Pourquoi `Unstated` existe
///
/// C'est le champ qui décide de ce qu'un échec **exclut**. Une recherche à faible puissance qui
/// ne trouve rien n'exclut presque rien ; la même recherche à forte puissance exclut beaucoup.
///
/// Une puissance non déclarée n'est donc pas une forte puissance, et ce n'est pas non plus une
/// faible : c'est une absence d'information, et [`NegativeResult::excludes`] refuse d'en tirer
/// quoi que ce soit. Traiter l'absence comme une valeur ferait dire à un résultat négatif ce que
/// personne n'a mesuré — exactement ce que §8.4 refuse pour les scores de confiance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Power {
    /// Une puissance statistique chiffrée, dans `[0, 1]`, avec ce qu'elle mesure.
    Statistical {
        /// La valeur.
        value: f64,
        /// Ce que le chiffre mesure — taille d'effet détectable, seuil, méthode.
        basis: String,
    },
    /// Une exhaustivité formelle : l'espace a été parcouru en entier.
    Exhaustive {
        /// Ce qui a été parcouru en entier.
        over: String,
    },
    /// Non déclarée. **Pas une faible puissance** : une absence d'information.
    Unstated,
}

impl Power {
    /// Vrai quand la puissance permet de conclure quelque chose.
    ///
    /// Le seuil de 0.8 est une **politique**, pas une lecture de la spec : §18.7 demande que le
    /// champ existe sans chiffrer ce qui suffit. Il vit ici en constante pour être discuté d'un
    /// seul endroit.
    #[must_use]
    pub fn is_conclusive(&self) -> bool {
        match self {
            Self::Statistical { value, .. } => *value >= CONCLUSIVE_POWER,
            Self::Exhaustive { .. } => true,
            Self::Unstated => false,
        }
    }
}

/// Le seuil au-delà duquel une puissance statistique permet d'exclure. Politique, pas spec.
pub const CONCLUSIVE_POWER: f64 = 0.8;

/// Ce que l'échec permet d'exclure — la troisième question de §18.7.
///
/// « Cette attaque a-t-elle déjà été tentée, dans quelles conditions, et **qu'est-ce que son échec
/// exclut réellement** ? »
///
/// Le mot « réellement » est le sujet de ce type. Un résultat négatif dit toujours quelque chose
/// sur ce qui a été tenté ; il ne dit presque rien sur ce qui n'a pas été tenté, et c'est la
/// confusion des deux qui fait qu'une piste abandonnée est réputée close.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Exclusion {
    /// L'échec exclut quelque chose, dans les limites du scope parcouru.
    Excludes {
        /// Ce qui est exclu.
        statement: String,
        /// Où cela vaut — jamais au-delà du `search_space` et de l'`applicability_scope`.
        within: String,
    },
    /// L'échec n'exclut rien de vérifiable, et voici pourquoi.
    ///
    /// C'est un **résultat**, pas une absence de résultat : savoir qu'une tentative n'a rien
    /// prouvé évite de la refaire en croyant qu'elle avait prouvé quelque chose.
    ExcludesNothing {
        /// Pourquoi.
        reason: String,
    },
}

/// Un résultat négatif — les onze champs de §18.7, dans l'ordre du texte.
///
/// Invariant 12 : « les résultats négatifs et conflits ne sont jamais supprimés pour rendre le
/// graphe *propre* ». Il n'existe dans ce module aucune fonction qui en retire un, et le test de
/// sortie de W1.g le vérifie **sur tout le workspace**, pas seulement ici.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NegativeResult {
    /// La question ou l'hypothèse visée.
    pub question_or_hypothesis: String,
    /// La méthode employée.
    pub method: String,
    /// Ses paramètres.
    pub parameters: String,
    /// L'espace de recherche parcouru.
    pub search_space: String,
    /// Les conditions expérimentales.
    pub conditions: String,
    /// Ce qui s'est passé.
    pub outcome: String,
    /// La puissance statistique ou formelle.
    pub statistical_or_formal_power: Power,
    /// Les limitations connues.
    pub known_limitations: Vec<String>,
    /// Le mode d'échec.
    pub failure_mode: String,
    /// Les artefacts produits.
    #[serde(default)]
    pub artifacts: Vec<RevisionId>,
    /// Le domaine d'applicabilité de la conclusion négative.
    pub applicability_scope: String,
}

impl NegativeResult {
    /// Ce que cet échec exclut réellement — §18.7.
    ///
    /// Trois refus, et chacun correspond à une manière dont une piste se ferme à tort :
    ///
    /// - **puissance non déclarée** : personne ne sait si l'échec est informatif ;
    /// - **puissance insuffisante** : l'échec est compatible avec l'existence de ce qu'on cherchait ;
    /// - **espace de recherche vide** : on n'a pas cherché.
    ///
    /// Quand il exclut, l'exclusion est **bornée** par le scope déclaré. Un résultat négatif obtenu
    /// sur un intervalle ne dit rien au-delà, et laisser l'énoncé sans borne est la façon dont
    /// « nous n'avons pas trouvé X ici » devient « X n'existe pas ».
    #[must_use]
    pub fn excludes(&self) -> Exclusion {
        if matches!(self.statistical_or_formal_power, Power::Unstated) {
            return Exclusion::ExcludesNothing {
                reason:
                    "puissance non déclarée : une puissance absente n'est pas une forte puissance"
                        .to_owned(),
            };
        }
        if !self.statistical_or_formal_power.is_conclusive() {
            return Exclusion::ExcludesNothing {
                reason: format!(
                    "puissance insuffisante (seuil {CONCLUSIVE_POWER}) : l'échec est compatible avec l'existence de ce qui était cherché"
                ),
            };
        }
        if self.search_space.trim().is_empty() {
            return Exclusion::ExcludesNothing {
                reason: "espace de recherche non déclaré : on ne sait pas où l'on a cherché"
                    .to_owned(),
            };
        }
        Exclusion::Excludes {
            statement: format!(
                "`{}` n'a pas été obtenu par `{}`",
                self.question_or_hypothesis, self.method
            ),
            within: format!("{} ; {}", self.search_space, self.applicability_scope),
        }
    }

    /// La question de §18.7, posée par un moteur de recherche.
    ///
    /// « Cette attaque a-t-elle déjà été tentée, dans quelles conditions ? » — la réponse est le
    /// couple méthode + conditions, et c'est sur lui qu'un futur chercheur reconnaîtra sa propre
    /// tentative avant de la refaire.
    #[must_use]
    pub fn attempt_signature(&self) -> String {
        format!(
            "{} | {} | {}",
            self.method.trim(),
            self.parameters.trim(),
            self.conditions.trim()
        )
    }

    /// Ce qui manque à un résultat négatif pour être exploitable.
    #[must_use]
    pub fn findings(&self) -> Vec<String> {
        let mut findings = Vec::new();
        for (name, value) in [
            ("question_or_hypothesis", &self.question_or_hypothesis),
            ("method", &self.method),
            ("outcome", &self.outcome),
            ("failure_mode", &self.failure_mode),
            ("applicability_scope", &self.applicability_scope),
        ] {
            if value.trim().is_empty() {
                findings.push(format!("`{name}` vide"));
            }
        }
        if matches!(self.statistical_or_formal_power, Power::Unstated) {
            // Pas une erreur : un résultat négatif sans puissance déclarée reste un fait qu'il faut
            // conserver. Mais il n'exclut rien, et le dire évite qu'on le cite comme s'il excluait.
            findings.push(
                "puissance non déclarée : ce résultat est conservé mais n'exclut rien (§18.7)"
                    .to_owned(),
            );
        }
        findings
    }
}
