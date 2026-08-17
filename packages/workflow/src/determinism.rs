//! Les règles de déterminisme — `docs/SPEC_V1.md` §11.3.

use crate::definition::{Effect, Step, WorkflowDefinition};

/// Une des six règles de §11.3.
///
/// Les six sont transcrites plutôt que résumées, et l'enum les rend dénombrables : une règle qu'on
/// laisse tomber devient une variante supprimée, donc un test rouge, et non un paragraphe que
/// personne ne relit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rule {
    /// « aucun appel LLM, réseau, filesystem ou horloge non encapsulé dans une activity/step ».
    EffectsEncapsulated,
    /// « IDs métier créés avant l'entrée dans le backend de workflow ».
    IdsBeforeEntry,
    /// « side effects idempotents ».
    IdempotentSideEffects,
    /// « versions de workflow explicites ».
    ExplicitVersions,
    /// « tests de replay pour les versions supportées ».
    ReplayTests,
    /// « migrations contrôlées des workflows longue durée ».
    ControlledMigrations,
}

/// Comment une règle tient.
///
/// La distinction n'est pas cosmétique : une règle tenue par le type ne peut pas être violée, une
/// règle tenue par un filet peut l'être sans que le filet le voie, et une règle tenue par un
/// décompte ne dit rien tant que personne ne lit le décompte. Les confondre reviendrait à croire
/// que les six règles ont la même force.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Enforcement {
    /// Le type rend la faute impossible à écrire.
    ByConstruction,
    /// Un filet la cherche dans ce qui est déclaré, faute de pouvoir la rendre impossible.
    ByNet,
    /// Un décompte la rend visible : ce qui manque est signalé, jamais simplement absent.
    ByCoverage,
}

impl Rule {
    /// Les six, dans l'ordre de §11.3.
    pub const ALL: [Self; 6] = [
        Self::EffectsEncapsulated,
        Self::IdsBeforeEntry,
        Self::IdempotentSideEffects,
        Self::ExplicitVersions,
        Self::ReplayTests,
        Self::ControlledMigrations,
    ];

    /// La règle, telle que §11.3 l'écrit.
    #[must_use]
    pub const fn statement(self) -> &'static str {
        match self {
            Self::EffectsEncapsulated => {
                "aucun appel LLM, réseau, filesystem ou horloge non encapsulé dans une activity/step"
            }
            Self::IdsBeforeEntry => "IDs métier créés avant l'entrée dans le backend de workflow",
            Self::IdempotentSideEffects => "side effects idempotents",
            Self::ExplicitVersions => "versions de workflow explicites",
            Self::ReplayTests => "tests de replay pour les versions supportées",
            Self::ControlledMigrations => "migrations contrôlées des workflows longue durée",
        }
    }

    /// Par quoi la règle tient, dans ce paquet.
    ///
    /// Deux mécanismes pour la première : la forme de [`Step`] empêche de **déclarer** un effet
    /// hors activity, et [`definition_findings`] cherche celui qu'un nom trahit. Aucun des deux ne
    /// voit le corps du pas — ce que voit W3.b, quand un moteur existera pour l'exécuter.
    #[must_use]
    pub const fn enforcement(self) -> &'static [Enforcement] {
        // Les arms sont groupés par **force** et non dans l'ordre de §11.3 : ce qui se lit ici est
        // l'inégalité des gardes, et c'est elle qui compte au moment de décider laquelle croire.
        match self {
            Self::EffectsEncapsulated | Self::IdsBeforeEntry => {
                &[Enforcement::ByConstruction, Enforcement::ByNet]
            }
            Self::IdempotentSideEffects | Self::ExplicitVersions | Self::ControlledMigrations => {
                &[Enforcement::ByConstruction]
            }
            Self::ReplayTests => &[Enforcement::ByCoverage],
        }
    }
}

/// Les marqueurs qui trahissent un effet dans un nom de pas.
///
/// # Pourquoi les noms
///
/// Le type interdit de **déclarer** un effet hors activity ; il ne peut pas voir ce que le pas
/// fera. Un pas nommé `fetch_manifest` et déclaré déterministe est un aveu écrit, et c'est la
/// seule trace qu'une définition — de la donnée, pas du code — puisse en garder.
///
/// Les marqueurs sont anglais parce que les identifiants du dépôt le sont, la prose restant
/// française. Un filet qui chercherait `recuperer_` ne trouverait rien dans le code réel.
pub const EFFECT_MARKERS: [(&str, Effect); 24] = [
    ("llm", Effect::Llm),
    ("prompt", Effect::Llm),
    ("completion", Effect::Llm),
    ("embedding", Effect::Llm),
    ("inference", Effect::Llm),
    ("fetch", Effect::Network),
    ("http", Effect::Network),
    ("download", Effect::Network),
    ("upload", Effect::Network),
    ("publish", Effect::Network),
    ("notify", Effect::Network),
    ("file", Effect::Filesystem),
    ("disk", Effect::Filesystem),
    ("spool", Effect::Filesystem),
    ("tmp", Effect::Filesystem),
    ("now", Effect::Clock),
    ("sleep", Effect::Clock),
    ("elapsed", Effect::Clock),
    ("wall_clock", Effect::Clock),
    ("random", Effect::Random),
    ("uuid", Effect::Random),
    ("ulid", Effect::Random),
    ("shuffle", Effect::Random),
    ("nonce", Effect::Random),
];

/// Ce qu'un nom de pas laisse voir.
///
/// La comparaison porte sur les **jetons** du nom, pas sur ses sous-chaînes : `known` contient
/// `now` sans lire l'horloge, et un filet qui crie sur `known_inputs` serait désarmé au premier
/// agacement. Un marqueur composé — `wall_clock` — se cherche comme une suite de jetons contiguë.
#[must_use]
pub fn suspected_effects(name: &str) -> Vec<(&'static str, Effect)> {
    let tokens: Vec<String> = name
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_lowercase)
        .collect();

    let mut found = Vec::new();
    for (marker, effect) in EFFECT_MARKERS {
        let needle: Vec<&str> = marker.split('_').collect();
        let matched = tokens.windows(needle.len()).any(|window| {
            window
                .iter()
                .zip(&needle)
                .all(|(token, part)| token == part)
        });
        if matched {
            found.push((marker, effect));
        }
    }
    found
}

/// Ce qu'un balayage de définition a trouvé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeterminismFinding {
    /// Un pas déclaré déterministe dont le nom annonce un effet.
    UnencapsulatedEffect {
        /// Le pas.
        step: String,
        /// Le marqueur trouvé.
        marker: &'static str,
        /// L'effet qu'il trahit.
        effect: Effect,
    },
    /// Une activity dont le nom annonce un effet qu'elle n'a pas déclaré.
    ///
    /// Moins grave que la précédente — l'effet a lieu au bon endroit — mais une déclaration
    /// incomplète fausse tout ce qui se décide à partir d'elle : les retries, les timeouts, et ce
    /// qu'un audit croit savoir de ce que le workflow touche.
    UndeclaredEffect {
        /// L'activity.
        activity: String,
        /// Le marqueur trouvé.
        marker: &'static str,
        /// L'effet non déclaré.
        effect: Effect,
    },
}

impl DeterminismFinding {
    /// La règle de §11.3 que le constat met en cause.
    #[must_use]
    pub const fn rule(&self) -> Rule {
        Rule::EffectsEncapsulated
    }
}

/// Balayer une définition.
///
/// Rend des constats plutôt que de lever : le public est un test, et une liste complète se corrige
/// mieux qu'une erreur à la fois.
#[must_use]
pub fn definition_findings(definition: &WorkflowDefinition) -> Vec<DeterminismFinding> {
    let mut findings = Vec::new();
    for step in definition.steps() {
        match step {
            Step::Deterministic { name } => {
                findings.extend(suspected_effects(name).into_iter().map(|(marker, effect)| {
                    DeterminismFinding::UnencapsulatedEffect {
                        step: name.clone(),
                        marker,
                        effect,
                    }
                }));
            }
            Step::Activity(activity) => {
                findings.extend(
                    suspected_effects(activity.name())
                        .into_iter()
                        .filter(|(_, effect)| !activity.effects().contains(effect))
                        .map(|(marker, effect)| DeterminismFinding::UndeclaredEffect {
                            activity: activity.name().to_owned(),
                            marker,
                            effect,
                        }),
                );
            }
        }
    }
    findings
}

/// Les marqueurs d'un identifiant frappé à l'exécution.
///
/// §11.3, deuxième règle : les IDs métier sont « créés **avant** l'entrée dans le backend de
/// workflow ». Un workflow qui frappe un identifiant en chemin en produit un neuf à chaque replay,
/// et l'objet scientifique change d'identité en étant simplement rejoué — le contraire de ce que
/// §11.2 demande, « rejoué ou repris avec un autre backend **sans changer l'identité des objets
/// scientifiques** ».
///
/// # Pourquoi les marqueurs sont assemblés
///
/// Le balayage passe aussi sur ce fichier. Une table écrite d'un bloc se signalerait elle-même, et
/// la façon habituelle de s'en sortir — exclure le fichier qui porte la table — ouvrirait dans la
/// garde exactement le trou qu'elle est censée fermer. `concat!` produit le marqueur à la
/// compilation sans que la ligne source le contienne.
///
/// La comparaison est **sensible à la casse** : ce sont des identifiants Rust, et `SystemTime` ne
/// se confond avec rien.
pub const MINTING_MARKERS: [&str; 9] = [
    concat!("from", "_parts"),
    concat!("System", "Time"),
    concat!("Instant", "::now"),
    concat!("Utc", "::now"),
    concat!("thread", "_rng"),
    concat!("get", "random"),
    concat!("Uuid", "::new"),
    concat!("Ulid", "::new"),
    concat!("rand", "::"),
];

/// Un identifiant frappé là où il ne devrait pas l'être.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MintingFinding {
    /// Où.
    pub location: String,
    /// Le marqueur trouvé.
    pub marker: &'static str,
    /// La ligne fautive.
    pub line: String,
}

impl MintingFinding {
    /// La règle de §11.3 que le constat met en cause.
    #[must_use]
    pub const fn rule(&self) -> Rule {
        Rule::IdsBeforeEntry
    }
}

/// Chercher une frappe d'identifiant dans un texte source.
///
/// Les lignes de commentaire sont ignorées : nommer `SystemTime` pour dire qu'on ne s'en sert pas
/// n'est pas s'en servir, et l'inverse ferait échouer la garde sur sa propre documentation.
///
/// Le balayage vise les **sources** du paquet, pas ses tests : un test qui fabrique un identifiant
/// de fixture fait le contraire du danger — il fixe une valeur au lieu d'en tirer une neuve à
/// chaque exécution.
#[must_use]
pub fn minting_findings(location: &str, source: &str) -> Vec<MintingFinding> {
    let mut findings = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") || trimmed.starts_with('*') || trimmed.starts_with('#') {
            continue;
        }
        for marker in MINTING_MARKERS {
            if line.contains(marker) {
                findings.push(MintingFinding {
                    location: location.to_owned(),
                    marker,
                    line: line.trim().to_owned(),
                });
            }
        }
    }
    findings
}
