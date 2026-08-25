//! Test de sortie de W7.b — **les cinq formes de contamination de §16.6, chacune par un cas
//! adverse qui essaie de la produire.**
//!
//! # Pourquoi « adverse » et pas « par construction »
//!
//! `docs/10` signale ce point comme facile à rater : la prévention « doit être testée par un cas
//! adverse explicite et **pas seulement par construction** ». La différence est celle entre « je ne
//! vois pas comment ça arriverait » et « voici comment on le fait arriver ».
//!
//! Chaque cas ci-dessous **construit** la contamination, puis exige qu'elle soit vue. Un test qui
//! se contenterait de vérifier qu'un contexte propre reste propre ne dirait rien : c'est le cas
//! facile, et c'est celui qu'on obtient sans y penser.

use locus_domain::{Confidentiality, RevisionId};
use locus_protocol::{Id, IdKind, Timestamp, id::Agent};
use locus_review::{
    Contamination, ContextItem, Recipient, contamination::Finding, contradictions_dropped, inspect,
};

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

/// Un élément de contexte sain, que chaque cas adverse abîme d'une seule façon.
///
/// Partir du sain et n'introduire **qu'un** défaut est ce qui rend le constat attribuable : si le
/// cas partait d'un élément déjà douteux sur trois plans, on ne saurait pas lequel a été vu.
fn clean(seed: u8) -> ContextItem {
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

fn blind_reviewer() -> Recipient {
    Recipient {
        agent_id: id::<Agent>(2),
        worker_id: "vm-02".to_owned(),
        blind_to_generator: true,
        clearance: Confidentiality::Internal,
    }
}

fn kinds(findings: &[Finding]) -> Vec<Contamination> {
    findings.iter().map(|finding| finding.kind).collect()
}

// ---------------------------------------------------------------------------------------------
// Le cas facile, pour que les cinq autres veuillent dire quelque chose
// ---------------------------------------------------------------------------------------------

#[test]
fn un_contexte_sain_ne_produit_aucun_constat() {
    let findings = inspect(
        &[clean(1), clean(2)],
        &blind_reviewer(),
        Timestamp::from_millis(1_700_000_000_000),
    );
    assert!(findings.is_empty(), "{findings:#?}");
}

// ---------------------------------------------------------------------------------------------
// 1 — Le raisonnement du générateur atteint un relecteur aveugle
// ---------------------------------------------------------------------------------------------

/// L'attaque : glisser le transcript de génération dans le contexte d'un relecteur que la politique
/// rend aveugle. C'est l'invariant 11 pris de face — et c'est la forme la plus banale, parce
/// qu'elle ressemble à « donner du contexte utile ».
#[test]
fn adverse_le_transcript_du_generateur_glisse_dans_le_contexte_du_relecteur() {
    let mut leak = clean(1);
    leak.is_generator_reasoning = true;

    let findings = inspect(
        &[leak, clean(2)],
        &blind_reviewer(),
        Timestamp::from_millis(1_700_000_000_000),
    );
    assert_eq!(
        kinds(&findings),
        vec![Contamination::GeneratorReasoningLeaked]
    );
    assert_eq!(findings[0].revision, revision(1));
}

/// Et le même élément, pour un relecteur **non** aveugle, n'est pas une contamination : la
/// politique décide, pas la nature de l'élément. Sans ce cas, la détection serait « tout transcript
/// est interdit », ce que §16.6 ne dit pas.
#[test]
fn le_meme_transcript_n_est_pas_une_fuite_pour_un_relecteur_non_aveugle() {
    let mut reasoning = clean(1);
    reasoning.is_generator_reasoning = true;

    let mut informed = blind_reviewer();
    informed.blind_to_generator = false;

    assert!(
        inspect(
            &[reasoning],
            &informed,
            Timestamp::from_millis(1_700_000_000_000)
        )
        .is_empty()
    );
}

// ---------------------------------------------------------------------------------------------
// 2 — Un claim réfuté se propage comme contexte par défaut
// ---------------------------------------------------------------------------------------------

/// L'attaque : laisser une revendication réfutée dans le contexte, où elle sera lue comme un acquis.
/// Elle n'est pas effacée du graphe — l'invariant 12 l'interdit — mais la garder **et** la servir
/// par défaut sont deux choses différentes.
#[test]
fn adverse_un_claim_refute_reste_dans_le_contexte_par_defaut() {
    let mut refuted = clean(1);
    refuted.is_refuted = true;

    let findings = inspect(
        &[refuted, clean(2)],
        &blind_reviewer(),
        Timestamp::from_millis(1_700_000_000_000),
    );
    assert_eq!(
        kinds(&findings),
        vec![Contamination::RefutedClaimPropagated]
    );
    assert!(
        findings[0].detail.contains("réfutée"),
        "{}",
        findings[0].detail
    );
}

// ---------------------------------------------------------------------------------------------
// 3 — Une donnée confidentielle atteint un worker non autorisé
// ---------------------------------------------------------------------------------------------

/// L'attaque : faire passer une donnée `restricted` à un relecteur dont le worker n'est habilité
/// qu'à `internal`. Le plafond de §16.2 est un **ordre**, donc la comparaison est décidable — et
/// c'est ce qui permet de refuser sans énumérer toutes les combinaisons.
#[test]
fn adverse_une_donnee_restreinte_atteint_un_worker_habilite_a_moins() {
    let mut secret = clean(1);
    secret.classification = Confidentiality::Restricted;

    let findings = inspect(
        &[secret],
        &blind_reviewer(),
        Timestamp::from_millis(1_700_000_000_000),
    );
    assert_eq!(
        kinds(&findings),
        vec![Contamination::ConfidentialDataOnUnauthorisedWorker]
    );
}

/// Le plafond laisse passer ce qui est **en dessous**, et c'est ce qui distingue un plafond d'une
/// égalité stricte : un relecteur habilité `confidential` lit aussi du `public`.
#[test]
fn un_plafond_laisse_passer_ce_qui_est_en_dessous() {
    let mut public = clean(1);
    public.classification = Confidentiality::Public;
    let mut cleared = blind_reviewer();
    cleared.clearance = Confidentiality::Confidential;

    assert!(
        inspect(
            &[public],
            &cleared,
            Timestamp::from_millis(1_700_000_000_000)
        )
        .is_empty()
    );

    let mut secret = clean(2);
    secret.classification = Confidentiality::Restricted;
    assert_eq!(
        kinds(&inspect(
            &[secret],
            &cleared,
            Timestamp::from_millis(1_700_000_000_000)
        )),
        vec![Contamination::ConfidentialDataOnUnauthorisedWorker],
        "et il refuse ce qui est au-dessus"
    );
}

// ---------------------------------------------------------------------------------------------
// 4 — Consensus circulaire
// ---------------------------------------------------------------------------------------------

/// L'attaque : deux agents qui se citent mutuellement, sans qu'aucun ne cite de source externe. La
/// conviction se soutient d'elle-même, et rien dans le contexte ne le signale — chaque élément a
/// l'air correctement sourcé, puisqu'il cite quelque chose.
#[test]
fn adverse_deux_agents_se_citent_mutuellement_sans_source_externe() {
    let mut first = clean(1);
    first.is_external_source = false;
    first.cites = vec![revision(2)];
    let mut second = clean(2);
    second.is_external_source = false;
    second.cites = vec![revision(1)];

    let findings = inspect(
        &[first, second],
        &blind_reviewer(),
        Timestamp::from_millis(1_700_000_000_000),
    );
    assert_eq!(
        kinds(&findings),
        vec![
            Contamination::CircularConsensus,
            Contamination::CircularConsensus
        ],
        "les deux membres du cycle sont signalés : réparer d'un côté ne suffit pas"
    );
}

/// Un cycle **avec** source externe n'est pas un consensus circulaire : les deux agents s'appuient
/// sur quelque chose. Sans ce cas, la détection interdirait toute citation mutuelle, ce qui
/// interdirait la discussion.
#[test]
fn un_cycle_qui_cite_une_source_externe_n_est_pas_circulaire() {
    let mut first = clean(1);
    first.is_external_source = false;
    first.cites = vec![revision(2)];
    let mut second = clean(2);
    second.is_external_source = true; // celui-ci cite le monde extérieur
    second.cites = vec![revision(1)];

    assert!(
        inspect(
            &[first, second],
            &blind_reviewer(),
            Timestamp::from_millis(1_700_000_000_000)
        )
        .is_empty()
    );
}

/// Un cycle plus long, pour que la détection ne soit pas « A cite B qui cite A » en dur.
#[test]
fn adverse_un_cycle_a_trois_est_aussi_circulaire() {
    let mut first = clean(1);
    first.is_external_source = false;
    first.cites = vec![revision(2)];
    let mut second = clean(2);
    second.is_external_source = false;
    second.cites = vec![revision(3)];
    let mut third = clean(3);
    third.is_external_source = false;
    third.cites = vec![revision(1)];

    let findings = inspect(
        &[first, second, third],
        &blind_reviewer(),
        Timestamp::from_millis(1_700_000_000_000),
    );
    assert_eq!(findings.len(), 3);
    assert!(
        findings
            .iter()
            .all(|finding| finding.kind == Contamination::CircularConsensus)
    );
}

// ---------------------------------------------------------------------------------------------
// 5 — Une contradiction se perd à la synthèse
// ---------------------------------------------------------------------------------------------

/// L'attaque : produire une synthèse qui ne mentionne pas une contradiction connue. C'est la forme
/// la plus difficile à voir, parce que la synthèse amputée est **plus lisible** que celle qui garde
/// la contradiction — elle a l'air meilleure.
#[test]
fn adverse_une_synthese_oublie_une_contradiction_connue() {
    let known = [revision(1), revision(2)];
    let mentioned = [revision(1)];

    let findings = contradictions_dropped(&known, &mentioned);
    assert_eq!(kinds(&findings), vec![Contamination::ContradictionDropped]);
    assert_eq!(findings[0].revision, revision(2));
}

#[test]
fn une_synthese_qui_garde_toutes_les_contradictions_ne_produit_rien() {
    let known = [revision(1), revision(2)];
    assert!(contradictions_dropped(&known, &known).is_empty());
}

// ---------------------------------------------------------------------------------------------
// Les cinq formes existent, et l'inspection les rend toutes
// ---------------------------------------------------------------------------------------------

#[test]
fn les_cinq_formes_de_16_6_sont_nommees() {
    let slugs: Vec<&str> = Contamination::ALL
        .into_iter()
        .map(Contamination::slug)
        .collect();
    assert_eq!(
        slugs,
        vec![
            "generator_reasoning_leaked",
            "refuted_claim_propagated",
            "confidential_data_on_unauthorised_worker",
            "circular_consensus",
            "contradiction_dropped"
        ],
        "§16.6 en nomme cinq ; en oublier une la rendrait indétectable et rien ne le dirait"
    );
}

/// Une contamination trouvée n'exclut pas les autres. S'arrêter au premier constat ferait réparer
/// une fuite en laissant les suivantes — et le rapport donnerait l'impression du contraire.
#[test]
fn un_contexte_contamine_de_trois_facons_produit_trois_constats() {
    let mut wrong = clean(1);
    wrong.is_generator_reasoning = true;
    wrong.is_refuted = true;
    wrong.classification = Confidentiality::Restricted;

    let findings = inspect(
        &[wrong],
        &blind_reviewer(),
        Timestamp::from_millis(1_700_000_000_000),
    );
    assert_eq!(findings.len(), 3);
    assert_eq!(
        kinds(&findings),
        vec![
            Contamination::GeneratorReasoningLeaked,
            Contamination::RefutedClaimPropagated,
            Contamination::ConfidentialDataOnUnauthorisedWorker
        ]
    );
}
