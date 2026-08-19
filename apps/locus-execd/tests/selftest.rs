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
    BOOT_ID_PATH_VARIABLE, Execution, HOST_BOOT_ID_VARIABLE, INCONCLUSIVE_EXIT_CODE,
    LAUNCH_ATTEMPTS, PROBE_COMMANDS, PodmanBackend, ProbeContext, QUOTA_BYTES_VARIABLE,
    QUOTA_TARGET_VARIABLE, RUNNING_TEMPLATE, RestrictedProfile, Runner, SANDBOX_GONE,
    SANDBOX_REFUSED, SeccompProfiles, TRANSIENT_EXIT_CODES, Trial, UNREACHABLE_RUNTIME,
    UNREACHABLE_TARGET_EXIT_CODE, UNRUNNABLE_EXIT_CODES, UNWRITABLE_TARGET_EXIT_CODE, Workload,
    boot_id_from, certify, exec_arguments, probe_command, run_suite, unrunnable,
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
        if let Some(answer) = alive(arguments) {
            return Ok(answer);
        }
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
        if let Some(answer) = alive(arguments) {
            return Ok(answer);
        }
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

/// La réponse d'un double à la question « la sandbox tourne-t-elle ? ».
///
/// `W5.p` fait demander l'état de la sandbox après chaque sonde qui n'a rien rendu. Un double qui ne
/// répondrait pas à cette question précise la verrait lue comme « le runtime a répondu, et elle ne
/// tourne plus » — et toutes les sondes suivantes seraient rapportées comme n'ayant rien eu pour les
/// lancer. Les doubles de ce fichier modélisent une sandbox **vivante** ; ils le disent.
fn alive(arguments: &[String]) -> Option<Execution> {
    arguments
        .iter()
        .any(|argument| argument == RUNNING_TEMPLATE)
        .then(|| Execution {
            code: 0,
            stdout: "true\n".to_owned(),
            stderr: String::new(),
        })
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

/// Un backend prêt, et la mission à laquelle la suite sera soumise.
///
/// `W5.r` : `run_suite` ouvre une sandbox par sonde et la retire derrière elle. Il n'y a donc plus
/// de sandbox à créer d'avance, et plus d'identifiant à porter d'un appel à l'autre — ce qui voyage
/// à sa place est la **spécification**. Le nom de la fonction reste juste : ce qui est prêt à
/// éprouver, c'est le backend.
fn started<R: Runner>(runner: R) -> (PodmanBackend<R>, SandboxSpec) {
    (backend(runner), mission(SandboxLevel::S3))
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
    let (mut execd, spec) = started(ProbingRunner::airtight());
    let results = run_suite(&mut execd, &spec);
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
    let (mut execd, spec) = started(ProbingRunner::new(Vec::new()));
    let results = run_suite(&mut execd, &spec);
    assert!(
        results
            .iter()
            .all(|trial| trial.observed() == Observed::Succeeded),
        "un runtime qui laisse tout passer doit produire seize succès, pas seize blocages"
    );
}

#[test]
fn la_sonde_est_lancee_dans_la_sandbox_qui_tourne() {
    let (mut execd, spec) = started(ProbingRunner::airtight());
    let _ = run_suite(&mut execd, &spec);

    let calls = execd.runner().calls.lock().expect("verrou");
    let first = calls
        .iter()
        .find(|call| call[0] == "exec")
        .expect("une sonde a été lancée");
    assert_eq!(first[1], "locus-0001", "dans la sandbox, pas à côté d'elle");
    assert!(
        first.ends_with(
            probe_command("write_outside_workspace")
                .expect("commande")
                .iter()
                .map(|part| (*part).to_owned())
                .collect::<Vec<_>>()
                .as_slice()
        ),
        "et c'est bien la commande de la première sonde de SUITE : {first:?}"
    );
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
    // La première sandbox est créée et démarrée par un runtime qui répond, puis le runtime
    // disparaît : c'est le cas d'un Podman tué en cours de campagne. La première sonde est donc
    // lancée dans le vide — `UNREACHABLE_RUNTIME` — et les quinze suivantes n'ont même plus de quoi
    // ouvrir leur sandbox — `SANDBOX_REFUSED`. Deux ignorances, et pas une : la première dit « je
    // l'ai lancée et personne n'a répondu », la seconde « je n'ai pas pu la lancer ».
    let (mut execd, spec) = started(VanishingRuntime::new(2));
    let results = run_suite(&mut execd, &spec);

    assert!(
        results.iter().all(|trial| matches!(
            trial.observed(),
            Observed::NotRun {
                reason
            } if reason == UNREACHABLE_RUNTIME
        )),
        "un runtime disparu n'a rien refusé : il n'a pas répondu, et `W5.s` sait maintenant le \
         dire — {results:?}"
    );
    assert_eq!(
        results[0].observed(),
        Observed::NotRun {
            reason: UNREACHABLE_RUNTIME
        },
        "la seule dont la sandbox existait encore a bien été lancée, et n'a rien obtenu"
    );

    let verdict = certify(&mut execd, &spec, SandboxLevel::S3);
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
    let (mut execd, spec) = started(ProbingRunner::airtight());
    assert_eq!(
        certify(&mut execd, &spec, SandboxLevel::S3),
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
    let (mut execd, spec) = started(ProbingRunner::new(leaky));

    match certify(&mut execd, &spec, SandboxLevel::S3) {
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
    let (mut execd, spec) = started(ProbingRunner::airtight());
    let verdict = certify(&mut execd, &spec, SandboxLevel::S0);
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
    let verdict = certify(&mut execd, &mission(SandboxLevel::S2), SandboxLevel::S2);
    assert_eq!(
        verdict,
        Standing::Trusted {
            level: SandboxLevel::S2
        }
    );

    let calls = execd.runner().calls.lock().expect("verrou");
    let verbs: Vec<&str> = calls
        .iter()
        .map(|call| call[0].as_str())
        .filter(|verb| *verb != "inspect")
        .collect();
    assert_eq!(
        verbs,
        ["create", "start", "exec", "stop", "rm"].repeat(SUITE.len()),
        "le cycle complet, une fois par sonde : arrêter n'est pas retirer, et sans le retrait le \
         nom reste pris — trois passages de CI l'ont payé"
    );
}

/// **Aucune sonde ne partage sa sandbox avec une autre** — le test de sortie de `W5.r`.
///
/// C'est la propriété que quatre sprints ont cherchée en la contournant. `W5.n` a découvert que
/// `exceed_pid_quota` rendait les sondes suivantes inlançables, `W5.o` a fait retenter, `W5.p` a
/// écarté la sandbox morte, `W5.q` a lu le refus du runtime : un cgroup PID saturé que plus
/// personne ne peut vider, parce que le shell de la sonde meurt avant son propre nettoyage.
///
/// Aucun nettoyage ne peut être promis par ce qui est en train d'être épuisé. Ce qui peut l'être,
/// c'est qu'il n'y ait rien à nettoyer : seize sandboxes, seize noms, aucun état partagé. La
/// contamination cesse d'être évitée — elle devient **inexprimable**.
#[test]
fn aucune_sonde_ne_partage_sa_sandbox_avec_une_autre() {
    let mut execd = backend(ProbingRunner::airtight());
    let _ = certify(&mut execd, &mission(SandboxLevel::S2), SandboxLevel::S2);

    let calls = execd.runner().calls.lock().expect("verrou");
    let sandboxes: Vec<&str> = calls
        .iter()
        .filter(|call| call[0] == "exec")
        .map(|call| call[1].as_str())
        .collect();
    assert_eq!(
        sandboxes.len(),
        SUITE.len(),
        "seize sondes, seize lancements"
    );
    let distinct: std::collections::BTreeSet<&str> = sandboxes.iter().copied().collect();
    assert_eq!(
        distinct.len(),
        SUITE.len(),
        "deux sondes dans la même sandbox, et l'une peut de nouveau faire mentir l'autre : \
         {sandboxes:?}"
    );
}

/// **Un démarrage qui échoue retire quand même**, et c'est le cas le plus silencieux.
///
/// Rien ne tourne, donc rien ne signale la fuite — mais le nom reste pris. Avec une sandbox par
/// sonde, la faute serait seize fois plus fréquente qu'avant : elle est donc épinglée seize fois.
#[test]
fn un_demarrage_qui_echoue_ne_laisse_pas_le_nom_pris() {
    let mut execd = backend(FailingStart::default());
    let verdict = certify(&mut execd, &mission(SandboxLevel::S2), SandboxLevel::S2);
    assert!(
        matches!(verdict, Standing::NotTrusted { .. }),
        "rien n'a été éprouvé : le seul verdict permis est le refus de confiance"
    );

    let calls = execd.runner().calls.lock().expect("verrou");
    let created = calls.iter().filter(|call| call[0] == "create").count();
    let removed = calls.iter().filter(|call| call[0] == "rm").count();
    assert_eq!(created, SUITE.len());
    assert_eq!(
        removed, created,
        "chaque conteneur créé doit être retiré, même si rien n'a jamais tourné dedans : {calls:?}"
    );
}

/// Un niveau que le backend ne sait pas tenir se **rapporte**, il ne s'échappe pas.
///
/// L'ancienne signature rendait `Err` et le rapport était vide. Seize absences nommées, chacune
/// portant le mot du runtime, disent la même chose et la disent **dans la table** — et le verdict
/// rendu là-dessus reste juste : rien n'a été vérifié, donc pas de confiance.
#[test]
fn certify_refuse_un_niveau_que_le_backend_ne_sait_pas_tenir() {
    let mut execd = backend(ProbingRunner::airtight());
    let spec = mission(SandboxLevel::S4);
    let trials = run_suite(&mut execd, &spec);

    assert!(
        trials.iter().all(|trial| trial.observed()
            == Observed::NotRun {
                reason: SANDBOX_REFUSED
            }),
        "aucune sonde n'a pu être ouverte : {trials:?}"
    );
    assert!(
        trials
            .iter()
            .all(|trial| trial.detail().is_some_and(|why| why.contains("S4"))),
        "et le refus dit lequel : sans le mot du runtime, la table n'apprendrait rien"
    );
    assert!(matches!(
        certify(&mut execd, &spec, SandboxLevel::S4),
        Standing::NotTrusted { .. }
    ));
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
        if let Some(answer) = alive(arguments) {
            return Ok(answer);
        }
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
    let spec = mission(SandboxLevel::S3);

    let results = run_suite(&mut execd, &spec);
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

    match certify(&mut execd, &spec, SandboxLevel::S3) {
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
        let spec = mission(SandboxLevel::S3);
        let results = run_suite(&mut execd, &spec);
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
    let spec = mission(SandboxLevel::S3);
    assert_eq!(
        certify(&mut execd, &spec, SandboxLevel::S3),
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
    let spec = mission(SandboxLevel::S3);
    let results = run_suite(&mut execd, &spec);
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
        certify(&mut execd, &spec, SandboxLevel::S3),
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
        let (mut execd, spec) = started(FixedCode(code));
        let trials = run_suite(&mut execd, &spec);
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
    let (mut execd, spec) = started(VanishingRuntime::new(2));
    let trials = run_suite(&mut execd, &spec);
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
    let (mut execd, spec) = started(ProbingRunner::new(Vec::new()));
    let trials = run_suite(&mut execd, &spec);
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
    let (mut execd, spec) = started(FlakyLaunch::new(255, 3));
    let trials = run_suite(&mut execd, &spec);
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
    let (mut execd, spec) = started(runner);
    let trials = run_suite(&mut execd, &spec);
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
    let (mut execd, spec) = started(FlakyLaunch::new(127, u32::MAX));
    let trials = run_suite(&mut execd, &spec);
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
        if let Some(answer) = alive(arguments) {
            return Ok(answer);
        }
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

// ---------------------------------------------------------------------------------------------
// W5.p — une sandbox morte est dite morte
// ---------------------------------------------------------------------------------------------

/// **« Il n'y avait rien pour la lancer » n'est pas « le runtime n'a pas pu ».**
///
/// Les deux se répareraient ailleurs : le premier en comprenant ce qui a tué la sandbox, le second
/// en cherchant un runtime fatigué. `W5.o` faisait retenter les lancements refusés en supposant la
/// cause transitoire — un cgroup occupé se libère. Le premier passage réel a démenti la supposition,
/// et ce qui ne se libère pas n'était pas occupé.
///
/// Le rapport reste **complet** : chaque sonde y figure, avec la raison exacte pour laquelle elle
/// n'a rien mesuré. Une suite tronquée se lirait comme une suite passée.
#[test]
fn les_sondes_qui_suivent_une_sandbox_morte_le_disent() {
    let (mut execd, spec) = started(DyingSandbox::after(SURVIVED));
    let trials = run_suite(&mut execd, &spec);

    assert_eq!(trials.len(), SUITE.len(), "le rapport reste complet");
    let gone: Vec<&Trial> = trials
        .iter()
        .filter(|trial| {
            trial.observed()
                == (Observed::NotRun {
                    reason: SANDBOX_GONE,
                })
        })
        .collect();
    assert!(
        !gone.is_empty(),
        "la sandbox meurt en cours de campagne : les sondes suivantes doivent le dire"
    );
    assert!(
        gone.iter().all(|trial| trial.code().is_none()),
        "aucune commande n'a été lancée : leur prêter un code inventerait une observation"
    );
    assert_eq!(
        trials.last().expect("la suite n'est pas vide").observed(),
        Observed::NotRun {
            reason: SANDBOX_GONE
        },
        "un runtime durablement fâché le reste : la dernière sonde le constate comme les autres"
    );

    // La sonde qui **constate** la mort le dit elle aussi. Lui laisser « le runtime n'a pas pu »
    // enverrait chercher un runtime fatigué à l'endroit précis où le conteneur a disparu.
    assert_eq!(
        trials[SURVIVED].observed(),
        Observed::NotRun {
            reason: SANDBOX_GONE
        },
        "la première sonde qui échoue est celle qui découvre la mort : c'est elle qui la nomme"
    );

    // Et chacune est quand même **lancée** : depuis `W5.r`, la mort d'une sandbox n'atteint que la
    // sonde qui était dedans. Les suivantes ouvrent la leur, et la découvrent morte à leur tour
    // parce que ce double-ci modélise un runtime durablement fâché — pas parce qu'elles auraient
    // hérité de quoi que ce soit.
    let calls = execd.runner().calls.lock().expect("verrou");
    let launches = calls
        .iter()
        .filter(|call| call.first().is_some_and(|verb| verb == "exec"))
        .count();
    assert_eq!(
        launches,
        SUITE.len(),
        "chaque sonde a eu sa chance — une seule fois : une sandbox morte ne redevient pas vivante, \
         et lui redemander six tentatives ferait payer une minute pour réapprendre ce qu'on sait"
    );
    assert_eq!(
        calls
            .iter()
            .filter(|call| call.first().is_some_and(|verb| verb == "create"))
            .count(),
        SUITE.len(),
        "et chacune dans la sienne"
    );
}

/// Combien de sondes la sandbox de ce test laisse aboutir avant de mourir.
const SURVIVED: usize = 3;

/// **Un runtime muet ne fait pas déclarer la sandbox morte.**
///
/// « Je n'ai pas pu demander » n'est pas « elle ne tourne plus ». Confondre les deux ferait écrire
/// qu'il n'y avait rien pour lancer la sonde alors qu'on n'en sait rien — et c'est très exactement
/// la faute que `W5.n` et `W5.o` ont passé deux sprints à retirer d'ici.
#[test]
fn un_runtime_muet_ne_fait_pas_declarer_la_sandbox_morte() {
    let (mut execd, spec) = started(VanishingRuntime::new(2));
    let trials = run_suite(&mut execd, &spec);
    assert!(
        trials.iter().all(|trial| trial.observed()
            != (Observed::NotRun {
                reason: SANDBOX_GONE
            })),
        "sans réponse du runtime, l'état de la sandbox est inconnu — et l'inconnu ne s'écrit pas \
         comme un constat"
    );
}

/// Une sandbox qui cesse de tourner après un nombre donné de sondes.
struct DyingSandbox {
    calls: Mutex<Vec<Vec<String>>>,
    survives: usize,
    launched: Mutex<usize>,
}

impl DyingSandbox {
    fn after(survives: usize) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            survives,
            launched: Mutex::new(0),
        }
    }

    fn dead(&self) -> bool {
        *self.launched.lock().expect("verrou") > self.survives
    }
}

impl Runner for DyingSandbox {
    fn run(&self, arguments: &[String]) -> Result<Execution, RuntimeError> {
        self.calls.lock().expect("verrou").push(arguments.to_vec());
        if arguments
            .iter()
            .any(|argument| argument == RUNNING_TEMPLATE)
        {
            return Ok(Execution {
                code: 0,
                stdout: if self.dead() { "false\n" } else { "true\n" }.to_owned(),
                stderr: String::new(),
            });
        }
        if arguments.first().is_none_or(|verb| verb != "exec") {
            return Ok(Execution {
                code: 0,
                stdout: String::new(),
                stderr: String::new(),
            });
        }
        let mut launched = self.launched.lock().expect("verrou");
        *launched += 1;
        let alive = *launched <= self.survives;
        Ok(Execution {
            // Une fois morte, le runtime refuse de lancer quoi que ce soit : c'est le 255 observé
            // sur l'hôte réel.
            code: if alive { 1 } else { 255 },
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}

/// **Une réponse qu'on ne sait pas lire n'est pas une mort.**
///
/// Le runtime a répondu, mais pas « true » ni « false ». Ranger cela avec « ne tourne plus » ferait
/// déclarer mortes des sandboxes sur une réponse qu'on n'a pas comprise — troisième forme de la même
/// faute, après « le runtime n'a pas pu » et « je n'ai pas pu demander ».
#[test]
fn une_reponse_illisible_ne_fait_pas_declarer_la_sandbox_morte() {
    let (mut execd, spec) = started(Unreadable);
    let trials = run_suite(&mut execd, &spec);
    assert!(
        trials.iter().all(|trial| trial.observed()
            != (Observed::NotRun {
                reason: SANDBOX_GONE
            })),
        "« je n'ai pas compris » ne s'écrit pas « il n'y avait rien pour la lancer »"
    );
}

/// Un runtime dont l'inspection répond autre chose que « true » ou « false ».
struct Unreadable;

impl Runner for Unreadable {
    fn run(&self, arguments: &[String]) -> Result<Execution, RuntimeError> {
        if arguments
            .iter()
            .any(|argument| argument == RUNNING_TEMPLATE)
        {
            return Ok(Execution {
                code: 0,
                stdout: "<no value>\n".to_owned(),
                stderr: String::new(),
            });
        }
        // Les lancements sont refusés par un code **transitoire** : sans cela la question de l'état
        // de la sandbox ne se poserait jamais, et le test ne mesurerait rien de ce qu'il prétend.
        Ok(Execution {
            code: i32::from(arguments.first().is_some_and(|verb| verb == "exec")) * 255,
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}

/// **La sandbox est interrogée à chaque tentative, y compris la dernière.**
///
/// Placer le constat de vie derrière la condition de reprise aurait un effet unique et discret : une
/// sandbox qui meurt pendant la dernière tentative serait rapportée « le runtime n'a pas pu » au lieu
/// de « il n'y avait rien pour la lancer ». Un mutant a montré que rien ne le tenait — cinq refus
/// suivis d'une mort est exactement le cas que le budget de reprise rend possible.
#[test]
fn la_mort_survenue_a_la_derniere_tentative_est_nommee() {
    let (mut execd, spec) = started(DiesLast::default());
    let trials = run_suite(&mut execd, &spec);
    assert_eq!(
        trials.first().expect("la suite n'est pas vide").observed(),
        Observed::NotRun {
            reason: SANDBOX_GONE
        },
        "la mort découverte au dernier essai se nomme comme celle découverte au premier"
    );
}

/// Un runtime qui refuse les lancements et dont la sandbox ne meurt qu'au dernier essai.
#[derive(Default)]
struct DiesLast {
    inspections: Mutex<u32>,
}

impl Runner for DiesLast {
    fn run(&self, arguments: &[String]) -> Result<Execution, RuntimeError> {
        if arguments
            .iter()
            .any(|argument| argument == RUNNING_TEMPLATE)
        {
            let mut inspections = self.inspections.lock().expect("verrou");
            *inspections += 1;
            return Ok(Execution {
                code: 0,
                stdout: if *inspections < LAUNCH_ATTEMPTS {
                    "true\n"
                } else {
                    "false\n"
                }
                .to_owned(),
                stderr: String::new(),
            });
        }
        Ok(Execution {
            code: i32::from(arguments.first().is_some_and(|verb| verb == "exec")) * 255,
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}

// ---------------------------------------------------------------------------------------------
// W5.q — ce que le runtime dit en refusant
// ---------------------------------------------------------------------------------------------

/// **Un refus qui s'explique et un refus muet ne sont pas le même refus.**
///
/// C'était la dernière chose que le harnais jetait. `W5.m` a mis le code à côté du verdict, et le
/// code a nommé le motif ; `W5.n`, `W5.o` et `W5.p` ont écarté trois hypothèses l'une après l'autre,
/// et le refus persiste sur un conteneur vivant. Ce qu'on n'avait jamais lu est ce que le runtime
/// **écrit**.
#[test]
fn ce_que_le_runtime_ecrit_en_refusant_est_conserve() {
    let (mut execd, spec) = started(Explaining(
        "crun: cannot fork: Resource temporarily unavailable\n",
    ));
    let trials = run_suite(&mut execd, &spec);
    let first = trials.first().expect("la suite n'est pas vide");
    assert_eq!(
        first.detail(),
        Some("crun: cannot fork: Resource temporarily unavailable"),
        "le refus porte ce qui l'explique, mot pour mot — sans le saut de ligne que tout runtime \
         ajoute, qui ferait passer deux fois le même message pour deux messages"
    );
    assert_eq!(first.code(), Some(255), "et le code, qui ne l'explique pas");
}

/// **Un saut de ligne seul n'est pas une explication.**
///
/// Un runtime qui ferme son flux d'erreur sur un `\n` n'a rien dit. Sans nettoyage, ce `\n`
/// deviendrait un détail : chaque refus muet de la suite en porterait un, et la distinction que
/// [`ce_que_le_runtime_ecrit_en_refusant_est_conserve`] est venue chercher — qui parle, qui se tait
/// — serait rendue à l'œil du lecteur au lieu d'être portée par le rapport.
#[test]
fn un_saut_de_ligne_seul_n_est_pas_une_explication() {
    let (mut execd, spec) = started(Explaining("  \n"));
    let trials = run_suite(&mut execd, &spec);
    assert!(
        trials.iter().all(|trial| trial.detail().is_none()),
        "de l'espace n'est pas une parole"
    );
}

/// **Rien d'écrit se dit `None`, pas une chaîne vide.**
///
/// Les deux se ressembleraient dans un rapport où tout le monde porte une chaîne, et la distinction
/// qu'on est venu chercher disparaîtrait : un refus muet et un refus qui s'explique n'appellent pas
/// la même suite.
#[test]
fn un_refus_muet_se_distingue_d_un_refus_qui_parle() {
    let (mut execd, spec) = started(Explaining(""));
    let trials = run_suite(&mut execd, &spec);
    assert!(
        trials.iter().all(|trial| trial.detail().is_none()),
        "une chaîne vide n'est pas ce que le runtime a dit : c'est qu'il n'a rien dit"
    );
}

/// **Un détail n'est jamais une copie de la raison.**
///
/// La `reason` d'un `NotRun` est ce que *nous* avons constaté ; le détail est ce que quelqu'un
/// d'autre a **écrit**. Recopier la première dans le second donnerait à notre propre constat
/// l'autorité d'un témoignage, et le rapport ne distinguerait plus « on nous a expliqué » de « nous
/// avons supposé ». C'est la même règle qu'ailleurs dans ce fichier : pas vérifié n'est jamais
/// réussi, et notre constat n'est jamais la parole d'un autre.
///
/// `W5.r` a rendu ce test plus fort qu'il n'était : il affirmait « aucune absence ne porte de
/// détail », ce qui était vrai par accident — aucune absence n'avait alors de message à porter. Une
/// sandbox qui ne s'ouvre pas en a un, et le test aurait dû être réécrit plutôt qu'assoupli.
#[test]
fn un_detail_n_est_jamais_une_copie_de_la_raison() {
    let mut absences: Vec<Trial> = Vec::new();
    // Un runtime qui s'évapore, un runtime absent d'emblée, un niveau que le backend ne tient pas :
    // les trois chemins qui produisent des absences, et aucun ne doit se citer lui-même.
    let (mut execd, spec) = started(VanishingRuntime::new(2));
    absences.extend(run_suite(&mut execd, &spec));
    let (mut execd, spec) = started(AbsentRuntime);
    absences.extend(run_suite(&mut execd, &spec));
    let mut execd = backend(ProbingRunner::airtight());
    absences.extend(run_suite(&mut execd, &mission(SandboxLevel::S4)));

    let named: Vec<&Trial> = absences
        .iter()
        .filter(|trial| matches!(trial.observed(), Observed::NotRun { .. }))
        .collect();
    assert_eq!(
        named.len(),
        absences.len(),
        "les trois doubles ne produisent que des absences : {absences:?}"
    );
    assert!(
        named.iter().all(|trial| match trial.observed() {
            Observed::NotRun { reason } => trial.detail() != Some(reason),
            _ => true,
        }),
        "une absence qui se cite elle-même se lirait comme une absence expliquée : {named:?}"
    );
}

/// Une sonde qui aboutit ne porte pas de détail — il n'y a rien à expliquer.
#[test]
fn une_sonde_qui_aboutit_ne_porte_aucun_detail() {
    let (mut execd, spec) = started(ProbingRunner::new(Vec::new()));
    let trials = run_suite(&mut execd, &spec);
    assert!(
        trials
            .iter()
            .all(|trial| trial.detail().is_none() && trial.observed() == Observed::Succeeded),
        "un rapport qui commenterait les succès noierait les refus qui, eux, s'expliquent"
    );
}

/// Un runtime qui refuse les lancements en disant pourquoi.
struct Explaining(&'static str);

impl Runner for Explaining {
    fn run(&self, arguments: &[String]) -> Result<Execution, RuntimeError> {
        if let Some(answer) = alive(arguments) {
            return Ok(answer);
        }
        let probing = arguments.first().is_some_and(|verb| verb == "exec");
        Ok(Execution {
            code: if probing { 255 } else { 0 },
            stdout: String::new(),
            stderr: if probing {
                self.0.to_owned()
            } else {
                String::new()
            },
        })
    }
}

// ---------------------------------------------------------------------------------------------
// W5.j — la sonde disque écrit là où le quota mord
// ---------------------------------------------------------------------------------------------

/// **La cible du quota voyage jusqu'à la sonde**, et seulement quand il y en a une.
#[test]
fn la_cible_du_quota_est_passee_a_la_sonde_quand_il_y_en_a_une() {
    let id = SandboxId::new("locus-0001").expect("identifiant");
    let command = probe_command("exceed_disk_quota").expect("commande");

    let with = exec_arguments(
        &id,
        command,
        &ProbeContext {
            quota: Some(("/work".to_owned(), 512 << 20)),
            host_boot_id: None,
        },
    );
    assert!(
        with.windows(2)
            .any(|pair| pair[0] == "--env" && pair[1] == format!("{QUOTA_TARGET_VARIABLE}=/work")),
        "sans le chemin, la sonde ne sait pas où le quota mord : {with:?}"
    );
    assert!(
        with.windows(2).any(|pair| pair[0] == "--env"
            && pair[1] == format!("{QUOTA_BYTES_VARIABLE}={}", 512u64 << 20)),
        "sans la taille, la sonde écrirait un nombre choisi d'avance : {with:?}"
    );

    let without = exec_arguments(&id, command, &ProbeContext::default());
    assert!(
        !without.iter().any(|argument| argument == "--env"),
        "rien à borner, rien à annoncer : {without:?}"
    );
}

/// **Une cible qu'on ne peut pas écrire n'est pas un blocage.**
///
/// C'est le piège que `W5.j` répare, déplacé d'un cran : la sonde écrivait à la racine, en lecture
/// seule dès `S2`, et son échec ressortait comme une preuve que le quota mordait. Déplacer la sonde
/// vers l'espace de travail sans ce code réservé reproduirait exactement la même fausse preuve dès
/// que l'espace de travail ne serait pas inscriptible.
#[test]
fn une_cible_non_inscriptible_est_un_aveu_pas_un_blocage() {
    assert_eq!(
        unrunnable(UNWRITABLE_TARGET_EXIT_CODE),
        Some("la sonde n'a pas pu conclure : ce sur quoi elle devait écrire ne s'écrit pas"),
        "sans motif, 122 se lirait comme un blocage ordinaire"
    );
    for other in [INCONCLUSIVE_EXIT_CODE, UNREACHABLE_TARGET_EXIT_CODE] {
        assert_ne!(
            unrunnable(other),
            unrunnable(UNWRITABLE_TARGET_EXIT_CODE),
            "lire, atteindre et écrire sont trois ignorances, et elles ne se réparent pas pareil"
        );
    }
}

/// La sonde disque **sort en 0 sans cible, sans rien tenter**.
///
/// # Pourquoi `PATH` est vidé, et ce que cela prouve
///
/// Vérifier le seul code de sortie ne suffit pas : une rédaction qui se rabattrait sur `/` faute de
/// cible écrirait quand même, et rendrait `0` si l'écriture aboutissait. Un mutant l'a montré, en
/// écrivant quatre gigaoctets à la racine pour passer le test.
///
/// Avec un `PATH` vide, le shell ne trouve plus `dd`. La sonde correcte ne s'en aperçoit pas — elle
/// sort avant d'en avoir besoin. Une sonde qui tenterait d'écrire échouerait sur `dd` introuvable.
/// Le test épingle donc « elle n'essaie pas », et non « elle a réussi à essayer ».
#[test]
fn la_sonde_disque_reussit_sans_rien_tenter_quand_aucune_borne_n_est_declaree() {
    let command = probe_command("exceed_disk_quota").expect("commande");
    let script = command.last().expect("le script est le dernier argument");
    let run = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(script)
        .env_remove(QUOTA_TARGET_VARIABLE)
        .env("PATH", "")
        .output()
        .expect("sh répond");
    assert_eq!(
        run.status.code(),
        Some(0),
        "sans cible, la sonde n'a rien à mesurer et le dit en réussissant — sans écrire nulle part"
    );

    // Et la chaîne vide vaut l'absence : une variable posée à « rien » n'est pas une cible.
    let empty = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(script)
        .env(QUOTA_TARGET_VARIABLE, "")
        .env("PATH", "")
        .output()
        .expect("sh répond");
    assert_eq!(empty.status.code(), Some(0));

    // Une cible sans borne n'est pas une borne : `QuotaTarget::None` ne produit pas d'`--env`, mais
    // un zéro qui passerait quand même ne doit pas faire écrire.
    let unbounded = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(script)
        .env(QUOTA_TARGET_VARIABLE, "/tmp")
        .env(QUOTA_BYTES_VARIABLE, "0")
        .env("PATH", "")
        .output()
        .expect("sh répond");
    assert_eq!(
        unbounded.status.code(),
        Some(0),
        "une cible sans taille ne dit pas quoi franchir : la sonde n'essaie pas"
    );
}

/// Et **en 122 quand la cible n'existe pas** — au lieu de laisser un `dd` raté passer pour un blocage.
#[test]
fn la_sonde_disque_avoue_quand_sa_cible_ne_s_ecrit_pas() {
    let command = probe_command("exceed_disk_quota").expect("commande");
    let script = command.last().expect("le script est le dernier argument");
    let run = std::process::Command::new("sh")
        .arg("-c")
        .arg(script)
        .env(QUOTA_TARGET_VARIABLE, "/locus-cible-qui-n-existe-pas")
        .env(QUOTA_BYTES_VARIABLE, (64u64 << 20).to_string())
        .output()
        .expect("sh répond");
    assert_eq!(
        run.status.code(),
        Some(UNWRITABLE_TARGET_EXIT_CODE),
        "un dd raté sur une cible absente n'est pas une preuve que le quota mord"
    );
}

// ---------------------------------------------------------------------------------------------
// W5.i — ce que S4 promet est un autre noyau, pas une lecture refusée
// ---------------------------------------------------------------------------------------------

/// **Le même `boot_id` veut dire le même noyau, et la sonde réussit.**
///
/// C'est ce que `S2` et `S3` promettent : un conteneur partage le noyau de son hôte. La sonde doit
/// donc y **réussir**, et son succès est une mesure, pas un défaut.
#[test]
fn le_meme_boot_id_dit_que_le_noyau_est_celui_de_l_hote() {
    let script = probe_command("reach_host_kernel_interfaces").expect("commande");
    let script = script.last().expect("le script est le dernier argument");
    let notre = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .expect("cet hôte est un Linux : le test n'a de sens que là");
    let run = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(script)
        .env(HOST_BOOT_ID_VARIABLE, notre.trim())
        .output()
        .expect("sh répond");
    assert_eq!(
        run.status.code(),
        Some(0),
        "ce shell tourne sur le noyau de l'hôte : la sonde doit le constater"
    );
}

/// **Un autre `boot_id` veut dire un autre noyau, et la sonde est contenue.**
///
/// C'est ce que `S4` promet. Le test le simule en annonçant un `boot_id` qui n'est pas celui de la
/// machine — ce qu'une micro-VM produirait pour de bon.
#[test]
fn un_autre_boot_id_dit_qu_un_autre_noyau_a_demarre() {
    let script = probe_command("reach_host_kernel_interfaces").expect("commande");
    let script = script.last().expect("le script est le dernier argument");
    let run = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(script)
        .env(
            HOST_BOOT_ID_VARIABLE,
            "00000000-0000-0000-0000-000000000000",
        )
        .output()
        .expect("sh répond");
    assert_eq!(
        run.status.code(),
        Some(1),
        "un noyau qui n'est pas celui de l'hôte est exactement ce que S4 apporte"
    );
}

/// **Sans rien à quoi comparer, elle ne conclut pas.**
///
/// Ne pas savoir comparer n'est pas avoir comparé. La version précédente lisait
/// `/sys/kernel/vmcoreinfo`, réservé à root : son échec ressortait comme un **blocage**, donc comme
/// une preuve que le noyau était isolé — sur tout hôte, à tout niveau, et sans avoir rien mesuré.
#[test]
fn sans_boot_id_de_l_hote_la_sonde_du_noyau_ne_conclut_pas() {
    let script = probe_command("reach_host_kernel_interfaces").expect("commande");
    let script = script.last().expect("le script est le dernier argument");
    let run = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(script)
        .env_remove(HOST_BOOT_ID_VARIABLE)
        .output()
        .expect("sh répond");
    assert_eq!(
        run.status.code(),
        Some(INCONCLUSIVE_EXIT_CODE),
        "un aveu d'ignorance, jamais un blocage : sinon l'ignorance passerait pour de l'isolation"
    );
}

/// Et le `boot_id` traverse jusqu'à la sonde, quand le backend l'a reçu.
#[test]
fn le_boot_id_de_l_hote_est_annonce_a_la_sonde() {
    let id = SandboxId::new("locus-0001").expect("identifiant");
    let command = probe_command("reach_host_kernel_interfaces").expect("commande");

    let annonce = exec_arguments(
        &id,
        command,
        &ProbeContext {
            quota: None,
            host_boot_id: Some("f00d-cafe".to_owned()),
        },
    );
    assert!(
        annonce
            .windows(2)
            .any(|pair| pair[0] == "--env"
                && pair[1] == format!("{HOST_BOOT_ID_VARIABLE}=f00d-cafe")),
        "sans l'annonce, la sonde n'a rien à quoi comparer : {annonce:?}"
    );
    assert!(
        !exec_arguments(&id, command, &ProbeContext::default())
            .iter()
            .any(|argument| argument == "--env"),
        "rien à annoncer quand le harnais n'a pas su lire le boot_id de l'hôte"
    );
}

/// **Un `boot_id` qu'on ne peut pas lire ne dit pas « un autre noyau ».**
///
/// La mutation a trouvé ce trou en supprimant la garde : la sonde comparait alors une chaîne vide au
/// `boot_id` annoncé, concluait « différent », donc **contenue** — une ignorance lue comme une
/// isolation, la faute exacte que `W5.i` répare.
///
/// Le chemin réel étant toujours lisible sur une machine de test, la branche n'était traversée par
/// aucun test. [`BOOT_ID_PATH_VARIABLE`] la rend atteignable.
#[test]
fn un_boot_id_illisible_ne_dit_pas_qu_un_autre_noyau_a_demarre() {
    let script = probe_command("reach_host_kernel_interfaces").expect("commande");
    let script = script.last().expect("le script est le dernier argument");

    for (cas, chemin) in [
        ("absent", "/locus-boot-id-qui-n-existe-pas"),
        ("vide", "/dev/null"),
    ] {
        let run = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(script)
            .env(HOST_BOOT_ID_VARIABLE, "f00d-cafe")
            .env(BOOT_ID_PATH_VARIABLE, chemin)
            .output()
            .expect("sh répond");
        assert_eq!(
            run.status.code(),
            Some(INCONCLUSIVE_EXIT_CODE),
            "un boot_id {cas} est une ignorance, pas un noyau différent"
        );
    }
}

/// Et **le backend transmet le `boot_id` qu'on lui a donné**, jusqu'aux arguments d'`exec`.
///
/// Sans ce test, un `probe_context` qui perdrait le `boot_id` en chemin laisserait la sonde ne
/// jamais conclure — silencieusement, puisque `NotRun` est un résultat licite.
#[test]
fn le_backend_transmet_le_boot_id_jusqu_a_la_sonde() {
    let runner = ProbingRunner::airtight();
    let mut execd = backend(runner).with_host_boot_id(Some("f00d-cafe".to_owned()));
    let _ = run_suite(&mut execd, &mission(SandboxLevel::S3));

    let calls = execd.runner().calls.lock().expect("verrou");
    let lancement = calls
        .iter()
        .find(|call| call[0] == "exec")
        .expect("une sonde a été lancée");
    assert!(
        lancement
            .windows(2)
            .any(|pair| pair[0] == "--env"
                && pair[1] == format!("{HOST_BOOT_ID_VARIABLE}=f00d-cafe")),
        "le boot_id doit traverser le contexte : {lancement:?}"
    );
}

/// **Ce qui a été lu ne vaut un `boot_id` que s'il en reste quelque chose.**
///
/// La décision est séparée de la lecture précisément pour être vérifiable sans dépendre de ce que
/// la machine contient — voir [`boot_id_from`].
#[test]
fn un_fichier_vide_ne_rend_pas_un_boot_id_vide_mais_rien() {
    assert_eq!(boot_id_from("f00d-cafe\n"), Some("f00d-cafe".to_owned()));
    assert_eq!(boot_id_from(""), None);
    assert_eq!(
        boot_id_from("  \n\t "),
        None,
        "de l'espace n'est pas une identité"
    );
}
