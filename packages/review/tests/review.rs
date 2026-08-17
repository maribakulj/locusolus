//! Test de sortie de W7.a — **le dossier se fige avant l'attribution, l'indépendance se constate,
//! et un finding sans preuve ne décide de rien.**
//!
//! Les trois disent la même chose sous trois angles : ce qui fonde une revue doit être vérifiable
//! après coup par quelqu'un qui n'y était pas.

use locus_domain::{ContentHash, RevisionId};
use locus_protocol::{
    Id, IdKind, Timestamp,
    id::{Agent, Branch},
};
use locus_review::{
    Blindness, DossierError, Draft, Finding, Frozen, IndependenceRequirement, Party, Review,
    ReviewError, Severity, Verdict, attest,
};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn revision(seed: u8) -> RevisionId {
    id::<locus_domain::ids::RevisionKind>(seed)
}

fn hash() -> ContentHash {
    ContentHash::parse(&format!("sha256:{}", "ab".repeat(32))).expect("hash bien formé")
}

fn draft() -> Draft {
    Draft::open("dossier-0001", vec![revision(1)])
        .expect("dossier valide")
        .asking("la preuve du lemme 3 tient-elle sans l'hypothèse de compacité ?")
        .excluding("le transcript de génération")
        .blind_to(Blindness::GeneratorTranscript)
        .requiring(IndependenceRequirement::DistinctIndependenceGroup)
        .requiring(IndependenceRequirement::DistinctWorker)
        .requiring(IndependenceRequirement::NoGeneratorTranscript)
}

fn frozen() -> Frozen {
    draft().freeze(hash()).expect("dossier figé")
}

fn generator() -> Party {
    Party {
        agent_id: id::<Agent>(1),
        worker_id: "vm-01".to_owned(),
        independence_group: Some("formalisation".to_owned()),
        holds_generator_transcript: true,
    }
}

fn independent_reviewer() -> Party {
    Party {
        agent_id: id::<Agent>(2),
        worker_id: "vm-02".to_owned(),
        independence_group: Some("logique".to_owned()),
        holds_generator_transcript: false,
    }
}

fn evidenced_finding() -> Finding {
    Finding::new(
        revision(1),
        "hypothèse non employée",
        Severity::Major,
        Verdict::Refutes,
        vec![revision(7)],
    )
    .expect("finding valide")
}

// ---------------------------------------------------------------------------------------------
// Le dossier se fige avant l'attribution
// ---------------------------------------------------------------------------------------------

/// §17.3 : « toute modification entraîne une nouvelle version ou un addendum explicitement
/// visible ». Les deux issues existent, et **aucune troisième** : `Frozen` n'a pas de méthode qui
/// change ce que le relecteur consultera. Ce test dit la conséquence observable ; c'est le type qui
/// dit la garantie.
#[test]
fn un_dossier_fige_ne_se_modifie_que_par_addendum_ou_nouvelle_version() {
    let attributed = frozen();
    assert!(attributed.addenda().is_empty());

    let annotated = attributed
        .clone()
        .with_addendum("le corpus B est arrivé après");
    assert_eq!(annotated.addenda(), ["le corpus B est arrivé après"]);
    assert_eq!(
        annotated.content_hash(),
        attributed.content_hash(),
        "un addendum n'altère pas ce qui a été figé : c'est ce qui permet de dire, après coup, ce \
         que le relecteur avait sous les yeux"
    );
    assert_eq!(annotated.questions(), attributed.questions());

    // L'autre issue : repartir d'un brouillon, donc produire un dossier à re-figer.
    let revised = attributed
        .revise()
        .asking("et sans l'hypothèse de séparabilité ?");
    let refrozen = revised.freeze(hash()).expect("nouvelle version");
    assert_eq!(refrozen.questions().len(), 2);
    assert_eq!(
        attributed.questions().len(),
        1,
        "la version attribuée n'a pas bougé"
    );
}

#[test]
fn un_dossier_sans_question_ne_se_fige_pas() {
    // §17.1 exige « les questions posées ». Sans elles, le relecteur décide seul de ce qu'il
    // examine, et sa couverture devient inopposable.
    let mute = Draft::open("dossier-0002", vec![revision(1)]).expect("dossier valide");
    assert_eq!(mute.freeze(hash()), Err(DossierError::NoQuestion));
}

#[test]
fn un_dossier_sans_cible_ni_identifiant_est_refuse() {
    assert_eq!(
        Draft::open("  ", vec![revision(1)]),
        Err(DossierError::EmptyId)
    );
    assert_eq!(
        Draft::open("dossier-0003", Vec::new()),
        Err(DossierError::NoTarget),
        "relire « en général » n'est pas relire"
    );
}

#[test]
fn ce_qui_est_exclu_est_nomme() {
    // §17.1 : la revue rend explicite « ce qui a été exclu ». Une exclusion non nommée est
    // indistinguable d'un oubli.
    let dossier = frozen();
    assert_eq!(dossier.excluded(), ["le transcript de génération"]);
    assert!(
        dossier
            .blindness()
            .contains(&Blindness::GeneratorTranscript)
    );
}

// ---------------------------------------------------------------------------------------------
// L'indépendance se constate
// ---------------------------------------------------------------------------------------------

#[test]
fn un_relecteur_distinct_sur_les_trois_plans_est_independant() {
    let attestation = attest(&frozen(), &generator(), &independent_reviewer());
    assert!(attestation.holds());
    assert_eq!(attestation.satisfied().len(), 3);
    assert!(attestation.violated().is_empty());
}

/// §14.4 : deux relecteurs du même groupe ne comptent pas comme indépendants. Le groupe vient du
/// template (W13.c) et descend à l'instance, ce qui rend la question décidable sans remonter à une
/// version courante qui change avec le temps.
#[test]
fn deux_agents_du_meme_groupe_ne_sont_pas_independants() {
    let mut same_group = independent_reviewer();
    same_group.independence_group = Some("formalisation".to_owned());

    let attestation = attest(&frozen(), &generator(), &same_group);
    assert!(!attestation.holds());
    assert!(
        attestation
            .violated()
            .contains(&IndependenceRequirement::DistinctIndependenceGroup)
    );
    assert!(
        attestation
            .satisfied()
            .contains(&IndependenceRequirement::DistinctWorker),
        "le reste de l'attestation reste vrai : un refus n'est pas un verdict global"
    );
}

/// L'absence de preuve n'est pas une preuve, cinquième occurrence. Deux groupes **inconnus** ne
/// sont pas deux groupes différents, et conclure l'inverse ferait de l'ignorance une garantie
/// d'indépendance — exactement le contraire de ce que §14.4 demande.
#[test]
fn deux_groupes_inconnus_ne_sont_pas_distincts() {
    let mut anonymous_generator = generator();
    anonymous_generator.independence_group = None;
    let mut anonymous_reviewer = independent_reviewer();
    anonymous_reviewer.independence_group = None;

    let attestation = attest(&frozen(), &anonymous_generator, &anonymous_reviewer);
    assert!(
        attestation
            .violated()
            .contains(&IndependenceRequirement::DistinctIndependenceGroup)
    );
}

/// Invariant 11, littéralement : « les reviewers indépendants ne reçoivent pas le raisonnement
/// privé […] du générateur ».
#[test]
fn un_relecteur_qui_a_lu_le_transcript_n_est_pas_independant() {
    let mut informed = independent_reviewer();
    informed.holds_generator_transcript = true;

    let attestation = attest(&frozen(), &generator(), &informed);
    assert!(!attestation.holds());
    assert!(
        attestation
            .violated()
            .contains(&IndependenceRequirement::NoGeneratorTranscript)
    );
}

#[test]
fn le_meme_worker_ne_satisfait_pas_l_exigence_de_worker_distinct() {
    let mut colocated = independent_reviewer();
    colocated.worker_id = "vm-01".to_owned();
    let attestation = attest(&frozen(), &generator(), &colocated);
    assert!(
        attestation
            .violated()
            .contains(&IndependenceRequirement::DistinctWorker)
    );
}

/// Une exigence que le dossier n'a pas posée n'est ni satisfaite ni violée : elle n'est pas
/// évaluée. L'attestation dit ce que **ce** dossier demandait, pas ce qu'on aurait pu demander.
#[test]
fn l_attestation_ne_juge_que_ce_que_le_dossier_exige() {
    let minimal = Draft::open("dossier-0004", vec![revision(1)])
        .expect("dossier valide")
        .asking("une question")
        .requiring(IndependenceRequirement::DistinctWorker)
        .freeze(hash())
        .expect("figé");

    let mut informed = independent_reviewer();
    informed.holds_generator_transcript = true;

    let attestation = attest(&minimal, &generator(), &informed);
    assert!(
        attestation.holds(),
        "le transcript n'était pas exigé absent : l'attestation ne l'invente pas"
    );
    assert_eq!(attestation.satisfied().len(), 1);
}

// ---------------------------------------------------------------------------------------------
// La revue rendue
// ---------------------------------------------------------------------------------------------

#[test]
fn une_revue_porte_son_attestation_et_sa_couverture() {
    let review = Review::render(
        &frozen(),
        &generator(),
        &independent_reviewer(),
        vec![evidenced_finding()],
        "la preuve du lemme 3, ligne à ligne",
    )
    .expect("revue valide")
    .limited_by("le corpus B n'a pas pu être consulté");

    assert!(review.is_independent());
    assert_eq!(review.dossier_id(), "dossier-0001");
    assert_eq!(review.reviewer(), id::<Agent>(2));
    assert_eq!(review.limitations().len(), 1);
}

/// Une revue non indépendante **reste une revue** : elle est rendue, elle est consignée. Ce
/// qu'elle ne peut pas faire est compter comme la revue indépendante que la politique exigeait.
/// L'écarter effacerait un travail réel ; la confondre avec une revue indépendante serait pire.
#[test]
fn une_revue_non_independante_existe_mais_ne_compte_pas_comme_independante() {
    let mut same_group = independent_reviewer();
    same_group.independence_group = Some("formalisation".to_owned());

    let review = Review::render(
        &frozen(),
        &generator(),
        &same_group,
        vec![evidenced_finding()],
        "relecture rapide",
    )
    .expect("la revue est rendue");

    assert!(!review.is_independent());
    assert_eq!(review.findings().len(), 1, "ses constats restent lisibles");
}

#[test]
fn relire_son_propre_travail_n_est_pas_une_revue() {
    assert_eq!(
        Review::render(
            &frozen(),
            &generator(),
            &generator(),
            vec![evidenced_finding()],
            "auto-relecture"
        ),
        Err(ReviewError::ReviewerIsAuthor)
    );
}

#[test]
fn une_revue_sans_couverture_est_refusee() {
    assert_eq!(
        Review::render(
            &frozen(),
            &generator(),
            &independent_reviewer(),
            Vec::new(),
            "   "
        ),
        Err(ReviewError::EmptyField { field: "coverage" })
    );
}

// ---------------------------------------------------------------------------------------------
// Un finding sans preuve ne décide de rien
// ---------------------------------------------------------------------------------------------

/// §17.5 : « un finding **sans preuve concrète** est un commentaire non bloquant et ne peut à lui
/// seul changer un niveau de validation. » La gravité déclarée ne suffit donc pas — sans quoi il
/// suffirait d'écrire `blocking` pour bloquer.
#[test]
fn un_finding_sans_preuve_ne_lie_personne_meme_declare_bloquant() {
    let loud = Finding::new(
        revision(1),
        "doute méthodologique",
        Severity::Blocking,
        Verdict::Refutes,
        Vec::new(),
    )
    .expect("finding valide");
    assert!(!loud.is_binding());

    let evidenced = evidenced_finding();
    assert!(evidenced.is_binding());

    let review = Review::render(
        &frozen(),
        &generator(),
        &independent_reviewer(),
        vec![loud, evidenced],
        "les deux points",
    )
    .expect("revue valide");
    assert_eq!(
        review.binding_findings().len(),
        1,
        "un seul des deux peut changer un niveau de validation"
    );
}

#[test]
fn un_finding_mineur_avec_preuve_ne_lie_pas_non_plus() {
    let minor = Finding::new(
        revision(1),
        "coquille",
        Severity::Minor,
        Verdict::Supports,
        vec![revision(7)],
    )
    .expect("finding valide");
    assert!(
        !minor.is_binding(),
        "avoir une preuve ne rend pas bloquant : les deux conditions comptent"
    );
}

/// §17.7 : la méta-revue « distingue absence de preuve, contradiction et réfutation ». Les trois
/// verdicts existent séparément, et `insufficient` n'est pas `refutes` — les confondre
/// transformerait un manque en résultat.
#[test]
fn absence_de_preuve_contradiction_et_refutation_sont_trois_verdicts() {
    let slugs: Vec<&str> = Verdict::ALL.into_iter().map(Verdict::slug).collect();
    assert_eq!(
        slugs,
        vec!["supports", "refutes", "insufficient", "not_applicable"]
    );
    assert_ne!(Verdict::Insufficient, Verdict::Refutes);
}

#[test]
fn les_quatre_gravites_de_17_5_sont_ordonnees() {
    assert!(Severity::Info < Severity::Minor);
    assert!(Severity::Major < Severity::Blocking);
    assert_eq!(
        Severity::ALL
            .into_iter()
            .map(Severity::slug)
            .collect::<Vec<_>>(),
        vec!["info", "minor", "major", "blocking"]
    );
}

#[test]
fn un_finding_sans_type_de_probleme_est_refuse() {
    assert_eq!(
        Finding::new(
            revision(1),
            "  ",
            Severity::Info,
            Verdict::Supports,
            Vec::new()
        ),
        Err(ReviewError::EmptyField {
            field: "issue_type"
        })
    );
}

/// Le dossier vise des révisions, jamais des concepts : §7.7 fait de `revision_id` l'identité d'une
/// version immuable, et relire « le dernier état » d'un objet rendrait la revue non reproductible.
#[test]
fn le_dossier_vise_des_revisions() {
    let dossier = frozen();
    assert_eq!(dossier.targets(), [revision(1)]);
    assert_ne!(revision(1), revision(2));
    let _ = id::<Branch>(1);
}
