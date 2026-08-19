//! Test de sortie de W4.d.2 — ADR 0004, `docs/SPEC_V1.md` §21.6, §19.3, `docs/03`.
//!
//! **Le driver demande au runtime exactement ce que le plan a décidé, et il atteste de ce qu'il
//! observe — jamais de ce qu'il a demandé.**
//!
//! La seconde moitié est celle qui ne se négocie pas. `runtime.rs` l'écrit déjà : « un broker qui
//! composerait l'attestation à partir de ce qu'il avait demandé attesterait de sa propre
//! demande ». Un driver qui rendrait `plan.level()` passerait tous les tests de conformité de
//! W4.a en ayant tout raté, et personne ne le saurait jamais.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use locus_execd::linux::{
    Execution, InvocationError, PodmanBackend, RestrictedProfile, Runner, SeccompProfiles,
    SystemRunner, Workload, create_arguments, inspect_arguments, plan,
};
use locus_execd::{RuntimeError, RuntimePort, SandboxId};
use locus_execution::{
    Conformance, Mount, MountMode, NetworkMode, ResourceSpec, SandboxLevel, SandboxProfile,
    SandboxSpec, conformance,
};

// ---------------------------------------------------------------------------------------------
// Le double de lanceur
// ---------------------------------------------------------------------------------------------

/// Un lanceur qui n'exécute rien : il enregistre ce qu'on lui demande et rend ce qu'on lui a dit
/// de rendre. C'est ce qui permet de vérifier les arguments, l'analyse des sorties et les chemins
/// d'erreur là où aucun runtime rootless n'est garanti — c'est-à-dire en CI.
struct ScriptedRunner {
    calls: Mutex<Vec<Vec<String>>>,
    answers: Mutex<Vec<Execution>>,
}

impl ScriptedRunner {
    fn new(answers: Vec<Execution>) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            answers: Mutex::new(answers),
        }
    }

    /// Le lanceur ordinaire : tout réussit, `inspect` rend un conteneur pleinement confiné.
    fn confined() -> Self {
        Self::new(vec![
            ok(""),
            ok(""),
            ok(&observations(
                "none",
                "true",
                "no-new-privileges,seccomp=restricted.json",
            )),
        ])
    }

    fn call(&self, index: usize) -> Vec<String> {
        self.calls.lock().expect("verrou")[index].clone()
    }
}

impl Runner for ScriptedRunner {
    fn run(&self, arguments: &[String]) -> Result<Execution, RuntimeError> {
        self.calls.lock().expect("verrou").push(arguments.to_vec());
        let mut answers = self.answers.lock().expect("verrou");
        if answers.is_empty() {
            return Err(RuntimeError::Unavailable {
                detail: "le script est épuisé".to_owned(),
            });
        }
        Ok(answers.remove(0))
    }
}

/// Un lanceur qui n'a pas de podman à lancer — le cas d'un hôte sans runtime.
struct AbsentRuntime;

impl Runner for AbsentRuntime {
    fn run(&self, _arguments: &[String]) -> Result<Execution, RuntimeError> {
        Err(RuntimeError::Unavailable {
            detail: "podman : No such file or directory (os error 2)".to_owned(),
        })
    }
}

fn ok(stdout: &str) -> Execution {
    Execution {
        code: 0,
        stdout: stdout.to_owned(),
        stderr: String::new(),
    }
}

/// La sortie d'inspection, avec les trois champs qui décident du niveau observé.
fn observations(network: &str, readonly: &str, security: &str) -> String {
    let private = [
        "userns=private",
        "pidns=private",
        "ipcns=private",
        "utsns=private",
    ]
    .join("\n");
    format!(
        "status=running\nmemory=2147483648\npids=128\ncpu_quota=150000\ncpu_period=100000\n\
         readonly={readonly}\nnetwork={network}\n{private}\nsecurity={security}"
    )
}

// ---------------------------------------------------------------------------------------------
// Fixtures de mission
// ---------------------------------------------------------------------------------------------

/// Un profil par défaut-refus, donc porteur de la posture restreinte : `RestrictedProfile` ne se
/// construit pas autrement, et c'est le type qui porte la garantie.
fn profiles() -> SeccompProfiles {
    SeccompProfiles {
        restricted: Some(
            RestrictedProfile::parse(
                "/etc/locus/seccomp/restricted.json",
                r#"{ "defaultAction": "SCMP_ACT_ERRNO", "syscalls": [] }"#,
            )
            .expect("profil par défaut-refus"),
        ),
    }
}

fn workload() -> Workload {
    Workload::new(
        "ghcr.io/locus/base@sha256:0123456789abcdef",
        vec!["/usr/bin/locus-run".to_owned()],
    )
    .expect("image avec digest et commande explicite")
}

fn mission(level: SandboxLevel, network: NetworkMode, mounts: Vec<Mount>) -> SandboxSpec {
    SandboxSpec::new(
        level,
        SandboxProfile::UntrustedRepository,
        network,
        mounts,
        ResourceSpec::new(1_500, 2 << 30, 128, 0, 600).expect("quotas non nuls"),
    )
    .expect("spécification valide")
}

fn backend<R: Runner>(runner: R) -> PodmanBackend<R> {
    PodmanBackend::new(runner, profiles(), workload())
}

// ---------------------------------------------------------------------------------------------
// L'invocation dit ce que le plan a décidé
// ---------------------------------------------------------------------------------------------

#[test]
fn le_workload_refuse_une_image_sans_digest() {
    let refused = Workload::new("ghcr.io/locus/base:latest", vec!["/bin/sh".to_owned()]);
    assert_eq!(
        refused,
        Err(InvocationError::ImageWithoutDigest {
            image: "ghcr.io/locus/base:latest".to_owned()
        }),
        "une étiquette désigne une image différente selon le jour"
    );
    assert_eq!(
        Workload::new("ghcr.io/locus/base@sha256:00", Vec::new()),
        Err(InvocationError::EmptyCommand)
    );
}

#[test]
fn un_namespace_demande_ne_produit_aucun_argument_et_un_namespace_partage_en_produit_un() {
    let isolated = plan(&mission(SandboxLevel::S3, NetworkMode::Deny, Vec::new())).expect("plan");
    let arguments =
        create_arguments(&isolated, &workload(), &profiles(), "locus-0001").expect("invocation");
    for flag in ["--userns=host", "--pid=host", "--ipc=host", "--uts=host"] {
        assert!(
            !arguments.contains(&flag.to_owned()),
            "{flag} partagerait un namespace que S3 crée"
        );
    }
    assert!(arguments.contains(&"--network=none".to_owned()));

    let bare = plan(&mission(SandboxLevel::S0, NetworkMode::Full, Vec::new())).expect("plan");
    let arguments =
        create_arguments(&bare, &workload(), &profiles(), "locus-0002").expect("invocation");
    for flag in [
        "--userns=host",
        "--pid=host",
        "--ipc=host",
        "--uts=host",
        "--cgroupns=host",
        "--network=host",
    ] {
        assert!(
            arguments.contains(&flag.to_owned()),
            "sans {flag}, Podman créerait le namespace que S0 ne demande pas"
        );
    }
}

#[test]
fn les_quotas_traversent_l_invocation_sans_se_perdre() {
    let confinement =
        plan(&mission(SandboxLevel::S2, NetworkMode::Full, Vec::new())).expect("plan");
    let arguments =
        create_arguments(&confinement, &workload(), &profiles(), "locus-0001").expect("invocation");
    for expected in [
        "--cpu-quota=150000",
        "--cpu-period=100000",
        "--memory=2147483648",
        "--pids-limit=128",
    ] {
        assert!(
            arguments.contains(&expected.to_owned()),
            "{expected} manque"
        );
    }
}

#[test]
fn l_horizon_n_est_pas_passe_au_runtime() {
    let confinement =
        plan(&mission(SandboxLevel::S2, NetworkMode::Full, Vec::new())).expect("plan");
    assert_eq!(confinement.wall_clock_seconds(), 600);
    let arguments =
        create_arguments(&confinement, &workload(), &profiles(), "locus-0001").expect("invocation");
    assert!(
        !arguments.iter().any(|argument| argument.contains("600")),
        "l'horizon est compté par le broker : le passer ferait croire qu'un runtime le tient"
    );
}

#[test]
fn la_posture_restreinte_sans_profil_est_refusee() {
    let confinement =
        plan(&mission(SandboxLevel::S2, NetworkMode::Full, Vec::new())).expect("plan");
    let refused = create_arguments(
        &confinement,
        &workload(),
        &SeccompProfiles::default(),
        "locus-0001",
    );
    assert_eq!(
        refused,
        Err(InvocationError::RestrictedProfileMissing),
        "revendiquer la posture restreinte avec le profil par défaut serait la revendiquer sans la tenir"
    );
}

#[test]
fn un_montage_lecture_seule_le_reste_jusqu_a_l_argument() {
    let confinement = plan(&mission(
        SandboxLevel::S2,
        NetworkMode::Full,
        vec![Mount::new("/srv/corpus", "/work", MountMode::ReadOnly).expect("licite")],
    ))
    .expect("plan");
    let arguments =
        create_arguments(&confinement, &workload(), &profiles(), "locus-0001").expect("invocation");
    assert!(
        arguments.contains(&"type=bind,source=/srv/corpus,destination=/work,ro".to_owned()),
        "{arguments:?}"
    );
}

#[test]
fn l_image_et_la_commande_ferment_la_ligne() {
    let confinement =
        plan(&mission(SandboxLevel::S2, NetworkMode::Full, Vec::new())).expect("plan");
    let arguments =
        create_arguments(&confinement, &workload(), &profiles(), "locus-0001").expect("invocation");
    let tail = &arguments[arguments.len() - 2..];
    assert_eq!(
        tail,
        [
            "ghcr.io/locus/base@sha256:0123456789abcdef".to_owned(),
            "/usr/bin/locus-run".to_owned()
        ],
        "un argument après la commande serait lu comme un argument de la commande"
    );
}

// ---------------------------------------------------------------------------------------------
// Le cycle de vie
// ---------------------------------------------------------------------------------------------

#[test]
fn creer_demarrer_arreter_passent_les_bons_arguments() {
    let runner = ScriptedRunner::new(vec![ok("  \n"), ok(""), ok("")]);
    let mut execd = backend(runner);
    let id = execd
        .create(&mission(SandboxLevel::S3, NetworkMode::Deny, Vec::new()))
        .expect("création");
    assert_eq!(id.as_str(), "locus-0001");
    execd.start(&id).expect("démarrage");
    execd.stop(&id).expect("arrêt");

    assert_eq!(execd.runner_call(0)[0], "create");
    assert_eq!(execd.runner_call(1), vec!["start", "locus-0001"]);
    assert_eq!(execd.runner_call(2), vec!["stop", "locus-0001"]);
}

#[test]
fn une_sandbox_inconnue_est_refusee_sans_lancer_quoi_que_ce_soit() {
    let mut execd = backend(ScriptedRunner::new(Vec::new()));
    let ghost = SandboxId::new("locus-9999").expect("identifiant non vide");
    assert_eq!(
        execd.start(&ghost),
        Err(RuntimeError::Unknown { id: ghost.clone() })
    );
    assert_eq!(
        execd.attestation(&ghost),
        Err(RuntimeError::Unknown { id: ghost })
    );
}

#[test]
fn un_hote_sans_podman_le_dit_au_lieu_de_pretendre() {
    let mut execd = backend(AbsentRuntime);
    let refused = execd.create(&mission(SandboxLevel::S2, NetworkMode::Full, Vec::new()));
    assert!(
        matches!(refused, Err(RuntimeError::Unavailable { .. })),
        "un backend sans runtime doit refuser, pas rendre un identifiant : {refused:?}"
    );
}

#[test]
fn un_code_de_sortie_non_nul_devient_une_erreur_qui_porte_stderr() {
    let runner = ScriptedRunner::new(vec![Execution {
        code: 125,
        stdout: String::new(),
        stderr: "Error: short-name resolution enforced but cannot prompt".to_owned(),
    }]);
    let mut execd = backend(runner);
    let refused = execd.create(&mission(SandboxLevel::S2, NetworkMode::Full, Vec::new()));
    match refused {
        Err(RuntimeError::Unavailable { detail }) => {
            assert!(detail.contains("125"), "{detail}");
            assert!(detail.contains("short-name"), "{detail}");
        }
        other => panic!("un échec de podman doit remonter tel quel : {other:?}"),
    }
}

#[test]
fn un_niveau_hors_de_portee_est_refuse_avant_tout_lancement() {
    let runner = ScriptedRunner::new(Vec::new());
    let mut execd = backend(runner);
    let refused = execd.create(&mission(SandboxLevel::S4, NetworkMode::Full, Vec::new()));
    assert!(matches!(refused, Err(RuntimeError::Unsupported { .. })));
    assert_eq!(
        execd.runner_calls(),
        0,
        "décider après avoir lancé laisserait derrière soi ce qui a déjà été créé"
    );
}

// ---------------------------------------------------------------------------------------------
// L'attestation vient de l'observation
// ---------------------------------------------------------------------------------------------

#[test]
fn l_attestation_interroge_le_runtime_et_cite_ce_qu_il_a_dit() {
    let mut execd = backend(ScriptedRunner::confined());
    let id = execd
        .create(&mission(SandboxLevel::S3, NetworkMode::Deny, Vec::new()))
        .expect("création");
    execd.start(&id).expect("démarrage");
    let attested = execd.attestation(&id).expect("attestation");

    assert_eq!(execd.runner_call(2), inspect_arguments("locus-0001"));
    assert_eq!(attested.applied_level(), SandboxLevel::S3);
    assert_eq!(attested.attested_by(), "podman-rootless");
    assert!(
        attested
            .evidence()
            .iter()
            .any(|line| line == "network=none"),
        "la preuve doit citer ce qui a été lu : {:?}",
        attested.evidence()
    );
}

/// Le test qui décide de la valeur de ce module. Le plan demandait `S3` ; le runtime a rendu un
/// conteneur au réseau de l'hôte. Un driver qui attesterait sa demande dirait `S3` et
/// `conformance` serait content.
#[test]
fn un_confinement_plus_faible_que_demande_se_voit_dans_l_attestation() {
    let runner = ScriptedRunner::new(vec![
        ok(""),
        ok(""),
        ok(&observations("host", "true", "no-new-privileges")),
    ]);
    let mut execd = backend(runner);
    let demanded = mission(SandboxLevel::S3, NetworkMode::Deny, Vec::new());
    let id = execd.create(&demanded).expect("création");
    execd.start(&id).expect("démarrage");
    let attested = execd.attestation(&id).expect("attestation");

    assert_eq!(
        attested.applied_level(),
        SandboxLevel::S2,
        "le réseau de l'hôte n'est pas un réseau isolé, quoi qu'on ait demandé"
    );
    assert!(
        conformance(&demanded, &attested).is_err(),
        "un downgrade sans approbation doit être refusé par W4.a"
    );
}

#[test]
fn un_conteneur_conforme_passe_la_confrontation_de_w4a() {
    let mut execd = backend(ScriptedRunner::confined());
    let demanded = mission(SandboxLevel::S3, NetworkMode::Deny, Vec::new());
    let id = execd.create(&demanded).expect("création");
    execd.start(&id).expect("démarrage");
    let attested = execd.attestation(&id).expect("attestation");
    assert_eq!(
        conformance(&demanded, &attested),
        Ok(Conformance::Conforms),
        "la boucle W4.a → W4.d → W4.a doit se fermer sur une mission ordinaire"
    );
}

#[test]
fn les_namespaces_partages_font_descendre_le_niveau_observe() {
    let cases = [
        ("userns=host", SandboxLevel::S0),
        ("pidns=host", SandboxLevel::S1),
        ("readonly=false", SandboxLevel::S1),
    ];
    for (relaxed, expected) in cases {
        let (key, value) = relaxed.split_once('=').expect("paire");
        let observed = observations("none", "true", "no-new-privileges")
            .lines()
            .map(|line| {
                if line.starts_with(&format!("{key}=")) {
                    format!("{key}={value}")
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let runner = ScriptedRunner::new(vec![ok(""), ok(&observed)]);
        let mut execd = backend(runner);
        let id = execd
            .create(&mission(SandboxLevel::S3, NetworkMode::Deny, Vec::new()))
            .expect("création");
        assert_eq!(
            execd.attestation(&id).expect("attestation").applied_level(),
            expected,
            "avec {relaxed}, le niveau observé devrait être {}",
            expected.code()
        );
    }
}

#[test]
fn sans_no_new_privileges_rien_n_est_confine() {
    let runner = ScriptedRunner::new(vec![ok(""), ok(&observations("none", "true", ""))]);
    let mut execd = backend(runner);
    let id = execd
        .create(&mission(SandboxLevel::S3, NetworkMode::Deny, Vec::new()))
        .expect("création");
    assert_eq!(
        execd.attestation(&id).expect("attestation").applied_level(),
        SandboxLevel::S0
    );
}

#[test]
fn un_champ_d_inspection_manquant_empeche_d_attester() {
    let truncated = "status=running\nmemory=2147483648";
    let runner = ScriptedRunner::new(vec![ok(""), ok(truncated)]);
    let mut execd = backend(runner);
    let id = execd
        .create(&mission(SandboxLevel::S2, NetworkMode::Full, Vec::new()))
        .expect("création");
    match execd.attestation(&id) {
        Err(RuntimeError::Unsupported { capability }) => {
            assert!(capability.contains("champ d'inspection"), "{capability}");
        }
        other => panic!("un champ absent n'est pas une valeur par défaut : {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------
// Accès au double, pour lire ce qui a été demandé
// ---------------------------------------------------------------------------------------------

trait Inspectable {
    fn runner_call(&self, index: usize) -> Vec<String>;
    fn runner_calls(&self) -> usize;
}

impl Inspectable for PodmanBackend<ScriptedRunner> {
    fn runner_call(&self, index: usize) -> Vec<String> {
        self.runner().call(index)
    }

    fn runner_calls(&self) -> usize {
        self.runner().calls.lock().expect("verrou").len()
    }
}

// ---------------------------------------------------------------------------------------------
// W5.r — aucun appel au runtime ne dure indéfiniment
// ---------------------------------------------------------------------------------------------

/// **Le seul chemin qui lance un vrai processus était le seul qu'aucun test ne traversait.**
///
/// `SystemRunner::run` appelait `Command::output()`, qui attend sans limite — contre la règle du
/// dépôt, « timeouts et cancellation », à l'unique endroit qu'aucun test ne traversait.
///
/// Il a été trouvé en cherchant autre chose : le job de CI de `W5.r` a paru pendre. Il ne pendait
/// pas — il avait fini en trois minutes et demie, et l'état rapporté était périmé. Le défaut trouvé
/// en route est réel quand même : `W5.r` fait passer les appels non bornés d'une poignée à
/// quatre-vingts par campagne, et un broker privilégié qui attend sans fin ne rapporte rien.
#[test]
fn un_appel_qui_ne_rend_pas_la_main_est_abandonne() {
    let runner = SystemRunner::new()
        .with_program("sleep")
        .with_budget(Duration::from_millis(100));

    // Trente secondes, et pas six cents : trois cents fois le budget suffisent à prouver que la
    // borne mord, et si un jour elle cesse de mordre, ce test durera trente secondes au lieu de dix
    // minutes. Un test qui se plante doit le dire vite.
    let started = Instant::now();
    let outcome = runner.run(&["30".to_owned()]);
    let waited = started.elapsed();

    match outcome {
        Err(RuntimeError::Unavailable { detail }) => {
            assert!(
                detail.contains("n'a pas rendu la main"),
                "le motif dit que l'appel a été abandonné, pas qu'un binaire manque : {detail}"
            );
        }
        other => panic!("un appel sans fin doit être abandonné, pas attendu : {other:?}"),
    }
    assert!(
        waited < Duration::from_secs(5),
        "abandonné veut dire tout de suite : {waited:?}"
    );
}

/// Et un appel qui répond dans son budget répond **normalement** — sortie comprise.
///
/// Sans cette moitié, une borne qui tuerait tout passerait le test précédent.
#[test]
fn un_appel_qui_repond_dans_son_budget_rend_ce_qu_il_a_ecrit() {
    let runner = SystemRunner::new()
        .with_program("echo")
        .with_budget(Duration::from_secs(10));
    let execution = runner.run(&["locus".to_owned()]).expect("echo répond");
    assert_eq!(execution.code, 0);
    assert_eq!(execution.stdout.trim(), "locus");
    assert!(execution.stderr.is_empty());
}

/// **Et un refus rend son code**, pas un zéro.
///
/// C'est la moitié qui manquait : `echo` réussit toujours, donc un lanceur qui écrirait `code: 0` en
/// dur passait le test précédent. Or tout le harnais de sondes lit ce code — `W5.m` l'a mis à côté
/// du verdict, `W5.n` y a trouvé le 255 — et un zéro inventé y ferait lire un succès partout.
#[test]
fn un_appel_qui_echoue_rend_son_code_et_ce_qu_il_a_dit() {
    let runner = SystemRunner::new()
        .with_program("sh")
        .with_budget(Duration::from_secs(10));
    let execution = runner
        .run(&["-c".to_owned(), "printf 'refus\\n' >&2; exit 3".to_owned()])
        .expect("sh répond");
    assert_eq!(execution.code, 3, "le code vient du processus, pas de nous");
    assert_eq!(execution.stderr.trim(), "refus");
    assert!(execution.stdout.is_empty());
}
