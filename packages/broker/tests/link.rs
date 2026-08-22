//! Le lien, exercé sur une vraie socket — `W4.h`, ADR 0028.
//!
//! Ces tests ouvrent une socket réelle plutôt que de simuler un transport. Un test de lien qui
//! n'ouvrirait rien vérifierait la traduction et pas la liaison, et c'est exactement la classe de
//! test que `W22.f` a trouvée verte et vide.

use std::io::{BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::thread;

use locus_broker::frame::{FrameError, read_frame, write_frame};
use locus_broker::port::{BrokerError, BrokerPort, Loopback};
use locus_broker::protocol::{Ask, Missing, PROTOCOL, Request, Response, Verdict};
use locus_broker::unix::{DIRECTORY_MODE, SOCKET_MODE, UnixSocketBroker, answer, listen};
use locus_lep::SandboxLevel;

/// Un répertoire de travail qui se nettoie tout seul.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("locus-broker-{name}-{}", std::process::id()));
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

/// Un broker qui répond une fois, puis s'arrête.
///
/// Une connexion à la fois est ce que l'ADR 0028 décision 6 décrit, et le test l'exerce tel quel
/// plutôt que d'inventer un serveur que le dépôt n'a pas.
fn serve_once(path: &Path, verdict: Verdict) -> thread::JoinHandle<()> {
    let listener = listen(path).expect("écoute");
    thread::spawn(move || {
        let (stream, _) = listener.accept().expect("connexion");
        answer(&stream, |_| verdict).expect("réponse");
    })
}

#[test]
fn l_aller_retour_traverse_une_vraie_socket() {
    let scratch = Scratch::new("aller-retour");
    let path = scratch.socket();
    let server = serve_once(
        &path,
        Verdict::Provable {
            ceiling: SandboxLevel::S3,
        },
    );

    let broker = UnixSocketBroker::at(&path);
    let verdict = broker.readiness().expect("un verdict");

    assert_eq!(
        verdict,
        Verdict::Provable {
            ceiling: SandboxLevel::S3
        }
    );
    server.join().expect("le serveur se termine");
}

/// **Un hôte insuffisant rend tous ses manques, pas le premier.**
///
/// C'est la règle de `admit` — « toutes les raisons, pas la première » — et elle doit survivre au
/// fil. Un lien qui n'en transmettrait qu'un ferait corriger une condition, relancer, découvrir la
/// suivante, autant d'allers-retours qu'il manque de conditions.
#[test]
fn les_manques_traversent_tous_et_dans_l_ordre() {
    let scratch = Scratch::new("manques");
    let path = scratch.socket();
    let attendus = vec![
        Missing::Unavailable {
            what: "cgroup v2".to_owned(),
            reason: "monté en v1".to_owned(),
        },
        Missing::Undetermined {
            what: "quota de projet".to_owned(),
            reason: "aucune racine de stockage déclarée".to_owned(),
        },
    ];
    let server = serve_once(
        &path,
        Verdict::HostShort {
            ceiling: SandboxLevel::S1,
            missing: attendus.clone(),
        },
    );

    let verdict = UnixSocketBroker::at(&path).readiness().expect("un verdict");

    let Verdict::HostShort { ceiling, missing } = verdict else {
        panic!("le verdict devait être HostShort");
    };
    assert_eq!(ceiling, SandboxLevel::S1);
    assert_eq!(missing, attendus, "l'ordre et le nombre sont conservés");
    server.join().expect("le serveur se termine");
}

/// **« Injoignable » et « refusé » ne se confondent pas** — ADR 0028 décision 4.
///
/// Les deux envoient chercher à des endroits opposés : démarrer un service, ou corriger une
/// identité. Le test tient la distinction par le **type** et non par la lecture d'une phrase, parce
/// qu'une phrase se reformule et qu'un type non.
#[test]
fn un_broker_eteint_n_est_pas_un_broker_qui_refuse() {
    let scratch = Scratch::new("injoignable");
    let path = scratch.socket();

    let echec = UnixSocketBroker::at(&path)
        .readiness()
        .expect_err("rien n'écoute");

    let BrokerError::Unreachable { endpoint, .. } = &echec else {
        panic!("un broker éteint est injoignable, pas autre chose : {echec:?}");
    };
    assert_eq!(endpoint, &path.display().to_string());

    // Et le refus, lui, est un verdict : le broker a parlé.
    let refus = serve_once(
        &path,
        Verdict::Refused {
            why: "appelant non admis".to_owned(),
        },
    );
    let verdict = UnixSocketBroker::at(&path).readiness().expect("il a parlé");
    assert!(matches!(verdict, Verdict::Refused { .. }));
    refus.join().expect("le serveur se termine");
}

/// **Un désaccord de protocole se dit, il ne s'interprète pas.**
#[test]
fn un_appelant_d_une_autre_version_est_refuse_en_le_disant() {
    let scratch = Scratch::new("version");
    let path = scratch.socket();
    let listener = listen(&path).expect("écoute");
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("connexion");
        answer(&stream, |_| {
            panic!("le répondeur ne doit pas être atteint par une autre version")
        })
        .expect("réponse");
    });

    let stream = UnixStream::connect(&path).expect("connexion");
    let mut writer = &stream;
    write_frame(
        &mut writer,
        &Request {
            protocol: "broker/9.9".to_owned(),
            ask: Ask::Readiness,
        },
    )
    .expect("requête écrite");
    let mut reader = BufReader::new(&stream);
    let response: Response = read_frame(&mut reader).expect("réponse lue");

    assert_eq!(response.protocol, PROTOCOL);
    let Verdict::Refused { why } = response.verdict else {
        panic!("une autre version doit être refusée");
    };
    assert!(
        why.contains("broker/9.9") && why.contains(PROTOCOL),
        "le refus nomme les deux versions, sinon il n'aide personne : {why}"
    );
    server.join().expect("le serveur se termine");
}

/// **La socket et son répertoire portent les permissions de l'ADR 0028 décision 2.**
///
/// Vérifié sur le système de fichiers, pas sur la constante : une constante qui ne serait pas
/// appliquée serait exactement l'affirmation non vérifiée que l'ADR 0025 rend coûteuse.
#[test]
fn la_socket_et_son_repertoire_sont_fermes() {
    let scratch = Scratch::new("permissions");
    let path = scratch.socket();
    let _listener = listen(&path).expect("écoute");

    let socket = std::fs::metadata(&path).expect("la socket existe");
    let directory = std::fs::metadata(path.parent().expect("un parent")).expect("le répertoire");

    // **Des littéraux, pas les constantes.** La première rédaction comparait le mode lu à
    // `SOCKET_MODE`, ce qui rendait le test vrai pour n'importe quelle valeur de la constante :
    // une passe de mutants a ouvert la socket à tous et le test est resté vert. Un test qui
    // s'appuie sur ce qu'il vérifie ne vérifie rien.
    assert_eq!(
        socket.permissions().mode() & 0o777,
        0o600,
        "la socket doit être fermée à tout autre utilisateur"
    );
    assert_eq!(
        directory.permissions().mode() & 0o777,
        0o700,
        "le répertoire ne doit pas être énumérable"
    );
    // Et les constantes disent bien cela, pour qu'un appelant qui les lit ne soit pas trompé.
    assert_eq!(SOCKET_MODE, 0o600);
    assert_eq!(DIRECTORY_MODE, 0o700);
}

/// **Une ligne terminée mais trop longue est refusée aussi.**
///
/// Le test voisin envoie une ligne **sans fin** : il exerce la branche qui accumule. Celle-ci
/// franchit la borne sur une ligne qui **se termine**, et une passe de mutants a montré que rien ne
/// l'exerçait — la borne pouvait cesser de compter ce qui précédait le dernier morceau sans qu'un
/// test bronche.
#[test]
fn une_ligne_terminee_mais_trop_longue_est_refusee() {
    let scratch = Scratch::new("borne-terminee");
    let path = scratch.socket();
    let listener = listen(&path).expect("écoute");
    let lecteur = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("connexion");
        let mut reader = BufReader::new(&stream);
        read_frame::<Request, _>(&mut reader)
    });

    let mut stream = UnixStream::connect(&path).expect("connexion");
    let bloc = vec![b'x'; 64 * 1024];
    for _ in 0..6 {
        if stream.write_all(&bloc).is_err() {
            break;
        }
    }
    let _ = stream.write_all(b"\n");
    let _ = stream.flush();

    let echec = lecteur
        .join()
        .expect("le lecteur se termine")
        .expect_err("la borne doit être franchie");
    assert!(
        matches!(echec, FrameError::TooLong { .. }),
        "une ligne terminée au-delà de la borne se refuse pour sa longueur : {echec:?}"
    );
}

/// **Un flux fermé sans rien dire n'est pas un cadre illisible.**
///
/// Les deux se soignent différemment : l'un est un fait de lien, l'autre un désaccord de
/// vocabulaire. Une passe de mutants a montré que rien ne les séparait — on pouvait lire la
/// fermeture comme une ligne vide, donc comme du JSON invalide.
#[test]
fn un_flux_ferme_sans_rien_dire_est_une_fermeture_pas_un_cadre_illisible() {
    let scratch = Scratch::new("ferme");
    let path = scratch.socket();
    let listener = listen(&path).expect("écoute");
    let lecteur = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("connexion");
        let mut reader = BufReader::new(&stream);
        read_frame::<Request, _>(&mut reader)
    });

    drop(UnixStream::connect(&path).expect("connexion"));

    let echec = lecteur
        .join()
        .expect("le lecteur se termine")
        .expect_err("rien n'a été dit");
    assert!(
        matches!(echec, FrameError::Closed),
        "une fermeture est une fermeture, pas un cadre illisible : {echec:?}"
    );
}

/// **Un broker qui meurt en cours d'échange est injoignable, pas illisible.**
///
/// C'est la même distinction, vue du client. Le classer illisible enverrait relire un protocole
/// quand il faut relire un journal de service.
#[test]
fn un_broker_qui_meurt_en_cours_d_echange_est_injoignable() {
    let scratch = Scratch::new("mort-subite");
    let path = scratch.socket();
    let listener = listen(&path).expect("écoute");
    let server = thread::spawn(move || {
        // Accepter, lire la requête, puis fermer sans répondre.
        let (stream, _) = listener.accept().expect("connexion");
        let mut reader = BufReader::new(&stream);
        let _: Result<Request, _> = read_frame(&mut reader);
        drop(stream);
    });

    let echec = UnixSocketBroker::at(&path)
        .readiness()
        .expect_err("le broker est mort au milieu");

    assert!(
        matches!(echec, BrokerError::Unreachable { .. }),
        "une mort en cours d'échange est un fait de lien : {echec:?}"
    );
    server.join().expect("le serveur se termine");
}

/// **Le client refuse une réponse d'une autre version.**
///
/// Le pendant du refus côté serveur, et une passe de mutants a montré que seul le serveur était
/// éprouvé : le client pouvait accepter n'importe quelle version sans qu'un test bronche.
#[test]
fn le_client_refuse_une_reponse_d_une_autre_version() {
    let scratch = Scratch::new("version-reponse");
    let path = scratch.socket();
    let listener = listen(&path).expect("écoute");
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("connexion");
        let mut reader = BufReader::new(&stream);
        let _: Request = read_frame(&mut reader).expect("requête lue");
        let mut writer = &stream;
        write_frame(
            &mut writer,
            &Response {
                protocol: "broker/9.9".to_owned(),
                verdict: Verdict::Provable {
                    ceiling: SandboxLevel::S3,
                },
            },
        )
        .expect("réponse écrite");
    });

    let echec = UnixSocketBroker::at(&path)
        .readiness()
        .expect_err("la version ne correspond pas");

    let BrokerError::Malformed { why } = &echec else {
        panic!("un désaccord de version est un désaccord de vocabulaire : {echec:?}");
    };
    assert!(
        why.contains("broker/9.9") && why.contains(PROTOCOL),
        "le refus nomme les deux versions : {why}"
    );
    server.join().expect("le serveur se termine");
}

/// **Une socket résiduelle se remplace ; un fichier ordinaire ne se détruit pas.**
///
/// Le chemin de la socket vient d'une configuration qu'un humain a écrite. Effacer ce qui s'y
/// trouve parce qu'on voulait le nom serait une destruction de donnée décidée par un daemon.
#[test]
fn une_socket_morte_se_remplace_mais_un_fichier_ordinaire_survit() {
    let scratch = Scratch::new("residuel");
    let path = scratch.socket();

    let premier = listen(&path).expect("première écoute");
    drop(premier);
    let second = listen(&path).expect("la socket résiduelle est remplacée");
    drop(second);

    let occupe = scratch.0.join("occupe.sock");
    std::fs::write(
        &occupe,
        "une donnée que personne n'a le droit d'effacer".as_bytes(),
    )
    .expect("écriture");
    let refus = listen(&occupe);
    assert!(refus.is_err(), "un fichier ordinaire ne se remplace pas");
    assert_eq!(
        std::fs::read(&occupe).expect("il est toujours là"),
        "une donnée que personne n'a le droit d'effacer".as_bytes()
    );
}

/// **La borne de taille protège le lecteur** — ADR 0028 décision 7.
///
/// Le test envoie plus que la borne **sans jamais de fin de ligne** : c'est le cas qui compte, celui
/// où un lecteur non borné accumulerait sans fin. Une ligne trop longue mais terminée serait un cas
/// plus facile, et le passer aurait laissé croire que la borne protège alors qu'elle constaterait.
#[test]
fn une_ligne_sans_fin_est_refusee_avant_d_etre_accumulee() {
    let scratch = Scratch::new("borne");
    let path = scratch.socket();
    let listener = listen(&path).expect("écoute");
    let verdict = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("connexion");
        let mut reader = BufReader::new(&stream);
        read_frame::<Request, _>(&mut reader)
    });

    let mut stream = UnixStream::connect(&path).expect("connexion");
    let bloc = vec![b'x'; 64 * 1024];
    // Assez pour franchir MAX_LINE, et jamais de `\n`. L'écriture peut échouer quand le lecteur a
    // déjà abandonné — c'est le comportement attendu, pas une erreur du test.
    for _ in 0..8 {
        if stream.write_all(&bloc).is_err() {
            break;
        }
    }
    let _ = stream.flush();

    let echec = verdict
        .join()
        .expect("le lecteur se termine")
        .expect_err("la borne doit être franchie");
    let FrameError::TooLong { read } = echec else {
        panic!("une ligne sans fin doit être refusée pour sa longueur : {echec:?}");
    };
    assert!(
        read > locus_broker::frame::MAX_LINE,
        "le refus dit combien a été lu, et c'est au-delà de la borne : {read}"
    );
}

/// **Le port se contracte de la même façon en mémoire et sur la socket.**
///
/// L'implémentation de référence existe pour que le contrat soit exerçable sans hôte — au sens où
/// `packages/event-store` en a une. Si les deux ne rendaient pas les mêmes formes, l'une des deux
/// mentirait à ses appelants.
#[test]
fn le_port_rend_les_memes_formes_en_memoire_et_sur_la_socket() {
    let attendu = Verdict::Provable {
        ceiling: SandboxLevel::S2,
    };
    let memoire = Loopback::answering(attendu.clone());
    assert_eq!(memoire.readiness().expect("un verdict"), attendu);

    let scratch = Scratch::new("contrat");
    let path = scratch.socket();
    let server = serve_once(&path, attendu.clone());
    let socket = UnixSocketBroker::at(&path);
    assert_eq!(socket.readiness().expect("un verdict"), attendu);
    assert_eq!(socket.endpoint(), path.display().to_string());
    server.join().expect("le serveur se termine");

    let eteint = Loopback::unreachable("/nulle/part", "rien n'écoute");
    let echec = eteint.readiness().expect_err("injoignable");
    assert!(matches!(echec, BrokerError::Unreachable { .. }));
    assert_eq!(eteint.endpoint(), "/nulle/part");
}

/// Un lecteur qui rend des morceaux de taille choisie.
///
/// Une socket découpe comme elle veut, et un `Cursor` rend tout d'un coup : ni l'un ni l'autre ne
/// permet d'atteindre l'état qui compte ici — un tampon **déjà rempli** au moment où le saut de
/// ligne apparaît. Le test qui suit en a besoin, et une passe de mutants a montré que sans lui la
/// branche restait inexplorée.
struct Chunked {
    data: Vec<u8>,
    at: usize,
    chunk: usize,
}

impl std::io::Read for Chunked {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let end = (self.at + self.chunk).min(self.data.len());
        let available = (end - self.at).min(out.len());
        out[..available].copy_from_slice(&self.data[self.at..self.at + available]);
        self.at += available;
        Ok(available)
    }
}

impl std::io::BufRead for Chunked {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        let end = (self.at + self.chunk).min(self.data.len());
        Ok(&self.data[self.at..end])
    }

    fn consume(&mut self, amount: usize) {
        self.at = (self.at + amount).min(self.data.len());
    }
}

/// **La borne compte ce qui précède le dernier morceau.**
///
/// La ligne fait exactement `MAX_LINE + 1` octets avant son saut de ligne. Au moment où celui-ci
/// apparaît, le tampon porte déjà `MAX_LINE` octets et le morceau courant n'en ajoute qu'un : une
/// borne qui ne regarderait que le morceau courant accepterait la ligne.
///
/// C'est le seul survivant de la passe de mutants de `W4.h`, et il ne se tuait ni sur une socket —
/// qui découpe comme elle veut — ni sur un `Cursor`, qui rend tout d'un coup et fait franchir la
/// borne au premier morceau.
#[test]
fn la_borne_compte_ce_qui_precede_le_dernier_morceau() {
    let mut data = vec![b'x'; locus_broker::frame::MAX_LINE + 1];
    data.push(b'\n');
    let mut reader = Chunked {
        data,
        at: 0,
        chunk: 8192,
    };

    let echec = read_frame::<Request, _>(&mut reader).expect_err("la borne doit être franchie");

    let FrameError::TooLong { read } = echec else {
        panic!("une ligne d'un octet de trop se refuse pour sa longueur : {echec:?}");
    };
    assert_eq!(
        read,
        locus_broker::frame::MAX_LINE + 1,
        "le refus dit la longueur totale, pas celle du dernier morceau"
    );
}
