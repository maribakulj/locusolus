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
use locus_protocol::{Id, Timestamp, id::Agent};

use crate::contamination::{ContextItem, Recipient, inspect};

/// Qui voit le travail de qui — un **port**.
///
/// ADR 0016 décision 11 : « recâbler une relation change qui peut lire quoi ». La réponse vient du
/// domaine de coordination, mais ce crate ne l'importe pas : il pose la seule question dont la
/// construction d'une vue a besoin, comme `EpistemicIndex` le fait dans l'autre sens.
///
/// # Il retire, il n'ajoute jamais
///
/// §16.3 : les embeddings « ne contournent pas les ACL ». Une relation de coordination ne le peut
/// pas davantage, et la garantie est structurelle plutôt que promise — ce port ne rend qu'un
/// `bool` que [`ContextView::build_under`] combine **par un et logique** avec le filtre de
/// contamination. Il n'existe aucun chemin par lequel une visibilité déclarée fasse entrer ce
/// qu'un autre refus écarte.
pub trait Visible {
    /// `viewer` peut-il voir ce que `producer` a produit ?
    fn sees(&self, viewer: Id<Agent>, producer: Id<Agent>) -> bool;
}

/// Le port dans sa forme sans contrainte : tout est visible.
///
/// C'est ce que [`ContextView::build`] passe, et c'est **le même calcul** que sous une visibilité
/// déclarée — pas un second chemin. Un chemin « sans visibilité » écrit à part divergerait le jour
/// où l'un des deux est corrigé, et personne ne saurait lequel des deux dit la vérité.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unrestricted;

impl Visible for Unrestricted {
    fn sees(&self, _viewer: Id<Agent>, _producer: Id<Agent>) -> bool {
        true
    }
}

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

/// Ce que le filtre a retenu et ce qu'il a écarté — une vue **pas encore scellée**.
///
/// # Pourquoi ce type existe — `W20.ac`
///
/// Une [`ContextView`] est « adressée par hash » (§16.2), et son empreinte porte sur le document
/// qui la transporte : c'est celui-là que le worker recalcule avant de démarrer (§12.3). Or ce
/// document nomme ce que le filtre a écarté — il ne peut donc pas être écrit avant que le filtre ait
/// tourné, ni hashé avant d'être écrit.
///
/// Sans ce type, la seule façon d'obtenir le résultat du filtre était de fournir d'avance
/// l'empreinte qu'on cherchait à calculer. `Filtered` nomme le moment qui manquait, et il rend le
/// sceau opposable : [`Filtered::seal`] est le seul endroit d'où une `ContextView` sort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filtered {
    included: Vec<RevisionId>,
    redactions: Vec<Redaction>,
    confidentiality_ceiling: Confidentiality,
    source_event_watermark: u64,
}

impl Filtered {
    /// Ce que le filtre a retenu.
    #[must_use]
    pub fn included(&self) -> &[RevisionId] {
        &self.included
    }

    /// Ce qu'il a écarté, et pourquoi.
    #[must_use]
    pub fn redactions(&self) -> &[Redaction] {
        &self.redactions
    }

    /// Le plafond de confidentialité appliqué.
    #[must_use]
    pub const fn confidentiality_ceiling(&self) -> Confidentiality {
        self.confidentiality_ceiling
    }

    /// L'instant du journal auquel le filtre s'est arrêté.
    #[must_use]
    pub const fn source_event_watermark(&self) -> u64 {
        self.source_event_watermark
    }

    /// Sceller ce contenu sous une empreinte.
    ///
    /// Ce crate ne calcule pas l'empreinte, et ce n'est pas un oubli : elle porte sur le **document
    /// du fil** — la forme de `schemas/lep/1.0/context-view.schema.json` —, que ce crate ne
    /// construit pas. La calculer ici sur une autre forme en donnerait une seconde définition, et le
    /// worker recalculerait la sienne : deux empreintes pour une vue, donc un refus d'intégrité sur
    /// une vue correcte.
    #[must_use]
    pub fn seal(self, content_hash: ContentHash) -> ContextView {
        ContextView {
            included: self.included,
            redactions: self.redactions,
            confidentiality_ceiling: self.confidentiality_ceiling,
            source_event_watermark: self.source_event_watermark,
            content_hash,
        }
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
        at: Timestamp,
    ) -> Result<Self, ContextViewError> {
        Self::build_under(
            candidates,
            recipient,
            source_event_watermark,
            content_hash,
            &Unrestricted,
            at,
        )
    }

    /// Le contenu filtré, **avant** qu'une empreinte le scelle.
    ///
    /// # Pourquoi les deux moments sont séparés — `W20.ac`
    ///
    /// [`ContextView::build`] reçoit son `content_hash` de l'appelant, et **rien n'attache cette
    /// empreinte au contenu** : elle est prise telle quelle. C'était sans conséquence tant que
    /// personne ne la vérifiait ; §12.3 dit que le worker recalcule l'empreinte de la vue avant de
    /// démarrer, et l'empreinte qu'il recalcule porte sur le **document du fil**, qui ne peut pas
    /// exister avant que le filtre ait dit ce qu'il écarte.
    ///
    /// D'où l'ordre : filtrer, écrire le document, le hasher, sceller. Fondre les deux moments
    /// obligerait l'appelant à inventer une empreinte pour obtenir ce dont il a besoin pour la
    /// calculer.
    ///
    /// # Errors
    ///
    /// Les mêmes que [`ContextView::build`] — c'est le même calcul.
    pub fn filter(
        candidates: &[(ContextItem, u64)],
        recipient: &Recipient,
        source_event_watermark: u64,
        at: Timestamp,
    ) -> Result<Filtered, ContextViewError> {
        Self::filter_under(
            candidates,
            recipient,
            source_event_watermark,
            &Unrestricted,
            at,
        )
    }

    /// La même construction, sous une visibilité déclarée.
    ///
    /// # Ce que la visibilité change, et ce qu'elle ne change pas
    ///
    /// Elle **retire**. Un élément produit par un agent que le destinataire ne voit pas est écarté,
    /// et consigné comme le reste : une exclusion non nommée est indistinguable d'un oubli. Elle
    /// n'ajoute rien — un élément que la contamination écarte reste écarté quelle que soit la
    /// visibilité déclarée, parce que les deux filtres se composent par un **et**.
    ///
    /// Un élément qu'aucun agent n'a produit n'est pas concerné : la visibilité est une relation
    /// entre agents, et couper une vue de ses sources externes sous couvert d'organisation serait
    /// une autre faute.
    ///
    /// # Errors
    ///
    /// Les mêmes que [`ContextView::build`] — c'est le même calcul.
    pub fn build_under(
        candidates: &[(ContextItem, u64)],
        recipient: &Recipient,
        source_event_watermark: u64,
        content_hash: ContentHash,
        visible: &impl Visible,
        at: Timestamp,
    ) -> Result<Self, ContextViewError> {
        Ok(
            Self::filter_under(candidates, recipient, source_event_watermark, visible, at)?
                .seal(content_hash),
        )
    }

    /// Le contenu filtré sous une visibilité déclarée, **avant** qu'une empreinte le scelle.
    ///
    /// C'est le seul endroit où le filtrage s'écrit : [`ContextView::build`],
    /// [`ContextView::build_under`] et [`ContextView::filter`] y mènent tous. Un second chemin
    /// divergerait le jour où l'un des deux est corrigé, et personne ne saurait lequel dit la vérité
    /// — l'argument que [`Unrestricted`] porte déjà pour la visibilité.
    ///
    /// # Errors
    ///
    /// [`ContextViewError::BeyondWatermark`] quand un élément provient d'un événement postérieur au
    /// watermark.
    pub fn filter_under(
        candidates: &[(ContextItem, u64)],
        recipient: &Recipient,
        source_event_watermark: u64,
        visible: &impl Visible,
        at: Timestamp,
    ) -> Result<Filtered, ContextViewError> {
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
            let mut reasons: Vec<String> = inspect(std::slice::from_ref(item), recipient, at)
                .iter()
                .map(|finding| finding.kind.slug().to_owned())
                .collect();
            if let Some(producer) = item.produced_by
                && !visible.sees(recipient.agent_id, producer)
            {
                reasons.push(format!("not_visible_to_recipient({producer})"));
            }
            if reasons.is_empty() {
                included.push(item.revision);
            } else {
                redactions.push(Redaction {
                    revision: item.revision,
                    reason: reasons.join(", "),
                });
            }
        }

        Ok(Filtered {
            included,
            redactions,
            confidentiality_ceiling: recipient.clearance,
            source_event_watermark,
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
