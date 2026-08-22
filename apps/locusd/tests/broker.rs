//! Ce que `locusd` sait — et ne sait pas — de l'Execution Fabric. `W4.h`, ADR 0028 décision 4.

use locus_broker::port::{BrokerError, BrokerPort, Loopback};
use locus_broker::protocol::{Missing, Verdict};
use locus_lep::SandboxLevel;
use locusd::broker::Standing;

/// Un port qui rend l'erreur qu'on lui donne, pour exercer les chemins que `Loopback` ne couvre pas.
struct Failing(BrokerError);

impl BrokerPort for Failing {
    fn endpoint(&self) -> String {
        "épreuve".to_owned()
    }

    fn readiness(&self) -> Result<Verdict, BrokerError> {
        Err(self.0.clone())
    }
}

#[test]
fn un_broker_pret_permet_l_execution() {
    let standing = Standing::probe(&Loopback::answering(Verdict::Provable {
        ceiling: SandboxLevel::S3,
    }));

    assert_eq!(
        standing,
        Standing::Ready {
            ceiling: SandboxLevel::S3
        }
    );
    assert!(standing.permits_execution());
    assert_eq!(standing.refusal(), None);
}

/// **Un hôte insuffisant ne permet pas l'exécution.**
///
/// Admettre une mission sur un hôte qui ne prouve pas son niveau serait le downgrade silencieux que
/// §21.6 interdit, pris au moment où personne ne regarde. Le test le tient sur la **permission**, pas
/// sur la phrase.
#[test]
fn un_hote_insuffisant_ne_permet_pas_l_execution_et_nomme_ses_manques() {
    let standing = Standing::probe(&Loopback::answering(Verdict::HostShort {
        ceiling: SandboxLevel::S1,
        missing: vec![Missing::Unavailable {
            what: "cgroup v2".to_owned(),
            reason: "monté en v1".to_owned(),
        }],
    }));

    assert!(!standing.permits_execution());
    let refus = standing.refusal().expect("un refus");
    assert!(
        refus.contains("cgroup v2") && refus.contains("monté en v1"),
        "le refus nomme le manque et sa raison : {refus}"
    );
}

/// **Les quatre états ne se confondent pas, et le test le tient par le type.**
///
/// Une phrase se reformule ; un type non. C'est la leçon des paires de refus de `wire.rs`, où deux
/// motifs qui se ressemblent sont tenus par égalité stricte sur leur code plutôt que par lecture.
#[test]
fn injoignable_refuse_et_hote_court_sont_trois_etats_distincts() {
    let injoignable = Standing::probe(&Loopback::unreachable("/run/absent.sock", "rien n'écoute"));
    let refuse = Standing::probe(&Loopback::answering(Verdict::Refused {
        why: "appelant non admis".to_owned(),
    }));
    let court = Standing::probe(&Loopback::answering(Verdict::HostShort {
        ceiling: SandboxLevel::S1,
        missing: Vec::new(),
    }));

    assert!(matches!(injoignable, Standing::Unreachable { .. }));
    assert!(matches!(refuse, Standing::Refused { .. }));
    assert!(matches!(court, Standing::HostShort { .. }));
    assert_ne!(injoignable, refuse);
    assert_ne!(refuse, court);

    // Aucun des trois ne permet l'exécution, et c'est la seule chose qu'ils ont en commun.
    for standing in [&injoignable, &refuse, &court] {
        assert!(!standing.permits_execution());
        assert!(standing.refusal().is_some());
    }
}

/// **Un broker illisible est joignable, et le dire autrement enverrait démarrer un service qui
/// tourne.**
///
/// C'est le cas d'un écart de version entre les deux binaires. Le classer `Unreachable` ferait
/// chercher une panne de service là où il y a un désaccord de protocole — la faute exacte que la
/// décision 4 sépare, rencontrée un cran plus loin.
#[test]
fn un_broker_illisible_n_est_pas_un_broker_eteint() {
    let illisible = Standing::probe(&Failing(BrokerError::Malformed {
        why: "le broker parle broker/9.9".to_owned(),
    }));
    let trop_long = Standing::probe(&Failing(BrokerError::TooLong { read: 999_999 }));

    assert!(
        matches!(illisible, Standing::Refused { .. }),
        "un broker qui a parlé n'est pas injoignable : {illisible:?}"
    );
    assert!(matches!(trop_long, Standing::Refused { .. }));
    let phrase = illisible.to_string();
    assert!(
        phrase.contains("broker/9.9"),
        "la cause voyage jusqu'à l'exploitant : {phrase}"
    );
}

/// **Le message de démarrage nomme un endroit réel.**
///
/// Un « broker injoignable » sans chemin fait chercher partout. Le test lit la phrase parce que
/// c'est elle que l'exploitant lira, et il vérifie qu'elle porte l'endroit — pas sa formulation.
#[test]
fn l_injoignable_nomme_ou_l_on_a_essaye() {
    let standing = Standing::probe(&Loopback::unreachable(
        "/run/locus/broker.sock",
        "aucun fichier de ce type",
    ));

    let phrase = standing.to_string();
    assert!(
        phrase.contains("/run/locus/broker.sock") && phrase.contains("aucun fichier de ce type"),
        "l'endroit et la cause doivent tous deux être là : {phrase}"
    );
}
