//! Le binaire `locusd` — `W20.d`.
//!
//! Il assemble, rend compte, et s'arrête. Il n'écoute rien : l'ADR 0018 autorise `axum` sans
//! l'introduire, et `W20.f` livrera le fil. Une boucle d'attente sans transport serait un serveur
//! qui n'écoute rien, ce qui se distingue mal d'un serveur en panne — alors qu'un processus qui se
//! termine en disant ce qu'il a câblé ne laisse aucun doute sur son état.
//!
//! Le code de sortie porte le verdict : `0` si les quatre projections sont saines, `1` si l'une est
//! en quarantaine. Un daemon qui rendrait `0` avec une projection morte annoncerait une disponibilité
//! qu'il n'a pas.

use std::process::ExitCode;

use locusd::composition::Runtime;

fn main() -> ExitCode {
    let mut runtime = Runtime::in_memory();
    let readiness = runtime.catch_up();

    println!("{readiness}");

    if readiness.is_ready() {
        ExitCode::SUCCESS
    } else {
        eprintln!(
            "locusd : projection(s) en quarantaine — {}",
            readiness.quarantined().join(", ")
        );
        ExitCode::FAILURE
    }
}
