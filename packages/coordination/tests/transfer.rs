//! Test de sortie de `W21.i` — **`handed_over_attempts`**, ADR 0024.
//!
//! 1. La mesure se lit du `Handover` de `W16.e` et de rien d'autre.
//! 2. Un `kill` ne produit **aucune** mesure : abandonner n'est pas transmettre.
//! 3. Un relais à zéro tentative n'est pas une absence de relais.
//! 4. Aucune signature ne parle de taille ni de volume.

use locus_coordination::{HandedOver, Handover, HandoverError, InstanceState, Outcome};
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

/// Un passage de témoin après un drain qui laisse `remaining` tentatives en vol.
fn relais(from: u8, to: u8, remaining: usize) -> Handover {
    Handover::after_drain(agent(from), agent(to), Outcome::Draining { remaining })
        .expect("un drain passe la main")
}

// ---------------------------------------------------------------------------------------------
// 1. Ce que la mesure compte
// ---------------------------------------------------------------------------------------------

/// **Les tentatives transmises s'additionnent, et les relais se comptent à part.**
#[test]
fn les_tentatives_et_les_relais_se_comptent_separement() {
    let mesure = HandedOver::over(&[relais(1, 2, 3), relais(2, 3, 4), relais(3, 4, 0)]);

    assert_eq!(mesure.attempts(), 7);
    assert_eq!(mesure.transfers(), 3);
}

/// **Aucun relais rend deux zéros.**
#[test]
fn aucun_relais_rend_deux_zeros() {
    let mesure = HandedOver::over(&[]);

    assert_eq!(mesure.attempts(), 0);
    assert_eq!(mesure.transfers(), 0);
}

// ---------------------------------------------------------------------------------------------
// 2. Abandonner n'est pas transmettre
// ---------------------------------------------------------------------------------------------

/// **Un `kill` ne produit aucun passage de témoin, donc rien que cette mesure sache lire.**
///
/// C'est le test qui porte l'item. `Outcome::Killed` porte lui aussi un compte — combien de
/// tentatives ont été **abandonnées**, et il le porte même quand il vaut zéro. Une mesure qui
/// additionnerait les deux dirait qu'une reconfiguration a transmis cinq tentatives là où cinq ont
/// été **perdues** : deux issues opposées sous un même nombre, et celle qui coûte le plus cher
/// rendue invisible.
///
/// La confusion est **inexprimable** plutôt qu'évitée par discipline : `after_drain` refuse tout ce
/// qui n'est pas un drain, et il n'existe aucun autre constructeur.
#[test]
fn un_kill_ne_produit_aucun_passage_de_temoin() {
    let refus = Handover::after_drain(agent(1), agent(2), Outcome::Killed { abandoned: 5 })
        .expect_err("un kill abandonne, il ne passe pas la main");

    assert_eq!(refus, HandoverError::NotDraining);
}

/// **Un nœud posé n'a rien à transmettre non plus.**
///
/// `Settled` est la troisième issue, et elle ne passe pas la main davantage qu'un `kill` — pour une
/// raison différente : il n'y a rien en vol, pas parce qu'on l'a abandonné mais parce que tout est
/// fini.
#[test]
fn un_noeud_pose_ne_passe_pas_la_main() {
    let refus = Handover::after_drain(
        agent(1),
        agent(2),
        Outcome::Settled(InstanceState::Completed),
    )
    .expect_err("un nœud posé n'a rien à transmettre");

    assert_eq!(refus, HandoverError::NotDraining);
}

/// **Cinq abandonnées et cinq transmises ne rendent jamais la même chose.**
///
/// La démonstration chiffrée de la distinction. Le lot de gauche ne contient aucun relais — le kill
/// n'en produit pas — donc la mesure est vide ; celui de droite en contient un, à cinq tentatives.
#[test]
fn abandonner_cinq_et_transmettre_cinq_ne_se_confondent_pas() {
    // Le kill ne peut pas entrer dans la mesure : il ne produit pas de `Handover`.
    assert!(Handover::after_drain(agent(1), agent(2), Outcome::Killed { abandoned: 5 }).is_err());
    let perdu = HandedOver::over(&[]);
    let transmis = HandedOver::over(&[relais(1, 2, 5)]);

    assert_eq!(perdu.attempts(), 0);
    assert_eq!(perdu.transfers(), 0);
    assert_eq!(transmis.attempts(), 5);
    assert_eq!(transmis.transfers(), 1);
    assert_ne!(perdu, transmis);
}

// ---------------------------------------------------------------------------------------------
// 3. Un relais gratuit reste un relais
// ---------------------------------------------------------------------------------------------

/// **Un relais à zéro tentative n'est pas une absence de relais.**
///
/// Deux nœuds peuvent se relayer sans que rien ne soit en vol : c'est un fait, et un bon — la
/// reconfiguration n'a rien coûté. Le confondre avec « aucun relais n'a eu lieu » ferait lire une
/// organisation qui se recompose sans frais comme une organisation qui ne se recompose pas.
///
/// Les deux ont le **même** nombre de tentatives ; c'est le compte des relais qui les sépare.
#[test]
fn un_relais_gratuit_n_est_pas_une_absence_de_relais() {
    let aucun = HandedOver::over(&[]);
    let gratuit = HandedOver::over(&[relais(1, 2, 0)]);

    assert_eq!(
        aucun.attempts(),
        gratuit.attempts(),
        "les deux transmettent zéro tentative"
    );
    assert_ne!(
        aucun.transfers(),
        gratuit.transfers(),
        "et c'est le compte des relais qui les distingue"
    );
    assert_eq!(gratuit.transfers(), 1);
}

/// **Un nœud ne se passe pas le témoin à lui-même.**
#[test]
fn un_relais_vers_soi_meme_est_refuse() {
    let refus = Handover::after_drain(agent(1), agent(1), Outcome::Draining { remaining: 2 })
        .expect_err("un relais réflexif est refusé");

    assert_eq!(refus, HandoverError::ToItself);
}

// ---------------------------------------------------------------------------------------------
// 4. Aucun octet, aucun jugement
// ---------------------------------------------------------------------------------------------

/// **Aucune signature ne parle de taille ni de volume.**
///
/// L'ADR 0019 condition 3 interdit la copie de contexte qui produirait des octets. Une métrique de
/// volume vaudrait donc zéro en permanence — un cadran qu'on finirait par croire cassé plutôt que
/// juste — ou ferait ajouter la copie, et **créerait le coût qu'elle prétend observer**.
///
/// Les motifs visent des signatures : la documentation explique longuement pourquoi il n'y a pas
/// d'octets, et un test qui refuserait le mot mordrait sur son propre motif.
#[test]
fn la_source_ne_parle_ni_de_taille_ni_de_volume() {
    let source = include_str!("../src/transfer.rs");

    for interdit in [
        "fn bytes",
        "fn size",
        "fn volume",
        "fn len",
        "fn weight",
        "const MIN",
        "const MAX",
        "fn is_healthy",
        "fn score",
        "enum Verdict",
    ] {
        assert!(
            !source.contains(interdit),
            "« {interdit} » ferait lire un volume là où il y a un compte de tentatives"
        );
    }
}
