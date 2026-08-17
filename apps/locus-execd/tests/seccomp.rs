//! Test de sortie de W4.d.4 — ADR 0004, `docs/03`, `docs/SPEC_V1.md` §21.6.
//!
//! **Un profil qui ne refuse pas ce que la posture promet est refusé, et le refus nomme les appels
//! manquants.**
//!
//! Ce dépôt ne fournit pas de profil : un profil par défaut-refus est une liste de plusieurs
//! centaines d'appels autorisés dont l'exactitude ne se démontre qu'en l'exécutant. En écrire un
//! sans hôte pour l'éprouver produirait soit une sandbox qui casse tout, soit une sandbox qui
//! autorise ce qu'elle prétend refuser. Ce qui est livré est la vérification — mécanique, donc
//! opposable — de celui que le déploiement apporte.

use std::fs;

use locus_execd::linux::{MUST_DENY, ProfileError, RestrictedProfile};

/// Un profil par défaut-refus : tout ce qui n'est pas nommé est refusé.
const DEFAULT_DENY: &str = r#"{
  "defaultAction": "SCMP_ACT_ERRNO",
  "architectures": ["SCMP_ARCH_X86_64"],
  "syscalls": [{ "names": ["read", "write", "clone"], "action": "SCMP_ACT_ALLOW" }]
}"#;

/// Un profil par défaut-permissif qui refuse nommément ce que la posture exige.
fn default_allow_with_denials() -> String {
    format!(
        r#"{{
  "defaultAction": "SCMP_ACT_ALLOW",
  "syscalls": [{{ "names": [{}], "action": "SCMP_ACT_KILL_PROCESS" }}]
}}"#,
        MUST_DENY.map(|name| format!("\"{name}\"")).join(", ")
    )
}

/// La liste elle-même, épinglée par son contenu.
///
/// Les autres tests comparent `permitted` à `MUST_DENY`, ce qui reste vrai quelle que soit la
/// liste : ils vérifient la mécanique, pas ce qu'elle refuse. Celui-ci nomme les huit, avec la
/// raison de chacun, pour qu'en retirer un soit un acte visible dans le diff et non une ligne
/// supprimée par commodité.
#[test]
fn la_liste_nomme_la_creation_de_namespace_et_le_chargement_de_code_noyau() {
    assert_eq!(
        MUST_DENY.to_vec(),
        vec![
            // Créer ou rejoindre un namespace depuis l'intérieur.
            "unshare",
            "setns",
            // Charger, remplacer ou décharger du code noyau.
            "init_module",
            "finit_module",
            "delete_module",
            "kexec_load",
            "kexec_file_load",
            "bpf",
        ],
        "la posture restreinte ne promet ni plus ni moins que ces deux familles"
    );
}

#[test]
fn un_profil_par_defaut_refus_porte_la_posture() {
    let profile = RestrictedProfile::parse("/etc/locus/restricted.json", DEFAULT_DENY)
        .expect("un défaut-refus refuse tout ce qu'il ne nomme pas");
    assert_eq!(profile.path(), "/etc/locus/restricted.json");
}

#[test]
fn un_profil_permissif_qui_refuse_nommement_porte_aussi_la_posture() {
    RestrictedProfile::parse("/etc/locus/deny-list.json", &default_allow_with_denials())
        .expect("les huit appels sont refusés par leur nom");
}

#[test]
fn un_profil_qui_laisse_tout_passer_est_refuse_et_nomme_les_huit() {
    let permissive = r#"{ "defaultAction": "SCMP_ACT_ALLOW", "syscalls": [] }"#;
    match RestrictedProfile::parse("/etc/locus/vide.json", permissive) {
        Err(ProfileError::Permissive { permitted, path }) => {
            assert_eq!(path, "/etc/locus/vide.json");
            assert_eq!(
                permitted,
                MUST_DENY.map(str::to_owned).to_vec(),
                "le refus nomme tous les appels, pour qu'on corrige en une fois"
            );
        }
        other => panic!("un profil sans refus ne porte pas la posture restreinte : {other:?}"),
    }
}

#[test]
fn une_seule_permission_de_trop_suffit_a_refuser_le_profil() {
    let profile = r#"{
  "defaultAction": "SCMP_ACT_ERRNO",
  "syscalls": [{ "names": ["bpf"], "action": "SCMP_ACT_ALLOW" }]
}"#;
    match RestrictedProfile::parse("/etc/locus/presque.json", profile) {
        Err(ProfileError::Permissive { permitted, .. }) => {
            assert_eq!(permitted, vec!["bpf".to_owned()]);
        }
        other => panic!("un profil presque bon n'est pas un profil bon : {other:?}"),
    }
}

#[test]
fn la_premiere_regle_qui_nomme_l_appel_decide() {
    let contradictory = r#"{
  "defaultAction": "SCMP_ACT_ERRNO",
  "syscalls": [
    { "names": ["unshare"], "action": "SCMP_ACT_ALLOW" },
    { "names": ["unshare"], "action": "SCMP_ACT_ERRNO" }
  ]
}"#;
    match RestrictedProfile::parse("/etc/locus/contradictoire.json", contradictory) {
        Err(ProfileError::Permissive { permitted, .. }) => {
            assert_eq!(permitted, vec!["unshare".to_owned()]);
        }
        other => panic!(
            "un profil dont deux règles se contredisent ne doit pas obtenir le bénéfice du doute : {other:?}"
        ),
    }
}

#[test]
fn les_actions_qui_laissent_passer_ne_comptent_pas_comme_des_refus() {
    for action in ["SCMP_ACT_ALLOW", "SCMP_ACT_LOG", "SCMP_ACT_NOTIFY"] {
        let profile = format!(r#"{{ "defaultAction": "{action}", "syscalls": [] }}"#);
        assert!(
            matches!(
                RestrictedProfile::parse("/etc/locus/action.json", &profile),
                Err(ProfileError::Permissive { .. })
            ),
            "« {action} » n'empêche pas l'appel d'aboutir"
        );
    }
}

#[test]
fn la_forme_a_nom_unique_est_reconnue_comme_la_forme_a_liste() {
    let singular = format!(
        r#"{{
  "defaultAction": "SCMP_ACT_ALLOW",
  "syscalls": [{}]
}}"#,
        MUST_DENY
            .map(|name| format!(r#"{{ "name": "{name}", "action": "SCMP_ACT_ERRNO" }}"#))
            .join(", ")
    );
    RestrictedProfile::parse("/etc/locus/singulier.json", &singular)
        .expect("les deux formes du schéma OCI sont acceptées");
}

#[test]
fn un_json_illisible_est_refuse_et_le_dit() {
    match RestrictedProfile::parse("/etc/locus/tronque.json", "{ \"defaultAction\": ") {
        Err(ProfileError::Unreadable { path, detail }) => {
            assert_eq!(path, "/etc/locus/tronque.json");
            assert!(!detail.trim().is_empty());
        }
        other => panic!("un profil illisible n'est pas un profil permissif : {other:?}"),
    }
}

#[test]
fn un_profil_absent_du_disque_est_refuse_sans_paniquer() {
    let missing = std::env::temp_dir().join("locus-w4d4-inexistant.json");
    let _ = fs::remove_file(&missing);
    assert!(matches!(
        RestrictedProfile::read(&missing),
        Err(ProfileError::Unreadable { .. })
    ));
}

#[test]
fn un_profil_lu_sur_le_disque_porte_son_chemin() {
    let path = std::env::temp_dir().join("locus-w4d4-restricted.json");
    fs::write(&path, DEFAULT_DENY).expect("fichier écrit");
    let profile = RestrictedProfile::read(&path).expect("profil valide");
    assert_eq!(profile.path(), path.display().to_string());
}

/// `clone` est délibérément hors de la liste : un profil peut l'autoriser tout en refusant
/// `CLONE_NEWUSER` par un filtre d'argument, et tout programme à threads en a besoin. Vérifier les
/// filtres d'arguments demanderait un second interpréteur, c'est-à-dire un second endroit où se
/// tromper — le constat est ici pour que personne ne « complète » la liste sans le savoir.
#[test]
fn clone_n_est_pas_dans_la_liste_et_c_est_delibere() {
    assert!(!MUST_DENY.contains(&"clone"));
    assert!(!MUST_DENY.contains(&"clone3"));
    RestrictedProfile::parse("/etc/locus/threads.json", DEFAULT_DENY)
        .expect("un profil qui autorise clone reste acceptable");
}
