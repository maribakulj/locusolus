//! Test de sortie de W11.b — **les secrets sont dehors, et il n'y a pas d'endroit où les mettre.**
//!
//! `docs/05` : « les secrets sont externes ». Ce n'est pas une consigne d'hygiène : c'est une
//! propriété du format. Le document n'offre aucun champ où écrire une valeur, et le motif du schéma
//! refuse ce qui n'est pas une référence.
//!
//! La raison est qu'un secret écrit dans un fichier de configuration ne s'arrête pas là. Il part
//! dans un dépôt, dans une sauvegarde, dans un rapport de bug, dans le presse-papier de qui
//! diagnostique — et aucune de ces copies ne se révoque.

use std::fs;
use std::path::PathBuf;

use locus_deployment::{ConfigError, DeploymentConfig, ProfileError, ProfileKind, SecretScheme};

fn wire() -> locus_lep::Deployment {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "schemas",
        "examples",
        "deployment-single-node-vm.json",
    ]
    .iter()
    .collect();
    let text = fs::read_to_string(&path).expect("exemple lisible");
    // L'exemple porte son bloc `_fixture`, que le schéma ne connaît pas : le harnais le retire
    // avant de valider, et ce test fait pareil.
    let mut document: serde_json::Value = serde_json::from_str(&text).expect("JSON valide");
    document.as_object_mut().expect("objet").remove("_fixture");
    serde_json::from_value(document).expect("document conforme")
}

// ---------------------------------------------------------------------------------------------
// Les secrets sont dehors
// ---------------------------------------------------------------------------------------------

#[test]
fn un_document_bien_forme_se_relit() {
    let config = DeploymentConfig::from_wire(&wire()).expect("document valide");
    assert_eq!(config.profile().kind(), ProfileKind::SingleNodeVm);
    assert_eq!(config.adapters().len(), 4);
    assert_eq!(config.secrets().len(), 2);
    assert_eq!(config.secrets()[0].scheme(), SecretScheme::Env);
    assert_eq!(config.secrets()[0].reference(), "env:LOCUS_PGPASSWORD");
}

/// Le cœur de W11.b. `explain` dit où le déploiement va chercher son mot de passe — c'est du
/// diagnostic — et ne le résout jamais. Une commande qui résoudrait « pour aider » afficherait le
/// secret sur le terminal de qui la lance, et dans le journal de session qui va avec.
#[test]
fn explain_dit_ou_chercher_le_secret_jamais_ce_qu_il_vaut() {
    let rendu = DeploymentConfig::from_wire(&wire())
        .expect("document valide")
        .explain();

    assert!(
        rendu.contains("secret event-store.password ← env:LOCUS_PGPASSWORD"),
        "{rendu}"
    );
    // Et rien qui ressemble à une valeur : le seul texte qui suit la flèche est la référence.
    for ligne in rendu.lines().filter(|ligne| ligne.starts_with("secret ")) {
        let apres = ligne.split('←').nth(1).expect("une flèche").trim();
        assert!(
            SecretScheme::ALL
                .iter()
                .any(|scheme| apres.starts_with(&format!("{scheme}:"))),
            "« {apres} » n'est pas une référence"
        );
    }
}

/// Une valeur en clair là où une référence est attendue. Le schéma la refuse par motif ; le domaine
/// la refuse aussi, parce qu'un producteur qui construit la valeur en mémoire ne passe par aucun
/// validateur JSON.
#[test]
fn un_secret_en_clair_est_refuse_par_le_domaine_aussi() {
    let mut document = wire();
    document.secret_refs = Some(vec![locus_lep::DeploymentSecretRefsItem {
        name: "event-store.password".to_owned(),
        reference: "hunter2".to_owned(),
    }]);
    assert_eq!(
        DeploymentConfig::from_wire(&document),
        Err(ConfigError::MalformedSecretRef {
            value: "hunter2".to_owned()
        })
    );
}

/// Un schéma que rien ne sait suivre est refusé plutôt qu'accepté en espérant : `s3://` ressemble à
/// une référence et n'en est pas une ici, et l'accepter ferait échouer le déploiement au premier
/// démarrage plutôt qu'à la lecture.
#[test]
fn un_schema_de_secret_inconnu_est_refuse() {
    let mut document = wire();
    document.secret_refs = Some(vec![locus_lep::DeploymentSecretRefsItem {
        name: "x".to_owned(),
        reference: "s3:bucket/key".to_owned(),
    }]);
    assert!(matches!(
        DeploymentConfig::from_wire(&document),
        Err(ConfigError::MalformedSecretRef { .. })
    ));
}

#[test]
fn une_reference_sans_cible_est_refusee() {
    let mut document = wire();
    document.secret_refs = Some(vec![locus_lep::DeploymentSecretRefsItem {
        name: "x".to_owned(),
        reference: "env:   ".to_owned(),
    }]);
    assert!(matches!(
        DeploymentConfig::from_wire(&document),
        Err(ConfigError::MalformedSecretRef { .. })
    ));
}

#[test]
fn un_secret_sans_nom_est_refuse() {
    let mut document = wire();
    document.secret_refs = Some(vec![locus_lep::DeploymentSecretRefsItem {
        name: "  ".to_owned(),
        reference: "env:X".to_owned(),
    }]);
    assert_eq!(
        DeploymentConfig::from_wire(&document),
        Err(ConfigError::Profile(ProfileError::EmptyField {
            field: "secret.name"
        }))
    );
}

#[test]
fn les_quatre_schemas_de_secret_existent_sous_leur_nom() {
    let slugs: Vec<&str> = SecretScheme::ALL.iter().map(|s| s.slug()).collect();
    assert_eq!(slugs, vec!["env", "file", "keychain", "vault"]);
    for scheme in SecretScheme::ALL {
        assert_eq!(SecretScheme::from_slug(scheme.slug()), Some(scheme));
    }
    assert_eq!(SecretScheme::from_slug("literal"), None);
}

// ---------------------------------------------------------------------------------------------
// Ce que le schéma ne peut pas dire
// ---------------------------------------------------------------------------------------------

/// La liste d'adaptateurs est une liste **pour que ce soit détectable**. Un objet JSON aurait laissé
/// le second écraser le premier sans bruit, et personne n'aurait su lequel des deux backends était
/// actif — la question que `explain` existe pour trancher.
#[test]
fn un_role_declare_deux_fois_est_refuse() {
    let mut document = wire();
    document.adapters.push(locus_lep::DeploymentAdaptersItem {
        role: "workflow".to_owned(),
        implementation: "temporal-cloud".to_owned(),
    });
    assert_eq!(
        DeploymentConfig::from_wire(&document),
        Err(ConfigError::DuplicateRole {
            role: "workflow".to_owned()
        })
    );
}

/// Le type engendré ne porte l'énumération que comme une chaîne : c'est au domaine de refuser un
/// sixième profil, sans quoi `--profile kubernetes` construirait un déploiement que rien ne sait
/// vérifier.
#[test]
fn un_profil_hors_des_cinq_est_refuse() {
    let mut document = wire();
    document.profile = "kubernetes".to_owned();
    assert_eq!(
        DeploymentConfig::from_wire(&document),
        Err(ConfigError::UnknownProfile {
            value: "kubernetes".to_owned()
        })
    );
}

// ---------------------------------------------------------------------------------------------
// `locus deployment explain`
// ---------------------------------------------------------------------------------------------

/// §27.2 : « affiche **exactement** quels backends sont actifs ». Tous les rôles déclarés, et aucun
/// autre — un backend oublié à l'écran est un backend qu'on croit absent.
#[test]
fn explain_nomme_exactement_les_backends_actifs() {
    let config = DeploymentConfig::from_wire(&wire()).expect("document valide");
    let rendu = config.explain();

    let annonces: Vec<&str> = rendu
        .lines()
        .filter_map(|ligne| ligne.strip_prefix("backend "))
        .collect();
    assert_eq!(annonces.len(), config.adapters().len());

    for (role, implementation) in config.adapters() {
        assert!(
            annonces.contains(&format!("{role} : {implementation}").as_str()),
            "{role} manque : {rendu}"
        );
    }
}

#[test]
fn explain_nomme_le_profil() {
    let rendu = DeploymentConfig::from_wire(&wire())
        .expect("document valide")
        .explain();
    assert!(rendu.starts_with("profil : single-node-vm\n"), "{rendu}");
}

/// Les capabilities déclarées arrivent au profil, donc au client : §27.1 demande qu'une limite soit
/// déclarée plutôt que contournée, et une limite que le document énonce sans que personne ne la
/// porte n'est pas déclarée.
#[test]
fn les_capabilites_du_document_arrivent_au_profil() {
    let surface = DeploymentConfig::from_wire(&wire())
        .expect("document valide")
        .profile()
        .client_surface();
    let annoncees: Vec<&str> = surface.capabilities.iter().map(String::as_str).collect();
    assert_eq!(annoncees, vec!["cpu", "no-gpu"]);
}
