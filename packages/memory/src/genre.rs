//! Le genre d'une mémoire — ADR 0022 décisions 1 et 1 bis.
//!
//! # Deux dimensions, et elles ne se remplacent pas
//!
//! [`crate::Level`] dit **qui a le droit de voir** ; [`Genre`] dit **ce que le lecteur a le droit
//! d'en faire**. Les aplatir coûte cher, et trois cas suffisent à le montrer : sans `Formal`, un
//! lemme vérifié par un checker se range par proximité d'embedding à côté d'une conjecture, et la
//! machine cesse de distinguer « démontré » de « qui ressemble à » ; sans `Negative` distinct, la
//! `negative_result_policy` de §16.2 n'a aucun ensemble sur lequel réserver un budget ; sans
//! `MetaMemory` séparée, l'utilité passée d'un document finit par entrer dans son score de vérité,
//! ce qui est le biais de citation reconstruit avec de l'apprentissage automatique.
//!
//! # Pourquoi `Genre` et non `Kind`
//!
//! `Kind` est pris — `compaction::Kind` est exporté par ce crate. Deux `Kind` dans un `use` seraient
//! renommés à l'import par chaque appelant, ce qui est la duplication de vocabulaire sous une autre
//! forme. Ce crate a d'ailleurs déjà tranché la même collision dans le même sens : `dedup::Candidate`
//! est exporté comme `DuplicateCandidate` parce que `retrieval::Candidate` tenait le nom.
//!
//! # Aucune conversion, et c'est la règle du crate
//!
//! `separated.rs` l'a établie pour les deux retrievals : « aucune conversion n'est écrivable, parce
//! que le préfixe fait partie de l'identité ». Elle vaut ici et elle est **plus forte** : une
//! conversion de genre serait une conversion d'**autorité**, c'est-à-dire l'affirmation qu'un objet
//! est vrai pour une raison qui ne l'a jamais établi. Un objet formel ne devient pas sémantique
//! parce qu'il est beaucoup cité ; une stratégie qui a souvent marché ne devient pas une preuve.
//!
//! La factorisation par le haut est refusée pour la même raison : un trait générique « ce qui peut
//! être rangé, retrouvé, promu » reconstruirait la conversion en la rendant invisible. La
//! duplication est le choix correct, et cette phrase est sa justification.

use std::fmt;

/// Les dix genres de l'ADR 0022 décision 1 — liste close.
///
/// L'ordre est celui du tableau de l'ADR, pour qu'on puisse comparer les deux. Il n'est **pas** un
/// rang : aucun genre n'est plus fort qu'un autre, ils disent des choses différentes. Ce type ne
/// dérive donc ni `PartialOrd` ni `Ord`, comme `Mode` de `coordination` et pour la même raison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Genre {
    /// Attempts, actions, échecs, décisions. Fait autorité : l'histoire observée — le journal.
    Episodic,
    /// Claims validés, concepts, relations. Fait autorité : la validation épistémique §8.1.
    Semantic,
    /// Lemmes vérifiés, termes de preuve, dépendances. Fait autorité : un vérificateur, **jamais un
    /// consensus**.
    Formal,
    /// Échecs, contre-exemples, routes impossibles. Fait autorité : l'observation ou la
    /// vérification — et l'invariant 12, qui interdit de les supprimer pour faire propre.
    Negative,
    /// Skills, workflows, outils réutilisables. Fait autorité : des tests exécutables.
    Procedural,
    /// Tactiques, patterns de décomposition. Fait autorité : l'utilité empirique mesurée.
    Strategic,
    /// Sources, citations, provenance bibliographique. Fait autorité : la provenance de source.
    Literature,
    /// Résultats de calcul, expériences numériques. Fait autorité : la reproductibilité §19.
    Computational,
    /// Qui sait quoi, qui a besoin de quoi. **Temporaire, jamais canonique.**
    Coordination,
    /// Fiabilité d'une source, utilité passée d'un retrieval. Métadonnée apprise — elle influence le
    /// rang, **jamais la validité**.
    MetaMemory,
}

impl Genre {
    /// Les dix, dans l'ordre du tableau de l'ADR 0022.
    pub const ALL: [Self; 10] = [
        Self::Episodic,
        Self::Semantic,
        Self::Formal,
        Self::Negative,
        Self::Procedural,
        Self::Strategic,
        Self::Literature,
        Self::Computational,
        Self::Coordination,
        Self::MetaMemory,
    ];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Episodic => "episodic",
            Self::Semantic => "semantic",
            Self::Formal => "formal",
            Self::Negative => "negative",
            Self::Procedural => "procedural",
            Self::Strategic => "strategic",
            Self::Literature => "literature",
            Self::Computational => "computational",
            Self::Coordination => "coordination",
            Self::MetaMemory => "meta-memory",
        }
    }

    /// Le relire.
    ///
    /// `None` plutôt qu'un défaut : un genre inconnu rabattu sur `Semantic` ferait passer pour un
    /// claim validé quelque chose que personne n'a validé, et c'est la faute que la dimension existe
    /// pour empêcher.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|genre| genre.slug() == value)
    }

    /// Vrai quand un objet de ce genre peut porter une contribution de similarité vectorielle.
    ///
    /// **Faux pour `Formal` seul**, et c'est la décision 2 de l'ADR 0022 : l'autorité d'un objet
    /// formel est un vérificateur, et un score de proximité n'a aucune relation avec elle. Le refus
    /// se pose à la construction du candidat — voir `retrieval::Candidate::new` — plutôt que dans
    /// `Ranking::of`, qui ne connaît pas le candidat : un `Ranking` valide qui deviendrait invalide
    /// en étant attaché serait un état intermédiaire invalide représentable.
    #[must_use]
    pub const fn admits_vector_similarity(self) -> bool {
        !matches!(self, Self::Formal)
    }

    /// Vrai quand un objet de ce genre peut soutenir une conclusion.
    ///
    /// **Faux pour `MetaMemory` seul.** Elle influence le rang et jamais la validité : la fiabilité
    /// passée d'une source n'est pas une raison de croire ce qu'elle dit aujourd'hui. Ce crate ne
    /// connaît ni `Support` ni `Inference` — c'est `packages/graph` qui les tient —, et l'interdit
    /// s'y applique par l'absence de conversion. Ce prédicat est ce qu'un appelant interroge pour ne
    /// pas réénumérer les dix.
    #[must_use]
    pub const fn may_support_a_conclusion(self) -> bool {
        !matches!(self, Self::MetaMemory)
    }
}

impl fmt::Display for Genre {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Ce qui sait dire de quel genre relève une clé — un **port**, fourni par l'appelant.
///
/// # Pourquoi un port, et pourquoi il a le droit de ne pas savoir
///
/// Quatre genres recouvrent des distinctions que le dépôt encode déjà ailleurs :
/// `Negative` ↔ `CoreObjectType::NegativeResult` et l'agrégat de §18.7 ; `Formal` ↔
/// `FormalizationStatus` ; `Computational` ↔ `reproducibility` ; `Coordination` ↔ le crate de
/// coordination. Laisser le genre se déclarer sans jamais le confronter en ferait une seconde source
/// de vérité, qui divergerait le jour où l'une des deux serait corrigée.
///
/// Mais le faire **dériver** obligerait `packages/memory` à connaître `graph`, `artifacts` et
/// `domain`, et un rangement échouerait faute de résolveur — ce qui est absurde pour une mémoire.
///
/// D'où la forme retenue : le genre reste **déclaré**, et là où un type est connu, le désaccord est
/// un **refus**. Une clé qu'aucun port ne résout est **acceptée** — l'ignorance n'est pas un
/// démenti, et c'est la règle que `xiiif` applique déjà en ne collapsant pas `unverified` sur
/// `broken`.
pub trait GenreOracle {
    /// Le genre que le reste du système attribue à cette clé, s'il en connaît un.
    fn genre_of(&self, key: &str) -> Option<Genre>;
}

/// Un oracle qui ne sait rien — le défaut, et il accepte tout.
///
/// Utile à un appelant qui n'a pas de résolveur sous la main, et **honnête** : il ne prétend pas
/// vérifier. Un oracle qui rendrait le genre déclaré pour le confirmer serait pire que rien, puisque
/// la vérification passerait toujours.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Unknowing;

impl GenreOracle for Unknowing {
    fn genre_of(&self, _key: &str) -> Option<Genre> {
        None
    }
}
