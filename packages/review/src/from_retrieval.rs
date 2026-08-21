//! La jonction `Results → ContextView` — `W17.n`, ADR 0022 décision 6 bis.
//!
//! # Ce que cet item construit, et qui n'existait pas
//!
//! Deux sous-systèmes avaient été construits séparément et **ne se connaissaient pas** : la
//! `ContextView` vit ici, `retrieve` vit dans `packages/memory`, et les deux `Cargo.toml` le
//! confirmaient. `ContextView::build` prend des `ContextItem` clés par `RevisionId` ; `retrieve`
//! rend des `Candidate` clés par `String`. Aucun chemin ne menait de l'un à l'autre.
//!
//! Le reçu n'est donc pas un type à ajouter : il est ce qui **relie**. La dépendance va de `review`
//! vers `memory`, et la direction se justifie — une vue de contexte se construit **depuis** un
//! retrieval, jamais l'inverse.
//!
//! # Le condensat est calculé ici, et c'est ce qui rend le rejeu vérifiable
//!
//! `ContextView::build` reçoit son `ContentHash` en paramètre : il est fourni, pas calculé. Tant que
//! personne ne le calculait, « deux constructions rendent la même vue » était une affirmation sur
//! l'appelant. Cette jonction le calcule depuis une forme canonique des révisions retenues, donc
//! deux constructions sur les mêmes entrées rendent le **même** condensat sans que l'appelant ait à
//! s'en occuper — et un rejeu depuis le reçu se compare par égalité stricte.
//!
//! # La contestation vise le reçu
//!
//! Rien ici ne rend une `ContextView` modifiable ni contestable : elle est ce qui a été vu, et c'est
//! un fait. Ce qui se conteste est la manière dont elle a été constituée, et c'est le reçu qui la
//! porte.

use std::collections::BTreeMap;
use std::fmt;
use std::fmt::Write as _;

use locus_domain::ContentHash;
use locus_memory::{Excluded, Exclusion, Plan, ReceiptError, Results, RetrievalReceipt};

use crate::contamination::{ContextItem, Recipient};
use crate::context_view::{ContextView, ContextViewError};

/// La ligne d'en-tête de la forme canonique d'une vue.
const VIEW_MAGIC: &str = "context-view/1";

/// Construire une vue **et** son reçu depuis un retrieval.
///
/// `corpus` associe à chaque candidat retenu l'élément de contexte et la position d'événement dont
/// il provient. Un candidat retenu qu'on ne sait pas résoudre est **écarté en le consignant** — une
/// exclusion silencieuse serait indistinguable d'un oubli, et c'est ce que le reçu existe pour
/// rendre impossible.
///
/// # Errors
///
/// [`JunctionError::View`] quand la vue refuse un élément — au-delà du watermark, notamment ;
/// [`JunctionError::Receipt`] quand le reçu refuse un motif ou une clé.
pub fn view_from_retrieval(
    plan: &Plan,
    results: &Results,
    corpus: &BTreeMap<String, (ContextItem, u64)>,
    recipient: &Recipient,
    watermark: u64,
) -> Result<(ContextView, RetrievalReceipt), JunctionError> {
    let mut elements = Vec::new();
    let mut retenues = Vec::new();
    let mut exclusions = Vec::new();

    for candidat in results.included() {
        let Some((item, position)) = corpus.get(candidat.key()) else {
            exclusions.push(Exclusion::motivated(
                candidat.key(),
                "retenu par le classement, mais introuvable dans le corpus servi",
            )?);
            continue;
        };
        retenues.push(item.revision.to_string());
        elements.push((item.clone(), *position));
    }

    // Les exclusions du retrieval lui-même, traduites avec leur motif. Elles portent déjà une
    // raison typée ; la perdre en chemin rendrait le reçu muet là où il doit être le plus précis.
    for exclu in results.excluded() {
        exclusions.push(motif_de(exclu)?);
    }

    let digest = digest_of(&retenues);
    let vue = ContextView::build(&elements, recipient, watermark, digest)?;

    let recu = RetrievalReceipt::write(
        plan,
        watermark,
        results.included().len() + results.excluded().len(),
        retenues,
        exclusions,
        Vec::new(),
    )?;
    Ok((vue, recu))
}

/// Rejouer un reçu sur le même corpus, et rendre la vue qu'il décrit.
///
/// # Ce que « rejouer » prouve, et ce qu'il ne prouve pas
///
/// Il prouve que le reçu suffit à reconstituer **la même vue**, condensat compris — donc qu'il n'a
/// rien caché de ce qui a été retenu. Il ne prouve pas que le classement se reproduirait : cela
/// dépend de la fonction de classement, que le reçu **nomme** sans la détenir, et
/// [`RetrievalReceipt::promises_replay`] dit si elle est nommée.
///
/// # Errors
///
/// [`JunctionError::View`] quand la vue refuse un élément ; [`JunctionError::Unresolvable`] quand le
/// corpus ne contient plus une révision que le reçu dit avoir retenue — un rejeu qui inventerait à
/// la place rendrait une vue plausible et fausse.
pub fn replay_receipt(
    receipt: &RetrievalReceipt,
    corpus: &BTreeMap<String, (ContextItem, u64)>,
    recipient: &Recipient,
) -> Result<ContextView, JunctionError> {
    let par_revision: BTreeMap<String, &(ContextItem, u64)> = corpus
        .values()
        .map(|entry| (entry.0.revision.to_string(), entry))
        .collect();

    let mut elements = Vec::new();
    for revision in receipt.included() {
        let Some((item, position)) = par_revision.get(revision).copied() else {
            return Err(JunctionError::Unresolvable {
                revision: revision.clone(),
            });
        };
        elements.push((item.clone(), *position));
    }

    let digest = digest_of(receipt.included());
    Ok(ContextView::build(
        &elements,
        recipient,
        receipt.watermark(),
        digest,
    )?)
}

/// Le condensat d'une vue — celui de la forme canonique de ses révisions retenues, **dans l'ordre**.
///
/// Dans l'ordre, et non trié : l'ordre des inclusions est le classement, et deux vues qui retiennent
/// les mêmes révisions dans un ordre différent ne servent pas la même chose au lecteur.
fn digest_of(revisions: &[String]) -> ContentHash {
    let mut canonical = String::from(VIEW_MAGIC);
    for revision in revisions {
        let _ = write!(canonical, "\n{revision}");
    }
    canonical.push('\n');
    ContentHash::of(canonical.as_bytes())
}

/// Le motif d'une exclusion de retrieval, sous une forme qu'un lecteur comprend.
fn motif_de(excluded: &Excluded) -> Result<Exclusion, ReceiptError> {
    match excluded {
        Excluded::BeyondClearance {
            key,
            classification,
            clearance,
        } => Exclusion::motivated(
            key,
            format!("classification {classification:?} au-delà de l'habilitation {clearance:?}"),
        ),
        Excluded::BeyondBudget { key, rank } => {
            Exclusion::motivated(key, format!("rang {rank}, au-delà du budget du plan"))
        }
    }
}

/// Pourquoi une jonction échoue.
#[derive(Debug, Clone, PartialEq)]
pub enum JunctionError {
    /// La vue a refusé un élément.
    View(ContextViewError),
    /// Le reçu a refusé un champ.
    Receipt(ReceiptError),
    /// Le corpus ne contient plus une révision que le reçu dit avoir retenue.
    Unresolvable {
        /// Laquelle.
        revision: String,
    },
}

impl From<ContextViewError> for JunctionError {
    fn from(error: ContextViewError) -> Self {
        Self::View(error)
    }
}

impl From<ReceiptError> for JunctionError {
    fn from(error: ReceiptError) -> Self {
        Self::Receipt(error)
    }
}

impl fmt::Display for JunctionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::View(error) => write!(formatter, "la vue refuse : {error}"),
            Self::Receipt(error) => write!(formatter, "le reçu refuse : {error}"),
            Self::Unresolvable { revision } => write!(
                formatter,
                "« {revision} » est au reçu et absente du corpus : un rejeu qui inventerait à sa \
                 place rendrait une vue plausible et fausse"
            ),
        }
    }
}

impl std::error::Error for JunctionError {}
