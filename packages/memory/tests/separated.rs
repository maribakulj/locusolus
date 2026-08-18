//! Test de sortie de W17.c — **les trois garanties de l'item.**
//!
//! 1. Les deux retrievals répondent à des questions différentes, sur des **types disjoints**.
//! 2. Aucune fonction ne convertit un résultat de l'un en résultat de l'autre — et c'est une
//!    impossibilité à la compilation, pas une garde.
//! 3. Aucun trait générique ne les factorise.
//!
//! # Ce que ce fichier a corrigé du plan
//!
//! Le test de sortie annonçait que « la septième frontière l'étend à ce cas ». En l'écrivant, il est
//! apparu que la frontière n'est **pas** le bon outil ici : les deux familles vivent dans le même
//! crate, donc le module qui les expose doit forcément voir les deux, et la règle aurait exigé une
//! exception sur la racine — une garde qui s'excepte à l'endroit exact où la faute s'écrirait.
//!
//! Ce qui tient la séparation est plus fort : `packages/protocol` fait du préfixe une partie de
//! l'identité, et `Id::parse` refuse un préfixe étranger. Une conversion devrait fabriquer une
//! identité qu'elle ne peut pas fabriquer. C'est une impossibilité à la compilation, là où la
//! frontière n'aurait été qu'un motif cherché dans du texte.

use locus_domain::{Confidentiality, RevisionId, ids::RevisionKind};
use locus_memory::{
    EpistemicEntry, EpistemicHit, OrganisationalEntry, OrganisationalHit, Ranking, Signal,
    epistemic, organisational,
};
use locus_protocol::{Id, IdKind, ParseIdError, Timestamp, id::Agent};

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn revision(seed: u8) -> RevisionId {
    id::<RevisionKind>(seed)
}

fn agent(seed: u8) -> Id<Agent> {
    id::<Agent>(seed)
}

fn ranking(total: f64) -> Ranking {
    Ranking::of(&[(Signal::Lexical, total)]).expect("le score expose ses facteurs")
}

fn known(seed: u8, classification: Confidentiality, total: f64) -> EpistemicEntry {
    EpistemicEntry {
        revision: revision(seed),
        classification,
        is_negative: false,
        ranking: ranking(total),
    }
}

fn worked(seed: u8, classification: Confidentiality, total: f64) -> OrganisationalEntry {
    OrganisationalEntry {
        agent: agent(seed),
        classification,
        is_negative: false,
        ranking: ranking(total),
    }
}

// ---------------------------------------------------------------------------------------------
// 1. Deux questions, deux types
// ---------------------------------------------------------------------------------------------

/// Le même corpus, deux questions, deux réponses qui ne se déduisent pas l'une de l'autre.
///
/// Savoir qu'un agent a beaucoup produit ne dit rien de ce que ses productions valent, et
/// l'inverse non plus.
#[test]
fn the_two_retrievals_answer_different_questions() {
    let known = epistemic(
        &[
            known(1, Confidentiality::Internal, 2.0),
            known(2, Confidentiality::Internal, 1.0),
        ],
        Confidentiality::Internal,
        10,
    );
    let who = organisational(
        &[
            worked(1, Confidentiality::Internal, 1.0),
            worked(2, Confidentiality::Internal, 2.0),
        ],
        Confidentiality::Internal,
        10,
    );

    assert_eq!(
        known.iter().map(|hit| hit.revision).collect::<Vec<_>>(),
        [revision(1), revision(2)]
    );
    assert_eq!(
        who.iter().map(|hit| hit.agent).collect::<Vec<_>>(),
        [agent(2), agent(1)],
        "l'ordre est celui des scores, et il n'a rien à voir avec l'autre réponse"
    );
}

/// Le moteur **est** partagé, et c'est voulu : deux moteurs divergeraient, et l'un des deux
/// finirait par laisser passer ce que l'autre refuse.
#[test]
fn both_retrievals_obey_the_same_acl_and_the_same_budget() {
    let secret_revision = epistemic(
        &[known(1, Confidentiality::Restricted, f64::MAX)],
        Confidentiality::Internal,
        10,
    );
    let secret_agent = organisational(
        &[worked(1, Confidentiality::Restricted, f64::MAX)],
        Confidentiality::Internal,
        10,
    );
    assert!(secret_revision.is_empty());
    assert!(secret_agent.is_empty());

    let capped = epistemic(
        &[
            known(1, Confidentiality::Public, 2.0),
            known(2, Confidentiality::Public, 1.0),
        ],
        Confidentiality::Public,
        1,
    );
    assert_eq!(capped.len(), 1);

    let also_capped = organisational(
        &[
            worked(1, Confidentiality::Public, 2.0),
            worked(2, Confidentiality::Public, 1.0),
        ],
        Confidentiality::Public,
        1,
    );
    assert_eq!(
        also_capped.len(),
        1,
        "le budget vaut des deux côtés : le vérifier d'un seul laisserait l'autre s'en affranchir"
    );
}

/// La marque de résultat négatif traverse **entière**, des deux côtés.
///
/// L'aplatir ici reviendrait à taire les résultats négatifs une couche plus haut que le moteur, où
/// plus personne ne regarde — et l'invariant 12 tomberait sans qu'aucun filtre ne s'écrive. Le
/// mutant qui la force à `false` meurt ici.
#[test]
fn the_negative_mark_survives_both_retrievals() {
    let refuted = EpistemicEntry {
        is_negative: true,
        ..known(1, Confidentiality::Public, 1.0)
    };
    let found = epistemic(&[refuted], Confidentiality::Public, 10);
    assert_eq!(found.len(), 1, "un résultat négatif n'est pas écarté");
    assert!(found[0].is_negative, "et il arrive marqué");

    let dissenting = OrganisationalEntry {
        is_negative: true,
        ..worked(1, Confidentiality::Public, 1.0)
    };
    let who = organisational(&[dissenting], Confidentiality::Public, 10);
    assert_eq!(who.len(), 1);
    assert!(who[0].is_negative);
}

// ---------------------------------------------------------------------------------------------
// 2. La conversion est impossible, pas seulement interdite
// ---------------------------------------------------------------------------------------------

/// **Le test qui porte la garantie.**
///
/// Le préfixe fait partie de l'identité (`packages/protocol`). Une conversion d'un résultat en
/// l'autre devrait fabriquer une identité qu'elle n'a pas : ni directement, puisque les types
/// diffèrent, ni par un aller-retour en chaîne de caractères, puisque `rev_…` ne se relit pas
/// comme un `agt_…`.
#[test]
fn a_revision_never_reads_as_an_agent_nor_the_reverse() {
    let as_text = revision(1).to_string();
    assert_eq!(
        Id::<Agent>::parse(&as_text).expect_err("préfixe étranger"),
        ParseIdError::WrongPrefix { expected: "agent" }
    );

    let back = agent(1).to_string();
    assert!(
        Id::<RevisionKind>::parse(&back).is_err(),
        "et dans l'autre sens aussi"
    );
}

/// Les deux résultats ne sont pas comparables, et ne le deviennent pas par le score.
///
/// Deux hits de même score restent deux choses différentes : c'est le sujet qui diffère, pas le
/// rang. Une égalité de score qui vaudrait égalité de résultat rouvrirait la conversion par la
/// bande.
#[test]
fn two_hits_with_the_same_score_remain_two_different_things() {
    let known: Vec<EpistemicHit> = epistemic(
        &[known(1, Confidentiality::Public, 1.0)],
        Confidentiality::Public,
        10,
    );
    let who: Vec<OrganisationalHit> = organisational(
        &[worked(1, Confidentiality::Public, 1.0)],
        Confidentiality::Public,
        10,
    );

    assert_eq!(
        known[0].ranking, who[0].ranking,
        "le score, lui, est le même"
    );
    assert_ne!(
        known[0].revision.to_string(),
        who[0].agent.to_string(),
        "et pourtant les deux ne désignent pas la même chose"
    );
}

// ---------------------------------------------------------------------------------------------
// 3. Aucun trait ne les factorise
// ---------------------------------------------------------------------------------------------

/// Vérification par l'absence, sur ce que le crate expose.
///
/// Un trait « ce qui peut être cherché et classé » sur les deux serait la conversion reconstruite :
/// dès qu'un appelant écrit une fonction sur `impl Searchable`, les deux domaines se retraversent
/// sans qu'aucune ligne ne s'appelle « convertir ». C'est l'argument de l'ADR 0016 décision 9 pour
/// les familles d'objection, et il vaut mot pour mot ici.
///
/// Le test lit le source du crate plutôt que sa surface, parce qu'un trait absent ne se teste pas
/// autrement — et il nomme les mots qu'on serait tenté d'employer.
#[test]
fn no_trait_factors_the_two_families() {
    let source = include_str!("../src/separated.rs");
    for tempting in [
        "trait Searchable",
        "trait Retrievable",
        "trait Hit",
        "impl From<",
    ] {
        assert!(
            !source.contains(tempting),
            "« {tempting} » serait la conversion reconstruite"
        );
    }
}

/// Et le module ne relit jamais une clé pour retrouver une identité.
///
/// Le retour se fait par lecture d'une table construite à l'aller. Reparser la clé aurait rendu la
/// conversion écrivable pour de bon : il aurait suffi de reparser avec l'autre type.
#[test]
fn identities_come_back_from_a_table_never_from_a_key() {
    let source = include_str!("../src/separated.rs");
    assert!(
        !source.contains("::parse("),
        "reparser une clé rouvrirait la conversion"
    );
    assert!(source.contains("by_key.get("));
}
