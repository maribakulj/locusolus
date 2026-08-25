//! Test de sortie de `W23.c` — **l'ordonnanceur d'instances, au-dessus du placement d'hôte.**
//!
//! Trois clauses, celles de la roadmap :
//!
//! 1. `spawn`, `suspend`, `drain`, `kill` se lisent de `coordination::lifecycle` et de rien
//!    d'autre ; `remplacer`, `scinder`, `fusionner`, `connecter`, `déconnecter` se lisent de
//!    `crate::version` — l'ordonnanceur **compose** et ne redéfinit rien, tenu par l'absence de
//!    tout verbe qui lui soit propre ;
//! 2. `place`, qui vit chez `locus-execd`, reste **seul juge de l'hôte** ;
//! 3. les namespaces réellement écrits sont ceux des deux familles composées, et jamais un
//!    troisième — vérifié dans `apps/locusd/tests/scheduler.rs`, où les faits s'écrivent.
//!
//! # Ce que cet item ferme, et pourquoi la clause 1 valait la correction
//!
//! Les deux premières clauses n'étaient pas satisfaisables telles qu'écrites : la version d'origine
//! faisait lire `remplacer` à `coordination::lifecycle`, qui ne le porte pas — l'en-tête du module
//! dit lui-même qu'il vit dans `crate::version` sous `REPLACE_NODE`. `W0.20` l'a trouvé en lisant le
//! code plutôt que la ligne. Ce fichier code la clause **corrigée**.

use locus_coordination::lifecycle::{Command, Lifecycle, LifecycleError};
use locus_coordination::scheduler::{SchedulerDecision, admit};
use locus_coordination::version::Operation;
use locus_coordination::{InstanceState, Relation, RelationKind};
use locus_protocol::id::Agent;
use locus_protocol::{Id, IdKind, Timestamp};

const NOW: Timestamp = Timestamp::from_millis(1_700_000_000_000);

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(NOW, entropy).expect("l'instant de fixture tient sur 48 bits")
}

fn agent(seed: u8) -> Id<Agent> {
    id::<Agent>(seed)
}

fn sachant(node: Id<Agent>, state: InstanceState) -> Lifecycle {
    Lifecycle::new().knowing(node, state)
}

/// Les quatre opérations qui font sortir un nœud, avec le nœud qu'elles font sortir.
fn sorties() -> Vec<(&'static str, Operation, Vec<Id<Agent>>)> {
    vec![
        (
            "REMOVE_NODE",
            Operation::RemoveNode(agent(1)),
            vec![agent(1)],
        ),
        (
            "REPLACE_NODE",
            Operation::ReplaceNode {
                from: agent(1),
                to: agent(2),
            },
            vec![agent(1)],
        ),
        (
            "SPLIT_NODE",
            Operation::SplitNode {
                node: agent(1),
                into: (agent(2), agent(3)),
                follows_first: std::collections::BTreeSet::new(),
            },
            vec![agent(1)],
        ),
        (
            "MERGE_NODES",
            Operation::MergeNodes {
                first: agent(1),
                second: agent(4),
                into: agent(5),
            },
            vec![agent(1), agent(4)],
        ),
    ]
}

/// Les opérations qui ne font sortir personne.
fn non_sorties() -> Vec<(&'static str, Operation)> {
    let relation = Relation {
        from: agent(1),
        to: agent(2),
        kind: RelationKind::Review,
    };
    vec![
        ("ADD_NODE", Operation::AddNode(agent(9))),
        ("ADD_EDGE", Operation::AddEdge(relation)),
        ("REMOVE_EDGE", Operation::RemoveEdge(relation)),
        (
            "SET_ROLE",
            Operation::SetRole {
                node: agent(1),
                from: None,
                to: Some("relecteur".to_owned()),
            },
        ),
    ]
}

// ---------------------------------------------------------------------------------------------
// 1. Aucun verbe propre : le vocabulaire est l'union des deux familles
// ---------------------------------------------------------------------------------------------

/// Les quatre verbes de cycle de vie viennent de `lifecycle`, et l'ordonnanceur les porte tels
/// quels.
///
/// `SchedulerDecision::Lifecycle` transporte un `lifecycle::Command` sans le traduire. S'il portait
/// son propre énuméré, une commande ajoutée au domaine aurait deux domiciles, et le jour où l'un
/// des deux est corrigé personne ne saurait lequel décrit ce qui sera exécuté.
#[test]
fn les_quatre_verbes_de_cycle_de_vie_sont_ceux_du_domaine() {
    for command in Command::ALL {
        let decision = SchedulerDecision::Lifecycle {
            node: agent(1),
            command,
        };
        match decision {
            SchedulerDecision::Lifecycle { command: porte, .. } => assert_eq!(porte, command),
            SchedulerDecision::Structural(_) => panic!("une décision de cycle de vie"),
        }
    }
}

/// **Piloter une instance ne fait sortir personne de la version.**
///
/// C'est la séparation que `docs/13` demande et que l'en-tête de `lifecycle` explique : le cycle de
/// vie porte sur l'instance qui tourne, la version sur la structure. Un `kill` qui retirerait le
/// nœud confondrait les deux, et la trace ne dirait plus si l'agent est parti ou s'il est mort.
#[test]
fn une_commande_de_cycle_de_vie_ne_fait_sortir_personne() {
    for command in Command::ALL {
        let decision = SchedulerDecision::Lifecycle {
            node: agent(1),
            command,
        };
        assert!(decision.departures().is_empty(), "« {command} »");
    }
}

/// Les **quatre** opérations qui font sortir un nœud sont reconnues, et les autres non.
///
/// Le `match` de `departures` est exhaustif : une opération ajoutée à `Operation` sans réponse à
/// cette question ne compile pas. Ce test vérifie que les réponses données sont les bonnes — le
/// compilateur garantit qu'elles existent, pas qu'elles sont justes.
#[test]
fn les_sorties_de_version_sont_exactement_celles_qu_on_croit() {
    for (nom, operation, attendues) in sorties() {
        let departs = SchedulerDecision::Structural(operation).departures();
        assert_eq!(departs, attendues, "« {nom} »");
    }
    for (nom, operation) in non_sorties() {
        assert!(
            SchedulerDecision::Structural(operation)
                .departures()
                .is_empty(),
            "« {nom} » ne fait sortir personne"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 2. La seule règle que l'ordonnanceur ajoute
// ---------------------------------------------------------------------------------------------

/// Un nœud dont l'instance **tourne** ne quitte pas la version.
///
/// C'est la règle que `may_leave_the_version` porte depuis `W13` et que **personne n'appliquait** :
/// hors de ses propres tests, elle n'avait aucun appelant, et `Version::apply` retire un nœud sans
/// jamais demander si son instance tourne. Ce n'est pas un lecteur sans producteur — c'est une règle
/// sans applicateur, et la conséquence est la même.
#[test]
fn un_noeud_dont_l_instance_tourne_ne_quitte_pas_la_version() {
    for etat in InstanceState::ALL {
        if etat.is_terminal() {
            continue;
        }
        for (nom, operation, _) in sorties() {
            let refus = admit(
                &sachant(agent(1), etat),
                &SchedulerDecision::Structural(operation),
            )
            .expect_err("l'instance tourne encore");
            match refus {
                LifecycleError::StillRunning { node, state } => {
                    assert_eq!(node, agent(1).to_string(), "« {nom} » depuis {etat}");
                    assert_eq!(state, etat);
                }
                autre => panic!("« {nom} » depuis {etat} : {autre:?}"),
            }
        }
    }
}

/// Une instance **terminée** laisse partir son nœud, dans les trois états terminaux.
#[test]
fn une_instance_terminee_laisse_partir_son_noeud() {
    for etat in InstanceState::ALL {
        if !etat.is_terminal() {
            continue;
        }
        for (nom, operation, _) in sorties() {
            assert!(
                admit(
                    &sachant(agent(1), etat),
                    &SchedulerDecision::Structural(operation)
                )
                .is_ok(),
                "« {nom} » depuis {etat}"
            );
        }
    }
}

/// Un nœud que le cycle de vie **ne connaît pas** part sans cérémonie.
///
/// Un membre déclaré dans la version qu'on n'a jamais réveillé n'a pas d'instance ; refuser son
/// retrait obligerait à provisionner puis tuer une instance pour retirer un nom, ce qui écrirait
/// deux faits pour un acte qui n'a pas eu lieu.
#[test]
fn un_noeud_sans_instance_part_sans_ceremonie() {
    for (nom, operation, _) in sorties() {
        assert!(
            admit(&Lifecycle::new(), &SchedulerDecision::Structural(operation)).is_ok(),
            "« {nom} »"
        );
    }
}

/// `MERGE_NODES` interroge ses **deux** sources, pas seulement la première.
///
/// Une fusion fait sortir deux nœuds. N'en vérifier qu'un laisserait passer la moitié des cas —
/// et c'est le genre d'oubli qu'un test sur un seul nœud ne montre jamais.
#[test]
fn une_fusion_interroge_ses_deux_sources() {
    let fusion = Operation::MergeNodes {
        first: agent(1),
        second: agent(4),
        into: agent(5),
    };
    for vivant in [agent(1), agent(4)] {
        let refus = admit(
            &sachant(vivant, InstanceState::Active),
            &SchedulerDecision::Structural(fusion.clone()),
        )
        .expect_err("l'une des deux sources tourne");
        assert!(
            matches!(refus, LifecycleError::StillRunning { ref node, .. } if node == &vivant.to_string()),
            "{refus:?}"
        );
    }
}

/// Une commande de cycle de vie n'est **jamais** refusée par cette règle.
///
/// L'ordonnanceur n'ajoute qu'une règle, et elle porte sur les sorties de version. Valider ici les
/// commandes de cycle de vie referait le travail de `Lifecycle::command` et les deux copies
/// divergeraient — c'est ce que le module dit ne pas faire, et voici la vérification.
#[test]
fn une_commande_de_cycle_de_vie_n_est_jamais_refusee_ici() {
    for command in Command::ALL {
        for etat in InstanceState::ALL {
            assert!(
                admit(
                    &sachant(agent(1), etat),
                    &SchedulerDecision::Lifecycle {
                        node: agent(1),
                        command,
                    }
                )
                .is_ok(),
                "« {command} » depuis {etat} : c'est `Lifecycle::command` qui tranche, pas `admit`"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 3. `place` reste seul juge de l'hôte
// ---------------------------------------------------------------------------------------------

/// L'ordonnanceur d'instances **ne peut pas** choisir d'hôte.
///
/// Tenu par l'absence, et au niveau où la propriété se décide : `packages/coordination` ne déclare
/// aucun crate d'exécution. Chercher un `use` dans les sources laisserait passer une dépendance
/// ajoutée sans import encore écrit, et la propriété voulue est « personne ne **peut** », pas
/// « personne n'a encore » — c'est l'arbitrage de `W23.a`, dont le test lit `Cargo.toml` plutôt que
/// de chercher un `derive`.
#[test]
fn le_crate_ne_depend_d_aucun_crate_d_execution() {
    let manifeste = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
        .expect("le crate lit son propre manifeste");
    for interdit in ["locus-execd", "locus-broker", "locus-deployment"] {
        assert!(
            !manifeste.contains(interdit),
            "« {interdit} » : `place` de `W4.g` reste seul juge de l'hôte, et l'ordonnanceur \
             d'instances décide **au sujet** d'instances sans dire où elles tournent"
        );
    }
}

/// Aucune décision ne porte d'hôte.
///
/// Le complément du test précédent, au niveau du type : même si le crate voyait un placeur,
/// `SchedulerDecision` n'a pas de quoi transporter un hôte. Ses deux variantes portent une identité
/// d'agent et une opération de version, et rien qui nomme une machine.
#[test]
fn aucune_decision_ne_porte_d_hote() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/scheduler.rs"))
        .expect("le module de production est lisible depuis son propre crate");
    for interdit in ["worker_id", "SandboxSpec", "Candidate", "Placement", "host"] {
        let code: String = source
            .lines()
            .filter(|ligne| !ligne.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !code.contains(interdit),
            "« {interdit} » : une décision d'ordonnancement ne nomme pas de machine"
        );
    }
}
