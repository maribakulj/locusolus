//! Le cockpit à quatre vues — `docs/10` W17, `docs/SPEC_V1.md` §23.
//!
//! # Quatre vues, une seule sélection
//!
//! Plan, vivant, trace, épistémique. `docs/10` demande une « sélection synchronisée par
//! `Id<Agent>` », et la façon de la tenir décide de tout : **il n'y a pas quatre sélections qu'on
//! synchronise, il y en a une que quatre vues lisent.**
//!
//! La différence n'est pas de style. Quatre états plus un mécanisme de synchronisation dérivent dès
//! qu'un chemin oublie de notifier — et la dérive est silencieuse, puisque chaque vue reste
//! cohérente avec elle-même. Un opérateur lirait la trace d'un agent en croyant lire celle qu'il a
//! sélectionnée dans le plan. Ici, [`Cockpit`] ne détient qu'un champ : il n'existe aucun chemin par
//! lequel deux vues divergent, parce qu'il n'y a rien à faire diverger.
//!
//! # Le canvas produit une commande, jamais une écriture
//!
//! `docs/10` : « le canvas produit une commande, jamais une écriture ». [`Requested`] est donc une
//! **demande** : elle nomme un verbe et un sujet, et n'expose rien qui l'applique. Ce n'est pas une
//! discipline d'appel — il n'y a pas de méthode à ne pas appeler, comme pour la `Simulation` de
//! `packages/policy` et l'`Acceptance` de `packages/coordination`.
//!
//! Le verbe reste opaque ici, et c'est délibéré : ce que les verbes signifient appartient à la
//! command API (§22), pas au canvas. Les énumérer dans la vue ferait du cockpit l'endroit où l'on
//! décide de ce qui peut être demandé, alors que c'est l'endroit où l'on demande.

use std::fmt;

use locus_protocol::{Id, id::Agent};

/// Les quatre vues du cockpit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Pane {
    /// Ce qui est prévu.
    Plan,
    /// Ce qui tourne.
    Live,
    /// Ce qui s'est passé.
    Trace,
    /// Ce qui est cru, et pourquoi.
    Epistemic,
}

impl Pane {
    /// Les quatre, dans l'ordre de `docs/10`.
    pub const ALL: [Self; 4] = [Self::Plan, Self::Live, Self::Trace, Self::Epistemic];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::Live => "live",
            Self::Trace => "trace",
            Self::Epistemic => "epistemic",
        }
    }
}

impl fmt::Display for Pane {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Ce qui est sélectionné, et depuis où.
///
/// L'origine est conservée pour le journal — savoir qu'une sélection vient de la trace plutôt que
/// du plan aide à relire une session — mais elle **ne change pas** ce que les autres vues montrent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    agent: Id<Agent>,
    origin: Pane,
}

impl Selection {
    /// L'agent désigné.
    #[must_use]
    pub const fn agent(self) -> Id<Agent> {
        self.agent
    }

    /// La vue depuis laquelle il a été désigné.
    #[must_use]
    pub const fn origin(self) -> Pane {
        self.origin
    }
}

/// Le cockpit : quatre vues qui lisent **une** sélection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cockpit {
    selected: Option<Selection>,
}

impl Cockpit {
    /// Un cockpit sans sélection.
    #[must_use]
    pub const fn new() -> Self {
        Self { selected: None }
    }

    /// Désigner un agent depuis une vue.
    ///
    /// Les trois autres suivent — non pas parce qu'on les notifie, mais parce qu'elles lisent le
    /// même champ.
    pub const fn select(&mut self, from: Pane, agent: Id<Agent>) -> Selection {
        let selection = Selection {
            agent,
            origin: from,
        };
        self.selected = Some(selection);
        selection
    }

    /// Ce que cette vue montre comme sélectionné.
    ///
    /// La même chose dans les quatre. Le paramètre existe pour que l'appelant écrive ce qu'il
    /// demande, pas parce que la réponse en dépend.
    #[must_use]
    pub const fn selection_in(&self, _pane: Pane) -> Option<Selection> {
        self.selected
    }

    /// Oublier la sélection.
    pub const fn clear(&mut self) {
        self.selected = None;
    }
}

/// Ce qu'un geste de canvas produit : une **demande**.
///
/// Elle n'expose rien qui l'applique. Un geste qui écrirait ferait du canvas un chemin de mutation
/// parallèle à la command API, sans approbation, sans trace et sans `expected_revision`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requested {
    verb: String,
    subject: Id<Agent>,
    from: Pane,
}

impl Requested {
    /// Ce qui est demandé.
    #[must_use]
    pub fn verb(&self) -> &str {
        &self.verb
    }

    /// Sur qui.
    #[must_use]
    pub const fn subject(&self) -> Id<Agent> {
        self.subject
    }

    /// Depuis quelle vue.
    #[must_use]
    pub const fn origin(&self) -> Pane {
        self.from
    }
}

/// Traduire un geste de canvas en demande.
///
/// # Errors
///
/// [`CockpitError::EmptyVerb`] pour un geste qui ne demande rien : il traverserait la command API
/// pour y être refusé, en ayant occupé un journal au passage.
pub fn gesture(from: Pane, verb: &str, subject: Id<Agent>) -> Result<Requested, CockpitError> {
    if verb.trim().is_empty() {
        return Err(CockpitError::EmptyVerb);
    }
    Ok(Requested {
        verb: verb.to_owned(),
        subject,
        from,
    })
}

/// Ce qui empêche un geste de devenir une demande.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CockpitError {
    /// Un geste qui ne demande rien.
    EmptyVerb,
}

impl fmt::Display for CockpitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyVerb => formatter.write_str(
                "un geste qui ne demande rien traverserait la command API pour y être refusé, en \
                 ayant occupé un journal au passage",
            ),
        }
    }
}

impl std::error::Error for CockpitError {}
