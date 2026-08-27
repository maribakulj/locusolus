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
//! processus qui l'a demandée est une fuite.
//!
//! # La racine se **bâtit**, elle ne s'emprunte pas
//!
//! La première rédaction montait la racine de l'hôte — `--ro-bind / /`, ou `--bind / /` quand le plan
//! n'exigeait pas la lecture seule. C'était la traduction la plus courte, et elle était fausse sur
//! quatre points, tous **mesurés** plutôt que raisonnés :
//!
//! 1. **Le home de l'utilisateur était monté**, ce que `CLAUDE.md` interdit en toutes lettres. Sous
//!    racine inscriptible — `S0`, `S1` — un `echo` depuis la sandbox vers `/home/<user>/…`
//!    **atteignait le fichier sur l'hôte**. La sonde `write_host_home` est contenue à partir de
//!    `S1` ; elle ne l'était pas.
//! 2. **Tout le système de fichiers de l'hôte était lisible**, `/etc/shadow` compris.
//!    `read_host_filesystem` et `read_host_secret_files` sont contenues à partir de `S2` ; elles ne
//!    l'étaient pas.
//! 3. **`--unshare-pid` était cosmétique** : sans remontage de `/proc`, la sandbox lisait la table
//!    des processus de l'hôte. Cent quarante entrées mesurées, contre cinq une fois `/proc` remonté.
//! 4. **`/dev/null` n'était pas inscriptible**, la racine entière étant en lecture seule. Une
//!    commande aussi banale que `… 2>/dev/null` échouait alors pour une raison étrangère au
//!    confinement, ce qu'une campagne lirait comme un verdict.
//!
//! Aucun des quatre n'était visible dans les arguments produits : ils se lisaient tous « racine en
//! lecture seule, namespaces retirés ». C'est la limite d'un test qui compare des chaînes, et c'est
//! pourquoi plusieurs tests d'ici lancent le vrai programme.
//!
//! La racine est donc composée : un `--tmpfs /` neuf, [`SYSTEM_TREE`] emprunté en lecture seule,
//! [`SYSTEM_LINKS`] pour qu'un `/usr` fusionné se retrouve, un `/proc` et un `/dev` à elle. Ce que la
//! mission veut en plus passe par ses **montages**, où cela se voit et s'atteste. Quand le plan
//! demande une racine en lecture seule, `--remount-ro /` la scelle **après** les montages — mesuré :
//! l'espace de travail posé avant reste inscriptible, et son écriture atteint bien l'hôte.
//!
//! Conséquence à ne pas perdre : sous `--ro-bind / /`, `bwrap` ne pouvait pas créer un point de
//! montage absent, et ce module portait un `uncreatable_targets` pour le signaler avant lancement.
//! Sur une racine `tmpfs`, il le crée — mesuré aussi. La garde a donc été **retirée** plutôt que
//! conservée par prudence : une garde qui crie sur ce qui est juste se fait désactiver, et celle-ci
//! n'avait plus rien à signaler.

use std::collections::BTreeSet;

use super::plan::{ConfinementPlan, Namespace, NetworkPosture, SeccompPosture};

/// Le programme.
pub const PROGRAM: &str = "bwrap";

/// Le nom du mécanisme, tel que `SandboxAttestation.backend` de `lep/1.0` l'attend.
///
/// Le mot est celui du protocole, pas un synonyme : `CLAUDE.md` interdit un vocabulaire parallèle, et
/// « mécanisme » à côté de `backend` en serait un.
pub const BACKEND: &str = "bubblewrap";

/// Le seul répertoire de l'hôte que la racine bâtie emprunte.
///
/// # Pourquoi celui-là, et pourquoi lui seul
///
/// La commande a besoin d'un interpréteur et de ses bibliothèques ; sur toute distribution à `/usr`
/// fusionné — c'est-à-dire toutes celles que ce dépôt vise —, `/usr` les porte **et rien d'autre de
/// sensible**. `/etc` n'y est pas : c'est là que vivent `/etc/shadow` et les configurations de
/// l'hôte, et la sonde `read_host_secret_files` les nomme. `/home` n'y est pas non plus, et
/// `CLAUDE.md` est explicite — « ne monte jamais le home utilisateur … dans une sandbox par
/// défaut ». Ce qu'une mission veut de plus passe par ses **montages**, où cela se voit et s'atteste.
pub const SYSTEM_TREE: &str = "/usr";

/// Les liens que la racine bâtie pose pour qu'un `/usr` fusionné se retrouve.
///
/// `(cible, lien)`, dans l'ordre où `bwrap` les recevra. Ils sont posés **sans condition** : un lien
/// pendant est inoffensif, et le rendre conditionnel demanderait de lire l'hôte, ce que ce module ne
/// fait pas — c'est ce qui le garde pur et testable sans machine.
pub const SYSTEM_LINKS: &[(&str, &str)] = &[
    ("usr/bin", "/bin"),
    ("usr/lib", "/lib"),
    ("usr/lib64", "/lib64"),
    ("usr/sbin", "/sbin"),
];

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

/// Les arguments qui enveloppent une commande dans le confinement de ce plan.
///
/// La commande vient **après** : `bwrap <arguments> -- <commande>`. Le séparateur est rendu par cette
/// fonction, de sorte qu'un appelant ne puisse pas l'oublier et voir sa commande lue comme des
/// options — un `--tmpfs` dans un nom de fichier deviendrait un montage.
#[must_use]
pub fn wrap_arguments(plan: &ConfinementPlan, command: &[String]) -> Vec<String> {
    // Ce préfixe ne dépend pas du plan : il est ce qu'une sandbox de ce mécanisme **est**, avant
    // même qu'on sache ce qu'elle doit confiner.
    //
    // - `--die-with-parent` toujours : une sandbox qui survivrait à qui l'a demandée est une fuite.
    // - La racine est **bâtie** plutôt qu'empruntée — voir [`SYSTEM_TREE`] et l'en-tête du module
    //   pour ce que la première rédaction, `--ro-bind / /`, laissait passer, et qui a été mesuré.
    // - `--proc` n'est pas un confort : sans lui, `--unshare-pid` est **cosmétique**. Mesuré — une
    //   sandbox qui retirait le namespace PID sans remonter `/proc` lisait quand même la table des
    //   processus de l'hôte, cent quarante entrées ; avec, elle en voit cinq, les siennes. La sonde
    //   `observe_host_processes` porte exactement là-dessus.
    // - `--dev` non plus. Sous une racine en lecture seule, le `/dev/null` de l'hôte est visible et
    //   **non inscriptible** : toute commande qui écrit `2>/dev/null` échoue alors pour une raison
    //   étrangère au confinement, et une campagne lirait ce refus comme un verdict. C'est la
    //   confusion entre « pas mesuré » et « refusé » que `W5.n` à `W5.q` ont mis quatre sprints à
    //   retirer du harnais ; l'y réintroduire par une racine incomplète serait la refaire.
    let mut arguments = vec![
        "--die-with-parent".to_owned(),
        "--tmpfs".to_owned(),
        "/".to_owned(),
        "--ro-bind".to_owned(),
        SYSTEM_TREE.to_owned(),
        SYSTEM_TREE.to_owned(),
    ];
    for (cible, lien) in SYSTEM_LINKS {
        arguments.push("--symlink".to_owned());
        arguments.push((*cible).to_owned());
        arguments.push((*lien).to_owned());
    }
    arguments.push("--proc".to_owned());
    arguments.push("/proc".to_owned());
    arguments.push("--dev".to_owned());
    arguments.push("/dev".to_owned());

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

    if plan.read_only_rootfs() {
        // **Après** les montages, et c'est ce qui rend la clause utilisable. Mesuré : `--remount-ro
        // /` scelle le tmpfs de la racine et laisse inscriptible un `--bind` posé avant lui —
        // l'espace de travail de la mission écrit donc toujours, et son écriture atteint bien
        // l'hôte. Posée avant les montages, la clause ne scellerait rien de ce qu'ils ajoutent ;
        // posée là, elle scelle la racine sans sceller la mission.
        arguments.push("--remount-ro".to_owned());
        arguments.push("/".to_owned());
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
