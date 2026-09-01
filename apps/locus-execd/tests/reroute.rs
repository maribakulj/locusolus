//! Test de sortie de W4.g.2 — ADR 0004, `docs/SPEC_V1.md` §12.3, §12.2.
//!
//! **Une tâche réattribuée conserve son numéro d'attempt, l'hôte qui l'a perdue n'est jamais
//! rechoisi, et l'épuisement distingue « tous tombés » de « aucun ne convenait ».**
//!
//! §12.3 le dit mot pour mot : « une tâche réattribuée conserve le numéro d'attempt ». C'est la
//! clause la moins intuitive et la plus structurante — un reroutage n'est pas une nouvelle
//! tentative, c'est la même, déplacée.

use locus_execd::{
    Attempt, Attested, Candidate, HostCapabilities, RefusalReason, RerouteError, Rerouting, reroute,
};
use locus_execution::{
    Mount, NetworkMode, ResourceSpec, SandboxLevel, SandboxProfile, SandboxSpec, Standing,
};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn host(best: SandboxLevel) -> HostCapabilities {
    HostCapabilities::new(
        best,
        ResourceSpec::new(8_000, 32 << 30, 4_096, 1 << 40, 86_400).expect("quotas non nuls"),
        vec!["deny", "connector_only", "allowlist", "full"],
    )
    // Le mécanisme annoncé, et le même sous lequel la campagne conclut : ces tests-ci parlent de
    // reroutage, pas de rapprochement de mécanismes — ADR 0035 décision 3 a ses propres cas.
    .employing(MECANISME)
}

/// Le mécanisme que ces hôtes annoncent et sous lequel leurs campagnes concluent.
const MECANISME: &str = "bubblewrap";

fn proven(worker: &str, level: SandboxLevel) -> Candidate {
    Candidate::new(worker, host(level)).attested(Attested {
        backend: MECANISME.to_owned(),
        standing: Standing::Trusted { level },
    })
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

fn attempt(number: u32) -> Attempt {
    Attempt::new(number).expect("un numéro d'attempt vaut au moins 1")
}

// ---------------------------------------------------------------------------------------------
// L'invariant de §12.3
// ---------------------------------------------------------------------------------------------

#[test]
fn le_numero_d_attempt_survit_au_reroutage() {
    let candidates = [proven("a", SandboxLevel::S2), proven("b", SandboxLevel::S2)];
    let lost = attempt(3).lost_on("a");
    let verdict = reroute(&mission(SandboxLevel::S2), &candidates, &lost);

    assert_eq!(
        verdict,
        Rerouting::Rerouted {
            worker: "b".to_owned(),
            level: SandboxLevel::S2,
            attempt: 3,
        },
        "un reroutage n'est pas une nouvelle tentative : c'est la même, déplacée"
    );
}

#[test]
fn le_numero_survit_aussi_a_l_epuisement() {
    let solo = [proven("a", SandboxLevel::S2)];
    let verdict = reroute(&mission(SandboxLevel::S2), &solo, &attempt(7).lost_on("a"));
    assert_eq!(verdict.attempt(), 7);
    assert!(matches!(verdict, Rerouting::Exhausted { .. }));
}

#[test]
fn le_numero_zero_n_existe_pas_dans_le_protocole() {
    assert_eq!(Attempt::new(0), Err(RerouteError::ZeroAttempt));
    assert!(Attempt::new(1).is_ok());
}

// ---------------------------------------------------------------------------------------------
// L'hôte tombé ne revient pas
// ---------------------------------------------------------------------------------------------

#[test]
fn un_hote_qui_a_perdu_la_tentative_n_est_jamais_rechoisi() {
    // « a » serait choisi en temps normal : plafond prouvé le plus bas, et premier dans l'ordre.
    let candidates = [proven("a", SandboxLevel::S1), proven("b", SandboxLevel::S1)];
    let placed_again = reroute(
        &mission(SandboxLevel::S1),
        &candidates,
        &attempt(1).lost_on("a"),
    );

    assert_eq!(
        placed_again,
        Rerouting::Rerouted {
            worker: "b".to_owned(),
            level: SandboxLevel::S1,
            attempt: 1,
        },
        "sans exclusion, la mission tournerait en rond sur la même machine morte"
    );
}

#[test]
fn constater_deux_fois_la_meme_perte_ne_change_rien() {
    let once = attempt(2).lost_on("a");
    let twice = attempt(2).lost_on("a").lost_on("a");
    assert_eq!(
        once, twice,
        "un task.orphaned rejoué n'est pas une seconde perte"
    );
    assert_eq!(once.lost().len(), 1);
}

#[test]
fn l_exclusion_precede_le_choix() {
    // « a » est meilleur candidat que « b » — plafond plus bas — et il est perdu. Si l'exclusion
    // venait après le choix, le placement rendrait « a », le reroutage le rejetterait, et « b »
    // ne serait jamais essayé.
    let candidates = [proven("a", SandboxLevel::S1), proven("b", SandboxLevel::S3)];
    assert!(matches!(
        reroute(&mission(SandboxLevel::S1), &candidates, &attempt(1).lost_on("a")),
        Rerouting::Rerouted { ref worker, .. } if worker == "b"
    ));
}

// ---------------------------------------------------------------------------------------------
// L'épuisement dit laquelle des deux pannes c'est
// ---------------------------------------------------------------------------------------------

#[test]
fn tous_tombes_et_aucun_ne_convenait_sont_deux_epuisements_distincts() {
    let fleet = [proven("a", SandboxLevel::S2), proven("b", SandboxLevel::S2)];

    let all_down = reroute(
        &mission(SandboxLevel::S2),
        &fleet,
        &attempt(1).lost_on("a").lost_on("b"),
    );
    match all_down {
        Rerouting::Exhausted {
            already_lost,
            shortfalls,
            ..
        } => {
            assert_eq!(already_lost, vec!["a".to_owned(), "b".to_owned()]);
            assert!(
                shortfalls.is_empty(),
                "aucun hôte ne restait à examiner : c'est une panne d'infrastructure"
            );
        }
        Rerouting::Rerouted { .. } => panic!("les deux hôtes sont tombés"),
    }

    let none_fit = reroute(&mission(SandboxLevel::S3), &fleet, &attempt(1));
    match none_fit {
        Rerouting::Exhausted {
            already_lost,
            shortfalls,
            ..
        } => {
            assert!(
                already_lost.is_empty(),
                "personne n'a essayé : c'est une mission mal dimensionnée"
            );
            assert_eq!(shortfalls.len(), 2);
            assert!(shortfalls.iter().all(|(_, reasons)| {
                reasons
                    .iter()
                    .any(|reason| matches!(reason, RefusalReason::LevelUnavailable { .. }))
            }));
        }
        Rerouting::Rerouted { .. } => panic!("aucun des deux ne sait confiner en S3"),
    }
}

#[test]
fn l_epuisement_melange_nomme_les_deux_familles() {
    let mixed = [
        proven("tombe", SandboxLevel::S2),
        proven("faible", SandboxLevel::S1),
    ];
    let verdict = reroute(
        &mission(SandboxLevel::S2),
        &mixed,
        &attempt(4).lost_on("tombe"),
    );
    match verdict {
        Rerouting::Exhausted {
            attempt,
            already_lost,
            shortfalls,
        } => {
            assert_eq!(attempt, 4);
            assert_eq!(already_lost, vec!["tombe".to_owned()]);
            assert_eq!(shortfalls.len(), 1);
            assert_eq!(shortfalls[0].0, "faible");
        }
        Rerouting::Rerouted { .. } => panic!("le seul hôte restant ne sait pas confiner en S2"),
    }
}

#[test]
fn sans_aucun_candidat_l_epuisement_reste_lisible() {
    let verdict = reroute(&mission(SandboxLevel::S2), &[], &attempt(1));
    assert_eq!(
        verdict,
        Rerouting::Exhausted {
            attempt: 1,
            already_lost: Vec::new(),
            shortfalls: Vec::new(),
        },
        "les deux listes vides disent « on ne m'a proposé personne », et c'est une information"
    );
}

// ---------------------------------------------------------------------------------------------
// Le reroutage n'affaiblit rien
// ---------------------------------------------------------------------------------------------

#[test]
fn un_hote_non_prouve_ne_devient_pas_acceptable_parce_que_les_autres_sont_tombes() {
    let candidates = [
        proven("prouve", SandboxLevel::S2),
        Candidate::new("annonce-seulement", host(SandboxLevel::S3)),
    ];
    let verdict = reroute(
        &mission(SandboxLevel::S2),
        &candidates,
        &attempt(1).lost_on("prouve"),
    );
    match verdict {
        Rerouting::Exhausted { shortfalls, .. } => {
            assert!(
                shortfalls[0]
                    .1
                    .iter()
                    .any(|reason| matches!(reason, RefusalReason::LevelNotAttested { .. })),
                "l'urgence n'est pas une preuve : {shortfalls:?}"
            );
        }
        Rerouting::Rerouted { .. } => {
            panic!("un hôte sans campagne ne devient pas trusted parce qu'il ne reste que lui")
        }
    }
}

#[test]
fn le_niveau_reroute_reste_celui_qu_exige_la_mission() {
    let candidates = [proven("a", SandboxLevel::S3), proven("b", SandboxLevel::S3)];
    assert!(matches!(
        reroute(&mission(SandboxLevel::S1), &candidates, &attempt(1).lost_on("a")),
        Rerouting::Rerouted { level, .. } if level == SandboxLevel::S1
    ));
}
