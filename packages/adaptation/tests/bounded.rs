//! Test de sortie de W18.c — **les trois garanties de l'item.**
//!
//! 1. La classe de risque ne se déclare pas : elle se calcule de `region::threatens`, et un
//!    proposeur n'a nulle part où l'écrire.
//! 2. En `bounded`, une opération dont la classe dépasse le plafond est refusée **en nommant
//!    l'invariant**, pas le plafond.
//! 3. `operator` n'est jamais tenu par un agent, et `Author::Agent` n'a pas de chemin vers lui.

use std::fmt::Write as _;

use locus_adaptation::{Ceiling, Denial, Operator, OperatorError, RiskClass, autonomously};
use locus_coordination::{
    ApprovalMode, Author, CoordinationMode, Diff, Digest, Invariant, Mode, Operation, Refusal,
    Region, Relation, RelationKind, Verdict, Version,
};
use locus_domain::ContentHash;
use locus_policy::{Outcome, Verb};
use locus_protocol::{Id, IdKind, Timestamp, id::Agent};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

/// Le source sans ses commentaires — ce que le module **fait**, pas ce qu'il explique.
fn code_of(source: &str) -> String {
    source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn agent(seed: u8) -> Id<Agent> {
    id::<Agent>(seed)
}

fn reviews(from: Id<Agent>, to: Id<Agent>) -> Relation {
    Relation {
        from,
        to,
        kind: RelationKind::Review,
    }
}

fn sees(from: Id<Agent>, to: Id<Agent>) -> Relation {
    Relation {
        from,
        to,
        kind: RelationKind::Visibility,
    }
}

const PRIME: u64 = 0x0000_0100_0000_01b3;
const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

struct Fnv;

impl Digest for Fnv {
    fn digest(&self, canonical: &str) -> ContentHash {
        let mut digest = String::with_capacity(64);
        for salt in 0_u64..4 {
            let mut hash = OFFSET ^ salt.wrapping_mul(PRIME);
            for byte in canonical.bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(PRIME);
            }
            write!(digest, "{hash:016x}").expect("écrire dans une String n'échoue pas");
        }
        ContentHash::parse(&format!("sha256:{digest}")).expect("64 hexadécimaux minuscules")
    }
}

fn base() -> Version {
    Version::root(
        &[agent(1), agent(2), agent(3)],
        &[],
        CoordinationMode::Blackboard,
        None,
        &Fnv,
    )
    .expect("la fixture est acyclique")
}

fn diff_of(base: &Version, operations: Vec<Operation>) -> Diff {
    Diff::declaring(base, operations, &Fnv).expect("les opérations s'appliquent")
}

/// Une région qui n'exige ni humain ni ombre — celle où `bounded` a quelque chose à dire.
fn autonomous_region() -> Region {
    Region::declare(
        "coeur",
        &[agent(1), agent(2), agent(3)],
        &Operation::NAMES,
        1,
        8,
        8,
        ApprovalMode::Peer,
        false,
    )
    .expect("région valide")
}

fn allowed() -> Outcome {
    Outcome::Decided {
        verb: Verb::Allow,
        by: "coordination/allow-in-core".to_owned(),
    }
}

fn verdict(region: &Region, base: &Version, diff: &Diff) -> Verdict {
    region.admits(base, diff, &Fnv).expect("le diff s'applique")
}

// ---------------------------------------------------------------------------------------------
// 1. La classe de risque est dérivée, jamais déclarée
// ---------------------------------------------------------------------------------------------

/// Un proposeur n'a nulle part où écrire sa classe de risque.
///
/// Le test lit le **code**, commentaires retirés : expliquer pourquoi un constructeur est absent le
/// fait apparaître dans la prose, et un test qui confondrait les deux interdirait d'écrire la
/// raison.
#[test]
fn a_risk_class_cannot_be_declared() {
    let source = code_of(include_str!("../src/bounded.rs"));
    for forbidden in [
        "RiskClass::new",
        "fn declaring",
        "impl From<",
        "pub threatened",
        "fn set_class",
    ] {
        assert!(
            !source.contains(forbidden),
            "« {forbidden} » laisserait un proposeur choisir son propre plafond"
        );
    }
    // Un seul site construit la structure, et c'est `of`.
    let constructions = source
        .match_indices("RiskClass {")
        .filter(|(offset, _)| {
            !source[..*offset].ends_with("pub struct ") && !source[..*offset].ends_with("impl ")
        })
        .count();
    assert_eq!(constructions, 1);
}

/// La classe se calcule, et elle est l'**union** des menaces du lot.
///
/// Un maximum en cacherait une : deux opérations qui menacent chacune un invariant différent en
/// menacent deux ensemble.
#[test]
fn the_risk_class_is_the_union_of_what_the_batch_threatens() {
    let base = base();

    // Une arête de visibilité ne menace rien.
    let harmless = diff_of(&base, vec![Operation::AddEdge(sees(agent(1), agent(2)))]);
    assert!(RiskClass::of(&harmless).threatens_nothing());

    // Une arête de revue menace l'acyclicité.
    let risky = diff_of(&base, vec![Operation::AddEdge(reviews(agent(1), agent(2)))]);
    assert_eq!(
        RiskClass::of(&risky).threatened(),
        &[Invariant::ReviewAcyclicity].into_iter().collect()
    );

    // Les deux ensemble menacent ce que la seconde menace, et rien de moins.
    let both = diff_of(
        &base,
        vec![
            Operation::AddEdge(sees(agent(1), agent(2))),
            Operation::AddEdge(reviews(agent(2), agent(3))),
        ],
    );
    assert_eq!(
        RiskClass::of(&both).threatened(),
        &[Invariant::ReviewAcyclicity].into_iter().collect()
    );
}

/// Un lot vide ne menace rien, et « ne menace rien » n'est pas « est vide ».
#[test]
fn threatening_nothing_is_not_being_empty() {
    let base = base();
    let harmless = diff_of(&base, vec![Operation::AddNode(agent(9))]);
    assert!(!harmless.is_empty());
    assert!(RiskClass::of(&harmless).threatens_nothing());
}

// ---------------------------------------------------------------------------------------------
// 2. Le refus nomme l'invariant
// ---------------------------------------------------------------------------------------------

/// **La garantie centrale.** Le plafond dépassé est refusé en nommant l'invariant, pas un compte.
#[test]
fn exceeding_the_ceiling_names_the_invariant() {
    let base = base();
    let region = autonomous_region();
    let diff = diff_of(&base, vec![Operation::AddEdge(reviews(agent(1), agent(2)))]);

    let denied = autonomously(
        Mode::Bounded,
        &allowed(),
        &verdict(&region, &base, &diff),
        &diff,
        &Ceiling::untouchable(),
    )
    .expect_err("le plafond le plus bas ne tolère rien");

    assert_eq!(
        denied,
        Denial::ThreatensInvariant {
            invariant: Invariant::ReviewAcyclicity,
        }
    );
    // Le message porte le nom, jamais un chiffre.
    let said = denied.to_string();
    assert!(said.contains("review-acyclicity"), "{said}");
    assert!(!said.contains('0') && !said.contains('1'), "{said}");
}

/// Le même lot passe quand le plafond nomme l'invariant qu'il menace.
#[test]
fn a_ceiling_that_names_the_invariant_admits_the_batch() {
    let base = base();
    let region = autonomous_region();
    let diff = diff_of(&base, vec![Operation::AddEdge(reviews(agent(1), agent(2)))]);

    let autonomy = autonomously(
        Mode::Bounded,
        &allowed(),
        &verdict(&region, &base, &diff),
        &diff,
        &Ceiling::tolerating(&[Invariant::ReviewAcyclicity]),
    )
    .expect("le plafond tolère exactement ce que le lot menace");

    assert_eq!(autonomy.region(), "coeur");
    assert_eq!(autonomy.by(), "coordination/allow-in-core");
    assert_eq!(
        autonomy.class().threatened(),
        &[Invariant::ReviewAcyclicity].into_iter().collect()
    );
}

/// Et un lot qui ne menace rien passe sous le plafond le plus bas.
#[test]
fn a_harmless_batch_passes_under_the_lowest_ceiling() {
    let base = base();
    let region = autonomous_region();
    let diff = diff_of(&base, vec![Operation::AddEdge(sees(agent(1), agent(2)))]);

    let autonomy = autonomously(
        Mode::Bounded,
        &allowed(),
        &verdict(&region, &base, &diff),
        &diff,
        &Ceiling::untouchable(),
    )
    .expect("rien de menacé, rien à tolérer");
    assert!(autonomy.class().threatens_nothing());
}

/// Hors de `bounded`, il n'y a pas d'autonomie — et le refus dit quel mode est en vigueur.
#[test]
fn no_other_mode_dispenses_with_approval() {
    let base = base();
    let region = autonomous_region();
    let diff = diff_of(&base, vec![Operation::AddEdge(sees(agent(1), agent(2)))]);
    let verdict = verdict(&region, &base, &diff);

    for mode in [Mode::Observed, Mode::Assisted, Mode::Operator] {
        let denied = autonomously(mode, &allowed(), &verdict, &diff, &Ceiling::untouchable())
            .expect_err("seul `bounded` dispense de l'approbation");
        assert_eq!(denied, Denial::NotBounded { mode });
    }
    assert!(Mode::Bounded.dispenses_with_approval());
}

/// Sans `allow` du moteur, rien ne passe — et le silence tombe avec le reste.
#[test]
fn nothing_but_allow_opens_the_bounded_path() {
    let base = base();
    let region = autonomous_region();
    let diff = diff_of(&base, vec![Operation::AddEdge(sees(agent(1), agent(2)))]);
    let verdict = verdict(&region, &base, &diff);

    let refusals = [
        Outcome::NoRule,
        Outcome::Conflict {
            priority: 10,
            rules: vec!["a".to_owned(), "b".to_owned()],
        },
        Outcome::Decided {
            verb: Verb::Deny,
            by: "r".to_owned(),
        },
        Outcome::Decided {
            verb: Verb::RequireApproval {
                approver_role: "pi".to_owned(),
            },
            by: "r".to_owned(),
        },
        Outcome::Decided {
            verb: Verb::Modify {
                constraint: "c".to_owned(),
            },
            by: "r".to_owned(),
        },
    ];
    for outcome in &refusals {
        let denied = autonomously(
            Mode::Bounded,
            outcome,
            &verdict,
            &diff,
            &Ceiling::untouchable(),
        )
        .expect_err("rien d'autre qu'`allow` n'est une autorisation");
        assert_eq!(denied, Denial::PolicyDidNotAllow);
    }
}

/// Une région qui exige un humain garde son humain : un mode ne surclasse pas un périmètre.
#[test]
fn a_region_demanding_a_human_keeps_its_human() {
    let base = base();
    let region = Region::declare(
        "sensible",
        &[agent(1), agent(2), agent(3)],
        &Operation::NAMES,
        1,
        8,
        8,
        ApprovalMode::Human,
        false,
    )
    .expect("région valide");
    let diff = diff_of(&base, vec![Operation::AddEdge(sees(agent(1), agent(2)))]);

    let denied = autonomously(
        Mode::Bounded,
        &allowed(),
        &verdict(&region, &base, &diff),
        &diff,
        &Ceiling::untouchable(),
    )
    .expect_err("`human` n'est pas levé par un mode");
    assert_eq!(
        denied,
        Denial::HumanApprovalRequired {
            region: "sensible".to_owned(),
        }
    );
}

/// Une région qui exige une ombre la garde aussi.
#[test]
fn a_region_demanding_a_shadow_keeps_its_shadow() {
    let base = base();
    let region = Region::declare(
        "ombre",
        &[agent(1), agent(2), agent(3)],
        &Operation::NAMES,
        1,
        8,
        8,
        ApprovalMode::Peer,
        true,
    )
    .expect("région valide");
    let diff = diff_of(&base, vec![Operation::AddEdge(sees(agent(1), agent(2)))]);

    let denied = autonomously(
        Mode::Bounded,
        &allowed(),
        &verdict(&region, &base, &diff),
        &diff,
        &Ceiling::untouchable(),
    )
    .expect_err("`require_shadow` n'est pas levé par un mode");
    assert_eq!(
        denied,
        Denial::ShadowRequired {
            region: "ombre".to_owned(),
        }
    );
}

/// Un lot vetoé globalement ne devient pas autonome non plus, et le refus nomme l'invariant rompu.
///
/// Le veto de W15.c est distinct du plafond de ce module : là, un invariant est **rompu** par un
/// chemin passant hors de la région ; ici, il est seulement **menacé** et le plafond ne le tolère
/// pas. Les confondre ferait chercher un plafond mal réglé quand c'est la cohérence globale qui a
/// mordu.
#[test]
fn a_vetoed_batch_does_not_become_autonomous() {
    let base = Version::root(
        &[agent(1), agent(2), agent(3), agent(4)],
        &[reviews(agent(2), agent(4)), reviews(agent(4), agent(1))],
        CoordinationMode::Blackboard,
        None,
        &Fnv,
    )
    .expect("la fixture est acyclique");
    let region = autonomous_region();
    // `1 → 2` referme `1 → 2 → 4 → 1`, par `agent(4)` qui est hors de la région.
    let diff = diff_of(&base, vec![Operation::AddEdge(reviews(agent(1), agent(2)))]);

    let denied = autonomously(
        Mode::Bounded,
        &allowed(),
        &verdict(&region, &base, &diff),
        &diff,
        &Ceiling::tolerating(&[Invariant::ReviewAcyclicity]),
    )
    .expect_err("un veto global n'est pas rattrapé par un plafond large");
    assert_eq!(
        denied,
        Denial::RegionVetoed {
            invariant: Invariant::ReviewAcyclicity,
        }
    );
}

/// Un lot que la région refuse ne devient pas autonome — et le refus dit quelle borne a mordu.
#[test]
fn a_batch_the_region_refused_does_not_become_autonomous() {
    let base = base();
    let region = Region::declare(
        "gel",
        &[agent(1), agent(2), agent(3)],
        &["REMOVE_EDGE"],
        1,
        8,
        8,
        ApprovalMode::Peer,
        false,
    )
    .expect("région valide");
    let diff = diff_of(&base, vec![Operation::AddNode(agent(9))]);

    let denied = autonomously(
        Mode::Bounded,
        &allowed(),
        &verdict(&region, &base, &diff),
        &diff,
        &Ceiling::untouchable(),
    )
    .expect_err("la région a déjà refusé");
    let Denial::RegionRefused(refusal) = denied else {
        panic!("le refus de la région doit être transporté tel quel");
    };
    assert_eq!(refusal.bound(), "allowed_ops");
    assert!(matches!(refusal, Refusal::OperationNotAllowed { .. }));
}

// ---------------------------------------------------------------------------------------------
// 3. `operator` n'est jamais tenu par un agent
// ---------------------------------------------------------------------------------------------

/// Un humain nommé prend la main ; un agent, jamais.
#[test]
fn operator_is_held_by_a_named_human_never_an_agent() {
    let operator = Operator::taking(&Author::Human("usr-marie".to_owned()), Mode::Operator)
        .expect("un humain nommé prend la main");
    assert_eq!(operator.principal(), "usr-marie");

    let refused = Operator::taking(&Author::Agent(agent(7)), Mode::Operator)
        .expect_err("un agent ne prend jamais la main");
    assert!(matches!(refused, OperatorError::NotAHuman { .. }));
    assert!(refused.to_string().contains("jamais un agent"));
}

/// Et sous aucun mode un agent n'y arrive : le refus n'est pas une particularité d'`operator`.
#[test]
fn no_mode_lets_an_agent_take_the_operator_seat() {
    for mode in Mode::ALL {
        assert!(Operator::taking(&Author::Agent(agent(7)), mode).is_err());
    }
}

/// Un humain non plus, hors d'un déploiement en `operator` — et le refus dit lequel des deux manque.
///
/// Les deux refus sont distincts parce qu'ils se corrigent différemment : le mode se change, ce qui
/// est « un acte gouverné et journalisé » ; être un agent ne se corrige pas.
#[test]
fn a_human_outside_operator_mode_does_not_take_the_seat() {
    for mode in [Mode::Observed, Mode::Assisted, Mode::Bounded] {
        let refused = Operator::taking(&Author::Human("usr-marie".to_owned()), mode)
            .expect_err("les opérations privilégiées demandent `operator`");
        assert_eq!(refused, OperatorError::NotOperatorMode { mode });
    }
}

/// `operator` ne laisse pas non plus un agent **proposer** — c'est le plus privilégié et le plus
/// fermé.
#[test]
fn operator_is_the_most_privileged_and_the_most_closed() {
    let robot = Author::Agent(agent(7));
    let human = Author::Human("usr-marie".to_owned());

    assert!(!Mode::Observed.allows(&robot));
    assert!(Mode::Assisted.allows(&robot));
    assert!(Mode::Bounded.allows(&robot));
    assert!(!Mode::Operator.allows(&robot));

    // Un humain propose sous tout mode : le mode borne les agents, pas l'institution.
    for mode in Mode::ALL {
        assert!(mode.allows(&human));
    }
}

/// Les quatre modes ne forment **pas** une échelle, et rien ne permet de les classer.
///
/// C'est l'échelle d'autorité à barreaux que `CLAUDE.md` interdit nommément. `operator` en est la
/// réfutation : le plus privilégié et celui qui permet à un agent le moins.
#[test]
fn the_four_modes_are_not_a_ladder() {
    let source = code_of(include_str!("../../coordination/src/proposal.rs"));
    let start = source
        .find("pub enum Mode {")
        .expect("l'énumération existe");
    let derive = &source[start.saturating_sub(200)..start];
    for forbidden in ["PartialOrd", "Ord"] {
        assert!(
            !derive.contains(forbidden),
            "« {forbidden} » ferait des quatre modes une échelle"
        );
    }
    for forbidden in ["fn level", "fn rank", "fn is_at_least", "fn stronger"] {
        assert!(
            !source.contains(forbidden),
            "« {forbidden} » ferait des quatre modes une échelle"
        );
    }
    assert_eq!(Mode::ALL.len(), 4);
    let named: Vec<&str> = Mode::ALL.iter().map(|mode| mode.slug()).collect();
    assert_eq!(
        named,
        vec!["observed", "assisted", "bounded", "operator"],
        "l'ordre est celui du tableau de l'ADR, et il n'est pas un rang"
    );
}

/// Rien dans ce module ne produit une approbation.
///
/// `bounded` **retire** l'approbation, il ne la confie pas au proposeur. Un `Approved` fabriqué au
/// nom d'un agent mettrait un nom sur un jugement que personne n'a porté.
#[test]
fn nothing_here_produces_an_approval() {
    let source = code_of(include_str!("../src/bounded.rs"));
    // `ApprovalMode` et `HumanApprovalRequired` portent le mot et ne sont pas visés : lire le mode
    // d'approbation qu'une région **déclare** n'est pas produire une approbation. Ce qui est
    // interdit est la valeur `Approved`, et le verbe qui la fabrique.
    for absent in ["Approved", "fn approve"] {
        assert!(
            !source.contains(absent),
            "« {absent} » mettrait un nom sur un jugement que personne n'a porté"
        );
    }
}
