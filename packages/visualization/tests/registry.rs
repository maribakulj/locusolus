//! Test de sortie de W9.b — **l'artefact suggère, le client choisit.**
//!
//! §23.5 : « un artefact déclare des hints mais le client choisit le meilleur viewer disponible. »
//! Invariant 10 : « xiiif n'est pas requis par les agents. »
//!
//! Les deux tiennent par une seule règle : **la capacité admet, la suggestion ordonne.** Un hint ne
//! peut que reclasser des viewers qui savent déjà rendre le media type. Si une suggestion pouvait
//! *admettre*, un producteur d'artefacts déciderait à distance de ce qui s'ouvre chez un lecteur —
//! et « le client choisit » serait faux.

use locus_visualization::{ArtifactViewerRegistry, Choice, RegistryError, Viewer, ViewerRequest};

fn viewer(id: &str, media_types: &[&str], hints: &[&str]) -> Viewer {
    Viewer::declare(id, media_types, hints).expect("déclaration valide")
}

fn request(media_type: &str, hints: &[&str]) -> ViewerRequest {
    ViewerRequest {
        media_type: media_type.to_owned(),
        hints: hints.iter().map(|hint| (*hint).to_owned()).collect(),
    }
}

fn registry(viewers: Vec<Viewer>) -> ArtifactViewerRegistry {
    viewers
        .into_iter()
        .try_fold(ArtifactViewerRegistry::new(), ArtifactViewerRegistry::with)
        .expect("registre valide")
}

// ---------------------------------------------------------------------------------------------
// La capacité admet, la suggestion ordonne
// ---------------------------------------------------------------------------------------------

/// Le cœur de W9.b, et la traduction exécutable de l'invariant 10. L'artefact réclame `iiif` ;
/// le viewer IIIF est bien déclaré, mais il ne sait pas rendre ce media type. La suggestion est
/// donc ignorée — pas honorée « au mieux » — et le client ouvre ce qui sait rendre.
#[test]
fn une_suggestion_ne_fait_jamais_entrer_un_viewer_incapable() {
    let registre = registry(vec![
        viewer("native-image", &["image/jpeg"], &["image"]),
        viewer("xiiif", &["application/ld+json"], &["iiif"]),
    ]);

    assert_eq!(
        registre.choose(&request("image/jpeg", &["iiif"])),
        Choice::Fallback {
            viewer: "native-image".to_owned()
        }
    );
}

/// Et la moitié positive de la même règle : parmi ceux qui savent rendre, la suggestion décide.
#[test]
fn parmi_ceux_qui_savent_rendre_la_suggestion_decide() {
    let registre = registry(vec![
        viewer("native-image", &["image/jpeg"], &["image"]),
        viewer("web-tiles", &["image/jpeg"], &["deep-zoom"]),
    ]);

    assert_eq!(
        registre.choose(&request("image/jpeg", &["deep-zoom"])),
        Choice::Honoured {
            viewer: "web-tiles".to_owned(),
            hint: "deep-zoom".to_owned()
        }
    );
}

/// Les suggestions sont ordonnées : l'artefact dit sa préférence, et la première qu'un viewer
/// capable sait honorer gagne. Sans cet ordre, deux hints tous deux honorables seraient départagés
/// par l'ordre d'itération du registre, donc par un détail.
#[test]
fn les_suggestions_sont_prises_dans_l_ordre_de_l_artefact() {
    let registre = registry(vec![
        viewer("mirador", &["application/ld+json"], &["iiif"]),
        viewer("brut", &["application/ld+json"], &["json"]),
    ]);

    assert_eq!(
        registre.choose(&request("application/ld+json", &["json", "iiif"])),
        Choice::Honoured {
            viewer: "brut".to_owned(),
            hint: "json".to_owned()
        }
    );
    assert_eq!(
        registre.choose(&request("application/ld+json", &["iiif", "json"])),
        Choice::Honoured {
            viewer: "mirador".to_owned(),
            hint: "iiif".to_owned()
        }
    );
}

/// À suggestion égale, c'est l'ordre de déclaration qui tranche — la préférence du client, l'autre
/// moitié de « le client choisit ».
#[test]
fn a_suggestion_egale_la_preference_du_client_tranche() {
    let dabord_natif = registry(vec![
        viewer("native-image", &["image/jpeg"], &["image"]),
        viewer("externe", &["image/jpeg"], &["image"]),
    ]);
    let dabord_externe = registry(vec![
        viewer("externe", &["image/jpeg"], &["image"]),
        viewer("native-image", &["image/jpeg"], &["image"]),
    ]);

    assert_eq!(
        dabord_natif.choose(&request("image/jpeg", &["image"])),
        Choice::Honoured {
            viewer: "native-image".to_owned(),
            hint: "image".to_owned()
        }
    );
    assert_eq!(
        dabord_externe.choose(&request("image/jpeg", &["image"])),
        Choice::Honoured {
            viewer: "externe".to_owned(),
            hint: "image".to_owned()
        }
    );
}

/// Et sur le chemin du repli aussi. Le test précédent n'éprouvait que la branche où une suggestion
/// est honorée ; l'ordre de déclaration décide **également** quand aucune ne l'est, et une mutation
/// qui prenait le dernier candidat y survivait sans rien casser.
#[test]
fn sans_suggestion_honorable_la_preference_du_client_tranche_aussi() {
    let registre = registry(vec![
        viewer("native-image", &["image/jpeg"], &["image"]),
        viewer("externe", &["image/jpeg"], &["externe"]),
    ]);

    assert_eq!(
        registre.choose(&request("image/jpeg", &["holo-deck"])),
        Choice::Fallback {
            viewer: "native-image".to_owned()
        }
    );
    assert_eq!(
        registre.choose(&request("image/jpeg", &[])),
        Choice::Fallback {
            viewer: "native-image".to_owned()
        }
    );
}

// ---------------------------------------------------------------------------------------------
// Choisir ne peut pas échouer
// ---------------------------------------------------------------------------------------------

/// Une suggestion que personne ne connaît se replie : elle n'annule pas le rendu, et elle ne
/// produit pas d'erreur. C'est ce qui permet à un producteur d'artefacts d'écrire un hint que ce
/// client-ci ne connaît pas encore sans casser sa lecture.
#[test]
fn un_hint_inconnu_se_replie_sans_echouer() {
    let registre = registry(vec![viewer("native-image", &["image/jpeg"], &["image"])]);

    assert_eq!(
        registre.choose(&request("image/jpeg", &["holo-deck"])),
        Choice::Fallback {
            viewer: "native-image".to_owned()
        }
    );
}

/// Et l'absence totale de viewer laisse l'artefact atteignable. `choose` ne rend pas de `Result` :
/// un artefact qu'aucun viewer ne sait rendre se télécharge, il ne « plante » pas. Le media type
/// voyage avec le refus pour que l'appelant puisse encore proposer quelque chose.
#[test]
fn sans_aucun_viewer_l_artefact_reste_atteignable() {
    assert_eq!(
        ArtifactViewerRegistry::new().choose(&request("model/gltf+json", &["gltf"])),
        Choice::NoViewer {
            media_type: "model/gltf+json".to_owned()
        }
    );
}

/// Un registre garni mais sans rien pour ce media type : même réponse. Ce n'est pas le vide du
/// registre qui compte, c'est l'absence de capacité.
#[test]
fn un_media_type_que_personne_ne_rend_est_declare_tel_quel() {
    let registre = registry(vec![viewer("native-image", &["image/jpeg"], &["image"])]);
    assert_eq!(
        registre.choose(&request("chemical/x-pdb", &["molecule"])),
        Choice::NoViewer {
            media_type: "chemical/x-pdb".to_owned()
        }
    );
}

/// Un artefact sans aucune suggestion se rend quand même : les hints sont facultatifs, comme §23.5
/// le dit et comme l'invariant 10 l'exige.
#[test]
fn un_artefact_sans_suggestion_se_rend_quand_meme() {
    let registre = registry(vec![viewer("native-image", &["image/jpeg"], &["image"])]);
    assert_eq!(
        registre.choose(&request("image/jpeg", &[])),
        Choice::Fallback {
            viewer: "native-image".to_owned()
        }
    );
}

// ---------------------------------------------------------------------------------------------
// Ce qu'un viewer déclare
// ---------------------------------------------------------------------------------------------

#[test]
fn un_joker_de_famille_couvre_sa_famille_et_pas_une_autre() {
    let tout_image = viewer("native-image", &["image/*"], &["image"]);
    assert!(tout_image.handles("image/jpeg"));
    assert!(tout_image.handles("image/svg+xml"));
    assert!(!tout_image.handles("model/gltf+json"));
    // Et pas un préfixe qui ressemble : `imagerie/x` n'est pas de la famille `image`.
    assert!(!tout_image.handles("imagerie/x"));
}

#[test]
fn un_viewer_sans_identite_est_refuse() {
    assert_eq!(
        Viewer::declare("  ", &["image/jpeg"], &["image"]),
        Err(RegistryError::EmptyField { field: "viewer.id" })
    );
    assert_eq!(
        Viewer::declare("v", &["  "], &["image"]),
        Err(RegistryError::EmptyField {
            field: "viewer.media_type"
        })
    );
    assert_eq!(
        Viewer::declare("v", &["image/jpeg"], &[" "]),
        Err(RegistryError::EmptyField {
            field: "viewer.hint"
        })
    );
}

/// Deux viewers du même nom rendraient le choix dépendant de l'ordre d'itération plutôt que de la
/// préférence déclarée — et un client qui remplace un viewer croirait l'avoir remplacé.
#[test]
fn deux_viewers_de_meme_identite_sont_refuses() {
    assert_eq!(
        ArtifactViewerRegistry::new()
            .with(viewer("v", &["image/jpeg"], &[]))
            .expect("premier")
            .with(viewer("v", &["text/html"], &[])),
        Err(RegistryError::DuplicateViewer { id: "v".to_owned() })
    );
}

// ---------------------------------------------------------------------------------------------
// La table de `docs/07` est exécutable
// ---------------------------------------------------------------------------------------------

/// Une table de routage que rien n'exécute se désaccorde du code sans que personne ne le voie.
/// Les dix familles de `docs/07` sont donc parcourues ici, chacune par un media type réel.
#[test]
fn le_registre_de_reference_route_les_dix_familles() {
    let registre = ArtifactViewerRegistry::reference();
    let cas: [(&str, &str, &str); 10] = [
        ("text/markdown", "text", "emacs-native"),
        ("image/png", "image", "native-image"),
        ("text/html", "html", "webview"),
        ("application/ld+json", "iiif", "xiiif"),
        ("application/vnd.locus.graph+json", "graph", "web-graph"),
        ("model/gltf-binary", "gltf", "three-js"),
        ("application/vnd.laszip", "point-cloud", "potree"),
        ("chemical/x-pdb", "molecule", "mol-star"),
        ("application/vnd.vtk", "volume", "vtk-js"),
        ("application/x-ipynb+json", "notebook", "jupyter"),
    ];

    for (media_type, hint, attendu) in cas {
        assert_eq!(
            registre.choose(&request(media_type, &[hint])),
            Choice::Honoured {
                viewer: attendu.to_owned(),
                hint: hint.to_owned()
            },
            "{media_type} devrait aller à {attendu}"
        );
    }
    assert_eq!(registre.identities().len(), 10);
}

/// Le registre de référence n'est pas une obligation : un client qui n'a que le strict nécessaire
/// reste un client. C'est l'invariant 10 vu du côté du déploiement — un poste sans xiiif fonctionne.
#[test]
fn un_client_sans_xiiif_lit_quand_meme_ses_artefacts() {
    let registre = registry(vec![viewer("brut", &["application/ld+json"], &["json"])]);
    assert!(!registre.identities().contains("xiiif"));
    assert_eq!(
        registre.choose(&request("application/ld+json", &["iiif"])),
        Choice::Fallback {
            viewer: "brut".to_owned()
        }
    );
}

/// La table de `docs/07` est une table de **routage** : deux viewers qui revendiqueraient le même
/// media type rendraient la destination dépendante de l'ordre de déclaration, donc d'un détail
/// qu'aucune ligne du document ne mentionne. Un viewer qui s'étend sur le territoire d'un autre
/// resterait invisible tant que celui-ci serait déclaré en premier — c'est exactement le mutant qui
/// avait survécu au premier passage.
#[test]
fn dans_le_registre_de_reference_chaque_media_type_a_un_seul_viewer() {
    let registre = ArtifactViewerRegistry::reference();
    let media_types = [
        "text/markdown",
        "text/org",
        "image/png",
        "image/jpeg",
        "image/svg+xml",
        "application/pdf",
        "text/html",
        "application/ld+json",
        "application/json",
        "application/vnd.locus.graph+json",
        "model/gltf+json",
        "model/gltf-binary",
        "application/vnd.laszip",
        "chemical/x-mmcif",
        "chemical/x-pdb",
        "application/vnd.vtk",
        "application/x-ipynb+json",
    ];

    for media_type in media_types {
        let capables: Vec<&str> = registre
            .viewers()
            .iter()
            .filter(|viewer| viewer.handles(media_type))
            .map(locus_visualization::Viewer::id)
            .collect();
        assert_eq!(
            capables.len(),
            1,
            "{media_type} est revendiqué par {capables:?}"
        );
    }
}
