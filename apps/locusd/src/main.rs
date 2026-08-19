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
//! # La surface est en lecture seule
//!
//! `W20.g` sert §22.4 et §22.1. Aucune commande de §22.3 : `Transaction::submit` prend `&mut self`,
//! et la couche HTTP ne tient qu'un `&Runtime`. Sérialiser les écritures — verrou, file, acteur —
//! est une décision qui mérite son item.

use std::process::ExitCode;
use std::sync::Arc;

use locusd::composition::Runtime;
use locusd::http::{DEFAULT_BIND, router, served};

fn main() -> ExitCode {
    let mut runtime = Runtime::in_memory();
    let readiness = runtime.catch_up();
    println!("{readiness}");

    if !readiness.is_ready() {
        eprintln!(
            "locusd : projection(s) en quarantaine — {}. Le port n'est pas ouvert : servir des lectures périmées serait pire que refuser.",
            readiness.quarantined().join(", ")
        );
        return ExitCode::FAILURE;
    }

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
