//! Test de sortie de W18.d — **les trois garanties de l'item.**
//!
//! 1. Une capacité nouvelle n'entre que par un `Published` de W5.b, et aucun constructeur ne la
//!    fabrique depuis autre chose.
//! 2. Le refus nomme laquelle des conditions manque, plutôt que de dire « non ».
//! 3. Du code injecté n'est pas une valeur exprimable, et c'est un test d'absence qui le dit.

use locus_adaptation::{Admission, AdmissionError, Extension, admit};
use locus_coordination::{Author, Capability};
use locus_environments::{
    BuildError, EnvironmentBlueprint, HealthOutcome, HealthResult, Image, Locked, Lockfile,
    Published, Requirements, Sbom, Severity, Signature, ToolchainProfile,
};
use locus_execution::ResourceSpec;
use locus_policy::{Outcome, Verb};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

/// Le source sans ses commentaires — ce que le module **fait**, pas ce qu'il explique.
fn code_of(source: &str) -> String {
    source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

const DIGEST: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn blueprint() -> EnvironmentBlueprint {
    EnvironmentBlueprint::new(
        "dh-v1",
        "1.0.0",
        vec![ToolchainProfile::Base, ToolchainProfile::Dh],
        Image::new(DIGEST, None).expect("digest bien formé"),
        Requirements::minimum(
            ResourceSpec::new(4_000, 8 << 30, 512, 20 << 30, 3_600).expect("quotas non nuls"),
        ),
    )
    .expect("blueprint valide")
}

/// Une image publiée qui a démontré ce que `checks` nomme.
fn published(checks: &[&str]) -> Published {
    build(checks).expect("la chaîne de W5.b va au bout")
}

fn build(checks: &[&str]) -> Result<Published, BuildError> {
    Locked::new(
        blueprint(),
        vec![Lockfile {
            path: "uv.lock".to_owned(),
            hash: DIGEST.to_owned(),
        }],
    )?
    .built(DIGEST)
    .inventoried(Sbom {
        components: 412,
        document_hash: DIGEST.to_owned(),
    })?
    .scanned(Vec::new(), Severity::High)?
    .tested(
        checks
            .iter()
            .map(|name| HealthResult {
                name: (*name).to_owned(),
                outcome: HealthOutcome::Passed,
            })
            .collect(),
    )?
    .published(Signature {
        key_id: "locus-release".to_owned(),
        value: "3045…".to_owned(),
    })
}

fn capability(name: &str) -> Capability {
    Capability::new(name).expect("un nom non vide")
}

fn allowed() -> Outcome {
    Outcome::Decided {
        verb: Verb::Allow,
        by: "capability/allow-dh".to_owned(),
    }
}

fn robot() -> Author {
    Author::Human("agent-dh".to_owned())
}

fn steward() -> Author {
    Author::Human("usr-marie".to_owned())
}

// ---------------------------------------------------------------------------------------------
// 1. Une capacité n'entre que par un `Published`
// ---------------------------------------------------------------------------------------------

/// Le chemin nominal : extension gouvernée, `allow`, approbateur distinct, capacité démontrée.
#[test]
fn a_capability_enters_through_a_published_blueprint() {
    let published = published(&["sparql", "alto"]);
    let admission: Admission = admit(
        Extension::Governed,
        &allowed(),
        &robot(),
        &steward(),
        &capability("sparql"),
        &published,
    )
    .expect("les quatre conditions sont réunies");

    assert_eq!(admission.capability(), &capability("sparql"));
    assert_eq!(admission.image_digest(), DIGEST);
    assert_eq!(admission.signing_key(), "locus-release");
    assert_eq!(admission.approved_by(), &steward());
    assert_eq!(admission.rule(), "capability/allow-dh");
}

/// Aucun constructeur ne fabrique une admission depuis autre chose qu'un `Published`.
///
/// Le test lit le **code**, commentaires retirés : expliquer pourquoi un constructeur est absent le
/// fait apparaître dans la prose, et un test qui confondrait les deux interdirait d'écrire la
/// raison.
#[test]
fn nothing_but_admit_produces_an_admission() {
    let source = code_of(include_str!("../src/admission.rs"));
    for forbidden in [
        "Admission::new",
        "fn declaring",
        "impl From<",
        "pub capability:",
        "fn trusting",
    ] {
        assert!(
            !source.contains(forbidden),
            "« {forbidden} » ferait entrer une capacité sans passer par la chaîne de W5.b"
        );
    }
    // Un seul site construit la structure, et il est dans `admit`.
    let constructions = source
        .match_indices("Admission {")
        .filter(|(offset, _)| {
            !source[..*offset].ends_with("pub struct ") && !source[..*offset].ends_with("impl ")
        })
        .count();
    assert_eq!(constructions, 1);
    // Et `admit` exige la preuve des six étapes.
    assert!(source.contains("published: &Published"));
}

/// La chaîne de W5.b ne se saute pas : sans lockfile il n'y a pas de `Published` à présenter.
///
/// Le test n'exerce pas ce module — il exerce ce que ce module **exige**. Que la chaîne refuse est
/// déjà tenu par W5.b ; ce qui est vérifié ici est qu'aucune admission n'existe quand elle refuse,
/// parce qu'il n'y a rien à passer à `admit`.
#[test]
fn without_the_chain_there_is_no_published_to_present() {
    let refused = Locked::new(blueprint(), Vec::new())
        .expect_err("une image sans lockfile ne se reconstruit pas");
    assert_eq!(refused, BuildError::NoLockfile);
}

// ---------------------------------------------------------------------------------------------
// 2. Le refus nomme la condition qui manque
// ---------------------------------------------------------------------------------------------

/// Un déploiement qui n'admet rien le dit, et ne dit que cela.
#[test]
fn a_forbidden_extension_says_so() {
    let refused = admit(
        Extension::Forbidden,
        &allowed(),
        &robot(),
        &steward(),
        &capability("sparql"),
        &published(&["sparql"]),
    )
    .expect_err("`forbidden` est le défaut, et il ferme");
    assert_eq!(refused, AdmissionError::ExtensionForbidden);
    assert_eq!(Extension::default(), Extension::Forbidden);
}

/// Sans `allow`, rien n'entre — et le silence tombe avec le reste.
#[test]
fn nothing_but_allow_opens_the_admission() {
    let published = published(&["sparql"]);
    for outcome in [
        Outcome::NoRule,
        Outcome::Conflict {
            priority: 10,
            rules: vec!["a".to_owned(), "b".to_owned()],
        },
        Outcome::Decided {
            verb: Verb::Deny,
            by: "r".to_owned(),
        },
        Outcome::Decided {
            verb: Verb::RequireApproval {
                approver_role: "pi".to_owned(),
            },
            by: "r".to_owned(),
        },
    ] {
        let refused = admit(
            Extension::Governed,
            &outcome,
            &robot(),
            &steward(),
            &capability("sparql"),
            &published,
        )
        .expect_err("rien d'autre qu'`allow` n'est une autorisation");
        assert_eq!(refused, AdmissionError::PolicyDidNotAllow);
    }
}

/// Le demandeur n'approuve pas sa propre extension de capacité.
///
/// C'est la forme la plus directe du problème de l'agent auto-modifiant : élargir seul ce qu'on a le
/// droit de faire.
#[test]
fn the_requester_does_not_approve_its_own_extension() {
    let refused = admit(
        Extension::Governed,
        &allowed(),
        &robot(),
        &robot(),
        &capability("sparql"),
        &published(&["sparql"]),
    )
    .expect_err("`forbid_self_approval` ne se relâche dans aucun mode");
    assert!(matches!(refused, AdmissionError::SelfApproval { .. }));
    assert!(refused.to_string().contains("forbid_self_approval"));
}

/// **La garantie centrale du refus.** Une capacité qu'aucune vérification ne nomme n'est pas admise,
/// et le refus dit ce que l'image a réellement démontré.
#[test]
fn a_capability_the_image_did_not_demonstrate_is_refused() {
    let refused = admit(
        Extension::Governed,
        &allowed(),
        &robot(),
        &steward(),
        &capability("cuda"),
        &published(&["sparql", "alto"]),
    )
    .expect_err("une image signée n'est pas une image apte");
    assert_eq!(
        refused,
        AdmissionError::NotDemonstrated {
            capability: "cuda".to_owned(),
            checks: vec!["sparql".to_owned(), "alto".to_owned()],
        }
    );
    let said = refused.to_string();
    assert!(said.contains("cuda"), "{said}");
    assert!(said.contains("sparql, alto"), "{said}");
}

/// La comparaison est exacte : un préfixe ne démontre pas ce qui le prolonge.
#[test]
fn a_prefix_does_not_demonstrate_what_extends_it() {
    let published = published(&["sparql"]);
    let refused = admit(
        Extension::Governed,
        &allowed(),
        &robot(),
        &steward(),
        &capability("sparql-write"),
        &published,
    )
    .expect_err("`sparql` ne démontre pas `sparql-write`");
    assert!(matches!(refused, AdmissionError::NotDemonstrated { .. }));

    // Et l'inverse non plus : un nom plus court n'est pas démontré par un plus long.
    let refused = admit(
        Extension::Governed,
        &allowed(),
        &robot(),
        &steward(),
        &capability("spa"),
        &published,
    )
    .expect_err("`sparql` ne démontre pas `spa`");
    assert!(matches!(refused, AdmissionError::NotDemonstrated { .. }));
}

/// Les quatre refus sont distincts, et l'ordre des conditions ne les mélange pas.
///
/// Un refus qui dirait « non » enverrait relire quatre politiques à la main. « L'image ne l'a pas
/// démontrée » n'appelle pas du tout la même suite que « le déploiement n'admet rien ».
#[test]
fn the_four_denials_are_distinct() {
    let published = published(&["sparql"]);
    let denials = [
        admit(
            Extension::Forbidden,
            &allowed(),
            &robot(),
            &steward(),
            &capability("sparql"),
            &published,
        ),
        admit(
            Extension::Governed,
            &Outcome::NoRule,
            &robot(),
            &steward(),
            &capability("sparql"),
            &published,
        ),
        admit(
            Extension::Governed,
            &allowed(),
            &robot(),
            &robot(),
            &capability("sparql"),
            &published,
        ),
        admit(
            Extension::Governed,
            &allowed(),
            &robot(),
            &steward(),
            &capability("cuda"),
            &published,
        ),
    ];
    let said: Vec<String> = denials
        .iter()
        .map(|denial| {
            denial
                .as_ref()
                .expect_err("chacune manque une condition")
                .to_string()
        })
        .collect();
    let distinct: std::collections::BTreeSet<&String> = said.iter().collect();
    assert_eq!(distinct.len(), 4, "quatre refus, quatre phrases : {said:?}");
}

// ---------------------------------------------------------------------------------------------
// 3. Du code injecté n'est pas une valeur exprimable
// ---------------------------------------------------------------------------------------------

/// Ce module ne nomme rien qui puisse porter du code.
///
/// Ce que la littérature appelle « système de plugins » fait circuler du code qu'un processus
/// charge ; une admission fait circuler un digest d'image que `locus-execd` fait tourner sous
/// sandbox. Il n'y a pas de champ à remplir, donc pas de garantie partielle à tenir.
#[test]
fn no_type_here_can_carry_code() {
    let source = code_of(include_str!("../src/admission.rs"));
    for absent in [
        "source_code",
        "script",
        "plugin",
        "eval",
        "dlopen",
        "load_library",
        "dylib",
        "wasm",
        "entrypoint",
        "unsafe",
    ] {
        assert!(
            !source.contains(absent),
            "« {absent} » ferait de l'admission un chargement de code"
        );
    }
}

/// Et ce qu'une admission transporte se limite à ce qu'une mission a besoin de savoir.
///
/// Le `Published` reste chez celui qui l'a construit : le dupliquer ici ferait de l'admission un
/// second dépôt de la preuve, qui divergerait du premier.
#[test]
fn an_admission_carries_a_digest_not_an_environment() {
    let source = code_of(include_str!("../src/admission.rs"));
    let start = source
        .find("pub struct Admission {")
        .expect("la structure existe");
    let end = source[start..].find('}').expect("elle se referme") + start;
    let declaration = &source[start..end];
    for absent in ["Published", "Sbom", "Lockfile", "EnvironmentBlueprint"] {
        assert!(
            !declaration.contains(absent),
            "« {absent} » ferait de l'admission un second dépôt de la preuve"
        );
    }
    assert!(declaration.contains("image_digest: String"));
}
