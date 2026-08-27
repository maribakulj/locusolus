//! Test de sortie de `W5.af.1` — **la traduction d'un plan de confinement en arguments
//! `bubblewrap`**, et ce que ce mécanisme n'applique pas.
//!
//! # Ce que cette tranche livre, et ce qu'elle ne livre pas
//!
//! Elle livre la **traduction** : `ConfinementPlan` → arguments `bwrap`, pure et testable sans hôte,
//! plus ce qu'une invocation ne portera pas. C'est la pièce sur laquelle un backend s'appuiera, et
//! elle est finie.
//!
//! Elle ne livre pas le backend — `create`, `start`, `attestation` — ni la campagne. Deux questions
//! restent nommées dans la ligne de `W5.af` : ce que `persist_after_teardown` mesure quand le
//! démontage **est** la sortie du processus, et ce que `attestation()` rapporte pour un mécanisme
//! sans conteneur à réinspecter.
//!
//! # Deux tests lancent `bwrap` pour de vrai, ou le disent
//!
//! Les autres sont purs : ils vérifient qu'on écrit ce qu'on croit écrire. Deux prennent la sortie et
//! la donnent au vrai programme — parce qu'une traduction juste sur le papier et fausse à l'exécution
//! est exactement ce qu'un test pur ne peut pas voir. Sans `bwrap` sur l'hôte, ils **échouent en le
//! disant** plutôt que de passer.

use locus_execd::linux::bubblewrap::{
    BACKEND, PROGRAM, obtained_namespaces, uncreatable_targets, unenforced, wrap_arguments,
};
use locus_execd::linux::plan::{Namespace, NetworkPosture, SeccompPosture, plan};
use locus_execution::{
    Mount, MountMode, NetworkMode, ResourceSpec, SandboxLevel, SandboxProfile, SandboxSpec,
};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

/// Une cible de montage qui **existe déjà sur l'hôte**.
///
/// Sous `--ro-bind / /`, `bubblewrap` ne peut pas créer un point de montage — mesuré, et `--dir`
/// échoue pareil. Une fixture qui viserait `/travail` échouerait donc au lancement, pour une raison
/// étrangère à ce qu'on éprouve. `uncreatable_targets` existe précisément pour que ce cas se dise au
/// lieu de se découvrir dans un message de `bwrap`.
const CIBLE_EXISTANTE: &str = "/mnt";

/// La source du montage, **qui existe réellement**.
///
/// Les tests purs se contenteraient d'un chemin inventé ; les deux qui lancent `bwrap` non — il
/// refuse une source absente, et un test qui lirait ce refus comme « la racine est en lecture
/// seule » conclurait juste pour une raison fausse. C'est le genre d'accord fortuit qui survit
/// longtemps.
fn source_de_travail() -> String {
    let chemin =
        std::path::PathBuf::from(std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".to_owned()))
            .join(format!("locus-w5af-travail-{}", std::process::id()));
    std::fs::create_dir_all(&chemin).expect("la source du montage se crée");
    chemin.to_string_lossy().into_owned()
}

/// Une mission ordinaire, avec un montage inscriptible.
///
/// **Sans quota disque**, et c'est `W5.j` qui l'impose plutôt qu'un choix de commodité : un quota ne
/// s'applique qu'à une racine inscriptible, donc en dessous de `S2`. Au-delà, `plan` refuse — d'abord
/// `QuotaWithoutWritableSpace` sans montage, puis `DiskQuotaNotEnforceable` avec un bind d'hôte, qui
/// ne porte pas de quota non plus. Les deux refus sont justes, et une fixture qui les contournerait
/// éprouverait un plan que le dépôt ne produit pas.
///
/// Le montage sert la traduction : c'est lui qui fait apparaître un `--bind` distinct de la racine.
fn spec(level: SandboxLevel) -> SandboxSpec {
    let network = if level >= SandboxLevel::S3 {
        NetworkMode::Deny
    } else {
        NetworkMode::Full
    };
    let travail = Mount::new(&source_de_travail(), CIBLE_EXISTANTE, MountMode::ReadWrite)
        .expect("montage licite");
    SandboxSpec::new(
        level,
        SandboxProfile::MathCompute,
        network,
        vec![travail],
        ResourceSpec::new(2_000, 4 << 30, 256, 0, 600).expect("quotas non nuls"),
    )
    .expect("une spec valide")
}

/// Une mission `S1` **avec** quota disque — le seul niveau où il s'applique.
///
/// Elle existe pour que la clause « le quota n'est pas porté par bubblewrap » soit éprouvée sur un
/// plan qui en demande réellement un, et non affirmée sur un plan qui n'en a jamais eu.
fn spec_avec_quota_disque() -> SandboxSpec {
    SandboxSpec::new(
        SandboxLevel::S1,
        SandboxProfile::MathCompute,
        NetworkMode::Full,
        Vec::new(),
        ResourceSpec::new(2_000, 4 << 30, 256, 10 << 30, 600).expect("quotas non nuls"),
    )
    .expect("une spec valide")
}

fn arguments(level: SandboxLevel) -> Vec<String> {
    let confinement = plan(&spec(level)).expect("le plan se calcule");
    wrap_arguments(&confinement, &["/bin/true".to_owned()])
}

fn position(arguments: &[String], aiguille: &str) -> Option<usize> {
    arguments.iter().position(|argument| argument == aiguille)
}

fn bwrap_disponible() -> bool {
    std::process::Command::new(PROGRAM)
        .arg("--version")
        .output()
        .is_ok()
}

// ---------------------------------------------------------------------------------------------
// Ce que la traduction produit
// ---------------------------------------------------------------------------------------------

/// La commande vient **après un `--`**, et le séparateur est rendu par la fonction.
///
/// Un appelant qui devrait l'ajouter lui-même finirait par l'oublier, et sa commande serait lue comme
/// des options de `bwrap` — un `--tmpfs` dans un nom de fichier deviendrait un montage.
#[test]
fn la_commande_est_separee_des_options() {
    let arguments = arguments(SandboxLevel::S2);
    let separateur = position(&arguments, "--").expect("le séparateur est posé");

    assert_eq!(
        arguments.get(separateur + 1).map(String::as_str),
        Some("/bin/true"),
        "la commande suit immédiatement le séparateur"
    );
    assert_eq!(separateur + 2, arguments.len(), "et rien ne la suit");
}

/// `--die-with-parent` est posé **toujours**.
///
/// Une sandbox qui survivrait au processus qui l'a demandée est une fuite, et c'est la seule option
/// que ce module ajoute d'office.
#[test]
fn la_sandbox_meurt_avec_qui_l_a_demandee() {
    for level in [SandboxLevel::S1, SandboxLevel::S2, SandboxLevel::S3] {
        assert!(
            position(&arguments(level), "--die-with-parent").is_some(),
            "niveau {level:?} : une sandbox orpheline est une fuite"
        );
    }
}

/// La racine suit ce que le plan dit, et pas l'inverse.
///
/// Le test lit le plan plutôt que de coder le niveau en dur, sans quoi il vérifierait sa propre
/// hypothèse au lieu de la traduction.
#[test]
fn la_racine_suit_le_plan() {
    for level in [SandboxLevel::S1, SandboxLevel::S2, SandboxLevel::S3] {
        let confinement = plan(&spec(level)).expect("le plan se calcule");
        let attendu = if confinement.read_only_rootfs() {
            "--ro-bind"
        } else {
            "--bind"
        };
        let arguments = wrap_arguments(&confinement, &["/bin/true".to_owned()]);
        let racine = position(&arguments, attendu).expect("la racine est montée");
        assert_eq!(arguments.get(racine + 1).map(String::as_str), Some("/"));
        assert_eq!(arguments.get(racine + 2).map(String::as_str), Some("/"));
    }
}

/// **`Mount` n'a pas de drapeau, et n'en reçoit pas un voisin.**
///
/// `bubblewrap` crée toujours un namespace de montage, et il n'existe pas de `--unshare-mount`.
/// Rendre un drapeau voisin pour ne pas rendre `None` aurait retiré un namespace que personne n'a
/// demandé : la faute la plus discrète qu'un traducteur d'options puisse commettre, parce qu'elle
/// confine **plus** que demandé et ne casse donc rien visiblement.
///
/// La première rédaction de ce module donnait `--unshare-pid` à `Mount`.
#[test]
fn le_namespace_de_montage_n_emprunte_pas_le_drapeau_d_un_autre() {
    let confinement = plan(&spec(SandboxLevel::S1)).expect("le plan se calcule");
    let demande_mount = confinement.namespaces().contains(&Namespace::Mount);
    let demande_pid = confinement.namespaces().contains(&Namespace::Pid);
    let arguments = wrap_arguments(&confinement, &["/bin/true".to_owned()]);

    assert!(
        demande_mount,
        "la fixture demande bien un namespace de montage"
    );
    assert!(
        position(&arguments, "--unshare-mount").is_none(),
        "ce drapeau n'existe pas dans bubblewrap"
    );
    assert_eq!(
        position(&arguments, "--unshare-pid").is_some(),
        demande_pid,
        "`--unshare-pid` n'apparaît que si le plan demande `Pid` — jamais parce que `Mount` a été \
         traduit de travers"
    );

    // Et il est bien **obtenu**, sans avoir été demandé sur la ligne de commande.
    assert!(obtained_namespaces(&confinement).contains(&Namespace::Mount));
}

/// Le réseau est retiré **exactement quand le plan le demande**, et jamais deux fois.
///
/// # Une borne haute seule ne dit rien
///
/// La première rédaction n'affirmait que « au plus une fois ». Une traduction qui n'aurait **jamais**
/// écrit `--unshare-net` l'aurait satisfaite — et un mutant l'a montré, une fois le harnais de
/// mutation réparé. Une assertion « au plus » a besoin de son « au moins ».
#[test]
fn le_reseau_est_retire_exactement_quand_il_le_faut() {
    for level in [SandboxLevel::S1, SandboxLevel::S2, SandboxLevel::S3] {
        let confinement = plan(&spec(level)).expect("le plan se calcule");
        let arguments = wrap_arguments(&confinement, &["/bin/true".to_owned()]);
        let combien = arguments
            .iter()
            .filter(|argument| *argument == "--unshare-net")
            .count();

        let attendu = usize::from(!matches!(confinement.network(), NetworkPosture::Host));
        assert_eq!(
            combien,
            attendu,
            "niveau {level:?}, posture {:?} : `--unshare-net` écrit {combien} fois, attendu {attendu}",
            confinement.network()
        );
    }
}

/// **L'invariant dont la traduction dépend**, épinglé sur toutes les paires constructibles.
///
/// `wrap_arguments` ne retire plus le réseau lui-même : la boucle des namespaces le fait. Cela n'est
/// juste que si une posture non-`Host` implique **toujours** `Namespace::Network` — vérifié ici sur
/// chaque paire (niveau, mode) que `plan` accepte, et non supposé.
///
/// Si `plan.rs` cessait un jour de tenir cet accord, c'est ce test qui rougirait, et non la
/// traduction qui laisserait passer un réseau en silence.
#[test]
fn une_posture_non_hote_implique_toujours_le_namespace_reseau() {
    let mut examinees = 0_usize;
    for level in [
        SandboxLevel::S0,
        SandboxLevel::S1,
        SandboxLevel::S2,
        SandboxLevel::S3,
        SandboxLevel::S4,
    ] {
        for mode in [
            NetworkMode::Full,
            NetworkMode::Deny,
            NetworkMode::ConnectorOnly,
            NetworkMode::Allowlist {
                hosts: vec!["example.org".to_owned()],
            },
        ] {
            let Ok(spec) = SandboxSpec::new(
                level,
                SandboxProfile::MathCompute,
                mode.clone(),
                Vec::new(),
                ResourceSpec::new(2_000, 4 << 30, 256, 0, 600).expect("quotas non nuls"),
            ) else {
                continue;
            };
            let Ok(confinement) = plan(&spec) else {
                continue;
            };
            examinees += 1;

            if !matches!(confinement.network(), NetworkPosture::Host) {
                assert!(
                    confinement.namespaces().contains(&Namespace::Network),
                    "{level:?} / {mode:?} : posture non-`Host` sans `Namespace::Network` — la \
                     traduction laisserait le réseau en place"
                );
            }
        }
    }
    assert!(
        examinees >= 4,
        "{examinees} paires examinées : le balayage n'a rien vu, et son verdict ne vaut rien"
    );
}

/// Et les deux postures sont bien **exercées** par les fixtures.
///
/// Sans ce test, le précédent pourrait ne voir qu'un seul côté : trois niveaux qui donneraient tous
/// la même posture rendraient sa comparaison vraie sans rien départager.
#[test]
fn les_fixtures_exercent_les_deux_postures_de_reseau() {
    let postures: Vec<bool> = [SandboxLevel::S1, SandboxLevel::S2, SandboxLevel::S3]
        .into_iter()
        .map(|level| {
            matches!(
                plan(&spec(level)).expect("le plan se calcule").network(),
                NetworkPosture::Host
            )
        })
        .collect();

    assert!(
        postures.contains(&true) && postures.contains(&false),
        "les fixtures doivent couvrir un plan qui garde le réseau et un qui le retire : {postures:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// Ce que le mécanisme n'applique pas — et qui se dit
// ---------------------------------------------------------------------------------------------

/// **Les limites cgroup ne sont pas appliquées, et la liste le nomme.**
///
/// Ce n'est pas une option manquante : `bubblewrap` compose des namespaces et des montages, et n'a
/// aucun cgroup. Une attestation qui annoncerait le niveau sans dire cela affirmerait un confinement
/// que le mécanisme n'applique pas — ce que l'ADR 0035 refuse.
#[test]
fn les_limites_cgroup_sont_nommees_comme_non_appliquees() {
    let confinement = plan(&spec(SandboxLevel::S2)).expect("le plan se calcule");
    assert!(
        !confinement.cgroup().is_empty(),
        "la fixture réserve bien des ressources, sinon le test ne dirait rien"
    );

    let manquantes = unenforced(&confinement);
    for limite in confinement.cgroup() {
        assert!(
            manquantes.iter().any(|nommee| nommee.limit == limite.file),
            "« {} » est écrite par le plan et n'est pas nommée comme non appliquée",
            limite.file
        );
    }
    assert!(
        manquantes.iter().all(|nommee| !nommee.because.is_empty()),
        "chaque manque dit pourquoi"
    );
}

/// **Le quota disque est nommé lui aussi**, sur le seul niveau où le plan en produit un.
///
/// Sans ce test, la branche `disk_bytes` ne serait jamais atteinte : la fixture ordinaire n'en
/// demande pas, parce que `W5.j` ne le permet pas au-delà de `S1`.
#[test]
fn le_quota_disque_est_nomme_comme_non_applique() {
    let confinement = plan(&spec_avec_quota_disque()).expect("le plan se calcule");
    assert!(
        confinement.disk_bytes() > 0,
        "la fixture demande bien un quota, sinon le test ne dirait rien"
    );

    let manquantes = unenforced(&confinement);
    assert!(
        manquantes.iter().any(|nommee| nommee.limit == "disk_bytes"),
        "le quota est demandé et n'est pas nommé comme non appliqué : {manquantes:?}"
    );
}

/// **Le filtre d'appels système est nommé lui aussi**, sur un plan qui en demande un.
///
/// Trouvé par un mutant, une fois le harnais réparé : aucune fixture n'exerçait une posture autre
/// qu'`Unconfined`, donc la branche n'était jamais atteinte et sa suppression ne faisait rougir
/// personne.
///
/// `bwrap --seccomp` prend un **descripteur** vers un filtre BPF déjà compilé, pas un nom de profil.
/// Une génération d'arguments pure ne peut pas en fournir un, et omettre l'option en silence
/// reviendrait à taire le filtre absent.
#[test]
fn le_filtre_seccomp_est_nomme_comme_non_applique() {
    let confine = [SandboxLevel::S1, SandboxLevel::S2, SandboxLevel::S3]
        .into_iter()
        .map(|level| plan(&spec(level)).expect("le plan se calcule"))
        .find(|confinement| !matches!(confinement.seccomp(), SeccompPosture::Unconfined))
        .expect("au moins un niveau demande un filtre, sinon la clause n'a rien à éprouver");

    let manquantes = unenforced(&confine);
    assert!(
        manquantes.iter().any(|nommee| nommee.limit == "seccomp"),
        "un filtre est demandé et n'est pas nommé comme non appliqué : {manquantes:?}"
    );
}

/// **Une mission qui ne réserve rien ne produit aucun manque**, et c'est la contre-épreuve.
///
/// Sans elle, une implémentation qui rendrait une liste constante satisferait les deux tests
/// précédents.
#[test]
fn un_plan_sans_reservation_ne_produit_aucun_manque() {
    // `S0` **sans montage** : le plan refuse un montage sans namespace — `MountsNeedNamespace` —, et
    // ce refus est juste. La fixture ordinaire en porte un, donc elle ne convient pas ici.
    let nu = SandboxSpec::new(
        SandboxLevel::S0,
        SandboxProfile::MathCompute,
        NetworkMode::Full,
        Vec::new(),
        ResourceSpec::new(2_000, 4 << 30, 256, 0, 600).expect("quotas non nuls"),
    )
    .expect("une spec valide");
    let confinement = plan(&nu).expect("le plan se calcule");
    let manquantes = unenforced(&confinement);

    let rien_reserve = confinement.cgroup().is_empty()
        && confinement.disk_bytes() == 0
        && matches!(confinement.seccomp(), SeccompPosture::Unconfined);
    assert_eq!(
        manquantes.is_empty(),
        rien_reserve,
        "la liste est vide exactement quand rien n'est réservé : {manquantes:?}"
    );
}

/// Le nom du mécanisme est celui du protocole.
#[test]
fn le_mecanisme_porte_le_nom_du_protocole() {
    assert_eq!(BACKEND, "bubblewrap");
    assert_ne!(
        BACKEND, PROGRAM,
        "le programme s'appelle `bwrap`, pas le mécanisme"
    );
}

// ---------------------------------------------------------------------------------------------
// La différence réelle avec podman
// ---------------------------------------------------------------------------------------------

/// **Une cible que l'hôte ne porte pas est signalée**, au lieu d'échouer dans un message de `bwrap`.
///
/// `podman` bâtit une racine neuve depuis une image et y crée n'importe quel point de montage ;
/// `bubblewrap` compose une **vue de la racine de l'hôte**, et sous `--ro-bind / /` il ne peut rien y
/// créer — mesuré, `Can't mkdir: Read-only file system`, et `--dir` échoue pareil.
#[test]
fn une_cible_absente_de_l_hote_est_signalee() {
    let confinement = plan(&spec(SandboxLevel::S2)).expect("le plan se calcule");
    assert!(
        confinement.read_only_rootfs(),
        "la question ne se pose que sous racine en lecture seule"
    );

    let manquantes = uncreatable_targets(&confinement, |_| false);
    assert_eq!(manquantes.len(), 1);
    assert_eq!(manquantes[0].target, CIBLE_EXISTANTE);

    assert!(uncreatable_targets(&confinement, |_| true).is_empty());

    // Et avec le vrai système de fichiers, la fixture passe — sinon les deux tests vivants
    // échoueraient pour cette raison-là plutôt que pour ce qu'ils éprouvent.
    assert!(
        uncreatable_targets(&confinement, |chemin| std::path::Path::new(chemin).is_dir())
            .is_empty(),
        "la fixture vise une cible qui existe"
    );
}

/// Sous racine **inscriptible**, la question ne se pose pas.
///
/// `bubblewrap` peut alors créer le point de montage, et signaler une cible absente serait crier sur
/// ce qui est juste — la leçon de `W22.d`.
#[test]
fn sous_racine_inscriptible_aucune_cible_n_est_signalee() {
    let confinement = plan(&spec(SandboxLevel::S1)).expect("le plan se calcule");
    assert!(!confinement.read_only_rootfs());
    assert!(uncreatable_targets(&confinement, |_| false).is_empty());
}

// ---------------------------------------------------------------------------------------------
// Et la traduction est exercée pour de vrai
// ---------------------------------------------------------------------------------------------

/// **Les arguments produits sont acceptés par le vrai programme**, ou le test le dit.
#[test]
fn les_arguments_produits_confinent_reellement() {
    assert!(
        bwrap_disponible(),
        "NON MESURÉ : « {PROGRAM} » est introuvable sur cet hôte, donc la traduction n'a été \
         vérifiée que sur le papier. L'installer, ou porter cette vérification là où le worker \
         tourne. Ce test échoue plutôt que de passer."
    );

    let confinement = plan(&spec(SandboxLevel::S2)).expect("le plan se calcule");
    let arguments = wrap_arguments(
        &confinement,
        &["/bin/sh".to_owned(), "-c".to_owned(), "hostname".to_owned()],
    );

    let execution = std::process::Command::new(PROGRAM)
        .args(&arguments)
        .output()
        .expect("bwrap se lance");

    assert!(
        execution.status.success(),
        "bwrap a refusé les arguments produits : {}",
        String::from_utf8_lossy(&execution.stderr)
    );
    assert!(
        !String::from_utf8_lossy(&execution.stdout).trim().is_empty(),
        "la commande a bien tourné dans la sandbox"
    );
}

/// **La racine en lecture seule l'est réellement.**
///
/// La vérification qui compte : `--ro-bind / /` doit refuser une écriture. Un test qui lirait
/// seulement les arguments accepterait une traduction qui monte la racine en écriture.
#[test]
fn la_racine_en_lecture_seule_refuse_une_ecriture() {
    assert!(
        bwrap_disponible(),
        "NON MESURÉ : « {PROGRAM} » est introuvable sur cet hôte"
    );

    let confinement = plan(&spec(SandboxLevel::S2)).expect("le plan se calcule");
    assert!(
        confinement.read_only_rootfs(),
        "la fixture demande bien une racine en lecture seule"
    );

    // Un témoin **propre à cette exécution**. La première rédaction visait `/essai-w5af`, un chemin
    // global : un run antérieur l'a laissé sur l'hôte, et le suivant a échoué sur une affirmation
    // devenue fausse pour une raison qui ne le concernait pas. Un test qui dépend de l'état laissé
    // par ses prédécesseurs ne mesure plus ce qu'il annonce.
    let temoin = format!("/essai-w5af-{}", std::process::id());
    assert!(
        !std::path::Path::new(&temoin).exists(),
        "le témoin est neuf : sinon son absence finale ne prouverait rien"
    );

    let arguments = wrap_arguments(
        &confinement,
        &[
            "/bin/sh".to_owned(),
            "-c".to_owned(),
            format!("echo essai > {temoin} 2>/dev/null && echo ECRIT || echo REFUSE"),
        ],
    );

    let execution = std::process::Command::new(PROGRAM)
        .args(&arguments)
        .output()
        .expect("bwrap se lance");
    let sortie = String::from_utf8_lossy(&execution.stdout).trim().to_owned();

    assert_eq!(
        sortie,
        "REFUSE",
        "la racine annoncée en lecture seule a accepté une écriture : {}",
        String::from_utf8_lossy(&execution.stderr)
    );
    assert!(
        !std::path::Path::new(&temoin).exists(),
        "et rien n'a fui vers l'hôte"
    );
}
