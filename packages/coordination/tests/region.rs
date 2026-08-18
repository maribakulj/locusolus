//! Test de sortie de W15.c — **les trois garanties de l'item.**
//!
//! 1. Une opération hors de la région déclarée ou hors de `allowed_ops` est refusée, et le refus
//!    nomme laquelle des bornes a mordu.
//! 2. Un lot accepté localement mais qui casse un invariant global est vetoé, et le veto nomme
//!    l'invariant — et les agents pris dedans.
//! 3. L'acceptation locale seule ne commit jamais.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use locus_coordination::{
    ApprovalMode, Diff, Digest, Invariant, Operation, Refusal, Region, RegionError, Relation,
    RelationKind, Verdict, Version, threatens,
};
use locus_domain::ContentHash;
use locus_protocol::{Id, IdKind, Timestamp, id::Agent};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

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

/// L'organisation du module : `1 → 2 → 4 → 1` **n'existe pas encore**, mais `2 → 4` et `4 → 1` oui.
///
/// `agent(4)` est **hors** de toute région déclarée ici : c'est lui qui rend le veto global
/// nécessaire, puisqu'un critère local ne le regarde pas.
fn base() -> Version {
    Version::root(
        &[agent(1), agent(2), agent(3), agent(4)],
        &[reviews(agent(2), agent(4)), reviews(agent(4), agent(1))],
        &Fnv,
    )
    .expect("la fixture est acyclique")
}

/// Une région sur `1`, `2` et `3`, large sur toutes ses bornes.
fn permissive() -> Region {
    Region::declare(
        "coeur",
        &[agent(1), agent(2), agent(3)],
        &Operation::NAMES,
        1,
        8,
        8,
        ApprovalMode::Human,
        true,
    )
    .expect("région valide")
}

fn diff_of(base: &Version, operations: Vec<Operation>) -> Diff {
    Diff::declaring(base, operations, &Fnv).expect("les opérations s'appliquent")
}

// ---------------------------------------------------------------------------------------------
// 1. Les quatre bornes qui interdisent, chacune nommée
// ---------------------------------------------------------------------------------------------

#[test]
fn an_operation_outside_the_declared_region_is_refused() {
    let base = base();
    // `agent(4)` n'est pas dans la région.
    let diff = diff_of(
        &base,
        vec![Operation::RemoveEdge(reviews(agent(4), agent(1)))],
    );
    let Verdict::Refused(refusal) = permissive()
        .admits(&base, &diff, &Fnv)
        .expect("le diff s'applique")
    else {
        panic!("une opération hors région doit être refusée");
    };
    assert_eq!(refusal.bound(), "region");
    assert!(matches!(refusal, Refusal::OutOfRegion { .. }));
}

#[test]
fn an_operation_absent_from_allowed_ops_is_refused() {
    let base = base();
    let region = Region::declare(
        "gel",
        &[agent(1), agent(2), agent(3)],
        &["REMOVE_EDGE"],
        1,
        8,
        8,
        ApprovalMode::Human,
        false,
    )
    .expect("région valide");
    let diff = diff_of(&base, vec![Operation::AddNode(agent(9))]);

    let Verdict::Refused(refusal) = region
        .admits(&base, &diff, &Fnv)
        .expect("le diff s'applique")
    else {
        panic!("une opération hors `allowed_ops` doit être refusée");
    };
    assert_eq!(refusal.bound(), "allowed_ops");
}

/// Le risque est **dérivé**, pas déclaré : il compte les invariants qu'une opération peut menacer.
///
/// Une région à `risk_ceiling` nul est donc une région où rien ne peut menacer l'acyclicité — et
/// c'est vérifiable plutôt que promis. Un risque que le proposeur aurait déclaré serait une
/// auto-évaluation sous plafond, c'est-à-dire une borne qu'on contourne.
#[test]
fn an_operation_over_the_risk_ceiling_is_refused() {
    let base = base();
    let prudent = Region::declare(
        "prudente",
        &[agent(1), agent(2), agent(3)],
        &Operation::NAMES,
        0,
        8,
        8,
        ApprovalMode::Human,
        false,
    )
    .expect("région valide");

    let adds_an_edge = diff_of(&base, vec![Operation::AddEdge(reviews(agent(1), agent(3)))]);
    let Verdict::Refused(refusal) = prudent
        .admits(&base, &adds_an_edge, &Fnv)
        .expect("le diff s'applique")
    else {
        panic!("ajouter une arête peut fermer un cycle");
    };
    assert_eq!(refusal.bound(), "risk_ceiling");

    // Retirer ne crée aucun chemin, donc ne menace rien, donc passe sous le même plafond nul.
    let removes_an_edge = diff_of(
        &base,
        vec![Operation::RemoveEdge(reviews(agent(2), agent(4)))],
    );
    let region_with_four = Region::declare(
        "prudente-large",
        &[agent(1), agent(2), agent(3), agent(4)],
        &Operation::NAMES,
        0,
        8,
        8,
        ApprovalMode::Human,
        false,
    )
    .expect("région valide");
    assert!(
        region_with_four
            .admits(&base, &removes_an_edge, &Fnv)
            .expect("le diff s'applique")
            .is_admissible()
    );
}

/// « Delta » se mesure en différence symétrique, pas en solde.
///
/// Ajouter deux agents et en retirer deux laisse un solde net nul tout en changeant quatre
/// identités. Un plafond sur le solde laisserait passer ce lot ; un plafond sur ce qui a changé le
/// refuse, et c'est bien le rayon d'explosion que GRAFT veut borner.
#[test]
fn a_net_balance_would_not_have_bounded_anything() {
    let isolated = Version::root(&[agent(1), agent(2), agent(3)], &[], &Fnv).expect("sans arête");
    let narrow = Region::declare(
        "étroite",
        &[agent(1), agent(2), agent(3), agent(8), agent(9)],
        &Operation::NAMES,
        1,
        2,
        8,
        ApprovalMode::Human,
        false,
    )
    .expect("région valide");

    // Deux entrent, deux sortent : le solde net est **nul**, et quatre identités ont changé.
    let churn = diff_of(
        &isolated,
        vec![
            Operation::AddNode(agent(8)),
            Operation::AddNode(agent(9)),
            Operation::RemoveNode(agent(1)),
            Operation::RemoveNode(agent(2)),
        ],
    );
    let produced = churn.replay(&isolated, &Fnv).expect("rejeu");
    assert_eq!(
        produced.members().len(),
        isolated.members().len(),
        "un plafond sur le solde n'aurait rien vu : il est nul"
    );

    let Verdict::Refused(refusal) = narrow
        .admits(&isolated, &churn, &Fnv)
        .expect("le diff s'applique")
    else {
        panic!("quatre identités ont changé sous un plafond de deux");
    };
    assert_eq!(refusal.bound(), "max_nodes_delta");
}

/// Le même piège du côté des arêtes : deux entrent, deux sortent, le solde est nul.
#[test]
fn an_edge_churn_with_a_null_balance_is_still_four_changes() {
    let base = Version::root(
        &[agent(1), agent(2), agent(3), agent(4)],
        &[reviews(agent(1), agent(2)), reviews(agent(3), agent(4))],
        &Fnv,
    )
    .expect("acyclique");
    let narrow = Region::declare(
        "étroite",
        &[agent(1), agent(2), agent(3), agent(4)],
        &Operation::NAMES,
        1,
        8,
        2,
        ApprovalMode::Human,
        false,
    )
    .expect("région valide");

    let churn = diff_of(
        &base,
        vec![
            Operation::AddEdge(reviews(agent(2), agent(3))),
            Operation::AddEdge(reviews(agent(1), agent(4))),
            Operation::RemoveEdge(reviews(agent(1), agent(2))),
            Operation::RemoveEdge(reviews(agent(3), agent(4))),
        ],
    );
    let produced = churn.replay(&base, &Fnv).expect("rejeu");
    assert_eq!(
        produced.relations().len(),
        base.relations().len(),
        "un plafond sur le solde n'aurait rien vu"
    );

    let Verdict::Refused(refusal) = narrow
        .admits(&base, &churn, &Fnv)
        .expect("le diff s'applique")
    else {
        panic!("quatre arêtes ont changé sous un plafond de deux");
    };
    assert_eq!(refusal.bound(), "max_edges_delta");
}

/// Aucune opération ne fait entrer une identité **hors** de la région.
///
/// Les trois cas se ressemblent et se ratent séparément : une arête dont seule la **cible** est
/// dehors, un remplacement dont seul le **remplaçant** est dehors, une fusion dont seule
/// l'**identité produite** est dehors. Vérifier la source d'une arête sans vérifier sa cible
/// laisserait une région recâbler vers n'importe qui ; ne vérifier que le sortant d'un remplacement
/// laisserait une région faire entrer qui elle veut.
#[test]
fn no_operation_smuggles_an_identity_outside_the_region() {
    let base = base();
    let region = permissive(); // agent(4) et agent(9) sont dehors.
    let cases = [
        (
            "la cible d'une arête",
            Operation::AddEdge(reviews(agent(3), agent(4))),
        ),
        (
            "le remplaçant",
            Operation::ReplaceNode {
                from: agent(3),
                to: agent(9),
            },
        ),
        (
            "l'identité produite par une fusion",
            Operation::MergeNodes {
                first: agent(1),
                second: agent(3),
                into: agent(9),
            },
        ),
    ];

    for (what, operation) in cases {
        let diff = diff_of(&base, vec![operation]);
        let verdict = region
            .admits(&base, &diff, &Fnv)
            .expect("le diff s'applique");
        let Verdict::Refused(refusal) = verdict else {
            panic!("{what} est hors région et doit être refusé : {verdict:?}");
        };
        assert_eq!(refusal.bound(), "region", "{what}");
    }
}

#[test]
fn too_many_edges_changing_is_refused() {
    let base = base();
    let narrow = Region::declare(
        "étroite",
        &[agent(1), agent(2), agent(3)],
        &Operation::NAMES,
        1,
        8,
        1,
        ApprovalMode::Human,
        false,
    )
    .expect("région valide");
    let diff = diff_of(
        &base,
        vec![
            Operation::AddEdge(reviews(agent(1), agent(3))),
            Operation::AddEdge(reviews(agent(3), agent(2))),
        ],
    );

    let Verdict::Refused(refusal) = narrow
        .admits(&base, &diff, &Fnv)
        .expect("le diff s'applique")
    else {
        panic!("deux arêtes changent sous un plafond d'une");
    };
    assert_eq!(refusal.bound(), "max_edges_delta");
}

/// Les quatre bornes qui interdisent portent chacune son nom, et elles sont **quatre**.
///
/// `approval_mode` et `require_shadow` ne sont pas dans cette liste : elles exigent, elles
/// n'interdisent pas. Leur donner un refus ferait attendre un blocage qui ne vient jamais.
#[test]
fn four_bounds_refuse_and_two_oblige() {
    let named: BTreeSet<&str> = [
        Refusal::OutOfRegion {
            region: String::new(),
            operation: String::new(),
            node: String::new(),
        },
        Refusal::OperationNotAllowed {
            region: String::new(),
            operation: String::new(),
        },
        Refusal::OverRisk {
            region: String::new(),
            operation: String::new(),
            risk: 0,
            ceiling: 0,
        },
        Refusal::TooManyNodes {
            region: String::new(),
            changed: 0,
            ceiling: 0,
        },
        Refusal::TooManyEdges {
            region: String::new(),
            changed: 0,
            ceiling: 0,
        },
    ]
    .iter()
    .map(Refusal::bound)
    .collect();

    for obligation in ["approval_mode", "require_shadow"] {
        assert!(
            !named.contains(obligation),
            "{obligation} exige, elle n'interdit pas"
        );
    }
    for prohibition in [
        "region",
        "allowed_ops",
        "risk_ceiling",
        "max_nodes_delta",
        "max_edges_delta",
    ] {
        assert!(
            named.contains(prohibition),
            "{prohibition} doit pouvoir mordre"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 2. Le veto global voit ce que la région ne peut pas voir
// ---------------------------------------------------------------------------------------------

/// **Le test qui justifie le dispositif entier.**
///
/// La région contient `1`, `2` et `3` ; l'organisation porte déjà `2 → 4 → 1` avec `agent(4)`
/// **dehors**. Ajouter `1 → 2` ne touche que des nœuds de la région, donc le critère local
/// l'accepte — et le cycle `1 → 2 → 4 → 1` vient de se fermer. Trois agents qui se relisent en rond
/// ne sont relus par personne.
///
/// Un veto qui ne regarderait que la région serait un critère local écrit deux fois.
#[test]
fn a_locally_accepted_batch_can_still_be_vetoed_by_a_path_outside_the_region() {
    let base = base();
    let diff = diff_of(&base, vec![Operation::AddEdge(reviews(agent(1), agent(2)))]);

    let Verdict::Vetoed {
        accepted,
        invariant,
        witness,
    } = permissive()
        .admits(&base, &diff, &Fnv)
        .expect("le diff s'applique")
    else {
        panic!("le cycle passe par agent(4), que la région ne contient pas");
    };

    assert_eq!(accepted.region(), "coeur", "la région, elle, avait accepté");
    assert_eq!(invariant, Invariant::ReviewAcyclicity);
    let cycle: BTreeSet<String> = witness.into_iter().collect();
    assert_eq!(
        cycle,
        [agent(1), agent(2), agent(4)]
            .iter()
            .map(ToString::to_string)
            .collect::<BTreeSet<String>>(),
        "le veto nomme les agents pris dans le cycle, sinon il faut le chercher à la main"
    );
}

/// Le veto ne mord pas quand rien n'est rompu — sinon on apprend à l'ignorer.
#[test]
fn a_coherent_batch_is_admissible() {
    let base = base();
    let diff = diff_of(&base, vec![Operation::AddEdge(reviews(agent(3), agent(1)))]);
    assert!(
        permissive()
            .admits(&base, &diff, &Fnv)
            .expect("le diff s'applique")
            .is_admissible()
    );
}

/// Ce que chaque opération **peut** menacer, une par une.
///
/// Retirer ne crée aucun chemin ; remplacer est un isomorphisme ; scinder ne fait que répartir des
/// arêtes existantes. Seuls ajouter une arête et fusionner deux nœuds rapprochent deux extrémités.
#[test]
fn only_two_operations_can_close_a_cycle() {
    let threatening = [
        Operation::AddEdge(reviews(agent(1), agent(2))),
        Operation::MergeNodes {
            first: agent(1),
            second: agent(2),
            into: agent(9),
        },
    ];
    let harmless = [
        Operation::AddNode(agent(9)),
        Operation::RemoveNode(agent(3)),
        Operation::RemoveEdge(reviews(agent(2), agent(4))),
        Operation::ReplaceNode {
            from: agent(3),
            to: agent(9),
        },
        Operation::SplitNode {
            node: agent(2),
            into: (agent(8), agent(9)),
            follows_first: BTreeSet::new(),
        },
    ];

    for operation in &threatening {
        assert!(
            threatens(operation).contains(&Invariant::ReviewAcyclicity),
            "{operation} rapproche deux extrémités"
        );
    }
    for operation in &harmless {
        assert!(
            threatens(operation).is_empty(),
            "{operation} ne crée aucun chemin de revue"
        );
    }
}

/// La fusion ferme un cycle sans qu'aucune arête n'ait été ajoutée.
///
/// `1 → 2` et `2 → 4` ; fusionner `1` et `4` donne `fusionné → 2 → fusionné`. C'est pour cela que
/// `MERGE_NODES` compte dans le risque dérivé.
#[test]
fn merging_two_ends_of_a_path_closes_a_cycle() {
    let base = Version::root(
        &[agent(1), agent(2), agent(4)],
        &[reviews(agent(1), agent(2)), reviews(agent(2), agent(4))],
        &Fnv,
    )
    .expect("chaîne acyclique");
    let region = Region::declare(
        "tout",
        &[agent(1), agent(2), agent(4), agent(9)],
        &Operation::NAMES,
        1,
        8,
        8,
        ApprovalMode::Peer,
        false,
    )
    .expect("région valide");
    let diff = diff_of(
        &base,
        vec![Operation::MergeNodes {
            first: agent(1),
            second: agent(4),
            into: agent(9),
        }],
    );

    let verdict = region
        .admits(&base, &diff, &Fnv)
        .expect("le diff s'applique");
    assert!(
        matches!(
            verdict,
            Verdict::Vetoed {
                invariant: Invariant::ReviewAcyclicity,
                ..
            }
        ),
        "aucune arête ajoutée, et pourtant un cycle : {verdict:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// 3. L'acceptation locale ne commit jamais
// ---------------------------------------------------------------------------------------------

/// Une acceptation **énonce ce qui reste à faire**. Elle n'expose rien qui écrive.
///
/// La garantie est dans le type, pas dans une discipline d'appel : il n'y a pas de méthode à ne pas
/// appeler, comme pour la `Simulation` de `packages/policy`.
#[test]
fn an_acceptance_states_obligations_and_offers_no_way_to_commit() {
    let base = base();
    let diff = diff_of(&base, vec![Operation::AddNode(agent(9))]);
    let region = Region::declare(
        "coeur",
        &[agent(1), agent(2), agent(3), agent(9)],
        &Operation::NAMES,
        1,
        8,
        8,
        ApprovalMode::Human,
        true,
    )
    .expect("région valide");

    let Verdict::Admissible(acceptance) = region
        .admits(&base, &diff, &Fnv)
        .expect("le diff s'applique")
    else {
        panic!("ce lot est admissible");
    };
    assert_eq!(acceptance.requires_approval(), ApprovalMode::Human);
    assert!(acceptance.requires_shadow());
}

/// Les deux obligations traversent le veto : un lot vetoé garde ce que la région exigeait.
///
/// Les perdre ferait croire qu'un lot vetoé n'avait aucune obligation, et la version corrigée
/// repartirait sans ombre ni approbation.
#[test]
fn a_vetoed_batch_keeps_what_the_region_required() {
    let base = base();
    let diff = diff_of(&base, vec![Operation::AddEdge(reviews(agent(1), agent(2)))]);
    let Verdict::Vetoed { accepted, .. } = permissive()
        .admits(&base, &diff, &Fnv)
        .expect("le diff s'applique")
    else {
        panic!("ce lot est vetoé");
    };
    assert_eq!(accepted.requires_approval(), ApprovalMode::Human);
    assert!(accepted.requires_shadow());
}

// ---------------------------------------------------------------------------------------------
// La déclaration d'une région
// ---------------------------------------------------------------------------------------------

/// Une région ne s'autorise pas une opération qui n'existe pas.
///
/// `SET_ROLE` est nommée par `docs/13` et attend son lecteur (W15.e). L'accepter ici ne permettrait
/// rien pendant que l'auteur de la région croirait le contraire — le pire des deux états.
#[test]
fn a_region_cannot_allow_an_operation_that_does_not_exist_yet() {
    let error = Region::declare(
        "coeur",
        &[agent(1)],
        &["ADD_NODE", "SET_ROLE"],
        1,
        8,
        8,
        ApprovalMode::Human,
        false,
    )
    .expect_err("SET_ROLE n'existe pas");
    assert_eq!(
        error,
        RegionError::UnknownOperation {
            operation: "SET_ROLE".to_owned()
        }
    );
}

#[test]
fn a_region_without_a_name_or_without_nodes_is_refused() {
    assert_eq!(
        Region::declare("  ", &[agent(1)], &[], 0, 0, 0, ApprovalMode::Human, false)
            .expect_err("anonyme"),
        RegionError::EmptyName
    );
    assert_eq!(
        Region::declare("coeur", &[], &[], 0, 0, 0, ApprovalMode::Human, false)
            .expect_err("sans nœud"),
        RegionError::EmptyRegion
    );
}

/// Une région ne se prononce pas sur un lot qui ne s'applique pas.
///
/// Elle dirait quelque chose d'un état qui n'existera jamais, et son verdict serait cité comme s'il
/// portait sur quelque chose.
#[test]
fn a_region_says_nothing_about_a_diff_that_does_not_apply() {
    let base = base();
    let diff = diff_of(&base, vec![Operation::AddNode(agent(9))]);
    let moved_on = base
        .apply(&Operation::AddNode(agent(7)), &Fnv)
        .expect("quelqu'un d'autre a commité");

    let error = permissive()
        .admits(&moved_on, &diff, &Fnv)
        .expect_err("la base a bougé");
    assert!(error.needs_rebase());
}
