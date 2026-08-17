//! Les onze workflows obligatoires, définis — `docs/SPEC_V1.md` §11.2.

use locus_domain::StableId;

use crate::definition::{
    Activity, DefinitionError, Effect, Idempotency, Step, WorkflowDefinition, WorkflowVersion,
};
use crate::kind::WorkflowKind;

/// La version sous laquelle le catalogue est écrit.
///
/// Une constante, et non un `1` répété onze fois : les onze définitions évoluent ensemble tant que
/// rien ne les fait diverger, et une version par workflow inviterait à en bouger une seule sans
/// s'apercevoir que le rejeu des dix autres continue de dire `v1`.
pub const CATALOG_VERSION: WorkflowVersion = WorkflowVersion::new(1);

/// La définition d'un des onze workflows de §11.2.
///
/// # Ce que ce catalogue est, et ce qu'il n'est pas
///
/// §11.2 énumère les onze workflows et n'en décrit les pas nulle part. Les suites ci-dessous sont
/// donc **arbitrées**, chacune à partir de la section qui décrit le processus correspondant : §13
/// pour le portefeuille, §17 pour la revue, §19 pour les environnements et la reproduction, §21
/// pour les sandboxes, §25 pour la fédération. Ce sont des squelettes exacts sur la **forme** — où
/// un effet a le droit d'avoir lieu, ce que chaque activity dédoublonne — et provisoires sur le
/// détail métier, que W4 à W8 rempliront.
///
/// Ce qui compte dès maintenant est que la forme soit vérifiée sur du contenu réel plutôt que sur
/// des fixtures écrites pour passer : les gardes de W3.a tournaient jusqu'ici sur trois exemples
/// choisis par celui-là même qui les avait écrites.
///
/// # Pourquoi une seule fonction plutôt qu'onze
///
/// Le `match` est exhaustif. Un douzième workflow ajouté à [`WorkflowKind`] ne compilera pas tant
/// qu'il n'aura pas de définition — la liste de §11.2 et le catalogue ne peuvent pas diverger en
/// silence.
///
/// # Errors
///
/// [`DefinitionError::NoSubject`] si `subject` est vide — §11.3 veut les identifiants métier créés
/// avant l'entrée dans le backend, et la clé d'idempotence des activities en dépend.
pub fn definition(
    kind: WorkflowKind,
    version: WorkflowVersion,
    subject: Vec<StableId>,
) -> Result<WorkflowDefinition, DefinitionError> {
    let Some(first) = subject.first() else {
        return Err(DefinitionError::NoSubject);
    };
    // La clé d'idempotence est ancrée sur l'objet, pas sur l'exécution : deux tentatives du même
    // workflow sur le même objet doivent se dédoublonner, sinon la reprise après incident refait
    // l'effet une seconde fois.
    let anchor = first.to_string();

    let steps = match kind {
        WorkflowKind::Program => program(&anchor),
        WorkflowKind::Workstream => workstream(&anchor),
        WorkflowKind::Branch => branch(&anchor),
        WorkflowKind::Task => task(&anchor),
        WorkflowKind::Review => review(&anchor),
        WorkflowKind::Reproduction => reproduction(&anchor),
        WorkflowKind::MemoryCuration => memory_curation(&anchor),
        WorkflowKind::Portfolio => portfolio(&anchor),
        WorkflowKind::EnvironmentBuild => environment_build(&anchor),
        WorkflowKind::SandboxLifecycle => sandbox_lifecycle(&anchor),
        WorkflowKind::Federation => federation(&anchor),
    }?;

    WorkflowDefinition::new(kind, version, subject, steps)
}

/// §13 — la campagne : ouvrir des axes, les arbitrer, conclure.
fn program(anchor: &str) -> Result<Vec<Step>, DefinitionError> {
    Ok(vec![
        det("record_program_charter")?,
        act("open_workstreams", &[Effect::Network], anchor)?,
        det("await_workstream_reports")?,
        det("score_portfolio")?,
        act("publish_program_outcome", &[Effect::Network], anchor)?,
    ])
}

/// §13 — un axe de travail : ouvrir une branche, la suivre, la refermer.
fn workstream(anchor: &str) -> Result<Vec<Step>, DefinitionError> {
    Ok(vec![
        det("plan_workstream")?,
        act("open_branch", &[Effect::Network], anchor)?,
        det("collect_branch_results")?,
        act("close_workstream", &[Effect::Network], anchor)?,
    ])
}

/// §7.1 et §18 — la vie d'une branche épistémique.
///
/// `merge_or_reopen` est déterministe : décider est du calcul sur des faits déjà acquis. Ce qui
/// touche au monde est ce qui suit, et c'est une activity.
fn branch(anchor: &str) -> Result<Vec<Step>, DefinitionError> {
    Ok(vec![
        det("open_branch_record")?,
        act("admit_branch", &[Effect::Network], anchor)?,
        det("accumulate_evidence")?,
        act("request_validation", &[Effect::Network], anchor)?,
        det("merge_or_reopen")?,
        act("record_branch_conclusion", &[Effect::Network], anchor)?,
    ])
}

/// §11 et §12 — une tâche confiée à un worker.
///
/// La réservation précède l'exécution : invariant 6, « les ressources sont réservées avant
/// exécution ; elles ne sont pas supposées illimitées ». La libération est un pas à part entière,
/// et non un `finally` implicite — c'est elle que W3.e compensera.
fn task(anchor: &str) -> Result<Vec<Step>, DefinitionError> {
    Ok(vec![
        det("verify_prerequisites")?,
        compensated(
            act("reserve_resources", &[Effect::Network], anchor)?,
            "release_resources",
        )?,
        act(
            "materialize_context",
            &[Effect::Network, Effect::Filesystem],
            anchor,
        )?,
        act("dispatch_attempt", &[Effect::Network], anchor)?,
        det("decide_next_state")?,
        act("release_resources", &[Effect::Network], anchor)?,
    ])
}

/// §17 — la revue comme protocole, pas comme agent unique.
///
/// `select_independent_reviewers` est déterministe et vient **après** l'assemblage du dossier :
/// l'invariant 11 dit que les reviewers indépendants ne reçoivent pas le raisonnement privé du
/// générateur, et c'est le dossier — pas le reviewer — qui décide de ce qui est transmis.
fn review(anchor: &str) -> Result<Vec<Step>, DefinitionError> {
    Ok(vec![
        act("assemble_review_dossier", &[Effect::Network], anchor)?,
        det("select_independent_reviewers")?,
        act("collect_findings", &[Effect::Network, Effect::Llm], anchor)?,
        act("collect_rebuttal", &[Effect::Network, Effect::Llm], anchor)?,
        det("decide_review_outcome")?,
        act("record_review_decision", &[Effect::Network], anchor)?,
    ])
}

/// §19 et §29 — reproduire un résultat depuis son environnement épinglé.
fn reproduction(anchor: &str) -> Result<Vec<Step>, DefinitionError> {
    Ok(vec![
        det("pin_environment")?,
        act(
            "rebuild_environment",
            &[Effect::Network, Effect::Filesystem],
            anchor,
        )?,
        act("rerun_attempt", &[Effect::Network], anchor)?,
        det("compare_outputs")?,
        act("record_reproduction_verdict", &[Effect::Network], anchor)?,
    ])
}

/// §16 — curer la mémoire du laboratoire.
///
/// Le seul workflow des onze dont un pas appelle un modèle : résumer est un appel LLM, et §11.3 le
/// range parmi les effets. Il est donc dans une activity, déclaré, et son résultat sera enregistré
/// dans l'historique — sans quoi un rejeu redemanderait au modèle et obtiendrait autre chose.
fn memory_curation(anchor: &str) -> Result<Vec<Step>, DefinitionError> {
    Ok(vec![
        det("select_curation_candidates")?,
        act("summarize_candidates", &[Effect::Llm], anchor)?,
        det("score_retention")?,
        act(
            "write_memory_index",
            &[Effect::Network, Effect::Filesystem],
            anchor,
        )?,
        det("record_curation_outcome")?,
    ])
}

/// §13 — arbitrer entre campagnes.
fn portfolio(anchor: &str) -> Result<Vec<Step>, DefinitionError> {
    Ok(vec![
        act("collect_branch_indicators", &[Effect::Network], anchor)?,
        det("score_branches")?,
        det("decide_allocation")?,
        act("apply_allocation", &[Effect::Network], anchor)?,
    ])
}

/// §19.3 et §19.4 — construire un environnement d'exécution.
///
/// `record_image_digest` est **naturellement** idempotent : le digest est l'identité de l'image, et
/// réenregistrer le même digest ne change rien. C'est le seul pas du catalogue dans ce cas, et il
/// le dit plutôt que de porter une clé qui laisserait croire à une déduplication.
fn environment_build(anchor: &str) -> Result<Vec<Step>, DefinitionError> {
    Ok(vec![
        det("resolve_toolchain_profiles")?,
        act("resolve_lockfiles", &[Effect::Network], anchor)?,
        act(
            "build_image",
            &[Effect::Network, Effect::Filesystem],
            anchor,
        )?,
        act("scan_image", &[Effect::Network], anchor)?,
        natural(
            "record_image_digest",
            &[Effect::Network],
            "le digest est l'identité de l'image : le réenregistrer ne change rien",
        )?,
    ])
}

/// §21 et invariant 5 — le cycle de vie d'une sandbox.
///
/// `collect_attestation` lit l'heure : une attestation sans instant n'atteste de rien de datable.
/// L'effet [`Effect::Clock`] est donc déclaré, et c'est exactement ce que §11.3 demande — non pas
/// que le temps ne soit jamais lu, mais qu'il ne le soit **que** dans une activity.
fn sandbox_lifecycle(anchor: &str) -> Result<Vec<Step>, DefinitionError> {
    Ok(vec![
        det("validate_sandbox_spec")?,
        compensated(
            act("reserve_sandbox_resources", &[Effect::Network], anchor)?,
            "release_sandbox_resources",
        )?,
        compensated(
            act("start_sandbox", &[Effect::Network], anchor)?,
            "stop_sandbox",
        )?,
        act(
            "collect_attestation",
            &[Effect::Network, Effect::Clock],
            anchor,
        )?,
        act("stop_sandbox", &[Effect::Network], anchor)?,
        act("release_sandbox_resources", &[Effect::Network], anchor)?,
    ])
}

/// §25 — échanger avec une instance fédérée.
fn federation(anchor: &str) -> Result<Vec<Step>, DefinitionError> {
    Ok(vec![
        act("verify_peer_identity", &[Effect::Network], anchor)?,
        det("negotiate_exchange_scope")?,
        act(
            "export_shareable_bundle",
            &[Effect::Network, Effect::Filesystem],
            anchor,
        )?,
        act(
            "import_peer_bundle",
            &[Effect::Network, Effect::Filesystem],
            anchor,
        )?,
        det("record_federation_outcome")?,
    ])
}

fn det(name: &str) -> Result<Step, DefinitionError> {
    Step::deterministic(name)
}

fn act(name: &str, effects: &[Effect], anchor: &str) -> Result<Step, DefinitionError> {
    let idempotency = Idempotency::key(&format!("{name}:{anchor}"))?;
    Ok(Step::Activity(Activity::new(
        name,
        effects.iter().copied(),
        idempotency,
    )?))
}

/// Déclarer par quelle activity un pas se défait — §11.4.
///
/// Ce qui est compensé ici est **technique** : une réservation, un lease, une sandbox démarrée.
/// §11.4 le dit en toutes lettres et ajoute que les compensations « ne réécrivent jamais l'histoire
/// épistémique ». Aucun pas qui enregistre un fait scientifique n'a donc de compensation, et ce
/// n'est pas un oubli : défaire un fait observé n'est pas une compensation, c'est une falsification.
fn compensated(step: Step, by: &str) -> Result<Step, DefinitionError> {
    match step {
        Step::Activity(activity) => Ok(Step::Activity(activity.compensating(by)?)),
        Step::Deterministic { name } => Err(DefinitionError::UnknownCompensation {
            activity: name,
            compensation: by.to_owned(),
        }),
    }
}

fn natural(name: &str, effects: &[Effect], rationale: &str) -> Result<Step, DefinitionError> {
    Ok(Step::Activity(Activity::new(
        name,
        effects.iter().copied(),
        Idempotency::natural(rationale)?,
    )?))
}
