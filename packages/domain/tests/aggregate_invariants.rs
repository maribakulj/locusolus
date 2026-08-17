//! Property tests sur les agrégats de §7.1 et les objets de §7.3 — le test de sortie de W1.b.
//!
//! Même générateur qu'en W1.a, et pour les mêmes raisons : déterministe, sans dépendance, un échec
//! se rejoue en relançant.

use locus_domain::{
    Branch, BranchState, Condition, CoreObjectType, ObjectType, Origin, ParseObjectTypeError,
    RevisionId, TaskState, TransitionError, ValidationWitness, implies_validated_claims,
    transition,
};
use locus_protocol::Timestamp;

/// Un générateur congruentiel linéaire. Voir la note de `envelope_invariants.rs`.
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

    fn below(&mut self, bound: usize) -> usize {
        usize::try_from(self.next() >> 33).unwrap_or(0) % bound
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

fn branch(rng: &mut Rng, state: BranchState) -> Branch {
    let origin = if rng.below(2) == 0 {
        Origin::Root
    } else {
        Origin::Fork {
            branch_id: "br_origine".to_owned(),
            revision: rng.revision(),
        }
    };
    Branch {
        id: "br_1".to_owned(),
        workstream_id: "wsm_1".to_owned(),
        title: "titre".to_owned(),
        objective: "objectif".to_owned(),
        origin,
        head_revision: rng.revision(),
        state,
        revision: u32::try_from(rng.below(20)).unwrap_or(0),
    }
}

fn complete_witness() -> ValidationWitness {
    ValidationWitness {
        policy_id: "pol_1".to_owned(),
        conditions: vec![
            Condition {
                statement: "au moins une revue indépendante".to_owned(),
                satisfied: true,
            },
            Condition {
                statement: "aucune objection ouverte".to_owned(),
                satisfied: true,
            },
        ],
    }
}

const CASES: usize = 300;

// ————————————————————————————————— §7.1, la branche —————————————————————————————————

#[test]
fn merged_is_terminal_except_by_an_explicit_reopen() {
    // §7.1, invariant 3. Une transition ordinaire depuis `merged` serait une réouverture qui ne
    // dit pas son nom, et le texte demande qu'elle soit explicite.
    let mut rng = Rng::new(11);
    for _ in 0..CASES {
        let merged = branch(&mut rng, BranchState::Merged);
        for target in BranchState::ALL {
            if target == BranchState::Merged {
                continue;
            }
            assert_eq!(
                merged.transition(target).unwrap_err(),
                TransitionError::MergedIsTerminal,
                "sortie de `merged` vers `{target}` sans reopen"
            );
        }
        // Et `reopen` en sort — c'est la seule porte, et elle porte son nom.
        let reopened = merged.reopen(BranchState::Exploring);
        assert_eq!(reopened.state, BranchState::Exploring);
        assert_eq!(reopened.revision, merged.revision + 1);
    }
}

#[test]
fn a_branch_is_never_validated_without_a_complete_witness() {
    // §7.1, invariant 4. Les conditions viennent d'une politique que ce crate ne connaît pas ; ce
    // qu'il garantit, c'est qu'on ne passe pas à `validated` sans avoir répondu à la question.
    let mut rng = Rng::new(12);
    for _ in 0..CASES {
        let current = branch(&mut rng, BranchState::Substantiated);

        // Par la voie ordinaire : refusé, faute de témoin.
        assert_eq!(
            current.transition(BranchState::Validated).unwrap_err(),
            TransitionError::ValidationWitnessMissing
        );

        // Avec une condition non satisfaite : refusé, et le refus la nomme.
        let partial = ValidationWitness {
            policy_id: "pol_1".to_owned(),
            conditions: vec![
                Condition {
                    statement: "au moins une revue indépendante".to_owned(),
                    satisfied: true,
                },
                Condition {
                    statement: "aucune objection ouverte".to_owned(),
                    satisfied: false,
                },
            ],
        };
        match current.validate(&partial).unwrap_err() {
            TransitionError::ValidationConditionsUnmet { unmet } => {
                assert_eq!(unmet, vec!["aucune objection ouverte".to_owned()]);
            }
            other => panic!("refus inattendu : {other}"),
        }

        // Témoin vide : refusé aussi. « Aucune condition » n'est pas « toutes satisfaites ».
        let empty = ValidationWitness {
            policy_id: "pol_1".to_owned(),
            conditions: Vec::new(),
        };
        assert!(current.validate(&empty).is_err());

        // Témoin complet : accepté.
        let validated = current
            .validate(&complete_witness())
            .expect("témoin complet");
        assert_eq!(validated.state, BranchState::Validated);
    }
}

#[test]
fn a_merged_branch_cannot_be_validated_either() {
    // Le croisement des invariants 3 et 4 : un témoin complet ne rouvre pas `merged`.
    let mut rng = Rng::new(13);
    let merged = branch(&mut rng, BranchState::Merged);
    assert_eq!(
        merged.validate(&complete_witness()).unwrap_err(),
        TransitionError::MergedIsTerminal
    );
}

#[test]
fn a_fork_carries_its_origin_revision_or_none_at_all() {
    // §7.1, invariant 2 : « un fork référence exactement la révision d'origine ». Les deux champs
    // ne se remplissent jamais l'un sans l'autre — une branche qui saurait de quelle branche elle
    // est issue sans savoir à quelle révision aurait un point de départ qui bouge.
    let mut rng = Rng::new(14);
    for _ in 0..CASES {
        let current = branch(&mut rng, BranchState::Exploring);
        match &current.origin {
            Origin::Root => assert!(current.fork_revision().is_none()),
            Origin::Fork {
                branch_id,
                revision,
            } => {
                assert!(!branch_id.is_empty());
                assert_eq!(current.fork_revision(), Some(revision));
            }
        }
    }
}

#[test]
fn a_branch_keeps_exactly_one_head_across_every_transition() {
    // §7.1, invariant 1. Le type le garantit — `head_revision` est un identifiant, pas une
    // collection — et ce test vérifie qu'aucune transition ne le déplace au passage.
    let mut rng = Rng::new(15);
    for _ in 0..CASES {
        let before = branch(&mut rng, BranchState::Exploring);
        for target in BranchState::ALL {
            if target == BranchState::Validated {
                continue;
            }
            let after = before.transition(target).expect("transition permise");
            assert_eq!(after.head_revision, before.head_revision, "le head a bougé");
        }
        let validated = before
            .validate(&complete_witness())
            .expect("témoin complet");
        assert_eq!(validated.head_revision, before.head_revision);
    }
}

#[test]
fn archiving_deletes_nothing() {
    // §7.1, invariant 5 : « `archived` ne supprime aucun objet ». La garantie se teste par
    // l'absence : archiver ne change que l'état et le rang de révision.
    let mut rng = Rng::new(16);
    for _ in 0..CASES {
        let before = branch(&mut rng, BranchState::Suspended);
        let archived = before
            .transition(BranchState::Archived)
            .expect("transition permise");
        assert_eq!(archived.state, BranchState::Archived);
        assert_eq!(archived.head_revision, before.head_revision);
        assert_eq!(archived.origin, before.origin);
        assert_eq!(archived.id, before.id);
        assert_eq!(archived.title, before.title);
        assert_eq!(archived.revision, before.revision + 1);
    }
}

// ————————————————————————————————— §7.1, la tâche —————————————————————————————————

#[test]
fn every_arrow_of_the_task_graph_is_the_one_the_text_draws() {
    // Les flèches de §7.1, une par une. Une flèche absente est une transition interdite, et c'est
    // ce qui rend la table utile : elle attrape ce que personne n'a autorisé.
    for (from, to) in [
        (TaskState::Proposed, TaskState::Queued),
        (TaskState::Queued, TaskState::Leased),
        (TaskState::Leased, TaskState::Running),
        (TaskState::Running, TaskState::WaitingForTool),
        (TaskState::Running, TaskState::WaitingForHuman),
        (TaskState::Running, TaskState::WaitingForReview),
        (TaskState::Running, TaskState::Succeeded),
        (TaskState::Running, TaskState::Failed),
        (TaskState::Running, TaskState::Cancelled),
        (TaskState::Running, TaskState::TimedOut),
        (TaskState::Leased, TaskState::Orphaned),
        (TaskState::Running, TaskState::Orphaned),
        (TaskState::Orphaned, TaskState::Queued),
        (TaskState::Succeeded, TaskState::Accepted),
        (TaskState::Succeeded, TaskState::Rejected),
        (TaskState::Succeeded, TaskState::Superseded),
    ] {
        assert!(transition(from, to).is_ok(), "{from} → {to} refusée");
    }

    // Et quelques flèches que le texte ne dessine pas.
    for (from, to) in [
        (TaskState::Proposed, TaskState::Running),
        (TaskState::Queued, TaskState::Succeeded),
        (TaskState::WaitingForHuman, TaskState::Succeeded),
        (TaskState::Failed, TaskState::Running),
        (TaskState::Accepted, TaskState::Rejected),
    ] {
        let refusal = transition(from, to).unwrap_err();
        assert_eq!(refusal.from, from);
        assert_eq!(refusal.to, to);
        // Le refus nomme les sorties possibles plutôt que d'envoyer relire un diagramme.
        assert_eq!(refusal.allowed, from.allowed().to_vec());
    }
}

#[test]
fn a_wait_returns_to_running_and_never_jumps_to_a_verdict() {
    // Sauter de l'attente au résultat serait rendre un verdict sans avoir repris le travail qu'on
    // avait suspendu.
    for waiting in [
        TaskState::WaitingForTool,
        TaskState::WaitingForHuman,
        TaskState::WaitingForReview,
    ] {
        assert_eq!(waiting.allowed(), &[TaskState::Running]);
        for target in TaskState::ALL {
            if target != TaskState::Running {
                assert!(transition(waiting, target).is_err(), "{waiting} → {target}");
            }
        }
    }
}

#[test]
fn succeeded_never_implies_validated_claims() {
    // §7.1 : « une tâche `succeeded` signifie que le worker a rempli son contrat technique. Elle ne
    // signifie pas que ses claims sont validés. » La réponse est écrite quelque part plutôt que
    // sous-entendue, et le jour où quelqu'un voudra la changer, le diff le montrera.
    for state in TaskState::ALL {
        assert!(!implies_validated_claims(state), "{state}");
    }
    // `succeeded` n'est pas terminal : le verdict institutionnel reste à rendre, et il n'est pas
    // automatique.
    assert!(!TaskState::Succeeded.is_terminal());
    assert_eq!(
        TaskState::Succeeded.allowed(),
        &[
            TaskState::Accepted,
            TaskState::Rejected,
            TaskState::Superseded
        ]
    );
}

#[test]
fn an_orphan_goes_back_to_the_queue_and_nowhere_else() {
    // `leased/running → orphaned → queued` : la tâche repart, et elle repart par la file.
    assert_eq!(TaskState::Orphaned.allowed(), &[TaskState::Queued]);
    assert!(TaskState::Leased.can_reach(TaskState::Orphaned));
    assert!(TaskState::Running.can_reach(TaskState::Orphaned));
    // Un orphelin ne se remet pas directement à courir : il repasse par une attribution.
    assert!(transition(TaskState::Orphaned, TaskState::Running).is_err());
}

#[test]
fn terminal_states_are_named_not_deduced() {
    // Les nommer évite de les déduire d'une liste vide, et rend un ajout futur visible.
    let terminal: Vec<TaskState> = TaskState::ALL
        .into_iter()
        .filter(|state| state.is_terminal())
        .collect();
    assert_eq!(
        terminal,
        vec![
            TaskState::Failed,
            TaskState::Cancelled,
            TaskState::TimedOut,
            TaskState::Accepted,
            TaskState::Rejected,
            TaskState::Superseded,
        ]
    );
}

// ————————————————————————————————— §7.3, les objets —————————————————————————————————

#[test]
fn the_forty_core_types_are_all_there_without_duplicates() {
    assert_eq!(CoreObjectType::ALL.len(), 40);
    let mut names: Vec<&str> = CoreObjectType::ALL
        .iter()
        .map(|kind| kind.as_str())
        .collect();
    names.sort_unstable();
    let count = names.len();
    names.dedup();
    assert_eq!(names.len(), count, "un type core en double");

    // Quelques-uns nommément, dont ceux que l'invariant 12 protège.
    for name in [
        "Claim",
        "Inference",
        "NegativeResult",
        "Conflict",
        "Counterexample",
        "TransferCertificate",
    ] {
        assert!(CoreObjectType::parse(name).is_some(), "{name} absent");
    }
}

#[test]
fn an_extension_can_never_shadow_a_core_type() {
    // §7.3 : « les extensions ne doivent pas modifier la signification des types core ». Un pack
    // qui déclarerait son propre `Claim` ne modifierait pas le type core — il le remplacerait,
    // silencieusement, et le graphe contiendrait deux notions qu'aucune lecture ne saurait séparer.
    for kind in CoreObjectType::ALL {
        let shadow = format!("mondepack/{}", kind.as_str());
        assert_eq!(
            ObjectType::parse(&shadow).unwrap_err(),
            ParseObjectTypeError::ShadowsCoreType,
            "{shadow} accepté"
        );
    }

    // Une extension d'un autre nom passe, et se distingue d'un type core.
    let extension = ObjectType::parse("chimie/Spectre").expect("extension bien formée");
    assert!(!extension.is_core());
    assert_eq!(extension.to_string(), "chimie/Spectre");
}

#[test]
fn a_type_name_round_trips_through_json() {
    let mut rng = Rng::new(17);
    for _ in 0..CASES {
        let kind = CoreObjectType::ALL[rng.below(CoreObjectType::ALL.len())];
        let core = ObjectType::Core(kind);
        let text = serde_json::to_string(&core).expect("sérialisable");
        let back: ObjectType = serde_json::from_str(&text).expect("relisible");
        assert_eq!(core, back);
        assert!(back.is_core());
    }

    let extension = ObjectType::parse("physique/Observation").expect("bien formée");
    let text = serde_json::to_string(&extension).expect("sérialisable");
    assert_eq!(
        serde_json::from_str::<ObjectType>(&text).expect("relisible"),
        extension
    );
}

#[test]
fn a_malformed_type_name_is_refused_rather_than_guessed() {
    assert_eq!(
        ObjectType::parse("Claimm").unwrap_err(),
        ParseObjectTypeError::UnknownCoreType
    );
    assert_eq!(
        ObjectType::parse("/Spectre").unwrap_err(),
        ParseObjectTypeError::Empty
    );
    assert_eq!(
        ObjectType::parse("chimie/").unwrap_err(),
        ParseObjectTypeError::Empty
    );
    // La casse compte : `claim` n'est pas `Claim`, et le normaliser inventerait une équivalence.
    assert!(ObjectType::parse("claim").is_err());
}
