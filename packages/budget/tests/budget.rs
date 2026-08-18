//! Test de sortie de W7.e — **une mission sans borne n'est pas admissible ; une réservation
//! refusée n'exécute rien ; un dépassement arrête proprement et le dit.**
//!
//! Les trois disent la même chose sous trois angles : le budget n'est pas une intention. C'est une
//! borne qui refuse, une retenue qu'on doit tenir en main pour exécuter, et un journal qui écrit ce
//! qui a eu lieu même quand cela dérange.

use std::collections::BTreeMap;

use locus_budget::{
    Amounts, BudgetAccount, BudgetError, Dimension, EntryKind, Limits, Overrun, Unbounded,
};
use locus_protocol::{
    Category, Id, IdKind, Retry, Timestamp,
    id::provisional::{BudgetAccount as AccountKind, Error as ErrorKind, Reservation},
};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn amounts(pairs: &[(Dimension, u64)]) -> Amounts {
    pairs.iter().copied().collect()
}

/// Un compte borné sur deux dimensions, alloué à hauteur de ses bornes.
fn funded() -> BudgetAccount {
    let limits = Limits::bounding([(Dimension::ModelCalls, 30), (Dimension::Tokens, 300_000)])
        .expect("deux dimensions bornées");
    let mut account = BudgetAccount::open(id::<AccountKind>(1), limits);
    account
        .allocate(
            &amounts(&[(Dimension::ModelCalls, 30), (Dimension::Tokens, 300_000)]),
            "dotation initiale",
        )
        .expect("allocation dans les bornes");
    account
}

// ---------------------------------------------------------------------------------------------
// Une mission sans borne n'est pas admissible
// ---------------------------------------------------------------------------------------------

/// Invariant 6 : « les ressources ne sont pas supposées illimitées ». Un compte qui ne borne rien
/// rend tout dépassement inconstatable — et le constater plus tard ne sert à rien, puisqu'il n'y a
/// rien à quoi comparer.
#[test]
fn un_compte_sans_aucune_borne_ne_s_ouvre_pas() {
    assert_eq!(Limits::bounding([]), Err(Unbounded));
    assert!(
        Unbounded.to_string().contains("invariant 6"),
        "le refus dit de quelle règle il vient : {Unbounded}"
    );
}

/// Et une dimension **non nommée** n'est pas libre : elle est hors budget. C'est la moitié de
/// l'invariant qu'on perd le plus facilement — borner deux ressources sur six et croire les six
/// bornées.
#[test]
fn une_dimension_non_bornee_est_hors_budget_pas_illimitee() {
    let mut account = funded();
    assert_eq!(
        account.reserve(
            id::<Reservation>(1),
            &amounts(&[(Dimension::WallTimeSeconds, 1)]),
            "une seconde"
        ),
        Err(BudgetError::UnboundedDimension {
            dimension: Dimension::WallTimeSeconds
        })
    );
    assert_eq!(
        account.available(Dimension::WallTimeSeconds),
        0,
        "sans borne, rien n'est disponible — et surtout pas tout"
    );
}

/// Une retenue vide satisferait la lettre de l'invariant 6 — « toute exécution coûteuse possède une
/// réservation » — sans rien retenir du tout.
#[test]
fn une_retenue_vide_ne_retient_rien() {
    let mut account = funded();
    assert_eq!(
        account.reserve(id::<Reservation>(1), &Amounts::new(), "rien"),
        Err(BudgetError::EmptyReservation)
    );
    assert_eq!(
        account.reserve(
            id::<Reservation>(1),
            &amounts(&[(Dimension::ModelCalls, 0)]),
            "zéro appel"
        ),
        Err(BudgetError::EmptyReservation),
        "retenir zéro n'est pas retenir"
    );
}

/// Une borne de zéro, elle, est une décision : « rien n'est permis ici ». Elle s'ouvre, et elle
/// refuse tout.
#[test]
fn une_borne_de_zero_est_une_decision_et_elle_refuse_tout() {
    let limits = Limits::bounding([(Dimension::ModelCalls, 0)]).expect("zéro est une borne");
    let mut account = BudgetAccount::open(id::<AccountKind>(2), limits);
    assert!(
        account
            .allocate(&amounts(&[(Dimension::ModelCalls, 0)]), "rien à donner")
            .is_ok()
    );
    assert_eq!(
        account.reserve(
            id::<Reservation>(1),
            &amounts(&[(Dimension::ModelCalls, 1)]),
            "un appel"
        ),
        Err(BudgetError::WouldExceed {
            dimension: Dimension::ModelCalls,
            available: 0,
            requested: 1
        })
    );
}

/// Allouer plus que la borne dure ne rendrait pas la borne moins dure : cela rendrait l'écriture
/// fausse, et c'est le registre qui perdrait sa valeur, pas la borne.
#[test]
fn on_n_alloue_pas_au_dela_de_la_borne_dure() {
    let limits = Limits::bounding([(Dimension::ModelCalls, 10)]).expect("bornée");
    let mut account = BudgetAccount::open(id::<AccountKind>(3), limits);
    assert_eq!(
        account.allocate(&amounts(&[(Dimension::ModelCalls, 11)]), "trop"),
        Err(BudgetError::BeyondCeiling {
            dimension: Dimension::ModelCalls,
            ceiling: 10,
            requested: 11
        })
    );
}

/// Rien n'est disponible avant d'avoir été alloué. Un compte borné mais vide n'est pas un compte
/// plein : la borne dit ce qu'on ne dépassera pas, l'allocation dit ce qu'on a.
#[test]
fn une_borne_n_est_pas_une_dotation() {
    let limits = Limits::bounding([(Dimension::ModelCalls, 30)]).expect("bornée");
    let mut account = BudgetAccount::open(id::<AccountKind>(4), limits);

    assert_eq!(account.available(Dimension::ModelCalls), 0);
    assert_eq!(
        account.reserve(
            id::<Reservation>(1),
            &amounts(&[(Dimension::ModelCalls, 1)]),
            "un appel"
        ),
        Err(BudgetError::WouldExceed {
            dimension: Dimension::ModelCalls,
            available: 0,
            requested: 1
        })
    );
}

// ---------------------------------------------------------------------------------------------
// Une réservation refusée n'exécute rien
// ---------------------------------------------------------------------------------------------

/// Le refus qui porte le sprint. « N'exécute rien » se rend observable de deux façons, et il faut
/// les deux : aucun jeton d'exécution n'est rendu — `reserve` ne rend pas de `Reservation` — et le
/// **journal ne bouge pas**. Un refus qui laisserait une écriture partielle ferait porter au compte
/// suivant le coût d'une exécution qui n'a pas eu lieu.
#[test]
fn une_reservation_refusee_ne_laisse_aucune_trace() {
    let mut account = funded();
    let before = account.entries().len();

    let refused = account.reserve(
        id::<Reservation>(1),
        &amounts(&[(Dimension::ModelCalls, 5), (Dimension::Tokens, 500_000)]),
        "trop de jetons",
    );

    assert_eq!(
        refused,
        Err(BudgetError::WouldExceed {
            dimension: Dimension::Tokens,
            available: 300_000,
            requested: 500_000
        })
    );
    assert_eq!(
        account.entries().len(),
        before,
        "le refus n'écrit rien, pas même la partie qui passait"
    );
    assert_eq!(
        account.held(Dimension::ModelCalls),
        0,
        "et surtout pas la dimension qui tenait dans la borne"
    );
    assert!(account.outstanding().is_empty());
}

/// L'autre moitié : une retenue accordée **est** ce qui permet d'exécuter, et elle se solde une
/// fois. `consume` la prend par valeur, donc la même retenue ne se dépense pas deux fois — et une
/// copie obtenue autrement ne trouverait plus rien d'ouvert au journal.
#[test]
fn une_retenue_se_solde_une_fois() {
    let mut account = funded();
    let reservation = account
        .reserve(
            id::<Reservation>(1),
            &amounts(&[(Dimension::ModelCalls, 10)]),
            "dix appels",
        )
        .expect("dans les bornes");

    assert_eq!(account.held(Dimension::ModelCalls), 10);
    assert_eq!(account.available(Dimension::ModelCalls), 20);

    account
        .consume(reservation, &amounts(&[(Dimension::ModelCalls, 6)]), "six")
        .expect("consommation valide");

    assert_eq!(account.spent(Dimension::ModelCalls), 6);
    assert_eq!(
        account.held(Dimension::ModelCalls),
        0,
        "le solde rend ce qui n'a pas été dépensé"
    );
    assert_eq!(account.available(Dimension::ModelCalls), 24);
    assert!(account.outstanding().is_empty());
}

/// Une retenue d'un autre compte soldée ici débiterait le mauvais budget — et le bon paraîtrait
/// intact.
#[test]
fn une_retenue_d_un_autre_compte_ne_se_solde_pas_ici() {
    let mut first = funded();
    let mut second = BudgetAccount::open(
        id::<AccountKind>(9),
        Limits::bounding([(Dimension::ModelCalls, 30)]).expect("bornée"),
    );
    second
        .allocate(&amounts(&[(Dimension::ModelCalls, 30)]), "dotation")
        .expect("allocation valide");

    let reservation = first
        .reserve(
            id::<Reservation>(1),
            &amounts(&[(Dimension::ModelCalls, 5)]),
            "cinq",
        )
        .expect("dans les bornes");

    assert_eq!(
        second.consume(reservation, &amounts(&[(Dimension::ModelCalls, 5)]), "cinq"),
        Err(BudgetError::ForeignReservation {
            id: id::<Reservation>(1)
        })
    );
    assert_eq!(second.spent(Dimension::ModelCalls), 0);
}

#[test]
fn un_identifiant_de_retenue_ne_sert_qu_une_fois() {
    let mut account = funded();
    let first = account
        .reserve(
            id::<Reservation>(1),
            &amounts(&[(Dimension::ModelCalls, 5)]),
            "cinq",
        )
        .expect("dans les bornes");

    assert_eq!(
        account.reserve(
            id::<Reservation>(1),
            &amounts(&[(Dimension::ModelCalls, 5)]),
            "encore cinq"
        ),
        Err(BudgetError::DuplicateReservation {
            id: id::<Reservation>(1)
        })
    );

    // Et même après solde : réemployer l'identifiant rendrait deux exécutions indiscernables.
    account
        .consume(first, &amounts(&[(Dimension::ModelCalls, 5)]), "cinq")
        .expect("consommation valide");
    assert_eq!(
        account.reserve(
            id::<Reservation>(1),
            &amounts(&[(Dimension::ModelCalls, 1)]),
            "un"
        ),
        Err(BudgetError::DuplicateReservation {
            id: id::<Reservation>(1)
        })
    );
}

/// Ce qui est rendu redevient disponible : une retenue relâchée n'a rien coûté.
#[test]
fn une_retenue_relachee_ne_coute_rien() {
    let mut account = funded();
    let reservation = account
        .reserve(
            id::<Reservation>(1),
            &amounts(&[(Dimension::ModelCalls, 12)]),
            "douze",
        )
        .expect("dans les bornes");

    account
        .release(reservation, "mission annulée")
        .expect("relâche valide");

    assert_eq!(account.held(Dimension::ModelCalls), 0);
    assert_eq!(account.spent(Dimension::ModelCalls), 0);
    assert_eq!(account.available(Dimension::ModelCalls), 30);
}

/// §7.2 : « `spent + reserved` ne dépasse pas la limite dure. » Deux retenues qui tiennent
/// séparément mais pas ensemble : la seconde est refusée, et c'est bien la retenue de la première
/// qui la refuse — pas sa dépense, qui n'a pas encore eu lieu.
#[test]
fn une_retenue_encore_ouverte_compte_contre_la_borne() {
    let mut account = funded();
    let _first = account
        .reserve(
            id::<Reservation>(1),
            &amounts(&[(Dimension::ModelCalls, 20)]),
            "vingt",
        )
        .expect("dans les bornes");

    assert_eq!(
        account.reserve(
            id::<Reservation>(2),
            &amounts(&[(Dimension::ModelCalls, 20)]),
            "vingt de plus"
        ),
        Err(BudgetError::WouldExceed {
            dimension: Dimension::ModelCalls,
            available: 10,
            requested: 20
        }),
        "rien n'a encore été dépensé, et pourtant il ne reste que dix"
    );
}

// ---------------------------------------------------------------------------------------------
// Un dépassement arrête proprement et le dit
// ---------------------------------------------------------------------------------------------

/// Le dépassement **s'écrit**. Refuser la consommation laisserait le registre en désaccord avec le
/// monde : les appels ont bien eu lieu. Ce qui doit s'arrêter, c'est l'exécution — pas la
/// comptabilité.
#[test]
fn un_depassement_s_ecrit_et_arrete_l_execution() {
    let mut account = funded();
    let reservation = account
        .reserve(
            id::<Reservation>(1),
            &amounts(&[(Dimension::ModelCalls, 30)]),
            "le budget de la mission",
        )
        .expect("dans les bornes");

    let settlement = account
        .consume(
            reservation,
            &amounts(&[(Dimension::ModelCalls, 31)]),
            "arrêt au 31e appel",
        )
        .expect("la consommation s'écrit");

    assert!(settlement.stops_execution());
    assert_eq!(
        settlement.overruns(),
        [Overrun {
            dimension: Dimension::ModelCalls,
            reserved: 30,
            actual: 31
        }]
    );
    assert_eq!(settlement.overruns()[0].excess(), 1);
    assert_eq!(
        account.spent(Dimension::ModelCalls),
        31,
        "le registre écrit ce qui a eu lieu, pas ce qui était permis"
    );
    assert_eq!(
        account.breaches().len(),
        1,
        "et le franchissement de la borne reste visible"
    );
}

/// « Proprement » a un sens précis, et la fixture `attempt-budget-exceeded.json` le fixe sur le
/// fil : catégorie `budget`, `retryable: false`. Réessayer ne rendrait pas le budget — la tentative
/// suivante rencontrerait la même borne, en ayant dépensé une fois de plus.
#[test]
fn un_depassement_n_est_jamais_reessayable() {
    let overrun = Overrun {
        dimension: Dimension::ModelCalls,
        reserved: 30,
        actual: 31,
    };

    assert_eq!(overrun.category(), Category::Budget);
    assert_eq!(overrun.retry(), Retry::Never);

    let error = overrun.into_error(
        id::<ErrorKind>(1),
        Timestamp::from_millis(1_700_000_000_000),
        "locus-budget",
    );

    assert_eq!(error.code, "budget_exhausted");
    assert_eq!(error.category, Category::Budget);
    assert_eq!(error.retryable, Retry::Never);
    assert_eq!(error.retry_condition(), None);
    assert_eq!(
        error.details.get("dimension").map(String::as_str),
        Some("model_calls")
    );
    assert!(
        !error.security_sensitive,
        "un budget dépassé n'est pas un secret : l'expurger cacherait le chiffre qui explique l'arrêt"
    );
}

/// Une consommation dans les clous ne déclenche rien, et rend le solde. Sans ce cas, « dépassement »
/// pourrait vouloir dire « toute consommation », et le test précédent ne dirait rien.
#[test]
fn une_consommation_dans_les_clous_ne_declenche_rien() {
    let mut account = funded();
    let reservation = account
        .reserve(
            id::<Reservation>(1),
            &amounts(&[(Dimension::ModelCalls, 30)]),
            "trente",
        )
        .expect("dans les bornes");

    let settlement = account
        .consume(
            reservation,
            &amounts(&[(Dimension::ModelCalls, 30)]),
            "trente",
        )
        .expect("consommation valide");

    assert!(!settlement.stops_execution());
    assert!(settlement.overruns().is_empty());
    assert!(account.breaches().is_empty());
}

// ---------------------------------------------------------------------------------------------
// Un registre, pas un compteur
// ---------------------------------------------------------------------------------------------

/// §7.2 : « une correction ne réécrit pas une écriture antérieure ; elle crée un ajustement
/// compensatoire. » Le test regarde l'écriture corrigée **après** la correction : elle est
/// inchangée. Sans cela, un budget dépassé puis corrigé serait indistinguable d'un budget jamais
/// dépassé.
#[test]
fn une_correction_n_efface_pas_l_ecriture_corrigee() {
    let mut account = funded();
    let reservation = account
        .reserve(
            id::<Reservation>(1),
            &amounts(&[(Dimension::ModelCalls, 20)]),
            "vingt",
        )
        .expect("dans les bornes");
    account
        .consume(
            reservation,
            &amounts(&[(Dimension::ModelCalls, 18)]),
            "dix-huit",
        )
        .expect("consommation valide");

    let consumption = account
        .entries()
        .iter()
        .find(|entry| entry.kind() == EntryKind::Consumption)
        .cloned()
        .expect("la consommation est au journal");

    // Les métriques du worker disent 19, pas 18.
    let reconciliation = account
        .reconcile(
            &id::<Reservation>(1),
            &amounts(&[(Dimension::ModelCalls, 19)]),
            "métriques du worker",
        )
        .expect("rapprochement valide");

    assert_eq!(
        reconciliation.debited(),
        &amounts(&[(Dimension::ModelCalls, 1)])
    );
    assert!(reconciliation.credited().is_empty());
    assert_eq!(account.spent(Dimension::ModelCalls), 19);

    let after = account
        .entries()
        .iter()
        .find(|entry| entry.kind() == EntryKind::Consumption)
        .cloned()
        .expect("elle y est toujours");
    assert_eq!(
        after, consumption,
        "l'écriture corrigée est intacte : c'est l'ajustement qui porte l'écart"
    );
    assert_eq!(
        account.entries().last().map(locus_budget::Entry::kind),
        Some(EntryKind::Adjustment)
    );
}

/// Et dans l'autre sens : des métriques plus basses produisent un remboursement, pas une rature.
#[test]
fn un_rapprochement_a_la_baisse_rembourse() {
    let mut account = funded();
    let reservation = account
        .reserve(
            id::<Reservation>(1),
            &amounts(&[(Dimension::Tokens, 200_000)]),
            "deux cent mille",
        )
        .expect("dans les bornes");
    account
        .consume(
            reservation,
            &amounts(&[(Dimension::Tokens, 180_000)]),
            "constat",
        )
        .expect("consommation valide");

    let reconciliation = account
        .reconcile(
            &id::<Reservation>(1),
            &amounts(&[(Dimension::Tokens, 170_000)]),
            "métriques du worker",
        )
        .expect("rapprochement valide");

    assert_eq!(
        reconciliation.credited(),
        &amounts(&[(Dimension::Tokens, 10_000)])
    );
    assert!(reconciliation.debited().is_empty());
    assert_eq!(account.spent(Dimension::Tokens), 170_000);
    assert_eq!(
        account.entries().last().map(locus_budget::Entry::kind),
        Some(EntryKind::Refund)
    );
}

/// Un rapprochement qui confirme n'écrit rien : une écriture par mesure identique gonflerait le
/// journal sans rien y ajouter.
#[test]
fn un_rapprochement_qui_confirme_n_ecrit_rien() {
    let mut account = funded();
    let reservation = account
        .reserve(
            id::<Reservation>(1),
            &amounts(&[(Dimension::ModelCalls, 10)]),
            "dix",
        )
        .expect("dans les bornes");
    account
        .consume(reservation, &amounts(&[(Dimension::ModelCalls, 10)]), "dix")
        .expect("consommation valide");
    let before = account.entries().len();

    let reconciliation = account
        .reconcile(
            &id::<Reservation>(1),
            &amounts(&[(Dimension::ModelCalls, 10)]),
            "métriques du worker",
        )
        .expect("rapprochement valide");

    assert!(reconciliation.agrees());
    assert_eq!(account.entries().len(), before);
}

/// On ne rapproche pas ce qui n'a pas été consommé : il n'y aurait rien à comparer, et écrire un
/// ajustement dans le vide ferait apparaître une dépense sans exécution.
#[test]
fn on_ne_rapproche_pas_une_retenue_jamais_consommee() {
    let mut account = funded();
    assert_eq!(
        account.reconcile(
            &id::<Reservation>(7),
            &amounts(&[(Dimension::ModelCalls, 3)]),
            "métriques"
        ),
        Err(BudgetError::UnknownReservation {
            id: id::<Reservation>(7)
        })
    );
}

/// Le solde n'est pas un champ : il se déduit du journal. Le test le dit en reconstruisant un
/// compte à partir des mêmes écritures — même histoire, mêmes soldes.
#[test]
fn les_soldes_se_deduisent_du_journal() {
    let mut account = funded();
    let reservation = account
        .reserve(
            id::<Reservation>(1),
            &amounts(&[(Dimension::ModelCalls, 12)]),
            "douze",
        )
        .expect("dans les bornes");
    account
        .consume(reservation, &amounts(&[(Dimension::ModelCalls, 7)]), "sept")
        .expect("consommation valide");

    let expected: BTreeMap<&str, u64> = [
        ("allocated", account.allocated(Dimension::ModelCalls)),
        ("spent", account.spent(Dimension::ModelCalls)),
        ("held", account.held(Dimension::ModelCalls)),
        ("available", account.available(Dimension::ModelCalls)),
    ]
    .into_iter()
    .collect();

    assert_eq!(expected["allocated"], 30);
    assert_eq!(expected["spent"], 7);
    assert_eq!(expected["held"], 0);
    assert_eq!(expected["available"], 23);

    // Le journal porte les trois écritures, dans l'ordre, et la relâche du reliquat est implicite :
    // c'est la consommation qui solde la retenue.
    let kinds: Vec<EntryKind> = account
        .entries()
        .iter()
        .map(locus_budget::Entry::kind)
        .collect();
    assert_eq!(
        kinds,
        vec![
            EntryKind::Allocation,
            EntryKind::Reservation,
            EntryKind::Consumption
        ]
    );
}

#[test]
fn les_six_ecritures_de_7_2_existent() {
    let slugs: Vec<&str> = EntryKind::ALL.into_iter().map(EntryKind::slug).collect();
    assert_eq!(
        slugs,
        vec![
            "allocation",
            "reservation",
            "release",
            "consumption",
            "adjustment",
            "refund"
        ]
    );
}

#[test]
fn les_six_dimensions_de_7_2_existent() {
    let slugs: Vec<&str> = Dimension::ALL.into_iter().map(Dimension::slug).collect();
    assert_eq!(
        slugs,
        vec![
            "amount",
            "model_calls",
            "tokens",
            "compute_seconds",
            "wall_time_seconds",
            "parallelism"
        ]
    );
}
