//! Test de sortie de W4.a — `docs/SPEC_V1.md` §21.6, §21.7, §21.9, invariants 5 et 6.
//!
//! **Un niveau appliqué sous le niveau exigé est refusé, et l'écart qui est autorisé produit son
//! événement de sécurité — sans que personne ait à y penser.**
//!
//! La seconde moitié est la moins évidente et la plus importante. §21.6 exige deux choses
//! conjointes pour un downgrade : une approbation explicite **et** un événement de sécurité.
//! Approuver est un geste que quelqu'un pose ; consigner est un geste que personne ne réclame. Le
//! test vérifie donc qu'il n'existe **aucun chemin** qui accepte un écart sans rendre l'événement.

use locus_execution::{
    Accelerator, Approval, AttestationError, Conformance, FORBIDDEN_MOUNT_MARKERS, Mount,
    MountMode, NetworkMode, ResourceError, ResourceSpec, SandboxAttestation, SandboxLevel,
    SandboxProfile, SandboxSpec, SecurityEvent, SecurityEventError, SecurityEventKind, SpecError,
    conformance, forbidden_marker, secret_marker,
};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn resources() -> ResourceSpec {
    ResourceSpec::new(2_000, 4 << 30, 512, 8 << 30, 900).expect("quotas non nuls")
}

fn spec_at(level: SandboxLevel, mounts: Vec<Mount>) -> SandboxSpec {
    SandboxSpec::new(
        level,
        SandboxProfile::UntrustedRepository,
        NetworkMode::Deny,
        mounts,
        resources(),
    )
    .expect("spécification valide")
}

fn attestation_at(level: SandboxLevel) -> SandboxAttestation {
    SandboxAttestation::new(
        level,
        "locus-execd@worker-7",
        vec![
            "cgroup=/locus/task-1".to_owned(),
            "seccomp=strict".to_owned(),
        ],
    )
    .expect("attestation valide")
}

fn approval() -> Approval {
    Approval::new("marie", "corpus DH illisible sous S4, arbitrage du 17/08")
        .expect("acteur et raison non vides")
}

// ---------------------------------------------------------------------------------------------
// §21.6 — l'échelle, et ce qu'elle permet de comparer
// ---------------------------------------------------------------------------------------------

#[test]
fn les_six_niveaux_de_21_6_forment_une_echelle() {
    let codes: Vec<&str> = SandboxLevel::ALL.iter().map(|level| level.code()).collect();
    assert_eq!(codes, vec!["S0", "S1", "S2", "S3", "S4", "S5"]);

    let slugs: Vec<&str> = SandboxLevel::ALL.iter().map(|level| level.slug()).collect();
    assert_eq!(
        slugs,
        vec![
            "unsandboxed-explicit",
            "os-write-contained",
            "container-rootless",
            "container-isolated-network",
            "microvm-high-risk",
            "remote-trusted-enclave-or-equivalent",
        ]
    );

    // L'ordre est significatif ici, contrairement à `ValidationLevel` qui refuse `Ord` — la règle
    // « un downgrade est interdit » n'a de sens que si l'on peut comparer.
    for window in SandboxLevel::ALL.windows(2) {
        assert!(window[0] < window[1], "{:?} < {:?}", window[0], window[1]);
    }
    assert!(SandboxLevel::S4.satisfies(SandboxLevel::S2));
    assert!(!SandboxLevel::S2.satisfies(SandboxLevel::S4));
}

#[test]
fn un_niveau_inconnu_reste_inconnu() {
    for level in SandboxLevel::ALL {
        assert_eq!(SandboxLevel::parse(level.code()), Some(level));
        assert_eq!(SandboxLevel::parse(level.slug()), Some(level));
    }
    // Ni `S0` — ce qui ouvrirait la sandbox — ni `S5`, ce qui masquerait la faute de frappe en la
    // rendant inoffensive.
    assert_eq!(SandboxLevel::parse("S6"), None);
    assert_eq!(SandboxLevel::parse("microvm"), None);
}

#[test]
fn les_sept_profils_de_21_6_sont_la_et_ne_portent_pas_de_niveau() {
    let slugs: Vec<&str> = SandboxProfile::ALL
        .iter()
        .map(|profile| profile.slug())
        .collect();
    assert_eq!(
        slugs,
        vec![
            "interactive-local",
            "readonly-review",
            "network-allowlisted",
            "math-compute",
            "dh-corpus",
            "untrusted-repository",
            "microvm-high-risk",
        ],
        "§21.6 les énumère sans dire à quel niveau chacun s'exécute ; leur en attribuer un ici \
         serait écrire une politique de sécurité dans un type"
    );
}

// ---------------------------------------------------------------------------------------------
// Le test de sortie — première moitié : le downgrade est refusé
// ---------------------------------------------------------------------------------------------

#[test]
fn un_niveau_applique_sous_le_plancher_est_refuse() {
    let spec = spec_at(SandboxLevel::S4, Vec::new());
    let applied = attestation_at(SandboxLevel::S2);

    assert_eq!(
        conformance(&spec, &applied),
        Err(AttestationError::Downgrade {
            required: SandboxLevel::S4,
            applied: SandboxLevel::S2,
        }),
        "un worker qui applique moins que demandé n'a pas exécuté la mission, il en a exécuté une autre"
    );
}

#[test]
fn un_niveau_au_moins_egal_passe_sans_evenement() {
    let spec = spec_at(SandboxLevel::S2, Vec::new());
    for applied in [SandboxLevel::S2, SandboxLevel::S3, SandboxLevel::S5] {
        assert_eq!(
            conformance(&spec, &attestation_at(applied)),
            Ok(Conformance::Conforms),
            "{applied:?} tient le plancher S2 : il n'y a rien à consigner"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Le test de sortie — seconde moitié : l'écart autorisé produit son événement
// ---------------------------------------------------------------------------------------------

#[test]
fn un_downgrade_approuve_rend_l_evenement_de_securite_dans_le_verdict() {
    let spec = spec_at(SandboxLevel::S4, Vec::new());
    let applied = attestation_at(SandboxLevel::S2).with_approval(approval());

    let Ok(Conformance::ApprovedDeviation { events }) = conformance(&spec, &applied) else {
        panic!("un downgrade approuvé doit passer, et rendre son événement");
    };
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind(), SecurityEventKind::SandboxDowngrade);
    assert_eq!(events[0].actor(), "marie");
    assert!(events[0].decision().contains("S4"));
    assert!(events[0].decision().contains("S2"));
    assert!(
        events[0].decision().contains("arbitrage du 17/08"),
        "la raison de l'approbation fait partie de la décision consignée"
    );
    assert_eq!(
        events[0].evidence(),
        applied.evidence(),
        "les preuves techniques du worker sont celles que l'audit relira"
    );
}

#[test]
fn il_n_existe_aucun_chemin_qui_accepte_un_ecart_sans_evenement() {
    // Le cœur de W4.a. `Conformance` n'a que deux variantes : `Conforms`, qui n'admet aucun écart,
    // et `ApprovedDeviation`, qui **porte** les événements. Il n'y a pas de troisième forme, donc
    // pas de « accepté, à journaliser plus tard » — le trou exact que §21.6 nomme en exigeant les
    // deux conditions ensemble.
    let cases = [
        (SandboxLevel::S4, SandboxLevel::S2, true),
        (SandboxLevel::S4, SandboxLevel::S4, false),
        (SandboxLevel::S0, SandboxLevel::S0, false),
        (SandboxLevel::S5, SandboxLevel::S0, true),
    ];
    for (required, applied, deviates) in cases {
        let spec = spec_at(required, Vec::new());
        let attestation = attestation_at(applied).with_approval(approval());
        match conformance(&spec, &attestation).expect("approuvé, donc accepté") {
            Conformance::Conforms => assert!(
                !deviates,
                "{applied:?} sous {required:?} est un écart et ne doit pas passer pour conforme"
            ),
            Conformance::ApprovedDeviation { events } => {
                assert!(deviates);
                assert!(
                    !events.is_empty(),
                    "un écart sans événement ne se construit pas"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Les montages : la seconde garantie, et la même forme
// ---------------------------------------------------------------------------------------------

#[test]
fn les_montages_que_la_politique_interdit_sont_refuses() {
    let interdits = [
        "/home/marie/corpus",
        "/root/.config",
        "/var/run/docker.sock",
        "/run/podman/podman.sock",
        "/home/marie/.ssh",
        "/etc/shadow",
        "/run/secrets/db",
    ];
    for source in interdits {
        let refused = Mount::new(source, "/work", MountMode::ReadOnly)
            .expect_err("CLAUDE.md interdit cette famille de montage");
        assert!(
            matches!(refused, SpecError::ForbiddenMount { .. }),
            "{source} : {refused}"
        );
    }
}

#[test]
fn le_filet_des_montages_attrape_et_ne_crie_pas_sur_le_reste() {
    // Sans la seconde moitié, une fonction qui refuserait tout passerait le test précédent.
    for source in ["/srv/corpus", "/opt/toolchains/lean", "/var/lib/locus/work"] {
        assert_eq!(forbidden_marker(source), None, "{source} est licite");
        Mount::new(source, "/work", MountMode::ReadOnly).expect("montage ordinaire");
    }
    // La casse ne sauve personne : c'est le même socket.
    assert!(forbidden_marker("/var/run/Docker.sock").is_some());
    assert_eq!(FORBIDDEN_MOUNT_MARKERS.len(), 14);
}

#[test]
fn un_montage_interdit_sous_approbation_produit_son_propre_evenement() {
    let mount = Mount::approved(
        "/var/run/docker.sock",
        "/var/run/docker.sock",
        MountMode::ReadWrite,
        Approval::new("marie", "migration ponctuelle d'images, ticket LOC-42")
            .expect("approbation valide")
            .with_ticket("LOC-42"),
    )
    .expect("dérogation nommée");

    let spec = spec_at(SandboxLevel::S4, vec![mount]);
    // Le niveau, lui, est parfaitement tenu.
    let Ok(Conformance::ApprovedDeviation { events }) =
        conformance(&spec, &attestation_at(SandboxLevel::S4))
    else {
        panic!("le montage sous dérogation est un écart, même à niveau tenu");
    };
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].kind(),
        SecurityEventKind::ForbiddenMountApproved,
        "un socket de runtime monté dans une micro-VM reste un socket de runtime monté : le \
         confinement du niveau ne rachète pas le trou qu'on y a percé"
    );
}

#[test]
fn approuver_ce_qui_est_permis_est_refuse() {
    let refused = Mount::approved("/srv/corpus", "/work", MountMode::ReadOnly, approval())
        .expect_err("ce montage n'avait besoin d'aucune dérogation");
    assert!(matches!(refused, SpecError::PointlessApproval { .. }));
}

#[test]
fn deux_montages_ne_visent_pas_le_meme_point() {
    let error = SandboxSpec::new(
        SandboxLevel::S2,
        SandboxProfile::MathCompute,
        NetworkMode::Deny,
        vec![
            Mount::new("/srv/a", "/work", MountMode::ReadOnly).expect("montage"),
            Mount::new("/srv/b", "/work", MountMode::ReadOnly).expect("montage"),
        ],
        resources(),
    )
    .expect_err("lequel gagne dépendrait de l'ordre d'application");
    assert!(matches!(error, SpecError::DuplicateTarget { .. }));
}

#[test]
fn un_chemin_relatif_est_refuse() {
    assert!(matches!(
        Mount::new("corpus", "/work", MountMode::ReadOnly),
        Err(SpecError::RelativePath { .. })
    ));
}

// ---------------------------------------------------------------------------------------------
// Invariant 6 — rien n'est supposé illimité
// ---------------------------------------------------------------------------------------------

#[test]
fn aucune_ressource_ne_se_declare_sans_borne() {
    // `ResourceSpec` n'a ni `Default`, ni quota optionnel, ni variante « sans limite » : les
    // lignes suivantes ne compileraient pas.
    //     let _ = ResourceSpec::default();
    //     let _ = ResourceSpec { cpu_millis: None, .. };
    for (cpu, memory, pids, seconds, quota) in [
        (0, 1, 1, 1, "cpu_millis"),
        (1, 0, 1, 1, "memory_bytes"),
        (1, 1, 0, 1, "pids"),
        (1, 1, 1, 0, "wall_clock_seconds"),
    ] {
        assert_eq!(
            ResourceSpec::new(cpu, memory, pids, 0, seconds),
            Err(ResourceError::Zero { quota })
        );
    }
    // Zéro disque, en revanche, est un choix licite : une exécution sans droit d'écriture.
    ResourceSpec::new(1, 1, 1, 0, 1).expect("zéro disque est un choix, pas un oubli");
}

#[test]
fn le_placement_compare_quota_par_quota() {
    let capacity = ResourceSpec::new(4_000, 8 << 30, 1_024, 16 << 30, 3_600).expect("capacité");
    assert!(resources().fits_within(&capacity));

    // Beaucoup de mémoire et trop peu de PID : un score agrégé laisserait passer.
    let narrow = ResourceSpec::new(4_000, 64 << 30, 8, 16 << 30, 3_600).expect("capacité");
    assert!(!resources().fits_within(&narrow));
}

#[test]
fn le_gpu_est_une_capability_et_non_une_dimension_de_tout() {
    let plain = resources();
    assert_eq!(plain.accelerator(), None, "invariant 8");

    let wanted = resources()
        .with_accelerator(Accelerator {
            kind: "cuda".to_owned(),
            count: 1,
            memory_bytes: 16 << 30,
        })
        .expect("accélérateur valide");
    // Un worker sans accélérateur ne convient pas ; l'inverse est vrai.
    assert!(!wanted.fits_within(&plain));
    assert!(plain.fits_within(&wanted));

    assert!(matches!(
        resources().with_accelerator(Accelerator {
            kind: "  ".to_owned(),
            count: 1,
            memory_bytes: 1,
        }),
        Err(ResourceError::EmptyKind)
    ));
}

// ---------------------------------------------------------------------------------------------
// §21.9 — l'événement de sécurité ne recopie pas ce qu'il protège
// ---------------------------------------------------------------------------------------------

#[test]
fn un_evenement_de_securite_refuse_de_porter_un_secret() {
    // Les appâts sont assemblés à l'exécution : écrits d'un bloc, ils feraient de ce fichier un
    // endroit où un scanner de secrets trouve des motifs. Même précaution qu'en W2.14.
    let bait_key = format!("-----{} {}-----", "BEGIN", "PRIVATE KEY");
    let bait_aws = format!("{}{}", "AKIA", "IOSFODNN7EXAMPLE");
    let bait_header = format!("{}: {} eyJhbGciOi", "authorization", "Bearer");

    for line in [bait_key, bait_aws, bait_header] {
        let refused = SecurityEvent::new(
            SecurityEventKind::SandboxDowngrade,
            "marie",
            "sandbox/dh-corpus",
            "downgrade approuvé",
            vec![line.clone()],
        )
        .expect_err("le journal de sécurité serait l'endroit où l'on accumule ce qu'on protège");
        assert!(matches!(refused, SecurityEventError::LeakedSecret { .. }));
        assert!(secret_marker(&line).is_some());
    }
}

#[test]
fn le_filet_des_secrets_ne_crie_pas_sur_une_preuve_ordinaire() {
    // Sans ce test, une fonction refusant tout passerait le précédent — et aucun événement de
    // sécurité ne pourrait plus être consigné, ce qui supprimerait la garantie en la renforçant.
    let evidence = vec![
        "cgroup=/locus/task-1".to_owned(),
        "seccomp=strict".to_owned(),
        "image=sha256:9f2b".to_owned(),
    ];
    for line in &evidence {
        assert_eq!(secret_marker(line), None, "{line}");
    }
    SecurityEvent::new(
        SecurityEventKind::SandboxDowngrade,
        "marie",
        "sandbox/dh-corpus",
        "downgrade approuvé",
        evidence,
    )
    .expect("une preuve technique ordinaire se consigne");
}

#[test]
fn les_quatre_champs_de_21_9_sont_exiges() {
    for (actor, scope, decision, field) in [
        ("", "s", "d", "actor"),
        ("a", "  ", "d", "scope"),
        ("a", "s", "", "decision"),
    ] {
        assert_eq!(
            SecurityEvent::new(
                SecurityEventKind::SandboxDowngrade,
                actor,
                scope,
                decision,
                Vec::new()
            ),
            Err(SecurityEventError::Empty { field })
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Attestation et approbation : ce qui ne s'écrit pas
// ---------------------------------------------------------------------------------------------

#[test]
fn une_attestation_sans_preuve_ou_sans_temoin_ne_se_construit_pas() {
    assert_eq!(
        SandboxAttestation::new(SandboxLevel::S4, "  ", vec!["cgroup=x".to_owned()]),
        Err(AttestationError::EmptyAttester)
    );
    assert_eq!(
        SandboxAttestation::new(SandboxLevel::S4, "worker-7", vec!["   ".to_owned()]),
        Err(AttestationError::NoEvidence),
        "« j'ai appliqué S4 » sans rien qui le montre est une affirmation ; l'invariant 5 demande \
         une attestation"
    );
}

#[test]
fn une_approbation_sans_raison_ne_se_construit_pas() {
    assert!(Approval::new("marie", "   ").is_err());
    assert!(Approval::new("", "raison").is_err());
}

#[test]
fn une_allowlist_vide_est_refusee() {
    assert_eq!(
        NetworkMode::allowlist(vec![String::new(), "  ".to_owned()]),
        Err(SpecError::EmptyAllowlist)
    );
    let mode = NetworkMode::allowlist(vec!["iiif.example.org".to_owned()]).expect("allowlist");
    assert_eq!(mode.slug(), "allowlist");
    assert_eq!(NetworkMode::Deny.slug(), "deny");
    assert_eq!(NetworkMode::ConnectorOnly.slug(), "connector_only");
    assert_eq!(NetworkMode::Full.slug(), "full");
}
