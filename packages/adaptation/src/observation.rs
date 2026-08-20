//! Le producteur d'observations — `W18.g`, le capteur qui manquait.
//!
//! # Ce qui manquait, exactement
//!
//! Les onze `Trigger` de §14.5 existaient, et **rien ne disait quand ils se déclenchent**. Un agent
//! affirmait « il y a une lacune de domaine » et le système le croyait. Une observation est ce qui
//! remplace l'affirmation par une mesure : une valeur, recalculable depuis le journal, qui **cite**
//! ce dont elle est tirée.
//!
//! # Le capteur mesure, la politique décide — et rien ne franchit cette frontière
//!
//! **Aucun chemin de type ne mène d'une [`Observation`] à un [`crate::Trigger`].** Pas de `From`,
//! pas de méthode, pas de fonction. La correspondance vit dans une politique, versionnée, avec ses
//! seuils — et c'est là qu'elle doit vivre, parce qu'un seuil est une décision : « à partir de
//! combien de conflits ouverts faut-il agir » n'a pas de réponse dans les données.
//!
//! Un capteur qui porterait un seuil trancherait cette question en silence, et le changer
//! demanderait de recompiler au lieu de commiter une politique. C'est pourquoi il n'existe **aucun
//! champ** où un seuil pourrait s'écrire, et pourquoi un test le tient par l'absence.
//!
//! # Une source muette rend une observation **absente**
//!
//! `None`, jamais `Some(0.0)`. « Aucun conflit ouvert » et « la source des conflits n'a pas
//! répondu » sont deux états qu'une politique traite différemment, et les fondre ferait lire un
//! silence comme une bonne nouvelle. C'est la règle du dépôt, la même que `unverified` contre
//! `broken` et que `None` contre `Some(0.0)` pour une couverture de reçu.
//!
//! # Le nom
//!
//! `Observation` et non `Signal` : `memory::retrieval::Signal` existe et désigne un facteur de
//! classement. Deux `Signal` dans un `use` seraient renommés à l'import par chaque appelant.

use std::fmt;

use locus_domain::RevisionId;

/// Ce qu'une observation mesure — six sources, toutes déjà construites ailleurs.
///
/// La liste est close, et chaque entrée nomme **où la donnée existe déjà**. Une septième n'entrera
/// que lorsqu'une source exécutable et testée existera : une sorte d'observation sans source est
/// une mesure que personne ne peut prendre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObservationKind {
    /// Combien de conflits sont ouverts — `packages/graph`, §9.4.
    OpenConflicts,
    /// Où en est la propagation de validation — `packages/validation`, §8.1.
    ValidationDepth,
    /// Un indicateur de portefeuille — `packages/portfolio`, §13.2.
    PortfolioIndicator,
    /// Le taux de désaccord entre relecteurs — `packages/review`, §20.
    ReviewDisagreement,
    /// Le taux d'échec de reproduction — le `ReproductionWorkflow` de §11.2.
    ReproductionFailure,
    /// La part de ce qu'un retrieval a **exclu** — le reçu de `W17.n`, §16.
    ///
    /// C'est celle qui n'existait pas avant `W17.n` : sans reçu, une lacune de domaine était
    /// affirmée par un agent et non lue. C'est aussi la seule qui rende `DomainGapDetected`
    /// auditable, et c'est pourquoi l'ADR 0022 décision 6 la nomme.
    DomainGap,
}

impl ObservationKind {
    /// Les six, dans l'ordre où leurs sources ont été construites.
    pub const ALL: [Self; 6] = [
        Self::OpenConflicts,
        Self::ValidationDepth,
        Self::PortfolioIndicator,
        Self::ReviewDisagreement,
        Self::ReproductionFailure,
        Self::DomainGap,
    ];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::OpenConflicts => "open-conflicts",
            Self::ValidationDepth => "validation-depth",
            Self::PortfolioIndicator => "portfolio-indicator",
            Self::ReviewDisagreement => "review-disagreement",
            Self::ReproductionFailure => "reproduction-failure",
            Self::DomainGap => "domain-gap",
        }
    }

    /// La relire.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.slug() == value)
    }
}

impl fmt::Display for ObservationKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Une mesure prise sur le journal, à un préfixe donné.
///
/// # Ce qu'elle porte, et ce qu'elle ne porte pas
///
/// Elle porte une sorte, une valeur, les révisions dont elle est tirée, et le **watermark** du
/// préfixe sur lequel elle a été calculée. Elle ne porte **aucun seuil**, aucun verdict, aucune
/// suite à donner : ce sont des décisions, et elles vivent dans une politique.
#[derive(Debug, Clone, PartialEq)]
pub struct Observation {
    kind: ObservationKind,
    value: f64,
    cites: Vec<RevisionId>,
    watermark: u64,
}

impl Observation {
    /// Prendre une mesure.
    ///
    /// # Errors
    ///
    /// [`ObservationError::Uncited`] quand aucune révision n'est citée — une observation dont on ne
    /// sait pas d'où elle vient ne se recalcule pas, donc ne se conteste pas, donc n'est qu'une
    /// affirmation de plus ; [`ObservationError::NotFinite`] pour une valeur qui n'est pas un
    /// nombre, parce qu'une politique comparerait alors une chose à une non-chose.
    pub fn measured(
        kind: ObservationKind,
        value: f64,
        cites: Vec<RevisionId>,
        watermark: u64,
    ) -> Result<Self, ObservationError> {
        if cites.is_empty() {
            return Err(ObservationError::Uncited { kind });
        }
        if !value.is_finite() {
            return Err(ObservationError::NotFinite { kind, value });
        }
        Ok(Self {
            kind,
            value,
            cites,
            watermark,
        })
    }

    /// Ce qu'elle mesure.
    #[must_use]
    pub const fn kind(&self) -> ObservationKind {
        self.kind
    }

    /// Sa valeur.
    #[must_use]
    pub const fn value(&self) -> f64 {
        self.value
    }

    /// Les révisions dont elle est tirée — **jamais vide**.
    #[must_use]
    pub fn cites(&self) -> &[RevisionId] {
        &self.cites
    }

    /// Le préfixe du journal sur lequel elle a été calculée.
    ///
    /// Deux observations de même sorte et de watermarks différents ne se comparent pas : elles
    /// mesurent deux mondes.
    #[must_use]
    pub const fn watermark(&self) -> u64 {
        self.watermark
    }
}

/// Ce qui prend une mesure — un port, fourni par l'appelant.
///
/// # Pourquoi un port, et pourquoi il rend une option
///
/// Les six sources vivent dans six crates que `packages/adaptation` n'a aucune raison de connaître
/// toutes. Chacune sait lire son propre journal ; ce module sait ce qu'une observation **est**, et
/// ce qu'elle n'a pas le droit de porter.
///
/// `None` dit que la source n'a rien à dire — pas qu'elle dit zéro. Un capteur qui rendrait
/// `Some(0.0)` faute de réponse ferait lire un silence comme une mesure, et une politique agirait
/// sur une donnée que personne n'a prise.
pub trait Sensor {
    /// Ce que ce capteur mesure.
    fn kind(&self) -> ObservationKind;

    /// La mesure au préfixe donné, ou **rien** si la source est muette.
    fn observe(&self, watermark: u64) -> Option<Observation>;
}

/// Prendre toutes les mesures disponibles à ce préfixe.
///
/// Les sources muettes sont **absentes** du résultat, et non présentes à zéro. Le compte rendu est
/// donc plus court quand une source ne répond pas, ce qui est l'information qu'on veut : une
/// politique qui reçoit cinq observations sur six sait qu'il lui en manque une.
pub fn observe_all(sensors: &[&dyn Sensor], watermark: u64) -> Vec<Observation> {
    sensors
        .iter()
        .filter_map(|sensor| sensor.observe(watermark))
        .collect()
}

/// Pourquoi une observation ne se prend pas.
#[derive(Debug, Clone, PartialEq)]
pub enum ObservationError {
    /// Aucune révision citée.
    Uncited {
        /// Ce qu'on voulait mesurer.
        kind: ObservationKind,
    },
    /// Une valeur qui n'est pas un nombre.
    NotFinite {
        /// Ce qu'on voulait mesurer.
        kind: ObservationKind,
        /// Ce qui a été donné.
        value: f64,
    },
}

impl fmt::Display for ObservationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Uncited { kind } => write!(
                formatter,
                "une observation « {kind} » sans révision citée ne se recalcule pas, donc ne se \
                 conteste pas, donc n'est qu'une affirmation de plus"
            ),
            Self::NotFinite { kind, value } => write!(
                formatter,
                "une observation « {kind} » de valeur {value} : une politique comparerait une chose \
                 à une non-chose"
            ),
        }
    }
}

impl std::error::Error for ObservationError {}
