//! Une migration de schéma — `docs/SPEC_V1.md` §10.2 et §10.4.

use std::fmt;

use serde_json::Value;

/// Une version de schéma. `schema_version` dans l'enveloppe de §10.1.
pub type SchemaVersion = u32;

/// Ce qu'une migration perd en montant.
///
/// # Pourquoi c'est un type et pas un commentaire
///
/// §10.4 : « les changements incompatibles créent une nouvelle version de message ». Certains de
/// ces changements sont **destructifs** — fusionner deux champs, arrondir une précision, laisser
/// tomber une distinction devenue inutile. La migration monte quand même ; ce qui ne doit pas
/// arriver, c'est qu'elle prétende ensuite savoir redescendre.
///
/// Déclarer la perte est donc ce qui rend l'irréversibilité **exécutoire** plutôt que documentée :
/// [`Migration::down`] n'existe pas quand il y a une perte, et la chaîne refuse de redescendre à
/// travers l'étape au lieu de rendre un document appauvri qui aurait l'air complet.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Loss {
    /// Les champs ou distinctions que la montée fait disparaître.
    pub fields: Vec<String>,
    /// Pourquoi la perte est acceptée.
    pub rationale: String,
}

/// La fonction qui transforme un document d'une version à une autre.
///
/// Nommée plutôt qu'écrite en place : le type complet apparaît quatre fois, et un lecteur qui doit
/// le déchiffrer à chaque occurrence lit la signature au lieu de lire l'intention.
pub type Transform = Box<dyn Fn(&Value) -> Result<Value, MigrationError> + Send + Sync>;

/// Une migration d'une version de schéma à la suivante.
///
/// # `down` est optionnel, et son absence est une affirmation
///
/// Une migration sans `down` est **irréversible**, et c'est une déclaration, pas un oubli :
/// [`Migration::new`] exige de dire ce qui est perdu. Une migration qui prétendrait redescendre en
/// inventant les valeurs manquantes produirait un document v1 qui n'a jamais existé — et il aurait
/// l'air d'un document v1 authentique, ce qui est pire que l'échec.
pub struct Migration {
    from: SchemaVersion,
    to: SchemaVersion,
    description: String,
    up: Transform,
    down: Option<Transform>,
    loss: Option<Loss>,
}

impl fmt::Debug for Migration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Migration")
            .field("from", &self.from)
            .field("to", &self.to)
            .field("description", &self.description)
            .field("reversible", &self.down.is_some())
            .field("loss", &self.loss)
            // `up` et `down` sont des fermetures : elles n'ont pas de représentation utile, et
            // `finish_non_exhaustive` le dit plutôt que de laisser croire à un `Debug` complet.
            .finish_non_exhaustive()
    }
}

impl Migration {
    /// Une migration **réversible** : elle monte et elle sait redescendre.
    ///
    /// Le test de sortie de W1.h — « migration aller-retour » — porte sur celles-ci, et il vérifie
    /// que la descente rend exactement le document d'origine.
    pub fn reversible<Up, Down>(
        from: SchemaVersion,
        to: SchemaVersion,
        description: &str,
        up: Up,
        down: Down,
    ) -> Self
    where
        Up: Fn(&Value) -> Result<Value, MigrationError> + Send + Sync + 'static,
        Down: Fn(&Value) -> Result<Value, MigrationError> + Send + Sync + 'static,
    {
        Self {
            from,
            to,
            description: description.to_owned(),
            up: Box::new(up),
            down: Some(Box::new(down)),
            loss: None,
        }
    }

    /// Une migration **irréversible**, avec ce qu'elle perd.
    ///
    /// Le paramètre `loss` n'est pas décoratif : c'est lui qui apparaît dans le refus quand
    /// quelqu'un tente de redescendre, et qui dit **pourquoi** on ne peut pas.
    pub fn lossy<Up>(
        from: SchemaVersion,
        to: SchemaVersion,
        description: &str,
        up: Up,
        loss: Loss,
    ) -> Self
    where
        Up: Fn(&Value) -> Result<Value, MigrationError> + Send + Sync + 'static,
    {
        Self {
            from,
            to,
            description: description.to_owned(),
            up: Box::new(up),
            down: None,
            loss: Some(loss),
        }
    }

    /// La version de départ.
    #[must_use]
    pub const fn from(&self) -> SchemaVersion {
        self.from
    }

    /// La version d'arrivée.
    #[must_use]
    pub const fn to(&self) -> SchemaVersion {
        self.to
    }

    /// Ce que la migration fait, en une phrase.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Vrai quand la migration sait redescendre.
    #[must_use]
    pub const fn is_reversible(&self) -> bool {
        self.down.is_some()
    }

    /// Ce que la montée perd, quand elle perd quelque chose.
    #[must_use]
    pub const fn loss(&self) -> Option<&Loss> {
        self.loss.as_ref()
    }

    /// Monter d'une version.
    ///
    /// # Errors
    ///
    /// Rend [`MigrationError`] quand le document ne se laisse pas migrer.
    pub fn apply_up(&self, document: &Value) -> Result<Value, MigrationError> {
        (self.up)(document)
    }

    /// Redescendre d'une version.
    ///
    /// # Errors
    ///
    /// [`MigrationError::Irreversible`] quand la migration a déclaré une perte — et le refus porte
    /// la liste des champs perdus, parce que c'est ce que l'appelant a besoin de savoir pour
    /// décider s'il peut s'en passer.
    pub fn apply_down(&self, document: &Value) -> Result<Value, MigrationError> {
        match &self.down {
            Some(down) => down(document),
            None => Err(MigrationError::Irreversible {
                from: self.from,
                to: self.to,
                lost: self
                    .loss
                    .as_ref()
                    .map(|loss| loss.fields.clone())
                    .unwrap_or_default(),
                rationale: self
                    .loss
                    .as_ref()
                    .map(|loss| loss.rationale.clone())
                    .unwrap_or_default(),
            }),
        }
    }
}

/// Ce qui peut empêcher une migration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationError {
    /// Le document n'a pas la forme attendue.
    Malformed {
        /// Ce qui manquait ou ce qui n'allait pas.
        reason: String,
    },
    /// Aucune migration ne mène de `from` à `to`.
    NoPath {
        /// La version de départ.
        from: SchemaVersion,
        /// La version demandée.
        to: SchemaVersion,
    },
    /// L'étape ne sait pas redescendre, et voici ce qu'elle avait perdu.
    Irreversible {
        /// La version de départ de l'étape.
        from: SchemaVersion,
        /// Sa version d'arrivée.
        to: SchemaVersion,
        /// Les champs perdus.
        lost: Vec<String>,
        /// Pourquoi la perte avait été acceptée.
        rationale: String,
    },
}

impl fmt::Display for MigrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { reason } => write!(formatter, "document non migrable : {reason}"),
            Self::NoPath { from, to } => {
                write!(formatter, "aucun chemin de migration de v{from} à v{to}")
            }
            Self::Irreversible {
                from,
                to,
                lost,
                rationale,
            } => write!(
                formatter,
                "la migration v{from} → v{to} est irréversible : elle a perdu {} ({rationale})",
                if lost.is_empty() {
                    "des informations non déclarées".to_owned()
                } else {
                    lost.join(", ")
                }
            ),
        }
    }
}

impl std::error::Error for MigrationError {}
