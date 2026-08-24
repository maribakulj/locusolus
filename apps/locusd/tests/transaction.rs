//! Le test de sortie de `W20.b` — le handler transactionnel comme port.

use std::cell::RefCell;

use locus_event_store::{
    Actor, ActorKind, Draft as EventDraft, EventStore, EventType, MemoryEventStore,
};
use locus_protocol::id::{Agent, Command, Event, Project, Workspace};
use locus_protocol::{Id, IdKind, Timestamp};
use locusd::{Batch, CommandEnvelope, CommandError, Decide, Family, Revision, Transaction};

const NOW: Timestamp = Timestamp::from_millis(1_700_000_000_000);

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn event(stream: &str, seed: u8) -> EventDraft {
    EventDraft {
        event_id: id::<Event>(seed),
        event_type: EventType::parse("branch.forked").expect("type valide"),
        schema_version: 1,
        stream_id: stream.to_owned(),
        workspace_id: id::<Workspace>(2),
        project_id: id::<Project>(4),
        program_id: None,
        branch_id: None,
        actor: Actor {
            principal_id: id::<Agent>(3),
            kind: ActorKind::Agent,
            delegation_id: None,
        },
        occurred_at: NOW,
        causation_id: id::<Command>(seed),
        correlation_id: None,
        trace_id: None,
        payload: serde_json::json!({ "seed": seed }),
        payload_hash: format!("sha256:{}", "ab".repeat(32)),
    }
}

fn commande(seed: u8, key: &str, revision: u64) -> CommandEnvelope {
    CommandEnvelope::mutating(
        id::<Command>(seed),
        "branch.fork",
        id::<Workspace>(2),
        id::<Agent>(3),
        key,
        Revision::new(revision),
    )
    .expect("commande bien formée")
}

/// La même clé, un autre principal — donc une autre portée.
fn commande_autre_principal(seed: u8, key: &str, revision: u64) -> CommandEnvelope {
    CommandEnvelope::mutating(
        id::<Command>(seed),
        "branch.fork",
        id::<Workspace>(2),
        id::<Agent>(99),
        key,
        Revision::new(revision),
    )
    .expect("commande bien formée")
}

/// Un décideur de test : il compte ses appels, et refuse ce qu'on lui dit de refuser.
///
/// Il ne détient **aucun** journal, et c'est le point du trait : sa signature ne lui en donne pas.
#[derive(Default)]
struct Decideur {
    refuse: Option<CommandError>,
    stream: String,
    appels: RefCell<usize>,
}

impl Decideur {
    fn sur(stream: &str) -> Self {
        Self {
            refuse: None,
            stream: stream.to_owned(),
            appels: RefCell::new(0),
        }
    }

    fn refusant(stream: &str, refus: CommandError) -> Self {
        Self {
            refuse: Some(refus),
            stream: stream.to_owned(),
            appels: RefCell::new(0),
        }
    }

    fn appels(&self) -> usize {
        *self.appels.borrow()
    }
}

impl Decide for Decideur {
    type State = ();

    fn decide(
        &self,
        _: &CommandEnvelope,
        (): &Self::State,
    ) -> Result<Vec<EventDraft>, CommandError> {
        *self.appels.borrow_mut() += 1;
        if let Some(refus) = &self.refuse {
            return Err(refus.clone());
        }
        // La graine vient du compteur d'appels, et non de la commande : deux clés de même longueur
        // donneraient sinon deux événements de même identifiant dans un stream.
        let graine = u8::try_from(*self.appels.borrow()).unwrap_or(u8::MAX);
        Ok(vec![event(&self.stream, graine)])
    }
}

// ---------------------------------------------------------------------------------------------
// 1. Aucun chemin de type n'écrit sans handler, et un test le tient par l'absence
// ---------------------------------------------------------------------------------------------

/// **La garantie tient par la signature, pas par la discipline.**
///
/// `Decide::decide` reçoit l'état et rend des brouillons. Aucun de ses paramètres n'est un journal,
/// et le trait n'a pas de méthode par défaut qui en fabrique un : un décideur n'a rien en main qui
/// sache écrire. C'est ce que le test lit dans la source du trait — la propriété étant l'**absence**
/// d'un paramètre, il n'y a rien à appeler pour la constater.
#[test]
fn un_decideur_ne_recoit_jamais_de_journal() {
    let source = include_str!("../src/handler.rs");
    let signature = source
        .split("fn decide(")
        .nth(1)
        .and_then(|rest| rest.split(')').next())
        .expect("le trait porte une méthode `decide`");

    for interdit in ["EventStore", "Append", "store"] {
        assert!(
            !signature.contains(interdit),
            "« {interdit} » dans la signature de `decide` : un décideur pourrait écrire"
        );
    }
}

/// **Et un second chemin, indépendant du premier : personne d'autre n'écrit.**
///
/// Le type couvre les décideurs ; il ne couvre pas un module de `locusd` qui se procurerait un
/// journal sans être un décideur du tout. Le `grep` couvre celui-là. Deux vérifications qui
/// n'attrapent pas la même chose valent mieux qu'une seule qu'on croirait complète.
#[test]
fn seule_la_transaction_ecrit() {
    let modules = [
        ("command.rs", include_str!("../src/command.rs")),
        ("error.rs", include_str!("../src/error.rs")),
        ("handler.rs", include_str!("../src/handler.rs")),
        ("outcome.rs", include_str!("../src/outcome.rs")),
        ("lib.rs", include_str!("../src/lib.rs")),
    ];

    for (nom, source) in modules {
        let code: String = source
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !code.contains(".append("),
            "`{nom}` écrit dans le journal : seule `transaction.rs` en a le droit"
        );
    }

    // Et `transaction.rs` n'écrit qu'une fois : deux `append` seraient deux occasions d'en réussir
    // un et de rater l'autre, ce qui est exactement l'écriture partielle que §9.2 interdit.
    let transaction = include_str!("../src/transaction.rs");
    let ecritures = transaction
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .filter(|line| line.contains("store.append("))
        .count();
    assert_eq!(ecritures, 1, "une seule écriture, dans `write`");
}

// ---------------------------------------------------------------------------------------------
// 2. Un handler qui échoue ne laisse aucun événement écrit
// ---------------------------------------------------------------------------------------------

/// **Un refus n'écrit rien**, et le journal est interrogé pour le dire.
///
/// Vérifier que le verdict est un refus ne suffirait pas : un refus rendu après une écriture est
/// exactement le défaut qu'on cherche. C'est donc le stream qu'on relit.
#[test]
fn un_handler_qui_refuse_ne_laisse_aucun_evenement() {
    let transaction = Transaction::new(MemoryEventStore::new());
    let decideur = Decideur::refusant(
        "branch/br_01",
        CommandError::Policy {
            policy: "branch.protected".to_owned(),
            detail: "la branche est protégée".to_owned(),
        },
    );

    let verdict = transaction.submit(&decideur, &commande(1, "idem-1", 0), &(), NOW);

    assert_eq!(
        verdict.refused().map(CommandError::family),
        Some(Family::Policy)
    );
    assert!(
        transaction
            .store()
            .read_stream("branch/br_01", 0)
            .is_empty()
    );
    assert_eq!(
        transaction.store().stream_count(),
        0,
        "aucun stream n'est né"
    );
}

/// Un lot atomique dont **une** commande refuse n'écrit rien du tout — pas même ce qui précédait.
#[test]
fn un_lot_atomique_qui_refuse_a_mi_parcours_n_ecrit_rien() {
    let transaction = Transaction::new(MemoryEventStore::new());
    let decideur = Decideur::refusant(
        "branch/br_01",
        CommandError::Budget {
            budget: "tokens".to_owned(),
            detail: "dépassé".to_owned(),
        },
    );

    let lot = Batch::Atomic(vec![commande(1, "a", 0), commande(2, "b", 0)]);
    let verdicts = transaction.submit_batch(&decideur, &lot, &(), NOW);

    assert_eq!(
        verdicts.len(),
        1,
        "le lot a un seul verdict : il est un tout"
    );
    assert_eq!(
        verdicts[0].refused().map(CommandError::family),
        Some(Family::Budget)
    );
    assert_eq!(transaction.store().stream_count(), 0);
}

// ---------------------------------------------------------------------------------------------
// 3. L'idempotence, et sa portée
// ---------------------------------------------------------------------------------------------

/// **Resoumettre la même clé rend le même résultat, sans second effet.**
///
/// Les deux moitiés comptent, et la seconde est celle qu'on oublie : rendre le bon résultat en
/// ayant réécrit serait un doublon qu'aucun client ne verrait. Le décideur compte donc ses appels,
/// et le stream est relu.
#[test]
fn une_resoumission_rend_le_meme_resultat_sans_second_effet() {
    let transaction = Transaction::new(MemoryEventStore::new());
    let decideur = Decideur::sur("branch/br_01");

    let premier = transaction.submit(&decideur, &commande(1, "retry-1", 0), &(), NOW);
    let revision = premier.accepted().expect("acceptée").revision;

    // Une resoumission : même clé, même portée, un `command_id` que le client n'a jamais vu.
    let second = transaction.submit(&decideur, &commande(7, "retry-1", 0), &(), NOW);

    assert_eq!(second.accepted().map(|a| a.revision), Some(revision));
    assert_eq!(
        decideur.appels(),
        1,
        "le décideur n'a pas été rappelé : la resoumission ne décide pas à nouveau"
    );
    assert_eq!(
        transaction.store().read_stream("branch/br_01", 0).len(),
        1,
        "un seul événement : le second effet n'a pas eu lieu"
    );
}

/// **Deux portées, la même clé, deux commandes distinctes.**
///
/// Deux clients qui choisissent `retry-1` ne se sont pas concertés. Confondre leurs soumissions
/// rendrait à l'un le succès d'une commande qu'il n'a jamais émise — un faux succès, qui est pire
/// qu'une erreur parce qu'il ne se signale pas.
#[test]
fn deux_portees_ne_confondent_pas_la_meme_cle() {
    let transaction = Transaction::new(MemoryEventStore::new());
    let decideur = Decideur::sur("branch/br_01");

    let un = transaction.submit(&decideur, &commande(1, "retry-1", 0), &(), NOW);
    let revision_un = un.accepted().expect("acceptée").revision;

    // Même clé, autre principal : autre portée, donc autre soumission.
    let deux = transaction.submit(
        &decideur,
        &commande_autre_principal(8, "retry-1", revision_un.get()),
        &(),
        NOW,
    );

    assert_eq!(decideur.appels(), 2, "la seconde portée a bien décidé");
    let revision_deux = deux.accepted().expect("acceptée").revision;
    assert_ne!(
        revision_un, revision_deux,
        "deux écritures distinctes, deux révisions"
    );
    assert_eq!(transaction.store().read_stream("branch/br_01", 0).len(), 2);
}

// ---------------------------------------------------------------------------------------------
// 4. Un lot n'est atomique que s'il se déclare tel
// ---------------------------------------------------------------------------------------------

/// **La déclaration est obligatoire**, et les deux natures ne se comportent pas pareil.
///
/// §22.5 : « les batch commands sont atomiques uniquement si explicitement déclarées ». La phrase
/// se lit comme une permission ; c'est une interdiction du défaut. Il n'existe pas de constructeur
/// qui prenne un `Vec` sans dire lequel des deux il est — le choix est obligatoire, pas l'atomicité.
#[test]
fn un_lot_n_est_atomique_que_declare() {
    assert!(Batch::Atomic(vec![commande(1, "a", 0)]).is_atomic());
    assert!(!Batch::Sequential(vec![commande(1, "a", 0)]).is_atomic());

    // Séquentiel : le refus arrête le lot, et **ce qui précède reste écrit**. C'est ce que « non
    // atomique » veut dire, et c'est pour cela qu'il faut le déclarer.
    let transaction = Transaction::new(MemoryEventStore::new());
    let ecrivain = Decideur::sur("branch/br_01");
    let premier = transaction.submit(&ecrivain, &commande(1, "a", 0), &(), NOW);
    let revision = premier.accepted().expect("acceptée").revision;

    let refuseur = Decideur::refusant(
        "branch/br_01",
        CommandError::Security {
            detail: "clé révoquée".to_owned(),
        },
    );
    let lot = Batch::Sequential(vec![commande(2, "b", revision.get())]);
    let verdicts = transaction.submit_batch(&refuseur, &lot, &(), NOW);

    assert_eq!(verdicts.len(), 1);
    assert!(verdicts[0].refused().is_some());
    assert_eq!(
        transaction.store().read_stream("branch/br_01", 0).len(),
        1,
        "le refus n'a pas défait ce qui était déjà écrit : le lot n'était pas atomique"
    );
}

/// Un lot séquentiel s'arrête au premier refus, et ne rend pas de verdict pour ce qu'il n'a pas
/// tenté.
///
/// Un vecteur de même longueur que le lot, complété par des refus fabriqués, laisserait croire que
/// les commandes suivantes ont été soumises et rejetées. Elles n'ont pas été soumises.
#[test]
fn un_lot_sequentiel_ne_rend_pas_de_verdict_pour_ce_qu_il_n_a_pas_tente() {
    let transaction = Transaction::new(MemoryEventStore::new());
    let refuseur = Decideur::refusant(
        "branch/br_01",
        CommandError::Unavailable {
            detail: "projection en retard".to_owned(),
        },
    );

    let lot = Batch::Sequential(vec![
        commande(1, "a", 0),
        commande(2, "b", 0),
        commande(3, "c", 0),
    ]);
    let verdicts = transaction.submit_batch(&refuseur, &lot, &(), NOW);

    assert_eq!(verdicts.len(), 1, "arrêté au premier refus");
    assert_eq!(
        refuseur.appels(),
        1,
        "les deux suivantes n'ont pas été tentées"
    );
}

/// Un lot atomique vise **un seul stream**, faute de quoi il est refusé avant d'écrire.
///
/// Promettre une atomicité inter-streams que le journal ne peut pas tenir serait pire que la
/// refuser : le refus se voit, la promesse non tenue non.
#[test]
fn un_lot_atomique_inter_streams_est_refuse_avant_d_ecrire() {
    struct DeuxStreams;
    impl Decide for DeuxStreams {
        type State = ();
        fn decide(
            &self,
            command: &CommandEnvelope,
            (): &Self::State,
        ) -> Result<Vec<EventDraft>, CommandError> {
            Ok(vec![event(
                &format!("branch/{}", command.idempotency_key()),
                1,
            )])
        }
    }

    let transaction = Transaction::new(MemoryEventStore::new());
    let lot = Batch::Atomic(vec![commande(1, "un", 0), commande(2, "deux", 0)]);
    let verdicts = transaction.submit_batch(&DeuxStreams, &lot, &(), NOW);

    assert_eq!(verdicts.len(), 1);
    assert_eq!(
        verdicts[0].refused().map(CommandError::family),
        Some(Family::Validation)
    );
    assert_eq!(transaction.store().stream_count(), 0, "rien n'a été écrit");
}

// ---------------------------------------------------------------------------------------------
// 5. Le conflit remonte sous la forme de §22.5
// ---------------------------------------------------------------------------------------------

/// Un conflit du journal devient le conflit de §22.5, avec la ressource à relire.
#[test]
fn un_conflit_du_journal_devient_le_conflit_de_la_spec() {
    let transaction = Transaction::new(MemoryEventStore::new());
    let decideur = Decideur::sur("branch/br_01");

    transaction
        .submit(&decideur, &commande(1, "a", 0), &(), NOW)
        .accepted()
        .expect("la première écrit");

    // La seconde annonce encore « stream inexistant » : elle n'a pas relu.
    let verdict = transaction.submit(&decideur, &commande(2, "b", 0), &(), NOW);

    let Some(CommandError::Conflict(conflit)) = verdict.refused() else {
        panic!("un conflit, et pas autre chose : {verdict:?}");
    };
    assert_eq!(conflit.expected, Revision::INITIAL);
    assert_eq!(conflit.current, Revision::new(1));
    assert_eq!(conflit.resource.path(), "branch/br_01");
}

/// Un décideur qui ne décide rien est un défaut **interne**, pas une faute du client.
///
/// Lui rendre `validation` l'enverrait chercher une erreur dans une requête qui n'en a pas.
#[test]
fn un_decideur_qui_ne_decide_rien_est_un_defaut_interne() {
    struct Muet;
    impl Decide for Muet {
        type State = ();
        fn decide(
            &self,
            _: &CommandEnvelope,
            (): &Self::State,
        ) -> Result<Vec<EventDraft>, CommandError> {
            Ok(Vec::new())
        }
    }

    let transaction = Transaction::new(MemoryEventStore::new());
    let verdict = transaction.submit(&Muet, &commande(1, "a", 0), &(), NOW);

    assert_eq!(
        verdict.refused().map(CommandError::family),
        Some(Family::Internal)
    );
    // Le message nomme le décideur, et pas le journal. Les deux chemins existent — la garde de
    // `write` et le refus du journal — et un test qui ne lirait que la famille ne saurait pas
    // lequel a répondu : ils rendent tous deux `internal`. Or le second passerait par un `append`
    // sur un stream sans nom, ce qui est une écriture tentée pour rien.
    let Some(CommandError::Internal { detail }) = verdict.refused() else {
        panic!("un défaut interne : {verdict:?}");
    };
    assert!(
        detail.starts_with("le handler n'a décidé aucun événement"),
        "« {detail} » : ce message-ci vient de la garde de `write`. Celui du journal dit « lot vide », \
         et les confondre voudrait dire qu'un `append` a été tenté sur un stream sans nom"
    );
}

/// **Une saturation se dit `unavailable`, et le refus nomme la borne.**
///
/// C'est la famille qui compte : `internal` enverrait le client ouvrir un ticket, `validation` lui
/// ferait chercher une faute dans sa requête. Seul `unavailable` dit « retente plus tard ».
///
/// La borne est ici mise à zéro : c'est la seule façon d'éprouver le chemin sans fabriquer mille
/// écritures concurrentes, et une borne nulle est une configuration valide — un service qui
/// n'accepte aucune écriture est un service qui le dit, pas un service en panne.
#[test]
fn une_saturation_est_un_refus_unavailable_qui_nomme_la_borne() {
    let decideur = Decideur::sur("epistemic_object:1");
    let transaction = Transaction::bounded(MemoryEventStore::new(), 0);

    let verdict = transaction.submit(&decideur, &commande(1, "idem-1", 0), &(), NOW);

    let refus = verdict.refused().expect("une borne à zéro refuse");
    assert_eq!(
        refus.family(),
        Family::Unavailable,
        "une saturation n'est ni une panne ni une faute du client"
    );
    let dit = refus.to_string();
    assert!(
        dit.contains('0') && dit.contains("saturé"),
        "le refus nomme la borne et dit que retenter aboutira : {dit}"
    );

    // Et rien n'a été écrit : le refus arrive **avant** le journal.
    assert_eq!(transaction.store().stream_count(), 0);
}
