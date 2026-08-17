//! La chaîne de migrations — `docs/SPEC_V1.md` §10.4.

use serde_json::Value;

use crate::migration::{Migration, MigrationError, SchemaVersion};

/// La fenêtre de compatibilité de §10.4.
///
/// « Les producteurs supportent au minimum la version courante **et la version précédente** de LEP
/// pendant une fenêtre de migration. » Deux versions, donc — et le nombre vit en constante parce
/// que c'est un minimum du texte, pas une limite technique.
pub const MINIMUM_SUPPORTED_VERSIONS: usize = 2;

/// Une suite ordonnée de migrations, de la version la plus ancienne à la plus récente.
///
/// # Ce que la chaîne garantit
///
/// - **Contiguïté** : chaque étape part de la version où la précédente est arrivée. Un trou ferait
///   une chaîne qui monte sans passer par toutes les formes qu'un document a réellement eues.
/// - **Refus plutôt que saut** : redescendre à travers une étape irréversible **échoue**, et le
///   refus dit ce qui avait été perdu. Rendre un document appauvri qui aurait l'air complet serait
///   pire que l'échec.
#[derive(Debug, Default)]
pub struct MigrationChain {
    steps: Vec<Migration>,
}

impl MigrationChain {
    /// Une chaîne vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Ajouter une étape.
    ///
    /// # Panics
    ///
    /// Panique si l'étape ne part pas de la version d'arrivée de la précédente. C'est une erreur de
    /// programmation, pas une entrée : une chaîne trouée ne se rattrape pas à l'exécution, et la
    /// découvrir au premier document migré coûterait plus cher que de la refuser ici.
    #[must_use]
    pub fn with(mut self, step: Migration) -> Self {
        if let Some(last) = self.steps.last() {
            assert!(
                step.from() == last.to(),
                "chaîne trouée : l'étape part de v{} alors que la précédente arrive à v{}",
                step.from(),
                last.to()
            );
        }
        assert!(
            step.to() == step.from() + 1,
            "une étape saute des versions : v{} → v{}",
            step.from(),
            step.to()
        );
        self.steps.push(step);
        self
    }

    /// La version la plus ancienne que la chaîne sait lire.
    #[must_use]
    pub fn oldest(&self) -> Option<SchemaVersion> {
        self.steps.first().map(Migration::from)
    }

    /// La version courante — celle où la chaîne arrive.
    #[must_use]
    pub fn current(&self) -> Option<SchemaVersion> {
        self.steps.last().map(Migration::to)
    }

    /// Les étapes, en lecture.
    #[must_use]
    pub fn steps(&self) -> &[Migration] {
        &self.steps
    }

    /// Vrai quand la chaîne couvre la fenêtre minimale de §10.4.
    ///
    /// Une chaîne qui ne saurait lire que la version courante ne supporterait pas « la version
    /// précédente », et un producteur qui n'a pas encore migré serait refusé du jour au lendemain.
    #[must_use]
    pub fn covers_minimum_window(&self) -> bool {
        match (self.oldest(), self.current()) {
            (Some(oldest), Some(current)) => {
                usize::try_from(current - oldest).unwrap_or(0) + 1 >= MINIMUM_SUPPORTED_VERSIONS
            }
            _ => false,
        }
    }

    /// Monter un document de `from` jusqu'à la version courante.
    ///
    /// # Errors
    ///
    /// [`MigrationError::NoPath`] si aucune étape ne part de `from`, ou si la chaîne est vide.
    pub fn upcast_to_current(
        &self,
        document: &Value,
        from: SchemaVersion,
    ) -> Result<Value, MigrationError> {
        let Some(current) = self.current() else {
            return Err(MigrationError::NoPath { from, to: from });
        };
        self.upcast(document, from, current)
    }

    /// Monter un document de `from` à `to`.
    ///
    /// # Errors
    ///
    /// [`MigrationError::NoPath`] quand la chaîne ne couvre pas l'intervalle, ou quand `to`
    /// précède `from` — remonter le temps se demande par [`MigrationChain::downcast`], et confondre
    /// les deux ferait appliquer un `up` là où un `down` était voulu.
    pub fn upcast(
        &self,
        document: &Value,
        from: SchemaVersion,
        to: SchemaVersion,
    ) -> Result<Value, MigrationError> {
        if to < from {
            return Err(MigrationError::NoPath { from, to });
        }
        if to == from {
            return Ok(document.clone());
        }
        let mut current = document.clone();
        let mut version = from;
        while version < to {
            let step = self
                .steps
                .iter()
                .find(|step| step.from() == version)
                .ok_or(MigrationError::NoPath { from, to })?;
            current = step.apply_up(&current)?;
            version = step.to();
        }
        Ok(current)
    }

    /// Redescendre un document de `from` à `to`.
    ///
    /// # Errors
    ///
    /// [`MigrationError::Irreversible`] dès qu'une étape du chemin a déclaré une perte — et le
    /// refus porte les champs perdus. C'est le cœur de W1.h : une chaîne qui redescendrait à
    /// travers une étape destructive rendrait un document ancien **qui n'a jamais existé**, et il
    /// aurait l'air authentique.
    pub fn downcast(
        &self,
        document: &Value,
        from: SchemaVersion,
        to: SchemaVersion,
    ) -> Result<Value, MigrationError> {
        if to > from {
            return Err(MigrationError::NoPath { from, to });
        }
        if to == from {
            return Ok(document.clone());
        }
        let mut current = document.clone();
        let mut version = from;
        while version > to {
            let step = self
                .steps
                .iter()
                .find(|step| step.to() == version)
                .ok_or(MigrationError::NoPath { from, to })?;
            current = step.apply_down(&current)?;
            version = step.from();
        }
        Ok(current)
    }

    /// Les étapes irréversibles du chemin de `from` à `to`, avec ce qu'elles perdent.
    ///
    /// Existe pour qu'on puisse **demander avant de tenter**. Une migration destructive n'est pas
    /// une faute : c'est parfois la seule façon d'avancer. Ce qui est une faute, c'est de le
    /// découvrir au moment où l'on avait besoin de redescendre.
    #[must_use]
    pub fn irreversible_between(&self, from: SchemaVersion, to: SchemaVersion) -> Vec<&Migration> {
        let (low, high) = if from <= to { (from, to) } else { (to, from) };
        self.steps
            .iter()
            .filter(|step| step.from() >= low && step.to() <= high && !step.is_reversible())
            .collect()
    }
}
