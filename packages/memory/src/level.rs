//! Les sept niveaux de mémoire — `docs/SPEC_V1.md` §16.1.
//!
//! # Sept, et ils ne se déduisent pas les uns des autres
//!
//! De la mémoire privée d'un agent à la mémoire disciplinaire. La liste est **close** : une mémoire
//! dont le niveau n'est pas nommé n'existe pas, parce qu'un niveau décide de qui peut lire — et une
//! mémoire sans portée déclarée finit par être lue par tout le monde, faute de raison de refuser.
//!
//! L'ordre est celui de la section, et il va du plus étroit au plus large. Il se compare, ce qui
//! permet de dire qu'une mémoire d'équipe est plus étroite qu'une mémoire de programme sans
//! réénumérer.
//!
//! # Canonique ou projection : la frontière du dernier paragraphe
//!
//! « Le graphe, les événements et les artefacts sont canoniques. Les résumés et embeddings sont des
//! projections régénérables. » Perdre une projection coûte un recalcul ; perdre un canonique coûte
//! la vérité institutionnelle (invariant 2). Le type porte donc la distinction, et [`Shelf::store`]
//! refuse de ranger un canonique en le déclarant régénérable — l'inverse de la faute qu'on
//! surveille d'habitude, et la seule qui compte ici : une compaction qui se croirait canonique
//! deviendrait la source.

use std::collections::BTreeMap;
use std::fmt;

/// Les sept niveaux de §16.1, du plus étroit au plus large.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    /// Mémoire privée d'agent.
    AgentPrivate,
    /// Mémoire d'équipe.
    Team,
    /// Mémoire de branche.
    Branch,
    /// Mémoire de workstream.
    Workstream,
    /// Mémoire de programme.
    Program,
    /// Mémoire inter-programmes.
    CrossProgram,
    /// Mémoire disciplinaire.
    Disciplinary,
}

impl Level {
    /// Les sept, dans l'ordre de §16.1 — du plus étroit au plus large.
    pub const ALL: [Self; 7] = [
        Self::AgentPrivate,
        Self::Team,
        Self::Branch,
        Self::Workstream,
        Self::Program,
        Self::CrossProgram,
        Self::Disciplinary,
    ];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::AgentPrivate => "agent-private",
            Self::Team => "team",
            Self::Branch => "branch",
            Self::Workstream => "workstream",
            Self::Program => "program",
            Self::CrossProgram => "cross-program",
            Self::Disciplinary => "disciplinary",
        }
    }

    /// Le relire.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|level| level.slug() == value)
    }

    /// Vrai quand ce niveau est au moins aussi large que `other`.
    ///
    /// L'ordre de §16.1 est celui de la portée, et le rendre comparable évite qu'un appelant
    /// réénumère les sept pour poser une question que la liste répond déjà.
    #[must_use]
    pub fn is_at_least_as_wide_as(self, other: Self) -> bool {
        self >= other
    }
}

impl fmt::Display for Level {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Ce qu'une mémoire est : la vérité, ou son résumé.
///
/// §16.1 : « le graphe, les événements et les artefacts sont canoniques. Les résumés et embeddings
/// sont des projections régénérables. »
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Substance {
    /// Vérité institutionnelle. **Ne se régénère pas** : la perdre coûte ce qu'elle disait.
    Canonical,
    /// Dérivée, et régénérable depuis les canoniques. La perdre coûte un recalcul.
    Projection,
}

impl Substance {
    /// Vrai quand la perdre ne coûte qu'un recalcul.
    #[must_use]
    pub const fn is_regenerable(self) -> bool {
        matches!(self, Self::Projection)
    }

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Canonical => "canonical",
            Self::Projection => "projection",
        }
    }
}

impl fmt::Display for Substance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Ce qu'on range dans un niveau.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    key: String,
    level: Level,
    substance: Substance,
}

impl Entry {
    /// Sa clé.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Son niveau.
    #[must_use]
    pub const fn level(&self) -> Level {
        self.level
    }

    /// Ce qu'elle est.
    #[must_use]
    pub const fn substance(&self) -> Substance {
        self.substance
    }
}

/// Le rangement : ce que chaque niveau retient.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Shelf {
    entries: BTreeMap<String, Entry>,
}

impl Shelf {
    /// Une mémoire vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Ranger une entrée.
    ///
    /// # Errors
    ///
    /// [`MemoryError::EmptyKey`] pour une entrée sans clé — elle ne se retrouve pas, donc elle est
    /// perdue en étant rangée, ce qui est pire que de ne pas la ranger ;
    /// [`MemoryError::AlreadyStored`] pour une clé déjà prise, parce qu'écraser en silence ferait
    /// disparaître un canonique derrière une projection du même nom.
    pub fn store(
        &mut self,
        key: &str,
        level: Level,
        substance: Substance,
    ) -> Result<&Entry, MemoryError> {
        if key.trim().is_empty() {
            return Err(MemoryError::EmptyKey);
        }
        if let Some(existing) = self.entries.get(key) {
            return Err(MemoryError::AlreadyStored {
                key: key.to_owned(),
                level: existing.level,
            });
        }
        let entry = Entry {
            key: key.to_owned(),
            level,
            substance,
        };
        Ok(self.entries.entry(key.to_owned()).or_insert(entry))
    }

    /// Ce qui est rangé à cette clé.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Entry> {
        self.entries.get(key)
    }

    /// Ce que ce niveau retient.
    pub fn at(&self, level: Level) -> impl Iterator<Item = &Entry> {
        self.entries
            .values()
            .filter(move |entry| entry.level == level)
    }

    /// Ce qu'une reconstruction depuis les canoniques devrait refaire.
    ///
    /// Exactement les projections, et rien d'autre. C'est la question que §9.1 pose de l'autre côté
    /// — « les vecteurs et graph databases sont des projections reconstructibles » — et y répondre
    /// depuis la mémoire évite qu'un opérateur ait à deviner ce qu'une purge lui coûtera.
    pub fn regenerable(&self) -> impl Iterator<Item = &Entry> {
        self.entries
            .values()
            .filter(|entry| entry.substance.is_regenerable())
    }

    /// Ce qu'aucune reconstruction ne rendrait.
    pub fn irreplaceable(&self) -> impl Iterator<Item = &Entry> {
        self.entries
            .values()
            .filter(|entry| !entry.substance.is_regenerable())
    }
}

/// Ce qui empêche de ranger une mémoire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryError {
    /// Une entrée sans clé.
    EmptyKey,
    /// Une clé déjà prise.
    AlreadyStored {
        /// Laquelle.
        key: String,
        /// À quel niveau elle est déjà rangée.
        level: Level,
    },
}

impl fmt::Display for MemoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyKey => formatter.write_str(
                "une entrée sans clé ne se retrouve pas : la ranger est pire que ne pas la ranger",
            ),
            Self::AlreadyStored { key, level } => write!(
                formatter,
                "« {key} » est déjà rangée en « {level} » : écraser en silence ferait disparaître un \
                 canonique derrière une projection du même nom"
            ),
        }
    }
}

impl std::error::Error for MemoryError {}
