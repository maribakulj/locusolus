//! Ce qui autorise une exception, et ce que l'exception laisse comme trace — §21.4, §21.9.

use std::fmt;

/// Une dérogation nommée.
///
/// # Pourquoi un type et pas un booléen
///
/// §21.6 dit qu'un downgrade est interdit « sauf approbation **explicite** ». Un `bool` serait
/// explicite au sens du compilateur et anonyme au sens de l'audit : il dirait que quelqu'un a
/// approuvé, sans dire qui ni pourquoi, c'est-à-dire précisément ce qu'un audit vient chercher.
///
/// L'acteur et la raison sont donc exigés, non vides. Le ticket est optionnel parce que tous les
/// déploiements n'ont pas de système de tickets — mais son absence ne dispense de rien.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Approval {
    actor: String,
    reason: String,
    ticket: Option<String>,
}

impl Approval {
    /// Enregistrer une dérogation.
    ///
    /// # Errors
    ///
    /// [`ApprovalError::EmptyActor`] ou [`ApprovalError::EmptyReason`]. Une raison vide est le cas
    /// qui compte : c'est celui qu'on écrit quand on est pressé, et celui qu'on relit six mois plus
    /// tard sans pouvoir reconstituer ce qui semblait évident.
    pub fn new(actor: &str, reason: &str) -> Result<Self, ApprovalError> {
        if actor.trim().is_empty() {
            return Err(ApprovalError::EmptyActor);
        }
        if reason.trim().is_empty() {
            return Err(ApprovalError::EmptyReason);
        }
        Ok(Self {
            actor: actor.to_owned(),
            reason: reason.to_owned(),
            ticket: None,
        })
    }

    /// Rattacher un ticket.
    #[must_use]
    pub fn with_ticket(mut self, ticket: &str) -> Self {
        self.ticket = Some(ticket.to_owned());
        self
    }

    /// Qui a approuvé.
    #[must_use]
    pub fn actor(&self) -> &str {
        &self.actor
    }

    /// Pourquoi.
    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }

    /// Le ticket, s'il y en a un.
    #[must_use]
    pub fn ticket(&self) -> Option<&str> {
        self.ticket.as_deref()
    }
}

/// Ce qui empêche une approbation d'exister.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalError {
    /// Personne ne l'a signée.
    EmptyActor,
    /// Aucune raison n'est donnée.
    EmptyReason,
}

impl fmt::Display for ApprovalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyActor => {
                formatter.write_str("une approbation sans acteur n'est approuvée par personne")
            }
            Self::EmptyReason => formatter
                .write_str("une approbation sans raison est celle qu'on ne peut plus reconstituer"),
        }
    }
}

impl std::error::Error for ApprovalError {}

/// Ce qu'un événement de sécurité constate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityEventKind {
    /// Un niveau d'isolation inférieur à celui exigé a été appliqué, sous approbation.
    SandboxDowngrade,
    /// Un montage que la politique interdit a été déclaré, sous approbation.
    ForbiddenMountApproved,
}

impl SecurityEventKind {
    /// Le nom de l'événement.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::SandboxDowngrade => "sandbox.downgrade",
            Self::ForbiddenMountApproved => "sandbox.forbidden_mount_approved",
        }
    }
}

/// Un événement de sécurité — §21.9.
///
/// « Les événements de sécurité sont append-only et séparés des logs applicatifs ordinaires. Ils
/// contiennent l'acteur, le scope, la décision de politique et les preuves techniques, **sans
/// enregistrer les secrets**. »
///
/// Les quatre champs du texte sont donc obligatoires, et la dernière clause est **exécutoire** :
/// [`SecurityEvent::new`] refuse une preuve qui porte un marqueur de secret. Un journal de sécurité
/// qui recopierait un token serait le seul endroit du système où l'on aurait accumulé, exprès et
/// durablement, ce qu'on cherche à protéger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityEvent {
    kind: SecurityEventKind,
    actor: String,
    scope: String,
    decision: String,
    evidence: Vec<String>,
}

impl SecurityEvent {
    /// Consigner un événement.
    ///
    /// # Errors
    ///
    /// [`SecurityEventError::Empty`] si l'un des quatre champs du texte manque,
    /// [`SecurityEventError::LeakedSecret`] si une preuve porte un marqueur de secret.
    pub fn new(
        kind: SecurityEventKind,
        actor: &str,
        scope: &str,
        decision: &str,
        evidence: Vec<String>,
    ) -> Result<Self, SecurityEventError> {
        for (field, value) in [("actor", actor), ("scope", scope), ("decision", decision)] {
            if value.trim().is_empty() {
                return Err(SecurityEventError::Empty { field });
            }
        }
        for line in &evidence {
            if let Some(marker) = secret_marker(line) {
                return Err(SecurityEventError::LeakedSecret { marker });
            }
        }
        Ok(Self {
            kind,
            actor: actor.to_owned(),
            scope: scope.to_owned(),
            decision: decision.to_owned(),
            evidence,
        })
    }

    /// Ce qui est constaté.
    #[must_use]
    pub const fn kind(&self) -> SecurityEventKind {
        self.kind
    }

    /// Qui.
    #[must_use]
    pub fn actor(&self) -> &str {
        &self.actor
    }

    /// Sur quoi.
    #[must_use]
    pub fn scope(&self) -> &str {
        &self.scope
    }

    /// La décision de politique.
    #[must_use]
    pub fn decision(&self) -> &str {
        &self.decision
    }

    /// Les preuves techniques.
    #[must_use]
    pub fn evidence(&self) -> &[String] {
        &self.evidence
    }
}

/// Les marqueurs qui trahissent un secret dans une preuve.
///
/// Assemblés par `concat!` : ce sont des formes réelles, et une table écrite d'un bloc ferait de ce
/// fichier un endroit où un scanner de secrets trouve des motifs. Même précaution qu'en W3.a pour
/// les marqueurs de frappe d'identifiant, et pour une raison voisine — une garde ne doit pas être
/// le problème qu'elle cherche.
pub const SECRET_MARKERS: [&str; 8] = [
    concat!("BEGIN ", "PRIVATE KEY"),
    concat!("BEGIN ", "RSA PRIVATE KEY"),
    concat!("AKIA", ""),
    concat!("Bearer", " "),
    concat!("authorization", ":"),
    concat!("password", "="),
    concat!("api", "_key="),
    concat!("secret", "_access_key"),
];

/// Le marqueur de secret que porte cette ligne, s'il y en a un.
///
/// La comparaison est faite en minuscules pour les marqueurs qui sont des noms de champ, et telle
/// quelle pour ceux qui sont des préfixes de valeur — `AKIA` en minuscules attraperait des mots
/// ordinaires.
#[must_use]
pub fn secret_marker(line: &str) -> Option<&'static str> {
    let lowered = line.to_lowercase();
    SECRET_MARKERS.into_iter().find(|marker| {
        if marker.chars().any(char::is_uppercase) {
            line.contains(marker)
        } else {
            lowered.contains(*marker)
        }
    })
}

/// Ce qui empêche un événement de sécurité d'exister.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityEventError {
    /// Un des champs exigés par §21.9 est vide.
    Empty {
        /// Lequel.
        field: &'static str,
    },
    /// Une preuve porte un secret.
    LeakedSecret {
        /// Le marqueur reconnu.
        marker: &'static str,
    },
}

impl fmt::Display for SecurityEventError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty { field } => write!(
                formatter,
                "§21.9 exige « {field} » : un événement de sécurité incomplet ne s'audite pas"
            ),
            Self::LeakedSecret { marker } => write!(
                formatter,
                "une preuve porte « {marker} » : le journal de sécurité serait l'endroit où l'on accumule ce qu'on protège"
            ),
        }
    }
}

impl std::error::Error for SecurityEventError {}
