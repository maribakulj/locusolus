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
use std::time::Duration;

use locus_execution::{SandboxAttestation, SandboxLevel, SandboxSpec};

use super::campaign::ProbeHost;
use super::invocation::{
    INSPECTED_FIELDS, SeccompProfiles, Workload, create_arguments, inspect_arguments,
};
use super::plan::plan;
use super::selftest::{ProbeContext, exec_arguments};
// Réexportés : `driver::Execution` et `driver::Runner` restent des chemins valides pour qui les
// lisait ici avant que `W5.af.3` ne leur donne un second consommateur.
pub use super::process::{CALL_BUDGET, Execution, Runner, SystemRunner};
use crate::runtime::{RuntimeError, RuntimePort, SandboxId};

/// Le nom du mécanisme de confinement que ce driver applique — `W5.ae`, ADR 0035 décision 1.
///
/// `lep/1.0` range `backend` parmi les champs **requis** de `SandboxAttestation`, et le mot voyage
/// donc déjà sur le fil. La constante existe pour qu'il n'y en ait **qu'un** : le driver l'écrit dans
/// l'attestation qu'il produit, la campagne de self-tests l'écrit dans ce qu'elle dépose, et deux
/// littéraux auraient divergé au premier remaniement — auquel moment un enregistrement parfaitement
/// valide aurait cessé d'être reconnu comme portant sur ce mécanisme-ci.
pub const BACKEND: &str = "podman-rootless";

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
            backend: BACKEND,
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
        SandboxAttestation::new(level, BACKEND, evidence(&observed)).map_err(|error| {
            RuntimeError::Unsupported {
                capability: format!("attestation : {error}"),
            }
        })
    }
}

impl<R: Runner> ProbeHost for PodmanBackend<R> {
    fn probe(
        &self,
        id: &SandboxId,
        command: &[&str],
        context: &ProbeContext,
    ) -> Result<Execution, RuntimeError> {
        self.runner.run(&exec_arguments(id, command, context))
    }

    /// Pour podman, « éprouvable » et « en marche » coïncident : `podman exec` entre dans un
    /// conteneur, et un conteneur arrêté n'en accepte pas. C'est [`PodmanBackend::is_running`] qui
    /// répond, sans rien changer à ce qu'elle mesurait déjà.
    fn is_probeable(&self, id: &SandboxId) -> Option<bool> {
        self.is_running(id)
    }

    fn launch_pause(&self) -> Duration {
        self.launch_pause
    }

    fn host_boot_id(&self) -> Option<&str> {
        self.host_boot_id.as_deref()
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
