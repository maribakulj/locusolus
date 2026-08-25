//! Test de sortie de `W23.a` — **le port de persistance d'état d'instance, et son implémentation de
//! référence en mémoire.**
//!
//! Les quatre clauses de la ligne de roadmap, une section chacune :
//!
//! 1. une instance se reconstruit depuis son état persisté et rend **exactement** le même état,
//!    condensat compris ;
//! 2. **aucun objet d'agent ne traverse une frontière de processus**, tenu par l'absence de type
//!    sérialisable portant un comportement ;
//! 3. le port n'a **pas** de variante « peu importe l'état » ;
//! 4. un backend externe n'est **pas** choisi.

use std::fs;
use std::path::Path;

use locus_coordination::agent::{AgentInstance, AgentTemplate, InstanceState, TemplateStatus};
use locus_coordination::state::{
    AgentState, AgentStateStore, Expectation, MemoryAgentStateStore, StateFormat,
};
use locus_protocol::{
    Id, Timestamp,
    id::{Branch, Program, provisional::Team as TeamKind},
};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn id<K: locus_protocol::IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn template() -> AgentTemplate {
    AgentTemplate::new(
        id(1),
        "Reviewer logique",
        "LogicalReviewer",
        3,
        TemplateStatus::Active,
    )
    .expect("template valide")
    .in_independence_group("groupe-a")
}

/// Une instance qui porte **tous** ses champs facultatifs.
///
/// Une fixture minimale laisserait les `Option` à `None`, et l'aller-retour ne dirait alors rien de
/// ce qui les distingue d'une valeur — or c'est exactement là que la forme canonique peut confondre.
fn instance_complete() -> AgentInstance {
    AgentInstance::provision(id(2), &template())
        .expect("le template est instanciable")
        .in_program(id::<Program>(3))
        .on_branch(id::<Branch>(4))
        .in_team(id::<TeamKind>(5))
        .on_worker("canterel-01")
}

// ---------------------------------------------------------------------------------------------
// 1. Une instance se reconstruit à l'identique, condensat compris
// ---------------------------------------------------------------------------------------------

/// **L'aller-retour rend exactement la même instance.**
///
/// La clause dit « exactement », et le test la prend au mot : l'instance reconstruite est comparée à
/// l'originale **entière**, pas champ par champ. Une comparaison champ par champ oublierait celui
/// qu'on ajoutera l'an prochain, et c'est précisément le champ qui se perdrait en silence.
#[test]
fn une_instance_se_reconstruit_a_l_identique() {
    let origine = instance_complete();

    let etat = AgentState::of(&origine);
    let reconstruite = etat.restore().expect("l'état décrit une instance valide");

    assert_eq!(reconstruite, origine);
}

/// **Le condensat survit à l'aller-retour, et il couvre ce qui distingue deux états.**
///
/// « Rend exactement le même état, **condensat compris** ». Un condensat qui ne changerait pas quand
/// l'état change ne dirait rien ; le test le vérifie donc dans les deux sens — stable sur le même
/// état, différent sur un état voisin.
#[test]
fn le_condensat_survit_a_l_aller_retour_et_distingue_deux_etats() {
    let etat = AgentState::of(&instance_complete());
    let relu = AgentState::decode(&etat.encode()).expect("la forme canonique se relit");

    assert_eq!(relu, etat);
    assert_eq!(relu.digest(), etat.digest());

    // Un seul champ change — l'état de §7.1 — et le condensat doit bouger.
    let avance = AgentState::of(
        &instance_complete()
            .moved_to(InstanceState::Active)
            .expect("une instance provisionnée peut devenir active"),
    );
    assert_ne!(avance.digest(), etat.digest());
}

/// **Un champ facultatif absent ne se confond pas avec un champ vide.**
///
/// `worker_id: None` et `worker_id: Some("")` sont deux états différents, et une forme canonique qui
/// les écrirait pareil ferait rendre au condensat la même valeur pour deux instances distinctes.
/// C'est la règle que ce dépôt applique partout — une absence n'est pas une valeur — appliquée à une
/// canonicalisation.
#[test]
fn une_absence_ne_se_confond_pas_avec_une_valeur_vide() {
    let sans = AgentState::of(&AgentInstance::provision(id(2), &template()).expect("instanciable"));

    // `on_worker("")` est refusé par le domaine, donc l'état vide ne peut venir que d'un support
    // qui l'écrirait — et c'est ce que la relecture doit distinguer.
    //
    // Le champ est posé **par sa position**, et non par un `replace` sur la forme entière : la
    // première rédaction remplaçait le premier `-` rencontré, qui est celui de `program_id`, et le
    // test échouait en parlant du mauvais champ. Une chirurgie de chaîne sur une forme canonique
    // doit viser un champ, pas un motif.
    let mut champs: Vec<String> = sans.encode().split('\u{1f}').map(str::to_owned).collect();
    champs[6] = "=".to_owned();
    let vide = champs.join("\u{1f}");
    assert_ne!(vide, sans.encode());

    let relu = AgentState::decode(&vide).expect("la forme reste lisible");
    assert_ne!(relu.digest(), sans.digest());
    // Et le domaine refuse de la reposer : ce que l'écriture interdit n'entre pas par la lecture.
    assert!(relu.restore().is_err());
}

/// **Ce que l'écriture refuse n'entre pas par la lecture.**
///
/// Un support peut rendre n'importe quoi — un fichier édité à la main, une version antérieure du
/// format, un octet retourné. `from_state` reste donc **vérifiant** : il refuse ce que `provision`
/// aurait refusé, et une version nulle en fait partie.
///
/// Trouvé par une passe de mutants : retirer la garde ne faisait rougir aucun test. La forme
/// canonique se relit parfaitement — `0` est un entier valide — et c'est **au moment de reposer
/// l'instance** que le domaine doit dire non.
#[test]
fn une_version_nulle_se_relit_mais_ne_se_repose_pas() {
    let etat = AgentState::of(&instance_complete());
    let mut champs: Vec<String> = etat.encode().split('\u{1f}').map(str::to_owned).collect();
    champs[2] = "0".to_owned();

    let relu = AgentState::decode(&champs.join("\u{1f}")).expect("« 0 » est un entier lisible");

    assert!(
        relu.restore().is_err(),
        "une version nulle est entrée par la lecture, alors que `provision` la refuse"
    );
}

/// **Un slug d'état inconnu refuse.**
///
/// `Provisioned` par défaut ferait **revivre une instance terminée** — la faute la plus coûteuse que
/// cette relecture puisse commettre, et elle serait invisible.
#[test]
fn un_etat_inconnu_refuse_plutot_que_de_se_ranger() {
    let etat = AgentState::of(&instance_complete());
    let falsifie = etat
        .encode()
        .replace(InstanceState::Provisioned.slug(), "zombie");

    assert_eq!(
        AgentState::decode(&falsifie),
        Err(StateFormat::UnknownState {
            slug: "zombie".to_owned()
        })
    );
}

/// **Une instance terminale se reconstruit.**
///
/// Reconstruire n'est pas une transition. Passer par `moved_to` ferait de `Completed` un état
/// irrécupérable, alors que c'est précisément un état qu'on doit pouvoir relire.
#[test]
fn une_instance_terminale_se_reconstruit() {
    let terminee = instance_complete()
        .moved_to(InstanceState::Completed)
        .expect("une instance provisionnée peut se terminer");

    let reconstruite = AgentState::of(&terminee)
        .restore()
        .expect("un état terminal se repose tel quel");

    assert_eq!(reconstruite, terminee);
    assert_eq!(reconstruite.state(), InstanceState::Completed);
}

// ---------------------------------------------------------------------------------------------
// 2. Aucun objet d'agent ne traverse une frontière de processus
// ---------------------------------------------------------------------------------------------

/// **`packages/coordination` ne dépend de `serde` sous aucune forme.**
///
/// C'est la clause « aucun objet d'agent ne traverse une frontière de processus, tenu par l'absence
/// de type sérialisable portant un comportement », et elle se tient ici **par construction** : s'il
/// n'existe dans ce crate aucun type sérialisable, il n'en existe a fortiori aucun qui porte un
/// comportement.
///
/// Le test lit le `Cargo.toml` plutôt que le code. Chercher `#[derive(Serialize)]` dans les sources
/// laisserait passer une implémentation manuelle, et surtout laisserait la porte ouverte : la
/// propriété qu'on veut n'est pas « personne n'a encore dérivé », c'est « personne ne **peut**
/// dériver ».
#[test]
fn le_crate_n_a_aucune_dependance_serde() {
    let manifeste = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .expect("le manifeste du crate se lit");

    assert!(
        !manifeste.contains("serde"),
        "`packages/coordination` a gagné une dépendance `serde` : un type sérialisable y devient \
         exprimable, et avec lui un objet d'agent qui traverse une frontière de processus. \
         Si le besoin est réel, c'est l'ADR 0026 décision 2 qu'il faut amender, pas ce test.\n{manifeste}"
    );
}

/// **Ce qui traverse est une forme canonique, c'est-à-dire du texte.**
///
/// Le pendant positif du test précédent : un adaptateur a bien de quoi écrire. Ce qu'il écrit est
/// une chaîne, et ce qu'il relit en rend une instance — sans qu'aucun type de ce crate ait eu besoin
/// d'être sérialisable.
#[test]
fn ce_qui_traverse_est_du_texte_et_il_suffit() {
    let origine = instance_complete();

    // Le seul aller-retour qu'un support a besoin de faire.
    let ecrit: String = AgentState::of(&origine).encode();
    let relu = AgentState::decode(&ecrit).expect("ce qui a été écrit se relit");

    assert_eq!(relu.restore().expect("l'état est valide"), origine);
}

// ---------------------------------------------------------------------------------------------
// 3. Le port n'a pas de variante « peu importe l'état »
// ---------------------------------------------------------------------------------------------

/// **Une lecture rend l'état *et* sa révision, jamais l'un sans l'autre.**
///
/// Un appelant qui relirait un état sans sa révision n'aurait pas de quoi le réécrire sans écraser
/// ce qu'un autre a fait entre-temps. La propriété est tenue par le type : `load` rend un `Stored`,
/// qui porte les deux.
#[test]
fn une_lecture_rend_l_etat_et_sa_revision() {
    let support = MemoryAgentStateStore::new();
    let etat = AgentState::of(&instance_complete());

    let revision = support
        .save(&etat, Expectation::Absent)
        .expect("rien n'était écrit");

    let lu = support.load(etat.id()).expect("l'état vient d'être écrit");
    assert_eq!(lu.state, etat);
    assert_eq!(lu.revision, revision);
}

/// **Écrire sur une instance qu'on croyait absente, alors qu'elle existe, est refusé.**
///
/// C'est la faute que l'attente évite : deux exécutions concurrentes de la même instance, dont la
/// seconde écrase l'état de la première sans que rien ne le dise.
#[test]
fn ecrire_en_croyant_l_instance_absente_est_refuse() {
    let support = MemoryAgentStateStore::new();
    let etat = AgentState::of(&instance_complete());
    support
        .save(&etat, Expectation::Absent)
        .expect("rien n'était écrit");

    let refus = support
        .save(&etat, Expectation::Absent)
        .expect_err("quelque chose est écrit, désormais");

    assert_eq!(refus.id, etat.id());
    assert_eq!(refus.expected, Expectation::Absent);
    assert_eq!(refus.actual, Some(1));
    // Le message nomme les deux côtés : un refus qui dirait seulement « conflit » enverrait relire
    // l'état pour savoir lequel des deux est en retard.
    let dit = refus.to_string();
    assert!(dit.contains("rien d'écrit"), "{dit}");
    assert!(dit.contains("révision 1"), "{dit}");
}

/// **Écrire depuis une révision périmée est refusé, et le refus nomme les deux.**
#[test]
fn ecrire_depuis_une_revision_perimee_est_refuse() {
    let support = MemoryAgentStateStore::new();
    let etat = AgentState::of(&instance_complete());
    support.save(&etat, Expectation::Absent).expect("première");
    support.save(&etat, Expectation::At(1)).expect("seconde");

    let refus = support
        .save(&etat, Expectation::At(1))
        .expect_err("la révision 1 n'est plus la courante");

    assert_eq!(refus.expected, Expectation::At(1));
    assert_eq!(refus.actual, Some(2));
}

/// **Écrire sur une instance absente en croyant une révision est refusé aussi.**
///
/// Le symétrique du cas précédent, et il ne se déduit pas de lui : `At(1)` sur rien du tout est une
/// attente fausse dans l'autre sens, et un support qui la traiterait comme une première écriture
/// ferait apparaître une instance que personne n'a provisionnée.
#[test]
fn ecrire_une_revision_sur_une_instance_absente_est_refuse() {
    let support = MemoryAgentStateStore::new();
    let etat = AgentState::of(&instance_complete());

    let refus = support
        .save(&etat, Expectation::At(1))
        .expect_err("rien n'est écrit pour cette instance");

    assert_eq!(refus.actual, None);
    assert!(support.is_empty(), "un refus n'écrit rien");
}

/// **Deux instances ne se confondent pas.**
#[test]
fn deux_instances_sont_conservees_a_part() {
    let support = MemoryAgentStateStore::new();
    let premiere = AgentState::of(&instance_complete());
    let seconde = AgentState::of(
        &AgentInstance::provision(id(9), &template()).expect("le template est instanciable"),
    );

    support
        .save(&premiere, Expectation::Absent)
        .expect("écrite");
    support.save(&seconde, Expectation::Absent).expect("écrite");

    assert_eq!(support.len(), 2);
    assert_eq!(
        support.load(premiere.id()).expect("conservée").state,
        premiere
    );
    assert_eq!(
        support.load(seconde.id()).expect("conservée").state,
        seconde
    );
}

/// **Une instance jamais écrite ne se lit pas.**
#[test]
fn une_instance_jamais_ecrite_ne_se_lit_pas() {
    let support = MemoryAgentStateStore::new();

    assert!(support.load(id(42)).is_none());
}
