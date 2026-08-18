//! La boucle rapide — celle qui change la **capacité**, jamais la structure.
//!
//! `docs/10_V1_ROADMAP.md`, W18 : « boucle rapide sur la capacité — routage de modèle, choix
//! d'outil, sélection de skill, retry, routes éphémères ; boucle lente sur la structure. »
//!
//! # Ce que ce module ne nomme pas
//!
//! Ni `Operation`, ni `Change`, ni `Relation`, ni `Version`, ni `Proposal`. Un test lit le source
//! et le vérifie. Ce n'est pas de la pédanterie : la boucle rapide s'exécute sans approbation, à la
//! latence d'un appel de modèle, et une seule fonction qui rendrait une opération de coordination
//! ferait d'elle un chemin de mutation du graphe sans décision, sans trace et sans révision de base.
//! Les deux boucles ont la même forme dans l'esprit de qui les écrit — « ajuster quelque chose » —
//! et rien d'autre que l'absence de vocabulaire ne les tient séparées.
//!
//! # Tout expire, pas seulement les routes
//!
//! La roadmap qualifie d'« éphémères » les seules routes. Elles ne sont pourtant pas le seul
//! ajustement qui deviendrait une structure en durant : un routage de modèle permanent est une
//! spécialisation d'agent, une sélection de skill permanente est un rôle. Ici **chaque** adaptation
//! porte sa fenêtre, et il n'existe pas d'adaptation sans fin. C'est ce qui rend vraie la phrase que
//! l'item demande : deux adaptations rapides ne s'accumulent jamais en une structure que personne
//! n'a approuvée, parce qu'aucune n'est là pour l'accumulation suivante.
//!
//! # La fenêtre est semi-ouverte
//!
//! `[from, until)`. Une borne haute incluse ferait se chevaucher deux fenêtres consécutives sur
//! exactement un instant — et à cet instant-là, deux routages de modèle seraient vivants pour le
//! même agent. Un défaut d'une milliseconde par transition est celui qu'on ne reproduit jamais.

use std::collections::BTreeSet;
use std::fmt;

use locus_protocol::{Id, Timestamp, id::Agent};

/// Ce qu'une adaptation rapide ajuste — les cinq de la roadmap.
///
/// Aucune ne touche à l'appartenance, à la topologie ni au mode d'une équipe : ce sont les objets de
/// la boucle lente, et ils passent par une proposition.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Adjustment {
    /// Router vers un autre modèle.
    ModelRouting {
        /// Lequel.
        model: String,
    },
    /// Rendre un outil disponible.
    ToolChoice {
        /// Lequel.
        tool: String,
    },
    /// Rendre un skill disponible.
    SkillSelection {
        /// Lequel.
        skill: String,
    },
    /// Réessayer, au plus tant de fois.
    Retry {
        /// Combien. Zéro veut dire « ne pas réessayer », ce qui est une décision et non une absence.
        attempts: u8,
    },
    /// Ouvrir une route éphémère vers un autre agent.
    ///
    /// C'est l'ajustement qui ressemble le plus à une arête, et c'est pourquoi il est ici plutôt
    /// que dans la boucle lente : une route qui durerait **serait** une arête, et devrait alors se
    /// proposer, s'approuver et se commiter. Sa fenêtre est ce qui l'en distingue.
    EphemeralRoute {
        /// Vers qui.
        to: Id<Agent>,
    },
}

impl Adjustment {
    /// Les cinq sortes, sous le nom de la roadmap.
    pub const KINDS: [&'static str; 5] = [
        "model_routing",
        "tool_choice",
        "skill_selection",
        "retry",
        "ephemeral_route",
    ];

    /// Sa sorte.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::ModelRouting { .. } => "model_routing",
            Self::ToolChoice { .. } => "tool_choice",
            Self::SkillSelection { .. } => "skill_selection",
            Self::Retry { .. } => "retry",
            Self::EphemeralRoute { .. } => "ephemeral_route",
        }
    }

    /// Vrai quand deux ajustements de cette sorte ne peuvent pas être vivants ensemble.
    ///
    /// Un agent a **un** modèle et **un** budget de réessai : deux valeurs simultanées ne se
    /// départagent pas, et laisser la plus récente gagner ferait dépendre le comportement de l'ordre
    /// d'adoption, que personne ne relit. Un outil, un skill et une route sont au contraire additifs
    /// — en ouvrir un second n'invalide pas le premier.
    #[must_use]
    pub const fn is_exclusive(&self) -> bool {
        matches!(self, Self::ModelRouting { .. } | Self::Retry { .. })
    }

    fn named_field(&self) -> Option<(&'static str, &str)> {
        match self {
            Self::ModelRouting { model } => Some(("model", model)),
            Self::ToolChoice { tool } => Some(("tool", tool)),
            Self::SkillSelection { skill } => Some(("skill", skill)),
            Self::Retry { .. } | Self::EphemeralRoute { .. } => None,
        }
    }
}

impl fmt::Display for Adjustment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.kind())
    }
}

/// Un ajustement porté par un agent, sur une fenêtre bornée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adaptation {
    subject: Id<Agent>,
    adjustment: Adjustment,
    from: Timestamp,
    until: Timestamp,
}

impl Adaptation {
    /// Adopter un ajustement pour une durée bornée.
    ///
    /// # Errors
    ///
    /// [`FastError::EmptyWindow`] quand `until` n'est pas strictement après `from`. Il n'existe pas
    /// d'adaptation sans fin, et une fenêtre vide n'est pas une fenêtre infinie — c'est un
    /// ajustement que rien n'appliquerait jamais, adopté par erreur.
    ///
    /// [`FastError::EmptyName`] pour un modèle, un outil ou un skill sans nom.
    pub fn lasting(
        subject: Id<Agent>,
        adjustment: Adjustment,
        from: Timestamp,
        until: Timestamp,
    ) -> Result<Self, FastError> {
        if until <= from {
            return Err(FastError::EmptyWindow { from, until });
        }
        if let Some((field, value)) = adjustment.named_field()
            && value.trim().is_empty()
        {
            return Err(FastError::EmptyName { field });
        }
        Ok(Self {
            subject,
            adjustment,
            from,
            until,
        })
    }

    /// L'agent ajusté.
    #[must_use]
    pub const fn subject(&self) -> Id<Agent> {
        self.subject
    }

    /// L'ajustement.
    #[must_use]
    pub const fn adjustment(&self) -> &Adjustment {
        &self.adjustment
    }

    /// Le début de la fenêtre, inclus.
    #[must_use]
    pub const fn from(&self) -> Timestamp {
        self.from
    }

    /// La fin de la fenêtre, **exclue**.
    #[must_use]
    pub const fn until(&self) -> Timestamp {
        self.until
    }

    /// Vrai quand cette adaptation est vivante à cet instant.
    #[must_use]
    pub fn covers(&self, instant: Timestamp) -> bool {
        self.from <= instant && instant < self.until
    }

    fn overlaps(&self, other: &Self) -> bool {
        self.from < other.until && other.from < self.until
    }
}

/// L'état de la boucle rapide : ce qui est adopté, et quand.
///
/// Aucune méthode ne rend un objet durable. Ce qui se lit d'un `Fast` se lit **à un instant**, et le
/// même `Fast` interrogé plus tard ne rend rien.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fast {
    adaptations: Vec<Adaptation>,
}

impl Fast {
    /// Aucune adaptation.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adopter une adaptation de plus.
    ///
    /// # Errors
    ///
    /// [`FastError::Overlapping`] quand une adaptation exclusive de la même sorte couvre déjà une
    /// partie de la fenêtre pour le même agent. Le conflit est refusé, pas arbitré : le résoudre en
    /// silence — la dernière adoptée gagne, la plus longue gagne — ferait dépendre le modèle qui
    /// répond de l'ordre dans lequel deux ajustements sont arrivés.
    pub fn adopting(mut self, adaptation: Adaptation) -> Result<Self, FastError> {
        if adaptation.adjustment.is_exclusive()
            && let Some(existing) = self.adaptations.iter().find(|held| {
                held.subject == adaptation.subject
                    && held.adjustment.kind() == adaptation.adjustment.kind()
                    && held.overlaps(&adaptation)
            })
        {
            return Err(FastError::Overlapping {
                kind: adaptation.adjustment.kind(),
                from: existing.from,
                until: existing.until,
            });
        }
        self.adaptations.push(adaptation);
        Ok(self)
    }

    /// Les adaptations vivantes à cet instant.
    pub fn live_at(&self, instant: Timestamp) -> impl Iterator<Item = &Adaptation> {
        self.adaptations
            .iter()
            .filter(move |adaptation| adaptation.covers(instant))
    }

    /// Le modèle vers lequel cet agent est routé à cet instant, s'il l'est.
    ///
    /// `None` ne veut pas dire « le modèle par défaut » : ce module ne connaît aucun défaut. Il veut
    /// dire qu'aucun routage n'est vivant, et c'est à l'appelant de savoir ce qu'il fait sans.
    #[must_use]
    pub fn model_for(&self, subject: Id<Agent>, instant: Timestamp) -> Option<&str> {
        self.live_at(instant)
            .filter(|adaptation| adaptation.subject == subject)
            .find_map(|adaptation| match &adaptation.adjustment {
                Adjustment::ModelRouting { model } => Some(model.as_str()),
                _ => None,
            })
    }

    /// Les agents vers lesquels une route éphémère est ouverte depuis `subject` à cet instant.
    #[must_use]
    pub fn routes_from(&self, subject: Id<Agent>, instant: Timestamp) -> BTreeSet<Id<Agent>> {
        self.live_at(instant)
            .filter(|adaptation| adaptation.subject == subject)
            .filter_map(|adaptation| match adaptation.adjustment {
                Adjustment::EphemeralRoute { to } => Some(to),
                _ => None,
            })
            .collect()
    }

    /// Les outils disponibles pour cet agent à cet instant.
    #[must_use]
    pub fn tools_for(&self, subject: Id<Agent>, instant: Timestamp) -> BTreeSet<&str> {
        self.live_at(instant)
            .filter(|adaptation| adaptation.subject == subject)
            .filter_map(|adaptation| match &adaptation.adjustment {
                Adjustment::ToolChoice { tool } => Some(tool.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Les skills disponibles pour cet agent à cet instant.
    #[must_use]
    pub fn skills_for(&self, subject: Id<Agent>, instant: Timestamp) -> BTreeSet<&str> {
        self.live_at(instant)
            .filter(|adaptation| adaptation.subject == subject)
            .filter_map(|adaptation| match &adaptation.adjustment {
                Adjustment::SkillSelection { skill } => Some(skill.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Le nombre d'adaptations adoptées, vivantes ou non.
    #[must_use]
    pub fn len(&self) -> usize {
        self.adaptations.len()
    }

    /// Vrai quand rien n'a été adopté.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.adaptations.is_empty()
    }
}

/// Ce qui empêche une adaptation rapide d'exister ou d'être adoptée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FastError {
    /// Une fenêtre vide ou renversée.
    EmptyWindow {
        /// Le début demandé.
        from: Timestamp,
        /// La fin demandée.
        until: Timestamp,
    },
    /// Un modèle, un outil ou un skill sans nom.
    EmptyName {
        /// Lequel.
        field: &'static str,
    },
    /// Deux ajustements exclusifs de la même sorte se chevauchent pour le même agent.
    Overlapping {
        /// La sorte.
        kind: &'static str,
        /// Le début de celui déjà tenu.
        from: Timestamp,
        /// Sa fin.
        until: Timestamp,
    },
}

impl fmt::Display for FastError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyWindow { from, until } => write!(
                formatter,
                "la fenêtre {from}..{until} est vide : une adaptation rapide dure, ou n'existe pas"
            ),
            Self::EmptyName { field } => {
                write!(formatter, "`{field}` est sans nom, donc sans destinataire")
            }
            Self::Overlapping { kind, from, until } => write!(
                formatter,
                "un ajustement `{kind}` couvre déjà {from}..{until} pour cet agent, et deux ne se départagent pas"
            ),
        }
    }
}

impl std::error::Error for FastError {}
