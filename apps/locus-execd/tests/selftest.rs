//! Test de sortie de W4.d.3 — ADR 0004, `docs/SPEC_V1.md` §21.6, §32.3.
//!
//! **La suite de W4.b rend un `Standing` pour ce backend, et une sonde qui n'a pas pu être lancée
//! est `NotRun` avec sa raison — jamais un succès.**
//!
//! C'est le moment où W4 se referme sur lui-même. W4.b avait écrit ce qu'il faut tenter et à quel
//! niveau ça doit échouer, sans backend pour le tenter ; W4.d.2 avait écrit le backend, sans rien
//! qui le juge. Ici la suite passe contre le driver, et le seul résultat interdit est qu'un hôte
//! sans runtime obtienne la confiance faute de contre-preuve.

use std::sync::Mutex;

use locus_execd::linux::{
    Execution, PROBE_COMMANDS, PodmanBackend, RestrictedProfile, Runner, SeccompProfiles,
    UNREACHABLE_RUNTIME, Workload, assess, certify, exec_arguments, probe_command, run_suite,
};
use locus_execd::{RuntimeError, RuntimePort, SandboxId};
use locus_execution::{
    Mount, NetworkMode, Observed, ResourceSpec, SUITE, SandboxLevel, SandboxProfile, SandboxSpec,
    Standing, Verdict,
};

// ---------------------------------------------------------------------------------------------
// Un runtime scripté, qui répond à chaque sonde ce qu'on lui a dit de répondre
// ---------------------------------------------------------------------------------------------

/// Rend `blocked` pour chaque sonde dont le nom est listé, et `0` pour les autres.
struct ProbingRunner {
    blocked: Vec<&'static str>,
    calls: Mutex<Vec<Vec<String>>>,
}

impl ProbingRunner {
    fn new(blocked: Vec<&'static str>) -> Self {
        Self {
            blocked,
            calls: Mutex::new(Vec::new()),
        }
    }

    /// Un runtime qui contient tout : chaque sonde échoue.
    fn airtight() -> Self {
        Self::new(SUITE.iter().map(|probe| probe.name).collect())
    }
}

impl Runner for ProbingRunner {
    fn run(&self, arguments: &[String]) -> Result<Execution, RuntimeError> {
        self.calls.lock().expect("verrou").push(arguments.to_vec());
        let joined = arguments.join(" ");
        let blocked = self.blocked.iter().any(|name| {
            probe_command(name).is_some_and(|command| joined.ends_with(&command.join(" ")))
        });
        Ok(Execution {
            code: i32::from(blocked),
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}

/// Un runtime qui n'exécute rien : le cas d'un hôte sans Podman.
struct AbsentRuntime;

impl Runner for AbsentRuntime {
    fn run(&self, _arguments: &[String]) -> Result<Execution, RuntimeError> {
        Err(RuntimeError::Unavailable {
            detail: "podman : No such file or directory (os error 2)".to_owned(),
        })
    }
}

fn workload() -> Workload {
    Workload::new(
        "ghcr.io/locus/base@sha256:0123456789abcdef",
        vec!["/usr/bin/locus-run".to_owned()],
    )
    .expect("image avec digest")
}

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

fn mission(level: SandboxLevel) -> SandboxSpec {
    let network = if level >= SandboxLevel::S3 {
        NetworkMode::Deny
    } else {
        NetworkMode::Full
    };
    SandboxSpec::new(
        level,
        SandboxProfile::UntrustedRepository,
        network,
        Vec::<Mount>::new(),
        ResourceSpec::new(1_000, 1 << 30, 64, 0, 300).expect("quotas non nuls"),
    )
    .expect("spécification valide")
}

fn backend<R: Runner>(runner: R) -> PodmanBackend<R> {
    PodmanBackend::new(runner, profiles(), workload())
}

fn started<R: Runner>(runner: R) -> (PodmanBackend<R>, SandboxId) {
    let mut execd = backend(runner);
    let id = execd.create(&mission(SandboxLevel::S3)).expect("création");
    execd.start(&id).expect("démarrage");
    (execd, id)
}

// ---------------------------------------------------------------------------------------------
// La couverture : pas de sonde sans commande, pas de commande orpheline
// ---------------------------------------------------------------------------------------------

#[test]
fn les_seize_sondes_ont_chacune_une_commande() {
    for probe in &SUITE {
        assert!(
            probe_command(probe.name).is_some(),
            "« {} » n'a aucune commande : elle serait Inconclusive sans que personne sache pourquoi",
            probe.name
        );
    }
}

#[test]
fn aucune_commande_ne_vise_une_sonde_qui_n_existe_pas() {
    for (name, _) in PROBE_COMMANDS {
        assert!(
            SUITE.iter().any(|probe| probe.name == name),
            "« {name} » ne correspond à aucune sonde de la suite"
        );
    }
    assert_eq!(PROBE_COMMANDS.len(), SUITE.len());
}

#[test]
fn le_rapport_porte_chaque_sonde_exactement_une_fois() {
    let (execd, id) = started(ProbingRunner::airtight());
    let results = run_suite(&execd, &id);
    assert_eq!(results.len(), SUITE.len());
    for probe in &SUITE {
        assert_eq!(
            results
                .iter()
                .filter(|(name, _)| *name == probe.name)
                .count(),
            1,
            "« {} » devrait apparaître une fois et une seule",
            probe.name
        );
    }
}

// ---------------------------------------------------------------------------------------------
// La convention de sortie
// ---------------------------------------------------------------------------------------------

#[test]
fn un_code_nul_veut_dire_que_la_sonde_a_reussi() {
    let (execd, id) = started(ProbingRunner::new(Vec::new()));
    let results = run_suite(&execd, &id);
    assert!(
        results
            .iter()
            .all(|(_, observed)| *observed == Observed::Succeeded),
        "un runtime qui laisse tout passer doit produire seize succès, pas seize blocages"
    );
}

#[test]
fn la_sonde_est_lancee_dans_la_sandbox_qui_tourne() {
    let (execd, id) = started(ProbingRunner::airtight());
    let _ = run_suite(&execd, &id);
    let expected = exec_arguments(
        &id,
        probe_command("write_outside_workspace").expect("commande"),
    );
    assert_eq!(expected[0], "exec");
    assert_eq!(expected[1], "locus-0001");
}

// ---------------------------------------------------------------------------------------------
// Ce qui n'a pas pu être lancé n'a rien prouvé
// ---------------------------------------------------------------------------------------------

#[test]
fn un_hote_sans_runtime_ne_prouve_rien_et_n_obtient_pas_la_confiance() {
    let mut execd = backend(AbsentRuntime);
    let refused = execd.create(&mission(SandboxLevel::S3));
    assert!(
        matches!(refused, Err(RuntimeError::Unavailable { .. })),
        "sans runtime, la sandbox ne se crée pas : {refused:?}"
    );
}

#[test]
fn une_sonde_non_lancee_est_notrun_avec_sa_raison() {
    // La sandbox est créée et démarrée par un runtime qui répond, puis le runtime disparaît :
    // c'est le cas d'un Podman tué en cours de campagne, et le seul où `run_suite` rencontre un
    // échec de lancement sonde par sonde.
    let (execd, id) = started(VanishingRuntime::new(2));
    let results = run_suite(&execd, &id);

    assert!(
        results.iter().all(|(_, observed)| matches!(
            observed,
            Observed::NotRun {
                reason
            } if *reason == UNREACHABLE_RUNTIME
        )),
        "un runtime disparu ne bloque pas les sondes, il empêche de les lancer : {results:?}"
    );

    let verdict = assess(&execd, &id, SandboxLevel::S3);
    match verdict {
        Standing::NotTrusted { level, blocking } => {
            assert_eq!(level, SandboxLevel::S3);
            assert_eq!(
                blocking.len(),
                SUITE.len(),
                "les seize sondes sont critiques : aucune ne s'efface"
            );
            assert!(
                blocking
                    .iter()
                    .all(|verdict| matches!(verdict, Verdict::Inconclusive { .. }))
            );
        }
        Standing::Trusted { .. } => {
            panic!("accorder la confiance faute de contre-preuve est le seul résultat interdit")
        }
    }
}

/// Un runtime qui répond aux `n` premiers appels puis disparaît.
struct VanishingRuntime {
    remaining: Mutex<u32>,
}

impl VanishingRuntime {
    const fn new(remaining: u32) -> Self {
        Self {
            remaining: Mutex::new(remaining),
        }
    }
}

impl Runner for VanishingRuntime {
    fn run(&self, _arguments: &[String]) -> Result<Execution, RuntimeError> {
        let mut remaining = self.remaining.lock().expect("verrou");
        if *remaining == 0 {
            return Err(RuntimeError::Unavailable {
                detail: "le service podman s'est arrêté".to_owned(),
            });
        }
        *remaining -= 1;
        Ok(Execution {
            code: 0,
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Le verdict
// ---------------------------------------------------------------------------------------------

#[test]
fn un_backend_qui_contient_tout_est_trusted_au_niveau_annonce() {
    let (execd, id) = started(ProbingRunner::airtight());
    assert_eq!(
        assess(&execd, &id, SandboxLevel::S3),
        Standing::Trusted {
            level: SandboxLevel::S3
        }
    );
}

#[test]
fn une_sonde_qui_echappe_retire_la_confiance_et_se_nomme() {
    let leaky: Vec<&'static str> = SUITE
        .iter()
        .map(|probe| probe.name)
        .filter(|name| *name != "read_host_secret_files")
        .collect();
    let (execd, id) = started(ProbingRunner::new(leaky));

    match assess(&execd, &id, SandboxLevel::S3) {
        Standing::NotTrusted { blocking, .. } => {
            assert_eq!(blocking.len(), 1);
            assert!(
                matches!(
                    &blocking[0],
                    Verdict::Escaped { probe, .. } if *probe == "read_host_secret_files"
                ),
                "{blocking:?}"
            );
        }
        Standing::Trusted { .. } => panic!("une sonde échappée ne laisse pas un backend trusted"),
    }
}

#[test]
fn au_niveau_ou_rien_n_est_promis_tout_contenir_n_est_pas_un_echec_de_confiance() {
    let (execd, id) = started(ProbingRunner::airtight());
    let verdict = assess(&execd, &id, SandboxLevel::S0);
    assert_eq!(
        verdict,
        Standing::Trusted {
            level: SandboxLevel::S0
        },
        "le sur-confinement se signale, il ne retire pas la confiance — W4.b l'a tranché"
    );
}

// ---------------------------------------------------------------------------------------------
// Le cycle complet
// ---------------------------------------------------------------------------------------------

#[test]
fn certify_cree_demarre_eprouve_et_arrete() {
    let mut execd = backend(ProbingRunner::airtight());
    let verdict = certify(&mut execd, &mission(SandboxLevel::S2), SandboxLevel::S2)
        .expect("campagne menée à son terme");
    assert_eq!(
        verdict,
        Standing::Trusted {
            level: SandboxLevel::S2
        }
    );

    let calls = execd.runner().calls.lock().expect("verrou");
    assert_eq!(calls[0][0], "create");
    assert_eq!(calls[1][0], "start");
    assert_eq!(
        calls.last().expect("au moins un appel")[0],
        "stop",
        "une campagne qui laisserait le conteneur tourner finirait par saturer l'hôte"
    );
    assert_eq!(calls.len(), SUITE.len() + 3);
}

#[test]
fn certify_refuse_un_niveau_que_le_backend_ne_sait_pas_tenir() {
    let mut execd = backend(ProbingRunner::airtight());
    let refused = certify(&mut execd, &mission(SandboxLevel::S4), SandboxLevel::S4);
    assert!(
        matches!(refused, Err(RuntimeError::Unsupported { .. })),
        "rendre un Standing sur une sandbox qui n'existe pas serait un verdict sur rien : {refused:?}"
    );
}
