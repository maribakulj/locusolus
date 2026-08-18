//! Les écritures — `docs/SPEC_V1.md` §7.2.
//!
//! « Le budget est un registre, pas un compteur mutable isolé. » La phrase décide de la forme du
//! module : les soldes ne sont pas des champs, ils se **déduisent** des écritures. Un compteur
//! entretenu à côté du journal serait une seconde vérité, et c'est toujours la seconde qui ment.

use std::fmt;

use locus_protocol::{Id, id::provisional::Reservation};

use crate::dimension::Amounts;

/// Les six écritures obligatoires de §7.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntryKind {
    /// Créditer le compte, dans la limite de ses bornes.
    Allocation,
    /// Retenir de quoi exécuter.
    Reservation,
    /// Rendre une retenue non employée.
    Release,
    /// Constater ce qui a été dépensé.
    Consumption,
    /// Corriger à la hausse, sans réécrire l'écriture corrigée.
    Adjustment,
    /// Corriger à la baisse, sans réécrire l'écriture corrigée.
    Refund,
}

impl EntryKind {
    /// Les six.
    pub const ALL: [Self; 6] = [
        Self::Allocation,
        Self::Reservation,
        Self::Release,
        Self::Consumption,
        Self::Adjustment,
        Self::Refund,
    ];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Allocation => "allocation",
            Self::Reservation => "reservation",
            Self::Release => "release",
            Self::Consumption => "consumption",
            Self::Adjustment => "adjustment",
            Self::Refund => "refund",
        }
    }
}

impl fmt::Display for EntryKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Une écriture, définitive.
///
/// # Ce qu'on ne peut pas en faire
///
/// La modifier. Aucun champ n'est public en écriture et le compte n'expose aucun accès mutable à
/// son journal : §7.2 exige qu'« une correction ne réécrive pas une écriture antérieure ; elle crée
/// un ajustement compensatoire ». Une écriture rectifiable rendrait le registre indistinguable d'un
/// compteur, et un budget dépassé puis corrigé indistinguable d'un budget jamais dépassé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    sequence: u64,
    kind: EntryKind,
    reservation: Option<Id<Reservation>>,
    amounts: Amounts,
    reason: String,
}

impl Entry {
    pub(crate) fn new(
        sequence: u64,
        kind: EntryKind,
        reservation: Option<Id<Reservation>>,
        amounts: Amounts,
        reason: &str,
    ) -> Self {
        Self {
            sequence,
            kind,
            reservation,
            amounts,
            reason: reason.to_owned(),
        }
    }

    /// Son rang dans le journal.
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Ce qu'elle fait.
    #[must_use]
    pub const fn kind(&self) -> EntryKind {
        self.kind
    }

    /// La réservation concernée, s'il y en a une.
    #[must_use]
    pub const fn reservation(&self) -> Option<&Id<Reservation>> {
        self.reservation.as_ref()
    }

    /// Les quantités en jeu.
    #[must_use]
    pub const fn amounts(&self) -> &Amounts {
        &self.amounts
    }

    /// Pourquoi elle a été passée.
    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

impl fmt::Display for Entry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "#{} {} : {}",
            self.sequence, self.kind, self.reason
        )
    }
}
