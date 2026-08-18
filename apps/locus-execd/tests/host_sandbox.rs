//! Test de sortie de `W5.f` — **la validation sémantique des sondes contre une sandbox réelle.**
//!
//! « Sur un hôte capable de `S2`, chaque sonde produit le verdict que son `contained_from`
//! annonce — c'est la seule chose que ni `sh -n` ni un double ne peuvent dire. »
//!
//! # Ce que les autres tests ne peuvent pas dire
//!
//! `tests/podman.rs` pilote un `ScriptedRunner` : il vérifie les arguments passés au runtime,
//! l'analyse des sorties et les chemins d'erreur. C'est exactement ce qu'il faut « là où aucun
//! runtime rootless n'est garanti — c'est-à-dire en CI », et c'est aussi sa limite : un double
//! rend ce qu'on lui a dit de rendre. Il ne sait pas si `cpu.max` mord, si `--userns` ferme
//! vraiment la vue sur les processus de l'hôte, ni si le profil seccomp refuse `unshare`.
//!
//! `selftest.rs` vérifie que chaque sonde est du shell que `sh -n` accepte. Une syntaxe correcte
//! n'est pas une sémantique correcte : `[ "${after:-0}" -eq "${before:-0}" ]` se parse
//! parfaitement et ne prouve rien tant que personne n'a vu `nr_throttled` bouger.
//!
//! Ce test est le seul du dépôt qui fasse **tourner** les seize sondes dans un conteneur réel.
//!
//! # Pourquoi il est `#[ignore]`
//!
//! Il exige un hôte capable de `S2` : un Podman rootless en état de marche, des cgroups v2, et
//! une image. Un test qui se sauterait tout seul quand ces conditions manquent ressemblerait en
//! tout point à un test qui passe — c'est la leçon que `--require-emacs` a déjà coûtée. `ignored`
//! apparaît dans la sortie de `cargo test` ; « sauté en silence » n'y apparaît pas.
//!
//! Il se lance donc explicitement :
//!
//! ```text
//! LOCUS_PROBE_IMAGE=docker.io/library/alpine@sha256:… \
//!   cargo test -p locus-execd --test host_sandbox -- --ignored --nocapture
//! ```
//!
//! # Ce que le test imprime avant d'affirmer
//!
//! La table complète — sonde, attente, observation, verdict — **avant** toute assertion. Un échec
//! doit dire *laquelle* des seize n'a pas tenu et *comment*, pas seulement que le niveau n'est pas
//! tenu. C'est la moitié utile d'un premier passage sur un hôte qu'on ne connaît pas encore.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

use locus_execd::linux::{
    MUST_DENY, PodmanBackend, RestrictedProfile, SeccompProfiles, SystemRunner, Workload, run_suite,
};
use locus_execd::{RuntimePort, SandboxId};
use locus_execution::{
    Expectation, Mount, MountMode, NetworkMode, Observed, ResourceSpec, SUITE, SandboxLevel,
    SandboxProfile, SandboxSpec, Standing, Verdict, expectation, judge,
};

/// La variable qui porte l'image, digest compris.
const IMAGE: &str = "LOCUS_PROBE_IMAGE";

/// Le niveau que `W5.f` nomme.
const LEVEL: SandboxLevel = SandboxLevel::S2;

// ---------------------------------------------------------------------------------------------
// Le profil seccomp du test — et pourquoi il n'est pas livré comme profil de déploiement
// ---------------------------------------------------------------------------------------------

/// Un profil restreint minimal, écrit par le test dans un fichier temporaire.
///
/// `tests/seccomp.rs` explique pourquoi le dépôt **ne livre pas** de profil : « un profil par
/// défaut-refus est une liste de plusieurs centaines d'appels autorisés dont l'exactitude ne se
/// démontre qu'en l'exécutant. En écrire un sans hôte pour l'éprouver produirait soit une sandbox
/// qui casse tout, soit une sandbox qui autorise ce qu'elle prétend refuser. »
///
/// Celui-ci ne prétend pas être ce profil-là. Il est **défaut-permissif** et refuse nommément les
/// huit appels de [`MUST_DENY`] : la posture exacte que `RestrictedProfile` vérifie, et rien de
/// plus. Il ne casse rien, il ne promet rien d'autre, et il vit dans un fichier temporaire plutôt
/// que dans le dépôt — précisément pour que personne ne le prenne pour le profil de production.
///
/// Ce que ce test peut dire, et que `tests/seccomp.rs` ne pouvait pas : que la posture restreinte
/// **s'applique** sur un vrai runtime. La sonde `escalate_to_root` et le refus d'`unshare` sont
/// ce qui le montre.
fn write_restricted_profile() -> (PathBuf, RestrictedProfile) {
    let body = format!(
        r#"{{
  "defaultAction": "SCMP_ACT_ALLOW",
  "syscalls": [{{ "names": [{}], "action": "SCMP_ACT_ERRNO" }}]
}}"#,
        MUST_DENY
            .iter()
            .map(|name| format!("\"{name}\""))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let path = env::temp_dir().join(format!("locus-probe-seccomp-{}.json", process::id()));
    fs::write(&path, &body).expect("le profil doit pouvoir s'écrire dans le répertoire temporaire");
    let display = path.to_string_lossy().into_owned();
    let profile = RestrictedProfile::parse(&display, &body)
        .expect("le profil du test refuse les huit appels que la posture exige");
    (path, profile)
}

// ---------------------------------------------------------------------------------------------
// La spécification éprouvée
// ---------------------------------------------------------------------------------------------

/// Ce que la mission d'épreuve exige.
///
/// `NetworkMode::Full` et non `Deny` : `S2` ne s'appelle pas `container-isolated-network`, et
/// `plan` refuse explicitement un mode autre que `full` en deçà de `S3` — « un processus rootless
/// sans namespace réseau voit le réseau de l'hôte, et dire "deny" là-dessus serait un mensonge ».
/// Les deux sondes réseau sont donc `Allowed` à ce niveau, et **doivent réussir**.
///
/// Un montage en lecture seule d'un répertoire temporaire : la sandbox doit avoir un espace de
/// travail légitime, sans quoi `exceed_disk_quota` éprouverait une racine en lecture seule plutôt
/// qu'un quota.
fn probed_spec(workspace: &str) -> SandboxSpec {
    SandboxSpec::new(
        LEVEL,
        SandboxProfile::UntrustedRepository,
        NetworkMode::Full,
        vec![Mount::new(workspace, "/work", MountMode::ReadWrite).expect("montage licite")],
        ResourceSpec::new(1_000, 512 << 20, 128, 1 << 30, 300).expect("quotas non nuls"),
    )
    .expect("spécification valide")
}

// ---------------------------------------------------------------------------------------------
// Le test
// ---------------------------------------------------------------------------------------------

/// Les seize sondes, dans un conteneur rootless réel, à `S2`.
///
/// # Ce qui est affirmé, et dans quel ordre
///
/// D'abord que **quelque chose a tourné** : si les seize sont `NotRun`, l'hôte n'a pas de runtime
/// et le test ne dit rien du confinement — le distinguer d'un échec de confinement est la
/// différence entre « il manque une machine » et « la sandbox ne tient pas ».
///
/// Ensuite, sonde par sonde, que le verdict est `Holds`. Un `Escaped` est un trou ; un
/// `OverContained` est un backend plus strict qu'annoncé, qui fera échouer des missions légitimes
/// de façon inexplicable ; un `Inconclusive` n'a rien prouvé. Les trois sont des échecs de cet
/// item, et chacun est nommé séparément.
///
/// Enfin le `Standing`, qui est la forme sous laquelle le reste du système lit ce verdict.
#[test]
#[ignore = "exige un hôte capable de S2 : Podman rootless, cgroups v2, et LOCUS_PROBE_IMAGE"]
fn les_seize_sondes_rendent_a_s2_le_verdict_que_leur_contained_from_annonce() {
    let image = env::var(IMAGE).unwrap_or_else(|_| {
        panic!(
            "{IMAGE} doit porter une image avec son digest (…@sha256:…) — sans image, il n'y a \
             rien à éprouver, et une image sans digest n'est pas reproductible"
        )
    });
    let workload = Workload::new(&image, vec!["sleep".to_owned(), "600".to_owned()])
        .expect("une image à digest et une commande non vide");

    let workspace = env::temp_dir().join(format!("locus-probe-work-{}", process::id()));
    fs::create_dir_all(&workspace).expect("l'espace de travail doit pouvoir se créer");
    let spec = probed_spec(&workspace.to_string_lossy());

    let (profile_path, restricted) = write_restricted_profile();
    let mut backend = PodmanBackend::new(
        SystemRunner,
        SeccompProfiles {
            restricted: Some(restricted),
        },
        workload,
    );

    let results = run_probes(&mut backend, &spec);
    let _ = fs::remove_file(&profile_path);
    let _ = fs::remove_dir_all(&workspace);

    report(&results);

    assert!(
        results
            .iter()
            .any(|(_, observed)| !matches!(observed, Observed::NotRun { .. })),
        "aucune des seize sondes n'a pu être lancée : cet hôte n'a pas de runtime rootless en \
         état de marche. Ce n'est pas un échec de confinement, et le lire comme tel ferait \
         chercher une faille là où il manque une machine."
    );

    let mut failures = Vec::new();
    for probe in &SUITE {
        let observed = observation(&results, probe.name);
        match judge(probe, LEVEL, observed) {
            Verdict::Holds => {}
            other => failures.push(other),
        }
    }
    assert!(
        failures.is_empty(),
        "{} sonde(s) ne rendent pas à {} le verdict que leur `contained_from` annonce :\n{}",
        failures.len(),
        LEVEL.code(),
        failures
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
    );

    assert_eq!(
        locus_execution::standing(LEVEL, &results),
        Standing::Trusted { level: LEVEL },
        "les seize tiennent une à une : le `Standing` doit le dire aussi, sans quoi c'est \
         l'agrégation qui est fausse et non les sondes"
    );
}

// ---------------------------------------------------------------------------------------------
// Utilitaires
// ---------------------------------------------------------------------------------------------

/// Créer, démarrer, éprouver, arrêter — ou rendre seize `NotRun` nommant l'erreur du runtime.
///
/// [`certify`] rend une erreur quand la sandbox n'a pas démarré, et « une sandbox qui n'a pas
/// démarré n'a rien à éprouver ». Ici il faut malgré tout une ligne par sonde, parce que le
/// rapport imprimé est la moitié utile d'un premier passage : une table vide ne dirait pas
/// laquelle des seize manque.
fn run_probes(
    backend: &mut PodmanBackend<SystemRunner>,
    spec: &SandboxSpec,
) -> Vec<(&'static str, Observed)> {
    match backend.create(spec) {
        Ok(id) => probe_started(backend, &id),
        Err(error) => {
            eprintln!("la sandbox n'a pas été créée : {error}");
            all_unrun()
        }
    }
}

fn probe_started(
    backend: &mut PodmanBackend<SystemRunner>,
    id: &SandboxId,
) -> Vec<(&'static str, Observed)> {
    if let Err(error) = backend.start(id) {
        eprintln!("la sandbox n'a pas démarré : {error}");
        return all_unrun();
    }
    let results = run_suite(backend, id);
    let _ = backend.stop(id);
    results
}

/// Seize lignes disant que rien n'a tourné.
fn all_unrun() -> Vec<(&'static str, Observed)> {
    SUITE
        .iter()
        .map(|probe| {
            (
                probe.name,
                Observed::NotRun {
                    reason: "le runtime n'a pas exécuté la sonde",
                },
            )
        })
        .collect()
}

/// Ce qu'une sonde a produit, ou l'aveu qu'elle est absente du rapport.
fn observation(results: &[(&'static str, Observed)], name: &str) -> Observed {
    results.iter().find(|(probe, _)| *probe == name).map_or(
        Observed::NotRun {
            reason: "sonde absente du rapport",
        },
        |(_, observed)| *observed,
    )
}

/// La table, imprimée avant toute assertion.
fn report(results: &[(&'static str, Observed)]) {
    println!("\nsondes à {} — hôte réel\n", LEVEL.code());
    for probe in &SUITE {
        let observed = observation(results, probe.name);
        println!(
            "  {:<32} attendu {:<10} observé {:<28} → {}",
            probe.name,
            match expectation(probe, LEVEL) {
                Expectation::Contained => "contenue",
                Expectation::Allowed => "permise",
            },
            match observed {
                Observed::Succeeded => "réussie".to_owned(),
                Observed::Blocked => "bloquée".to_owned(),
                Observed::NotRun { reason } => format!("non lancée ({reason})"),
            },
            match judge(probe, LEVEL, observed) {
                Verdict::Holds => "tient".to_owned(),
                other => other.to_string(),
            }
        );
    }
    println!();
}
