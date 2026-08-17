//! `Decision` et `ApprovalRequest` — `docs/SPEC_V1.md` §7.1, et §20 pour la gouvernance humaine.

use std::fmt;

use locus_protocol::{
    Id,
    id::provisional::{Approval, Decision as DecisionKind},
};

/// L'état d'une décision, tel que §7.1 l'énumère.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecisionState {
    /// Proposée.
    Proposed,
    /// Approuvée.
    Approved,
    /// Rejetée.
    Rejected,
    /// Révoquée après coup.
    Revoked,
}

impl DecisionState {
    /// Les quatre de §7.1.
    pub const ALL: [Self; 4] = [
        Self::Proposed,
        Self::Approved,
        Self::Rejected,
        Self::Revoked,
    ];

    /// Le nom employé par §7.1.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Revoked => "revoked",
        }
    }

    /// Relire un état.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|state| state.slug() == value)
    }

    /// Les états atteignables depuis celui-ci.
    ///
    /// # Ce que `revoked` dit, et que `rejected` ne dit pas
    ///
    /// Une décision **approuvée** peut être révoquée : c'est ce qui arrive quand on découvre après
    /// coup qu'elle n'aurait pas dû l'être. Une décision rejetée ne se révoque pas — il n'y a rien
    /// à défaire. Et une révocation ne ramène pas à `proposed` : la trace de l'approbation reste,
    /// invariant 12.
    #[must_use]
    pub fn allowed(self) -> &'static [Self] {
        match self {
            Self::Proposed => &[Self::Approved, Self::Rejected],
            Self::Approved => &[Self::Revoked],
            Self::Rejected | Self::Revoked => &[],
        }
    }
}

impl fmt::Display for DecisionState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Une décision — §7.1.
///
/// Le champ `decision_type` porte le nom que §7.1 lui donne, et clippy le trouve redondant avec
/// celui du type. `CLAUDE.md` tranche : « les objets […] sont ceux de `SPEC_V1.md` §7.1 […] **sous
/// leur nom**. Aucun vocabulaire parallèle. » Le renommer en `kind` créerait exactement ce
/// vocabulaire parallèle, pour une gêne de lecture d'un mot.
#[expect(
    clippy::struct_field_names,
    reason = "le nom du champ est celui de SPEC_V1 §7.1"
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    id: Id<DecisionKind>,
    decision_type: String,
    rationale: String,
    made_by: String,
    state: DecisionState,
}

impl Decision {
    /// Proposer une décision.
    ///
    /// # La justification n'est pas facultative
    ///
    /// §7.1 porte `rationale` et `evidence_refs`, et §20 fait de la décision l'objet que la
    /// gouvernance relit. Une décision sans justification consigne qu'un choix a eu lieu, jamais
    /// pourquoi — c'est-à-dire exactement ce qui manque six mois plus tard.
    ///
    /// # Errors
    ///
    /// [`DecisionError::EmptyField`] pour un type, une justification ou un auteur vides.
    pub fn propose(
        id: Id<DecisionKind>,
        decision_type: &str,
        rationale: &str,
        made_by: &str,
    ) -> Result<Self, DecisionError> {
        for (field, value) in [
            ("decision_type", decision_type),
            ("rationale", rationale),
            ("made_by", made_by),
        ] {
            if value.trim().is_empty() {
                return Err(DecisionError::EmptyField { field });
            }
        }
        Ok(Self {
            id,
            decision_type: decision_type.to_owned(),
            rationale: rationale.to_owned(),
            made_by: made_by.to_owned(),
            state: DecisionState::Proposed,
        })
    }

    /// Franchir une transition.
    ///
    /// # Errors
    ///
    /// [`DecisionError::Forbidden`] quand la table la refuse.
    pub fn moved_to(mut self, next: DecisionState) -> Result<Self, DecisionError> {
        if !self.state.allowed().contains(&next) {
            return Err(DecisionError::Forbidden {
                from: self.state,
                to: next,
            });
        }
        self.state = next;
        Ok(self)
    }

    /// Son identifiant.
    #[must_use]
    pub const fn id(&self) -> Id<DecisionKind> {
        self.id
    }

    /// Son type.
    #[must_use]
    pub fn decision_type(&self) -> &str {
        &self.decision_type
    }

    /// Sa justification.
    #[must_use]
    pub fn rationale(&self) -> &str {
        &self.rationale
    }

    /// Qui l'a prise.
    #[must_use]
    pub fn made_by(&self) -> &str {
        &self.made_by
    }

    /// Son état.
    #[must_use]
    pub const fn state(&self) -> DecisionState {
        self.state
    }
}

/// L'état d'une demande d'approbation, tel que §7.1 l'énumère.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApprovalState {
    /// En attente.
    Pending,
    /// Approuvée.
    Approved,
    /// Rejetée.
    Rejected,
    /// Expirée.
    Expired,
    /// Annulée par le demandeur.
    Cancelled,
}

impl ApprovalState {
    /// Les cinq de §7.1.
    pub const ALL: [Self; 5] = [
        Self::Pending,
        Self::Approved,
        Self::Rejected,
        Self::Expired,
        Self::Cancelled,
    ];

    /// Le nom employé par §7.1.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Expired => "expired",
            Self::Cancelled => "cancelled",
        }
    }

    /// Relire un état.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|state| state.slug() == value)
    }

    /// Vrai quand la demande attend encore quelqu'un.
    #[must_use]
    pub const fn is_pending(self) -> bool {
        matches!(self, Self::Pending)
    }
}

impl fmt::Display for ApprovalState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Une demande d'approbation humaine — §7.1 : « objet explicite permettant de **suspendre
/// durablement** un workflow en attente d'une décision humaine ».
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRequest {
    id: Id<Approval>,
    action: String,
    impact: String,
    requested_by: String,
    required_roles: Vec<String>,
    state: ApprovalState,
}

impl ApprovalRequest {
    /// Demander une approbation.
    ///
    /// # Pourquoi les rôles requis ne peuvent pas être vides
    ///
    /// Une demande que personne n'est désigné pour trancher attend indéfiniment, et « suspendre
    /// durablement » deviendrait « suspendre pour toujours ». §20 fait de l'approbation un acte
    /// nominatif ; l'exiger ici est la forme opposable de cette phrase.
    ///
    /// # Errors
    ///
    /// [`DecisionError::EmptyField`] pour une action, un impact ou un demandeur vides, et
    /// [`DecisionError::NoRequiredRoles`] quand aucun rôle n'est désigné.
    pub fn request(
        id: Id<Approval>,
        action: &str,
        impact: &str,
        requested_by: &str,
        required_roles: Vec<String>,
    ) -> Result<Self, DecisionError> {
        for (field, value) in [
            ("action", action),
            ("impact", impact),
            ("requested_by", requested_by),
        ] {
            if value.trim().is_empty() {
                return Err(DecisionError::EmptyField { field });
            }
        }
        if required_roles.iter().all(|role| role.trim().is_empty()) {
            return Err(DecisionError::NoRequiredRoles);
        }
        Ok(Self {
            id,
            action: action.to_owned(),
            impact: impact.to_owned(),
            requested_by: requested_by.to_owned(),
            required_roles,
            state: ApprovalState::Pending,
        })
    }

    /// Répondre à la demande.
    ///
    /// # Errors
    ///
    /// [`DecisionError::AlreadyAnswered`] quand elle ne l'attend plus : une réponse à une demande
    /// close écraserait la première, et c'est la première qui a débloqué le workflow.
    pub fn answered(mut self, state: ApprovalState) -> Result<Self, DecisionError> {
        if !self.state.is_pending() {
            return Err(DecisionError::AlreadyAnswered { state: self.state });
        }
        self.state = state;
        Ok(self)
    }

    /// Son identifiant.
    #[must_use]
    pub const fn id(&self) -> Id<Approval> {
        self.id
    }

    /// L'action soumise.
    #[must_use]
    pub fn action(&self) -> &str {
        &self.action
    }

    /// Son impact annoncé.
    #[must_use]
    pub fn impact(&self) -> &str {
        &self.impact
    }

    /// Qui l'a demandée.
    #[must_use]
    pub fn requested_by(&self) -> &str {
        &self.requested_by
    }

    /// Les rôles habilités à trancher.
    #[must_use]
    pub fn required_roles(&self) -> &[String] {
        &self.required_roles
    }

    /// Son état.
    #[must_use]
    pub const fn state(&self) -> ApprovalState {
        self.state
    }
}

/// Ce qui empêche une décision ou une demande d'exister ou d'avancer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionError {
    /// Un champ obligatoire vide.
    EmptyField {
        /// Lequel.
        field: &'static str,
    },
    /// Aucun rôle désigné pour trancher.
    NoRequiredRoles,
    /// Une transition que la table refuse.
    Forbidden {
        /// D'où.
        from: DecisionState,
        /// Vers où.
        to: DecisionState,
    },
    /// Une demande déjà tranchée.
    AlreadyAnswered {
        /// Son état.
        state: ApprovalState,
    },
}

impl fmt::Display for DecisionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(formatter, "le champ « {field} » est vide"),
            Self::NoRequiredRoles => formatter.write_str(
                "une demande que personne n'est désigné pour trancher attend indéfiniment",
            ),
            Self::Forbidden { from, to } => {
                write!(formatter, "« {from} » ne mène pas à « {to} »")
            }
            Self::AlreadyAnswered { state } => {
                write!(formatter, "cette demande est déjà « {state} »")
            }
        }
    }
}

impl std::error::Error for DecisionError {}
