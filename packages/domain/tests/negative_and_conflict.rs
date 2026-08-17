//! « Aucun chemin de code ne supprime un conflit » — le test de sortie de W1.g.

use std::path::{Path, PathBuf};

use locus_domain::{
    CONCLUSIVE_POWER, Conflict, ConflictLog, ConflictOrigin, Exclusion, NegativeResult, Power,
    RevisionId, Verdict, conflicts_from_merge,
};
use locus_protocol::Timestamp;

/// Un générateur congruentiel linéaire. Même choix que dans les paquets voisins.
struct Rng(u64);

impl Rng {
    const fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn revision(&mut self) -> RevisionId {
        let mut entropy = [0u8; 10];
        for byte in &mut entropy {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            *byte = u8::try_from(self.0 >> 56).unwrap_or(0);
        }
        RevisionId::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
            .expect("instant dans les bornes")
    }
}

fn conflict(id: &str, first: RevisionId, second: RevisionId) -> Conflict {
    Conflict {
        id: id.to_owned(),
        first,
        second,
        origin: ConflictOrigin::Merge,
        statement: "les deux mesures s'excluent".to_owned(),
        branch_id: "br_main".to_owned(),
        declared_at: "2026-08-17T09:00:00.000Z".to_owned(),
        verdict: Verdict::Unresolved,
    }
}

// ————————————————————————— Le test de sortie de W1.g —————————————————————————

/// La racine du workspace, remontée depuis le manifeste de ce crate.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("packages/domain vit deux niveaux sous la racine")
        .to_path_buf()
}

/// Tous les fichiers source du workspace, `target/` exclu.
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
fn no_code_path_anywhere_in_the_workspace_deletes_a_conflict() {
    // Invariant 12 : « les résultats négatifs et conflits ne sont jamais supprimés pour rendre le
    // graphe *propre* ».
    //
    // Le test balaie **tout le workspace**, pas seulement le module qui déclare la garantie : une
    // garantie qui ne tiendrait que chez elle n'en serait pas une. Le jour où un paquet voisin
    // ajoutera un `remove` sur des conflits, c'est ici que ça rougira.
    let root = workspace_root();
    let mut files = Vec::new();
    sources(&root, &mut files);
    assert!(
        files.len() > 15,
        "le balayage n'a trouvé que {} fichiers : il ne regarde pas au bon endroit",
        files.len()
    );

    // Les formes par lesquelles un conflit disparaîtrait. Cherchées sur la ligne, avec le mot
    // `conflict` à proximité : un `remove` sur une table quelconque n'est pas le sujet.
    let removals = [
        "remove", "retain", "drain", "clear", "prune", "forget", "purge", "delete", "truncate",
        "pop",
    ];

    let mut offenders = Vec::new();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        for (number, line) in text.lines().enumerate() {
            let lowered = line.to_lowercase();
            if !lowered.contains("conflict") {
                continue;
            }
            // Une ligne de commentaire qui **nomme** l'interdit n'est pas une violation : c'est la
            // documentation de l'interdit, et l'exclure ferait échouer le test sur sa propre
            // justification.
            let trimmed = lowered.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with('*') {
                continue;
            }
            for removal in removals {
                if lowered.contains(&format!(".{removal}("))
                    || lowered.contains(&format!("fn {removal}"))
                {
                    offenders.push(format!(
                        "{}:{} — {}",
                        file.strip_prefix(&root).unwrap_or(file).display(),
                        number + 1,
                        line.trim()
                    ));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "un chemin de code supprime un conflit :\n{}",
        offenders.join("\n")
    );
}

#[test]
fn resolving_a_conflict_keeps_both_sides() {
    // §18.4, point 3 : une fusion « conserve les claims incompatibles ». Trancher, c'est décider
    // lequel des deux camps l'emporte et pourquoi — pas effacer le camp perdant.
    let mut rng = Rng::new(401);
    let winner = rng.revision();
    let loser = rng.revision();
    let mut log = ConflictLog::new();
    log.declare(conflict("cfl_1", winner, loser));

    assert!(log.record_verdict(
        "cfl_1",
        Verdict::Prevails {
            side: winner,
            rationale: "reproduit deux fois".to_owned(),
        }
    ));

    // Le conflit est toujours là, tranché.
    assert_eq!(log.len(), 1);
    assert!(log.open().is_empty());
    let resolved = log.get("cfl_1").expect("le conflit est conservé");
    assert!(!resolved.is_open());
    // Et surtout : les deux camps se relisent après le verdict.
    assert_eq!(resolved.sides(), [winner, loser]);
    assert!(
        resolved.sides().contains(&loser),
        "le camp perdant a disparu"
    );
    assert_eq!(resolved.statement, "les deux mesures s'excluent");
}

#[test]
fn an_unresolved_conflict_may_stay_open_forever() {
    // Un graphe qui ne peut pas porter de désaccord durable est un graphe qui force une réponse
    // avant qu'elle existe.
    let mut rng = Rng::new(402);
    let mut log = ConflictLog::new();
    log.declare(conflict("cfl_1", rng.revision(), rng.revision()));
    assert_eq!(log.open().len(), 1);
    assert_eq!(log.all().len(), 1);
    // Rien dans l'API ne permet de le faire taire.
    assert_eq!(log.get("cfl_1").map(Conflict::is_open), Some(true));
}

#[test]
fn a_second_declaration_does_not_erase_a_verdict() {
    // Redéclarer un désaccord déjà tranché ne doit pas effacer la décision qui l'a tranché.
    let mut rng = Rng::new(403);
    let first = rng.revision();
    let second = rng.revision();
    let mut log = ConflictLog::new();
    log.declare(conflict("cfl_1", first, second));
    log.record_verdict("cfl_1", Verdict::BothSuperseded { by: rng.revision() });

    log.declare(conflict("cfl_1", first, second));
    assert!(
        !log.get("cfl_1").expect("conservé").is_open(),
        "la redéclaration a rouvert un conflit tranché"
    );
}

#[test]
fn a_verdict_on_an_unknown_conflict_is_refused() {
    // Inventer une entrée pour porter un verdict ferait exister un désaccord que personne n'a
    // déclaré.
    let mut log = ConflictLog::new();
    assert!(!log.record_verdict("cfl_fantome", Verdict::Unresolved));
    assert!(log.is_empty());
}

#[test]
fn a_merge_produces_conflicts_to_declare_never_objects_to_drop() {
    // §18.4, point 7 : « créer des `Conflict` explicites ». La fonction rend ce qu'il faut
    // déclarer, et jamais une liste d'objets à retirer — une fusion qui trancherait d'elle-même
    // produirait un graphe propre et faux.
    let mut rng = Rng::new(404);
    let pairs = vec![
        (rng.revision(), rng.revision()),
        (rng.revision(), rng.revision()),
    ];
    let produced = conflicts_from_merge(&pairs, "br_main", "2026-08-17T09:00:00.000Z");
    assert_eq!(produced.len(), 2);
    for (index, conflict) in produced.iter().enumerate() {
        assert!(conflict.is_open(), "une fusion a tranché d'elle-même");
        assert_eq!(conflict.origin, ConflictOrigin::Merge);
        assert_eq!(conflict.sides(), [pairs[index].0, pairs[index].1]);
    }
}

// ————————————————————————— Résultats négatifs — §18.7 —————————————————————————

fn negative(power: Power, search_space: &str) -> NegativeResult {
    NegativeResult {
        question_or_hypothesis: "le catalyseur B accélère la réaction".to_owned(),
        method: "criblage systématique".to_owned(),
        parameters: "pH 6–8, 20–40 °C, 12 concentrations".to_owned(),
        search_space: search_space.to_owned(),
        conditions: "atmosphère inerte".to_owned(),
        outcome: "aucune accélération mesurable".to_owned(),
        statistical_or_formal_power: power,
        known_limitations: vec!["un seul lot de réactif".to_owned()],
        failure_mode: "effet sous le seuil de détection".to_owned(),
        artifacts: Vec::new(),
        applicability_scope: "solvants polaires".to_owned(),
    }
}

#[test]
fn an_unstated_power_excludes_nothing() {
    // La troisième question de §18.7 : « qu'est-ce que son échec exclut réellement ? » Une
    // puissance non déclarée n'est pas une forte puissance, et ce n'est pas non plus une faible :
    // c'est une absence d'information.
    let result = negative(Power::Unstated, "12 concentrations");
    match result.excludes() {
        Exclusion::ExcludesNothing { reason } => assert!(reason.contains("non déclarée")),
        Exclusion::Excludes { .. } => panic!("un échec sans puissance a exclu quelque chose"),
    }
    // Mais le résultat est **conservé** : savoir qu'une tentative n'a rien prouvé évite de la
    // refaire en croyant qu'elle avait prouvé quelque chose.
    assert!(
        result
            .findings()
            .iter()
            .any(|line| line.contains("conservé"))
    );

    // Et la puissance elle-même le dit, indépendamment de `excludes` : une absence n'est pas une
    // valeur, et les deux gardes doivent tenir séparément — la seconde a été ajoutée après qu'une
    // mutation de la première soit passée inaperçue.
    assert!(!Power::Unstated.is_conclusive());
    assert!(
        !Power::Statistical {
            value: CONCLUSIVE_POWER - 0.01,
            basis: "juste en dessous".to_owned(),
        }
        .is_conclusive()
    );
    assert!(
        Power::Exhaustive {
            over: "tout".to_owned()
        }
        .is_conclusive()
    );
}

#[test]
fn an_underpowered_search_excludes_nothing_either() {
    let weak = negative(
        Power::Statistical {
            value: 0.4,
            basis: "effet de taille 0.3 détectable".to_owned(),
        },
        "12 concentrations",
    );
    match weak.excludes() {
        Exclusion::ExcludesNothing { reason } => assert!(reason.contains("insuffisante")),
        Exclusion::Excludes { .. } => panic!("une recherche faible a exclu quelque chose"),
    }
}

#[test]
fn a_powerful_search_excludes_within_its_scope_and_never_beyond() {
    // « Nous n'avons pas trouvé X ici » devient « X n'existe pas » exactement quand l'énoncé perd
    // ses bornes.
    let strong = negative(
        Power::Statistical {
            value: 0.95,
            basis: "effet de taille 0.1 détectable".to_owned(),
        },
        "pH 6–8, 20–40 °C",
    );
    match strong.excludes() {
        Exclusion::Excludes { statement, within } => {
            assert!(statement.contains("criblage systématique"));
            assert!(within.contains("pH 6–8"));
            assert!(within.contains("solvants polaires"));
        }
        Exclusion::ExcludesNothing { .. } => panic!("une recherche puissante n'a rien exclu"),
    }

    // Exhaustif exclut aussi — mais toujours borné à ce qui a été parcouru.
    let exhaustive = negative(
        Power::Exhaustive {
            over: "les 4096 configurations".to_owned(),
        },
        "les 4096 configurations",
    );
    assert!(matches!(exhaustive.excludes(), Exclusion::Excludes { .. }));
}

#[test]
fn a_powerful_search_over_nothing_excludes_nothing() {
    // Le troisième refus : une puissance forte sur un espace non déclaré. On ne sait pas où l'on a
    // cherché, donc on ne sait pas ce qui a été écarté.
    let nowhere = negative(
        Power::Statistical {
            value: 0.99,
            basis: "très sensible".to_owned(),
        },
        "   ",
    );
    match nowhere.excludes() {
        Exclusion::ExcludesNothing { reason } => assert!(reason.contains("espace de recherche")),
        Exclusion::Excludes { .. } => panic!("un échec sans espace déclaré a exclu quelque chose"),
    }
}

#[test]
fn the_attempt_signature_answers_has_this_been_tried() {
    // §18.7 : « cette attaque a-t-elle déjà été tentée, dans quelles conditions ? » La réponse est
    // le couple méthode + paramètres + conditions, et c'est sur lui qu'un futur chercheur
    // reconnaîtra sa propre tentative avant de la refaire.
    let result = negative(Power::Unstated, "12 concentrations");
    let signature = result.attempt_signature();
    assert!(signature.contains("criblage systématique"));
    assert!(signature.contains("pH 6–8"));
    assert!(signature.contains("atmosphère inerte"));
    // Deux tentatives identiques ont la même signature — c'est ce qui les rend trouvables.
    assert_eq!(
        signature,
        negative(Power::Unstated, "autre espace").attempt_signature()
    );
}

#[test]
fn the_eleven_fields_of_the_text_are_all_required() {
    // §18.7 énumère onze champs ; aucun n'est facultatif dans le type. `findings` signale ceux
    // qu'on aurait laissés vides.
    let empty = NegativeResult {
        question_or_hypothesis: String::new(),
        method: String::new(),
        outcome: "   ".to_owned(),
        failure_mode: String::new(),
        applicability_scope: String::new(),
        ..negative(Power::Unstated, "x")
    };
    let findings = empty.findings();
    assert!(findings.len() >= 5, "{findings:?}");
    assert!(
        negative(
            Power::Exhaustive {
                over: "tout".to_owned()
            },
            "tout"
        )
        .findings()
        .is_empty()
    );
}
