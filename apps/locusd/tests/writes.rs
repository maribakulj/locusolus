//! La sérialisation des écritures, **mesurée** — `W20.h`, ADR 0029 décisions 2 et 6.
//!
//! Le test de sortie de l'item demande que deux agrégats distincts « ne s'attendent pas, et qu'un
//! test le **mesure** plutôt que de le décrire ». Ces tests mesurent : ils tiennent un verrou pour de
//! vrai et regardent si l'autre passe.
//!
//! # Pourquoi la mesure est un ordre d'arrivée et non une durée
//!
//! Comparer des millisecondes ferait dépendre le verdict de la charge de la machine, et une garde
//! qui rougit quand le runner est occupé se fait désactiver. Ce qui est mesuré est donc un **ordre
//! d'événements** : le second travail finit-il pendant que le premier tient encore son verrou ?
//! C'est vrai ou faux, pas plus ou moins rapide.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier};
use std::time::Duration;

use locusd::writes::{Admitted, MAX_PENDING, StreamLocks};

/// **Deux streams distincts ne s'attendent pas.**
///
/// Le premier fil tient le verrou de `mission:a` et ne le rend qu'une fois prévenu. Le second écrit
/// sur `mission:b` **pendant** ce temps, et le test l'observe : sans indépendance par stream, il
/// serait encore bloqué.
#[test]
fn deux_streams_distincts_ne_s_attendent_pas() {
    let locks = Arc::new(StreamLocks::new());
    let tient = Arc::new(Barrier::new(2));
    let relache = Arc::new(AtomicBool::new(false));
    let b_est_passe = Arc::new(AtomicBool::new(false));

    let premier = {
        let (locks, tient, relache, b_est_passe) = (
            Arc::clone(&locks),
            Arc::clone(&tient),
            Arc::clone(&relache),
            Arc::clone(&b_est_passe),
        );
        std::thread::spawn(move || {
            locks.with("mission:a", || {
                tient.wait();
                // Attendre que `b` ait fini, en tenant toujours le verrou de `a`.
                while !relache.load(Ordering::Acquire) {
                    std::thread::yield_now();
                }
                // Le constat qui porte le test : `b` a abouti alors que `a` n'a jamais rendu son
                // verrou.
                assert!(
                    b_est_passe.load(Ordering::Acquire),
                    "un stream distinct doit passer pendant qu'un autre est tenu"
                );
            })
        })
    };

    tient.wait();
    let verdict = locks.with("mission:b", || "écrit");
    b_est_passe.store(true, Ordering::Release);
    relache.store(true, Ordering::Release);

    assert_eq!(verdict, Admitted::Done("écrit"));
    premier.join().expect("le premier fil se termine");
}

/// **Deux écritures sur le même stream se sérialisent.**
///
/// L'inverse du test précédent, et il se mesure de la même façon : le second fil ne doit **pas**
/// pouvoir entrer tant que le premier tient.
#[test]
fn deux_ecritures_sur_le_meme_stream_font_la_queue() {
    let locks = Arc::new(StreamLocks::new());
    let tient = Arc::new(Barrier::new(2));
    let relache = Arc::new(AtomicBool::new(false));
    let second_entre = Arc::new(AtomicBool::new(false));

    let premier = {
        let (locks, tient, relache, second_entre) = (
            Arc::clone(&locks),
            Arc::clone(&tient),
            Arc::clone(&relache),
            Arc::clone(&second_entre),
        );
        std::thread::spawn(move || {
            locks.with("mission:a", || {
                tient.wait();
                // Laisser au second tout le temps d'entrer s'il le pouvait.
                std::thread::sleep(Duration::from_millis(50));
                assert!(
                    !second_entre.load(Ordering::Acquire),
                    "personne d'autre n'entre sur le même stream tant que celui-ci tient"
                );
                relache.store(true, Ordering::Release);
            })
        })
    };

    tient.wait();
    let verdict = locks.with("mission:a", || {
        second_entre.store(true, Ordering::Release);
        "écrit"
    });

    assert_eq!(verdict, Admitted::Done("écrit"));
    assert!(
        relache.load(Ordering::Acquire),
        "le second n'est entré qu'après que le premier a fini"
    );
    premier.join().expect("le premier fil se termine");
}

/// **La borne franchie refuse, et le refus la nomme.**
#[test]
fn au_dela_de_la_borne_le_service_refuse_en_la_nommant() {
    let locks = Arc::new(StreamLocks::with_limit(2));
    let tient = Arc::new(Barrier::new(3));
    let relache = Arc::new(AtomicBool::new(false));

    let mut occupants = Vec::new();
    for numero in 0..2 {
        let (locks, tient, relache) =
            (Arc::clone(&locks), Arc::clone(&tient), Arc::clone(&relache));
        occupants.push(std::thread::spawn(move || {
            locks.with(&format!("mission:{numero}"), || {
                tient.wait();
                while !relache.load(Ordering::Acquire) {
                    std::thread::yield_now();
                }
            })
        }));
    }

    tient.wait();
    assert_eq!(locks.admitted(), 2, "les deux places sont prises");

    let refus = locks.with("mission:troisième", || "ne doit pas s'exécuter");
    assert_eq!(refus, Admitted::Saturated { limit: 2 });

    relache.store(true, Ordering::Release);
    for occupant in occupants {
        occupant.join().expect("les occupants se terminent");
    }

    // Et la place se rend : la borne n'est pas une porte qui se ferme pour de bon.
    assert_eq!(locks.admitted(), 0);
    assert_eq!(
        locks.with("mission:troisième", || "écrit"),
        Admitted::Done("écrit")
    );
}

/// **La table ne fuit pas.**
///
/// Une entrée par stream jamais réclamée ne se voit qu'après des mois de fonctionnement, et c'est ce
/// qui rend cette fuite-là coûteuse : elle est invisible en revue et invisible en test court. Mille
/// streams éphémères la rendraient visible.
#[test]
fn la_table_de_verrous_ne_grossit_pas_sans_fin() {
    let locks = StreamLocks::new();
    for numero in 0..1_000 {
        let verdict = locks.with(&format!("éphémère:{numero}"), || numero);
        assert_eq!(verdict, Admitted::Done(numero));
    }
    assert_eq!(
        locks.tracked(),
        0,
        "aucun verrou n'est plus tenu ni attendu"
    );
    assert!(
        locks.slots() <= 512,
        "la table brute reste bornée par le balayage : {} entrées pour mille streams",
        locks.slots()
    );
    assert_eq!(locks.admitted(), 0);
}

/// **La borne par défaut est celle que la constante annonce.**
///
/// Vérifié contre le **littéral** et non contre la constante : comparer une valeur à elle-même est
/// vrai pour n'importe quelle valeur, et une passe de mutants sur `W4.h` a montré qu'un test écrit
/// ainsi reste vert quand la valeur change.
#[test]
fn la_borne_par_defaut_est_celle_qui_est_publiee() {
    assert_eq!(MAX_PENDING, 1024);
    assert_eq!(StreamLocks::new().limit(), 1024);
    assert_eq!(StreamLocks::with_limit(7).limit(), 7);
}

/// **Un balayage ne retire jamais un verrou que quelqu'un tient.**
///
/// C'est la seule chose que le balayage ne doit pas faire : retirer une entrée vivante ferait
/// qu'un fil neuf, ne la trouvant plus, en créerait une seconde — et deux écritures se
/// recouvriraient sur le même stream.
///
/// # Pourquoi ce test n'a pas besoin d'une course
///
/// La table en références faibles a rendu **inexprimable** le retrait d'une entrée attendue par la
/// voie du compte de références ; il restait le balayage, qui est un prédicat et qu'aucun type ne
/// peut border. Une passe de mutants l'a montré en remplaçant le tri par un vidage complet.
///
/// Mais l'invariant s'observe directement : un verrou tenu doit rester **compté** après un
/// balayage. Nul besoin de faire courir deux fils l'un contre l'autre — il suffit de tenir, de
/// provoquer le balayage, et de regarder ce qui reste.
#[test]
fn un_balayage_ne_retire_jamais_un_verrou_tenu() {
    use locusd::writes::PRUNE_AT;

    let locks = Arc::new(StreamLocks::new());
    let tient = Arc::new(Barrier::new(2));
    let relache = Arc::new(AtomicBool::new(false));

    let porteur = {
        let (locks, tient, relache) =
            (Arc::clone(&locks), Arc::clone(&tient), Arc::clone(&relache));
        std::thread::spawn(move || {
            locks.with("mission:tenue", || {
                tient.wait();
                while !relache.load(Ordering::Acquire) {
                    std::thread::yield_now();
                }
            })
        })
    };

    tient.wait();
    assert_eq!(locks.tracked(), 1, "le verrou tenu est compté");

    // Assez d'entrées éphémères pour franchir le seuil et déclencher le balayage. Chacune est morte
    // dès que son `with` rend la main, donc un tri correct ne laisse vivante que celle qu'on tient.
    for numero in 0..=PRUNE_AT {
        let verdict = locks.with(&format!("éphémère:{numero}"), || numero);
        assert_eq!(verdict, Admitted::Done(numero));
    }

    assert_eq!(
        locks.tracked(),
        1,
        "le balayage n'emporte que du mort : le verrou tenu doit survivre"
    );

    relache.store(true, Ordering::Release);
    porteur.join().expect("le porteur se termine");
    assert_eq!(locks.tracked(), 0);
}
