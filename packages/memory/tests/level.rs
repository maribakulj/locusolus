//! Test de sortie de W17.a — **les trois garanties de l'item.**
//!
//! 1. Les sept niveaux de §16.1 se lisent sous leur nom.
//! 2. Ce qui est **canonique** ne se déclare jamais régénérable, et ce qui est projection le
//!    déclare toujours.
//! 3. Une mémoire dont le niveau n'est pas nommé n'existe pas.

use locus_memory::{Level, MemoryError, Shelf, Substance};

// ---------------------------------------------------------------------------------------------
// 1. Les sept, sous leur nom
// ---------------------------------------------------------------------------------------------

#[test]
fn the_seven_levels_of_the_section_are_there_under_their_names() {
    let slugs: Vec<&str> = Level::ALL.iter().map(|level| level.slug()).collect();
    assert_eq!(
        slugs,
        [
            "agent-private",
            "team",
            "branch",
            "workstream",
            "program",
            "cross-program",
            "disciplinary",
        ]
    );
    for level in Level::ALL {
        assert_eq!(Level::parse(level.slug()), Some(level));
    }
}

/// La liste est **close** : un niveau décide de qui peut lire, et une mémoire sans portée déclarée
/// finit par être lue par tout le monde, faute de raison de refuser.
#[test]
fn a_level_nobody_named_does_not_exist() {
    for invented in ["global", "session", "scratch", "shared", "public"] {
        assert_eq!(
            Level::parse(invented),
            None,
            "« {invented} » n'est pas un niveau de §16.1"
        );
    }
}

/// L'ordre de la section est celui de la portée, et il se compare.
///
/// Le rendre comparable évite qu'un appelant réénumère les sept pour poser une question à laquelle
/// la liste répond déjà — et une réénumération ailleurs finirait par diverger de celle-ci.
#[test]
fn the_order_of_the_section_is_the_order_of_scope() {
    assert!(Level::Disciplinary.is_at_least_as_wide_as(Level::AgentPrivate));
    assert!(Level::Program.is_at_least_as_wide_as(Level::Team));
    assert!(!Level::Team.is_at_least_as_wide_as(Level::Program));
    assert!(
        Level::Team.is_at_least_as_wide_as(Level::Team),
        "« au moins aussi large » inclut l'égalité"
    );

    let mut sorted = Level::ALL;
    sorted.sort_unstable();
    assert_eq!(
        sorted,
        Level::ALL,
        "la liste est déjà dans l'ordre de portée"
    );
}

// ---------------------------------------------------------------------------------------------
// 2. Canonique ou projection — la frontière du dernier paragraphe
// ---------------------------------------------------------------------------------------------

/// « Le graphe, les événements et les artefacts sont canoniques. Les résumés et embeddings sont des
/// projections régénérables. »
///
/// Perdre une projection coûte un recalcul ; perdre un canonique coûte la vérité institutionnelle
/// (invariant 2). Les deux ne se lisent donc pas pareil, et le type le porte.
#[test]
fn what_is_canonical_is_never_regenerable() {
    assert!(!Substance::Canonical.is_regenerable());
    assert!(Substance::Projection.is_regenerable());
    assert_ne!(Substance::Canonical.slug(), Substance::Projection.slug());
}

/// Une purge de projections doit dire exactement ce qu'elle coûte.
///
/// C'est la question que §9.1 pose de l'autre côté — « les vecteurs et graph databases sont des
/// projections reconstructibles ». Y répondre depuis la mémoire évite qu'un opérateur ait à deviner.
#[test]
fn a_shelf_says_what_a_purge_would_cost() {
    let mut shelf = Shelf::new();
    shelf
        .store("evt-1", Level::Branch, Substance::Canonical)
        .expect("un événement");
    shelf
        .store("art-1", Level::Program, Substance::Canonical)
        .expect("un artefact");
    shelf
        .store("embedding-1", Level::Team, Substance::Projection)
        .expect("un embedding");
    shelf
        .store("summary-1", Level::AgentPrivate, Substance::Projection)
        .expect("un résumé");

    let regenerable: Vec<&str> = shelf.regenerable().map(locus_memory::Entry::key).collect();
    let irreplaceable: Vec<&str> = shelf
        .irreplaceable()
        .map(locus_memory::Entry::key)
        .collect();

    assert_eq!(regenerable, ["embedding-1", "summary-1"]);
    assert_eq!(irreplaceable, ["art-1", "evt-1"]);
    assert!(
        regenerable.iter().all(|key| !irreplaceable.contains(key)),
        "les deux listes ne se recoupent jamais"
    );
}

#[test]
fn a_level_keeps_only_what_was_stored_in_it() {
    let mut shelf = Shelf::new();
    shelf
        .store("a", Level::Team, Substance::Projection)
        .expect("rangée");
    shelf
        .store("b", Level::Program, Substance::Projection)
        .expect("rangée");

    let team: Vec<&str> = shelf
        .at(Level::Team)
        .map(locus_memory::Entry::key)
        .collect();
    assert_eq!(team, ["a"]);
    assert_eq!(shelf.at(Level::Disciplinary).count(), 0);
    assert_eq!(
        shelf.get("a").map(locus_memory::Entry::level),
        Some(Level::Team)
    );
}

// ---------------------------------------------------------------------------------------------
// 3. Ce qui ne se retrouve pas n'est pas rangé
// ---------------------------------------------------------------------------------------------

#[test]
fn an_entry_without_a_key_is_refused() {
    let mut shelf = Shelf::new();
    assert_eq!(
        shelf
            .store("  ", Level::Team, Substance::Projection)
            .expect_err("sans clé"),
        MemoryError::EmptyKey
    );
}

/// Écraser en silence ferait disparaître un canonique derrière une projection du même nom.
///
/// C'est la forme que prend la perte de la vérité institutionnelle : rien n'échoue, et la source
/// est devenue son propre résumé.
#[test]
fn storing_over_an_existing_key_is_refused() {
    let mut shelf = Shelf::new();
    shelf
        .store("claim-1", Level::Branch, Substance::Canonical)
        .expect("rangée");
    assert_eq!(
        shelf
            .store("claim-1", Level::Team, Substance::Projection)
            .expect_err("la clé est prise"),
        MemoryError::AlreadyStored {
            key: "claim-1".to_owned(),
            level: Level::Branch,
        }
    );
    assert_eq!(
        shelf.get("claim-1").map(locus_memory::Entry::substance),
        Some(Substance::Canonical),
        "et le canonique est toujours là"
    );
}

/// §16.6 n'est pas ici, et c'est délibéré.
///
/// Les cinq préventions de contamination vivent dans `packages/review` depuis W7.b, écrites par cas
/// adverses. Deux listes de cinq divergeraient, et la seconde aurait l'air aussi vraie que la
/// première. Ce test tient l'absence en nommant ce qu'on serait tenté de recopier.
#[test]
fn the_five_preventions_are_not_duplicated_here() {
    for elsewhere in [
        "generator_reasoning_leaked",
        "refuted_claim_propagated",
        "circular_consensus",
    ] {
        assert_eq!(
            Level::parse(elsewhere),
            None,
            "« {elsewhere} » vit dans packages/review, pas ici"
        );
    }
}
