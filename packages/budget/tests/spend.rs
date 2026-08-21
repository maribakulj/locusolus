//! Test de sortie de `W21.m` — **la classification de dépense**, ADR 0024.
//!
//! 1. Une écriture passée par une méthode qui ne classe pas se lit **non classée**, jamais
//!    « travail ».
//! 2. Les soldes **héritent** de la retenue : rembourser de la coordination reste de la
//!    coordination.
//! 3. Aucune classification ne se déduit du texte libre de `reason`.
//! 4. Ce que la classification débloque — les deux termes de `communication_tokens` se comptent, et
//!    les non classées restent à part.

use locus_budget::{
    Amounts, BudgetAccount, Classification, Dimension, Entry, EntryKind, Limits, Spend,
};
use locus_protocol::{
    Id, IdKind, Timestamp,
    id::provisional::{BudgetAccount as AccountKind, Reservation as ReservationKind},
};

/// Le code d'un fichier, c'est sa source moins ses commentaires — voir `W21.j`.
fn code_seul(source: &str) -> String {
    source
        .lines()
        .filter(|ligne| !ligne.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn compte() -> BudgetAccount {
    BudgetAccount::open(
        id::<AccountKind>(1),
        Limits::bounding([(Dimension::Tokens, 1_000_000)]).expect("une borne suffit"),
    )
}

fn jetons(combien: u64) -> Amounts {
    [(Dimension::Tokens, combien)].into_iter().collect()
}

fn retenue(seed: u8) -> Id<ReservationKind> {
    id::<ReservationKind>(seed)
}

fn du_genre(compte: &BudgetAccount, genre: EntryKind) -> Vec<&Entry> {
    compte
        .entries()
        .iter()
        .filter(|entry| entry.kind() == genre)
        .collect()
}

// ---------------------------------------------------------------------------------------------
// 1. Une écriture sans le champ est non classée, jamais « travail »
// ---------------------------------------------------------------------------------------------

/// **Une écriture passée sans classification se lit non classée.**
///
/// Le test qui porte l'item. `allocate` et `reserve` sont les méthodes d'avant la migration : elles
/// n'ont pas changé de signature, et les écritures qu'elles produisent n'ont **pas** de
/// classification. Le défaut serviable — supposer du travail — serait majoritairement juste, donc
/// invisible, et fausserait `communication_tokens` dans le sens qui rassure.
#[test]
fn une_ecriture_sans_le_champ_est_non_classee_jamais_du_travail() {
    let mut compte = compte();
    compte
        .allocate(&jetons(10_000), "dotation initiale")
        .expect("dotation licite");
    let retenue = compte
        .reserve(retenue(10), &jetons(400), "une retenue d'avant")
        .expect("retenue licite");
    compte
        .consume(retenue, &jetons(380), "constat")
        .expect("constat licite");

    for entry in compte.entries() {
        assert_eq!(
            entry.spend(),
            Classification::Unclassified,
            "#{} {}",
            entry.sequence(),
            entry.kind()
        );
        assert_eq!(entry.spend().spend(), None);
        assert!(!entry.spend().is_classified());
        assert_ne!(
            entry.spend(),
            Classification::Classified(Spend::Work),
            "« non classé » ne devient jamais « travail » par défaut"
        );
    }
}

/// **L'ignorance n'est pas une valeur que le classificateur peut choisir.**
///
/// `Spend` porte deux valeurs, et pas de troisième pour « on ne sait pas ». Une énumération à trois
/// barreaux laisserait un appelant *déclarer* non classé, ce qui est une affirmation ; ici l'absence
/// se constate, elle ne se déclare pas. Même séparation que les deux verdicts de `xiiif` §19.
#[test]
fn l_ignorance_n_est_pas_un_troisieme_objet_de_depense() {
    assert_eq!(Spend::ALL.len(), 2);
    assert_eq!(Spend::ALL, [Spend::Coordination, Spend::Work]);

    for objet in Spend::ALL {
        let classee = Classification::from(objet);
        assert_eq!(classee.spend(), Some(objet));
        assert!(classee.is_classified());
        assert_ne!(classee, Classification::Unclassified);
    }

    // Et le défaut du type est l'ignorance, pas l'une des deux.
    assert_eq!(Classification::default(), Classification::Unclassified);
}

/// **Une retenue que le journal ne connaît pas se lit non classée, pas « travail ».**
///
/// Le classificateur est une recherche, et une recherche qui ne trouve rien doit répondre quelque
/// chose. Répondre « travail » serait le défaut serviable une fois de plus, déplacé du champ vers
/// la recherche — invisible, majoritairement juste, et faux dans le sens qui rassure.
///
/// La lecture est publique précisément pour que cette branche soit **atteignable** : une branche que
/// l'API ne peut pas exercer est une branche que les tests ne tiennent pas.
#[test]
fn une_retenue_inconnue_se_lit_non_classee() {
    let mut compte = compte();
    compte
        .allocate_for(&jetons(10_000), "dotation", Spend::Work)
        .expect("dotation licite");
    compte
        .reserve_for(
            retenue(20),
            &jetons(100),
            "négociation",
            Spend::Coordination,
        )
        .expect("retenue licite");

    assert_eq!(
        compte.classification_of(&retenue(20)).spend(),
        Some(Spend::Coordination),
        "une retenue connue rend son objet"
    );
    assert_eq!(
        compte.classification_of(&retenue(99)),
        Classification::Unclassified,
        "une retenue inconnue n'est pas du travail"
    );
    assert_ne!(
        compte.classification_of(&retenue(99)),
        Classification::Classified(Spend::Work)
    );
}

/// **Les deux objets ne portent pas le même nom.**
///
/// `slug` et `Display` sont ce qu'un rapport imprime. Deux valeurs distinctes qui s'affichent
/// pareil rendraient `communication_tokens` illisible **au moment de le lire**, alors que le calcul,
/// lui, serait juste — le pire endroit pour une faute.
#[test]
fn les_deux_objets_portent_des_noms_distincts() {
    assert_eq!(Spend::Coordination.slug(), "coordination");
    assert_eq!(Spend::Work.slug(), "work");
    assert_ne!(Spend::Coordination.slug(), Spend::Work.slug());

    assert_eq!(Spend::Coordination.to_string(), "coordination");
    assert_eq!(Spend::Work.to_string(), "work");

    assert_eq!(
        Classification::Classified(Spend::Coordination).to_string(),
        "coordination"
    );
    assert_eq!(Classification::Unclassified.to_string(), "non classée");
    assert_ne!(
        Classification::Unclassified.to_string(),
        Classification::Classified(Spend::Work).to_string(),
        "l'ignorance ne s'imprime pas comme du travail non plus"
    );
}

/// **Une écriture classée porte son objet, et les deux objets se distinguent.**
#[test]
fn une_ecriture_classee_porte_son_objet() {
    let mut compte = compte();
    compte
        .allocate_for(&jetons(10_000), "dotation", Spend::Work)
        .expect("dotation licite");
    compte
        .reserve_for(
            retenue(20),
            &jetons(100),
            "négociation",
            Spend::Coordination,
        )
        .expect("retenue licite");

    let allocations = du_genre(&compte, EntryKind::Allocation);
    let retenues = du_genre(&compte, EntryKind::Reservation);

    assert_eq!(allocations[0].spend().spend(), Some(Spend::Work));
    assert_eq!(retenues[0].spend().spend(), Some(Spend::Coordination));
    assert_ne!(allocations[0].spend(), retenues[0].spend());
}

// ---------------------------------------------------------------------------------------------
// 2. Les soldes héritent
// ---------------------------------------------------------------------------------------------

/// **Rendre, constater et rapprocher héritent de la retenue.**
///
/// Rembourser de la coordination reste de la coordination. Redemander l'objet à chaque solde
/// ouvrirait la porte à deux réponses pour la même retenue, et le journal porterait une
/// contradiction que personne n'aurait voulue.
#[test]
fn les_soldes_heritent_de_la_retenue() {
    let mut compte = compte();
    compte
        .allocate_for(&jetons(10_000), "dotation", Spend::Work)
        .expect("dotation licite");
    let id = retenue(20);
    let tenue = compte
        .reserve_for(id, &jetons(500), "transmission", Spend::Coordination)
        .expect("retenue licite");
    compte
        .consume(tenue, &jetons(500), "constat")
        .expect("constat licite");
    // Chaque rapprochement se compare à la **consommation enregistrée**, pas au cumul des
    // corrections : 700 puis 300 contre un constat de 500 donnent un ajustement puis un
    // remboursement, ce qui est exactement ce qu'il faut ici pour exercer les deux mouvements.
    let a_la_hausse = compte
        .reconcile(&id, &jetons(700), "métriques du worker")
        .expect("rapprochement licite");
    let a_la_baisse = compte
        .reconcile(&id, &jetons(300), "métriques corrigées")
        .expect("rapprochement licite");

    assert!(!a_la_hausse.agrees(), "le constat a été dépassé");
    assert!(!a_la_baisse.agrees(), "puis revu à la baisse");
    assert_eq!(du_genre(&compte, EntryKind::Adjustment).len(), 1);
    assert_eq!(du_genre(&compte, EntryKind::Refund).len(), 1);

    for entry in compte.entries() {
        let attendu = match entry.kind() {
            EntryKind::Allocation => Spend::Work,
            _ => Spend::Coordination,
        };
        assert_eq!(
            entry.spend().spend(),
            Some(attendu),
            "#{} {}",
            entry.sequence(),
            entry.kind()
        );
    }
}

/// **Une retenue rendue hérite aussi — et une retenue non classée n'anoblit rien.**
///
/// L'héritage n'invente pas : ce qui descend d'une retenue non classée reste non classé. Un héritage
/// qui « comblerait » l'ignorance en aval serait exactement le défaut par le travail, déplacé d'un
/// cran.
#[test]
fn l_heritage_n_invente_pas_ce_que_personne_n_a_dit() {
    let mut compte = compte();
    compte
        .allocate(&jetons(10_000), "dotation")
        .expect("dotation licite");
    let classee = compte
        .reserve_for(retenue(30), &jetons(300), "arbitrage", Spend::Coordination)
        .expect("retenue licite");
    let muette = compte
        .reserve(retenue(31), &jetons(300), "une retenue d'avant")
        .expect("retenue licite");

    compte.release(classee, "inutilisée").expect("rendu licite");
    compte.release(muette, "inutilisée").expect("rendu licite");

    let rendus = du_genre(&compte, EntryKind::Release);
    assert_eq!(rendus.len(), 2);
    assert_eq!(rendus[0].spend().spend(), Some(Spend::Coordination));
    assert_eq!(
        rendus[1].spend(),
        Classification::Unclassified,
        "un rendu ne classe pas ce que la retenue n'a pas dit"
    );
}

/// **Deux retenues d'objets différents ne se contaminent pas.**
#[test]
fn deux_retenues_ne_se_contaminent_pas() {
    let mut compte = compte();
    compte
        .allocate_for(&jetons(10_000), "dotation", Spend::Work)
        .expect("dotation licite");
    let coord = compte
        .reserve_for(
            retenue(20),
            &jetons(100),
            "négociation",
            Spend::Coordination,
        )
        .expect("retenue licite");
    let travail = compte
        .reserve_for(retenue(21), &jetons(900), "calcul", Spend::Work)
        .expect("retenue licite");

    compte
        .consume(coord, &jetons(100), "constat")
        .expect("constat licite");
    compte
        .consume(travail, &jetons(900), "constat")
        .expect("constat licite");

    let constats = du_genre(&compte, EntryKind::Consumption);
    assert_eq!(constats.len(), 2);
    assert_eq!(constats[0].spend().spend(), Some(Spend::Coordination));
    assert_eq!(constats[1].spend().spend(), Some(Spend::Work));
}

// ---------------------------------------------------------------------------------------------
// 3. Rien ne se déduit de `reason`
// ---------------------------------------------------------------------------------------------

/// **Le classificateur ne lit pas le texte libre.**
///
/// Une justesse qui dépendrait de la rédaction de chaque appelant se dégraderait au premier qui
/// écrit autrement — et se dégraderait en silence, puisqu'un motif non reconnu retomberait sur le
/// défaut.
///
/// Les motifs lisent le **code seul**, la source privée de ses commentaires, pour la raison établie
/// en `W21.j` : une anti-garde qui lit la prose mord sur la phrase qui explique l'absence qu'elle
/// surveille.
#[test]
fn le_classificateur_ne_lit_pas_le_motif() {
    let code = code_seul(include_str!("../src/spend.rs"));
    assert!(
        code.contains("pub fn") || code.contains("pub(crate) fn"),
        "le nettoyage a trop enlevé : ce test ne lit plus ce qu'il croit lire"
    );

    for interdit in [
        "reason",
        "contains(",
        "starts_with",
        "to_lowercase",
        "&str",
        "String",
    ] {
        assert!(
            !code.contains(interdit),
            "« {interdit} » ouvrirait la porte à une classification déduite du texte libre"
        );
    }
}

/// **Le motif n'influence rien : deux écritures aux motifs opposés ont la même classification.**
///
/// La garde précédente lit la source ; celle-ci exerce le comportement. Un motif qui contient le mot
/// « coordination » sur une retenue déclarée `Work` reste `Work`, et réciproquement.
#[test]
fn un_motif_trompeur_ne_change_rien() {
    let mut compte = compte();
    compte
        .allocate_for(&jetons(10_000), "dotation", Spend::Work)
        .expect("dotation licite");
    let menteuse = compte
        .reserve_for(
            retenue(40),
            &jetons(100),
            "coordination handoff négociation",
            Spend::Work,
        )
        .expect("retenue licite");
    compte
        .consume(menteuse, &jetons(100), "coordination coordination")
        .expect("constat licite");

    for entry in du_genre(&compte, EntryKind::Consumption) {
        assert_eq!(
            entry.spend().spend(),
            Some(Spend::Work),
            "le motif dit le contraire de la déclaration, et c'est la déclaration qui compte"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 4. Ce que la classification débloque
// ---------------------------------------------------------------------------------------------

/// **Les deux termes de `communication_tokens` se comptent, et les non classées restent à part.**
///
/// `W21.l` divisera les jetons de coordination par les jetons totaux. Ce test ne livre pas la
/// métrique — il vérifie que la dépendance technique qui la bloquait est bien levée : les jetons
/// consommés se répartissent en trois tas dont aucun n'absorbe l'ignorance.
#[test]
fn les_trois_tas_de_jetons_se_separent() {
    let mut compte = compte();
    compte
        .allocate(&jetons(100_000), "dotation")
        .expect("dotation licite");

    let declarees = [
        (50_u8, 120_u64, Some(Spend::Coordination)),
        (51, 800, Some(Spend::Work)),
        (52, 80, Some(Spend::Coordination)),
        (53, 500, None),
    ];
    for (seed, montant, objet) in declarees {
        let tenue = match objet {
            Some(objet) => compte.reserve_for(retenue(seed), &jetons(montant), "retenue", objet),
            None => compte.reserve(retenue(seed), &jetons(montant), "retenue"),
        }
        .expect("retenue licite");
        compte
            .consume(tenue, &jetons(montant), "constat")
            .expect("constat licite");
    }

    let mut coordination = 0_u64;
    let mut travail = 0_u64;
    let mut non_classes = 0_u64;
    for entry in du_genre(&compte, EntryKind::Consumption) {
        let montant = entry
            .amounts()
            .get(&Dimension::Tokens)
            .copied()
            .unwrap_or(0);
        match entry.spend().spend() {
            Some(Spend::Coordination) => coordination += montant,
            Some(Spend::Work) => travail += montant,
            None => non_classes += montant,
        }
    }

    assert_eq!(coordination, 200);
    assert_eq!(travail, 800);
    assert_eq!(
        non_classes, 500,
        "les non classées se comptent à part et n'entrent dans aucun des deux termes"
    );
    assert_eq!(
        coordination + travail,
        1_000,
        "le dénominateur de W21.l ne contient que ce que quelqu'un a déclaré"
    );
}
