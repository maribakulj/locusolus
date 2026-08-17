//! « Migration aller-retour » — le test de sortie de W1.h.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use locus_migrations::{
    Loss, Migration, MigrationChain, MigrationError, PROVIDER_MARKERS, provider_findings,
};

/// v1 → v2 : `author` devient `created_by`. Renommage pur, donc réversible.
fn rename_author() -> Migration {
    Migration::reversible(
        1,
        2,
        "`author` devient `created_by`",
        |document| {
            let mut object =
                document
                    .as_object()
                    .cloned()
                    .ok_or_else(|| MigrationError::Malformed {
                        reason: "document non objet".to_owned(),
                    })?;
            let author = object
                .remove("author")
                .ok_or_else(|| MigrationError::Malformed {
                    reason: "`author` absent".to_owned(),
                })?;
            object.insert("created_by".to_owned(), author);
            Ok(Value::Object(object))
        },
        |document| {
            let mut object =
                document
                    .as_object()
                    .cloned()
                    .ok_or_else(|| MigrationError::Malformed {
                        reason: "document non objet".to_owned(),
                    })?;
            let created_by =
                object
                    .remove("created_by")
                    .ok_or_else(|| MigrationError::Malformed {
                        reason: "`created_by` absent".to_owned(),
                    })?;
            object.insert("author".to_owned(), created_by);
            Ok(Value::Object(object))
        },
    )
}

/// v2 → v3 : ajout d'un champ à valeur par défaut. Réversible en le retirant.
fn add_scope() -> Migration {
    Migration::reversible(
        2,
        3,
        "ajout de `scope`, par défaut `global`",
        |document| {
            let mut object =
                document
                    .as_object()
                    .cloned()
                    .ok_or_else(|| MigrationError::Malformed {
                        reason: "document non objet".to_owned(),
                    })?;
            object
                .entry("scope".to_owned())
                .or_insert_with(|| json!("global"));
            Ok(Value::Object(object))
        },
        |document| {
            let mut object =
                document
                    .as_object()
                    .cloned()
                    .ok_or_else(|| MigrationError::Malformed {
                        reason: "document non objet".to_owned(),
                    })?;
            object.remove("scope");
            Ok(Value::Object(object))
        },
    )
}

/// v3 → v4 : deux champs de confiance fusionnent en un. **Destructif** : la moyenne ne se
/// désagrège pas, et prétendre le contraire produirait deux valeurs inventées.
fn merge_confidence() -> Migration {
    Migration::lossy(
        3,
        4,
        "`confidence_low` et `confidence_high` fusionnent en `confidence`",
        |document| {
            let mut object =
                document
                    .as_object()
                    .cloned()
                    .ok_or_else(|| MigrationError::Malformed {
                        reason: "document non objet".to_owned(),
                    })?;
            let low = object
                .remove("confidence_low")
                .and_then(|value| value.as_f64())
                .unwrap_or(0.0);
            let high = object
                .remove("confidence_high")
                .and_then(|value| value.as_f64())
                .unwrap_or(0.0);
            object.insert("confidence".to_owned(), json!(f64::midpoint(low, high)));
            Ok(Value::Object(object))
        },
        Loss {
            fields: vec!["confidence_low".to_owned(), "confidence_high".to_owned()],
            rationale: "une moyenne ne se désagrège pas : redescendre inventerait deux bornes"
                .to_owned(),
        },
    )
}

fn reversible_chain() -> MigrationChain {
    MigrationChain::new()
        .with(rename_author())
        .with(add_scope())
}

fn full_chain() -> MigrationChain {
    reversible_chain().with(merge_confidence())
}

fn document_v1() -> Value {
    json!({
        "statement": "le solvant n'explique pas l'écart",
        "author": "agent_1",
        "confidence_low": 0.4,
        "confidence_high": 0.8,
    })
}

// ————————————————————————— Le test de sortie de W1.h —————————————————————————

#[test]
fn a_reversible_migration_round_trips_exactly() {
    // « Migration aller-retour » : monter puis redescendre rend **exactement** le document
    // d'origine. Pas « équivalent », pas « à un champ près » — exactement, sinon un replay produit
    // un document que le producteur n'a jamais écrit.
    let chain = reversible_chain();
    let original = document_v1();

    let up = chain.upcast(&original, 1, 3).expect("montée");
    assert_eq!(up["created_by"], json!("agent_1"));
    assert_eq!(up["scope"], json!("global"));
    assert!(up.get("author").is_none());

    let back = chain.downcast(&up, 3, 1).expect("descente");
    assert_eq!(back, original, "l'aller-retour n'a pas rendu l'original");

    // Et à chaque palier intermédiaire, pas seulement d'un bout à l'autre.
    for step in 1..=3 {
        let intermediate = chain.upcast(&original, 1, step).expect("montée partielle");
        let returned = chain
            .downcast(&intermediate, step, 1)
            .expect("descente partielle");
        assert_eq!(returned, original, "aller-retour rompu au palier v{step}");
    }
}

#[test]
fn an_irreversible_step_refuses_to_come_back_and_says_what_it_lost() {
    // Le cœur de W1.h. Une chaîne qui redescendrait à travers une étape destructive rendrait un
    // document ancien **qui n'a jamais existé**, et il aurait l'air authentique.
    let chain = full_chain();
    let original = document_v1();

    let up = chain.upcast(&original, 1, 4).expect("montée");
    assert_eq!(up["confidence"], json!(f64::midpoint(0.4, 0.8)));
    assert!(up.get("confidence_low").is_none());

    let refusal = chain.downcast(&up, 4, 1).unwrap_err();
    match &refusal {
        MigrationError::Irreversible {
            from,
            to,
            lost,
            rationale,
        } => {
            assert_eq!((*from, *to), (3, 4));
            assert_eq!(
                lost,
                &vec!["confidence_low".to_owned(), "confidence_high".to_owned()]
            );
            assert!(rationale.contains("moyenne"));
        }
        other => panic!("refus inattendu : {other}"),
    }
    // Le message dit ce qu'il faut savoir sans ouvrir le code.
    assert!(refusal.to_string().contains("confidence_low"));

    // Et la partie réversible reste redescendable : l'irréversibilité s'arrête où elle commence.
    let at_v3 = chain.upcast(&original, 1, 3).expect("montée");
    assert_eq!(chain.downcast(&at_v3, 3, 1).expect("descente"), original);
}

#[test]
fn the_irreversible_steps_can_be_asked_about_before_trying() {
    // Une migration destructive n'est pas une faute : c'est parfois la seule façon d'avancer. Ce
    // qui est une faute, c'est de le découvrir au moment où l'on avait besoin de redescendre.
    let chain = full_chain();
    assert!(chain.irreversible_between(1, 3).is_empty());
    let blocking = chain.irreversible_between(1, 4);
    assert_eq!(blocking.len(), 1);
    assert_eq!(blocking[0].from(), 3);
    assert!(!blocking[0].is_reversible());
    assert_eq!(blocking[0].loss().map(|loss| loss.fields.len()), Some(2));
}

#[test]
fn upcasting_to_current_walks_the_whole_chain() {
    let chain = full_chain();
    assert_eq!(chain.oldest(), Some(1));
    assert_eq!(chain.current(), Some(4));
    let up = chain
        .upcast_to_current(&document_v1(), 1)
        .expect("montée jusqu'à la version courante");
    assert!(up.get("confidence").is_some());
    assert!(up.get("author").is_none());
}

#[test]
fn a_version_the_chain_does_not_cover_is_refused_not_guessed() {
    let chain = full_chain();
    // Une version antérieure à ce que la chaîne sait lire.
    assert_eq!(
        chain.upcast(&document_v1(), 0, 4).unwrap_err(),
        MigrationError::NoPath { from: 0, to: 4 }
    );
    // Et une montée demandée à l'envers : remonter le temps se demande par `downcast`, et
    // confondre les deux ferait appliquer un `up` là où un `down` était voulu.
    assert_eq!(
        chain.upcast(&document_v1(), 3, 1).unwrap_err(),
        MigrationError::NoPath { from: 3, to: 1 }
    );
    assert_eq!(
        chain.downcast(&document_v1(), 1, 3).unwrap_err(),
        MigrationError::NoPath { from: 1, to: 3 }
    );
}

#[test]
fn migrating_to_the_same_version_is_the_identity() {
    let chain = full_chain();
    let original = document_v1();
    assert_eq!(chain.upcast(&original, 2, 2).expect("identité"), original);
    assert_eq!(chain.downcast(&original, 2, 2).expect("identité"), original);
}

#[test]
fn the_chain_covers_the_minimum_window_of_the_text() {
    // §10.4 : « les producteurs supportent au minimum la version courante **et la version
    // précédente** ». Une chaîne qui ne saurait lire que la version courante refuserait du jour au
    // lendemain un producteur qui n'a pas encore migré.
    assert!(full_chain().covers_minimum_window());
    assert!(reversible_chain().covers_minimum_window());
    assert!(!MigrationChain::new().covers_minimum_window());
}

#[test]
fn a_malformed_document_is_refused_rather_than_repaired() {
    // Une migration qui compléterait un champ manquant produirait un document v2 dont la valeur
    // n'a jamais été écrite par personne.
    let chain = reversible_chain();
    let without_author = json!({ "statement": "x" });
    match chain.upcast(&without_author, 1, 2).unwrap_err() {
        MigrationError::Malformed { reason } => assert!(reason.contains("author")),
        other => panic!("refus inattendu : {other}"),
    }
    assert!(chain.upcast(&json!("pas un objet"), 1, 2).is_err());
}

// ————————————————————————— Portabilité — §4.1 —————————————————————————

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("packages/migrations vit deux niveaux sous la racine")
        .to_path_buf()
}

fn sources(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == "target" || name == "node_modules" || name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            sources(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
}

#[test]
fn no_domain_object_names_an_infrastructure_provider() {
    // §4.1 : « aucun objet `Project`, `Branch`, `Claim`, `Review`, `Task` ou `Artifact` ne doit
    // dépendre d'un fournisseur d'infrastructure ».
    //
    // `boundaries.json` vérifie les **imports** ; il ne voit ni les noms de champ ni les
    // littéraux. Un `Claim` qui porterait `s3_bucket` ne violerait aucune règle d'import et
    // rendrait pourtant l'objet indéplaçable — c'est ce trou que ce test couvre.
    let root = workspace_root();
    let mut files = Vec::new();
    for package in ["domain", "graph", "event-store", "validation"] {
        sources(&root.join("packages").join(package), &mut files);
    }
    assert!(
        files.len() > 10,
        "le balayage n'a trouvé que {} fichiers",
        files.len()
    );

    let mut findings = Vec::new();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        let location = file
            .strip_prefix(&root)
            .unwrap_or(file)
            .display()
            .to_string();
        findings.extend(provider_findings(&location, &text));
    }
    assert!(
        findings.is_empty(),
        "un objet de domaine nomme un fournisseur :\n{}",
        findings
            .iter()
            .map(|finding| format!(
                "{} — `{}` : {}",
                finding.location, finding.marker, finding.line
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn the_portability_scan_catches_what_it_is_for() {
    // Un filet qui n'attrape rien n'est pas un filet : sans ce test, le précédent passerait aussi
    // sur une fonction qui rendrait toujours la liste vide.
    let bait = "pub struct Claim {\n    pub s3_bucket: String,\n}";
    let caught = provider_findings("fixture", bait);
    assert_eq!(caught.len(), 1);
    assert_eq!(caught[0].marker, "s3_");

    // Et un commentaire qui **nomme** un fournisseur pour dire qu'on n'en dépend pas n'est pas une
    // dépendance : l'inverse ferait échouer la garde sur sa propre documentation.
    let comment = "// aucun `s3_bucket` ici, et c'est le point";
    assert!(provider_findings("fixture", comment).is_empty());
    assert!(PROVIDER_MARKERS.len() > 5);
}
