//! « Reconstruction depuis zéro = état courant » — le test de sortie de W1.d.

use locus_event_store::{
    Actor, ActorKind, Append, Draft, EventStore, EventType, Expected, MemoryEventStore,
};
use locus_projections::{
    ConflictRegistry, Health, Projection, ProjectionRunner, ValidationState, verify,
};
use locus_protocol::{
    Id, Timestamp,
    id::{Agent, Command, Event, Project, Workspace},
};

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

    fn below(&mut self, bound: usize) -> usize {
        usize::try_from(self.next() >> 33).unwrap_or(0) % bound
    }

    fn id<K: locus_protocol::IdKind>(&mut self) -> Id<K> {
        let mut entropy = [0u8; 10];
        for byte in &mut entropy {
            *byte = u8::try_from(self.next() >> 56).unwrap_or(0);
        }
        Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
            .expect("instant dans les bornes")
    }
}

fn recorded() -> Timestamp {
    Timestamp::from_millis(1_700_000_100_000)
}

fn draft(rng: &mut Rng, stream: &str, kind: &str, payload: serde_json::Value) -> Draft {
    Draft {
        event_id: rng.id::<Event>(),
        event_type: EventType::parse(kind).expect("type valide"),
        schema_version: 1,
        stream_id: stream.to_owned(),
        workspace_id: rng.id::<Workspace>(),
        project_id: rng.id::<Project>(),
        program_id: None,
        branch_id: None,
        actor: Actor {
            principal_id: rng.id::<Agent>(),
            kind: ActorKind::Agent,
            delegation_id: None,
        },
        occurred_at: Timestamp::from_millis(1_700_000_000_000),
        causation_id: rng.id::<Command>(),
        correlation_id: None,
        trace_id: None,
        payload,
        payload_hash: format!("sha256:{}", "ab".repeat(32)),
    }
}

/// Écrire un événement dans le journal, en lisant la révision courante d'abord.
///
/// La lecture préalable est ce que `Expected` sans variante « peu importe » impose (ADR 0012). Le
/// test la fait donc aussi — et c'est bien ainsi : un test qui contournerait la contrainte ne
/// testerait pas le système livré.
fn push(
    store: &mut MemoryEventStore,
    rng: &mut Rng,
    stream: &str,
    kind: &str,
    payload: serde_json::Value,
) {
    let event = draft(rng, stream, kind, payload);
    let command_id = rng.id::<Command>();
    let expected = store
        .revision(stream)
        .map_or(Expected::NoStream, Expected::Exact);
    store
        .append(
            Append {
                stream_id: stream.to_owned(),
                expected,
                command_id,
                events: vec![event],
            },
            recorded(),
        )
        .expect("écriture permise");
}

const STATUSES: [&str; 5] = ["draft", "staged", "under_review", "contested", "validated"];
const LEVELS: [&str; 4] = [
    "unassessed",
    "traceable",
    "internally_checked",
    "independently_reviewed",
];

/// Un journal arbitraire mais bien formé : des objets épistémiques et des conflits.
fn populate(store: &mut MemoryEventStore, rng: &mut Rng, steps: usize) {
    let mut declared: Vec<String> = Vec::new();
    for step in 0..steps {
        let stream = format!("claim_{:02}", rng.below(8));
        if rng.below(3) == 0 {
            let conflict_id = format!("cfl_{step:03}");
            push(
                store,
                rng,
                &stream,
                "conflict.declared",
                serde_json::json!({ "conflict_id": conflict_id, "statement": "désaccord" }),
            );
            declared.push(conflict_id);
            continue;
        }
        // Un conflit déjà déclaré se résout parfois — et l'entrée reste, invariant 12.
        if rng.below(4) == 0 && !declared.is_empty() {
            let index = rng.below(declared.len());
            let conflict_id = declared[index].clone();
            push(
                store,
                rng,
                &stream,
                "conflict.resolved",
                serde_json::json!({ "conflict_id": conflict_id }),
            );
            continue;
        }
        let status = STATUSES[rng.below(STATUSES.len())];
        let level = LEVELS[rng.below(LEVELS.len())];
        push(
            store,
            rng,
            &stream,
            "epistemic_object.staged",
            serde_json::json!({ "status": status, "validation_level": level }),
        );
    }
}

// ————————————————————————— Le test de sortie de W1.d —————————————————————————

#[test]
fn a_rebuild_from_zero_equals_the_current_state() {
    // §9.5 : « une projection peut être détruite et reconstruite ». La propriété est ce qui rend une
    // projection **secondaire** : une projection qu'on ne saurait pas reconstruire serait une
    // seconde source de vérité, ce que §9.1 réserve au journal.
    for seed in 41..49 {
        let mut rng = Rng::new(seed);
        let mut store = MemoryEventStore::new();
        populate(&mut store, &mut rng, 60);

        // L'état courant, construit incrémentalement — quelques passages successifs, comme en
        // production où le pilote rattrape le journal par à-coups.
        let mut live = ProjectionRunner::new(ValidationState::new());
        live.catch_up(&store);
        populate(&mut store, &mut rng, 40);
        live.catch_up(&store);
        populate(&mut store, &mut rng, 20);
        let progress = live.catch_up(&store);
        assert_eq!(progress.health, Health::Healthy);

        // La reconstruction, depuis zéro, sur une projection neuve.
        let mut rebuilt = ProjectionRunner::new(ValidationState::new());
        rebuilt.rebuild(&store);

        assert_eq!(
            live.projection().checksum(),
            rebuilt.projection().checksum(),
            "reconstruction ≠ état courant (graine {seed})"
        );
        assert_eq!(
            live.projection().watermark(),
            rebuilt.projection().watermark()
        );
        assert_eq!(live.projection(), rebuilt.projection());
        // Et le watermark est celui du dernier événement du journal.
        assert_eq!(
            live.projection().watermark(),
            u64::try_from(store.export().len()).expect("journal borné")
        );
    }
}

#[test]
fn the_same_property_holds_for_the_conflict_registry() {
    // Deux projections, pour que la propriété porte sur le port et pas sur une implémentation.
    for seed in 51..55 {
        let mut rng = Rng::new(seed);
        let mut store = MemoryEventStore::new();
        populate(&mut store, &mut rng, 80);

        let mut live = ProjectionRunner::new(ConflictRegistry::new());
        live.catch_up(&store);
        populate(&mut store, &mut rng, 40);
        live.catch_up(&store);

        let mut rebuilt = ProjectionRunner::new(ConflictRegistry::new());
        rebuilt.rebuild(&store);
        assert_eq!(live.projection(), rebuilt.projection(), "graine {seed}");
    }
}

#[test]
fn destroying_and_rebuilding_the_same_runner_restores_it() {
    // La variante que §9.5 nomme littéralement : « détruite et reconstruite », sur place.
    let mut rng = Rng::new(61);
    let mut store = MemoryEventStore::new();
    populate(&mut store, &mut rng, 50);

    let mut runner = ProjectionRunner::new(ValidationState::new());
    runner.catch_up(&store);
    let before = runner.projection().checksum();
    assert!(!before.is_empty());

    runner.rebuild(&store);
    assert_eq!(runner.projection().checksum(), before);
}

#[test]
fn verify_agrees_when_nothing_diverges_and_names_what_does() {
    // §9.5 : « un outil compare événements et projections ». La reconstruction se fait sur une
    // copie neuve — une vérification qui détruirait ce qu'elle vérifie réparerait la divergence en
    // même temps qu'elle la découvrirait, ce qui est la définition d'une réparation silencieuse.
    let mut rng = Rng::new(71);
    let mut store = MemoryEventStore::new();
    populate(&mut store, &mut rng, 60);

    let mut live = ProjectionRunner::new(ValidationState::new());
    live.catch_up(&store);

    let report = verify(&live, ValidationState::new, &store);
    assert!(report.agrees(), "{:?}", report.findings());
    assert_eq!(report.findings(), Vec::<String>::new());
    assert_eq!(report.projection, "validation_state");

    // Une projection en retard diverge, et le rapport dit sur quoi.
    populate(&mut store, &mut rng, 20);
    let late = verify(&live, ValidationState::new, &store);
    assert!(!late.agrees());
    assert!(
        late.findings()
            .iter()
            .any(|line| line.contains("watermark"))
    );

    // Et la vérification n'a pas touché à la projection vérifiée.
    assert_eq!(live.projection().watermark(), report.live_watermark);
}

// ————————————————————————— Quarantaine — §9.5 —————————————————————————

#[test]
fn a_faulty_event_quarantines_the_projection_without_touching_the_journal() {
    // « Les erreurs de projection sont mises en quarantaine sans bloquer l'écriture canonique. »
    let mut rng = Rng::new(81);
    let mut store = MemoryEventStore::new();
    populate(&mut store, &mut rng, 20);

    let mut runner = ProjectionRunner::new(ValidationState::new());
    runner.catch_up(&store);
    let healthy_watermark = runner.projection().watermark();

    // Un événement épistémique sans `validation_level` : la projection ne l'invente pas.
    push(
        &mut store,
        &mut rng,
        "claim_99",
        "epistemic_object.staged",
        serde_json::json!({ "status": "validated" }),
    );

    let progress = runner.catch_up(&store);
    match &progress.health {
        Health::Quarantined { error } => {
            assert!(error.reason.contains("validation_level"));
            assert!(error.position > healthy_watermark);
        }
        Health::Healthy => panic!("la projection a accepté un événement inapplicable"),
    }

    // Le journal, lui, n'a rien perdu : la quarantaine ne bloque pas l'écriture canonique.
    let before = store.export().len();
    populate(&mut store, &mut rng, 10);
    assert!(store.export().len() > before);

    // Et la projection en quarantaine n'avance plus — sauter l'événement fautif lui donnerait un
    // état que la reconstruction ne reproduirait pas.
    let stalled = runner.catch_up(&store);
    assert_eq!(stalled.applied, 0);
    assert!(matches!(stalled.health, Health::Quarantined { .. }));
}

#[test]
fn a_rebuild_lifts_the_quarantine() {
    // Une reconstruction est une seconde chance : une projection qui resterait en quarantaine après
    // avoir été reconstruite ne pourrait jamais s'en sortir, même la cause corrigée.
    let mut rng = Rng::new(91);
    let mut store = MemoryEventStore::new();
    populate(&mut store, &mut rng, 15);

    let mut runner = ProjectionRunner::new(ConflictRegistry::new());
    runner.catch_up(&store);

    // Un conflit résolu sans avoir été déclaré : défaut de journal, pas occasion d'inventer.
    push(
        &mut store,
        &mut rng,
        "claim_00",
        "conflict.resolved",
        serde_json::json!({ "conflict_id": "cfl_fantome" }),
    );
    assert!(matches!(
        runner.catch_up(&store).health,
        Health::Quarantined { .. }
    ));

    // La reconstruction lève la quarantaine — et retombe sur la même faute, puisque le journal la
    // contient toujours. C'est le comportement voulu : le défaut est dans le journal, pas dans le
    // pilote.
    let after = runner.rebuild(&store);
    assert!(matches!(after.health, Health::Quarantined { .. }));
    assert!(after.applied > 0, "la reconstruction n'a rien rejoué");
}

// ————————————————————————— Ce que les projections ne font pas —————————————————————————

#[test]
fn the_conflict_registry_never_drops_a_resolved_conflict() {
    // Invariant 12 : « les résultats négatifs et conflits ne sont jamais supprimés pour rendre le
    // graphe propre ». Le mot vise exactement ce que ferait une projection ordinaire : ne garder
    // que les conflits ouverts, parce que ce sont les seuls qu'on interroge.
    let mut rng = Rng::new(101);
    let mut store = MemoryEventStore::new();
    let stream = "claim_00";

    push(
        &mut store,
        &mut rng,
        stream,
        "conflict.declared",
        serde_json::json!({ "conflict_id": "cfl_1", "statement": "les mesures divergent" }),
    );
    push(
        &mut store,
        &mut rng,
        stream,
        "conflict.resolved",
        serde_json::json!({ "conflict_id": "cfl_1" }),
    );

    let mut runner = ProjectionRunner::new(ConflictRegistry::new());
    runner.catch_up(&store);

    let registry = runner.projection();
    assert_eq!(registry.len(), 1, "le conflit résolu a disparu");
    assert!(registry.open().is_empty());
    let entry = registry.all().first().copied().expect("une entrée");
    assert!(!entry.is_open());
    assert_eq!(entry.statement, "les mesures divergent");
    // La résolution est datée, pas effacée.
    assert!(entry.resolved_at.is_some());
}

#[test]
fn no_projection_offers_a_way_to_remove_an_entry() {
    // La garantie se tient par l'absence. Le jour où quelqu'un ajoutera une méthode de retrait, ce
    // test le lui rappellera avant la revue.
    let registry = include_str!("../src/conflict_registry.rs");
    for forbidden in ["fn remove", "fn drop_entry", "fn prune", "fn forget"] {
        assert!(!registry.contains(forbidden), "`{forbidden}` existe");
    }
    // `reset` fait exception, et ce n'en est pas une : reconstruire n'est pas supprimer. Le
    // registre repart du journal, qui contient tout.
    assert!(registry.contains("fn reset"));
}

#[test]
fn the_validation_state_never_derives_a_level_from_a_status() {
    // §7.4, vu depuis la lecture. W1.a l'a rendu vrai dans le domaine ; une projection qui
    // compléterait le niveau manquant rendrait la garantie fausse là où tout le monde regarde.
    let source = include_str!("../src/validation_state.rs");
    for forbidden in ["level_from_status", "unwrap_or(\"", "unwrap_or_default()"] {
        assert!(
            !source.contains(forbidden),
            "`{forbidden}` dans la projection"
        );
    }
}

#[test]
fn the_watermark_advances_over_events_the_projection_ignores() {
    // Le watermark dit où la projection en est du **journal**, pas de ce qu'elle a retenu. Ne pas
    // l'avancer sur un événement écarté ferait relire à chaque passage tout ce qu'elle a ignoré.
    let mut rng = Rng::new(111);
    let mut store = MemoryEventStore::new();
    let stream = "claim_00";
    for _ in 0..5 {
        push(
            &mut store,
            &mut rng,
            stream,
            "task.queued",
            serde_json::json!({ "rien": "pour cette projection" }),
        );
    }

    let mut runner = ProjectionRunner::new(ValidationState::new());
    let progress = runner.catch_up(&store);
    assert_eq!(progress.applied, 5);
    assert_eq!(runner.projection().watermark(), 5);
    assert!(runner.projection().is_empty());

    // Un second passage ne relit rien.
    assert_eq!(runner.catch_up(&store).applied, 0);
}
