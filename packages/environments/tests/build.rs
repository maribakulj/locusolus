//! Test de sortie de W5.b — `docs/SPEC_V1.md` §19.5, §19.7.
//!
//! **Aucun chemin ne signe une image non scannée, ne publie un digest dont les tests n'ont pas
//! tourné, ni ne tolère une vérification qu'on n'a pas su lancer.**
//!
//! §19.5 énumère la suite — « lockfile, SBOM, scan, tests, signature et publication par digest ».
//! Une suite écrite en prose se saute : il suffit d'appeler la dernière fonction. Ici chaque étape
//! consomme la preuve de la précédente, si bien que l'ordre n'est pas une consigne mais la seule
//! façon de composer les types. Ce que les tests vérifient est donc surtout **ce que chaque étape
//! refuse** — la composition, elle, est vérifiée par le compilateur.

use locus_environments::{
    BuildError, EnvironmentBlueprint, Finding, HealthOutcome, HealthResult, Image, Locked,
    Lockfile, Requirements, Sbom, Severity, Signature, ToolchainProfile,
};
use locus_execution::ResourceSpec;

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

const DIGEST: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn blueprint() -> EnvironmentBlueprint {
    EnvironmentBlueprint::new(
        "ml-cpu-v1",
        "1.0.0",
        vec![ToolchainProfile::Base, ToolchainProfile::MlCpu],
        Image::new(DIGEST, None).expect("digest bien formé"),
        Requirements::minimum(
            ResourceSpec::new(4_000, 8 << 30, 512, 20 << 30, 3_600).expect("quotas non nuls"),
        ),
    )
    .expect("blueprint valide")
}

fn lockfiles() -> Vec<Lockfile> {
    vec![Lockfile {
        path: "uv.lock".to_owned(),
        hash: DIGEST.to_owned(),
    }]
}

fn sbom() -> Sbom {
    Sbom {
        components: 412,
        document_hash: DIGEST.to_owned(),
    }
}

fn healthy() -> Vec<HealthResult> {
    vec![HealthResult {
        name: "torch".to_owned(),
        outcome: HealthOutcome::Passed,
    }]
}

fn signature() -> Signature {
    Signature {
        key_id: "locus-release".to_owned(),
        value: "3045…".to_owned(),
    }
}

fn locked() -> Locked {
    Locked::new(blueprint(), lockfiles()).expect("des dépendances verrouillées")
}

// ---------------------------------------------------------------------------------------------
// La chaîne complète
// ---------------------------------------------------------------------------------------------

#[test]
fn une_chaine_complete_publie_une_image_et_porte_ses_preuves() {
    let published = locked()
        .built(DIGEST)
        .inventoried(sbom())
        .expect("inventaire non vide")
        .scanned(Vec::new(), Severity::High)
        .expect("aucune vulnérabilité")
        .tested(healthy())
        .expect("les vérifications passent")
        .published(signature())
        .expect("signature utilisable");

    assert_eq!(published.image().digest(), DIGEST);
    assert_eq!(published.blueprint().environment_id(), "ml-cpu-v1");
    assert_eq!(published.sbom().components, 412);
    assert_eq!(published.lockfiles().len(), 1);
    assert_eq!(published.health().len(), 1);
    assert_eq!(published.signature().key_id, "locus-release");
}

/// Sous le plafond ne veut pas dire aucune : les trouvailles tolérées restent attachées, parce que
/// publier une image avec des vulnérabilités connues sans les porter reviendrait à les oublier.
#[test]
fn les_trouvailles_tolerees_restent_attachees() {
    let tolerated = Finding {
        id: "CVE-2026-0001".to_owned(),
        severity: Severity::Medium,
        component: "libxml2".to_owned(),
    };
    let published = locked()
        .built(DIGEST)
        .inventoried(sbom())
        .expect("inventaire non vide")
        .scanned(vec![tolerated.clone()], Severity::High)
        .expect("sous le plafond")
        .tested(healthy())
        .expect("les vérifications passent")
        .published(signature())
        .expect("signature utilisable");
    assert_eq!(published.findings_tolerated(), [tolerated]);
}

// ---------------------------------------------------------------------------------------------
// Ce que chaque étape refuse
// ---------------------------------------------------------------------------------------------

#[test]
fn sans_lockfile_la_chaine_ne_demarre_pas() {
    assert_eq!(
        Locked::new(blueprint(), Vec::new()),
        Err(BuildError::NoLockfile),
        "§19.7 fait de R2 « l'environnement verrouillé » ; sans lockfile on le promettrait sans le tenir"
    );
}

#[test]
fn un_sbom_vide_n_est_pas_un_inventaire() {
    let empty = Sbom {
        components: 0,
        document_hash: DIGEST.to_owned(),
    };
    assert_eq!(
        locked().built(DIGEST).inventoried(empty),
        Err(BuildError::EmptyInventory)
    );

    let unhashed = Sbom {
        components: 12,
        document_hash: "  ".to_owned(),
    };
    assert_eq!(
        locked().built(DIGEST).inventoried(unhashed),
        Err(BuildError::EmptyInventory),
        "un inventaire qu'on ne peut pas désigner ne prouve pas ce qu'il liste"
    );
}

/// « Scanné » ne veut pas dire « propre ». Un scan qui rend des vulnérabilités et laisse passer
/// l'image donne à la chaîne l'apparence d'un contrôle sans le contrôle.
#[test]
fn une_vulnerabilite_au_plafond_arrete_la_chaine_et_se_nomme() {
    let critical = Finding {
        id: "CVE-2026-9999".to_owned(),
        severity: Severity::Critical,
        component: "openssl".to_owned(),
    };
    let refused = locked()
        .built(DIGEST)
        .inventoried(sbom())
        .expect("inventaire non vide")
        .scanned(vec![critical], Severity::High);

    match refused {
        Err(BuildError::VulnerabilityAboveCeiling {
            id,
            component,
            severity,
            ceiling,
        }) => {
            assert_eq!(id, "CVE-2026-9999");
            assert_eq!(component, "openssl");
            assert_eq!(severity, Severity::Critical);
            assert_eq!(ceiling, Severity::High);
        }
        other => panic!("une critique au-dessus du plafond doit arrêter la chaîne : {other:?}"),
    }
}

#[test]
fn le_plafond_est_atteint_et_pas_seulement_depasse() {
    let at_ceiling = Finding {
        id: "CVE-2026-0002".to_owned(),
        severity: Severity::High,
        component: "zlib".to_owned(),
    };
    assert!(
        locked()
            .built(DIGEST)
            .inventoried(sbom())
            .expect("inventaire non vide")
            .scanned(vec![at_ceiling], Severity::High)
            .is_err(),
        "un plafond « High » tolère jusqu'à Medium : sinon il ne plafonne rien"
    );
}

#[test]
fn le_refus_nomme_la_pire_trouvaille_et_pas_la_premiere() {
    let findings = vec![
        Finding {
            id: "CVE-2026-0003".to_owned(),
            severity: Severity::High,
            component: "zlib".to_owned(),
        },
        Finding {
            id: "CVE-2026-9999".to_owned(),
            severity: Severity::Critical,
            component: "openssl".to_owned(),
        },
    ];
    let refused = locked()
        .built(DIGEST)
        .inventoried(sbom())
        .expect("inventaire non vide")
        .scanned(findings, Severity::High);
    assert!(
        matches!(
            refused,
            Err(BuildError::VulnerabilityAboveCeiling { ref id, .. }) if id == "CVE-2026-9999"
        ),
        "corriger la première laisserait la pire : {refused:?}"
    );
}

#[test]
fn une_image_sans_verification_ne_prouve_pas_qu_elle_est_utilisable() {
    assert_eq!(
        locked()
            .built(DIGEST)
            .inventoried(sbom())
            .expect("inventaire non vide")
            .scanned(Vec::new(), Severity::High)
            .expect("aucune vulnérabilité")
            .tested(Vec::new()),
        Err(BuildError::NoHealthCheck)
    );
}

#[test]
fn une_verification_echouee_arrete_la_chaine() {
    let failed = vec![HealthResult {
        name: "torch".to_owned(),
        outcome: HealthOutcome::Failed {
            detail: "ModuleNotFoundError: torch".to_owned(),
        },
    }];
    assert!(matches!(
        locked()
            .built(DIGEST)
            .inventoried(sbom())
            .expect("inventaire non vide")
            .scanned(Vec::new(), Severity::High)
            .expect("aucune vulnérabilité")
            .tested(failed),
        Err(BuildError::HealthCheckFailed { .. })
    ));
}

/// Le refus qui compte. Une vérification qu'on n'a pas su lancer n'a rien prouvé, et la compter
/// comme un succès ferait d'un outil manquant une preuve de santé — c'est le même refus que
/// `Observed::NotRun` de la suite de sandbox, et il porte son propre nom pour la même raison :
/// « la commande a échoué » et « je n'ai pas su la lancer » envoient chercher à deux endroits.
#[test]
fn une_verification_non_lancee_est_distincte_d_un_echec() {
    let not_run = vec![HealthResult {
        name: "torch".to_owned(),
        outcome: HealthOutcome::NotRun {
            reason: "le runtime n'a pas répondu".to_owned(),
        },
    }];
    match locked()
        .built(DIGEST)
        .inventoried(sbom())
        .expect("inventaire non vide")
        .scanned(Vec::new(), Severity::High)
        .expect("aucune vulnérabilité")
        .tested(not_run)
    {
        Err(BuildError::HealthCheckNotRun { name, reason }) => {
            assert_eq!(name, "torch");
            assert!(reason.contains("runtime"));
        }
        other => panic!("l'absence de preuve n'est pas une preuve : {other:?}"),
    }
}

#[test]
fn une_publication_sans_signature_utilisable_est_refusee() {
    for broken in [
        Signature {
            key_id: String::new(),
            value: "3045…".to_owned(),
        },
        Signature {
            key_id: "locus-release".to_owned(),
            value: "   ".to_owned(),
        },
    ] {
        assert_eq!(
            locked()
                .built(DIGEST)
                .inventoried(sbom())
                .expect("inventaire non vide")
                .scanned(Vec::new(), Severity::High)
                .expect("aucune vulnérabilité")
                .tested(healthy())
                .expect("les vérifications passent")
                .published(broken),
            Err(BuildError::UnsignedPublication)
        );
    }
}

#[test]
fn un_digest_de_build_mal_forme_ne_devient_pas_une_image() {
    let refused = locked()
        .built("ghcr.io/locus/ml-cpu:latest")
        .inventoried(sbom())
        .expect("inventaire non vide")
        .scanned(Vec::new(), Severity::High)
        .expect("aucune vulnérabilité")
        .tested(healthy())
        .expect("les vérifications passent")
        .published(signature());
    assert!(
        matches!(refused, Err(BuildError::Blueprint(_))),
        "le dernier maillon ne relâche pas ce que le premier exigeait : {refused:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// L'ordre est tenu par les types
// ---------------------------------------------------------------------------------------------
//
// Il n'y a rien à tester ici : `Built` n'a pas de `published`, donc signer sans scanner n'est pas
// un chemin à interdire mais un chemin qui n'existe pas. La garantie est vérifiée par le bloc
// `compile_fail` de la documentation de `locus_environments::build`, que `cargo test --doc`
// exécute — un test d'intégration ne peut pas la porter, les doctests ne tournant que sur la lib.
