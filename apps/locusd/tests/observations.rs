//! Ce que le daemon dit d'un placement refusé — `W20.aa`, §10.2.
//!
//! # Ce que ces tests protègent
//!
//! Un `204` de §15.2 couvre deux états que le daemon distingue et que personne d'autre ne peut
//! distinguer : « la file n'avait rien » et « une mission était là, le broker a dit non ». La
//! première propriété tenue ici est donc une **asymétrie** : le second cas parle, le premier se
//! tait, et le silence devient un renseignement plutôt qu'une ambiguïté.
//!
//! La seconde est que les sept motifs de §10.2 **ne se fondent pas**. `packages/lep` les a séparés
//! parce qu'ils envoient à des endroits opposés ; une phrase qui dirait « l'hôte ne convient pas »
//! annulerait ce travail au dernier mètre, là où un humain lit.

use locus_broker::protocol::Shortfall;
use locus_lep::{NetworkMode, Reason, SandboxLevel};
use locusd::observations::{MemoryObservations, Observations, unplaced_note};

/// Un manque, sous le nom d'un worker.
fn manque(worker: &str, reasons: Vec<Reason>) -> Shortfall {
    Shortfall {
        worker: worker.to_owned(),
        reasons,
    }
}

// ---------------------------------------------------------------------------------------------
// 1. La phrase dit **où aller**, motif par motif.
// ---------------------------------------------------------------------------------------------

/// **Un niveau indisponible et un niveau non attesté sont deux phrases différentes.**
///
/// `packages/lep` le dit dans sa propre documentation : « l'hôte ne sait pas faire » envoie chercher
/// une autre machine, « l'hôte l'annonce sans l'avoir prouvé » envoie lancer une campagne de
/// self-tests. Les fondre ferait acheter du matériel pour un problème d'attestation.
///
/// Tenu par **deux phrases distinctes** et non par la présence d'un mot : deux rendus qui se
/// ressembleraient passeraient une assertion de contenu.
#[test]
fn indisponible_et_non_atteste_ne_se_confondent_pas() {
    let indisponible = unplaced_note(
        "task_1",
        &[manque(
            "w1",
            vec![Reason::LevelUnavailable {
                required: SandboxLevel::S3,
                best: SandboxLevel::S1,
            }],
        )],
    );
    let non_atteste = unplaced_note(
        "task_1",
        &[manque(
            "w1",
            vec![Reason::LevelNotAttested {
                required: SandboxLevel::S3,
                proven: None,
            }],
        )],
    );

    assert_ne!(indisponible, non_atteste);
    assert!(
        indisponible.contains("changer de machine"),
        "{indisponible}"
    );
    assert!(non_atteste.contains("self-tests"), "{non_atteste}");
    assert!(
        !indisponible.contains("self-tests"),
        "un niveau indisponible n'envoie pas lancer des self-tests : {indisponible}"
    );
}

/// **« Jamais prouvé » et « prouvé trop bas » sont deux ignorances différentes.**
///
/// `proven: None` veut dire qu'aucune campagne n'a conclu ; `proven: Some(niveau)` veut dire que
/// l'hôte a **échoué** à passer les self-tests au-dessus. La première envoie lancer la campagne, la
/// seconde dit qu'elle a déjà tourné. Le champ est un `Option` précisément pour cette raison, et le
/// collapser au rendu perdrait ce que le type a pris soin de garder.
#[test]
fn aucune_campagne_et_campagne_qui_a_conclu_trop_bas_se_distinguent() {
    let jamais = unplaced_note(
        "task_1",
        &[manque(
            "w1",
            vec![Reason::LevelNotAttested {
                required: SandboxLevel::S3,
                proven: None,
            }],
        )],
    );
    let trop_bas = unplaced_note(
        "task_1",
        &[manque(
            "w1",
            vec![Reason::LevelNotAttested {
                required: SandboxLevel::S3,
                proven: Some(SandboxLevel::S1),
            }],
        )],
    );

    assert_ne!(jamais, trop_bas);
    assert!(jamais.contains("aucune campagne"), "{jamais}");
    assert!(trop_bas.contains("échoué"), "{trop_bas}");
}

/// **La capacité et la borne de quota sont deux phrases différentes.**
///
/// « La capacité manque » envoie libérer de la place ou réduire la réservation ; « la borne n'est
/// pas applicable ici » envoie changer de système de fichiers. Les fondre ferait réduire une
/// réservation qui aurait échoué de la même façon à un octet — c'est écrit tel quel dans
/// `packages/lep`, né de `W5.g` et `W5.j`.
#[test]
fn capacite_et_quota_non_applicable_ne_se_confondent_pas() {
    let capacite = unplaced_note("task_1", &[manque("w1", vec![Reason::CapacityExceeded])]);
    let quota = unplaced_note(
        "task_1",
        &[manque(
            "w1",
            vec![Reason::DiskQuotaNotEnforceable {
                requested: 4096,
                why: "overlayfs sans projet quota".to_owned(),
            }],
        )],
    );

    assert_ne!(capacite, quota);
    assert!(capacite.contains("réduire la réservation"), "{capacite}");
    assert!(
        quota.contains("pas réduire la réservation"),
        "le quota non applicable dit explicitement de **ne pas** réduire : {quota}"
    );
    assert!(quota.contains("4096"), "{quota}");
    assert!(quota.contains("overlayfs"), "{quota}");
}

/// **Un accélérateur absent et un accélérateur hors sandbox sont deux phrases différentes.**
///
/// Le dire « absent » enverrait chercher du matériel au lieu de choisir entre le conteneur et
/// l'accélérateur.
#[test]
fn accelerateur_absent_et_hors_sandbox_ne_se_confondent_pas() {
    let absent = unplaced_note(
        "task_1",
        &[manque(
            "w1",
            vec![Reason::AcceleratorUnavailable {
                kind: "cuda".to_owned(),
            }],
        )],
    );
    let dehors = unplaced_note(
        "task_1",
        &[manque(
            "w1",
            vec![Reason::AcceleratorOutsideSandbox {
                kind: "cuda".to_owned(),
                required: SandboxLevel::S3,
                native_level: SandboxLevel::S1,
            }],
        )],
    );

    assert_ne!(absent, dehors);
    assert!(absent.contains("aucun accélérateur"), "{absent}");
    assert!(dehors.contains("est sur cet hôte"), "{dehors}");
}

/// **Le mode réseau refusé est nommé.**
#[test]
fn le_mode_reseau_refuse_est_nomme() {
    let note = unplaced_note(
        "task_1",
        &[manque(
            "w1",
            vec![Reason::NetworkModeUnsupported {
                mode: NetworkMode::Deny,
            }],
        )],
    );

    assert!(note.contains("Deny"), "{note}");
}

// ---------------------------------------------------------------------------------------------
// 2. Ce que la phrase porte, et ce qu'elle ne porte jamais.
// ---------------------------------------------------------------------------------------------

/// **Tous les motifs d'un worker sont rendus, pas seulement le premier.**
///
/// `Shortfall::reasons` porte « **tout** ce qui lui manquait, dans l'ordre où le répondant l'a
/// constaté » — sa propre documentation le dit. N'en rendre qu'un ferait corriger un manque pour
/// buter sur le suivant, un tour de chaîne réelle à chaque fois.
#[test]
fn tous_les_motifs_sont_rendus() {
    let note = unplaced_note(
        "task_1",
        &[manque(
            "w1",
            vec![
                Reason::CapacityExceeded,
                Reason::NetworkModeUnsupported {
                    mode: NetworkMode::Deny,
                },
            ],
        )],
    );

    assert!(note.contains("capacité"), "{note}");
    assert!(note.contains("Deny"), "{note}");
}

/// **Chaque worker examiné est nommé, avec ses propres motifs.**
#[test]
fn chaque_worker_examine_est_nomme() {
    let note = unplaced_note(
        "task_1",
        &[
            manque("w1", vec![Reason::CapacityExceeded]),
            manque(
                "w2",
                vec![Reason::LevelUnavailable {
                    required: SandboxLevel::S3,
                    best: SandboxLevel::S2,
                }],
            ),
        ],
    );

    assert!(note.contains("w1"), "{note}");
    assert!(note.contains("w2"), "{note}");
    assert!(note.contains("2 worker(s)"), "{note}");
}

/// **Un worker refusé sans motif est dit comme tel, pas comme « aucun manque ».**
///
/// Un répondant qui dit non sans dire pourquoi est une information — sur le répondant. La rendre par
/// une liste vide enverrait chercher un manque d'hôte qui n'a jamais été constaté.
#[test]
fn un_refus_sans_motif_est_dit_comme_tel() {
    let note = unplaced_note("task_1", &[manque("w1", Vec::new())]);

    assert!(note.contains("sans motif"), "{note}");
}

/// **Aucun worker soumis est un défaut du daemon, et la phrase le dit.**
///
/// `lep_claim` soumet exactement un worker. Si le broker rendait une liste vide, ce serait le daemon
/// qui aurait un défaut ; rendre « l'hôte ne convient pas » enverrait alors chercher une machine
/// pour un bug.
#[test]
fn aucun_worker_soumis_accuse_le_daemon_et_non_l_hote() {
    let note = unplaced_note("task_1", &[]);

    assert!(note.contains("défaut du daemon"), "{note}");
    assert!(
        !note.contains("manque d'hôte —"),
        "la phrase n'envoie pas chercher une machine : {note}"
    );
}

/// **La tâche est nommée dans tous les cas.**
///
/// Sans elle, une note lue dans un journal de daemon ne dit pas *laquelle* des missions en file a
/// été rendue — et un exploitant qui en a trois n'apprend rien.
#[test]
fn la_tache_est_toujours_nommee() {
    for shortfalls in [
        Vec::new(),
        vec![manque("w1", vec![Reason::CapacityExceeded])],
    ] {
        let note = unplaced_note("task_01HZ", &shortfalls);
        assert!(note.contains("task_01HZ"), "{note}");
    }
}

// ---------------------------------------------------------------------------------------------
// 3. Le puits, et l'asymétrie du silence.
// ---------------------------------------------------------------------------------------------

/// **Le puits de test garde ce qu'on lui donne, dans l'ordre.**
///
/// Le pendant positif de tout ce qui précède : un puits qui perdrait ses notes rendrait vert
/// n'importe quel test d'absence écrit contre lui.
#[test]
fn le_puits_garde_ce_qu_on_lui_donne() {
    let puits = MemoryObservations::new();
    assert_eq!(puits.notes(), Vec::<String>::new());

    puits.unplaced("première");
    puits.unplaced("seconde");

    assert_eq!(
        puits.notes(),
        vec!["première".to_owned(), "seconde".to_owned()]
    );
}
