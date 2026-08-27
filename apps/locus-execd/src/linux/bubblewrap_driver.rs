//! Le mécanisme `bubblewrap` derrière le port — `W5.af.3`, ADR 0035 décision 4.
//!
//! # Ce que ce module est, et ce que `bubblewrap.rs` reste
//!
//! `bubblewrap.rs` traduit un plan en arguments, et ne lance rien : c'est ce qui le rend vérifiable
//! sans hôte. Ce module-ci **lance**, comme `driver.rs` le fait pour podman. Les deux noms ne sont
//! pas encore symétriques — `driver.rs` porte le driver podman sans le dire dans son nom — et les
//! renommer serait un remaniement à lui seul.
//!
//! # `create` et `start` sont de la **comptabilité**, et c'est un fait sur le mécanisme
//!
//! `podman create` fabrique un conteneur qui existe sur l'hôte, avec un nom et une couche
//! inscriptible ; `podman start` y lance un processus. `bubblewrap` n'a ni l'un ni l'autre : mesuré
//! en `W0.23`, il ne garde **rien** entre deux invocations — un `tmpfs` écrit à la première est vide
//! à la seconde — et il n'a ni `exec`, ni `enter`, ni `attach`.
//!
//! Une sandbox `bwrap` n'existe donc que **pendant** la commande qu'elle enveloppe. [`RuntimePort`]
//! est quand même honoré, parce que ce qu'il promet à ses appelants reste vrai : après `create` puis
//! `start`, la sandbox est utilisable ; après `remove`, son nom est libre. Ce qui change est ce que
//! ces verbes **font** — ici, tenir un registre du plan calculé, ce qui n'est pas rien : c'est ce
//! registre qui permet à une sonde et à une attestation de porter sur le **même** confinement.
//!
//! Ce module ne prétend donc pas qu'un processus tourne entre deux sondes, et
//! [`ProbeHost::is_probeable`] porte le nom de la question que la campagne pose réellement.
//!
//! # L'attestation n'a rien à réinspecter, et n'invente rien pour autant
//!
//! `driver.rs` demande à podman ce qu'il a appliqué au conteneur qui existe encore. Ici il n'y a
//! plus de conteneur — mais une sandbox `bwrap` est **entièrement déterminée par ses arguments**.
//! Rouvrir avec les mêmes arguments et demander de l'intérieur (voir
//! [`super::bubblewrap::INSPECTION`]) est donc une inspection au sens plein : ce qui est relu est ce
//! que le même plan produit, et non un souvenir de ce qu'on avait demandé.
//!
//! Le niveau attesté est dérivé de ce qui a été **constaté**, jamais du plan — c'est la propriété
//! que `driver.rs` appelle « la seule qui ne se négocie pas », et elle vaut ici mot pour mot.
//!
//! Ce que l'attestation dit **en plus** : les limites que ce mécanisme n'applique pas, par
//! [`super::bubblewrap::unenforced`], sous leurs noms de fichiers de contrôleur. L'ADR 0035 refuse
//! qu'une attestation annonce un niveau sans dire ce qui, dans ce niveau, n'est pas tenu.

use std::collections::BTreeMap;
use std::time::Duration;

use locus_execution::{SandboxAttestation, SandboxLevel, SandboxSpec};

use super::bubblewrap::{
    BACKEND, INSPECTED_NAMESPACES, INSPECTION, PROGRAM, unenforced, wrap_arguments_with_env,
};
use super::campaign::ProbeHost;
use super::plan::{ConfinementPlan, plan};
use super::process::{Execution, Runner};
use super::selftest::{
    HOST_BOOT_ID_VARIABLE, ProbeContext, QUOTA_BYTES_VARIABLE, QUOTA_TARGET_VARIABLE,
};
use crate::runtime::{RuntimeError, RuntimePort, SandboxId};

/// La pause avant la première reprise d'un lancement que le mécanisme n'a pas pu faire.
///
/// La même que celle du driver podman, et pour la même raison : elle double à chaque tentative, et
/// la somme couvre le pire cas connu.
pub const FIRST_LAUNCH_PAUSE: Duration = Duration::from_millis(100);

/// Le mécanisme `bubblewrap`, derrière [`RuntimePort`].
pub struct BubblewrapBackend<R: Runner> {
    runner: R,
    created: BTreeMap<SandboxId, ConfinementPlan>,
    counter: u32,
    launch_pause: Duration,
    host_boot_id: Option<String>,
    host_namespaces: BTreeMap<String, String>,
}

impl<R: Runner> BubblewrapBackend<R> {
    /// Construire le mécanisme.
    ///
    /// Aucun `Workload` en paramètre, et c'est une différence de fond avec podman : `Workload`
    /// désigne une **image par digest**, et `bubblewrap` n'a pas d'image. Ce qu'il exécute est la
    /// commande qu'on lui donne, dans une racine composée depuis l'hôte. C'est aussi pourquoi
    /// l'attestation ne porte aucun `image_digest`.
    ///
    /// `const fn`, comme [`super::driver::PodmanBackend::new`] et pour la même garantie : construire
    /// un mécanisme ne touche pas le système. Les faits d'hôte entrent par
    /// [`BubblewrapBackend::with_host_boot_id`] et [`BubblewrapBackend::with_host_namespaces`], où
    /// ils se voient.
    pub const fn new(runner: R) -> Self {
        Self {
            runner,
            created: BTreeMap::new(),
            counter: 0,
            launch_pause: FIRST_LAUNCH_PAUSE,
            host_boot_id: None,
            host_namespaces: BTreeMap::new(),
        }
    }

    /// Dire au mécanisme quel est le `boot_id` de l'hôte.
    #[must_use]
    pub fn with_host_boot_id(mut self, boot_id: Option<String>) -> Self {
        self.host_boot_id = boot_id;
        self
    }

    /// Dire au mécanisme quels namespaces l'hôte occupe, pour que l'attestation puisse **comparer**.
    ///
    /// # Pourquoi une comparaison, et pas une supposition
    ///
    /// De l'intérieur d'une sandbox, `readlink /proc/self/ns/user` rend un inode. Seul, il ne dit
    /// rien : c'est sa **différence** avec celui de l'hôte qui dit qu'un namespace a été obtenu.
    /// Sans les valeurs de l'hôte, l'attestation ne peut donc pas conclure — et elle le dira, plutôt
    /// que de supposer que le drapeau demandé a été honoré.
    ///
    /// C'est la même discipline que `host_boot_id` : un fait d'hôte entre par un appel qui se voit,
    /// jamais par une lecture cachée dans un constructeur.
    #[must_use]
    pub fn with_host_namespaces(mut self, namespaces: BTreeMap<String, String>) -> Self {
        self.host_namespaces = namespaces;
        self
    }

    /// Changer la pause entre deux tentatives de lancement.
    #[must_use]
    pub const fn with_launch_pause(mut self, pause: Duration) -> Self {
        self.launch_pause = pause;
        self
    }

    /// Le lanceur, pour qu'un test puisse lire ce qui lui a été demandé.
    pub const fn runner(&self) -> &R {
        &self.runner
    }

    /// Le nom que portera la prochaine sandbox.
    fn next_name(&mut self) -> String {
        self.counter += 1;
        format!("locus-bw-{:04}", self.counter)
    }

    /// Le plan enregistré pour cette sandbox.
    fn known(&self, id: &SandboxId) -> Result<&ConfinementPlan, RuntimeError> {
        self.created
            .get(id)
            .ok_or_else(|| RuntimeError::Unknown { id: id.clone() })
    }

    /// Les variables que la commande recevra, dans l'ordre où `bwrap` les prendra.
    fn declared(context: &ProbeContext) -> Vec<(String, String)> {
        let mut declared = Vec::new();
        if let Some((path, bytes)) = &context.quota {
            declared.push((QUOTA_TARGET_VARIABLE.to_owned(), path.clone()));
            declared.push((QUOTA_BYTES_VARIABLE.to_owned(), bytes.to_string()));
        }
        if let Some(boot_id) = &context.host_boot_id {
            declared.push((HOST_BOOT_ID_VARIABLE.to_owned(), boot_id.clone()));
        }
        declared
    }
}

impl<R: Runner> RuntimePort for BubblewrapBackend<R> {
    /// Calculer le plan et l'enregistrer. **Rien ne tourne.**
    fn create(&mut self, spec: &SandboxSpec) -> Result<SandboxId, RuntimeError> {
        let confinement = plan(spec).map_err(|error| RuntimeError::Unsupported {
            capability: error.to_string(),
        })?;
        let id = SandboxId::new(&self.next_name())?;
        self.created.insert(id.clone(), confinement);
        Ok(id)
    }

    /// Rien à démarrer : la sandbox naît avec la commande qu'elle enveloppe.
    ///
    /// L'appel n'est pas vide de sens pour autant — il **refuse** un identifiant inconnu, ce qui est
    /// exactement ce qu'un appelant attend de lui, et ce qui empêche une sonde de porter sur une
    /// sandbox que personne n'a créée.
    fn start(&mut self, id: &SandboxId) -> Result<(), RuntimeError> {
        self.known(id)?;
        Ok(())
    }

    /// Rien à arrêter : le processus enveloppé est déjà sorti quand on arrive ici.
    ///
    /// `--die-with-parent` est ce qui le garantit, et il est posé sur **toute** invocation.
    fn stop(&mut self, id: &SandboxId) -> Result<(), RuntimeError> {
        self.known(id)?;
        Ok(())
    }

    /// Rendre le nom, et le plan avec.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::Unknown`] pour une sandbox jamais créée. « Je ne l'ai jamais eue » et « je
    /// l'ai rendue » sont deux faits différents, et les confondre laisserait croire à un nettoyage
    /// qui n'a pas eu lieu.
    fn remove(&mut self, id: &SandboxId) -> Result<(), RuntimeError> {
        self.known(id)?;
        self.created.remove(id);
        Ok(())
    }

    /// Rouvrir une sandbox avec le **même plan**, et lui demander ce qu'elle a obtenu.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::Unknown`] pour une sandbox inconnue, [`RuntimeError::Refused`] quand `bwrap`
    /// n'a pas pu ouvrir la sandbox d'inspection, [`RuntimeError::Unsupported`] quand ce qu'elle a
    /// répondu ne porte pas les champs attendus — un champ absent n'est pas une valeur par défaut.
    fn attestation(&self, id: &SandboxId) -> Result<SandboxAttestation, RuntimeError> {
        let confinement = self.known(id)?;
        let arguments = wrap_arguments_with_env(
            confinement,
            &[],
            &["/bin/sh".to_owned(), "-c".to_owned(), INSPECTION.to_owned()],
        );
        let execution = self.runner.run(&arguments)?;
        if execution.code != 0 {
            return Err(RuntimeError::Refused {
                backend: BACKEND,
                verb: "inspection".to_owned(),
                code: execution.code,
                detail: execution.stderr.trim().to_owned(),
            });
        }
        let observed = observations(&execution.stdout)?;
        let obtained = self.obtained(&observed)?;
        let level = observed_level(&observed, &obtained);
        SandboxAttestation::new(level, BACKEND, evidence(&observed, confinement)).map_err(|error| {
            RuntimeError::Unsupported {
                capability: format!("attestation : {error}"),
            }
        })
    }
}

impl<R: Runner> BubblewrapBackend<R> {
    /// Les namespaces que la sandbox a réellement obtenus, par comparaison avec ceux de l'hôte.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::Unsupported`] quand les namespaces de l'hôte n'ont pas été fournis. Sans eux,
    /// un inode relu ne se compare à rien, et conclure reviendrait à croire le drapeau demandé sur
    /// parole — ce que cette attestation existe précisément pour éviter.
    fn obtained(
        &self,
        observed: &BTreeMap<String, String>,
    ) -> Result<BTreeMap<String, bool>, RuntimeError> {
        if self.host_namespaces.is_empty() {
            return Err(RuntimeError::Unsupported {
                capability: "les namespaces de l'hôte, sans lesquels un inode relu ne se compare \
                             à rien"
                    .to_owned(),
            });
        }
        let mut obtenus = BTreeMap::new();
        for namespace in INSPECTED_NAMESPACES {
            let Some(dedans) = observed.get(namespace) else {
                return Err(RuntimeError::Unsupported {
                    capability: format!("le namespace « {namespace} » dans la réponse"),
                });
            };
            let Some(dehors) = self.host_namespaces.get(namespace) else {
                return Err(RuntimeError::Unsupported {
                    capability: format!("le namespace « {namespace} » de l'hôte"),
                });
            };
            obtenus.insert(namespace.to_owned(), dedans != dehors);
        }
        Ok(obtenus)
    }
}

/// Relire la réponse de la sandbox.
///
/// # Errors
///
/// [`RuntimeError::Unsupported`] quand un champ attendu manque. Un champ absent n'est pas une valeur
/// par défaut : le shell a peut-être échoué à mi-chemin, et deviner ferait attester un confinement
/// sur une lecture qui n'a pas eu lieu.
fn observations(stdout: &str) -> Result<BTreeMap<String, String>, RuntimeError> {
    let mut observed = BTreeMap::new();
    for line in stdout.lines() {
        if let Some((key, value)) = line.split_once('=') {
            observed.insert(key.trim().to_owned(), value.trim().to_owned());
        }
    }
    for field in ["readonly", "no_new_privs", "route"] {
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
/// Dérivé de ce qui a été **constaté**, jamais de ce qui avait été demandé. L'échelle est celle de
/// `driver.rs`, parce que c'est celle de `SandboxLevel` et non celle d'un mécanisme.
///
/// `S4` n'est pas atteignable ici, et ce n'est pas un oubli : il promet un **autre noyau**, et
/// `bubblewrap` compose des namespaces sur celui de l'hôte. Un mécanisme qui prétendrait `S4` sans
/// changer de noyau attesterait ce que l'ADR 0035 appelle une preuve portant sur autre chose.
fn observed_level(
    observed: &BTreeMap<String, String>,
    obtained: &BTreeMap<String, bool>,
) -> SandboxLevel {
    let obtenu = |namespace: &str| obtained.get(namespace).copied().unwrap_or(false);
    let no_new_privs = observed
        .get("no_new_privs")
        .is_some_and(|value| value == "1");

    if !obtenu("user") || !no_new_privs {
        return SandboxLevel::S0;
    }
    let contained = obtenu("pid")
        && obtenu("ipc")
        && obtenu("uts")
        && observed
            .get("readonly")
            .is_some_and(|value| value == "true");
    if !contained {
        return SandboxLevel::S1;
    }
    if obtenu("net") && observed.get("route").is_some_and(|value| value == "absent") {
        SandboxLevel::S3
    } else {
        SandboxLevel::S2
    }
}

/// Le témoignage : ce qui a été lu, tel qu'il a été lu, **et** ce que ce mécanisme n'applique pas.
///
/// Les deux moitiés voyagent ensemble parce qu'un exploitant qui lit « `S2` sous bubblewrap » doit
/// pouvoir savoir dans la même phrase que la borne mémoire de sa mission est tenue ailleurs, ou pas
/// du tout. Séparer les deux ferait de la seconde une note de bas de page que personne n'ouvre.
fn evidence(observed: &BTreeMap<String, String>, confinement: &ConfinementPlan) -> Vec<String> {
    let mut temoignage: Vec<String> = observed
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect();
    temoignage.extend(
        unenforced(confinement)
            .into_iter()
            .map(|manquante| format!("unenforced:{}", manquante.limit)),
    );
    temoignage
}

impl<R: Runner> ProbeHost for BubblewrapBackend<R> {
    /// Ouvrir une sandbox autour de la commande de la sonde, et rendre ce qu'elle a écrit.
    ///
    /// C'est ici que la différence de mécanisme se voit le mieux : `podman exec` **entre** dans un
    /// conteneur qui tourne, tandis qu'ici la sonde **est** la commande enveloppée. La campagne ne
    /// voit pas la différence, parce que ce qu'elle demande — « éprouve ceci dans ce confinement » —
    /// est vrai des deux côtés.
    fn probe(
        &self,
        id: &SandboxId,
        command: &[&str],
        context: &ProbeContext,
    ) -> Result<Execution, RuntimeError> {
        let confinement = self.known(id)?;
        let commande: Vec<String> = command.iter().map(|part| (*part).to_owned()).collect();
        let arguments = wrap_arguments_with_env(confinement, &Self::declared(context), &commande);
        self.runner.run(&arguments)
    }

    /// Y a-t-il encore une sandbox à éprouver ? Ici, la question est celle du **registre**.
    ///
    /// Rien ne tourne entre deux sondes, donc « la sandbox tourne-t-elle » n'aurait pas de réponse
    /// honnête. Ce que la campagne demande vraiment — reste-t-il quelque chose à éprouver — se lit
    /// dans le registre : tant que le plan y est, une sonde peut encore être ouverte.
    ///
    /// Jamais `None` : la question ne passe par aucun appel qui pourrait ne pas répondre. C'est un
    /// fait sur ce mécanisme, pas une simplification — et c'est pour cela que le port rend un
    /// `Option`, que l'autre mécanisme, lui, remplit vraiment.
    fn is_probeable(&self, id: &SandboxId) -> Option<bool> {
        Some(self.created.contains_key(id))
    }

    fn launch_pause(&self) -> Duration {
        self.launch_pause
    }

    fn host_boot_id(&self) -> Option<&str> {
        self.host_boot_id.as_deref()
    }
}

/// Lire les namespaces que ce processus occupe, pour les donner à comparer.
///
/// # Pourquoi c'est une fonction libre, et pas un constructeur
///
/// Elle **touche l'hôte**. La garder hors de [`BubblewrapBackend::new`] est ce qui laisse la
/// construction déterministe et testable sans machine — même discipline que
/// `super::driver::host_boot_id`.
///
/// Les entrées absentes sont **omises**, jamais remplies d'une valeur de repli : une comparaison
/// contre un repli inventé rendrait « namespace obtenu » sur un `/proc` illisible.
#[must_use]
pub fn host_namespaces() -> BTreeMap<String, String> {
    let mut lus = BTreeMap::new();
    for namespace in INSPECTED_NAMESPACES {
        if let Ok(cible) = std::fs::read_link(format!("/proc/self/ns/{namespace}")) {
            lus.insert(namespace.to_owned(), cible.to_string_lossy().into_owned());
        }
    }
    lus
}

/// Le programme que ce mécanisme lance, pour qui compose un [`super::process::SystemRunner`].
#[must_use]
pub const fn program() -> &'static str {
    PROGRAM
}
