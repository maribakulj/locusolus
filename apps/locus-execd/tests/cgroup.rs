//! Test de sortie de `W5.ai` tranche 1 — ADR 0036 décision 3.
//!
//! **Le broker lit ce que l'hôte lui délègue, et il refuse quand rien ne l'est.**
//!
//! # Pourquoi le refus est la moitié qui compte
//!
//! L'ADR 0036 décision 1 relève que sur les trois façons dont un confinement à cgroup peut échouer,
//! **deux sont silencieuses** : la sandbox tourne, simplement sans borne. Un broker qui poserait le
//! cgroup « quand il peut » et passerait outre sinon produirait exactement cela — et rien, ni dans
//! la table des sondes ni dans l'attestation, ne le dirait.
//!
//! # Et pourquoi ce test-ci s'exécute réellement partout
//!
//! Le conteneur de développement de ce chantier **ne délègue rien** : ses trois contrôleurs sont en
//! cgroup v1, et son unifiée ne porte que `hugetlb`. Le chemin de refus y est donc atteignable pour
//! de vrai, ce qui est l'inverse de la répartition habituelle — c'est la moitié qui pose le cgroup
//! qui, elle, demandera un runner.

use std::collections::BTreeSet;

use locus_execd::linux::probe::{HostFacts, REQUIRED_CONTROLLERS, Reader};
use locus_execd::linux::{Delegation, NotDelegated};

// ---------------------------------------------------------------------------------------------
// Un hôte que l'on décrit ligne à ligne
// ---------------------------------------------------------------------------------------------

/// Une lecture scriptée : ce que l'hôte répond, fichier par fichier.
struct FauxHote {
    fichiers: Vec<(String, String)>,
}

impl FauxHote {
    /// Un hôte qui porte une hiérarchie unifiée, et y délègue exactement ces contrôleurs.
    fn deleguant(controleurs: &str) -> Self {
        Self {
            fichiers: vec![
                (
                    "/sys/fs/cgroup/cgroup.controllers".to_owned(),
                    controleurs.to_owned(),
                ),
                ("/proc/self/cgroup".to_owned(), "0::/locus.slice".to_owned()),
                (
                    "/sys/fs/cgroup/locus.slice/cgroup.controllers".to_owned(),
                    controleurs.to_owned(),
                ),
            ],
        }
    }

    /// Un hôte sans hiérarchie unifiée du tout.
    fn sans_hierarchie() -> Self {
        Self {
            fichiers: Vec::new(),
        }
    }
}

impl Reader for FauxHote {
    fn read(&self, path: &str) -> Option<String> {
        self.fichiers
            .iter()
            .find(|(chemin, _)| chemin == path)
            .map(|(_, contenu)| contenu.clone())
    }
}

fn ensemble(mots: &[&str]) -> BTreeSet<String> {
    mots.iter().map(|mot| (*mot).to_owned()).collect()
}

// ---------------------------------------------------------------------------------------------
// Ce qui est délégué
// ---------------------------------------------------------------------------------------------

/// Les trois contrôleurs délégués : la délégation se lit, et elle les porte.
#[test]
fn les_trois_controleurs_delegues_donnent_une_delegation() {
    let facts = HostFacts::probe(&FauxHote::deleguant("cpu memory pids io"));
    let delegation = Delegation::read(&facts).expect("la délégation se lit");

    for controleur in REQUIRED_CONTROLLERS {
        assert!(
            delegation.carries(controleur),
            "« {controleur} » est porté : {:?}",
            delegation.controllers()
        );
    }
    assert!(
        delegation.carries("io"),
        "et ce que l'hôte délègue en plus n'est pas jeté"
    );
}

// ---------------------------------------------------------------------------------------------
// Ce qui est refusé, et les deux refus ne se confondent pas
// ---------------------------------------------------------------------------------------------

/// **Pas de hiérarchie unifiée** : la question ne se pose même pas, et le refus le dit ainsi.
#[test]
fn sans_hierarchie_unifiee_le_refus_nomme_le_fichier_absent() {
    let facts = HostFacts::probe(&FauxHote::sans_hierarchie());
    let refus = Delegation::read(&facts).expect_err("rien n'est délégué");

    assert!(
        matches!(refus, NotDelegated::NoUnifiedHierarchy { .. }),
        "c'est le premier refus, pas le second : {refus:?}"
    );
    let dit = refus.to_string();
    assert!(
        dit.contains("cgroup.controllers"),
        "le refus nomme le fichier qu'un exploitant ira regarder : {dit}"
    );
}

/// **Hiérarchie présente, contrôleurs absents** : l'autre refus, et il envoie ailleurs.
///
/// Les deux ne se réparent pas au même endroit — l'un fait monter une hiérarchie, l'autre fait
/// **déléguer**. Un refus unique enverrait la moitié des exploitants chercher la mauvaise chose.
#[test]
fn une_hierarchie_sans_les_controleurs_est_un_autre_refus() {
    let facts = HostFacts::probe(&FauxHote::deleguant("io hugetlb"));
    let refus = Delegation::read(&facts).expect_err("les contrôleurs manquent");

    let NotDelegated::MissingControllers { missing, available } = &refus else {
        panic!("c'est le second refus : {refus:?}");
    };
    assert_eq!(
        missing,
        &ensemble(&["cpu", "memory", "pids"]),
        "les trois manquants sont nommés, un par un"
    );
    assert_eq!(
        available,
        &ensemble(&["hugetlb", "io"]),
        "et ce qui est là aussi, pour qu'on voie que la lecture a eu lieu"
    );

    let dit = refus.to_string();
    assert!(
        dit.contains("subtree_control"),
        "le refus dit **où** le déploiement doit agir : {dit}"
    );
    assert!(
        dit.contains("sans que rien ne le signale"),
        "et pourquoi passer outre serait pire qu'échouer : {dit}"
    );
}

/// **Un seul contrôleur manquant suffit à refuser.**
///
/// Deux bornes sur trois n'est pas « presque le niveau » : c'est une sandbox dont une ressource
/// n'est pas bornée, et le nom du niveau promet les trois.
#[test]
fn un_seul_controleur_manquant_suffit() {
    let facts = HostFacts::probe(&FauxHote::deleguant("cpu memory"));
    let refus = Delegation::read(&facts).expect_err("« pids » manque");

    let NotDelegated::MissingControllers { missing, .. } = &refus else {
        panic!("c'est le refus par contrôleurs : {refus:?}");
    };
    assert_eq!(missing, &ensemble(&["pids"]));
}

/// Les deux refus ont deux **phrases** différentes.
///
/// Un test d'égalité de variantes passerait sur deux messages identiques, et c'est le message que
/// lit l'exploitant.
#[test]
fn les_deux_refus_ne_disent_pas_la_meme_chose() {
    let sans = Delegation::read(&HostFacts::probe(&FauxHote::sans_hierarchie()))
        .expect_err("rien n'est délégué");
    let partiel = Delegation::read(&HostFacts::probe(&FauxHote::deleguant("io")))
        .expect_err("les contrôleurs manquent");

    assert_ne!(sans.to_string(), partiel.to_string());
}

// ---------------------------------------------------------------------------------------------
// Et contre l'hôte réel
// ---------------------------------------------------------------------------------------------

/// **Contre cet hôte-ci, la lecture aboutit à un verdict** — quel qu'il soit.
///
/// Le test n'exige ni délégation ni refus : ce que la machine offre lui appartient. Il exige que la
/// lecture **conclue**, et il imprime ce qu'elle a conclu, parce que c'est exactement le fait dont
/// la tranche suivante dépendra.
///
/// Mesuré au moment de l'écrire : le conteneur de développement rend le refus
/// `NoUnifiedHierarchy` — ses trois contrôleurs sont en cgroup v1 et son unifiée ne porte que
/// `hugetlb` —, tandis que le runner de CI porte `cpu, cpuset, io, memory, pids`. Les deux chemins
/// sont donc exercés par le seul fait de tourner aux deux endroits.
#[test]
fn la_lecture_conclut_contre_l_hote_reel() {
    let facts = HostFacts::read_host();
    match Delegation::read(&facts) {
        Ok(delegation) => {
            println!("cet hôte délègue : {:?}", delegation.controllers());
            for controleur in REQUIRED_CONTROLLERS {
                assert!(
                    delegation.carries(controleur),
                    "une délégation rendue porte les trois, sinon `read` aurait refusé"
                );
            }
        }
        Err(refus) => {
            println!("cet hôte ne délègue pas : {refus}");
            assert!(
                !refus.to_string().is_empty(),
                "un refus dit pourquoi ; un refus muet ne se répare pas"
            );
        }
    }
}
