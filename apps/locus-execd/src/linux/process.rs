//! Lancer un programme et lire ce qu'il a rendu — la machinerie que **les deux** mécanismes
//! partagent.
//!
//! # Pourquoi ce module existe, et pourquoi il n'est pas dans `driver.rs`
//!
//! [`Execution`], [`Runner`] et [`SystemRunner`] n'ont rien de podman : ils lancent un programme,
//! bornent son temps, drainent ses flux et rendent son code de sortie. `W5.af.3` a donné un second
//! consommateur à ces trois-là — le mécanisme `bubblewrap` —, et un type partagé qui reste chez l'un
//! des deux consommateurs se lit « bubblewrap dépend du driver podman », ce qui est faux et ce que
//! personne ne devrait avoir à démentir en relisant.
//!
//! Les chemins publics n'ont pas bougé : `linux::{Execution, Runner, SystemRunner, CALL_BUDGET}`
//! désignent les mêmes choses qu'avant, et `driver.rs` les réexporte pour ses propres lecteurs.
//!
//! # Ce qui reste hors test
//!
//! [`SystemRunner::run`], qui lance un vrai processus — et **le borne** : voir [`CALL_BUDGET`]. Tout
//! le reste se vérifie contre un double, donc en CI, où aucun runtime n'est garanti.

use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::runtime::RuntimeError;

/// Ce qu'un processus a rendu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Execution {
    /// Le code de sortie.
    pub code: i32,
    /// La sortie standard.
    pub stdout: String,
    /// La sortie d'erreur.
    pub stderr: String,
}

/// Lancer un programme, quel qu'il soit.
///
/// Le port ne nomme aucun mécanisme : `podman` le traverse depuis `W4.d`, `bwrap` depuis `W5.af.3`.
/// C'est lui qui rend la construction des arguments, l'analyse des sorties et tous les chemins
/// d'erreur vérifiables **sans** le programme réel — donc en CI, où aucun runtime n'est garanti.
///
/// `&self` et non `&mut self` : lancer un processus ne mute rien du lanceur, et
/// [`crate::runtime::RuntimePort::attestation`] prend `&self`. Un port qui exigerait `&mut`
/// forcerait l'attestation à devenir mutante, c'est-à-dire à pouvoir changer ce dont elle témoigne.
pub trait Runner: Send + Sync {
    /// Lancer avec ces arguments.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::Unavailable`] quand le binaire est introuvable ou refuse de démarrer.
    fn run(&self, arguments: &[String]) -> Result<Execution, RuntimeError>;
}

/// Combien de temps un appel à `podman` a le droit de durer.
///
/// # Ce qui manquait, et comment on l'a vu
///
/// La rédaction précédente appelait `Command::output()`, qui attend **sans limite**. C'était le seul
/// endroit du dépôt sans borne, contre sa propre règle — « timeouts et cancellation » — et il y
/// tenait parce que c'était aussi le seul chemin qu'aucun test ne traversait.
///
/// Il a été trouvé en cherchant autre chose : le job de CI de `W5.r` a paru pendre, et l'hypothèse
/// était qu'une sandbox saturée en PID bloquait son propre démontage. **L'hypothèse était fausse** —
/// le job avait fini en trois minutes et demie, et c'est l'état rapporté qui était périmé. Le défaut
/// trouvé en route, lui, est réel : un broker privilégié qui peut attendre indéfiniment ne rapporte
/// rien, et un rapport qui n'arrive pas est pire qu'un rapport qui dit « je n'ai pas su ».
///
/// `W5.r` n'a donc pas créé le risque, il l'a multiplié par seize : un appel non borné par campagne
/// devient quatre-vingts.
///
/// Soixante secondes : très au-dessus de ce que `create`, `start`, `exec`, `stop` ou `rm` demandent
/// sur un hôte sain — la campagne complète en tient cent quatre-vingts pour quatre-vingts appels —
/// donc jamais atteint quand tout va bien, et très en dessous de ce qu'une attente sans fin coûte
/// quand ça va mal.
pub const CALL_BUDGET: Duration = Duration::from_secs(60);

/// Le lanceur réel.
#[derive(Debug, Clone, Copy)]
pub struct SystemRunner {
    program: &'static str,
    budget: Duration,
}

impl Default for SystemRunner {
    fn default() -> Self {
        Self {
            program: "podman",
            budget: CALL_BUDGET,
        }
    }
}

impl SystemRunner {
    /// Un lanceur au programme et au budget par défaut.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Le programme que ce lanceur exécutera.
    ///
    /// Lisible parce que le binaire le **nomme** au démarrage : un broker qui annoncerait « driver
    /// construit » sans dire lequel laisserait un exploitant deviner, et c'est en devinant qu'on
    /// lit une capacité pour une autre. Le champ reste privé — il ne se règle que par
    /// [`Self::with_program`], et jamais depuis une entrée.
    #[must_use]
    pub const fn program(&self) -> &'static str {
        self.program
    }

    /// Le même, avec un autre plafond par appel.
    #[must_use]
    pub const fn with_budget(mut self, budget: Duration) -> Self {
        self.budget = budget;
        self
    }

    /// Le même, visant un autre programme.
    ///
    /// # Pourquoi cette porte existe, et ce qu'elle n'ouvre pas
    ///
    /// L'en-tête de ce module disait que `SystemRunner::run` reste hors test. C'était vrai et c'est
    /// précisément là que l'absence de borne a pu vivre : le seul chemin qui lance un vrai processus
    /// était le seul qu'aucun test ne traversait. Un `&'static str` suffit à le traverser — un test
    /// vise un programme qui dort, avec un budget d'un dixième de seconde, et constate que l'appel
    /// est abandonné.
    ///
    /// `&'static str` et non `String` : le programme se nomme **dans le code**. Il ne vient pas
    /// d'une configuration, d'une variable d'environnement ni d'un message — `locus-execd` est le
    /// composant privilégié du système, et lui laisser prendre son binaire d'une entrée serait
    /// exactement la porte qu'ADR 0004 ferme en séparant le broker du control plane.
    #[must_use]
    pub const fn with_program(mut self, program: &'static str) -> Self {
        self.program = program;
        self
    }
}

/// À quelle cadence on redemande si le processus a fini.
const POLL: Duration = Duration::from_millis(20);

impl Runner for SystemRunner {
    fn run(&self, arguments: &[String]) -> Result<Execution, RuntimeError> {
        let mut child = Command::new(self.program)
            .args(arguments)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| RuntimeError::Unavailable {
                detail: format!("{} : {error}", self.program),
            })?;

        // Les deux flux sont drainés par des fils dédiés. Les laisser dans le tuyau ferait bloquer
        // `podman` dès qu'il écrit plus que la capacité du tube — et un processus bloqué en écriture
        // ne finit jamais, ce qui rendrait le budget inopérant au moment précis où il sert.
        let out = drain(child.stdout.take());
        let err = drain(child.stderr.take());

        let deadline = Instant::now() + self.budget;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {}
                Err(error) => {
                    return Err(RuntimeError::Unavailable {
                        detail: format!("{} : {error}", self.program),
                    });
                }
            }
            if Instant::now() >= deadline {
                // Tuer d'abord, rendre ensuite : les fils de drainage attendent la fermeture des
                // tuyaux, et un processus qu'on abandonnerait sans le tuer les retiendrait.
                let _ = child.kill();
                let _ = child.wait();
                return Err(RuntimeError::Unavailable {
                    detail: format!(
                        "{} {} n'a pas rendu la main en {} s : appel abandonné",
                        self.program,
                        arguments.first().map_or("", String::as_str),
                        self.budget.as_secs()
                    ),
                });
            }
            thread::sleep(POLL);
        };

        Ok(Execution {
            code: status.code().unwrap_or(-1),
            stdout: out.join().unwrap_or_default(),
            stderr: err.join().unwrap_or_default(),
        })
    }
}

/// Lire un flux jusqu'au bout, dans un fil à part.
fn drain<S: Read + Send + 'static>(stream: Option<S>) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut read = String::new();
        if let Some(mut stream) = stream {
            let mut bytes = Vec::new();
            if stream.read_to_end(&mut bytes).is_ok() {
                read = String::from_utf8_lossy(&bytes).into_owned();
            }
        }
        read
    })
}
