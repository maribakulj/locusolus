//! Test de sortie de `W16.e`, seconde moitié — **émettre un message écrit un événement, et un seul.**
//!
//! La moitié qui vit dans `packages/coordination` tient les epochs, les trois verdicts et le passage
//! de témoin. Celle-ci tient ce que l'ADR 0019 appelle sa condition 1 : **aucun second stockage
//! durable**. Elle le vérifie sur le journal réel après la transaction, et non sur ce que
//! `decide` rend — compter ce qu'une fonction rend ne dit rien de ce qu'elle a écrit ailleurs en
//! chemin, et c'est précisément la faute qu'un courtier introduirait.

use locus_coordination::CoordinationMode;
use locus_coordination::messaging::Message;
use locus_coordination::version::{Digest, Version};
use locus_domain::ContentHash;
use locus_event_store::{EVENT_NAMESPACES, EventStore, EventType};
use locus_protocol::id::{Agent, Command, Event, Project, Workspace};
use locus_protocol::{Id, IdKind, Timestamp};
use locusd::messaging::{MessageContext, Send};
use locusd::{Revision, Runtime};

const NOW: Timestamp = Timestamp::from_millis(1_700_000_000_000);

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(NOW, entropy).expect("l'instant de fixture tient sur 48 bits")
}

struct Fnv;

impl Digest for Fnv {
    fn digest(&self, canonical: &str) -> ContentHash {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for byte in canonical.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        ContentHash::parse(&format!("sha256:{hash:064x}")).expect("forme de condensat valide")
    }
}

fn commande(seed: u8) -> locusd::CommandEnvelope {
    commande_a(seed, Revision::INITIAL)
}

/// La même, avec la révision de stream que l'émetteur croit voir.
///
/// Elle est **exigée** et non devinée : c'est le contrôle de concurrence optimiste de §22.2, et il
/// s'est manifesté dès le second message de ce fichier — la seconde émission a été refusée par un
/// `Conflict { expected: 0, current: 1 }` tant que le test réutilisait la révision initiale. Le
/// refus est le bon comportement : deux émetteurs qui écrivent dans la même boîte sans se voir
/// écraseraient l'ordre que §10.2 garantit.
fn commande_a(seed: u8, attendue: Revision) -> locusd::CommandEnvelope {
    locusd::CommandEnvelope::mutating(
        id::<Command>(seed),
        "message.send",
        id::<Workspace>(2),
        id::<Agent>(3),
        format!("idem-{seed}"),
        attendue,
    )
    .expect("commande bien formée")
}

fn contexte() -> MessageContext {
    MessageContext {
        project_id: id::<Project>(4),
        event_id: id::<Event>(9),
        occurred_at: NOW,
        payload_hash: format!("sha256:{}", "ab".repeat(32)),
    }
}

fn epoque() -> Version {
    Version::root(
        &[id::<Agent>(1), id::<Agent>(2)],
        &[],
        CoordinationMode::Blackboard,
        None,
        &Fnv,
    )
    .expect("version racine")
}

// ---------------------------------------------------------------------------------------------
// 1. `message` est une famille du journal, signalée comme ajout local
// ---------------------------------------------------------------------------------------------

/// La famille entre **avec son consommateur**, et pas avant.
///
/// §10.3 ne cite `message` nulle part : c'est l'ADR 0019 qui l'ajoute, en décidant qu'un message est
/// un événement plutôt qu'un transport parallèle. Le test tient les deux moitiés de cette phrase —
/// la famille existe, et elle est bien la dernière de la liste, là où les ajouts locaux sont
/// signalés plutôt que fondus dans la partie normative.
#[test]
fn message_est_une_famille_du_journal() {
    assert!(EVENT_NAMESPACES.contains(&"message"));
    assert_eq!(EVENT_NAMESPACES.last(), Some(&"message"));

    let kind = EventType::parse("message.sent").expect("famille reconnue");
    assert_eq!(kind.namespace(), "message");
    assert_eq!(kind.verb(), "sent");
}

// ---------------------------------------------------------------------------------------------
// 2. Aucun second stockage durable
// ---------------------------------------------------------------------------------------------

/// **Un message écrit un événement, et le journal est le seul endroit où il atterrit.**
///
/// C'est la condition 1 de l'ADR 0019, et l'argument par lequel le courtier dédié a été écarté : deux
/// stockages durables du même fait sont deux vérités, qui divergent le jour où l'une est purgée.
///
/// Le test compte les faits **dans le journal**, pas dans ce que `decide` a rendu. La différence
/// n'est pas rhétorique : une implémentation qui rendrait un brouillon et pousserait en plus dans
/// une file passerait le second test et échouerait celui-ci.
#[test]
fn emettre_un_message_ecrit_un_evenement_et_un_seul() {
    let runtime = Runtime::in_memory();
    let epoch = epoque();
    let handler = Send {
        message: Message::sent(id::<Agent>(1), id::<Agent>(2), &epoch, "revue demandée"),
    };

    let accepte = runtime
        .transaction()
        .submit(&handler, &commande(1), &contexte(), NOW);
    assert!(accepte.accepted().is_some(), "{:?}", accepte.refused());

    let boite = format!("agent/{}", id::<Agent>(2));
    let ecrits = runtime.transaction().store().read_stream(&boite, 0);
    assert_eq!(ecrits.len(), 1, "un message, un fait");
    assert_eq!(ecrits[0].event_type.to_string(), "message.sent");

    // La charge porte l'epoch **de l'émetteur** : un lecteur du journal doit pouvoir rendre le
    // verdict de réception sans relire le code qui a écrit l'événement.
    let charge = &ecrits[0].payload;
    assert_eq!(charge["epoch"], epoch.id().to_string());
    assert_eq!(charge["from"], id::<Agent>(1).to_string());
    assert_eq!(charge["to"], id::<Agent>(2).to_string());
    assert_eq!(charge["subject"], "revue demandée");

    // Et rien n'a été écrit ailleurs : la boîte de l'émetteur est vide, parce que le stream est
    // celui du destinataire — sans quoi lire « ce qui m'a été dit » serait une jointure.
    let expediteur = format!("agent/{}", id::<Agent>(1));
    assert!(
        runtime
            .transaction()
            .store()
            .read_stream(&expediteur, 0)
            .is_empty()
    );
}

/// **Deux messages au même destinataire sont ordonnés**, et c'est ce que le journal donne gratis.
///
/// §10.2 : ordre total par stream. C'est la propriété qu'un courtier devrait réimplémenter, et
/// réimplémenter moins bien puisqu'elle serait testée une seconde fois plutôt qu'une seule.
#[test]
fn deux_messages_au_meme_destinataire_sont_ordonnes() {
    let runtime = Runtime::in_memory();
    let epoch = epoque();

    for (seed, sujet, attendue) in [
        (1_u8, "premier", Revision::INITIAL),
        (2, "second", Revision::new(1)),
    ] {
        let handler = Send {
            message: Message::sent(id::<Agent>(1), id::<Agent>(2), &epoch, sujet),
        };
        let issue =
            runtime
                .transaction()
                .submit(&handler, &commande_a(seed, attendue), &contexte(), NOW);
        assert!(issue.accepted().is_some(), "{:?}", issue.refused());
    }

    let boite = format!("agent/{}", id::<Agent>(2));
    let ecrits = runtime.transaction().store().read_stream(&boite, 0);
    assert_eq!(ecrits.len(), 2);
    assert_eq!(ecrits[0].payload["subject"], "premier");
    assert_eq!(ecrits[1].payload["subject"], "second");
    assert!(
        ecrits[0].stream_revision < ecrits[1].stream_revision,
        "l'ordre total par stream est ce qui rend une messagerie inutile à réécrire"
    );
}

/// **Ni file, ni spool, ni courtier**, tenu par l'absence comme `W20.b` tient les écritures.
///
/// La garde jumelle vit dans `packages/coordination/tests/messaging.rs` pour le domaine ; celle-ci
/// couvre le côté qui a accès au journal, donc celui où la tentation d'un cache durable est réelle.
#[test]
fn aucun_second_stockage_durable() {
    let source = include_str!("../src/messaging.rs");
    for forbidden in [
        "VecDeque",
        "Vec<Message>",
        "inbox",
        "Inbox",
        "queue",
        "spool",
        "broker",
    ] {
        assert!(
            !source.contains(forbidden),
            "« {forbidden} » dans messaging.rs : un second stockage durable du même fait est une seconde vérité (ADR 0019, condition 1)"
        );
    }
}
