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
//! # Quatre tests, et aucun ne remplace un autre
//!
//! Le premier demande si cet hôte tient `S2` sous une mission qui réserve du disque. Le deuxième
//! éprouve les **quinze** sondes qui ne dépendent pas du quota disque, et **n'établit jamais
//! `S2`** — l'exclusion y est nommée plutôt que silencieuse. Le troisième regarde le réseau depuis
//! l'intérieur, et il a servi une fois à réfuter celui qui l'avait écrit. Le quatrième constate que
//! le retrait rend le nom, en le redemandant.
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
    MUST_DENY, PodmanBackend, RestrictedProfile, Runner, SANDBOX_REFUSED, SeccompProfiles,
    SystemRunner, Trial, Workload, exec_arguments, run_suite, verdicts,
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
        locus_execution::standing(LEVEL, &verdicts(&results)),
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
fn probe(spec: &SandboxSpec, tag: &str) -> Result<Vec<Trial>, String> {
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
        SystemRunner::new(),
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
) -> Result<Vec<Trial>, String> {
    let results = run_suite(backend, spec);
    refused_throughout(&results).map_or(Ok(results), Err)
}

/// Le message du runtime quand **aucune** des seize sandboxes n'a pu être ouverte.
///
/// `W5.r` fait ouvrir une sandbox par sonde : un refus de la spécification ne s'échappe donc plus
/// par une erreur, il se rapporte seize fois. La distinction que ce fichier tenait reste vraie et
/// devient plus précise — **toutes** refusées veut dire zéro observation et une raison, ce qui
/// n'appelle pas la même lecture qu'une table ; **certaines** refusées est une observation, neuve,
/// et elle a sa place dans la table.
fn refused_throughout(results: &[Trial]) -> Option<String> {
    let refused: Vec<&Trial> = results
        .iter()
        .filter(|trial| {
            trial.observed()
                == Observed::NotRun {
                    reason: SANDBOX_REFUSED,
                }
        })
        .collect();
    (refused.len() == results.len())
        .then(|| {
            refused
                .iter()
                .find_map(|trial| trial.detail())
                .map(str::to_owned)
        })
        .flatten()
}

/// Arrêter **et retirer** la sandbox, par le port.
///
/// `W5.l` a mis le retrait au port, sous son nom. La version précédente de cette fonction passait
/// par le runner parce que le port n'avait pas l'opération — une dette assumée qui n'existe plus.
///
/// `run_suite` ne passe plus par ici depuis `W5.r` — il démonte lui-même la sandbox de chaque sonde.
/// Ce qui reste sont les deux tests qui ouvrent un conteneur pour leur propre compte : celui qui
/// regarde le réseau depuis l'intérieur, et celui qui vérifie qu'un nom est rendu.
fn teardown(backend: &mut PodmanBackend<SystemRunner>, id: &SandboxId) {
    let _ = backend.stop(id);
    let _ = backend.remove(id);
}

/// Ce qu'une sonde a produit, ou l'aveu qu'elle est absente du rapport.
fn observation(results: &[Trial], name: &str) -> Observed {
    results.iter().find(|trial| trial.name() == name).map_or(
        Observed::NotRun {
            reason: "sonde absente du rapport",
        },
        Trial::observed,
    )
}

/// Ce que le runtime a écrit en refusant, ou rien.
///
/// `W5.q` : c'était la dernière chose que le harnais jetait. Trois hypothèses sont tombées sur les
/// sondes qui rendent 255 — cgroup transitoire, sandbox morte, contamination — et la seule chose
/// qu'on n'avait pas lue est ce que le runtime **dit** en refusant.
fn detail_of(results: &[Trial], name: &str) -> String {
    results
        .iter()
        .find(|trial| trial.name() == name)
        .and_then(Trial::detail)
        .map_or_else(String::new, indented)
}

/// Le détail, en retrait sous sa ligne — **chaque** ligne du détail.
///
/// Le premier passage réel a montré la faute que la relecture n'avait pas vue : un `\n` en tête et
/// aucun en queue, donc une ligne vide avant le détail et la ligne suivante du tableau collée
/// derrière lui. Le tableau devenait illisible exactement là où il devient utile.
///
/// Le retrait vaut pour toutes les lignes, pas seulement la première : un runtime écrit parfois
/// plusieurs lignes, et une suite désindentée se lirait comme des colonnes du tableau.
fn indented(detail: &str) -> String {
    use std::fmt::Write as _;

    detail.lines().fold(String::new(), |mut rendered, line| {
        let _ = writeln!(rendered, "      ↳ {line}");
        rendered
    })
}

/// Le rendu du détail n'a pas besoin d'un hôte : il se vérifie ici, et il tourne partout.
///
/// Les quatre tests de ce fichier sont `#[ignore]` parce qu'ils exigent un runtime. Celui-ci ne
/// l'est pas — sinon la faute que le premier passage a rendue visible aurait attendu le passage
/// suivant pour l'être une seconde fois.
#[test]
fn le_detail_est_en_retrait_et_ferme_sa_ligne() {
    assert_eq!(indented("conmon: rien"), "      ↳ conmon: rien\n");
    assert_eq!(
        indented("premiere\nseconde"),
        "      ↳ premiere\n      ↳ seconde\n",
        "une suite désindentée se lirait comme des colonnes"
    );
    assert_eq!(indented(""), "", "rien à dire ne prend pas de ligne");
}

/// Le code brut que la commande a rendu, quand il y en a eu un.
///
/// `—` et non `0` pour l'absence : un runtime qui n'a pas répondu n'a pas de code, et afficher un
/// zéro y ferait lire un succès.
fn code_of(results: &[Trial], name: &str) -> String {
    results
        .iter()
        .find(|trial| trial.name() == name)
        .and_then(Trial::code)
        .map_or_else(|| "—".to_owned(), |code| code.to_string())
}

/// Les verdicts qui ne sont pas `Holds`, les sondes nommées dans `excluded` mises à part.
fn verdicts_other_than_holds(results: &[Trial], excluded: &[&str]) -> Vec<String> {
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
fn report(results: &[Trial]) {
    println!("\nsondes à {} — hôte réel\n", LEVEL.code());
    for probe in &SUITE {
        let observed = observation(results, probe.name);
        println!(
            "  {:<32} attendu {:<10} code {:>4}  observé {:<28} → {}",
            probe.name,
            match expectation(probe, LEVEL) {
                Expectation::Contained => "contenue",
                Expectation::Allowed => "permise",
            },
            code_of(results, probe.name),
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
        print!("{}", detail_of(results, probe.name));
    }
    println!();
}

// ---------------------------------------------------------------------------------------------
// W5.k — le réseau déclaré, et celui que la sandbox obtient
// ---------------------------------------------------------------------------------------------

/// **La sandbox du plan présente-t-elle le réseau que la mission a déclaré ?**
///
/// # Ce que le premier hôte réel a montré, et qui n'est pas un défaut de sonde
///
/// À `S2` avec `NetworkMode::Full`, `plan` rend `NetworkPosture::Host` et `create_arguments` émet
/// bien `--network=host` — vérifié hors conteneur, sur les arguments eux-mêmes. Pourtant les deux
/// sondes réseau ressortent **bloquées**, c'est-à-dire que leur constat de route ne trouve aucune
/// route par défaut. Or sur le même hôte, un `podman run --network=host` nu voit la route, résout
/// les noms, et rend `200` sur `example.org`.
///
/// Le constat de route n'est pas en cause : rejoué hors ligne sur la sortie réelle de
/// `/proc/net/route`, il trouve la route ; sur un namespace vide, il ne la trouve pas. Ce qui reste
/// est que **la sandbox n'obtient pas le réseau que la mission a déclaré**, silencieusement.
///
/// C'est le miroir exact du quota disque de `W5.g` : là une borne déclarée n'était pas applicable,
/// ici une permission déclarée n'est pas accordée. Les deux se voient de la même façon — en
/// regardant depuis l'intérieur — et pas autrement.
///
/// # Ce que ce test affirme
///
/// Que la posture déclarée et ce que la sandbox présente coïncident. Il imprime d'abord ce qu'elle
/// voit, parce qu'un échec doit dire **quoi** manque et non seulement que quelque chose manque : la
/// suite du travail est de trouver lequel des drapeaux du plan produit cet écart, et cela se
/// bissecte sur une table, pas sur un verdict.
#[test]
#[ignore = "exige Podman rootless et LOCUS_PROBE_IMAGE"]
fn la_sandbox_presente_le_reseau_que_la_mission_declare() {
    let workspace = workspace_dir("reseau");
    let spec = probed_spec(&workspace.to_string_lossy(), 0);
    let seen = match inspect_network(&spec) {
        Ok(seen) => seen,
        Err(refusal) => {
            let _ = fs::remove_dir_all(&workspace);
            panic!(
                "aucune sandbox n'a été créée : il n'y a donc **rien** à dire de son réseau. \
                 Conclure ici que la route manque serait présenter une absence d'observation comme \
                 une observation — la faute même que cet item reproche aux sondes. Ce que le \
                 runtime a dit :\n\n    {refusal}"
            );
        }
    };
    let _ = fs::remove_dir_all(&workspace);

    println!("\nce que la sandbox du plan voit du réseau\n\n{seen}\n");

    assert!(
        seen.lines().skip(1).any(|line| {
            let mut fields = line.split_whitespace();
            fields.next().is_some() && fields.next() == Some("00000000")
        }),
        "la mission déclare `NetworkMode::Full`, le plan rend `NetworkPosture::Host` et les \
         arguments portent `--network=host` — la sandbox devrait donc voir la route par défaut de \
         l'hôte. Elle ne la voit pas, et c'est une permission déclarée qui n'est pas accordée."
    );
}

/// Ce que la sandbox voit de `/proc/net/route`, lu depuis l'intérieur — ou pourquoi il n'y a rien.
///
/// # Un `Result`, et pas une chaîne qui contiendrait l'erreur
///
/// Le premier passage de ce test a rendu, à la place de la table de routage, le message
/// « le nom de conteneur `locus-0001` est déjà utilisé » — les trois tests du fichier tournent dans
/// le même processus et chacun construit son propre `PodmanBackend`, dont le compteur repart à zéro.
/// Le test a alors **affirmé que la sandbox ne voyait pas la route**, alors qu'aucune sandbox
/// n'existait.
///
/// C'est mot pour mot la faute que cet item reproche aux sondes : présenter une absence
/// d'observation comme une observation. Le type l'empêche désormais.
fn inspect_network(spec: &SandboxSpec) -> Result<String, String> {
    let image = env::var(IMAGE).expect("LOCUS_PROBE_IMAGE");
    let workload = Workload::new(&image, vec!["sleep".to_owned(), "600".to_owned()])
        .expect("une image à digest et une commande non vide");
    let (profile_path, restricted) = write_restricted_profile("reseau");
    let mut backend = PodmanBackend::new(
        SystemRunner::new(),
        SeccompProfiles {
            restricted: Some(restricted),
        },
        workload,
    );

    let seen = match backend.create(spec) {
        Ok(id) => {
            let read = match backend.start(&id) {
                Ok(()) => backend
                    .runner()
                    .run(&exec_arguments(
                        &id,
                        &["sh", "-c", "cat /proc/net/route 2>&1"],
                    ))
                    .map(|execution| execution.stdout)
                    .map_err(|error| error.to_string()),
                Err(error) => Err(error.to_string()),
            };
            teardown(&mut backend, &id);
            read
        }
        Err(error) => Err(error.to_string()),
    };
    let _ = fs::remove_file(&profile_path);
    seen
}

// ---------------------------------------------------------------------------------------------
// W5.l — arrêter n'est pas retirer
// ---------------------------------------------------------------------------------------------

/// **Le nom est libre après le retrait**, constaté en le redemandant.
///
/// # Pourquoi c'est ce test-là et pas un autre
///
/// « Le conteneur n'existe plus » ne se vérifie pas en demandant au backend qui vient de l'oublier :
/// il répondrait ce qu'il a noté, pas ce que l'hôte tient. Ce qui se vérifie est ce qui manquait au
/// suivant — **le nom**. Deux backends successifs, chacun avec son compteur repartant de zéro,
/// demandent donc le même `locus-0001`, et le second doit l'obtenir.
///
/// C'est très exactement le scénario qui a rendu trois passages de CI illisibles : le second
/// conteneur échouait avec « the container name `locus-0001` is already in use », et le harnais
/// lisait cette erreur là où il attendait un verdict de confinement.
///
/// La couche inscriptible s'en va avec le nom — `podman rm` retire les deux — et il n'y a pas de
/// façon de constater l'une sans l'autre depuis ici. Le test affirme donc ce qu'il peut affirmer, et
/// pas davantage.
#[test]
#[ignore = "exige Podman rootless et LOCUS_PROBE_IMAGE"]
fn le_nom_est_libre_apres_le_retrait() {
    let workspace = workspace_dir("retrait");
    let spec = probed_spec(&workspace.to_string_lossy(), 0);

    let first = claim_name(&spec, "retrait-un");
    let second = claim_name(&spec, "retrait-deux");
    let _ = fs::remove_dir_all(&workspace);

    let taken = first.expect("le premier conteneur doit se créer");
    let reused = second.unwrap_or_else(|error| {
        panic!(
            "le second backend redemande « {taken} » et ne l'obtient pas : le retrait n'a donc pas \
             rendu le nom. C'est ce qui faisait lire, à la place d'un verdict de confinement, une \
             erreur de nom. Ce que le runtime a dit :\n\n    {error}"
        )
    });
    assert_eq!(
        taken, reused,
        "les deux backends repartent du même compteur : sans cela le test ne redemanderait pas le \
         même nom, et ne prouverait rien"
    );
}

/// Créer une sandbox, en rendre le nom, puis la retirer — ou dire pourquoi elle n'a pas été créée.
fn claim_name(spec: &SandboxSpec, tag: &str) -> Result<String, String> {
    let image = env::var(IMAGE).expect("LOCUS_PROBE_IMAGE");
    let workload = Workload::new(&image, vec!["sleep".to_owned(), "600".to_owned()])
        .expect("une image à digest et une commande non vide");
    let (profile_path, restricted) = write_restricted_profile(tag);
    let mut backend = PodmanBackend::new(
        SystemRunner::new(),
        SeccompProfiles {
            restricted: Some(restricted),
        },
        workload,
    );
    let claimed = match backend.create(spec) {
        Ok(id) => {
            let name = id.as_str().to_owned();
            teardown(&mut backend, &id);
            Ok(name)
        }
        Err(error) => Err(error.to_string()),
    };
    let _ = fs::remove_file(&profile_path);
    claimed
}
