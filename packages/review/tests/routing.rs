//! Test de sortie de `W24.b` — **l'appariement sémantique dans l'ensemble autorisé.**
//!
//! Trois clauses, celles de la roadmap :
//!
//! 1. l'ensemble candidat est calculé **avant** l'appariement, et l'appariement ne peut pas
//!    l'élargir, **tenu par la signature** ;
//! 2. un test exhibe un pair **sémantiquement parfait et non autorisé**, et vérifie qu'il n'est
//!    jamais sélectionné ;
//! 3. l'aveuglement du reviewer survit à tout appariement, exercé sur une **revue indépendante
//!    réelle**.
//!
//! La deuxième est celle qui porte l'item : c'est le cas exact que la source dont l'ADR 0026 tire ce
//! mécanisme laisserait passer, puisque chez elle la souscription vient de l'agent.

use locus_domain::{Confidentiality, ContentHash, RevisionId};
use locus_protocol::{Id, IdKind, Timestamp, id::Agent};
use locus_review::dossier::{Blindness, Draft, IndependenceRequirement};
use locus_review::review::{Party, attest};
use locus_review::{Audience, ContextItem, ContextView, Peer, Recipient, Subscription};

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

fn destinataire(seed: u8) -> Recipient {
    Recipient {
        agent_id: id::<Agent>(seed),
        worker_id: format!("vm-{seed:02}"),
        blind_to_generator: true,
        clearance: Confidentiality::Internal,
    }
}

/// Un pair autorisé sur les révisions données.
fn pair(seed: u8, revisions: &[u8]) -> Peer {
    let candidates: Vec<(ContextItem, u64)> =
        revisions.iter().map(|graine| (item(*graine), 1)).collect();
    let vue = ContextView::build(
        &candidates,
        &destinataire(seed),
        10,
        hash("ab"),
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect("la fixture est cohérente");
    Peer::authorised(destinataire(seed), Subscription::derived_from(&vue))
}

// ---------------------------------------------------------------------------------------------
// 1. L'ensemble est clos avant l'appariement
// ---------------------------------------------------------------------------------------------

/// L'appariement rend un membre **de l'audience**, et rien d'autre.
///
/// C'est la clause 1 lue au niveau où elle se tient : le type de retour emprunte à l'audience, donc
/// un pair extérieur n'a aucune façon de sortir de cette fonction. Ce test le constate ; le
/// compilateur le garantit.
#[test]
fn l_apparie_est_toujours_un_membre_de_l_audience() {
    let audience = Audience::of(vec![pair(2, &[1]), pair(3, &[1, 2]), pair(4, &[1])]);
    let choisi = audience
        .best(|peer| i64::try_from(peer.subscription().revisions().len()).unwrap_or(0))
        .expect("l'audience n'est pas vide");

    assert!(
        audience.members().contains(choisi),
        "l'apparié sort de l'audience"
    );
    assert_eq!(choisi.recipient().agent_id, id::<Agent>(3));
}

/// Une audience **vide** ne rend personne, et surtout pas un défaut.
///
/// « Personne n'est autorisé » et « je n'ai pas su choisir » sont la même valeur ici, et c'est bien :
/// dans les deux cas il n'y a rien à qui s'adresser. Ce qui serait faux serait un destinataire par
/// défaut, c'est-à-dire un pair que personne n'a autorisé.
#[test]
fn une_audience_vide_ne_rend_personne() {
    let vide = Audience::of(Vec::new());
    assert!(vide.is_empty());
    assert!(vide.best(|_| 1_000).is_none());
}

/// À score égal, le **premier** — l'appariement est reproductible.
///
/// Deux rejeux du même journal doivent choisir le même pair, sans quoi la trace ne dit plus ce qui
/// s'est passé. Même arbitrage que `place` de `W4.g`.
#[test]
fn a_score_egal_l_appariement_est_reproductible() {
    let audience = Audience::of(vec![pair(2, &[1]), pair(3, &[1]), pair(4, &[1])]);
    for _ in 0..5 {
        let choisi = audience.best(|_| 7).expect("l'audience n'est pas vide");
        assert_eq!(choisi.recipient().agent_id, id::<Agent>(2));
    }
}

// ---------------------------------------------------------------------------------------------
// 2. Le pair sémantiquement parfait et non autorisé
// ---------------------------------------------------------------------------------------------

/// **Un pair parfait mais absent de l'audience n'est jamais sélectionné.**
///
/// C'est la clause qui porte l'item, et le cas exact que la source de l'ADR 0026 laisserait passer :
/// chez elle, la souscription vient de l'agent, donc le pair parfait s'autorise lui-même.
///
/// La fonction d'affinité ci-dessous fait tout ce qu'un mécanisme sémantique peut faire — elle
/// **connaît** le pair idéal, elle le reconnaîtrait entre mille, et elle donne à tous les autres un
/// score dérisoire. Elle ne peut pas pour autant le faire élire : elle ne voit qu'un membre à la fois
/// et rend un nombre. Il n'y a rien à quoi l'ajouter.
#[test]
fn un_pair_parfait_et_non_autorise_n_est_jamais_selectionne() {
    let parfait = pair(9, &[1, 2, 3]);
    let audience = Audience::of(vec![pair(2, &[1]), pair(3, &[1])]);

    let choisi = audience
        .best(|peer| if peer == &parfait { i64::MAX } else { -1 })
        .expect("l'audience n'est pas vide");

    assert_ne!(choisi, &parfait);
    assert_ne!(choisi.recipient().agent_id, id::<Agent>(9));
    assert!(audience.members().contains(choisi));
    assert!(
        !audience.members().contains(&parfait),
        "le pair parfait n'a jamais été autorisé"
    );
}

/// L'appariement ne change pas **ce que le pair reçoit**.
///
/// Choisir un destinataire et élargir son accès sont deux actes différents, et le second n'appartient
/// pas à ce module. La souscription de l'apparié est celle qu'il avait avant — `W24.a` la rend
/// dérivable de la seule `ContextView`, et rien ici ne la reconstruit.
#[test]
fn l_appariement_ne_change_pas_ce_que_le_pair_recoit() {
    let etroit = pair(2, &[1]);
    let avant = etroit.subscription().clone();
    let audience = Audience::of(vec![etroit, pair(3, &[1, 2])]);

    let choisi = audience
        .best(|peer| {
            if peer.recipient().agent_id == id::<Agent>(2) {
                10
            } else {
                0
            }
        })
        .expect("l'audience n'est pas vide");

    assert_eq!(choisi.subscription(), &avant);
    assert!(!choisi.subscription().admits(&revision(2)));
}

// ---------------------------------------------------------------------------------------------
// 3. L'aveuglement survit à l'appariement, sur une revue indépendante réelle
// ---------------------------------------------------------------------------------------------

/// **L'attestation d'indépendance est la même avant et après l'appariement.**
///
/// Pas « l'appariement respecte l'aveuglement » — ce qui supposerait qu'il le regarde — mais
/// « l'appariement n'a rien à voir avec lui ». L'indépendance est une propriété du dossier et des
/// parties, constatée par `attest`, et ce module ne porte aucune fonction qui la prenne.
///
/// La revue est réelle : un dossier gelé, ses trois exigences de §17.1, un générateur et un relecteur
/// sur des workers distincts et dans des groupes distincts.
#[test]
fn l_aveuglement_du_reviewer_survit_a_l_appariement() {
    let dossier = Draft::open("dossier-1", vec![revision(1)])
        .expect("un dossier avec une cible")
        .asking("la méthode tient-elle ?")
        .blind_to(Blindness::GeneratorTranscript)
        .requiring(IndependenceRequirement::DistinctIndependenceGroup)
        .requiring(IndependenceRequirement::DistinctWorker)
        .requiring(IndependenceRequirement::NoGeneratorTranscript)
        .freeze(hash("ab"))
        .expect("le dossier est complet");

    let generateur = Party {
        agent_id: id::<Agent>(1),
        worker_id: "vm-01".to_owned(),
        independence_group: Some("groupe-a".to_owned()),
        holds_generator_transcript: true,
    };
    let relecteur = Party {
        agent_id: id::<Agent>(3),
        worker_id: "vm-03".to_owned(),
        independence_group: Some("groupe-b".to_owned()),
        holds_generator_transcript: false,
    };

    let avant = attest(&dossier, &generateur, &relecteur);
    assert!(avant.holds(), "la fixture est bien une revue indépendante");

    // On apparie, avec une affinité qui vise précisément ce relecteur.
    let audience = Audience::of(vec![pair(2, &[1]), pair(3, &[1])]);
    let choisi = audience
        .best(|peer| {
            if peer.recipient().agent_id == relecteur.agent_id {
                100
            } else {
                0
            }
        })
        .expect("l'audience n'est pas vide");
    assert_eq!(choisi.recipient().agent_id, relecteur.agent_id);

    let apres = attest(&dossier, &generateur, &relecteur);
    assert_eq!(apres, avant, "apparier ne constate rien de nouveau");
    assert!(apres.holds());
    assert!(
        choisi.recipient().blind_to_generator,
        "l'aveuglement du destinataire n'a pas bougé"
    );
}

/// Le module d'appariement **ne voit** ni dossier, ni partie, ni attestation.
///
/// Tenu par l'absence, dans la source : la seule façon de garantir qu'apparier ne négocie pas
/// l'indépendance est qu'il n'ait pas de quoi la lire. Une garde qui vérifierait « l'appariement n'a
/// pas modifié l'attestation » supposerait qu'il puisse la toucher, ce qui est déjà trop.
#[test]
fn le_module_d_appariement_ne_voit_pas_l_independance() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/routing.rs"))
        .expect("le module de production est lisible depuis son propre crate");
    let code: String = source
        .lines()
        .filter(|ligne| !ligne.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    for interdit in [
        "Frozen",
        "Party",
        "attest",
        "IndependenceAttestation",
        "Blindness",
    ] {
        assert!(
            !code.contains(interdit),
            "« {interdit} » : apparier n'a pas à connaître l'indépendance pour ne pas la négocier"
        );
    }
}
