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
use std::time::Duration;

use locus_execd::linux::{
    Execution, INCONCLUSIVE_EXIT_CODE, LAUNCH_ATTEMPTS, PROBE_COMMANDS, PodmanBackend,
    RestrictedProfile, Runner, SeccompProfiles, TRANSIENT_EXIT_CODES, Trial, UNREACHABLE_RUNTIME,
    UNREACHABLE_TARGET_EXIT_CODE, UNRUNNABLE_EXIT_CODES, Workload, assess, certify, exec_arguments,
    probe_command, run_suite, unrunnable,
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

/// Un runtime qui crée volontiers et refuse de démarrer.
///
/// Le cas le plus silencieux d'une fuite de nom : rien ne tourne, donc rien ne la signale, et
/// pourtant le conteneur existe et son nom est pris.
#[derive(Default)]
struct FailingStart {
    calls: Mutex<Vec<Vec<String>>>,
}

impl Runner for FailingStart {
    fn run(&self, arguments: &[String]) -> Result<Execution, RuntimeError> {
        self.calls.lock().expect("verrou").push(arguments.to_vec());
        let code = i32::from(arguments.first().is_some_and(|verb| verb == "start"));
        Ok(Execution {
            code,
            stdout: String::new(),
            stderr: if code == 0 {
                String::new()
            } else {
                "le démarrage a échoué".to_owned()
            },
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

/// Le backend des tests, **sans pause de reprise**.
///
/// `W5.o` fait retenter une sonde que le runtime n'a pas pu lancer, avec des pauses qui doublent et
/// dont la somme couvre le pire cas contre un vrai runtime. Contre un double, ces pauses ne mesurent
/// rien et coûtent tout : la suite dormait cinquante secondes pour éprouver une reprise dont chaque
/// itération est immédiate.
///
/// Les mettre à zéro ici n'affaiblit pas ce que les tests vérifient — le **nombre** de tentatives —
/// et c'est ce nombre, pas la durée, qui décide si une sonde a été mesurée.
fn backend<R: Runner>(runner: R) -> PodmanBackend<R> {
    PodmanBackend::new(runner, profiles(), workload()).with_launch_pause(Duration::ZERO)
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
                .filter(|trial| trial.name() == probe.name)
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
            .all(|trial| trial.observed() == Observed::Succeeded),
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
        results.iter().all(|trial| matches!(
            trial.observed(),
            Observed::NotRun {
                reason
            } if reason == UNREACHABLE_RUNTIME
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

/// **Le cycle se termine par un retrait, pas par un arrêt.**
///
/// La version précédente de ce test s'arrêtait à `stop`, avec la bonne raison — « une campagne qui
/// laisserait le conteneur tourner finirait par saturer l'hôte » — et une conclusion insuffisante.
/// `podman stop` arrête les processus et laisse le **nom** et la **couche inscriptible** ; c'est le
/// nom qui manque au suivant. Trois passages de CI ont échoué sur « the container name
/// `locus-0001` is already in use » avant qu'on le remarque, et le harnais lisait cette erreur là
/// où il attendait un verdict de confinement.
///
/// Les deux appels sont donc épinglés **dans l'ordre**, et pas seulement le dernier : un retrait
/// sans arrêt préalable marcherait ici — `rm --force` y pourvoit — mais ferait disparaître la
/// distinction que le port porte délibérément entre « ne tourne plus » et « n'existe plus ».
#[test]
fn certify_cree_demarre_eprouve_arrete_puis_retire() {
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
    let tail: Vec<&str> = calls
        .iter()
        .rev()
        .take(2)
        .map(|call| call[0].as_str())
        .collect();
    assert_eq!(
        tail,
        vec!["rm", "stop"],
        "arrêter n'est pas retirer : sans le second, le nom reste pris et le prochain conteneur          échoue là où on attend un verdict"
    );
    assert_eq!(calls.len(), SUITE.len() + 4);
}

/// **Un démarrage qui échoue retire quand même**, et c'est le cas le plus silencieux.
///
/// Rien ne tourne, donc rien ne signale la fuite — mais le nom reste pris. La version précédente de
/// `certify` rendait l'erreur par `?` et abandonnait un conteneur créé et jamais démarré.
#[test]
fn un_demarrage_qui_echoue_ne_laisse_pas_le_nom_pris() {
    let mut execd = backend(FailingStart::default());
    certify(&mut execd, &mission(SandboxLevel::S2), SandboxLevel::S2)
        .expect_err("un démarrage qui échoue n'a rien à éprouver");

    let calls = execd.runner().calls.lock().expect("verrou");
    assert!(
        calls.iter().any(|call| call[0] == "rm"),
        "le conteneur a été créé : il doit être retiré, même si rien n'a jamais tourné dedans —          {calls:?}"
    );
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

// ---------------------------------------------------------------------------------------------
// W5.c — la sonde absente n'est pas une sonde bloquée
// ---------------------------------------------------------------------------------------------

/// Rend le code donné pour chaque sonde nommée, et `1` — un blocage franc — pour les autres.
struct BrokenImage {
    missing: Vec<&'static str>,
    code: i32,
}

impl Runner for BrokenImage {
    fn run(&self, arguments: &[String]) -> Result<Execution, RuntimeError> {
        // Le cycle de vie réussit : ce qu'on met à l'épreuve est la lecture des codes de sortie
        // des sondes, pas la création du conteneur.
        if arguments.first().map(String::as_str) != Some("exec") {
            return Ok(Execution {
                code: 0,
                stdout: String::new(),
                stderr: String::new(),
            });
        }
        let joined = arguments.join(" ");
        let absent = self.missing.iter().any(|name| {
            probe_command(name).is_some_and(|command| joined.ends_with(&command.join(" ")))
        });
        Ok(Execution {
            code: if absent { self.code } else { 1 },
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}

/// Le test qui corrige W4.d.3. Ce commit-là lisait **tout** code non nul comme `Blocked`, et
/// déclarait au ledger que six sondes visant des binaires absents « se lisent comme des blocages ».
/// La dette était réelle, mais dans le mauvais sens : une image incomplète rendait le backend
/// **plus** digne de confiance, puisque `Blocked` est exactement ce qu'un niveau promet.
#[test]
fn une_sonde_absente_de_l_image_ne_prouve_pas_l_isolation() {
    let runner = BrokenImage {
        missing: vec!["exceed_cpu_quota", "open_outbound_connection"],
        code: 127,
    };
    let mut execd = backend(runner);
    let id = execd.create(&mission(SandboxLevel::S3)).expect("création");

    let results = run_suite(&execd, &id);
    let absent: Vec<Observed> = results
        .iter()
        .filter(|trial| {
            trial.name() == "exceed_cpu_quota" || trial.name() == "open_outbound_connection"
        })
        .map(Trial::observed)
        .collect();
    assert_eq!(absent.len(), 2);
    assert!(
        absent
            .iter()
            .all(|observed| matches!(observed, Observed::NotRun { .. })),
        "127 dit « la sonde est absente », pas « la sonde a été contenue » : {absent:?}"
    );

    match assess(&execd, &id, SandboxLevel::S3) {
        Standing::NotTrusted { blocking, .. } => {
            assert_eq!(
                blocking.len(),
                2,
                "les deux sondes absentes empêchent la confiance, les quatorze autres tiennent"
            );
            assert!(
                blocking
                    .iter()
                    .all(|verdict| matches!(verdict, Verdict::Inconclusive { .. }))
            );
        }
        Standing::Trusted { .. } => {
            panic!("une image incomplète ne rend pas un backend plus digne de confiance")
        }
    }
}

#[test]
fn les_trois_codes_reserves_disent_chacun_ce_qui_manque() {
    for (code, expected) in UNRUNNABLE_EXIT_CODES {
        assert_eq!(unrunnable(code), Some(expected));
        let mut execd = backend(BrokenImage {
            missing: vec!["escalate_to_root"],
            code,
        });
        let id = execd.create(&mission(SandboxLevel::S3)).expect("création");
        let results = run_suite(&execd, &id);
        let observed = results
            .iter()
            .find(|trial| trial.name() == "escalate_to_root")
            .map(Trial::observed)
            .expect("la sonde est au rapport");
        assert_eq!(
            observed,
            Observed::NotRun { reason: expected },
            "le code {code} doit dire ce qui manque"
        );
    }
}

#[test]
fn un_blocage_franc_reste_un_blocage() {
    for code in [1, 2, 13, 137] {
        assert_eq!(
            unrunnable(code),
            None,
            "le code {code} est un refus du noyau, pas une sonde manquante"
        );
    }
    let mut execd = backend(BrokenImage {
        missing: Vec::new(),
        code: 1,
    });
    let id = execd.create(&mission(SandboxLevel::S3)).expect("création");
    assert_eq!(
        assess(&execd, &id, SandboxLevel::S3),
        Standing::Trusted {
            level: SandboxLevel::S3
        },
        "un backend qui contient tout franchement reste trusted"
    );
}

/// La table elle-même, épinglée par son contenu — la leçon de W4.d.4 appliquée ici. Le test qui
/// itère sur `UNRUNNABLE_EXIT_CODES` reste vrai quelle que soit la table : il vérifie la mécanique,
/// pas ce qu'elle couvre. Celui-ci nomme les quatre codes et dit pourquoi chacun est réservé.
///
/// **255 est arrivé en quatrième, et il a coûté cher avant d'être vu.** Toutes les sondes situées
/// après `exceed_pid_quota` le rendaient : elle sature le quota de PID, `podman exec` ne peut plus
/// forker, et il abandonne avec son code générique. Les quatre suivantes étaient donc rapportées
/// « bloquées », c'est-à-dire **contenues**, alors qu'elles n'avaient pas tourné du tout — trois
/// sur-confinements qui n'existaient pas, et un « tient » que personne n'avait mérité.
#[test]
fn la_table_couvre_les_quatre_codes_que_posix_et_podman_reservent() {
    let codes: Vec<i32> = UNRUNNABLE_EXIT_CODES
        .into_iter()
        .map(|(code, _)| code)
        .collect();
    assert_eq!(
        codes,
        vec![
            // Podman : le runtime n'a pas su démarrer la commande.
            125, // POSIX : la commande existe mais n'est pas exécutable.
            126, // POSIX : la commande est introuvable.
            127, // Podman : son code d'erreur générique, rendu quand l'exec n'a pas eu lieu.
            255,
        ],
        "en retirer un ferait relire ce code comme un blocage, donc comme une preuve d'isolation"
    );
}

/// **Aucune sonde de la suite ne sort volontairement en 255.**
///
/// C'est ce qui autorise à lire ce code comme « n'a pas été lancée » plutôt que comme un verdict.
/// Si une sonde venait à l'utiliser, le catalogage deviendrait faux et masquerait son résultat —
/// le test le dirait avant.
#[test]
fn aucune_sonde_ne_sort_volontairement_en_255() {
    for (name, command) in PROBE_COMMANDS {
        let joined = command.join(" ");
        assert!(
            !joined.contains("exit 255"),
            "« {name} » utiliserait un code que le harnais lit comme « pas lancée » : {joined}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// W5.d — les sondes voyagent avec le harnais
// ---------------------------------------------------------------------------------------------

/// Aucune sonde ne dépend d'un binaire que l'image devrait porter. C'est une garantie **par
/// absence** : le chemin qui les hébergeait autrefois ne doit plus apparaître nulle part.
#[test]
fn aucune_sonde_n_attend_un_binaire_de_l_image() {
    for (name, command) in PROBE_COMMANDS {
        let joined = command.join(" ");
        assert!(
            !joined.contains("/usr/libexec"),
            "« {name} » attend encore un binaire de l'image : {joined}"
        );
    }
}

/// Le trou que ce test ferme est celui de W5.c, une couche plus bas : une erreur d'analyse dans un
/// script rend le code 2, que le harnais lit comme un blocage, donc comme une preuve d'isolation.
/// `sh -n` analyse sans exécuter, et il tourne ici, sur cette machine, sans conteneur.
///
/// # Ce que `sh -n` attrape, et ce qu'il n'attrape pas
///
/// Il attrape ce que le **shell** ne sait pas analyser : un `do` sans `done`, un guillemet non
/// fermé, une substitution non terminée. Il n'attrape pas la mauvaise utilisation d'une
/// **commande** — un `[` sans `]` passe l'analyse et échoue à l'exécution, parce que `[` est un
/// programme et non une construction du langage. Une mutation l'a montré : le crochet retiré, ce
/// test restait vert. La borne est donc écrite plutôt que supposée, et ce qui reste à couvrir
/// demande une sandbox réelle — c'est la dette nommée au ledger.
#[test]
fn chaque_sonde_est_du_shell_syntaxiquement_valide() {
    let mut checked = 0;
    for (name, command) in PROBE_COMMANDS {
        assert_eq!(command.first().copied(), Some("sh"), "« {name} »");
        assert_eq!(command.get(1).copied(), Some("-c"), "« {name} »");
        let script = command.get(2).copied().expect("un script");
        assert!(!script.trim().is_empty(), "« {name} » a un script vide");

        let checked_syntax = std::process::Command::new("sh")
            .arg("-n")
            .arg("-c")
            .arg(script)
            .output()
            .expect("sh est disponible sur la machine de test");
        assert!(
            checked_syntax.status.success(),
            "« {name} » n'est pas du shell valide : {}\n{script}",
            String::from_utf8_lossy(&checked_syntax.stderr)
        );
        checked += 1;
    }
    assert_eq!(
        checked,
        SUITE.len(),
        "les seize sondes doivent être analysées, pas seulement celles qui se laissent lire"
    );
}

/// Une sonde qui n'a pas pu conclure ne dit pas « contenu ». Troisième instance du même refus,
/// après le 127 de W5.c et le `NotRun` de W4.b : cette fois ce n'est pas la sonde qui manque, c'est
/// ce dont la sonde avait besoin.
#[test]
fn une_sonde_qui_n_a_pas_pu_conclure_est_notrun() {
    assert!(unrunnable(INCONCLUSIVE_EXIT_CODE).is_some());

    let mut execd = backend(BrokenImage {
        missing: vec!["exceed_memory_quota"],
        code: INCONCLUSIVE_EXIT_CODE,
    });
    let id = execd.create(&mission(SandboxLevel::S3)).expect("création");
    let results = run_suite(&execd, &id);
    let observed = results
        .iter()
        .find(|trial| trial.name() == "exceed_memory_quota")
        .map(Trial::observed)
        .expect("la sonde est au rapport");
    assert!(
        matches!(observed, Observed::NotRun { .. }),
        "« ce que je devais lire n'était pas là » n'est pas « j'ai été contenue » : {observed:?}"
    );
    assert!(matches!(
        assess(&execd, &id, SandboxLevel::S3),
        Standing::NotTrusted { .. }
    ));
}

// ---------------------------------------------------------------------------------------------
// W5.h — les sondes que le premier hôte réel a démenties
// ---------------------------------------------------------------------------------------------

/// **Deux ignorances, jamais une.** 120 et 121 ne disent pas la même chose et ne se réparent pas
/// pareil.
///
/// 120 : « ce que je devais **lire** n'était pas là » — un `cpu.stat` absent, un `curl` manquant.
/// On répare en complétant l'image. 121 : « ce que je devais **atteindre** n'a pas répondu » — un
/// réseau qui ne mène nulle part, un service que le déploiement filtre. On répare en changeant
/// d'hôte, ou en renonçant à la mesure.
///
/// Les fondre rendrait la seconde invisible, et c'est très exactement l'erreur que `W5.f` a trouvée
/// sur un hôte réel : trois sondes lues comme **bloquées**, donc comme une preuve d'isolation,
/// alors que le réseau de l'hôte ne menait nulle part.
#[test]
fn lire_ce_qui_manque_et_atteindre_ce_qui_ne_repond_pas_sont_deux_ignorances() {
    let unread = unrunnable(INCONCLUSIVE_EXIT_CODE).expect("120 est réservé");
    let unreached = unrunnable(UNREACHABLE_TARGET_EXIT_CODE).expect("121 est réservé");
    assert_ne!(
        unread, unreached,
        "les fondre ferait disparaître la seconde, et c'est celle qu'un hôte réel produit"
    );
    assert_ne!(INCONCLUSIVE_EXIT_CODE, UNREACHABLE_TARGET_EXIT_CODE);
}

/// Ni l'un ni l'autre n'est un blocage : les deux portent une raison, et c'est cette raison qui
/// empêche `judge` d'en faire une preuve d'isolation.
#[test]
fn aucune_des_deux_ignorances_ne_vaut_un_blocage() {
    for code in [INCONCLUSIVE_EXIT_CODE, UNREACHABLE_TARGET_EXIT_CODE] {
        assert!(
            unrunnable(code).is_some(),
            "{code} doit porter sa raison, sans quoi il se lit comme un blocage"
        );
    }
}

/// **La sonde inversée.** `read_process_environment` ne vise plus `/proc/1`.
///
/// Dans un namespace PID, `/proc/1` est l'init **du conteneur** : la sonde réussissait d'autant
/// plus sûrement que le confinement était correct, et comme elle est `critical`, tout hôte bien
/// configuré se voyait refuser la confiance. Le test tient la correction par l'**absence** de
/// l'ancienne cible, parce que c'est elle qui portait la faute.
#[test]
fn la_sonde_d_environnement_ne_vise_plus_l_init_du_conteneur() {
    let command = probe_command("read_process_environment")
        .expect("la sonde existe")
        .join(" ");
    assert!(
        !command.contains("/proc/1/environ"),
        "« /proc/1 » désigne l'init du conteneur dès qu'un namespace PID existe : {command}"
    );
    assert!(
        command.contains("/proc/self/cgroup"),
        "le discriminant est le cgroup **du processus lui-même** : sans le lire, la sonde n'a rien \
         à quoi comparer et le premier processus venu passe pour étranger — {command}"
    );
    assert!(
        command.contains("/cgroup"),
        "et elle doit lire celui de chaque processus candidat, sans quoi la comparaison n'a qu'un \
         côté : {command}"
    );
}

/// **Les deux sondes réseau constatent d'abord s'il y a une route.**
///
/// Sans ce constat, un `curl` qui échoue ne distingue pas « la sandbox a coupé le réseau » de
/// « l'hôte ne mène nulle part ». `S3` s'appelle `container-isolated-network` : ce qu'il contient
/// **est** le namespace, et un namespace réseau vide n'a pas de route par défaut.
#[test]
fn les_sondes_reseau_distinguent_le_namespace_du_monde() {
    for name in ["open_outbound_connection", "reach_cloud_metadata_service"] {
        let command = probe_command(name).expect("la sonde existe").join(" ");
        assert!(
            command.contains("/proc/net/route"),
            "« {name} » doit constater l'absence de route avant de conclure : {command}"
        );
        assert!(
            command.contains("exit 121"),
            "« {name} » doit pouvoir dire que la cible n'a pas répondu, plutôt que « bloquée » : \
             {command}"
        );
    }
}

/// Et elle n'attend aucun binaire de l'image pour cela — `W5.d` vaut aussi pour la correction.
#[test]
fn le_constat_de_route_ne_demande_rien_a_l_image() {
    for name in ["open_outbound_connection", "reach_cloud_metadata_service"] {
        let command = probe_command(name).expect("la sonde existe").join(" ");
        assert!(
            !command.contains("ip route"),
            "« ip » est un binaire que l'image peut ne pas porter ; « /proc/net/route » existe \
             toujours : {command}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// W5.m — le code de sortie voyage à côté du verdict
// ---------------------------------------------------------------------------------------------

/// **Deux codes différents, un même verdict, et on peut les distinguer.**
///
/// C'est tout l'objet de l'item. `Observed` a trois valeurs, et c'est le bon compte pour juger ;
/// mais plusieurs codes très différents tombent dans « bloquée », et sans le code brut rien ne dit
/// **où** la sonde s'est arrêtée. Quand `open_outbound_connection` est ressortie bloquée sur un hôte
/// dont un autre test montrait la route par défaut, la question « au constat de route, à `curl`, ou
/// avant ? » n'avait aucune réponse dans le rapport.
#[test]
fn deux_codes_qui_bloquent_restent_discernables() {
    let mut seen = Vec::new();
    for code in [1, 13] {
        let (execd, id) = started(FixedCode(code));
        let trials = run_suite(&execd, &id);
        let trial = trials
            .iter()
            .find(|trial| trial.name() == "escalate_to_root")
            .expect("la sonde est au rapport");
        assert_eq!(
            trial.observed(),
            Observed::Blocked,
            "les deux codes se jugent pareil — c'est justement pourquoi le verdict ne suffit pas"
        );
        seen.push(trial.code());
    }
    assert_eq!(
        seen,
        vec![Some(1), Some(13)],
        "le verdict les confond, le rapport ne doit pas"
    );
}

/// **Un runtime qui n'a pas répondu n'a pas de code**, et ce n'est pas zéro.
///
/// `None` plutôt qu'un `-1` ou un `0` : les deux sont des valeurs que quelqu'un finirait par lire
/// comme un vrai code de sortie, et `0` signifierait un succès. L'absence reste une absence.
#[test]
fn un_runtime_muet_ne_rend_aucun_code() {
    // Créée et démarrée par un runtime qui répond, puis le runtime disparaît : c'est le seul
    // chemin par lequel `run_suite` rencontre un échec de lancement sonde par sonde.
    let (execd, id) = started(VanishingRuntime::new(2));
    let trials = run_suite(&execd, &id);
    assert!(
        trials.iter().all(|trial| trial.code().is_none()),
        "aucune commande n'a tourné : leur prêter un code inventerait une observation"
    );
    assert!(
        trials
            .iter()
            .all(|trial| matches!(trial.observed(), Observed::NotRun { .. })),
        "et le verdict reste « pas lancée »"
    );
}

/// Le code d'un succès est rapporté aussi, et il vaut zéro.
///
/// Sans cela le rapport ne porterait le code que des échecs, et « pas de code » voudrait dire deux
/// choses : réussi, ou pas lancé.
#[test]
fn le_code_d_un_succes_est_rapporte_lui_aussi() {
    let (execd, id) = started(ProbingRunner::new(Vec::new()));
    let trials = run_suite(&execd, &id);
    assert!(
        trials
            .iter()
            .all(|trial| trial.code() == Some(0) && trial.observed() == Observed::Succeeded),
        "un rapport qui tairait le code des succès rendrait « pas de code » ambigu"
    );
}

/// Un runtime qui rend un code fixe **aux sondes**, et réussit le cycle de vie.
///
/// Le distinguo est nécessaire : un runtime qui échouerait aussi à `create` ne laisserait jamais
/// arriver jusqu'aux sondes, et le test ne mesurerait rien.
struct FixedCode(i32);

impl Runner for FixedCode {
    fn run(&self, arguments: &[String]) -> Result<Execution, RuntimeError> {
        let probing = arguments.first().is_some_and(|verb| verb == "exec");
        Ok(Execution {
            code: if probing { self.0 } else { 0 },
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}

// ---------------------------------------------------------------------------------------------
// W5.o — une sonde ne contamine pas la suivante
// ---------------------------------------------------------------------------------------------

/// **Un lancement qui échoue de façon transitoire est retenté, et la mesure a lieu.**
///
/// `W5.n` a montré le coût de l'inverse : `exceed_pid_quota` saturait le quota de PID, `podman exec`
/// ne pouvait plus forker pour les quatre sondes suivantes, et le harnais rapportait quatre verdicts
/// qui n'étaient l'observation de rien. Le catalogage de 255 les a fait passer de fausse preuve à
/// aveu d'ignorance ; il ne les a pas rendues **mesurables**.
#[test]
fn une_sonde_que_le_runtime_n_a_pas_pu_lancer_est_retentee() {
    let (execd, id) = started(FlakyLaunch::new(255, 3));
    let trials = run_suite(&execd, &id);
    let first = trials.first().expect("la suite n'est pas vide");
    assert_eq!(
        first.observed(),
        Observed::Blocked,
        "après trois refus de lancement, la commande a fini par tourner et rendre son verdict"
    );
    assert_eq!(
        first.code(),
        Some(1),
        "le code rapporté est celui de la tentative qui a abouti, pas celui des refus"
    );
}

/// **Un refus qui persiste reste un aveu**, et il est borné.
///
/// Réessayer indéfiniment transformerait une sonde qui ne peut pas tourner en une campagne qui ne
/// finit pas. Le budget est fixe et se lit dans `LAUNCH_ATTEMPTS`.
#[test]
fn un_refus_qui_persiste_reste_un_aveu_et_le_nombre_de_tentatives_est_borne() {
    let runner = FlakyLaunch::new(255, u32::MAX);
    let (execd, id) = started(runner);
    let trials = run_suite(&execd, &id);
    let first = trials.first().expect("la suite n'est pas vide");
    assert!(
        matches!(first.observed(), Observed::NotRun { .. }),
        "ce qui n'a jamais été lancé n'a rien prouvé, quel que soit le nombre d'essais"
    );
    assert_eq!(first.code(), Some(255));

    let launches = execd
        .runner()
        .calls
        .lock()
        .expect("verrou")
        .iter()
        .filter(|call| call.first().is_some_and(|verb| verb == "exec"))
        .count();
    assert_eq!(
        launches,
        LAUNCH_ATTEMPTS as usize * SUITE.len(),
        "chaque sonde a droit au même budget, et à pas un essai de plus"
    );
}

/// **Une sonde absente de l'image n'est pas retentée.**
///
/// 126 et 127 sont des propriétés de l'image : elle ne gagnera pas un binaire entre deux essais, et
/// réessayer ne ferait que retarder l'aveu. C'est la distinction que `TRANSIENT_EXIT_CODES` porte,
/// et la confondre rendrait chaque campagne sur une image incomplète six fois plus lente sans rien
/// apprendre de plus.
#[test]
fn une_sonde_absente_de_l_image_n_est_pas_retentee() {
    let (execd, id) = started(FlakyLaunch::new(127, u32::MAX));
    let trials = run_suite(&execd, &id);
    assert!(
        trials
            .iter()
            .all(|trial| matches!(trial.observed(), Observed::NotRun { .. })),
        "127 reste « absente de l'image »"
    );

    let launches = execd
        .runner()
        .calls
        .lock()
        .expect("verrou")
        .iter()
        .filter(|call| call.first().is_some_and(|verb| verb == "exec"))
        .count();
    assert_eq!(
        launches,
        SUITE.len(),
        "une tentative par sonde : l'image ne changera pas d'ici la suivante"
    );
}

/// Les deux familles sont disjointes, et aucune n'est vide.
///
/// Un `TRANSIENT_EXIT_CODES` vide désactiverait silencieusement toute reprise ; un qui contiendrait
/// 127 ferait boucler six fois sur une image incomplète. Le test nomme les deux fautes.
#[test]
fn les_codes_transitoires_sont_un_sous_ensemble_strict_des_codes_reserves() {
    let reserved: Vec<i32> = UNRUNNABLE_EXIT_CODES
        .into_iter()
        .map(|(code, _)| code)
        .collect();
    assert!(
        !TRANSIENT_EXIT_CODES.is_empty(),
        "aucune reprise ne serait tentée"
    );
    for code in TRANSIENT_EXIT_CODES {
        assert!(
            reserved.contains(&code),
            "{code} serait retenté sans être lu comme « pas lancée » : la reprise masquerait un verdict"
        );
    }
    for definitive in [126, 127] {
        assert!(
            !TRANSIENT_EXIT_CODES.contains(&definitive),
            "{definitive} est une propriété de l'image : la retenter retarde l'aveu sans rien changer"
        );
    }
}

/// Un runtime qui refuse de lancer les `n` premières fois, puis bloque franchement.
struct FlakyLaunch {
    calls: Mutex<Vec<Vec<String>>>,
    refusals: Mutex<u32>,
    code: i32,
    budget: u32,
}

impl FlakyLaunch {
    fn new(code: i32, budget: u32) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            refusals: Mutex::new(0),
            code,
            budget,
        }
    }
}

impl Runner for FlakyLaunch {
    fn run(&self, arguments: &[String]) -> Result<Execution, RuntimeError> {
        self.calls.lock().expect("verrou").push(arguments.to_vec());
        if arguments.first().is_none_or(|verb| verb != "exec") {
            return Ok(Execution {
                code: 0,
                stdout: String::new(),
                stderr: String::new(),
            });
        }
        let mut refusals = self.refusals.lock().expect("verrou");
        let code = if *refusals < self.budget {
            *refusals += 1;
            self.code
        } else {
            1
        };
        Ok(Execution {
            code,
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}
