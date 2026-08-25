//! Test de sortie de `W25.c` — **la fabric d'inférence comme capacité admise.**
//!
//! Trois clauses, celles de la roadmap :
//!
//! 1. elle n'entre que par un `Published` de `W5.b`, comme le raisonneur d'ontologie de `W18.h` ;
//! 2. **aucun crate n'acquiert de dépendance vers un moteur de service**, tenu par
//!    `dependencies.json` et par la garde de frontières ;
//! 3. son absence dégrade la **latence** et **jamais** la correction, et un test l'exerce en la
//!    retirant.
//!
//! La troisième porte l'item, et elle n'est pas tenue par une promesse : elle est tenue par la forme
//! de `Plan`. Ce qui détermine la réponse et ce qui détermine la vitesse sont deux champs séparés, et
//! retirer la fabric ne touche que le second.

use locus_adaptation::{Admission, Extension, Fabric, Request, admit, plan};
use locus_coordination::{Author, Capability};
use locus_environments::{
    BuildError, EnvironmentBlueprint, HealthOutcome, HealthResult, Image, Locked, Lockfile,
    Published, Requirements, Sbom, Severity, Signature, ToolchainProfile,
};
use locus_execution::ResourceSpec;
use locus_policy::{Outcome, Verb};

const DIGEST: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

// ---------------------------------------------------------------------------------------------
// Fixtures — les mêmes que `tests/reasoner.rs`, parce que c'est le même chemin
// ---------------------------------------------------------------------------------------------

fn blueprint(digest: &str) -> EnvironmentBlueprint {
    EnvironmentBlueprint::new(
        "dh-v1",
        "1.0.0",
        vec![ToolchainProfile::Base, ToolchainProfile::Dh],
        Image::new(digest, None).expect("digest bien formé"),
        Requirements::minimum(
            ResourceSpec::new(4_000, 8 << 30, 512, 20 << 30, 3_600).expect("quotas non nuls"),
        ),
    )
    .expect("blueprint valide")
}

fn build(digest: &str, checks: &[&str]) -> Result<Published, BuildError> {
    Locked::new(
        blueprint(digest),
        vec![Lockfile {
            path: "uv.lock".to_owned(),
            hash: digest.to_owned(),
        }],
    )?
    .built(digest)
    .inventoried(Sbom {
        components: 412,
        document_hash: digest.to_owned(),
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

fn admise(digest: &str, nom: &str) -> Admission {
    admit(
        Extension::Governed,
        &Outcome::Decided {
            verb: Verb::Allow,
            by: "capability/allow-fabric".to_owned(),
        },
        &Author::Human("agent-dh".to_owned()),
        &Author::Human("usr-marie".to_owned()),
        &Capability::new(nom).expect("un nom non vide"),
        &build(digest, &[nom]).expect("la chaîne de W5.b va au bout"),
    )
    .expect("les quatre conditions sont réunies")
}

fn requete() -> Request {
    Request::asking(
        "tu es un assistant de recherche rigoureux",
        "le catalyseur A tient-il au-delà de 300 °C ?",
    )
}

// ---------------------------------------------------------------------------------------------
// 1. Elle n'entre que par un `Published`
// ---------------------------------------------------------------------------------------------

/// **Aucun constructeur ne fabrique une fabric autrement.**
///
/// `Fabric::admitted` prend une `Admission`, et une `Admission` n'existe que par `admit`, qui exige
/// un `Published` de `W5.b`. Le chemin de gouvernance est donc le seul, **par signature et non par
/// discipline** — c'est ce que `W18.h` a établi pour le raisonneur, et ce module ne le rejoue pas
/// autrement.
#[test]
fn une_fabric_n_entre_que_par_une_admission() {
    let fabric = Fabric::admitted(admise(DIGEST, "prefix-cache"));
    assert_eq!(fabric.image_digest(), DIGEST);
    assert_eq!(fabric.admission().capability().to_string(), "prefix-cache");
}

/// La fabric est désignée par **digest**, jamais par nom.
///
/// `W18.h` en a écrit la raison, et elle vaut ici : une substitution de capacité par nom ne produit
/// pas d'erreur, elle produit des réponses plausibles fondées sur autre chose. Deux fabrics de même
/// nom et de digests différents sont deux capacités.
#[test]
fn la_fabric_est_designee_par_digest_jamais_par_nom() {
    const AUTRE: &str = "sha256:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
    let une = Fabric::admitted(admise(DIGEST, "prefix-cache"));
    let autre = Fabric::admitted(admise(AUTRE, "prefix-cache"));

    assert_eq!(
        une.admission().capability().to_string(),
        autre.admission().capability().to_string(),
        "même nom"
    );
    assert_ne!(une.image_digest(), autre.image_digest());
    assert_ne!(une, autre, "et donc deux capacités");
}

/// Le module ne construit **aucun** moteur, et n'en nomme aucun.
///
/// Le pendant, dans la source, de la règle 8 des frontières : « aucun crate n'acquiert de dépendance
/// vers un moteur de service d'inférence ». La garde le tient au niveau des manifestes et des
/// imports ; ce test le tient au niveau du vocabulaire, parce qu'un moteur nommé en dur serait le
/// premier pas vers la dépendance.
#[test]
fn le_module_ne_nomme_aucun_moteur_de_service() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/fabric.rs"))
        .expect("le module de production est lisible depuis son propre crate");
    let code: String = source
        .lines()
        .filter(|ligne| !ligne.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    for interdit in [
        "vllm",
        "sglang",
        "tensorrt",
        "ollama",
        "triton",
        "llama_cpp",
    ] {
        assert!(
            !code.contains(interdit),
            "« {interdit} » : le dépôt ordonnance une fabric, il n'en embarque aucune"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 3. L'absence dégrade la latence, jamais la correction
// ---------------------------------------------------------------------------------------------

/// **La clause qui porte l'item.**
///
/// Le même plan, avec et sans fabric : ce qui détermine la réponse est **identique**, et le seul
/// écart est l'accélération. Ce n'est pas une comparaison de deux exécutions — le dépôt n'exécute
/// aucun moteur — mais de ce qui part à l'exécution, qui est là où la correction se joue.
#[test]
fn retirer_la_fabric_ne_change_que_l_acceleration() {
    let fabric = Fabric::admitted(admise(DIGEST, "prefix-cache"));

    let avec = plan(requete(), Some(&fabric));
    let sans = plan(requete(), None);

    assert_eq!(
        avec.request(),
        sans.request(),
        "ce qui détermine la réponse est le même"
    );
    assert!(avec.acceleration().is_some());
    assert!(sans.acceleration().is_none());
    assert_ne!(avec, sans, "et les deux plans se distinguent quand même");

    // Le plan accéléré, dépouillé de son accélération, **est** le plan non accéléré. C'est la forme
    // la plus courte de la clause : tout ce qui reste après avoir retiré la vitesse est ce qui
    // détermine le résultat.
    assert_eq!(avec.without_acceleration(), sans);
}

/// **L'accélération ne porte aucun champ dont dépendrait un résultat.**
///
/// Tenu par l'absence dans la source. Une accélération qui porterait un modèle, un gabarit, une
/// température ou une graine ne serait plus une accélération : ce serait un second chemin de
/// décision, et son absence changerait la réponse au lieu de la retarder.
///
/// C'est le même idiome que `W24.c` — et la même prudence : le scan retire d'abord les commentaires,
/// parce qu'une recherche par sous-chaîne se restreint à ce qu'elle doit lire.
#[test]
fn l_acceleration_ne_porte_rien_qui_determine_un_resultat() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/fabric.rs"))
        .expect("le module de production est lisible depuis son propre crate");
    let debut = source
        .find("pub struct Acceleration {")
        .expect("la structure existe");
    let fin = source[debut..].find("\n}").expect("elle se ferme") + debut;
    let champs: String = source[debut..fin]
        .lines()
        .filter(|ligne| !ligne.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    for interdit in [
        "model",
        "prompt",
        "template",
        "temperature",
        "seed",
        "output",
        "response",
        "answer",
    ] {
        assert!(
            !champs.contains(interdit),
            "« {interdit} » dans `Acceleration` ferait de son absence un changement de réponse"
        );
    }
}

/// Réutiliser un **préfixe** n'est pas réutiliser une **réponse**.
///
/// C'est là qu'un cache devient faux. L'accélération compte des jetons de préfixe déjà calculés —
/// une propriété de la requête présente — et ne porte aucune réponse d'une requête passée. Un cache
/// de résultats ferait dépendre ce qu'on rend de ce qu'on a rendu avant, et l'absence de fabric
/// changerait alors les conclusions.
///
/// Le test le montre par ce qui varie : deux requêtes de **même préfixe** et de suffixes différents
/// obtiennent la même réutilisation, ce qui est exactement ce qu'un cache de préfixe fait — et ce
/// qu'un cache de réponses ne ferait jamais.
#[test]
fn l_acceleration_porte_un_prefixe_et_pas_une_reponse() {
    let fabric = Fabric::admitted(admise(DIGEST, "prefix-cache"));
    let commune = "tu es un assistant de recherche rigoureux";

    let une = fabric.accelerating(&Request::asking(commune, "première question ?"));
    let autre = fabric.accelerating(&Request::asking(commune, "une tout autre question ?"));

    assert_eq!(
        une.reusable_prefix_tokens(),
        autre.reusable_prefix_tokens(),
        "le préfixe partagé se réutilise, quelle que soit la question"
    );
    assert!(une.reusable_prefix_tokens() > 0);
}

/// Un préfixe vide ne rend **rien** à réutiliser, et un suffixe vide ne désagrège pas.
///
/// Les deux bouts : une accélération qui annoncerait un gain sur une requête qui n'en offre aucun
/// serait une promesse, et `CLAUDE.md` en refuse le principe.
#[test]
fn ce_qui_n_offre_rien_a_accelerer_n_accelere_rien() {
    let fabric = Fabric::admitted(admise(DIGEST, "prefix-cache"));

    let sans_prefixe = fabric.accelerating(&Request::asking("", "une question"));
    assert_eq!(sans_prefixe.reusable_prefix_tokens(), 0);

    let sans_suffixe = fabric.accelerating(&Request::asking("un préfixe", ""));
    assert!(
        !sans_suffixe.disaggregated(),
        "rien à décoder : il n'y a pas deux phases à séparer"
    );
}

/// L'absence de fabric est le **fonctionnement nominal**, pas un cas d'erreur.
///
/// `plan` prend un `Option<&Fabric>`, donc un déploiement qui n'a admis aucune fabric ne gère aucune
/// erreur : il planifie. C'est ce que « capacité admise » veut dire — quelque chose qu'on peut ne
/// pas avoir.
#[test]
fn l_absence_de_fabric_est_le_fonctionnement_nominal() {
    let sans = plan(requete(), None);
    assert_eq!(sans.request(), &requete());
    assert!(sans.acceleration().is_none());
    assert_eq!(
        sans.without_acceleration(),
        sans,
        "retirer ce qui n'est pas là ne change rien"
    );
}
