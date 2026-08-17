//! Le driver de build — `docs/SPEC_V1.md` §19.5, ADR 0004.
//!
//! # Pourquoi il est ici et non dans `packages/environments`
//!
//! Construire une image est un acte de runtime : ça lance `buildah`, ça écrit dans le stockage de
//! conteneurs, ça peut pousser vers un registre. ADR 0004 réserve ces actes à `locus-execd`, et la
//! raison vaut autant pour le build que pour l'exécution — un processus qui sait construire une
//! image sait produire celle qu'il veut.
//!
//! `packages/environments` garde donc le vocabulaire et la chaîne ; ce module lance le premier
//! maillon.
//!
//! # Ce que le driver ne peut pas faire, et par construction
//!
//! Aller plus loin que [`Built`]. La chaîne de W5.b est une suite de types, et
//! [`BuildDriver::build`] rend son deuxième état : le SBOM, le scan, les tests et la signature
//! viennent d'autres outils, et aucun raccourci ne mène d'ici à une image publiée. Ce n'est pas une
//! discipline à tenir, c'est ce que les types permettent.
//!
//! # Le réseau du build n'est pas celui d'une mission
//!
//! §19.5 : « les extensions de dépendances demandées par un agent déclenchent un build séparé
//! **avec réseau autorisé**, lockfile, SBOM, scan, tests, signature et publication par digest. Une
//! mission standard ne peut pas `curl | bash` ». Le build résout des dépendances, donc il sort ;
//! c'est précisément pour cela qu'il est séparé de la mission, et qu'il finit par un scan.

use std::fmt;

use locus_environments::{Built, EnvironmentBlueprint, Locked};

use crate::linux::driver::Runner;

/// Où sont les fichiers à partir desquels l'image se construit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildContext {
    /// Le répertoire de contexte.
    pub directory: String,
    /// Le fichier d'instructions, relatif au contexte.
    pub containerfile: String,
}

/// Les arguments de `podman build` qui construisent ce blueprint.
///
/// # Ce que les arguments portent, et ce qu'ils ne portent pas
///
/// Ils portent les variables **non secrètes** du blueprint — `EnvironmentBlueprint::with_variable`
/// a déjà refusé les autres — et deux étiquettes qui inscrivent l'identité et la version dans
/// l'image elle-même : une image retrouvée sans son blueprint doit pouvoir dire de quoi elle est la
/// construction.
///
/// Ils ne portent **pas** le digest du blueprint. Le digest est ce que le build produit ; le passer
/// en entrée reviendrait à demander au build de confirmer ce qu'on savait déjà, c'est-à-dire à
/// n'attester de rien.
#[must_use]
pub fn build_arguments(blueprint: &EnvironmentBlueprint, context: &BuildContext) -> Vec<String> {
    let mut arguments = vec![
        "build".to_owned(),
        "--network=host".to_owned(),
        "--file".to_owned(),
        context.containerfile.clone(),
        "--label".to_owned(),
        format!("locus.environment={}", blueprint.environment_id()),
        "--label".to_owned(),
        format!("locus.version={}", blueprint.version()),
    ];
    for profile in blueprint.toolchains() {
        arguments.push("--build-arg".to_owned());
        arguments.push(format!(
            "LOCUS_TOOLCHAIN_{}=1",
            profile.slug().replace('-', "_")
        ));
    }
    for (name, value) in blueprint.variables() {
        arguments.push("--build-arg".to_owned());
        arguments.push(format!("{name}={value}"));
    }
    arguments.push(context.directory.clone());
    arguments
}

/// Le driver de build.
pub struct BuildDriver<R: Runner> {
    runner: R,
}

impl<R: Runner> BuildDriver<R> {
    /// Construire le driver.
    pub const fn new(runner: R) -> Self {
        Self { runner }
    }

    /// Le lanceur, pour qu'un test puisse lire ce qui lui a été demandé.
    pub const fn runner(&self) -> &R {
        &self.runner
    }

    /// Lancer le build, et rendre le deuxième état de la chaîne.
    ///
    /// # Le digest vient du runtime, jamais du blueprint
    ///
    /// Même règle qu'en W4.d.2 pour l'attestation : composer le digest à partir de ce qu'on
    /// attendait attesterait de sa propre attente. Ici il est lu sur la sortie du build.
    ///
    /// # Errors
    ///
    /// [`BuildDriverError::Runtime`] quand `podman` ne répond pas ou rend un code non nul, et
    /// [`BuildDriverError::NoDigest`] quand il réussit sans nommer de digest — un succès muet n'est
    /// pas un succès, et prendre le silence pour l'image attendue serait la même faute qu'ailleurs.
    pub fn build(&self, locked: Locked, context: &BuildContext) -> Result<Built, BuildDriverError> {
        let arguments = build_arguments(locked.blueprint(), context);
        let execution = self
            .runner
            .run(&arguments)
            .map_err(|error| BuildDriverError::Runtime {
                detail: error.to_string(),
            })?;
        if execution.code != 0 {
            return Err(BuildDriverError::Runtime {
                detail: format!(
                    "podman build a rendu {} : {}",
                    execution.code,
                    execution.stderr.trim()
                ),
            });
        }
        let digest = execution
            .stdout
            .lines()
            .rev()
            .map(str::trim)
            .find(|line| line.starts_with("sha256:"))
            .ok_or(BuildDriverError::NoDigest)?;
        Ok(locked.built(digest))
    }
}

/// Ce qui empêche un build d'aboutir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildDriverError {
    /// Le runtime n'a pas répondu, ou a répondu par un échec.
    Runtime {
        /// Ce qu'il a dit.
        detail: String,
    },
    /// Le build a réussi sans nommer de digest.
    NoDigest,
}

impl fmt::Display for BuildDriverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Runtime { detail } => write!(formatter, "build : {detail}"),
            Self::NoDigest => formatter.write_str(
                "le build a réussi sans nommer de digest : un succès muet n'est pas un succès",
            ),
        }
    }
}

impl std::error::Error for BuildDriverError {}
