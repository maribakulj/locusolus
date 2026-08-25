//! L'affectation classe → modèle — `W25.a`, ADR 0026 décision 6.
//!
//! # Une valeur de politique, jamais une constante de code
//!
//! Le domaine déclare une **classe de cognition** ; c'est ici qu'on décide quel modèle la sert. La
//! séparation est le levier tout entier : « frontière pour planifier, bon marché pour exécuter » est
//! une décision d'exploitation, qui change quand les prix changent, et qui ne doit rien coûter à
//! changer.
//!
//! # Ce module ne connaît pas le type `CognitionClass`, et c'est voulu
//!
//! Il indexe par **slug**. `packages/policy` ne dépend d'aucun autre crate du dépôt — pas même du
//! domaine — et cet item ne le fait pas changer.
//!
//! C'est ce qui rend vraie la clause qui porte l'item, *changer l'affectation ne change aucun type* :
//! l'affectation est une table de chaînes vers des chaînes, portée par une version. Ajouter un modèle,
//! en retirer un, permuter les deux — rien de tout cela ne touche une signature, ici ou ailleurs.
//!
//! Prendre un `CognitionClass` en paramètre aurait été plus « typé » et aurait cassé exactement la
//! propriété demandée : la politique aurait alors une opinion sur l'énumération du domaine, et
//! ajouter un barreau serait devenu un changement de type traversant.
//!
//! # Versionnée et visible
//!
//! §20.5 demande « politique et version ». Une résolution rend donc [`Resolved`], qui porte la
//! version de l'affectation qui a répondu — de la même façon que [`crate::Fired`] porte celle de la
//! règle qui a matché. Sans elle, deux exploitations lisant la même trace ne sauraient pas si elles
//! parlent de la même affectation.
//!
//! **Ce n'est pas un [`crate::Fired`]**, et le fondre dedans dirait qu'une règle s'est déclenchée
//! alors qu'aucune ne l'a fait. Une affectation n'a pas de verbe, pas de priorité, et ne décide rien
//! au sens de §20.2 : elle répond à une question.

use std::collections::BTreeMap;
use std::fmt;

/// L'affectation d'un modèle à chaque classe de cognition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
    version: u32,
    models: BTreeMap<String, String>,
}

impl Assignment {
    /// Poser une affectation versionnée.
    ///
    /// # Errors
    ///
    /// [`AssignmentError::EmptyClass`] et [`AssignmentError::EmptyModel`] : une entrée dont l'un des
    /// deux côtés est vide ne sert à rien et se lirait, dans une trace, comme une résolution qui a
    /// abouti. [`AssignmentError::DuplicateClass`] : deux valeurs pour une classe rendraient la
    /// résolution dépendante de l'ordre d'insertion.
    pub fn versioned(
        version: u32,
        entries: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Self, AssignmentError> {
        let mut models = BTreeMap::new();
        for (class, model) in entries {
            if class.trim().is_empty() {
                return Err(AssignmentError::EmptyClass);
            }
            if model.trim().is_empty() {
                return Err(AssignmentError::EmptyModel { class });
            }
            if models.insert(class.clone(), model).is_some() {
                return Err(AssignmentError::DuplicateClass { class });
            }
        }
        Ok(Self { version, models })
    }

    /// Quel modèle sert cette classe ?
    ///
    /// Rend `None` quand l'affectation ne dit rien de cette classe. **Pas de modèle par défaut** :
    /// un défaut ferait tourner une mission sur un modèle que personne n'a choisi, et le silence
    /// serait lu comme une décision — ce que `Outcome::NoRule` distingue déjà pour les règles, et
    /// pour la même raison.
    #[must_use]
    pub fn resolve(&self, class: &str) -> Option<Resolved> {
        self.models.get(class).map(|model| Resolved {
            class: class.to_owned(),
            model: model.clone(),
            version: self.version,
        })
    }

    /// Sa version — §20.5, « politique et version ».
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Les classes affectées, dans l'ordre.
    pub fn classes(&self) -> impl Iterator<Item = &str> {
        self.models.keys().map(String::as_str)
    }
}

/// Ce qu'une résolution rend, et qui entre dans la trace de §20.5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// La classe demandée.
    pub class: String,
    /// Le modèle qui la sert.
    pub model: String,
    /// La version de l'affectation qui a répondu.
    pub version: u32,
}

impl fmt::Display for Resolved {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} → {} (affectation v{})",
            self.class, self.model, self.version
        )
    }
}

/// Ce qui empêche une affectation d'exister.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignmentError {
    /// Une entrée sans classe.
    EmptyClass,
    /// Une classe sans modèle.
    EmptyModel {
        /// Laquelle.
        class: String,
    },
    /// Deux modèles pour la même classe.
    DuplicateClass {
        /// Laquelle.
        class: String,
    },
}

impl fmt::Display for AssignmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyClass => formatter.write_str(
                "une affectation sans classe : elle se lirait dans une trace comme une résolution \
                 qui a abouti",
            ),
            Self::EmptyModel { class } => write!(
                formatter,
                "« {class} » n'a pas de modèle : une classe affectée à rien n'est pas affectée"
            ),
            Self::DuplicateClass { class } => write!(
                formatter,
                "« {class} » est affectée deux fois : la résolution dépendrait de l'ordre \
                 d'insertion"
            ),
        }
    }
}

impl std::error::Error for AssignmentError {}
