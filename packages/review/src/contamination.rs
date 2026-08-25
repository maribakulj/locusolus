//! La prévention de contamination — `docs/SPEC_V1.md` §16.6.
//!
//! # Pourquoi ce module s'écrit par cas adverses
//!
//! `docs/10` le signale comme « facile à rater et coûteux à réparer » : la prévention de
//! contamination « doit être testée par un cas adverse explicite et **pas seulement par
//! construction** ». La différence est celle entre « je ne vois pas comment ça arriverait » et
//! « voici comment on le fait arriver, et voici pourquoi ça échoue ».
//!
//! Chacune des cinq formes que §16.6 nomme a donc, dans `tests/contamination.rs`, un cas qui
//! **essaie** la contamination. Le module fournit ce qu'il faut pour la détecter ; les tests
//! fournissent l'adversaire.
//!
//! # Ce que ce module est
//!
//! Un ensemble de **constats**, pas un filtre. Il regarde un contexte déjà constitué et dit ce qui
//! y est contaminé. Un filtre qui empêcherait la contamination au moment de la construction serait
//! préférable — et W7.c le fera, pour la `ContextView` — mais un filtre non éprouvé est un filtre
//! qu'on croit efficace, et l'ordre voulu par `docs/10` est d'écrire l'adversaire d'abord.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use locus_domain::{Confidentiality, RevisionId};
use locus_protocol::{Id, id::Agent};

/// Les cinq formes de contamination que §16.6 nomme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Contamination {
    /// « Partage du raisonnement du générateur avec un reviewer aveugle. »
    GeneratorReasoningLeaked,
    /// « Propagation d'un claim réfuté comme contexte par défaut. »
    RefutedClaimPropagated,
    /// « Réutilisation d'une donnée confidentielle dans un modèle ou worker non autorisé. »
    ConfidentialDataOnUnauthorisedWorker,
    /// « Consensus circulaire où des agents se citent mutuellement sans source externe. »
    CircularConsensus,
    /// « Oubli des contradictions lors de la synthèse. »
    ContradictionDropped,
}

impl Contamination {
    /// Les cinq, dans l'ordre de §16.6.
    pub const ALL: [Self; 5] = [
        Self::GeneratorReasoningLeaked,
        Self::RefutedClaimPropagated,
        Self::ConfidentialDataOnUnauthorisedWorker,
        Self::CircularConsensus,
        Self::ContradictionDropped,
    ];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::GeneratorReasoningLeaked => "generator_reasoning_leaked",
            Self::RefutedClaimPropagated => "refuted_claim_propagated",
            Self::ConfidentialDataOnUnauthorisedWorker => {
                "confidential_data_on_unauthorised_worker"
            }
            Self::CircularConsensus => "circular_consensus",
            Self::ContradictionDropped => "contradiction_dropped",
        }
    }
}

impl fmt::Display for Contamination {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Un élément qu'un contexte porte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextItem {
    /// La révision citée.
    pub revision: RevisionId,
    /// Vrai quand cet élément est le raisonnement privé du générateur.
    pub is_generator_reasoning: bool,
    /// Vrai quand la revendication portée a été réfutée.
    pub is_refuted: bool,
    /// La classification de la donnée.
    pub classification: Confidentiality,
    /// Ce que cet élément cite — pour détecter un consensus circulaire.
    pub cites: Vec<RevisionId>,
    /// Vrai quand la source est extérieure au laboratoire.
    pub is_external_source: bool,
    /// L'agent qui l'a produit.
    pub produced_by: Option<Id<Agent>>,
}

/// Ce que le destinataire du contexte est autorisé à voir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipient {
    /// L'agent destinataire.
    pub agent_id: Id<Agent>,
    /// Le worker où il tourne.
    pub worker_id: String,
    /// Vrai quand la politique le rend aveugle au raisonnement du générateur.
    pub blind_to_generator: bool,
    /// Le plafond de confidentialité que son worker est habilité à recevoir.
    ///
    /// Un plafond, pas une liste : §16.2 porte `confidentiality_ceiling`, et un ordre croissant de
    /// sensibilité rend la comparaison décidable au lieu d'exiger une énumération exhaustive.
    pub clearance: Confidentiality,
}

/// Ce qu'une contamination constatée dit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// De quelle forme.
    pub kind: Contamination,
    /// Sur quelle révision.
    pub revision: RevisionId,
    /// Ce qu'on en dit.
    pub detail: String,
}

impl fmt::Display for Finding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} : {}", self.kind, self.detail)
    }
}

/// Le rang de sensibilité, croissant.
///
/// §16.2 parle de **plafond** de confidentialité : la comparaison suppose donc un ordre, et
/// `Confidentiality` est documenté comme « croissant en sensibilité ». Le rendre explicite ici
/// évite qu'un `match` recopié ailleurs finisse par en changer l'ordre sans qu'on s'en aperçoive.
///
/// `pub(crate)` depuis `W24.a` : la souscription compare le même plafond, et la seule façon de tenir
/// la phrase ci-dessus est de lui donner **cette** fonction plutôt qu'une copie.
pub(crate) const fn rank(classification: Confidentiality) -> u8 {
    match classification {
        Confidentiality::Public => 0,
        Confidentiality::Internal => 1,
        Confidentiality::Confidential => 2,
        Confidentiality::Restricted => 3,
    }
}

/// Inspecter un contexte destiné à un relecteur.
///
/// Rend **tous** les constats, pas le premier : une contamination trouvée n'exclut pas les autres,
/// et s'arrêter au premier ferait réparer une fuite en laissant les quatre autres.
#[must_use]
pub fn inspect(items: &[ContextItem], recipient: &Recipient) -> Vec<Finding> {
    let mut findings = Vec::new();

    for item in items {
        if item.is_generator_reasoning && recipient.blind_to_generator {
            findings.push(Finding {
                kind: Contamination::GeneratorReasoningLeaked,
                revision: item.revision,
                detail: "le raisonnement du générateur atteint un relecteur aveugle".to_owned(),
            });
        }
        if item.is_refuted {
            findings.push(Finding {
                kind: Contamination::RefutedClaimPropagated,
                revision: item.revision,
                detail: "une revendication réfutée entre dans le contexte par défaut".to_owned(),
            });
        }
        if rank(item.classification) > rank(recipient.clearance) {
            findings.push(Finding {
                kind: Contamination::ConfidentialDataOnUnauthorisedWorker,
                revision: item.revision,
                detail: format!(
                    "une donnée « {:?} » atteint un worker habilité « {:?} »",
                    item.classification, recipient.clearance
                ),
            });
        }
    }

    findings.extend(circular(items));
    findings
}

/// Détecter un consensus circulaire — un cycle de citations sans aucune source externe.
///
/// # Ce que « circulaire » veut dire ici, et ce que ça ne veut pas dire
///
/// Deux agents qui se citent mutuellement **et** citent tous deux une source externe ne forment pas
/// un consensus circulaire : ils s'appuient sur quelque chose. Ce que §16.6 vise est le cas où le
/// cycle est **toute** la fondation — la conviction se soutient d'elle-même. La détection porte
/// donc sur les composantes de citations dont aucun membre n'a de source externe.
fn circular(items: &[ContextItem]) -> Vec<Finding> {
    let known: BTreeMap<RevisionId, &ContextItem> =
        items.iter().map(|item| (item.revision, item)).collect();

    let mut findings = Vec::new();
    for item in items {
        let mut seen = BTreeSet::new();
        if reaches_itself(item, &known, &mut seen)
            && !seen.iter().any(|revision| {
                known
                    .get(revision)
                    .is_some_and(|member| member.is_external_source)
            })
        {
            findings.push(Finding {
                kind: Contamination::CircularConsensus,
                revision: item.revision,
                detail: "un cycle de citations dont aucun membre ne cite de source externe"
                    .to_owned(),
            });
        }
    }
    findings
}

/// Vrai quand un élément se rejoint lui-même en suivant ses citations, en notant le chemin.
fn reaches_itself(
    start: &ContextItem,
    known: &BTreeMap<RevisionId, &ContextItem>,
    visited: &mut BTreeSet<RevisionId>,
) -> bool {
    let mut stack: Vec<RevisionId> = start.cites.clone();
    while let Some(revision) = stack.pop() {
        if revision == start.revision {
            visited.insert(revision);
            return true;
        }
        if !visited.insert(revision) {
            continue;
        }
        if let Some(item) = known.get(&revision) {
            stack.extend(item.cites.iter().copied());
        }
    }
    false
}

/// Vérifier qu'une synthèse n'a pas perdu de contradiction.
///
/// § 16.6, cinquième forme. Une synthèse qui laisse tomber une contradiction ne se signale pas :
/// elle est **plus lisible** que celle qui la garde, et c'est ce qui la rend dangereuse. La
/// vérification est donc à part — elle ne regarde pas le contexte d'entrée mais ce que la synthèse
/// en a fait.
#[must_use]
pub fn contradictions_dropped(
    contradictions: &[RevisionId],
    synthesis_mentions: &[RevisionId],
) -> Vec<Finding> {
    let mentioned: BTreeSet<RevisionId> = synthesis_mentions.iter().copied().collect();
    contradictions
        .iter()
        .filter(|revision| !mentioned.contains(revision))
        .map(|revision| Finding {
            kind: Contamination::ContradictionDropped,
            revision: *revision,
            detail: "une contradiction connue n'apparaît pas dans la synthèse".to_owned(),
        })
        .collect()
}
