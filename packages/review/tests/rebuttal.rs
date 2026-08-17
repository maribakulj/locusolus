//! Test de sortie de W7.d — **un rebuttal ne s'écrit pas sans constat ; une méta-revue ne relit
//! pas sa propre revue ; le désaccord survit à la synthèse.**
//!
//! Les trois protègent la même chose : qu'une procédure de revue reste une procédure, et ne
//! devienne pas une façon de produire un accord.

use locus_domain::RevisionId;
use locus_protocol::{Id, IdKind, Timestamp, id::Agent};
use locus_review::{
    Blindness, Draft, Finding, Frozen, IndependenceRequirement, Party, Rebuttal, RebuttalError,
    RecheckPolicy, Recommendation, Review, Severity, Verdict, assign_recheck, meta_review,
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

fn frozen() -> Frozen {
    Draft::open("dossier-0001", vec![revision(1)])
        .expect("dossier valide")
        .asking("la preuve tient-elle ?")
        .blind_to(Blindness::GeneratorTranscript)
        .requiring(IndependenceRequirement::DistinctIndependenceGroup)
        .freeze(
            locus_domain::ContentHash::parse(&format!("sha256:{}", "ab".repeat(32)))
                .expect("hash bien formé"),
        )
        .expect("dossier figé")
}

fn party(seed: u8, group: &str) -> Party {
    Party {
        agent_id: id::<Agent>(seed),
        worker_id: format!("vm-{seed:02}"),
        independence_group: Some(group.to_owned()),
        holds_generator_transcript: false,
    }
}

fn generator() -> Party {
    party(1, "formalisation")
}

fn refuting(seed: u8, group: &str) -> Review {
    Review::render(
        &frozen(),
        &generator(),
        &party(seed, group),
        vec![
            Finding::new(
                revision(1),
                "hypothèse non employée",
                Severity::Blocking,
                Verdict::Refutes,
                vec![revision(9)],
            )
            .expect("finding valide"),
        ],
        "la preuve, ligne à ligne",
    )
    .expect("revue valide")
}

fn supporting(seed: u8, group: &str) -> Review {
    Review::render(
        &frozen(),
        &generator(),
        &party(seed, group),
        vec![
            Finding::new(
                revision(1),
                "vérification indépendante",
                Severity::Info,
                Verdict::Supports,
                vec![revision(8)],
            )
            .expect("finding valide"),
        ],
        "la preuve, relue",
    )
    .expect("revue valide")
}

// ---------------------------------------------------------------------------------------------
// Un rebuttal ne s'écrit pas sans constat
// ---------------------------------------------------------------------------------------------

#[test]
fn un_rebuttal_vise_un_constat_et_dit_quelque_chose() {
    let rebuttal = Rebuttal::to_finding(
        revision(1),
        id::<Agent>(1),
        "l'hypothèse est employée au lemme 2, ligne 7",
    )
    .expect("rebuttal valide")
    .accepting("la formulation était ambiguë")
    .contesting("l'hypothèse serait inutile")
    .corrected_by(revision(20))
    .requesting_recheck();

    assert_eq!(rebuttal.finding_target(), &revision(1));
    assert_eq!(rebuttal.accepted().len(), 1);
    assert_eq!(rebuttal.contested().len(), 1);
    assert_eq!(rebuttal.corrections(), [revision(20)]);
    assert!(rebuttal.requests_recheck());
}

#[test]
fn une_reponse_vide_ne_repond_pas() {
    assert_eq!(
        Rebuttal::to_finding(revision(1), id::<Agent>(1), "   "),
        Err(RebuttalError::EmptyResponse)
    );
}

/// §17.6 : « la politique peut imposer un nouveau reviewer pour éviter l'auto-justification. » Les
/// deux politiques existent, et le choix est explicite — un défaut caché dans le code déciderait à
/// la place de la politique.
#[test]
fn la_politique_decide_qui_peut_reprendre_un_constat() {
    let initial = id::<Agent>(2);

    assert!(assign_recheck(RecheckPolicy::InitialReviewer, initial, initial).is_ok());
    assert_eq!(
        assign_recheck(RecheckPolicy::FreshReviewer, initial, initial),
        Err(RebuttalError::SelfJustification),
        "se relire soi-même est l'auto-justification que §17.6 nomme"
    );
    assert!(assign_recheck(RecheckPolicy::FreshReviewer, initial, id::<Agent>(3)).is_ok());
}

#[test]
fn le_defaut_laisse_le_relecteur_initial_reprendre() {
    // §17.6 dit « le reviewer initial **peut** effectuer un recheck » avant de mentionner la
    // politique plus stricte : le défaut suit le texte, et la restriction se demande.
    assert_eq!(RecheckPolicy::default(), RecheckPolicy::InitialReviewer);
}

// ---------------------------------------------------------------------------------------------
// Une méta-revue ne relit pas sa propre revue
// ---------------------------------------------------------------------------------------------

#[test]
fn un_meta_relecteur_ne_peut_pas_avoir_signe_une_des_revues() {
    let reviews = [refuting(2, "logique"), supporting(3, "statistique")];
    assert_eq!(
        meta_review(&reviews, id::<Agent>(2)),
        Err(RebuttalError::MetaReviewsItself),
        "une méta-revue de sa propre revue ne compare rien : elle se répète"
    );
    assert!(meta_review(&reviews, id::<Agent>(9)).is_ok());
}

/// §17.7 : la méta-revue « mesure l'indépendance **effective** ». Trois revues dont deux partagent
/// un groupe n'en font pas trois indépendantes, et compter les revues plutôt que leur indépendance
/// ferait passer un consensus pour une convergence.
#[test]
fn l_independance_effective_se_compte_et_n_est_pas_le_nombre_de_revues() {
    let reviews = [
        refuting(2, "logique"),
        // Même groupe que le générateur : cette revue n'est pas indépendante.
        supporting(3, "formalisation"),
    ];
    let meta = meta_review(&reviews, id::<Agent>(9)).expect("méta-revue valide");

    assert_eq!(reviews.len(), 2);
    assert_eq!(
        meta.effective_independence(),
        1,
        "deux revues, une seule indépendante"
    );
}

/// §17.7 : « détecte les findings corrélés ou copiés ». Deux relecteurs qui rendent exactement les
/// mêmes verdicts sur les mêmes cibles n'apportent pas deux avis — ce qui ne prouve pas la copie,
/// et c'est pour cela que la méta-revue le **signale** au lieu de conclure.
#[test]
fn deux_revues_identiques_sont_signalees_comme_correlees() {
    let reviews = [refuting(2, "logique"), refuting(3, "statistique")];
    let meta = meta_review(&reviews, id::<Agent>(9)).expect("méta-revue valide");

    assert_eq!(
        meta.correlated_reviewers(),
        [(id::<Agent>(2), id::<Agent>(3))]
    );
}

#[test]
fn deux_revues_differentes_ne_sont_pas_correlees() {
    let reviews = [refuting(2, "logique"), supporting(3, "statistique")];
    let meta = meta_review(&reviews, id::<Agent>(9)).expect("méta-revue valide");
    assert!(meta.correlated_reviewers().is_empty());
}

/// Deux relecteurs qui n'ont **rien** trouvé n'ont pas trouvé la même chose : ils n'ont rien
/// trouvé. Les signaler comme corrélés reviendrait à traiter l'absence de constat comme un constat
/// partagé, et à soupçonner de copie deux revues qui n'ont fait que ne rien reprocher.
///
/// Une revue sans constat est légitime — `Review::render` n'exige que la couverture — donc le cas
/// se produit sans qu'on le cherche.
#[test]
fn deux_revues_sans_aucun_constat_ne_sont_pas_correlees() {
    let silent = |seed: u8, group: &str| {
        Review::render(
            &frozen(),
            &generator(),
            &party(seed, group),
            Vec::new(),
            "la preuve, relue sans rien à redire",
        )
        .expect("une revue sans constat est une revue")
    };

    let reviews = [silent(2, "logique"), silent(3, "statistique")];
    let meta = meta_review(&reviews, id::<Agent>(9)).expect("méta-revue valide");

    assert!(
        meta.correlated_reviewers().is_empty(),
        "ne rien trouver deux fois n'est pas se copier"
    );
    assert_eq!(meta.recommendation(), Recommendation::Validate);
}

// ---------------------------------------------------------------------------------------------
// Le désaccord survit à la synthèse
// ---------------------------------------------------------------------------------------------

/// La garantie qui porte le sprint. §17.7 : la méta-revue « ne masque **jamais** les opinions
/// minoritaires ». Un désaccord résolu en le taisant produirait une recommandation nette et fausse.
#[test]
fn un_desaccord_produit_contest_et_garde_l_avis_minoritaire() {
    let reviews = [
        supporting(2, "logique"),
        supporting(3, "statistique"),
        refuting(4, "adversarial"),
    ];
    let meta = meta_review(&reviews, id::<Agent>(9)).expect("méta-revue valide");

    assert_eq!(
        meta.recommendation(),
        Recommendation::Contest,
        "le désaccord ne se résout pas à la synthèse : il y survit"
    );
    assert_eq!(
        meta.minority_opinions(),
        [id::<Agent>(4)],
        "l'unique réfutation est gardée, pas noyée dans deux soutiens"
    );
}

/// Et dans l'autre sens : l'unique **soutien** au milieu de réfutations est tout aussi minoritaire.
/// Ne garder que les opposants ferait disparaître la voix qui va contre le courant dominant, ce qui
/// est exactement ce que la règle cherche à empêcher.
#[test]
fn l_avis_minoritaire_est_le_moins_nombreux_quel_qu_il_soit() {
    let reviews = [
        refuting(2, "logique"),
        refuting(3, "statistique"),
        supporting(4, "adversarial"),
    ];
    let meta = meta_review(&reviews, id::<Agent>(9)).expect("méta-revue valide");

    assert_eq!(meta.recommendation(), Recommendation::Contest);
    assert_eq!(meta.minority_opinions(), [id::<Agent>(4)]);
}

#[test]
fn l_unanimite_favorable_recommande_de_valider() {
    let reviews = [supporting(2, "logique"), supporting(3, "statistique")];
    let meta = meta_review(&reviews, id::<Agent>(9)).expect("méta-revue valide");
    assert_eq!(meta.recommendation(), Recommendation::Validate);
    assert!(meta.minority_opinions().is_empty());
}

/// §17.7 demande que la méta-revue « distingue absence de preuve, contradiction et réfutation ».
/// Un relecteur qui dit « il n'y a pas de quoi conclure » ne réfute pas — et ne valide pas non plus.
/// Recommander `validate` ferait de l'absence de preuve une preuve, ce qui est la première des trois
/// confusions que §17.7 nomme.
#[test]
fn une_absence_de_preuve_ne_se_valide_pas_elle_se_revise() {
    let inconclusive = Review::render(
        &frozen(),
        &generator(),
        &party(3, "statistique"),
        vec![
            Finding::new(
                revision(1),
                "l'échantillon ne permet pas de trancher",
                Severity::Major,
                Verdict::Insufficient,
                vec![revision(7)],
            )
            .expect("finding valide"),
        ],
        "la preuve, relue",
    )
    .expect("revue valide");

    let reviews = [supporting(2, "logique"), inconclusive];
    let meta = meta_review(&reviews, id::<Agent>(9)).expect("méta-revue valide");

    assert_eq!(
        meta.recommendation(),
        Recommendation::Revise,
        "personne ne réfute, mais rien n'est établi"
    );
}

/// Et §17.5 vaut pour l'insuffisance comme pour la réfutation : dire « il n'y a pas de quoi
/// conclure » sans montrer sur quoi porte le manque reste un commentaire. Sans ce test, un doute non
/// étayé suffirait à empêcher une validation, ce qui donnerait à l'absence d'argument le pouvoir
/// qu'on refuse à l'absence de preuve.
#[test]
fn une_insuffisance_sans_preuve_n_empeche_pas_la_validation() {
    let unevidenced = Review::render(
        &frozen(),
        &generator(),
        &party(3, "statistique"),
        vec![
            Finding::new(
                revision(1),
                "j'ai un doute sur l'échantillon",
                Severity::Major,
                Verdict::Insufficient,
                Vec::new(),
            )
            .expect("finding valide"),
        ],
        "lecture rapide",
    )
    .expect("revue valide");

    let reviews = [supporting(2, "logique"), unevidenced];
    let meta = meta_review(&reviews, id::<Agent>(9)).expect("méta-revue valide");
    assert_eq!(meta.recommendation(), Recommendation::Validate);
}

#[test]
fn l_unanimite_defavorable_recommande_de_rejeter() {
    let reviews = [refuting(2, "logique"), refuting(3, "statistique")];
    let meta = meta_review(&reviews, id::<Agent>(9)).expect("méta-revue valide");
    assert_eq!(meta.recommendation(), Recommendation::Reject);
}

/// Aucune revue indépendante : il n'y a pas de quoi trancher. Recommander `validate` ferait d'un
/// **défaut de procédure** un verdict scientifique, ce qui est la façon la plus discrète de rendre
/// une revue inutile.
#[test]
fn sans_aucune_revue_independante_la_meta_revue_escalade() {
    let reviews = [
        supporting(2, "formalisation"),
        supporting(3, "formalisation"),
    ];
    let meta = meta_review(&reviews, id::<Agent>(9)).expect("méta-revue valide");

    assert_eq!(meta.effective_independence(), 0);
    assert_eq!(
        meta.recommendation(),
        Recommendation::HumanEscalation,
        "deux revues concordantes mais non indépendantes ne valident rien"
    );
}

#[test]
fn les_six_recommandations_de_17_7_existent() {
    let slugs: Vec<&str> = Recommendation::ALL
        .into_iter()
        .map(Recommendation::slug)
        .collect();
    assert_eq!(
        slugs,
        vec![
            "validate",
            "revise",
            "contest",
            "reject",
            "reproduce",
            "human_escalation"
        ]
    );
}

/// Un constat sans preuve ne pèse pas dans la synthèse non plus : §17.5 vaut partout, et une
/// réfutation non étayée ne doit pas suffire à faire basculer une méta-revue.
#[test]
fn une_refutation_sans_preuve_ne_fait_pas_basculer_la_meta_revue() {
    let unevidenced = Review::render(
        &frozen(),
        &generator(),
        &party(4, "adversarial"),
        vec![
            Finding::new(
                revision(1),
                "doute",
                Severity::Blocking,
                Verdict::Refutes,
                Vec::new(),
            )
            .expect("finding valide"),
        ],
        "lecture rapide",
    )
    .expect("revue valide");

    let reviews = [supporting(2, "logique"), unevidenced];
    let meta = meta_review(&reviews, id::<Agent>(9)).expect("méta-revue valide");
    assert_eq!(
        meta.recommendation(),
        Recommendation::Validate,
        "sans preuve, la réfutation reste un commentaire — §17.5"
    );
}
