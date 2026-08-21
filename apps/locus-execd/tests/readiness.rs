//! Test de sortie de `W22.c` — **le binaire ne nie plus ce que son crate exporte**, ADR 0025.
//!
//! 1. Le driver est **construit**, pas décrit.
//! 2. Le refus nomme ce qui manque **réellement**, fait par fait, et il vient de l'hôte.
//! 3. Le point d'entrée ne porte aucune phrase de refus — la vérification en rouge est faite en
//!    niant une capacité réellement exportée.
//! 4. Un hôte court et un hôte capable ne rendent pas le même code de sortie.

use std::fs;
use std::path::{Path, PathBuf};

use locus_execd::linux::{HostFacts, SystemRunner};
use locus_execd::readiness::Readiness;
use locus_execution::SandboxLevel;

/// Le code d'un fichier, c'est sa source moins ses commentaires — voir `W21.j`.
fn code_seul(source: &str) -> String {
    source
        .lines()
        .filter(|ligne| !ligne.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn ecrire(root: &Path, chemin: &str, contenu: &str) {
    let cible = root.join(chemin);
    fs::create_dir_all(cible.parent().expect("un parent")).expect("créer l'arbre de fixture");
    fs::write(cible, contenu).expect("écrire la fixture");
}

/// Un hôte qui délègue tout ce que `S2` demande.
fn hote_capable(nom: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("locus-w22c-{nom}"));
    let _ = fs::remove_dir_all(&root);
    ecrire(
        &root,
        "sys/fs/cgroup/cgroup.controllers",
        "cpu memory pids\n",
    );
    ecrire(&root, "proc/self/cgroup", "0::/user.slice/session.scope\n");
    ecrire(
        &root,
        "sys/fs/cgroup/user.slice/session.scope/cgroup.controllers",
        "cpu io memory pids\n",
    );
    ecrire(&root, "proc/sys/user/max_user_namespaces", "15000\n");
    ecrire(
        &root,
        "proc/sys/kernel/seccomp/actions_avail",
        "kill_process kill_thread trap errno user_notif trace log allow\n",
    );
    root
}

/// Le même, privé de sa hiérarchie unifiée : il ne tient pas un conteneur sans privilèges.
fn hote_court(nom: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("locus-w22c-{nom}"));
    let _ = fs::remove_dir_all(&root);
    ecrire(&root, "proc/sys/user/max_user_namespaces", "15000\n");
    ecrire(
        &root,
        "proc/sys/kernel/seccomp/actions_avail",
        "kill_process kill_thread trap errno user_notif trace log allow\n",
    );
    root
}

// ---------------------------------------------------------------------------------------------
// 1. Le driver est construit, pas décrit
// ---------------------------------------------------------------------------------------------

/// **Le point d'entrée construit le driver et le nomme.**
///
/// Le test qui porte l'item. La version précédente de `main.rs` annonçait « aucun driver de runtime
/// n'est encore branché » pendant que le crate exportait `SystemRunner`, la seule fonction du dépôt
/// qui exécute `podman`. Construire le driver **dans** le point d'entrée est ce qui rend la
/// négation inexprimable : on ne peut pas déclarer absent ce qu'on vient d'instancier.
#[test]
fn le_point_d_entree_construit_le_driver() {
    let code = code_seul(include_str!("../src/main.rs"));
    assert!(
        code.contains("SystemRunner::new()"),
        "le point d'entrée doit **construire** le driver, pas en parler"
    );
    assert_eq!(
        SystemRunner::new().program(),
        "podman",
        "et le programme se nomme, sinon « driver construit » ne dit pas lequel"
    );
}

// ---------------------------------------------------------------------------------------------
// 2. Le refus nomme ce qui manque réellement
// ---------------------------------------------------------------------------------------------

/// **Un hôte qui ne tient pas le plancher est refusé fait par fait, pas en bloc.**
///
/// Un exploitant à qui l'on dit quel contrôleur cgroup n'est pas délégué corrige en une commande ;
/// un exploitant à qui l'on dit « non » change de machine. C'est la règle d'`admit`, qui accumule
/// ses refus au lieu de rendre le premier.
#[test]
fn un_hote_court_nomme_ce_qui_manque() {
    let etat = Readiness::assess(&HostFacts::read(&hote_court("court")));

    assert!(!etat.is_provable());
    assert!(
        etat.missing().len() >= 2,
        "au moins la hiérarchie unifiée et un contrôleur : {:?}",
        etat.missing()
    );
    let dit = etat.to_string();
    assert!(dit.contains("cgroup v2"), "{dit}");
    assert!(
        dit.contains("driver construit"),
        "le driver, lui, existe : {dit}"
    );
    assert_eq!(etat.ceiling(), SandboxLevel::S1);
}

/// **Un hôte qui tient le plancher est déclaré utilisable, et son plafond est rendu.**
#[test]
fn un_hote_capable_est_utilisable() {
    let etat = Readiness::assess(&HostFacts::read(&hote_capable("capable")));

    assert!(etat.is_provable());
    assert!(etat.missing().is_empty());
    assert!(etat.ceiling() >= Readiness::FLOOR);
    let dit = etat.to_string();
    assert!(dit.contains("peut porter une sandbox"), "{dit}");
    assert!(
        dit.contains("driver construit"),
        "les **deux** constats disent que le driver existe — c'est le fait que le binaire niait, et \
         un seul des deux chemins le dirait que l'autre pourrait le taire : {dit}"
    );
}

/// **Le plafond est rendu dans les deux cas.**
///
/// « `S1`, et voici pourquoi pas `S2` » est une réponse ; « rien » n'en est pas une. Un rapport qui
/// tairait le plafond d'un hôte court le rendrait indiscernable d'un hôte sans aucun confinement.
#[test]
fn le_plafond_est_rendu_meme_quand_l_hote_est_court() {
    let court = Readiness::assess(&HostFacts::read(&hote_court("plafond-court")));
    let capable = Readiness::assess(&HostFacts::read(&hote_capable("plafond-capable")));

    assert_eq!(court.ceiling(), SandboxLevel::S1);
    assert!(capable.ceiling() > court.ceiling());
    assert_ne!(court, capable);
}

/// **Le refus vient de l'hôte, pas d'un texte écrit dans le module.**
///
/// Deux hôtes différemment cassés ne rendent pas le même refus. Si le message était une constante,
/// ils rendraient le même — et il vieillirait sans que rien ne le dise, ce qui est exactement la
/// faute que cet item corrige.
#[test]
fn deux_hotes_casses_autrement_ne_rendent_pas_le_meme_refus() {
    let sans_cgroup = Readiness::assess(&HostFacts::read(&hote_court("sans-cgroup")));

    let racine = hote_capable("sans-userns");
    let _ = fs::remove_file(racine.join("proc/sys/user/max_user_namespaces"));
    let sans_userns = Readiness::assess(&HostFacts::read(&racine));

    assert!(!sans_cgroup.is_provable());
    assert!(!sans_userns.is_provable());
    assert_ne!(
        sans_cgroup.to_string(),
        sans_userns.to_string(),
        "un refus constant ne dirait rien de la machine qu'on a sous la main"
    );
}

// ---------------------------------------------------------------------------------------------
// 3. Aucune phrase de refus dans le point d'entrée
// ---------------------------------------------------------------------------------------------

/// **Le point d'entrée ne porte aucune phrase qui déclare une absence.**
///
/// La vérification en rouge de cet item se fait en **niant une capacité réellement exportée** —
/// remettre « aucun driver de runtime n'est encore branché » dans `main.rs` fait échouer ce test —
/// et non en éditant le message pour qu'il passe.
///
/// Les motifs visent des tournures de **négation**, pas des noms de symboles : la version fautive
/// ne nommait aucun symbole, elle disait « aucun driver ». Une garde qui aurait cherché
/// `SystemRunner` dans le message ne l'aurait jamais attrapée.
///
/// Ce que le binaire a le droit de dire est ce qu'il a **calculé** : `Readiness` compose son texte
/// à partir de `HostFacts`, donc il ne peut pas se périmer sans que l'hôte change.
#[test]
fn le_point_d_entree_ne_declare_aucune_absence() {
    let code = code_seul(include_str!("../src/main.rs"));

    for interdit in [
        "aucun driver",
        "aucun runtime",
        "n'est encore",
        "pas encore",
        "n'existe pas",
        "non branché",
        "refuse de",
    ] {
        assert!(
            !code.contains(interdit),
            "« {interdit} » est une affirmation d'absence dans un point d'entrée : ADR 0025, une \
             capacité niée est une promesse négative"
        );
    }

    assert!(
        code.contains("Readiness::assess"),
        "le nettoyage a trop enlevé, ou le constat n'est plus calculé"
    );
}

// ---------------------------------------------------------------------------------------------
// 4. Le code de sortie distingue
// ---------------------------------------------------------------------------------------------

/// **Un hôte capable et un hôte court ne rendent pas le même code de sortie.**
///
/// C'est ce qu'un superviseur lit, et c'est la seule partie du constat qui parvienne à un
/// programme. `locusd` fait déjà la même chose pour une projection en quarantaine.
#[test]
fn le_code_de_sortie_distingue_les_deux_cas() {
    let capable = Readiness::assess(&HostFacts::read(&hote_capable("sortie-capable")));
    let court = Readiness::assess(&HostFacts::read(&hote_court("sortie-court")));

    assert!(capable.is_provable());
    assert!(!court.is_provable());
    assert_ne!(capable.is_provable(), court.is_provable());

    let code = code_seul(include_str!("../src/main.rs"));
    assert!(
        code.contains("is_provable()"),
        "le code de sortie suit le constat"
    );
    assert!(code.contains("ExitCode::SUCCESS"));
    assert!(code.contains("ExitCode::FAILURE"));
}

/// **`HostFacts` ne sépare pas `S2` de `S3`, et c'est pour cela que la valeur exacte du plancher ne
/// s'exerce pas.**
///
/// Une passe de mutation a montré que remonter [`Readiness::FLOOR`] de `S2` à `S3` ne fait échouer
/// aucun test — et la cause n'est pas un trou de couverture : `missing_for` n'exige **rien** de plus
/// pour `S3` que pour `S2`. Ce qui sépare les deux niveaux est l'isolation réseau, et elle ne se lit
/// pas dans `/proc` : elle se vérifie sur une sandbox vivante, par la suite de sondes de `W5.k`.
///
/// Un hôte qui prouve `S2` sans prouver `S3` est donc **inexprimable** pour ce lecteur de faits.
/// Plutôt que de retirer le plancher — dont le nom dit juste ce que le backend Podman exige — le
/// fait est épinglé ici : si quelqu'un ajoute un jour une sonde réseau à `HostFacts`, ce test
/// rougira et lui dira que le plancher vient de prendre un sens.
#[test]
fn les_faits_d_hote_ne_separent_pas_s2_de_s3() {
    for racine in [hote_capable("s2s3-capable"), hote_court("s2s3-court")] {
        let facts = HostFacts::read(&racine);
        assert_eq!(
            facts.missing_for(SandboxLevel::S2),
            facts.missing_for(SandboxLevel::S3),
            "l'isolation réseau n'est pas un fait d'hôte statique : voir W5.k"
        );
    }
}
