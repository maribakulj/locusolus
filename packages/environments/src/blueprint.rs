//! L'`EnvironmentBlueprint` — `docs/SPEC_V1.md` §19.3, §19.4, §19.7, §21.8.
//!
//! # Ce que ce type ajoute au schéma
//!
//! `schemas/environments/1.0/environment-blueprint.schema.json` existe depuis W0.5 et refuse déjà
//! beaucoup : champs obligatoires, digest bien formé, mode réseau connu. Un schéma JSON ne sait pas
//! exprimer les invariants **entre** champs, ni ceux qui portent sur des valeurs qu'il ne peut que
//! typer. Ce type porte les trois que le schéma laisse passer :
//!
//! 1. un profil de toolchain répété — l'ordre de composition décide de ce qui écrase quoi, et deux
//!    occurrences d'un même profil rendent le résultat dépendant de l'implémentation du builder ;
//! 2. un préféré inférieur au minimum — le blueprint dirait alors que l'exécution *préfère* moins
//!    que ce qu'elle exige, et le scheduler choisirait selon celui des deux qu'il lit ;
//! 3. une variable d'environnement qui porte un secret — le schéma dit lui-même qu'il ne peut que
//!    « refuser de prévoir une place » ; ici on peut refuser la valeur, par deux tables distinctes
//!    dont [`SECRET_NAME_MARKERS`] explique pourquoi elles ne se fondent pas.
//!
//! # Ce qu'il ne fait pas
//!
//! Construire l'image. Le blueprint **déclare** ; W5.b construit, scanne, signe et attache le
//! digest. Un type qui saurait faire les deux ferait de la déclaration un effet de bord de la
//! construction, et un environnement ne serait plus descriptible avant d'exister.

use std::collections::BTreeSet;
use std::fmt;

use locus_execution::{ResourceSpec, secret_marker};

use crate::toolchain::ToolchainProfile;

/// Les fragments qui, dans un **nom** de variable, annoncent un secret.
///
/// # Pourquoi une seconde table, et pourquoi ce n'est pas une duplication
///
/// `locus_execution::SECRET_MARKERS` répond à une autre question : « cette **preuve** d'événement
/// de sécurité porte-t-elle un secret ? ». Elle vise des valeurs — `AKIA…`, `Bearer …`,
/// `api_key=…` — et elle a raison de ne pas contenir `token` : une preuve qui dit « le token de
/// session a expiré » est une preuve légitime, et la refuser rendrait les événements de sécurité
/// inécrivables.
///
/// Ici la question est « ce **nom** de variable annonce-t-il qu'on s'apprête à mettre un secret
/// dans un blueprint ? », et `HF_TOKEN` y répond oui. Deux surfaces, deux tables ; les fondre
/// obligerait l'une des deux à mentir. La table des valeurs reste employée telle quelle sur la
/// valeur — c'est la composition des deux qui couvre le cas.
///
/// La comparaison est faite en minuscules : un nom de variable n'est pas un préfixe de valeur, et
/// `hf_token` doit être refusé comme `HF_TOKEN`.
pub const SECRET_NAME_MARKERS: [&str; 7] = [
    concat!("tok", "en"),
    concat!("sec", "ret"),
    concat!("pass", "word"),
    concat!("pass", "phrase"),
    concat!("cred", "ential"),
    concat!("api", "key"),
    concat!("private", "key"),
];

/// Le marqueur qu'un nom de variable porte, s'il en porte un.
///
/// # Pourquoi la comparaison porte sur des segments et non sur la chaîne entière
///
/// Le nom est découpé en segments — sur les séparateurs et sur les changements de casse — puis
/// chaque marqueur est comparé aux segments et à leurs **concaténations contiguës**. `API_KEY`,
/// `api-key`, `apikey` et `ApiKey` donnent tous `api` + `key`, donc `apikey` : les quatre
/// orthographes disent la même chose et ne pas les reconnaître toutes laisserait passer les autres.
///
/// La comparaison par sous-chaîne, essayée d'abord, refusait `TOKENIZERS_PARALLELISM` — une
/// variable `HuggingFace` parfaitement ordinaire — parce qu'elle contient `token`. Un garde qui
/// refuse des noms légitimes se fait désactiver, et un garde désactivé ne garde rien.
#[must_use]
pub fn secret_name_marker(name: &str) -> Option<&'static str> {
    let segments = segments(name);
    SECRET_NAME_MARKERS.into_iter().find(|marker| {
        (0..segments.len())
            .any(|from| (from..segments.len()).any(|to| segments[from..=to].concat() == **marker))
    })
}

/// Découper `OPENAI_API_KEY`, `openai-api-key` et `openAiApiKey` en `["openai", "api", "key"]`.
fn segments(name: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut previous_lower = false;
    for character in name.chars() {
        if !character.is_ascii_alphanumeric() {
            if !current.is_empty() {
                segments.push(std::mem::take(&mut current));
            }
            previous_lower = false;
            continue;
        }
        if character.is_ascii_uppercase() && previous_lower && !current.is_empty() {
            segments.push(std::mem::take(&mut current));
        }
        previous_lower = character.is_ascii_lowercase() || character.is_ascii_digit();
        current.push(character.to_ascii_lowercase());
    }
    if !current.is_empty() {
        segments.push(current);
    }
    segments
}

/// L'image, désignée par digest.
///
/// §21.8 : par digest, jamais par tag. Un tag est mutable, et un environnement dont l'image peut
/// changer sous lui n'est pas verrouillé — donc ne tient pas le niveau `R2` de §19.7, qui est
/// « environnement verrouillé ».
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    reference: Option<String>,
    digest: String,
}

impl Image {
    /// Déclarer une image par son digest.
    ///
    /// # Errors
    ///
    /// [`BlueprintError::MalformedDigest`] pour un digest qui n'est pas un `sha256:` suivi de
    /// soixante-quatre caractères hexadécimaux.
    pub fn new(digest: &str, reference: Option<&str>) -> Result<Self, BlueprintError> {
        let hex = digest
            .strip_prefix("sha256:")
            .filter(|hex| hex.len() == 64 && hex.chars().all(|char| char.is_ascii_hexdigit()));
        if hex.is_none() {
            return Err(BlueprintError::MalformedDigest {
                digest: digest.to_owned(),
            });
        }
        Ok(Self {
            reference: reference.map(str::to_owned),
            digest: digest.to_owned(),
        })
    }

    /// Le digest.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// La référence lisible, quand il y en a une. Elle documente, elle ne désigne pas.
    #[must_use]
    pub fn reference(&self) -> Option<&str> {
        self.reference.as_deref()
    }
}

/// Ce que l'environnement demande en ressources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requirements {
    minimum: ResourceSpec,
    preferred: Option<ResourceSpec>,
}

impl Requirements {
    /// Déclarer un minimum seul.
    #[must_use]
    pub const fn minimum(minimum: ResourceSpec) -> Self {
        Self {
            minimum,
            preferred: None,
        }
    }

    /// Ajouter un préféré.
    ///
    /// # Errors
    ///
    /// [`BlueprintError::PreferredBelowMinimum`] quand le préféré ne contient pas le minimum. Le
    /// blueprint dirait alors que l'exécution préfère moins qu'elle n'exige, et le placement
    /// choisirait selon celui des deux qu'il lit en premier.
    pub fn preferring(mut self, preferred: ResourceSpec) -> Result<Self, BlueprintError> {
        if !self.minimum.quotas_fit_within(&preferred) {
            return Err(BlueprintError::PreferredBelowMinimum);
        }
        self.preferred = Some(preferred);
        Ok(self)
    }

    /// Ce qu'il faut au minimum.
    #[must_use]
    pub const fn required(&self) -> &ResourceSpec {
        &self.minimum
    }

    /// Ce qui serait mieux, s'il a été déclaré.
    #[must_use]
    pub const fn preferred(&self) -> Option<&ResourceSpec> {
        self.preferred.as_ref()
    }
}

/// Ce qu'un environnement déclare — §19.3.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentBlueprint {
    environment_id: String,
    version: String,
    toolchains: Vec<ToolchainProfile>,
    image: Image,
    resources: Requirements,
    env: Vec<(String, String)>,
}

impl EnvironmentBlueprint {
    /// Déclarer un environnement.
    ///
    /// # Errors
    ///
    /// [`BlueprintError::EmptyIdentity`] pour un identifiant ou une version vides — §19.7 fait de
    /// la version la condition du niveau `R2`, et une version vide ne verrouille rien ;
    /// [`BlueprintError::NoToolchain`] pour une liste vide ;
    /// [`BlueprintError::DuplicateToolchain`] pour un profil répété.
    pub fn new(
        environment_id: &str,
        version: &str,
        toolchains: Vec<ToolchainProfile>,
        image: Image,
        resources: Requirements,
    ) -> Result<Self, BlueprintError> {
        if environment_id.trim().is_empty() || version.trim().is_empty() {
            return Err(BlueprintError::EmptyIdentity);
        }
        if toolchains.is_empty() {
            return Err(BlueprintError::NoToolchain);
        }
        let mut seen = BTreeSet::new();
        if let Some(repeated) = toolchains
            .iter()
            .find(|profile| !seen.insert(**profile))
            .copied()
        {
            return Err(BlueprintError::DuplicateToolchain { profile: repeated });
        }
        Ok(Self {
            environment_id: environment_id.to_owned(),
            version: version.to_owned(),
            toolchains,
            image,
            resources,
            env: Vec::new(),
        })
    }

    /// Ajouter une variable d'environnement **non secrète**.
    ///
    /// # Errors
    ///
    /// [`BlueprintError::SecretInEnvironment`] quand le **nom** annonce un secret
    /// ([`SECRET_NAME_MARKERS`]) ou que la **valeur** en porte un
    /// (`locus_execution::secret_marker`). Le schéma dit qu'il « ne peut pas empêcher d'y mettre un
    /// token, mais peut refuser de prévoir une place pour en mettre un » ; ici on peut refuser la
    /// valeur, et les deux tables répondent à deux questions distinctes — voir
    /// [`SECRET_NAME_MARKERS`].
    pub fn with_variable(mut self, name: &str, value: &str) -> Result<Self, BlueprintError> {
        let marker = secret_name_marker(name).or_else(|| secret_marker(value));
        if let Some(marker) = marker {
            return Err(BlueprintError::SecretInEnvironment {
                name: name.to_owned(),
                marker,
            });
        }
        self.env.push((name.to_owned(), value.to_owned()));
        Ok(self)
    }

    /// L'identifiant.
    #[must_use]
    pub fn environment_id(&self) -> &str {
        &self.environment_id
    }

    /// La version.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Les profils, dans l'ordre de composition.
    #[must_use]
    pub fn toolchains(&self) -> &[ToolchainProfile] {
        &self.toolchains
    }

    /// L'image.
    #[must_use]
    pub const fn image(&self) -> &Image {
        &self.image
    }

    /// Les ressources.
    #[must_use]
    pub const fn resources(&self) -> &Requirements {
        &self.resources
    }

    /// Les variables non secrètes.
    #[must_use]
    pub fn variables(&self) -> &[(String, String)] {
        &self.env
    }

    /// Vrai quand un profil natif interdit d'emporter cet environnement en conteneur.
    ///
    /// Le pendant, côté environnement, de la portée d'accélérateur de W4.f. Un blueprint qui
    /// compose `ml-mps` décrit une machine, pas une image : `docs/03` en tire le worker de
    /// confiance séparé.
    #[must_use]
    pub fn is_native_only(&self) -> bool {
        self.toolchains
            .iter()
            .any(|profile| profile.is_native_only())
    }
}

/// Ce qui empêche un blueprint d'exister.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlueprintError {
    /// Un identifiant ou une version vides.
    EmptyIdentity,
    /// Aucun profil de toolchain.
    NoToolchain,
    /// Un profil répété.
    DuplicateToolchain {
        /// Le profil en double.
        profile: ToolchainProfile,
    },
    /// Un digest mal formé, ou une image désignée par tag.
    MalformedDigest {
        /// Ce qui a été donné.
        digest: String,
    },
    /// Un préféré qui ne contient pas le minimum.
    PreferredBelowMinimum,
    /// Une variable qui porte un secret.
    SecretInEnvironment {
        /// Le nom de la variable.
        name: String,
        /// Le marqueur reconnu.
        marker: &'static str,
    },
}

impl fmt::Display for BlueprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIdentity => formatter
                .write_str("un environnement sans identifiant ni version ne verrouille rien"),
            Self::NoToolchain => {
                formatter.write_str("un environnement sans profil ne décrit aucune image")
            }
            Self::DuplicateToolchain { profile } => write!(
                formatter,
                "le profil « {profile} » est composé deux fois : l'ordre déciderait de ce qui écrase quoi"
            ),
            Self::MalformedDigest { digest } => write!(
                formatter,
                "« {digest} » n'est pas un digest sha256 : une image par tag peut changer sous l'environnement"
            ),
            Self::PreferredBelowMinimum => formatter
                .write_str("le préféré ne contient pas le minimum : l'environnement préférerait moins qu'il n'exige"),
            Self::SecretInEnvironment { name, marker } => write!(
                formatter,
                "la variable « {name} » porte « {marker} » : les variables d'un blueprint sont non secrètes"
            ),
        }
    }
}

impl std::error::Error for BlueprintError {}
