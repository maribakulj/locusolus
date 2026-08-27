//! Le cgroup que le déploiement délègue au broker — `W5.ai`, ADR 0036 décision 3.
//!
//! # Pourquoi ce module existe
//!
//! La campagne de `W5.af.3` rend `NotTrusted` à `S2` en bloquant sur trois sondes de quota, et les
//! trois sont `NotRun` : elles n'ont **rien lu**. `bubblewrap` n'écrit aucun cgroup — c'est hors de
//! son objet —, donc les bornes de ressources doivent être posées **autour** de `bwrap`, par qui le
//! lance. L'ADR 0004 dit qui : `locus-execd`, le service privilégié.
//!
//! # Ce que ce module fait, et ce qu'il ne fait pas encore
//!
//! Il **lit** ce que l'hôte délègue, et il **refuse** quand rien ne l'est. Poser effectivement le
//! cgroup et y déplacer le processus est la tranche suivante ; ce module en est la condition, et il
//! est complet pour ce qu'il annonce.
//!
//! Ce découpage n'est pas de commodité. L'ADR 0036 décision 1 relève que sur les trois façons dont
//! un confinement à cgroup peut échouer, **deux sont silencieuses** — la sandbox tourne, simplement
//! sans borne. Le refus est donc la moitié qui porte la garantie, et c'est aussi la seule que le
//! conteneur de développement de ce chantier puisse éprouver pour de vrai : il ne délègue rien.
//!
//! # La lecture qui compte n'est pas celle de la racine
//!
//! `probe.rs` l'établit déjà et le dit : « `cgroup.subtree_control` d'un parent décide de ce que ses
//! enfants voient. On lit donc le `cgroup.controllers` de **notre propre** répertoire, qui est la
//! seule liste que nous pourrons effectivement écrire. » Ce module ne refait pas cette lecture — il
//! s'appuie sur [`HostFacts`], parce que deux sources pour un même fait finissent par diverger.

use std::collections::BTreeSet;
use std::fmt;

use super::probe::{HostFacts, REQUIRED_CONTROLLERS};

/// Ce que le broker peut poser autour d'une sandbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delegation {
    controllers: BTreeSet<String>,
}

/// Pourquoi le broker ne peut pas poser de borne.
///
/// # Deux refus, parce qu'il y a deux causes
///
/// Elles ne se réparent pas au même endroit : l'une envoie monter une hiérarchie unifiée, l'autre
/// envoie **déléguer** des contrôleurs à ce processus-ci. Les confondre enverrait chercher la
/// mauvaise chose, ce qui est la faute que ce dépôt traque partout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotDelegated {
    /// Pas de hiérarchie unifiée lisible : la question ne se pose même pas.
    NoUnifiedHierarchy {
        /// Ce que l'hôte a répondu.
        reason: String,
    },
    /// Une hiérarchie existe, et les contrôleurs qu'il faut n'y sont pas délégués à ce processus.
    MissingControllers {
        /// Ceux qui manquent, sous leur nom de contrôleur.
        missing: BTreeSet<String>,
        /// Ceux qui sont bien là — pour qu'un exploitant voie que la lecture a eu lieu.
        available: BTreeSet<String>,
    },
}

impl fmt::Display for NotDelegated {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoUnifiedHierarchy { reason } => write!(
                formatter,
                "aucun cgroup n'est délégué à ce processus : {reason}. Les bornes de ressources ne \
                 peuvent donc pas être posées autour de la sandbox, et le niveau qui les promet ne \
                 sera pas attesté"
            ),
            Self::MissingControllers { missing, available } => write!(
                formatter,
                "les contrôleurs {} ne sont pas délégués à ce processus (délégués : {}). Le \
                 déploiement doit les activer dans le « cgroup.subtree_control » du parent ; sans \
                 eux, une sandbox tournerait sans borne **sans que rien ne le signale**",
                nommer(missing),
                if available.is_empty() {
                    "aucun".to_owned()
                } else {
                    nommer(available)
                }
            ),
        }
    }
}

impl std::error::Error for NotDelegated {}

/// Une liste de contrôleurs, lisible par un humain et stable d'une exécution à l'autre.
fn nommer(controleurs: &BTreeSet<String>) -> String {
    controleurs
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

impl Delegation {
    /// Lire ce que l'hôte délègue, ou dire pourquoi il ne délègue pas.
    ///
    /// # Pourquoi les faits entrent par paramètre
    ///
    /// [`HostFacts`] sait déjà lire les contrôleurs délégués **à ce processus**, et sa lecture est
    /// testable sans machine. Refaire la lecture ici donnerait deux sources pour un même fait, et
    /// deux sources pour un même fait finissent toujours par diverger — c'est la raison pour
    /// laquelle une sonde envisagée pour cet item a été abandonnée en découvrant qu'elle existait.
    ///
    /// # Errors
    ///
    /// [`NotDelegated`] quand la hiérarchie manque, ou quand les contrôleurs de
    /// [`REQUIRED_CONTROLLERS`] n'y sont pas tous délégués.
    pub fn read(facts: &HostFacts) -> Result<Self, NotDelegated> {
        if !facts.cgroup_v2().is_available() {
            return Err(NotDelegated::NoUnifiedHierarchy {
                reason: raison(facts),
            });
        }
        let available: BTreeSet<String> = facts.controllers().clone();
        let missing: BTreeSet<String> = REQUIRED_CONTROLLERS
            .iter()
            .filter(|nom| !available.contains(**nom))
            .map(|nom| (*nom).to_owned())
            .collect();
        if missing.is_empty() {
            return Ok(Self {
                controllers: available,
            });
        }
        Err(NotDelegated::MissingControllers { missing, available })
    }

    /// Les contrôleurs délégués.
    #[must_use]
    pub const fn controllers(&self) -> &BTreeSet<String> {
        &self.controllers
    }

    /// Porte-t-elle ce contrôleur ?
    #[must_use]
    pub fn carries(&self, controller: &str) -> bool {
        self.controllers.contains(controller)
    }
}

/// Ce que l'hôte a répondu sur la hiérarchie, tel qu'il l'a répondu.
///
/// La phrase de [`super::probe::Support`] est reprise **sans être réécrite** : elle nomme le fichier
/// qui manque, et c'est ce qu'un exploitant ira regarder.
fn raison(facts: &HostFacts) -> String {
    match facts.cgroup_v2() {
        super::probe::Support::Available => {
            // Inatteignable : l'appelant a vérifié `is_available` juste avant. La branche existe
            // pour que le compilateur n'ait pas à le croire sur parole.
            "la hiérarchie est disponible".to_owned()
        }
        super::probe::Support::Unavailable { reason }
        | super::probe::Support::Undetermined { reason } => reason.clone(),
    }
}
