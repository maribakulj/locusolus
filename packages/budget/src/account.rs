//! Le compte de budget — `docs/SPEC_V1.md` §7.2, invariant 6.
//!
//! # Ce que la réservation empêche, et ce que le registre constate
//!
//! Ce sont deux rôles, et les confondre casse l'un des deux. La **réservation** est ce qui empêche :
//! elle est refusée quand la borne ne suit pas, et une réservation refusée ne rend aucun jeton
//! d'exécution. Le **registre** est ce qui constate : il enregistre ce qui a été dépensé, y compris
//! au-delà de ce qui était retenu. Un registre qui refuserait d'écrire un dépassement serait un
//! souhait, pas un journal — et le dépassement disparaîtrait précisément là où il fallait le voir.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use locus_protocol::{
    Category, Id, Retry, StructuredError, Timestamp,
    id::provisional::{
        BudgetAccount as AccountKind, Error as ErrorKind, Reservation as ReservationKind,
    },
};

use crate::dimension::{Amounts, Dimension};
use crate::ledger::{Entry, EntryKind};
use crate::limits::Limits;

/// Le droit d'exécuter, à concurrence de ce qui est retenu.
///
/// # Pourquoi c'est un type, et pas un booléen
///
/// Invariant 6 : « les ressources sont réservées avant exécution ». Une valeur que seul
/// [`BudgetAccount::reserve`] peut produire rend la règle indéfaisable : là où une exécution exige
/// une `Reservation`, il n'existe aucune façon d'exécuter sans en avoir obtenu une. Un booléen
/// `has_budget` se fabriquerait ; ceci ne se fabrique pas.
///
/// Le type n'est pas `Clone` : [`BudgetAccount::consume`] et [`BudgetAccount::release`] le prennent
/// par valeur, donc une retenue se solde une fois.
#[derive(Debug, PartialEq, Eq)]
pub struct Reservation {
    id: Id<ReservationKind>,
    account: Id<AccountKind>,
    amounts: Amounts,
}

impl Reservation {
    /// Son identifiant.
    #[must_use]
    pub const fn id(&self) -> &Id<ReservationKind> {
        &self.id
    }

    /// Le compte qui l'a accordée.
    #[must_use]
    pub const fn account(&self) -> &Id<AccountKind> {
        &self.account
    }

    /// Ce qui est retenu.
    #[must_use]
    pub const fn amounts(&self) -> &Amounts {
        &self.amounts
    }
}

/// Un dépassement constaté sur une dimension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Overrun {
    /// La dimension dépassée.
    pub dimension: Dimension,
    /// Ce qui était retenu.
    pub reserved: u64,
    /// Ce qui a été dépensé.
    pub actual: u64,
}

impl Overrun {
    /// De combien.
    #[must_use]
    pub const fn excess(&self) -> u64 {
        self.actual.saturating_sub(self.reserved)
    }

    /// Sa catégorie sur le fil.
    #[must_use]
    pub const fn category(&self) -> Category {
        Category::Budget
    }

    /// Sa politique de nouvelle tentative.
    ///
    /// Jamais. Réessayer ne rendrait pas le budget : la tentative suivante rencontrerait la même
    /// borne, en ayant dépensé une fois de plus. C'est ce que la fixture
    /// `schemas/examples/attempt-budget-exceeded.json` fixe sur le fil — `retryable: false`, et
    /// l'état `failed` plutôt que `cancelled`, parce que rien n'a été annulé.
    #[must_use]
    pub const fn retry(&self) -> Retry {
        Retry::Never
    }

    /// L'erreur structurée correspondante.
    ///
    /// L'identifiant et l'instant viennent de l'appelant : le domaine ne lit pas d'horloge, sans
    /// quoi il ne serait plus rejouable.
    #[must_use]
    pub fn into_error(
        self,
        error_id: Id<ErrorKind>,
        occurred_at: Timestamp,
        component: &str,
    ) -> StructuredError {
        let mut details = BTreeMap::new();
        details.insert("dimension".to_owned(), self.dimension.slug().to_owned());
        details.insert("reserved".to_owned(), self.reserved.to_string());
        details.insert("actual".to_owned(), self.actual.to_string());
        StructuredError {
            error_id,
            code: "budget_exhausted".to_owned(),
            category: self.category(),
            retryable: self.retry(),
            mission_id: None,
            attempt: None,
            component: component.to_owned(),
            message: self.to_string(),
            details,
            caused_by: None,
            security_sensitive: false,
            occurred_at,
        }
    }
}

impl fmt::Display for Overrun {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "budget de {} {} atteint : {} dépensés, {} de trop",
            self.reserved,
            self.dimension,
            self.actual,
            self.excess()
        )
    }
}

/// Ce qu'une consommation a soldé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settlement {
    released: Amounts,
    overruns: Vec<Overrun>,
}

impl Settlement {
    /// Ce qui a été rendu, faute d'avoir été dépensé.
    #[must_use]
    pub const fn released(&self) -> &Amounts {
        &self.released
    }

    /// Les dépassements constatés.
    #[must_use]
    pub fn overruns(&self) -> &[Overrun] {
        &self.overruns
    }

    /// Vrai quand l'exécution doit s'arrêter.
    #[must_use]
    pub const fn stops_execution(&self) -> bool {
        !self.overruns.is_empty()
    }
}

/// Ce qu'un rapprochement a corrigé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reconciliation {
    debited: Amounts,
    credited: Amounts,
}

impl Reconciliation {
    /// Ce que les métriques du worker ont ajouté.
    #[must_use]
    pub const fn debited(&self) -> &Amounts {
        &self.debited
    }

    /// Ce qu'elles ont rendu.
    #[must_use]
    pub const fn credited(&self) -> &Amounts {
        &self.credited
    }

    /// Vrai quand le rapprochement n'a rien changé.
    #[must_use]
    pub fn agrees(&self) -> bool {
        self.debited.is_empty() && self.credited.is_empty()
    }
}

/// Les soldes déduits du journal.
#[derive(Debug, Default)]
struct Balances {
    allocated: Amounts,
    held: Amounts,
    spent: Amounts,
    outstanding: BTreeMap<Id<ReservationKind>, Amounts>,
    consumed: BTreeMap<Id<ReservationKind>, Amounts>,
}

/// Le compte — un registre, pas un compteur.
///
/// §7.2 ouvre par cette phrase, et elle décide de la forme du type : aucun solde n'est un champ.
/// `allocated`, `held` et `spent` se **déduisent** des écritures à chaque lecture. Un compteur
/// entretenu à côté du journal serait une seconde vérité, et c'est toujours la seconde qui ment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetAccount {
    id: Id<AccountKind>,
    limits: Limits,
    entries: Vec<Entry>,
}

impl BudgetAccount {
    /// Ouvrir un compte borné.
    #[must_use]
    pub const fn open(id: Id<AccountKind>, limits: Limits) -> Self {
        Self {
            id,
            limits,
            entries: Vec::new(),
        }
    }

    /// Son identifiant.
    #[must_use]
    pub const fn id(&self) -> &Id<AccountKind> {
        &self.id
    }

    /// Ses bornes.
    #[must_use]
    pub const fn limits(&self) -> &Limits {
        &self.limits
    }

    /// Le journal, en lecture seule.
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Créditer le compte.
    ///
    /// # Errors
    ///
    /// [`BudgetError::UnboundedDimension`] pour une dimension que le compte ne borne pas, et
    /// [`BudgetError::BeyondCeiling`] quand l'allocation dépasserait la borne dure — allouer plus
    /// que la borne ne rendrait pas la borne moins dure, cela rendrait seulement l'écriture fausse.
    pub fn allocate(&mut self, amounts: &Amounts, reason: &str) -> Result<(), BudgetError> {
        let balances = self.fold();
        for (dimension, amount) in amounts {
            let ceiling = self.ceiling_of(*dimension)?;
            let already = at(&balances.allocated, *dimension);
            if already.saturating_add(*amount) > ceiling {
                return Err(BudgetError::BeyondCeiling {
                    dimension: *dimension,
                    ceiling,
                    requested: already.saturating_add(*amount),
                });
            }
        }
        self.append(EntryKind::Allocation, None, amounts.clone(), reason);
        Ok(())
    }

    /// Retenir de quoi exécuter.
    ///
    /// # Rien n'est écrit si quoi que ce soit est refusé
    ///
    /// Toutes les vérifications précèdent la première écriture. Une réservation refusée laisse le
    /// journal **exactement** dans l'état où elle l'a trouvé : c'est ce que « une réservation
    /// refusée n'exécute rien » veut dire une fois qu'on le rend observable. Un refus qui laisserait
    /// une trace partielle ferait porter au compte suivant le coût d'une exécution qui n'a pas eu
    /// lieu.
    ///
    /// # Errors
    ///
    /// [`BudgetError::EmptyReservation`] pour une retenue vide — elle satisferait la lettre de
    /// l'invariant 6 en ne retenant rien ; [`BudgetError::UnboundedDimension`] pour une dimension
    /// hors budget ; [`BudgetError::DuplicateReservation`] pour un identifiant déjà employé ;
    /// [`BudgetError::WouldExceed`] quand la borne ne suit pas.
    pub fn reserve(
        &mut self,
        id: Id<ReservationKind>,
        amounts: &Amounts,
        reason: &str,
    ) -> Result<Reservation, BudgetError> {
        if amounts.is_empty() || amounts.values().all(|amount| *amount == 0) {
            return Err(BudgetError::EmptyReservation);
        }
        let balances = self.fold();
        if balances.outstanding.contains_key(&id) || balances.consumed.contains_key(&id) {
            return Err(BudgetError::DuplicateReservation { id });
        }
        for (dimension, amount) in amounts {
            self.ceiling_of(*dimension)?;
            let available = self.available_from(&balances, *dimension);
            if *amount > available {
                return Err(BudgetError::WouldExceed {
                    dimension: *dimension,
                    available,
                    requested: *amount,
                });
            }
        }

        self.append(EntryKind::Reservation, Some(id), amounts.clone(), reason);
        Ok(Reservation {
            id,
            account: self.id,
            amounts: amounts.clone(),
        })
    }

    /// Rendre une retenue non employée.
    ///
    /// # Errors
    ///
    /// [`BudgetError::ForeignReservation`] pour une retenue d'un autre compte, et
    /// [`BudgetError::UnknownReservation`] pour une retenue déjà soldée.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "prendre la retenue par valeur est la garantie : elle ne survit pas à son solde"
    )]
    pub fn release(&mut self, reservation: Reservation, reason: &str) -> Result<(), BudgetError> {
        // La retenue est démembrée ici : elle n'existe plus après cet appel, ce qui est exactement
        // ce que « une retenue se solde une fois » veut dire au niveau du type.
        let Reservation { id, account, .. } = reservation;
        let held = self.outstanding_of(id, account)?;
        self.append(EntryKind::Release, Some(id), held, reason);
        Ok(())
    }

    /// Constater ce qui a été dépensé, et solder la retenue.
    ///
    /// # Le dépassement s'écrit
    ///
    /// Quand le constat dépasse la retenue, la consommation est écrite **telle quelle** et le
    /// dépassement est rapporté. Refuser l'écriture laisserait le registre en désaccord avec le
    /// monde : les ressources ont bien été dépensées, et un journal qui l'ignore rend le dépassement
    /// invisible là où il fallait le voir.
    ///
    /// # Errors
    ///
    /// [`BudgetError::ForeignReservation`] et [`BudgetError::UnknownReservation`], comme
    /// [`BudgetAccount::release`] ; [`BudgetError::UnboundedDimension`] quand le constat porte sur
    /// une dimension hors budget.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "prendre la retenue par valeur est la garantie : elle ne survit pas à son solde"
    )]
    pub fn consume(
        &mut self,
        reservation: Reservation,
        actual: &Amounts,
        reason: &str,
    ) -> Result<Settlement, BudgetError> {
        let Reservation { id, account, .. } = reservation;
        let held = self.outstanding_of(id, account)?;
        for dimension in actual.keys() {
            self.ceiling_of(*dimension)?;
        }

        let mut overruns = Vec::new();
        let mut released = Amounts::new();
        for dimension in Dimension::ALL {
            let reserved = at(&held, dimension);
            let spent = at(actual, dimension);
            if spent > reserved {
                overruns.push(Overrun {
                    dimension,
                    reserved,
                    actual: spent,
                });
            } else if reserved > spent {
                released.insert(dimension, reserved - spent);
            }
        }

        self.append(EntryKind::Consumption, Some(id), actual.clone(), reason);
        Ok(Settlement { released, overruns })
    }

    /// Rapprocher une consommation des métriques du worker — §7.2.
    ///
    /// # Une correction ne réécrit rien
    ///
    /// L'écart devient une écriture de plus : [`EntryKind::Adjustment`] à la hausse,
    /// [`EntryKind::Refund`] à la baisse. L'écriture corrigée reste dans le journal, inchangée —
    /// sans quoi un budget dépassé puis corrigé serait indistinguable d'un budget jamais dépassé.
    ///
    /// # Errors
    ///
    /// [`BudgetError::UnknownReservation`] quand cette retenue n'a jamais été consommée : il n'y a
    /// alors rien à rapprocher.
    pub fn reconcile(
        &mut self,
        reservation: &Id<ReservationKind>,
        observed: &Amounts,
        reason: &str,
    ) -> Result<Reconciliation, BudgetError> {
        let balances = self.fold();
        let recorded = balances
            .consumed
            .get(reservation)
            .cloned()
            .ok_or(BudgetError::UnknownReservation { id: *reservation })?;

        let mut debited = Amounts::new();
        let mut credited = Amounts::new();
        for dimension in Dimension::ALL {
            let before = at(&recorded, dimension);
            let after = at(observed, dimension);
            if after > before {
                debited.insert(dimension, after - before);
            } else if before > after {
                credited.insert(dimension, before - after);
            }
        }

        if !debited.is_empty() {
            self.append(
                EntryKind::Adjustment,
                Some(*reservation),
                debited.clone(),
                reason,
            );
        }
        if !credited.is_empty() {
            self.append(
                EntryKind::Refund,
                Some(*reservation),
                credited.clone(),
                reason,
            );
        }
        Ok(Reconciliation { debited, credited })
    }

    /// Ce qui a été alloué sur une dimension.
    #[must_use]
    pub fn allocated(&self, dimension: Dimension) -> u64 {
        at(&self.fold().allocated, dimension)
    }

    /// Ce qui est retenu et non encore soldé.
    #[must_use]
    pub fn held(&self, dimension: Dimension) -> u64 {
        at(&self.fold().held, dimension)
    }

    /// Ce qui a été dépensé.
    #[must_use]
    pub fn spent(&self, dimension: Dimension) -> u64 {
        at(&self.fold().spent, dimension)
    }

    /// Ce qui reste réservable.
    #[must_use]
    pub fn available(&self, dimension: Dimension) -> u64 {
        self.available_from(&self.fold(), dimension)
    }

    /// Les dimensions où le dépensé a franchi la borne.
    ///
    /// §7.2 : « `spent + reserved` ne dépasse pas la limite dure. » La réservation le garantit à
    /// l'octroi ; ce que cette méthode rend, c'est ce qui a franchi la borne **malgré** l'octroi —
    /// une exécution qui a dépensé plus qu'elle n'avait retenu. Le fait reste au journal jusqu'à ce
    /// qu'un ajustement le solde.
    #[must_use]
    pub fn breaches(&self) -> Vec<Overrun> {
        let balances = self.fold();
        self.limits
            .dimensions()
            .filter_map(|dimension| {
                let ceiling = self.limits.ceiling(dimension)?;
                let engaged = at(&balances.spent, dimension) + at(&balances.held, dimension);
                (engaged > ceiling).then_some(Overrun {
                    dimension,
                    reserved: ceiling,
                    actual: engaged,
                })
            })
            .collect()
    }

    /// Les retenues encore ouvertes.
    #[must_use]
    pub fn outstanding(&self) -> BTreeSet<Id<ReservationKind>> {
        self.fold().outstanding.into_keys().collect()
    }

    // -- interne -----------------------------------------------------------------------------

    fn ceiling_of(&self, dimension: Dimension) -> Result<u64, BudgetError> {
        self.limits
            .ceiling(dimension)
            .ok_or(BudgetError::UnboundedDimension { dimension })
    }

    fn available_from(&self, balances: &Balances, dimension: Dimension) -> u64 {
        let Some(ceiling) = self.limits.ceiling(dimension) else {
            return 0;
        };
        let funded = ceiling.min(at(&balances.allocated, dimension));
        funded
            .saturating_sub(at(&balances.spent, dimension))
            .saturating_sub(at(&balances.held, dimension))
    }

    fn outstanding_of(
        &self,
        id: Id<ReservationKind>,
        account: Id<AccountKind>,
    ) -> Result<Amounts, BudgetError> {
        if account != self.id {
            return Err(BudgetError::ForeignReservation { id });
        }
        self.fold()
            .outstanding
            .remove(&id)
            .ok_or(BudgetError::UnknownReservation { id })
    }

    fn append(
        &mut self,
        kind: EntryKind,
        reservation: Option<Id<ReservationKind>>,
        amounts: Amounts,
        reason: &str,
    ) {
        let sequence = self.entries.len() as u64;
        self.entries
            .push(Entry::new(sequence, kind, reservation, amounts, reason));
    }

    /// Déduire les soldes du journal — la seule façon de les connaître.
    fn fold(&self) -> Balances {
        let mut balances = Balances::default();
        for entry in &self.entries {
            match entry.kind() {
                EntryKind::Allocation => add_into(&mut balances.allocated, entry.amounts()),
                EntryKind::Reservation => {
                    add_into(&mut balances.held, entry.amounts());
                    if let Some(id) = entry.reservation() {
                        balances.outstanding.insert(*id, entry.amounts().clone());
                    }
                }
                EntryKind::Release => {
                    subtract_from(&mut balances.held, entry.amounts());
                    if let Some(id) = entry.reservation() {
                        balances.outstanding.remove(id);
                    }
                }
                EntryKind::Consumption => {
                    add_into(&mut balances.spent, entry.amounts());
                    if let Some(id) = entry.reservation() {
                        if let Some(held) = balances.outstanding.remove(id) {
                            subtract_from(&mut balances.held, &held);
                        }
                        balances.consumed.insert(*id, entry.amounts().clone());
                    }
                }
                EntryKind::Adjustment => add_into(&mut balances.spent, entry.amounts()),
                EntryKind::Refund => subtract_from(&mut balances.spent, entry.amounts()),
            }
        }
        balances
    }
}

fn at(amounts: &Amounts, dimension: Dimension) -> u64 {
    amounts.get(&dimension).copied().unwrap_or(0)
}

fn add_into(target: &mut Amounts, amounts: &Amounts) {
    for (dimension, amount) in amounts {
        *target.entry(*dimension).or_insert(0) += *amount;
    }
}

fn subtract_from(target: &mut Amounts, amounts: &Amounts) {
    for (dimension, amount) in amounts {
        let slot = target.entry(*dimension).or_insert(0);
        *slot = slot.saturating_sub(*amount);
    }
}

/// Ce qui empêche une écriture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetError {
    /// Une dimension que le compte ne borne pas.
    UnboundedDimension {
        /// Laquelle.
        dimension: Dimension,
    },
    /// Une allocation au-delà de la borne dure.
    BeyondCeiling {
        /// La dimension.
        dimension: Dimension,
        /// La borne.
        ceiling: u64,
        /// Ce que l'allocation porterait au total.
        requested: u64,
    },
    /// Une retenue qui ne retient rien.
    EmptyReservation,
    /// Un identifiant de retenue déjà employé.
    DuplicateReservation {
        /// Lequel.
        id: Id<ReservationKind>,
    },
    /// La borne ne suit pas.
    WouldExceed {
        /// La dimension.
        dimension: Dimension,
        /// Ce qui restait.
        available: u64,
        /// Ce qui était demandé.
        requested: u64,
    },
    /// Une retenue accordée par un autre compte.
    ForeignReservation {
        /// Laquelle.
        id: Id<ReservationKind>,
    },
    /// Une retenue inconnue ou déjà soldée.
    UnknownReservation {
        /// Laquelle.
        id: Id<ReservationKind>,
    },
}

impl fmt::Display for BudgetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnboundedDimension { dimension } => write!(
                formatter,
                "« {dimension} » n'est bornée par aucune limite : rien ne peut y être réservé, \
                 parce que rien n'y serait dépassable"
            ),
            Self::BeyondCeiling {
                dimension,
                ceiling,
                requested,
            } => write!(
                formatter,
                "allouer {requested} sur « {dimension} » dépasserait la borne dure de {ceiling}"
            ),
            Self::EmptyReservation => formatter.write_str(
                "une retenue vide satisferait la lettre de l'invariant 6 sans rien retenir",
            ),
            Self::DuplicateReservation { id } => {
                write!(formatter, "la retenue « {id} » existe déjà")
            }
            Self::WouldExceed {
                dimension,
                available,
                requested,
            } => write!(
                formatter,
                "retenir {requested} sur « {dimension} » alors qu'il en reste {available} : \
                 l'exécution n'a pas lieu"
            ),
            Self::ForeignReservation { id } => write!(
                formatter,
                "la retenue « {id} » vient d'un autre compte : la solder ici débiterait le mauvais \
                 budget"
            ),
            Self::UnknownReservation { id } => {
                write!(formatter, "la retenue « {id} » est inconnue ou déjà soldée")
            }
        }
    }
}

impl std::error::Error for BudgetError {}
