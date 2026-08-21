//! Le retrieval hybride — `docs/SPEC_V1.md` §16.3.
//!
//! # Les deux phrases normatives, et ce qu'elles coûtent
//!
//! « Le ranking **DOIT** exposer ses facteurs. Les embeddings **ne peuvent pas** contourner les
//! ACL. » Ce sont les deux seules obligations en majuscules de la section, et chacune se tient ici
//! par la forme des types plutôt que par une discipline d'appel.
//!
//! # Un score n'est jamais un nombre nu
//!
//! [`Ranking`] ne se construit pas sans ses contributions. Il n'existe aucun chemin par lequel un
//! score arrive sans dire d'où il vient — pas parce qu'on a pensé à l'interdire, mais parce que le
//! constructeur refuse une liste vide. Un flottant nu se compare, se trie et se cite, et personne
//! ne peut dire pourquoi il vaut ce qu'il vaut : c'est le cas où l'obligation de §16.3 se perd sans
//! que rien n'échoue.
//!
//! # L'ACL est appliquée avant le score, pas contre lui
//!
//! Un filtre appliqué **après** le classement dépend de son ordre d'exécution, et il suffit d'un
//! `sort` déplacé pour qu'un document restreint sorte en tête. Ici, [`retrieve`] écarte d'abord ce
//! que l'habilitation refuse, et le classement ne voit jamais ces candidats. Un score maximal
//! n'a donc rien à contourner : il n'est pas dans la course. Le test l'exerce avec `f64::MAX`.
//!
//! # Ce qui est écarté est nommé
//!
//! Comme les `redactions` de §16.2. Une exclusion silencieuse rend deux résultats indiscernables —
//! celui qui n'avait rien à écarter et celui qui a tout écarté — et un chercheur conclurait que la
//! mémoire ne contient rien sur son sujet.
//!
//! # Le budget de contexte tronque, et le dit
//!
//! §16.3 nomme le « budget de contexte » parmi les signaux, et c'est aussi une borne. Ce qui tombe
//! sous la borne est **nommé**, pour la même raison : une troncature silencieuse se lit comme « il
//! n'y avait que cela ».

use std::collections::BTreeMap;
use std::fmt;

use locus_domain::Confidentiality;

use crate::genre::Genre;
use crate::plan::{Escalation, Plan, Provenance};

/// Les dix signaux que §16.3 combine, dans l'ordre du texte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Signal {
    /// Traversée de graphe.
    GraphTraversal,
    /// Recherche lexicale.
    Lexical,
    /// Recherche vectorielle.
    Vector,
    /// Identifiants exacts, citations et formules.
    ExactIdentifiers,
    /// Temporalité.
    Temporality,
    /// Niveau de validation.
    ValidationLevel,
    /// Branche et confidentialité.
    BranchAndConfidentiality,
    /// Diversité des sources.
    SourceDiversity,
    /// Résultats négatifs.
    ///
    /// Un **signal**, pas un filtre. L'invariant 12 refuse qu'on supprime les résultats négatifs
    /// pour rendre le graphe propre ; les taire au retrieval reviendrait au même, en moins visible.
    NegativeResults,
    /// Budget de contexte.
    ContextBudget,
}

impl Signal {
    /// Les dix, dans l'ordre de §16.3.
    pub const ALL: [Self; 10] = [
        Self::GraphTraversal,
        Self::Lexical,
        Self::Vector,
        Self::ExactIdentifiers,
        Self::Temporality,
        Self::ValidationLevel,
        Self::BranchAndConfidentiality,
        Self::SourceDiversity,
        Self::NegativeResults,
        Self::ContextBudget,
    ];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::GraphTraversal => "graph-traversal",
            Self::Lexical => "lexical",
            Self::Vector => "vector",
            Self::ExactIdentifiers => "exact-identifiers",
            Self::Temporality => "temporality",
            Self::ValidationLevel => "validation-level",
            Self::BranchAndConfidentiality => "branch-and-confidentiality",
            Self::SourceDiversity => "source-diversity",
            Self::NegativeResults => "negative-results",
            Self::ContextBudget => "context-budget",
        }
    }
}

impl fmt::Display for Signal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Un score, avec ce qui l'a produit.
///
/// Il **ne se construit pas** sans ses contributions : c'est ainsi que « le ranking DOIT exposer ses
/// facteurs » devient vrai plutôt que promis.
#[derive(Debug, Clone, PartialEq)]
pub struct Ranking {
    contributions: BTreeMap<Signal, f64>,
}

impl Ranking {
    /// Composer un score depuis les signaux qui l'ont produit.
    ///
    /// # Errors
    ///
    /// [`RetrievalError::NoFactorsExposed`] pour une liste vide — un score sans facteurs est un
    /// nombre nu, et §16.3 l'interdit ; [`RetrievalError::NotFinite`] pour une contribution qui
    /// n'est pas un nombre fini, parce que `NaN` se propagerait dans le tri en le rendant muet
    /// plutôt que faux, ce qui est pire — on ne le verrait pas.
    pub fn of(contributions: &[(Signal, f64)]) -> Result<Self, RetrievalError> {
        if contributions.is_empty() {
            return Err(RetrievalError::NoFactorsExposed);
        }
        for (signal, value) in contributions {
            if !value.is_finite() {
                return Err(RetrievalError::NotFinite { signal: *signal });
            }
        }
        Ok(Self {
            contributions: contributions.iter().copied().collect(),
        })
    }

    /// Le total, qui n'existe qu'à côté de ses facteurs.
    #[must_use]
    pub fn total(&self) -> f64 {
        self.contributions.values().sum()
    }

    /// Ce qui a contribué, et de combien.
    pub fn factors(&self) -> impl Iterator<Item = (Signal, f64)> {
        self.contributions
            .iter()
            .map(|(signal, value)| (*signal, *value))
    }

    /// La contribution d'un signal, s'il en a une.
    #[must_use]
    pub fn contribution(&self, signal: Signal) -> Option<f64> {
        self.contributions.get(&signal).copied()
    }
}

/// Un candidat au retrieval.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    key: String,
    classification: Confidentiality,
    genre: Genre,
    ranking: Ranking,
    provenance: Provenance,
}

impl Candidate {
    /// Un candidat, **si le couple genre/score est admissible**.
    ///
    /// # Pourquoi le refus est ici et non dans `Ranking::of`
    ///
    /// ADR 0022 décision 2 : un objet `Formal` ne se classe pas par similarité vectorielle, son
    /// autorité étant un vérificateur. `Ranking::of` ne connaît pas le candidat et ne peut donc pas
    /// poser ce refus ; le poser après coup laisserait exister un `Ranking` valide qui deviendrait
    /// invalide en étant attaché, c'est-à-dire un état intermédiaire invalide représentable — ce que
    /// ce dépôt évite partout ailleurs.
    ///
    /// Les champs cessent d'être publics pour cette raison, et pour elle seule : un littéral de
    /// structure contournerait la vérification sans qu'aucun test ne s'en aperçoive.
    ///
    /// # Errors
    ///
    /// [`RetrievalError::VectorOnFormal`] pour le couple interdit.
    pub fn new(
        key: impl Into<String>,
        classification: Confidentiality,
        genre: Genre,
        ranking: Ranking,
    ) -> Result<Self, RetrievalError> {
        let key = key.into();
        if !genre.admits_vector_similarity()
            && ranking
                .contribution(Signal::Vector)
                .is_some_and(|value| value != 0.0)
        {
            return Err(RetrievalError::VectorOnFormal { key });
        }
        Ok(Self {
            key,
            classification,
            genre,
            ranking,
            provenance: Provenance::Direct,
        })
    }

    /// Le même candidat, obtenu **après une escalade**.
    ///
    /// Le distinguer par le type et non par une convention : un préfixe de clé ou un drapeau se
    /// perdrait à la première sérialisation, et l'escalade change la nature de la preuve — un
    /// résultat trouvé après élargissement du périmètre de branche n'a pas été obtenu sous les
    /// mêmes contraintes d'isolation, dont §12.4 dépend.
    #[must_use]
    pub fn obtained_after(mut self, escalation: Escalation) -> Self {
        self.provenance = Provenance::AfterEscalation(escalation);
        self
    }

    /// D'où il vient.
    #[must_use]
    pub const fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    /// Sa clé.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Sa classification.
    #[must_use]
    pub const fn classification(&self) -> Confidentiality {
        self.classification
    }

    /// Son genre — ADR 0022 décision 1.
    #[must_use]
    pub const fn genre(&self) -> Genre {
        self.genre
    }

    /// Vrai quand il porte un résultat négatif — **jamais une raison de l'écarter**.
    ///
    /// Lit le genre plutôt qu'un booléen à part : un drapeau qui pouvait contredire le genre était
    /// une seconde source de vérité pour la même question, et l'ADR 0022 décision 1 bis refuse
    /// exactement cela.
    #[must_use]
    pub fn is_negative(&self) -> bool {
        self.genre == Genre::Negative
    }

    /// Son score, facteurs compris.
    #[must_use]
    pub const fn ranking(&self) -> &Ranking {
        &self.ranking
    }
}

/// Pourquoi un candidat n'est pas dans le résultat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Excluded {
    /// L'habilitation du demandeur ne couvre pas sa classification.
    BeyondClearance {
        /// Lequel.
        key: String,
        /// Ce qu'il exige.
        classification: Confidentiality,
        /// Ce que le demandeur a.
        clearance: Confidentiality,
    },
    /// Il est tombé sous le budget de contexte.
    ///
    /// Nommé, jamais tronqué en silence : une troncature muette se lit comme « il n'y avait que
    /// cela ».
    BeyondBudget {
        /// Lequel.
        key: String,
        /// Son rang, à partir de 1.
        rank: usize,
    },
}

/// Ce qu'un retrieval rend.
#[derive(Debug, Clone, PartialEq)]
pub struct Results {
    included: Vec<Candidate>,
    excluded: Vec<Excluded>,
}

impl Results {
    /// Ce qui est rendu, du meilleur au moins bon.
    #[must_use]
    pub fn included(&self) -> &[Candidate] {
        &self.included
    }

    /// Ce qui a été écarté, et pourquoi.
    #[must_use]
    pub fn excluded(&self) -> &[Excluded] {
        &self.excluded
    }
}

/// Le rang de sensibilité, croissant.
///
/// `Confidentiality` est déclaré croissant en sensibilité dans `locus-domain`, mais l'ordre n'est
/// pas dérivé sur le type : le rendre explicite ici évite qu'un `match` recopié ailleurs en change
/// l'ordre sans qu'on s'en aperçoive. `packages/review` tient la même table pour la même raison, et
/// les deux se recoupent exprès.
const fn rank(classification: Confidentiality) -> u8 {
    match classification {
        Confidentiality::Public => 0,
        Confidentiality::Internal => 1,
        Confidentiality::Confidential => 2,
        Confidentiality::Restricted => 3,
    }
}

/// Chercher.
///
/// # L'ordre des étapes est la garantie
///
/// L'habilitation écarte **avant** que quoi que ce soit soit classé. Un filtre appliqué après le
/// tri dépendrait de son ordre d'exécution, et il suffirait d'un `sort` déplacé pour qu'un document
/// restreint sorte en tête. Ici le classement ne voit jamais les candidats refusés : un score
/// maximal n'a rien à contourner, il n'est pas dans la course.
///
/// Le tri est déterministe — total décroissant, puis clé — parce qu'un résultat qui changerait
/// d'ordre à contenu égal ferait douter de la mémoire plutôt que du tri.
#[must_use]
pub fn retrieve(plan: &Plan, candidates: &[Candidate], clearance: Confidentiality) -> Results {
    let budget = plan.budget();
    let mut excluded = Vec::new();
    let mut allowed: Vec<Candidate> = Vec::new();

    for candidate in candidates {
        if rank(candidate.classification()) > rank(clearance) {
            excluded.push(Excluded::BeyondClearance {
                key: candidate.key().to_owned(),
                classification: candidate.classification(),
                clearance,
            });
            continue;
        }
        allowed.push(candidate.clone());
    }

    allowed.sort_by(|left, right| {
        right
            .ranking
            .total()
            .partial_cmp(&left.ranking.total())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.key.cmp(&right.key))
    });

    // La réserve de négatifs — ADR 0022 décision 2, et elle appartient au **plan**.
    //
    // Nulle par défaut, parce que `retrieve` d'avant `W17.l` ne lisait jamais `is_negative` dans ce
    // chemin : la mettre dans le genre aurait changé silencieusement du code livré. Quand elle
    // existe, les négatifs les mieux classés prennent leurs places d'abord, et l'exclusion tombe
    // **ailleurs** — c'est ce que « un budget saturé exclut d'abord ailleurs » veut dire.
    let reserve = plan.negative_reserve().min(budget);
    let mut reserved: Vec<usize> = Vec::new();
    if reserve > 0 {
        for (position, candidate) in allowed.iter().enumerate() {
            if candidate.is_negative() && reserved.len() < reserve {
                reserved.push(position);
            }
        }
    }

    let mut included = Vec::new();
    let mut ordinary = 0_usize;
    for (position, candidate) in allowed.into_iter().enumerate() {
        let is_reserved = reserved.contains(&position);
        if !is_reserved {
            if ordinary + reserved.len() >= budget {
                excluded.push(Excluded::BeyondBudget {
                    key: candidate.key().to_owned(),
                    rank: position + 1,
                });
                continue;
            }
            ordinary += 1;
        }
        included.push(candidate);
    }

    Results { included, excluded }
}

/// Ce qui empêche un score d'exister.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetrievalError {
    /// Une contribution vectorielle sur un objet formel — ADR 0022 décision 2.
    ///
    /// L'autorité d'un objet formel est un vérificateur, et un score de proximité n'a aucune
    /// relation avec elle. Le laisser passer ferait ranger un lemme démontré par ressemblance, et la
    /// machine cesserait de distinguer « démontré » de « qui ressemble à ».
    VectorOnFormal {
        /// Lequel.
        key: String,
    },
    /// Un score sans facteurs.
    NoFactorsExposed,
    /// Une contribution qui n'est pas un nombre fini.
    NotFinite {
        /// De quel signal.
        signal: Signal,
    },
}

impl fmt::Display for RetrievalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VectorOnFormal { key } => write!(
                formatter,
                "« {key} » est formel et porte une contribution de similarité vectorielle : son \
                 autorité est un vérificateur, qu'un score de proximité ne remplace pas"
            ),
            Self::NoFactorsExposed => formatter.write_str(
                "un score sans facteurs est un nombre nu : il se compare, se trie et se cite, et \
                 personne ne peut dire pourquoi il vaut ce qu'il vaut",
            ),
            Self::NotFinite { signal } => write!(
                formatter,
                "la contribution de « {signal} » n'est pas un nombre fini : elle se propagerait \
                 dans le tri en le rendant muet plutôt que faux, ce qui est pire"
            ),
        }
    }
}

impl std::error::Error for RetrievalError {}
