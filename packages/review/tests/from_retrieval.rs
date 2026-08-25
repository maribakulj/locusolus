//! Test de sortie de `W17.n` — **le reçu, et la jonction qui n'existait pas.**
//!
//! Sept propriétés, celles du tableau de `docs/10` :
//!
//! 1. un chemin mène de `Results` à `ContextView` là où les deux crates s'ignoraient ;
//! 2. le reçu se rejoue et rend la **même** vue, condensat compris, par égalité stricte ;
//! 3. une exclusion sans motif n'est pas constructible ;
//! 4. la contestation vise le **reçu** et non la vue — aucun chemin de type ne permet l'inverse ;
//! 5. le reçu ne détient rien que le journal n'ait écrit ;
//! 6. sa forme canonique refuse les caractères de contrôle ;
//! 7. la couverture en contre-preuve est rendue **même à zéro**.

use std::collections::BTreeMap;

use locus_domain::{Confidentiality, RevisionId};
use locus_memory::{
    Candidate, Coverage, Exclusion, Genre, Plan, Ranking, ReceiptError, RetrievalReceipt, Signal,
    retrieve,
};
use locus_protocol::{Id, Timestamp};
use locus_review::from_retrieval::{JunctionError, replay_receipt, view_from_retrieval};
use locus_review::{ContextItem, Recipient};

const NOW: Timestamp = Timestamp::from_millis(1_700_000_000_000);

fn revision(seed: u8) -> RevisionId {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(NOW, entropy).expect("l'instant de fixture tient sur 48 bits")
}

fn item(seed: u8, classification: Confidentiality) -> ContextItem {
    ContextItem {
        revision: revision(seed),
        is_generator_reasoning: false,
        is_refuted: false,
        classification,
        cites: Vec::new(),
        is_external_source: false,
        produced_by: None,
        disclosed: None,
    }
}

fn candidat(seed: u8, classification: Confidentiality, total: f64) -> Candidate {
    Candidate::new(
        format!("k{seed}"),
        classification,
        Genre::Semantic,
        Ranking::of(&[(Signal::Lexical, total)]).expect("un score à un facteur"),
    )
    .expect("sémantique admet le lexical")
}

fn destinataire() -> Recipient {
    Recipient {
        agent_id: Id::from_parts(NOW, [0_u8; 10]).expect("fixture"),
        worker_id: "wk-1".to_owned(),
        blind_to_generator: true,
        clearance: Confidentiality::Internal,
    }
}

/// Trois candidats, dont un au-delà de l'habilitation.
fn corpus() -> BTreeMap<String, (ContextItem, u64)> {
    [
        ("k1".to_owned(), (item(1, Confidentiality::Internal), 10)),
        ("k2".to_owned(), (item(2, Confidentiality::Internal), 11)),
        ("k3".to_owned(), (item(3, Confidentiality::Restricted), 12)),
    ]
    .into_iter()
    .collect()
}

// ---------------------------------------------------------------------------------------------
// 1 et 2 — la jonction, et le rejeu
// ---------------------------------------------------------------------------------------------

/// **Un chemin mène de `Results` à `ContextView`.**
///
/// Avant cet item, `packages/review` et `packages/memory` ne se connaissaient pas : les deux
/// `Cargo.toml` le montraient, et `ContextView::build` prenait des `ContextItem` clés par
/// `RevisionId` quand `retrieve` rendait des `Candidate` clés par `String`. Ce test est la preuve
/// que le chemin existe — il n'aurait pas compilé hier.
#[test]
fn un_chemin_mene_du_retrieval_a_la_vue_de_contexte() {
    let plan = Plan::compatible(10).expect("budget licite");
    let resultats = retrieve(
        &plan,
        &[
            candidat(1, Confidentiality::Internal, 0.9),
            candidat(2, Confidentiality::Internal, 0.5),
            candidat(3, Confidentiality::Restricted, 0.8),
        ],
        Confidentiality::Internal,
    );

    let (vue, recu) = view_from_retrieval(
        &plan,
        &resultats,
        &corpus(),
        &destinataire(),
        100,
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect("la jonction tient");

    // Deux retenus, le restreint écarté par l'habilitation — et l'exclusion est **motivée**.
    assert_eq!(vue.included().len(), 2);
    assert_eq!(recu.included().len(), 2);
    assert_eq!(recu.considered(), 3);
    assert_eq!(recu.exclusions().len(), 1);
    assert!(
        recu.exclusions()[0].reason().contains("habilitation"),
        "{}",
        recu.exclusions()[0].reason()
    );
}

/// **Le reçu se rejoue et rend la même vue, condensat compris.**
///
/// L'égalité est stricte sur le `ContentHash` : c'est ce qui dit que le reçu n'a rien caché de ce
/// qui a été retenu. Sans le condensat, deux vues aux mêmes révisions dans un autre ordre
/// passeraient pour la même.
#[test]
fn le_recu_se_rejoue_et_rend_la_meme_vue() {
    let plan = Plan::compatible(10).expect("budget licite");
    let resultats = retrieve(
        &plan,
        &[
            candidat(1, Confidentiality::Internal, 0.9),
            candidat(2, Confidentiality::Internal, 0.5),
        ],
        Confidentiality::Internal,
    );
    let (vue, recu) = view_from_retrieval(
        &plan,
        &resultats,
        &corpus(),
        &destinataire(),
        100,
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect("la jonction tient");

    let rejouee = replay_receipt(
        &recu,
        &corpus(),
        &destinataire(),
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect("le reçu se rejoue");

    assert_eq!(rejouee.content_hash(), vue.content_hash());
    assert_eq!(rejouee.included(), vue.included());

    // Et le reçu **ne promet pas** ce qu'il ne peut pas tenir : les scores viennent de l'appelant.
    assert!(
        !recu.promises_replay(),
        "un plan compatible déclare `caller-supplied`, donc aucun rejeu du classement"
    );
}

/// **Un rejeu sur un corpus amputé refuse au lieu d'inventer.**
///
/// Rendre une vue plus courte serait rendre une vue plausible et fausse : rien dans la réponse ne
/// dirait qu'il manque quelque chose.
#[test]
fn un_rejeu_sur_un_corpus_ampute_refuse() {
    let plan = Plan::compatible(10).expect("budget licite");
    let resultats = retrieve(
        &plan,
        &[
            candidat(1, Confidentiality::Internal, 0.9),
            candidat(2, Confidentiality::Internal, 0.5),
        ],
        Confidentiality::Internal,
    );
    let (_, recu) = view_from_retrieval(
        &plan,
        &resultats,
        &corpus(),
        &destinataire(),
        100,
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect("la jonction tient");

    let mut ampute = corpus();
    ampute.remove("k2");

    let refus = replay_receipt(
        &recu,
        &ampute,
        &destinataire(),
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect_err("k2 a disparu");
    assert!(
        matches!(refus, JunctionError::Unresolvable { .. }),
        "{refus}"
    );
    assert!(refus.to_string().contains("plausible"), "{refus}");
}

// ---------------------------------------------------------------------------------------------
// 3, 6 et 7 — ce que le reçu refuse d'écrire
// ---------------------------------------------------------------------------------------------

/// **Une exclusion sans motif n'est pas constructible.**
#[test]
fn une_exclusion_sans_motif_n_est_pas_constructible() {
    assert_eq!(
        Exclusion::motivated("k1", ""),
        Err(ReceiptError::UnmotivatedExclusion {
            key: "k1".to_owned()
        })
    );
    assert_eq!(
        Exclusion::motivated("k1", "   "),
        Err(ReceiptError::UnmotivatedExclusion {
            key: "k1".to_owned()
        })
    );
    assert!(Exclusion::motivated("k1", "au-delà du budget").is_ok());
}

/// **La forme canonique du reçu refuse les caractères de contrôle**, comme les quatre de `W17.h`.
///
/// Un motif qui forgerait une ligne insérerait une exclusion que personne n'a écrite — dans le
/// document même qui existe pour rendre les exclusions lisibles.
#[test]
fn le_recu_refuse_un_champ_qui_forgerait_une_ligne() {
    for forge in ["motif\nout\tk9\tinventé", "motif\tavec tabulation"] {
        let refus = Exclusion::motivated("k1", forge).expect_err("caractère de contrôle");
        assert!(matches!(refus, ReceiptError::ForgesALine { .. }), "{refus}");
    }

    // Une clé forgée est refusée aussi, et à l'écriture du reçu.
    let plan = Plan::compatible(10).expect("budget licite");
    let refus = RetrievalReceipt::write(
        &plan,
        100,
        1,
        vec!["k1\nin\tk9".to_owned()],
        Vec::new(),
        Vec::new(),
    )
    .expect_err("clé forgée");
    assert!(matches!(refus, ReceiptError::ForgesALine { .. }), "{refus}");
}

/// **La couverture est rendue même quand elle vaut zéro.**
///
/// `None` dit « non mesurée », `Some(0.0)` dit « mesurée et nulle » — et la seconde est une
/// information : on a cherché une contre-preuve et il n'y en avait pas. Les fondre ferait lire
/// « aucune contre-preuve » là où personne n'a regardé.
#[test]
fn une_couverture_nulle_ne_se_confond_pas_avec_une_couverture_absente() {
    let plan = Plan::compatible(10).expect("budget licite");
    let base =
        RetrievalReceipt::write(&plan, 100, 0, Vec::new(), Vec::new(), Vec::new()).expect("reçu");

    let non_mesuree = base.clone();
    let mesuree_nulle = base
        .clone()
        .with_coverage(None, Some(Coverage::measured(0.0).expect("0 est une part")));

    assert_eq!(non_mesuree.counter_evidence_coverage(), None);
    assert_eq!(
        mesuree_nulle.counter_evidence_coverage().map(Coverage::get),
        Some(0.0)
    );

    // Les deux formes canoniques diffèrent, donc les deux condensats aussi : la distinction survit
    // à l'écriture, ce qui est le seul endroit où elle compte.
    assert_ne!(non_mesuree.canonical(), mesuree_nulle.canonical());
    assert_ne!(non_mesuree.digest(), mesuree_nulle.digest());
    assert!(non_mesuree.canonical().contains("counter\t-"));
    assert!(mesuree_nulle.canonical().contains("counter\t0.0000"));

    // Une couverture qui n'est pas une proportion est refusée.
    assert!(Coverage::measured(1.5).is_err());
    assert!(Coverage::measured(f64::NAN).is_err());
}

// ---------------------------------------------------------------------------------------------
// 4 et 5 — ce que le reçu est, et ce qu'il n'est pas
// ---------------------------------------------------------------------------------------------

/// **La contestation vise le reçu, jamais la vue.**
///
/// §16.2 rend la `ContextView` immuable et adressée par hash : contester ce qui a été vu n'a pas de
/// sens, c'est un fait. Ce qui se conteste est la manière dont elle a été constituée. Le test le
/// tient par l'absence — aucun chemin de type ne rend une vue modifiable ou contestable.
#[test]
fn la_contestation_vise_le_recu_et_non_la_vue() {
    let source = include_str!("../src/context_view.rs");
    for interdit in [
        "pub fn objection",
        "pub fn contest",
        "pub fn dispute",
        "&mut self",
        "pub fn set_",
    ] {
        assert!(
            !source.contains(interdit),
            "« {interdit} » dans context_view.rs : une vue est ce qui a été vu, donc un fait"
        );
    }
}

/// **Le reçu ne détient aucun contenu** — des identités, pas des documents.
///
/// Un reçu qui embarquerait ce qu'il a servi serait un second stockage du même fait, et c'est
/// l'argument par lequel l'ADR 0019 a écarté le courtier de messages.
#[test]
fn le_recu_ne_detient_aucun_contenu() {
    let source = include_str!("../../memory/src/receipt.rs");
    // Les motifs visent des **déclarations de champ**, pas des mots. Un premier essai interdisait
    // « document », qui est un mot de français que la documentation du module emploie pour dire
    // exactement ce que la garde veut obtenir — une garde qui se déclenche sur sa propre
    // justification est une garde qu'on finit par assouplir.
    for interdit in [
        "Vec<u8>",
        "body:",
        "payload:",
        "content:",
        "document:",
        "ContextItem",
    ] {
        assert!(
            !source.contains(interdit),
            "« {interdit} » dans receipt.rs : un reçu porte des identités, pas des documents"
        );
    }
}

/// La réserve de négatifs est **écrite même à zéro**, comme la couverture.
#[test]
fn la_reserve_de_negatifs_est_ecrite_meme_a_zero() {
    let plan = Plan::compatible(10).expect("budget licite");
    let recu =
        RetrievalReceipt::write(&plan, 100, 0, Vec::new(), Vec::new(), Vec::new()).expect("reçu");

    assert_eq!(recu.negative_reserve(), 0);
    assert!(
        recu.canonical().contains("reserve\t0"),
        "une garantie absente et une garantie nulle ne se lisent pas pareil : {}",
        recu.canonical()
    );
}

/// **Deux vues aux mêmes révisions dans un ordre différent ne sont pas la même vue.**
///
/// La documentation de la jonction dit que le condensat porte l'ordre — « l'ordre des inclusions est
/// le classement », et deux vues qui retiennent les mêmes révisions dans un autre ordre ne servent
/// pas la même chose au lecteur. **Rien ne le tenait** : un mutant qui triait les révisions avant de
/// calculer le condensat a survécu à toute la suite. C'est la quatrième fois de cette série qu'une
/// phrase du module décrit une propriété que personne ne vérifie.
#[test]
fn le_condensat_d_une_vue_porte_l_ordre_des_inclusions() {
    let plan = Plan::compatible(10).expect("budget licite");

    // Deux reçus, mêmes révisions, ordres opposés.
    let dans_l_ordre = RetrievalReceipt::write(
        &plan,
        100,
        2,
        vec![revision(1).to_string(), revision(2).to_string()],
        Vec::new(),
        Vec::new(),
    )
    .expect("reçu");
    let a_l_envers = RetrievalReceipt::write(
        &plan,
        100,
        2,
        vec![revision(2).to_string(), revision(1).to_string()],
        Vec::new(),
        Vec::new(),
    )
    .expect("reçu");

    let une = replay_receipt(
        &dans_l_ordre,
        &corpus(),
        &destinataire(),
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect("rejeu");
    let autre = replay_receipt(
        &a_l_envers,
        &corpus(),
        &destinataire(),
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect("rejeu");

    assert_ne!(
        une.content_hash(),
        autre.content_hash(),
        "un condensat qui trierait ferait passer deux classements différents pour le même"
    );
    // Et les deux vues retiennent bien le même ensemble : c'est l'ordre, et rien d'autre, qui les
    // sépare — sans ce second assert, le test passerait aussi si l'une des deux perdait un élément.
    let mut gauche = une.included().to_vec();
    let mut droite = autre.included().to_vec();
    gauche.sort_unstable();
    droite.sort_unstable();
    assert_eq!(gauche, droite);
}
