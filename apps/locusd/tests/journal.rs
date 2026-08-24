//! Le test de sortie de `W20.m` — le journal durable, câblé.
//!
//! # Les trois clauses, et celle qui a été déplacée
//!
//! L'item en portait trois. Deux sont ici : le profil choisit le backend **sans que
//! `composition.rs` nomme un backend**, et un redémarrage ne perd rien. La troisième — appeler le
//! driver bloquant hors du fil du runtime asynchrone — est devenue `W20.p`, et la raison est écrite
//! plutôt que tue : elle change la convention d'appel de toute la couche HTTP, c'est une propriété
//! de **latence sous charge** et non de correction, et les deux premières forment une capacité
//! complète sans elle. Découper vaut mieux que livrer à moitié en le taisant.

use std::sync::Arc;

use locus_deployment::ProfileKind;
use locus_event_store::{MemoryEventStore, PostgresEventStore};
use locus_policy::Policy;
use locus_protocol::id::{Agent, Command as CommandId, Event as EventId, Project, Workspace};
use locus_protocol::{Id, IdKind, Timestamp};
use locusd::journal::{Choice, Refusal, promises_durability};
use locusd::lep::{Desk, Identities, MemoryQueue, MemoryRegistry, Offer, WorkerIdentity};
use locusd::{CommandError, Runtime};

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

// ---------------------------------------------------------------------------------------------
// 1. Un profil qui promet la durabilité ne démarre pas sur un journal volatile.
// ---------------------------------------------------------------------------------------------

/// **Les quatre profils qui hébergent un control plane refusent un journal volatile.**
///
/// Un `single-node-vm` qui repartirait vide à chaque redémarrage mentirait à tout ce qui s'y
/// connecte, et le mensonge serait silencieux : le daemon a l'air d'aller bien. Le refus est au
/// démarrage, avant d'ouvrir le port, pour la même raison que `main.rs` refuse de servir avec une
/// projection en quarantaine — un refus se voit.
#[test]
fn un_profil_qui_promet_la_durabilite_refuse_un_journal_volatile() {
    for profile in ProfileKind::ALL {
        let verdict = Choice::decide(profile.slug(), None);
        if promises_durability(profile) {
            assert_eq!(
                verdict,
                Err(Refusal::VolatileUnderDurableProfile { profile }),
                "« {profile} » héberge un control plane : il ne peut pas démarrer volatile"
            );
        } else {
            assert_eq!(
                verdict,
                Ok(Choice::Volatile),
                "« {profile} » met tout sur un poste : un journal volatile y est un choix, pas un \
                 mensonge"
            );
        }
    }
}

/// **Et un seul profil ne le promet pas.**
///
/// Le pendant du test précédent, et celui qui l'empêche de passer pour de mauvaises raisons : si
/// `promises_durability` rendait `false` partout, la boucle ci-dessus n'éprouverait plus rien.
#[test]
fn un_seul_des_cinq_profils_ne_promet_pas_la_durabilite() {
    let sans: Vec<&str> = ProfileKind::ALL
        .into_iter()
        .filter(|profile| !promises_durability(*profile))
        .map(ProfileKind::slug)
        .collect();
    assert_eq!(sans, vec!["personal-local"]);
}

/// **Une adresse vide n'est pas une adresse.**
///
/// La traiter comme telle ferait échouer la connexion plus tard, avec un message de driver au lieu
/// d'un refus de configuration — et un exploitant lirait « base injoignable » là où il faut lire
/// « vous n'avez rien renseigné ».
#[test]
fn un_journal_vide_ne_compte_pas_pour_un_journal() {
    assert!(matches!(
        Choice::decide("single-node-vm", Some("   ".to_owned())),
        Err(Refusal::VolatileUnderDurableProfile { .. })
    ));
    assert_eq!(
        Choice::decide("single-node-vm", Some("host=x".to_owned())),
        Ok(Choice::Durable("host=x".to_owned()))
    );
}

/// **Un profil inconnu est refusé en nommant les cinq.**
#[test]
fn un_profil_inconnu_est_refuse_en_nommant_les_cinq() {
    let Err(refusal) = Choice::decide("personal-laptop", None) else {
        panic!("un profil inconnu ne démarre pas");
    };
    let dit = refusal.to_string();
    for profile in ProfileKind::ALL {
        assert!(
            dit.contains(profile.slug()),
            "le refus doit nommer « {profile} » : {dit}"
        );
    }
}

/// **Ce qui est annoncé au démarrage ne cite jamais l'adresse.**
///
/// Une chaîne de connexion porte un mot de passe. `CLAUDE.md` interdit de journaliser une créance,
/// et l'imprimer au démarrage la mettrait dans tous les journaux de supervision — l'endroit exact
/// d'où on la copie dans un rapport de bug.
#[test]
fn le_demarrage_ne_cite_jamais_la_chaine_de_connexion() {
    let secret = "host=db user=locus password=tres-secret dbname=locus";
    let choix = Choice::Durable(secret.to_owned());
    assert!(!choix.describe().contains("tres-secret"));
    assert!(!choix.describe().contains("host=db"));
    assert!(choix.describe().contains("durable"));
}

// ---------------------------------------------------------------------------------------------
// 2. `composition.rs` ne nomme aucun backend — la seule chose que `S` garantissait.
// ---------------------------------------------------------------------------------------------

/// **Substituer le driver ne touche pas le composition root.**
///
/// `W20.d` l'affirmait : le driver « se substitue **sans toucher à ce fichier** — c'est la seule
/// chose que le paramètre de type est là pour garantir ». L'affirmation est restée invérifiée tant
/// qu'il n'existait qu'un backend, comme celle de `W1.c` sur la suite de contract tests, démentie
/// en `W20.i`. Celle-ci tient : la substitution a lieu dans le binaire.
///
/// Le test lit le source plutôt que les types, parce que c'est l'**absence** qu'il faut vérifier, et
/// qu'une absence ne se compile pas.
#[test]
fn le_composition_root_ne_nomme_aucun_backend_concret() {
    let source = include_str!("../src/composition.rs");
    assert!(
        !source.contains("Postgres"),
        "`composition.rs` nomme un backend concret : le paramètre `S` cesse alors de servir à quoi \
         que ce soit, et substituer un driver redevient une modification du composition root"
    );
    // `MemoryEventStore` y est nommé, et c'est **voulu** : `Runtime::in_memory()` est l'assemblage
    // du profil `personal-local`, écrit dans un `impl` séparé qui ne contraint pas `Runtime<S>`.
    // Le distinguer évite que ce test se lise « aucun backend », ce qui serait faux.
    assert!(source.contains("impl Runtime<MemoryEventStore>"));
}

// ---------------------------------------------------------------------------------------------
// 3. Un redémarrage ne perd rien — la clause centrale de `W12.d`.
// ---------------------------------------------------------------------------------------------

/// Une source d'identifiants déterministe, pour que deux démarrages soient comparables.
#[derive(Debug, Default)]
struct Identites {
    prochain: std::sync::atomic::AtomicU8,
}

impl Identities for Identites {
    fn events(&self, count: usize) -> Result<Vec<Id<EventId>>, CommandError> {
        Ok((0..count)
            .map(|_| {
                id::<EventId>(
                    self.prochain
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                )
            })
            .collect())
    }

    fn command(&self) -> Result<Id<CommandId>, CommandError> {
        Ok(id::<CommandId>(
            self.prochain
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        ))
    }
}

fn offre() -> Offer {
    let chemin =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas/examples");
    let lire = |nom: &str| -> serde_json::Value {
        let brut = std::fs::read_to_string(chemin.join(nom)).expect("fixture lisible");
        let mut valeur: serde_json::Value = serde_json::from_str(&brut).expect("JSON valide");
        valeur.as_object_mut().expect("un objet").remove("_fixture");
        valeur
    };
    let mission: locus_lep::MissionEnvelope =
        serde_json::from_value(lire("mission-envelope-nominal.json")).expect("mission");
    let mut lease: locus_lep::Lease =
        serde_json::from_value(lire("lease-expired.json")).expect("bail");
    lease.task_id.clone_from(&mission.task_id);
    "canterel-vm-linux-01".clone_into(&mut lease.worker_id);
    Offer { mission, lease }
}

fn desk(offres: usize) -> Desk {
    let file = Arc::new(MemoryQueue::new());
    for _ in 0..offres {
        file.push(offre());
    }
    let registre = Arc::new(MemoryRegistry::new());
    registre.admit(
        "creance",
        WorkerIdentity {
            worker_id: "canterel-vm-linux-01".to_owned(),
            workspace_id: id::<Workspace>(2),
            principal_id: id::<Agent>(3),
        },
    );
    // `W20.q` : la réclamation demande le placement au broker. Un `Loopback` qui **place** est ce
    // qu'il faut ici — ce que ce fichier éprouve est la durabilité du journal, pas le placement, et
    // laisser le lien absent ferait échouer la réclamation pour une raison hors sujet.
    Desk::new(file, registre, Arc::new(Identites::default())).placing(Arc::new(
        locus_broker::port::Loopback::answering(locus_broker::protocol::Verdict::Placed {
            worker: "canterel-vm-linux-01".to_owned(),
            level: locus_lep::SandboxLevel::S3,
        }),
    ))
}

/// Ce que le worker annonce — la fixture Linux de `W0.7`, telle quelle.
fn manifeste() -> locus_lep::CapabilityManifest {
    let chemin =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas/examples");
    let brut = std::fs::read_to_string(chemin.join("capability-manifest-vm-linux.json"))
        .expect("fixture lisible");
    let mut valeur: serde_json::Value = serde_json::from_str(&brut).expect("JSON valide");
    valeur.as_object_mut().expect("un objet").remove("_fixture");
    serde_json::from_value(valeur).expect("manifeste")
}

fn submitted() -> locusd::lep::Submitted {
    locusd::lep::Submitted {
        idempotency_key: "idem-redemarrage".to_owned(),
        project_id: id::<Project>(4),
        occurred_at: Timestamp::from_millis(1_700_000_000_000),
    }
}

/// **`locusd` redémarre, et tout est encore là** — la clause centrale de `W12.d`.
///
/// Un **second** `Runtime` sur la même base est, du point de vue du journal, exactement un
/// redémarrage : ses quatre projections partent vides et se reconstruisent depuis les faits écrits.
/// Ce que ce test exige est que les queries de §22.4 rendent après ce qu'elles rendaient avant.
///
/// Sans base, il **dit** ce qu'il n'a pas fait plutôt que de passer en silence — la règle de l'ADR
/// 0030 décision 4, appliquée ici aussi : un verdict vert sans journal durable dirait « survit au
/// redémarrage » là où il faut lire « pas exécuté ».
#[test]
fn un_redemarrage_ne_perd_rien() {
    let Ok(url) = std::env::var("LOCUS_TEST_POSTGRES") else {
        eprintln!(
            "journal: redémarrage non éprouvé — `LOCUS_TEST_POSTGRES` absent. Ce verdict ne porte \
             pas sur la durabilité."
        );
        return;
    };

    let premier = PostgresEventStore::connect(&url).expect("la base répond");
    premier.truncate_for_tests().expect("journal vide");
    let daemon = Runtime::assemble(premier, Policy::new()).with_lep(desk(1));
    let offre = daemon
        .lep_claim(
            "creance",
            Some(&manifeste()),
            &submitted(),
            Timestamp::from_millis(1_700_000_000_000),
        )
        .expect("la réclamation aboutit");
    assert!(offre.is_some(), "une offre attendait dans la file");

    let avant_timeline = daemon.timeline(None, None).expect("timeline lisible");
    let avant_workers = daemon.workers(None, None).expect("workers lisibles");
    assert_eq!(avant_timeline.items.len(), 1, "un fait a été écrit");
    assert_eq!(
        avant_workers.items,
        // Le graphe d'exécution préfixe ses nœuds par leur sorte : `worker:` ici. Écrit en clair
        // plutôt que reconstruit, pour que ce test dise ce qu'un client lit réellement.
        vec!["worker:canterel-vm-linux-01".to_owned()],
        "et la projection l'a vu — `W20.l`"
    );
    drop(daemon);

    // Le redémarrage : une nouvelle connexion, un nouveau `Runtime`, des projections vides.
    let second = PostgresEventStore::connect(&url).expect("la base répond encore");
    let redemarre = Runtime::assemble(second, Policy::new()).with_lep(desk(0));
    assert!(
        redemarre
            .workers(None, None)
            .expect("lisible")
            .items
            .is_empty(),
        "avant le rattrapage, les projections d'un daemon neuf sont vides — sinon ce test ne \
         prouverait rien du rejeu"
    );
    let readiness = redemarre.catch_up();
    assert!(readiness.is_ready(), "{:?}", readiness.quarantined());

    assert_eq!(
        redemarre
            .timeline(None, None)
            .expect("timeline")
            .items
            .len(),
        avant_timeline.items.len(),
        "le journal a survécu"
    );
    assert_eq!(
        redemarre.workers(None, None).expect("workers").items,
        avant_workers.items,
        "et les projections se sont reconstruites depuis lui"
    );
}

/// **Le même scénario sur un journal volatile perd tout** — ce qui rend le test précédent probant.
///
/// Sans ce pendant, « tout est encore là » pourrait tenir à une projection qui se souvient plutôt
/// qu'à un journal qui dure. Ici le second `Runtime` part sur un `MemoryEventStore` neuf, et il ne
/// retrouve rien : c'est exactement ce qu'un profil durable ne doit pas pouvoir faire, et c'est
/// pourquoi `Choice::decide` le refuse.
#[test]
fn un_journal_volatile_ne_survit_pas_a_un_redemarrage() {
    let daemon = Runtime::assemble(MemoryEventStore::new(), Policy::new()).with_lep(desk(1));
    daemon
        .lep_claim(
            "creance",
            Some(&manifeste()),
            &submitted(),
            Timestamp::from_millis(1_700_000_000_000),
        )
        .expect("la réclamation aboutit");
    assert_eq!(
        daemon.timeline(None, None).expect("timeline").items.len(),
        1
    );

    let redemarre = Runtime::assemble(MemoryEventStore::new(), Policy::new()).with_lep(desk(0));
    redemarre.catch_up();
    assert!(
        redemarre
            .timeline(None, None)
            .expect("timeline")
            .items
            .is_empty(),
        "un journal en mémoire ne survit pas, et c'est ce que `Choice::decide` refuse de promettre"
    );
}
