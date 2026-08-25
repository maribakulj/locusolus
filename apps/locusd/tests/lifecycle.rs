//! Test de sortie de `W20.ae` — **les transitions de cycle de vie d'instance atteignent le
//! journal.**
//!
//! # Ce que ce fichier doit prouver
//!
//! `coordination::lifecycle` était une machine à états correcte et éprouvée **dont les décisions ne
//! sortaient jamais** : elle rend un `Outcome`, aucun crate hors de son propre n'importait le
//! module, et l'ADR 0026 affirmait pourtant qu'elle « journalise les transitions ». `W0.20` a
//! corrigé l'affirmation ; cet item écrit le producteur.
//!
//! Quatre propriétés, et aucune ne se déduit des autres :
//!
//! 1. les **quatre** commandes écrivent leur fait, sous un verbe **dérivé** de `Command::slug()` ;
//! 2. l'issue voyage sous son nom, avec son compte — `remaining` pour un drain en cours,
//!    `abandoned` pour un `kill` **même nul** ;
//! 3. l'état résultant vient du domaine, et un drain sur un nœud occupé **ne change pas** l'état ;
//! 4. ce que le domaine refuse n'entre pas dans le journal, et sous la famille `policy`.
//!
//! La première est celle qui tient le vocabulaire : un `match` sur `Command` produisant quatre
//! littéraux passerait les trois autres sans rien dire, et c'est exactement le second vocabulaire
//! que `CLAUDE.md` interdit.

use locus_coordination::agent::InstanceState;
use locus_coordination::lifecycle::{Command, Lifecycle, Quiescence};
use locus_event_store::{ActorKind, EventStore, MemoryEventStore};
use locus_protocol::id::{Agent, Command as CommandId, Event, Project, Workspace};
use locus_protocol::{Id, IdKind, Timestamp};
use locusd::{
    Apply, CommandEnvelope, CommandError, LepContext, Outcome, Revision, Transaction,
    event_type_of, stream_of_instance,
};

const NOW: Timestamp = Timestamp::from_millis(1_700_000_000_000);

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(NOW, entropy).expect("l'instant de fixture tient sur 48 bits")
}

fn noeud() -> Id<Agent> {
    id::<Agent>(7)
}

fn contexte(seed: u8) -> LepContext {
    LepContext {
        project_id: id::<Project>(4),
        event_ids: vec![id::<Event>(seed)],
        occurred_at: NOW,
        payload_hash: format!("sha256:{}", "ab".repeat(32)),
    }
}

fn commande(seed: u8, revision: u64) -> CommandEnvelope {
    CommandEnvelope::mutating(
        id::<CommandId>(seed),
        "agent.lifecycle",
        id::<Workspace>(2),
        id::<Agent>(3),
        format!("cle-{seed}"),
        Revision::new(revision),
    )
    .expect("commande bien formée")
}

/// Un scheduler qui connaît déjà `noeud()` dans l'état donné — la reconstitution depuis le journal.
fn sachant(state: InstanceState) -> Lifecycle {
    Lifecycle::new().knowing(noeud(), state)
}

fn appliquer(
    transaction: &Transaction<MemoryEventStore>,
    seed: u8,
    revision: u64,
    lifecycle: Lifecycle,
    command: Command,
    quiescence: Quiescence,
) -> Outcome {
    transaction.submit(
        &Apply {
            node: noeud(),
            lifecycle,
            command,
            quiescence,
        },
        &commande(seed, revision),
        &contexte(seed),
        NOW,
    )
}

/// Les charges écrites sur le stream de l'instance, dans l'ordre.
fn charges(transaction: &Transaction<MemoryEventStore>) -> Vec<serde_json::Value> {
    transaction
        .store()
        .read_stream(&stream_of_instance(noeud()), 0)
        .into_iter()
        .map(|recorded| recorded.payload.clone())
        .collect()
}

// ---------------------------------------------------------------------------------------------
// 1. Les quatre commandes écrivent, et leur verbe est dérivé
// ---------------------------------------------------------------------------------------------

/// Aucune des quatre n'est muette, et le namespace est celui de §10.3 qui n'avait jamais servi.
///
/// L'ordre est celui du domaine : `spawn` fonde l'identité, `suspend` l'écarte, `drain` la laisse
/// finir, `kill` l'arrête. Chacun part de l'état où le précédent l'a laissée, ce qui est aussi la
/// façon de vérifier que l'état écrit est bien celui que le domaine a rangé.
#[test]
fn les_quatre_commandes_ecrivent_chacune_leur_fait() {
    let transaction = Transaction::new(MemoryEventStore::new());

    let etapes = [
        (Command::Spawn, Lifecycle::new(), Quiescence::of(0)),
        (
            Command::Suspend,
            sachant(InstanceState::Active),
            Quiescence::of(0),
        ),
        (
            Command::Drain,
            sachant(InstanceState::Active),
            Quiescence::of(0),
        ),
        (
            Command::Kill,
            sachant(InstanceState::Active),
            Quiescence::of(0),
        ),
    ];

    for (rang, (command, lifecycle, quiescence)) in etapes.into_iter().enumerate() {
        let seed = u8::try_from(rang).expect("quatre étapes") + 1;
        let revision = u64::try_from(rang).expect("quatre étapes");
        let verdict = appliquer(&transaction, seed, revision, lifecycle, command, quiescence);
        assert!(
            matches!(verdict, Outcome::Accepted(_)),
            "« {command} » : {verdict:?}"
        );
    }

    let ecrits = transaction
        .store()
        .read_stream(&stream_of_instance(noeud()), 0);
    let types: Vec<String> = ecrits
        .iter()
        .map(|recorded| recorded.event_type.to_string())
        .collect();
    assert_eq!(
        types,
        vec![
            "agent.spawned",
            "agent.suspended",
            "agent.drained",
            "agent.killed"
        ]
    );
}

/// Le verbe est **calculé** depuis `Command::slug()`, et le module ne porte aucun des quatre en
/// littéral.
///
/// # Pourquoi ce test lit la source
///
/// Une première rédaction comparait `event_type_of(command)` à `format!("agent.{}ed", slug)` : une
/// **tautologie**, qui restitue l'implémentation au lieu de la contraindre. Un `match` sur
/// `Command` rendant quatre littéraux l'aurait passée sans broncher, puisque les deux formes sont
/// extensionnellement égales sur les quatre commandes d'aujourd'hui — et ce `match` est exactement
/// le second vocabulaire que `CLAUDE.md` interdit.
///
/// La propriété voulue n'est pas « les quatre valeurs sont les bonnes » mais « **personne ne peut**
/// écrire une cinquième valeur à côté du domaine ». Elle ne s'observe pas à l'exécution : elle
/// s'observe dans la source. C'est le même arbitrage que `W23.a`, dont le test lit `Cargo.toml`
/// plutôt que de chercher un `#[derive(Serialize)]`.
#[test]
fn aucun_verbe_n_est_ecrit_en_litteral_dans_le_module() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lifecycle.rs"))
        .expect("le module de production est lisible depuis son propre crate");
    for command in Command::ALL {
        // La forme **entre guillemets** : la documentation du module cite les quatre verbes entre
        // accents graves, ce qui est une explication et non une seconde source de vérité.
        let litteral = format!("\"agent.{}ed\"", command.slug());
        assert!(
            !source.contains(&litteral),
            "{litteral} est écrit en dur : le verbe doit être dérivé de `Command::slug()`, sans \
             quoi le journal a un vocabulaire que le domaine ne connaît pas"
        );
    }
}

/// Les quatre types produits sont des types d'événement de §10.3.
///
/// Faible mais pas vide : `EventType::parse` refuse un namespace inconnu et une moitié vide, donc
/// ce test attrape le jour où un slug cesserait de se régulariser en quelque chose de lisible. Ce
/// qu'il **ne** dit pas, c'est que le verbe est du bon français — aucun test ne le dira, et c'est
/// le test ci-dessus qui garantit qu'il n'y a qu'un seul endroit où le corriger.
#[test]
fn les_quatre_types_produits_sont_des_types_d_evenement() {
    for command in Command::ALL {
        let produit = event_type_of(command);
        assert!(
            locus_event_store::EventType::parse(&produit).is_ok(),
            "« {produit} » doit être un type d'événement de §10.3"
        );
    }
}

/// L'acteur est le **système**, comme pour `task.assigned`.
///
/// Piloter une instance est une décision du plan de contrôle. `kind` dit qui a décidé,
/// `principal_id` sous quelle autorité — les confondre obligerait à inventer un principal système
/// que rien n'enrôle.
#[test]
fn l_acteur_est_le_systeme_et_le_principal_reste_celui_de_la_commande() {
    let transaction = Transaction::new(MemoryEventStore::new());
    assert!(matches!(
        appliquer(
            &transaction,
            1,
            0,
            Lifecycle::new(),
            Command::Spawn,
            Quiescence::of(0)
        ),
        Outcome::Accepted(_)
    ));

    let ecrits = transaction
        .store()
        .read_stream(&stream_of_instance(noeud()), 0);
    assert_eq!(ecrits[0].actor.kind, ActorKind::System);
    assert_eq!(ecrits[0].actor.principal_id, id::<Agent>(3));
}

// ---------------------------------------------------------------------------------------------
// 2. L'issue voyage sous son nom, avec son compte
// ---------------------------------------------------------------------------------------------

/// Un drain sur un nœud **occupé** dit ce qu'il reste, et **ne change pas** l'état.
///
/// C'est la quiescence locale de `docs/13`. Un fait qui rendrait `completed` sur un nœud encore
/// occupé mentirait sur ce qui tourne, et un exploitant retirerait le nœud de la version alors
/// qu'il travaille encore.
#[test]
fn un_drain_sur_un_noeud_occupe_porte_le_reste_et_laisse_l_etat() {
    let transaction = Transaction::new(MemoryEventStore::new());
    assert!(matches!(
        appliquer(
            &transaction,
            1,
            0,
            sachant(InstanceState::Active),
            Command::Drain,
            Quiescence::of(3)
        ),
        Outcome::Accepted(_)
    ));

    let charge = &charges(&transaction)[0];
    assert_eq!(charge["outcome"], "draining");
    assert_eq!(charge["remaining"], 3);
    assert_eq!(
        charge["state"], "active",
        "un drain en cours ne termine rien : {charge}"
    );
}

/// Un drain sur un nœud **quiescent** termine, et ne porte pas de reste.
#[test]
fn un_drain_sur_un_noeud_quiescent_termine_et_ne_porte_pas_de_reste() {
    let transaction = Transaction::new(MemoryEventStore::new());
    assert!(matches!(
        appliquer(
            &transaction,
            1,
            0,
            sachant(InstanceState::Active),
            Command::Drain,
            Quiescence::of(0)
        ),
        Outcome::Accepted(_)
    ));

    let charge = &charges(&transaction)[0];
    assert_eq!(charge["outcome"], "settled");
    assert_eq!(charge["state"], "completed");
    assert!(
        charge.get("remaining").is_none(),
        "rien ne reste : {charge}"
    );
}

/// Un `kill` porte son compte **même quand il vaut zéro**.
///
/// C'est ce qui sépare un arrêt propre d'un arrêt coûteux, et le type de domaine prend déjà soin de
/// le porter dans les deux cas. Omettre le champ à zéro rendrait les deux indiscernables pour un
/// lecteur du journal — et c'est le lecteur, pas l'appelant, que ce fait sert.
#[test]
fn un_kill_porte_son_compte_meme_nul() {
    for (en_vol, attendu) in [(0_usize, 0_u64), (2, 2)] {
        let transaction = Transaction::new(MemoryEventStore::new());
        assert!(matches!(
            appliquer(
                &transaction,
                1,
                0,
                sachant(InstanceState::Active),
                Command::Kill,
                Quiescence::of(en_vol)
            ),
            Outcome::Accepted(_)
        ));

        let charge = &charges(&transaction)[0];
        assert_eq!(charge["outcome"], "killed");
        assert_eq!(charge["abandoned"], attendu, "{charge}");
        assert_eq!(charge["state"], "terminated");
    }
}

/// L'état écrit est celui que le **domaine** range, pour les six états de §7.1 qu'il produit.
///
/// Le recalculer ici — fût-ce par un `match` de trois lignes — ferait une seconde machine à états,
/// exactement ce que `coordination::lifecycle` dit ne pas être. Ce test le tient en comparant, pour
/// chaque commande licite, le `state` du fait à celui qu'un `Lifecycle` de référence range hors du
/// journal.
#[test]
fn l_etat_ecrit_est_celui_que_le_domaine_range() {
    let cas = [
        (Command::Spawn, None, Quiescence::of(0)),
        (
            Command::Suspend,
            Some(InstanceState::Active),
            Quiescence::of(0),
        ),
        (
            Command::Drain,
            Some(InstanceState::Active),
            Quiescence::of(0),
        ),
        (
            Command::Drain,
            Some(InstanceState::Active),
            Quiescence::of(5),
        ),
        (
            Command::Kill,
            Some(InstanceState::Waiting),
            Quiescence::of(1),
        ),
    ];

    for (rang, (command, depart, quiescence)) in cas.into_iter().enumerate() {
        let lifecycle = depart.map_or_else(Lifecycle::new, sachant);
        let mut reference = lifecycle.clone();
        reference
            .command(noeud(), command, quiescence)
            .expect("le cas est licite");
        let attendu = reference.state(noeud()).expect("le domaine range un état");

        let transaction = Transaction::new(MemoryEventStore::new());
        let seed = u8::try_from(rang).expect("cinq cas") + 1;
        assert!(matches!(
            appliquer(&transaction, seed, 0, lifecycle, command, quiescence),
            Outcome::Accepted(_)
        ));
        assert_eq!(
            charges(&transaction)[0]["state"],
            attendu.to_string(),
            "« {command} » depuis {depart:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 3. Ce que le domaine refuse n'entre pas dans le journal
// ---------------------------------------------------------------------------------------------

/// Les **cinq** refus du domaine sont des politiques, et aucun n'écrit.
///
/// §22.5 n'a pas de famille « absent » — les huit sont closes —, donc `NoSuchInstance` ne fait pas
/// exception plutôt que d'inventer la neuvième. Dans les cinq cas la requête est bien écrite et
/// c'est l'état qui s'y oppose : rendre `validation` enverrait l'appelant relire sa requête, où il
/// ne trouverait rien.
#[test]
fn les_refus_du_domaine_sont_des_politiques_et_n_ecrivent_rien() {
    let refus = [
        // `spawn` sur une instance qui existe.
        (
            Command::Spawn,
            Some(InstanceState::Active),
            Quiescence::of(0),
        ),
        // Toute autre commande sur une instance absente.
        (Command::Suspend, None, Quiescence::of(0)),
        // Une instance terminée ne se ranime pas (§14.2).
        (
            Command::Drain,
            Some(InstanceState::Terminated),
            Quiescence::of(0),
        ),
        (
            Command::Kill,
            Some(InstanceState::Completed),
            Quiescence::of(0),
        ),
        // Suspendre ce qui n'était pas au tour.
        (
            Command::Suspend,
            Some(InstanceState::Provisioned),
            Quiescence::of(0),
        ),
    ];

    for (rang, (command, depart, quiescence)) in refus.into_iter().enumerate() {
        let transaction = Transaction::new(MemoryEventStore::new());
        let lifecycle = depart.map_or_else(Lifecycle::new, sachant);
        let seed = u8::try_from(rang).expect("cinq refus") + 1;
        let verdict = appliquer(&transaction, seed, 0, lifecycle, command, quiescence);

        match verdict {
            Outcome::Refused(CommandError::Policy { policy, .. }) => {
                assert_eq!(policy, "agent.lifecycle", "« {command} » depuis {depart:?}");
            }
            autre => panic!("« {command} » depuis {depart:?} : {autre:?}"),
        }
        assert!(
            charges(&transaction).is_empty(),
            "un refus n'écrit rien : « {command} » depuis {depart:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 4. Le journal suffit : la population s'y relit
// ---------------------------------------------------------------------------------------------

/// **Ce que `W23.b` attendait** : une identité et son état se relisent du journal seul.
///
/// Le blocage de `W23.b` n'était pas la jointure — `W20.ad` l'a livrée — mais que la **population
/// elle-même** n'atteignait pas le journal : `nominal` compterait des identités que rien n'y écrit,
/// et zéro est la valeur qu'un compteur vide rend quand il fonctionne. Ce test tient la propriété
/// sans construire les compteurs : un stream `agent/…` par identité, et l'état courant est celui du
/// dernier fait — donc `Lifecycle::knowing` reconstitue le scheduler, ce que sa docstring annonce
/// depuis `W13` sans que rien ne l'ait jamais fait.
#[test]
fn une_instance_et_son_etat_se_relisent_du_journal_seul() {
    let transaction = Transaction::new(MemoryEventStore::new());
    assert!(matches!(
        appliquer(
            &transaction,
            1,
            0,
            Lifecycle::new(),
            Command::Spawn,
            Quiescence::of(0)
        ),
        Outcome::Accepted(_)
    ));
    assert!(matches!(
        appliquer(
            &transaction,
            2,
            1,
            sachant(InstanceState::Active),
            Command::Kill,
            Quiescence::of(0)
        ),
        Outcome::Accepted(_)
    ));

    // Ce qu'un lecteur du journal fait, et rien de plus : repérer les streams `agent/…`, prendre
    // le dernier fait de chacun, en lire l'état.
    let flux = transaction.store().feed(0);
    let faits: Vec<&serde_json::Value> = flux
        .iter()
        .filter(|entry| entry.event.event_type.namespace() == "agent")
        .map(|entry| &entry.event.payload)
        .collect();
    assert_eq!(faits.len(), 2, "les deux commandes ont écrit");

    // `unwrap_or_default` ici rendrait `""` pour un champ **absent**, et l'ensemble compterait
    // alors une identité vide : « la réponse est zéro » lu comme « il n'y a pas eu de réponse »,
    // qui est la faute que ce dépôt nomme partout. Une première rédaction l'a commise, et un
    // mutant renommant `agent_id` y a survécu.
    let identites: std::collections::BTreeSet<&str> = faits
        .iter()
        .map(|payload| {
            payload["agent_id"].as_str().unwrap_or_else(|| {
                panic!("`agent_id` est le champ qui nomme l'identité : {payload}")
            })
        })
        .collect();
    assert_eq!(identites.len(), 1, "une seule identité a été pilotée");
    assert!(identites.contains(noeud().to_string().as_str()));

    let charges = charges(&transaction);
    assert_eq!(charges.len(), 2);
    assert_eq!(charges[0]["state"], "provisioned", "spawn fonde `nominal`");
    assert_eq!(
        charges[1]["state"], "terminated",
        "le dernier fait porte l'état courant"
    );
}

/// **Un stream par instance**, et c'est ce qui rend `nominal` calculable sans jointure.
///
/// # Pourquoi il faut deux instances pour le voir
///
/// Une première rédaction ne pilotait qu'un nœud, et lisait son stream par `stream_of_instance` —
/// la fonction que le producteur emploie lui aussi. Un mutant faisant écrire **toutes** les
/// instances dans `agent/global` y a survécu : producteur et lecteur se trompaient de concert, donc
/// le test ne pouvait pas les départager. C'est la forme générale d'un test qui interroge son sujet
/// par le sujet lui-même.
///
/// Avec deux identités, la propriété redevient observable : le stream de l'une ne porte pas les
/// faits de l'autre. Un seul stream global la ferait tomber, et c'est ce qui compte — le verrou est
/// par instance (`W20.h`), et deux schedulers pilotant deux nœuds ne doivent pas se sérialiser.
#[test]
fn chaque_instance_a_son_stream() {
    let autre: Id<Agent> = id::<Agent>(9);
    assert_ne!(noeud(), autre);

    let transaction = Transaction::new(MemoryEventStore::new());
    assert!(matches!(
        appliquer(
            &transaction,
            1,
            0,
            Lifecycle::new(),
            Command::Spawn,
            Quiescence::of(0)
        ),
        Outcome::Accepted(_)
    ));
    let verdict = transaction.submit(
        &Apply {
            node: autre,
            lifecycle: Lifecycle::new(),
            command: Command::Spawn,
            quiescence: Quiescence::of(0),
        },
        &commande(2, 0),
        &contexte(2),
        NOW,
    );
    assert!(matches!(verdict, Outcome::Accepted(_)), "{verdict:?}");

    assert_ne!(
        stream_of_instance(noeud()),
        stream_of_instance(autre),
        "deux identités, deux streams"
    );
    for (node, voisin) in [(noeud(), autre), (autre, noeud())] {
        let ecrits = transaction
            .store()
            .read_stream(&stream_of_instance(node), 0);
        assert_eq!(ecrits.len(), 1, "le stream de {node} porte son seul fait");
        assert_eq!(
            ecrits[0].payload["agent_id"],
            node.to_string(),
            "le stream de {node} ne porte rien de {voisin}"
        );
    }
}
