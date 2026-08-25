//! Test de sortie de `W4.i` — **la créance de pair sur le lien du broker.**
//!
//! Trois clauses, celles de la roadmap :
//!
//! 1. les deux barrières admettent des ensembles **différents**, exercé sur une socket en `0660`
//!    avec un groupe partagé où un appelant du groupe est admis par les permissions et **refusé**
//!    par la politique ;
//! 2. le refus est un `Verdict::Refused`, **jamais une fermeture**, et un test le distingue d'un
//!    broker éteint ;
//! 3. la dépendance que la créance exige entre par `dependencies.json` avec son arbre **mesuré**.
//!
//! # Ce que `W4.h` ne pouvait pas livrer, et pourquoi
//!
//! L'ADR 0028 décision 2 annonçait deux barrières « gratuites ». Les deux moitiés étaient fausses :
//! `UnixStream::peer_cred` est instable, et la politique envisagée — « le même utilisateur que le
//! broker » — admettait **exactement** l'ensemble que `0600` admet déjà. La créance ne sépare
//! quelque chose qu'à partir du moment où la politique cesse d'être « le même » pour devenir
//! « celui-là ».
//!
//! # La clause 1 a besoin d'un second utilisateur, et ce test le dit quand il n'en a pas
//!
//! Un ensemble « membre du groupe mais pas l'uid attendu » n'est pas peuplé sur une machine qui n'a
//! qu'une identité. Le test qui l'exerce cherche donc un second uid, et **échoue bruyamment** s'il
//! n'en trouve pas plutôt que de passer en silence : une clause non exercée n'est pas une clause
//! tenue. C'est la discipline que `W5.f` et `W5.h` ont posée pour les sondes de sandbox.

use std::io::{BufReader, Write};
use std::os::fd::AsFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use locus_broker::peer::{Admission, PeerIdentity, PeerPolicy, admit};
use locus_broker::protocol::{Request, Response, Verdict};
use locus_broker::unix::{
    SHARED_DIRECTORY_MODE, SHARED_SOCKET_MODE, SOCKET_MODE, answer_checked, listen, listen_shared,
};
use locus_broker::{FrameError, read_frame, write_frame};
use locus_lep::SandboxLevel;

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn scratch(nom: &str) -> PathBuf {
    let base = std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".to_owned());
    let chemin = PathBuf::from(base).join(format!("locus-w4i-{nom}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&chemin);
    std::fs::create_dir_all(&chemin).expect("le répertoire de test se crée");
    chemin
}

fn requete() -> Request {
    Request::readiness()
}

/// Ce que le répondeur rendrait **s'il était appelé**.
///
/// Il ne l'est pas : la créance est lue avant lui. Le verdict est choisi distinct de tout refus,
/// pour qu'une réponse qui le porterait se voie immédiatement.
fn verdict_temoin() -> Verdict {
    Verdict::Placed {
        worker: "wrk-temoin".to_owned(),
        level: SandboxLevel::S2,
    }
}

/// Mon propre uid, **lu par l'API sous test**.
///
/// Une paire de sockets vers moi-même rend ma propre créance. C'est plus honnête qu'un `getuid` :
/// si la lecture de créance est cassée, ce helper l'est aussi, et les tests le disent au lieu de
/// comparer une valeur juste à une valeur fausse.
fn mien() -> u32 {
    let (a, _b) = UnixStream::pair().expect("une paire de sockets");
    rustix::net::sockopt::socket_peercred(a.as_fd())
        .expect("ma propre créance se lit")
        .uid
        .as_raw()
}

/// Mon propre gid, par le même chemin.
fn mon_gid() -> u32 {
    let (a, _b) = UnixStream::pair().expect("une paire de sockets");
    rustix::net::sockopt::socket_peercred(a.as_fd())
        .expect("ma propre créance se lit")
        .gid
        .as_raw()
}

/// Un second uid présent sur cet hôte, s'il y en a un.
///
/// `LOCUS_W4I_PEER_UID` d'abord — c'est ainsi qu'un environnement de CI le fournit sans que le test
/// devine. À défaut, `/etc/passwd` est lu : un compte ordinaire suffit, il n'a pas besoin d'exister
/// pour de bon puisque la clause porte sur les **ensembles**, pas sur une connexion réelle depuis ce
/// compte.
fn second_uid() -> Option<u32> {
    if let Ok(declare) = std::env::var("LOCUS_W4I_PEER_UID")
        && let Ok(uid) = declare.trim().parse::<u32>()
        && uid != mien()
    {
        return Some(uid);
    }
    let passwd = std::fs::read_to_string("/etc/passwd").ok()?;
    passwd
        .lines()
        .filter_map(|ligne| ligne.split(':').nth(2)?.parse::<u32>().ok())
        .find(|uid| *uid != mien())
}

// ---------------------------------------------------------------------------------------------
// 1. Les deux barrières admettent des ensembles différents
// ---------------------------------------------------------------------------------------------

/// **En `0600`, les deux ensembles coïncident** — et c'est la raison pour laquelle `W4.h` avait
/// raison de ne rien livrer.
///
/// Le test le montre plutôt que de le raconter : la socket par défaut admet le propriétaire, et une
/// politique « mon propre uid » admet exactement le même. L'écart est vide, la seconde barrière ne
/// sépare rien.
#[test]
fn en_0600_les_deux_barrieres_admettent_le_meme_ensemble() {
    let racine = scratch("meme");
    let chemin = racine.join("broker.sock");
    let ecoute = listen(&chemin).expect("la socket s'ouvre");

    let mode = std::fs::metadata(&chemin)
        .expect("la socket existe")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, SOCKET_MODE, "le défaut reste 0600");

    // Seul le propriétaire passe les permissions ; et la politique « moi » admet le propriétaire.
    // Les deux lignes du tableau sont identiques, donc leur écart est vide.
    let client = UnixStream::connect(&chemin).expect("le propriétaire se connecte");
    let (accepte, _) = ecoute.accept().expect("la connexion arrive");
    let admission = admit(&accepte, PeerPolicy::only(mien()));
    assert!(admission.is_admitted());
    drop(client);
}

/// **En `0660` avec un groupe partagé, ils diffèrent** — et c'est l'item.
///
/// La clause exige un appelant **admis par les permissions et refusé par la politique**. Un tel
/// appelant est un membre du groupe dont l'uid n'est pas celui qu'on attend, ce qui demande une
/// seconde identité sur la machine.
///
/// Le test la cherche et **échoue en le disant** s'il n'en trouve pas : une clause qu'on saute en
/// silence n'est pas une clause tenue, et un test vert qui n'a rien exercé est pire qu'un test
/// absent — c'est la règle 3 du rythme de session, « un compteur qui n'a rien lu ne vaut pas zéro ».
///
/// Fournir le second uid : `LOCUS_W4I_PEER_UID`, ou n'importe quel compte du système autre que
/// celui qui exécute les tests.
#[test]
fn en_0660_les_deux_barrieres_admettent_des_ensembles_differents() {
    let autre = second_uid().unwrap_or_else(|| {
        panic!(
            "NON MESURÉ : aucun second uid sur cet hôte, donc l'ensemble « membre du groupe mais \
             pas l'uid attendu » est vide et la clause 1 n'a rien à exercer. Poser \
             LOCUS_W4I_PEER_UID, ou créer un compte. Ce test échoue plutôt que de passer : une \
             clause non exercée n'est pas une clause tenue."
        )
    });
    assert_ne!(autre, mien(), "le second uid est bien un autre");

    let racine = scratch("partage");
    let chemin = racine.join("broker.sock");
    let politique = PeerPolicy::only(mien());
    let ecoute = listen_shared(&chemin, politique).expect("la socket partagée s'ouvre");
    assert_eq!(
        ecoute.policy(),
        politique,
        "la politique est **retenue** par l'écoute, pas seulement prise en paramètre"
    );

    let mode = std::fs::metadata(&chemin)
        .expect("la socket existe")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, SHARED_SOCKET_MODE, "0660, et pas 0600");
    let mode_repertoire = std::fs::metadata(&racine)
        .expect("le répertoire existe")
        .permissions()
        .mode()
        & 0o777;
    let _ = mode_repertoire;

    // **La ligne du tableau qui compte.** Un appelant du groupe, d'uid `autre`, est admis par les
    // permissions — c'est ce que `0660` veut dire — et refusé par la politique, qui nomme un uid.
    let membre_du_groupe = PeerIdentity {
        uid: autre,
        gid: mon_gid(),
        pid: 4242,
    };
    assert!(
        !politique.admits(membre_du_groupe),
        "admis par les permissions, refusé par la politique : c'est l'écart que l'item existe pour \
         produire"
    );

    // Et la contre-épreuve, sans laquelle un refus universel satisferait la ligne du dessus :
    // l'uid attendu, lui, obtient le verdict du répondeur et non un refus de créance.
    let fil = std::thread::spawn(move || {
        let mut appele = false;
        let resultat = ecoute.serve_once(|_| {
            appele = true;
            verdict_temoin()
        });
        (resultat.is_ok(), appele)
    });
    let mut client = UnixStream::connect(&chemin).expect("l'uid attendu se connecte");
    write_frame(&mut client, &requete()).expect("la requête part");
    client.flush().expect("le tube se vide");
    let mut lecteur = BufReader::new(&client);
    let reponse: Response = read_frame(&mut lecteur).expect("une réponse arrive");
    assert_eq!(reponse.verdict, verdict_temoin(), "l'uid attendu passe");
    let (ok, appele) = fil.join().expect("le fil se termine");
    assert!(ok);
    assert!(appele, "le répondeur est atteint quand la créance convient");

    // L'écart entre les deux ensembles n'est donc pas vide, ce qui était faux en `0600`.
    assert_ne!(SHARED_SOCKET_MODE, SOCKET_MODE);
    assert_eq!(SHARED_DIRECTORY_MODE & 0o050, 0o050, "le groupe traverse");
}

/// **L'écoute partagée applique sa politique** — pas seulement la retient.
///
/// Trouvé par un mutant survivant : `serve_once` appelant `answer` au lieu d'`answer_checked` ne
/// faisait rougir aucun test, parce que le seul appelant exercé était l'uid **admis** — pour qui les
/// deux chemins rendent la même chose.
///
/// C'était le trou exact que `SharedListener` venait de fermer côté type : la politique ne peut plus
/// se perdre en route, mais rien ne vérifiait qu'elle servait. Retenir et appliquer sont deux actes,
/// et seul le second se voit sur un appelant refusé.
#[test]
fn l_ecoute_partagee_applique_sa_politique_et_pas_seulement_la_retient() {
    let racine = scratch("applique");
    let chemin = racine.join("broker.sock");
    let impossible = PeerPolicy::only(mien().wrapping_add(1));
    let ecoute = listen_shared(&chemin, impossible).expect("la socket partagée s'ouvre");

    let fil = std::thread::spawn(move || {
        let mut appele = false;
        let resultat = ecoute.serve_once(|_| {
            appele = true;
            verdict_temoin()
        });
        (resultat.is_ok(), appele)
    });

    let mut client = UnixStream::connect(&chemin).expect("la connexion s'établit");
    write_frame(&mut client, &requete()).expect("la requête part");
    client.flush().expect("le tube se vide");
    let mut lecteur = BufReader::new(&client);
    let reponse: Response = read_frame(&mut lecteur).expect("une réponse arrive");

    match reponse.verdict {
        Verdict::Refused { why } => assert!(why.contains("créance de pair"), "{why}"),
        autre => panic!("l'écoute partagée doit appliquer sa politique, et non rendre {autre:?}"),
    }
    let (ok, appele) = fil.join().expect("le fil se termine");
    assert!(ok);
    assert!(
        !appele,
        "un appelant que la politique écarte n'atteint pas le répondeur"
    );
}

/// La politique décide sur l'**uid**, jamais sur le gid.
///
/// Décider sur le gid rendrait les deux ensembles à nouveau identiques : c'est exactement ce que les
/// permissions `0660` tiennent déjà, et cet item existe pour que la seconde barrière sépare autre
/// chose que la première.
#[test]
fn la_politique_decide_sur_l_uid_et_pas_sur_le_gid() {
    let politique = PeerPolicy::only(1000);

    let bon_uid_autre_gid = PeerIdentity {
        uid: 1000,
        gid: 4242,
        pid: 1,
    };
    let autre_uid_bon_gid = PeerIdentity {
        uid: 1001,
        gid: 0,
        pid: 2,
    };

    assert!(politique.admits(bon_uid_autre_gid), "le gid ne décide pas");
    assert!(
        !politique.admits(autre_uid_bon_gid),
        "le gid ne rattrape pas un uid inattendu"
    );
}

// ---------------------------------------------------------------------------------------------
// 2. Un refus est un verdict, jamais une fermeture
// ---------------------------------------------------------------------------------------------

/// **Refusé sur le fil**, avec son motif — et le répondeur n'est jamais appelé.
///
/// La créance est lue **avant** la requête, donc un appelant écarté n'atteint pas le code qui crée
/// des conteneurs.
#[test]
fn un_appelant_refuse_recoit_un_verdict_et_pas_une_fermeture() {
    let racine = scratch("refus");
    let chemin = racine.join("broker.sock");
    let ecoute = listen(&chemin).expect("la socket s'ouvre");

    // Une politique qui n'admet personne d'existant : l'appelant réel est refusé.
    let impossible = PeerPolicy::only(mien().wrapping_add(1));

    let fil = std::thread::spawn(move || {
        let (accepte, _) = ecoute.accept().expect("la connexion arrive");
        let mut appele = false;
        let resultat = answer_checked(&accepte, impossible, |_| {
            appele = true;
            verdict_temoin()
        });
        (resultat.is_ok(), appele)
    });

    let mut client = UnixStream::connect(&chemin).expect("la connexion s'établit");
    write_frame(&mut client, &requete()).expect("la requête part");
    client.flush().expect("le tube se vide");

    let mut lecteur = BufReader::new(&client);
    let reponse: Response =
        read_frame(&mut lecteur).expect("le broker a **parlé**, il n'a pas coupé");
    match reponse.verdict {
        Verdict::Refused { why } => {
            assert!(why.contains("créance de pair"), "le motif se nomme : {why}");
            assert!(why.contains("uid"), "et il dit ce qui n'allait pas : {why}");
        }
        autre => panic!("un refus de créance est un verdict, pas {autre:?}"),
    }

    let (ok, repondeur_appele) = fil.join().expect("le fil se termine");
    assert!(ok, "un appelant refusé n'est pas une erreur de transport");
    assert!(
        !repondeur_appele,
        "la créance est lue avant la requête : un appelant écarté n'atteint pas le répondeur"
    );
}

/// **Refusé ne ressemble pas à éteint**, et c'est la clause 2.
///
/// Un broker éteint donne une erreur de connexion ; un broker qui refuse donne une réponse lisible.
/// Les confondre ferait passer la première mise en service à chercher un problème de réseau qui
/// n'existe pas — ADR 0028 décision 4.
#[test]
fn un_refus_ne_ressemble_pas_a_un_broker_eteint() {
    let racine = scratch("eteint");
    let chemin = racine.join("broker.sock");

    // Éteint : rien n'écoute.
    let eteint = UnixStream::connect(&chemin);
    assert!(
        eteint.is_err(),
        "sans écoute, la connexion elle-même échoue"
    );

    // Refusé : la connexion réussit, et une réponse arrive.
    let ecoute = listen(&chemin).expect("la socket s'ouvre");
    let impossible = PeerPolicy::only(mien().wrapping_add(1));
    let fil = std::thread::spawn(move || {
        let (accepte, _) = ecoute.accept().expect("la connexion arrive");
        answer_checked(&accepte, impossible, |_| verdict_temoin())
    });

    let mut client = UnixStream::connect(&chemin).expect("avec écoute, la connexion réussit");
    write_frame(&mut client, &requete()).expect("la requête part");
    client.flush().expect("le tube se vide");
    let mut lecteur = BufReader::new(&client);
    let reponse: Result<Response, FrameError> = read_frame(&mut lecteur);

    assert!(
        reponse.is_ok(),
        "un refus se lit ; c'est ce qui le distingue d'un broker éteint"
    );
    let _ = fil.join();
}

/// Une créance **illisible** n'est ni admise ni refusée : elle est non mesurée.
///
/// Trois issues et pas deux. Ne pas avoir pu lire la créance et l'avoir lue et refusée envoient
/// chercher des choses opposées — une socket dans un état inattendu contre une usurpation.
///
/// Ce que la troisième issue ne fait **pas** : accorder. Sur ce lien, qui commande la création de
/// conteneurs, une créance illisible ne se traite pas comme un laissez-passer.
#[test]
fn une_creance_illisible_n_est_ni_admise_ni_refusee() {
    let illisible = Admission::Unreadable {
        why: "ENOTSOCK".to_owned(),
    };
    assert!(!illisible.is_admitted(), "non mesuré n'est jamais accordé");

    let motif = illisible.why().expect("un motif existe");
    assert!(motif.contains("illisible"), "{motif}");

    // Et le motif n'est pas celui d'un refus d'identité : un exploitant qui lit l'un ne vérifie pas
    // les mêmes choses que celui qui lit l'autre.
    let refuse = Admission::Refused {
        identity: PeerIdentity {
            uid: 1001,
            gid: 1002,
            pid: 7,
        },
        expected_uid: 1000,
    };
    assert_ne!(illisible.why(), refuse.why());
    assert!(refuse.why().expect("un motif").contains("attendu 1000"));
}

// ---------------------------------------------------------------------------------------------
// 3. La dépendance est déclarée, avec son arbre mesuré
// ---------------------------------------------------------------------------------------------

/// `rustix` entre par `dependencies.json`, **avec son arbre chiffré et sa portée étroite**.
///
/// La roadmap l'exige « comme l'ADR 0018 l'a fait pour le sien » : un nombre, pas une impression. Le
/// test lit l'entrée plutôt que de faire confiance à la relecture, et vérifie que la portée n'est
/// pas `*` — une créance de pair n'a rien à faire ailleurs que sur le lien du broker.
#[test]
fn la_dependance_est_declaree_avec_son_arbre_mesure() {
    let racine = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("le crate vit sous packages/");
    let declaration: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(racine.join("dependencies.json"))
            .expect("dependencies.json est lisible"),
    )
    .expect("dependencies.json est du JSON");

    let entree = declaration["allowed"]
        .as_array()
        .expect("allowed est une liste")
        .iter()
        .find(|entree| entree["crate"] == "rustix")
        .expect("rustix est déclarée");

    assert_eq!(entree["scope"], "packages/broker", "portée étroite");
    assert_eq!(entree["adr"], "0028");

    let why = entree["why"].as_str().expect("un motif écrit");
    assert!(
        why.contains("3 paquets"),
        "l'arbre est **mesuré**, pas estimé : {why}"
    );
    assert!(
        why.contains("nix") && why.contains("uds"),
        "les concurrents sont mesurés aussi, pas supposés : {why}"
    );
    assert!(
        why.contains("libc"),
        "l'absence de lien C dans le processus privilégié est la raison du choix : {why}"
    );
}

/// L'ADR 0028 **dit maintenant la vérité** sur ce qu'une créance coûte.
///
/// Sa décision 2 annonçait « `UnixStream::peer_cred` de la bibliothèque standard » et « les deux
/// sont gratuites ». Les deux étaient faux, et `crate::unix` le consignait depuis `W4.h` — mais
/// l'ADR, lui, ne l'avait jamais dit. Une prose qui affirme ce que le code ne tient pas est un
/// défaut, même quand le code a raison.
#[test]
fn l_adr_ne_promet_plus_une_creance_gratuite() {
    let racine = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("le crate vit sous packages/");
    let adr = std::fs::read_to_string(racine.join("docs/adr/0028-le-lien-vers-le-broker.md"))
        .expect("l'ADR est lisible");

    assert!(
        !adr.contains("Les deux sont gratuites"),
        "la phrase était fausse et elle est corrigée"
    );
    assert!(
        adr.contains("instable"),
        "l'ADR dit maintenant ce que la créance coûte"
    );
    assert!(adr.contains("W4.i"), "et il nomme l'item qui l'a livrée");
}
