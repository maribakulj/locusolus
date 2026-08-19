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
    BACKEND_CEILING, CPU_PERIOD_MICROSECONDS, ConfinementPlan, HostFacts, LocalReader, Missing,
    NO_STORAGE_DECLARED, Namespace, NetworkPosture, PlanError, QuotaTarget, REQUIRED_CONTROLLERS,
    RestrictedProfile, SeccompPosture, SeccompProfiles, Support, Workload, create_arguments, plan,
};
use locus_execd::{Admission, HostCapabilities, RefusalReason, admit};
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
    // Un espace de travail inscriptible, parce que `W5.j` refuse un quota disque à `S2` sans lui :
    // la racine y est en lecture seule, et une borne qui n'a rien à borner est une garantie absente
    // dont tout le chemin a l'air de fonctionner.
    let spec = SandboxSpec::new(
        SandboxLevel::S2,
        SandboxProfile::UntrustedRepository,
        NetworkMode::Full,
        vec![Mount::new("/srv/work", "/work", MountMode::ReadWrite).expect("montage licite")],
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

/// La preuve porte **un constat par fait établi**, et le compte se met à jour avec les faits.
///
/// `W5.g` en ajoute un cinquième — le quota disque — parce que §21.6 veut un témoignage et non une
/// affirmation. Un fait lu qui n'apparaîtrait pas dans la preuve serait un fait que l'attestation
/// tait, donc un fait que personne ne peut contester.
#[test]
fn la_preuve_couvre_les_cinq_constats() {
    let facts = HostFacts::read(&capable_host("preuve"));
    let evidence = facts.evidence();
    assert_eq!(evidence.len(), 5);
    assert!(evidence.iter().all(|line| !line.trim().is_empty()));
    assert!(
        evidence.iter().any(|line| line.starts_with("quota disque")),
        "le quota disque est un fait de l'hôte : il se joint à l'attestation comme les autres"
    );
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
    assert_eq!(facts.evidence().len(), 5);
}

// ---------------------------------------------------------------------------------------------
// W5.g — le quota disque, lu et non appris en échouant
// ---------------------------------------------------------------------------------------------

/// Un `/proc/self/mountinfo` minimal mais réaliste, avec le système de fichiers voulu.
fn mountinfo(root: &Path, storage_point: &str, filesystem: &str, options: &str) {
    write(
        root,
        "proc/self/mountinfo",
        &format!(
            "25 30 0:23 / / rw,relatime shared:1 - ext4 /dev/sda1 rw\n\
             31 25 0:24 / {storage_point} rw,relatime shared:2 - {filesystem} /dev/sdb1 {options}\n"
        ),
    );
}

/// **Sans racine de stockage déclarée, le fait est indéterminé** — et l'indétermination n'est pas
/// une disponibilité.
///
/// Un quota disque est une propriété du chemin où le runtime écrira, pas de l'hôte en général.
/// Deviner un chemin rendrait un fait sur un autre système de fichiers que celui qui sera écrit.
#[test]
fn sans_racine_de_stockage_le_quota_disque_est_indetermine() {
    let root = capable_host("sans-stockage");
    let facts = HostFacts::read(&root);
    assert_eq!(
        facts.disk_quota(),
        &Support::Undetermined {
            reason: NO_STORAGE_DECLARED.to_owned()
        }
    );
    assert!(
        facts.unenforceable_disk_quota().is_some(),
        "« je n'ai pas su regarder » ne vaut pas « c'est disponible » : le doute ne s'arrondit pas \
         vers le haut ici non plus"
    );
}

/// **Un stockage sur ext4 est un refus, et le refus nomme le système de fichiers.**
///
/// C'est le fait que `W5.f` a payé un vrai `podman create` pour découvrir : « storage option
/// overlay.size and overlay.inodes only supported for backingFS XFS. Found extfs ».
#[test]
fn un_stockage_sur_ext4_ne_peut_pas_porter_de_quota() {
    let root = capable_host("ext4");
    mountinfo(&root, "/var/lib/containers", "ext4", "rw");
    let facts = HostFacts::read(&root).with_storage(
        &LocalReader { root: root.clone() },
        "/var/lib/containers/storage",
    );
    let why = facts
        .unenforceable_disk_quota()
        .expect("ext4 ne porte pas de quota de projet");
    assert!(
        why.contains("ext4"),
        "le refus doit nommer le système de fichiers : {why}"
    );
    assert!(
        matches!(facts.disk_quota(), Support::Unavailable { .. }),
        "c'est un refus établi, pas un doute : {:?}",
        facts.disk_quota()
    );
}

/// **XFS sans `prjquota` est un refus aussi**, et il ne dit pas la même chose.
///
/// Podman n'exige que « backingFS XFS », mais un XFS monté sans quota de projet échouera quand
/// même, plus tard et ailleurs. Ce qui n'a pas été constaté n'est pas acquis.
#[test]
fn xfs_sans_quota_de_projet_est_refuse_et_le_dit_autrement() {
    let root = capable_host("xfs-nu");
    mountinfo(&root, "/var/lib/containers", "xfs", "rw,attr2");
    let reader = LocalReader { root: root.clone() };
    let nu = HostFacts::read(&root)
        .with_storage(&reader, "/var/lib/containers/storage")
        .unenforceable_disk_quota()
        .expect("xfs sans prjquota ne porte pas de quota");

    let equipped = capable_host("xfs-prjquota");
    mountinfo(&equipped, "/var/lib/containers", "xfs", "rw,prjquota");
    let facts = HostFacts::read(&equipped).with_storage(
        &LocalReader {
            root: equipped.clone(),
        },
        "/var/lib/containers/storage",
    );
    assert_eq!(facts.disk_quota(), &Support::Available);
    assert!(facts.unenforceable_disk_quota().is_none());
    assert!(
        nu.contains("prjquota"),
        "« le système de fichiers ne convient pas » et « il convient mais n'est pas monté pour » \
         ne s'inspectent pas au même endroit : {nu}"
    );
}

/// **Le montage retenu est le plus long préfixe**, pas le premier venu.
///
/// Prendre `/` pour un stockage qui vit sur un volume monté plus bas rendrait un verdict sur le
/// mauvais système de fichiers — et, ici, un verdict flatteur.
#[test]
fn le_montage_retenu_est_celui_qu_on_traverse() {
    let root = capable_host("prefixe");
    // La racine est en XFS avec quota ; le stockage vit sous un ext4 monté plus bas.
    write(
        &root,
        "proc/self/mountinfo",
        "25 30 0:23 / / rw shared:1 - xfs /dev/sda1 rw,prjquota\n\
         31 25 0:24 / /var/lib/containers rw shared:2 - ext4 /dev/sdb1 rw\n",
    );
    let facts = HostFacts::read(&root).with_storage(
        &LocalReader { root: root.clone() },
        "/var/lib/containers/storage",
    );
    let why = facts
        .unenforceable_disk_quota()
        .expect("c'est l'ext4 du dessous qui décide, pas le xfs de la racine");
    assert!(why.contains("ext4"), "{why}");
}

/// Un nom qui **commence** par un point de montage n'est pas dessous.
///
/// `/var` couvre `/var/lib` mais pas `/variable`. La comparaison se fait au segment, sans quoi un
/// répertoire voisin passerait pour un descendant.
#[test]
fn un_repertoire_voisin_n_est_pas_sous_le_montage() {
    let root = capable_host("voisin");
    write(
        &root,
        "proc/self/mountinfo",
        "25 30 0:23 / / rw shared:1 - xfs /dev/sda1 rw,prjquota\n\
         31 25 0:24 / /var rw shared:2 - ext4 /dev/sdb1 rw\n",
    );
    let facts = HostFacts::read(&root)
        .with_storage(&LocalReader { root: root.clone() }, "/variable/storage");
    assert_eq!(
        facts.disk_quota(),
        &Support::Available,
        "« /variable » n'est pas sous « /var » : c'est la racine en xfs qui décide"
    );
}

/// **La chaîne entière : lu → déclaré → refusé, avant toute création.**
///
/// C'est le test de sortie de `W5.g`. Le fait est lu sur l'hôte, il devient une déclaration, et
/// l'admission décide dessus. Sans ce pont, le fait serait lu et jamais consulté, ce qui reviendrait
/// exactement à ne pas le lire — et `podman create` resterait l'endroit où on l'apprend.
#[test]
fn une_mission_qui_reserve_du_disque_est_refusee_avant_toute_creation() {
    let root = capable_host("chaine");
    mountinfo(&root, "/var/lib/containers", "ext4", "rw");
    let facts = HostFacts::read(&root).with_storage(
        &LocalReader { root: root.clone() },
        "/var/lib/containers/storage",
    );

    let mut host = HostCapabilities::new(
        SandboxLevel::S3,
        ResourceSpec::new(8_000, 32 << 30, 4_096, 256 << 30, 86_400).expect("capacité"),
        vec!["deny", "connector-only", "allowlist", "full"],
    );
    if let Some(why) = facts.unenforceable_disk_quota() {
        host = host.without_disk_quota(&why);
    }

    let reserving = spec_with_disk(1 << 30);
    let Admission::Refused { reasons } = admit(&reserving, &host) else {
        panic!("une mission qui réserve du disque sur un hôte sans quota doit être refusée");
    };
    let named = reasons
        .iter()
        .find_map(|reason| match reason {
            RefusalReason::DiskQuotaNotEnforceable { why, .. } => Some(why.clone()),
            _ => None,
        })
        .expect("le refus doit porter le motif de quota");
    assert!(
        named.contains("ext4"),
        "le refus nomme le système de fichiers : {named}"
    );

    assert!(
        !reasons.contains(&RefusalReason::CapacityExceeded),
        "« la capacité manque » enverrait libérer de la place ; ici réduire la réservation ne \
         changerait rien — les deux ne se réparent pas au même endroit : {reasons:?}"
    );

    // Et la même mission sans réservation de disque passe : c'est la borne qui est refusée, pas
    // l'hôte.
    assert_eq!(
        admit(&spec_with_disk(0), &host),
        Admission::Admitted {
            level: SandboxLevel::S2
        },
        "un hôte sans quota disque reste parfaitement utilisable par une mission qui n'en réserve pas"
    );
}

fn spec_with_disk(disk_bytes: u64) -> SandboxSpec {
    SandboxSpec::new(
        SandboxLevel::S2,
        SandboxProfile::UntrustedRepository,
        NetworkMode::Full,
        Vec::new(),
        ResourceSpec::new(1_000, 512 << 20, 128, disk_bytes, 300).expect("quotas non nuls"),
    )
    .expect("spécification valide")
}

/// **L'admission décide sans toucher au runtime**, et c'est ce qui la rend antérieure à la création.
///
/// « Aucun chemin ne laisse `podman create` être l'endroit où on l'apprend » ne se vérifie pas en
/// suivant les appels — il en suffirait d'un, ajouté plus tard, pour rouvrir le trou. Ce test le
/// tient par l'**absence** : le module d'admission ne connaît ni runtime, ni processus, ni driver.
/// Il ne peut donc pas apprendre en essayant, quelle que soit la bonne volonté de son prochain
/// lecteur.
///
/// Le test lit le **code**, commentaires retirés : expliquer pourquoi un terme est absent le fait
/// apparaître dans la prose, et un test qui confondrait les deux interdirait d'écrire la raison.
#[test]
fn l_admission_ne_connait_aucun_runtime() {
    let source: String = include_str!("../src/admission.rs")
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("//") && !trimmed.starts_with("///")
        })
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "Runner",
        "PodmanBackend",
        "RuntimePort",
        "process::Command",
        "podman",
        "storage-opt",
    ] {
        assert!(
            !source.contains(forbidden),
            "« {forbidden} » ferait de l'admission un endroit où l'on peut apprendre en essayant"
        );
    }
}

/// **ext4 monté avec `prjquota` est refusé quand même**, et c'est le système de fichiers qui le dit.
///
/// ext4 sait porter des quotas de projet — la configuration existe et n'a rien d'exotique. Podman
/// la refuse malgré tout : « only supported for backingFS XFS ». Les deux conditions sont donc
/// **indépendantes**, et vérifier l'option sans vérifier le système de fichiers laisserait passer
/// un hôte que le runtime rejettera. C'est un mutant qui l'a montré : désactiver le contrôle de
/// système de fichiers ne cassait rien tant que toutes les fixtures ext4 étaient aussi sans
/// `prjquota`.
#[test]
fn ext4_avec_quota_de_projet_est_refuse_pour_le_systeme_de_fichiers() {
    let root = capable_host("ext4-prjquota");
    mountinfo(&root, "/var/lib/containers", "ext4", "rw,prjquota");
    let facts = HostFacts::read(&root).with_storage(
        &LocalReader { root: root.clone() },
        "/var/lib/containers/storage",
    );
    let why = facts
        .unenforceable_disk_quota()
        .expect("Podman n'accepte que XFS, quelle que soit l'option de montage");
    assert!(
        why.contains("ext4"),
        "le refus nomme le système de fichiers : {why}"
    );
    assert!(
        !why.contains("prjquota"),
        "le quota de projet EST activé ici : le dire manquant enverrait remonter le volume avec \
         une option qu'il porte déjà — {why}"
    );
}

// ---------------------------------------------------------------------------------------------
// W5.j — le quota s'applique là où la sandbox peut écrire
// ---------------------------------------------------------------------------------------------

/// **La racine inscriptible porte le quota — tant qu'elle est inscriptible.**
#[test]
fn en_deca_de_s2_le_quota_porte_sur_la_couche_inscriptible() {
    let spec = SandboxSpec::new(
        SandboxLevel::S1,
        SandboxProfile::UntrustedRepository,
        NetworkMode::Full,
        Vec::new(),
        ResourceSpec::new(1_000, 1 << 30, 64, 2 << 30, 300).expect("quotas non nuls"),
    )
    .expect("spécification valide");
    let applied = plan(&spec).expect("plan réalisable");

    assert!(
        !applied.read_only_rootfs(),
        "S1 n'y monte pas de racine en lecture seule"
    );
    assert_eq!(applied.quota_target(), &QuotaTarget::WritableRoot);
}

/// **À partir de `S2`, il porte sur l'espace de travail.**
///
/// La racine y est en lecture seule : `--storage-opt size=` y dimensionnerait une couche que
/// personne n'écrit. Le seul endroit inscriptible est le montage de la mission.
#[test]
fn a_partir_de_s2_le_quota_porte_sur_l_espace_de_travail() {
    let spec = SandboxSpec::new(
        SandboxLevel::S2,
        SandboxProfile::UntrustedRepository,
        NetworkMode::Full,
        vec![
            Mount::new("/srv/lecture", "/ro", MountMode::ReadOnly).expect("montage licite"),
            Mount::new("/srv/work", "/work", MountMode::ReadWrite).expect("montage licite"),
        ],
        ResourceSpec::new(1_000, 1 << 30, 64, 2 << 30, 300).expect("quotas non nuls"),
    )
    .expect("spécification valide");
    let applied = plan(&spec).expect("plan réalisable");

    assert!(applied.read_only_rootfs());
    assert_eq!(
        applied.quota_target(),
        &QuotaTarget::Workspace {
            target: "/work".to_owned()
        },
        "un montage en lecture seule ne peut pas porter un quota d'écriture"
    );
}

/// **Sans quota réservé, il n'y a pas de cible** — et pas de `--storage-opt` non plus.
#[test]
fn sans_quota_reserve_il_n_y_a_rien_a_appliquer() {
    let spec = SandboxSpec::new(
        SandboxLevel::S2,
        SandboxProfile::UntrustedRepository,
        NetworkMode::Full,
        Vec::new(),
        ResourceSpec::new(1_000, 1 << 30, 64, 0, 300).expect("quotas non nuls"),
    )
    .expect("spécification valide");
    let applied = plan(&spec).expect("plan réalisable");

    assert_eq!(applied.quota_target(), &QuotaTarget::None);
    assert_eq!(applied.disk_bytes(), 0);
}

/// **Un quota là où rien n'est inscriptible est refusé**, et le refus nomme le niveau.
///
/// C'est la forme la plus tranquille d'une garantie absente : le quota serait déclaré, accepté,
/// transmis au runtime, et n'aurait rien à borner. Tout le chemin aurait l'air de fonctionner.
#[test]
fn un_quota_sans_espace_inscriptible_est_refuse_au_plan() {
    let spec = SandboxSpec::new(
        SandboxLevel::S2,
        SandboxProfile::UntrustedRepository,
        NetworkMode::Full,
        vec![Mount::new("/srv/corpus", "/corpus", MountMode::ReadOnly).expect("montage licite")],
        ResourceSpec::new(1_000, 1 << 30, 64, 2 << 30, 300).expect("quotas non nuls"),
    )
    .expect("spécification valide");

    assert_eq!(
        plan(&spec),
        Err(PlanError::QuotaWithoutWritableSpace {
            level: SandboxLevel::S2
        }),
        "un montage en lecture seule ne rend rien inscriptible"
    );
}

/// Et l'invocation applique le quota **là où la cible le dit**, jamais ailleurs.
#[test]
fn l_invocation_place_le_quota_sur_la_cible_du_plan() {
    let workload = Workload::new("ghcr.io/locus/base@sha256:00", vec!["/bin/sh".to_owned()])
        .expect("workload");
    let profiles = SeccompProfiles {
        restricted: Some(
            RestrictedProfile::parse(
                "/etc/locus/seccomp/restricted.json",
                r#"{ "defaultAction": "SCMP_ACT_ERRNO", "syscalls": [] }"#,
            )
            .expect("profil par défaut-refus"),
        ),
    };

    let workspace = SandboxSpec::new(
        SandboxLevel::S2,
        SandboxProfile::UntrustedRepository,
        NetworkMode::Full,
        vec![Mount::new("/srv/work", "/work", MountMode::ReadWrite).expect("montage licite")],
        ResourceSpec::new(1_000, 1 << 30, 64, 2 << 30, 300).expect("quotas non nuls"),
    )
    .expect("spécification valide");
    let arguments = create_arguments(
        &plan(&workspace).expect("plan"),
        &workload,
        &profiles,
        "locus-0001",
    )
    .expect("invocation");
    assert!(
        arguments.contains(&format!(
            "type=volume,destination=/work,volume-opt=size={}",
            2u64 << 30
        )),
        "le quota doit border l'espace de travail : {arguments:?}"
    );
    assert!(
        !arguments.iter().any(|argument| argument == "--storage-opt"),
        "à S2 la couche inscriptible n'est écrite par personne : {arguments:?}"
    );

    let root = SandboxSpec::new(
        SandboxLevel::S1,
        SandboxProfile::UntrustedRepository,
        NetworkMode::Full,
        Vec::new(),
        ResourceSpec::new(1_000, 1 << 30, 64, 2 << 30, 300).expect("quotas non nuls"),
    )
    .expect("spécification valide");
    let arguments = create_arguments(
        &plan(&root).expect("plan"),
        &workload,
        &profiles,
        "locus-0002",
    )
    .expect("invocation");
    assert!(
        arguments.contains(&format!("size={}", 2u64 << 30)),
        "à S1 la racine est inscriptible, et c'est elle que le quota borne : {arguments:?}"
    );
}
