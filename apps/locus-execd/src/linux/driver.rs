//! Le driver rootless : le seul endroit du dépôt qui lance un runtime de containers.
//!
//! # La seule chose qui compte ici
//!
//! `runtime.rs` l'écrit à propos de [`crate::runtime::RuntimePort::attestation`] : « c'est le
//! worker qui atteste, pas le broker ; un broker qui composerait l'attestation à partir de ce
//! qu'il avait demandé attesterait de sa propre demande ». Ce module en tire la conséquence : le
//! niveau attesté est **dérivé de ce que Podman dit du conteneur qui tourne**, jamais du plan qui
//! l'a créé. Si le confinement obtenu est plus faible que celui demandé, l'attestation le dit, et
//! `locus_execution::conformance` refuse — ou exige l'approbation et produit son événement.
//!
//! # Pourquoi le lancement passe par un port
//!
//! [`Runner`] est un port, comme `TemporalGateway` l'est pour W3. Il rend la construction des
//! arguments, l'analyse des sorties et tous les chemins d'erreur vérifiables sans Podman — donc
//! en CI, où aucun runtime rootless n'est garanti. Ce qui reste hors test est
//! [`SystemRunner::run`], qui lance un processus — et **le borne** : voir [`CALL_BUDGET`].

use std::collections::BTreeMap;
use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use locus_execution::{SandboxAttestation, SandboxLevel, SandboxSpec};

use super::invocation::{
    INSPECTED_FIELDS, SeccompProfiles, Workload, create_arguments, inspect_arguments,
};
use super::plan::plan;
use crate::runtime::{RuntimeError, RuntimePort, SandboxId};

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

/// Lancer `podman`.
///
/// `&self` et non `&mut self` : lancer un processus ne mute rien du lanceur, et
/// [`RuntimePort::attestation`] prend `&self`. Un port qui exigerait `&mut` forcerait l'attestation
/// à devenir mutante, c'est-à-dire à pouvoir changer ce dont elle témoigne.
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

/// Le backend Podman rootless.
pub struct PodmanBackend<R: Runner> {
    runner: R,
    profiles: SeccompProfiles,
    workload: Workload,
    created: BTreeMap<SandboxId, SandboxSpec>,
    counter: u32,
    launch_pause: Duration,
    host_boot_id: Option<String>,
}

/// Le gabarit qui ne demande qu'une chose : la sandbox tourne-t-elle ?
///
/// Distinct de celui de l'attestation, qui relit douze champs. Demander les douze pour n'en lire
/// qu'un ferait payer une inspection complète à chaque sonde qui n'a rien rendu.
pub const RUNNING_TEMPLATE: &str = "{{.State.Running}}";

/// La pause avant la première reprise d'un lancement, quand le runtime n'a pas pu.
///
/// Elle double à chaque tentative ; avec `LAUNCH_ATTEMPTS`, la somme couvre le pire cas connu.
pub const FIRST_LAUNCH_PAUSE: Duration = Duration::from_millis(100);

impl<R: Runner> PodmanBackend<R> {
    /// Construire le backend.
    ///
    /// Le workload est fourni à la construction parce que `SandboxSpec` ne porte pas d'image :
    /// c'est l'`EnvironmentBlueprint` de §19.3 qui la porte, et W5 la fournira. Ce paramètre est
    /// la place que cette dépendance occupera.
    pub const fn new(runner: R, profiles: SeccompProfiles, workload: Workload) -> Self {
        Self {
            runner,
            profiles,
            workload,
            created: BTreeMap::new(),
            counter: 0,
            launch_pause: FIRST_LAUNCH_PAUSE,
            host_boot_id: None,
        }
    }

    /// Dire au backend quel est le `boot_id` de l'hôte.
    ///
    /// # Pourquoi ce n'est pas lu à la construction
    ///
    /// [`PodmanBackend::new`] est `const fn`, et le rester est une garantie : construire un backend
    /// ne touche pas le système. Un constructeur qui lirait `/proc` en passant ferait dépendre la
    /// construction d'un hôte, donc rendrait tout test de construction sensible à la machine — ce
    /// que ce dépôt refuse (« aucune dépendance implicite à une machine de développeur »).
    ///
    /// La lecture est donc explicite, par [`host_boot_id`], et son absence est un fait que la sonde
    /// sait dire : sans `boot_id` d'hôte, `reach_host_kernel_interfaces` ne conclut pas.
    #[must_use]
    pub fn with_host_boot_id(mut self, boot_id: Option<String>) -> Self {
        self.host_boot_id = boot_id;
        self
    }

    /// Le `boot_id` de l'hôte, quand il a été fourni.
    #[must_use]
    pub fn host_boot_id(&self) -> Option<&str> {
        self.host_boot_id.as_deref()
    }

    /// Changer la pause entre deux tentatives de lancement d'une sonde.
    ///
    /// # Pourquoi c'est réglable, et pourquoi le défaut n'est pas zéro
    ///
    /// `W5.o` fait retenter une sonde que le runtime n'a pas pu lancer, avec des pauses qui doublent
    /// et dont la somme couvre le pire cas connu — une sonde précédente qui tient le cgroup PID le
    /// temps de ses `sleep`. Ce budget se compte en secondes, et il est juste **contre un vrai
    /// runtime**.
    ///
    /// Contre un double, il ne mesure rien et coûte tout : la suite de tests dormait cinquante
    /// secondes pour éprouver une reprise dont chaque itération est immédiate. Le passer à zéro dans
    /// les tests n'affaiblit pas ce qu'ils vérifient — le **nombre** de tentatives — et c'est ce
    /// nombre, pas la durée, qui décide si une sonde a été mesurée.
    #[must_use]
    pub const fn with_launch_pause(mut self, pause: Duration) -> Self {
        self.launch_pause = pause;
        self
    }

    /// La pause avant la première reprise ; elle double ensuite.
    #[must_use]
    pub const fn launch_pause(&self) -> Duration {
        self.launch_pause
    }

    /// Le lanceur, pour qu'un test puisse lire ce qui lui a été demandé.
    pub const fn runner(&self) -> &R {
        &self.runner
    }

    /// Le nom que portera la prochaine sandbox.
    fn next_name(&mut self) -> String {
        self.counter += 1;
        format!("locus-{:04}", self.counter)
    }

    /// Lancer, et faire d'un code non nul une erreur nommée.
    fn expect_success(&self, arguments: &[String]) -> Result<Execution, RuntimeError> {
        let execution = self.runner.run(arguments)?;
        if execution.code == 0 {
            return Ok(execution);
        }
        // `Refused` et non `Unavailable` : le runtime a **répondu**. `W5.s` a séparé les deux parce
        // que « je n'ai pas pu demander » et « on m'a répondu non » n'envoient pas chercher la même
        // chose, et que les confondre remontait jusqu'au rapport de sondes.
        Err(RuntimeError::Refused {
            verb: arguments.first().cloned().unwrap_or_default(),
            code: execution.code,
            detail: execution.stderr.trim().to_owned(),
        })
    }
}

impl<R: Runner> RuntimePort for PodmanBackend<R> {
    fn create(&mut self, spec: &SandboxSpec) -> Result<SandboxId, RuntimeError> {
        let confinement = plan(spec).map_err(|error| RuntimeError::Unsupported {
            capability: error.to_string(),
        })?;
        let name = self.next_name();
        let arguments = create_arguments(&confinement, &self.workload, &self.profiles, &name)
            .map_err(|error| RuntimeError::Unsupported {
                capability: error.to_string(),
            })?;
        self.expect_success(&arguments)?;
        let id = SandboxId::new(&name)?;
        self.created.insert(id.clone(), spec.clone());
        Ok(id)
    }

    fn start(&mut self, id: &SandboxId) -> Result<(), RuntimeError> {
        self.known(id)?;
        self.expect_success(&["start".to_owned(), id.as_str().to_owned()])?;
        Ok(())
    }

    fn stop(&mut self, id: &SandboxId) -> Result<(), RuntimeError> {
        self.known(id)?;
        self.expect_success(&["stop".to_owned(), id.as_str().to_owned()])?;
        Ok(())
    }

    fn remove(&mut self, id: &SandboxId) -> Result<(), RuntimeError> {
        self.known(id)?;
        // `--force` parce qu'un retrait doit aboutir même sur une sandbox encore en marche : le
        // rôle de cette méthode est de **rendre le nom**, et un retrait qui exigerait un arrêt
        // préalable laisserait le nom pris exactement dans le cas où on a le plus besoin de le
        // libérer — celui où la suite s'est mal passée.
        self.expect_success(&[
            "rm".to_owned(),
            "--force".to_owned(),
            id.as_str().to_owned(),
        ])?;
        // Retirée du registre **après** le succès du runtime. L'inverse rendrait la sandbox
        // inconnue du backend alors qu'elle existe encore sur l'hôte, et plus personne n'aurait de
        // quoi la retirer.
        self.created.remove(id);
        Ok(())
    }

    fn attestation(&self, id: &SandboxId) -> Result<SandboxAttestation, RuntimeError> {
        self.known(id)?;
        let execution = self.expect_success(&inspect_arguments(id.as_str()))?;
        let observed = observations(&execution.stdout)?;
        let level = observed_level(&observed);
        SandboxAttestation::new(level, "podman-rootless", evidence(&observed)).map_err(|error| {
            RuntimeError::Unsupported {
                capability: format!("attestation : {error}"),
            }
        })
    }
}

impl<R: Runner> PodmanBackend<R> {
    /// La sandbox tourne-t-elle encore ? `None` quand on n'a pas pu le demander.
    ///
    /// # Trois réponses, parce qu'il y a trois états
    ///
    /// `Some(true)` : elle tourne. `Some(false)` : le runtime a répondu, et elle ne tourne plus.
    /// `None` : le runtime n'a pas répondu, et **on ne sait pas**.
    ///
    /// Rendre un booléen forcerait la troisième dans l'une des deux autres. Vers `false`, un
    /// runtime muet ferait déclarer mortes des sandboxes bien vivantes, et l'appelant écrirait
    /// « la sandbox ne tournait plus » là où la vérité est « je n'ai pas pu demander ». C'est
    /// exactement la faute que `W5.n` et `W5.o` ont passé deux sprints à retirer du harnais.
    #[must_use]
    pub fn is_running(&self, id: &SandboxId) -> Option<bool> {
        self.runner
            .run(&[
                "inspect".to_owned(),
                "--format".to_owned(),
                RUNNING_TEMPLATE.to_owned(),
                id.as_str().to_owned(),
            ])
            .ok()
            .filter(|execution| execution.code == 0)
            .and_then(|execution| match execution.stdout.trim() {
                "true" => Some(true),
                "false" => Some(false),
                // Ni l'un ni l'autre : le runtime a répondu quelque chose qu'on ne sait pas lire.
                // C'est une **troisième** ignorance, et la ranger avec « ne tourne plus » ferait
                // déclarer mortes des sandboxes sur une réponse qu'on n'a pas comprise.
                _ => None,
            })
    }

    fn known(&self, id: &SandboxId) -> Result<(), RuntimeError> {
        if self.created.contains_key(id) {
            return Ok(());
        }
        Err(RuntimeError::Unknown { id: id.clone() })
    }
}

/// Relire la sortie d'inspection.
///
/// # Errors
///
/// [`RuntimeError::Unsupported`] quand un champ attendu manque. Un champ absent n'est pas une
/// valeur par défaut : Podman a peut-être changé de nom de champ, et deviner ferait attester un
/// confinement sur une lecture qui n'a pas eu lieu.
fn observations(stdout: &str) -> Result<BTreeMap<String, String>, RuntimeError> {
    let mut observed = BTreeMap::new();
    for line in stdout.lines() {
        if let Some((key, value)) = line.split_once('=') {
            observed.insert(key.trim().to_owned(), value.trim().to_owned());
        }
    }
    for field in INSPECTED_FIELDS {
        if !observed.contains_key(field) {
            return Err(RuntimeError::Unsupported {
                capability: format!("champ d'inspection « {field} »"),
            });
        }
    }
    Ok(observed)
}

/// Le niveau que ces observations soutiennent.
///
/// Dérivé de ce qui a été **constaté**, jamais de ce qui avait été demandé. C'est la seule
/// propriété de ce module qui ne se négocie pas : elle est ce qui rend le downgrade visible.
fn observed_level(observed: &BTreeMap<String, String>) -> SandboxLevel {
    let shares = |key: &str| observed.get(key).is_some_and(|value| value == "host");
    let private = |key: &str| !shares(key);
    let no_new_privileges = observed
        .get("security")
        .is_some_and(|value| value.contains("no-new-privileges"));

    if !private("userns") || !no_new_privileges {
        return SandboxLevel::S0;
    }
    let contained = private("pidns")
        && private("ipcns")
        && private("utsns")
        && observed
            .get("readonly")
            .is_some_and(|value| value == "true");
    if !contained {
        return SandboxLevel::S1;
    }
    let isolated_network = observed
        .get("network")
        .is_some_and(|value| value == "none" || value == "slirp4netns");
    if isolated_network {
        SandboxLevel::S3
    } else {
        SandboxLevel::S2
    }
}

/// Le témoignage : ce qui a été lu, tel qu'il a été lu.
fn evidence(observed: &BTreeMap<String, String>) -> Vec<String> {
    observed
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect()
}

/// Lire le `boot_id` du noyau qui tourne ici.
///
/// # Ce qu'il vaut comme discriminant
///
/// Un UUID que le noyau régénère à chaque démarrage. Un conteneur **partage** celui de son hôte,
/// parce qu'il partage son noyau ; une micro-VM démarre le sien. C'est ce qui permet à
/// `reach_host_kernel_interfaces` de constater ce que `S4` promet — un autre noyau — plutôt que de
/// constater qu'une lecture est refusée, ce qui ne distingue pas les deux.
///
/// `None` quand le fichier n'est pas lisible : un hôte non-Linux, un `/proc` absent. L'absence n'est
/// pas un échec du backend, c'est un fait que la sonde saura dire — elle ne conclura pas.
#[must_use]
pub fn host_boot_id() -> Option<String> {
    boot_id_from(&std::fs::read_to_string(BOOT_ID_PATH).ok()?)
}

/// Le fichier où le noyau publie l'identité de son démarrage.
pub const BOOT_ID_PATH: &str = "/proc/sys/kernel/random/boot_id";

/// Ce qu'un contenu de fichier vaut comme `boot_id`.
///
/// # Pourquoi c'est une fonction à part
///
/// [`host_boot_id`] lit un fichier, et sur toute machine où les tests tournent ce fichier existe et
/// n'est pas vide : la branche « ce que j'ai lu ne vaut rien » n'était traversée par aucun test. La
/// mutation l'a montrée en supprimant le filtre sans que rien ne morde. Séparer la **décision** de
/// la **lecture** la rend vérifiable sans dépendre de ce que la machine contient.
///
/// Un fichier vide, ou qui ne contient que de l'espace, ne rend pas un `boot_id` vide : il ne rend
/// **rien**. Annoncer une chaîne vide à la sonde lui ferait comparer son `boot_id` à rien, donc
/// conclure « un autre noyau » — une ignorance lue comme une isolation.
#[must_use]
pub fn boot_id_from(read: &str) -> Option<String> {
    let trimmed = read.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}
