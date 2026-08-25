//! Test de sortie de `W24.a` — **la souscription dérivée de la `ContextView`.**
//!
//! Trois clauses, celles de la roadmap :
//!
//! 1. une souscription se calcule de la `ContextView` **et de rien d'autre** ;
//! 2. **aucun chemin de type ne permet à un agent d'écrire sa propre souscription**, tenu par
//!    l'absence ;
//! 3. un agent qui demande un élargissement passe par la demande d'extension existante, jamais par
//!    sa souscription — et un test exhibe les deux chemins pour montrer qu'ils ne se rejoignent pas.
//!
//! # La clause 3 a été vérifiée avant d'être codée
//!
//! Elle parle d'une « demande d'extension **existante** », et trois clauses fausses avaient déjà été
//! trouvées dans la journée. Elle existe : `context.extension_requested`, dans
//! `canterel/backend/cli/src/locus/context-materializer.ts`. La vérification a coûté un item — la
//! citation la rattache à `repos/canterel/SPEC_V1.md` §12.4, dont le numéro désigne **ici** la
//! backpressure, et c'est ce qui a produit `W0.21`.

use locus_domain::{Confidentiality, ContentHash, RevisionId};
use locus_protocol::{Id, IdKind, Timestamp, id::Agent};
use locus_review::{ContextItem, ContextView, Recipient, Subscription};

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

fn item(seed: u8, classification: Confidentiality) -> ContextItem {
    ContextItem {
        revision: revision(seed),
        is_generator_reasoning: false,
        is_refuted: false,
        classification,
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

/// Une vue sur les révisions données, arrêtée au watermark 10.
fn vue(seeds: &[u8]) -> ContextView {
    let candidates: Vec<(ContextItem, u64)> = seeds
        .iter()
        .map(|seed| (item(*seed, Confidentiality::Internal), 1))
        .collect();
    ContextView::build(
        &candidates,
        &reviewer(),
        10,
        hash("ab"),
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect("la fixture est cohérente")
}

// ---------------------------------------------------------------------------------------------
// 1. De la vue, et de rien d'autre
// ---------------------------------------------------------------------------------------------

/// Les quatre valeurs de la souscription viennent de la vue, sans exception.
///
/// C'est la clause 1 prise au mot : « de la `ContextView` **et de rien d'autre** ». Chaque champ est
/// confronté à son origine plutôt qu'à une constante, ce qui est la façon de vérifier qu'aucun n'a
/// une seconde source.
#[test]
fn tout_ce_que_porte_la_souscription_vient_de_la_vue() {
    let vue = vue(&[1, 2, 3]);
    let souscription = Subscription::derived_from(&vue);

    assert_eq!(souscription.revisions().len(), vue.included().len());
    for incluse in vue.included() {
        assert!(souscription.admits(incluse), "{incluse} est dans la vue");
    }
    assert_eq!(souscription.ceiling(), vue.confidentiality_ceiling());
    assert_eq!(souscription.watermark(), vue.source_event_watermark());
    assert_eq!(souscription.view(), vue.content_hash());
}

/// Ce que la vue **n'inclut pas** n'est pas souscrit.
///
/// Le symétrique du test précédent, et il ne s'en déduit pas : une souscription qui admettrait tout
/// passerait le premier sans broncher.
#[test]
fn ce_qui_n_est_pas_dans_la_vue_n_est_pas_souscrit() {
    let souscription = Subscription::derived_from(&vue(&[1, 2]));
    assert!(souscription.admits(&revision(1)));
    assert!(
        !souscription.admits(&revision(9)),
        "la révision 9 n'a jamais été candidate"
    );
}

/// Le plafond est celui de la vue, et il s'applique dans les deux sens.
///
/// Une garde qui ne dirait que « refusé » serait exacte et inutile — c'est l'arbitrage que `W2.23` a
/// posé pour `remote_inference`, et il vaut ici à l'identique.
#[test]
fn le_plafond_de_la_vue_s_applique_dans_les_deux_sens() {
    let souscription = Subscription::derived_from(&vue(&[1]));
    assert_eq!(souscription.ceiling(), Confidentiality::Internal);
    assert!(souscription.clears(Confidentiality::Public));
    assert!(souscription.clears(Confidentiality::Internal));
    assert!(!souscription.clears(Confidentiality::Confidential));
    assert!(!souscription.clears(Confidentiality::Restricted));
}

/// Deux vues différentes donnent des souscriptions **distinguables**, même à révisions égales.
///
/// C'est ce que le condensat porte : sans lui, une autorisation survivrait à la vue qui la fondait,
/// et rien ne dirait laquelle des deux on tient.
#[test]
fn deux_vues_donnent_des_souscriptions_distinguables() {
    let une = ContextView::build(
        &[(item(1, Confidentiality::Internal), 1)],
        &reviewer(),
        10,
        hash("ab"),
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect("cohérente");
    let autre = ContextView::build(
        &[(item(1, Confidentiality::Internal), 1)],
        &reviewer(),
        10,
        hash("cd"),
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect("cohérente");

    let premiere = Subscription::derived_from(&une);
    let seconde = Subscription::derived_from(&autre);
    assert_eq!(premiere.revisions(), seconde.revisions());
    assert_ne!(premiere.view(), seconde.view());
    assert_ne!(premiere, seconde);
}

/// La même vue donne **la même** souscription : dériver n'introduit rien.
#[test]
fn deriver_deux_fois_la_meme_vue_rend_la_meme_souscription() {
    let vue = vue(&[1, 2]);
    assert_eq!(
        Subscription::derived_from(&vue),
        Subscription::derived_from(&vue)
    );
}

// ---------------------------------------------------------------------------------------------
// 2. Aucun chemin de type pour qu'un agent écrive la sienne
// ---------------------------------------------------------------------------------------------

/// **Un seul constructeur, et il prend une vue.**
///
/// La clause dit « tenu par l'absence », et l'absence se lit dans la source. Un test qui essaierait
/// d'appeler un constructeur inexistant ne compilerait pas — donc ne dirait rien à qui ajoute le
/// constructeur plus tard, puisqu'il compilerait alors. Ce qui suit reste vrai dans les deux cas :
/// il **compte** les portes d'entrée.
///
/// Même arbitrage que `W23.a`, dont le test lit `Cargo.toml`, et que `W20.ae`, dont le test refuse
/// les littéraux dans la source : la propriété voulue est « personne ne **peut** », pas « personne
/// n'a encore ».
#[test]
fn le_type_n_a_qu_une_porte_d_entree_et_elle_prend_une_vue() {
    let source =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/subscription.rs"))
            .expect("le module de production est lisible depuis son propre crate");

    let code: Vec<&str> = source
        .lines()
        .filter(|ligne| !ligne.trim_start().starts_with("//"))
        .collect();

    let constructeurs: Vec<&&str> = code
        .iter()
        .filter(|ligne| ligne.contains("pub fn") || ligne.contains("pub const fn"))
        .filter(|ligne| ligne.contains("-> Self") || ligne.contains("derived_from"))
        .collect();
    assert_eq!(
        constructeurs.len(),
        1,
        "une seule porte d'entrée, et ce sont celles-ci : {constructeurs:?}"
    );
    assert!(constructeurs[0].contains("derived_from(view: &ContextView)"));

    for interdit in [
        "impl Default",
        "Deserialize",
        "pub revisions:",
        "pub ceiling:",
    ] {
        assert!(
            !code.join("\n").contains(interdit),
            "« {interdit} » ouvrirait un second chemin"
        );
    }
}

/// Le crate ne **peut** pas désérialiser une souscription.
///
/// `packages/review` ne dépend de `serde` sous aucune forme : il n'y existe donc aucun type
/// désérialisable, et a fortiori aucun qu'un agent pourrait envoyer sur le fil pour se déclarer une
/// souscription. Le test lit le manifeste plutôt que les sources — chercher un `derive` laisserait
/// passer une implémentation manuelle, et la propriété n'est pas « personne n'a dérivé ».
#[test]
fn le_crate_ne_peut_pas_deserialiser_une_souscription() {
    let manifeste = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
        .expect("le crate lit son propre manifeste");
    assert!(
        !manifeste.contains("serde"),
        "une souscription venue du fil serait une souscription déclarée par l'agent"
    );
}

// ---------------------------------------------------------------------------------------------
// 3. Les deux chemins ne se rejoignent pas
// ---------------------------------------------------------------------------------------------

/// **Élargir demande de reconstruire la vue**, et l'agent n'en a pas les moyens.
///
/// Le chemin A — la souscription — est en lecture seule : rien n'y prend une révision et l'y ajoute.
/// Le chemin B — l'élargissement — repasse par `ContextView::build`, qui exige les **candidats** et
/// le **destinataire**. Un agent ne fournit ni l'un ni l'autre : il émet
/// `context.extension_requested`, et c'est le plan de contrôle qui décide.
///
/// Ce test exhibe les deux bouts. Ce qu'il montre n'est pas qu'une fonction refuse, mais qu'il n'y a
/// **rien à appeler** : la souscription élargie n'existe qu'après une seconde construction de vue,
/// faite avec des entrées que l'agent n'a pas.
#[test]
fn elargir_passe_par_la_vue_et_jamais_par_la_souscription() {
    let etroite = Subscription::derived_from(&vue(&[1]));
    assert!(!etroite.admits(&revision(2)));

    // Le plan de contrôle admet un candidat de plus — c'est **lui** qui tient les candidats et le
    // destinataire, et c'est tout ce que « décision Locus Solus » veut dire.
    let large = Subscription::derived_from(&vue(&[1, 2]));
    assert!(large.admits(&revision(2)));

    // Et la souscription étroite n'a pas bougé : elle n'est pas un état qu'on modifie, c'est une
    // dérivation qu'on refait.
    assert!(!etroite.admits(&revision(2)));
    assert_ne!(etroite, large);
}

/// **Aucune fonction ne prend une révision et élargit une souscription.**
///
/// Le complément du test précédent, au niveau de la surface : le module n'expose ni `grant`, ni
/// `extend`, ni `insert`, ni `with_revision`. C'est la moitié Rust de ce que le worker tient déjà de
/// son côté — « il n'existe volontairement aucune fonction qui l'accorde ».
#[test]
fn aucune_fonction_n_elargit_une_souscription() {
    let source =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/subscription.rs"))
            .expect("le module de production est lisible depuis son propre crate");
    let code: String = source
        .lines()
        .filter(|ligne| !ligne.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    for interdit in [
        "fn grant",
        "fn extend",
        "fn insert",
        "fn with_revision",
        "fn allow",
        "&mut self",
    ] {
        assert!(
            !code.contains(interdit),
            "« {interdit} » rejoindrait les deux chemins que §12.4 sépare"
        );
    }
}
