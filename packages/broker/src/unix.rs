//! Le tube local : le client, l'écoute, et la barrière à l'entrée — ADR 0028 décisions 1 et 2.
//!
//! # La barrière, et ce qu'une seconde aurait ajouté — ou pas
//!
//! La socket est créée en `0600`, dans un répertoire `0700`. Sur une socket de domaine Unix, ces
//! permissions sont **appliquées au `connect`** : seul le propriétaire peut s'y brancher. Ce n'est
//! pas une convention, c'est le noyau.
//!
//! L'ADR 0028 devait porter une seconde barrière — la créance de pair, `SO_PEERCRED` — présentée
//! comme gratuite et comme la vraie protection. **Les deux moitiés de cette phrase étaient
//! fausses**, et la vérification à l'écriture l'a montré :
//!
//! 1. **Elle n'est pas gratuite.** `UnixStream::peer_cred` est instable sur Rust stable, et
//!    `unsafe_code = "forbid"` dans les lints d'espace de travail ne se contourne ni par `allow` ni
//!    par `expect` — c'est le sens de `forbid`. L'obtenir demande donc un crate externe **dans le
//!    processus privilégié**, ce que tout le reste de cet ADR passe son temps à éviter.
//! 2. **Elle n'ajoute rien aujourd'hui.** La politique envisagée était « le même utilisateur que le
//!    broker ». Or `0600` admet déjà exactement cet ensemble-là. Deux barrières qui laissent passer
//!    les mêmes appelants ne sont pas une défense en profondeur, c'est une redondance qui coûte une
//!    dépendance.
//!
//! Elle commencerait à séparer quelque chose le jour où `locusd` et `locus-execd` tournent sous
//! **deux utilisateurs différents** — socket en `0660` avec un groupe partagé, et le broker
//! vérifiant que l'appelant est bien l'uid de `locusd`, pas le sien. Ce jour-là la politique cesse
//! d'être « le même » pour devenir « celui-là », et c'est `W4.i`, dont le test de sortie est
//! précisément que les deux barrières admettent des ensembles **différents**.
//!
//! # Le refus se dit sur le fil, il ne coupe pas la connexion
//!
//! Un appelant qui parle une autre version reçoit un [`Verdict::Refused`] et non une fermeture
//! sèche. Sans cela, la première mise en service se passerait à chercher une panne de réseau qui
//! n'existe pas — ADR 0028 décision 4, où « injoignable » et « refusé » sont deux choses.
//!
//! # Ce que ce module n'a pas
//!
//! Pas de runtime asynchrone, pas de file, pas de parallélisme : le broker traite une connexion à la
//! fois. Écrire un ordonnanceur pour un lien qui porte une question de démarrage serait de
//! l'abstraction spéculative. La borne qui **est** posée est celle qui protège, et elle est dans
//! [`crate::frame`].

use std::io::BufReader;
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use locus_lep::{CapabilityManifest, ResourceSpec, SandboxSpec};

use crate::frame::{FrameError, read_frame, write_frame};
use crate::port::{BrokerError, BrokerPort, Placement, as_placement};
use crate::protocol::{PROTOCOL, Request, Response, Verdict};

/// Les permissions de la socket : lecture et écriture pour son seul propriétaire.
pub const SOCKET_MODE: u32 = 0o600;

/// Les permissions du répertoire qui la porte : traversable par son seul propriétaire.
///
/// Le mode de la socket suffirait à refuser le `connect`, mais un répertoire traversable laisse
/// **énumérer** ce qui s'y trouve. Un broker dont l'existence et le chemin sont lisibles par tous
/// donne gratuitement la moitié d'une reconnaissance.
pub const DIRECTORY_MODE: u32 = 0o700;

/// Le client : `locusd` parle au broker par ce chemin.
#[derive(Debug, Clone)]
pub struct UnixSocketBroker {
    path: PathBuf,
}

impl UnixSocketBroker {
    /// Un client qui parlera à cette socket.
    ///
    /// Rien n'est ouvert ici : un lien qui se connecterait à la construction rendrait impossible de
    /// construire un `locusd` pendant que le broker est éteint, ce que la décision 4 exige.
    #[must_use]
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Où ce client parle.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn ask(&self, request: &Request) -> Result<Verdict, BrokerError> {
        let stream = UnixStream::connect(&self.path).map_err(|error| BrokerError::Unreachable {
            endpoint: self.path.display().to_string(),
            why: error.to_string(),
        })?;
        let mut writer = &stream;
        write_frame(&mut writer, request).map_err(|error| self.transport(&error))?;
        let mut reader = BufReader::new(&stream);
        let response: Response = read_frame(&mut reader).map_err(|error| self.transport(&error))?;
        if response.protocol != PROTOCOL {
            return Err(BrokerError::Malformed {
                why: format!(
                    "le broker parle {} et ce client parle {PROTOCOL}",
                    response.protocol
                ),
            });
        }
        Ok(response.verdict)
    }

    /// Traduire un défaut de cadre en défaut de lien, **sans les aplatir**.
    ///
    /// Une ligne trop longue et une ligne illisible ne se soignent pas de la même façon, et une
    /// fermeture en cours d'échange est un fait de lien : la confondre avec une réponse mal formée
    /// enverrait relire un protocole quand il faut relire un journal de service.
    fn transport(&self, error: &FrameError) -> BrokerError {
        match error {
            FrameError::TooLong { read } => BrokerError::TooLong { read: *read },
            FrameError::Malformed { why } => BrokerError::Malformed { why: why.clone() },
            FrameError::Closed => BrokerError::Unreachable {
                endpoint: self.path.display().to_string(),
                why: "le broker a fermé sans répondre".to_owned(),
            },
            FrameError::Io { why } => BrokerError::Unreachable {
                endpoint: self.path.display().to_string(),
                why: why.clone(),
            },
        }
    }
}

impl BrokerPort for UnixSocketBroker {
    fn endpoint(&self) -> String {
        self.path.display().to_string()
    }

    fn readiness(&self) -> Result<Verdict, BrokerError> {
        self.ask(&Request::readiness())
    }

    fn place(
        &self,
        manifest: &CapabilityManifest,
        sandbox: &SandboxSpec,
        resources: &ResourceSpec,
    ) -> Result<Placement, BrokerError> {
        as_placement(self.ask(&Request::place(
            manifest.clone(),
            sandbox.clone(),
            resources.clone(),
        ))?)
    }
}

/// Ce qui peut empêcher le broker d'écouter.
#[derive(Debug)]
pub enum ListenError {
    /// La socket n'a pas pu être créée.
    Bind {
        /// Le chemin visé.
        path: PathBuf,
        /// Ce que le système en a dit.
        why: String,
    },
    /// Les permissions n'ont pas pu être posées, sur la socket ou sur son répertoire.
    ///
    /// Distinct de [`ListenError::Bind`] : une socket ouverte au monde est **pire** qu'une absence de
    /// socket, donc l'échec est fatal ici plutôt que journalisé.
    Permissions {
        /// Le chemin concerné.
        path: PathBuf,
        /// Ce que le système en a dit.
        why: String,
    },
}

impl std::fmt::Display for ListenError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bind { path, why } => {
                write!(
                    formatter,
                    "écoute impossible sur {} — {why}",
                    path.display()
                )
            }
            Self::Permissions { path, why } => write!(
                formatter,
                "permissions impossibles à poser sur {} — {why} ; une socket ouverte au monde est \
                 pire qu'une absence de socket",
                path.display()
            ),
        }
    }
}

impl std::error::Error for ListenError {}

/// Ouvrir la socket d'écoute, avec ses permissions et celles de son répertoire.
///
/// # Le fichier résiduel
///
/// Une socket dont le processus est mort laisse son fichier derrière elle, et `bind` échoue dessus.
/// On le retire **uniquement** s'il s'agit bien d'une socket : effacer un fichier ordinaire parce
/// qu'il porte le nom qu'on voulait serait une destruction de donnée décidée par un daemon, et le
/// chemin de la socket est en général donné par une configuration qu'un humain a écrite.
///
/// # Errors
///
/// [`ListenError::Bind`] si la socket ne s'ouvre pas, [`ListenError::Permissions`] si un mode ne se
/// pose pas.
pub fn listen(path: &Path) -> Result<UnixListener, ListenError> {
    if let Some(directory) = path.parent() {
        if directory.as_os_str().is_empty() {
            // Chemin relatif nu : le répertoire est le courant, qui n'appartient pas à ce module.
        } else {
            std::fs::create_dir_all(directory).map_err(|error| ListenError::Bind {
                path: directory.to_path_buf(),
                why: error.to_string(),
            })?;
            std::fs::set_permissions(directory, std::fs::Permissions::from_mode(DIRECTORY_MODE))
                .map_err(|error| ListenError::Permissions {
                    path: directory.to_path_buf(),
                    why: error.to_string(),
                })?;
        }
    }
    if let Ok(existing) = std::fs::symlink_metadata(path)
        && existing.file_type().is_socket()
    {
        let _ = std::fs::remove_file(path);
    }
    let listener = UnixListener::bind(path).map_err(|error| ListenError::Bind {
        path: path.to_path_buf(),
        why: error.to_string(),
    })?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(SOCKET_MODE)).map_err(
        |error| ListenError::Permissions {
            path: path.to_path_buf(),
            why: error.to_string(),
        },
    )?;
    Ok(listener)
}

/// Traiter une connexion : lire une requête, vérifier le protocole, répondre.
///
/// Le répondeur reçoit la question et rend le verdict ; il ne sait rien du transport, et ce module
/// ne sait rien des hôtes. C'est ce qui permet à `locus-execd` de fournir son `Readiness` sans que
/// ce crate connaisse `podman`, et c'est ce qui tient la quatrième frontière par le graphe de
/// paquets plutôt que par une recherche de texte.
///
/// # Errors
///
/// [`FrameError`] quand la requête ne se lit pas ou que la réponse ne s'écrit pas. Un appelant
/// refusé n'est **pas** une erreur : il reçoit un [`Verdict::Refused`], donc `Ok`.
pub fn answer<F>(stream: &UnixStream, respond: F) -> Result<(), FrameError>
where
    F: FnOnce(&Request) -> Verdict,
{
    let mut reader = BufReader::new(stream);
    let request: Request = read_frame(&mut reader)?;
    let verdict = if request.protocol == PROTOCOL {
        respond(&request)
    } else {
        Verdict::Refused {
            why: format!(
                "l'appelant parle {} et le broker parle {PROTOCOL} : un désaccord de protocole se \
                 dit, il ne s'interprète pas",
                request.protocol
            ),
        }
    };
    let mut writer = stream;
    write_frame(&mut writer, &Response::new(verdict))
}
