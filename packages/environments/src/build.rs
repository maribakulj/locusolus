//! La chaîne de construction — `docs/SPEC_V1.md` §19.5, §19.7, `docs/03`.
//!
//! # Ce que la chaîne garantit
//!
//! §19.5 énumère la suite : « lockfile, SBOM, scan, tests, signature et publication par digest ».
//! Une suite écrite en prose se saute : il suffit d'appeler la dernière fonction. Ici chaque étape
//! **consomme** la preuve de la précédente et rend la sienne, si bien qu'il n'existe aucun chemin
//! qui signe une image non scannée, ou qui publie un digest dont les tests n'ont jamais tourné.
//! L'ordre n'est pas une consigne, c'est la seule façon de composer les types.
//!
//! ```text
//! Locked → Built → Inventoried → Scanned → Tested → Published
//! ```
//!
//! # La garantie, vérifiée par le compilateur
//!
//! `Built` n'a pas de `published` : signer sans scanner n'est pas un chemin à interdire, c'est un
//! chemin qui n'existe pas. Le bloc suivant est vérifié par `cargo test --doc` — il doit **ne pas**
//! compiler.
//!
//! ```compile_fail
//! use locus_environments::{Locked, Signature};
//! fn saute(locked: Locked) {
//!     locked.built("sha256:0").published(Signature {
//!         key_id: "k".to_owned(),
//!         value: "v".to_owned(),
//!     });
//! }
//! ```
//!
//! # Ce que la chaîne ne garantit pas, et qu'il faut dire
//!
//! Qu'une [`crate::Image`] vienne d'une chaîne. `Image::new` reste publique, parce que décrire un
//! environnement **déjà publié** — lu d'un registre, reçu d'un pair — est un autre acte que le
//! construire. La garantie porte sur *ce build-ci*, pas sur toute image qui existe. Prétendre le
//! contraire aurait obligé à rendre indescriptible ce qui existait avant nous.

use std::fmt;

use crate::blueprint::{BlueprintError, EnvironmentBlueprint, Image};

/// Un fichier de verrouillage, avec le hash de son contenu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lockfile {
    /// Son chemin dans le contexte de build.
    pub path: String,
    /// Le hash de son contenu.
    pub hash: String,
}

/// L'inventaire des composants de l'image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sbom {
    /// Combien de composants y sont listés.
    pub components: usize,
    /// Le hash du document lui-même.
    pub document_hash: String,
}

/// La gravité d'une vulnérabilité, du moins grave au plus grave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Faible.
    Low,
    /// Moyenne.
    Medium,
    /// Élevée.
    High,
    /// Critique.
    Critical,
}

impl fmt::Display for Severity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Low => "faible",
            Self::Medium => "moyenne",
            Self::High => "élevée",
            Self::Critical => "critique",
        })
    }
}

/// Ce que le scanner a trouvé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// L'identifiant de la vulnérabilité.
    pub id: String,
    /// Sa gravité.
    pub severity: Severity,
    /// Le composant concerné.
    pub component: String,
}

/// Ce qu'un health check a produit.
///
/// Les trois cas, et le troisième est celui qui compte : une vérification qu'on n'a pas su lancer
/// n'a rien prouvé. C'est le même refus que `Observed::NotRun` de la suite de sandbox, et pour la
/// même raison — la compter comme un succès ferait d'un outil manquant une preuve de santé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthOutcome {
    /// Elle est passée.
    Passed,
    /// Elle a échoué.
    Failed {
        /// Ce que la commande a dit.
        detail: String,
    },
    /// Elle n'a pas pu être lancée.
    NotRun {
        /// Ce qui l'a empêchée.
        reason: String,
    },
}

/// Le résultat d'une vérification nommée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthResult {
    /// Le nom de la vérification, tel que le blueprint l'a déclarée.
    pub name: String,
    /// Ce qu'elle a produit.
    pub outcome: HealthOutcome,
}

/// La signature de l'image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    /// La clé qui a signé.
    pub key_id: String,
    /// La signature elle-même.
    pub value: String,
}

/// Étape 1 — les dépendances sont verrouillées.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locked {
    blueprint: EnvironmentBlueprint,
    lockfiles: Vec<Lockfile>,
}

impl Locked {
    /// Verrouiller les dépendances d'un blueprint.
    ///
    /// # Errors
    ///
    /// [`BuildError::NoLockfile`] pour une liste vide. §19.7 fait de `R2` « l'environnement
    /// verrouillé » : une image construite sans lockfile ne se reconstruit pas à l'identique, et
    /// la publier reviendrait à promettre `R2` sans le tenir.
    pub fn new(
        blueprint: EnvironmentBlueprint,
        lockfiles: Vec<Lockfile>,
    ) -> Result<Self, BuildError> {
        if lockfiles.is_empty() {
            return Err(BuildError::NoLockfile);
        }
        Ok(Self {
            blueprint,
            lockfiles,
        })
    }

    /// Les lockfiles.
    #[must_use]
    pub fn lockfiles(&self) -> &[Lockfile] {
        &self.lockfiles
    }

    /// Étape 2 — l'image est construite.
    #[must_use]
    pub fn built(self, layers_digest: &str) -> Built {
        Built {
            locked: self,
            layers_digest: layers_digest.to_owned(),
        }
    }
}

/// Étape 2 — l'image existe, et rien n'a encore été vérifié dessus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Built {
    locked: Locked,
    layers_digest: String,
}

impl Built {
    /// Étape 3 — l'inventaire est produit.
    ///
    /// # Errors
    ///
    /// [`BuildError::EmptyInventory`] pour un SBOM qui ne liste aucun composant : un inventaire
    /// vide n'est pas un inventaire, c'est un scanner qui n'a pas tourné.
    pub fn inventoried(self, sbom: Sbom) -> Result<Inventoried, BuildError> {
        if sbom.components == 0 || sbom.document_hash.trim().is_empty() {
            return Err(BuildError::EmptyInventory);
        }
        Ok(Inventoried { built: self, sbom })
    }
}

/// Étape 3 — les composants sont connus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inventoried {
    built: Built,
    sbom: Sbom,
}

impl Inventoried {
    /// Étape 4 — le scan est passé, et son verdict est tenu.
    ///
    /// # « Scanné » ne veut pas dire « propre »
    ///
    /// Un scan qui rend des vulnérabilités et laisse passer l'image donne à la chaîne l'apparence
    /// d'un contrôle sans le contrôle. Le plafond est donc un **argument** : la politique décide de
    /// ce qu'elle tolère, et ce qui le dépasse arrête la chaîne en nommant la pire trouvaille.
    ///
    /// # Errors
    ///
    /// [`BuildError::VulnerabilityAboveCeiling`] dès qu'une trouvaille atteint le plafond.
    pub fn scanned(self, findings: Vec<Finding>, ceiling: Severity) -> Result<Scanned, BuildError> {
        if let Some(worst) = findings
            .iter()
            .filter(|finding| finding.severity >= ceiling)
            .max_by_key(|finding| finding.severity)
        {
            return Err(BuildError::VulnerabilityAboveCeiling {
                id: worst.id.clone(),
                component: worst.component.clone(),
                severity: worst.severity,
                ceiling,
            });
        }
        Ok(Scanned {
            inventoried: self,
            findings,
        })
    }
}

/// Étape 4 — le scan a conclu sous le plafond.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scanned {
    inventoried: Inventoried,
    findings: Vec<Finding>,
}

impl Scanned {
    /// Les trouvailles tolérées. Sous le plafond ne veut pas dire aucune.
    #[must_use]
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    /// Étape 5 — les vérifications de santé ont tourné.
    ///
    /// # Errors
    ///
    /// [`BuildError::NoHealthCheck`] pour une liste vide — une image dont rien n'a été vérifié ne
    /// prouve pas qu'elle est utilisable ; [`BuildError::HealthCheckFailed`] pour un échec ;
    /// [`BuildError::HealthCheckNotRun`] pour une vérification qu'on n'a pas su lancer, refus
    /// distinct du précédent parce qu'il envoie chercher ailleurs.
    pub fn tested(self, results: Vec<HealthResult>) -> Result<Tested, BuildError> {
        if results.is_empty() {
            return Err(BuildError::NoHealthCheck);
        }
        for result in &results {
            match &result.outcome {
                HealthOutcome::Passed => {}
                HealthOutcome::Failed { detail } => {
                    return Err(BuildError::HealthCheckFailed {
                        name: result.name.clone(),
                        detail: detail.clone(),
                    });
                }
                HealthOutcome::NotRun { reason } => {
                    return Err(BuildError::HealthCheckNotRun {
                        name: result.name.clone(),
                        reason: reason.clone(),
                    });
                }
            }
        }
        Ok(Tested {
            scanned: self,
            results,
        })
    }
}

/// Étape 5 — l'image fait ce qu'elle annonce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tested {
    scanned: Scanned,
    results: Vec<HealthResult>,
}

impl Tested {
    /// Étape 6 — l'image est signée, et publiée par digest.
    ///
    /// # Errors
    ///
    /// [`BuildError::UnsignedPublication`] pour une signature sans clé ou sans valeur, et
    /// [`BuildError::Blueprint`] quand le digest produit par le build n'en est pas un.
    pub fn published(self, signature: Signature) -> Result<Published, BuildError> {
        if signature.key_id.trim().is_empty() || signature.value.trim().is_empty() {
            return Err(BuildError::UnsignedPublication);
        }
        let digest = self.scanned.inventoried.built.layers_digest.clone();
        let image = Image::new(&digest, None).map_err(BuildError::Blueprint)?;
        Ok(Published {
            tested: self,
            signature,
            image,
        })
    }
}

/// Étape 6 — l'environnement est publié.
///
/// Cette valeur est la preuve que les six étapes ont eu lieu, dans l'ordre : elle ne se construit
/// pas autrement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Published {
    tested: Tested,
    signature: Signature,
    image: Image,
}

impl Published {
    /// L'image publiée.
    #[must_use]
    pub const fn image(&self) -> &Image {
        &self.image
    }

    /// La signature.
    #[must_use]
    pub const fn signature(&self) -> &Signature {
        &self.signature
    }

    /// Le blueprint d'origine.
    #[must_use]
    pub const fn blueprint(&self) -> &EnvironmentBlueprint {
        &self.tested.scanned.inventoried.built.locked.blueprint
    }

    /// L'inventaire.
    #[must_use]
    pub const fn sbom(&self) -> &Sbom {
        &self.tested.scanned.inventoried.sbom
    }

    /// Les vérifications de santé, toutes passées.
    #[must_use]
    pub fn health(&self) -> &[HealthResult] {
        &self.tested.results
    }

    /// Les lockfiles qui verrouillent l'environnement.
    #[must_use]
    pub fn lockfiles(&self) -> &[Lockfile] {
        self.tested.scanned.inventoried.built.locked.lockfiles()
    }

    /// Les vulnérabilités tolérées par la politique de scan.
    ///
    /// Sous le plafond ne veut pas dire aucune. Publier une image porteuse de vulnérabilités
    /// connues sans les emporter reviendrait à les oublier au moment précis où quelqu'un pourrait
    /// encore décider de ne pas s'en servir.
    #[must_use]
    pub fn findings_tolerated(&self) -> &[Finding] {
        self.tested.scanned.findings()
    }
}

/// Ce qui arrête la chaîne.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildError {
    /// Aucun lockfile : l'environnement ne serait pas verrouillé.
    NoLockfile,
    /// Un SBOM qui ne liste rien.
    EmptyInventory,
    /// Une vulnérabilité au moins aussi grave que le plafond toléré.
    VulnerabilityAboveCeiling {
        /// Son identifiant.
        id: String,
        /// Le composant concerné.
        component: String,
        /// Sa gravité.
        severity: Severity,
        /// Le plafond que la politique tolérait.
        ceiling: Severity,
    },
    /// Aucune vérification de santé.
    NoHealthCheck,
    /// Une vérification a échoué.
    HealthCheckFailed {
        /// Laquelle.
        name: String,
        /// Ce qu'elle a dit.
        detail: String,
    },
    /// Une vérification n'a pas pu être lancée.
    HealthCheckNotRun {
        /// Laquelle.
        name: String,
        /// Ce qui l'a empêchée.
        reason: String,
    },
    /// Une publication sans signature utilisable.
    UnsignedPublication,
    /// Le digest produit par le build n'en est pas un.
    Blueprint(BlueprintError),
}

impl fmt::Display for BuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoLockfile => formatter.write_str(
                "aucun lockfile : l'image ne se reconstruirait pas à l'identique, donc pas de R2",
            ),
            Self::EmptyInventory => formatter
                .write_str("un SBOM sans composant n'est pas un inventaire, c'est un scan absent"),
            Self::VulnerabilityAboveCeiling {
                id,
                component,
                severity,
                ceiling,
            } => write!(
                formatter,
                "{id} sur « {component} » est de gravité {severity}, au-dessus du plafond {ceiling}"
            ),
            Self::NoHealthCheck => formatter
                .write_str("aucune vérification de santé : rien ne dit que l'image est utilisable"),
            Self::HealthCheckFailed { name, detail } => {
                write!(formatter, "la vérification « {name} » a échoué : {detail}")
            }
            Self::HealthCheckNotRun { name, reason } => write!(
                formatter,
                "la vérification « {name} » n'a pas pu être lancée : {reason}"
            ),
            Self::UnsignedPublication => {
                formatter.write_str("une image se publie signée, par clé nommée")
            }
            Self::Blueprint(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for BuildError {}
