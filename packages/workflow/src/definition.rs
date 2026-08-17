//! Ce qu'un workflow déclare — `docs/SPEC_V1.md` §11.1 et §11.3.

use std::collections::BTreeSet;
use std::fmt;

use locus_domain::StableId;

use crate::kind::WorkflowKind;

/// Une version de workflow.
///
/// §11.3 : « versions de workflow explicites ». D'où l'absence de `Default` : une version implicite
/// est exactement ce que la règle interdit. Un moteur durable rejoue une exécution avec le code de
/// la version sous laquelle elle a démarré ; si la version n'a jamais été dite, il n'y a rien à
/// rejouer et le replay produit un résultat qui n'est pas une reprise mais une réexécution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkflowVersion(u32);

impl WorkflowVersion {
    /// Une version, énoncée.
    #[must_use]
    pub const fn new(number: u32) -> Self {
        Self(number)
    }

    /// Le numéro.
    #[must_use]
    pub const fn number(self) -> u32 {
        self.0
    }
}

impl fmt::Display for WorkflowVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "v{}", self.0)
    }
}

/// Un effet que le monde extérieur observe, ou qui rend deux exécutions différentes.
///
/// §11.3 en nomme quatre : « aucun appel LLM, réseau, filesystem ou horloge non encapsulé dans une
/// activity/step ».
///
/// [`Effect::Random`] est une **addition** au texte, et elle s'assume : un pas qui tire au sort
/// rejoue autrement, ce qui est précisément la panne que la règle existe pour empêcher. Le texte
/// énumère les appels sortants ; l'aléa est le seul non-déterminisme qui ne sorte de nulle part, et
/// l'omettre laisserait passer le plus discret des quatre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Effect {
    /// Un appel de modèle.
    Llm,
    /// Un appel réseau.
    Network,
    /// Une lecture ou une écriture de fichier.
    Filesystem,
    /// Une lecture de l'horloge.
    Clock,
    /// Un tirage au sort. Addition à la liste de §11.3 — voir la documentation du type.
    Random,
}

impl Effect {
    /// Les cinq, dans l'ordre de §11.3 puis l'addition.
    pub const ALL: [Self; 5] = [
        Self::Llm,
        Self::Network,
        Self::Filesystem,
        Self::Clock,
        Self::Random,
    ];

    /// Le nom court de l'effet.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Llm => "llm",
            Self::Network => "network",
            Self::Filesystem => "filesystem",
            Self::Clock => "clock",
            Self::Random => "random",
        }
    }
}

impl fmt::Display for Effect {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

/// Sur quoi repose l'idempotence d'une activity.
///
/// §11.3 : « side effects idempotents ». La règle est courte et c'est ce qui la rend facile à
/// oublier — d'où deux constructeurs et **pas de troisième forme**, comme
/// `locus_migrations::Migration` : une activity qui ne sait dire ni sa clé de déduplication ni
/// pourquoi elle est naturellement rejouable ne se construit pas.
///
/// Ce n'est pas une preuve d'idempotence — aucun type ne la donnerait. C'est l'endroit où
/// quelqu'un y réfléchit une fois, au moment où c'est encore bon marché.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Idempotency {
    basis: Basis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Basis {
    /// Une clé de déduplication : le second appel avec la même clé ne refait rien.
    Key(String),
    /// L'opération est rejouable telle quelle, et voici pourquoi.
    Natural(String),
}

impl Idempotency {
    /// Une clé de déduplication portée par l'appel.
    ///
    /// # Errors
    ///
    /// [`DefinitionError::EmptyName`] si la clé est vide : une clé vide dédoublonne tout avec tout.
    pub fn key(value: &str) -> Result<Self, DefinitionError> {
        reject_blank(value, "clé d'idempotence")?;
        Ok(Self {
            basis: Basis::Key(value.to_owned()),
        })
    }

    /// L'opération est naturellement rejouable, et la raison est écrite.
    ///
    /// # Errors
    ///
    /// [`DefinitionError::EmptyName`] si la raison est vide. « Naturellement idempotent » sans
    /// justification est une affirmation que personne n'a vérifiée, et elle a le même air que
    /// celle qui l'a été.
    pub fn natural(rationale: &str) -> Result<Self, DefinitionError> {
        reject_blank(rationale, "raison d'idempotence naturelle")?;
        Ok(Self {
            basis: Basis::Natural(rationale.to_owned()),
        })
    }

    /// La clé de déduplication, quand l'idempotence en repose sur une.
    #[must_use]
    pub fn dedup_key(&self) -> Option<&str> {
        match &self.basis {
            Basis::Key(key) => Some(key),
            Basis::Natural(_) => None,
        }
    }

    /// La raison, quand l'idempotence est naturelle.
    #[must_use]
    pub fn rationale(&self) -> Option<&str> {
        match &self.basis {
            Basis::Key(_) => None,
            Basis::Natural(rationale) => Some(rationale),
        }
    }
}

/// La frontière durable où un effet a le droit d'exister.
///
/// Tout ce qui sort du processus vit ici, et nulle part ailleurs : c'est la première règle de
/// §11.3, et elle est portée par la **forme** de [`Step`] — un pas déterministe n'a pas de champ
/// où loger un effet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Activity {
    name: String,
    effects: BTreeSet<Effect>,
    idempotency: Idempotency,
}

impl Activity {
    /// Déclarer une activity.
    ///
    /// Les effets peuvent être vides : une frontière durable sert aussi aux reprises et aux
    /// délais, pas seulement aux appels sortants. Ce qui ne peut pas être vide, c'est
    /// l'idempotence — et un nom qui annonce un effet non déclaré est signalé par
    /// [`crate::determinism::definition_findings`], parce que le type ne peut pas voir dans le
    /// corps du pas ce que son nom dit tout haut.
    ///
    /// # Errors
    ///
    /// [`DefinitionError::EmptyName`] ou [`DefinitionError::NameWithWhitespace`] selon le nom.
    pub fn new(
        name: &str,
        effects: impl IntoIterator<Item = Effect>,
        idempotency: Idempotency,
    ) -> Result<Self, DefinitionError> {
        reject_blank(name, "nom d'activity")?;
        reject_whitespace(name)?;
        Ok(Self {
            name: name.to_owned(),
            effects: effects.into_iter().collect(),
            idempotency,
        })
    }

    /// Le nom du pas.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Les effets déclarés.
    #[must_use]
    pub const fn effects(&self) -> &BTreeSet<Effect> {
        &self.effects
    }

    /// Sur quoi repose son idempotence.
    #[must_use]
    pub const fn idempotency(&self) -> &Idempotency {
        &self.idempotency
    }
}

/// Un pas de workflow.
///
/// Deux formes, et la distinction est le cœur de §11.3 : un pas déterministe **n'a pas de champ**
/// pour un effet. Ce n'est pas une convention qu'on peut contourner en écrivant le contraire — la
/// faute ne s'exprime pas dans le type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// De la logique pure, rejouable à volonté.
    Deterministic {
        /// Le nom du pas.
        name: String,
    },
    /// Une frontière durable, seul endroit où un effet a lieu.
    Activity(Activity),
}

impl Step {
    /// Un pas déterministe.
    ///
    /// # Errors
    ///
    /// [`DefinitionError::EmptyName`] ou [`DefinitionError::NameWithWhitespace`] selon le nom.
    pub fn deterministic(name: &str) -> Result<Self, DefinitionError> {
        reject_blank(name, "nom de pas")?;
        reject_whitespace(name)?;
        Ok(Self::Deterministic {
            name: name.to_owned(),
        })
    }

    /// Le nom du pas, quelle que soit sa forme.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Deterministic { name } => name,
            Self::Activity(activity) => activity.name(),
        }
    }
}

/// La définition d'un workflow, indépendante du moteur qui l'exécutera.
///
/// §11.1 : Locus Solus « ne code aucun invariant métier directement contre Temporal ». Ce type est
/// l'endroit où l'on peut vérifier cette phrase : il ne connaît aucun backend, et W3.b lui en
/// donnera un sans le modifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowDefinition {
    kind: WorkflowKind,
    version: WorkflowVersion,
    subject: Vec<StableId>,
    steps: Vec<Step>,
}

impl WorkflowDefinition {
    /// Déclarer un workflow.
    ///
    /// `subject` porte les identifiants métier des objets que le workflow fait avancer, et il ne
    /// peut pas être vide : §11.3 exige que les « IDs métier [soient] créés **avant** l'entrée dans
    /// le backend de workflow ». Les exiger à la construction est la forme opposable de cette
    /// phrase — un workflow qui frapperait ses identifiants en chemin en produirait de neufs à
    /// chaque replay, et l'objet scientifique changerait d'identité en étant simplement rejoué.
    ///
    /// # Errors
    ///
    /// [`DefinitionError::NoSteps`] pour une définition sans pas, [`DefinitionError::NoSubject`]
    /// pour un sujet vide, [`DefinitionError::DuplicateStep`] si deux pas portent le même nom —
    /// un historique de replay les rendrait indiscernables — et les erreurs de nom des pas.
    pub fn new(
        kind: WorkflowKind,
        version: WorkflowVersion,
        subject: Vec<StableId>,
        steps: Vec<Step>,
    ) -> Result<Self, DefinitionError> {
        if steps.is_empty() {
            return Err(DefinitionError::NoSteps);
        }
        if subject.is_empty() {
            return Err(DefinitionError::NoSubject);
        }
        let mut seen = BTreeSet::new();
        for step in &steps {
            reject_blank(step.name(), "nom de pas")?;
            reject_whitespace(step.name())?;
            if !seen.insert(step.name().to_owned()) {
                return Err(DefinitionError::DuplicateStep {
                    name: step.name().to_owned(),
                });
            }
        }
        Ok(Self {
            kind,
            version,
            subject,
            steps,
        })
    }

    /// Lequel des onze.
    #[must_use]
    pub const fn kind(&self) -> WorkflowKind {
        self.kind
    }

    /// Sa version, énoncée.
    #[must_use]
    pub const fn version(&self) -> WorkflowVersion {
        self.version
    }

    /// Les identifiants métier, frappés avant l'entrée dans le moteur.
    #[must_use]
    pub fn subject(&self) -> &[StableId] {
        &self.subject
    }

    /// Ses pas, dans l'ordre.
    #[must_use]
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    /// Les activities, dans l'ordre.
    pub fn activities(&self) -> impl Iterator<Item = &Activity> {
        self.steps.iter().filter_map(|step| match step {
            Step::Activity(activity) => Some(activity),
            Step::Deterministic { .. } => None,
        })
    }
}

/// Ce qui empêche une définition d'exister.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefinitionError {
    /// Un texte obligatoire est vide.
    EmptyName {
        /// Ce qui manquait.
        what: &'static str,
    },
    /// Un nom porte une espace : il ne survivrait pas à un identifiant de moteur.
    NameWithWhitespace {
        /// Le nom fautif.
        name: String,
    },
    /// Deux pas portent le même nom.
    DuplicateStep {
        /// Le nom en double.
        name: String,
    },
    /// Une définition sans pas.
    NoSteps,
    /// Une définition sans identifiant métier.
    NoSubject,
}

impl fmt::Display for DefinitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName { what } => write!(formatter, "{what} vide"),
            Self::NameWithWhitespace { name } => {
                write!(formatter, "le nom « {name} » porte une espace")
            }
            Self::DuplicateStep { name } => write!(
                formatter,
                "deux pas s'appellent « {name} » : un replay ne saurait pas les distinguer"
            ),
            Self::NoSteps => formatter.write_str("une définition de workflow sans pas"),
            Self::NoSubject => formatter.write_str(
                "aucun identifiant métier : §11.3 les veut créés avant l'entrée dans le backend",
            ),
        }
    }
}

impl std::error::Error for DefinitionError {}

fn reject_blank(value: &str, what: &'static str) -> Result<(), DefinitionError> {
    if value.trim().is_empty() {
        return Err(DefinitionError::EmptyName { what });
    }
    Ok(())
}

fn reject_whitespace(name: &str) -> Result<(), DefinitionError> {
    if name.chars().any(char::is_whitespace) {
        return Err(DefinitionError::NameWithWhitespace {
            name: name.to_owned(),
        });
    }
    Ok(())
}
