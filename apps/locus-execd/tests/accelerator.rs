//! Test de sortie de W4.f — ADR 0004, `docs/03` « Local macOS », `docs/05`, `docs/SPEC_V1.md` §12.2.
//!
//! **Sur un hôte où l'accélérateur n'existe qu'en natif, une mission a le conteneur ou
//! l'accélérateur, jamais les deux — et le refus dit lequel des deux il faut lâcher.**
//!
//! `docs/05` : « les capacités macOS natives telles que MPS/MLX sont exposées par un worker de
//! confiance **séparé** ». Le mot « séparé » n'est pas une préférence d'organisation. Metal est une
//! API de macOS, un invité Linux dans une VM n'y a pas accès, et c'est exactement le genre de
//! contrainte qu'on fusionne par optimisme parce que « la machine a bien un GPU ».

use locus_execd::admission::AcceleratorReach;
use locus_execd::{Admission, HostCapabilities, RefusalReason, admit};
use locus_execution::{
    Accelerator, Mount, NetworkMode, ResourceSpec, SandboxLevel, SandboxProfile, SandboxSpec,
};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn metal() -> Accelerator {
    Accelerator {
        kind: "mps".to_owned(),
        count: 1,
        memory_bytes: 8 << 30,
    }
}

fn quotas() -> ResourceSpec {
    ResourceSpec::new(2_000, 8 << 30, 256, 0, 1_800).expect("quotas non nuls")
}

fn capacity() -> ResourceSpec {
    ResourceSpec::new(8_000, 32 << 30, 4_096, 1 << 40, 86_400)
        .expect("quotas non nuls")
        .with_accelerator(metal())
        .expect("accélérateur déclaré")
}

fn mission(level: SandboxLevel, accelerated: bool) -> SandboxSpec {
    let resources = if accelerated {
        quotas()
            .with_accelerator(metal())
            .expect("accélérateur exigé")
    } else {
        quotas()
    };
    let network = if level >= SandboxLevel::S3 {
        NetworkMode::Deny
    } else {
        NetworkMode::Full
    };
    SandboxSpec::new(
        level,
        SandboxProfile::MathCompute,
        network,
        Vec::<Mount>::new(),
        resources,
    )
    .expect("spécification valide")
}

/// Un `MacBook` : la VM Linux confine jusqu'à `S3`, Metal n'est atteignable qu'en natif, où il n'y a
/// pas de conteneur — donc `S1` au mieux.
fn macbook() -> HostCapabilities {
    HostCapabilities::new(
        SandboxLevel::S3,
        capacity(),
        vec!["deny", "connector_only", "allowlist", "full"],
    )
    .native_only(SandboxLevel::S1)
}

/// Un hôte Linux avec un GPU passé au conteneur : l'accélérateur est dans la sandbox.
fn linux_gpu() -> HostCapabilities {
    HostCapabilities::new(
        SandboxLevel::S3,
        capacity(),
        vec!["deny", "connector_only", "allowlist", "full"],
    )
}

// ---------------------------------------------------------------------------------------------
// La règle
// ---------------------------------------------------------------------------------------------

#[test]
fn le_conteneur_et_l_accelerateur_ne_se_cumulent_pas_sur_un_hote_natif() {
    let refused = admit(&mission(SandboxLevel::S3, true), &macbook());
    match refused {
        Admission::Refused { reasons } => {
            assert!(
                reasons.iter().any(|reason| matches!(
                    reason,
                    RefusalReason::AcceleratorOutsideSandbox { kind, required, native_level }
                        if kind == "mps"
                            && *required == SandboxLevel::S3
                            && *native_level == SandboxLevel::S1
                )),
                "{reasons:?}"
            );
        }
        Admission::Admitted { .. } => {
            panic!("Metal n'entre pas dans un invité Linux parce qu'on l'a demandé poliment")
        }
    }
}

#[test]
fn la_meme_mission_au_niveau_natif_est_admise() {
    assert_eq!(
        admit(&mission(SandboxLevel::S1, true), &macbook()),
        Admission::Admitted {
            level: SandboxLevel::S1
        },
        "c'est le sens du « worker de confiance » : il tourne bas, donc on ne lui confie pas tout"
    );
}

#[test]
fn une_mission_sans_accelerateur_garde_le_plafond_du_conteneur() {
    assert_eq!(
        admit(&mission(SandboxLevel::S3, false), &macbook()),
        Admission::Admitted {
            level: SandboxLevel::S3
        },
        "la contrainte porte sur l'accélérateur, pas sur l'hôte"
    );
}

#[test]
fn un_accelerateur_dans_la_sandbox_ne_fait_pas_tomber_le_plafond() {
    assert_eq!(
        admit(&mission(SandboxLevel::S3, true), &linux_gpu()),
        Admission::Admitted {
            level: SandboxLevel::S3
        },
        "un GPU passé au conteneur est une ressource comme une autre"
    );
}

// ---------------------------------------------------------------------------------------------
// Le refus dit quoi faire
// ---------------------------------------------------------------------------------------------

/// Deux refus qui se ressemblent et n'appellent pas la même action : l'un envoie chercher du
/// matériel, l'autre demande de choisir entre le conteneur et l'accélérateur. Les confondre ferait
/// commander un Mac à quelqu'un qui en a déjà un.
#[test]
fn absent_et_hors_sandbox_sont_deux_refus_distincts() {
    let without = HostCapabilities::new(
        SandboxLevel::S3,
        ResourceSpec::new(8_000, 32 << 30, 4_096, 1 << 40, 86_400).expect("quotas non nuls"),
        vec!["deny", "full"],
    );
    let Admission::Refused { reasons } = admit(&mission(SandboxLevel::S3, true), &without) else {
        panic!("un hôte sans accélérateur doit refuser")
    };
    assert!(
        reasons
            .iter()
            .any(|reason| matches!(reason, RefusalReason::AcceleratorUnavailable { .. })),
        "{reasons:?}"
    );
    assert!(
        !reasons
            .iter()
            .any(|reason| matches!(reason, RefusalReason::AcceleratorOutsideSandbox { .. })),
        "l'accélérateur n'est pas « hors sandbox » : il n'est nulle part"
    );
}

#[test]
fn le_refus_porte_toujours_toutes_les_conditions_manquantes() {
    let cramped = HostCapabilities::new(
        SandboxLevel::S3,
        ResourceSpec::new(500, 1 << 30, 32, 0, 60)
            .expect("quotas non nuls")
            .with_accelerator(metal())
            .expect("accélérateur déclaré"),
        vec!["full"],
    )
    .native_only(SandboxLevel::S1);

    let Admission::Refused { reasons } = admit(&mission(SandboxLevel::S3, true), &cramped) else {
        panic!("cet hôte manque de tout")
    };
    assert!(
        reasons
            .iter()
            .any(|reason| matches!(reason, RefusalReason::AcceleratorOutsideSandbox { .. }))
    );
    assert!(
        reasons
            .iter()
            .any(|reason| matches!(reason, RefusalReason::CapacityExceeded))
    );
    assert!(
        reasons
            .iter()
            .any(|reason| matches!(reason, RefusalReason::NetworkModeUnsupported { .. })),
        "un refus qui s'arrête à la première condition fait corriger une chose à la fois : {reasons:?}"
    );
}

#[test]
fn le_message_du_refus_nomme_les_deux_niveaux() {
    let reason = RefusalReason::AcceleratorOutsideSandbox {
        kind: "mps".to_owned(),
        required: SandboxLevel::S3,
        native_level: SandboxLevel::S1,
    };
    let message = reason.to_string();
    assert!(message.contains("mps"), "{message}");
    assert!(message.contains("S1"), "{message}");
    assert!(message.contains("S3"), "{message}");
}

// ---------------------------------------------------------------------------------------------
// La portée est déclarée, pas devinée
// ---------------------------------------------------------------------------------------------

#[test]
fn la_portee_par_defaut_est_la_sandbox() {
    assert_eq!(linux_gpu().reach(), &AcceleratorReach::InsideSandbox);
    assert_eq!(
        macbook().reach(),
        &AcceleratorReach::NativeOnly {
            native_level: SandboxLevel::S1
        }
    );
}

#[test]
fn le_plafond_effectif_depend_de_la_mission_et_pas_seulement_de_l_hote() {
    let host = macbook();
    assert_eq!(
        host.level_for(&mission(SandboxLevel::S3, false)),
        SandboxLevel::S3
    );
    assert_eq!(
        host.level_for(&mission(SandboxLevel::S3, true)),
        SandboxLevel::S1,
        "c'est la mission qui, en demandant l'accélérateur, sort du conteneur"
    );
}
