//! Le test de sortie de `W20.p` — un fil de travail occupé à attendre la base ne famine plus rien.
//!
//! # Ce que ce fichier démontre, et pourquoi il monte un vrai serveur
//!
//! La propriété est de **latence sous charge**, pas de correction : un daemon qui appelle le journal
//! depuis son fil `tokio` répond juste, simplement il ne répond plus à rien d'autre pendant ce
//! temps. Aucun test unitaire ne peut le voir. Il faut un runtime dont le nombre de fils de travail
//! est connu, un journal qui bloque pour de vrai, et deux requêtes réelles.
//!
//! Le harnais est donc : un runtime à **un** fil de travail sur un fil système à lui, un
//! [`EventStore`] qui s'arrête sur commande, et un client en `std::net::TcpStream` avec délai de
//! lecture — pas de `tokio::time`, dont la feature n'est pas activée (ADR 0018 : les features
//! s'énumèrent une par une).
//!
//! # Rouge avant vert, vérifié
//!
//! `un_appel_lent_ne_famine_pas_le_daemon` échoue quand les handlers appellent le daemon sur le fil
//! du runtime : la seconde requête n'obtient pas de réponse avant l'expiration du délai. C'est la
//! seule forme dans laquelle cette clause veut dire quelque chose.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::Duration;

use locus_event_store::{
    Append, AppendError, Appended, Envelope, EventStore, MemoryEventStore, Sequenced,
};
use locus_protocol::Timestamp;
use locusd::composition::Runtime;
use locusd::http::router;
use locusd::offload::{Budget, MAX_BLOCKING, Offload, Offloaded};

/// Le tourniquet : un journal qui s'arrête là où on le lui demande.
///
/// Il ne feint pas la lenteur par une attente : il **bloque** jusqu'à ce qu'on l'ouvre, ce qui rend
/// le test déterministe. Une temporisation aurait rendu le verdict dépendant de la charge de la
/// machine de CI — « aucune dépendance implicite à une machine de développeur ».
#[derive(Debug, Default)]
struct Tourniquet {
    arme: AtomicBool,
    ouvert: Mutex<bool>,
    signal: Condvar,
}

impl Tourniquet {
    /// À partir de maintenant, `feed` s'arrête.
    fn armer(&self) {
        self.arme.store(true, Ordering::SeqCst);
    }

    /// Laisser passer ce qui attend.
    fn ouvrir(&self) {
        *self
            .ouvert
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = true;
        self.signal.notify_all();
    }

    fn attendre(&self) {
        if !self.arme.load(Ordering::SeqCst) {
            return;
        }
        let mut ouvert = self
            .ouvert
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while !*ouvert {
            ouvert = self
                .signal
                .wait(ouvert)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }
}

/// Un journal qui bloque sur `feed`, et se comporte normalement partout ailleurs.
struct JournalLent {
    inner: MemoryEventStore,
    tourniquet: Arc<Tourniquet>,
    entree: Mutex<Option<mpsc::Sender<()>>>,
}

impl EventStore for JournalLent {
    fn append(&self, command: Append, recorded_at: Timestamp) -> Result<Appended, AppendError> {
        self.inner.append(command, recorded_at)
    }

    fn read_stream(&self, stream_id: &str, from: u64) -> Vec<Envelope> {
        self.inner.read_stream(stream_id, from)
    }

    fn revision(&self, stream_id: &str) -> Option<u64> {
        self.inner.revision(stream_id)
    }

    fn feed(&self, from: u64) -> Vec<Sequenced> {
        if self.tourniquet.arme.load(Ordering::SeqCst) {
            // Signaler **avant** de bloquer : le test sait alors que la première requête est
            // réellement dans le journal, et n'a pas à deviner par une temporisation.
            if let Some(entree) = self
                .entree
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
            {
                let _ = entree.send(());
            }
            self.tourniquet.attendre();
        }
        self.inner.feed(from)
    }

    fn export(&self) -> Vec<Envelope> {
        self.inner.export()
    }
}

/// Un `GET` en socket brut, avec un délai de lecture — le client ne doit jamais pendre.
fn demander(adresse: &str, cible: &str, delai: Duration) -> Option<String> {
    let mut flux = TcpStream::connect(adresse).ok()?;
    flux.set_read_timeout(Some(delai)).ok()?;
    let requete = format!("GET {cible} HTTP/1.1\r\nHost: {adresse}\r\nConnection: close\r\n\r\n");
    flux.write_all(requete.as_bytes()).ok()?;
    let mut reponse = Vec::new();
    match flux.read_to_end(&mut reponse) {
        Ok(_) => Some(String::from_utf8_lossy(&reponse).into_owned()),
        // Le délai a expiré : le daemon n'a pas répondu. C'est **le** symptôme de la famine, et
        // c'est un fait du test, pas une panne du harnais.
        Err(_) => None,
    }
}

// ---------------------------------------------------------------------------------------------
// 1. La clause centrale : un fil, deux requêtes, et la seconde passe.
// ---------------------------------------------------------------------------------------------

/// **Un daemon à un seul fil de travail sert une seconde requête pendant qu'une première attend la
/// base.**
///
/// C'est la clause de sortie de `W20.p`, et elle échoue si les handlers appellent le daemon sur le
/// fil du runtime : ce fil est alors dans `feed`, et plus rien n'est accepté — pas même la sonde de
/// santé qu'un exploitant interroge pour comprendre ce qui se passe.
#[test]
fn un_appel_lent_ne_famine_pas_le_daemon() {
    let tourniquet = Arc::new(Tourniquet::default());
    let (entre, entree) = mpsc::channel();
    let journal = JournalLent {
        inner: MemoryEventStore::new(),
        tourniquet: Arc::clone(&tourniquet),
        entree: Mutex::new(Some(entre)),
    };
    let runtime = Runtime::assemble(journal, locus_policy::Policy::new());
    let readiness = runtime.catch_up();
    assert!(readiness.is_ready(), "{:?}", readiness.quarantined());

    let (adresse_out, adresse_in) = mpsc::channel();
    let service = Arc::clone(&tourniquet);
    std::thread::spawn(move || {
        let asynchrone = tokio::runtime::Builder::new_multi_thread()
            // **Un** fil de travail : c'est toute la démonstration. Avec deux, la famine ne se voit
            // pas — et un daemon réel sous charge est toujours dans le cas « tous les fils sont
            // pris », que ce test reproduit avec le plus petit nombre possible.
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("runtime asynchrone");
        asynchrone.block_on(async move {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("port libre");
            adresse_out
                .send(listener.local_addr().expect("adresse").to_string())
                .expect("l'adresse remonte");
            // Le tourniquet ne s'arme qu'une fois le serveur monté : `catch_up` a déjà appelé
            // `feed`, et bloquer là aurait empêché le daemon de démarrer.
            service.armer();
            let _ = axum::serve(listener, router(Arc::new(runtime))).await;
        });
    });
    let adresse = adresse_in.recv().expect("le serveur annonce son adresse");

    // La première requête entre dans le journal et n'en sort pas.
    let lente = adresse.clone();
    let premiere =
        std::thread::spawn(move || demander(&lente, "/timeline", Duration::from_secs(30)));
    entree
        .recv_timeout(Duration::from_secs(10))
        .expect("la première requête doit atteindre le journal");

    // La seconde ne touche pas le journal, et doit être servie **maintenant**.
    let sante = demander(&adresse, "/projections/status", Duration::from_secs(5));

    tourniquet.ouvrir();
    let premiere = premiere.join().expect("le fil de la première se termine");

    let sante = sante.expect(
        "le daemon n'a pas répondu pendant qu'une lecture attendait la base : un fil de travail \
         occupé à attendre le journal ne doit affamer personne (W20.p)",
    );
    assert!(
        sante.starts_with("HTTP/1.1 200"),
        "la sonde de santé répond pendant l'attente :\n{sante}"
    );
    let premiere = premiere.expect("et la première aboutit une fois le journal débloqué");
    assert!(
        premiere.starts_with("HTTP/1.1 200"),
        "la première requête aboutit, simplement plus tard :\n{premiere}"
    );
}

// ---------------------------------------------------------------------------------------------
// 2. La borne, et le refus qui la nomme.
// ---------------------------------------------------------------------------------------------

/// **La borne refuse au-delà, et le refus la nomme.**
///
/// « Une attente sans limite est une panne qui ne se déclare pas » — la phrase est de
/// `crate::writes`, et elle vaut ici : le pool bloquant de `tokio` a sa propre borne, haute, et s'y
/// fier ferait apparaître la saturation comme une latence que personne ne sait attribuer.
#[test]
fn la_borne_refuse_au_dela_et_le_refus_la_nomme() {
    let budget = Arc::new(Budget::with_limit(2));

    let premier = budget.admit().expect("la première place est libre");
    let second = budget.admit().expect("la seconde aussi");
    assert_eq!(budget.in_flight(), 2);
    assert!(
        budget.admit().is_none(),
        "la troisième franchit la borne : rien n'est tenté"
    );

    let refus = Offloaded::<()>::Saturated { limit: 2 }
        .or_refuse()
        .expect_err("une saturation est un refus");
    assert_eq!(refus.family(), locusd::Family::Unavailable);
    assert!(
        refus.to_string().contains('2'),
        "le refus nomme la borne, pour qu'elle ne soit pas un réglage caché : {refus}"
    );

    // Et la place se rend à la destruction, panique comprise.
    drop(second);
    assert_eq!(budget.in_flight(), 1);
    let repris = budget.admit().expect("la place rendue est reprenable");
    drop((premier, repris));
    assert_eq!(budget.in_flight(), 0);
}

/// **Une place tenue par un travail qui panique est rendue quand même.**
///
/// Sans cela, la capacité du daemon baisserait d'un cran à chaque panique, jusqu'à ce qu'il refuse
/// tout sans que rien n'ait changé — une panne qui se construit lentement et dont la cause est loin.
#[test]
fn une_panique_rend_sa_place() {
    let budget = Arc::new(Budget::with_limit(1));
    let capture = std::panic::catch_unwind({
        let budget = Arc::clone(&budget);
        move || {
            let _permit = budget.admit().expect("une place");
            panic!("le travail échoue");
        }
    });

    assert!(capture.is_err(), "la panique a bien eu lieu");
    assert_eq!(
        budget.in_flight(),
        0,
        "la place est rendue : c'est ce qu'un RAII garantit et qu'un `if` oublierait"
    );
}

/// **La borne franchie rend `503`, pas `500`.**
///
/// §22.5 : `unavailable` dit « retente », `internal` dit « ouvre un ticket ». Un daemon saturé qui
/// rendrait `500` ferait ouvrir des tickets pour de la charge.
#[tokio::test]
async fn un_daemon_sature_rend_503_et_nomme_la_borne() {
    let runtime = Arc::new(Runtime::in_memory());
    // Une borne de zéro : tout est saturé, tout de suite. C'est le seul moyen d'atteindre ce chemin
    // sans lancer soixante-quatre requêtes lentes, et ce qui est éprouvé est le refus, pas le compte.
    let desk = Offload::bounded(Arc::clone(&runtime), 0);

    let refus = desk
        .run(Runtime::readiness)
        .await
        .expect_err("une borne de zéro refuse tout");

    assert_eq!(refus.family(), locusd::Family::Unavailable);
    assert!(
        refus.to_string().contains('0'),
        "le refus nomme la borne : {refus}"
    );
}

/// **Le travail passe quand la borne le permet — sinon le test précédent ne prouverait rien.**
#[tokio::test]
async fn sous_la_borne_le_travail_a_lieu_hors_du_fil() {
    let runtime = Arc::new(Runtime::in_memory());
    runtime.catch_up();
    let desk = Offload::new(Arc::clone(&runtime));

    let readiness = desk
        .run(Runtime::readiness)
        .await
        .expect("sous la borne, le travail a lieu");

    assert!(
        readiness.is_ready(),
        "le travail a bien eu lieu, et il a rendu ce que le daemon sait : {:?}",
        readiness.quarantined()
    );
    assert_eq!(
        desk.budget().in_flight(),
        0,
        "la place est rendue quand le travail s'achève"
    );
    assert_eq!(desk.budget().limit(), MAX_BLOCKING);
}

// ---------------------------------------------------------------------------------------------
// 3. Aucun handler n'appelle le daemon directement.
// ---------------------------------------------------------------------------------------------

/// **Aucun handler ne tient un `Runtime`, donc aucun ne peut l'appeler sur le fil du runtime.**
///
/// La convention ne se tient pas par discipline : elle se tient parce que le type qu'un handler
/// reçoit est un [`Offload`], qui n'expose pas le daemon. Cette garde de source tient l'autre
/// moitié — un handler qui reprendrait l'ancien état ferait rougir la CI, au lieu de réintroduire en
/// silence la famine que cet item corrige.
///
/// C'est la même forme que la règle 4 de `boundaries.json` pour les sockets de runtime, et que
/// `W20.q` a posée pour le placement.
#[test]
fn aucun_handler_ne_tient_le_daemon() {
    let source = include_str!("../src/http.rs");

    assert!(
        !source.contains("State<Arc<Runtime"),
        "un handler qui reçoit le daemon l'appellera sur le fil du runtime : l'état du routeur est \
         `Offload<S>`, et c'est ce qui rend la famine inexprimable"
    );

    // Et le passage au pool bloquant a **un** point d'entrée. Deux conventions d'appel seraient deux
    // politiques, et la seconde serait celle qu'on oublie de border.
    assert_eq!(
        source.matches("desk.run(").count(),
        1,
        "un seul endroit cède au pool bloquant"
    );
    assert_eq!(
        source.matches("async fn hors_du_fil").count(),
        1,
        "et il porte un nom, pour que la garde ci-dessus ait quelque chose à nommer"
    );
}
