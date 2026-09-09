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
use locus_execd::linux::probe::Reader;
use locus_execd::linux::{
    BACKEND, HostFacts, PodmanBackend, RestrictedProfile, SeccompProfiles, SystemRunner, Workload,
    certify,
};
#[cfg(target_os = "macos")]
use locus_execd::macos::MachineFacts;
use locus_execd::readiness::Readiness;
use locus_execution::spec::{Mount, MountMode, SandboxSpec};
use locus_execution::{NetworkMode, ResourceSpec, SandboxLevel, SandboxProfile};

/// L'option qui fait écouter le broker.
const LISTEN: &str = "--listen";

/// L'option qui conduit une campagne et dépose ce qu'elle conclut.
const CERTIFY: &str = "--certify";

fn main() -> ExitCode {
    // Le driver, construit sans condition. C'est la capacité que le crate exporte, et ce binaire
    // n'a plus le droit de la nier : la construire ici est ce qui rend la négation inexprimable.
    let driver = SystemRunner::new();
    println!("locus-execd : driver {}", driver.program());

    let facts = faits(&driver);
    for line in facts.evidence() {
        println!("  {line}");
    }

    let readiness = Readiness::assess(&facts);
    println!("locus-execd : {readiness}");
    // `W5.w` : l'empreinte de cet hôte, imprimée **avant** toute question d'attestation. Qui prépare
    // un fichier n'en a pas encore, donc `annonce` ne peut pas la lui dire — et sans elle il
    // écrirait un enregistrement que ce daemon écartera sans qu'il sache pourquoi.
    //
    // Aucun secret : ce sont les capacités lues du noyau, celles-là mêmes que `evidence()` vient
    // d'imprimer en clair juste au-dessus.
    println!(
        "  empreinte de cet hôte : {}",
        locus_execd::attestation::fingerprint(&facts)
    );

    if std::env::args().skip(1).any(|argument| argument == CERTIFY) {
        return certifier(&driver, &facts);
    }

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

    // `W5.z` : les attestations conservées, si l'exploitant en a posé.
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

/// Rien ne se lit : les faits d'un hôte qu'on ne peut pas interroger.
///
/// `HostFacts::probe` traite une lecture absente comme une indétermination, ce qui rend un plafond
/// `S0` et un `evidence()` qui nomme chaque manque. C'est exactement ce qu'on veut dire quand la
/// machine ne tourne pas — mieux qu'un `HostFacts` fabriqué, qui affirmerait des valeurs que
/// personne n'a lues.
#[cfg(target_os = "macos")]
struct RienALire;

#[cfg(target_os = "macos")]
impl Reader for RienALire {
    fn read(&self, _path: &str) -> Option<String> {
        None
    }
}

/// Les faits du noyau **qui confine**.
///
/// # Sur macOS, ce n'est pas celui qui appelle
///
/// `docs/03` fixe le profil : « host macOS + VM Linux légère + containers rootless par mission ».
/// Le conteneur tourne donc dans un noyau Linux, et `/sys/fs/cgroup` lu depuis macOS répond « rien »
/// pour une machine parfaitement capable — ce que ce binaire faisait, en plafonnant à `S0` un hôte
/// qui pouvait tenir `S3`.
///
/// `crate::macos` existait déjà, testé, avec son lecteur qui traverse `podman machine ssh`. Il
/// n'était appelé par personne : c'est le seul défaut que cette fonction corrige, et il était de
/// câblage, pas de conception.
///
/// Le plafond reste `S3`. Une VM **partagée** entre les missions n'est pas une micro-VM par
/// mission, et `S4` promet à une mission à haut risque son propre noyau ; `HostFacts::ceiling`
/// borne déjà à `BACKEND_CEILING`, donc lire l'invité ne relève rien qu'on n'ait le droit
/// d'annoncer.
#[cfg(target_os = "macos")]
fn faits(driver: &SystemRunner) -> HostFacts {
    let machine = MachineFacts::read(driver);
    // L'état de la machine est imprimé ici, avant les faits : une machine arrêtée n'est pas un
    // noyau incapable, c'est un service à démarrer, et le lecteur doit pouvoir faire la différence
    // sans lire le code.
    println!("  {}", machine.state());
    machine
        .guest()
        .cloned()
        .unwrap_or_else(|| HostFacts::probe(&RienALire))
}

/// Les faits de l'hôte, là où le noyau qui appelle est celui qui confine.
#[cfg(not(target_os = "macos"))]
fn faits(_driver: &SystemRunner) -> HostFacts {
    HostFacts::read_host()
}

/// Conduire la campagne, et déposer ce qu'elle conclut — `--certify`.
///
/// # Le chaînon qui manquait
///
/// `certify` conduisait déjà la campagne, `record` fabriquait déjà l'enregistrement, `emit` le
/// sérialisait déjà — et **aucun chemin de production ne les appelait**. La CI elle-même exerçait
/// les seize sondes par `cargo test --ignored`, sans que le verdict survive au processus. Le
/// résultat se lisait au premier placement : « rien ne sera placé au-dessus de S0 », sur un hôte
/// qui venait de prouver `S3`.
///
/// C'est la troisième fois que ce dépôt rencontre cette forme — une capacité complète, testée, sans
/// appelant — et la troisième correction est la même : écrire l'appelant, sans rien inventer.
///
/// # Rien n'est deviné
///
/// Les cinq entrées viennent de l'exploitant, et leur absence **refuse** au lieu de choisir : voir
/// [`locus_execd::attestation::campaign_inputs`]. Le niveau certifié est celui que l'hôte **prouve**
/// — le lui faire dépasser serait signer ce qu'on n'a pas mesuré.
///
/// # Un `NotTrusted` ne se dépose pas
///
/// `record` rend `None`, et rien n'est écrit. Un fichier absent dit « pas d'attestation » ;
/// un fichier qui porterait un échec dirait « attestation », et il faudrait le lire pour savoir.
fn certifier(driver: &SystemRunner, facts: &HostFacts) -> ExitCode {
    let inputs = match locus_execd::attestation::campaign_inputs(
        |name| std::env::var(name).ok(),
        |path| std::fs::read_to_string(path).ok(),
    ) {
        Ok(inputs) => inputs,
        Err(refus) => {
            eprintln!("locus-execd : {refus}");
            return ExitCode::FAILURE;
        }
    };

    let profile = match RestrictedProfile::parse(&inputs.profile_path, &inputs.profile_body) {
        Ok(profile) => profile,
        Err(erreur) => {
            eprintln!("locus-execd : profil seccomp refusé — {erreur}");
            return ExitCode::FAILURE;
        }
    };

    // # Le niveau vient de la **spécification**, pas du plafond de l'hôte
    //
    // Le premier jet certifiait au plafond que l'hôte prouve. C'était faux d'une façon instructive :
    // l'hôte prouvait `S3`, la campagne partait à `S3`, et elle échouait — parce que la spec ci-
    // dessous ouvre le réseau (`NetworkMode::Full`), ce que `S3 container-isolated-network` promet
    // précisément de ne pas faire. Le plafond dit ce que le **noyau** peut soutenir ; le niveau
    // attesté dit ce que **cette campagne** a éprouvé, et les deux ne se confondent que si la spec
    // suit le niveau.
    //
    // `S2` est donc le niveau de cette spec, et c'est celui de la campagne du dépôt — même
    // `SandboxProfile`, même mode réseau, mêmes bornes. Certifier plus haut demande une spec qui
    // isole le réseau, c'est-à-dire une autre campagne : elle aura son item plutôt qu'un paramètre
    // ici, parce qu'elle éprouve autre chose.
    let level = SandboxLevel::S2;
    let plafond = Readiness::assess(facts).ceiling();
    if plafond < level {
        eprintln!(
            "locus-execd : cet hôte plafonne à {}, sous le {} que cette campagne éprouve",
            plafond.code(),
            level.code()
        );
        return ExitCode::FAILURE;
    }
    let spec = match campagne_spec(&inputs.workspace, level) {
        Ok(spec) => spec,
        Err(refus) => {
            eprintln!("locus-execd : {refus}");
            return ExitCode::FAILURE;
        }
    };

    let workload = match Workload::new(&inputs.image, vec!["sleep".to_owned(), "600".to_owned()]) {
        Ok(workload) => workload,
        Err(erreur) => {
            eprintln!("locus-execd : image de sonde refusée — {erreur}");
            return ExitCode::FAILURE;
        }
    };

    println!(
        "locus-execd : campagne à {} sur {}",
        level.code(),
        inputs.image
    );
    let mut backend = PodmanBackend::new(
        SystemRunner::new(),
        SeccompProfiles {
            restricted: Some(profile),
        },
        workload,
    )
    .with_host_boot_id(boot_id_du_noyau_qui_confine(driver));

    let standing = certify(&mut backend, &spec, level);
    let Some(attestation) = locus_execd::attestation::record(
        &inputs.worker_id,
        &standing,
        facts,
        BACKEND,
        maintenant(),
    ) else {
        // Le refus **nomme les sondes**. Sans elles il disait « la campagne n'a pas tenu S2 », ce
        // qui envoie relire seize sondes pour en trouver une — et ne distingue pas un échappement,
        // qui est un défaut de confinement, d'une sonde non concluante, qui est un défaut de
        // mesure. Les deux se réparent à des endroits opposés : l'un dans le plan, l'autre dans
        // l'hôte ou dans la sonde.
        eprintln!(
            "locus-execd : la campagne n'a pas tenu {} — rien n'est déposé",
            level.code()
        );
        if let locus_execution::selftest::Standing::NotTrusted { blocking, .. } = &standing {
            for verdict in blocking {
                eprintln!("  {verdict}");
            }
        }
        return ExitCode::FAILURE;
    };

    let contenu = match locus_execd::attestation::emit(&[attestation], &inputs.out) {
        Ok(contenu) => contenu,
        Err(refus) => {
            eprintln!("locus-execd : {refus}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(erreur) = std::fs::write(&inputs.out, contenu) {
        eprintln!("locus-execd : « {} » ne s'écrit pas — {erreur}", inputs.out);
        return ExitCode::FAILURE;
    }
    println!(
        "locus-execd : {} attesté pour {}, déposé dans {}",
        level.code(),
        inputs.worker_id,
        inputs.out
    );
    ExitCode::SUCCESS
}

/// La spécification que la campagne éprouve.
///
/// # Recopiée, jamais inventée
///
/// Bornes, profil et mode réseau sont ceux de la campagne du dépôt —
/// `apps/locus-execd/tests/host_sandbox.rs`. Une campagne qui réserverait autre chose éprouverait
/// autre chose, et l'attestation ne parlerait plus de ce que la CI mesure : deux phrases sur le même
/// hôte qui ne disent pas la même chose, sans que rien ne signale laquelle vaut.
///
/// # Errors
///
/// Une chaîne qui nomme ce qui a été refusé — un montage impossible, des quotas nuls, une spec
/// invalide. Rendue plutôt qu'imprimée : la fonction ne décide pas de la sortie du processus.
fn campagne_spec(workspace: &str, level: SandboxLevel) -> Result<SandboxSpec, String> {
    let mount = Mount::new(workspace, "/work", MountMode::ReadWrite)
        .map_err(|erreur| format!("espace de travail refusé — {erreur}"))?;
    let resources = ResourceSpec::new(1_000, 512 << 20, 128, 0, 300)
        .map_err(|erreur| format!("ressources refusées — {erreur}"))?;
    SandboxSpec::new(
        level,
        SandboxProfile::UntrustedRepository,
        NetworkMode::Full,
        vec![mount],
        resources,
    )
    .map_err(|erreur| format!("spécification refusée — {erreur}"))
}

/// L'instant courant, en millisecondes depuis l'époque.
fn maintenant() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |ecoule| i64::try_from(ecoule.as_millis()).unwrap_or(0))
}

/// Le `boot_id` du noyau **qui confine**, sur cette plateforme.
///
/// Sur macOS, ce n'est pas celui du processus courant : le conteneur partage le noyau de la VM.
/// Sans cette distinction, `reach_host_kernel_interfaces` n'a rien à quoi comparer et ne conclut
/// pas — ce qui, à `S2`, compte comme non mesuré et fait échouer la campagne pour une raison qui
/// n'a rien à voir avec le confinement.
#[cfg(target_os = "macos")]
fn boot_id_du_noyau_qui_confine(driver: &SystemRunner) -> Option<String> {
    MachineFacts::read(driver).boot_id().map(str::to_owned)
}

/// Le `boot_id` du noyau courant, là où c'est lui qui confine.
#[cfg(not(target_os = "macos"))]
fn boot_id_du_noyau_qui_confine(_driver: &SystemRunner) -> Option<String> {
    locus_execd::linux::host_boot_id()
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
