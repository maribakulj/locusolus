//! Test de sortie de W4.g.1 — ADR 0004, `docs/SPEC_V1.md` §12.2, §21.6.
//!
//! **Un hôte ne reçoit que le niveau qu'il a prouvé tenir, le refus dit ce qui manquait à chaque
//! candidat, et le même journal replacé deux fois place au même endroit.**
//!
//! §12.2 demande une sandbox « disponible **et attestée** ». Le mot « attestée » était resté sans
//! consommateur : `HostCapabilities` annonçait un niveau et rien ne demandait à l'hôte de l'avoir
//! tenu. W4.d.3 a produit le juge ; ce module le branche.

use locus_execd::admission::AcceleratorReach;
use locus_execd::{Candidate, HostCapabilities, Placement, RefusalReason, place};
use locus_execution::{
    Mount, NetworkMode, ResourceSpec, SandboxLevel, SandboxProfile, SandboxSpec, Standing,
};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn capacity() -> ResourceSpec {
    ResourceSpec::new(8_000, 32 << 30, 4_096, 1 << 40, 86_400).expect("quotas non nuls")
}

fn host(best: SandboxLevel) -> HostCapabilities {
    HostCapabilities::new(
        best,
        capacity(),
        vec!["deny", "connector_only", "allowlist", "full"],
    )
}

fn mission(level: SandboxLevel) -> SandboxSpec {
    let network = if level >= SandboxLevel::S3 {
        NetworkMode::Deny
    } else {
        NetworkMode::Full
    };
    SandboxSpec::new(
        level,
        SandboxProfile::UntrustedRepository,
        network,
        Vec::<Mount>::new(),
        ResourceSpec::new(1_000, 1 << 30, 64, 0, 300).expect("quotas non nuls"),
    )
    .expect("spécification valide")
}

/// Un candidat qui annonce et qui a prouvé.
fn proven(worker: &str, level: SandboxLevel) -> Candidate {
    Candidate::new(worker, host(level)).attested(Standing::Trusted { level })
}

// ---------------------------------------------------------------------------------------------
// La confiance se prouve
// ---------------------------------------------------------------------------------------------

#[test]
fn un_hote_qui_n_a_rien_prouve_ne_recoit_rien_au_dessus_de_s0() {
    let fresh = Candidate::new("worker-neuf", host(SandboxLevel::S3));
    let Placement::Refused { shortfalls } = place(&mission(SandboxLevel::S2), &[fresh]) else {
        panic!("annoncer n'est pas prouver")
    };
    assert!(
        shortfalls[0].1.iter().any(|reason| matches!(
            reason,
            RefusalReason::LevelNotAttested { required, proven }
                if *required == SandboxLevel::S2 && proven.is_none()
        )),
        "{shortfalls:?}"
    );
}

#[test]
fn s0_ne_demande_aucune_preuve_puisqu_il_ne_promet_rien() {
    let fresh = Candidate::new("worker-neuf", host(SandboxLevel::S0));
    assert_eq!(
        place(&mission(SandboxLevel::S0), &[fresh]),
        Placement::Placed {
            worker: "worker-neuf".to_owned(),
            level: SandboxLevel::S0
        }
    );
}

#[test]
fn une_campagne_perdue_ne_vaut_pas_une_campagne_gagnee() {
    let failed =
        Candidate::new("worker-perce", host(SandboxLevel::S3)).attested(Standing::NotTrusted {
            level: SandboxLevel::S3,
            blocking: Vec::new(),
        });
    let Placement::Refused { shortfalls } = place(&mission(SandboxLevel::S2), &[failed]) else {
        panic!("un backend qui a échappé une sonde n'est pas trusted")
    };
    assert!(
        shortfalls[0]
            .1
            .iter()
            .any(|reason| matches!(reason, RefusalReason::LevelNotAttested { proven: None, .. }))
    );
}

#[test]
fn une_preuve_a_un_niveau_superieur_couvre_les_niveaux_inferieurs() {
    let strong = proven("worker-fort", SandboxLevel::S3);
    assert_eq!(
        place(&mission(SandboxLevel::S1), &[strong]),
        Placement::Placed {
            worker: "worker-fort".to_owned(),
            level: SandboxLevel::S1
        },
        "prouver S3 prouve a fortiori S1 : l'échelle est ordonnée"
    );
}

#[test]
fn l_attestation_s_ajoute_a_l_admission_sans_la_remplacer() {
    let starved = Candidate::new(
        "worker-etroit",
        HostCapabilities::new(
            SandboxLevel::S3,
            ResourceSpec::new(100, 1 << 20, 4, 0, 10).expect("quotas non nuls"),
            vec!["deny"],
        ),
    )
    .attested(Standing::Trusted {
        level: SandboxLevel::S3,
    });

    let Placement::Refused { shortfalls } = place(&mission(SandboxLevel::S3), &[starved]) else {
        panic!("un hôte prouvé mais sans mémoire ne convient pas")
    };
    assert!(
        shortfalls[0]
            .1
            .iter()
            .any(|reason| matches!(reason, RefusalReason::CapacityExceeded)),
        "un hôte peut avoir prouvé S3 et manquer de mémoire : {shortfalls:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// Le refus porte tous les candidats
// ---------------------------------------------------------------------------------------------

#[test]
fn le_refus_nomme_ce_qui_manquait_a_chacun() {
    let unproven = Candidate::new("a-sans-preuve", host(SandboxLevel::S3));
    let weak = proven("b-trop-faible", SandboxLevel::S1);
    let Placement::Refused { shortfalls } = place(&mission(SandboxLevel::S3), &[unproven, weak])
    else {
        panic!("aucun des deux ne convient")
    };

    assert_eq!(
        shortfalls.len(),
        2,
        "ne garder que le plus proche ferait corriger un hôte à la fois"
    );
    assert_eq!(shortfalls[0].0, "a-sans-preuve");
    assert_eq!(shortfalls[1].0, "b-trop-faible");
    assert!(
        shortfalls[1]
            .1
            .iter()
            .any(|reason| matches!(reason, RefusalReason::LevelUnavailable { .. })),
        "le second manque de confinement, pas de preuve : {:?}",
        shortfalls[1].1
    );
}

#[test]
fn sans_candidat_le_refus_le_dit_au_lieu_de_rester_muet() {
    let refused = place(&mission(SandboxLevel::S2), &[]);
    assert_eq!(
        refused,
        Placement::Refused {
            shortfalls: Vec::new()
        }
    );
    assert!(refused.to_string().contains("aucun candidat"));
}

// ---------------------------------------------------------------------------------------------
// Le choix, et sa reproductibilité
// ---------------------------------------------------------------------------------------------

#[test]
fn le_plafond_le_plus_bas_qui_convient_l_emporte() {
    let candidates = [
        proven("z-fort", SandboxLevel::S3),
        proven("a-juste", SandboxLevel::S1),
    ];
    assert_eq!(
        place(&mission(SandboxLevel::S1), &candidates),
        Placement::Placed {
            worker: "a-juste".to_owned(),
            level: SandboxLevel::S1
        },
        "un S3 consommé par une mission S1 est un S3 indisponible pour celle qui en avait besoin"
    );
}

#[test]
fn a_plafond_egal_l_ordre_est_celui_de_l_identifiant() {
    let ordered = [proven("b", SandboxLevel::S2), proven("a", SandboxLevel::S2)];
    let reversed = [proven("a", SandboxLevel::S2), proven("b", SandboxLevel::S2)];
    let expected = Placement::Placed {
        worker: "a".to_owned(),
        level: SandboxLevel::S2,
    };
    assert_eq!(place(&mission(SandboxLevel::S2), &ordered), expected);
    assert_eq!(
        place(&mission(SandboxLevel::S2), &reversed),
        expected,
        "deux rejeux du même journal doivent placer au même endroit"
    );
}

#[test]
fn le_niveau_place_est_celui_qu_exige_la_mission_jamais_celui_de_l_hote() {
    assert_eq!(
        place(
            &mission(SandboxLevel::S1),
            &[proven("fort", SandboxLevel::S3)]
        ),
        Placement::Placed {
            worker: "fort".to_owned(),
            level: SandboxLevel::S1
        },
        "appliquer plus que demandé est le sur-confinement que W4.b nomme"
    );
}

// ---------------------------------------------------------------------------------------------
// La composition avec W4.f
// ---------------------------------------------------------------------------------------------

#[test]
fn un_hote_natif_prouve_reste_ecarte_d_une_mission_conteneurisee_accelereee() {
    use locus_execution::Accelerator;

    let metal = Accelerator {
        kind: "mps".to_owned(),
        count: 1,
        memory_bytes: 8 << 30,
    };
    let mac = Candidate::new(
        "macbook",
        HostCapabilities::new(
            SandboxLevel::S3,
            capacity()
                .with_accelerator(metal.clone())
                .expect("accélérateur déclaré"),
            vec!["deny", "full"],
        )
        .native_only(SandboxLevel::S1),
    )
    .attested(Standing::Trusted {
        level: SandboxLevel::S3,
    });
    assert_eq!(
        mac.capabilities().reach(),
        &AcceleratorReach::NativeOnly {
            native_level: SandboxLevel::S1
        }
    );

    let accelerated = SandboxSpec::new(
        SandboxLevel::S3,
        SandboxProfile::MathCompute,
        NetworkMode::Deny,
        Vec::<Mount>::new(),
        ResourceSpec::new(1_000, 1 << 30, 64, 0, 300)
            .expect("quotas non nuls")
            .with_accelerator(metal)
            .expect("accélérateur exigé"),
    )
    .expect("spécification valide");

    let Placement::Refused { shortfalls } = place(&accelerated, &[mac]) else {
        panic!("une preuve de confinement ne fait pas entrer Metal dans le conteneur")
    };
    assert!(
        shortfalls[0]
            .1
            .iter()
            .any(|reason| matches!(reason, RefusalReason::AcceleratorOutsideSandbox { .. })),
        "{shortfalls:?}"
    );
}
