//! Test de sortie de `W21.j` — **`agent_lifetime`**, ADR 0024.
//!
//! 1. Une instance encore en place n'a **pas** de durée close, et n'est pas mesurée jusqu'à
//!    maintenant.
//! 2. Le module ne lit aucune horloge — c'est ce qui rend la règle précédente tenable.
//! 3. Trois façons de partir, et les fondre ferait lire une flotte tuée comme une flotte finie.
//! 4. Aucun chemin vers ce que l'instance a accompli.

use locus_coordination::{InstanceState, Lifetime, LifetimeError, Span};
use locus_protocol::Timestamp;

fn instant(millis: i64) -> Timestamp {
    Timestamp::from_millis(millis)
}

// ---------------------------------------------------------------------------------------------
// 1. Encore en place n'est pas une durée
// ---------------------------------------------------------------------------------------------

/// **Une instance toujours là ne porte aucune durée.**
///
/// Le test qui porte l'item. Une durée arrêtée à l'instant de lecture **change à chaque lecture** :
/// deux rapports produits à dix minutes d'intervalle donneraient deux valeurs pour le même passé, et
/// le second aurait l'air d'un fait nouveau. Ce n'est pas un fait du journal, c'est un fait de la
/// montre de celui qui lit.
#[test]
fn une_instance_encore_en_place_n_a_pas_de_duree() {
    let sejour = Span::standing(instant(1_000));

    assert_eq!(sejour.lifetime(), Lifetime::Standing);
    assert_eq!(sejour.lifetime().millis(), None);
    assert_eq!(sejour.lifetime().ended_as(), None);
}

/// **Une instance partie porte sa durée et son état de sortie.**
#[test]
fn une_instance_partie_porte_sa_duree() {
    let sejour = Span::left(instant(1_000), instant(4_500), InstanceState::Completed)
        .expect("une sortie licite");

    assert_eq!(
        sejour.lifetime(),
        Lifetime::Closed {
            millis: 3_500,
            ended_as: InstanceState::Completed
        }
    );
    assert_eq!(sejour.lifetime().millis(), Some(3_500));
}

/// **Un séjour de durée nulle est une durée, pas une absence de durée.**
///
/// Une instance entrée et sortie au même instant a bel et bien fini son séjour ; la confondre avec
/// une instance encore là ferait attendre une sortie qui a déjà eu lieu.
#[test]
fn un_sejour_instantane_reste_un_sejour_clos() {
    let eclair =
        Span::left(instant(7), instant(7), InstanceState::Failed).expect("une sortie licite");

    assert_eq!(eclair.lifetime().millis(), Some(0));
    assert_ne!(eclair.lifetime(), Lifetime::Standing);
}

// ---------------------------------------------------------------------------------------------
// 2. Aucune horloge
// ---------------------------------------------------------------------------------------------

/// **Le module ne lit aucune horloge, donc `Standing` ne peut pas devenir une durée.**
///
/// Une règle qui dépendrait de la discipline d'appel tomberait au premier appelant pressé. Ici la
/// seule façon d'obtenir un instant est de le **recevoir** : il n'y a pas d'instant courant à
/// soustraire, même par erreur.
///
/// Les motifs visent des **formes de code** — un import, un appel — pas des mots, et cette fois la
/// règle est **vérifiée** au lieu d'être appliquée de mémoire : voir [`forme_de_code`].
#[test]
fn la_source_ne_lit_aucune_horloge() {
    let code = code_seul(include_str!("../src/lifetime.rs"));
    assert!(
        code.contains("pub fn"),
        "le nettoyage a trop enlevé : ce test ne lit plus ce qu'il croit lire"
    );

    for interdit in [
        "std::time",
        "SystemTime",
        "Instant::now",
        "::now()",
        "fn now",
        "UNIX_EPOCH",
    ] {
        assert!(
            !code.contains(interdit),
            "« {interdit} » permettrait de transformer « encore en place » en une durée"
        );
    }
}

/// **Le code d'un fichier, c'est sa source moins ses commentaires.**
///
/// Huit fois dans ce dépôt, une anti-garde a mordu sur la phrase de documentation qui *expliquait*
/// l'absence qu'elle surveille. Sept fois, la réparation a été de corriger le motif et d'écrire dans
/// le commentaire qu'il faut viser une forme de code. La huitième a eu lieu **dans un test dont le
/// commentaire portait déjà cette phrase**, à deux lignes du motif fautif.
///
/// Ce n'est donc pas l'attention qui manque. La première réparation tentée ici classait les
/// **motifs** — déclaration, appel, constante — et elle a échoué en une minute : `Instant::now` est
/// un chemin, donc du code, et pourtant rien ne le distingue de `std::time`, qui s'écrit tel quel
/// dans une phrase. Deux chemins, deux natures, une seule syntaxe.
///
/// La bonne réponse est de l'autre côté : ne pas classer l'aiguille, **nettoyer la botte de foin**.
/// Une anti-garde regarde du code, et le code est la source privée de ses commentaires. La prose
/// redevient alors libre de nommer ce qu'elle interdit — ce qui est le seul moyen d'expliquer une
/// absence sans la déclencher.
fn code_seul(source: &str) -> String {
    source
        .lines()
        .filter(|ligne| !ligne.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------------------------
// 3. Trois façons de partir, trois états qui restent
// ---------------------------------------------------------------------------------------------

/// **Les trois sorties sont acceptées et se distinguent.**
///
/// Terminer, échouer et être arrêtée sont trois façons de partir. Les fondre ferait lire une flotte
/// qu'on tue sans arrêt comme une flotte qui finit son travail.
#[test]
fn les_trois_sorties_se_distinguent() {
    for etat in [
        InstanceState::Completed,
        InstanceState::Failed,
        InstanceState::Terminated,
    ] {
        let sejour = Span::left(instant(0), instant(10), etat).expect("une sortie licite");
        assert_eq!(sejour.lifetime().ended_as(), Some(etat), "{etat}");
        assert_eq!(sejour.lifetime().millis(), Some(10), "{etat}");
    }
}

/// **Les trois états non terminaux sont refusés comme sortie.**
///
/// `provisioned`, `active` et `waiting` décrivent une instance **encore là**. Les accepter clôrait
/// un séjour qui continue, et produirait une durée pour quelque chose qui n'est pas fini — ce que
/// tout cet item existe pour empêcher.
#[test]
fn un_etat_non_terminal_n_est_pas_une_sortie() {
    for etat in [
        InstanceState::Provisioned,
        InstanceState::Active,
        InstanceState::Waiting,
    ] {
        let refus = Span::left(instant(0), instant(10), etat).expect_err("pas une sortie");
        assert_eq!(refus, LifetimeError::NotAnExit { state: etat }, "{etat}");
    }
}

/// **Une sortie antérieure à l'entrée est refusée.**
///
/// L'accepter rendrait une durée négative — ou, pire, une durée absolue qui aurait l'air juste.
#[test]
fn une_sortie_avant_l_entree_est_refusee() {
    let refus = Span::left(instant(500), instant(200), InstanceState::Completed)
        .expect_err("ordre impossible");

    assert_eq!(
        refus,
        LifetimeError::LeftBeforeEntering {
            entered: instant(500),
            left: instant(200)
        }
    );
}

// ---------------------------------------------------------------------------------------------
// 4. Rien de ce que l'instance a accompli
// ---------------------------------------------------------------------------------------------

/// **Aucun chemin vers un résultat, et rien ne juge.**
///
/// Une instance qui a tenu longtemps peut n'avoir rien produit, et une instance courte peut avoir
/// tout fait ; lire l'une pour l'autre est la faute que le mot « lifetime » invite naturellement.
#[test]
fn la_source_ne_mene_a_aucun_resultat() {
    let code = code_seul(include_str!("../src/lifetime.rs"));

    for interdit in [
        "produced",
        "output",
        "fn result",
        "Artifact",
        "fn tasks",
        "const MIN",
        "const MAX",
        "fn is_healthy",
        "fn score",
        "enum Verdict",
    ] {
        assert!(
            !code.contains(interdit),
            "« {interdit} » ferait lire une durée comme une productivité"
        );
    }
}
