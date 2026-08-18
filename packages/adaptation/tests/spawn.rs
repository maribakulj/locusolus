//! Test de sortie de W18.a — **les trois garanties de l'item.**
//!
//! 1. Les onze déclencheurs de §14.5 se lisent sous leur nom, et un douzième n'existe pas.
//! 2. Une proposition à qui il manque un champ n'est pas construite, et le refus nomme le champ.
//! 3. Aucun chemin ne mène d'un déclencheur à une flotte sans passer par la réponse du moteur de
//!    politique — tenu par l'absence de constructeur d'[`Admitted`].

use std::time::Duration;

use locus_adaptation::{
    Admitted, Disposition, Draft, SpawnError, SpawnProposal, Trigger, Undecided,
};
use locus_budget::{Dimension, Limits};
use locus_coordination::{Capability, Command};
use locus_policy::{Facts, Outcome, Policy, Rule, Verb};

/// Le source sans ses commentaires — ce que le module **fait**, pas ce qu'il explique.
fn code_of(source: &str) -> String {
    source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn cost() -> Limits {
    Limits::bounding([(Dimension::Tokens, 100_000), (Dimension::ModelCalls, 40)])
        .expect("deux dimensions bornées")
}

/// Les neuf clés, toutes remplies.
fn draft(reason: Trigger) -> Draft {
    Draft {
        reason,
        missing_capability: Capability::new("lean4").expect("un nom non vide"),
        expected_information_gain: 0.4,
        diversity_contribution: 0.7,
        cost_estimate: cost(),
        time_to_live: Duration::from_secs(3_600),
        termination_condition: "la formalisation compile ou trois échecs consécutifs".to_owned(),
        context_policy: "ctx-policy/reviewer-isolation".to_owned(),
        review_policy: "review-policy/independent-two".to_owned(),
    }
}

fn complete(reason: Trigger) -> SpawnProposal {
    SpawnProposal::declare(draft(reason)).expect("une proposition à neuf champs")
}

// ---------------------------------------------------------------------------------------------
// 1. Les onze, sous leur nom
// ---------------------------------------------------------------------------------------------

/// Le bloc de §14.5, recopié ici comme fixture.
///
/// Le test compare à une constante écrite à la main plutôt qu'à `Trigger::ALL` reformaté : comparer
/// une liste à elle-même passerait quoi qu'on ait écrit dans l'énumération.
const SPEC_14_5: [&str; 11] = [
    "domain_gap_detected",
    "review_disagreement",
    "barrier_encountered",
    "branch_stagnation",
    "formalization_blocked",
    "counterexample_needed",
    "new_method_found",
    "bridge_candidate",
    "high_uncertainty",
    "reproduction_failure",
    "source_conflict",
];

#[test]
fn the_eleven_triggers_read_under_their_spec_name() {
    let named: Vec<&str> = Trigger::ALL.iter().map(|trigger| trigger.slug()).collect();
    assert_eq!(named, SPEC_14_5.to_vec());
}

#[test]
fn each_spec_name_round_trips() {
    for name in SPEC_14_5 {
        let trigger = Trigger::from_slug(name).expect("un nom de §14.5 se relit");
        assert_eq!(trigger.slug(), name);
    }
}

/// Un douzième déclencheur n'existe pas, et un nom inconnu ne devient pas le premier de la liste.
#[test]
fn a_twelfth_trigger_does_not_exist() {
    assert_eq!(Trigger::ALL.len(), 11);
    for unknown in [
        "budget_exceeded",
        "operator_request",
        "domain_gap",
        "",
        "DOMAIN_GAP_DETECTED",
    ] {
        assert!(
            Trigger::from_slug(unknown).is_none(),
            "« {unknown} » n'est pas un déclencheur de §14.5 et ne doit pas en devenir un"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 2. Neuf champs, et l'absence nommée
// ---------------------------------------------------------------------------------------------

/// Les neuf clés du bloc YAML de §14.5 sont les neuf champs de `Draft`, sous leur nom.
///
/// Le test lit la déclaration de la structure : une clé renommée « pour la clarté » ferait diverger
/// le type de la spec sans que rien d'autre échoue.
#[test]
fn the_nine_keys_of_the_spec_are_the_nine_fields() {
    const SPEC_KEYS: [&str; 9] = [
        "reason",
        "missing_capability",
        "expected_information_gain",
        "diversity_contribution",
        "cost_estimate",
        "time_to_live",
        "termination_condition",
        "context_policy",
        "review_policy",
    ];
    let source = code_of(include_str!("../src/spawn.rs"));
    let declaration = struct_body(&source, "pub struct Draft {");
    let declared: Vec<&str> = declaration
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub "))
        .filter_map(|line| line.split(':').next())
        .collect();
    assert_eq!(declared, SPEC_KEYS.to_vec());
}

/// Chacun des trois champs textuels manquants est refusé **sous son nom**.
///
/// Un refus qui dirait seulement « proposition invalide » ferait relire les neuf clés à la main.
#[test]
fn a_missing_text_field_is_refused_by_its_name() {
    for (field, termination, context, review) in [
        (
            "termination_condition",
            "   ",
            "ctx-policy/x",
            "review-policy/y",
        ),
        ("context_policy", "trois échecs", "", "review-policy/y"),
        ("review_policy", "trois échecs", "ctx-policy/x", "  "),
    ] {
        let refused = SpawnProposal::declare(Draft {
            termination_condition: termination.to_owned(),
            context_policy: context.to_owned(),
            review_policy: review.to_owned(),
            ..draft(Trigger::HighUncertainty)
        })
        .expect_err("un champ vide n'est pas une proposition");
        assert_eq!(refused, SpawnError::EmptyField { field });
    }
}

/// Un gain attendu ou une contribution de diversité hors de `0..=1` est refusé, `NaN` compris.
#[test]
fn a_value_claim_outside_the_unit_range_is_refused() {
    for value in [-0.01_f64, 1.01, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let refused = SpawnProposal::declare(Draft {
            expected_information_gain: value,
            ..draft(Trigger::HighUncertainty)
        })
        .expect_err("une proportion hors plage n'est pas une proportion");
        assert!(matches!(
            refused,
            SpawnError::NotAProportion {
                field: "expected_information_gain",
                ..
            }
        ));
    }
}

/// Une durée de vie nulle n'est pas « pas de limite ».
#[test]
fn a_zero_time_to_live_is_refused() {
    let refused = SpawnProposal::declare(Draft {
        time_to_live: Duration::ZERO,
        ..draft(Trigger::BranchStagnation)
    })
    .expect_err("zéro n'est pas une durée de vie");
    assert_eq!(refused, SpawnError::ZeroTimeToLive);
}

/// Un coût sans aucune dimension bornée n'est pas un coût — et c'est `budget` qui le dit, pas ce
/// module : une dimension non nommée est **hors budget**, pas libre.
#[test]
fn an_unbounded_cost_estimate_cannot_even_be_built() {
    assert!(Limits::bounding([]).is_err());
}

// ---------------------------------------------------------------------------------------------
// 3. Aucun chemin du déclencheur à la flotte sans le moteur
// ---------------------------------------------------------------------------------------------

/// La proposition ne sait pas fabriquer d'agent.
///
/// Le test lit le **code**, commentaires retirés : expliquer pourquoi un terme est absent le fait
/// apparaître dans la prose, et un test qui confondrait les deux interdirait d'écrire la raison.
#[test]
fn nothing_but_dispose_produces_an_admission() {
    let source = code_of(include_str!("../src/spawn.rs"));
    for forbidden in [
        "fn admit(",
        "fn approve",
        "fn spawn(",
        "Admitted::new",
        "fn new(",
    ] {
        assert!(
            !source.contains(forbidden),
            "« {forbidden} » ouvrirait un chemin vers une flotte qui ne passe pas par le moteur"
        );
    }

    // Le bloc `impl Admitted` ne construit ni ne rend un `Self` : ses trois membres lisent.
    let block = impl_block(&source, "impl Admitted {");
    assert!(!block.contains("-> Self"), "{block}");
    assert!(!block.contains("Self {"), "{block}");

    // Un seul site construit la structure, et il est dans `dispose` — donc derrière un verdict.
    // La déclaration et le bloc `impl` portent la même sous-chaîne ; ils sont retirés par leur nom
    // plutôt qu'en ajustant un compte, qu'un ajout ultérieur ferait « corriger » sans réfléchir.
    let constructions = source
        .match_indices("Admitted {")
        .filter(|(offset, _)| {
            !source[..*offset].ends_with("pub struct ") && !source[..*offset].ends_with("impl ")
        })
        .count();
    assert_eq!(constructions, 1);
    assert_eq!(
        source.matches("Disposition::Accepted(Admitted {").count(),
        1
    );
}

/// Le corps d'une déclaration de structure, par appariement d'accolades.
fn struct_body<'a>(source: &'a str, header: &str) -> &'a str {
    impl_block(source, header)
}

/// Le corps d'un bloc `impl`, par appariement d'accolades.
///
/// Chercher `-> Self` dans le fichier entier confondrait `Admitted` avec `SpawnProposal`, qui a le
/// droit d'en rendre un : c'est le bloc qui porte la garantie, pas le fichier.
fn impl_block<'a>(source: &'a str, header: &str) -> &'a str {
    let start = source.find(header).expect("le bloc existe");
    let body = &source[start + header.len()..];
    let mut depth = 1_i32;
    for (offset, character) in body.char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &body[..offset];
                }
            }
            _ => {}
        }
    }
    panic!("bloc `{header}` non refermé");
}

/// Et l'admission ne se lit que par une `Option` : aucune variante ne se déplie en supposant l'accord.
#[test]
fn a_refusal_yields_no_admission() {
    let refused = locus_adaptation::dispose(
        complete(Trigger::SourceConflict),
        &Outcome::Decided {
            verb: Verb::Deny,
            by: "spawn/deny-unbudgeted".to_owned(),
        },
    );
    assert_eq!(
        refused,
        Disposition::Refused {
            by: "spawn/deny-unbudgeted".to_owned()
        }
    );
    assert!(refused.admitted().is_none());
}

/// Les quatre réponses de §14.5, chacune distinguée de l'admission.
#[test]
fn the_four_answers_of_the_spec_are_distinct() {
    let by = "spawn/rule".to_owned();
    let cases = [
        (Verb::Allow, true),
        (Verb::Deny, false),
        (
            Verb::Modify {
                constraint: "ttl <= 900s".to_owned(),
            },
            false,
        ),
        (
            Verb::RequireApproval {
                approver_role: "principal-investigator".to_owned(),
            },
            false,
        ),
    ];
    for (verb, admits) in cases {
        let disposition = locus_adaptation::dispose(
            complete(Trigger::NewMethodFound),
            &Outcome::Decided {
                verb: verb.clone(),
                by: by.clone(),
            },
        );
        assert_eq!(
            disposition.admitted().is_some(),
            admits,
            "« {verb} » ne dispose pas comme prévu"
        );
    }
}

/// Une admission porte la règle qui l'a portée, et n'autorise que `spawn`.
#[test]
fn an_admission_carries_its_rule_and_authorises_only_spawn() {
    let disposition = locus_adaptation::dispose(
        complete(Trigger::DomainGapDetected),
        &Outcome::Decided {
            verb: Verb::Allow,
            by: "spawn/allow-formalisation".to_owned(),
        },
    );
    let admitted: &Admitted = disposition.admitted().expect("`allow` admet");
    assert_eq!(admitted.by(), "spawn/allow-formalisation");
    assert_eq!(admitted.command(), Command::Spawn);
    assert_eq!(admitted.proposal().reason(), Trigger::DomainGapDetected);
}

/// Le silence n'est pas une autorisation — et c'est le cas qui fait la phrase de §14.5.
///
/// Un agent qui propose un spawn qu'aucune règle ne couvre est exactement l'agent qui « crée
/// librement une flotte non bornée » si le silence vaut accord. Il vaut `Undecided::Silent`.
#[test]
fn silence_is_not_permission() {
    let disposition =
        locus_adaptation::dispose(complete(Trigger::HighUncertainty), &Outcome::NoRule);
    assert_eq!(disposition, Disposition::Undecided(Undecided::Silent));
    assert!(disposition.admitted().is_none());
}

/// Un conflit non plus, et il se rend au lieu de se trancher.
#[test]
fn a_conflict_is_rendered_not_resolved() {
    let disposition = locus_adaptation::dispose(
        complete(Trigger::BridgeCandidate),
        &Outcome::Conflict {
            priority: 50,
            rules: vec![
                "spawn/allow-bridges".to_owned(),
                "spawn/deny-bridges".to_owned(),
            ],
        },
    );
    assert_eq!(
        disposition,
        Disposition::Undecided(Undecided::Conflict {
            priority: 50,
            rules: vec![
                "spawn/allow-bridges".to_owned(),
                "spawn/deny-bridges".to_owned()
            ],
        })
    );
    assert!(disposition.admitted().is_none());
}

/// Le cinquième verbe de §20.2 n'est pas une des quatre réponses de §14.5, et n'admet rien.
#[test]
fn require_tasks_is_not_one_of_the_four() {
    let disposition = locus_adaptation::dispose(
        complete(Trigger::ReproductionFailure),
        &Outcome::Decided {
            verb: Verb::RequireTasks {
                tasks: vec!["rerun-under-r3".to_owned()],
            },
            by: "spawn/reproduce-first".to_owned(),
        },
    );
    assert_eq!(
        disposition,
        Disposition::Undecided(Undecided::TasksFirst {
            tasks: vec!["rerun-under-r3".to_owned()],
        })
    );
    assert!(disposition.admitted().is_none());
}

// ---------------------------------------------------------------------------------------------
// Les faits : ce que le moteur sait, et ce qu'il ne doit pas savoir
// ---------------------------------------------------------------------------------------------

/// Les prétentions de valeur ne sont pas des faits.
///
/// §13.4 fait de `G` et `D` des termes que le portefeuille calcule. Une règle qui s'accrocherait au
/// chiffre annoncé par le proposeur le laisserait choisir son propre verdict.
#[test]
fn the_value_claims_are_not_facts() {
    let facts = complete(Trigger::CounterexampleNeeded).facts();
    let keys: Vec<&str> = facts.entries().into_iter().map(|(key, _)| key).collect();
    for absent in [
        "spawn.expected_information_gain",
        "spawn.diversity_contribution",
    ] {
        assert!(
            !keys.contains(&absent),
            "« {absent} » laisserait le proposeur choisir son verdict"
        );
    }
}

/// Le coût, lui, en est un — c'est une borne, et la réservation ne croit personne.
#[test]
fn the_cost_ceilings_are_facts() {
    let facts = complete(Trigger::CounterexampleNeeded).facts();
    assert!(facts.holds("spawn.cost.tokens", "100000"));
    assert!(facts.holds("spawn.cost.model_calls", "40"));
    assert!(facts.holds("spawn.reason", "counterexample_needed"));
    assert!(facts.holds("spawn.time_to_live_seconds", "3600"));
    // Une dimension que l'estimation ne borne pas n'apparaît pas : elle est hors budget, pas libre.
    assert!(
        facts
            .entries()
            .iter()
            .all(|(key, _)| *key != "spawn.cost.amount")
    );
}

/// De bout en bout : une vraie politique, un vrai verdict, une admission.
#[test]
fn a_real_policy_decides_the_spawn() {
    let policy = Policy::new()
        .with(
            Rule::declare(
                "spawn/allow-formalisation",
                1,
                10,
                &[("spawn.reason", "formalization_blocked")],
                Verb::Allow,
            )
            .expect("une règle déclarée"),
        )
        .expect("un identifiant neuf")
        .with(
            Rule::declare(
                "spawn/approve-expensive",
                1,
                20,
                &[("spawn.cost.tokens", "100000")],
                Verb::RequireApproval {
                    approver_role: "principal-investigator".to_owned(),
                },
            )
            .expect("une règle déclarée"),
        )
        .expect("un identifiant neuf");

    let proposal = complete(Trigger::FormalizationBlocked);
    let evaluation = policy.evaluate(&proposal.facts());
    let disposition = locus_adaptation::dispose(proposal, evaluation.outcome());

    // La priorité explicite tranche : l'approbation l'emporte sur l'autorisation.
    assert_eq!(
        disposition,
        Disposition::ApprovalRequired {
            approver_role: "principal-investigator".to_owned(),
            by: "spawn/approve-expensive".to_owned(),
        }
    );
    assert!(disposition.admitted().is_none());
    assert_eq!(evaluation.trace().len(), 2);
}

/// Et une proposition dont aucune règle ne parle reste sans admission, politique ou pas.
#[test]
fn a_policy_that_says_nothing_admits_nothing() {
    let policy = Policy::new()
        .with(
            Rule::declare(
                "spawn/allow-formalisation",
                1,
                10,
                &[("spawn.reason", "formalization_blocked")],
                Verb::Allow,
            )
            .expect("une règle déclarée"),
        )
        .expect("un identifiant neuf");

    let proposal = complete(Trigger::SourceConflict);
    let facts: Facts = proposal.facts();
    let evaluation = policy.evaluate(&facts);
    let disposition = locus_adaptation::dispose(proposal, evaluation.outcome());
    assert_eq!(disposition, Disposition::Undecided(Undecided::Silent));
}
