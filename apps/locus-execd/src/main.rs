//! Le point d'entrée du broker.
//!
//! Il ne démarre rien tant qu'aucun driver n'existe : W4.c livre la frontière — le port, la
//! décision d'admission, la garde qui vérifie que personne d'autre ne parle à un runtime — et
//! W4.d le premier driver. Un binaire qui prétendrait servir alors qu'il n'a rien à quoi parler
//! serait exactement le « sandbox factice » que l'ADR 0004 interdit dans son plan de rollback.

use std::process::ExitCode;

fn main() -> ExitCode {
    eprintln!(
        "locus-execd : aucun driver de runtime n'est encore branché (W4.d).\n\
         Ce binaire tient la frontière — port, admission, garde de socket — et refuse de \
         prétendre servir."
    );
    ExitCode::FAILURE
}
