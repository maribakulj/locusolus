//! « Invalider une prémisse propage correctement » — le test de sortie de W1.f.

use std::collections::{BTreeMap, BTreeSet};

use locus_domain::{RevisionId, ValidationLevel};
use locus_graph::{FormalizationStatus, Graph, Inference, Relation, RelationKind, Strength};
use locus_protocol::Timestamp;
use locus_validation::{
    Condition, DEPENDENCY_RELATIONS, InvalidatingEvent, PriorAssessment, PriorAssessments,
    ReassessmentMark, Trigger, TypePolicy, propagate,
};

/// Un générateur congruentiel linéaire. Même choix que dans les paquets voisins.
struct Rng(u64);

impl Rng {
    const fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn revision(&mut self) -> RevisionId {
        let mut entropy = [0u8; 10];
        for byte in &mut entropy {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            *byte = u8::try_from(self.0 >> 56).unwrap_or(0);
        }
        RevisionId::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
            .expect("instant dans les bornes")
    }
}

fn inference(id: &str, premises: Vec<RevisionId>, conclusion: RevisionId) -> Inference {
    Inference {
        id: id.to_owned(),
        inference_kind: "deduction".to_owned(),
        premise_ids: premises,
        conclusion_ids: vec![conclusion],
        assumption_ids: Vec::new(),
        rule: "modus ponens".to_owned(),
        scope: "corpus de référence".to_owned(),
        formalization_status: FormalizationStatus::Informal,
        evidence_refs: Vec::new(),
        author: "agent_1".to_owned(),
        review_status: "pending".to_owned(),
    }
}

fn relation(id: &str, from: RevisionId, to: RevisionId, kind: RelationKind) -> Relation {
    Relation {
        id: id.to_owned(),
        from,
        to,
        kind,
        author: "agent_1".to_owned(),
        scope: "global".to_owned(),
        strength: Strength::new(0.9).expect("force bornée"),
        justification: "établi".to_owned(),
        evidence_refs: Vec::new(),
        revision: 1,
    }
}

fn policy() -> TypePolicy {
    TypePolicy {
        object_type: "Claim".to_owned(),
        discipline: "chimie".to_owned(),
        minimal_evidence: vec!["un run reproductible".to_owned()],
        mandatory_reviews: vec!["revue indépendante".to_owned()],
        automatable_checks: vec!["cohérence des unités".to_owned()],
        inapplicable_levels: BTreeSet::new(),
        promotion_conditions: vec![Condition {
            level: "independently_reviewed".to_owned(),
            requirement: "une revue satisfaite".to_owned(),
        }],
        demotion_conditions: Vec::new(),
        invalidating_events: InvalidatingEvent::ALL.into_iter().collect(),
    }
}

fn prior(level: ValidationLevel, why: &str) -> PriorAssessment {
    PriorAssessment {
        level,
        justification: why.to_owned(),
    }
}

// ————————————————————————— Le test de sortie de W1.f —————————————————————————

#[test]
fn refuting_a_premise_propagates_through_the_whole_chain() {
    // §8.3, les cinq points, sur une chaîne à trois étages :
    //
    //   prémisse ──(inf_1)──► conclusion ──(inf_2)──► claim final
    //        │
    //        └──(depends_on)── dérivé
    //
    // et un objet **indépendant** à côté, pour que « propage correctement » veuille aussi dire
    // « ne propage pas là où il ne faut pas ».
    let mut rng = Rng::new(301);
    let premise = rng.revision();
    let other_premise = rng.revision();
    let conclusion = rng.revision();
    let final_claim = rng.revision();
    let derived = rng.revision();
    let unrelated = rng.revision();

    let mut graph = Graph::new();
    graph.add_inference(inference("inf_1", vec![premise, other_premise], conclusion));
    graph.add_inference(inference("inf_2", vec![conclusion], final_claim));
    graph.add_relation(relation(
        "rel_1",
        derived,
        premise,
        RelationKind::DerivedFrom,
    ));
    // Un objet sans lien, et une citation — qui ne propage pas, voir plus bas.
    graph.add_relation(relation("rel_2", unrelated, premise, RelationKind::Cites));

    let mut priors: PriorAssessments = BTreeMap::new();
    priors.insert(
        conclusion,
        prior(ValidationLevel::IndependentlyReviewed, "revue de 2026-03"),
    );
    priors.insert(
        final_claim,
        prior(ValidationLevel::Traceable, "provenance complète"),
    );

    let result = propagate(
        &graph,
        Trigger {
            revision_id: premise,
            event: InvalidatingEvent::Refuted,
        },
        &priors,
        Some(&policy()),
    );

    // 1. « Identifie les objets transitivement dépendants ». Trois : la conclusion directe, le
    //    claim final au second étage, et le dérivé par la relation.
    let mut found = result.dependents.clone();
    found.sort_unstable();
    let mut expected = vec![conclusion, final_claim, derived];
    expected.sort_unstable();
    assert_eq!(
        found, expected,
        "la propagation n'atteint pas tout le monde"
    );

    // Et elle s'arrête où il faut : l'autre prémisse de `inf_1` ne dépend pas de la première, et
    // l'objet qui cite ne dépend pas de ce qu'il cite.
    assert!(!result.dependents.contains(&other_premise));
    assert!(!result.dependents.contains(&unrelated));
    assert!(
        !result.dependents.contains(&premise),
        "l'objet invalidé s'est marqué lui-même"
    );

    // 2. « Ne les réfute pas automatiquement sans règle disciplinaire ». Rien dans le résultat ne
    //    porte un niveau : la propagation pose une question, elle n'y répond pas.
    let source = include_str!("../src/propagation.rs");
    for forbidden in [
        "fn refute",
        "fn demote",
        "fn downgrade",
        "new_level",
        "resulting_level",
    ] {
        assert!(!source.contains(forbidden), "`{forbidden}` existe");
    }

    // 3. « Les marque `needs_reassessment` ». Une marque par dépendant, avec sa distance.
    assert_eq!(result.marks.len(), 3);
    let by_id: BTreeMap<RevisionId, &ReassessmentMark> = result
        .marks
        .iter()
        .map(|mark| (mark.revision_id, mark))
        .collect();
    assert_eq!(by_id[&conclusion].distance, 1);
    assert_eq!(by_id[&derived].distance, 1);
    assert_eq!(
        by_id[&final_claim].distance, 2,
        "le second étage a été aplati"
    );
    assert!(by_id[&conclusion].reason.contains("réfuté"));

    // 4. « Ouvre des tâches de réévaluation selon la politique ». Une par dépendant.
    assert_eq!(result.tasks.len(), 3);
    assert!(result.tasks.iter().all(|task| task.discipline == "chimie"));
    assert_eq!(result.tasks[0].requirements, vec!["un run reproductible"]);

    // 5. « Conserve le niveau et la justification antérieurs dans l'historique ». Sans cette trace,
    //    une réévaluation repartirait de zéro et le travail qui avait mené à L3 serait perdu au
    //    lieu d'être remis en question.
    assert_eq!(
        by_id[&conclusion].prior,
        Some(prior(
            ValidationLevel::IndependentlyReviewed,
            "revue de 2026-03"
        ))
    );
    assert_eq!(
        by_id[&final_claim].prior,
        Some(prior(ValidationLevel::Traceable, "provenance complète"))
    );
    // Et un dépendant dont on ne savait rien porte `None`, pas L0 : « je ne sais pas ce qu'il
    // valait » et « il ne valait rien » ne sont pas la même information.
    assert_eq!(by_id[&derived].prior, None);
}

#[test]
fn the_three_invalidating_events_all_propagate() {
    // §8.3 : « réfuté, retiré ou révisé ». Les trois posent la même question, même si elles ne
    // reçoivent pas forcément la même réponse.
    let mut rng = Rng::new(302);
    let premise = rng.revision();
    let conclusion = rng.revision();
    let mut graph = Graph::new();
    graph.add_inference(inference("inf_1", vec![premise], conclusion));

    for event in InvalidatingEvent::ALL {
        let result = propagate(
            &graph,
            Trigger {
                revision_id: premise,
                event,
            },
            &BTreeMap::new(),
            Some(&policy()),
        );
        assert_eq!(
            result.dependents,
            vec![conclusion],
            "{event:?} ne propage pas"
        );
    }
}

#[test]
fn a_discipline_that_does_not_call_this_event_invalidating_stops_the_propagation() {
    // §8.2, dernière puce : c'est la discipline qui déclare « les événements qui invalident les
    // dépendants ». Une révision peut laisser une conclusion debout dans un domaine et la faire
    // tomber dans un autre.
    let mut rng = Rng::new(303);
    let premise = rng.revision();
    let conclusion = rng.revision();
    let mut graph = Graph::new();
    graph.add_inference(inference("inf_1", vec![premise], conclusion));

    let tolerant = TypePolicy {
        invalidating_events: [InvalidatingEvent::Refuted].into_iter().collect(),
        ..policy()
    };
    let result = propagate(
        &graph,
        Trigger {
            revision_id: premise,
            event: InvalidatingEvent::Revised,
        },
        &BTreeMap::new(),
        Some(&tolerant),
    );
    assert!(result.dependents.is_empty());
    // Et le silence se dit : sans constat, « aucun dépendant » et « la politique a refusé de
    // propager » se ressembleraient.
    assert!(
        result
            .findings
            .iter()
            .any(|line| line.contains("invalidant"))
    );
}

#[test]
fn without_a_policy_the_marks_are_made_and_the_missing_tasks_are_named() {
    // §8.3 dit « marque `needs_reassessment` », pas « marque si une discipline le demande ». Ce qui
    // manque sans politique, ce sont les tâches du point 4 — et une liste vide se lirait « rien à
    // réévaluer ».
    let mut rng = Rng::new(304);
    let premise = rng.revision();
    let conclusion = rng.revision();
    let mut graph = Graph::new();
    graph.add_inference(inference("inf_1", vec![premise], conclusion));

    let result = propagate(
        &graph,
        Trigger {
            revision_id: premise,
            event: InvalidatingEvent::Refuted,
        },
        &BTreeMap::new(),
        None,
    );
    assert_eq!(result.dependents, vec![conclusion]);
    assert_eq!(result.marks.len(), 1);
    assert!(result.tasks.is_empty());
    assert!(
        result
            .findings
            .iter()
            .any(|line| line.contains("aucune tâche")),
        "le manque de tâches n'est pas signalé"
    );
}

#[test]
fn a_cycle_terminates() {
    // Un graphe épistémique CONTIENT des cycles — deux claims qui se soutiennent mutuellement, une
    // définition qui s'appuie sur un cas qui l'instancie. Une propagation qui ne les supporterait
    // pas boucherait au premier corpus réel.
    let mut rng = Rng::new(305);
    let first = rng.revision();
    let second = rng.revision();
    let third = rng.revision();
    let mut graph = Graph::new();
    graph.add_inference(inference("inf_1", vec![first], second));
    graph.add_inference(inference("inf_2", vec![second], third));
    graph.add_inference(inference("inf_3", vec![third], first));

    let result = propagate(
        &graph,
        Trigger {
            revision_id: first,
            event: InvalidatingEvent::Refuted,
        },
        &BTreeMap::new(),
        Some(&policy()),
    );
    // Deux dépendants, pas une boucle infinie — et l'objet invalidé ne se remarque pas lui-même
    // en repassant par le cycle.
    let mut found = result.dependents.clone();
    found.sort_unstable();
    let mut expected = vec![second, third];
    expected.sort_unstable();
    assert_eq!(found, expected);
    assert!(!result.dependents.contains(&first));
}

#[test]
fn citing_a_refuted_object_does_not_invalidate_the_citer() {
    // La liste des relations de dépendance est courte exprès. Citer un article réfuté ne rend pas
    // l'article citant faux : ça le rend discutable. Marquer tout le corpus citant à chaque
    // rétractation noierait les vrais dépendants.
    let mut rng = Rng::new(306);
    let source = rng.revision();
    let citer = rng.revision();
    let dependent = rng.revision();
    let mut graph = Graph::new();
    graph.add_relation(relation("rel_1", citer, source, RelationKind::Cites));
    graph.add_relation(relation(
        "rel_2",
        dependent,
        source,
        RelationKind::DependsOn,
    ));

    let result = propagate(
        &graph,
        Trigger {
            revision_id: source,
            event: InvalidatingEvent::Refuted,
        },
        &BTreeMap::new(),
        Some(&policy()),
    );
    assert_eq!(result.dependents, vec![dependent]);
    assert!(!DEPENDENCY_RELATIONS.contains(&RelationKind::Cites));
    assert!(DEPENDENCY_RELATIONS.contains(&RelationKind::DependsOn));
}

#[test]
fn one_premise_of_three_is_enough_to_mark_the_conclusion() {
    // Le lien avec W1.e : l'hyperarête fait tomber la conclusion dès qu'UNE prémisse est réfutée.
    // Sur trois arêtes indépendantes, il en resterait deux et la conclusion paraîtrait tenir.
    let mut rng = Rng::new(307);
    let premises: Vec<RevisionId> = (0..3).map(|_| rng.revision()).collect();
    let conclusion = rng.revision();
    let mut graph = Graph::new();
    graph.add_inference(inference("inf_1", premises.clone(), conclusion));

    for premise in &premises {
        let result = propagate(
            &graph,
            Trigger {
                revision_id: *premise,
                event: InvalidatingEvent::Refuted,
            },
            &BTreeMap::new(),
            Some(&policy()),
        );
        assert_eq!(result.dependents, vec![conclusion]);
    }
}

// ————————————————————————— La politique par type — §8.2 —————————————————————————

#[test]
fn a_policy_without_minimal_evidence_or_invalidating_events_is_reported() {
    let empty = TypePolicy {
        minimal_evidence: Vec::new(),
        invalidating_events: BTreeSet::new(),
        ..policy()
    };
    let findings = empty.findings();
    assert!(findings.iter().any(|line| line.contains("preuve minimale")));
    // Sans événement invalidant, rien ne déclenche jamais la propagation, et les dépendants d'une
    // prémisse réfutée resteraient tels quels sans que personne ne le décide.
    assert!(findings.iter().any(|line| line.contains("§8.3")));
    assert_eq!(policy().findings(), Vec::<String>::new());
}

#[test]
fn a_level_declared_inapplicable_is_not_reachable() {
    // §8.1 : « ces niveaux ne forment pas toujours une chaîne totale. Une interprétation historique
    // peut atteindre L3 et L6 sans être *reproduite* au sens expérimental. »
    let historical = TypePolicy {
        discipline: "histoire".to_owned(),
        inapplicable_levels: ["reproduced".to_owned()].into_iter().collect(),
        ..policy()
    };
    assert!(!historical.is_applicable(ValidationLevel::Reproduced));
    assert!(historical.is_applicable(ValidationLevel::IndependentlyReviewed));
    assert!(historical.is_applicable(ValidationLevel::InstitutionallyAccepted));
    assert_eq!(historical.findings(), Vec::<String>::new());

    // Et une politique qui déclarerait un niveau inapplicable tout en lui donnant une condition de
    // promotion se contredirait.
    let contradictory = TypePolicy {
        inapplicable_levels: ["independently_reviewed".to_owned()].into_iter().collect(),
        ..policy()
    };
    assert!(
        contradictory
            .findings()
            .iter()
            .any(|line| line.contains("inapplicable"))
    );
}

#[test]
fn no_function_here_takes_or_averages_a_confidence() {
    // §8.4 : « les scores de confiance des agents sont des métadonnées de calibration. Ils ne
    // remplacent ni les preuves, ni les revues, ni les niveaux de validation. Une moyenne de
    // confiance ne constitue jamais une procédure de décision par défaut. »
    for source in [
        include_str!("../src/propagation.rs"),
        include_str!("../src/policy.rs"),
    ] {
        for forbidden in ["confidence", "mean_", "average", "fn score"] {
            assert!(!source.contains(forbidden), "`{forbidden}` dans le crate");
        }
    }
}
