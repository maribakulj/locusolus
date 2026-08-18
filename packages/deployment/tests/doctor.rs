//! Test de sortie de W11.a — **un profil ne se déclare pas exécutable, il est vérifié.**
//!
//! §27.2 : « `locus doctor` **vérifie** dépendances, ports, versions, ressources, attestations et
//! accès. » `docs/05` : « `locus doctor` vérifie que le profil est réellement exécutable **avant
//! d'accepter des campagnes**. »
//!
//! La faute que ces phrases préviennent est courante et silencieuse : un fichier de configuration
//! qui énumère des adaptateurs, personne qui vérifie qu'ils sont là, et une campagne acceptée qui
//! échoue trois heures plus tard sur le premier appel. Le type l'empêche — `Profile` ne sait pas
//! répondre « suis-je exécutable », seul le croisement avec un inventaire le dit.

use std::collections::BTreeSet;

use locus_deployment::{Inventory, Presence, Profile, ProfileError, ProfileKind};

fn profil(kind: ProfileKind) -> Profile {
    Profile::declare(
        kind,
        "https://locus.example/api",
        &["postgres", "workflow", "object-store"],
    )
    .expect("profil valide")
}

fn tout_present() -> Inventory {
    Inventory::new()
        .observing("postgres", Presence::Present)
        .observing("workflow", Presence::Present)
        .observing("object-store", Presence::Present)
}

fn noms(set: &BTreeSet<String>) -> Vec<&str> {
    set.iter().map(String::as_str).collect()
}

// ---------------------------------------------------------------------------------------------
// Le verdict se constate
// ---------------------------------------------------------------------------------------------

#[test]
fn un_profil_dont_tout_est_constate_present_est_executable() {
    let verdict = profil(ProfileKind::SingleNodeVm).inspect(&tout_present());
    assert!(verdict.executable());
    assert!(verdict.missing().is_empty());
    assert!(verdict.unverified().is_empty());
}

/// Le cœur de W11.a. Un adaptateur déclaré mais absent rend le profil inexécutable — et le verdict
/// **nomme** ce qui manque, parce qu'un « non exécutable » sans raison ne se corrige pas.
#[test]
fn un_adaptateur_absent_rend_le_profil_inexecutable_et_le_nomme() {
    let inventaire = tout_present().observing("object-store", Presence::Absent);
    let verdict = profil(ProfileKind::SingleNodeVm).inspect(&inventaire);

    assert!(!verdict.executable());
    assert_eq!(noms(verdict.missing()), vec!["object-store"]);
    assert!(verdict.to_string().contains("object-store"));
}

/// « Pas vérifié » n'est pas « présent ». Compter l'ignorance comme un succès ferait déclarer un
/// profil exécutable par une **panne de la sonde** — c'est-à-dire au moment précis où il ne faut
/// pas.
#[test]
fn ce_qu_aucune_sonde_n_a_constate_ne_compte_pas_comme_present() {
    let inventaire = tout_present().observing("workflow", Presence::Unknown);
    let verdict = profil(ProfileKind::SingleNodeVm).inspect(&inventaire);

    assert!(!verdict.executable());
    assert_eq!(noms(verdict.unverified()), vec!["workflow"]);
    assert!(verdict.missing().is_empty(), "inconnu n'est pas absent");
}

/// Un adaptateur dont l'inventaire ne parle pas du tout est inconnu, pas absent. Ne pas avoir
/// regardé et avoir regardé sans rien trouver appellent deux gestes différents : sonder, ou
/// installer.
#[test]
fn un_adaptateur_dont_personne_n_a_parle_est_inconnu_pas_absent() {
    let verdict = profil(ProfileKind::SingleNodeVm).inspect(&Inventory::new());
    assert!(!verdict.executable());
    assert_eq!(
        noms(verdict.unverified()),
        vec!["object-store", "postgres", "workflow"]
    );
    assert!(verdict.missing().is_empty());
}

/// Les deux listes restent séparées jusqu'à l'impression. Les fondre en une seule ferait chercher
/// une installation là où il fallait réparer une sonde.
#[test]
fn absent_et_non_verifie_ne_se_confondent_pas_a_l_ecran() {
    let inventaire = tout_present()
        .observing("postgres", Presence::Absent)
        .observing("workflow", Presence::Unknown);
    let rendu = profil(ProfileKind::CloudPlatform)
        .inspect(&inventaire)
        .to_string();

    assert!(rendu.contains("absent : postgres"), "{rendu}");
    assert!(rendu.contains("non vérifié : workflow"), "{rendu}");
}

#[test]
fn un_profil_executable_le_dit_sans_liste() {
    let rendu = profil(ProfileKind::PersonalLocal)
        .inspect(&tout_present())
        .to_string();
    assert_eq!(rendu, "personal-local : exécutable");
}

// ---------------------------------------------------------------------------------------------
// Ce qu'un client voit
// ---------------------------------------------------------------------------------------------

/// `docs/05` : « les clients se connectent à une URL Locus. Ils ne connaissent pas la topologie
/// interne. » Deux profils aussi éloignés qu'un poste personnel et un hybride distribué exposent
/// donc la **même** valeur, à l'égalité près.
#[test]
fn deux_topologies_opposees_exposent_la_meme_surface_cliente() {
    let local = Profile::declare(
        ProfileKind::PersonalLocal,
        "https://locus.example/api",
        &["postgres-local", "workflow-local", "podman-machine"],
    )
    .expect("profil valide")
    .announcing("cpu");

    let hybride = Profile::declare(
        ProfileKind::DistributedHybrid,
        "https://locus.example/api",
        &[
            "postgres-cloud",
            "temporal-cloud",
            "runpod-gpu",
            "worker-on-prem",
        ],
    )
    .expect("profil valide")
    .announcing("cpu");

    assert_eq!(local.client_surface(), hybride.client_surface());
    assert_ne!(local.adapters(), hybride.adapters());
}

/// Et rien de la topologie ne filtre dans ce que le client reçoit — pas même le nom d'un
/// adaptateur, qui dirait déjà quel fournisseur est derrière.
#[test]
fn aucun_nom_d_adaptateur_ne_filtre_vers_le_client() {
    let profil = Profile::declare(
        ProfileKind::CloudPlatform,
        "https://locus.example/api",
        &["postgres-rds-interne", "temporal-cloud"],
    )
    .expect("profil valide");

    let surface = profil.client_surface();
    let rendu = format!("{surface:?}");
    for adaptateur in profil.adapters() {
        assert!(
            !rendu.contains(adaptateur.as_str()),
            "« {adaptateur} » a filtré vers le client : {rendu}"
        );
    }
}

/// Les capabilities, elles, sont faites pour être vues : §27.1 demande que les limites du
/// fournisseur soient déclarées plutôt que contournées, et un client qui les ignore demanderait
/// un GPU à un profil qui n'en a pas.
#[test]
fn les_capabilites_annoncees_arrivent_jusqu_au_client() {
    let surface = profil(ProfileKind::CloudPlatform)
        .announcing("cpu")
        .announcing("no-gpu")
        .client_surface();
    assert_eq!(noms(&surface.capabilities), vec!["cpu", "no-gpu"]);
}

// ---------------------------------------------------------------------------------------------
// Les cinq de §27.1
// ---------------------------------------------------------------------------------------------

#[test]
fn les_cinq_profils_existent_sous_leur_nom() {
    let slugs: Vec<&str> = ProfileKind::ALL.iter().map(|kind| kind.slug()).collect();
    assert_eq!(
        slugs,
        vec![
            "personal-local",
            "personal-node",
            "single-node-vm",
            "cloud-platform",
            "distributed-hybrid"
        ]
    );
    for kind in ProfileKind::ALL {
        assert_eq!(ProfileKind::from_slug(kind.slug()), Some(kind));
    }
    assert_eq!(ProfileKind::from_slug("kubernetes"), None);
}

/// Tous les profils passent la même vérification : §27.3 dit qu'ils « exposent la même API publique
/// et passent une suite de conformance commune ». Un profil dispensé serait un profil dont personne
/// ne sait s'il marche.
#[test]
fn aucun_profil_n_est_dispense_de_verification() {
    for kind in ProfileKind::ALL {
        let verdict = profil(kind).inspect(&Inventory::new());
        assert!(
            !verdict.executable(),
            "{kind} s'est déclaré exécutable sans inventaire"
        );
        assert_eq!(verdict.profile(), kind);
    }
}

// ---------------------------------------------------------------------------------------------
// Ce qu'un profil refuse d'être
// ---------------------------------------------------------------------------------------------

/// Un profil sans adaptateur passerait toute vérification sans rien avoir vérifié. C'est la façon
/// la plus discrète de rendre `locus doctor` inutile : la commande répondrait « exécutable », et
/// elle aurait raison.
#[test]
fn un_profil_sans_adaptateur_est_refuse() {
    assert_eq!(
        Profile::declare(ProfileKind::PersonalLocal, "https://locus.example", &[]),
        Err(ProfileError::NoAdapter)
    );
}

#[test]
fn une_url_ou_un_adaptateur_vide_est_refuse() {
    assert_eq!(
        Profile::declare(ProfileKind::PersonalLocal, "  ", &["postgres"]),
        Err(ProfileError::EmptyField { field: "endpoint" })
    );
    assert_eq!(
        Profile::declare(ProfileKind::PersonalLocal, "https://locus.example", &[" "]),
        Err(ProfileError::EmptyField { field: "adapter" })
    );
}
