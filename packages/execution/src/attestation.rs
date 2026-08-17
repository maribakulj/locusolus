//! Ce que le worker atteste, et ce que la confrontation produit — `docs/SPEC_V1.md` §21.6.

use std::fmt;

use crate::approval::{Approval, SecurityEvent, SecurityEventError, SecurityEventKind};
use crate::level::SandboxLevel;
use crate::spec::SandboxSpec;

/// Ce que le worker déclare avoir réellement appliqué — §21.6.
///
/// « Le worker atteste le niveau **réellement appliqué**. » Ce type est donc le témoignage d'un
/// tiers, pas une copie de la demande : le distinguer de [`SandboxSpec`] est ce qui permet de
/// constater un écart. Les fondre — un seul type avec un champ `level` — rendrait le downgrade
/// littéralement inexprimable, ce qui n'est pas la même chose que l'empêcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxAttestation {
    applied_level: SandboxLevel,
    attested_by: String,
    evidence: Vec<String>,
    approval: Option<Approval>,
}

impl SandboxAttestation {
    /// Attester un niveau appliqué.
    ///
    /// # Errors
    ///
    /// [`AttestationError::EmptyAttester`] pour un témoignage anonyme, et
    /// [`AttestationError::NoEvidence`] pour un témoignage sans preuve — « j'ai appliqué S4 » sans
    /// rien qui le montre est une affirmation, et l'invariant 5 demande une attestation.
    pub fn new(
        applied_level: SandboxLevel,
        attested_by: &str,
        evidence: Vec<String>,
    ) -> Result<Self, AttestationError> {
        if attested_by.trim().is_empty() {
            return Err(AttestationError::EmptyAttester);
        }
        if evidence.iter().all(|line| line.trim().is_empty()) {
            return Err(AttestationError::NoEvidence);
        }
        Ok(Self {
            applied_level,
            attested_by: attested_by.to_owned(),
            evidence,
            approval: None,
        })
    }

    /// Joindre l'approbation qui autorise un niveau inférieur à celui exigé.
    #[must_use]
    pub fn with_approval(mut self, approval: Approval) -> Self {
        self.approval = Some(approval);
        self
    }

    /// Le niveau réellement appliqué.
    #[must_use]
    pub const fn applied_level(&self) -> SandboxLevel {
        self.applied_level
    }

    /// Qui atteste.
    #[must_use]
    pub fn attested_by(&self) -> &str {
        &self.attested_by
    }

    /// Ce qui le montre.
    #[must_use]
    pub fn evidence(&self) -> &[String] {
        &self.evidence
    }

    /// L'approbation jointe, s'il y en a une.
    #[must_use]
    pub const fn approval(&self) -> Option<&Approval> {
        self.approval.as_ref()
    }
}

/// Le verdict de la confrontation entre ce qui était exigé et ce qui a été appliqué.
///
/// # Pourquoi l'événement est dans la valeur de retour
///
/// §21.6 : « Un downgrade est interdit sauf approbation explicite **et** événement de sécurité. »
/// Les deux conditions sont conjointes, et la seconde est celle qu'on oublie — approuver est un
/// geste que quelqu'un pose, consigner est un geste que personne ne réclame.
///
/// D'où la forme : [`conformance`] **produit** l'événement et le met dans le verdict. On ne peut
/// pas accepter un downgrade sans tenir l'événement en main, parce qu'il n'existe pas d'autre
/// chemin qui accepte. Un `bool` accompagné d'une consigne « pensez à journaliser » aurait laissé
/// exactement le trou que §21.6 nomme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Conformance {
    /// Le niveau appliqué tient le plancher exigé, et aucun montage n'a demandé de dérogation.
    Conforms,
    /// Ce qui a été appliqué s'écarte de ce qui était exigé, sous approbation — voici les
    /// événements que cet écart produit.
    ///
    /// La liste ne peut pas être vide : un écart sans événement ne se construit pas.
    ApprovedDeviation {
        /// Les événements de sécurité produits par l'écart.
        events: Vec<SecurityEvent>,
    },
}

/// Confronter ce qui était exigé à ce qui a été appliqué — §21.6.
///
/// # Errors
///
/// [`AttestationError::Downgrade`] quand le niveau appliqué est sous le plancher **sans**
/// approbation. C'est le refus central de W4.a : un worker qui applique moins que demandé et que
/// personne n'a autorisé n'a pas exécuté la mission, il en a exécuté une autre.
///
/// [`AttestationError::Event`] si un événement ne peut pas être consigné — un écart qu'on ne sait
/// pas journaliser n'est pas un écart qu'on accepte.
pub fn conformance(
    spec: &SandboxSpec,
    attestation: &SandboxAttestation,
) -> Result<Conformance, AttestationError> {
    let mut events = Vec::new();

    if !attestation.applied_level().satisfies(spec.minimum_level()) {
        let Some(approval) = attestation.approval() else {
            return Err(AttestationError::Downgrade {
                required: spec.minimum_level(),
                applied: attestation.applied_level(),
            });
        };
        events.push(SecurityEvent::new(
            SecurityEventKind::SandboxDowngrade,
            approval.actor(),
            &format!("sandbox/{}", spec.profile()),
            &format!(
                "downgrade approuvé de {} vers {} : {}",
                spec.minimum_level().code(),
                attestation.applied_level().code(),
                approval.reason()
            ),
            attestation.evidence().to_vec(),
        )?);
    }

    // Les montages sous dérogation produisent leur propre événement, même quand le niveau, lui,
    // est tenu. Un socket de runtime monté dans une micro-VM reste un socket de runtime monté :
    // le confinement du niveau ne rachète pas le trou qu'on y a percé.
    for mount in spec.approved_mounts() {
        let Some(approval) = mount.approval() else {
            continue;
        };
        events.push(SecurityEvent::new(
            SecurityEventKind::ForbiddenMountApproved,
            approval.actor(),
            &format!("sandbox/{}{}", spec.profile(), mount.target()),
            &format!(
                "montage approuvé de {} vers {} : {}",
                mount.source(),
                mount.target(),
                approval.reason()
            ),
            vec![format!("source={}", mount.source())],
        )?);
    }

    if events.is_empty() {
        Ok(Conformance::Conforms)
    } else {
        Ok(Conformance::ApprovedDeviation { events })
    }
}

/// Ce qui empêche une attestation d'exister, ou de conclure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttestationError {
    /// Un témoignage anonyme.
    EmptyAttester,
    /// Un témoignage sans preuve.
    NoEvidence,
    /// Le niveau appliqué est sous le plancher exigé, sans approbation.
    Downgrade {
        /// Le plancher.
        required: SandboxLevel,
        /// Ce qui a été appliqué.
        applied: SandboxLevel,
    },
    /// L'événement de sécurité ne peut pas être consigné.
    Event(SecurityEventError),
}

impl From<SecurityEventError> for AttestationError {
    fn from(error: SecurityEventError) -> Self {
        Self::Event(error)
    }
}

impl fmt::Display for AttestationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyAttester => {
                formatter.write_str("une attestation anonyme n'atteste de rien d'opposable")
            }
            Self::NoEvidence => formatter
                .write_str("une attestation sans preuve est une affirmation ; §21.6 veut l'autre"),
            Self::Downgrade { required, applied } => write!(
                formatter,
                "{} appliqué là où {} était exigé, sans approbation : la mission exécutée n'est pas celle qui était demandée",
                applied.code(),
                required.code()
            ),
            Self::Event(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for AttestationError {}
