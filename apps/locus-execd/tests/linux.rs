//! Test de sortie de W4.d — ADR 0004, `docs/SPEC_V1.md` §21.6, §21.7, `docs/03` §« Isolation ».
//!
//! **Le plan ne concède jamais plus que le niveau exigé, il refuse par son nom ce qu'un conteneur
//! rootless ne sait pas faire, et la lecture de l'hôte nomme ce qui manque au lieu de le
//! supposer.**
//!
//! Les deux moitiés tiennent ensemble. Un plan qui relâcherait quelque chose en montant d'un
//! niveau produirait le downgrade que §21.6 interdit, au moment où personne ne regarde ; une
//! détection optimiste ferait revendiquer un confinement que l'hôte ne porte pas, c'est-à-dire le
//! « sandbox factice » que le plan de rollback d'ADR 0004 nomme comme le seul échec inacceptable.

use std::fs;
use std::path::{Path, PathBuf};

use locus_execd::linux::{
    BACKEND_CEILING, CPU_PERIOD_MICROSECONDS, ConfinementPlan, HostFacts, Missing, Namespace,
    NetworkPosture, PlanError, REQUIRED_CONTROLLERS, SeccompPosture, Support, plan,
};
use locus_execution::{
    Approval, Mount, MountMode, NetworkMode, ResourceSpec, SandboxLevel, SandboxProfile,
    SandboxSpec,
};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn modest() -> ResourceSpec {
    ResourceSpec::new(1_500, 2 << 30, 128, 0, 600).expect("quotas non nuls")
}

fn mission(level: SandboxLevel, network: NetworkMode, mounts: Vec<Mount>) -> SandboxSpec {
    SandboxSpec::new(
        level,
        SandboxProfile::UntrustedRepository,
        network,
        mounts,
        modest(),
    )
    .expect("spécification valide")
}

/// La même mission à tous les niveaux, à une exception près qui est le sujet d'un test à elle
/// seule : `S0` à `S2` demandent `full`, seul mode qu'un `S0` puisse honorer, et `S3` demande
/// `deny`, parce que `S3` **est** le niveau au réseau isolé et que `full` l'y viderait.
fn ladder() -> Vec<ConfinementPlan> {
    [
        (SandboxLevel::S0, NetworkMode::Full),
        (SandboxLevel::S1, NetworkMode::Full),
        (SandboxLevel::S2, NetworkMode::Full),
        (SandboxLevel::S3, NetworkMode::Deny),
    ]
    .into_iter()
    .map(|(level, network)| plan(&mission(level, network, Vec::new())).expect("plan réalisable"))
    .collect()
}

// ---------------------------------------------------------------------------------------------
// La traduction : monter d'un niveau ne relâche rien
// ---------------------------------------------------------------------------------------------

#[test]
fn chaque_niveau_confine_au_moins_autant_que_le_precedent() {
    let plans = ladder();
    for pair in plans.windows(2) {
        let (lower, higher) = (&pair[0], &pair[1]);
        assert!(
            higher.confines_at_least(lower),
            "{} relâche quelque chose que {} tenait",
            higher.level().code(),
            lower.level().code()
        );
    }
}

#[test]
fn chaque_niveau_confine_strictement_plus_que_le_precedent() {
    let plans = ladder();
    for pair in plans.windows(2) {
        let (lower, higher) = (&pair[0], &pair[1]);
        assert!(
            !lower.confines_at_least(higher),
            "{} et {} confinent pareil : l'un des deux niveaux ne veut rien dire",
            lower.level().code(),
            higher.level().code()
        );
    }
}

#[test]
fn le_plan_applique_le_niveau_exige_et_pas_le_plafond() {
    let applied = plan(&mission(SandboxLevel::S1, NetworkMode::Full, Vec::new()))
        .expect("plan réalisable")
        .level();
    assert_eq!(
        applied,
        SandboxLevel::S1,
        "appliquer plus que demandé est le sur-confinement que W4.b nomme"
    );
}

#[test]
fn s0_ne_confine_rien_et_s3_confine_tout() {
    let plans = ladder();
    let (unsandboxed, isolated) = (&plans[0], &plans[3]);

    assert!(unsandboxed.namespaces().is_empty());
    assert_eq!(unsandboxed.seccomp(), SeccompPosture::Unconfined);
    assert!(!unsandboxed.no_new_privileges());
    assert!(!unsandboxed.read_only_rootfs());
    assert!(unsandboxed.dropped_capabilities().is_empty());

    assert_eq!(isolated.seccomp(), SeccompPosture::Restricted);
    assert!(isolated.no_new_privileges());
    assert!(isolated.read_only_rootfs());
    for expected in [
        Namespace::User,
        Namespace::Mount,
        Namespace::Pid,
        Namespace::Ipc,
        Namespace::Uts,
        Namespace::Cgroup,
    ] {
        assert!(
            isolated.namespaces().contains(&expected),
            "S3 devrait créer le namespace {}",
            expected.slug()
        );
    }
}

// ---------------------------------------------------------------------------------------------
// La traduction : ce qu'elle refuse, et par quel nom
// ---------------------------------------------------------------------------------------------

#[test]
fn le_backend_refuse_de_revendiquer_une_microvm_ou_une_enclave() {
    for level in [SandboxLevel::S4, SandboxLevel::S5] {
        let refused = plan(&mission(level, NetworkMode::Full, Vec::new()));
        assert_eq!(
            refused,
            Err(PlanError::LevelBeyondBackend {
                required: level,
                ceiling: BACKEND_CEILING,
            }),
            "un conteneur rootless ne devient pas une micro-VM parce qu'on le lui demande"
        );
    }
}

#[test]
fn un_reseau_autre_que_full_exige_un_namespace_reseau() {
    let modes = [
        NetworkMode::Deny,
        NetworkMode::ConnectorOnly,
        NetworkMode::allowlist(vec!["archive.org".to_owned()]).expect("liste non vide"),
    ];
    for mode in modes {
        for level in [SandboxLevel::S0, SandboxLevel::S1, SandboxLevel::S2] {
            let refused = plan(&mission(level, mode.clone(), Vec::new()));
            assert_eq!(
                refused,
                Err(PlanError::NetworkNeedsIsolation {
                    mode: mode.slug(),
                    minimum: SandboxLevel::S3,
                }),
                "sans namespace réseau, « {} » en {} verrait le réseau de l'hôte",
                mode.slug(),
                level.code()
            );
        }
    }
}

#[test]
fn le_mode_reseau_devient_une_posture_a_partir_de_s3() {
    let denied =
        plan(&mission(SandboxLevel::S3, NetworkMode::Deny, Vec::new())).expect("plan réalisable");
    assert_eq!(denied.network(), &NetworkPosture::Isolated);
    assert!(denied.namespaces().contains(&Namespace::Network));

    let hosts = vec!["archive.org".to_owned(), "gallica.bnf.fr".to_owned()];
    let allowed = plan(&mission(
        SandboxLevel::S3,
        NetworkMode::allowlist(hosts.clone()).expect("liste non vide"),
        Vec::new(),
    ))
    .expect("plan réalisable");
    assert_eq!(
        allowed.network(),
        &NetworkPosture::ProxiedAllowlist { hosts },
        "l'ordre des hôtes est celui de la mission"
    );
}

#[test]
fn le_reseau_de_l_hote_ne_cree_pas_de_namespace_reseau() {
    let full = plan(&mission(SandboxLevel::S2, NetworkMode::Full, Vec::new())).expect("réalisable");
    assert_eq!(full.network(), &NetworkPosture::Host);
    assert!(
        !full.namespaces().contains(&Namespace::Network),
        "créer un netns puis y brancher l'hôte serait dire une chose et en faire une autre"
    );
}

/// Ce refus n'était pas prévu : c'est le test de stricte croissance qui l'a exigé, en constatant
/// que sous `full` le plan `S3` était identique au plan `S2`. Un niveau qui ne change rien à ce
/// qui est appliqué est un niveau qu'on revendique sans le tenir.
#[test]
fn s3_refuse_le_reseau_de_l_hote_qui_le_viderait_de_son_contenu() {
    let refused = plan(&mission(SandboxLevel::S3, NetworkMode::Full, Vec::new()));
    assert_eq!(
        refused,
        Err(PlanError::IsolationContradictsNetwork {
            level: SandboxLevel::S3
        })
    );
    assert!(
        plan(&mission(SandboxLevel::S2, NetworkMode::Full, Vec::new())).is_ok(),
        "ce que la mission voulait reste possible : elle le demande sous son vrai nom"
    );
}

#[test]
fn s0_refuse_un_montage_et_un_quota_disque() {
    let mounted = plan(&mission(
        SandboxLevel::S0,
        NetworkMode::Full,
        vec![Mount::new("/srv/corpus", "/work", MountMode::ReadOnly).expect("licite")],
    ));
    assert_eq!(mounted, Err(PlanError::MountsNeedNamespace));

    let quota = SandboxSpec::new(
        SandboxLevel::S0,
        SandboxProfile::UntrustedRepository,
        NetworkMode::Full,
        Vec::new(),
        ResourceSpec::new(1_000, 1 << 30, 64, 1 << 30, 60).expect("quotas non nuls"),
    )
    .expect("spécification valide");
    assert_eq!(plan(&quota), Err(PlanError::QuotaNeedsContainment));
}

// ---------------------------------------------------------------------------------------------
// La traduction : ce que la mission décide, et qui ne dépend pas du niveau
// ---------------------------------------------------------------------------------------------

#[test]
fn les_quotas_sont_ecrits_a_tous_les_niveaux() {
    for applied in ladder() {
        let files: Vec<&str> = applied.cgroup().iter().map(|limit| limit.file).collect();
        assert_eq!(
            files,
            vec!["cpu.max", "memory.max", "pids.max"],
            "l'invariant 6 ne dépend pas du niveau d'isolation"
        );
    }
}

#[test]
fn le_quota_cpu_se_calcule_contre_la_periode() {
    let applied =
        plan(&mission(SandboxLevel::S2, NetworkMode::Full, Vec::new())).expect("plan réalisable");
    let cpu = &applied.cgroup()[0];
    assert_eq!(cpu.file, "cpu.max");
    assert_eq!(
        cpu.value,
        format!(
            "{} {CPU_PERIOD_MICROSECONDS}",
            1_500 * CPU_PERIOD_MICROSECONDS / 1_000
        ),
        "1,5 CPU sur une période de 100 ms font 150 ms de quota"
    );
}

#[test]
fn le_disque_et_l_horizon_ne_sont_pas_des_limites_cgroup() {
    let spec = SandboxSpec::new(
        SandboxLevel::S2,
        SandboxProfile::UntrustedRepository,
        NetworkMode::Full,
        Vec::new(),
        ResourceSpec::new(1_000, 1 << 30, 64, 4 << 30, 900).expect("quotas non nuls"),
    )
    .expect("spécification valide");
    let applied = plan(&spec).expect("plan réalisable");

    assert_eq!(applied.disk_bytes(), 4 << 30);
    assert_eq!(applied.wall_clock_seconds(), 900);
    for limit in applied.cgroup() {
        assert!(
            !limit.file.starts_with("io."),
            "cgroup v2 borne un débit, pas un espace : {} ne peut pas porter le quota disque",
            limit.file
        );
    }
}

#[test]
fn un_montage_approuve_porte_son_marqueur_jusqu_au_backend() {
    let approval = Approval::new(
        "marcel",
        "corpus personnel monté pour la reproduction du run 42",
    )
    .expect("approbation nommée");
    let deviation = Mount::approved(
        "/home/marcel/corpus",
        "/work",
        MountMode::ReadOnly,
        approval,
    )
    .expect("dérogation licite");
    let applied = plan(&mission(
        SandboxLevel::S2,
        NetworkMode::Full,
        vec![deviation],
    ))
    .expect("plan réalisable");

    let planned = &applied.mounts()[0];
    assert_eq!(planned.deviation, Some("/home/"));
    assert!(planned.read_only);
}

#[test]
fn un_montage_ordinaire_ne_porte_aucune_derogation() {
    let applied = plan(&mission(
        SandboxLevel::S2,
        NetworkMode::Full,
        vec![Mount::new("/srv/corpus", "/work", MountMode::ReadWrite).expect("licite")],
    ))
    .expect("plan réalisable");
    let planned = &applied.mounts()[0];
    assert_eq!(planned.deviation, None);
    assert!(!planned.read_only);
    assert_eq!(planned.target, "/work");
}

// ---------------------------------------------------------------------------------------------
// La lecture de l'hôte
// ---------------------------------------------------------------------------------------------

fn write(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("chemin avec parent")).expect("répertoire créé");
    fs::write(path, content).expect("fichier écrit");
}

/// Un hôte complet : hiérarchie unifiée, trois contrôleurs délégués, userns et seccomp.
fn capable_host(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("locus-w4d-{name}"));
    let _ = fs::remove_dir_all(&root);
    write(
        &root,
        "sys/fs/cgroup/cgroup.controllers",
        "cpu memory pids\n",
    );
    write(&root, "proc/self/cgroup", "0::/user.slice/session.scope\n");
    write(
        &root,
        "sys/fs/cgroup/user.slice/session.scope/cgroup.controllers",
        "cpu io memory pids\n",
    );
    write(&root, "proc/sys/user/max_user_namespaces", "15000\n");
    write(
        &root,
        "proc/sys/kernel/seccomp/actions_avail",
        "kill_process kill_thread trap errno user_notif trace log allow\n",
    );
    root
}

#[test]
fn un_hote_complet_soutient_le_plafond_du_backend() {
    let root = capable_host("complet");
    let facts = HostFacts::read(&root);
    assert_eq!(facts.ceiling(), BACKEND_CEILING);
    assert!(facts.missing_for(SandboxLevel::S3).is_empty());
    for controller in REQUIRED_CONTROLLERS {
        assert!(facts.controllers().contains(controller));
    }
}

#[test]
fn un_controleur_non_delegue_fait_tomber_le_plafond_et_se_nomme() {
    let root = capable_host("sans-pids");
    write(
        &root,
        "sys/fs/cgroup/user.slice/session.scope/cgroup.controllers",
        "cpu memory\n",
    );
    let facts = HostFacts::read(&root);

    assert_eq!(facts.ceiling(), SandboxLevel::S1);
    let missing = facts.missing_for(SandboxLevel::S2);
    assert!(
        missing.iter().any(|entry| matches!(
            entry,
            Missing::Unavailable { what, reason } if *what == "contrôleur cgroup" && reason.contains("pids")
        )),
        "le refus doit nommer le contrôleur manquant, sinon on cherche : {missing:?}"
    );
}

#[test]
fn des_namespaces_utilisateur_interdits_ramenent_a_s0() {
    let root = capable_host("sans-userns");
    write(&root, "proc/sys/user/max_user_namespaces", "0\n");
    let facts = HostFacts::read(&root);

    assert_eq!(facts.ceiling(), SandboxLevel::S0);
    assert!(matches!(
        facts.unprivileged_userns(),
        Support::Unavailable { .. }
    ));
}

#[test]
fn le_correctif_debian_absent_n_est_pas_un_refus() {
    let root = capable_host("sans-toggle-debian");
    assert!(
        !root
            .join("proc/sys/kernel/unprivileged_userns_clone")
            .exists()
    );
    assert_eq!(
        HostFacts::read(&root).ceiling(),
        BACKEND_CEILING,
        "unprivileged_userns_clone n'existe que sur les noyaux Debian : son absence ne dit rien"
    );

    write(&root, "proc/sys/kernel/unprivileged_userns_clone", "0\n");
    assert_eq!(
        HostFacts::read(&root).ceiling(),
        SandboxLevel::S0,
        "présent et à zéro, en revanche, c'est un refus"
    );
}

#[test]
fn un_fichier_illisible_est_indetermine_et_pas_un_refus() {
    let root = capable_host("cgroup-illisible");
    fs::remove_file(root.join("sys/fs/cgroup/user.slice/session.scope/cgroup.controllers"))
        .expect("fichier retiré");
    let facts = HostFacts::read(&root);

    assert!(matches!(facts.cgroup_v2(), Support::Undetermined { .. }));
    assert!(
        facts
            .missing_for(SandboxLevel::S2)
            .iter()
            .any(|entry| matches!(entry, Missing::Undetermined { .. })),
        "« je n'ai pas su regarder » et « le noyau refuse » ne se disent pas pareil"
    );
    assert_eq!(
        facts.ceiling(),
        SandboxLevel::S1,
        "le doute ne s'arrondit pas vers le haut"
    );
}

#[test]
fn sans_hierarchie_unifiee_le_refus_dit_cgroup_v1() {
    let root = capable_host("cgroup-v1");
    fs::remove_file(root.join("sys/fs/cgroup/cgroup.controllers")).expect("fichier retiré");
    let facts = HostFacts::read(&root);

    assert!(matches!(facts.cgroup_v2(), Support::Unavailable { .. }));
    assert_eq!(facts.ceiling(), SandboxLevel::S1);
}

#[test]
fn la_preuve_couvre_les_quatre_constats() {
    let facts = HostFacts::read(&capable_host("preuve"));
    let evidence = facts.evidence();
    assert_eq!(evidence.len(), 4);
    assert!(evidence.iter().all(|line| !line.trim().is_empty()));
}

/// La lecture de l'hôte réel. Elle n'affirme rien sur *cette* machine — la CI, un poste de
/// développement et un conteneur ne répondent pas la même chose, et un test qui exigerait `S3`
/// serait la dépendance implicite à une machine que `CLAUDE.md` interdit.
///
/// Ce qu'elle vérifie est plus fort : quelle que soit la réponse, le module ne panique pas, ne
/// revendique jamais au-delà du plafond, et rend une preuve non vide.
#[test]
fn la_lecture_de_l_hote_reel_reste_dans_ses_bornes() {
    let facts = HostFacts::read_host();
    assert!(
        facts.ceiling() <= BACKEND_CEILING,
        "un backend rootless ne peut pas soutenir {}",
        facts.ceiling().code()
    );
    assert!(facts.missing_for(facts.ceiling()).is_empty());
    assert_eq!(facts.evidence().len(), 4);
}
