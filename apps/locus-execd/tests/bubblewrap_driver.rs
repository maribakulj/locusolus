//! Test de sortie de `W5.af.3` — ADR 0035 décision 4.
//!
//! **La campagne des seize sondes s'exécute contre le mécanisme que le worker emploie, et
//! l'attestation qu'elle produit est dérivée de ce que la sandbox a répondu — jamais du plan.**
//!
//! La seconde moitié est celle qui ne se négocie pas, et elle est la même que pour podman : « un
//! broker qui composerait l'attestation à partir de ce qu'il avait demandé attesterait de sa propre
//! demande ». Un mécanisme qui rendrait `plan.level()` passerait toute la conformité en ayant tout
//! raté.
//!
//! # Deux moitiés, et la seconde échoue en le disant
//!
//! Les tests purs pilotent un double et vérifient les arguments, la comptabilité et les chemins
//! d'erreur — donc partout, y compris là où `bwrap` n'est pas installé. Les tests vivants lancent le
//! **vrai** programme : une traduction juste sur le papier et fausse à l'exécution est exactement ce
//! qu'un double ne peut pas voir. Sans `bwrap`, ils **échouent en le disant** plutôt que de se
//! sauter en silence — un saut muet ressemble en tout point à un succès.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;
use std::time::Duration;

use locus_execd::linux::Delegation;
use locus_execd::linux::bubblewrap::{
    BACKEND, INSPECTED_NAMESPACES, JOINING_PROGRAM, PROGRAM, unenforced,
};
use locus_execd::linux::bubblewrap_driver::BOUNDED_BACKEND;
use locus_execd::linux::bubblewrap_driver::host_namespaces;
use locus_execd::linux::probe::HostFacts;
use locus_execd::linux::{
    BubblewrapBackend, Execution, ProbeHost, Runner, SystemRunner, certify, host_boot_id, plan,
    run_suite,
};
use locus_execd::{RuntimeError, RuntimePort, SandboxId};
use locus_execution::{
    Mount, MountMode, NetworkMode, Observed, ResourceSpec, SandboxLevel, SandboxProfile,
    SandboxSpec,
};

// ---------------------------------------------------------------------------------------------
// Doubles et fixtures
// ---------------------------------------------------------------------------------------------

/// Un lanceur qui n'exécute rien : il note ce qu'on lui demande et rend ce qu'on lui a dit de
/// rendre.
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

    fn call(&self, index: usize) -> Vec<String> {
        self.calls.lock().expect("verrou")[index].clone()
    }

    fn count(&self) -> usize {
        self.calls.lock().expect("verrou").len()
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

fn ok(stdout: &str) -> Execution {
    Execution {
        code: 0,
        stdout: stdout.to_owned(),
        stderr: String::new(),
    }
}

/// Les namespaces que le double fera passer pour ceux de l'hôte.
fn hote() -> BTreeMap<String, String> {
    INSPECTED_NAMESPACES
        .iter()
        .map(|nom| ((*nom).to_owned(), format!("{nom}:[4026531000]")))
        .collect()
}

/// La réponse d'une sandbox : `obtenus` nomme les namespaces qui **diffèrent** de ceux de l'hôte.
fn reponse(obtenus: &[&str], readonly: &str, no_new_privs: &str, route: &str) -> String {
    let mut lignes: Vec<String> = INSPECTED_NAMESPACES
        .iter()
        .map(|nom| {
            let inode = if obtenus.contains(nom) {
                format!("{nom}:[4026532000]")
            } else {
                format!("{nom}:[4026531000]")
            };
            format!("{nom}={inode}")
        })
        .collect();
    lignes.push(format!("readonly={readonly}"));
    lignes.push(format!("no_new_privs={no_new_privs}"));
    lignes.push(format!("route={route}"));
    lignes.join("\n")
}

/// Tous les namespaces obtenus, racine scellée, privilèges figés, réseau retiré.
const TOUS: &[&str] = &["user", "pid", "ipc", "uts", "net", "cgroup", "mnt"];

fn spec(level: SandboxLevel) -> SandboxSpec {
    let network = if level >= SandboxLevel::S3 {
        NetworkMode::Deny
    } else {
        NetworkMode::Full
    };
    let travail =
        Mount::new(&source_de_travail(), "/travail", MountMode::ReadWrite).expect("montage licite");
    SandboxSpec::new(
        level,
        SandboxProfile::MathCompute,
        network,
        vec![travail],
        // Sans quota disque : `W5.j` l'impose, un quota ne s'applique qu'en dessous de `S2`.
        ResourceSpec::new(2_000, 4 << 30, 256, 0, 600).expect("quotas non nuls"),
    )
    .expect("une spec valide")
}

/// Une source de montage qui existe réellement, propre à cette exécution.
///
/// `bwrap` refuse une source absente, et un test qui lirait ce refus comme un verdict de confinement
/// conclurait juste pour une raison fausse.
fn source_de_travail() -> String {
    let chemin =
        std::path::PathBuf::from(std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".to_owned()))
            .join(format!("locus-bw-travail-{}", std::process::id()));
    std::fs::create_dir_all(&chemin).expect("la source du montage se crée");
    chemin.to_string_lossy().into_owned()
}

fn bwrap_disponible() -> bool {
    std::process::Command::new(PROGRAM)
        .arg("--version")
        .output()
        .is_ok_and(|sortie| sortie.status.success())
}

// ---------------------------------------------------------------------------------------------
// La comptabilité
// ---------------------------------------------------------------------------------------------

/// **`create` et `start` ne lancent rien**, et c'est un fait sur le mécanisme.
///
/// `podman create` fabrique un conteneur sur l'hôte ; `bwrap` n'a rien à fabriquer avant d'avoir une
/// commande à envelopper. Un test qui compterait les appels au lanceur après un `create` en attendrait
/// un, et c'est précisément l'hypothèse qu'il faut démentir.
#[test]
fn creer_et_demarrer_ne_lancent_rien() {
    let runner = ScriptedRunner::new(vec![]);
    let mut mecanisme = BubblewrapBackend::new(runner);

    let id = mecanisme
        .create(&spec(SandboxLevel::S2))
        .expect("le plan se calcule");
    mecanisme.start(&id).expect("le démarrage est comptable");
    mecanisme.stop(&id).expect("l'arrêt est comptable");

    assert_eq!(
        mecanisme.runner().count(),
        0,
        "aucun processus n'a été lancé : la sandbox naît avec la commande qu'elle enveloppe"
    );
}

/// Les quatre verbes **refusent** un identifiant qu'ils n'ont pas créé.
///
/// C'est ce qui reste de leur travail quand ils ne lancent rien, et ce n'est pas rien : sans ce
/// refus, une sonde pourrait porter sur une sandbox que personne n'a créée.
#[test]
fn les_verbes_refusent_un_identifiant_inconnu() {
    let mut mecanisme = BubblewrapBackend::new(ScriptedRunner::new(vec![]));
    let inconnu = SandboxId::new("jamais-creee").expect("l'identifiant est lisible");

    let attendu = RuntimeError::Unknown {
        id: inconnu.clone(),
    };
    assert_eq!(mecanisme.start(&inconnu), Err(attendu.clone()));
    assert_eq!(mecanisme.stop(&inconnu), Err(attendu.clone()));
    assert_eq!(mecanisme.remove(&inconnu), Err(attendu.clone()));
    assert!(matches!(
        mecanisme.attestation(&inconnu),
        Err(RuntimeError::Unknown { .. })
    ));
}

/// **Retirer rend le nom**, et retirer deux fois se dit.
///
/// « Je ne l'ai jamais eue » et « je l'ai rendue » sont deux faits différents ; les confondre
/// laisserait croire à un nettoyage qui n'a pas eu lieu.
#[test]
fn retirer_rend_le_nom_et_le_second_retrait_se_dit() {
    let mut mecanisme = BubblewrapBackend::new(ScriptedRunner::new(vec![]));
    let id = mecanisme
        .create(&spec(SandboxLevel::S1))
        .expect("le plan se calcule");

    assert_eq!(mecanisme.is_probeable(&id), Some(true));
    mecanisme.remove(&id).expect("le retrait aboutit");
    assert_eq!(mecanisme.is_probeable(&id), Some(false));
    assert!(matches!(
        mecanisme.remove(&id),
        Err(RuntimeError::Unknown { .. })
    ));
}

/// Deux sandboxes n'ont jamais le même nom.
#[test]
fn chaque_sandbox_recoit_son_nom() {
    let mut mecanisme = BubblewrapBackend::new(ScriptedRunner::new(vec![]));
    let un = mecanisme
        .create(&spec(SandboxLevel::S1))
        .expect("le plan se calcule");
    let deux = mecanisme
        .create(&spec(SandboxLevel::S1))
        .expect("le plan se calcule");
    assert_ne!(un, deux);
}

// ---------------------------------------------------------------------------------------------
// La sonde
// ---------------------------------------------------------------------------------------------

/// **La sonde est la commande enveloppée**, et son contexte voyage en variables déclarées.
///
/// `podman exec --env` fait la même chose de l'autre côté. Ce qui compte ici : que le contexte
/// n'arrive pas par l'environnement du parent, qui dépendrait de la machine.
#[test]
fn la_sonde_est_enveloppee_et_son_contexte_declare() {
    let runner = ScriptedRunner::new(vec![ok("")]);
    let mut mecanisme = BubblewrapBackend::new(runner).with_host_boot_id(Some("b00t".to_owned()));
    let id = mecanisme
        .create(&spec(SandboxLevel::S2))
        .expect("le plan se calcule");

    let contexte = locus_execd::linux::ProbeContext {
        quota: Some(("/travail".to_owned(), 4096)),
        host_boot_id: Some("b00t".to_owned()),
    };
    mecanisme
        .probe(&id, &["sh", "-c", "true"], &contexte)
        .expect("la sonde se lance");

    let appel = mecanisme.runner().call(0);
    let position = |mot: &str| appel.iter().position(|part| part == mot);

    let separateur = position("--").expect("le séparateur est posé");
    assert_eq!(
        &appel[separateur + 1..],
        &["sh".to_owned(), "-c".to_owned(), "true".to_owned()],
        "la commande vient après le séparateur, entière"
    );

    for (nom, valeur) in [
        ("LOCUS_QUOTA_TARGET", "/travail"),
        ("LOCUS_QUOTA_BYTES", "4096"),
        ("LOCUS_HOST_BOOT_ID", "b00t"),
    ] {
        let declaree = appel
            .windows(3)
            .position(|f| f[0] == "--setenv" && f[1] == nom)
            .map(|index| appel[index + 2].as_str());
        assert_eq!(declaree, Some(valeur), "« {nom} » est déclarée");
        assert!(
            position(nom).expect("la variable apparaît") < separateur,
            "et elle est déclarée **avant** le séparateur, sinon bwrap la lirait comme la commande"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// L'attestation
// ---------------------------------------------------------------------------------------------

/// **Le niveau attesté vient de ce que la sandbox a répondu**, pas du plan.
///
/// Le plan demande `S2` dans les trois cas ; les trois réponses en soutiennent trois niveaux
/// différents. Un mécanisme qui rendrait `plan.level()` passerait les trois en ayant tout raté.
#[test]
fn le_niveau_atteste_suit_la_reponse_et_non_le_plan() {
    let cas = [
        // Rien obtenu : pas même le namespace utilisateur.
        (reponse(&[], "false", "0", "present"), SandboxLevel::S0),
        // Utilisateur obtenu et privilèges figés, mais racine ouverte.
        (
            reponse(&["user"], "false", "1", "present"),
            SandboxLevel::S1,
        ),
        // Tout obtenu sauf le retrait du réseau.
        (
            reponse(
                &["user", "pid", "ipc", "uts", "cgroup", "mnt"],
                "true",
                "1",
                "present",
            ),
            SandboxLevel::S2,
        ),
        // Tout obtenu, réseau retiré et sans route.
        (reponse(TOUS, "true", "1", "absent"), SandboxLevel::S3),
    ];

    for (repondu, attendu) in cas {
        let runner = ScriptedRunner::new(vec![ok(&repondu)]);
        let mut mecanisme = BubblewrapBackend::new(runner).with_host_namespaces(hote());
        let id = mecanisme
            .create(&spec(SandboxLevel::S2))
            .expect("le plan se calcule");

        let atteste = mecanisme.attestation(&id).expect("l'attestation se lit");
        assert_eq!(atteste.applied_level(), attendu, "réponse : {repondu:?}");
        assert_eq!(atteste.attested_by(), BACKEND);
    }
}

/// **Sans les namespaces de l'hôte, l'attestation refuse** au lieu de croire le drapeau demandé.
///
/// Un inode relu ne se compare à rien tout seul. Conclure quand même reviendrait à attester ce qu'on
/// avait demandé — ce que cette attestation existe pour éviter.
#[test]
fn sans_les_namespaces_de_l_hote_l_attestation_refuse() {
    let runner = ScriptedRunner::new(vec![ok(&reponse(TOUS, "true", "1", "absent"))]);
    let mut mecanisme = BubblewrapBackend::new(runner);
    let id = mecanisme
        .create(&spec(SandboxLevel::S2))
        .expect("le plan se calcule");

    assert!(
        matches!(
            mecanisme.attestation(&id),
            Err(RuntimeError::Unsupported { .. })
        ),
        "une comparaison sans terme de comparaison ne conclut pas"
    );
}

/// **Un champ absent n'est pas une valeur par défaut.**
///
/// Le shell a pu échouer à mi-chemin. Deviner ferait attester un confinement sur une lecture qui n'a
/// pas eu lieu.
#[test]
fn un_champ_absent_de_la_reponse_refuse() {
    let tronquee = "user=user:[4026532000]\npid=pid:[4026532000]";
    let runner = ScriptedRunner::new(vec![ok(tronquee)]);
    let mut mecanisme = BubblewrapBackend::new(runner).with_host_namespaces(hote());
    let id = mecanisme
        .create(&spec(SandboxLevel::S2))
        .expect("le plan se calcule");

    assert!(matches!(
        mecanisme.attestation(&id),
        Err(RuntimeError::Unsupported { .. })
    ));
}

/// **Le témoignage porte ce que ce mécanisme n'applique pas**, sous les noms de fichiers de
/// contrôleur.
///
/// L'ADR 0035 refuse qu'une attestation annonce un niveau sans dire ce qui, dans ce niveau, n'est pas
/// tenu. Un exploitant qui lit « S3 sous bubblewrap » doit voir dans la même liste que sa borne
/// mémoire est tenue ailleurs, ou pas du tout.
#[test]
fn le_temoignage_nomme_les_limites_non_appliquees() {
    let runner = ScriptedRunner::new(vec![ok(&reponse(TOUS, "true", "1", "absent"))]);
    let mut mecanisme = BubblewrapBackend::new(runner).with_host_namespaces(hote());
    let id = mecanisme
        .create(&spec(SandboxLevel::S2))
        .expect("le plan se calcule");

    let atteste = mecanisme.attestation(&id).expect("l'attestation se lit");
    let temoignage = atteste.evidence().join("\n");

    for controleur in ["memory.max", "cpu.max"] {
        assert!(
            temoignage.contains(&format!("unenforced:{controleur}")),
            "« {controleur} » est nommé comme non appliqué : {temoignage}"
        );
    }
    assert!(
        temoignage.contains("readonly=true"),
        "et ce qui a été lu y est aussi, tel qu'il a été lu"
    );
}

/// **Un refus nomme le mécanisme qui refuse.**
///
/// Le message de `RuntimeError::Refused` disait `podman {verb}` **en dur**, dans un port qui ne
/// connaît aucun mécanisme. Tant qu'il n'y en avait qu'un, personne ne pouvait s'en apercevoir ;
/// avec deux, le même message aurait attribué à podman le refus d'un `bwrap`.
#[test]
fn un_refus_nomme_le_mecanisme_qui_refuse() {
    let echec = Execution {
        code: 1,
        stdout: String::new(),
        stderr: "bwrap: quelque chose".to_owned(),
    };
    let runner = ScriptedRunner::new(vec![echec]);
    let mut mecanisme = BubblewrapBackend::new(runner).with_host_namespaces(hote());
    let id = mecanisme
        .create(&spec(SandboxLevel::S2))
        .expect("le plan se calcule");

    let refus = mecanisme
        .attestation(&id)
        .expect_err("l'inspection a échoué");
    assert!(
        matches!(&refus, RuntimeError::Refused { backend, .. } if *backend == BACKEND),
        "le refus porte le nom du mécanisme : {refus:?}"
    );
    let dit = refus.to_string();
    assert!(
        dit.starts_with(BACKEND),
        "et le message le nomme en premier : {dit}"
    );
    assert!(
        !dit.contains("podman"),
        "sans attribuer ce refus à l'autre mécanisme : {dit}"
    );
}

// ---------------------------------------------------------------------------------------------
// Et la campagne tourne pour de vrai
// ---------------------------------------------------------------------------------------------

/// **Les seize sondes s'exécutent contre le vrai `bwrap`**, une sandbox par sonde.
///
/// C'est ce que l'ADR 0035 décision 4 demande : « `Proven` ne peut être rempli pour un worker réel
/// que par une campagne exerçant le mécanisme que ce worker emploie ». Le test n'exige pas un verdict
/// particulier de chaque sonde — c'est l'affaire de l'hôte — mais il exige que **les seize aient été
/// tentées** et qu'aucune ne soit restée sans réponse faute de mécanisme.
#[test]
fn la_campagne_tourne_contre_le_vrai_bwrap() {
    assert!(
        bwrap_disponible(),
        "NON MESURÉ : « {PROGRAM} » est introuvable sur cet hôte, donc la campagne n'a pas exercé \
         le mécanisme du worker. L'installer, ou porter cette vérification là où le worker tourne. \
         Ce test échoue plutôt que de passer."
    );

    let mut mecanisme = BubblewrapBackend::new(SystemRunner::new().with_program(PROGRAM))
        .with_host_namespaces(host_namespaces())
        .with_launch_pause(Duration::from_millis(0));

    let mission = spec(SandboxLevel::S2);
    let essais = run_suite(&mut mecanisme, &mission);

    assert_eq!(essais.len(), 16, "les seize sondes figurent au rapport");

    let noms: Vec<&str> = essais.iter().map(locus_execd::linux::Trial::name).collect();
    let mut tries = noms.clone();
    tries.sort_unstable();
    tries.dedup();
    assert_eq!(tries.len(), 16, "chacune y figure exactement une fois");

    // Une sonde « pas lancée » est un aveu d'ignorance, pas un verdict. Ici, la seule cause
    // légitime serait un hôte incomplet ; qu'elles soient **toutes** dans ce cas dirait que le
    // mécanisme n'a pas tourné du tout, ce que ce test existe pour attraper.
    let mesurees = essais
        .iter()
        .filter(|essai| !matches!(essai.observed(), locus_execution::Observed::NotRun { .. }))
        .count();
    assert!(
        mesurees > 0,
        "au moins une sonde a conclu : sinon rien n'a été mesuré. Rapport : {essais:?}"
    );
}

/// **La campagne dit exactement ce que `unenforced` annonce, et rien d'autre.**
///
/// C'est le test qui compte, parce qu'il fait se rencontrer deux choses écrites séparément : d'un
/// côté [`unenforced`], qui déclare *a priori* les limites que ce mécanisme ne portera pas ; de
/// l'autre la campagne, qui les éprouve *a posteriori* contre le vrai programme. Si l'une mentait,
/// les deux listes divergeraient.
///
/// Mesuré : le verdict est `NotTrusted` à `S2`, et il bloque sur **exactement** les trois sondes de
/// quota — `exceed_cpu_quota`, `exceed_memory_quota`, `exceed_pid_quota`, qui sont `cpu.max`,
/// `memory.max` et `pids.max`. Ce n'est pas un défaut à réparer : `bubblewrap` n'écrit aucun cgroup,
/// donc ces trois sondes n'ont rien à lire, et elles le disent au lieu de conclure. Un `Proven` pour
/// un worker sous bubblewrap demandera que ces bornes soient posées **autour** de `bwrap`, par qui
/// le lance.
///
/// Ce que le test n'exige **pas** : un verdict particulier. Il exige que le verdict et la déclaration
/// portent sur le même ensemble.
#[test]
fn ce_que_la_campagne_bloque_est_ce_que_unenforced_annonce() {
    assert!(
        bwrap_disponible(),
        "NON MESURÉ : « {PROGRAM} » est introuvable sur cet hôte"
    );

    let mission = spec(SandboxLevel::S2);
    let confinement = plan(&mission).expect("le plan se calcule");

    // Ce que la traduction déclare d'avance, restreint aux contrôleurs cgroup : ce sont les seuls
    // que la suite de sondes sait éprouver.
    let declarees: BTreeSet<String> = unenforced(&confinement)
        .into_iter()
        .map(|manquante| manquante.limit)
        .filter(|limite| limite.contains('.'))
        .collect();
    assert!(
        !declarees.is_empty(),
        "la fixture réserve bien des ressources : sinon la comparaison porterait sur du vide"
    );

    // Ce que la campagne constate.
    let mut mecanisme = BubblewrapBackend::new(SystemRunner::new().with_program(PROGRAM))
        .with_host_namespaces(host_namespaces())
        .with_host_boot_id(host_boot_id())
        .with_launch_pause(Duration::from_millis(0));
    let essais = run_suite(&mut mecanisme, &mission);

    let controleur = |sonde: &str| match sonde {
        "exceed_cpu_quota" => Some("cpu.max"),
        "exceed_memory_quota" => Some("memory.max"),
        "exceed_pid_quota" => Some("pids.max"),
        _ => None,
    };
    let constatees: BTreeSet<String> = essais
        .iter()
        .filter(|essai| matches!(essai.observed(), Observed::NotRun { .. }))
        .filter_map(|essai| controleur(essai.name()).map(str::to_owned))
        .collect();

    assert_eq!(
        constatees, declarees,
        "les limites que la campagne ne peut pas éprouver sont exactement celles que `unenforced` \
         déclare non portées.\n  campagne : {constatees:?}\n  déclarées : {declarees:?}"
    );

    // Et le verdict en tire la conséquence : pas de `Trusted` tant que ces trois-là n'ont rien lu.
    let verdict = certify(&mut mecanisme, &mission, SandboxLevel::S2);
    let annonce = format!("{verdict:?}");
    assert!(
        annonce.contains("NotTrusted") && annonce.contains("S2"),
        "le niveau annoncé n'est pas tenu, et le verdict le dit : {annonce}"
    );
    for sonde in [
        "exceed_cpu_quota",
        "exceed_memory_quota",
        "exceed_pid_quota",
    ] {
        assert!(
            annonce.contains(sonde),
            "« {sonde} » figure parmi les raisons du refus : {annonce}"
        );
    }
}

/// **Les neuf sondes de confinement sont contenues** contre le vrai programme.
///
/// C'est la racine bâtie de `W5.af.2`, éprouvée cette fois par la campagne plutôt que par un test
/// écrit à la main. Aucune n'est `Succeeded` : une seule le serait que le confinement ne tiendrait
/// pas là où son niveau le promet.
#[test]
fn les_sondes_de_confinement_sont_contenues_contre_le_vrai_bwrap() {
    assert!(
        bwrap_disponible(),
        "NON MESURÉ : « {PROGRAM} » est introuvable sur cet hôte"
    );

    let mut mecanisme = BubblewrapBackend::new(SystemRunner::new().with_program(PROGRAM))
        .with_host_namespaces(host_namespaces())
        .with_host_boot_id(host_boot_id())
        .with_launch_pause(Duration::from_millis(0));
    let essais = run_suite(&mut mecanisme, &spec(SandboxLevel::S2));

    // Les neuf que la racine bâtie doit contenir à `S2`. `reach_host_kernel_interfaces` n'en est
    // pas : elle constate un **noyau partagé**, ce qui est vrai de bubblewrap et ne devient faux
    // qu'à `S4`, hors de portée de ce mécanisme.
    for sonde in [
        "write_outside_workspace",
        "write_host_home",
        "persist_after_teardown",
        "read_host_filesystem",
        "read_host_secret_files",
        "read_process_environment",
        "access_container_runtime_socket",
        "escalate_to_root",
        "observe_host_processes",
    ] {
        let essai = essais
            .iter()
            .find(|essai| essai.name() == sonde)
            .unwrap_or_else(|| panic!("« {sonde} » figure au rapport"));
        assert_eq!(
            essai.observed(),
            Observed::Blocked,
            "« {sonde} » est contenue : {essai:?}"
        );
    }
}

/// **L'attestation contre le vrai programme dérive du réel.**
///
/// Une sandbox `bwrap` est entièrement déterminée par ses arguments : rouvrir avec les mêmes et
/// demander de l'intérieur est une inspection au sens plein. Ce test vérifie que ce qui en sort n'est
/// pas le plan recopié — le plan demande `S2`, et ce qui est attesté doit venir de la réponse.
#[test]
fn l_attestation_vivante_lit_ce_que_la_sandbox_repond() {
    assert!(
        bwrap_disponible(),
        "NON MESURÉ : « {PROGRAM} » est introuvable sur cet hôte"
    );

    let mut mecanisme = BubblewrapBackend::new(SystemRunner::new().with_program(PROGRAM))
        .with_host_namespaces(host_namespaces());
    let id = mecanisme
        .create(&spec(SandboxLevel::S2))
        .expect("le plan se calcule");
    mecanisme.start(&id).expect("le démarrage est comptable");

    let atteste = mecanisme.attestation(&id).expect("l'attestation se lit");
    assert_eq!(atteste.attested_by(), BACKEND);

    let temoignage = atteste.evidence().join("\n");
    assert!(
        temoignage.contains("readonly=true"),
        "la racine scellée par le plan l'est réellement : {temoignage}"
    );
    assert!(
        temoignage.contains("no_new_privs=1"),
        "et `no_new_privs` est **lu**, pas supposé : {temoignage}"
    );

    // Le plan de la fixture demande `S2`. Ce qui est attesté doit tenir sans jamais le recopier :
    // le niveau vient des namespaces réellement obtenus.
    assert!(
        atteste.applied_level() >= SandboxLevel::S2,
        "le confinement obtenu soutient au moins ce que la fixture demande : {temoignage}"
    );
}

/// Les namespaces de l'hôte se lisent, et ils sont **tous** là.
///
/// Une entrée manquante n'est pas remplie d'un repli : une comparaison contre un repli inventé
/// rendrait « namespace obtenu » sur un `/proc` illisible.
#[test]
fn les_namespaces_de_l_hote_se_lisent_sans_repli() {
    let lus = host_namespaces();
    for namespace in INSPECTED_NAMESPACES {
        assert!(
            lus.contains_key(namespace),
            "« {namespace} » a été lu sur cet hôte : {lus:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Le bornage : un second mécanisme, sous un second nom
// ---------------------------------------------------------------------------------------------

/// Un cgroup parent simulé — un répertoire ordinaire suffit à éprouver ce qui s'y écrit.
///
/// **Propre à chaque test**, et pas seulement au processus. Les tests d'un même binaire tournent en
/// parallèle **dans le même processus** : un chemin par PID est donc partagé, et deux tests y
/// allouent tous deux `locus-bw-0001` puisque le compteur de chaque backend repart à un. La
/// première rédaction l'a appris en voyant le refus de nom déjà pris — celui que `W5.ai.2` a posé
/// exprès — se déclencher entre deux tests plutôt que contre une vraie collision.
fn parent_cgroup(epreuve: &str) -> std::path::PathBuf {
    let racine = std::env::temp_dir().join(format!("locus-bwcg-{}-{epreuve}", std::process::id()));
    let _ = std::fs::remove_dir_all(&racine);
    std::fs::create_dir_all(&racine).expect("le parent se crée");
    racine
}

/// Une délégation qui porte les trois contrôleurs, lue depuis un hôte décrit.
fn delegation_complete() -> Delegation {
    struct Hote;
    impl locus_execd::linux::probe::Reader for Hote {
        fn read(&self, path: &str) -> Option<String> {
            match path {
                "/sys/fs/cgroup/cgroup.controllers"
                | "/sys/fs/cgroup/locus.slice/cgroup.controllers" => {
                    Some("cpu memory pids".to_owned())
                }
                "/proc/self/cgroup" => Some("0::/locus.slice".to_owned()),
                _ => None,
            }
        }
    }
    Delegation::read(&HostFacts::probe(&Hote)).expect("la délégation se lit")
}

/// **Sans cgroup, le mécanisme porte son nom simple, et le témoignage dit que les bornes manquent.**
#[test]
fn sans_cgroup_le_mecanisme_ne_borne_pas_et_le_dit() {
    let runner = ScriptedRunner::new(vec![ok(&reponse(TOUS, "true", "1", "absent"))]);
    let mut mecanisme = BubblewrapBackend::new(runner).with_host_namespaces(hote());
    assert_eq!(mecanisme.attested_backend(), BACKEND);
    assert!(!mecanisme.bounds_resources());

    let id = mecanisme
        .create(&spec(SandboxLevel::S2))
        .expect("le plan se calcule");
    let temoignage = mecanisme
        .attestation(&id)
        .expect("l'attestation se lit")
        .evidence()
        .join("\n");

    assert!(
        temoignage.contains("unenforced:memory.max"),
        "la borne n'est pas tenue, et le témoignage le dit : {temoignage}"
    );
    assert!(
        !temoignage.contains("enforced_by_broker_cgroup"),
        "et personne ne prétend la tenir : {temoignage}"
    );
}

/// **Avec cgroup, le mécanisme change de nom, et le témoignage dit *qui* tient les bornes.**
///
/// Les deux moitiés comptent. Le nom, parce que l'ADR 0036 refuse qu'un enregistrement affirme de
/// `bubblewrap` une propriété que seul le broker tient. Le témoignage, parce qu'écrire simplement
/// « tenue » laisserait croire que le mécanisme nommé la porte, et l'omettre laisserait croire
/// qu'elle n'est pas tenue du tout.
#[test]
fn avec_cgroup_le_mecanisme_change_de_nom_et_nomme_qui_borne() {
    let racine = parent_cgroup("nom-et-temoignage");
    let runner = ScriptedRunner::new(vec![ok(&reponse(TOUS, "true", "1", "absent"))]);
    let enveloppe = ScriptedRunner::new(vec![ok(&reponse(TOUS, "true", "1", "absent"))]);
    let mut mecanisme = BubblewrapBackend::new(runner)
        .with_host_namespaces(hote())
        .with_cgroup(delegation_complete(), racine.clone(), enveloppe);

    assert_eq!(mecanisme.attested_backend(), BOUNDED_BACKEND);
    assert_ne!(BOUNDED_BACKEND, BACKEND, "deux mécanismes, deux noms");
    assert!(mecanisme.bounds_resources());

    let id = mecanisme
        .create(&spec(SandboxLevel::S2))
        .expect("le plan se calcule et le cgroup se pose");

    // Le cgroup a réellement été posé, avec les limites du plan.
    let confinement = plan(&spec(SandboxLevel::S2)).expect("le plan se calcule");
    for limite in confinement.cgroup() {
        let ecrit = std::fs::read_to_string(racine.join(id.as_str()).join(limite.file))
            .unwrap_or_else(|_| panic!("« {} » a été écrit", limite.file));
        assert_eq!(ecrit, limite.value);
    }

    let atteste = mecanisme.attestation(&id).expect("l'attestation se lit");
    assert_eq!(atteste.attested_by(), BOUNDED_BACKEND);
    let temoignage = atteste.evidence().join("\n");
    assert!(
        temoignage.contains("enforced_by_broker_cgroup:memory.max"),
        "le témoignage nomme qui tient la borne : {temoignage}"
    );
    assert!(
        !temoignage.contains("unenforced:memory.max"),
        "et ne dit plus qu'elle manque : {temoignage}"
    );

    let _ = std::fs::remove_dir_all(&racine);
}

/// **La sonde passe par l'enveloppeur** quand un cgroup a été posé.
#[test]
fn avec_cgroup_la_sonde_entre_dans_le_cgroup_avant_de_se_lancer() {
    let racine = parent_cgroup("sonde-enveloppee");
    let enveloppe = ScriptedRunner::new(vec![ok("")]);
    let mut mecanisme = BubblewrapBackend::new(ScriptedRunner::new(vec![]))
        .with_host_namespaces(hote())
        .with_cgroup(delegation_complete(), racine.clone(), enveloppe);

    let id = mecanisme
        .create(&spec(SandboxLevel::S2))
        .expect("le cgroup se pose");
    let contexte = locus_execd::linux::ProbeContext {
        quota: None,
        host_boot_id: None,
    };
    mecanisme
        .probe(&id, &["sh", "-c", "true"], &contexte)
        .expect("la sonde se lance");

    // C'est le lanceur **du bwrap** qui n'a rien reçu : tout est passé par l'enveloppe.
    assert_eq!(
        mecanisme.runner().count(),
        0,
        "la sandbox n'est plus lancée directement quand un cgroup la borne"
    );

    let _ = std::fs::remove_dir_all(&racine);
}

/// **Une pose qui échoue ne rend pas d'identifiant.**
///
/// Un appelant qui recevrait un identifiant utilisable alors que la borne a échoué lancerait sa
/// sandbox **sans borne** — l'un des deux modes d'échec silencieux de l'ADR 0036.
#[test]
fn une_pose_qui_echoue_ne_rend_pas_d_identifiant() {
    let absent = std::env::temp_dir().join(format!("locus-bwcg-absent-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&absent);

    let mut mecanisme = BubblewrapBackend::new(ScriptedRunner::new(vec![]))
        .with_host_namespaces(hote())
        .with_cgroup(
            delegation_complete(),
            absent,
            ScriptedRunner::new(vec![ok("")]),
        );

    let refus = mecanisme
        .create(&spec(SandboxLevel::S2))
        .expect_err("le parent n'existe pas : la pose échoue");
    let dit = refus.to_string();
    assert!(
        dit.contains("cgroup"),
        "le refus nomme ce qui a échoué : {dit}"
    );
}

// ---------------------------------------------------------------------------------------------
// Et contre l'hôte réel : ce que le bornage y donne
// ---------------------------------------------------------------------------------------------

/// **Ce que cet hôte permet réellement du bornage** — mesuré, pas supposé.
///
/// Ce test n'exige pas qu'un cgroup se pose : la délégation appartient à l'hôte. Il exige que
/// l'issue soit **conclusive** — une pose, ou un refus nommé — et il l'imprime, parce que c'est le
/// fait dont la suite dépend.
///
/// C'est le motif de `W5.f`, « sonder ce que la CI sait faire », appliqué au cgroup : on mesure
/// d'abord, on resserre ensuite. Ce qu'on ne fait pas, c'est supposer.
///
/// Mesuré au moment de l'écrire : le conteneur de développement **ne délègue rien**, donc le refus
/// est le seul chemin qu'il puisse prendre. Le runner de CI délègue `cpu, cpuset, io, memory,
/// pids` — reste à savoir s'il permet d'**écrire** dans le `subtree_control` de notre propre
/// cgroup, ce que seule cette exécution dira.
#[test]
fn ce_que_cet_hote_permet_du_bornage() {
    let facts = HostFacts::read_host();
    let Ok(delegation) = Delegation::read(&facts) else {
        let refus = Delegation::read(&facts).expect_err("rien n'est délégué");
        println!("bornage impossible sur cet hôte : {refus}");
        assert!(
            !refus.to_string().is_empty(),
            "un refus dit pourquoi ; un refus muet ne se répare pas"
        );
        return;
    };

    println!("cet hôte délègue : {:?}", delegation.controllers());
    let confinement = plan(&spec(SandboxLevel::S2)).expect("le plan se calcule");
    let sous = std::path::PathBuf::from(own_cgroup_directory());
    let nom = format!("locus-epreuve-{}", std::process::id());

    match delegation.place(&sous, &nom, &confinement) {
        Ok(pose) => {
            println!("cgroup posé : {}", pose.directory().display());
            assert!(
                pose.directory().exists(),
                "une pose rendue a bien créé son répertoire"
            );
            match pose.remove() {
                Ok(()) => println!("et retiré"),
                Err(erreur) => println!("posé mais non retiré : {erreur}"),
            }
        }
        Err(erreur) => {
            // Un hôte qui délègue et refuse la pose est un fait sur cet hôte, pas un échec du
            // module. Ce qui serait un échec, c'est un refus qui ne dise pas lequel des quatre
            // fichiers a résisté.
            println!("cgroup non posé, et voici pourquoi : {erreur}");
            let dit = erreur.to_string();
            assert!(
                dit.contains('/'),
                "le refus nomme le fichier ou le répertoire qui a résisté : {dit}"
            );
        }
    }
}

/// Le répertoire cgroup de ce processus, ou la racine unifiée à défaut.
fn own_cgroup_directory() -> String {
    std::fs::read_to_string("/proc/self/cgroup")
        .ok()
        .and_then(|contenu| {
            contenu
                .lines()
                .find_map(|ligne| ligne.strip_prefix("0::"))
                .map(|relatif| format!("/sys/fs/cgroup/{}", relatif.trim().trim_start_matches('/')))
        })
        .unwrap_or_else(|| "/sys/fs/cgroup".to_owned())
}

/// Le programme d'enveloppe existe sur cet hôte.
///
/// Sans lui, un mécanisme borné ne pourrait pas entrer dans son cgroup — et il échouerait au
/// lancement plutôt qu'à la construction, c'est-à-dire trop tard pour qu'on sache pourquoi.
#[test]
fn le_programme_d_enveloppe_existe() {
    assert!(
        std::path::Path::new(JOINING_PROGRAM).exists(),
        "« {JOINING_PROGRAM} » est là : c'est lui qui entre dans le cgroup avant d'exécuter la \
         sandbox"
    );
}
