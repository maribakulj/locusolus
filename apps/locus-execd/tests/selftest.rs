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
    Execution, INCONCLUSIVE_EXIT_CODE, PROBE_COMMANDS, PodmanBackend, RestrictedProfile, Runner,
    SeccompProfiles, UNREACHABLE_RUNTIME, UNREACHABLE_TARGET_EXIT_CODE, UNRUNNABLE_EXIT_CODES,
    Workload, assess, certify, exec_arguments, probe_command, run_suite, unrunnable,
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
    let absent: Vec<&Observed> = results
        .iter()
        .filter(|(name, _)| *name == "exceed_cpu_quota" || *name == "open_outbound_connection")
        .map(|(_, observed)| observed)
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
            .find(|(name, _)| *name == "escalate_to_root")
            .map(|(_, observed)| observed)
            .expect("la sonde est au rapport");
        assert_eq!(
            observed,
            &Observed::NotRun { reason: expected },
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
/// pas ce qu'elle couvre. Celui-ci nomme les trois codes et dit pourquoi chacun est réservé.
#[test]
fn la_table_couvre_les_trois_codes_que_posix_et_podman_reservent() {
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
            127,
        ],
        "en retirer un ferait relire ce code comme un blocage, donc comme une preuve d'isolation"
    );
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
        .find(|(name, _)| *name == "exceed_memory_quota")
        .map(|(_, observed)| observed)
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
