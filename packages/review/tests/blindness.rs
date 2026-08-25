//! Test de sortie de `W26.d` — **l'aveuglement du reviewer, et le second verdict qui paie le
//! dévoilement.**
//!
//! Trois clauses, celles de la roadmap :
//!
//! 1. un `Disclosure` vers un reviewer dont la revue est **ouverte** n'est pas constructible, et le
//!    test le tient par l'absence — l'invariant 11 est une borne sur le **mécanisme**, pas un défaut
//!    qu'un motif surclasse ;
//! 2. après un verdict enregistré, un dévoilement produit un **second** verdict qui le porte dans sa
//!    provenance, et **les deux sont conservés** — l'invariant 12 interdit de faire disparaître le
//!    premier ;
//! 3. `contamination::inspect` distingue le dévoilement de la fuite, et **le défaut reste la
//!    fuite** : un élément sans dévoilement valide attaché reste `GeneratorReasoningLeaked`.
//!
//! # L'agencement, et pourquoi les deux mécanismes ne se font pas confiance
//!
//! La clause 1 refuse à l'octroi ; la clause 3 refuse à l'inspection. Ce n'est pas une redondance :
//! `Standing::OutsideReview` est une **affirmation de l'appelant** — ce crate ne tient pas le
//! registre de qui relit quoi —, et c'est la garde de contamination qui la rattrape en retestant
//! l'aveuglement de son côté.

use locus_domain::{Confidentiality, ContentHash, RevisionId};
use locus_protocol::{Id, IdKind, Timestamp, id::Agent};
use locus_review::contamination::Finding as ContaminationFinding;
use locus_review::disclosure::{
    Contestation, Disclosure, DisclosureError, Reconsidered, Scope, Standing,
};
use locus_review::rebuttal::Rebuttal;
use locus_review::{
    Blindness, Contamination, ContextItem, Draft, Frozen, IndependenceRequirement, Party,
    Recipient, Review, inspect,
};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn id_de<K: IdKind>(graine: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = graine;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn revision(graine: u8) -> RevisionId {
    id_de::<locus_domain::ids::RevisionKind>(graine)
}

fn instant(iso: &str) -> Timestamp {
    Timestamp::parse(iso).expect("instant bien formé")
}

fn octroi() -> Timestamp {
    instant("2026-08-25T12:00:00.000Z")
}

fn echeance() -> Timestamp {
    instant("2026-08-25T18:00:00.000Z")
}

const GENERATEUR: u8 = 1;
const RELECTEUR: u8 = 2;
const TIERS: u8 = 3;

fn partie(graine: u8, groupe: &str) -> Party {
    Party {
        agent_id: id_de::<Agent>(graine),
        worker_id: format!("vm-{graine:02}"),
        independence_group: Some(groupe.to_owned()),
        holds_generator_transcript: false,
    }
}

fn dossier() -> Frozen {
    Draft::open("dossier-0001", vec![revision(1)])
        .expect("dossier valide")
        .asking("la preuve tient-elle ?")
        .blind_to(Blindness::GeneratorTranscript)
        .requiring(IndependenceRequirement::DistinctIndependenceGroup)
        .freeze(
            ContentHash::parse(&format!("sha256:{}", "ab".repeat(32))).expect("hash bien formé"),
        )
        .expect("le dossier se fige")
}

/// **Un verdict enregistré**, au sens de l'ADR : une revue rendue.
fn verdict_de(graine: u8) -> Review {
    Review::render(
        &dossier(),
        &partie(GENERATEUR, "alpha"),
        &partie(graine, "beta"),
        Vec::new(),
        "les trois mesures ont été refaites",
    )
    .expect("la revue se rend")
}

fn motif() -> locus_review::disclosure::Motive {
    let tour = Rebuttal::to_finding(
        revision(1),
        id_de::<Agent>(RELECTEUR),
        "la mesure ne tient pas",
    )
    .expect("réponse non vide")
    .contesting("le protocole")
    .requesting_recheck();
    let mut contestation = Contestation::on(revision(1));
    for _ in 0..3 {
        contestation = contestation.then(&tour);
    }
    contestation
        .unresolved_after(2)
        .expect("trois tours dépassent une borne de deux")
}

/// Un dévoilement vers le relecteur, **après** son verdict.
fn devoilement_apres_verdict() -> Disclosure {
    Disclosure::granting(
        motif(),
        Scope::one("art_raisonnement", id_de::<Agent>(RELECTEUR)),
        &Standing::recorded(&verdict_de(RELECTEUR)),
        octroi(),
        echeance(),
    )
    .expect("un verdict enregistré ouvre le dévoilement")
    .0
}

fn source(fichier: &str) -> String {
    let brut = std::fs::read_to_string(format!("{}/src/{fichier}", env!("CARGO_MANIFEST_DIR")))
        .expect("le module de production est lisible depuis son propre crate");
    brut.lines()
        .filter(|ligne| !ligne.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn relecteur_aveugle() -> Recipient {
    Recipient {
        agent_id: id_de::<Agent>(RELECTEUR),
        worker_id: "vm-02".to_owned(),
        blind_to_generator: true,
        clearance: Confidentiality::Internal,
    }
}

/// Le raisonnement du générateur dans le contexte de quelqu'un — la matière de §16.6, forme 1.
fn raisonnement_du_generateur(devoile: Option<Disclosure>) -> ContextItem {
    ContextItem {
        revision: revision(9),
        is_generator_reasoning: true,
        is_refuted: false,
        classification: Confidentiality::Internal,
        cites: Vec::new(),
        is_external_source: false,
        produced_by: Some(id_de::<Agent>(GENERATEUR)),
        disclosed: devoile,
    }
}

fn genres(findings: &[ContaminationFinding]) -> Vec<Contamination> {
    findings.iter().map(|finding| finding.kind).collect()
}

// ---------------------------------------------------------------------------------------------
// 1. Une revue ouverte ne se dévoile pas — et c'est inconstructible
// ---------------------------------------------------------------------------------------------

/// **Il n'existe aucun `Standing` qui exprime une revue ouverte.**
///
/// C'est là qu'est la garantie, et elle n'est pas une vérification : une revue ouverte est
/// l'**absence** d'une `Review` rendue, et `Standing::recorded` en exige une. Qui n'a pas le verdict
/// n'a pas la valeur, donc n'a pas de `Standing`, donc n'obtient pas de dévoilement.
///
/// L'invariant 11 est ainsi une borne sur le mécanisme, et non un défaut qu'un motif surclasserait.
#[test]
fn aucune_signature_ne_prend_une_revue_ouverte() {
    let code = source("disclosure.rs");

    // L'énumération n'a pas de barreau pour l'ouverture, et rien n'en fabrique un.
    for interdit in [
        "Open",
        "InProgress",
        "Pending",
        "Ongoing",
        "fn open",
        "fn regardless",
        "fn override",
        "fn force",
    ] {
        assert!(
            !code.contains(interdit),
            "« {interdit} » ouvrirait un dévoilement vers une revue en cours"
        );
    }

    // Et `recorded` prend bien une revue rendue, qui est la preuve du verdict.
    assert!(
        code.contains("pub const fn recorded(review: &Review) -> Self"),
        "le seul témoin de verdict se construit d'une revue rendue"
    );

    // Deux barreaux, et le second n'est pas une porte vers l'ouverture.
    let debut = code
        .find("pub enum Standing {")
        .expect("l'énumération existe");
    let fin = code[debut..].find("\n}").expect("elle se ferme") + debut;
    let corps = &code[debut..fin];
    assert!(corps.len() > 40, "extraction vide : voir la règle 3");
    assert!(corps.contains("OutsideReview"));
    assert!(corps.contains("Recorded(Id<Agent>)"));
}

/// **La revue close de l'un ne referme pas celle de l'autre.**
///
/// Sans ce refus, il aurait suffi de présenter n'importe quel verdict enregistré pour dévoiler vers
/// n'importe qui — le témoin serait devenu un laissez-passer transférable.
#[test]
fn un_verdict_d_autrui_n_ouvre_pas_le_devoilement() {
    let refus = Disclosure::granting(
        motif(),
        Scope::one("art_raisonnement", id_de::<Agent>(RELECTEUR)),
        &Standing::recorded(&verdict_de(TIERS)),
        octroi(),
        echeance(),
    );

    assert!(matches!(
        refus,
        Err(DisclosureError::SettledSomeoneElse { .. })
    ));
}

/// Le même dévoilement, avec le verdict **du lecteur visé**, passe.
///
/// La contre-épreuve : sans elle, un refus universel satisferait le test précédent.
#[test]
fn le_verdict_du_lecteur_vise_ouvre_le_devoilement() {
    let devoile = devoilement_apres_verdict();
    assert_eq!(devoile.scope().reader(), &id_de::<Agent>(RELECTEUR));
}

// ---------------------------------------------------------------------------------------------
// 2. Le second verdict, et les deux conservés
// ---------------------------------------------------------------------------------------------

/// **Les deux verdicts sont conservés, et le premier reste lisible.**
///
/// L'invariant 12 interdit de faire disparaître un résultat gênant, et un verdict rendu aveugle puis
/// révisé après lecture du raisonnement adverse en est exactement un. **L'écart entre les deux est
/// l'information que le conflit prolongé cherchait** — l'effacer reviendrait à jeter la réponse.
#[test]
fn le_second_verdict_ne_remplace_pas_le_premier() {
    let aveugle = verdict_de(RELECTEUR);
    let couverture_aveugle = aveugle.coverage().to_owned();

    let informe = Review::render(
        &dossier(),
        &partie(GENERATEUR, "alpha"),
        &partie(RELECTEUR, "beta"),
        Vec::new(),
        "après lecture du raisonnement, la troisième mesure s'explique",
    )
    .expect("la revue se rend");

    let deux = Reconsidered::after(aveugle, informe, devoilement_apres_verdict())
        .expect("le dévoilement vise bien le relecteur");

    assert_eq!(deux.blind().coverage(), couverture_aveugle);
    assert_eq!(
        deux.informed().coverage(),
        "après lecture du raisonnement, la troisième mesure s'explique"
    );
    assert_ne!(
        deux.blind().coverage(),
        deux.informed().coverage(),
        "l'écart entre les deux est l'information cherchée"
    );
}

/// Le second verdict **porte le dévoilement dans sa provenance**.
///
/// C'est ce qui distingue une reconsidération d'un changement d'avis : un lecteur du dossier peut
/// nommer ce qui a été montré, et à quel titre.
#[test]
fn le_second_verdict_porte_le_devoilement_dans_sa_provenance() {
    let deux = Reconsidered::after(
        verdict_de(RELECTEUR),
        verdict_de(RELECTEUR),
        devoilement_apres_verdict(),
    )
    .expect("le dévoilement vise bien le relecteur");

    assert_eq!(
        deux.disclosure().motive().reason(),
        locus_review::Reason::UnresolvedObjection
    );
    assert_eq!(
        deux.disclosure().scope().reader(),
        &id_de::<Agent>(RELECTEUR)
    );
}

/// Une reconsidération se rend **au nom de qui a rendu le premier verdict**.
///
/// Les deux bouts : un dévoilement qui vise quelqu'un d'autre, et un second verdict signé par
/// quelqu'un d'autre. Sans ces refus, « reconsidérer » serait un mot pour « faire relire par un
/// autre », ce qui est une revue de plus et non une reconsidération.
#[test]
fn une_reconsideration_ne_change_pas_de_relecteur() {
    let devoile_ailleurs = Disclosure::granting(
        motif(),
        Scope::one("art_raisonnement", id_de::<Agent>(TIERS)),
        &Standing::recorded(&verdict_de(TIERS)),
        octroi(),
        echeance(),
    )
    .expect("un verdict enregistré ouvre le dévoilement")
    .0;

    assert!(matches!(
        Reconsidered::after(
            verdict_de(RELECTEUR),
            verdict_de(RELECTEUR),
            devoile_ailleurs
        ),
        Err(DisclosureError::SettledSomeoneElse { .. })
    ));

    assert!(matches!(
        Reconsidered::after(
            verdict_de(RELECTEUR),
            verdict_de(TIERS),
            devoilement_apres_verdict()
        ),
        Err(DisclosureError::SettledSomeoneElse { .. })
    ));
}

/// Rien n'efface le premier verdict.
///
/// Tenu par l'absence : pas de `supersede`, pas de `replace`, pas de `retract`. L'invariant 12 le
/// demande, et une signature qui l'offrirait serait utilisée le jour où le premier verdict gênera.
#[test]
fn aucune_signature_n_efface_le_premier_verdict() {
    let code = source("disclosure.rs");
    for interdit in [
        "fn supersede",
        "fn replace",
        "fn retract",
        "fn withdraw",
        "fn discard",
        "fn overrule",
    ] {
        assert!(
            !code.contains(interdit),
            "« {interdit} » ferait disparaître le premier verdict, que l'invariant 12 conserve"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 3. Le défaut reste la fuite
// ---------------------------------------------------------------------------------------------

/// **Sans dévoilement attaché : la fuite.** C'est le défaut, et il ne change pas.
///
/// « Présumer régulier ce qui n'est pas prouvé irrégulier ferait de l'oubli d'attacher le
/// dévoilement un silence » — ADR 0027 décision 6.
#[test]
fn sans_devoilement_le_raisonnement_du_generateur_reste_une_fuite() {
    let constats = inspect(
        &[raisonnement_du_generateur(None)],
        &relecteur_aveugle(),
        octroi(),
    );
    assert_eq!(
        genres(&constats),
        vec![Contamination::GeneratorReasoningLeaked]
    );
}

/// **Avec un dévoilement valide attaché : ce n'est plus une fuite.**
///
/// C'est la clause qui a été vue **rouge d'abord** : avant que `inspect` n'apprenne la différence,
/// ce test échouait en signalant une fuite sur un dévoilement régulier — précisément le
/// comportement que la leçon de `W22.d` proscrit, une garde qui crie sur ce qui est juste et se fait
/// désactiver.
#[test]
fn un_devoilement_valide_n_est_pas_une_fuite() {
    let constats = inspect(
        &[raisonnement_du_generateur(
            Some(devoilement_apres_verdict()),
        )],
        &relecteur_aveugle(),
        octroi(),
    );
    assert_eq!(
        genres(&constats),
        Vec::<Contamination>::new(),
        "un dévoilement régulier n'est pas une fuite"
    );
}

/// Un dévoilement **expiré** redevient une fuite.
///
/// L'échéance ne s'arrête pas à la lecture : un contexte qui porterait un dévoilement périmé est
/// exactement le cas où l'autorisation a cessé et où le contenu circule encore.
#[test]
fn un_devoilement_expire_redevient_une_fuite() {
    let constats = inspect(
        &[raisonnement_du_generateur(
            Some(devoilement_apres_verdict()),
        )],
        &relecteur_aveugle(),
        instant("2026-08-25T18:00:00.001Z"),
    );
    assert_eq!(
        genres(&constats),
        vec![Contamination::GeneratorReasoningLeaked]
    );
}

/// Un dévoilement qui vise **quelqu'un d'autre** ne couvre pas ce destinataire-ci.
///
/// Le cas adverse le plus utile : un dévoilement régulier existe, il est simplement attaché au
/// mauvais contexte. Une garde qui vérifierait seulement « un dévoilement est présent » le laisserait
/// passer, et l'attacher deviendrait une formalité.
#[test]
fn un_devoilement_vers_un_autre_lecteur_reste_une_fuite() {
    let ailleurs = Disclosure::granting(
        motif(),
        Scope::one("art_raisonnement", id_de::<Agent>(TIERS)),
        &Standing::recorded(&verdict_de(TIERS)),
        octroi(),
        echeance(),
    )
    .expect("un verdict enregistré ouvre le dévoilement")
    .0;

    let constats = inspect(
        &[raisonnement_du_generateur(Some(ailleurs))],
        &relecteur_aveugle(),
        octroi(),
    );
    assert_eq!(
        genres(&constats),
        vec![Contamination::GeneratorReasoningLeaked]
    );
}

/// Le dévoilement ne blanchit **que** la fuite de raisonnement.
///
/// Les quatre autres formes de §16.6 lui sont étrangères : un dévoilement autorise à lire un
/// raisonnement, il ne dit rien d'une donnée confidentielle sur un worker non habilité. Les
/// confondre ferait du dévoilement un passe-partout.
#[test]
fn un_devoilement_ne_blanchit_que_la_fuite_de_raisonnement() {
    let mut item = raisonnement_du_generateur(Some(devoilement_apres_verdict()));
    item.classification = Confidentiality::Restricted;

    let constats = inspect(&[item], &relecteur_aveugle(), octroi());
    assert_eq!(
        genres(&constats),
        vec![Contamination::ConfidentialDataOnUnauthorisedWorker],
        "la classification reste jugée, et la fuite de raisonnement est levée"
    );
}
