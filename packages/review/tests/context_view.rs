//! Test de sortie de W7.c — **deux vues du même instant du journal ont le même hash ; une vue qui
//! aurait vu un événement postérieur à son watermark est refusée.**
//!
//! §16.2 : « une `ContextView` est immuable, adressée par hash et rattachée à l'exécution. Elle
//! permet de savoir exactement ce que l'agent **pouvait** connaître. » Les deux mots qui portent
//! tout sont *immuable* et *watermark* : sans le premier, la vue dit ce qu'on sait aujourd'hui
//! plutôt que ce qu'on savait ; sans le second, elle ne dit pas de quand date ce « aujourd'hui ».

use locus_domain::{Confidentiality, ContentHash, RevisionId};
use locus_protocol::{Id, IdKind, Timestamp, id::Agent};
use locus_review::{ContextItem, ContextView, ContextViewError, Recipient};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn revision(seed: u8) -> RevisionId {
    id::<locus_domain::ids::RevisionKind>(seed)
}

fn hash(byte: &str) -> ContentHash {
    ContentHash::parse(&format!("sha256:{}", byte.repeat(32))).expect("hash bien formé")
}

fn item(seed: u8) -> ContextItem {
    ContextItem {
        revision: revision(seed),
        is_generator_reasoning: false,
        is_refuted: false,
        classification: Confidentiality::Internal,
        cites: Vec::new(),
        is_external_source: true,
        produced_by: Some(id::<Agent>(1)),
        disclosed: None,
    }
}

fn reviewer() -> Recipient {
    Recipient {
        agent_id: id::<Agent>(2),
        worker_id: "vm-02".to_owned(),
        blind_to_generator: true,
        clearance: Confidentiality::Internal,
    }
}

// ---------------------------------------------------------------------------------------------
// Le watermark borne ce que la vue peut contenir
// ---------------------------------------------------------------------------------------------

/// Le refus qui porte le sprint. Une vue qui contiendrait un événement postérieur à son watermark
/// ne dirait plus ce qu'on savait — et c'est la faute qu'on ne peut plus détecter après coup si on
/// ne la refuse pas ici.
#[test]
fn une_vue_ne_peut_pas_contenir_un_evenement_posterieur_a_son_watermark() {
    let refused = ContextView::build(
        &[(item(1), 42)],
        &reviewer(),
        10,
        hash("ab"),
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect_err("la position 42 est au-delà du watermark 10");

    assert_eq!(
        refused,
        ContextViewError::BeyondWatermark {
            revision: revision(1),
            position: 42,
            watermark: 10
        }
    );
    assert!(
        refused.to_string().contains("l'avenir"),
        "le refus dit pourquoi : {refused}"
    );
}

#[test]
fn un_evenement_exactement_au_watermark_est_connaissable() {
    // La borne est inclusive : le watermark est « jusqu'où on a lu », pas « avant où ».
    let view = ContextView::build(
        &[(item(1), 10)],
        &reviewer(),
        10,
        hash("ab"),
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect("la position 10 est atteinte");
    assert_eq!(view.included(), [revision(1)]);
    assert!(view.could_know(10));
    assert!(!view.could_know(11));
}

/// La question que §16.2 existe pour rendre décidable. Reprocher à un agent d'avoir ignoré un
/// événement postérieur à sa vue reviendrait à lui reprocher de n'être pas devin.
#[test]
fn la_vue_dit_ce_qui_etait_connaissable_et_ce_qui_ne_l_etait_pas() {
    let view = ContextView::build(
        &[(item(1), 5)],
        &reviewer(),
        10,
        hash("ab"),
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect("vue valide");
    assert!(view.could_know(0));
    assert!(view.could_know(10));
    assert!(!view.could_know(999));
    assert_eq!(view.source_event_watermark(), 10);
}

// ---------------------------------------------------------------------------------------------
// Le hash adresse la vue
// ---------------------------------------------------------------------------------------------

/// « Adressée par hash » : deux vues du même instant, construites des mêmes éléments pour le même
/// destinataire, sont **la même vue**. C'est ce qui permet à une mission de citer une vue plutôt
/// que de la recopier.
#[test]
fn deux_vues_du_meme_instant_sont_la_meme_vue() {
    let candidates = [(item(1), 3), (item(2), 7)];
    let first = ContextView::build(
        &candidates,
        &reviewer(),
        10,
        hash("ab"),
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect("vue valide");
    let second = ContextView::build(
        &candidates,
        &reviewer(),
        10,
        hash("ab"),
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect("vue valide");

    assert_eq!(first, second);
    assert_eq!(first.content_hash(), second.content_hash());
    assert_eq!(first.included(), second.included());
}

/// Et deux vues d'instants différents ne sont pas la même, même sur les mêmes éléments : le
/// watermark fait partie de ce que la vue dit.
#[test]
fn deux_vues_d_instants_differents_ne_se_confondent_pas() {
    let candidates = [(item(1), 3)];
    let early = ContextView::build(
        &candidates,
        &reviewer(),
        5,
        hash("ab"),
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect("vue valide");
    let late = ContextView::build(
        &candidates,
        &reviewer(),
        50,
        hash("cd"),
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect("vue valide");

    assert_ne!(early, late);
    assert_ne!(
        early.source_event_watermark(),
        late.source_event_watermark()
    );
    assert_eq!(
        early.included(),
        late.included(),
        "le contenu est le même : c'est l'instant qui diffère, et c'est justement ce qui doit se voir"
    );
}

// ---------------------------------------------------------------------------------------------
// La vue se construit filtrée, et ce qu'elle écarte est nommé
// ---------------------------------------------------------------------------------------------

/// Le filtre est celui de W7.b, écrit **avant** cette vue — donc pas écrit pour qu'elle passe.
#[test]
fn un_element_contamine_n_entre_pas_dans_la_vue() {
    let mut leak = item(1);
    leak.is_generator_reasoning = true;

    let view = ContextView::build(
        &[(leak, 3), (item(2), 4)],
        &reviewer(),
        10,
        hash("ab"),
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect("vue valide");

    assert_eq!(view.included(), [revision(2)]);
    assert_eq!(view.redactions().len(), 1);
    assert_eq!(view.redactions()[0].revision, revision(1));
    assert!(
        view.redactions()[0]
            .reason
            .contains("generator_reasoning_leaked"),
        "{}",
        view.redactions()[0].reason
    );
}

/// §16.2 porte `redactions` : ce qui a été retiré **fait partie** de ce que la vue dit. Une
/// exclusion silencieuse rendrait deux vues indiscernables — celle qui n'avait rien à écarter et
/// celle qui a tout écarté.
#[test]
fn une_vue_qui_a_tout_ecarte_ne_ressemble_pas_a_une_vue_qui_n_avait_rien_a_ecarter() {
    let mut secret = item(1);
    secret.classification = Confidentiality::Restricted;

    let redacted = ContextView::build(
        &[(secret, 3)],
        &reviewer(),
        10,
        hash("ab"),
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect("vue valide");
    let empty = ContextView::build(
        &[],
        &reviewer(),
        10,
        hash("ab"),
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect("vue valide");

    assert!(redacted.included().is_empty());
    assert!(empty.included().is_empty());
    assert_ne!(
        redacted.redactions().len(),
        empty.redactions().len(),
        "les deux vues sont vides, et elles ne disent pas la même chose"
    );
}

#[test]
fn le_plafond_de_confidentialite_de_la_vue_est_celui_du_destinataire() {
    let mut cleared = reviewer();
    cleared.clearance = Confidentiality::Confidential;
    let view = ContextView::build(
        &[(item(1), 3)],
        &cleared,
        10,
        hash("ab"),
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect("vue valide");
    assert_eq!(
        view.confidentiality_ceiling(),
        Confidentiality::Confidential
    );
}

/// Deux destinataires d'habilitations différentes ne reçoivent pas la même vue des mêmes éléments.
/// C'est l'isolation informationnelle, vue depuis la construction plutôt que depuis le constat.
#[test]
fn deux_destinataires_ne_recoivent_pas_la_meme_vue() {
    let mut secret = item(1);
    secret.classification = Confidentiality::Restricted;
    let candidates = [(secret, 3), (item(2), 4)];

    let ordinary = ContextView::build(
        &candidates,
        &reviewer(),
        10,
        hash("ab"),
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect("vue valide");

    let mut cleared = reviewer();
    cleared.clearance = Confidentiality::Restricted;
    let privileged = ContextView::build(
        &candidates,
        &cleared,
        10,
        hash("ab"),
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect("vue valide");

    assert_eq!(ordinary.included(), [revision(2)]);
    assert_eq!(privileged.included(), [revision(1), revision(2)]);
}

// ---------------------------------------------------------------------------------------------
// L'immuabilité
// ---------------------------------------------------------------------------------------------

/// Une vue ne s'augmente pas. Il n'existe aucune méthode qui ajoute un élément après construction,
/// et ce test dit la conséquence observable : voir plus demande une **autre** vue, avec son propre
/// watermark et son propre hash. Une vue qui grandirait cesserait de dire ce que l'agent pouvait
/// connaître au moment où elle a été arrêtée.
#[test]
fn voir_plus_demande_une_autre_vue() {
    let first = ContextView::build(
        &[(item(1), 3)],
        &reviewer(),
        5,
        hash("ab"),
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect("vue valide");
    let second = ContextView::build(
        &[(item(1), 3), (item(2), 8)],
        &reviewer(),
        10,
        hash("cd"),
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect("vue valide");

    assert_eq!(first.included().len(), 1);
    assert_eq!(second.included().len(), 2);
    assert_ne!(first.content_hash(), second.content_hash());
    assert_ne!(
        first.source_event_watermark(),
        second.source_event_watermark()
    );
}
