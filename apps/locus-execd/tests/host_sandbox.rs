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
//! Ce fichier est le seul du dépôt qui fasse **tourner** les sondes dans un conteneur réel.
//!
//! # Deux tests, et le second ne remplace pas le premier
//!
//! Le premier demande si cet hôte tient `S2` sous une mission qui réserve du disque. Le second
//! éprouve les **quinze** sondes qui ne dépendent pas du quota disque, et **n'établit jamais
//! `S2`** — l'exclusion y est nommée plutôt que silencieuse.
//!
//! Cette séparation vient du premier passage réel, qui a montré ce qu'aucun double ne pouvait
//! dire : le runner GitHub fait tourner Podman rootless, et `podman create` refuse malgré tout la
//! spécification, parce que `--storage-opt size=` n'existe que sur XFS. Un seul test aurait dû
//! choisir entre ne rien observer et conclure sur `S2` sans le quota — le premier n'apprend rien,
//! le second serait faux.
//!
//! # Pourquoi ils sont `#[ignore]`
//!
//! Ils exigent un hôte : un Podman rootless en état de marche, des cgroups v2, et une image. Un
//! test qui se sauterait tout seul quand ces conditions manquent ressemblerait en tout point à un
//! test qui passe — c'est la leçon que `--require-emacs` a déjà coûtée. `ignored` apparaît dans la
//! sortie de `cargo test` ; « sauté en silence » n'y apparaît pas.
//!
//! Ils se lancent donc explicitement :
//!
//! ```text
//! LOCUS_PROBE_IMAGE=docker.io/library/alpine@sha256:… \
//!   cargo test -p locus-execd --test host_sandbox -- --ignored --nocapture
//! ```
//!
//! # Ce qui s'imprime avant d'affirmer
//!
//! La table complète — sonde, attente, observation, verdict — **avant** toute assertion. Un échec
//! doit dire *laquelle* des seize n'a pas tenu et *comment*, pas seulement que le niveau n'est pas
//! tenu. C'est la moitié utile d'un premier passage sur un hôte qu'on ne connaît pas encore.
//!
//! Et quand la sandbox ne démarre pas, ce qui s'imprime est **le message du runtime, mot pour
//! mot** — pas une table de seize `NotRun`, qui ressemblerait à un échec de confinement alors
//! qu'il n'y a eu aucune observation.

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
/// Le `tag` distingue les fichiers des deux tests : `cargo test` les lance dans le même
/// processus, donc `process::id()` ne suffit pas à les séparer, et le nettoyage de l'un
/// effacerait le profil que l'autre est en train d'utiliser.
///
/// Ce que ce test peut dire, et que `tests/seccomp.rs` ne pouvait pas : que la posture restreinte
/// **s'applique** sur un vrai runtime. La sonde `escalate_to_root` et le refus d'`unshare` sont
/// ce qui le montre.
fn write_restricted_profile(tag: &str) -> (PathBuf, RestrictedProfile) {
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
    let path = env::temp_dir().join(format!("locus-probe-seccomp-{tag}-{}.json", process::id()));
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
/// Un montage en lecture-écriture d'un répertoire temporaire : la sandbox doit avoir un espace de
/// travail légitime.
///
/// `disk_bytes` est un paramètre parce que les deux tests de ce fichier en font deux usages
/// distincts, et que la différence est le sujet du premier — voir son en-tête.
fn probed_spec(workspace: &str, disk_bytes: u64) -> SandboxSpec {
    SandboxSpec::new(
        LEVEL,
        SandboxProfile::UntrustedRepository,
        NetworkMode::Full,
        vec![Mount::new(workspace, "/work", MountMode::ReadWrite).expect("montage licite")],
        ResourceSpec::new(1_000, 512 << 20, 128, disk_bytes, 300).expect("quotas non nuls"),
    )
    .expect("spécification valide")
}

// ---------------------------------------------------------------------------------------------
// Premier test — cet hôte tient-il `S2` ?
// ---------------------------------------------------------------------------------------------

/// Les seize sondes, dans un conteneur rootless réel, sous une mission qui réserve du disque.
///
/// # Trois états, et ils ne se réparent pas pareil
///
/// « Pas de runtime rootless » veut dire qu'il manque une machine. « Le runtime a refusé cette
/// spécification » veut dire que l'hôte ne sait pas porter l'une des bornes exigées. « Une sonde a
/// échappé » veut dire que le confinement ne tient pas. Les confondre envoie chercher une faille là
/// où il manque un système de fichiers, ou l'inverse.
///
/// Le premier passage de ce test l'a montré immédiatement : le runner GitHub **fait tourner Podman
/// rootless** — l'image s'est construite et la référence par digest s'est résolue — et `podman
/// create` a rendu 125 avec « storage option overlay.size and overlay.inodes only supported for
/// backingFS XFS. Found extfs ». Le quota disque de `ConfinementPlan::disk_bytes` devient un
/// `--storage-opt size=`, que Podman ne sait appliquer que sur XFS.
///
/// D'où la forme de ce test : quand la sandbox ne démarre pas, il rend **le message du runtime**,
/// mot pour mot, plutôt qu'un verdict sur le confinement qu'il n'a pas pu observer.
///
/// # Ce qui est affirmé, et dans quel ordre
///
/// Sonde par sonde, que le verdict est `Holds`. Un `Escaped` est un trou ; un `OverContained` est
/// un backend plus strict qu'annoncé, qui fera échouer des missions légitimes de façon
/// inexplicable ; un `Inconclusive` n'a rien prouvé. Les trois sont des échecs de cet item, et
/// chacun est nommé séparément. Puis le `Standing`, qui est la forme sous laquelle le reste du
/// système lit ce verdict.
#[test]
#[ignore = "exige un hôte capable de S2 : Podman rootless, cgroups v2, quota disque, LOCUS_PROBE_IMAGE"]
fn cet_hote_tient_il_s2_sous_une_mission_qui_reserve_du_disque() {
    let workspace = workspace_dir("plein");
    let spec = probed_spec(&workspace.to_string_lossy(), 1 << 30);

    let results = match probe(&spec, "plein") {
        Ok(results) => results,
        Err(refusal) => {
            let _ = fs::remove_dir_all(&workspace);
            panic!(
                "aucune sonde n'a été exercée : le runtime a refusé la spécification. Ce n'est ni \
                 une absence de runtime, ni un échec de confinement — les trois ne se réparent pas \
                 pareil. Ce que le runtime a dit :\n\n    {refusal}\n\nLe second test de ce \
                 fichier éprouve les quinze sondes qui ne dépendent pas du quota disque."
            );
        }
    };
    let _ = fs::remove_dir_all(&workspace);
    report(&results);

    let failures = verdicts_other_than_holds(&results, &[]);
    assert!(
        failures.is_empty(),
        "{} sonde(s) ne rendent pas à {} le verdict que leur `contained_from` annonce :\n{}",
        failures.len(),
        LEVEL.code(),
        failures.join("\n"),
    );
    assert_eq!(
        locus_execution::standing(LEVEL, &results),
        Standing::Trusted { level: LEVEL },
        "les seize tiennent une à une : le `Standing` doit le dire aussi, sans quoi c'est \
         l'agrégation qui est fausse et non les sondes"
    );
}

// ---------------------------------------------------------------------------------------------
// Second test — les quinze qui ne dépendent pas du quota disque
// ---------------------------------------------------------------------------------------------

/// La même suite sous une mission qui **ne réserve pas de disque**, `exceed_disk_quota` exclue.
///
/// # Pourquoi ce test existe, et ce qu'il n'établit pas
///
/// Sans quota disque, `podman create` n'émet aucun `--storage-opt` et la sandbox démarre sur un
/// hôte que le premier test ne peut pas éprouver. Quinze sondes deviennent alors observables, ce
/// qui est quinze seizièmes de la question de `W5.f` — et l'alternative était de n'en observer
/// aucune.
///
/// **Ce test n'établit jamais `S2`.** `exceed_disk_quota` est `contained_from: S2` : sans quota,
/// elle réussirait, et elle réussirait pour une raison qui ne dit rien du confinement. L'exclure et
/// conclure « `S2` tient » serait exactement la façon de croire une sandbox qu'on n'a pas testée.
/// Aucun `Standing` n'est donc calculé ici — c'est le travail du premier test, sur un hôte qui sait
/// porter le quota.
///
/// L'exclusion est **nommée**, pas silencieuse : le test affirme qu'il écarte une sonde et
/// laquelle. Une exclusion qu'on ne peut pas relire est une suite tronquée, et une suite tronquée
/// se lit comme une suite passée.
#[test]
#[ignore = "exige Podman rootless et LOCUS_PROBE_IMAGE ; n'établit pas S2"]
fn les_quinze_sondes_qui_ne_dependent_pas_du_quota_disque() {
    const EXCLUDED: &str = "exceed_disk_quota";
    assert!(
        SUITE.iter().any(|probe| probe.name == EXCLUDED),
        "la sonde écartée doit exister : l'écarter par un nom mort n'écarterait rien et le test \
         croirait couvrir seize sondes"
    );

    let workspace = workspace_dir("sans-disque");
    let spec = probed_spec(&workspace.to_string_lossy(), 0);

    let results = match probe(&spec, "sans-disque") {
        Ok(results) => results,
        Err(refusal) => {
            let _ = fs::remove_dir_all(&workspace);
            panic!(
                "la sandbox n'a pas démarré alors qu'aucun quota disque n'était demandé — le \
                 refus ne vient donc pas du système de fichiers. Ce que le runtime a dit :\n\n    \
                 {refusal}"
            );
        }
    };
    let _ = fs::remove_dir_all(&workspace);
    report(&results);

    let failures = verdicts_other_than_holds(&results, &[EXCLUDED]);
    assert!(
        failures.is_empty(),
        "{} des quinze sondes hors quota disque ne rendent pas à {} le verdict que leur \
         `contained_from` annonce :\n{}",
        failures.len(),
        LEVEL.code(),
        failures.join("\n"),
    );
}

// ---------------------------------------------------------------------------------------------
// Utilitaires
// ---------------------------------------------------------------------------------------------

/// Un espace de travail temporaire propre à ce test.
fn workspace_dir(tag: &str) -> PathBuf {
    let path = env::temp_dir().join(format!("locus-probe-work-{tag}-{}", process::id()));
    fs::create_dir_all(&path).expect("l'espace de travail doit pouvoir se créer");
    path
}

/// Créer, démarrer, éprouver, arrêter — ou rendre ce que le runtime a répondu.
///
/// L'erreur est une chaîne et non un `Vec` de seize `NotRun` : « le runtime a refusé la
/// spécification » n'est pas seize observations, c'est zéro observation et une raison. Les rendre
/// comme seize `NotRun` produirait une table qui ressemble à un échec de confinement.
fn probe(spec: &SandboxSpec, tag: &str) -> Result<Vec<(&'static str, Observed)>, String> {
    let image = env::var(IMAGE).unwrap_or_else(|_| {
        panic!(
            "{IMAGE} doit porter une image avec son digest (…@sha256:…) — sans image, il n'y a \
             rien à éprouver, et une image sans digest n'est pas reproductible"
        )
    });
    let workload = Workload::new(&image, vec!["sleep".to_owned(), "600".to_owned()])
        .expect("une image à digest et une commande non vide");

    let (profile_path, restricted) = write_restricted_profile(tag);
    let mut backend = PodmanBackend::new(
        SystemRunner,
        SeccompProfiles {
            restricted: Some(restricted),
        },
        workload,
    );

    let outcome = exercise(&mut backend, spec);
    let _ = fs::remove_file(&profile_path);
    outcome
}

fn exercise(
    backend: &mut PodmanBackend<SystemRunner>,
    spec: &SandboxSpec,
) -> Result<Vec<(&'static str, Observed)>, String> {
    let id: SandboxId = backend.create(spec).map_err(|error| error.to_string())?;
    if let Err(error) = backend.start(&id) {
        let _ = backend.stop(&id);
        return Err(error.to_string());
    }
    let results = run_suite(backend, &id);
    let _ = backend.stop(&id);
    Ok(results)
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

/// Les verdicts qui ne sont pas `Holds`, les sondes nommées dans `excluded` mises à part.
fn verdicts_other_than_holds(
    results: &[(&'static str, Observed)],
    excluded: &[&str],
) -> Vec<String> {
    SUITE
        .iter()
        .filter(|probe| !excluded.contains(&probe.name))
        .filter_map(
            |probe| match judge(probe, LEVEL, observation(results, probe.name)) {
                Verdict::Holds => None,
                other => Some(format!("  {other}")),
            },
        )
        .collect()
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
