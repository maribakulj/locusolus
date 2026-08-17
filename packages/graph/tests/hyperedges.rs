//! « Une inférence à 3 prémisses n'est pas 3 liens » — le test de sortie de W1.e.

use locus_domain::RevisionId;
use locus_graph::{
    Direction, FormalizationStatus, Graph, Inference, ObjectionTarget, Relation, RelationKind,
    Strength, Support,
};
use locus_protocol::Timestamp;

/// Un générateur congruentiel linéaire. Même choix que dans les paquets voisins.
struct Rng(u64);

impl Rng {
    const fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    fn revision(&mut self) -> RevisionId {
        let mut entropy = [0u8; 10];
        for byte in &mut entropy {
            *byte = u8::try_from(self.next() >> 56).unwrap_or(0);
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
        rule: "modus ponens généralisé".to_owned(),
        scope: "solvants polaires, 20–25 °C".to_owned(),
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
        strength: Strength::new(0.8).expect("force bornée"),
        justification: "mesures concordantes".to_owned(),
        evidence_refs: Vec::new(),
        revision: 1,
    }
}

// ————————————————————————— Le test de sortie de W1.e —————————————————————————

#[test]
fn three_premises_make_one_inference_not_three_links() {
    // §7.6 : « le système NE DOIT PAS réduire un raisonnement multi-prémisses à plusieurs arêtes
    // indépendantes. » Le test compare les deux graphes qu'on pourrait construire, et montre ce
    // que le second perd.
    let mut rng = Rng::new(201);
    let premises: Vec<RevisionId> = (0..3).map(|_| rng.revision()).collect();
    let conclusion = rng.revision();

    let mut hyper = Graph::new();
    hyper.add_inference(inference("inf_1", premises.clone(), conclusion));

    // Ce qu'aurait donné la réduction interdite : trois `supports` indépendants.
    let mut flattened = Graph::new();
    for (index, premise) in premises.iter().enumerate() {
        flattened.add_relation(relation(
            &format!("rel_{index}"),
            *premise,
            conclusion,
            RelationKind::Supports,
        ));
    }

    // 1. Le compte. Une hyperarête, pas trois arêtes.
    assert_eq!(hyper.inference_count(), 1);
    assert_eq!(hyper.relation_count(), 0);
    assert_eq!(flattened.relation_count(), 3);

    // 2. Ce qui étaye la conclusion : UN support portant les trois prémisses.
    let supports = hyper.supports_of(&conclusion);
    assert_eq!(supports.len(), 1, "trois soutiens au lieu d'un");
    match supports.first().expect("un support") {
        Support::Inference(found) => {
            assert_eq!(found.arity(), 3);
            assert!(found.is_multi_premise());
            assert_eq!(found.premise_ids, premises);
        }
        Support::Relation(_) => panic!("l'inférence a été rendue comme une relation binaire"),
    }
    // Le graphe aplati en rend trois, et c'est exactement l'erreur.
    assert_eq!(flattened.supports_of(&conclusion).len(), 3);

    // 3. Les prémisses minimales — §9.4. UN ensemble de trois, pas trois ensembles d'un.
    // « Il faut ces trois faits » contre « il suffit d'un des trois » : ce n'est pas la même
    // affirmation scientifique.
    let sets = hyper.minimal_premise_sets(&conclusion);
    assert_eq!(sets, vec![premises.clone()]);
    assert_eq!(sets.len(), 1);
    assert_eq!(sets[0].len(), 3);
    assert!(flattened.minimal_premise_sets(&conclusion).is_empty());

    // 4. Réfuter UNE prémisse casse l'inférence entière.
    for premise in &premises {
        let broken = hyper.inferences_broken_by(premise);
        assert_eq!(broken.len(), 1, "une prémisse réfutée ne casse pas le tout");
        assert_eq!(broken[0].id, "inf_1");
    }
    // Sur le graphe aplati, réfuter une prémisse laisse deux `supports` debout, et la conclusion
    // paraît encore soutenue aux deux tiers.
    let survivors = flattened
        .incoming(&conclusion)
        .into_iter()
        .filter(|edge| edge.from != premises[0])
        .count();
    assert_eq!(survivors, 2, "c'est précisément ce que §7.6 interdit");

    // 5. La règle et le scope ont un endroit où être contestés. Sur trois arêtes, ils n'existent
    // pas — et l'objection « le raisonnement ne tient pas, même si tous les faits sont vrais »
    // n'aurait aucune cible.
    let targets = hyper
        .supports_of(&conclusion)
        .into_iter()
        .find_map(|support| match support {
            Support::Inference(found) => Some(found.objection_targets()),
            Support::Relation(_) => None,
        })
        .expect("une inférence");
    assert!(targets.contains(&ObjectionTarget::Rule));
    assert!(targets.contains(&ObjectionTarget::Scope));
    assert!(targets.contains(&ObjectionTarget::Inference));
    assert_eq!(targets.len(), 3 + premises.len());
    for premise in &premises {
        assert!(targets.contains(&ObjectionTarget::Premise {
            revision_id: *premise
        }));
    }
}

#[test]
fn the_graph_offers_no_way_to_flatten_an_inference() {
    // La garantie se tient par l'absence : c'est exactement la fonction de commodité que quelqu'un
    // finira par vouloir écrire, et le jour venu ce test le lui rappellera avant la revue.
    let graph = include_str!("../src/graph.rs");
    let inference = include_str!("../src/inference.rs");
    for forbidden in [
        "fn flatten",
        "fn decompose",
        "fn as_edges",
        "fn to_relations",
        "fn explode",
    ] {
        assert!(!graph.contains(forbidden), "`{forbidden}` dans le graphe");
        assert!(
            !inference.contains(forbidden),
            "`{forbidden}` dans l'inférence"
        );
    }
    // Et aucune conversion implicite non plus.
    assert!(!inference.contains("impl From<Inference>"));
}

#[test]
fn one_premise_is_still_an_inference_not_a_relation() {
    // Le cas limite : une inférence à une seule prémisse ressemble à une arête. Elle n'en est pas
    // une — elle porte une règle et un scope, donc des cibles d'objection qu'une arête n'a pas.
    let mut rng = Rng::new(202);
    let premise = rng.revision();
    let conclusion = rng.revision();
    let mut graph = Graph::new();
    graph.add_inference(inference("inf_1", vec![premise], conclusion));

    let found = graph.minimal_premise_sets(&conclusion);
    assert_eq!(found, vec![vec![premise]]);
    assert_eq!(graph.relation_count(), 0);
    let supports = graph.supports_of(&conclusion);
    match supports.first().expect("un support") {
        Support::Inference(inference) => {
            assert!(!inference.is_multi_premise());
            assert_eq!(inference.objection_targets().len(), 4);
        }
        Support::Relation(_) => panic!("une inférence rendue comme relation"),
    }
}

#[test]
fn two_inferences_for_one_conclusion_are_two_alternatives() {
    // Deux inférences distinctes qui concluent la même chose sont deux chemins **alternatifs** :
    // réfuter l'une laisse l'autre. C'est le cas que la réduction en arêtes rend indistinguable
    // d'une seule inférence à quatre prémisses.
    let mut rng = Rng::new(203);
    let conclusion = rng.revision();
    let first: Vec<RevisionId> = (0..2).map(|_| rng.revision()).collect();
    let second: Vec<RevisionId> = (0..2).map(|_| rng.revision()).collect();

    let mut graph = Graph::new();
    graph.add_inference(inference("inf_1", first.clone(), conclusion));
    graph.add_inference(inference("inf_2", second.clone(), conclusion));

    let sets = graph.minimal_premise_sets(&conclusion);
    assert_eq!(sets.len(), 2, "deux chemins, pas un");
    assert!(sets.contains(&first));
    assert!(sets.contains(&second));
    // Réfuter une prémisse du premier ne casse que le premier.
    let broken = graph.inferences_broken_by(&first[0]);
    assert_eq!(broken.len(), 1);
    assert_eq!(broken[0].id, "inf_1");
}

// ————————————————————————— Relations typées — §7.5 —————————————————————————

#[test]
fn an_asymmetric_relation_is_never_traversable_backwards() {
    // §7.5 : « les relations non symétriques ne doivent pas être inférées en sens inverse ».
    // `A supports B` lu à l'envers ferait de la preuve une thèse ; `cites` ferait citer un article
    // de 2026 par un article de 1890.
    let mut rng = Rng::new(204);
    let from = rng.revision();
    let to = rng.revision();

    let mut one_way = 0;
    for kind in RelationKind::ALL {
        let edge = relation("rel", from, to, kind);
        let backwards = Graph::traversable_backwards(&edge);
        match kind.direction() {
            Direction::OneWay => {
                assert_eq!(backwards, None, "`{kind}` se laisse retourner");
                one_way += 1;
            }
            Direction::Symmetric => assert_eq!(backwards, Some(kind)),
            Direction::Converse(other) => assert_eq!(backwards, Some(other)),
        }
    }
    // Vingt-deux relations sur vingt-huit refusent l'inversion : deux sont symétriques, et quatre
    // forment deux paires de réciproques nommées.
    assert_eq!(one_way, 22);
    assert_eq!(RelationKind::ALL.len(), 28);
}

#[test]
fn the_named_converses_come_in_pairs() {
    // Une réciproque nommée doit se retrouver elle-même en deux sauts. Une paire dépareillée ferait
    // deux relations qui se renvoient à une troisième, ce qui n'a aucun sens.
    for kind in RelationKind::ALL {
        if let Direction::Converse(other) = kind.direction() {
            assert_eq!(
                other.direction(),
                Direction::Converse(kind),
                "`{kind}` et `{other}` ne sont pas réciproques l'une de l'autre"
            );
            assert_ne!(kind, other, "une réciproque de soi est une symétrie");
        }
    }
    assert_eq!(
        RelationKind::Generalizes.converse(),
        Some(RelationKind::Specializes)
    );
    assert_eq!(
        RelationKind::ForkedFrom.converse(),
        Some(RelationKind::MergedInto)
    );
}

#[test]
fn a_symmetric_relation_is_its_own_converse() {
    for kind in [RelationKind::Contradicts, RelationKind::AnalogousTo] {
        assert_eq!(kind.direction(), Direction::Symmetric);
        assert_eq!(kind.converse(), Some(kind));
    }
    // `supports` n'est PAS symétrique : deux thèses qui s'étayent mutuellement sont deux relations
    // écrites, pas une relation lue deux fois.
    assert_eq!(RelationKind::Supports.direction(), Direction::OneWay);
    assert_eq!(RelationKind::Supports.converse(), None);
}

#[test]
fn the_twenty_eight_core_relations_are_all_there() {
    assert_eq!(RelationKind::ALL.len(), 28);
    let mut names: Vec<&str> = RelationKind::ALL.iter().map(|kind| kind.as_str()).collect();
    names.sort_unstable();
    let count = names.len();
    names.dedup();
    assert_eq!(names.len(), count, "une relation en double");
    for name in ["depends_on", "refutes", "transfers_to", "anchored_in"] {
        assert!(RelationKind::parse(name).is_some(), "{name} absente");
    }
    assert!(RelationKind::parse("depends-on").is_none());
}

#[test]
fn a_relation_carries_what_makes_it_an_object() {
    // §7.5 : « une relation est un objet versionné avec auteur, scope, force, justification et
    // evidence refs ». Les cinq, plus la révision : une relation se conteste et se cite comme le
    // reste, ce qu'une arête anonyme ne permettrait pas.
    let mut rng = Rng::new(205);
    let edge = relation(
        "rel_1",
        rng.revision(),
        rng.revision(),
        RelationKind::Supports,
    );
    assert!(!edge.author.is_empty());
    assert!(!edge.scope.is_empty());
    assert!(!edge.justification.is_empty());
    assert_eq!(edge.revision, 1);
    assert!((edge.strength.value() - 0.8).abs() < f64::EPSILON);

    // Une force hors bornes n'est pas une force forte : c'est un chiffre dont personne ne sait ce
    // qu'il mesure.
    assert!(Strength::new(1.5).is_none());
    assert!(Strength::new(-0.1).is_none());
    assert!(Strength::new(f64::NAN).is_none());
    assert!(Strength::new(0.0).is_some());
    assert!(Strength::new(1.0).is_some());
}

// ————————————————————————— Ce qu'une inférence doit porter —————————————————————————

#[test]
fn an_inference_without_a_rule_or_a_scope_is_reported() {
    let mut rng = Rng::new(206);
    let sound = inference("inf_1", vec![rng.revision()], rng.revision());
    assert_eq!(sound.findings().0, Vec::<String>::new());

    let ruleless = Inference {
        rule: "   ".to_owned(),
        ..sound.clone()
    };
    assert!(ruleless.findings().0.iter().any(|f| f.contains("règle")));

    // Sans domaine de validité, l'objection « la règle ne vaut pas ici » n'a pas de cible.
    let scopeless = Inference {
        scope: String::new(),
        ..sound.clone()
    };
    assert!(scopeless.findings().0.iter().any(|f| f.contains("scope")));

    let premiseless = Inference {
        premise_ids: Vec::new(),
        ..sound.clone()
    };
    assert!(
        premiseless
            .findings()
            .0
            .iter()
            .any(|f| f.contains("prémisse"))
    );

    // Une prémisse répétée ne compte pas deux fois.
    let premise = rng.revision();
    let repeated = Inference {
        premise_ids: vec![premise, premise],
        ..sound
    };
    assert!(repeated.findings().0.iter().any(|f| f.contains("répétée")));
}

#[test]
fn assumptions_are_not_premises() {
    // §7.6 les sépare, et la séparation porte : une prémisse est affirmée, une hypothèse est
    // admise. Les confondre ferait passer pour établi ce qui a seulement été supposé.
    let mut rng = Rng::new(207);
    let premise = rng.revision();
    let assumption = rng.revision();
    let conclusion = rng.revision();
    let mut graph = Graph::new();
    graph.add_inference(Inference {
        assumption_ids: vec![assumption],
        ..inference("inf_1", vec![premise], conclusion)
    });

    // Les prémisses minimales ne contiennent pas les hypothèses.
    assert_eq!(graph.minimal_premise_sets(&conclusion), vec![vec![premise]]);
    // Et réfuter une hypothèse ne casse pas l'inférence par le même chemin qu'une prémisse : le
    // graphe ne prétend pas le contraire.
    assert!(graph.inferences_broken_by(&assumption).is_empty());
    assert_eq!(graph.inferences_broken_by(&premise).len(), 1);
}

#[test]
fn an_inference_round_trips_through_json() {
    let mut rng = Rng::new(208);
    let original = inference(
        "inf_1",
        (0..3).map(|_| rng.revision()).collect(),
        rng.revision(),
    );
    let text = serde_json::to_string(&original).expect("sérialisable");
    let back: Inference = serde_json::from_str(&text).expect("relisible");
    assert_eq!(original, back);
    // Les trois prémisses restent dans un seul champ, dans l'ordre.
    assert_eq!(back.premise_ids.len(), 3);
    assert_eq!(back.premise_ids, original.premise_ids);
}
