//! Test de sortie de W12.a — **ce qui n'a pas été éprouvé se nomme.**
//!
//! §29.4 nomme treize fautes, §29.5 quatorze attaques, §29.8 huit ablations. Ce sont des listes
//! **closes**, et c'est ce qui rend l'exercice vérifiable : une liste nommée permet de dire ce qui
//! n'a *pas* été éprouvé, ce qu'une intention générale ne permet jamais.
//!
//! Une release qui part sans avoir éprouvé le disque plein n'est pas nécessairement une faute. La
//! faute est de ne pas le savoir.

use locus_evaluation::{Family, Readiness, RegistryError, Standing, TrialRegistry};

fn tout_mene() -> TrialRegistry {
    let mut registre = TrialRegistry::new();
    for family in Family::ALL {
        for trial in family.trials() {
            registre = registre
                .exercised(family, trial, &format!("tests/{family}/{trial}.rs"))
                .expect("épreuve nommée");
        }
    }
    registre
}

// ---------------------------------------------------------------------------------------------
// Les listes closes de §29
// ---------------------------------------------------------------------------------------------

#[test]
fn les_trois_listes_ont_le_compte_que_29_annonce() {
    assert_eq!(Family::FaultInjection.trials().len(), 13, "§29.4");
    assert_eq!(Family::Security.trials().len(), 14, "§29.5");
    assert_eq!(Family::Ablation.trials().len(), 8, "§29.8");
}

/// Aucune épreuve ne se répète, ni dans sa famille ni ailleurs : deux entrées du même nom seraient
/// comptées deux fois, et l'une des deux pourrait rester non traitée sans que le total le montre.
#[test]
fn aucune_epreuve_n_est_nommee_deux_fois() {
    let mut tous: Vec<&str> = Family::ALL
        .iter()
        .flat_map(|family| family.trials().iter().copied())
        .collect();
    let total = tous.len();
    tous.sort_unstable();
    tous.dedup();
    assert_eq!(tous.len(), total, "35 épreuves distinctes attendues");
}

/// Un registre neuf porte **toutes** les épreuves, en « non traité ». C'est ce qui empêche d'en
/// oublier une en omettant simplement de l'inscrire — l'oubli le plus facile de tous.
#[test]
fn un_registre_neuf_porte_toutes_les_epreuves_en_non_traite() {
    let registre = TrialRegistry::new();
    assert_eq!(registre.unaddressed().len(), 13 + 14 + 8);
    for family in Family::ALL {
        for trial in family.trials() {
            assert_eq!(
                registre.standing(family, trial),
                Some(&Standing::Unaddressed),
                "{family}/{trial}"
            );
        }
    }
}

#[test]
fn une_epreuve_que_29_ne_nomme_pas_est_refusee() {
    assert_eq!(
        TrialRegistry::new().exercised(Family::Security, "sql-injection", "t"),
        Err(RegistryError::UnknownTrial {
            family: Family::Security,
            trial: "sql-injection".to_owned()
        })
    );
}

// ---------------------------------------------------------------------------------------------
// Écartée n'est pas oubliée
// ---------------------------------------------------------------------------------------------

/// Le cœur de W12.a. Les deux se ressemblent dans un rapport — aucune épreuve n'a été menée — et ne
/// se ressemblent pas du tout : l'une est une décision qu'on peut contester, l'autre est un oubli
/// que personne ne voit.
#[test]
fn ecartee_avec_raison_ne_bloque_pas_oubliee_bloque() {
    let ecartee = tout_mene()
        .waived(
            Family::FaultInjection,
            "malicious-federated-peer",
            "aucun pair fédéré en V1 (§25 hors périmètre)",
        )
        .expect("renonciation nommée");
    assert_eq!(ecartee.readiness(), Readiness::Ready { waivers: 1 });

    let oubliee = TrialRegistry::new();
    assert!(matches!(oubliee.readiness(), Readiness::Blocked { .. }));
}

/// Une renonciation sans raison est indiscernable d'un oubli, et c'est exactement la confusion que
/// ce registre existe pour empêcher.
#[test]
fn une_renonciation_sans_raison_est_refusee() {
    assert_eq!(
        TrialRegistry::new().waived(Family::Ablation, "without-graph", "   "),
        Err(RegistryError::EmptyField { field: "reason" })
    );
}

/// Et « éprouvé » sans dire par quoi ne se vérifie pas, donc ne vaut pas mieux que « pas éprouvé ».
#[test]
fn une_epreuve_menee_par_personne_est_refusee() {
    assert_eq!(
        TrialRegistry::new().exercised(Family::Ablation, "without-graph", ""),
        Err(RegistryError::EmptyField { field: "by" })
    );
}

/// Les renonciations se relisent : le verdict en donne le nombre, et le registre les rend avec leur
/// raison. Une release « prête » avec dix-sept renonciations n'est pas la même qu'une release prête
/// sans aucune, et le chiffre est ce qui pousse à regarder.
#[test]
fn les_renonciations_se_comptent_et_se_relisent() {
    let registre = tout_mene()
        .waived(
            Family::Security,
            "cross-tenant-leakage",
            "mono-tenant en V1",
        )
        .expect("nommée")
        .waived(
            Family::Ablation,
            "without-cross-programme-memory",
            "un seul programme",
        )
        .expect("nommée");

    assert_eq!(registre.readiness(), Readiness::Ready { waivers: 2 });
    assert!(registre.readiness().to_string().contains("2 renonciation"));

    let raisons: Vec<&str> = registre.waivers().iter().map(|(_, _, why)| *why).collect();
    assert!(raisons.contains(&"mono-tenant en V1"));
}

// ---------------------------------------------------------------------------------------------
// Le verdict
// ---------------------------------------------------------------------------------------------

/// Chacune des trente-cinq, laissée seule non traitée, bloque — et le verdict la **nomme**. Les
/// éprouver une par une est ce qui empêche qu'une épreuve devienne facultative sans que personne ne
/// s'en aperçoive : c'est le même geste que pour les cinq parties d'une sauvegarde.
#[test]
fn chaque_epreuve_laissee_seule_non_traitee_bloque_et_est_nommee() {
    for family in Family::ALL {
        for oubliee in family.trials() {
            let mut registre = TrialRegistry::new();
            for autre_famille in Family::ALL {
                for trial in autre_famille.trials() {
                    if autre_famille == family && trial == oubliee {
                        continue;
                    }
                    registre = registre
                        .exercised(autre_famille, trial, "tests")
                        .expect("épreuve nommée");
                }
            }

            let verdict = registre.readiness();
            let Readiness::Blocked { unaddressed } = &verdict else {
                panic!("{family}/{oubliee} non traitée et la release se dit prête");
            };
            assert_eq!(unaddressed.as_slice(), &[(family, *oubliee)]);
            assert!(
                verdict.to_string().contains(oubliee),
                "{oubliee} n'est pas nommée : {verdict}"
            );
        }
    }
}

#[test]
fn une_release_sans_renonciation_le_dit() {
    assert_eq!(
        tout_mene().readiness().to_string(),
        "prête ; aucune renonciation"
    );
}

/// Le verdict d'échec nomme la section, pas seulement l'épreuve : savoir qu'il manque `disk-full`
/// sans savoir que c'est de §29.4 oblige à chercher dans trois listes.
#[test]
fn le_verdict_bloque_nomme_la_section() {
    let verdict = TrialRegistry::new().readiness();
    let rendu = verdict.to_string();
    assert!(rendu.contains("§29.4/disk-full"), "{rendu}");
    assert!(rendu.contains("§29.5/ssrf"), "{rendu}");
    assert!(rendu.contains("§29.8/without-graph"), "{rendu}");
}

/// Reconsigner une épreuve remplace son état : une renonciation qu'on décide finalement d'éprouver
/// ne laisse pas deux entrées derrière elle.
#[test]
fn reconsigner_une_epreuve_remplace_son_etat() {
    let registre = TrialRegistry::new()
        .waived(Family::Ablation, "without-graph", "trop cher")
        .expect("nommée")
        .exercised(Family::Ablation, "without-graph", "tests/ablation/graph.rs")
        .expect("nommée");

    assert_eq!(
        registre.standing(Family::Ablation, "without-graph"),
        Some(&Standing::Exercised {
            by: "tests/ablation/graph.rs".to_owned()
        })
    );
    assert!(registre.waivers().is_empty());
}
