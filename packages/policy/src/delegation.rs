//! L'autorité et la délégation — `docs/SPEC_V1.md` §20.4.
//!
//! # La phrase qui décide de la forme
//!
//! « Les actions d'un agent sont attribuées **au principal agentique et à la délégation humaine ou
//! institutionnelle qui les autorise**. »
//!
//! Deux attributions, pas une. Un journal qui ne retiendrait que l'agent ferait porter à un
//! programme une décision qu'un humain a autorisée ; un journal qui ne retiendrait que le délégant
//! effacerait qui a agi. [`Attribution`] porte donc les deux, sans accesseur qui les résumerait —
//! c'est la même règle que les deux verdicts de §19.
//!
//! # Quatre bornes, et chacune se franchit différemment
//!
//! §20.4 donne à une `Delegation` une portée, une liste d'actions, un plafond de budget, un plafond
//! de confidentialité, une fenêtre de validité et un caractère révocable. Les dépasser ne se
//! ressemble pas : agir hors portée est une erreur d'aiguillage, dépasser un plafond est une
//! demande trop grande, agir après expiration est une autorisation périmée, agir sous révocation est
//! une autorisation retirée. [`Refusal`] les nomme séparément parce que la correction diffère à
//! chaque fois — et parce qu'un « non autorisé » sans motif ne se corrige pas.
//!
//! # Une délégation irrévocable ne se révoque pas
//!
//! `revocable` est un champ du texte, donc il décide de quelque chose. Une délégation déclarée
//! irrévocable refuse la révocation au lieu de l'accepter silencieusement : accepter en apparence et
//! continuer d'autoriser serait la pire des deux réponses, puisque le délégant croirait avoir agi.

use std::collections::BTreeSet;
use std::fmt;

/// Ce qu'une délégation autorise et jusqu'où.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delegation {
    delegator: String,
    delegate: String,
    actions: BTreeSet<String>,
    scope: String,
    budget_ceiling: u64,
    confidentiality_ceiling: u8,
    valid_from: u64,
    expires_at: u64,
    revocable: bool,
    revoked: bool,
}

impl Delegation {
    /// Accorder une délégation.
    ///
    /// `valid_from` et `expires_at` sont des instants opaques que l'appelant fournit : ce module ne
    /// lit aucune horloge, pour la même raison que le moteur de politique ne lit aucun fait qu'on ne
    /// lui a pas donné — une autorisation qui dépendrait de l'heure qu'il est ici ne se rejouerait
    /// pas.
    ///
    /// # Errors
    ///
    /// [`DelegationError::EmptyField`] pour un délégant, un délégataire ou une portée vide ;
    /// [`DelegationError::NoAction`] pour une délégation qui n'autorise rien — elle passerait pour
    /// une autorisation alors qu'elle n'en est pas une ; [`DelegationError::EmptyWindow`] quand la
    /// fenêtre de validité est vide ou inversée, ce qui autoriserait pendant zéro instant tout en
    /// ayant l'air d'une délégation valide.
    #[expect(clippy::too_many_arguments, reason = "les huit champs de §20.4")]
    pub fn grant(
        delegator: &str,
        delegate: &str,
        actions: &[&str],
        scope: &str,
        budget_ceiling: u64,
        confidentiality_ceiling: u8,
        valid_from: u64,
        expires_at: u64,
        revocable: bool,
    ) -> Result<Self, DelegationError> {
        for (field, value) in [
            ("delegator", delegator),
            ("delegate", delegate),
            ("scope", scope),
        ] {
            if value.trim().is_empty() {
                return Err(DelegationError::EmptyField { field });
            }
        }
        if actions.is_empty() {
            return Err(DelegationError::NoAction);
        }
        if expires_at <= valid_from {
            return Err(DelegationError::EmptyWindow {
                valid_from,
                expires_at,
            });
        }
        Ok(Self {
            delegator: delegator.to_owned(),
            delegate: delegate.to_owned(),
            actions: actions.iter().map(|action| (*action).to_owned()).collect(),
            scope: scope.to_owned(),
            budget_ceiling,
            confidentiality_ceiling,
            valid_from,
            expires_at,
            revocable,
            revoked: false,
        })
    }

    /// Qui délègue.
    #[must_use]
    pub fn delegator(&self) -> &str {
        &self.delegator
    }

    /// À qui.
    #[must_use]
    pub fn delegate(&self) -> &str {
        &self.delegate
    }

    /// Vrai quand elle a été révoquée.
    #[must_use]
    pub const fn is_revoked(&self) -> bool {
        self.revoked
    }

    /// Révoquer.
    ///
    /// # Errors
    ///
    /// [`DelegationError::NotRevocable`] pour une délégation déclarée irrévocable. Accepter en
    /// apparence et continuer d'autoriser serait la pire des deux réponses : le délégant croirait
    /// avoir agi.
    pub fn revoke(mut self) -> Result<Self, DelegationError> {
        if !self.revocable {
            return Err(DelegationError::NotRevocable);
        }
        self.revoked = true;
        Ok(self)
    }

    /// Cette délégation autorise-t-elle `request` ?
    ///
    /// Les bornes sont examinées dans l'ordre où elles rendent la demande sans objet : une
    /// délégation révoquée ou périmée n'autorise plus rien, quelle que soit l'action ; ensuite
    /// seulement viennent la portée, l'action et les plafonds.
    #[must_use]
    pub fn authorises(&self, request: &Request) -> Authorisation {
        if self.revoked {
            return Authorisation::Refused(Refusal::Revoked);
        }
        if request.at < self.valid_from || request.at >= self.expires_at {
            return Authorisation::Refused(Refusal::Expired {
                at: request.at,
                valid_from: self.valid_from,
                expires_at: self.expires_at,
            });
        }
        if request.scope != self.scope {
            return Authorisation::Refused(Refusal::OutOfScope {
                requested: request.scope.clone(),
                granted: self.scope.clone(),
            });
        }
        if !self.actions.contains(&request.action) {
            return Authorisation::Refused(Refusal::ActionNotGranted {
                action: request.action.clone(),
            });
        }
        if request.budget > self.budget_ceiling {
            return Authorisation::Refused(Refusal::OverBudget {
                requested: request.budget,
                ceiling: self.budget_ceiling,
            });
        }
        if request.confidentiality > self.confidentiality_ceiling {
            return Authorisation::Refused(Refusal::OverConfidentiality {
                requested: request.confidentiality,
                ceiling: self.confidentiality_ceiling,
            });
        }
        Authorisation::Granted(Attribution {
            agent: request.agent.clone(),
            authorised_by: self.delegator.clone(),
        })
    }
}

/// Ce qu'un agent demande à faire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// Qui agit.
    pub agent: String,
    /// Quelle action.
    pub action: String,
    /// Dans quelle portée.
    pub scope: String,
    /// Combien elle coûterait.
    pub budget: u64,
    /// Quel niveau de confidentialité elle touche.
    pub confidentiality: u8,
    /// À quel instant.
    pub at: u64,
}

/// À qui une action est attribuée — §20.4, les **deux** principals.
///
/// Il n'existe aucun accesseur qui les résumerait. Ne garder que l'agent ferait porter à un
/// programme une décision qu'un humain a autorisée ; ne garder que le délégant effacerait qui a agi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribution {
    /// Le principal agentique.
    pub agent: String,
    /// La délégation humaine ou institutionnelle qui l'autorise.
    pub authorised_by: String,
}

/// Le verdict d'une délégation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Authorisation {
    /// Autorisé, et attribué aux deux principals.
    Granted(Attribution),
    /// Refusé, avec le motif.
    Refused(Refusal),
}

impl Authorisation {
    /// Vrai quand l'action est autorisée.
    #[must_use]
    pub const fn is_granted(&self) -> bool {
        matches!(self, Self::Granted(_))
    }
}

/// Pourquoi une délégation n'autorise pas.
///
/// Cinq motifs, parce que la correction diffère à chaque fois : demander une autre portée, faire
/// étendre la délégation, réduire la demande, la renouveler, ou constater qu'elle a été retirée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// La délégation a été révoquée.
    Revoked,
    /// Hors de la fenêtre de validité.
    Expired {
        /// L'instant demandé.
        at: u64,
        /// Le début de validité.
        valid_from: u64,
        /// La fin.
        expires_at: u64,
    },
    /// Hors de la portée accordée.
    OutOfScope {
        /// Ce qui est demandé.
        requested: String,
        /// Ce qui est accordé.
        granted: String,
    },
    /// Cette action n'est pas dans la liste.
    ActionNotGranted {
        /// Laquelle.
        action: String,
    },
    /// Au-delà du plafond de budget.
    OverBudget {
        /// Ce qui est demandé.
        requested: u64,
        /// Le plafond.
        ceiling: u64,
    },
    /// Au-delà du plafond de confidentialité.
    OverConfidentiality {
        /// Ce qui est demandé.
        requested: u8,
        /// Le plafond.
        ceiling: u8,
    },
}

impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Revoked => formatter.write_str("délégation révoquée"),
            Self::Expired {
                at,
                valid_from,
                expires_at,
            } => write!(
                formatter,
                "hors fenêtre : {at} n'est pas dans [{valid_from}, {expires_at})"
            ),
            Self::OutOfScope { requested, granted } => write!(
                formatter,
                "hors portée : « {requested} » demandé, « {granted} » accordé"
            ),
            Self::ActionNotGranted { action } => {
                write!(formatter, "action non déléguée : « {action} »")
            }
            Self::OverBudget { requested, ceiling } => {
                write!(formatter, "budget {requested} au-delà du plafond {ceiling}")
            }
            Self::OverConfidentiality { requested, ceiling } => write!(
                formatter,
                "confidentialité {requested} au-delà du plafond {ceiling}"
            ),
        }
    }
}

/// Ce qui empêche une délégation d'exister ou d'être retirée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelegationError {
    /// Un champ obligatoire vide.
    EmptyField {
        /// Lequel.
        field: &'static str,
    },
    /// Une délégation qui n'autorise aucune action.
    NoAction,
    /// Une fenêtre de validité vide ou inversée.
    EmptyWindow {
        /// Le début.
        valid_from: u64,
        /// La fin.
        expires_at: u64,
    },
    /// Une délégation déclarée irrévocable.
    NotRevocable,
}

impl fmt::Display for DelegationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(formatter, "« {field} » est vide"),
            Self::NoAction => formatter.write_str(
                "une délégation qui n'autorise aucune action passerait pour une autorisation \
                 alors qu'elle n'en est pas une",
            ),
            Self::EmptyWindow {
                valid_from,
                expires_at,
            } => write!(
                formatter,
                "fenêtre [{valid_from}, {expires_at}) vide : elle autoriserait pendant zéro \
                 instant tout en ayant l'air d'une délégation valide"
            ),
            Self::NotRevocable => formatter.write_str(
                "délégation irrévocable : l'accepter en apparence et continuer d'autoriser serait \
                 pire, car le délégant croirait avoir agi",
            ),
        }
    }
}

impl std::error::Error for DelegationError {}
