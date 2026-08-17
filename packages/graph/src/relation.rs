//! Les relations typées — `docs/SPEC_V1.md` §7.5.

use std::fmt;

use serde::{Deserialize, Serialize};

use locus_domain::RevisionId;

/// Ce qu'une relation autorise dans l'autre sens.
///
/// §7.5, dernière phrase : « les relations non symétriques ne doivent **pas** être inférées en sens
/// inverse ». Une phrase courte, et la seule chose qui empêche un graphe de raconter le contraire
/// de ce qu'on y a écrit.
///
/// L'exemple qui coûte : `A supports B` lu à l'envers donnerait `B supports A`. Sur un graphe
/// argumentaire, cela retourne le sens de l'étayage — la preuve devient la thèse. Sur `cites`,
/// cela fait citer un article de 2026 par un article de 1890.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// La relation vaut dans les deux sens. `A contradicts B` ⇒ `B contradicts A`.
    Symmetric,
    /// La relation a une réciproque **d'un autre nom**. `A generalizes B` ⇒ `B specializes A`.
    Converse(RelationKind),
    /// La relation ne vaut que dans un sens, et rien ne se déduit de l'autre.
    ///
    /// Ce n'est pas « la réciproque est fausse » : c'est « la réciproque n'est pas déductible ».
    /// `A depends_on B` ne dit rien de ce que B fait de A, et inventer un nom pour ce silence
    /// serait exactement l'inférence que §7.5 interdit.
    OneWay,
}

/// Les vingt-huit relations core de §7.5, dans l'ordre du texte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RelationKind {
    /// Dépend de.
    DependsOn,
    /// Implique.
    Implies,
    /// Étaye.
    Supports,
    /// Réfute.
    Refutes,
    /// Contredit.
    Contradicts,
    /// Généralise.
    Generalizes,
    /// Spécialise.
    Specializes,
    /// Formalise.
    Formalizes,
    /// Cite.
    Cites,
    /// Dérive de.
    DerivedFrom,
    /// Produit par.
    ProducedBy,
    /// Consomme.
    Consumes,
    /// Teste.
    Tests,
    /// Reproduit.
    Reproduces,
    /// Échoue à cause de.
    FailsBecause,
    /// Bloqué par.
    BlockedBy,
    /// Analogue à.
    AnalogousTo,
    /// Transfère vers.
    TransfersTo,
    /// Relu par.
    ReviewedBy,
    /// Répond à.
    RespondsTo,
    /// Assigné à.
    AssignedTo,
    /// Forké depuis.
    ForkedFrom,
    /// Fusionné dans.
    MergedInto,
    /// Remplace.
    Supersedes,
    /// Instancie.
    Instantiates,
    /// Mesure.
    Measures,
    /// Interprète.
    Interprets,
    /// Ancré dans.
    AnchoredIn,
}

impl RelationKind {
    /// Les vingt-huit relations, dans l'ordre du texte.
    pub const ALL: [Self; 28] = [
        Self::DependsOn,
        Self::Implies,
        Self::Supports,
        Self::Refutes,
        Self::Contradicts,
        Self::Generalizes,
        Self::Specializes,
        Self::Formalizes,
        Self::Cites,
        Self::DerivedFrom,
        Self::ProducedBy,
        Self::Consumes,
        Self::Tests,
        Self::Reproduces,
        Self::FailsBecause,
        Self::BlockedBy,
        Self::AnalogousTo,
        Self::TransfersTo,
        Self::ReviewedBy,
        Self::RespondsTo,
        Self::AssignedTo,
        Self::ForkedFrom,
        Self::MergedInto,
        Self::Supersedes,
        Self::Instantiates,
        Self::Measures,
        Self::Interprets,
        Self::AnchoredIn,
    ];

    /// Ce que la relation autorise dans l'autre sens — §7.5.
    ///
    /// Trois symétriques seulement : `contradicts`, `analogous_to`, et rien d'autre ne l'est
    /// vraiment. `supports` ne l'est pas — deux thèses qui s'étayent mutuellement sont deux
    /// relations écrites, pas une relation lue deux fois. Deux paires de réciproques nommées :
    /// `generalizes`/`specializes` et `forked_from`/`merged_into`. Tout le reste est à sens unique.
    #[must_use]
    pub const fn direction(self) -> Direction {
        match self {
            // Symétriques : la relation dit la même chose des deux côtés.
            Self::Contradicts | Self::AnalogousTo => Direction::Symmetric,
            // Réciproques nommées : l'autre sens existe, et il porte un autre nom.
            Self::Generalizes => Direction::Converse(Self::Specializes),
            Self::Specializes => Direction::Converse(Self::Generalizes),
            // `forked_from` et `merged_into` décrivent le même événement vu des deux bouts.
            Self::ForkedFrom => Direction::Converse(Self::MergedInto),
            Self::MergedInto => Direction::Converse(Self::ForkedFrom),
            // Tout le reste : rien ne se déduit de l'autre sens.
            Self::DependsOn
            | Self::Implies
            | Self::Supports
            | Self::Refutes
            | Self::Formalizes
            | Self::Cites
            | Self::DerivedFrom
            | Self::ProducedBy
            | Self::Consumes
            | Self::Tests
            | Self::Reproduces
            | Self::FailsBecause
            | Self::BlockedBy
            | Self::TransfersTo
            | Self::ReviewedBy
            | Self::RespondsTo
            | Self::AssignedTo
            | Self::Supersedes
            | Self::Instantiates
            | Self::Measures
            | Self::Interprets
            | Self::AnchoredIn => Direction::OneWay,
        }
    }

    /// La relation qui vaut de la cible vers la source, quand il y en a une.
    ///
    /// `None` pour une relation à sens unique, et c'est **le point** : il n'existe aucune fonction
    /// qui retourne une relation quelconque. Un appelant qui veut parcourir à rebours doit soit
    /// trouver une réciproque nommée, soit chercher les arêtes entrantes — ce qui est une lecture,
    /// pas une déduction.
    #[must_use]
    pub const fn converse(self) -> Option<Self> {
        match self.direction() {
            Direction::Symmetric => Some(self),
            Direction::Converse(other) => Some(other),
            Direction::OneWay => None,
        }
    }

    /// Le nom canonique, celui du texte.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DependsOn => "depends_on",
            Self::Implies => "implies",
            Self::Supports => "supports",
            Self::Refutes => "refutes",
            Self::Contradicts => "contradicts",
            Self::Generalizes => "generalizes",
            Self::Specializes => "specializes",
            Self::Formalizes => "formalizes",
            Self::Cites => "cites",
            Self::DerivedFrom => "derived_from",
            Self::ProducedBy => "produced_by",
            Self::Consumes => "consumes",
            Self::Tests => "tests",
            Self::Reproduces => "reproduces",
            Self::FailsBecause => "fails_because",
            Self::BlockedBy => "blocked_by",
            Self::AnalogousTo => "analogous_to",
            Self::TransfersTo => "transfers_to",
            Self::ReviewedBy => "reviewed_by",
            Self::RespondsTo => "responds_to",
            Self::AssignedTo => "assigned_to",
            Self::ForkedFrom => "forked_from",
            Self::MergedInto => "merged_into",
            Self::Supersedes => "supersedes",
            Self::Instantiates => "instantiates",
            Self::Measures => "measures",
            Self::Interprets => "interprets",
            Self::AnchoredIn => "anchored_in",
        }
    }

    /// Lit un nom de relation core.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == name)
    }
}

impl fmt::Display for RelationKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for RelationKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RelationKind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        let text = <&str>::deserialize(deserializer)?;
        Self::parse(text).ok_or_else(|| D::Error::custom("relation absente de §7.5"))
    }
}

/// La force d'une relation — §7.5, « une relation est un objet versionné avec […] force ».
///
/// Bornée à `[0, 1]` à la construction. Une force hors bornes n'est pas une force forte : c'est un
/// chiffre dont personne ne sait ce qu'il mesure.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Strength(f64);

impl Strength {
    /// Construit une force.
    ///
    /// # Errors
    ///
    /// Rend `None` hors de `[0, 1]`, et pour `NaN`.
    #[must_use]
    pub fn new(value: f64) -> Option<Self> {
        (value.is_finite() && (0.0..=1.0).contains(&value)).then_some(Self(value))
    }

    /// La valeur.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.0
    }
}

/// Une relation typée entre deux révisions — §7.5.
///
/// « Une relation est un objet versionné avec auteur, scope, force, justification et evidence
/// refs. » Les cinq sont présents, et `revision` fait de la relation un objet à part entière
/// plutôt qu'une arête anonyme : une relation se révise, se conteste et se cite comme le reste.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relation {
    /// L'identité de la relation elle-même.
    pub id: String,
    /// La source.
    pub from: RevisionId,
    /// La cible.
    pub to: RevisionId,
    /// Le type.
    pub kind: RelationKind,
    /// Qui l'a posée.
    pub author: String,
    /// Le domaine de validité.
    pub scope: String,
    /// La force.
    pub strength: Strength,
    /// Pourquoi.
    pub justification: String,
    /// Sur quoi elle s'appuie.
    #[serde(default)]
    pub evidence_refs: Vec<RevisionId>,
    /// Le rang de révision de la relation.
    pub revision: u32,
}
