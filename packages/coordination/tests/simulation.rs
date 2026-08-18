//! Test de sortie de W16.c — **les trois garanties de l'item.**
//!
//! 1. Deux rejeux de la même trace rendent le même résultat.
//! 2. Un substitut d'environnement qui n'a pas la réponse le **dit** au lieu d'en inventer une.
//! 3. Un objet simulé n'existe **pas** comme type dans le domaine épistémique, et deux tests le
//!    tiennent par l'absence.

use locus_coordination::simulation::{Outcome, Verdict, run};
use locus_coordination::{Answer, Fidelity, Recorded};
use locus_domain::ValidationLevel;
use locus_protocol::{Id, IdKind, Timestamp, id::provisional::Decision as DecisionKind};

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn proposal() -> Id<DecisionKind> {
    id::<DecisionKind>(1)
}

/// Un environnement qui sait répondre aux trois questions du plan.
fn recorded() -> Recorded {
    Recorded::new()
        .answering("image-digest", "sha256:abc")
        .answering("toolchain", "rust-1.94")
        .answering("gpu", "absent")
}

const PLAN: [&str; 3] = ["image-digest", "toolchain", "gpu"];

// ---------------------------------------------------------------------------------------------
// 1. Le rejeu est déterministe
// ---------------------------------------------------------------------------------------------

/// Deux rejeux de la même trace rendent le même résultat.
///
/// Ce n'est pas une promesse : le rejeu ne consulte **rien** d'autre que le substitut — ni horloge,
/// ni ordre d'itération d'un conteneur non ordonné, ni environnement. Le déterminisme est une
/// conséquence de ce que la fonction peut voir.
#[test]
fn two_replays_of_one_trace_agree() {
    let first = run(proposal(), Fidelity::Replay, &PLAN, &recorded());
    let second = run(proposal(), Fidelity::Replay, &PLAN, &recorded());
    assert_eq!(first, second);
    assert!(first.verdict().is_complete());
}

/// Ce qui est observé se lit, il ne se devine pas.
///
/// La forme est figée en fixture : sans elle, une observation qui perdrait les questions ou les
/// réponses resterait déterministe — donc passerait les deux tests d'égalité — tout en ne disant
/// plus ce qui a été observé.
#[test]
fn what_is_observed_is_frozen() {
    let outcome = run(proposal(), Fidelity::Replay, &PLAN, &recorded());
    let Verdict::Complete { observed } = outcome.verdict() else {
        panic!("les trois questions ont une réponse");
    };
    assert_eq!(
        observed,
        "simulation/1\nreplay\nimage-digest\tsha256:abc\ntoolchain\trust-1.94\ngpu\tabsent\n"
    );
}

/// L'ordre du plan fait partie du résultat : deux plans qui interrogent les mêmes choses dans deux
/// ordres différents n'observent pas la même chose, et le prétendre effacerait une dépendance.
#[test]
fn the_order_of_the_plan_is_part_of_what_is_observed() {
    let straight = run(proposal(), Fidelity::Replay, &PLAN, &recorded());
    let reversed = run(
        proposal(),
        Fidelity::Replay,
        &["gpu", "toolchain", "image-digest"],
        &recorded(),
    );
    assert_ne!(straight.verdict(), reversed.verdict());
}

// ---------------------------------------------------------------------------------------------
// 2. Un substitut qui ne sait pas le dit
// ---------------------------------------------------------------------------------------------

/// La faute que ce module existe pour empêcher, et elle est silencieuse.
///
/// Un défaut — chaîne vide, zéro, « inconnu » — ferait **réussir** la simulation là où le run réel
/// aurait échoué. Prédire est la seule chose qu'on demande à une simulation : celle qui se trompe
/// dans ce sens-là est pire qu'absente, puisqu'on s'appuie dessus.
#[test]
fn a_substitute_that_does_not_know_says_so() {
    let thin = Recorded::new().answering("image-digest", "sha256:abc");
    assert_eq!(
        thin.ask("image-digest"),
        Answer::Recorded("sha256:abc".to_owned())
    );
    assert_eq!(
        thin.ask("gpu"),
        Answer::NotRecorded {
            question: "gpu".to_owned()
        },
        "jamais un défaut : une valeur inventée ne se distingue pas d'une valeur observée"
    );
}

/// Et la simulation ne conclut **rien** : elle rend ce qui manque.
///
/// « Pas vérifié » n'est jamais « réussi ». Un verdict rendu malgré des questions sans réponse
/// serait cité comme un résultat, et personne n'irait vérifier sur quoi il reposait.
#[test]
fn an_incomplete_simulation_reaches_no_verdict_and_names_what_is_missing() {
    let thin = Recorded::new().answering("image-digest", "sha256:abc");
    let outcome = run(proposal(), Fidelity::Replay, &PLAN, &thin);

    assert!(!outcome.verdict().is_complete());
    let Verdict::Incomplete { unanswered } = outcome.verdict() else {
        panic!("deux questions n'ont pas de réponse");
    };
    assert_eq!(unanswered, &["toolchain".to_owned(), "gpu".to_owned()]);
}

/// Un substitut vide ne conclut pas « rien à vérifier » : il conclut « rien de vérifié ».
#[test]
fn an_empty_substitute_never_passes() {
    let outcome = run(proposal(), Fidelity::Replay, &PLAN, &Recorded::new());
    assert!(!outcome.verdict().is_complete());
}

// ---------------------------------------------------------------------------------------------
// Le degré atteint, jamais celui qui était visé
// ---------------------------------------------------------------------------------------------

/// Un rejeu ne dit pas ce qu'un canari dirait.
///
/// Le résultat porte le degré **réellement atteint**. S'il portait celui qui était visé, une
/// simulation serait citée pour ce qu'elle n'a pas fait — et le canari est facultatif, donc le cas
/// est courant.
#[test]
fn an_outcome_carries_the_fidelity_it_reached() {
    for reached in Fidelity::ALL {
        let outcome = run(proposal(), reached, &PLAN, &recorded());
        assert_eq!(outcome.reached(), reached);
    }
    assert_eq!(
        Fidelity::ALL
            .iter()
            .map(|fidelity| fidelity.slug())
            .collect::<Vec<_>>(),
        ["replay", "recorded-environment", "shadow", "canary"],
        "quatre degrés, du moins fidèle au plus fidèle"
    );
    assert!(
        Fidelity::Replay < Fidelity::Canary,
        "l'ordre est celui de la fidélité, et il se compare"
    );
}

/// Deux degrés différents n'observent pas la même chose, même sur le même plan.
#[test]
fn the_reached_fidelity_is_part_of_what_is_observed() {
    let replayed = run(proposal(), Fidelity::Replay, &PLAN, &recorded());
    let shadowed = run(proposal(), Fidelity::Shadow, &PLAN, &recorded());
    assert_ne!(replayed.verdict(), shadowed.verdict());
}

// ---------------------------------------------------------------------------------------------
// 3. Un objet simulé n'existe pas dans le domaine épistémique
// ---------------------------------------------------------------------------------------------

/// ADR 0016, décision 9 : « la garantie est une **absence de type**, pas un champ de
/// classification ».
///
/// Un résultat de simulation désigne une **proposition**, et rien d'autre. Il ne peut pas nommer
/// une `RevisionId` — l'identité d'un objet épistémique — donc il ne peut pas être cité comme
/// preuve à propos d'un claim. La vérification porte sur la représentation : aucun identifiant de
/// révision n'y apparaît, et il n'y a pas de champ où en glisser un.
#[test]
fn a_simulated_outcome_cannot_name_an_epistemic_object() {
    let outcome: Outcome = run(proposal(), Fidelity::Shadow, &PLAN, &recorded());
    assert_eq!(outcome.proposal(), proposal());

    let rendered = format!("{outcome:?}");
    assert!(
        rendered.contains("proposal"),
        "il désigne la proposition qu'il simule"
    );
    for epistemic in ["revision", "RevisionId", "claim", "Claim"] {
        assert!(
            !rendered.contains(epistemic),
            "« {epistemic} » n'a rien à faire dans un résultat de simulation"
        );
    }
}

/// Et la machinerie de validation n'a aucun barreau où le faire entrer.
///
/// Décision 9 : « ajouter un niveau `simulated` ferait circuler la simulation » dans la propagation
/// de l'invalidation sur les niveaux de §8.1. Le test le tient par l'absence, en nommant les mots
/// qu'on serait tenté d'ajouter — pour que l'échec dise **lequel** est entré.
#[test]
fn the_validation_ladder_has_no_rung_for_a_simulation() {
    let rungs: Vec<&str> = ValidationLevel::ALL
        .iter()
        .map(|level| level.as_str())
        .collect();
    assert_eq!(
        rungs.len(),
        7,
        "les sept niveaux de §8.1, et pas un de plus"
    );
    for tempting in ["simulated", "shadow", "canary", "replayed", "dry-run"] {
        assert!(
            !rungs.contains(&tempting),
            "« {tempting} » ferait circuler la simulation dans la propagation de l'invalidation"
        );
    }
}
