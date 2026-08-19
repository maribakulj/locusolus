//! Le `CommandEnvelope` de `SPEC_V1.md` §22.2, et la règle de concurrence de §22.5.
//!
//! # « Toute commande mutante accepte `expected_revision` »
//!
//! La phrase de §22.5 dit « accepte ». Ce module en fait « **exige** », et l'écart est délibéré :
//! une commande mutante qui n'annonce pas la révision qu'elle croit modifier ne peut pas être
//! refusée pour conflit, donc elle écrase. « Accepte » décrit ce que le fil tolère ; ce qui protège
//! est que le serveur ne construise pas de commande sans.
//!
//! Deux portes, et elles ne disent pas la même chose :
//!
//! - [`CommandEnvelope::mutating`] **exige** la révision comme paramètre. Une commande mutante sans
//!   elle n'est pas seulement refusée, elle n'est pas **écrivable** — un bloc `compile_fail` le
//!   vérifie, et c'est le compilateur qui répond, pas un test.
//! - [`Draft::seal`] refuse en **nommant le champ**, parce qu'un chemin qui assemble une commande
//!   morceau par morceau — un décodeur, un client, une CLI — ne peut pas être tenu par la signature
//!   et a besoin qu'on lui dise lequel manque.
//!
//! Une seule des deux aurait laissé un trou : la première ne couvre pas ce qui vient du dehors, la
//! seconde ne couvre pas ce que le serveur écrit lui-même.
//!
//! # Ce que ce module ne fait pas
//!
//! Il ne transporte rien, ne route rien, n'exécute rien. §22.3 énumère quarante commandes ; aucune
//! n'est ici, parce qu'aucune n'a de handler. C'est la règle que `CLAUDE.md` pose pour les
//! énumérations — « une sorte n'entre que lorsqu'un consommateur exécutable et testé existe » — et
//! le `command_type` reste donc une **chaîne validée**, pas une liste close de quarante noms qui
//! auraient l'air implémentés.

use std::fmt;

use locus_protocol::Id;
use locus_protocol::id::{Agent, Command, Delegation, Workflow, Workspace};
use serde::{Deserialize, Serialize};

use crate::error::{CommandError, Revision};

/// Une commande, telle que §22.2 la décrit.
///
/// Les champs sont privés : elle se construit par [`CommandEnvelope::mutating`] ou par
/// [`Draft::seal`], et pas par littéral. Sans cela `expected_revision` serait un champ comme un
/// autre, qu'un appelant pourrait omettre en écrivant `..Default::default()`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandEnvelope {
    command_id: Id<Command>,
    command_type: String,
    schema_version: u32,
    workspace_id: Id<Workspace>,
    actor_principal_id: Id<Agent>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    delegation_id: Option<Id<Delegation>>,
    idempotency_key: String,
    expected_revision: Revision,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    correlation_id: Option<Id<Workflow>>,
}

impl CommandEnvelope {
    /// La version de schéma que ce module écrit — §22.2, `schema_version`.
    pub const SCHEMA_VERSION: u32 = 1;

    /// Une commande **mutante**, dont la révision attendue est exigée par la signature.
    ///
    /// # La première porte, vérifiée par le compilateur
    ///
    /// Une commande mutante sans `expected_revision` n'est pas refusée : elle n'est pas
    /// **écrivable**. Le bloc suivant est exécuté par `cargo test --doc` et doit **ne pas**
    /// compiler — sans lui, la garantie ne tiendrait qu'à la discipline de qui ajoutera un jour un
    /// constructeur de commodité.
    ///
    /// ```compile_fail
    /// use locusd::CommandEnvelope;
    /// use locus_protocol::Id;
    /// use locus_protocol::id::{Agent, Command, Workspace};
    ///
    /// fn ecraser(
    ///     command_id: Id<Command>,
    ///     workspace_id: Id<Workspace>,
    ///     actor: Id<Agent>,
    /// ) -> CommandEnvelope {
    ///     CommandEnvelope::mutating(command_id, "branch.fork", workspace_id, actor, "idem-1")
    ///         .expect("sans révision")
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// [`CommandError::Validation`] pour un `command_type` ou un `idempotency_key` vide, en nommant
    /// le champ. Jamais pour `expected_revision` : il n'est pas omissible ici.
    pub fn mutating(
        command_id: Id<Command>,
        command_type: impl Into<String>,
        workspace_id: Id<Workspace>,
        actor_principal_id: Id<Agent>,
        idempotency_key: impl Into<String>,
        expected_revision: Revision,
    ) -> Result<Self, CommandError> {
        let command_type = command_type.into();
        let idempotency_key = idempotency_key.into();
        non_empty("command_type", &command_type)?;
        non_empty("idempotency_key", &idempotency_key)?;
        Ok(Self {
            command_id,
            command_type,
            schema_version: Self::SCHEMA_VERSION,
            workspace_id,
            actor_principal_id,
            delegation_id: None,
            idempotency_key,
            expected_revision,
            correlation_id: None,
        })
    }

    /// La délégation sous laquelle l'acteur agit — §22.2, `delegation_id`.
    #[must_use]
    pub fn delegated_by(mut self, delegation: Id<Delegation>) -> Self {
        self.delegation_id = Some(delegation);
        self
    }

    /// Le workflow qui corrèle cette commande — §22.2, `correlation_id`.
    #[must_use]
    pub fn correlated_with(mut self, workflow: Id<Workflow>) -> Self {
        self.correlation_id = Some(workflow);
        self
    }

    /// Son identifiant.
    #[must_use]
    pub const fn command_id(&self) -> &Id<Command> {
        &self.command_id
    }

    /// Son type, tel que §22.3 le nomme.
    #[must_use]
    pub fn command_type(&self) -> &str {
        &self.command_type
    }

    /// La version de schéma de l'enveloppe.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Le workspace visé.
    #[must_use]
    pub const fn workspace_id(&self) -> &Id<Workspace> {
        &self.workspace_id
    }

    /// Le principal qui agit.
    #[must_use]
    pub const fn actor_principal_id(&self) -> &Id<Agent> {
        &self.actor_principal_id
    }

    /// La délégation, s'il y en a une.
    #[must_use]
    pub const fn delegation_id(&self) -> Option<&Id<Delegation>> {
        self.delegation_id.as_ref()
    }

    /// La clé d'idempotence.
    #[must_use]
    pub fn idempotency_key(&self) -> &str {
        &self.idempotency_key
    }

    /// La révision que la commande croit modifier.
    #[must_use]
    pub const fn expected_revision(&self) -> Revision {
        self.expected_revision
    }

    /// La corrélation, s'il y en a une.
    #[must_use]
    pub const fn correlation_id(&self) -> Option<&Id<Workflow>> {
        self.correlation_id.as_ref()
    }
}

/// Une commande en cours d'assemblage, pour les chemins qui la reçoivent morceau par morceau.
///
/// Un décodeur ou une CLI ne peut pas être tenu par une signature : il découvre les champs dans
/// l'ordre où ils arrivent. [`Draft::seal`] est donc la seconde porte, et elle **nomme** ce qui
/// manque au lieu de rendre un refus générique — un client qui reçoit « commande invalide » relit
/// la documentation, un client qui reçoit « `expected_revision` » corrige.
#[derive(Debug, Clone, Default)]
pub struct Draft {
    command_id: Option<Id<Command>>,
    command_type: Option<String>,
    workspace_id: Option<Id<Workspace>>,
    actor_principal_id: Option<Id<Agent>>,
    delegation_id: Option<Id<Delegation>>,
    idempotency_key: Option<String>,
    expected_revision: Option<Revision>,
    correlation_id: Option<Id<Workflow>>,
}

impl Draft {
    /// Un brouillon vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// L'identifiant de la commande.
    #[must_use]
    pub const fn command_id(mut self, id: Id<Command>) -> Self {
        self.command_id = Some(id);
        self
    }

    /// Son type.
    #[must_use]
    pub fn command_type(mut self, kind: impl Into<String>) -> Self {
        self.command_type = Some(kind.into());
        self
    }

    /// Le workspace visé.
    #[must_use]
    pub const fn workspace_id(mut self, id: Id<Workspace>) -> Self {
        self.workspace_id = Some(id);
        self
    }

    /// Le principal qui agit.
    #[must_use]
    pub const fn actor_principal_id(mut self, id: Id<Agent>) -> Self {
        self.actor_principal_id = Some(id);
        self
    }

    /// La délégation.
    #[must_use]
    pub const fn delegation_id(mut self, id: Id<Delegation>) -> Self {
        self.delegation_id = Some(id);
        self
    }

    /// La clé d'idempotence.
    #[must_use]
    pub fn idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = Some(key.into());
        self
    }

    /// La révision attendue.
    #[must_use]
    pub const fn expected_revision(mut self, revision: Revision) -> Self {
        self.expected_revision = Some(revision);
        self
    }

    /// La corrélation.
    #[must_use]
    pub const fn correlation_id(mut self, id: Id<Workflow>) -> Self {
        self.correlation_id = Some(id);
        self
    }

    /// Sceller le brouillon en commande mutante.
    ///
    /// # Errors
    ///
    /// [`CommandError::Validation`] nommant le **premier** champ manquant ou vide, dans l'ordre où
    /// §22.2 les écrit. L'ordre est celui du document plutôt que celui de la découverte : deux
    /// clients auxquels il manque les mêmes champs reçoivent le même message, et un message stable
    /// se cite dans un rapport de bug.
    pub fn seal(self) -> Result<CommandEnvelope, CommandError> {
        let command_id = required("command_id", self.command_id)?;
        let command_type = required("command_type", self.command_type)?;
        let workspace_id = required("workspace_id", self.workspace_id)?;
        let actor_principal_id = required("actor_principal_id", self.actor_principal_id)?;
        let idempotency_key = required("idempotency_key", self.idempotency_key)?;
        let expected_revision = required("expected_revision", self.expected_revision)?;

        let mut envelope = CommandEnvelope::mutating(
            command_id,
            command_type,
            workspace_id,
            actor_principal_id,
            idempotency_key,
            expected_revision,
        )?;
        if let Some(delegation) = self.delegation_id {
            envelope = envelope.delegated_by(delegation);
        }
        if let Some(correlation) = self.correlation_id {
            envelope = envelope.correlated_with(correlation);
        }
        Ok(envelope)
    }
}

fn required<T>(field: &str, value: Option<T>) -> Result<T, CommandError> {
    value.ok_or_else(|| CommandError::Validation {
        field: field.to_owned(),
        detail: "manquant".to_owned(),
    })
}

fn non_empty(field: &str, value: &str) -> Result<(), CommandError> {
    if value.trim().is_empty() {
        return Err(CommandError::Validation {
            field: field.to_owned(),
            detail: "vide".to_owned(),
        });
    }
    Ok(())
}

impl fmt::Display for CommandEnvelope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} ({}) sur {} à la révision {}",
            self.command_type, self.command_id, self.workspace_id, self.expected_revision
        )
    }
}
