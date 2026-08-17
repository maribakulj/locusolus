//! La `ContextView` — `docs/SPEC_V1.md` §16.2.
//!
//! # Ce qu'elle répond
//!
//! « Que savait-on, et à quel instant du journal ? » §16.2 : « une `ContextView` est immuable,
//! adressée par hash et rattachée à l'exécution. Elle permet de savoir exactement ce que l'agent
//! **pouvait** connaître. »
//!
//! Les deux mots qui font tout le travail sont **immuable** et **watermark**. Sans immuabilité, la
//! vue dit ce qu'on sait aujourd'hui plutôt que ce qu'on savait ; sans watermark, elle ne dit pas
//! de quand date ce « aujourd'hui ».
//!
//! # Ce que W7.b a rendu possible
//!
//! La vue se **construit filtrée** : les cas adverses de `contamination.rs` existaient avant elle,
//! donc ils n'ont pas pu être écrits pour qu'elle passe. Ce que le filtre écarte est consigné dans
//! `redactions` (§16.2) plutôt que supprimé en silence — une exclusion non nommée est
//! indistinguable d'un oubli, comme pour le dossier de revue.

use std::fmt;

use locus_domain::{Confidentiality, ContentHash, RevisionId};

use crate::contamination::{ContextItem, Recipient, inspect};

/// Ce qu'un élément écarté laisse comme trace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redaction {
    /// La révision écartée.
    pub revision: RevisionId,
    /// Pourquoi.
    pub reason: String,
}

impl fmt::Display for Redaction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} : {}", self.revision, self.reason)
    }
}

/// Une vue de contexte, immuable.
///
/// # Ce qu'on ne peut pas en faire
///
/// L'augmenter. Il n'existe aucune méthode qui ajoute un élément après construction : une vue qui
/// grandirait cesserait de dire ce que l'agent **pouvait** connaître au moment où elle a été
/// arrêtée. Pour voir plus, on en construit une autre, avec son propre watermark et son propre
/// hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextView {
    included: Vec<RevisionId>,
    redactions: Vec<Redaction>,
    confidentiality_ceiling: Confidentiality,
    source_event_watermark: u64,
    content_hash: ContentHash,
}

impl ContextView {
    /// Construire une vue **filtrée** pour un destinataire, arrêtée à un watermark.
    ///
    /// # Le filtre est celui de W7.b
    ///
    /// Chaque élément passe par [`crate::contamination::inspect`], et ce qu'il signale est écarté
    /// **en le consignant**. La vue ne peut donc pas contenir une contamination que W7.b sait
    /// nommer — et si W7.b apprend une forme de plus, la vue s'en protège sans être modifiée.
    ///
    /// # Errors
    ///
    /// [`ContextViewError::BeyondWatermark`] quand un élément provient d'un événement postérieur au
    /// watermark : une vue qui contiendrait l'avenir ne dirait plus ce qu'on savait, et c'est la
    /// faute que §16.2 rend impossible à détecter après coup si on ne la refuse pas ici.
    pub fn build(
        candidates: &[(ContextItem, u64)],
        recipient: &Recipient,
        source_event_watermark: u64,
        content_hash: ContentHash,
    ) -> Result<Self, ContextViewError> {
        let mut included = Vec::new();
        let mut redactions = Vec::new();

        for (item, position) in candidates {
            if *position > source_event_watermark {
                return Err(ContextViewError::BeyondWatermark {
                    revision: item.revision,
                    position: *position,
                    watermark: source_event_watermark,
                });
            }
            let findings = inspect(std::slice::from_ref(item), recipient);
            if findings.is_empty() {
                included.push(item.revision);
            } else {
                redactions.push(Redaction {
                    revision: item.revision,
                    reason: findings
                        .iter()
                        .map(|finding| finding.kind.slug())
                        .collect::<Vec<_>>()
                        .join(", "),
                });
            }
        }

        Ok(Self {
            included,
            redactions,
            confidentiality_ceiling: recipient.clearance,
            source_event_watermark,
            content_hash,
        })
    }

    /// Ce que la vue contient.
    #[must_use]
    pub fn included(&self) -> &[RevisionId] {
        &self.included
    }

    /// Ce qu'elle a écarté, et pourquoi.
    ///
    /// §16.2 porte `redactions` : ce qui a été retiré fait partie de ce que la vue dit. Une
    /// exclusion silencieuse rendrait deux vues indiscernables — celle qui n'avait rien à écarter
    /// et celle qui a tout écarté.
    #[must_use]
    pub fn redactions(&self) -> &[Redaction] {
        &self.redactions
    }

    /// Le plafond de confidentialité appliqué.
    #[must_use]
    pub const fn confidentiality_ceiling(&self) -> Confidentiality {
        self.confidentiality_ceiling
    }

    /// L'instant du journal auquel elle est arrêtée.
    #[must_use]
    pub const fn source_event_watermark(&self) -> u64 {
        self.source_event_watermark
    }

    /// Son hash de contenu.
    #[must_use]
    pub const fn content_hash(&self) -> &ContentHash {
        &self.content_hash
    }

    /// Vrai quand cette vue aurait pu connaître un événement à cette position.
    ///
    /// La question que §16.2 existe pour rendre décidable. Un événement au-delà du watermark
    /// n'était pas connaissable, et une revue qui reprocherait de l'avoir ignoré reprocherait de
    /// n'être pas devin.
    #[must_use]
    pub const fn could_know(&self, position: u64) -> bool {
        position <= self.source_event_watermark
    }
}

/// Ce qui empêche une vue d'exister.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextViewError {
    /// Un élément postérieur au watermark.
    BeyondWatermark {
        /// La révision fautive.
        revision: RevisionId,
        /// Sa position dans le journal.
        position: u64,
        /// Le watermark de la vue.
        watermark: u64,
    },
}

impl fmt::Display for ContextViewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BeyondWatermark {
                revision,
                position,
                watermark,
            } => write!(
                formatter,
                "« {revision} » vient de la position {position}, au-delà du watermark {watermark} \
                 : une vue qui contient l'avenir ne dit plus ce qu'on savait"
            ),
        }
    }
}

impl std::error::Error for ContextViewError {}
