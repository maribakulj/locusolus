//! Le binaire `locusd` — `W20.d` l'a assemblé, `W20.g` lui donne une surface.
//!
//! # Il ne sert pas s'il n'est pas prêt
//!
//! L'ordre est : assembler, rattraper, **rendre compte**, et servir seulement alors. Un daemon qui
//! ouvrirait son port avec une projection en quarantaine servirait des lectures périmées à des
//! clients qui n'ont aucun moyen de le savoir — c'est pire que de refuser, parce qu'un refus se
//! voit. Le code de sortie `1` et l'absence d'écoute disent la même chose, et un superviseur peut
//! agir sur l'un comme sur l'autre.
//!
//! # La surface écrit, depuis `W20.k` — et ce commentaire disait le contraire
//!
//! Il annonçait « aucune commande de §22.3 : `Transaction::submit` prend `&mut self` ». `W20.h` l'a
//! levé, et cette phrase a survécu six sprints à la condition qu'elle décrivait. C'est le
//! **troisième** fichier où elle traînait, après `http.rs` et `branch.rs`, corrigés en `W20.k` : une
//! affirmation fausse ne se propage pas par malveillance mais par copie, et elle ne s'efface que là
//! où quelqu'un la relit.
//!
//! `W20.k` sert les trois chemins de §15.2, qui écrivent, par la transaction.
//!
//! # Quel journal, et pourquoi ce n'est pas toujours celui qu'on veut
//!
//! `W20.m` : le backend vient du **profil de déploiement**. Un profil qui promet la durabilité à ses
//! clients ne démarre pas sur un journal volatile — voir [`locusd::journal`]. Le choix a lieu ici,
//! dans le binaire, et `composition.rs` ne nomme aucun backend concret : c'est la seule chose que le
//! paramètre de type de `Runtime<S>` était là pour garantir, et un test l'éprouve enfin.

use std::process::ExitCode;
use std::sync::Arc;

use locus_broker::unix::UnixSocketBroker;
use locus_event_store::{EventStore, PostgresEventStore};
use locusd::broker::Standing;
use locusd::composition::Runtime;
use locusd::http::{DEFAULT_BIND, router, served};
use locusd::journal::Choice;

/// Où le broker est attendu quand rien ne le dit.
///
/// Un chemin par défaut plutôt qu'une configuration obligatoire : le profil `personal-local` de
/// §27.1 met les deux binaires sur la même machine, et exiger un réglage pour le cas le plus courant
/// ferait échouer la première mise en service sur une variable d'environnement oubliée.
const DEFAULT_BROKER_SOCKET: &str = "/tmp/locus/broker.sock";

/// Le profil supposé quand rien ne le dit — celui qui promet le moins.
///
/// `personal-local` et non le plus capable : un défaut qui promettrait la durabilité ferait démarrer
/// un daemon volatile sous un profil qui jure le contraire, et c'est exactement ce que `W20.m`
/// existe pour empêcher. Le défaut le plus prudent est celui qui n'engage rien.
const DEFAULT_PROFILE: &str = "personal-local";

fn main() -> ExitCode {
    let profile = std::env::var("LOCUSD_PROFILE").unwrap_or_else(|_| DEFAULT_PROFILE.to_owned());
    let choice = match Choice::decide(&profile, std::env::var("LOCUSD_JOURNAL").ok()) {
        Ok(choice) => choice,
        Err(refusal) => {
            eprintln!("locusd : {refusal}");
            return ExitCode::FAILURE;
        }
    };
    println!("locusd : profil {profile} — {}", choice.describe());

    match choice {
        Choice::Volatile => demarrer(Runtime::in_memory()),
        Choice::Durable(url) => match PostgresEventStore::connect(&url) {
            // L'adresse n'est **pas** citée dans le refus : une chaîne de connexion porte un mot de
            // passe, et un message d'erreur le mettrait dans tous les journaux de supervision.
            Err(error) => {
                eprintln!("locusd : journal durable indisponible — {error}");
                ExitCode::FAILURE
            }
            Ok(store) => demarrer(Runtime::assemble(store, locus_policy::Policy::new())),
        },
    }
}

/// Démarrer sur le journal choisi.
///
/// Générique sur `S`, et c'est ce qui permet au choix de vivre dans le binaire : `composition.rs`
/// ne nomme aucun backend, `http.rs` non plus, et substituer un driver ne touche ni l'un ni l'autre.
fn demarrer<S: EventStore + Send + Sync + 'static>(runtime: Runtime<S>) -> ExitCode {
    let readiness = runtime.catch_up();
    println!("{readiness}");

    if !readiness.is_ready() {
        eprintln!(
            "locusd : projection(s) en quarantaine — {}. Le port n'est pas ouvert : servir des lectures périmées serait pire que refuser.",
            readiness.quarantined().join(", ")
        );
        return ExitCode::FAILURE;
    }

    // `W4.h` : l'état du lien vers `locus-execd` se dit **au démarrage**, et le daemon démarre quand
    // même. Le déclarer seulement au moment où quelqu'un bute dessus produirait un `locusd` qui a
    // l'air d'aller bien et qui échoue à la première mission réelle — ADR 0028 décision 4.
    let broker: Arc<dyn locus_broker::port::BrokerPort + Send + Sync> =
        Arc::new(UnixSocketBroker::at(
            std::env::var("LOCUSD_BROKER_SOCKET")
                .unwrap_or_else(|_| DEFAULT_BROKER_SOCKET.to_owned()),
        ));
    let standing = Standing::probe(broker.as_ref());
    println!("locusd : {standing}");
    if let Some(refus) = standing.refusal() {
        eprintln!("locusd : {refus}");
    }

    // `W20.q` : le **même** lien sert la réclamation. Deux clients vers deux chemins différents
    // auraient permis à un daemon d'annoncer un broker au démarrage et d'en interroger un autre à
    // la première mission, sans que rien ne le dise.
    let desk = runtime.lep().clone();
    let runtime = runtime.with_lep(desk.placing(broker));

    let adresse = std::env::var("LOCUSD_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_owned());
    println!("  écoute : http://{adresse} — {}", served().join(", "));

    let runtime_async = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(built) => built,
        Err(error) => {
            eprintln!("locusd : runtime asynchrone indisponible : {error}");
            return ExitCode::FAILURE;
        }
    };

    // `W20.p` : avec un journal durable, ces handlers appellent un driver **bloquant** depuis un fil
    // du runtime asynchrone. C'est une propriété de latence sous charge, pas une faute de
    // correction, et l'ADR 0030 décision 1 nomme déjà `spawn_blocking` comme réponse. L'écrire ici
    // demanderait de changer la convention d'appel de toute la couche HTTP : c'est un item, pas un
    // coin de celui-ci.
    runtime_async.block_on(async move {
        let listener = match tokio::net::TcpListener::bind(&adresse).await {
            Ok(listener) => listener,
            Err(error) => {
                eprintln!("locusd : impossible d'écouter sur {adresse} : {error}");
                return ExitCode::FAILURE;
            }
        };
        match axum::serve(listener, router(Arc::new(runtime))).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("locusd : le service s'est arrêté : {error}");
                ExitCode::FAILURE
            }
        }
    })
}
