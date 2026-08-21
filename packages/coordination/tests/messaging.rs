//! Test de sortie de `W16.e` — **epochs, messages tardifs, transfert d'état.**
//!
//! La ligne de roadmap demande quatre choses, et l'ADR 0019 dit pourquoi chacune compte :
//!
//! 1. émettre rend un événement du namespace `message` **et rien d'autre** — aucun second stockage
//!    durable. La moitié qui vit ici est l'absence ; la moitié qui produit l'événement vit dans
//!    `apps/locusd`, avec sa transaction, et son test l'accompagne ;
//! 2. un message d'un epoch antérieur est rapporté `Late` **en nommant les deux epochs**, jamais
//!    appliqué ni jeté en silence ;
//! 3. un epoch inconnu rend `Unknown` et non `Late` — deviner et ignorer sont deux fautes
//!    distinctes ;
//! 4. le passage de témoin d'un `drain` transmet ce que le nœud sortant **tenait**, et refuse un
//!    contexte de mission.

use std::fmt::Write as _;

use locus_coordination::lifecycle::{Command, Lifecycle, Quiescence};
use locus_coordination::messaging::{
    EpochError, Epochs, Handover, HandoverError, Message, Reception,
};
use locus_coordination::version::{Digest, Operation, Version};
use locus_coordination::{CoordinationMode, InstanceState, Relation, RelationKind};
use locus_domain::ContentHash;
use locus_protocol::{Id, IdKind, Timestamp, id::Agent};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn agent(seed: u8) -> Id<Agent> {
    id::<Agent>(seed)
}

fn reviews(from: Id<Agent>, to: Id<Agent>) -> Relation {
    Relation {
        from,
        to,
        kind: RelationKind::Review,
    }
}

/// Le même condensat jouet que `tests/version.rs` : déterministe, et fonction de ses octets seuls.
struct Fnv;

const PRIME: u64 = 0x0000_0100_0000_01b3;
const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

impl Digest for Fnv {
    fn digest(&self, canonical: &str) -> ContentHash {
        let mut digest = String::with_capacity(64);
        for salt in 0_u64..4 {
            let mut hash = OFFSET ^ salt.wrapping_mul(PRIME);
            for byte in canonical.bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(PRIME);
            }
            write!(digest, "{hash:016x}").expect("écrire dans une String n'échoue pas");
        }
        ContentHash::parse(&format!("sha256:{digest}")).expect("64 hexadécimaux minuscules")
    }
}

/// L'epoch initial : deux agents, une revue.
fn first() -> Version {
    Version::root(
        &[agent(1), agent(2)],
        &[reviews(agent(1), agent(2))],
        CoordinationMode::Blackboard,
        None,
        &Fnv,
    )
    .expect("la fixture est cohérente")
}

/// L'epoch suivant : un agent de plus entre. La graine change à chaque génération, sans quoi la
/// seconde application rendrait `NodeAlreadyPresent` au lieu d'un epoch petit-enfant.
fn next_after(from: &Version, entering: u8) -> Version {
    from.apply(&Operation::AddNode(agent(entering)), &Fnv)
        .expect("ajouter un nœud absent est licite")
}

/// L'enfant direct de la racine.
fn second(from: &Version) -> Version {
    next_after(from, 3)
}

/// Un epoch d'une **autre lignée** : même sorte de changement, autre agent, autre racine.
///
/// Ce n'est pas un epoch « du futur » ni « du passé » — c'est un epoch que le destinataire n'a
/// aucun moyen de situer, ce qui est exactement le cas que [`Reception::Unknown`] vise.
fn elsewhere() -> Version {
    Version::root(
        &[agent(7), agent(8)],
        &[reviews(agent(7), agent(8))],
        CoordinationMode::Blackboard,
        None,
        &Fnv,
    )
    .expect("la fixture est cohérente")
}

// ---------------------------------------------------------------------------------------------
// 1. Une suite d'epochs est une lignée, pas une collection
// ---------------------------------------------------------------------------------------------

/// Sans filiation vérifiée, « antérieur » n'aurait pas de sens entre deux hashes.
///
/// C'est la charnière de l'ADR 0019 décision 2 : l'epoch est une `Version`, donc il ne se compare
/// pas par `<`. Ce qui ordonne deux epochs est la filiation, et une suite qu'on pourrait remplir
/// d'epochs quelconques rendrait [`Reception::Late`] arbitraire — un epoch présent ne prouverait
/// plus qu'on l'a traversé.
#[test]
fn une_suite_d_epochs_refuse_un_maillon_qui_n_est_pas_l_enfant_du_courant() {
    let root = first();
    let next = second(&root);
    let epochs = Epochs::rooted(&root);

    let advanced = epochs
        .clone()
        .advanced_to(&next)
        .expect("l'enfant direct est accepté");
    assert_eq!(advanced.current(), next.id());
    assert_eq!(advanced.depth(), 2);

    // Un epoch d'une autre lignée n'a pas le courant pour parent.
    let stranger = elsewhere();
    let refused = epochs
        .clone()
        .advanced_to(&stranger)
        .expect_err("une racine ne succède à rien");
    assert_eq!(
        refused,
        EpochError::NotAChild {
            current: root.id().clone(),
            parent: None,
        }
    );

    // Sauter un maillon est refusé aussi : la suite doit être traversée, pas devinée.
    let third = next_after(&next, 4);
    let skipped = Epochs::rooted(&root)
        .advanced_to(&third)
        .expect_err("le petit-enfant n'est pas l'enfant");
    assert_eq!(
        skipped,
        EpochError::NotAChild {
            current: root.id().clone(),
            parent: Some(next.id().clone()),
        }
    );

    // Et le refus n'a rien consommé : la suite refusée est celle d'avant.
    assert_eq!(Epochs::rooted(&root).depth(), 1);
}

// ---------------------------------------------------------------------------------------------
// 2 et 3. Trois verdicts, et les deux fautes qu'ils séparent
// ---------------------------------------------------------------------------------------------

/// Le cas nominal : l'émetteur a agi sous l'epoch courant du destinataire.
#[test]
fn un_message_du_meme_epoch_est_delivre() {
    let root = first();
    let epochs = Epochs::rooted(&root);
    let message = Message::sent(agent(1), agent(2), &root, "revue demandée");

    assert_eq!(epochs.receive(&message), Reception::Delivered);
}

/// **Le verdict nomme les deux epochs**, et c'est la moitié qui compte.
///
/// Un destinataire qui lirait « tardif » sans savoir de quel epoch à quel epoch ne pourrait rien en
/// faire : ni rattraper, ni renvoyer, ni expliquer. Le test tient donc les deux champs par égalité
/// stricte, pas la variante seule — un `Late` qui perdrait `sent_under` passerait un test qui
/// n'observerait que la sorte.
#[test]
fn un_message_d_un_epoch_traverse_est_rapporte_tardif_en_nommant_les_deux() {
    let root = first();
    let next = second(&root);
    let epochs = Epochs::rooted(&root)
        .advanced_to(&next)
        .expect("l'enfant direct est accepté");

    // L'émetteur a agi sous l'epoch d'avant.
    let message = Message::sent(agent(1), agent(2), &root, "revue demandée");

    assert_eq!(
        epochs.receive(&message),
        Reception::Late {
            sent_under: root.id().clone(),
            current: next.id().clone(),
        }
    );

    // Rapporté, donc ni appliqué ni jeté : la suite du destinataire n'a pas bougé.
    assert_eq!(epochs.current(), next.id());
    assert_eq!(epochs.depth(), 2);
}

/// **`Unknown` n'est pas un `Late` atténué**, et les fondre rendrait un verdict plausible.
///
/// Un epoch jamais traversé peut venir d'une reconfiguration plus récente que celle du destinataire,
/// ou d'une lignée divergente. Les deux appellent des suites opposées — attendre, ou refuser — et
/// aucune information ne permet de choisir. Rendre `Late` reviendrait à deviner ; rendre `Delivered`
/// reviendrait à ignorer. Ce sont les deux fautes de la condition 2 de l'ADR 0019, et le troisième
/// verdict existe pour ne commettre ni l'une ni l'autre.
#[test]
fn un_epoch_jamais_traverse_est_inconnu_et_non_tardif() {
    let root = first();
    let next = second(&root);
    let epochs = Epochs::rooted(&root)
        .advanced_to(&next)
        .expect("l'enfant direct est accepté");

    let stranger = elsewhere();
    let message = Message::sent(agent(7), agent(2), &stranger, "revue demandée");
    let verdict = epochs.receive(&message);

    assert_eq!(
        verdict,
        Reception::Unknown {
            sent_under: stranger.id().clone(),
            current: next.id().clone(),
        }
    );

    // Par égalité stricte, et pas seulement par la sorte : le verdict d'un epoch inconnu et celui
    // d'un epoch traversé ne se confondent jamais, même quand les deux epochs cités sont différents.
    assert_ne!(
        verdict,
        Reception::Late {
            sent_under: stranger.id().clone(),
            current: next.id().clone(),
        }
    );

    // Et les deux phrasent différemment : un opérateur qui lit un journal doit les distinguer sans
    // avoir le type sous les yeux.
    let late = Reception::Late {
        sent_under: root.id().clone(),
        current: next.id().clone(),
    };
    assert_ne!(verdict.to_string(), late.to_string());
    assert!(verdict.to_string().contains("inconnu"));
    assert!(late.to_string().contains("tardif"));
}

/// Un epoch **futur** de la même lignée est inconnu, lui aussi, et c'est voulu.
///
/// Le destinataire n'a pas encore avancé ; il ne peut donc pas savoir que cet epoch le concerne.
/// L'inscrire comme `Late` inverserait le sens du mot, et le traiter comme `Delivered` appliquerait
/// un message émis sous une configuration que le destinataire n'a pas.
#[test]
fn un_epoch_pas_encore_atteint_est_inconnu_aussi() {
    let root = first();
    let next = second(&root);
    let epochs = Epochs::rooted(&root);

    let message = Message::sent(agent(1), agent(2), &next, "revue demandée");

    assert_eq!(
        epochs.receive(&message),
        Reception::Unknown {
            sent_under: next.id().clone(),
            current: root.id().clone(),
        }
    );
}

/// Le message porte l'epoch de **l'émetteur**, jamais celui du destinataire.
///
/// La faute serait invisible : un message qui adopterait l'epoch du lecteur serait toujours
/// `Delivered`, et les trois verdicts deviendraient un seul.
#[test]
fn un_message_porte_l_epoch_de_son_emetteur() {
    let root = first();
    let next = second(&root);
    let message = Message::sent(agent(1), agent(2), &root, "revue demandée");

    assert_eq!(message.epoch(), root.id());
    assert_ne!(message.epoch(), next.id());
    assert_eq!(message.from(), agent(1));
    assert_eq!(message.to(), agent(2));
    assert_eq!(message.subject(), "revue demandée");
}

// ---------------------------------------------------------------------------------------------
// 4. Le transfert d'état est un passage de témoin
// ---------------------------------------------------------------------------------------------

/// Il transmet ce que le sortant **tenait**, et il ne naît que d'un drain.
///
/// `kill` abandonne ce qu'il tenait et le dit — lui laisser passer la main ferait croire qu'un
/// successeur reprend un travail que personne ne reprend. Un nœud simplement suspendu, lui, n'a rien
/// à transmettre.
#[test]
fn le_passage_de_temoin_ne_nait_que_d_un_drain() {
    let mut scheduler = Lifecycle::new().knowing(agent(1), InstanceState::Active);

    let draining = scheduler
        .command(agent(1), Command::Drain, Quiescence::of(3))
        .expect("drainer un nœud actif est licite");
    let handover =
        Handover::after_drain(agent(1), agent(2), draining).expect("un drain passe la main");
    assert_eq!(handover.from(), agent(1));
    assert_eq!(handover.to(), agent(2));
    assert_eq!(handover.in_flight(), 3);

    // Un kill abandonne : il ne passe pas la main.
    let mut other = Lifecycle::new().knowing(agent(4), InstanceState::Active);
    let killed = other
        .command(agent(4), Command::Kill, Quiescence::of(2))
        .expect("tuer un nœud actif est licite");
    assert_eq!(
        Handover::after_drain(agent(4), agent(5), killed),
        Err(HandoverError::NotDraining)
    );

    // Un nœud posé n'a rien à transmettre.
    let mut third = Lifecycle::new().knowing(agent(6), InstanceState::Active);
    let suspended = third
        .command(agent(6), Command::Suspend, Quiescence::Quiescent)
        .expect("suspendre un nœud actif est licite");
    assert_eq!(
        Handover::after_drain(agent(6), agent(2), suspended),
        Err(HandoverError::NotDraining)
    );
}

/// Se passer le témoin à soi-même est un drain qui n'en est pas un.
#[test]
fn un_noeud_ne_se_passe_pas_le_temoin_a_lui_meme() {
    let mut scheduler = Lifecycle::new().knowing(agent(1), InstanceState::Active);
    let draining = scheduler
        .command(agent(1), Command::Drain, Quiescence::of(1))
        .expect("drainer un nœud actif est licite");

    assert_eq!(
        Handover::after_drain(agent(1), agent(1), draining),
        Err(HandoverError::ToItself)
    );
}

/// **Le passage de témoin ne transporte aucun contexte**, et le test le tient par l'absence.
///
/// `docs/13` fixe pour la V1 : « nouvel attempt, nouvelle vue, nouveau hash ». Un passage de témoin
/// qui emporterait une vue de contexte contournerait cette immuabilité sans la nommer — la vue porte
/// un hash obligatoire, et une copie qui voyage n'en a plus l'usage.
///
/// Le test lit la **source** du module plutôt qu'une liste de champs recopiée : c'est ce qui le rend
/// capable de voir arriver le champ qu'il interdit. Il double la garde de `proposal.rs`, qui refuse
/// déjà `context_view` dans tout le crate — deux vérifications indépendantes valent mieux qu'une,
/// et celle-ci nomme le module fautif au lieu du crate.
#[test]
fn le_passage_de_temoin_ne_porte_aucune_charge() {
    let source = include_str!("../src/messaging.rs");
    for forbidden in [
        "ContextView",
        "context_view",
        "payload",
        "content_hash",
        "transcript",
    ] {
        assert!(
            !source.contains(forbidden),
            "« {forbidden} » dans messaging.rs : un message qui transporte un contexte contourne « nouvel attempt, nouvelle vue, nouveau hash »"
        );
    }
}

/// **Aucun second stockage durable**, tenu par l'absence comme `W20.b` tient les écritures.
///
/// C'est la condition 1 de l'ADR 0019, et l'argument par lequel le courtier dédié a été écarté :
/// deux stockages durables du même fait sont deux vérités, qui divergent le jour où l'une est
/// purgée. La messagerie est un **usage** du journal ; ce module n'a donc rien à retenir.
///
/// Ce qu'il tient, et qui n'est pas un stockage de messages : la suite d'epochs d'un destinataire,
/// qui est son état de lecture — l'équivalent d'un cursor, pas d'une file. La distinction se lit
/// dans le type : `Vec<VersionId>`, jamais `Vec<Message>`.
#[test]
fn la_messagerie_ne_retient_aucun_message() {
    let source = include_str!("../src/messaging.rs");
    for forbidden in [
        "Vec<Message>",
        "VecDeque",
        "BTreeMap<Id<Agent>, Message",
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
