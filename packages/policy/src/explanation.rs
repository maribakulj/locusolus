//! L'explicabilité d'une décision automatisée — `docs/SPEC_V1.md` §20.5.
//!
//! # Les huit facettes, et celle qu'on omet toujours
//!
//! « Toute décision automatisée expose : politique et version ; données d'entrée ; règles
//! déclenchées ; scores et incertitudes ; **alternatives rejetées** ; approbations ; overrides ;
//! lien avec les événements produits. »
//!
//! Sept de ces huit se remplissent naturellement en construisant la décision. La huitième — les
//! alternatives rejetées — est la seule qu'il faut décider de garder, parce qu'elle n'existe nulle
//! part une fois la décision prise. C'est aussi celle qui rend une décision contestable : savoir
//! qu'un moteur a choisi A ne dit rien tant qu'on ignore s'il a même envisagé B.
//!
//! # Une alternative rejetée sans motif n'en est pas une
//!
//! « Nous avons envisagé B » sans dire pourquoi B a été écarté ne se conteste pas : il n'y a rien à
//! objecter. C'est une case cochée, et une case cochée dans un rapport d'explicabilité est pire que
//! son absence, parce qu'elle donne l'apparence d'avoir été remplie.
//!
//! # Un override reste visible
//!
//! §20.2 exige de « conserver les overrides humains ». Conserver veut dire que la décision
//! automatique **reste lisible à côté** : un override qui remplacerait la décision effacerait ce
//! que le moteur avait conclu, et personne ne pourrait plus dire si l'humain a corrigé une erreur
//! ou contourné une garde.

use std::fmt;

use crate::{Evaluation, Facts, Outcome};

/// Une piste que le moteur a écartée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejected {
    option: String,
    because: String,
}

impl Rejected {
    /// Consigner une alternative écartée.
    ///
    /// # Errors
    ///
    /// [`ExplanationError::EmptyField`] pour une option ou un motif vide. « Nous avons envisagé
    /// B » sans dire pourquoi ne se conteste pas : il n'y a rien à objecter, et la case cochée
    /// donne l'apparence d'un examen qui n'a pas eu lieu.
    pub fn considered(option: &str, because: &str) -> Result<Self, ExplanationError> {
        if option.trim().is_empty() {
            return Err(ExplanationError::EmptyField {
                field: "rejected.option",
            });
        }
        if because.trim().is_empty() {
            return Err(ExplanationError::EmptyField {
                field: "rejected.because",
            });
        }
        Ok(Self {
            option: option.to_owned(),
            because: because.to_owned(),
        })
    }

    /// Ce qui a été écarté.
    #[must_use]
    pub fn option(&self) -> &str {
        &self.option
    }

    /// Pourquoi.
    #[must_use]
    pub fn because(&self) -> &str {
        &self.because
    }
}

/// Ce qu'un humain a décidé par-dessus le moteur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Override {
    by: String,
    outcome: Outcome,
    because: String,
}

impl Override {
    /// Consigner un override.
    ///
    /// # Errors
    ///
    /// [`ExplanationError::EmptyField`] pour un auteur ou un motif vide : un override anonyme ou
    /// muet est indiscernable d'un défaut du moteur, et c'est précisément ce qu'il ne faut pas
    /// confondre.
    pub fn recorded(by: &str, outcome: Outcome, because: &str) -> Result<Self, ExplanationError> {
        if by.trim().is_empty() {
            return Err(ExplanationError::EmptyField {
                field: "override.by",
            });
        }
        if because.trim().is_empty() {
            return Err(ExplanationError::EmptyField {
                field: "override.because",
            });
        }
        Ok(Self {
            by: by.to_owned(),
            outcome,
            because: because.to_owned(),
        })
    }

    /// Qui.
    #[must_use]
    pub fn by(&self) -> &str {
        &self.by
    }

    /// Ce qui s'applique désormais.
    #[must_use]
    pub const fn outcome(&self) -> &Outcome {
        &self.outcome
    }

    /// Pourquoi.
    #[must_use]
    pub fn because(&self) -> &str {
        &self.because
    }
}

/// L'exposé d'une décision automatisée — les huit facettes de §20.5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Explanation {
    facts: Facts,
    evaluation: Evaluation,
    rejected: Vec<Rejected>,
    approvals: Vec<String>,
    overridden: Option<Override>,
    events: Vec<String>,
}

impl Explanation {
    /// Exposer une évaluation.
    #[must_use]
    pub fn of(facts: &Facts, evaluation: &Evaluation) -> Self {
        Self {
            facts: facts.clone(),
            evaluation: evaluation.clone(),
            rejected: Vec::new(),
            approvals: Vec::new(),
            overridden: None,
            events: Vec::new(),
        }
    }

    /// Consigner une alternative écartée.
    #[must_use]
    pub fn rejecting(mut self, rejected: Rejected) -> Self {
        self.rejected.push(rejected);
        self
    }

    /// Consigner une approbation.
    #[must_use]
    pub fn approved_by(mut self, approver: &str) -> Self {
        self.approvals.push(approver.to_owned());
        self
    }

    /// Consigner un override humain.
    ///
    /// La décision automatique **reste** : `outcome` continue de rendre ce que le moteur avait
    /// conclu, et [`Explanation::effective_outcome`] dit ce qui s'applique. Les fondre effacerait ce
    /// que le moteur avait dit, et personne ne pourrait plus distinguer une erreur corrigée d'une
    /// garde contournée.
    #[must_use]
    pub fn overridden_by(mut self, overridden: Override) -> Self {
        self.overridden = Some(overridden);
        self
    }

    /// Relier la décision aux événements qu'elle a produits.
    #[must_use]
    pub fn producing(mut self, event: &str) -> Self {
        self.events.push(event.to_owned());
        self
    }

    /// Les données d'entrée.
    #[must_use]
    pub const fn facts(&self) -> &Facts {
        &self.facts
    }

    /// L'évaluation — politique, versions, règles déclenchées.
    #[must_use]
    pub const fn evaluation(&self) -> &Evaluation {
        &self.evaluation
    }

    /// Ce que le moteur avait conclu, override ou pas.
    #[must_use]
    pub const fn machine_outcome(&self) -> &Outcome {
        self.evaluation.outcome()
    }

    /// Ce qui s'applique réellement.
    #[must_use]
    pub fn effective_outcome(&self) -> &Outcome {
        self.overridden
            .as_ref()
            .map_or_else(|| self.evaluation.outcome(), Override::outcome)
    }

    /// Les alternatives écartées.
    #[must_use]
    pub fn rejected(&self) -> &[Rejected] {
        &self.rejected
    }

    /// Les approbations.
    #[must_use]
    pub fn approvals(&self) -> &[String] {
        &self.approvals
    }

    /// L'override, s'il y en a un.
    #[must_use]
    pub const fn overridden(&self) -> Option<&Override> {
        self.overridden.as_ref()
    }

    /// Les événements produits.
    #[must_use]
    pub fn events(&self) -> &[String] {
        &self.events
    }

    /// Ce que §20.5 exige et que cet exposé ne porte pas.
    ///
    /// Une facette vide n'est pas toujours un manquement — une décision sans override n'a pas
    /// d'override à montrer. Deux le sont toujours : une décision sans **données d'entrée** ne se
    /// rejoue pas, et une décision sans **règle déclenchée** ne s'explique par rien. Les distinguer
    /// évite de crier au manquement sur des exposés complets, ce qui apprend à ignorer l'alarme.
    #[must_use]
    pub fn gaps(&self) -> Vec<Facet> {
        let mut gaps = Vec::new();
        if self.facts.entries().is_empty() {
            gaps.push(Facet::Inputs);
        }
        if self.evaluation.trace().is_empty() {
            gaps.push(Facet::FiredRules);
        }
        gaps
    }
}

/// Les facettes que §20.5 énumère.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Facet {
    /// Politique et version.
    PolicyAndVersion,
    /// Données d'entrée.
    Inputs,
    /// Règles déclenchées.
    FiredRules,
    /// Scores et incertitudes.
    ScoresAndUncertainty,
    /// Alternatives rejetées.
    RejectedAlternatives,
    /// Approbations.
    Approvals,
    /// Overrides.
    Overrides,
    /// Lien avec les événements produits.
    ProducedEvents,
}

impl Facet {
    /// Les huit, dans l'ordre de §20.5.
    pub const ALL: [Self; 8] = [
        Self::PolicyAndVersion,
        Self::Inputs,
        Self::FiredRules,
        Self::ScoresAndUncertainty,
        Self::RejectedAlternatives,
        Self::Approvals,
        Self::Overrides,
        Self::ProducedEvents,
    ];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::PolicyAndVersion => "policy-and-version",
            Self::Inputs => "inputs",
            Self::FiredRules => "fired-rules",
            Self::ScoresAndUncertainty => "scores-and-uncertainty",
            Self::RejectedAlternatives => "rejected-alternatives",
            Self::Approvals => "approvals",
            Self::Overrides => "overrides",
            Self::ProducedEvents => "produced-events",
        }
    }
}

impl fmt::Display for Facet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Ce qui empêche un exposé d'être consigné.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExplanationError {
    /// Un champ obligatoire vide.
    EmptyField {
        /// Lequel.
        field: &'static str,
    },
}

impl fmt::Display for ExplanationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(
                formatter,
                "« {field} » est vide : une case cochée dans un rapport d'explicabilité est pire \
                 que son absence, parce qu'elle donne l'apparence d'avoir été remplie"
            ),
        }
    }
}

impl std::error::Error for ExplanationError {}
