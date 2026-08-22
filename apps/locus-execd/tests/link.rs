//! Le couloir, traversé en entier — `W4.h`, ADR 0028.
//!
//! # Ce que ce fichier éprouve, et que rien d'autre n'éprouve
//!
//! Les tests de `packages/broker` exercent le tube contre un répondeur d'épreuve ; ceux de
//! `apps/locusd` exercent la lecture d'un verdict contre un port en mémoire. Aucun des deux ne
//! montre que **les deux binaires se parlent**, ce qui est exactement l'absence que `W22.c` a
//! trouvée : chaque moitié était cohérente séparément.
//!
//! Ici, le vrai lecteur d'hôte de `locus-execd` alimente le vrai serveur, le vrai client de `locusd`
//! le lit, et le verdict arrive dans le type que le daemon affiche au démarrage.
//!
//! # Pourquoi l'assertion n'exige pas un hôte capable
//!
//! Le runner de CI n'est pas une machine à sandbox, et `W5.f` l'a établi en le mesurant. Exiger
//! `Ready` ferait dépendre ce test des capacités de la machine qui l'exécute — « aucune dépendance
//! implicite à une machine de développeur ». Ce qui est éprouvé est que le couloir **rend un
//! verdict d'hôte réel** plutôt qu'une panne de lien, et la distinction entre les deux est tout
//! l'objet de l'ADR 0028 décision 4.

use std::path::PathBuf;
use std::thread;

use locus_broker::port::BrokerPort;
use locus_broker::protocol::Missing as WireMissing;
use locus_broker::unix::{UnixSocketBroker, listen};
use locus_execd::link::{serve, verdict};
use locus_execd::linux::{HostFacts, Missing};
use locusd::broker::Standing;

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("locus-execd-link-{name}"));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("répertoire de travail");
        Self(path)
    }

    fn socket(&self) -> PathBuf {
        self.0.join("broker.sock")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// **Le daemon interroge le broker, et reçoit un verdict d'hôte réel.**
#[test]
fn locusd_atteint_locus_execd_et_lit_ce_que_l_hote_prouve() {
    let scratch = Scratch::new("bout-en-bout");
    let path = scratch.socket();
    let listener = listen(&path).expect("écoute");
    let server = thread::spawn(move || {
        let facts = HostFacts::read_host();
        let (stream, _) = listener.accept().expect("connexion");
        locus_broker::unix::answer(&stream, |_| verdict(&facts)).expect("réponse");
    });

    let standing = Standing::probe(&UnixSocketBroker::at(&path));

    assert!(
        matches!(
            standing,
            Standing::Ready { .. } | Standing::HostShort { .. }
        ),
        "le couloir doit rendre un verdict d'hôte, pas une panne de lien : {standing}"
    );
    server.join().expect("le serveur se termine");
}

/// **Le broker éteint est injoignable, et le daemon le dit sans confondre.**
///
/// Le pendant du test précédent, et le plus important des deux : c'est celui qui échouerait si le
/// jour venait où « je n'ai pas pu demander » se lisait « on m'a dit non ».
#[test]
fn un_broker_qui_n_ecoute_pas_se_dit_injoignable_et_nomme_le_chemin() {
    let scratch = Scratch::new("eteint");
    let path = scratch.socket();

    let standing = Standing::probe(&UnixSocketBroker::at(&path));

    let Standing::Unreachable { endpoint, .. } = &standing else {
        panic!("un broker éteint est injoignable : {standing:?}");
    };
    assert_eq!(endpoint, &path.display().to_string());
    assert!(!standing.permits_execution());
}

/// **Le broker sert plusieurs demandes de suite, et une connexion fautive ne l'emporte pas.**
///
/// Un client qui coupe au milieu ne doit pas arrêter le service : ce serait un déni de service
/// ouvert à quiconque peut se connecter. Le test coupe pour de vrai, puis vérifie que la demande
/// suivante aboutit.
#[test]
fn une_connexion_fautive_n_arrete_pas_le_broker() {
    let scratch = Scratch::new("resilience");
    let path = scratch.socket();
    let listener = listen(&path).expect("écoute");
    // La boucle de service n'a pas d'arrêt : c'est un daemon, et lui en donner un pour la commodité
    // d'un test serait une API écrite pour son harnais. Le fil est donc détaché et meurt avec le
    // processus de test, ce qui est le comportement réel du binaire.
    thread::spawn(move || {
        let facts = HostFacts::read_host();
        serve(&listener, &facts, |_| {});
    });

    // Une connexion qui se ferme sans rien dire : le serveur doit la constater et poursuivre.
    drop(std::os::unix::net::UnixStream::connect(&path).expect("connexion"));

    let standing = Standing::probe(&UnixSocketBroker::at(&path));
    assert!(
        matches!(
            standing,
            Standing::Ready { .. } | Standing::HostShort { .. }
        ),
        "la demande suivante doit aboutir : {standing}"
    );

    // Et une troisième, pour que « il en a servi deux » ne se lise pas « il en sert exactement
    // deux » : la boucle ne compte rien, et le test ne doit pas laisser croire le contraire.
    assert!(
        UnixSocketBroker::at(&path).readiness().is_ok(),
        "la boucle sert sans compter"
    );
}

/// **Les deux sortes de manque restent deux, en traversant** — la distinction de `W5.h`.
///
/// « L'hôte ne l'offre pas » envoie changer de machine ; « on n'a pas pu l'établir » envoie regarder
/// pourquoi la lecture a échoué. Les fondre ferait acheter du matériel pour un problème de sonde.
///
/// Une passe de mutants a montré que **rien n'éprouvait cette traduction** : on pouvait rendre tout
/// manque indéterminé comme indisponible sans qu'un test bronche, alors que c'est la propriété que
/// le registre affirme conserver de bout en bout.
#[test]
fn un_manque_indetermine_ne_devient_pas_un_manque_indisponible() {
    let indisponible = locus_execd::link::missing(&Missing::Unavailable {
        what: "cgroup v2",
        reason: "monté en v1".to_owned(),
    });
    let indetermine = locus_execd::link::missing(&Missing::Undetermined {
        what: "quota de projet",
        reason: "aucune racine de stockage déclarée".to_owned(),
    });

    assert!(
        matches!(indisponible, WireMissing::Unavailable { .. }),
        "un manque indisponible reste indisponible : {indisponible:?}"
    );
    assert!(
        matches!(indetermine, WireMissing::Undetermined { .. }),
        "une ignorance n'est pas une absence : {indetermine:?}"
    );

    // Et le texte suit la sorte : deux phrases identiques rendraient la distinction invisible à qui
    // lit le rapport plutôt que le type.
    assert_ne!(indisponible.to_string(), indetermine.to_string());
    assert!(
        indetermine.to_string().contains("indéterminé"),
        "l'ignorance se dit : {indetermine}"
    );
}
