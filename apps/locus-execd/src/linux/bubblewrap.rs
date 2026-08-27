//! Les arguments de `bubblewrap` pour un plan de confinement — `W5.af.1`, ADR 0035 décision 4.
//!
//! # Pourquoi ce module existe, et ce qu'il n'est pas
//!
//! `plan.rs` produit un [`ConfinementPlan`] **neutre** : des namespaces, des limites cgroup, des
//! montages, une posture de réseau. `invocation.rs` le traduit en arguments **podman**. Ce module-ci
//! le traduit en arguments **bubblewrap**, et c'est tout ce qu'il fait — la traduction est pure,
//! testable sans hôte, et ne lance rien.
//!
//! C'est le mécanisme que `canterel` emploie, et l'ADR 0035 décision 4 dit pourquoi cela compte :
//! « `Proven` ne peut être rempli pour un worker réel que par une campagne exerçant le mécanisme que
//! ce worker emploie ». Attester `podman-rootless` pour un worker qui tourne sous `bubblewrap` est
//! le défaut `canterel-local` sous un autre nom.
//!
//! # Ce que `bubblewrap` ne sait pas faire, dit ici plutôt que découvert plus tard
//!
//! **Il n'écrit aucune limite cgroup.** Ni mémoire, ni CPU, ni PID, ni entrées/sorties : ce n'est pas
//! une option manquante, c'est hors de son objet — il compose des namespaces et des montages, et la
//! comptabilité des ressources appartient à qui l'appelle. `podman` écrit ces limites parce qu'il
//! gère un cgroup par conteneur ; `bwrap` n'en gère aucun.
//!
//! [`unenforced`] rend donc la liste des limites du plan **qu'une invocation ne portera pas**, et
//! elle n'est pas vide dès qu'une mission réserve quoi que ce soit. Deux raisons de la rendre plutôt
//! que de la taire :
//!
//! 1. Une attestation qui annoncerait le niveau sans dire cela affirmerait un confinement que le
//!    mécanisme n'applique pas — exactement ce que l'ADR 0035 refuse.
//! 2. Un exploitant qui lit « `S2` sous bubblewrap » doit pouvoir savoir que la borne mémoire de sa
//!    mission est tenue **ailleurs**, ou pas du tout.
//!
//! Ce que le module ne fait **pas** : décider quoi en conclure. Il rend le fait ; c'est l'attestation
//! qui le portera, et le placement qui en tirera un refus s'il y a lieu.
//!
//! # Le confinement, lui, se traduit
//!
//! Namespaces, racine en lecture seule, montages et réseau ont chacun leur forme en `bwrap`, et la
//! traduction est directe. `--die-with-parent` est posé **toujours** : une sandbox qui survivrait au
//! processus qui l'a demandée est une fuite, et c'est la seule option ajoutée d'office ici.

use std::collections::BTreeSet;

use super::plan::{ConfinementPlan, Namespace, NetworkPosture, SeccompPosture};

/// Le programme.
pub const PROGRAM: &str = "bwrap";

/// Le nom du mécanisme, tel que `SandboxAttestation.backend` de `lep/1.0` l'attend.
///
/// Le mot est celui du protocole, pas un synonyme : `CLAUDE.md` interdit un vocabulaire parallèle, et
/// « mécanisme » à côté de `backend` en serait un.
pub const BACKEND: &str = "bubblewrap";

/// Une limite du plan que `bubblewrap` **n'appliquera pas**.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Unenforced {
    /// La dimension, sous le **nom de fichier de contrôleur** que `plan.rs` lui donne —
    /// `memory.max`, `cpu.max`, `pids.max`.
    ///
    /// Le nom du contrôleur plutôt qu'un mot à nous : un exploitant qui lit ce rapport va ensuite
    /// chercher ce fichier, et un synonyme lui ferait chercher autre chose.
    pub limit: String,
    /// Pourquoi ce mécanisme ne la porte pas.
    pub because: &'static str,
}

/// Ce que ce plan demande et qu'une invocation `bwrap` ne tiendra pas.
///
/// Vide quand la mission ne réserve rien. Jamais « vide parce qu'on n'a pas regardé » : la liste est
/// dérivée des limites du plan, une par une.
#[must_use]
pub fn unenforced(plan: &ConfinementPlan) -> Vec<Unenforced> {
    let mut manquantes: Vec<Unenforced> = plan
        .cgroup()
        .iter()
        .map(|limit| Unenforced {
            limit: limit.file.to_owned(),
            because: "bubblewrap n'écrit aucun cgroup : il compose des namespaces et des montages, \
                      et la comptabilité des ressources appartient à qui l'appelle",
        })
        .collect();
    if !matches!(plan.seccomp(), SeccompPosture::Unconfined) {
        manquantes.push(Unenforced {
            limit: "seccomp".to_owned(),
            because: "`bwrap --seccomp` prend un descripteur vers un filtre BPF déjà compilé, pas un \
                      nom de profil : une génération d'arguments pure ne peut pas en fournir un, et \
                      omettre l'option en silence reviendrait à taire le filtre absent",
        });
    }
    if plan.disk_bytes() > 0 {
        manquantes.push(Unenforced {
            limit: "disk_bytes".to_owned(),
            because: "un quota d'espace se pose sur un système de fichiers, et bubblewrap n'en crée \
                      pas : son `--tmpfs` est borné par la mémoire de l'hôte, pas par la réservation \
                      de la mission",
        });
    }
    manquantes.sort();
    manquantes
}

/// Une cible de montage que **ce mécanisme ne peut pas créer**.
///
/// # Une différence réelle avec podman, et pas un détail de traduction
///
/// `podman` bâtit une racine neuve depuis une image : il y crée le point de montage qu'on lui
/// demande, quel qu'il soit. `bubblewrap` compose une **vue de la racine de l'hôte** ; sous
/// `--ro-bind / /`, il ne peut pas y créer un répertoire — mesuré : `Can't mkdir /travail:
/// Read-only file system`, et `--dir` échoue de la même façon.
///
/// Une cible qui n'existe pas sur l'hôte est donc **exprimable pour podman et pas pour
/// bubblewrap**. Ce n'est pas une limite à contourner en silence : un plan qui la porte échouera au
/// lancement, et il vaut mieux le savoir avant.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UncreatableTarget {
    /// La cible telle que le plan la nomme.
    pub target: String,
}

/// Les cibles de montage que l'hôte ne porte pas déjà.
///
/// # Pourquoi l'existence est **injectée**
///
/// Ce module reste pur : il ne touche pas au système de fichiers. `exists` est fourni par l'appelant,
/// ce qui rend la fonction testable sans hôte et garde la traduction déterministe — même discipline
/// que `PodmanBackend::new`, qui est `const fn` pour que construire un backend ne lise rien.
///
/// L'appelant réel passera `|chemin| std::path::Path::new(chemin).is_dir()`.
#[must_use]
pub fn uncreatable_targets(
    plan: &ConfinementPlan,
    exists: impl Fn(&str) -> bool,
) -> Vec<UncreatableTarget> {
    if !plan.read_only_rootfs() {
        // Racine inscriptible : bubblewrap peut créer le point de montage, et la question ne se pose
        // pas. Signaler une cible absente ici serait crier sur ce qui est juste — la leçon de
        // `W22.d`, qui dit qu'une garde qui le fait se fait désactiver.
        return Vec::new();
    }
    let mut manquantes: Vec<UncreatableTarget> = plan
        .mounts()
        .iter()
        .filter(|mount| !exists(&mount.target))
        .map(|mount| UncreatableTarget {
            target: mount.target.clone(),
        })
        .collect();
    manquantes.sort();
    manquantes
}

/// Les arguments qui enveloppent une commande dans le confinement de ce plan.
///
/// La commande vient **après** : `bwrap <arguments> -- <commande>`. Le séparateur est rendu par cette
/// fonction, de sorte qu'un appelant ne puisse pas l'oublier et voir sa commande lue comme des
/// options — un `--tmpfs` dans un nom de fichier deviendrait un montage.
#[must_use]
pub fn wrap_arguments(plan: &ConfinementPlan, command: &[String]) -> Vec<String> {
    let mut arguments = Vec::new();

    // Toujours : une sandbox qui survivrait à qui l'a demandée est une fuite.
    arguments.push("--die-with-parent".to_owned());

    // La racine. Sans elle, la commande n'a pas d'interpréteur à trouver.
    arguments.push(
        if plan.read_only_rootfs() {
            "--ro-bind"
        } else {
            "--bind"
        }
        .to_owned(),
    );
    arguments.push("/".to_owned());
    arguments.push("/".to_owned());

    for namespace in plan.namespaces() {
        if let Some(flag) = unshare_flag(*namespace) {
            arguments.push(flag.to_owned());
        }
    }

    // Le réseau ne se retire **pas** ici, et c'est délibéré : la boucle ci-dessus l'a déjà fait.
    //
    // Une première rédaction ajoutait un `--unshare-net` de rattrapage pour le cas d'une posture
    // non-`Host` sans `Namespace::Network`. Un mutant a montré que le retirer ne faisait rougir
    // personne, et la mesure a dit pourquoi : sur **toutes** les paires (niveau, mode) que `plan`
    // accepte, une posture non-`Host` porte toujours ce namespace. Le rattrapage était du code mort
    // déguisé en filet de sécurité — la pire forme, parce qu'on lui fait confiance.
    //
    // L'invariant dont dépend cette traduction est donc épinglé par un test, `plan.rs` étant libre
    // de changer : si une posture non-`Host` cessait d'impliquer le namespace, c'est là que ça
    // rougirait, et non ici en silence.
    //
    // Ce que bubblewrap sait faire, de toute façon : **retirer** le réseau, pas le filtrer. Un
    // connecteur ou une liste d'autorisation se tient au-dessus, et `unenforced` n'a rien à en dire
    // parce que le plan ne demande pas à ce mécanisme de les appliquer.

    for mount in plan.mounts() {
        arguments.push(
            if mount.read_only {
                "--ro-bind"
            } else {
                "--bind"
            }
            .to_owned(),
        );
        arguments.push(mount.source.clone());
        arguments.push(mount.target.clone());
    }

    if plan.no_new_privileges() {
        // `--new-session` détache le terminal de contrôle : sans lui, un processus confiné peut
        // réinjecter des caractères dans le terminal de l'appelant par `TIOCSTI`, ce qui rend le
        // confinement de sortie illusoire.
        arguments.push("--new-session".to_owned());
    }

    arguments.push("--".to_owned());
    arguments.extend(command.iter().cloned());
    arguments
}

/// Le drapeau `bwrap` qui retire ce namespace, quand il en existe un.
///
/// `None` pour [`Namespace::Mount`], et ce n'est pas un trou : `bubblewrap` crée **toujours** un
/// namespace de montage — c'est ce qu'il est —, et il n'existe pas de `--unshare-mount`. Rendre un
/// drapeau voisin « pour ne pas rendre `None` » aurait retiré un namespace que personne n'a demandé,
/// ce qui est la faute la plus discrète qu'un traducteur d'options puisse commettre : elle confine
/// **plus** que demandé, donc elle ne casse rien visiblement et survit à une relecture.
///
/// Ce que le plan demande est donc obtenu ; [`obtained_namespaces`] le dit sans passer par ici.
const fn unshare_flag(namespace: Namespace) -> Option<&'static str> {
    match namespace {
        Namespace::User => Some("--unshare-user"),
        Namespace::Mount => None,
        Namespace::Pid => Some("--unshare-pid"),
        Namespace::Ipc => Some("--unshare-ipc"),
        Namespace::Uts => Some("--unshare-uts"),
        Namespace::Network => Some("--unshare-net"),
        Namespace::Cgroup => Some("--unshare-cgroup"),
    }
}

/// Les namespaces que ce plan obtiendra réellement, pour confrontation.
///
/// `bubblewrap` crée **toujours** un namespace de montage : un plan qui demande `Mount` l'obtient
/// sans avoir de drapeau à écrire.
#[must_use]
pub fn obtained_namespaces(plan: &ConfinementPlan) -> BTreeSet<Namespace> {
    let mut obtenus: BTreeSet<Namespace> = plan.namespaces().clone();
    obtenus.insert(Namespace::Mount);
    if !matches!(plan.network(), NetworkPosture::Host) {
        obtenus.insert(Namespace::Network);
    }
    obtenus
}
