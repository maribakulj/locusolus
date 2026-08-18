//! Test de sortie de W11.c — **une sauvegarde ne se déclare pas cohérente, et une restauration
//! impossible se déclare au lieu de se faire.**
//!
//! §27.4 nomme cinq parties obligatoires et met les clés à part, « selon procédure ». §27.5 pose la
//! réserve qui compte : une campagne se restaure ailleurs « sous réserve des capabilities requises
//! par ses runs historiques ».
//!
//! Les deux fautes que ces phrases préviennent sont de la même famille — une sauvegarde qu'on croit
//! complète, une restauration qu'on croit faite — et elles ont ceci de commun qu'on ne les découvre
//! qu'au moment de s'en servir.

use std::collections::BTreeSet;

use locus_deployment::{Backup, BackupError, BackupPart, Coherence, KeyHandling, Restorability};

fn cles() -> KeyHandling {
    KeyHandling::Excluded {
        procedure: "clés-hors-bande, rotation trimestrielle".to_owned(),
    }
}

fn complete() -> Backup {
    Backup::taken(
        &[
            "event-store",
            "promoted-artifacts",
            "git-refs",
            "non-secret-config",
            "version-metadata",
        ],
        cles(),
    )
    .expect("sauvegarde décrite")
}

fn offert(capabilities: &[&str]) -> BTreeSet<String> {
    capabilities
        .iter()
        .map(|capability| (*capability).to_owned())
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Cohérente se calcule
// ---------------------------------------------------------------------------------------------

#[test]
fn les_cinq_parties_de_27_4_existent_sous_leur_nom() {
    let slugs: Vec<&str> = BackupPart::ALL.iter().map(|part| part.slug()).collect();
    assert_eq!(
        slugs,
        vec![
            "event-store",
            "promoted-artifacts",
            "git-refs",
            "non-secret-config",
            "version-metadata"
        ]
    );
    for part in BackupPart::ALL {
        assert_eq!(BackupPart::from_slug(part.slug()), Some(part));
    }
}

#[test]
fn une_sauvegarde_complete_est_coherente() {
    assert_eq!(complete().coherence(), Coherence::Coherent);
}

/// Le cœur de W11.c. Chacune des cinq, retirée seule, suffit à rendre la sauvegarde incohérente —
/// et le verdict **nomme** celle qui manque. Éprouver les cinq séparément est ce qui empêche
/// qu'une partie devienne facultative sans que personne ne s'en aperçoive.
#[test]
fn chacune_des_cinq_parties_manquante_rend_la_sauvegarde_incoherente() {
    for absente in BackupPart::ALL {
        let restantes: Vec<&str> = BackupPart::ALL
            .iter()
            .filter(|part| **part != absente)
            .map(|part| part.slug())
            .collect();
        let partielle = Backup::taken(&restantes, cles()).expect("sauvegarde décrite");

        let Coherence::Incomplete { missing } = partielle.coherence() else {
            panic!("{absente} manquante et la sauvegarde se dit cohérente");
        };
        assert_eq!(missing, BTreeSet::from([absente]));
    }
}

/// « Selon procédure » n'autorise pas le silence. Une sauvegarde d'où les clés sont absentes sans
/// qu'on sache pourquoi est indiscernable d'une sauvegarde où on les a oubliées — et les deux se
/// restaurent pareil : mal.
#[test]
fn une_procedure_de_cles_non_nommee_est_refusee() {
    assert_eq!(
        Backup::taken(
            &["event-store"],
            KeyHandling::Included {
                procedure: "   ".to_owned()
            }
        ),
        Err(BackupError::EmptyField {
            field: "keys.procedure"
        })
    );
}

/// Les clés incluses ou exclues sont deux décisions, pas une présence et une absence : les deux
/// nomment leur procédure, et les deux se relisent.
#[test]
fn les_cles_incluses_et_exclues_nomment_toutes_deux_leur_procedure() {
    let incluses = Backup::taken(
        &["event-store"],
        KeyHandling::Included {
            procedure: "coffre scellé".to_owned(),
        },
    )
    .expect("décrite");
    assert_eq!(incluses.keys().procedure(), "coffre scellé");
    assert_eq!(
        complete().keys().procedure(),
        "clés-hors-bande, rotation trimestrielle"
    );
}

/// §27.4, dernière phrase. Le refus est explicite parce que quelqu'un essaiera d'inclure une
/// sandbox en croyant être exhaustif — et une sauvegarde qui porte l'état d'une sandbox invite à la
/// restaurer, donc à traiter du jetable comme une source.
#[test]
fn une_sandbox_est_refusee_nommement() {
    assert_eq!(
        Backup::taken(&["event-store", "sandbox-run-42"], cles()),
        Err(BackupError::NotCanonical {
            part: "sandbox-run-42".to_owned()
        })
    );
}

#[test]
fn une_partie_que_27_4_ne_nomme_pas_est_refusee() {
    assert_eq!(
        Backup::taken(&["event-store", "logs"], cles()),
        Err(BackupError::UnknownPart {
            part: "logs".to_owned()
        })
    );
}

// ---------------------------------------------------------------------------------------------
// Restaurer ailleurs : déclarer, pas rejouer
// ---------------------------------------------------------------------------------------------

#[test]
fn une_sauvegarde_dont_l_hote_offre_tout_est_restaurable() {
    let sauvegarde = complete().requiring(&["cpu", "lean"]);
    assert_eq!(
        sauvegarde.restorable_on(&offert(&["cpu", "lean", "gpu"])),
        Restorability::Ready
    );
}

/// La réserve de §27.5. Restaurer sur un hôte qui n'a pas ce que les runs exigeaient produirait une
/// campagne qu'on croit intacte et qui ne se rejoue pas — et l'écart ne se verrait qu'à la première
/// reproduction, c'est-à-dire des semaines plus tard.
#[test]
fn un_hote_sans_les_capabilites_des_runs_historiques_est_declare_pas_rejoue() {
    let verdict = complete()
        .requiring(&["cpu", "gpu-cuda"])
        .restorable_on(&offert(&["cpu"]));

    assert_eq!(
        verdict,
        Restorability::MissingCapabilities {
            missing: BTreeSet::from(["gpu-cuda".to_owned()])
        }
    );
    assert!(verdict.to_string().contains("gpu-cuda"));
}

/// Une sauvegarde qui n'a pas relevé ce que ses runs exigeaient ne dit pas « rien n'est requis » :
/// personne n'a regardé. Répondre `Ready` ferait passer cette ignorance pour un feu vert, et c'est
/// la troisième fois que la même distinction se pose dans ce paquet.
#[test]
fn une_exigence_non_relevee_n_est_pas_une_absence_d_exigence() {
    assert_eq!(
        complete().restorable_on(&offert(&["cpu"])),
        Restorability::RequirementsUnknown
    );
    // Et relever une liste vide est une réponse, elle : personne n'exigeait rien.
    assert_eq!(
        complete().requiring(&[]).restorable_on(&offert(&[])),
        Restorability::Ready
    );
}

/// Une sauvegarde incohérente ne se juge pas sur l'hôte : la question ne se pose pas encore, et
/// répondre « il manque un GPU » ferait chercher du matériel quand il manque un event store.
#[test]
fn une_sauvegarde_incoherente_le_dit_avant_de_parler_de_l_hote() {
    let partielle = Backup::taken(&["event-store"], cles())
        .expect("décrite")
        .requiring(&["gpu-cuda"]);

    let verdict = partielle.restorable_on(&offert(&[]));
    assert!(matches!(verdict, Restorability::Incoherent { .. }));
    assert!(verdict.to_string().contains("git-refs"), "{verdict}");
    assert!(!verdict.to_string().contains("gpu-cuda"), "{verdict}");
}
