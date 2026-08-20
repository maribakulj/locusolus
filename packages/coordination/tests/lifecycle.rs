//! Test de sortie de W16.a — **les trois garanties de l'item.**
//!
//! 1. Une transition interdite est refusée en nommant l'état de départ **et** celui visé.
//! 2. Un nœud est drainé sans que rien d'autre soit arrêté, et la quiescence se **constate** au lieu
//!    de s'attendre.
//! 3. `kill` sur un nœud quiescent et sur un nœud occupé ne disent pas la même chose.

use locus_coordination::{
    Command, InstanceState, Lifecycle, LifecycleError, Outcome, Quiescence, may_leave_the_version,
};
use locus_protocol::{Id, IdKind, Timestamp, id::Agent};

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn agent(seed: u8) -> Id<Agent> {
    id::<Agent>(seed)
}

/// Trois nœuds actifs — de quoi vérifier qu'une commande n'en touche qu'un.
fn fleet() -> Lifecycle {
    Lifecycle::new()
        .knowing(agent(1), InstanceState::Active)
        .knowing(agent(2), InstanceState::Active)
        .knowing(agent(3), InstanceState::Waiting)
}

// ---------------------------------------------------------------------------------------------
// 1. Une transition interdite nomme les deux états
// ---------------------------------------------------------------------------------------------

#[test]
fn a_forbidden_transition_names_both_states() {
    let mut scheduler = Lifecycle::new().knowing(agent(1), InstanceState::Provisioned);
    let error = scheduler
        .command(agent(1), Command::Suspend, Quiescence::Quiescent)
        .expect_err("une instance seulement provisionnée n'est pas au tour");
    assert_eq!(
        error,
        LifecycleError::Forbidden {
            node: agent(1).to_string(),
            command: Command::Suspend,
            from: InstanceState::Provisioned,
            to: InstanceState::Waiting,
        },
        "« interdit » sans dire d'où ni vers où ne se corrige pas"
    );
}

/// Suspendre écarte du tour ce qui y était, et le nœud **attend** — il ne reste pas actif.
///
/// Le refus était testé, la réussite non : un `suspend` qui laisserait le nœud `active` n'écarterait
/// rien, et le scheduler continuerait de lui donner du travail en croyant l'avoir mis de côté.
#[test]
fn suspending_an_active_node_makes_it_wait() {
    let mut scheduler = fleet();
    assert_eq!(
        scheduler
            .command(agent(1), Command::Suspend, Quiescence::Quiescent)
            .expect("agent(1) est actif"),
        Outcome::Settled(InstanceState::Waiting)
    );
    assert_eq!(scheduler.state(agent(1)), Some(InstanceState::Waiting));
    assert_eq!(
        scheduler.state(agent(2)),
        Some(InstanceState::Active),
        "et lui seul"
    );

    let already_waiting = scheduler
        .command(agent(3), Command::Suspend, Quiescence::Quiescent)
        .expect_err("agent(3) attend déjà : il n'y a rien à écarter du tour");
    assert!(matches!(already_waiting, LifecycleError::Forbidden { .. }));
}

#[test]
fn a_terminated_instance_is_never_revived() {
    for terminal in [
        InstanceState::Completed,
        InstanceState::Failed,
        InstanceState::Terminated,
    ] {
        let mut scheduler = Lifecycle::new().knowing(agent(1), terminal);
        for command in [Command::Suspend, Command::Drain, Command::Kill] {
            let error = scheduler
                .command(agent(1), command, Quiescence::Quiescent)
                .expect_err("§14.2 : la ranimer effacerait la trace de sa fin");
            assert!(
                matches!(error, LifecycleError::AlreadyTerminal { .. }),
                "« {command} » sur « {terminal} »"
            );
        }
    }
}

#[test]
fn spawning_twice_is_refused_and_commanding_a_ghost_too() {
    let mut scheduler = Lifecycle::new();
    assert_eq!(
        scheduler
            .command(agent(1), Command::Spawn, Quiescence::Quiescent)
            .expect("un nœud neuf"),
        Outcome::Settled(InstanceState::Provisioned),
        "une instance naît provisionnée, jamais active"
    );
    assert!(matches!(
        scheduler
            .command(agent(1), Command::Spawn, Quiescence::Quiescent)
            .expect_err("elle existe déjà"),
        LifecycleError::AlreadySpawned { .. }
    ));
    assert!(matches!(
        scheduler
            .command(agent(7), Command::Drain, Quiescence::Quiescent)
            .expect_err("agent(7) n'existe pas"),
        LifecycleError::NoSuchInstance { .. }
    ));
}

// ---------------------------------------------------------------------------------------------
// 2. Drain local : un nœud, et lui seul
// ---------------------------------------------------------------------------------------------

/// La quiescence **locale** de `docs/13`, par opposition au drain global.
///
/// Drainer `agent(1)` ne doit rien changer à `agent(2)` ni à `agent(3)`. Un drain global aurait le
/// même effet apparent sur le nœud visé, et personne ne verrait la différence avant de perdre le
/// travail des deux autres.
#[test]
fn draining_one_node_stops_nothing_else() {
    let mut scheduler = fleet();
    let before: Vec<_> = scheduler.nodes().collect();

    let outcome = scheduler
        .command(agent(1), Command::Drain, Quiescence::of(2))
        .expect("agent(1) est actif");
    assert_eq!(outcome, Outcome::Draining { remaining: 2 });

    assert_eq!(
        scheduler.state(agent(1)),
        Some(InstanceState::Active),
        "un nœud qui draine n'a pas fini : son état ne change pas encore"
    );
    for (node, state) in before.iter().skip(1) {
        assert_eq!(scheduler.state(*node), Some(*state), "{node} n'a pas bougé");
    }
}

/// Le drain aboutit quand la quiescence est constatée, pas avant.
#[test]
fn a_drain_settles_only_once_the_node_is_quiescent() {
    let mut scheduler = fleet();
    assert_eq!(
        scheduler
            .command(agent(1), Command::Drain, Quiescence::of(1))
            .expect("actif"),
        Outcome::Draining { remaining: 1 }
    );
    assert_eq!(
        scheduler
            .command(agent(1), Command::Drain, Quiescence::of(0))
            .expect("actif"),
        Outcome::Settled(InstanceState::Completed),
        "la dernière tentative finie, le drain aboutit"
    );
    assert_eq!(scheduler.state(agent(1)), Some(InstanceState::Completed));
}

/// La quiescence est un **constat**, pas une attente.
///
/// `Quiescence::of` lit un nombre et rend un verdict. Rien dans ce module n'attend : un
/// `wait_for_quiescence` ferait tenir au scheduler une promesse dont il n'a pas les moyens, puisque
/// rien n'oblige un nœud à devenir quiescent — et l'appelant croirait que le drain finit toujours.
#[test]
fn quiescence_is_read_never_awaited() {
    assert_eq!(Quiescence::of(0), Quiescence::Quiescent);
    assert_eq!(Quiescence::of(3), Quiescence::Busy { attempts: 3 });
    assert_eq!(Quiescence::of(0).in_flight(), 0);
    assert_eq!(Quiescence::of(3).in_flight(), 3);
}

// ---------------------------------------------------------------------------------------------
// 3. Tuer dit ce que ça coûte
// ---------------------------------------------------------------------------------------------

/// Le compte est porté **même quand il vaut zéro**.
///
/// C'est ce qui distingue un arrêt propre d'un arrêt coûteux. Un `kill` qui rendrait la même chose
/// dans les deux cas cacherait à l'opérateur combien de tentatives viennent d'être perdues — et il
/// n'aurait aucune raison de le chercher.
#[test]
fn killing_says_what_it_abandons() {
    let mut clean = fleet();
    assert_eq!(
        clean
            .command(agent(1), Command::Kill, Quiescence::Quiescent)
            .expect("actif"),
        Outcome::Killed { abandoned: 0 }
    );

    let mut costly = fleet();
    assert_eq!(
        costly
            .command(agent(1), Command::Kill, Quiescence::of(4))
            .expect("actif"),
        Outcome::Killed { abandoned: 4 }
    );

    assert_ne!(
        Outcome::Killed { abandoned: 0 },
        Outcome::Killed { abandoned: 4 },
        "les deux arrêts ne disent pas la même chose"
    );
    assert_eq!(costly.state(agent(1)), Some(InstanceState::Terminated));
}

/// Tuer aboutit toujours ; drainer non. C'est toute la différence entre les deux commandes.
#[test]
fn killing_settles_where_draining_waits() {
    let busy = Quiescence::of(2);
    let mut scheduler = fleet();

    assert_eq!(
        scheduler
            .command(agent(2), Command::Drain, busy)
            .expect("actif"),
        Outcome::Draining { remaining: 2 }
    );
    assert_eq!(scheduler.state(agent(2)), Some(InstanceState::Active));

    assert_eq!(
        scheduler
            .command(agent(2), Command::Kill, busy)
            .expect("actif"),
        Outcome::Killed { abandoned: 2 }
    );
    assert_eq!(scheduler.state(agent(2)), Some(InstanceState::Terminated));
}

// ---------------------------------------------------------------------------------------------
// La règle qui relie ce module à la version
// ---------------------------------------------------------------------------------------------

/// Un nœud ne quitte pas la version pendant qu'il tourne.
///
/// `REMOVE_NODE` ne détient que des identités : la version ne peut pas savoir seule qu'une instance
/// travaille encore. Sans cette règle, une organisation dirait qu'un agent est parti alors qu'il
/// produit toujours — et le graphe institutionnel cesserait de décrire ce qui se passe.
#[test]
fn a_running_node_does_not_leave_the_version() {
    for running in [
        InstanceState::Provisioned,
        InstanceState::Active,
        InstanceState::Waiting,
    ] {
        let error = may_leave_the_version(agent(1), running)
            .expect_err("il travaille encore, ou s'apprête à le faire");
        assert!(
            matches!(error, LifecycleError::StillRunning { .. }),
            "{running}"
        );
    }
    for done in [
        InstanceState::Completed,
        InstanceState::Failed,
        InstanceState::Terminated,
    ] {
        assert!(may_leave_the_version(agent(1), done).is_ok(), "{done}");
    }
}

/// Le chemin complet : drainer jusqu'à la quiescence, **puis** retirer de la version.
#[test]
fn the_ordered_path_is_drain_then_remove() {
    let mut scheduler = fleet();
    let state = scheduler.state(agent(1)).expect("connu");
    assert!(
        may_leave_the_version(agent(1), state).is_err(),
        "on ne retire pas d'abord"
    );

    scheduler
        .command(agent(1), Command::Drain, Quiescence::of(1))
        .expect("actif");
    scheduler
        .command(agent(1), Command::Drain, Quiescence::of(0))
        .expect("actif");

    let settled = scheduler.state(agent(1)).expect("connu");
    assert_eq!(settled, InstanceState::Completed);
    assert!(may_leave_the_version(agent(1), settled).is_ok());
}

// ---------------------------------------------------------------------------------------------
// Quatre commandes, et pas neuf
// ---------------------------------------------------------------------------------------------

/// Vérification par l'absence — la même discipline que les opérations attributaires de W15.a.
///
/// `replace`, `split`, `merge`, `connect` et `disconnect` de `docs/13` **sont déjà** les opérations
/// de `crate::version`. Les réécrire ici produirait un second chemin qui divergerait du premier, et
/// personne ne saurait lequel décrit ce qui sera commité.
///
/// Les quatre autres — rerouter l'état, rejouer, migrer le contexte, livrer les messages —
/// attendaient une messagerie inter-agents. Elle existe depuis l'ADR 0019, et **le test ne change
/// pas** : livrer un message reste hors du scheduler, parce que le scheduler pilote des instances
/// tandis que la messagerie écrit et lit des faits. Le seul point de contact est `drain`, et il
/// passe par `messaging::Handover` plutôt que par une cinquième commande.
#[test]
fn the_scheduler_does_not_redefine_what_the_version_already_does() {
    let slugs: Vec<&str> = Command::ALL.iter().map(|command| command.slug()).collect();
    assert_eq!(slugs, ["spawn", "suspend", "drain", "kill"]);
    for elsewhere in [
        "replace",
        "split",
        "merge",
        "connect",
        "disconnect",
        "reroute",
        "replay",
        "migrate",
        "deliver",
    ] {
        assert!(
            !slugs.contains(&elsewhere),
            "« {elsewhere} » vit ailleurs, ou attend ce qui n'existe pas"
        );
    }
}
