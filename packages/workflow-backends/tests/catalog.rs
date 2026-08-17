//! Test de sortie de W3.c — `docs/SPEC_V1.md` §11.2.
//!
//! **Les onze workflows de §11.2 s'exécutent sur le backend de test et se rejouent à l'identique.**
//!
//! C'est le moment où les gardes de W3.a cessent de tourner sur des fixtures écrites par celui-là
//! même qui les avait écrites. Jusqu'ici le filet des noms voyait trois exemples choisis ; il voit
//! maintenant cinquante-quatre pas de contenu réel. La même chose s'était produite en W1.a, quand
//! la règle 1 de `boundaries.json` a enfin eu des fichiers à examiner.

use std::collections::BTreeSet;

use locus_domain::StableId;
use locus_protocol::Timestamp;
use locus_workflow::{
    CATALOG_VERSION, DefinitionError, Effect, Step, VersionRegistry, WorkflowBackend,
    WorkflowDefinition, WorkflowKind, WorkflowState, catalog, definition_findings, replay_coverage,
};
use locus_workflow_backends::{DeterministicBackend, block_on, replay};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

/// Un identifiant de sujet, fixé et distinct par workflow.
fn subject(kind: WorkflowKind) -> Vec<StableId> {
    let position = WorkflowKind::ALL
        .iter()
        .position(|candidate| *candidate == kind)
        .expect("les onze sont dans ALL");
    let mut entropy = [0_u8; 10];
    entropy[9] = u8::try_from(position + 1).expect("onze tient sur un octet");
    vec![
        StableId::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
            .expect("l'instant de fixture tient sur 48 bits"),
    ]
}

fn definitions() -> Vec<WorkflowDefinition> {
    WorkflowKind::ALL
        .into_iter()
        .map(|kind| {
            catalog::definition(kind, CATALOG_VERSION, subject(kind))
                .unwrap_or_else(|error| panic!("définition de {kind} : {error}"))
        })
        .collect()
}

/// Un moteur où chaque activity de la définition a un exécutant, et un résultat fixé.
fn staffed_for(definition: &WorkflowDefinition) -> DeterministicBackend {
    let mut backend = DeterministicBackend::new();
    for activity in definition.activities() {
        backend.register_activity(activity.name(), &format!("result:{}", activity.name()));
    }
    backend
}

// ---------------------------------------------------------------------------------------------
// §11.2 — les onze existent, et aucun ne peut manquer en silence
// ---------------------------------------------------------------------------------------------

#[test]
fn les_onze_ont_une_definition() {
    let defined: Vec<WorkflowKind> = definitions().iter().map(WorkflowDefinition::kind).collect();
    assert_eq!(defined, WorkflowKind::ALL.to_vec());

    // La propriété est en réalité tenue par le compilateur : `catalog::definition` fait un `match`
    // exhaustif, et un douzième workflow ne compilerait pas tant qu'il n'aurait pas de définition.
    // Ce test dit la conséquence observable ; le `match` dit la garantie.
    assert_eq!(defined.len(), WorkflowKind::ALL.len());
}

#[test]
fn un_workflow_sans_identifiant_metier_ne_se_definit_pas() {
    assert_eq!(
        catalog::definition(WorkflowKind::Task, CATALOG_VERSION, Vec::new()),
        Err(DefinitionError::NoSubject)
    );
}

// ---------------------------------------------------------------------------------------------
// §11.3 — la forme, vérifiée sur du contenu réel
// ---------------------------------------------------------------------------------------------

#[test]
fn aucune_definition_ne_laisse_un_effet_hors_activity() {
    for definition in definitions() {
        let findings = definition_findings(&definition);
        assert!(findings.is_empty(), "{} : {findings:#?}", definition.kind());
    }
}

#[test]
fn les_cles_d_idempotence_ne_se_marchent_pas_dessus() {
    // Deux activities d'un même workflow qui partageraient une clé se dédoublonneraient l'une
    // contre l'autre : le second effet ne se produirait jamais, et rien ne le dirait.
    for definition in definitions() {
        let mut seen = BTreeSet::new();
        for activity in definition.activities() {
            if let Some(key) = activity.idempotency().dedup_key() {
                assert!(
                    seen.insert(key.to_owned()),
                    "{} : deux activities partagent la clé « {key} »",
                    definition.kind()
                );
            } else {
                assert!(
                    activity.idempotency().rationale().is_some(),
                    "{} / {} : ni clé ni raison",
                    definition.kind(),
                    activity.name()
                );
            }
        }
    }
}

#[test]
fn les_quatre_effets_de_11_3_apparaissent_et_l_aleatoire_non() {
    let declared: BTreeSet<Effect> = definitions()
        .iter()
        .flat_map(|definition| {
            definition
                .activities()
                .flat_map(|activity| activity.effects().iter().copied())
                .collect::<Vec<_>>()
        })
        .collect();

    for effect in [
        Effect::Llm,
        Effect::Network,
        Effect::Filesystem,
        Effect::Clock,
    ] {
        assert!(
            declared.contains(&effect),
            "aucun des onze ne déclare {effect} : un catalogue monoculture ne prouve rien du filet"
        );
    }
    assert!(
        !declared.contains(&Effect::Random),
        "aucun des onze workflows de §11.2 n'a besoin de tirer au sort ; en déclarer un pour \
         remplir le tableau irait dans le mauvais sens"
    );
}

// ---------------------------------------------------------------------------------------------
// Le test de sortie
// ---------------------------------------------------------------------------------------------

#[test]
fn les_onze_s_executent_jusqu_au_bout_et_se_rejouent_a_l_identique() {
    let mut replayed_versions = Vec::new();

    for definition in definitions() {
        let mut backend = staffed_for(&definition);
        let handle = block_on(backend.start(&definition))
            .unwrap_or_else(|error| panic!("{} : démarrage — {error}", definition.kind()));
        let id = handle.id.clone();

        backend
            .run(&id)
            .unwrap_or_else(|error| panic!("{} : exécution — {error}", definition.kind()));

        let live = block_on(backend.inspect(&id)).expect("inspection");
        assert_eq!(
            live,
            WorkflowState::Completed,
            "{} n'est pas arrivé au bout",
            definition.kind()
        );

        let history = backend.history(&id).expect("historique").to_vec();
        drop(backend);

        let rejoined = replay(&definition, &history)
            .unwrap_or_else(|error| panic!("{} : rejeu — {error}", definition.kind()));
        assert_eq!(
            rejoined.state,
            live,
            "{} : rejeu ≠ vivant",
            definition.kind()
        );

        // Chaque activity a laissé son résultat dans l'historique, dans l'ordre des pas — sans quoi
        // un rejeu redemanderait au monde, et n'obtiendrait pas la même chose.
        let expected: Vec<(String, String)> = definition
            .steps()
            .iter()
            .filter_map(|step| match step {
                Step::Activity(activity) => Some((
                    activity.name().to_owned(),
                    format!("result:{}", activity.name()),
                )),
                Step::Deterministic { .. } => None,
            })
            .collect();
        assert_eq!(
            rejoined.activity_results,
            expected,
            "{} : les résultats rejoués ne sont pas ceux qui ont eu lieu",
            definition.kind()
        );

        replayed_versions.push((definition.kind(), definition.version()));
    }

    // §11.3, cinquième règle : « tests de replay pour les versions supportées ». Le décompte est
    // fait ici, à partir de ce que ce test a **réellement rejoué** — et non d'une liste écrite à
    // côté, qui resterait verte le jour où l'un des onze cesserait d'être rejoué.
    let registry = WorkflowKind::ALL
        .into_iter()
        .fold(VersionRegistry::new(), |registry: VersionRegistry, kind| {
            registry.support(kind, CATALOG_VERSION)
        });
    assert_eq!(registry.unsupported_kinds(), Vec::new());
    assert_eq!(
        replay_coverage(&registry, &replayed_versions),
        Vec::new(),
        "un des onze est supporté sans être rejoué, ou rejoué sans être supporté"
    );
}

#[test]
fn une_activity_sans_executant_arrete_le_workflow_au_bon_pas() {
    // Le moteur ne complète pas « au mieux » : il s'arrête là où l'exécutant manque, et
    // l'historique s'arrête avec lui. Vérifié sur un workflow réel du catalogue.
    let definition = catalog::definition(
        WorkflowKind::SandboxLifecycle,
        CATALOG_VERSION,
        subject(WorkflowKind::SandboxLifecycle),
    )
    .expect("définition valide");

    let mut backend = DeterministicBackend::new();
    backend.register_activity("reserve_sandbox_resources", "reserved");
    let id = block_on(backend.start(&definition)).expect("démarrage").id;

    backend
        .advance(&id)
        .expect("validate_sandbox_spec est déterministe");
    backend
        .advance(&id)
        .expect("reserve_sandbox_resources a un exécutant");
    backend
        .advance(&id)
        .expect_err("start_sandbox n'en a pas : le moteur refuse plutôt que d'inventer");

    assert_eq!(
        replay(&definition, backend.history(&id).expect("historique"))
            .expect("rejeu")
            .state,
        WorkflowState::Running { step: 2 },
        "l'exécution est restée au pas où l'exécutant manquait"
    );
}
