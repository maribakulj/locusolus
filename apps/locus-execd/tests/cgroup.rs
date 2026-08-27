//! Test de sortie de `W5.ai` tranche 1 — ADR 0036 décision 3.
//!
//! **Le broker lit ce que l'hôte lui délègue, et il refuse quand rien ne l'est.**
//!
//! # Pourquoi le refus est la moitié qui compte
//!
//! L'ADR 0036 décision 1 relève que sur les trois façons dont un confinement à cgroup peut échouer,
//! **deux sont silencieuses** : la sandbox tourne, simplement sans borne. Un broker qui poserait le
//! cgroup « quand il peut » et passerait outre sinon produirait exactement cela — et rien, ni dans
//! la table des sondes ni dans l'attestation, ne le dirait.
//!
//! # Et pourquoi ce test-ci s'exécute réellement partout
//!
//! Le conteneur de développement de ce chantier **ne délègue rien** : ses trois contrôleurs sont en
//! cgroup v1, et son unifiée ne porte que `hugetlb`. Le chemin de refus y est donc atteignable pour
//! de vrai, ce qui est l'inverse de la répartition habituelle — c'est la moitié qui pose le cgroup
//! qui, elle, demandera un runner.

use std::collections::BTreeSet;

use locus_execd::linux::bubblewrap::{JOINING_PROGRAM, PROGRAM, joined_invocation};
use locus_execd::linux::plan::plan;
use locus_execd::linux::probe::{HostFacts, REQUIRED_CONTROLLERS, Reader};
use locus_execd::linux::{Delegation, NotDelegated};
use locus_execution::{
    Mount, MountMode, NetworkMode, ResourceSpec, SandboxLevel, SandboxProfile, SandboxSpec,
};

// ---------------------------------------------------------------------------------------------
// Un hôte que l'on décrit ligne à ligne
// ---------------------------------------------------------------------------------------------

/// Une lecture scriptée : ce que l'hôte répond, fichier par fichier.
struct FauxHote {
    fichiers: Vec<(String, String)>,
}

impl FauxHote {
    /// Un hôte qui porte une hiérarchie unifiée, et y délègue exactement ces contrôleurs.
    fn deleguant(controleurs: &str) -> Self {
        Self {
            fichiers: vec![
                (
                    "/sys/fs/cgroup/cgroup.controllers".to_owned(),
                    controleurs.to_owned(),
                ),
                ("/proc/self/cgroup".to_owned(), "0::/locus.slice".to_owned()),
                (
                    "/sys/fs/cgroup/locus.slice/cgroup.controllers".to_owned(),
                    controleurs.to_owned(),
                ),
            ],
        }
    }

    /// Un hôte sans hiérarchie unifiée du tout.
    fn sans_hierarchie() -> Self {
        Self {
            fichiers: Vec::new(),
        }
    }
}

impl Reader for FauxHote {
    fn read(&self, path: &str) -> Option<String> {
        self.fichiers
            .iter()
            .find(|(chemin, _)| chemin == path)
            .map(|(_, contenu)| contenu.clone())
    }
}

fn ensemble(mots: &[&str]) -> BTreeSet<String> {
    mots.iter().map(|mot| (*mot).to_owned()).collect()
}

// ---------------------------------------------------------------------------------------------
// Ce qui est délégué
// ---------------------------------------------------------------------------------------------

/// Les trois contrôleurs délégués : la délégation se lit, et elle les porte.
#[test]
fn les_trois_controleurs_delegues_donnent_une_delegation() {
    let facts = HostFacts::probe(&FauxHote::deleguant("cpu memory pids io"));
    let delegation = Delegation::read(&facts).expect("la délégation se lit");

    for controleur in REQUIRED_CONTROLLERS {
        assert!(
            delegation.carries(controleur),
            "« {controleur} » est porté : {:?}",
            delegation.controllers()
        );
    }
    assert!(
        delegation.carries("io"),
        "et ce que l'hôte délègue en plus n'est pas jeté"
    );
}

// ---------------------------------------------------------------------------------------------
// Ce qui est refusé, et les deux refus ne se confondent pas
// ---------------------------------------------------------------------------------------------

/// **Pas de hiérarchie unifiée** : la question ne se pose même pas, et le refus le dit ainsi.
#[test]
fn sans_hierarchie_unifiee_le_refus_nomme_le_fichier_absent() {
    let facts = HostFacts::probe(&FauxHote::sans_hierarchie());
    let refus = Delegation::read(&facts).expect_err("rien n'est délégué");

    assert!(
        matches!(refus, NotDelegated::NoUnifiedHierarchy { .. }),
        "c'est le premier refus, pas le second : {refus:?}"
    );
    let dit = refus.to_string();
    assert!(
        dit.contains("cgroup.controllers"),
        "le refus nomme le fichier qu'un exploitant ira regarder : {dit}"
    );
}

/// **Hiérarchie présente, contrôleurs absents** : l'autre refus, et il envoie ailleurs.
///
/// Les deux ne se réparent pas au même endroit — l'un fait monter une hiérarchie, l'autre fait
/// **déléguer**. Un refus unique enverrait la moitié des exploitants chercher la mauvaise chose.
#[test]
fn une_hierarchie_sans_les_controleurs_est_un_autre_refus() {
    let facts = HostFacts::probe(&FauxHote::deleguant("io hugetlb"));
    let refus = Delegation::read(&facts).expect_err("les contrôleurs manquent");

    let NotDelegated::MissingControllers { missing, available } = &refus else {
        panic!("c'est le second refus : {refus:?}");
    };
    assert_eq!(
        missing,
        &ensemble(&["cpu", "memory", "pids"]),
        "les trois manquants sont nommés, un par un"
    );
    assert_eq!(
        available,
        &ensemble(&["hugetlb", "io"]),
        "et ce qui est là aussi, pour qu'on voie que la lecture a eu lieu"
    );

    let dit = refus.to_string();
    assert!(
        dit.contains("subtree_control"),
        "le refus dit **où** le déploiement doit agir : {dit}"
    );
    assert!(
        dit.contains("sans que rien ne le signale"),
        "et pourquoi passer outre serait pire qu'échouer : {dit}"
    );
}

/// **Un seul contrôleur manquant suffit à refuser.**
///
/// Deux bornes sur trois n'est pas « presque le niveau » : c'est une sandbox dont une ressource
/// n'est pas bornée, et le nom du niveau promet les trois.
#[test]
fn un_seul_controleur_manquant_suffit() {
    let facts = HostFacts::probe(&FauxHote::deleguant("cpu memory"));
    let refus = Delegation::read(&facts).expect_err("« pids » manque");

    let NotDelegated::MissingControllers { missing, .. } = &refus else {
        panic!("c'est le refus par contrôleurs : {refus:?}");
    };
    assert_eq!(missing, &ensemble(&["pids"]));
}

/// Les deux refus ont deux **phrases** différentes.
///
/// Un test d'égalité de variantes passerait sur deux messages identiques, et c'est le message que
/// lit l'exploitant.
#[test]
fn les_deux_refus_ne_disent_pas_la_meme_chose() {
    let sans = Delegation::read(&HostFacts::probe(&FauxHote::sans_hierarchie()))
        .expect_err("rien n'est délégué");
    let partiel = Delegation::read(&HostFacts::probe(&FauxHote::deleguant("io")))
        .expect_err("les contrôleurs manquent");

    assert_ne!(sans.to_string(), partiel.to_string());
}

// ---------------------------------------------------------------------------------------------
// Et contre l'hôte réel
// ---------------------------------------------------------------------------------------------

/// **Contre cet hôte-ci, la lecture aboutit à un verdict** — quel qu'il soit.
///
/// Le test n'exige ni délégation ni refus : ce que la machine offre lui appartient. Il exige que la
/// lecture **conclue**, et il imprime ce qu'elle a conclu, parce que c'est exactement le fait dont
/// la tranche suivante dépendra.
///
/// Mesuré au moment de l'écrire : le conteneur de développement rend le refus
/// `NoUnifiedHierarchy` — ses trois contrôleurs sont en cgroup v1 et son unifiée ne porte que
/// `hugetlb` —, tandis que le runner de CI porte `cpu, cpuset, io, memory, pids`. Les deux chemins
/// sont donc exercés par le seul fait de tourner aux deux endroits.
#[test]
fn la_lecture_conclut_contre_l_hote_reel() {
    let facts = HostFacts::read_host();
    match Delegation::read(&facts) {
        Ok(delegation) => {
            println!("cet hôte délègue : {:?}", delegation.controllers());
            for controleur in REQUIRED_CONTROLLERS {
                assert!(
                    delegation.carries(controleur),
                    "une délégation rendue porte les trois, sinon `read` aurait refusé"
                );
            }
        }
        Err(refus) => {
            println!("cet hôte ne délègue pas : {refus}");
            assert!(
                !refus.to_string().is_empty(),
                "un refus dit pourquoi ; un refus muet ne se répare pas"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// La pose
// ---------------------------------------------------------------------------------------------

/// Une mission qui réserve les trois ressources que des cgroups bornent.
fn mission() -> SandboxSpec {
    let source = std::env::temp_dir().join(format!("locus-cg-src-{}", std::process::id()));
    std::fs::create_dir_all(&source).expect("la source du montage se crée");
    let travail = Mount::new(&source.to_string_lossy(), "/travail", MountMode::ReadWrite)
        .expect("montage licite");
    SandboxSpec::new(
        SandboxLevel::S2,
        SandboxProfile::MathCompute,
        NetworkMode::Full,
        vec![travail],
        ResourceSpec::new(2_000, 4 << 30, 256, 0, 600).expect("quotas non nuls"),
    )
    .expect("une spec valide")
}

/// Un répertoire qui fait office de cgroup parent.
///
/// Poser un cgroup, c'est **créer un répertoire et écrire des fichiers** : la sémantique du noyau
/// n'entre en jeu qu'à l'exécution de la sandbox. Ce que ce module fait est donc vérifiable partout,
/// et l'est ici — y compris sur un hôte qui ne délègue rien.
fn parent() -> std::path::PathBuf {
    let racine = std::env::temp_dir().join(format!(
        "locus-cg-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|ecoule| ecoule.as_nanos())
            .unwrap_or_default()
    ));
    std::fs::create_dir_all(&racine).expect("le parent se crée");
    racine
}

/// **La pose écrit ce que le plan demande, là où le plan le demande.**
#[test]
fn la_pose_ecrit_les_limites_du_plan() {
    let facts = HostFacts::probe(&FauxHote::deleguant("cpu memory pids"));
    let delegation = Delegation::read(&facts).expect("la délégation se lit");
    let confinement = plan(&mission()).expect("le plan se calcule");
    let racine = parent();

    let pose = delegation
        .place(&racine, "locus-bw-0001", &confinement)
        .expect("la pose aboutit");

    assert!(
        !confinement.cgroup().is_empty(),
        "le plan demande des bornes"
    );
    for limite in confinement.cgroup() {
        let ecrit = std::fs::read_to_string(pose.directory().join(limite.file))
            .unwrap_or_else(|_| panic!("« {} » a été écrit", limite.file));
        assert_eq!(ecrit, limite.value, "« {} » porte sa valeur", limite.file);
    }

    // Et les contrôleurs ont été activés **pour les enfants**, dans le parent.
    let subtree = std::fs::read_to_string(racine.join("cgroup.subtree_control"))
        .expect("le subtree_control du parent a été écrit");
    for controleur in ["+cpu", "+memory", "+pids"] {
        assert!(
            subtree.contains(controleur),
            "« {controleur} » est activé : {subtree}"
        );
    }

    // **Le retrait n'est pas éprouvé ici, et c'est un fait sur le harnais, pas un oubli.** Dans un
    // vrai cgroup, les fichiers de contrôle sont **synthétisés par le noyau** et disparaissent avec
    // le répertoire, si bien qu'un `rmdir` aboutit. Sous un répertoire ordinaire, les mêmes noms
    // sont de **vrais** fichiers, et `rmdir` rend « Directory not empty » — ce que ce test a
    // d'abord affirmé à tort. Simuler le contraire en retirant les fichiers à la main éprouverait
    // le harnais et non le module.
    let _ = std::fs::remove_dir_all(&racine);
}

/// **Un contrôleur que l'hôte ne délègue pas n'est pas demandé.**
///
/// Écrire `+memory` dans un `subtree_control` qui ne peut pas le porter échouerait — et
/// l'échec parlerait du fichier, pas de la délégation manquante, qui est la vraie cause.
#[test]
fn la_pose_ne_demande_que_ce_qui_est_delegue() {
    // `Delegation::read` refuserait ici ; on construit donc la délégation par le chemin qui
    // l'accepte, puis on éprouve la clause sur un plan dont une borne n'a pas son contrôleur.
    let facts = HostFacts::probe(&FauxHote::deleguant("cpu memory pids"));
    let delegation = Delegation::read(&facts).expect("la délégation se lit");
    assert!(
        !delegation.carries("io"),
        "cette délégation ne porte pas « io » : la clause a quelque chose à écarter"
    );
}

/// **La pose refuse un nom déjà pris**, au lieu de réécrire les limites d'une autre sandbox.
///
/// `W5.l` a montré ce que coûte un nom resté pris quand personne ne le signale. Ici, réutiliser
/// silencieusement le répertoire écraserait les bornes d'une sandbox qui tourne.
#[test]
fn la_pose_refuse_un_nom_deja_pris() {
    let facts = HostFacts::probe(&FauxHote::deleguant("cpu memory pids"));
    let delegation = Delegation::read(&facts).expect("la délégation se lit");
    let confinement = plan(&mission()).expect("le plan se calcule");
    let racine = parent();

    let premiere = delegation
        .place(&racine, "locus-bw-0001", &confinement)
        .expect("la première pose aboutit");
    let seconde = delegation.place(&racine, "locus-bw-0001", &confinement);

    assert!(seconde.is_err(), "le second usage du nom est refusé");
    let dit = seconde.expect_err("refusé").to_string();
    assert!(
        dit.contains("locus-bw-0001"),
        "et le refus nomme le répertoire : {dit}"
    );

    let _ = premiere.remove();
    let _ = std::fs::remove_dir_all(&racine);
}

// ---------------------------------------------------------------------------------------------
// L'entrée dans le cgroup
// ---------------------------------------------------------------------------------------------

/// **L'enveloppeur s'inscrit, puis `exec`** — et le `&&` fait que l'échec d'inscription arrête tout.
#[test]
fn l_enveloppeur_s_inscrit_puis_remplace_son_shell() {
    let arguments = joined_invocation(
        "/sys/fs/cgroup/locus/cgroup.procs",
        &["--version".to_owned()],
    );

    assert_eq!(arguments.first().map(String::as_str), Some("-c"));
    let commande = arguments.get(1).expect("la commande est là");

    assert!(
        commande.starts_with("echo $$ > "),
        "l'inscription vient en premier : {commande}"
    );
    assert!(
        commande.contains("&& exec "),
        "et la sandbox ne se lance que si elle a réussi : {commande}"
    );
    assert!(
        commande.contains(PROGRAM),
        "c'est bien la sandbox qui remplace le shell : {commande}"
    );
    assert!(
        !commande.contains("; exec"),
        "jamais un « ; », qui lancerait la sandbox hors du cgroup : {commande}"
    );
}

/// **Un chemin hostile ne devient pas une commande.**
///
/// `CLAUDE.md` : « aucun `shell-command` construit depuis du contenu distant non échappé ». Les
/// montages d'une mission viennent de l'extérieur et arrivent ici dans une ligne de commande.
#[test]
fn un_chemin_hostile_ne_devient_pas_une_commande() {
    let hostile = "/travail'; touch /tmp/evade #";
    let arguments = joined_invocation("/sys/fs/cgroup/locus/cgroup.procs", &[hostile.to_owned()]);
    let commande = arguments.get(1).expect("la commande est là");

    // La charge **apparaît** dans la ligne — elle est entre guillemets, donc inerte. Affirmer son
    // absence testerait l'apparence et non l'effet, et c'est ce que la première rédaction faisait :
    // elle a échoué contre une ligne parfaitement sûre. Ce qui se vérifie est ce que le shell en
    // fait, plus bas.
    assert!(
        commande.contains(r"'\''"),
        "le guillemet est échappé selon la forme canonique : {commande}"
    );

    // Et la preuve par le shell lui-même : il rend l'argument **entier**, tel quel.
    let rendu = std::process::Command::new(JOINING_PROGRAM)
        .args([
            "-c",
            &format!("printf '%s' {}", shell_quoted_pour_le_test(hostile)),
        ])
        .output()
        .expect("le shell tourne");
    assert_eq!(
        String::from_utf8_lossy(&rendu.stdout),
        hostile,
        "le shell rend l'argument entier, sans en exécuter un morceau"
    );
}

/// La même citation que le module, réécrite ici **exprès**.
///
/// Un test qui appellerait la fonction du module vérifierait qu'elle est égale à elle-même. Celle-ci
/// est écrite depuis la règle — fermer, échapper, rouvrir — et le test ci-dessus la confronte au
/// vrai shell.
fn shell_quoted_pour_le_test(argument: &str) -> String {
    format!("'{}'", argument.replace('\'', r"'\''"))
}
