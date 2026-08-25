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

//! # Le mode d'écoute
//!
//! Sans argument, le binaire rend compte et sort — c'est ce que `W22.c` a livré. Avec
//! `--listen <chemin>`, il ouvre le tube de l'ADR 0028 et sert `locusd`, une connexion à la fois.
//! Toute la logique est dans [`locus_execd::link`] ; ce fichier reste une coquille, pour la même
//! raison qu'avant : ce qu'aucun test ne traverse vieillit sans que rien ne le dise.

use std::process::ExitCode;

use locus_execd::announced::NothingProven;
use locus_execd::link::serve;
use locus_execd::linux::{HostFacts, SystemRunner};
use locus_execd::readiness::Readiness;

/// L'option qui fait écouter le broker.
const LISTEN: &str = "--listen";

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

    let Some(path) = listen_path(std::env::args().skip(1)) else {
        return if readiness.is_provable() {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        };
    };

    // Un hôte insuffisant **écoute quand même**. Le refus est alors une réponse, pas un silence :
    // `locusd` doit pouvoir apprendre ce qui manque, et un broker qui se tairait pour cause d'hôte
    // incomplet se confondrait avec un broker éteint — les deux choses que l'ADR 0028 décision 4
    // sépare.
    let listener = match locus_broker::unix::listen(std::path::Path::new(&path)) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("locus-execd : {error}");
            return ExitCode::FAILURE;
        }
    };
    println!("locus-execd : à l'écoute sur {path}");

    // `W5.t` : les attestations conservées, si l'exploitant en a posé.
    //
    // Le commentaire qui vivait ici disait que « aucune campagne n'est conservée par ce binaire […]
    // c'est ce qui rend visible, au premier placement réel, qu'il manque la campagne ». Le premier
    // placement réel a eu lieu — par le harnais de `W12.f` — et il a rendu exactement cela. La
    // phrase a fait son travail ; ce qui la remplace est la source qu'elle appelait.
    //
    // Le défaut ne change pas : sans la variable, `NothingProven`, donc rien au-dessus de `S0`,
    // donc `level_not_attested`. Un fichier **nommé et illisible** refuse le démarrage, comme les
    // amorçages de `locusd` : un exploitant qui l'a posé veut que ses attestations comptent.
    let recorded = match locus_execd::attestation::load(|name| std::env::var(name).ok(), &facts) {
        Ok(recorded) => recorded,
        Err(refus) => {
            eprintln!("locus-execd : {refus}");
            return ExitCode::FAILURE;
        }
    };
    let proven: &dyn locus_execd::announced::Proven = if let Some(recorded) = recorded.as_ref() {
        println!("  {}", locus_execd::attestation::annonce(recorded));
        recorded
    } else {
        println!("  attestations : aucune — rien ne sera placé au-dessus de S0");
        &NothingProven
    };
    serve(&listener, &facts, proven, |trouble| {
        eprintln!("locus-execd : {trouble}");
    });
    ExitCode::SUCCESS
}

/// Lire `--listen <chemin>` dans les arguments.
///
/// Rendu comme une fonction plutôt qu'analysé en ligne pour qu'un test l'exerce : `main` n'est
/// traversé par aucun test, et c'est précisément ce qui avait laissé ce binaire mentir pendant des
/// mois sur ce que son crate exporte.
fn listen_path(arguments: impl Iterator<Item = String>) -> Option<String> {
    let mut arguments = arguments.skip_while(|argument| argument != LISTEN);
    arguments.next()?;
    arguments.next()
}

#[cfg(test)]
mod tests {
    use super::{LISTEN, listen_path};

    fn arguments(values: &[&str]) -> impl Iterator<Item = String> {
        values
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn sans_option_le_binaire_rend_compte_et_sort() {
        assert_eq!(listen_path(arguments(&[])), None);
        assert_eq!(listen_path(arguments(&["--autre", "chose"])), None);
    }

    #[test]
    fn l_option_rend_le_chemin_qui_la_suit() {
        assert_eq!(
            listen_path(arguments(&[LISTEN, "/run/locus/broker.sock"])),
            Some("/run/locus/broker.sock".to_owned())
        );
    }

    /// **Une option sans valeur n'écoute pas sur un chemin vide.**
    ///
    /// Sans ce cas, `--listen` seul aurait produit `Some("")` ou pire, et le broker aurait tenté de
    /// s'ouvrir sur un chemin que personne n'a écrit.
    #[test]
    fn l_option_sans_valeur_n_ecoute_pas() {
        assert_eq!(listen_path(arguments(&[LISTEN])), None);
    }
}
