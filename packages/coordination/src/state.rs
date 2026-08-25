//! L'état persisté d'une instance d'agent, et le port qui le conserve — `W23.a`, ADR 0026 décision 2.
//!
//! # Ce que l'ADR décide, et ce qui manquait
//!
//! « Une instance existe indépendamment de tout processus. Elle est **reconstruite** depuis son état
//! persisté au moment où on l'exécute, et **rejetée** ensuite ; aucun objet d'agent ne traverse une
//! frontière de processus. »
//!
//! [`crate::agent::AgentInstance`] porte déjà l'identité durable — les six `InstanceState` de §7.1,
//! `provision`, `moved_to` et ses transitions refusées. Ce qui manquait est le **port de persistance**
//! et le **protocole de reconstruction**, et rien d'autre.
//!
//! # Aucun `serde` ici, et c'est la garantie elle-même
//!
//! `packages/coordination` ne dépend de `serde` sous **aucune** forme, et ce module ne l'y introduit
//! pas. C'est ce qui fait tenir « aucun objet d'agent ne traverse une frontière de processus »
//! **par construction** plutôt que par discipline : il n'existe dans ce crate aucun type sérialisable,
//! donc a fortiori aucun type sérialisable portant un comportement.
//!
//! Un adaptateur a pourtant besoin d'une forme à écrire. C'est [`AgentState::encode`], une
//! canonicalisation **écrite à la main** — ce que §7.7 demande de toute façon d'un condensat : « les
//! hashes portent sur une canonicalisation stable ». Dériver `Serialize` aurait fait dépendre le
//! condensat de l'ordre des champs que le dérivé se trouve produire, c'est-à-dire d'un détail que
//! personne ne relit.
//!
//! # Reconstruire n'est pas une transition
//!
//! [`AgentState::restore`] repose l'état tel qu'il était, y compris terminal. Passer par `moved_to`
//! serait faux deux fois : la machine de §7.1 refuse de quitter un état terminal, donc une instance
//! `Completed` ne serait pas reconstructible ; et une reconstruction n'est **pas** un changement
//! d'état — c'est la même instance qu'on relit, pas une instance qu'on fait avancer. Les journaliser
//! comme des transitions ferait compter à `W21.j` des durées de vie qui n'ont pas eu lieu.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Mutex, PoisonError};

use locus_domain::ContentHash;
use locus_protocol::Id;
use locus_protocol::id::{Agent, Branch, Program, provisional::Team as TeamKind};

use crate::agent::{AgentError, AgentInstance, InstanceState};

/// L'état d'une instance, **sans comportement**.
///
/// De la donnée, et rien que de la donnée : aucune méthode qui décide, aucune qui transitionne. Ce
/// qui décide vit sur [`AgentInstance`], que ce type ne remplace pas — il la **décrit**, le temps
/// d'un aller-retour par un support.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentState {
    id: Id<Agent>,
    template_id: Id<Agent>,
    template_version: u32,
    program_id: Option<Id<Program>>,
    branch_id: Option<Id<Branch>>,
    team_id: Option<Id<TeamKind>>,
    worker_id: Option<String>,
    independence_group: Option<String>,
    state: InstanceState,
}

/// Le séparateur des champs dans la forme canonique.
///
/// Un caractère qu'aucun identifiant ni slug de §7.1 ne contient — les identifiants sont en
/// Crockford base32, les slugs en minuscules ASCII. Le choisir dans l'alphabet des valeurs aurait
/// rendu la forme ambiguë, et une ambiguïté dans une canonicalisation est un condensat qui confond
/// deux états.
const SEPARATOR: char = '\u{1f}';

/// Ce qu'un champ absent écrit. Distinct de la chaîne vide, qu'un `worker_id` pourrait porter.
const ABSENT: &str = "-";

impl AgentState {
    /// L'état de cette instance, tel qu'un support le conservera.
    #[must_use]
    pub fn of(instance: &AgentInstance) -> Self {
        Self {
            id: instance.id(),
            template_id: instance.template_id(),
            template_version: instance.template_version(),
            program_id: instance.program_id(),
            branch_id: instance.branch_id(),
            team_id: instance.team_id(),
            worker_id: instance.worker_id().map(str::to_owned),
            independence_group: instance.independence_group().map(str::to_owned),
            state: instance.state(),
        }
    }

    /// L'instance que cet état décrit.
    ///
    /// Voir le module : reconstruire n'est pas une transition, donc ce chemin ne passe pas par
    /// `moved_to` et repose un état terminal tel quel.
    ///
    /// # Errors
    ///
    /// [`AgentError`] quand l'état décrit une instance que le domaine refuse de construire.
    pub fn restore(&self) -> Result<AgentInstance, AgentError> {
        AgentInstance::from_state(
            self.id,
            self.template_id,
            self.template_version,
            self.program_id,
            self.branch_id,
            self.team_id,
            self.worker_id.as_deref(),
            self.independence_group.as_deref(),
            self.state,
        )
    }

    /// La forme canonique, celle qu'un support écrit et que le condensat couvre.
    ///
    /// L'ordre des champs est celui de §7.1 et il est **figé** : le changer changerait tous les
    /// condensats sans qu'aucune instance ait changé, ce que `W5.v` a payé sur l'empreinte d'hôte.
    #[must_use]
    pub fn encode(&self) -> String {
        let champs = [
            self.id.to_string(),
            self.template_id.to_string(),
            self.template_version.to_string(),
            self.program_id
                .map_or_else(|| ABSENT.to_owned(), |id| id.to_string()),
            self.branch_id
                .map_or_else(|| ABSENT.to_owned(), |id| id.to_string()),
            self.team_id
                .map_or_else(|| ABSENT.to_owned(), |id: Id<TeamKind>| id.to_string()),
            self.worker_id
                .clone()
                .map_or_else(|| ABSENT.to_owned(), |value| format!("={value}")),
            self.independence_group
                .clone()
                .map_or_else(|| ABSENT.to_owned(), |value| format!("={value}")),
            self.state.slug().to_owned(),
        ];
        champs.join(&SEPARATOR.to_string())
    }

    /// Relire une forme canonique.
    ///
    /// # Errors
    ///
    /// [`StateFormat`] quand la forme ne se relit pas — un champ manquant, un identifiant illisible,
    /// un slug d'état que §7.1 ne définit pas. Un slug inconnu **refuse** plutôt que de se ranger
    /// dans un état voisin : `Provisioned` par défaut ferait revivre une instance terminée.
    pub fn decode(text: &str) -> Result<Self, StateFormat> {
        let champs: Vec<&str> = text.split(SEPARATOR).collect();
        let [
            id,
            template_id,
            version,
            program,
            branch,
            team,
            worker,
            groupe,
            etat,
        ] = champs[..]
        else {
            return Err(StateFormat::FieldCount {
                seen: champs.len(),
                expected: 9,
            });
        };

        Ok(Self {
            id: lu(id, "id")?,
            template_id: lu(template_id, "template_id")?,
            template_version: version.parse().map_err(|_| StateFormat::Unreadable {
                field: "template_version",
            })?,
            program_id: optionnel(program, "program_id")?,
            branch_id: optionnel(branch, "branch_id")?,
            team_id: optionnel(team, "team_id")?,
            worker_id: texte(worker),
            independence_group: texte(groupe),
            state: InstanceState::parse(etat).ok_or(StateFormat::UnknownState {
                slug: etat.to_owned(),
            })?,
        })
    }

    /// Le condensat de l'état, sur sa forme canonique — §7.7.
    #[must_use]
    pub fn digest(&self) -> ContentHash {
        ContentHash::of(self.encode().as_bytes())
    }

    /// L'instance dont cet état parle.
    #[must_use]
    pub const fn id(&self) -> Id<Agent> {
        self.id
    }

    /// L'état de §7.1 qu'elle portait.
    #[must_use]
    pub const fn state(&self) -> InstanceState {
        self.state
    }
}

fn lu<K: locus_protocol::IdKind>(text: &str, field: &'static str) -> Result<Id<K>, StateFormat> {
    Id::parse(text).map_err(|_| StateFormat::Unreadable { field })
}

fn optionnel<K: locus_protocol::IdKind>(
    text: &str,
    field: &'static str,
) -> Result<Option<Id<K>>, StateFormat> {
    if text == ABSENT {
        return Ok(None);
    }
    lu(text, field).map(Some)
}

/// Un champ textuel facultatif : `-` est l'absence, `=…` une valeur, y compris vide.
fn texte(champ: &str) -> Option<String> {
    champ.strip_prefix('=').map(str::to_owned)
}

/// Pourquoi une forme canonique ne s'est pas relue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateFormat {
    /// Le nombre de champs ne correspond pas.
    FieldCount {
        /// Combien on en a lus.
        seen: usize,
        /// Combien §7.1 en demande.
        expected: usize,
    },
    /// Un champ n'est pas lisible sous sa forme.
    Unreadable {
        /// Lequel.
        field: &'static str,
    },
    /// Un état que §7.1 ne définit pas.
    UnknownState {
        /// Le slug lu.
        slug: String,
    },
}

impl fmt::Display for StateFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FieldCount { seen, expected } => write!(
                formatter,
                "état d'instance — {seen} champ(s) lus, {expected} attendus"
            ),
            Self::Unreadable { field } => {
                write!(formatter, "état d'instance — « {field} » illisible")
            }
            Self::UnknownState { slug } => write!(
                formatter,
                "état d'instance — « {slug} » n'est pas un état de §7.1"
            ),
        }
    }
}

impl std::error::Error for StateFormat {}

/// Ce qu'un support rend : l'état **et** la révision d'où il vient.
///
/// Les deux ensemble, jamais l'un sans l'autre. Un appelant qui relirait un état sans sa révision
/// n'aurait pas de quoi le réécrire sans écraser ce qu'un autre a fait entre-temps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    /// L'état conservé.
    pub state: AgentState,
    /// La révision à laquelle il a été écrit.
    pub revision: u64,
}

/// Ce qu'une écriture attend de ce qui est déjà là — ADR 0026 décision 2.
///
/// **Il n'y a pas de variante « peu importe ».** Écrire sans savoir depuis quelle révision n'a pas de
/// sens, et `Expected` de l'event store — `NoStream`, `Exact` — le dit déjà pour le journal. La
/// persistance d'instance hérite de la même exigence, parce que la faute qu'elle évite est la même :
/// deux exécutions concurrentes de la même instance, dont la seconde écrase l'état de la première
/// sans que rien ne le dise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expectation {
    /// Rien n'est encore écrit pour cette instance.
    Absent,
    /// Ce qui est écrit l'a été à cette révision.
    At(u64),
}

/// Ce qu'une écriture refuse, et pourquoi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateConflict {
    /// L'instance visée.
    pub id: Id<Agent>,
    /// Ce que l'appelant croyait.
    pub expected: Expectation,
    /// Ce qui est réellement écrit — `None` quand rien ne l'est.
    pub actual: Option<u64>,
}

impl fmt::Display for StateConflict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let attendu = match self.expected {
            Expectation::Absent => "rien d'écrit".to_owned(),
            Expectation::At(revision) => format!("la révision {revision}"),
        };
        let reel = self.actual.map_or_else(
            || "rien n'est écrit".to_owned(),
            |r| format!("la révision {r}"),
        );
        write!(
            formatter,
            "état d'instance « {} » — l'écriture attendait {attendu}, et {reel}",
            self.id
        )
    }
}

impl std::error::Error for StateConflict {}

/// Le port de persistance d'état d'instance — ADR 0026 décision 2.
///
/// Un trait, avec une implémentation de référence en mémoire, exactement comme
/// `packages/event-store` l'a fait. **Aucun backend externe n'est choisi ici**, et l'ADR dit
/// pourquoi : le seul système vérifié persiste un répertoire par agent, ce qui tient à 10 000 et
/// charge lourdement la couche de métadonnées du système de fichiers à 100 000. Figer ce choix
/// maintenant le figerait sur une mesure qui n'a pas été faite ici.
pub trait AgentStateStore: Send + Sync {
    /// L'état conservé pour cette instance, avec sa révision — ou rien.
    ///
    /// Il n'existe pas de lecture qui rendrait l'état **sans** sa révision : voir [`Expectation`].
    fn load(&self, id: Id<Agent>) -> Option<Stored>;

    /// Écrire cet état, si ce qui est là est bien ce que l'appelant croit.
    ///
    /// # Errors
    ///
    /// [`StateConflict`] quand l'attente ne correspond pas à ce qui est écrit.
    fn save(&self, state: &AgentState, expected: Expectation) -> Result<u64, StateConflict>;
}

/// L'implémentation de référence, en mémoire.
///
/// Elle n'est pas un bouchon : c'est le support que les tests du domaine emploient, et le seul que
/// ce dépôt livre tant qu'aucune mesure n'a tranché le backend.
#[derive(Debug, Default)]
pub struct MemoryAgentStateStore {
    /// `BTreeMap` et non `HashMap` : l'ordre d'itération est stable, ce qui rend un diagnostic
    /// reproductible.
    instances: Mutex<BTreeMap<Id<Agent>, Stored>>,
}

impl MemoryAgentStateStore {
    /// Un support vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Combien d'instances y sont conservées.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    /// Vrai quand aucune ne l'est.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<Id<Agent>, Stored>> {
        self.instances
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

impl AgentStateStore for MemoryAgentStateStore {
    fn load(&self, id: Id<Agent>) -> Option<Stored> {
        self.lock().get(&id).cloned()
    }

    fn save(&self, state: &AgentState, expected: Expectation) -> Result<u64, StateConflict> {
        let mut instances = self.lock();
        let actual = instances.get(&state.id()).map(|stored| stored.revision);

        let accorde = match (expected, actual) {
            (Expectation::Absent, None) => true,
            (Expectation::At(attendue), Some(reelle)) => attendue == reelle,
            _ => false,
        };
        if !accorde {
            return Err(StateConflict {
                id: state.id(),
                expected,
                actual,
            });
        }

        let revision = actual.map_or(1, |reelle| reelle + 1);
        instances.insert(
            state.id(),
            Stored {
                state: state.clone(),
                revision,
            },
        );
        Ok(revision)
    }
}
