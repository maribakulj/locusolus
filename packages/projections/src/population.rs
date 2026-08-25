//! Les trois compteurs de population — `W23.b`, ADR 0026 décision 1.
//!
//! # Ce que « supporter N agents » veut dire ici
//!
//! L'ADR 0026 s'engage sur une définition unique : « **N identités** capables de mémoriser, de
//! recevoir des événements, d'être ordonnancées et de participer à une campagne, dont un
//! sous-ensemble variable raisonne concurremment ». Les trois compteurs sont exactement les trois
//! nombres de cette phrase, et ils ne se confondent pas :
//!
//! | Compteur     | Ce qu'il compte                                              | D'où il vient          |
//! | ------------ | ------------------------------------------------------------ | ---------------------- |
//! | `nominal`    | les identités que le journal connaît                         | `agent.spawned`        |
//! | `active`     | celles qui ne sont pas dans un état terminal de §7.1         | le dernier `agent.*`   |
//! | `generating` | celles qui raisonnent — une tentative en vol leur est confiée | `task.assigned` × bail |
//!
//! L'ADR nomme quatre choses à ne pas confondre : « identités stockées, objets en mémoire, agents
//! simulés, acteurs concurremment actifs ». Cette projection ne compte que la première et la
//! quatrième, et jamais les deux du milieu — un objet en mémoire n'a pas de fait dans le journal, et
//! c'est très bien ainsi.
//!
//! # Pourquoi il a fallu quatre items pour que ce fichier puisse exister
//!
//! Trois passes ont été nécessaires rien que pour nommer le blocage, et l'histoire vaut d'être
//! gardée parce que chaque étape paraissait suffisante :
//!
//! 1. l'ADR 0026 disait « `generating` compte un fait qu'aucun journal n'écrit » — vrai, et trop
//!    vague pour se périmer tout seul ;
//! 2. un déblocage a visé le cycle de bail de `W20.k`, qui est bien journalisé mais nomme un
//!    **worker** et non une instance (`W0.19`) ;
//! 3. `W20.ad` a livré la jointure `task.assigned`, et n'a débloqué qu'**un tiers** : en énumérant
//!    tous les types d'événement que `locusd` écrit, `agent` n'y figurait pas du tout, donc la
//!    **population elle-même** n'atteignait pas le journal (`W20.ae`).
//!
//! La forme commune aux deux manques : le fait existait dans le domaine, son **producteur**
//! manquait. C'est ce que ce fichier suppose désormais acquis, et rien d'autre.
//!
//! # Deux provenances, et l'asymétrie est voulue
//!
//! L'appartenance à la population est une décision du **plan de contrôle** : les faits `agent.*` ne
//! sont retenus que quand leur acteur est le système, exactement comme `task.assigned` dans
//! [`crate::organisation_graph`], et pour la même raison — invariant 3, une population que les
//! workers écriraient serait une population qu'ils décident.
//!
//! Le **bail**, lui, est un fait de worker : c'est le worker qui réclame et qui rend, et `lep::fact`
//! pose `Agent` en le documentant. Exiger le système sur `task.leased` ne retiendrait rien, et
//! `generating` vaudrait zéro sur un système qui tourne — un zéro parfaitement faux, celui d'un
//! compteur qui n'a rien lu.
//!
//! # Aucun seuil, et c'est tenu par l'absence
//!
//! Ce module ne porte **aucune constante numérique**. L'ADR 0026 décision 3 refuse toute taille
//! décrétée avant que `W23.d` ait mesuré, et un compteur qui saurait dire « c'est trop » aurait
//! décidé à sa place. Un test lit la source et refuse une déclaration de constante, comme `W23.d`
//! demande que la taille de cellule soit tenue par l'absence de constante.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use locus_coordination::agent::InstanceState;
use locus_event_store::{ActorKind, Envelope};

use crate::projection::{Projection, ProjectionError, Watermark};

/// Le recensement d'une population, à un instant du journal.
///
/// # Les trois, ou rien
///
/// `Census` n'a ni `Default`, ni champ public, ni constructeur partiel. Un rapport qui ne porterait
/// qu'`active` laisserait son lecteur croire qu'il connaît la population : `active` seul ne dit ni
/// combien d'identités existent, ni combien raisonnent, et les trois questions se posent ensemble ou
/// pas du tout. Le type le rend inexprimable plutôt que déconseillé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Census {
    nominal: usize,
    active: usize,
    generating: usize,
}

impl Census {
    /// Recenser.
    ///
    /// # Errors
    ///
    /// [`CensusError`] quand `generating ≤ active ≤ nominal` est violé, en nommant **les trois**
    /// valeurs. Nommer la seule comparaison qui a échoué obligerait le lecteur à aller chercher les
    /// autres pour comprendre, et un recensement incohérent n'a pas de moitié saine.
    pub const fn new(
        nominal: usize,
        active: usize,
        generating: usize,
    ) -> Result<Self, CensusError> {
        if generating > active || active > nominal {
            return Err(CensusError {
                nominal,
                active,
                generating,
            });
        }
        Ok(Self {
            nominal,
            active,
            generating,
        })
    }

    /// Les identités que le journal connaît.
    #[must_use]
    pub const fn nominal(self) -> usize {
        self.nominal
    }

    /// Celles qui ne sont pas dans un état terminal.
    #[must_use]
    pub const fn active(self) -> usize {
        self.active
    }

    /// Celles à qui une tentative est confiée.
    #[must_use]
    pub const fn generating(self) -> usize {
        self.generating
    }
}

/// Un recensement qui ne peut pas être vrai.
///
/// Porte les trois valeurs : un lecteur doit pouvoir dire *laquelle* est aberrante sans relire le
/// journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CensusError {
    /// Les identités connues.
    pub nominal: usize,
    /// Les non terminales.
    pub active: usize,
    /// Celles qui raisonnent.
    pub generating: usize,
}

impl fmt::Display for CensusError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "recensement incohérent : generating={} ≤ active={} ≤ nominal={} est violé",
            self.generating, self.active, self.nominal
        )
    }
}

impl std::error::Error for CensusError {}

/// La population, reconstruite depuis le journal.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Population {
    /// L'état courant de chaque identité — le dernier fait `agent.*` de son stream.
    instances: BTreeMap<String, InstanceState>,
    /// À qui chaque tâche est confiée. La dernière assignation l'emporte : une tâche qui a changé
    /// de main est portée par son nouveau titulaire, et c'est lui qui raisonne.
    holders: BTreeMap<String, String>,
    /// Les tâches dont le bail est ouvert — `task.leased` l'ouvre, `run.completed` le referme.
    leased: BTreeSet<String>,
    watermark: Watermark,
}

impl Population {
    /// Une population vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Le recensement courant.
    ///
    /// # Panics
    ///
    /// Ne panique pas : `generating` est compté **parmi** les actives et `active` parmi les
    /// connues, donc l'invariant tient par construction et `Census::new` ne peut pas refuser. Le
    /// type reste faillible pour les appelants qui n'ont pas cette garantie — un recenseur qui
    /// prendrait ses trois nombres de trois sources différentes, par exemple.
    #[must_use]
    pub fn census(&self) -> Census {
        let nominal = self.instances.len();
        let active = self.actives().count();
        let generating = self
            .actives()
            .filter(|(agent_id, _)| self.holds_a_lease(agent_id))
            .count();
        Census::new(nominal, active, generating).unwrap_or_else(|refusal| {
            unreachable!(
                "`generating` est compté parmi les actives et `active` parmi les connues : {refusal}"
            )
        })
    }

    /// L'état d'une identité, si le journal la connaît.
    #[must_use]
    pub fn state_of(&self, agent_id: &str) -> Option<InstanceState> {
        self.instances.get(agent_id).copied()
    }

    /// Les identités non terminales.
    fn actives(&self) -> impl Iterator<Item = (&String, &InstanceState)> {
        self.instances
            .iter()
            .filter(|(_, state)| !state.is_terminal())
    }

    /// Cette identité a-t-elle une tentative en vol ?
    ///
    /// Une tâche compte pour son **titulaire courant** : `holders` porte la dernière assignation, et
    /// une tâche reprise par quelqu'un d'autre ne fait plus raisonner l'ancien.
    fn holds_a_lease(&self, agent_id: &str) -> bool {
        self.leased.iter().any(|task_id| {
            self.holders
                .get(task_id)
                .is_some_and(|held| held == agent_id)
        })
    }
}

/// Un champ texte non vide de la charge.
fn text<'a>(
    payload: &'a serde_json::Map<String, serde_json::Value>,
    field: &str,
) -> Option<&'a str> {
    payload
        .get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
}

/// La charge d'un événement, ou le refus qui dit ce qui manque.
fn object(
    position: u64,
    event: &Envelope,
) -> Result<&serde_json::Map<String, serde_json::Value>, ProjectionError> {
    event.payload.as_object().ok_or_else(|| ProjectionError {
        position,
        reason: format!("charge de « {} » non objet", event.event_type),
    })
}

/// Lire un champ obligatoire, ou refuser en disant lequel et pourquoi il compte.
fn required<'a>(
    position: u64,
    payload: &'a serde_json::Map<String, serde_json::Value>,
    field: &str,
    why: &str,
) -> Result<&'a str, ProjectionError> {
    text(payload, field).ok_or_else(|| ProjectionError {
        position,
        reason: format!("« {field} » absent : {why}"),
    })
}

impl Projection for Population {
    fn name(&self) -> &'static str {
        "population"
    }

    fn apply(&mut self, position: u64, event: &Envelope) -> Result<(), ProjectionError> {
        self.watermark = position;
        match (event.event_type.namespace(), event.event_type.verb()) {
            // L'appartenance à la population est une décision du plan de contrôle — invariant 3.
            // Un fait `agent.*` d'un autre acteur est journalisé, et c'est bien ainsi ; il n'est
            // simplement pas une source.
            ("agent", _) if event.actor.kind == ActorKind::System => {
                let payload = object(position, event)?;
                let agent_id = required(
                    position,
                    payload,
                    "agent_id",
                    "un fait de cycle de vie sans identité ne pilote rien",
                )?;
                let slug = required(
                    position,
                    payload,
                    "state",
                    "sans état, `active` ne se distingue pas de `nominal`",
                )?;
                // Un état inconnu **met en quarantaine** plutôt que de se ranger d'un côté. Le
                // supposer non terminal gonflerait `active`, le supposer terminal le raboterait, et
                // les deux rendraient un nombre que personne ne saurait faux. §9.5 : la quarantaine
                // n'empêche pas l'écriture canonique, elle empêche de croire la projection.
                let state = InstanceState::parse(slug).ok_or_else(|| ProjectionError {
                    position,
                    reason: format!(
                        "« {slug} » n'est pas un état d'instance de §7.1 : le ranger d'un côté \
                         rendrait `active` faux sans que rien ne le dise"
                    ),
                })?;
                self.instances.insert(agent_id.to_owned(), state);
            }
            // La jointure de `W20.ad` : la seule source d'un lien instance × tâche. Même garde
            // d'acteur, et pour la raison que la projection de `W13.g` documente déjà.
            ("task", "assigned") if event.actor.kind == ActorKind::System => {
                let payload = object(position, event)?;
                let task_id = required(
                    position,
                    payload,
                    "task_id",
                    "une assignation sans tâche ne confie rien",
                )?;
                let agent_id = required(
                    position,
                    payload,
                    "agent_id",
                    "c'est le lien que ce compteur existe pour joindre",
                )?;
                self.holders.insert(task_id.to_owned(), agent_id.to_owned());
            }
            // Le bail est un fait de **worker** : `lep::fact` pose `Agent` et le documente. Exiger
            // le système ici ne retiendrait rien, et `generating` vaudrait zéro sur un système qui
            // tourne.
            ("task", "leased") => {
                let payload = object(position, event)?;
                let task_id = required(
                    position,
                    payload,
                    "task_id",
                    "un bail sans tâche n'ouvre rien",
                )?;
                self.leased.insert(task_id.to_owned());
            }
            ("run", "completed") => {
                let payload = object(position, event)?;
                let task_id = required(
                    position,
                    payload,
                    "task_id",
                    "un achèvement sans tâche ne referme rien",
                )?;
                self.leased.remove(task_id);
            }
            _ => {}
        }
        Ok(())
    }

    fn watermark(&self) -> Watermark {
        self.watermark
    }

    fn reset(&mut self) {
        self.instances.clear();
        self.holders.clear();
        self.leased.clear();
        self.watermark = 0;
    }

    fn checksum(&self) -> String {
        // Les trois cartes, et non le recensement : deux populations différentes peuvent rendre
        // les mêmes trois nombres, et un résumé qui les confondrait laisserait passer la corruption
        // silencieuse que §9.5 demande de détecter.
        let instances: Vec<String> = self
            .instances
            .iter()
            .map(|(agent_id, state)| format!("{agent_id}={state}"))
            .collect();
        let holders: Vec<String> = self
            .holders
            .iter()
            .map(|(task_id, agent_id)| format!("{task_id}->{agent_id}"))
            .collect();
        let leased: Vec<&str> = self.leased.iter().map(String::as_str).collect();
        format!(
            "instances[{}]|holders[{}]|leased[{}]",
            instances.join(","),
            holders.join(","),
            leased.join(",")
        )
    }
}
