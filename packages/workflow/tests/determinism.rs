//! Test de sortie de W3.a — `docs/SPEC_V1.md` §11.2 et §11.3.
//!
//! **Un effet non encapsulé ne se déclare pas, et la liste de §11.2 ne se réduit pas en silence.**
//!
//! La roadmap tient W3 au groupe de commits et ne donne pas de test de sortie par item ; celui-ci
//! est arbitré ici et vaut pour W3.a. Il porte sur les deux choses que ce paquet peut réellement
//! garantir avant qu'un moteur existe : la **forme** des définitions, et le **décompte** de ce
//! qu'elles déclarent. Ce que fait le corps d'un pas ne se vérifie qu'en l'exécutant, donc en W3.b.

use std::fs;
use std::path::{Path, PathBuf};

use locus_domain::StableId;
use locus_protocol::Timestamp;
use locus_workflow::{
    Activity, CoverageFinding, DefinitionError, DeterminismFinding, Effect, Enforcement,
    Idempotency, MANDATORY_WORKFLOWS, RetirementError, Rule, Step, VersionRegistry,
    WorkflowDefinition, WorkflowKind, WorkflowVersion, definition_findings, minting_findings,
    replay_coverage, suspected_effects,
};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

/// Un identifiant de fixture, fixé et non tiré.
///
/// C'est le contraire du danger que `minting_findings` traque : un test qui fabrique une valeur
/// stable fixe l'identité, là où une frappe à l'exécution en produirait une neuve à chaque replay.
fn subject_id(seed: u8) -> StableId {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    StableId::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn key(value: &str) -> Idempotency {
    Idempotency::key(value).expect("clé non vide")
}

/// Un `TaskWorkflow` plausible : c'est lui qui prouve que le filet ne crie pas sur tout.
fn honest_task_workflow() -> WorkflowDefinition {
    let steps = vec![
        Step::deterministic("verify_prerequisites").expect("nom valide"),
        Step::Activity(
            Activity::new(
                "reserve_resources",
                [Effect::Network],
                key("reserve:task-1:attempt-1"),
            )
            .expect("activity valide"),
        ),
        Step::Activity(
            Activity::new(
                "materialize_context",
                [Effect::Network, Effect::Filesystem],
                key("materialize:task-1"),
            )
            .expect("activity valide"),
        ),
        Step::deterministic("decide_next_state").expect("nom valide"),
        Step::Activity(
            Activity::new(
                "upload_artifacts",
                [Effect::Network],
                Idempotency::natural("l'upload est adressé par empreinte de contenu (§19.1)")
                    .expect("raison non vide"),
            )
            .expect("activity valide"),
        ),
        Step::deterministic("record_outcome").expect("nom valide"),
    ];
    WorkflowDefinition::new(
        WorkflowKind::Task,
        WorkflowVersion::new(1),
        vec![subject_id(1)],
        steps,
    )
    .expect("définition valide")
}

// ---------------------------------------------------------------------------------------------
// §11.2 — la liste des onze
// ---------------------------------------------------------------------------------------------

/// Les onze noms de §11.2, transcrits. Toute divergence est rouge, dans un sens comme dans l'autre.
const SPEC_11_2: [&str; MANDATORY_WORKFLOWS] = [
    "ProgramWorkflow",
    "WorkstreamWorkflow",
    "BranchWorkflow",
    "TaskWorkflow",
    "ReviewWorkflow",
    "ReproductionWorkflow",
    "MemoryCurationWorkflow",
    "PortfolioWorkflow",
    "EnvironmentBuildWorkflow",
    "SandboxLifecycleWorkflow",
    "FederationWorkflow",
];

#[test]
fn les_onze_workflows_de_11_2_sont_tous_la() {
    let declared: Vec<&str> = WorkflowKind::ALL.iter().map(|kind| kind.name()).collect();
    assert_eq!(
        declared,
        SPEC_11_2.to_vec(),
        "la liste de §11.2 a bougé : onze workflows sont obligatoires, dans cet ordre"
    );
}

#[test]
fn chaque_nom_se_relit_et_un_nom_inconnu_reste_inconnu() {
    for kind in WorkflowKind::ALL {
        assert_eq!(WorkflowKind::parse(kind.name()), Some(kind));
    }
    // Une faute de frappe ne se range pas sous le workflow le plus proche.
    assert_eq!(WorkflowKind::parse("TaskWorklow"), None);
    assert_eq!(WorkflowKind::parse("Task"), None);
}

// ---------------------------------------------------------------------------------------------
// §11.3 règle 1 — les effets n'existent que dans une activity
// ---------------------------------------------------------------------------------------------

#[test]
fn un_pas_deterministe_n_a_aucun_champ_pour_un_effet() {
    // Garantie tenue par la forme : la seule façon de porter un `Effect` est de passer par une
    // `Activity`. Le test énonce la conséquence observable — tout effet déclaré appartient à une
    // activity — et la propriété est vérifiée par le compilateur, pas par cette assertion.
    let definition = honest_task_workflow();
    let declared: usize = definition
        .activities()
        .map(|activity| activity.effects().len())
        .sum();
    let carried: usize = definition
        .steps()
        .iter()
        .map(|step| match step {
            Step::Activity(activity) => activity.effects().len(),
            Step::Deterministic { .. } => 0,
        })
        .sum();
    assert_eq!(declared, carried);
    assert!(declared > 0, "la fixture doit déclarer des effets");
}

#[test]
fn un_pas_deterministe_dont_le_nom_avoue_un_effet_est_signale() {
    let definition = WorkflowDefinition::new(
        WorkflowKind::Review,
        WorkflowVersion::new(1),
        vec![subject_id(2)],
        vec![
            Step::deterministic("fetch_reviewer_context").expect("nom valide"),
            Step::deterministic("decide_verdict").expect("nom valide"),
        ],
    )
    .expect("définition valide");

    let findings = definition_findings(&definition);
    assert_eq!(
        findings,
        vec![DeterminismFinding::UnencapsulatedEffect {
            step: "fetch_reviewer_context".to_owned(),
            marker: "fetch",
            effect: Effect::Network,
        }]
    );
    assert_eq!(findings[0].rule(), Rule::EffectsEncapsulated);
}

#[test]
fn une_activity_qui_sous_declare_ses_effets_est_signalee() {
    let definition = WorkflowDefinition::new(
        WorkflowKind::Task,
        WorkflowVersion::new(1),
        vec![subject_id(3)],
        vec![Step::Activity(
            Activity::new("llm_critique", [Effect::Network], key("critique:1"))
                .expect("activity valide"),
        )],
    )
    .expect("définition valide");

    let findings = definition_findings(&definition);
    assert_eq!(
        findings,
        vec![DeterminismFinding::UndeclaredEffect {
            activity: "llm_critique".to_owned(),
            marker: "llm",
            effect: Effect::Llm,
        }],
        "l'effet a lieu au bon endroit, mais une déclaration incomplète fausse tout ce qui se \
         décide à partir d'elle"
    );
}

#[test]
fn le_filet_ne_crie_pas_sur_un_workflow_honnete() {
    // Sans ce test, un filet qui signalerait tout passerait les trois précédents.
    assert_eq!(definition_findings(&honest_task_workflow()), Vec::new());
}

#[test]
fn le_filet_compare_des_jetons_et_non_des_sous_chaines() {
    // `known` contient `now`. Un filet qui crierait ici serait désarmé au premier agacement.
    assert_eq!(suspected_effects("known_inputs"), Vec::new());
    assert_eq!(suspected_effects("normalize_claim"), Vec::new());
    assert_eq!(
        suspected_effects("read_now"),
        vec![("now", Effect::Clock)],
        "le jeton entier, lui, se voit"
    );
    assert_eq!(
        suspected_effects("read_wall_clock"),
        vec![("wall_clock", Effect::Clock)],
        "un marqueur composé se cherche comme une suite de jetons contiguë"
    );
}

// ---------------------------------------------------------------------------------------------
// §11.3 règle 2 — les IDs métier sont frappés avant l'entrée
// ---------------------------------------------------------------------------------------------

fn crate_sources() -> Vec<(String, String)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    read_rust_sources(&root)
}

fn read_rust_sources(directory: &Path) -> Vec<(String, String)> {
    let mut sources = Vec::new();
    for entry in fs::read_dir(directory).expect("le répertoire des sources existe") {
        let path = entry.expect("entrée lisible").path();
        if path.is_dir() {
            sources.extend(read_rust_sources(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let text = fs::read_to_string(&path).expect("source lisible");
            sources.push((path.display().to_string(), text));
        }
    }
    sources
}

#[test]
fn aucun_identifiant_n_est_frappe_dans_ce_paquet() {
    let sources = crate_sources();
    assert!(!sources.is_empty(), "le balayage doit voir des fichiers");

    let findings: Vec<_> = sources
        .iter()
        .flat_map(|(location, text)| minting_findings(location, text))
        .collect();

    assert!(
        findings.is_empty(),
        "§11.3 veut les IDs métier créés avant l'entrée dans le backend ; frappés ici : {findings:#?}"
    );
}

#[test]
fn le_balayage_de_frappe_attrape() {
    // Sans ce test, une fonction rendant toujours une liste vide passerait le précédent. Le
    // fichier des marqueurs est balayé comme les autres : la table y est assemblée par `concat!`
    // pour qu'aucune ligne ne porte un marqueur entier, plutôt que d'exclure le fichier — une
    // exclusion ouvrirait dans la garde le trou même qu'elle ferme.
    let bait = "let id = StableId::from_parts(Timestamp::from_millis(0), entropy);";
    let findings = minting_findings("appât", bait);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].marker, "from_parts");
    assert_eq!(findings[0].rule(), Rule::IdsBeforeEntry);

    // Une ligne de commentaire ne compte pas : nommer la faute pour la décrire n'est pas la faire.
    assert_eq!(minting_findings("appât", "// jamais de from_parts ici"), []);
}

#[test]
fn une_definition_sans_sujet_ne_se_construit_pas() {
    let error = WorkflowDefinition::new(
        WorkflowKind::Program,
        WorkflowVersion::new(1),
        Vec::new(),
        vec![Step::deterministic("plan").expect("nom valide")],
    )
    .expect_err("un workflow sans identifiant métier frapperait le sien en chemin");
    assert_eq!(error, DefinitionError::NoSubject);
}

// ---------------------------------------------------------------------------------------------
// §11.3 règles 3 et 4 — idempotence déclarée, versions explicites
// ---------------------------------------------------------------------------------------------

#[test]
fn l_idempotence_a_deux_formes_et_pas_de_troisieme() {
    let by_key = key("upload:sha256:abc");
    assert_eq!(by_key.dedup_key(), Some("upload:sha256:abc"));
    assert_eq!(by_key.rationale(), None);

    let natural = Idempotency::natural("l'écriture est adressée par empreinte").expect("non vide");
    assert_eq!(natural.dedup_key(), None);
    assert!(natural.rationale().is_some());

    // Ni clé vide, ni justification vide : « naturellement idempotent » sans raison a le même air
    // que la même phrase vérifiée.
    assert!(matches!(
        Idempotency::key("   "),
        Err(DefinitionError::EmptyName { .. })
    ));
    assert!(matches!(
        Idempotency::natural(""),
        Err(DefinitionError::EmptyName { .. })
    ));
}

#[test]
fn une_version_ne_se_deduit_pas() {
    // `WorkflowVersion` n'implémente pas `Default` : la ligne suivante ne compilerait pas.
    //     let version = WorkflowVersion::default();
    // §11.3 veut les versions explicites ; une version implicite est ce que la règle interdit.
    assert_eq!(WorkflowVersion::new(3).number(), 3);
    assert_eq!(WorkflowVersion::new(3).to_string(), "v3");
}

#[test]
fn deux_pas_ne_partagent_pas_un_nom() {
    let error = WorkflowDefinition::new(
        WorkflowKind::Branch,
        WorkflowVersion::new(1),
        vec![subject_id(4)],
        vec![
            Step::deterministic("decide").expect("nom valide"),
            Step::deterministic("decide").expect("nom valide"),
        ],
    )
    .expect_err("un historique de replay ne saurait pas les distinguer");
    assert_eq!(
        error,
        DefinitionError::DuplicateStep {
            name: "decide".to_owned()
        }
    );
}

// ---------------------------------------------------------------------------------------------
// §11.3 règles 5 et 6 — couverture de replay, retraits contrôlés
// ---------------------------------------------------------------------------------------------

#[test]
fn une_version_supportee_sans_test_de_replay_est_un_manque_nomme() {
    let registry = VersionRegistry::new()
        .support(WorkflowKind::Task, WorkflowVersion::new(1))
        .support(WorkflowKind::Task, WorkflowVersion::new(2))
        .support(WorkflowKind::Review, WorkflowVersion::new(1));

    let complete = replay_coverage(
        &registry,
        &[
            (WorkflowKind::Task, WorkflowVersion::new(1)),
            (WorkflowKind::Task, WorkflowVersion::new(2)),
            (WorkflowKind::Review, WorkflowVersion::new(1)),
        ],
    );
    assert_eq!(complete, Vec::new());

    let missing = replay_coverage(
        &registry,
        &[
            (WorkflowKind::Task, WorkflowVersion::new(1)),
            (WorkflowKind::Review, WorkflowVersion::new(1)),
        ],
    );
    assert_eq!(
        missing,
        vec![CoverageFinding::Untested {
            kind: WorkflowKind::Task,
            version: WorkflowVersion::new(2),
        }],
        "deux tests sur trois versions n'est pas une couverture, c'est un total"
    );
}

#[test]
fn un_test_de_replay_orphelin_est_signale_aussi() {
    let registry = VersionRegistry::new().support(WorkflowKind::Task, WorkflowVersion::new(2));
    let findings = replay_coverage(
        &registry,
        &[
            (WorkflowKind::Task, WorkflowVersion::new(1)),
            (WorkflowKind::Task, WorkflowVersion::new(2)),
        ],
    );
    assert_eq!(
        findings,
        vec![CoverageFinding::Stray {
            kind: WorkflowKind::Task,
            version: WorkflowVersion::new(1),
        }],
        "il passe, il compte dans le total, et il rejoue une forme que plus rien ne revendique"
    );
}

#[test]
fn un_registre_vide_nomme_les_onze_manquants() {
    assert_eq!(
        VersionRegistry::new().unsupported_kinds(),
        WorkflowKind::ALL.to_vec(),
        "l'absence de support doit se dire, pas se confondre avec le silence"
    );
}

#[test]
fn les_deux_retraits_dangereux_sont_refuses() {
    let mut registry = VersionRegistry::new()
        .support(WorkflowKind::Task, WorkflowVersion::new(1))
        .support(WorkflowKind::Task, WorkflowVersion::new(2));

    assert_eq!(
        registry.retire(WorkflowKind::Task, WorkflowVersion::new(2)),
        Err(RetirementError::Current {
            kind: WorkflowKind::Task,
            version: WorkflowVersion::new(2),
        })
    );
    assert_eq!(
        registry.retire(WorkflowKind::Task, WorkflowVersion::new(7)),
        Err(RetirementError::NotSupported {
            kind: WorkflowKind::Task,
            version: WorkflowVersion::new(7),
        })
    );

    // Retirer une version dépassée, elle, est le déroulement normal d'une migration contrôlée.
    registry
        .retire(WorkflowKind::Task, WorkflowVersion::new(1))
        .expect("une version dépassée se retire");
    assert_eq!(
        registry.supported(WorkflowKind::Task),
        vec![WorkflowVersion::new(2)]
    );

    assert_eq!(
        registry.retire(WorkflowKind::Task, WorkflowVersion::new(2)),
        Err(RetirementError::LastRemaining {
            kind: WorkflowKind::Task,
            version: WorkflowVersion::new(2),
        }),
        "la dernière restante n'est pas retirable : le workflow cesserait d'exister"
    );
}

// ---------------------------------------------------------------------------------------------
// Les six règles, et l'inégalité de leurs gardes
// ---------------------------------------------------------------------------------------------

#[test]
fn les_six_regles_de_11_3_sont_transcrites_et_chacune_dit_par_quoi_elle_tient() {
    assert_eq!(Rule::ALL.len(), 6);
    for rule in Rule::ALL {
        assert!(!rule.statement().is_empty());
        assert!(
            !rule.enforcement().is_empty(),
            "{rule:?} ne dit pas par quoi elle tient : une règle sans garde est un paragraphe"
        );
    }
    // La cinquième est la seule tenue par un décompte, et c'est exactement pourquoi elle est la
    // plus facile à laisser pourrir : rien ne casse quand personne ne lit le décompte.
    assert_eq!(Rule::ReplayTests.enforcement(), [Enforcement::ByCoverage]);
}
