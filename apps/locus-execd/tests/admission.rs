//! Test de sortie de W4.c — ADR 0004, `docs/SPEC_V1.md` §12.2, §21.6.
//!
//! **`locus-execd` refuse proprement une mission qu'il ne peut pas honorer — en nommant *toutes*
//! les conditions qui manquent — et il est le seul endroit du dépôt qui parle d'un socket de
//! runtime.**
//!
//! Les deux moitiés sont la même décision vue de deux côtés. Refuser proprement, c'est décider
//! avant d'agir, sur des capacités déclarées : un broker qui apprendrait ses limites en échouant
//! les découvrirait après avoir créé la moitié d'une sandbox. Et être le seul à connaître le
//! socket, c'est ce qui rend cette décision opposable — si `locusd` pouvait parler au runtime,
//! l'admission ne serait qu'une politesse.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use locus_execd::{Admission, HostCapabilities, RefusalReason, admit};
use locus_execution::{
    Accelerator, Mount, MountMode, NetworkMode, ResourceSpec, SandboxLevel, SandboxProfile,
    SandboxSpec,
};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn mission(level: SandboxLevel, network: NetworkMode, resources: ResourceSpec) -> SandboxSpec {
    SandboxSpec::new(
        level,
        SandboxProfile::UntrustedRepository,
        network,
        vec![Mount::new("/srv/corpus", "/work", MountMode::ReadOnly).expect("montage licite")],
        resources,
    )
    .expect("spécification valide")
}

fn modest() -> ResourceSpec {
    ResourceSpec::new(1_000, 2 << 30, 128, 4 << 30, 600).expect("quotas non nuls")
}

fn generous_host(level: SandboxLevel) -> HostCapabilities {
    HostCapabilities::new(
        level,
        ResourceSpec::new(8_000, 32 << 30, 4_096, 256 << 30, 86_400).expect("capacité"),
        vec!["deny", "connector_only", "allowlist", "full"],
    )
}

// ---------------------------------------------------------------------------------------------
// Première moitié : refuser proprement
// ---------------------------------------------------------------------------------------------

#[test]
fn une_mission_que_l_hote_sait_honorer_est_admise_au_niveau_exige() {
    let spec = mission(SandboxLevel::S3, NetworkMode::Deny, modest());
    assert_eq!(
        admit(&spec, &generous_host(SandboxLevel::S4)),
        Admission::Admitted {
            level: SandboxLevel::S3
        },
        "le niveau appliqué est celui qu'exige la mission, pas le meilleur de l'hôte : appliquer \
         davantage serait le sur-confinement que W4.b nomme"
    );
}

#[test]
fn un_hote_qui_ne_confine_pas_assez_refuse_au_lieu_de_degrader() {
    let spec = mission(SandboxLevel::S4, NetworkMode::Deny, modest());
    assert_eq!(
        admit(&spec, &generous_host(SandboxLevel::S2)),
        Admission::Refused {
            reasons: vec![RefusalReason::LevelUnavailable {
                required: SandboxLevel::S4,
                best: SandboxLevel::S2,
            }]
        },
        "admettre au niveau que l'hôte sait offrir serait le downgrade de §21.6, pris au moment où \
         personne ne regarde et sans l'approbation nommée que W4.a exige"
    );
}

#[test]
fn le_refus_nomme_toutes_les_conditions_qui_manquent() {
    // Le cœur de « refuser proprement ». Un refus qui ne nommerait que la première condition
    // ferait corriger une chose, réessayer, découvrir la suivante — un aller-retour par condition.
    let greedy = ResourceSpec::new(64_000, 512 << 30, 65_536, 1 << 40, 86_400)
        .expect("quotas non nuls")
        .with_accelerator(Accelerator {
            kind: "tpu".to_owned(),
            count: 8,
            memory_bytes: 64 << 30,
        })
        .expect("accélérateur valide");
    let spec = mission(
        SandboxLevel::S5,
        NetworkMode::allowlist(vec!["iiif.example.org".to_owned()]).expect("allowlist"),
        greedy,
    );

    let host = HostCapabilities::new(
        SandboxLevel::S2,
        ResourceSpec::new(4_000, 8 << 30, 512, 32 << 30, 3_600).expect("capacité"),
        vec!["deny"],
    );

    let Admission::Refused { reasons } = admit(&spec, &host) else {
        panic!("quatre conditions manquent : la mission ne peut pas être admise");
    };
    assert_eq!(
        reasons,
        vec![
            RefusalReason::LevelUnavailable {
                required: SandboxLevel::S5,
                best: SandboxLevel::S2,
            },
            RefusalReason::CapacityExceeded,
            RefusalReason::AcceleratorUnavailable {
                kind: "tpu".to_owned()
            },
            RefusalReason::NetworkModeUnsupported { mode: "allowlist" },
        ],
        "les quatre, d'un coup"
    );
}

#[test]
fn chaque_condition_se_constate_seule() {
    // Sans ce test, une fonction qui rendrait toujours les quatre raisons passerait le précédent.
    let host = generous_host(SandboxLevel::S4);

    // Capacité seule.
    let too_big = ResourceSpec::new(64_000, 2 << 30, 128, 4 << 30, 600).expect("quotas");
    assert_eq!(
        admit(
            &mission(SandboxLevel::S2, NetworkMode::Deny, too_big),
            &host
        ),
        Admission::Refused {
            reasons: vec![RefusalReason::CapacityExceeded]
        }
    );

    // Accélérateur seul.
    let wrong_gpu = modest()
        .with_accelerator(Accelerator {
            kind: "rocm".to_owned(),
            count: 1,
            memory_bytes: 8 << 30,
        })
        .expect("accélérateur valide");
    assert_eq!(
        admit(
            &mission(SandboxLevel::S2, NetworkMode::Deny, wrong_gpu),
            &host
        ),
        Admission::Refused {
            reasons: vec![RefusalReason::AcceleratorUnavailable {
                kind: "rocm".to_owned()
            }]
        }
    );

    // Mode réseau seul.
    let narrow = HostCapabilities::new(
        SandboxLevel::S4,
        ResourceSpec::new(8_000, 32 << 30, 4_096, 256 << 30, 86_400).expect("capacité"),
        vec!["deny"],
    );
    assert_eq!(
        admit(
            &mission(SandboxLevel::S2, NetworkMode::Full, modest()),
            &narrow
        ),
        Admission::Refused {
            reasons: vec![RefusalReason::NetworkModeUnsupported { mode: "full" }]
        }
    );
}

#[test]
fn un_accelerateur_demande_et_present_ne_refuse_pas() {
    let wanted = modest()
        .with_accelerator(Accelerator {
            kind: "cuda".to_owned(),
            count: 1,
            memory_bytes: 8 << 30,
        })
        .expect("accélérateur valide");
    // La capacité de l'hôte doit elle aussi porter l'accélérateur pour que le fit passe.
    let host = HostCapabilities::new(
        SandboxLevel::S4,
        ResourceSpec::new(8_000, 32 << 30, 4_096, 256 << 30, 86_400)
            .expect("capacité")
            .with_accelerator(Accelerator {
                kind: "cuda".to_owned(),
                count: 4,
                memory_bytes: 80 << 30,
            })
            .expect("accélérateur valide"),
        vec!["deny"],
    );
    assert_eq!(
        admit(&mission(SandboxLevel::S3, NetworkMode::Deny, wanted), &host),
        Admission::Admitted {
            level: SandboxLevel::S3
        }
    );
}

// ---------------------------------------------------------------------------------------------
// Seconde moitié : personne d'autre ne parle à un runtime
// ---------------------------------------------------------------------------------------------

/// Les marqueurs qui trahissent un **dialogue** avec un runtime de containers.
///
/// # Nommer n'est pas parler
///
/// La première version de cette table cherchait des chemins de socket — `docker.sock`,
/// `/var/run/docker`. Elle signalait `packages/execution`, qui les nomme dans
/// `FORBIDDEN_MOUNT_MARKERS` **pour les refuser** : le contraire exact de ce qu'on traque. Un
/// chemin est une donnée ; ouvrir une connexion, lire `DOCKER_HOST` ou lancer `docker` est un acte.
///
/// La table cherche donc des actes. Une garde qui confondrait les deux forcerait à exempter le
/// paquet qui écrit la politique de sécurité — c'est-à-dire à trouer la garde là où elle sert le
/// plus.
///
/// # Le proxy qui a expiré — `W4.h`
///
/// Cette table a longtemps porté `UnixStream::connect` et `os::unix::net` **nus**, comme proxy de
/// « socket de runtime ». Le proxy tenait tant que le dépôt n'avait aucune socket Unix légitime.
/// `W4.h` en a créé une — le lien de l'ADR 0028 entre `locusd` et ce broker-ci —, et le proxy s'est
/// mis à signaler le crate dont la raison d'être est précisément de tenir `locusd` **loin** du
/// runtime.
///
/// C'est la même expiration que celle du motif d'identifiant de la garde de roadmap en `W22.a` : une
/// approximation juste au moment où elle est écrite, et fausse dès qu'un cas légitime apparaît. Et
/// c'est la même leçon que `W22.d` : une garde qui crie sur ce qui est juste se fait désactiver.
///
/// La distinction retenue est celle que le proxy approximait : **ouvrir une socket Unix n'est pas
/// parler à un runtime ; ce qui identifie un runtime est la façon dont on le trouve.** Les trois
/// façons sont couvertes — lier son SDK, lire son adresse dans l'environnement, lancer sa CLI — et
/// [`connects_to_runtime_socket`] couvre la quatrième, le chemin en dur, en exigeant l'acte **et**
/// la cible sur la même ligne. Le chemin seul reste une donnée, donc `packages/execution` continue
/// de pouvoir les refuser sans se faire accuser de les appeler.
///
/// **Ce que la garde ne verra pas**, et qui est écrit plutôt que supposé : un chemin de runtime
/// tenu dans une variable, puis passé à `connect`. Elle ne le voit pas davantage pour
/// `Command::new(variable)`, et aucune lecture de texte ne le verra jamais. Ce qui l'attrape est le
/// graphe de paquets — `apps/locusd` ne dépend pas de ce crate — et la revue.
///
/// Assemblés par `concat!` : le balayage passe aussi sur ce fichier, et une table écrite d'un bloc
/// se signalerait elle-même. Même précaution qu'en W3.a et W4.a.
fn runtime_markers() -> Vec<String> {
    vec![
        concat!("bollard", "::").to_owned(),
        concat!("shiplift", "::").to_owned(),
        concat!("DOCKER_", "HOST").to_owned(),
        concat!("CONTAINER_", "HOST").to_owned(),
        concat!("Command::new(\"", "docker").to_owned(),
        concat!("Command::new(\"", "podman").to_owned(),
        concat!("Command::new(\"", "buildah").to_owned(),
        concat!("Command::new(\"", "nerdctl").to_owned(),
    ]
}

/// Les chemins où un runtime de containers écoute.
///
/// Assemblés par `concat!` pour la même raison que la table d'actes : le balayage passe sur ce
/// fichier.
fn runtime_socket_paths() -> Vec<String> {
    vec![
        concat!("docker", ".sock").to_owned(),
        concat!("podman", ".sock").to_owned(),
        concat!("containerd", ".sock").to_owned(),
        concat!("crio", ".sock").to_owned(),
    ]
}

/// Vrai quand une ligne **ouvre** une connexion **vers** un runtime.
///
/// Les deux sont exigés ensemble : le chemin seul est une donnée que `packages/execution` refuse
/// légitimement, et l'ouverture seule est ce que le lien de l'ADR 0028 fait vers une socket que ce
/// dépôt crée lui-même. Seule leur conjonction dit qu'on parle à un runtime.
fn connects_to_runtime_socket(line: &str) -> Option<String> {
    if !line.contains(concat!("connect", "(")) {
        return None;
    }
    runtime_socket_paths()
        .into_iter()
        .find(|path| line.contains(path.as_str()))
}

fn rust_sources(directory: &Path) -> Vec<(String, String)> {
    let mut sources = Vec::new();
    let Ok(entries) = fs::read_dir(directory) else {
        return sources;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.file_name().is_some_and(|name| name == "target") {
            continue;
        }
        if path.is_dir() {
            sources.extend(rust_sources(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs")
            && let Ok(text) = fs::read_to_string(&path)
        {
            sources.push((path.display().to_string(), text));
        }
    }
    sources
}

/// La racine du workspace, **sans segments `..`**.
///
/// `join("..").join("..")` donnerait un chemin qui marche pour lire les fichiers et qui contient
/// `locus-execd` dans chacune de ses chaînes. L'exclusion ci-dessous, écrite par sous-chaîne,
/// excluait alors la totalité de l'arbre : la garde ne regardait rien et restait verte. C'est une
/// mutation qui l'a révélé — la garde n'avait jamais rien gardé.
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("apps/locus-execd est à deux niveaux sous la racine")
        .to_path_buf()
}

/// Le répertoire de ce crate — le seul auquel parler à un runtime est permis.
fn own_crate() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn aucun_autre_crate_ne_parle_a_un_runtime_de_containers() {
    let root = workspace_root();
    let markers = runtime_markers();
    let mut offenders = BTreeSet::new();

    let own = own_crate();
    let mut examined = 0_usize;

    for area in ["packages", "apps"] {
        for (location, text) in rust_sources(&root.join(area)) {
            // Ce paquet-ci est le seul auquel c'est permis : c'est toute la décision de l'ADR 0004.
            // L'exclusion se fait sur un **préfixe de chemin** et non sur une sous-chaîne : un nom
            // de crate cherché n'importe où dans le chemin exclurait tout ce qui est balayé depuis
            // un chemin qui le contient.
            if Path::new(&location).starts_with(&own) {
                continue;
            }
            examined += 1;
            for line in text.lines() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("//") || trimmed.starts_with('*') {
                    continue;
                }
                for marker in &markers {
                    if line.contains(marker.as_str()) {
                        offenders.insert(format!("{location} : {marker}"));
                    }
                }
                if let Some(path) = connects_to_runtime_socket(line) {
                    offenders.insert(format!("{location} : connexion vers {path}"));
                }
            }
        }
    }

    // Le décompte fait partie de la garde, pas d'un test à côté : une garde qui n'examine rien
    // passe toujours, et c'est exactement ce qui était arrivé ici.
    assert!(
        examined > 20,
        "la garde n'a examiné que {examined} fichiers hors de son propre crate : elle ne garde rien"
    );
    assert!(
        offenders.is_empty(),
        "`locusd` ne détient jamais de socket de runtime, et personne d'autre non plus : {offenders:#?}"
    );
}

#[test]
fn le_balayage_des_sockets_attrape_un_acte() {
    // Sans ce test, la garde pourrait chercher des motifs qui n'existent nulle part et rester verte
    // pour de mauvaises raisons — c'est ce qui était arrivé à la règle 1 de `boundaries.json`,
    // verte sur zéro fichier jusqu'à W1.a.
    let markers = runtime_markers();
    for act in [
        format!(
            "let client = {}::Docker::connect_with_socket_defaults()?;",
            "bollard"
        ),
        format!("let host = std::env::var(\"{}{}\")?;", "DOCKER_", "HOST"),
        format!(
            "{}{}\").arg(\"ps\").output()?;",
            "Command::new(\"", "docker"
        ),
    ] {
        assert!(
            markers.iter().any(|marker| act.contains(marker.as_str())),
            "un acte réel doit être reconnu : {act}"
        );
    }

    // La quatrième façon : le chemin en dur, exigeant l'acte **et** la cible.
    for act in [
        format!(
            "let stream = {}::connect(\"/var/run/{}\")?;",
            "UnixStream", "docker.sock"
        ),
        format!(
            "{}::connect(\"/run/{}/{}\")",
            "UnixStream", "podman", "podman.sock"
        ),
    ] {
        assert!(
            connects_to_runtime_socket(&act).is_some(),
            "une connexion vers un runtime doit être reconnue : {act}"
        );
    }
}

/// **Ouvrir une socket Unix n'est pas parler à un runtime** — `W4.h`.
///
/// C'est la moitié que le proxy retiré confondait, et celle qui a fait crier la garde sur le crate
/// dont la raison d'être est de tenir `locusd` loin du runtime. Sans ce test, rien n'empêcherait de
/// remettre le proxy « pour être sûr », et la garde recommencerait à accuser ce qui est juste.
#[test]
fn ouvrir_une_socket_du_depot_n_est_pas_parler_a_un_runtime() {
    let markers = runtime_markers();
    for legitime in [
        format!("let stream = {}::connect(&self.path)?;", "UnixStream"),
        format!(
            "use std::{}::{{UnixListener, UnixStream}};",
            "os::unix::net"
        ),
        format!(
            "{}::connect(\"/run/locus/{}\")",
            "UnixStream", "broker.sock"
        ),
    ] {
        assert!(
            !markers
                .iter()
                .any(|marker| legitime.contains(marker.as_str())),
            "le lien de l'ADR 0028 n'est pas un dialogue avec un runtime : {legitime}"
        );
        assert!(
            connects_to_runtime_socket(&legitime).is_none(),
            "aucune cible de runtime n'est nommée : {legitime}"
        );
    }
}

#[test]
fn nommer_un_socket_pour_le_refuser_n_est_pas_en_parler() {
    // La politique de W4.a cite ces chemins dans sa liste d'interdits. Les signaler forcerait à
    // exempter le paquet qui écrit la politique de sécurité, c'est-à-dire à trouer la garde là où
    // elle sert le plus.
    let markers = runtime_markers();
    for naming in [
        format!("    \"{}\",", "docker.sock"),
        format!("    \"{}\",", "/var/run/docker"),
        "let spec = SandboxSpec::new(level, profile, network, mounts, resources)?;".to_owned(),
    ] {
        assert!(
            !markers
                .iter()
                .any(|marker| naming.contains(marker.as_str())),
            "nommer n'est pas parler : {naming}"
        );
    }
}
