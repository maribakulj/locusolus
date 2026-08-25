//! `AgentTemplate` et `AgentInstance` — `docs/SPEC_V1.md` §7.1, §14.2.

use std::fmt;

use locus_protocol::{
    Id,
    id::{Agent, Branch, Program, provisional::Team as TeamKind},
};

/// L'état d'une instance d'agent, tel que §7.1 l'énumère.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InstanceState {
    /// Provisionnée, pas encore active.
    Provisioned,
    /// Active.
    Active,
    /// En attente.
    Waiting,
    /// Terminée normalement.
    Completed,
    /// Terminée en échec.
    Failed,
    /// Arrêtée.
    Terminated,
}

impl InstanceState {
    /// Les six, dans l'ordre de §7.1.
    pub const ALL: [Self; 6] = [
        Self::Provisioned,
        Self::Active,
        Self::Waiting,
        Self::Completed,
        Self::Failed,
        Self::Terminated,
    ];

    /// Le nom employé par §7.1.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Provisioned => "provisioned",
            Self::Active => "active",
            Self::Waiting => "waiting",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Terminated => "terminated",
        }
    }

    /// Relire un état.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|state| state.slug() == value)
    }

    /// Vrai quand plus rien ne suit.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Terminated)
    }
}

impl fmt::Display for InstanceState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// L'état d'un template.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TemplateStatus {
    /// Employable.
    Active,
    /// Encore employable, mais à ne plus choisir.
    Deprecated,
    /// Plus employable.
    Disabled,
}

impl TemplateStatus {
    /// Les trois de §7.1.
    pub const ALL: [Self; 3] = [Self::Active, Self::Deprecated, Self::Disabled];

    /// Le nom employé par §7.1.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Deprecated => "deprecated",
            Self::Disabled => "disabled",
        }
    }

    /// Vrai quand une nouvelle instance peut naître de ce template.
    ///
    /// `deprecated` reste instanciable : §7.1 le distingue de `disabled`, et confondre les deux
    /// arrêterait des campagnes en cours au lieu d'en décourager de nouvelles.
    #[must_use]
    pub const fn is_instantiable(self) -> bool {
        matches!(self, Self::Active | Self::Deprecated)
    }
}

impl fmt::Display for TemplateStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Le modèle d'un rôle — §14.2 : « `AgentTemplate` définit le rôle et les contraintes ».
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTemplate {
    id: Id<Agent>,
    name: String,
    role: String,
    version: u32,
    status: TemplateStatus,
    review_independence_group: Option<String>,
}

impl AgentTemplate {
    /// Déclarer un template.
    ///
    /// # Errors
    ///
    /// [`AgentError::EmptyField`] pour un nom ou un rôle vides, [`AgentError::ZeroVersion`] pour
    /// une version nulle — l'identité d'un agent comprend « le template **et sa version** » (§7.1),
    /// et une version zéro ne désigne aucune révision.
    pub fn new(
        id: Id<Agent>,
        name: &str,
        role: &str,
        version: u32,
        status: TemplateStatus,
    ) -> Result<Self, AgentError> {
        for (field, value) in [("name", name), ("role", role)] {
            if value.trim().is_empty() {
                return Err(AgentError::EmptyField { field });
            }
        }
        if version == 0 {
            return Err(AgentError::ZeroVersion);
        }
        Ok(Self {
            id,
            name: name.to_owned(),
            role: role.to_owned(),
            version,
            status,
            review_independence_group: None,
        })
    }

    /// Le groupe d'indépendance de revue, quand le rôle en porte un (§14.4).
    #[must_use]
    pub fn in_independence_group(mut self, group: &str) -> Self {
        self.review_independence_group = Some(group.to_owned());
        self
    }

    /// Son identifiant.
    #[must_use]
    pub const fn id(&self) -> Id<Agent> {
        self.id
    }

    /// Son nom.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Le rôle qu'il définit — un des noms de §14.1.
    #[must_use]
    pub fn role(&self) -> &str {
        &self.role
    }

    /// Sa version.
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Son statut.
    #[must_use]
    pub const fn status(&self) -> TemplateStatus {
        self.status
    }

    /// Son groupe d'indépendance.
    #[must_use]
    pub fn review_independence_group(&self) -> Option<&str> {
        self.review_independence_group.as_deref()
    }
}

/// Une exécution située — §14.2 : « `AgentInstance` est une exécution située, traçable et
/// temporaire ».
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentInstance {
    id: Id<Agent>,
    template_id: Id<Agent>,
    template_version: u32,
    program_id: Option<Id<Program>>,
    branch_id: Option<Id<Branch>>,
    team_id: Option<Id<TeamKind>>,
    worker_id: Option<String>,
    independence_group: Option<String>,
    state: InstanceState,
}

impl AgentInstance {
    /// Instancier un template.
    ///
    /// # La version du template est copiée, pas référencée
    ///
    /// §7.1 : « l'identité d'un agent comprend le template, **sa version**, le modèle exact… ».
    /// Un template évolue ; une instance qui ne garderait que `template_id` changerait d'identité
    /// rétroactivement à chaque révision, et une revue d'il y a six mois cesserait de dire ce
    /// qu'elle disait.
    ///
    /// L'instance hérite aussi du groupe d'indépendance du template — c'est ce que §14.4 exige
    /// pour que deux relecteurs du même groupe ne soient pas comptés comme indépendants.
    ///
    /// # Errors
    ///
    /// [`AgentError::TemplateNotInstantiable`] quand le template est `disabled`.
    pub fn provision(id: Id<Agent>, template: &AgentTemplate) -> Result<Self, AgentError> {
        if !template.status().is_instantiable() {
            return Err(AgentError::TemplateNotInstantiable {
                status: template.status(),
            });
        }
        Ok(Self {
            id,
            template_id: template.id(),
            template_version: template.version(),
            program_id: None,
            branch_id: None,
            team_id: None,
            worker_id: None,
            independence_group: template.review_independence_group().map(ToOwned::to_owned),
            state: InstanceState::Provisioned,
        })
    }

    /// Reposer une instance depuis son état persisté — `W23.a`, ADR 0026 décision 2.
    ///
    /// # Ce chemin ne passe pas par `moved_to`, et c'est délibéré
    ///
    /// Reconstruire n'est **pas** une transition. Y passer serait faux deux fois : la machine de
    /// §7.1 refuse de quitter un état terminal, donc une instance `Completed` ne serait pas
    /// reconstructible ; et une reconstruction est la **même** instance qu'on relit, pas une
    /// instance qu'on fait avancer — les journaliser comme des transitions ferait compter à `W21.j`
    /// des durées de vie qui n'ont pas eu lieu.
    ///
    /// Le constructeur reste néanmoins **vérifiant** : un support qui rendrait une version nulle ou
    /// un champ présent et vide décrirait une instance que le domaine n'aurait jamais construite, et
    /// la reposer telle quelle ferait entrer par la lecture ce que l'écriture refuse.
    ///
    /// # Errors
    ///
    /// [`AgentError::ZeroVersion`] pour une version nulle, [`AgentError::EmptyField`] pour un
    /// `worker_id` ou un groupe d'indépendance présent et vide.
    #[allow(clippy::too_many_arguments)]
    pub fn from_state(
        id: Id<Agent>,
        template_id: Id<Agent>,
        template_version: u32,
        program_id: Option<Id<Program>>,
        branch_id: Option<Id<Branch>>,
        team_id: Option<Id<TeamKind>>,
        worker_id: Option<&str>,
        independence_group: Option<&str>,
        state: InstanceState,
    ) -> Result<Self, AgentError> {
        if template_version == 0 {
            return Err(AgentError::ZeroVersion);
        }
        for (field, value) in [
            ("worker_id", worker_id),
            ("independence_group", independence_group),
        ] {
            if value.is_some_and(str::is_empty) {
                return Err(AgentError::EmptyField { field });
            }
        }

        Ok(Self {
            id,
            template_id,
            template_version,
            program_id,
            branch_id,
            team_id,
            worker_id: worker_id.map(str::to_owned),
            independence_group: independence_group.map(str::to_owned),
            state,
        })
    }

    /// La situer dans un programme.
    #[must_use]
    pub const fn in_program(mut self, program_id: Id<Program>) -> Self {
        self.program_id = Some(program_id);
        self
    }

    /// La situer sur une branche.
    #[must_use]
    pub const fn on_branch(mut self, branch_id: Id<Branch>) -> Self {
        self.branch_id = Some(branch_id);
        self
    }

    /// La rattacher à une équipe.
    #[must_use]
    pub const fn in_team(mut self, team_id: Id<TeamKind>) -> Self {
        self.team_id = Some(team_id);
        self
    }

    /// Nommer le worker qui l'exécute.
    #[must_use]
    pub fn on_worker(mut self, worker_id: &str) -> Self {
        self.worker_id = Some(worker_id.to_owned());
        self
    }

    /// La faire changer d'état.
    ///
    /// # Errors
    ///
    /// [`AgentError::TerminalState`] quand elle est déjà terminée : une instance est « temporaire »
    /// (§14.2), et la ranimer effacerait la trace de sa fin.
    pub fn moved_to(mut self, next: InstanceState) -> Result<Self, AgentError> {
        if self.state.is_terminal() {
            return Err(AgentError::TerminalState { state: self.state });
        }
        self.state = next;
        Ok(self)
    }

    /// Son identifiant.
    #[must_use]
    pub const fn id(&self) -> Id<Agent> {
        self.id
    }

    /// Le template dont elle vient.
    #[must_use]
    pub const fn template_id(&self) -> Id<Agent> {
        self.template_id
    }

    /// La version du template au moment de l'instanciation.
    #[must_use]
    pub const fn template_version(&self) -> u32 {
        self.template_version
    }

    /// Le programme, quand elle en a un.
    #[must_use]
    pub const fn program_id(&self) -> Option<Id<Program>> {
        self.program_id
    }

    /// La branche, quand elle en a une.
    #[must_use]
    pub const fn branch_id(&self) -> Option<Id<Branch>> {
        self.branch_id
    }

    /// L'équipe, quand elle en a une.
    #[must_use]
    pub const fn team_id(&self) -> Option<Id<TeamKind>> {
        self.team_id
    }

    /// Le worker, quand il est connu.
    #[must_use]
    pub fn worker_id(&self) -> Option<&str> {
        self.worker_id.as_deref()
    }

    /// Son groupe d'indépendance, hérité du template.
    #[must_use]
    pub fn independence_group(&self) -> Option<&str> {
        self.independence_group.as_deref()
    }

    /// Son état.
    #[must_use]
    pub const fn state(&self) -> InstanceState {
        self.state
    }
}

/// Ce qui empêche un template ou une instance d'exister ou d'avancer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentError {
    /// Un champ obligatoire vide.
    EmptyField {
        /// Lequel.
        field: &'static str,
    },
    /// Une version nulle.
    ZeroVersion,
    /// Un template dont on ne peut plus instancier.
    TemplateNotInstantiable {
        /// Son statut.
        status: TemplateStatus,
    },
    /// Une instance déjà terminée.
    TerminalState {
        /// Son état.
        state: InstanceState,
    },
}

impl fmt::Display for AgentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(formatter, "le champ « {field} » est vide"),
            Self::ZeroVersion => {
                formatter.write_str("une version zéro ne désigne aucune révision de template")
            }
            Self::TemplateNotInstantiable { status } => {
                write!(formatter, "un template « {status} » ne s'instancie plus")
            }
            Self::TerminalState { state } => {
                write!(formatter, "une instance « {state} » ne se ranime pas")
            }
        }
    }
}

impl std::error::Error for AgentError {}
