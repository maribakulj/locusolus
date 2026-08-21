//! Le point d'entrée du broker.
//!
//! # Il ne décide de rien
//!
//! Il construit le driver, lit l'hôte, et imprime ce que [`locus_execd::Readiness`] en dit. Toute
//! la décision est dans le module, parce que la version précédente de ce fichier décidait seule et
//! qu'aucun test ne la traversait : elle a annoncé « aucun driver de runtime n'est encore branché »
//! pendant que le crate exportait [`locus_execd::linux::SystemRunner`], la seule fonction du dépôt
//! qui exécute `podman`.
//!
//! ADR 0025 : une affirmation sur l'état du système est une promesse, et une capacité **niée** est
//! une promesse négative. La parade n'est pas de mieux rédiger le message, c'est de faire du
//! constat une valeur que des tests exercent.

use std::process::ExitCode;

use locus_execd::linux::{HostFacts, SystemRunner};
use locus_execd::readiness::Readiness;

fn main() -> ExitCode {
    // Le driver, construit sans condition. C'est la capacité que le crate exporte, et ce binaire
    // n'a plus le droit de la nier : la construire ici est ce qui rend la négation inexprimable.
    let driver = SystemRunner::new();
    println!("locus-execd : driver {}", driver.program());

    let facts = HostFacts::read_host();
    for line in facts.evidence() {
        println!("  {line}");
    }

    let readiness = Readiness::assess(&facts);
    println!("locus-execd : {readiness}");

    if readiness.is_provable() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
