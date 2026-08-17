//! Test de sortie de W5.e — `docs/SPEC_V1.md` §19.5, ADR 0004.
//!
//! **Le driver construit ce que le blueprint déclare, lit le digest sur la sortie du runtime, et
//! ne sait pas aller plus loin que le deuxième maillon de la chaîne.**
//!
//! La dernière moitié n'a pas de test, et c'est voulu : `BuildDriver::build` rend un `Built`, et la
//! chaîne de W5.b ne mène de `Built` à une image publiée que par le SBOM, le scan, les tests et la
//! signature. Ce n'est pas une discipline à tenir, c'est ce que les types permettent — la garantie
//! est celle du `compile_fail` de `locus_environments::build`.

use std::sync::Mutex;

use locus_environments::{
    EnvironmentBlueprint, Image, Locked, Lockfile, Requirements, ToolchainProfile,
};
use locus_execd::linux::{Execution, Runner};
use locus_execd::{BuildContext, BuildDriver, BuildDriverError, RuntimeError, build_arguments};
use locus_execution::ResourceSpec;

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

const DIGEST: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
/// Ce que le build produit — distinct de ce que le blueprint portait, et c'est le sujet.
const BUILT: &str = "sha256:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

fn blueprint() -> EnvironmentBlueprint {
    EnvironmentBlueprint::new(
        "ml-cpu-v1",
        "1.4.0",
        vec![ToolchainProfile::Base, ToolchainProfile::PythonScience],
        Image::new(DIGEST, None).expect("digest bien formé"),
        Requirements::minimum(
            ResourceSpec::new(4_000, 8 << 30, 512, 20 << 30, 3_600).expect("quotas non nuls"),
        ),
    )
    .expect("blueprint valide")
    .with_variable("PYTHONUNBUFFERED", "1")
    .expect("variable non secrète")
}

fn locked() -> Locked {
    Locked::new(
        blueprint(),
        vec![Lockfile {
            path: "uv.lock".to_owned(),
            hash: DIGEST.to_owned(),
        }],
    )
    .expect("des dépendances verrouillées")
}

fn context() -> BuildContext {
    BuildContext {
        directory: "/srv/environments/ml-cpu".to_owned(),
        containerfile: "Containerfile".to_owned(),
    }
}

/// Un runtime scripté : il enregistre ce qu'on lui demande et rend ce qu'on lui a dit.
struct ScriptedRunner {
    calls: Mutex<Vec<Vec<String>>>,
    code: i32,
    stdout: String,
}

impl ScriptedRunner {
    fn succeeding(stdout: &str) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            code: 0,
            stdout: stdout.to_owned(),
        }
    }

    fn call(&self, index: usize) -> Vec<String> {
        self.calls.lock().expect("verrou")[index].clone()
    }
}

impl Runner for ScriptedRunner {
    fn run(&self, arguments: &[String]) -> Result<Execution, RuntimeError> {
        self.calls.lock().expect("verrou").push(arguments.to_vec());
        Ok(Execution {
            code: self.code,
            stdout: self.stdout.clone(),
            stderr: if self.code == 0 {
                String::new()
            } else {
                "error: unable to pull base image".to_owned()
            },
        })
    }
}

struct AbsentRuntime;

impl Runner for AbsentRuntime {
    fn run(&self, _arguments: &[String]) -> Result<Execution, RuntimeError> {
        Err(RuntimeError::Unavailable {
            detail: "podman : No such file or directory (os error 2)".to_owned(),
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Les arguments disent ce que le blueprint déclare
// ---------------------------------------------------------------------------------------------

#[test]
fn l_identite_et_la_version_sont_inscrites_dans_l_image() {
    let arguments = build_arguments(&blueprint(), &context());
    assert!(
        arguments.contains(&"locus.environment=ml-cpu-v1".to_owned()),
        "une image retrouvée sans son blueprint doit dire de quoi elle est la construction"
    );
    assert!(arguments.contains(&"locus.version=1.4.0".to_owned()));
}

#[test]
fn chaque_profil_devient_un_argument_de_build() {
    let arguments = build_arguments(&blueprint(), &context());
    assert!(arguments.contains(&"LOCUS_TOOLCHAIN_base=1".to_owned()));
    assert!(
        arguments.contains(&"LOCUS_TOOLCHAIN_python_science=1".to_owned()),
        "le tiret d'un nom de profil n'est pas licite dans un nom de variable : {arguments:?}"
    );
}

#[test]
fn les_variables_non_secretes_traversent_le_build() {
    let arguments = build_arguments(&blueprint(), &context());
    assert!(arguments.contains(&"PYTHONUNBUFFERED=1".to_owned()));
}

/// Le digest est ce que le build **produit**. Le passer en entrée reviendrait à demander au build
/// de confirmer ce qu'on savait déjà, c'est-à-dire à n'attester de rien.
#[test]
fn le_digest_du_blueprint_n_est_pas_passe_au_build() {
    let arguments = build_arguments(&blueprint(), &context());
    assert!(
        !arguments.iter().any(|argument| argument.contains(DIGEST)),
        "{arguments:?}"
    );
}

/// §19.5 : le build a le réseau, la mission ne l'a pas. C'est précisément pour cela qu'il est
/// séparé de la mission, et qu'il finit par un scan.
#[test]
fn le_build_a_le_reseau_que_la_mission_n_a_pas() {
    let arguments = build_arguments(&blueprint(), &context());
    assert!(arguments.contains(&"--network=host".to_owned()));
}

#[test]
fn le_contexte_ferme_la_ligne() {
    let arguments = build_arguments(&blueprint(), &context());
    assert_eq!(
        arguments.last().map(String::as_str),
        Some("/srv/environments/ml-cpu"),
        "un argument après le contexte serait lu comme un second contexte"
    );
}

// ---------------------------------------------------------------------------------------------
// Le digest vient du runtime
// ---------------------------------------------------------------------------------------------

#[test]
fn le_digest_est_lu_sur_la_sortie_du_build() {
    let runner = ScriptedRunner::succeeding(&format!("STEP 1/4\nCOMMIT\n{BUILT}\n"));
    let driver = BuildDriver::new(runner);
    let built = driver.build(locked(), &context()).expect("build réussi");

    let published = built
        .inventoried(locus_environments::Sbom {
            components: 12,
            document_hash: DIGEST.to_owned(),
        })
        .expect("inventaire non vide");
    let _ = published;
    assert_eq!(driver.runner().call(0)[0], "build");
}

/// Le digest **construit** n'est pas celui que le blueprint portait : un blueprint décrit une image
/// déjà publiée, un build en produit une nouvelle. Le test emploie donc deux digests distincts, sans
/// quoi il ne saurait pas dire si le driver a lu la sortie ou recopié son entrée.
#[test]
fn le_digest_vient_de_la_sortie_et_non_du_blueprint() {
    let older = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
    let runner = ScriptedRunner::succeeding(&format!("{older}\nSTEP 2/2\n{BUILT}\n"));
    let driver = BuildDriver::new(runner);
    let built = driver.build(locked(), &context()).expect("build réussi");
    // Le digest n'est pas exposé par `Built` ; la chaîne le porte jusqu'à la publication.
    let published = built
        .inventoried(locus_environments::Sbom {
            components: 1,
            document_hash: DIGEST.to_owned(),
        })
        .expect("inventaire non vide")
        .scanned(Vec::new(), locus_environments::Severity::High)
        .expect("aucune vulnérabilité")
        .tested(vec![locus_environments::HealthResult {
            name: "python".to_owned(),
            outcome: locus_environments::HealthOutcome::Passed,
        }])
        .expect("les vérifications passent")
        .published(locus_environments::Signature {
            key_id: "locus-release".to_owned(),
            value: "3045…".to_owned(),
        })
        .expect("signature utilisable");
    assert_eq!(
        published.image().digest(),
        BUILT,
        "une couche intermédiaire ne désigne pas l'image finale, et le blueprint non plus"
    );
    assert_ne!(published.image().digest(), DIGEST);
}

#[test]
fn un_build_muet_n_est_pas_un_build_reussi() {
    let runner = ScriptedRunner::succeeding("STEP 1/4\nCOMMIT\n");
    assert_eq!(
        BuildDriver::new(runner).build(locked(), &context()),
        Err(BuildDriverError::NoDigest),
        "prendre le silence pour l'image attendue serait la même faute qu'ailleurs"
    );
}

#[test]
fn un_code_non_nul_remonte_avec_ce_que_le_runtime_a_dit() {
    let runner = ScriptedRunner {
        calls: Mutex::new(Vec::new()),
        code: 125,
        stdout: String::new(),
    };
    match BuildDriver::new(runner).build(locked(), &context()) {
        Err(BuildDriverError::Runtime { detail }) => {
            assert!(detail.contains("125"), "{detail}");
            assert!(detail.contains("unable to pull"), "{detail}");
        }
        other => panic!("un échec de build doit remonter tel quel : {other:?}"),
    }
}

#[test]
fn un_hote_sans_runtime_le_dit_au_lieu_de_pretendre() {
    assert!(matches!(
        BuildDriver::new(AbsentRuntime).build(locked(), &context()),
        Err(BuildDriverError::Runtime { .. })
    ));
}
