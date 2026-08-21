//! Test de sortie de W18.b — **les trois garanties de l'item.**
//!
//! 1. Une adaptation rapide ne produit aucune opération de coordination, et aucun chemin de type ne
//!    le permet.
//! 2. Une adaptation lente est une `Proposal` de W13 et suit son chemin entier.
//! 3. Une route éphémère **expire**, et deux adaptations rapides ne s'accumulent jamais en une
//!    structure que personne n'a approuvée.

use std::collections::BTreeSet;

use locus_adaptation::{Adaptation, Adjustment, Fast, FastError, Trigger, slow};
use locus_coordination::{
    Author, ContentDigest, CoordinationMode, Diff, EpistemicIndex, Justification, Mode, Operation,
    Proposal, ProposalError, Relation, RelationKind, Version, approve, commit,
};
use locus_domain::RevisionId;
use locus_protocol::{
    Id, IdKind, Timestamp,
    id::{Agent, provisional::Approval, provisional::Decision as DecisionKind},
};

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

fn revision(seed: u8) -> RevisionId {
    id::<locus_domain::ids::RevisionKind>(seed)
}

struct KnownRevisions(BTreeSet<String>);

impl EpistemicIndex for KnownRevisions {
    fn contains(&self, revision: &RevisionId) -> bool {
        self.0.contains(&revision.to_string())
    }
}

fn index() -> KnownRevisions {
    KnownRevisions([revision(1).to_string()].into_iter().collect())
}

fn at(millis: i64) -> Timestamp {
    Timestamp::from_millis(millis)
}

fn lasting(subject: u8, adjustment: Adjustment, from: i64, until: i64) -> Adaptation {
    Adaptation::lasting(id::<Agent>(subject), adjustment, at(from), at(until))
        .expect("une fenêtre non vide")
}

/// L'organisation sur laquelle l'adaptation lente porte.
fn organisation() -> Version {
    Version::root(
        &[id::<Agent>(1), id::<Agent>(2)],
        &[],
        CoordinationMode::Blackboard,
        None,
        &ContentDigest,
    )
    .expect("fixture cohérente")
}

/// Ce qu'une adaptation lente change — un diff, depuis l'ADR 0021.
fn diff() -> Diff {
    Diff::declaring(
        &organisation(),
        vec![Operation::AddEdge(Relation {
            from: id::<Agent>(1),
            to: id::<Agent>(2),
            kind: RelationKind::Review,
        })],
        &ContentDigest,
    )
    .expect("l'opération s'applique")
}

// ---------------------------------------------------------------------------------------------
// 1. La boucle rapide ne touche pas la structure
// ---------------------------------------------------------------------------------------------

/// Les cinq ajustements de la roadmap, sous leur nom, et pas un sixième.
#[test]
fn the_fast_loop_adjusts_five_things() {
    assert_eq!(
        Adjustment::KINDS.to_vec(),
        vec![
            "model_routing",
            "tool_choice",
            "skill_selection",
            "retry",
            "ephemeral_route",
        ]
    );
    let sample = [
        Adjustment::ModelRouting {
            model: "m".to_owned(),
        },
        Adjustment::ToolChoice {
            tool: "t".to_owned(),
        },
        Adjustment::SkillSelection {
            skill: "s".to_owned(),
        },
        Adjustment::Retry { attempts: 2 },
        Adjustment::EphemeralRoute { to: id::<Agent>(9) },
    ];
    let kinds: Vec<&str> = sample.iter().map(Adjustment::kind).collect();
    assert_eq!(kinds, Adjustment::KINDS.to_vec());
}

/// La boucle rapide ne nomme aucun objet de la boucle lente.
///
/// Elle s'exécute sans approbation, à la latence d'un appel de modèle. Une seule fonction qui
/// rendrait une opération de coordination ferait d'elle un chemin de mutation du graphe sans
/// décision, sans trace et sans révision de base.
///
/// Le test lit le **code**, commentaires retirés : expliquer pourquoi un terme est absent le fait
/// apparaître dans la prose, et un test qui confondrait les deux interdirait d'écrire la raison.
#[test]
fn the_fast_loop_names_no_structural_type() {
    let source = code_of(include_str!("../src/fast.rs"));
    for absent in [
        "locus_coordination",
        "Operation",
        "Change",
        "Relation",
        "Version",
        "Proposal",
        "Committed",
    ] {
        assert!(
            !source.contains(absent),
            "« {absent} » ferait de la boucle rapide un chemin de mutation du graphe"
        );
    }
}

/// Et rien dans le crate ne convertit une adaptation rapide en objet de coordination.
///
/// La vérification porte sur `slow.rs` dans l'autre sens : il ne connaît pas `Adaptation`. Les deux
/// absences ensemble sont la séparation ; l'une seule laisserait la porte de l'autre côté.
#[test]
fn the_slow_loop_names_no_fast_type() {
    let source = code_of(include_str!("../src/slow.rs"));
    for absent in ["Adaptation", "Adjustment", "Fast", "crate::fast"] {
        assert!(
            !source.contains(absent),
            "« {absent} » rouvrirait la conversion par l'autre bout"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 2. La boucle lente est une proposition, et rien d'autre
// ---------------------------------------------------------------------------------------------

/// Le module de la boucle lente ne déclare aucun type.
///
/// Un type à soi serait un type qu'on peut construire sans passer par `Proposal::write` — donc sans
/// vérifier le mode, sans citer une révision existante, sans base de révision — puis qu'on
/// convertirait « au moment de committer », c'est-à-dire trop tard pour refuser.
#[test]
fn the_slow_loop_declares_no_type_of_its_own() {
    let source = code_of(include_str!("../src/slow.rs"));
    for forbidden in ["pub struct", "pub enum", "pub trait"] {
        assert!(
            !source.contains(forbidden),
            "« {forbidden} » ouvrirait une porte parallèle à `Proposal::write`"
        );
    }
    // Et une seule fonction publique : envelopper les quatre étapes de W13 serait une deuxième
    // signature, qui divergerait de la première au premier champ ajouté — et qui compilerait encore.
    assert_eq!(source.matches("pub fn ").count(), 1);
    assert!(source.contains("pub fn justify("));
}

/// La justification produite est une `Justification` de W13, sans emballage.
#[test]
fn the_slow_loop_returns_the_spec_type() {
    let justification: Justification =
        slow::justify(Trigger::BarrierEncountered, revision(1)).expect("un slug non vide");
    assert_eq!(justification.cites(), &revision(1));
}

/// Une adaptation lente cite l'un des onze déclencheurs, jamais autre chose.
#[test]
fn a_slow_adaptation_cites_one_of_the_eleven() {
    for trigger in Trigger::ALL {
        let justification = slow::justify(trigger, revision(1)).expect("un slug non vide");
        assert_eq!(justification.trigger(), trigger.slug());
    }
}

/// Et elle suit le chemin entier de W13 : proposer, approuver par un autre, committer.
#[test]
fn a_slow_adaptation_follows_the_whole_path() {
    let author = Author::Agent(id::<Agent>(1));
    let proposal = Proposal::write(
        id::<DecisionKind>(7),
        author,
        Mode::Assisted,
        4,
        diff(),
        slow::justify(Trigger::ReviewDisagreement, revision(1)).expect("un slug non vide"),
        &index(),
    )
    .expect("un agent propose en mode assisté");
    assert_eq!(proposal.justification().trigger(), "review_disagreement");

    let approved = approve(
        proposal,
        Author::Human("marcel".to_owned()),
        id::<Approval>(3),
    )
    .expect("un autre auteur approuve");
    let committed =
        commit(approved, 4, &organisation(), &ContentDigest).expect("la base est à jour");
    assert_eq!(committed.revision, 5);
    assert_eq!(
        committed.version.parent(),
        Some(organisation().id()),
        "une adaptation lente produit une version, comme toute proposition depuis l'ADR 0021"
    );
}

/// Le mode par défaut refuse à l'agent de proposer — l'ADR 0016 décision 8, tenue depuis ici.
#[test]
fn an_agent_cannot_adapt_the_structure_under_the_default_mode() {
    let refused = Proposal::write(
        id::<DecisionKind>(7),
        Author::Agent(id::<Agent>(1)),
        Mode::Observed,
        4,
        diff(),
        slow::justify(Trigger::BranchStagnation, revision(1)).expect("un slug non vide"),
        &index(),
    )
    .expect_err("`observed` ne permet pas de proposer");
    assert!(matches!(
        refused,
        ProposalError::NotAllowedToPropose {
            mode: Mode::Observed,
            ..
        }
    ));
}

/// Et le proposeur ne s'approuve pas lui-même, adaptation automatique ou pas.
#[test]
fn a_slow_adaptation_is_not_self_approved() {
    let author = Author::Agent(id::<Agent>(1));
    let proposal = Proposal::write(
        id::<DecisionKind>(7),
        author.clone(),
        Mode::Assisted,
        4,
        diff(),
        slow::justify(Trigger::NewMethodFound, revision(1)).expect("un slug non vide"),
        &index(),
    )
    .expect("un agent propose en mode assisté");
    let refused =
        approve(proposal, author, id::<Approval>(3)).expect_err("le proposeur n'approuve pas");
    assert!(matches!(refused, ProposalError::SelfApproval { .. }));
}

/// Une révision non citée est refusée : une adaptation automatique ne s'invente pas de motif.
#[test]
fn a_slow_adaptation_cites_an_existing_revision() {
    let refused = Proposal::write(
        id::<DecisionKind>(7),
        Author::Human("marcel".to_owned()),
        Mode::Assisted,
        4,
        diff(),
        slow::justify(Trigger::SourceConflict, revision(42)).expect("un slug non vide"),
        &index(),
    )
    .expect_err("la révision 42 n'existe pas");
    assert!(matches!(
        refused,
        ProposalError::UncitedJustification { .. }
    ));
}

// ---------------------------------------------------------------------------------------------
// 3. Tout expire, et rien ne s'accumule
// ---------------------------------------------------------------------------------------------

/// Une fenêtre vide ou renversée n'est pas une adaptation.
#[test]
fn there_is_no_endless_fast_adaptation() {
    for (from, until) in [(100, 100), (100, 99), (0, 0)] {
        let refused = Adaptation::lasting(
            id::<Agent>(1),
            Adjustment::Retry { attempts: 3 },
            at(from),
            at(until),
        )
        .expect_err("une adaptation rapide dure, ou n'existe pas");
        assert_eq!(
            refused,
            FastError::EmptyWindow {
                from: at(from),
                until: at(until),
            }
        );
    }
}

/// La fenêtre est semi-ouverte : `until` n'est pas couvert.
///
/// Une borne haute incluse ferait se chevaucher deux fenêtres consécutives sur exactement un
/// instant — et à cet instant-là, deux routages seraient vivants pour le même agent.
#[test]
fn the_window_is_half_open() {
    let adaptation = lasting(1, Adjustment::Retry { attempts: 3 }, 100, 200);
    assert!(!adaptation.covers(at(99)));
    assert!(adaptation.covers(at(100)));
    assert!(adaptation.covers(at(199)));
    assert!(!adaptation.covers(at(200)));
}

/// Une route éphémère expire, et l'agent ne voit plus rien.
#[test]
fn an_ephemeral_route_expires() {
    let subject = id::<Agent>(1);
    let target = id::<Agent>(2);
    let fast = Fast::new()
        .adopting(lasting(
            1,
            Adjustment::EphemeralRoute { to: target },
            100,
            200,
        ))
        .expect("rien ne la contrarie");

    assert_eq!(
        fast.routes_from(subject, at(150)),
        [target].into_iter().collect()
    );
    assert!(fast.routes_from(subject, at(200)).is_empty());
    assert!(fast.routes_from(subject, at(10_000)).is_empty());
    // Elle reste adoptée — l'histoire ne s'efface pas —, elle n'est simplement plus vivante.
    assert_eq!(fast.len(), 1);
}

/// **La garantie de l'item.** Trois routes successives ne font jamais une topologie à trois arêtes.
///
/// C'est la faute que la boucle rapide inviterait : chaque route est licite, et leur **union** est
/// une structure que personne n'a proposée, approuvée ni commitée. Le test balaie tous les instants
/// intéressants et vérifie qu'à aucun la vue ne dépasse une seule route.
#[test]
fn successive_routes_never_add_up_to_a_topology() {
    let subject = id::<Agent>(1);
    let fast = Fast::new()
        .adopting(lasting(
            1,
            Adjustment::EphemeralRoute { to: id::<Agent>(2) },
            0,
            10,
        ))
        .expect("rien ne la contrarie")
        .adopting(lasting(
            1,
            Adjustment::EphemeralRoute { to: id::<Agent>(3) },
            10,
            20,
        ))
        .expect("rien ne la contrarie")
        .adopting(lasting(
            1,
            Adjustment::EphemeralRoute { to: id::<Agent>(4) },
            20,
            30,
        ))
        .expect("rien ne la contrarie");

    let widest = (0..40)
        .map(|instant| fast.routes_from(subject, at(instant)).len())
        .max()
        .expect("la plage n'est pas vide");
    assert_eq!(widest, 1, "l'union des routes ne doit jamais être vivante");
    assert!(fast.routes_from(subject, at(30)).is_empty());
}

/// Deux routages de modèle qui se chevauchent sont refusés, pas arbitrés.
///
/// Les départager en silence — la dernière adoptée gagne — ferait dépendre le modèle qui répond de
/// l'ordre dans lequel deux ajustements sont arrivés.
#[test]
fn two_overlapping_exclusive_adjustments_are_refused() {
    let fast = Fast::new()
        .adopting(lasting(
            1,
            Adjustment::ModelRouting {
                model: "opus".to_owned(),
            },
            100,
            200,
        ))
        .expect("la première passe");
    let refused = fast
        .adopting(lasting(
            1,
            Adjustment::ModelRouting {
                model: "haiku".to_owned(),
            },
            150,
            250,
        ))
        .expect_err("deux routages simultanés ne se départagent pas");
    assert_eq!(
        refused,
        FastError::Overlapping {
            kind: "model_routing",
            from: at(100),
            until: at(200),
        }
    );
}

/// Le refus est **par agent** : deux agents peuvent être routés différemment au même instant.
///
/// C'est l'inverse de la faute précédente, et elle serait aussi coûteuse : un refus trop large
/// empêcherait d'ajuster une flotte, et on relâcherait la règle entière pour la contourner.
#[test]
fn two_agents_may_route_differently_at_the_same_instant() {
    let fast = Fast::new()
        .adopting(lasting(
            1,
            Adjustment::ModelRouting {
                model: "opus".to_owned(),
            },
            100,
            200,
        ))
        .expect("la première passe")
        .adopting(lasting(
            2,
            Adjustment::ModelRouting {
                model: "haiku".to_owned(),
            },
            100,
            200,
        ))
        .expect("un autre agent n'est pas le même agent");
    assert_eq!(fast.model_for(id::<Agent>(1), at(150)), Some("opus"));
    assert_eq!(fast.model_for(id::<Agent>(2), at(150)), Some("haiku"));
}

/// Et **par sorte** : un routage et un budget de réessai ne se contrarient pas.
#[test]
fn two_exclusive_adjustments_of_different_kinds_may_overlap() {
    let fast = Fast::new()
        .adopting(lasting(
            1,
            Adjustment::ModelRouting {
                model: "opus".to_owned(),
            },
            100,
            200,
        ))
        .expect("la première passe")
        .adopting(lasting(1, Adjustment::Retry { attempts: 3 }, 100, 200))
        .expect("un réessai n'est pas un routage");
    assert_eq!(fast.live_at(at(150)).count(), 2);
}

/// Mais deux routages **consécutifs** passent — c'est la fenêtre semi-ouverte qui le permet.
#[test]
fn two_consecutive_exclusive_adjustments_are_allowed() {
    let subject = id::<Agent>(1);
    let fast = Fast::new()
        .adopting(lasting(
            1,
            Adjustment::ModelRouting {
                model: "opus".to_owned(),
            },
            100,
            200,
        ))
        .expect("la première passe")
        .adopting(lasting(
            1,
            Adjustment::ModelRouting {
                model: "haiku".to_owned(),
            },
            200,
            300,
        ))
        .expect("la seconde commence là où la première finit");
    assert_eq!(fast.model_for(subject, at(199)), Some("opus"));
    assert_eq!(fast.model_for(subject, at(200)), Some("haiku"));
    assert_eq!(fast.model_for(subject, at(300)), None);
}

/// Un ajustement additif se cumule, lui — et c'est voulu.
#[test]
fn additive_adjustments_accumulate_within_their_window() {
    let subject = id::<Agent>(1);
    let fast = Fast::new()
        .adopting(lasting(
            1,
            Adjustment::ToolChoice {
                tool: "sparql".to_owned(),
            },
            0,
            100,
        ))
        .expect("rien ne la contrarie")
        .adopting(lasting(
            1,
            Adjustment::ToolChoice {
                tool: "iiif".to_owned(),
            },
            0,
            100,
        ))
        .expect("un outil de plus n'invalide pas le premier")
        .adopting(lasting(
            1,
            Adjustment::SkillSelection {
                skill: "alto".to_owned(),
            },
            0,
            100,
        ))
        .expect("rien ne la contrarie");
    assert_eq!(
        fast.tools_for(subject, at(50)),
        ["iiif", "sparql"].into_iter().collect()
    );
    assert_eq!(
        fast.skills_for(subject, at(50)),
        ["alto"].into_iter().collect()
    );
    // Et tout meurt ensemble.
    assert!(fast.tools_for(subject, at(100)).is_empty());
    assert!(fast.skills_for(subject, at(100)).is_empty());
}

/// L'adaptation d'un agent n'est jamais celle d'un autre.
#[test]
fn a_fast_adaptation_reaches_only_its_subject() {
    let fast = Fast::new()
        .adopting(lasting(
            1,
            Adjustment::ModelRouting {
                model: "opus".to_owned(),
            },
            0,
            100,
        ))
        .expect("rien ne la contrarie");
    assert_eq!(fast.model_for(id::<Agent>(1), at(50)), Some("opus"));
    assert_eq!(fast.model_for(id::<Agent>(2), at(50)), None);
}

/// Un modèle, un outil ou un skill sans nom est refusé sous le nom du champ.
#[test]
fn a_nameless_target_is_refused() {
    for (field, adjustment) in [
        (
            "model",
            Adjustment::ModelRouting {
                model: "  ".to_owned(),
            },
        ),
        (
            "tool",
            Adjustment::ToolChoice {
                tool: String::new(),
            },
        ),
        (
            "skill",
            Adjustment::SkillSelection {
                skill: "\t".to_owned(),
            },
        ),
    ] {
        let refused = Adaptation::lasting(id::<Agent>(1), adjustment, at(0), at(10))
            .expect_err("un ajustement sans destinataire n'ajuste rien");
        assert_eq!(refused, FastError::EmptyName { field });
    }
}

/// Un `Fast` vide ne route rien, et ne prétend pas router par défaut.
#[test]
fn an_empty_fast_loop_claims_no_default() {
    let fast = Fast::new();
    assert!(fast.is_empty());
    assert_eq!(fast.model_for(id::<Agent>(1), at(50)), None);
    assert!(fast.routes_from(id::<Agent>(1), at(50)).is_empty());
    assert_eq!(fast.live_at(at(50)).count(), 0);
}
