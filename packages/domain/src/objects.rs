//! Les types d'objets épistémiques — `docs/SPEC_V1.md` §7.3.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

/// Les quarante types core de §7.3, dans l'ordre du texte.
///
/// L'énumération est **fermée**, et c'est ce que « types core **obligatoires** » veut dire : un
/// pack disciplinaire ajoute des types, il n'en retire ni n'en redéfinit aucun. La phrase qui suit
/// la liste le dit sans détour — « les extensions ne doivent pas modifier la signification des
/// types core » — et [`ObjectType`] la rend exécutoire plutôt que déclarative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CoreObjectType {
    /// Une question de recherche.
    ResearchQuestion,
    /// Une définition.
    Definition,
    /// Une hypothèse de travail admise sans preuve.
    Assumption,
    /// Une hypothèse testable.
    Hypothesis,
    /// Une conjecture.
    Conjecture,
    /// Un énoncé proposé comme vrai.
    Claim,
    /// Un lemme.
    Lemma,
    /// Un théorème.
    Theorem,
    /// Une interprétation.
    Interpretation,
    /// Une méthode.
    Method,
    /// Une stratégie.
    Strategy,
    /// Une analogie.
    Analogy,
    /// Un certificat de transfert entre domaines.
    TransferCertificate,
    /// Un obstacle identifié.
    Barrier,
    /// Une question ouverte.
    OpenQuestion,
    /// Une objection.
    Objection,
    /// Un contre-exemple.
    Counterexample,
    /// Un résultat négatif. Invariant 12 : il ne se supprime pas.
    NegativeResult,
    /// Un échec.
    Failure,
    /// Un conflit. Invariant 12, encore.
    Conflict,
    /// Une inférence — nœud explicite, jamais une arête implicite (§7.6).
    Inference,
    /// Une expérience.
    Experiment,
    /// Une exécution.
    Run,
    /// Une source.
    Source,
    /// Une citation.
    Citation,
    /// Un jeu de données.
    Dataset,
    /// Un artefact.
    Artifact,
    /// Une figure.
    Figure,
    /// Un notebook.
    Notebook,
    /// Une révision de code.
    CodeRevision,
    /// Un énoncé formel.
    FormalStatement,
    /// Une preuve formelle.
    FormalProof,
    /// Une revue.
    Review,
    /// Une réponse à une revue.
    Rebuttal,
    /// Une reproduction.
    Reproduction,
    /// Une décision.
    Decision,
    /// Une sélection de corpus.
    CorpusSelection,
    /// Une mesure.
    Measurement,
    /// Une évaluation.
    Evaluation,
    /// Une synthèse.
    Synthesis,
}

impl CoreObjectType {
    /// Les quarante types, dans l'ordre du texte.
    pub const ALL: [Self; 40] = [
        Self::ResearchQuestion,
        Self::Definition,
        Self::Assumption,
        Self::Hypothesis,
        Self::Conjecture,
        Self::Claim,
        Self::Lemma,
        Self::Theorem,
        Self::Interpretation,
        Self::Method,
        Self::Strategy,
        Self::Analogy,
        Self::TransferCertificate,
        Self::Barrier,
        Self::OpenQuestion,
        Self::Objection,
        Self::Counterexample,
        Self::NegativeResult,
        Self::Failure,
        Self::Conflict,
        Self::Inference,
        Self::Experiment,
        Self::Run,
        Self::Source,
        Self::Citation,
        Self::Dataset,
        Self::Artifact,
        Self::Figure,
        Self::Notebook,
        Self::CodeRevision,
        Self::FormalStatement,
        Self::FormalProof,
        Self::Review,
        Self::Rebuttal,
        Self::Reproduction,
        Self::Decision,
        Self::CorpusSelection,
        Self::Measurement,
        Self::Evaluation,
        Self::Synthesis,
    ];

    /// Le nom canonique, celui du texte.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ResearchQuestion => "ResearchQuestion",
            Self::Definition => "Definition",
            Self::Assumption => "Assumption",
            Self::Hypothesis => "Hypothesis",
            Self::Conjecture => "Conjecture",
            Self::Claim => "Claim",
            Self::Lemma => "Lemma",
            Self::Theorem => "Theorem",
            Self::Interpretation => "Interpretation",
            Self::Method => "Method",
            Self::Strategy => "Strategy",
            Self::Analogy => "Analogy",
            Self::TransferCertificate => "TransferCertificate",
            Self::Barrier => "Barrier",
            Self::OpenQuestion => "OpenQuestion",
            Self::Objection => "Objection",
            Self::Counterexample => "Counterexample",
            Self::NegativeResult => "NegativeResult",
            Self::Failure => "Failure",
            Self::Conflict => "Conflict",
            Self::Inference => "Inference",
            Self::Experiment => "Experiment",
            Self::Run => "Run",
            Self::Source => "Source",
            Self::Citation => "Citation",
            Self::Dataset => "Dataset",
            Self::Artifact => "Artifact",
            Self::Figure => "Figure",
            Self::Notebook => "Notebook",
            Self::CodeRevision => "CodeRevision",
            Self::FormalStatement => "FormalStatement",
            Self::FormalProof => "FormalProof",
            Self::Review => "Review",
            Self::Rebuttal => "Rebuttal",
            Self::Reproduction => "Reproduction",
            Self::Decision => "Decision",
            Self::CorpusSelection => "CorpusSelection",
            Self::Measurement => "Measurement",
            Self::Evaluation => "Evaluation",
            Self::Synthesis => "Synthesis",
        }
    }

    /// Lit un nom de type core, ou rend `None`.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == name)
    }
}

impl fmt::Display for CoreObjectType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Le type d'un objet épistémique : core, ou apporté par un pack disciplinaire.
///
/// # Pourquoi une extension ne peut pas porter un nom core
///
/// §7.3 : « les extensions ne doivent pas modifier la signification des types core ». Un pack qui
/// déclarerait son propre `Claim` ne modifierait pas la signification du type core — il la
/// **remplacerait**, silencieusement, pour tous les objets écrits sous ce nom. Le graphe
/// contiendrait alors deux notions de `Claim` qu'aucune lecture ultérieure ne saurait séparer.
///
/// [`ObjectType::parse`] refuse donc une extension homonyme d'un type core. C'est la seule
/// interprétation de la phrase qui reste vraie une fois le pack installé : interdire l'homonymie,
/// plutôt qu'espérer que personne ne s'en serve.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ObjectType {
    /// L'un des quarante types de §7.3.
    Core(CoreObjectType),
    /// Un type apporté par un pack disciplinaire, préfixé par son namespace.
    Extension {
        /// Le namespace du pack — ce qui rend deux extensions homonymes distinguables.
        namespace: String,
        /// Le nom local, dans ce namespace.
        name: String,
    },
}

impl ObjectType {
    /// Lit un type d'objet.
    ///
    /// Un nom nu désigne un type core. Un nom `namespace/nom` désigne une extension.
    ///
    /// # Errors
    ///
    /// Rend [`ParseObjectTypeError`] pour un nom nu inconnu, pour une extension dont le nom local
    /// est celui d'un type core, ou pour une forme vide.
    pub fn parse(text: &str) -> Result<Self, ParseObjectTypeError> {
        let Some((namespace, name)) = text.split_once('/') else {
            return CoreObjectType::parse(text)
                .map(Self::Core)
                .ok_or(ParseObjectTypeError::UnknownCoreType);
        };
        if namespace.is_empty() || name.is_empty() {
            return Err(ParseObjectTypeError::Empty);
        }
        if CoreObjectType::parse(name).is_some() {
            return Err(ParseObjectTypeError::ShadowsCoreType);
        }
        Ok(Self::Extension {
            namespace: namespace.to_owned(),
            name: name.to_owned(),
        })
    }

    /// Vrai pour l'un des quarante types de §7.3.
    #[must_use]
    pub const fn is_core(&self) -> bool {
        matches!(self, Self::Core(_))
    }
}

impl fmt::Display for ObjectType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Core(kind) => formatter.write_str(kind.as_str()),
            Self::Extension { namespace, name } => write!(formatter, "{namespace}/{name}"),
        }
    }
}

impl Serialize for ObjectType {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ObjectType {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = <&str>::deserialize(deserializer)?;
        Self::parse(text).map_err(D::Error::custom)
    }
}

impl Serialize for CoreObjectType {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for CoreObjectType {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = <&str>::deserialize(deserializer)?;
        Self::parse(text).ok_or_else(|| D::Error::custom("type core inconnu"))
    }
}

/// Ce qui peut empêcher de lire un type d'objet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseObjectTypeError {
    /// Un nom nu qui n'est aucun des quarante types core.
    UnknownCoreType,
    /// Une extension dont le nom local est celui d'un type core.
    ShadowsCoreType,
    /// Un namespace ou un nom vide.
    Empty,
}

impl fmt::Display for ParseObjectTypeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::UnknownCoreType => "type core inconnu — une extension s'écrit `namespace/nom`",
            Self::ShadowsCoreType => {
                "une extension ne peut pas porter le nom d'un type core (§7.3)"
            }
            Self::Empty => "namespace ou nom vide",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ParseObjectTypeError {}
