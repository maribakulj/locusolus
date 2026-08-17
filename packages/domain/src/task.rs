//! La tâche et sa machine à états — `docs/SPEC_V1.md` §7.1.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Les états d'une tâche, tels que §7.1 les nomme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    /// Proposée.
    Proposed,
    /// En file.
    Queued,
    /// Attribuée à un worker sous lease.
    Leased,
    /// En cours.
    Running,
    /// En attente d'un outil.
    WaitingForTool,
    /// En attente d'un humain.
    WaitingForHuman,
    /// En attente d'une revue.
    WaitingForReview,
    /// Le worker a rempli son contrat **technique**. Voir la note du module.
    Succeeded,
    /// Échouée.
    Failed,
    /// Annulée.
    Cancelled,
    /// Expirée.
    TimedOut,
    /// Orpheline : le lease est perdu et personne ne travaille dessus.
    Orphaned,
    /// Acceptée par l'institution.
    Accepted,
    /// Rejetée par l'institution.
    Rejected,
    /// Remplacée.
    Superseded,
}

impl TaskState {
    /// Tous les états.
    pub const ALL: [Self; 15] = [
        Self::Proposed,
        Self::Queued,
        Self::Leased,
        Self::Running,
        Self::WaitingForTool,
        Self::WaitingForHuman,
        Self::WaitingForReview,
        Self::Succeeded,
        Self::Failed,
        Self::Cancelled,
        Self::TimedOut,
        Self::Orphaned,
        Self::Accepted,
        Self::Rejected,
        Self::Superseded,
    ];

    /// Les états atteignables depuis celui-ci — les flèches de §7.1, transcrites.
    ///
    /// Une flèche absente est une transition **interdite**, et c'est ce qui rend cette table utile :
    /// elle attrape une transition que personne n'a autorisée, au lieu de la laisser passer parce
    /// qu'aucun `if` ne la mentionnait.
    #[must_use]
    // Une transcription se lit ligne à ligne, en regard du texte. `Proposed` et `Orphaned` mènent
    // tous deux à `Queued` par coïncidence, pas par parenté : l'un n'a jamais été attribué, l'autre
    // a perdu son lease. Les fondre en un seul bras ferait disparaître cette différence du code, et
    // il faudrait les séparer à nouveau le jour où l'un des deux gagne une sortie.
    #[allow(clippy::match_same_arms)]
    pub fn allowed(self) -> &'static [Self] {
        match self {
            Self::Proposed => &[Self::Queued],
            Self::Queued => &[Self::Leased],
            // `leased/running → orphaned` : le lease se perd avant même que le travail commence.
            Self::Leased => &[Self::Running, Self::Orphaned],
            Self::Running => &[
                Self::WaitingForTool,
                Self::WaitingForHuman,
                Self::WaitingForReview,
                Self::Succeeded,
                Self::Failed,
                Self::Cancelled,
                Self::TimedOut,
                Self::Orphaned,
            ],
            // Une attente revient à l'exécution. Elle ne saute pas à `succeeded` : ce serait rendre
            // un résultat sans avoir repris le travail qu'on avait suspendu.
            Self::WaitingForTool | Self::WaitingForHuman | Self::WaitingForReview => {
                &[Self::Running]
            }
            // `orphaned → queued` : la tâche repart, sur un autre attempt.
            Self::Orphaned => &[Self::Queued],
            // Le verdict institutionnel. C'est la seule sortie de `succeeded`, et elle n'est pas
            // automatique — voir la note du module.
            Self::Succeeded => &[Self::Accepted, Self::Rejected, Self::Superseded],
            Self::Failed
            | Self::Cancelled
            | Self::TimedOut
            | Self::Accepted
            | Self::Rejected
            | Self::Superseded => &[],
        }
    }

    /// Vrai quand aucun état ne sort de celui-ci.
    #[must_use]
    pub fn is_terminal(self) -> bool {
        self.allowed().is_empty()
    }

    /// Vrai si `next` est atteignable depuis cet état.
    #[must_use]
    pub fn can_reach(self, next: Self) -> bool {
        self.allowed().contains(&next)
    }

    /// La forme textuelle canonique.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Queued => "queued",
            Self::Leased => "leased",
            Self::Running => "running",
            Self::WaitingForTool => "waiting_for_tool",
            Self::WaitingForHuman => "waiting_for_human",
            Self::WaitingForReview => "waiting_for_review",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::TimedOut => "timed_out",
            Self::Orphaned => "orphaned",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Superseded => "superseded",
        }
    }
}

impl fmt::Display for TaskState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Le refus d'une transition, avec les sorties possibles.
///
/// Nommer les sorties plutôt que dire « transition invalide » : la réponse est déjà dans la table,
/// et faire relire un diagramme pour l'y trouver est une dépense inutile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForbiddenTransition {
    /// L'état de départ.
    pub from: TaskState,
    /// L'état demandé.
    pub to: TaskState,
    /// Ce qui était possible.
    pub allowed: Vec<TaskState>,
}

impl fmt::Display for ForbiddenTransition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.allowed.is_empty() {
            return write!(
                formatter,
                "`{}` est terminal : aucune transition n'en sort, et surtout pas vers `{}`",
                self.from, self.to
            );
        }
        let names: Vec<&str> = self.allowed.iter().map(|state| state.as_str()).collect();
        write!(
            formatter,
            "`{}` → `{}` n'existe pas en §7.1 ; sorties possibles : {}",
            self.from,
            self.to,
            names.join(", ")
        )
    }
}

impl std::error::Error for ForbiddenTransition {}

/// Passer d'un état de tâche à un autre.
///
/// # Errors
///
/// Rend [`ForbiddenTransition`] pour toute flèche absente de §7.1.
pub fn transition(from: TaskState, to: TaskState) -> Result<TaskState, ForbiddenTransition> {
    if from.can_reach(to) {
        return Ok(to);
    }
    Err(ForbiddenTransition {
        from,
        to,
        allowed: from.allowed().to_vec(),
    })
}

/// Une tâche `succeeded` a-t-elle des claims validés ?
///
/// **Non**, et c'est la phrase que §7.1 ajoute juste après la liste des états : « une tâche
/// `succeeded` signifie que le worker a rempli son contrat technique. Elle ne signifie pas que ses
/// claims sont validés. »
///
/// Cette fonction existe pour que la réponse soit écrite quelque part plutôt que sous-entendue.
/// Elle rend toujours `false`, et un test le verrouille : le jour où quelqu'un voudra la faire
/// rendre `true` pour un cas particulier, il faudra qu'il l'écrive, et le diff le montrera.
#[must_use]
pub const fn implies_validated_claims(_state: TaskState) -> bool {
    false
}
