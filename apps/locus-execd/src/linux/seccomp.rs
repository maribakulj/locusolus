//! Le profil seccomp restreint : vérifié, jamais supposé.
//!
//! # Ce que ce module fait, et ce qu'il ne fait délibérément pas
//!
//! Il **ne fournit pas** de profil. Un profil seccomp par défaut-refus est une liste de plusieurs
//! centaines d'appels système autorisés, dont l'exactitude ne se démontre qu'en l'exécutant contre
//! des charges réelles. En écrire un ici, sans hôte pour l'éprouver, produirait soit une sandbox
//! qui casse tout, soit — bien pire — une sandbox qui autorise ce qu'elle prétend refuser. C'est
//! le « sandbox factice » que le plan de rollback d'ADR 0004 nomme comme le seul échec
//! inacceptable de ce workstream.
//!
//! Il **vérifie** le profil que le déploiement apporte. [`SeccompPosture::Restricted`] promet le
//! refus de la création de namespaces et du chargement de code noyau depuis l'intérieur ; un profil
//! qui ne les refuse pas ne porte pas cette posture, et le dire est mécanique.
//!
//! [`SeccompPosture::Restricted`]: super::plan::SeccompPosture
//!
//! # Ce que la vérification ne regarde pas, et pourquoi c'est écrit
//!
//! Les **filtres d'arguments**. Un profil peut autoriser `clone` en refusant `CLONE_NEWUSER` par un
//! filtre sur le premier argument, ce qui refuse bien la création d'un namespace utilisateur sans
//! refuser `clone` — dont tout programme à threads a besoin. Cette vérification-là demanderait un
//! second interpréteur, du modèle d'argument cette fois, c'est-à-dire un second endroit où se
//! tromper. `clone` n'est donc **pas** dans [`MUST_DENY`], et la liste ne retient que les appels
//! dont le seul usage est celui que la posture refuse.

use std::fmt;
use std::fs;
use std::path::Path;

use serde::Deserialize;

/// Les appels système qu'un profil restreint doit refuser.
///
/// La liste est exactement ce que la posture promet, ni plus ni moins : créer un namespace, et
/// charger ou décharger du code noyau. Chaque entrée est un appel dont il n'existe pas d'usage
/// légitime dans une sandbox de mission — c'est ce qui permet de le refuser par son nom, sans
/// regarder ses arguments.
pub const MUST_DENY: [&str; 8] = [
    "unshare",
    "setns",
    "init_module",
    "finit_module",
    "delete_module",
    "kexec_load",
    "kexec_file_load",
    "bpf",
];

/// Un profil restreint, lu et vérifié.
///
/// Il n'existe pas de chemin qui construise cette valeur sans passer la vérification : c'est le
/// type qui porte la garantie, pas une consigne d'appeler un validateur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestrictedProfile {
    path: String,
}

impl RestrictedProfile {
    /// Vérifier un profil déjà lu, et le rattacher au chemin qui le désigne.
    ///
    /// # Errors
    ///
    /// [`ProfileError::Unreadable`] pour un JSON illisible, [`ProfileError::Permissive`] quand des
    /// appels de [`MUST_DENY`] ne sont pas refusés — le refus les nomme tous, pour qu'on corrige le
    /// profil en une fois plutôt qu'un appel par tentative.
    pub fn parse(path: &str, source: &str) -> Result<Self, ProfileError> {
        let document: Document =
            serde_json::from_str(source).map_err(|error| ProfileError::Unreadable {
                path: path.to_owned(),
                detail: error.to_string(),
            })?;
        let permitted: Vec<&str> = MUST_DENY
            .into_iter()
            .filter(|name| !document.denies(name))
            .collect();
        if !permitted.is_empty() {
            return Err(ProfileError::Permissive {
                path: path.to_owned(),
                permitted: permitted.into_iter().map(str::to_owned).collect(),
            });
        }
        Ok(Self {
            path: path.to_owned(),
        })
    }

    /// Lire un profil sur le disque, puis le vérifier.
    ///
    /// # Errors
    ///
    /// [`ProfileError::Unreadable`] quand le fichier est absent ou illisible, plus les erreurs de
    /// [`RestrictedProfile::parse`].
    pub fn read(path: &Path) -> Result<Self, ProfileError> {
        let display = path.display().to_string();
        let source = fs::read_to_string(path).map_err(|error| ProfileError::Unreadable {
            path: display.clone(),
            detail: error.to_string(),
        })?;
        Self::parse(&display, &source)
    }

    /// Le chemin à passer au runtime.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }
}

/// La forme d'un profil seccomp, réduite à ce qui décide d'un refus.
#[derive(Debug, Deserialize)]
struct Document {
    #[serde(rename = "defaultAction")]
    default_action: String,
    #[serde(default)]
    syscalls: Vec<Rule>,
}

#[derive(Debug, Deserialize)]
struct Rule {
    #[serde(default)]
    names: Vec<String>,
    #[serde(default)]
    name: Option<String>,
    action: String,
}

impl Document {
    /// Cet appel est-il refusé ?
    ///
    /// # La règle de lecture, et son parti pris
    ///
    /// La **première** règle qui nomme l'appel décide ; s'il n'en existe aucune, l'action par
    /// défaut décide. C'est la lecture conservatrice : un profil dont deux règles se contredisent
    /// est un profil dont le comportement dépend de l'implémentation, et le supposer favorable
    /// serait accorder au profil le bénéfice de sa propre ambiguïté.
    fn denies(&self, syscall: &str) -> bool {
        self.syscalls
            .iter()
            .find(|rule| rule.covers(syscall))
            .map_or_else(
                || denying(&self.default_action),
                |rule| denying(&rule.action),
            )
    }
}

impl Rule {
    fn covers(&self, syscall: &str) -> bool {
        self.names.iter().any(|name| name == syscall) || self.name.as_deref() == Some(syscall)
    }
}

/// Les actions qui empêchent l'appel d'aboutir.
///
/// `SCMP_ACT_LOG` et `SCMP_ACT_ALLOW` laissent passer ; `SCMP_ACT_NOTIFY` remet la décision à un
/// superviseur externe, qui n'existe pas ici — un appel remis à personne aboutit.
fn denying(action: &str) -> bool {
    matches!(
        action,
        "SCMP_ACT_ERRNO"
            | "SCMP_ACT_KILL"
            | "SCMP_ACT_KILL_PROCESS"
            | "SCMP_ACT_KILL_THREAD"
            | "SCMP_ACT_TRAP"
    )
}

/// Ce qui empêche un profil de porter la posture restreinte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    /// Le fichier est absent, ou son contenu n'est pas un profil lisible.
    Unreadable {
        /// Le chemin.
        path: String,
        /// Ce que la lecture a dit.
        detail: String,
    },
    /// Le profil laisse passer des appels que la posture promet de refuser.
    Permissive {
        /// Le chemin.
        path: String,
        /// Les appels laissés passer, dans l'ordre de [`MUST_DENY`].
        permitted: Vec<String>,
    },
}

impl fmt::Display for ProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable { path, detail } => {
                write!(formatter, "profil seccomp « {path} » illisible : {detail}")
            }
            Self::Permissive { path, permitted } => write!(
                formatter,
                "le profil « {path} » laisse passer {} : il ne porte pas la posture restreinte",
                permitted
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

impl std::error::Error for ProfileError {}
