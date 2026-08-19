//! Test de sortie de W4.b — ADR 0004, `docs/SPEC_V1.md` §21.6, §32.3.
//!
//! **Chaque niveau que la suite couvre contient strictement plus que le précédent, et une sonde
//! qu'on n'a pas su lancer ne compte jamais comme une réussite.**
//!
//! Les deux moitiés disent la même chose par les deux bouts. La première : un niveau qui ne contient
//! rien de plus que le précédent n'est pas un niveau, c'est un synonyme — et un `SandboxSpec` qui
//! exigerait `S3` obtiendrait `S2` sans que rien ne le signale. La seconde : une sonde non exécutée
//! comptée comme bloquée ferait d'un outil manquant une preuve d'isolation, ce qui est la façon la
//! plus tranquille de croire une sandbox qu'on n'a jamais testée.

use std::collections::BTreeSet;

use locus_execution::{
    Dimension, Expectation, Observed, ResourceSpec, SELF_TESTABLE_LEVELS, SUITE, SandboxLevel,
    Standing, Verdict, expectation, judge, newly_contained, standing,
};

/// Une réservation qui **déclare** un quota disque.
///
/// `W5.j` : l'attente d'`exceed_disk_quota` dépend de ce que la mission a réservé, parce que le
/// disque est la seule ressource que `ResourceSpec` laisse valoir zéro. Les tests de couverture de
/// ce fichier raisonnent sur les niveaux ; ils passent donc une réservation qui déclare tout, pour
/// que ce soit bien le **niveau** qu'ils éprouvent et pas une ressource absente.
fn reserved() -> ResourceSpec {
    ResourceSpec::new(1_000, 1 << 30, 64, 1 << 30, 300).expect("quotas non nuls")
}

/// La même, sans quota disque — le cas ordinaire d'une mission qui n'en réserve pas.
fn unreserved() -> ResourceSpec {
    ResourceSpec::new(1_000, 1 << 30, 64, 0, 300).expect("quotas non nuls")
}

// ---------------------------------------------------------------------------------------------
// Première moitié : chaque niveau veut dire quelque chose
// ---------------------------------------------------------------------------------------------

#[test]
fn chaque_niveau_testable_contient_strictement_plus_que_le_precedent() {
    // `S0` excepté : « unsandboxed-explicit » ne contient rien, et c'est ce que son nom dit.
    for level in [
        SandboxLevel::S1,
        SandboxLevel::S2,
        SandboxLevel::S3,
        SandboxLevel::S4,
    ] {
        let gained = newly_contained(level);
        assert!(
            !gained.is_empty(),
            "{level} ne contient rien de plus que le niveau précédent : ce n'est pas un niveau, \
             c'est un synonyme, et une mission qui l'exige obtiendrait le précédent sans le savoir"
        );
    }
    assert!(
        newly_contained(SandboxLevel::S0).is_empty(),
        "S0 ne contient rien : c'est la définition de « unsandboxed-explicit »"
    );
}

#[test]
fn s5_ne_gagne_aucune_sonde_et_ce_n_est_pas_un_oubli() {
    // S5 promet une protection contre l'hôte lui-même. Une suite de self-tests s'exécute sur cet
    // hôte : une sonde qui prétendrait vérifier « l'opérateur ne peut pas lire ma mémoire » rendrait
    // le verdict que l'hôte aurait choisi de lui rendre. C'est une limite de méthode, pas une sonde
    // manquante — et ce test existe pour que personne ne « complète » la suite en inventant celle
    // qui ne peut pas exister.
    assert!(newly_contained(SandboxLevel::S5).is_empty());
    assert!(!SELF_TESTABLE_LEVELS.contains(&SandboxLevel::S5));
    assert_eq!(SELF_TESTABLE_LEVELS.len(), 5, "S0 à S4");
}

#[test]
fn le_confinement_ne_se_relache_jamais_en_montant() {
    // Une sonde contenue à un niveau doit l'être à tous les niveaux au-dessus. Sans cette
    // monotonie, monter d'un niveau pourrait rouvrir ce que le niveau d'en dessous fermait, et
    // « exiger davantage » cesserait d'être une phrase qui a un sens.
    for probe in &SUITE {
        let mut seen_contained = false;
        for level in SandboxLevel::ALL {
            match expectation(probe, level, &reserved()) {
                Expectation::Contained => seen_contained = true,
                Expectation::Allowed => assert!(
                    !seen_contained,
                    "« {} » redevient permise à {level} après avoir été contenue",
                    probe.name
                ),
            }
        }
        assert!(
            seen_contained,
            "« {} » n'est contenue à aucun niveau : elle ne teste rien",
            probe.name
        );
    }
}

#[test]
fn la_frontiere_de_chaque_sonde_est_exactement_la_ou_elle_le_dit() {
    for probe in &SUITE {
        assert_eq!(
            expectation(probe, probe.contained_from, &reserved()),
            Expectation::Contained,
            "« {} » doit être contenue dès {}",
            probe.name,
            probe.contained_from
        );
        // Le niveau juste en dessous, quand il existe, doit encore la laisser passer.
        let below = SandboxLevel::ALL
            .iter()
            .rev()
            .find(|level| **level < probe.contained_from);
        if let Some(below) = below {
            assert_eq!(
                expectation(probe, *below, &reserved()),
                Expectation::Allowed,
                "« {} » ne doit pas être contenue dès {below}",
                probe.name
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Couverture : aucune dimension de §32.3 n'est oubliée
// ---------------------------------------------------------------------------------------------

#[test]
fn les_sept_dimensions_sont_toutes_sondees() {
    let covered: BTreeSet<Dimension> = SUITE.iter().map(|probe| probe.dimension).collect();
    for dimension in Dimension::ALL {
        assert!(
            covered.contains(&dimension),
            "aucune sonde ne met à l'épreuve « {} » : une suite qui oublie une dimension entière \
             passe tous ses tests",
            dimension.slug()
        );
    }
}

#[test]
fn les_quatre_quotas_de_32_3_ont_chacun_leur_sonde() {
    // « quotas CPU/RAM/PID/disque vérifiés par self-tests » — les quatre nommément, parce qu'une
    // seule sonde « quotas » laisserait le PID passer, et c'est celui qui manque le plus souvent.
    for quota in [
        "exceed_cpu_quota",
        "exceed_memory_quota",
        "exceed_pid_quota",
        "exceed_disk_quota",
    ] {
        assert!(
            SUITE.iter().any(|probe| probe.name == quota),
            "§32.3 exige {quota}"
        );
    }
}

#[test]
fn chaque_sonde_dit_pourquoi_son_niveau_est_celui_la() {
    for probe in &SUITE {
        assert!(
            !probe.rationale.trim().is_empty(),
            "« {} » ne dit pas pourquoi elle est contenue à partir de {}",
            probe.name,
            probe.contained_from
        );
    }
    let names: BTreeSet<&str> = SUITE.iter().map(|probe| probe.name).collect();
    assert_eq!(names.len(), SUITE.len(), "deux sondes portent le même nom");
}

#[test]
fn les_seize_sondes_sont_critiques_et_c_est_affirme() {
    // La roadmap distingue les tests critiques — « un backend qui échoue un test critique n'est pas
    // trusted ». La distinction existe donc dans le type ; le fait qu'elle soit aujourd'hui vide est
    // affirmé plutôt que laissé ambigu. Le jour où quelqu'un ajoutera une sonde non critique, il
    // devra le décider ici, explicitement.
    assert!(
        SUITE.iter().all(|probe| probe.critical),
        "une sandbox n'a pas de contenu accessoire"
    );
}

// ---------------------------------------------------------------------------------------------
// Seconde moitié : ce qui n'a pas tourné n'a rien prouvé
// ---------------------------------------------------------------------------------------------

fn probe(name: &str) -> &'static locus_execution::Probe {
    SUITE
        .iter()
        .find(|probe| probe.name == name)
        .expect("sonde connue")
}

#[test]
fn les_quatre_verdicts_disent_quatre_choses_differentes() {
    let network = probe("open_outbound_connection");

    // Contenue et bloquée : conforme.
    assert_eq!(
        judge(network, SandboxLevel::S3, &reserved(), Observed::Blocked),
        Verdict::Holds
    );
    // Permise et réussie : conforme aussi — S2 ne promet pas de couper le réseau.
    assert_eq!(
        judge(network, SandboxLevel::S2, &reserved(), Observed::Succeeded),
        Verdict::Holds
    );
    // Contenue et réussie : échappement.
    assert_eq!(
        judge(network, SandboxLevel::S3, &reserved(), Observed::Succeeded),
        Verdict::Escaped {
            probe: "open_outbound_connection",
            level: SandboxLevel::S3,
        }
    );
    // Permise et bloquée : sur-confinement. Pas un trou, mais pas rien — une mission légitime
    // échouera sans que personne cherche du côté de l'isolation, puisqu'elle « va bien ».
    assert_eq!(
        judge(network, SandboxLevel::S2, &reserved(), Observed::Blocked),
        Verdict::OverContained {
            probe: "open_outbound_connection",
            level: SandboxLevel::S2,
        }
    );
}

#[test]
fn une_sonde_non_executee_n_est_ni_reussie_ni_bloquee() {
    let network = probe("open_outbound_connection");
    let verdict = judge(
        network,
        SandboxLevel::S3,
        &reserved(),
        Observed::NotRun {
            reason: "curl absent de l'image",
        },
    );
    assert_eq!(
        verdict,
        Verdict::Inconclusive {
            probe: "open_outbound_connection",
            reason: "curl absent de l'image",
        },
        "la compter comme bloquée ferait d'un outil manquant une preuve d'isolation"
    );
    assert!(
        verdict.denies_trust(true),
        "ADR 0004 : un test critique qu'on n'a pas su lancer n'a pas réussi ; accorder la confiance \
         faute de contre-preuve reviendrait à confondre l'absence de preuve avec la preuve"
    );
    assert!(
        !verdict.denies_trust(false),
        "sur une sonde non critique, l'inconnu ne suffit pas à refuser"
    );
}

#[test]
fn un_echappement_refuse_la_confiance_et_un_sur_confinement_ne_la_refuse_pas() {
    let escaped = Verdict::Escaped {
        probe: "x",
        level: SandboxLevel::S3,
    };
    let over = Verdict::OverContained {
        probe: "x",
        level: SandboxLevel::S2,
    };
    assert!(
        escaped.denies_trust(false),
        "même non critique, un échappement est un échappement"
    );
    assert!(!over.denies_trust(true));
    assert!(!Verdict::Holds.denies_trust(true));
}

// ---------------------------------------------------------------------------------------------
// Le verdict d'ensemble
// ---------------------------------------------------------------------------------------------

/// Un rapport complet où chaque sonde produit ce que le niveau demande.
fn perfect_report(level: SandboxLevel) -> Vec<(&'static str, Observed)> {
    SUITE
        .iter()
        .map(|probe| {
            let observed = match expectation(probe, level, &reserved()) {
                Expectation::Contained => Observed::Blocked,
                Expectation::Allowed => Observed::Succeeded,
            };
            (probe.name, observed)
        })
        .collect()
}

#[test]
fn un_backend_qui_tient_son_niveau_est_trusted() {
    for level in SELF_TESTABLE_LEVELS {
        assert_eq!(
            standing(level, &reserved(), &perfect_report(level)),
            Standing::Trusted { level },
            "{level} : toutes les sondes produisent ce que le niveau promet"
        );
    }
}

#[test]
fn un_seul_echappement_suffit_a_refuser_la_confiance() {
    let mut report = perfect_report(SandboxLevel::S3);
    for entry in &mut report {
        if entry.0 == "access_container_runtime_socket" {
            entry.1 = Observed::Succeeded;
        }
    }
    let Standing::NotTrusted { blocking, .. } = standing(SandboxLevel::S3, &reserved(), &report)
    else {
        panic!("un échappement sur le socket de runtime ne se rattrape pas");
    };
    assert_eq!(
        blocking,
        vec![Verdict::Escaped {
            probe: "access_container_runtime_socket",
            level: SandboxLevel::S3,
        }],
        "« presque trusted » n'existe pas : les missions ont tourné sans le confinement qu'elles \
         croyaient avoir"
    );
}

#[test]
fn une_sonde_absente_du_rapport_refuse_la_confiance() {
    // Le silence n'est pas un succès. Une suite tronquée — parce qu'un backend ne sait pas lancer
    // une sonde, ou parce que le rapport a été écrit à la main — ne doit pas se lire comme une
    // suite passée.
    let report: Vec<(&'static str, Observed)> = perfect_report(SandboxLevel::S4)
        .into_iter()
        .filter(|(name, _)| *name != "exceed_pid_quota")
        .collect();

    let Standing::NotTrusted { blocking, .. } = standing(SandboxLevel::S4, &reserved(), &report)
    else {
        panic!("une sonde absente doit bloquer la confiance");
    };
    assert_eq!(
        blocking,
        vec![Verdict::Inconclusive {
            probe: "exceed_pid_quota",
            reason: "sonde absente du rapport",
        }]
    );
}

#[test]
fn un_sur_confinement_n_empeche_pas_la_confiance_mais_se_lit() {
    // Le réseau coupé à S2 : le backend est plus strict que ce qu'il annonce. Ce n'est pas un trou,
    // donc la confiance tient — mais `judge` le nomme, pour que la mission qui échouera à cause de
    // ça trouve la cause écrite quelque part.
    let mut report = perfect_report(SandboxLevel::S2);
    for entry in &mut report {
        if entry.0 == "open_outbound_connection" {
            entry.1 = Observed::Blocked;
        }
    }
    assert_eq!(
        standing(SandboxLevel::S2, &reserved(), &report),
        Standing::Trusted {
            level: SandboxLevel::S2
        }
    );
    assert_eq!(
        judge(
            probe("open_outbound_connection"),
            SandboxLevel::S2,
            &reserved(),
            Observed::Blocked
        ),
        Verdict::OverContained {
            probe: "open_outbound_connection",
            level: SandboxLevel::S2,
        }
    );
}

// ---------------------------------------------------------------------------------------------
// W5.j — une borne que personne n'a demandée ne peut pas être franchie
// ---------------------------------------------------------------------------------------------

/// **`exceed_disk_quota` doit réussir quand la mission ne réserve aucun disque.**
///
/// Le disque est la seule ressource que `ResourceSpec` laisse valoir zéro : le CPU, la mémoire, les
/// PID et l'horizon sont refusés à zéro. Une mission sans quota disque n'a donc rien promis de
/// borner, et une sonde qui écrit alors sans entrave ne révèle aucun défaut.
///
/// Sans cette règle, toute mission ordinaire — et c'est le cas courant — verrait sa sonde disque
/// ressortir `Escaped` dès `S2`, c'est-à-dire « la sandbox ne tient pas ce qu'elle annonce ».
#[test]
fn la_sonde_disque_est_permise_quand_la_mission_ne_reserve_rien() {
    let disk = probe("exceed_disk_quota");
    for level in SELF_TESTABLE_LEVELS {
        assert_eq!(
            expectation(disk, level, &unreserved()),
            Expectation::Allowed,
            "à {level:?}, sans quota déclaré, il n'y a pas de borne à franchir"
        );
    }
}

/// Et **contenue dès `S2` quand la mission en réserve un** : la règle ne dissout pas la sonde.
#[test]
fn la_sonde_disque_reste_contenue_quand_la_mission_reserve() {
    let disk = probe("exceed_disk_quota");
    assert_eq!(
        expectation(disk, SandboxLevel::S2, &reserved()),
        Expectation::Contained
    );
    assert_eq!(
        expectation(disk, SandboxLevel::S1, &reserved()),
        Expectation::Allowed,
        "S1 ne borne pas le disque, quota déclaré ou non"
    );
}

/// **La réservation ne change l'attente d'aucune autre sonde.**
///
/// C'est ce qui empêche `requires` de devenir un second `contained_from` : quinze sondes sur seize
/// éprouvent des propriétés du **niveau**, et une réservation absente ne doit rien y changer.
#[test]
fn la_reservation_ne_deplace_que_la_sonde_disque() {
    for probe in &SUITE {
        if probe.name == "exceed_disk_quota" {
            continue;
        }
        for level in SELF_TESTABLE_LEVELS {
            assert_eq!(
                expectation(probe, level, &reserved()),
                expectation(probe, level, &unreserved()),
                "« {} » à {level:?} ne regarde pas ce que la mission a réservé",
                probe.name
            );
        }
    }
}

/// Et le verdict suit : réussir sans quota déclaré **tient**, au lieu de compter comme une évasion.
#[test]
fn reussir_sans_quota_declare_tient_au_lieu_de_compter_comme_une_evasion() {
    let disk = probe("exceed_disk_quota");
    assert_eq!(
        judge(disk, SandboxLevel::S2, &unreserved(), Observed::Succeeded),
        Verdict::Holds
    );
    assert_eq!(
        judge(disk, SandboxLevel::S2, &reserved(), Observed::Succeeded),
        Verdict::Escaped {
            probe: "exceed_disk_quota",
            level: SandboxLevel::S2
        },
        "avec un quota déclaré, écrire au-delà reste une évasion"
    );
}
