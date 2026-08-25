//! Le plafond de cognition — `W25.b`, ADR 0026 décision 6.
//!
//! # Ce que le plafond borne, et pourquoi ce n'est pas une septième dimension
//!
//! §7.2 a six dimensions, et l'ADR 0026 dit qu'« il manque la dimension de cognition et son
//! plafond ». Prise au pied de la lettre, cette phrase demanderait une septième valeur à
//! [`crate::Dimension`] — et il faudrait alors dire en quelle **unité** on la compte. Personne ne
//! sait : la cognition n'a pas d'unité propre, elle se paie en appels, en jetons et en argent, qui
//! sont déjà là.
//!
//! Ce qui manque n'est donc pas une unité, c'est une **clé**. Le plafond de cognition est un jeu de
//! [`crate::Limits`] — donc exprimé dans les dimensions de §7.2, et un dépassement nomme la
//! dimension — indexé par la **classe** qu'on dépense et par ce que la dépense **paie**.
//!
//! C'est exactement le levier que l'ADR décrit : « frontière pour planifier, bon marché pour
//! exécuter » se pose en bornant serré `Frontier` et large `Economy`, sans qu'aucune constante de
//! code ne dise lequel des deux est cher.
//!
//! # La classe entre comme **type**, et c'est un choix qui se défend
//!
//! `packages/policy` indexe son affectation par **slug** (`W25.a`), et ce crate-ci prend le type. Ce
//! n'est pas une incohérence, c'est la conséquence de ce que chacun fait d'une clé inconnue :
//!
//! - la politique **répond** — une classe qu'elle ne connaît pas rend `None`, et l'appelant sait
//!   qu'il n'a pas de modèle ;
//! - ce crate **borne** — et la question « quelles clés sont couvertes ? » doit avoir une réponse
//!   complète, ce qu'un espace de chaînes ne permet pas d'énumérer.
//!
//! Un test balaie donc les `2 × 2` couples de [`CognitionClass::ALL`] × [`Spend::ALL`], ce qui n'a de
//! sens que parce que les deux ensembles sont finis et connus.
//!
//! # Non bornée n'est pas libre
//!
//! [`crate::Limits`] le pose déjà pour ses dimensions : « une dimension non nommée n'est pas
//! *libre*, elle est **hors budget** — rien ne peut y être réservé ». Le plafond de cognition hérite
//! de la règle : un couple sans bornes ne laisse rien passer.
//!
//! C'est ce qui rend une clé manquante inoffensive. L'inverse — « pas de plafond, donc illimité » —
//! ferait d'un oubli de configuration une autorisation de dépenser, et c'est la faute que ce dépôt
//! nomme partout : le silence lu comme un accord.
//!
//! # Une dépense non classée n'entre dans aucun plafond
//!
//! `W21.l` et `W21.m` l'ont posé et ce module ne le rejoue pas autrement : [`Classification::Unclassified`]
//! ne devient pas « travail » par défaut. Ici, elle ne s'impute nulle part — ni à un plafond de
//! coordination, ni à un plafond de travail, ni à un plafond « divers » qui n'existe pas.
//!
//! Ce n'est pas une permission : c'est un refus. Une dépense qu'on ne sait pas classer ne peut pas
//! être autorisée, puisqu'on ne sait pas contre quoi la compter.

use std::collections::BTreeMap;

use locus_domain::CognitionClass;

use crate::dimension::Dimension;
use crate::limits::Limits;
use crate::spend::{Classification, Spend};

/// Ce qu'une dépense engage : une classe de cognition, et ce qu'elle paie.
///
/// Les deux ensemble, jamais l'une sans l'autre — c'est la clé du plafond, et une moitié de clé ne
/// désigne rien.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Charge {
    /// La classe dépensée.
    pub class: CognitionClass,
    /// Ce que la dépense paie.
    pub spend: Spend,
}

impl Charge {
    /// Les quatre couples possibles, dans l'ordre.
    ///
    /// Finis et énumérables — c'est ce que le typage de la classe achète, et ce qui rend la question
    /// « quelles clés sont couvertes ? » décidable.
    #[must_use]
    pub fn all() -> Vec<Self> {
        CognitionClass::ALL
            .into_iter()
            .flat_map(|class| {
                Spend::ALL
                    .into_iter()
                    .map(move |spend| Self { class, spend })
            })
            .collect()
    }
}

/// Les plafonds de cognition d'un compte.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CognitionLimits {
    ceilings: BTreeMap<Charge, Limits>,
}

impl CognitionLimits {
    /// Aucun plafond posé — donc **rien d'autorisé**, pas « tout autorisé ».
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Poser le plafond d'un couple.
    ///
    /// Reposer le même couple **remplace** : un plafond est une décision de politique courante, et
    /// deux valeurs simultanées pour une même clé rendraient la borne dépendante de l'ordre.
    #[must_use]
    pub fn bounding(mut self, charge: Charge, limits: Limits) -> Self {
        self.ceilings.insert(charge, limits);
        self
    }

    /// Le plafond d'un couple, s'il en a un.
    ///
    /// `None` ne veut **pas** dire « illimité » : voir l'en-tête du module.
    #[must_use]
    pub fn ceiling(&self, charge: Charge) -> Option<&Limits> {
        self.ceilings.get(&charge)
    }

    /// Les couples bornés.
    pub fn charges(&self) -> impl Iterator<Item = Charge> + '_ {
        self.ceilings.keys().copied()
    }

    /// Ce montant peut-il être dépensé sur cette dimension ?
    ///
    /// # Les trois façons de refuser, et elles ne se confondent pas
    ///
    /// - la dépense n'est **pas classée** — elle ne s'impute nulle part, et `W21.l` dit pourquoi
    ///   elle ne devient pas « travail » par défaut ;
    /// - le couple n'a **aucun plafond** — hors budget, comme une dimension non nommée de
    ///   [`Limits`] ;
    /// - la dimension n'est **pas bornée** dans ce plafond, ou le montant la dépasse.
    ///
    /// Les rendre distinctes est ce qui permet à un exploitant de savoir s'il doit classer sa
    /// dépense, poser un plafond, ou en relever un.
    #[must_use]
    pub fn admits(
        &self,
        class: CognitionClass,
        classification: Classification,
        dimension: Dimension,
        amount: u64,
    ) -> Verdict {
        let Some(spend) = classification.spend() else {
            return Verdict::Unclassified;
        };
        let charge = Charge { class, spend };
        let Some(limits) = self.ceiling(charge) else {
            return Verdict::OutsideBudget { charge };
        };
        let Some(ceiling) = limits.ceiling(dimension) else {
            return Verdict::Unbounded { charge, dimension };
        };
        if amount > ceiling {
            return Verdict::Over {
                charge,
                dimension,
                ceiling,
                requested: amount,
            };
        }
        Verdict::Admitted { charge, dimension }
    }
}

/// Ce qu'un plafond de cognition répond.
///
/// Quatre issues, et **aucune n'est un booléen** : un exploitant qui lit « refusé » sans savoir
/// laquelle des trois raisons s'applique ne sait pas quoi corriger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Admis.
    Admitted {
        /// Sur quel couple.
        charge: Charge,
        /// Sur quelle dimension.
        dimension: Dimension,
    },
    /// La dépense n'est pas classée : elle ne s'impute à aucun plafond.
    Unclassified,
    /// Le couple n'a aucun plafond — hors budget.
    OutsideBudget {
        /// Lequel.
        charge: Charge,
    },
    /// Le couple a un plafond, mais pas sur cette dimension.
    Unbounded {
        /// Lequel.
        charge: Charge,
        /// Laquelle — **nommée**, c'est la clause 1.
        dimension: Dimension,
    },
    /// Le montant dépasse la borne.
    Over {
        /// Lequel.
        charge: Charge,
        /// Laquelle — **nommée**, c'est la clause 1.
        dimension: Dimension,
        /// Ce qui était permis.
        ceiling: u64,
        /// Ce qui était demandé.
        requested: u64,
    },
}

impl Verdict {
    /// Vrai seulement quand la dépense est admise.
    #[must_use]
    pub const fn is_admitted(self) -> bool {
        matches!(self, Self::Admitted { .. })
    }

    /// La dimension en cause, quand il y en a une.
    ///
    /// `Unclassified` et `OutsideBudget` n'en nomment aucune, et c'est exact : dans les deux cas, la
    /// dimension n'a pas encore été atteinte. Prétendre le contraire ferait chercher un plafond de
    /// dimension à qui n'a pas de plafond du tout.
    #[must_use]
    pub const fn dimension(self) -> Option<Dimension> {
        match self {
            Self::Admitted { dimension, .. }
            | Self::Unbounded { dimension, .. }
            | Self::Over { dimension, .. } => Some(dimension),
            Self::Unclassified | Self::OutsideBudget { .. } => None,
        }
    }
}
