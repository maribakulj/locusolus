//! `communication_tokens` — ce que se mettre d'accord a coûté. `W21.l`, ADR 0024.
//!
//! # Ce que la métrique divise
//!
//! Les jetons **dépensés** à se coordonner, divisés par les jetons dépensés en tout. La
//! classification vient de [`locus_budget::Classification`], livrée par `W21.m` : elle est
//! **déclarée** à la retenue, jamais devinée.
//!
//! # « Dépensé » est ce que le registre appelle dépensé, pas autre chose
//!
//! Six mouvements existent, et trois seulement font une dépense : [`EntryKind::Consumption`] et
//! [`EntryKind::Adjustment`] ajoutent, [`EntryKind::Refund`] retire. Allouer, retenir et rendre
//! déplacent du **provisionnement** — une allocation de coordination généreuse jamais consommée ne
//! dit rien de ce que la coordination a coûté, et la compter ferait dire à la métrique le contraire
//! de ce qu'elle mesure.
//!
//! Ce sont exactement les trois mouvements et les deux signes que `BudgetAccount::spent` emploie.
//! Réinventer l'arithmétique ici en produirait une seconde, et c'est toujours la seconde qui ment.
//!
//! # Les non classées ne sont dans aucun des deux termes
//!
//! Ni au numérateur, ni au dénominateur. Les mettre au dénominateur ferait **baisser** la part de
//! coordination à chaque écriture que personne n'a classée, ce qui se lirait comme un progrès ; les
//! mettre au numérateur ferait l'inverse. Elles sont donc rendues à côté, comptées, et
//! [`Communication::declared`] dit sur quelle assiette la part se calcule.
//!
//! C'est le traitement des indécises de `W21.d`, et le même refus de collapse que les absences de
//! `xiiif` §19.
//!
//! # Une campagne entièrement non classée rend une absence, pas un zéro
//!
//! [`Share::NothingDeclared`]. Zéro voudrait dire « aucune coordination », ce qui est une bonne
//! nouvelle ; ne rien savoir n'en est pas une. La distinction est celle de `W21.k` entre « aucune
//! panne » et « aucune reprise », posée au même endroit : sur l'accesseur, pas dans la tête du
//! lecteur.
//!
//! # Ce que la métrique ne dit pas
//!
//! Si la coordination valait son prix. Une équipe qui ne se parle jamais dépense zéro et se trompe
//! ensemble ; une part élevée peut décrire une négociation coûteuse ou une négociation nécessaire.
//! Le module ne contient donc aucun seuil et aucun verdict — décision 9 de l'ADR 0024, tenue par un
//! test d'absence.

use std::fmt;

use locus_budget::{Classification, Dimension, Entry, EntryKind, Spend};

/// Ce qu'un journal de budget dit du coût de la coordination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Communication {
    coordination: u64,
    work: u64,
    unclassified: u64,
}

impl Communication {
    /// Relever les jetons dépensés d'un journal.
    ///
    /// # Errors
    ///
    /// [`CommunicationError::RefundedMoreThanSpent`] quand les remboursements d'un tas dépassent ce
    /// qui y a été dépensé. Le cas est atteignable — chaque rapprochement se compare à la
    /// consommation **enregistrée**, pas au cumul des corrections, donc deux rapprochements à la
    /// baisse remboursent deux fois le même écart. Saturer à zéro rendrait un nombre d'apparence
    /// normale là où le journal est incohérent, ce qui est précisément ce qu'un registre existe pour
    /// ne pas faire.
    pub fn over<'a>(
        entries: impl IntoIterator<Item = &'a Entry>,
    ) -> Result<Self, CommunicationError> {
        let mut spent = [0_i128; 3];
        for entry in entries {
            let Some(signe) = sign_of(entry.kind()) else {
                continue;
            };
            let tokens = i128::from(
                entry
                    .amounts()
                    .get(&Dimension::Tokens)
                    .copied()
                    .unwrap_or_default(),
            );
            spent[heap_of(entry.spend())] += signe * tokens;
        }

        let mut counted = Self::default();
        for (index, total) in spent.into_iter().enumerate() {
            let Ok(total) = u64::try_from(total) else {
                return Err(CommunicationError::RefundedMoreThanSpent {
                    spend: heap_name(index),
                    net: total,
                });
            };
            match index {
                0 => counted.coordination = total,
                1 => counted.work = total,
                _ => counted.unclassified = total,
            }
        }
        Ok(counted)
    }

    /// Les jetons dépensés à se coordonner.
    #[must_use]
    pub const fn coordination(self) -> u64 {
        self.coordination
    }

    /// Les jetons dépensés à faire le travail.
    #[must_use]
    pub const fn work(self) -> u64 {
        self.work
    }

    /// Les jetons dépensés que personne n'a classés — dans aucun des deux termes.
    #[must_use]
    pub const fn unclassified(self) -> u64 {
        self.unclassified
    }

    /// L'assiette sur laquelle la part se calcule : ce que quelqu'un a déclaré.
    #[must_use]
    pub const fn declared(self) -> u64 {
        self.coordination.saturating_add(self.work)
    }

    /// La part de coordination — `communication_tokens` proprement dit.
    #[must_use]
    pub fn share(self) -> Share {
        let declared = self.declared();
        if declared == 0 {
            return Share::NothingDeclared;
        }
        let brut = f64_of(self.coordination) / f64_of(declared);
        Share::Measured(quantise(brut))
    }
}

/// Ce que la part vaut, ou pourquoi elle ne vaut rien.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Share {
    /// La part de coordination, entre 0 et 1.
    Measured(f64),
    /// **Rien** n'a été classé : voir la documentation du module. Ce n'est pas zéro.
    NothingDeclared,
}

impl Share {
    /// La part, si elle a été mesurée.
    #[must_use]
    pub const fn value(self) -> Option<f64> {
        match self {
            Self::Measured(part) => Some(part),
            Self::NothingDeclared => None,
        }
    }
}

impl fmt::Display for Share {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Measured(part) => write!(formatter, "{part}"),
            Self::NothingDeclared => {
                formatter.write_str("rien de déclaré — la part n'a pas d'assiette")
            }
        }
    }
}

/// Pourquoi un relevé est refusé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommunicationError {
    /// Un tas a été remboursé au-delà de ce qui y a été dépensé.
    RefundedMoreThanSpent {
        /// Lequel.
        spend: Classification,
        /// Ce qui reste une fois les remboursements retirés — négatif, par construction.
        net: i128,
    },
}

impl fmt::Display for CommunicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RefundedMoreThanSpent { spend, net } => write!(
                formatter,
                "le tas « {spend} » a été remboursé au-delà de sa dépense — il reste {net} jetons, \
                 et saturer à zéro rendrait un nombre d'apparence normale sur un journal incohérent"
            ),
        }
    }
}

impl std::error::Error for CommunicationError {}

/// Le signe qu'un mouvement porte sur la dépense, ou `None` s'il n'en est pas une.
///
/// Trois mouvements sur six font une dépense. Les trois autres déplacent du provisionnement : voir
/// la documentation du module.
const fn sign_of(kind: EntryKind) -> Option<i128> {
    match kind {
        EntryKind::Consumption | EntryKind::Adjustment => Some(1),
        EntryKind::Refund => Some(-1),
        EntryKind::Allocation | EntryKind::Reservation | EntryKind::Release => None,
    }
}

/// Dans quel tas une écriture tombe. Trois tas, et le troisième n'est pas un mélange des deux.
const fn heap_of(classification: Classification) -> usize {
    match classification.spend() {
        Some(Spend::Coordination) => 0,
        Some(Spend::Work) => 1,
        None => 2,
    }
}

/// Le nom d'un tas, pour le dire dans un refus.
const fn heap_name(index: usize) -> Classification {
    match index {
        0 => Classification::Classified(Spend::Coordination),
        1 => Classification::Classified(Spend::Work),
        _ => Classification::Unclassified,
    }
}

/// Un `u64` en `f64`, sans que clippy ait à deviner l'intention.
#[expect(
    clippy::cast_precision_loss,
    reason = "au-delà de 2^53 jetons la perte est sous le pas de quantification de la part"
)]
fn f64_of(value: u64) -> f64 {
    value as f64
}

/// Quantifier au milliardième — voir `W21.f`, même raison et même pas.
///
/// Deux journaux qui décrivent le même partage doivent rendre la **même** valeur, sans quoi une
/// égalité stricte devient intestable et deux rapports identiques paraissent différents.
fn quantise(value: f64) -> f64 {
    const PAS: f64 = 1e9;
    (value * PAS).round() / PAS
}
