//! Test de sortie de `W16.d` — **la visibilité institutionnelle facultative des sous-agents
//! internes du harnais**, tranche 4 du mineur `lep/1.1`.
//!
//! Trois clauses, celles de la roadmap :
//!
//! 1. la visibilité est **facultative** — un harnais qui ne la demande pas n'émet rien, et un test
//!    le tient ;
//! 2. ce qui est rendu porte l'existence, la classe de cognition, le coût et le résultat, et **rien
//!    du contexte**, tenu par un test d'absence ;
//! 3. la lecture du raisonnement d'un sous-agent passe par `locus_memory::read` sous les trois
//!    classes, jamais par un chemin propre au harnais.
//!
//! # Ce que l'item attendait, et ce qui l'a débloqué
//!
//! `W16.d` a attendu deux choses successives : une **décision**, prise par l'ADR 0027 décision 7, et
//! un **lecteur**, livré par `W26.b`. Sa ligne posait comme question — « voir qu'un sous-agent
//! existe et voir son contexte sont deux choses » — ce qui est devenu la réponse.

use locus_lep::{Attempt, AttemptSubagentsItem, AttemptSubagentsItemCost};
use locusd::subagents::{Outcome, Visibility, seen};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn attempt(subagents: Option<Vec<AttemptSubagentsItem>>) -> Attempt {
    let brut = serde_json::json!({
        "protocol": "lep/1.0",
        "task_id": "tsk_catalyseur",
        "attempt": 3,
        "worker_id": "wrk-01",
        "state": "succeeded",
        "started_at": "2026-08-25T12:00:00.000Z"
    });
    let mut tentative: Attempt = serde_json::from_value(brut).expect("un attempt bien formé");
    tentative.subagents = subagents;
    tentative
}

fn sous_agent(nom: &str, classe: &str, resultat: &str) -> AttemptSubagentsItem {
    AttemptSubagentsItem {
        name: nom.to_owned(),
        cognition: classe.to_owned(),
        outcome: resultat.to_owned(),
        cost: None,
    }
}

fn source(chemin: &str) -> String {
    let brut = std::fs::read_to_string(format!("{}/{chemin}", env!("CARGO_MANIFEST_DIR")))
        .expect("le module de production est lisible depuis son propre crate");
    brut.lines()
        .filter(|ligne| !ligne.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------------------------
// 1. Facultative veut dire facultative
// ---------------------------------------------------------------------------------------------

/// **Un harnais qui ne déclare rien n'émet rien**, et ce n'est pas « aucun sous-agent ».
///
/// Les deux absences sont distinctes, et c'est la clause. « Ce harnais ne sait pas subdiviser » et
/// « ce harnais n'a pas subdivisé cette fois » appellent des questions différentes, et une seule des
/// deux se pose à l'exploitant.
///
/// C'est la faute que l'ADR 0017 décision 6 nomme sous un autre nom : « un `role` qui vaudrait
/// `research` faute de mieux rendrait *l'institution n'a pas dit* indiscernable de *l'institution a
/// dit `research`*, et c'est le second qui se croit tenu. »
#[test]
fn ne_rien_declarer_n_est_pas_declarer_aucun() {
    let muet = seen(&attempt(None));
    let vide = seen(&attempt(Some(Vec::new())));

    assert_eq!(muet, Visibility::NotDeclared);
    assert_eq!(vide, Visibility::Declared(Vec::new()));
    assert_ne!(muet, vide, "les deux absences ne se confondent pas");
}

/// **Le décompte le dit aussi**, et c'est là que la confusion se produirait.
///
/// `None` quand rien n'est déclaré, `Some(0)` quand le harnais a regardé. Un `0` dans les deux cas
/// serait le compteur qui n'a rien lu — règle 3 du rythme de session : « la réponse est zéro » et
/// « il n'y a pas eu de réponse » ne se rendent pas par la même valeur.
#[test]
fn le_decompte_distingue_zero_de_rien_lu() {
    assert_eq!(seen(&attempt(None)).count(), None);
    assert_eq!(seen(&attempt(Some(Vec::new()))).count(), Some(0));
    assert_eq!(
        seen(&attempt(Some(vec![sous_agent(
            "critique",
            "economy",
            "succeeded"
        )])))
        .count(),
        Some(1)
    );
}

/// Le champ est bien marqué `x-since: 1.1` et la feature existe, avec `since: 1.1`.
///
/// La tranche 4 est un **mineur**, et ce qui la rend mineure est vérifiable : un champ facultatif et
/// une feature négociée, pas une obligation nouvelle. Le test lit les deux documents plutôt que de
/// s'en remettre à la relecture.
#[test]
fn le_champ_et_la_feature_sont_marques_comme_mineurs() {
    let racine = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("le crate vit sous apps/, donc la racine est deux crans au-dessus")
        .to_path_buf();

    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(racine.join("schemas/lep/1.0/attempt.schema.json"))
            .expect("le schéma est lisible"),
    )
    .expect("le schéma est du JSON");
    let champ = &schema["properties"]["subagents"];
    assert_eq!(champ["x-since"], "1.1");
    assert!(
        !schema["required"]
            .as_array()
            .expect("required est une liste")
            .iter()
            .any(|nom| nom == "subagents"),
        "un champ facultatif ne s'exige pas : l'obliger ferait payer la fonctionnalité à ceux qui ne \
         l'utilisent pas"
    );

    let features: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(racine.join("schemas/lep/1.0/features.json"))
            .expect("features.json est lisible"),
    )
    .expect("features.json est du JSON");
    let negociee = features["features"]
        .as_array()
        .expect("features est une liste")
        .iter()
        .find(|feature| feature["name"] == "subagent-visibility")
        .expect("la feature est déclarée");
    assert_eq!(negociee["since"], "1.1");
}

/// **Un document `1.0` laisse le champ absent** — jamais rempli par un défaut.
///
/// C'est le test 2 de l'ADR 0017 décision 6, celui que l'ADR appelle « le plus important des deux »,
/// appliqué à cette tranche : un consommateur `1.1` qui reçoit un document `1.0` ne doit pas
/// fabriquer une liste vide, qui dirait « le harnais a regardé ».
#[test]
fn un_document_sans_le_champ_laisse_l_absence() {
    let brut = serde_json::json!({
        "protocol": "lep/1.0",
        "task_id": "tsk_catalyseur",
        "attempt": 1,
        "worker_id": "wrk-01",
        "state": "succeeded",
        "started_at": "2026-08-25T12:00:00.000Z"
    });
    let tentative: Attempt = serde_json::from_value(brut).expect("un attempt 1.0 bien formé");

    assert!(tentative.subagents.is_none());
    assert_eq!(seen(&tentative), Visibility::NotDeclared);
}

// ---------------------------------------------------------------------------------------------
// 2. Quatre choses, et rien du contexte
// ---------------------------------------------------------------------------------------------

/// L'existence, la classe, le coût et le résultat — les quatre, et ils arrivent entiers.
#[test]
fn les_quatre_choses_arrivent() {
    let declare = AttemptSubagentsItem {
        name: "critique".to_owned(),
        cognition: "frontier".to_owned(),
        outcome: "succeeded".to_owned(),
        cost: Some(AttemptSubagentsItemCost {
            calls: Some(4),
            tokens: Some(12_000),
            wall_time_seconds: Some(31.5),
        }),
    };

    let Visibility::Declared(vus) = seen(&attempt(Some(vec![declare]))) else {
        panic!("le harnais déclare");
    };
    let [seul] = vus.as_slice() else {
        panic!("un seul sous-agent");
    };

    assert_eq!(seul.name, "critique");
    assert_eq!(seul.cognition, "frontier");
    assert_eq!(seul.outcome, Some(Outcome::Succeeded));
    assert_eq!(seul.cost.calls, Some(4));
    assert_eq!(seul.cost.tokens, Some(12_000));
    assert_eq!(seul.cost.wall_time_seconds, Some(31.5));
}

/// **Rien du contexte**, ni côté schéma ni côté lecteur.
///
/// C'est la moitié de l'item. Un sous-agent reviewer interne au harnais ne doit pas devenir le
/// chemin par lequel le raisonnement privé du générateur remonte — l'invariant 11 l'interdit, et une
/// visibilité qui porterait un transcript le contournerait sans qu'aucune garde ne s'en aperçoive,
/// puisque personne n'aurait rien « fuité » : le champ l'aurait simplement transporté.
#[test]
fn rien_du_contexte_ni_du_raisonnement_ne_passe() {
    let code = source("src/subagents.rs");
    for interdit in [
        "context",
        "transcript",
        "reasoning",
        "prompt",
        "messages",
        "trace",
        "thought",
    ] {
        assert!(
            !code.contains(interdit),
            "« {interdit} » ferait de la visibilité le chemin que l'invariant 11 ferme"
        );
    }

    // Et le schéma est fermé : `additionalProperties: false` sur l'item, donc un harnais ne peut pas
    // y glisser un champ que la garde ne connaît pas.
    let racine = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("la racine est deux crans au-dessus");
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(racine.join("schemas/lep/1.0/attempt.schema.json"))
            .expect("le schéma est lisible"),
    )
    .expect("le schéma est du JSON");
    let item = &schema["properties"]["subagents"]["items"];
    assert_eq!(
        item["additionalProperties"], false,
        "un item ouvert laisserait passer un transcript sous un nom que personne ne lit"
    );

    let champs: Vec<&str> = item["properties"]
        .as_object()
        .expect("les propriétés sont un objet")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        champs,
        vec!["cognition", "cost", "name", "outcome"],
        "quatre champs, et une cinquième colonne serait la question à reposer"
    );
}

/// **Un résultat inconnu ne se remplit pas d'un défaut.**
///
/// La règle que `SandboxLevel::parse` pose : « un niveau inconnu traité comme `S0` ouvrirait la
/// sandbox, et traité comme `S5` masquerait une configuration fausse en la rendant inoffensive. »
/// Ici, un mot inconnu compté comme `Failed` inventerait un échec, et comme `Succeeded` un succès.
#[test]
fn un_resultat_inconnu_reste_inconnu() {
    assert_eq!(Outcome::parse("succeeded"), Some(Outcome::Succeeded));
    assert_eq!(Outcome::parse("failed"), Some(Outcome::Failed));
    assert_eq!(Outcome::parse("cancelled"), Some(Outcome::Cancelled));
    assert_eq!(Outcome::parse("aborted"), None);
    assert_eq!(Outcome::parse(""), None);

    let Visibility::Declared(vus) = seen(&attempt(Some(vec![sous_agent(
        "critique", "economy", "aborted",
    )]))) else {
        panic!("le harnais déclare");
    };
    assert_eq!(vus[0].outcome, None, "l'aveu s'appelle l'absence");
}

/// **Interrompu n'est pas échoué.**
///
/// Confondre les deux ferait lire un budget épuisé comme une erreur de sous-agent, et chercher un
/// défaut là où il n'y a qu'une borne.
#[test]
fn interrompu_n_est_pas_echoue() {
    assert_ne!(Outcome::Cancelled, Outcome::Failed);
    assert_ne!(Outcome::parse("cancelled"), Outcome::parse("failed"));
}

/// Un coût **non mesuré** n'est pas un coût nul.
///
/// Chaque composante reste `None` : zéro dirait « mesuré à zéro » là où la vérité est « non mesuré ».
#[test]
fn un_cout_non_mesure_n_est_pas_un_cout_nul() {
    let Visibility::Declared(vus) = seen(&attempt(Some(vec![sous_agent(
        "critique",
        "economy",
        "succeeded",
    )]))) else {
        panic!("le harnais déclare");
    };
    assert_eq!(vus[0].cost.calls, None);
    assert_eq!(vus[0].cost.tokens, None);
    assert_eq!(vus[0].cost.wall_time_seconds, None);
}

// ---------------------------------------------------------------------------------------------
// 3. La lecture du raisonnement passe par les trois classes
// ---------------------------------------------------------------------------------------------

/// **Aucun chemin propre au harnais** ne rend le raisonnement d'un sous-agent.
///
/// La lecture, quand elle est due, passe par `locus_memory::read` sous les trois classes de l'ADR
/// 0027 décision 2. Un chemin propre au harnais serait exactement la **quatrième classe de lecteur**
/// que l'ADR refuse — « un lecteur système ou un outil d'analyse qui lirait sans être ni le
/// générateur, ni l'institution, ni un pair autorisé serait la porte dérobée de ce mécanisme ».
///
/// Tenu par l'absence : ce module n'importe pas la mémoire, et n'ouvre aucune lecture.
#[test]
fn aucun_chemin_propre_au_harnais_ne_lit_un_raisonnement() {
    let code = source("src/subagents.rs");
    // Les trois premières aiguilles tiennent la propriété **exactement** : une lecture de mémoire est
    // impossible sans l'un de ces trois noms. Les suivantes visent un helper local qui ferait le
    // travail sous un autre nom.
    //
    // Une sixième aiguille, « fn read », a été retirée après avoir rougi sur `read_one`, qui lit un
    // **item déclaré** et aucun raisonnement. C'est le cinquième faux positif de l'idiome de scan de
    // source dans cette session, et il se corrige comme les quatre autres : en **resserrant**
    // l'aiguille sur ce qu'elle voulait dire, jamais en relâchant la garde. Ici le resserrement ne
    // coûte rien, parce que « fn read » n'ajoutait rien aux trois premières.
    for interdit in [
        "locus_memory",
        "Disclosed",
        "Reader::",
        "fn read_reasoning",
        "fn read_trace",
        "fn read_context",
        "fn disclose",
        "fn reveal",
    ] {
        assert!(
            !code.contains(interdit),
            "« {interdit} » : la lecture passe par les trois classes, et un chemin d'ici en serait \
             une quatrième"
        );
    }
}
