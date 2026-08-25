//! Clause 3 du test de sortie de `W23.c` — **les namespaces réellement écrits.**
//!
//! # Pourquoi cette clause vit ici et non dans le domaine
//!
//! `packages/coordination` n'écrit rien : c'est `locusd` qui tient le journal. Une clause sur ce qui
//! est **émis** ne peut donc se coder que du côté qui émet, et c'est aussi pourquoi elle n'était pas
//! codable avant `W20.ae` — le cycle de vie n'avait aucun producteur, donc aucun namespace à
//! comparer, et `W0.20` l'a nommé.
//!
//! # Ce que la clause dit, et ce qu'elle ne dit pas
//!
//! Une décision d'ordonnancement n'émet **que ce que les deux familles composées émettent** :
//! `agent.*` pour le cycle de vie, `team.*` pour la structure. Jamais un troisième namespace, et en
//! particulier aucun de portefeuille — c'est la formulation d'origine de `docs/13`, « une décision
//! locale ne produit aucun événement de portefeuille ».
//!
//! Une réécriture intermédiaire de cette clause disait « n'émet que des faits `agent.*` », ce qui
//! est **trop fort** : composer `REPLACE_NODE` passe par le chemin de version, qui écrit
//! `team.modified` depuis `W17.i`. Une clause trop forte est aussi inutilisable qu'une clause fausse
//! — on la code, elle rougit, et on affaiblit le code plutôt que la clause.

use std::collections::BTreeSet;

use locus_coordination::lifecycle::{Command, Lifecycle, Quiescence};
use locus_coordination::scheduler::{SchedulerDecision, admit};
use locus_coordination::version::{ContentDigest, Operation, Version};
use locus_coordination::{CoordinationMode, InstanceState};
use locus_event_store::{EventStore, MemoryEventStore};
use locus_protocol::id::{Agent, Branch, Command as CommandId, Event, Project, Workspace};
use locus_protocol::{Id, IdKind, Timestamp};
use locusd::{
    Apply, CommandEnvelope, Commit, LepContext, OrganisationContext, Outcome, Revision, Transaction,
};

const NOW: Timestamp = Timestamp::from_millis(1_700_000_000_000);

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(NOW, entropy).expect("l'instant de fixture tient sur 48 bits")
}

fn agent(seed: u8) -> Id<Agent> {
    id::<Agent>(seed)
}

fn commande(seed: u8) -> CommandEnvelope {
    CommandEnvelope::mutating(
        id::<CommandId>(seed),
        "agent.lifecycle",
        id::<Workspace>(2),
        agent(3),
        format!("cle-{seed}"),
        Revision::new(0),
    )
    .expect("commande bien formée")
}

/// La racine : deux agents, en tableau noir.
fn racine() -> Version {
    Version::root(
        &[agent(1), agent(2)],
        &[],
        CoordinationMode::Blackboard,
        None,
        &ContentDigest,
    )
    .expect("la fixture est cohérente")
}

/// Les namespaces écrits par une transaction, sans doublon.
fn namespaces(transaction: &Transaction<MemoryEventStore>) -> BTreeSet<String> {
    transaction
        .store()
        .feed(0)
        .iter()
        .map(|entry| entry.event.event_type.namespace().to_owned())
        .collect()
}

/// Une décision de **cycle de vie** n'écrit que `agent.*`.
#[test]
fn une_decision_de_cycle_de_vie_n_ecrit_que_des_faits_agent() {
    let decision = SchedulerDecision::Lifecycle {
        node: agent(1),
        command: Command::Kill,
    };
    let instances = Lifecycle::new().knowing(agent(1), InstanceState::Active);
    admit(&instances, &decision).expect("piloter une instance ne fait sortir personne");

    let transaction = Transaction::new(MemoryEventStore::new());
    let verdict = transaction.submit(
        &Apply {
            node: agent(1),
            lifecycle: instances,
            command: Command::Kill,
            quiescence: Quiescence::of(0),
        },
        &commande(1),
        &LepContext {
            project_id: id::<Project>(4),
            event_ids: vec![id::<Event>(1)],
            occurred_at: NOW,
            payload_hash: format!("sha256:{}", "ab".repeat(32)),
        },
        NOW,
    );
    assert!(matches!(verdict, Outcome::Accepted(_)), "{verdict:?}");

    assert_eq!(
        namespaces(&transaction),
        ["agent".to_owned()].into_iter().collect()
    );
}

/// Une décision **structurelle** n'écrit que `team.*`.
///
/// Et c'est le cas que la réécriture trop forte de la clause aurait fait rougir : composer une
/// opération de version passe par le chemin de `W17.i`, qui écrit `team.modified`. L'ordonnanceur
/// n'ajoute rien à ce que la famille composée émet — c'est le sens de « il compose ».
#[test]
fn une_decision_structurelle_n_ecrit_que_des_faits_team() {
    let operation = Operation::RemoveNode(agent(2));
    let decision = SchedulerDecision::Structural(operation.clone());
    // Le nœud n'a pas d'instance : rien ne s'oppose à sa sortie.
    admit(&Lifecycle::new(), &decision).expect("un nœud sans instance part sans cérémonie");

    let transaction = Transaction::new(MemoryEventStore::new());
    let verdict = transaction.submit(
        &Commit {
            base: racine(),
            operation,
            digest: ContentDigest,
        },
        &commande(2),
        &OrganisationContext {
            branch_id: id::<Branch>(9),
            project_id: id::<Project>(4),
            event_id: id::<Event>(2),
            occurred_at: NOW,
            payload_hash: format!("sha256:{}", "ab".repeat(32)),
        },
        NOW,
    );
    assert!(matches!(verdict, Outcome::Accepted(_)), "{verdict:?}");

    assert_eq!(
        namespaces(&transaction),
        ["team".to_owned()].into_iter().collect()
    );
}

/// Les deux ensemble n'écrivent **que** les deux namespaces composés.
///
/// C'est la clause proprement dite : pas de troisième famille, et en particulier rien qui ressemble
/// à un événement de portefeuille. Un ordonnanceur qui remonterait chaque décision locale au
/// portefeuille ferait de §13 le journal de tout ce qui bouge, ce que `docs/13` refuse en demandant
/// « quiescence locale d'un nœud plutôt que drain global ».
#[test]
fn aucune_troisieme_famille_n_est_ecrite() {
    let transaction = Transaction::new(MemoryEventStore::new());

    let instances = Lifecycle::new().knowing(agent(1), InstanceState::Active);
    assert!(matches!(
        transaction.submit(
            &Apply {
                node: agent(1),
                lifecycle: instances,
                command: Command::Suspend,
                quiescence: Quiescence::of(0),
            },
            &commande(1),
            &LepContext {
                project_id: id::<Project>(4),
                event_ids: vec![id::<Event>(1)],
                occurred_at: NOW,
                payload_hash: format!("sha256:{}", "ab".repeat(32)),
            },
            NOW,
        ),
        Outcome::Accepted(_)
    ));

    assert!(matches!(
        transaction.submit(
            &Commit {
                base: racine(),
                operation: Operation::AddNode(agent(7)),
                digest: ContentDigest,
            },
            &commande(2),
            &OrganisationContext {
                branch_id: id::<Branch>(9),
                project_id: id::<Project>(4),
                event_id: id::<Event>(2),
                occurred_at: NOW,
                payload_hash: format!("sha256:{}", "ab".repeat(32)),
            },
            NOW,
        ),
        Outcome::Accepted(_)
    ));

    let ecrits = namespaces(&transaction);
    assert_eq!(
        ecrits,
        ["agent".to_owned(), "team".to_owned()]
            .into_iter()
            .collect(),
        "les deux familles composées, et rien d'autre"
    );
}

/// Une décision **refusée** par l'ordonnanceur n'écrit rien du tout.
///
/// # Ce que la première rédaction ne prouvait pas
///
/// Elle appelait `admit`, constatait le refus, puis vérifiait qu'une transaction **neuve** était
/// vide. C'est vrai de toute transaction neuve : le test ne touchait jamais au chemin d'écriture, et
/// aurait passé avec un `admit` qui accepte tout.
///
/// Ce qui suit exerce le flux **gardé** — la façon dont un appelant compose les deux — et le
/// contraste porte la propriété : la même transaction, la même opération, et le seul écart est
/// l'état de l'instance.
#[test]
fn une_decision_refusee_n_ecrit_rien_la_meme_admise_ecrit() {
    /// Ce qu'un appelant fait : demander à l'ordonnanceur, puis n'écrire que s'il a dit oui.
    fn ordonnancer(
        transaction: &Transaction<MemoryEventStore>,
        instances: &Lifecycle,
        node: Id<Agent>,
        seed: u8,
    ) -> Result<Outcome, locus_coordination::lifecycle::LifecycleError> {
        let operation = Operation::RemoveNode(node);
        admit(instances, &SchedulerDecision::Structural(operation.clone()))?;
        Ok(transaction.submit(
            &Commit {
                base: racine(),
                operation,
                digest: ContentDigest,
            },
            &commande(seed),
            &OrganisationContext {
                branch_id: id::<Branch>(9),
                project_id: id::<Project>(4),
                event_id: id::<Event>(seed),
                occurred_at: NOW,
                payload_hash: format!("sha256:{}", "ab".repeat(32)),
            },
            NOW,
        ))
    }

    // L'instance tourne : la règle que personne n'appliquait depuis `W13` refuse, et rien n'est
    // écrit — `Version::apply` seul aurait retiré le nœud sans poser la question.
    let refusee = Transaction::new(MemoryEventStore::new());
    let vivante = Lifecycle::new().knowing(agent(2), InstanceState::Active);
    assert!(ordonnancer(&refusee, &vivante, agent(2), 1).is_err());
    assert!(
        namespaces(&refusee).is_empty(),
        "un départ refusé ne laisse pas de trace de départ"
    );

    // Même opération, même transaction neuve : l'instance est terminée, et le fait est écrit.
    let admise = Transaction::new(MemoryEventStore::new());
    let finie = Lifecycle::new().knowing(agent(2), InstanceState::Terminated);
    assert!(matches!(
        ordonnancer(&admise, &finie, agent(2), 2).expect("l'instance est terminée"),
        Outcome::Accepted(_)
    ));
    assert_eq!(
        namespaces(&admise),
        ["team".to_owned()].into_iter().collect()
    );
}
