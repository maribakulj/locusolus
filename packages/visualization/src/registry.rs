//! Le registre de viewers — `docs/SPEC_V1.md` §23.5 et `docs/07`.
//!
//! # La phrase et l'invariant qu'elle sert
//!
//! §23.5 : « un artefact déclare des **hints** mais le client **choisit** le meilleur viewer
//! disponible. » Invariant 10 : « xiiif n'est pas requis par les agents. »
//!
//! Les deux disent la même chose depuis deux endroits, et le registre la tient d'une seule façon :
//! **la capacité admet, la suggestion ordonne.** Un hint ne peut que reclasser des viewers qui
//! savent déjà rendre le media type ; il ne peut jamais en faire entrer un qui ne le sait pas. Un
//! artefact ne peut donc pas forcer l'ouverture de xiiif — ni d'aucun autre — parce que le seul
//! pouvoir qu'il a est de trier une liste que le client a constituée.
//!
//! Si la suggestion pouvait admettre, un producteur d'artefacts déciderait à distance de ce qui
//! s'ouvre sur la machine d'un lecteur, et « le client choisit » serait faux.
//!
//! # Choisir ne peut pas échouer
//!
//! [`ArtifactViewerRegistry::choose`] ne rend pas de `Result`. Ce n'est pas une commodité : un
//! artefact qu'aucun viewer ne sait rendre reste **atteignable** — on le télécharge, on l'ouvre
//! ailleurs, on lit son identité. Rendre une erreur ferait d'une absence de confort une panne, et
//! un appelant qui propage les erreurs afficherait « échec » là où il fallait afficher un lien.

use std::collections::BTreeSet;
use std::fmt;

/// Ce qu'un client sait ouvrir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Viewer {
    id: String,
    media_types: Vec<String>,
    hints: Vec<String>,
}

impl Viewer {
    /// Déclarer un viewer.
    ///
    /// `media_types` accepte la forme exacte (`image/jpeg`) et le joker de type
    /// (`image/*`). `hints` nomme les suggestions que ce viewer sait honorer.
    ///
    /// # Errors
    ///
    /// [`RegistryError::EmptyField`] pour une identité, un media type ou un hint vide.
    pub fn declare(id: &str, media_types: &[&str], hints: &[&str]) -> Result<Self, RegistryError> {
        if id.trim().is_empty() {
            return Err(RegistryError::EmptyField { field: "viewer.id" });
        }
        for media_type in media_types {
            if media_type.trim().is_empty() {
                return Err(RegistryError::EmptyField {
                    field: "viewer.media_type",
                });
            }
        }
        for hint in hints {
            if hint.trim().is_empty() {
                return Err(RegistryError::EmptyField {
                    field: "viewer.hint",
                });
            }
        }
        Ok(Self {
            id: id.to_owned(),
            media_types: media_types
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            hints: hints.iter().map(|value| (*value).to_owned()).collect(),
        })
    }

    /// Son identité.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Vrai quand ce viewer sait rendre `media_type`.
    #[must_use]
    pub fn handles(&self, media_type: &str) -> bool {
        self.media_types.iter().any(|declared| {
            declared == media_type
                || declared
                    .strip_suffix("/*")
                    .and_then(|family| media_type.split('/').next().map(|got| got == family))
                    .unwrap_or(false)
        })
    }

    /// Vrai quand ce viewer sait honorer `hint`.
    #[must_use]
    pub fn honours(&self, hint: &str) -> bool {
        self.hints.iter().any(|declared| declared == hint)
    }
}

/// Ce qu'un artefact demande — une suggestion, jamais une exigence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewerRequest {
    /// Ce que l'artefact est.
    pub media_type: String,
    /// Ce qu'il suggère, par ordre de préférence décroissante.
    pub hints: Vec<String>,
}

/// Ce que le client a décidé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    /// Un viewer sait rendre l'artefact et honore une des suggestions.
    Honoured {
        /// Lequel.
        viewer: String,
        /// La suggestion retenue.
        hint: String,
    },
    /// Un viewer sait rendre l'artefact ; aucune suggestion n'a pu être honorée.
    Fallback {
        /// Lequel.
        viewer: String,
    },
    /// Aucun viewer ne sait le rendre. L'artefact reste atteignable pour autant.
    NoViewer {
        /// Ce que l'artefact est, pour que l'appelant puisse encore le proposer au
        /// téléchargement ou à un outil externe.
        media_type: String,
    },
}

/// Le registre des viewers dont le client dispose.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArtifactViewerRegistry {
    viewers: Vec<Viewer>,
}

impl ArtifactViewerRegistry {
    /// Un registre vide.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            viewers: Vec::new(),
        }
    }

    /// Ajouter un viewer.
    ///
    /// L'ordre d'ajout est la préférence du **client** : à suggestion égale, le premier déclaré
    /// gagne. C'est la moitié de « le client choisit » qui n'est pas dans les hints.
    ///
    /// # Errors
    ///
    /// [`RegistryError::DuplicateViewer`] quand l'identité est déjà prise : deux viewers du même
    /// nom rendraient le choix dépendant de l'ordre d'itération plutôt que de la préférence.
    pub fn with(mut self, viewer: Viewer) -> Result<Self, RegistryError> {
        if self.viewers.iter().any(|known| known.id == viewer.id) {
            return Err(RegistryError::DuplicateViewer { id: viewer.id });
        }
        self.viewers.push(viewer);
        Ok(self)
    }

    /// Les viewers déclarés, dans l'ordre de préférence du client.
    #[must_use]
    pub fn viewers(&self) -> &[Viewer] {
        &self.viewers
    }

    /// Choisir pour `request`.
    ///
    /// **La capacité admet, la suggestion ordonne.** Seuls les viewers qui savent rendre le media
    /// type sont candidats ; les hints ne font que les classer. Un hint qui nomme un viewer absent,
    /// inconnu, ou incapable de ce media type est ignoré — pas refusé, ignoré : c'est une
    /// suggestion.
    ///
    /// Ne peut pas échouer. Un artefact qu'aucun viewer ne rend est un artefact qu'on télécharge,
    /// pas une erreur.
    #[must_use]
    pub fn choose(&self, request: &ViewerRequest) -> Choice {
        let candidates: Vec<&Viewer> = self
            .viewers
            .iter()
            .filter(|viewer| viewer.handles(&request.media_type))
            .collect();

        for hint in &request.hints {
            if let Some(viewer) = candidates.iter().find(|viewer| viewer.honours(hint)) {
                return Choice::Honoured {
                    viewer: viewer.id.clone(),
                    hint: hint.clone(),
                };
            }
        }

        candidates.first().map_or_else(
            || Choice::NoViewer {
                media_type: request.media_type.clone(),
            },
            |viewer| Choice::Fallback {
                viewer: viewer.id.clone(),
            },
        )
    }

    /// Le registre de référence — la table de `docs/07` et les exemples de §23.5.
    ///
    /// Elle vit ici plutôt qu'en prose seule pour qu'un test puisse la parcourir : une table de
    /// routage que rien n'exécute se désaccorde du code sans que personne ne le voie. Un client
    /// reste libre d'en composer une autre — c'est lui qui sait ce qu'il a.
    ///
    /// # Panics
    ///
    /// Jamais : les déclarations sont littérales et distinctes, et le test
    /// `le_registre_de_reference_route_les_dix_familles` le vérifie à chaque exécution.
    #[must_use]
    pub fn reference() -> Self {
        let entries: [(&str, &[&str], &[&str]); 10] = [
            ("emacs-native", &["text/markdown", "text/org"], &["text"]),
            (
                "native-image",
                &[
                    "image/png",
                    "image/jpeg",
                    "image/svg+xml",
                    "application/pdf",
                ],
                &["image"],
            ),
            ("webview", &["text/html"], &["html"]),
            (
                "xiiif",
                &["application/ld+json", "application/json"],
                &["iiif"],
            ),
            (
                "web-graph",
                &["application/vnd.locus.graph+json"],
                &["graph"],
            ),
            (
                "three-js",
                &["model/gltf+json", "model/gltf-binary"],
                &["gltf", "3d"],
            ),
            ("potree", &["application/vnd.laszip"], &["point-cloud"]),
            (
                "mol-star",
                &["chemical/x-mmcif", "chemical/x-pdb"],
                &["molecule"],
            ),
            ("vtk-js", &["application/vnd.vtk"], &["volume"]),
            ("jupyter", &["application/x-ipynb+json"], &["notebook"]),
        ];
        let mut registry = Self::new();
        for (id, media_types, hints) in entries {
            registry = registry
                .with(Viewer::declare(id, media_types, hints).expect("déclaration littérale"))
                .expect("les dix identités de `docs/07` sont distinctes");
        }
        registry
    }

    /// Les identités déclarées, triées — de quoi comparer deux registres.
    #[must_use]
    pub fn identities(&self) -> BTreeSet<&str> {
        self.viewers
            .iter()
            .map(|viewer| viewer.id.as_str())
            .collect()
    }
}

/// Ce qui empêche un registre d'exister.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// Un champ obligatoire vide.
    EmptyField {
        /// Lequel.
        field: &'static str,
    },
    /// Deux viewers de même identité.
    DuplicateViewer {
        /// Laquelle.
        id: String,
    },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(formatter, "« {field} » est vide"),
            Self::DuplicateViewer { id } => write!(
                formatter,
                "« {id} » est déjà déclaré : le choix dépendrait de l'ordre d'itération plutôt \
                 que de la préférence du client"
            ),
        }
    }
}

impl std::error::Error for RegistryError {}
