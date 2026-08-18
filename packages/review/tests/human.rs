//! Test de sortie de W7.h — **un verdict humain ne vaut jamais une validation.**
//!
//! `xiiif/SPEC_V1.md` §20 : « cette revue n'est pas une validation scientifique complète. Elle
//! produit un finding humain attachable à un `ReviewDossier`. »
//!
//! Deux exigences opposées. Le finding est réel — il s'attache, il se compte, il ne se perd pas —
//! et il ne peut jamais tenir lieu de preuve. Une seule porte fermée suffit à tenir les deux, et
//! c'est le premier test : aucun chemin ne rend `Supports`.

use locus_domain::{ContentHash, RevisionId, ids::RevisionKind};
use locus_protocol::{Id, IdKind, Timestamp};
use locus_review::{Draft, Frozen, HumanReview, HumanReviewError, HumanVerdict, Severity, Verdict};

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn revision(seed: u8) -> RevisionId {
    id::<RevisionKind>(seed)
}

fn dossier() -> Frozen {
    Draft::open("dossier-0007", vec![revision(1), revision(2)])
        .expect("dossier valide")
        .asking("la région revendiquée est-elle celle que la transcription cite ?")
        .freeze(ContentHash::parse(&format!("sha256:{}", "ab".repeat(32))).expect("hash"))
        .expect("dossier gelable")
}

fn review(verdict: Option<HumanVerdict>, comment: Option<&str>) -> HumanReview {
    HumanReview::record(
        "dossier-0007",
        revision(1),
        "human:archiviste-12",
        verdict,
        comment,
    )
    .expect("revue valide")
}

// ---------------------------------------------------------------------------------------------
// La porte fermée
// ---------------------------------------------------------------------------------------------

/// Le cœur de §20. Aucun verdict, aucune combinaison de verdict et de commentaire, aucune preuve
/// citée ne fait dire à une revue humaine que la revendication tient. Un coup d'œil dans une
/// visionneuse n'est pas une preuve, et le type est ce qui empêche de l'oublier.
#[test]
fn aucune_revue_humaine_ne_rend_supports() {
    for verdict in HumanVerdict::ALL {
        assert_ne!(
            verdict.verdict(),
            Verdict::Supports,
            "« {verdict} » ne peut pas soutenir une revendication"
        );
        for comment in [None, Some("tout est en ordre selon moi")] {
            let review = review(Some(verdict), comment).citing(vec![revision(2)]);
            assert_ne!(review.verdict_as_finding(), Verdict::Supports);
            let finding = review.attach_to(&dossier()).expect("finding attachable");
            assert_ne!(finding.verdict(), Verdict::Supports);
        }
    }
    assert_ne!(
        review(None, Some("la marge coupe la réclame")).verdict_as_finding(),
        Verdict::Supports
    );
}

/// `accept` est le cas où la tentation est la plus forte : quelqu'un a regardé et n'a rien trouvé
/// à redire. C'est une absence d'objection, pas une preuve — et §17.5 a déjà le mot pour cela.
#[test]
fn accepter_est_une_absence_d_objection_pas_une_preuve() {
    assert_eq!(HumanVerdict::Accept.verdict(), Verdict::Insufficient);
    assert_eq!(HumanVerdict::Accept.severity(), Severity::Info);
}

/// §19 dans le vocabulaire de §20. Une source qui a bougé est un fait sur la source ; le rendre
/// `refutes` ferait douter d'un run correct chaque fois qu'une bibliothèque remanie son site.
#[test]
fn une_source_qui_a_bouge_ne_refute_rien() {
    assert_eq!(
        HumanVerdict::SourceChanged.verdict(),
        Verdict::NotApplicable
    );
    assert_ne!(HumanVerdict::SourceChanged.verdict(), Verdict::Refutes);
}

/// Les deux verdicts qui contestent quelque chose le font, et ils ne se confondent pas :
/// `wrong-target` ne discute pas la conclusion, il dit que rien ne porte sur le bon objet.
#[test]
fn les_deux_verdicts_qui_contestent_ne_se_confondent_pas() {
    assert_eq!(HumanVerdict::NeedsCorrection.verdict(), Verdict::Refutes);
    assert_eq!(HumanVerdict::WrongTarget.verdict(), Verdict::Refutes);
    assert_eq!(HumanVerdict::NeedsCorrection.severity(), Severity::Major);
    assert_eq!(HumanVerdict::WrongTarget.severity(), Severity::Blocking);
}

// ---------------------------------------------------------------------------------------------
// Les cinq façons de s'exprimer de §20
// ---------------------------------------------------------------------------------------------

#[test]
fn les_quatre_verdicts_de_20_existent_sous_leur_nom() {
    let slugs: Vec<&str> = HumanVerdict::ALL.iter().map(|v| v.slug()).collect();
    assert_eq!(
        slugs,
        vec![
            "accept",
            "needs-correction",
            "wrong-target",
            "source-changed"
        ]
    );
    for verdict in HumanVerdict::ALL {
        assert_eq!(HumanVerdict::from_slug(verdict.slug()), Some(verdict));
    }
    assert_eq!(HumanVerdict::from_slug("validated"), None);
}

/// Chaque verdict laisse une trace distincte : deux qui produiraient le même finding se
/// compteraient comme un seul, et la revue perdrait ce qu'elle avait dit.
#[test]
fn chaque_facon_de_s_exprimer_laisse_une_trace_distincte() {
    let mut kinds: Vec<String> = HumanVerdict::ALL
        .iter()
        .map(|verdict| review(Some(*verdict), None).issue_type())
        .collect();
    kinds.push(review(None, Some("un doute sur la marge")).issue_type());

    let mut unique = kinds.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 5, "cinq façons, cinq traces : {kinds:?}");
    assert_eq!(kinds[4], "human-review:comment");
}

/// §20 dit « ou commentaire libre » : un enregistrement sans verdict est légitime.
#[test]
fn un_commentaire_libre_seul_est_une_revue() {
    let review = review(
        None,
        Some("la région est correcte, la marge coupe la réclame"),
    );
    assert_eq!(review.verdict(), None);
    assert_eq!(review.verdict_as_finding(), Verdict::Insufficient);
    assert_eq!(review.severity(), Severity::Info);
}

#[test]
fn une_revue_qui_ne_dit_rien_est_refusee() {
    assert_eq!(
        HumanReview::record("dossier-0007", revision(1), "human:x", None, None),
        Err(HumanReviewError::SaysNothing)
    );
    // Un commentaire vide ne dit rien non plus : accepter l'espace ferait passer la garde pour
    // une vérification de présence de champ, ce qu'elle n'est pas.
    assert_eq!(
        HumanReview::record("dossier-0007", revision(1), "human:x", None, Some("   ")),
        Err(HumanReviewError::SaysNothing)
    );
}

#[test]
fn un_dossier_ou_un_relecteur_vide_est_refuse() {
    assert_eq!(
        HumanReview::record(
            "  ",
            revision(1),
            "human:x",
            Some(HumanVerdict::Accept),
            None
        ),
        Err(HumanReviewError::EmptyField {
            field: "dossier_id"
        })
    );
    assert_eq!(
        HumanReview::record("d-1", revision(1), " ", Some(HumanVerdict::Accept), None),
        Err(HumanReviewError::EmptyField { field: "reviewer" })
    );
}

// ---------------------------------------------------------------------------------------------
// « attachable à un ReviewDossier »
// ---------------------------------------------------------------------------------------------

#[test]
fn une_revue_s_attache_au_dossier_qu_elle_vise() {
    let finding = review(Some(HumanVerdict::NeedsCorrection), None)
        .attach_to(&dossier())
        .expect("finding attachable");
    assert_eq!(finding.issue_type(), "human-review:needs-correction");
    assert_eq!(finding.verdict(), Verdict::Refutes);
}

/// Le dossier est figé avant attribution (§17.3). Une revue humaine qui porterait sur autre chose
/// que ce qu'il couvre l'élargirait sans jamais le contredire ouvertement — c'est la forme de
/// dérive qui ne se voit pas, donc celle qu'un type doit refuser.
#[test]
fn une_cible_hors_du_dossier_est_refusee() {
    let hors_dossier = HumanReview::record(
        "dossier-0007",
        revision(9),
        "human:x",
        Some(HumanVerdict::Accept),
        None,
    )
    .expect("revue valide");
    assert_eq!(
        hors_dossier.attach_to(&dossier()),
        Err(HumanReviewError::TargetNotInDossier)
    );
}

#[test]
fn une_revue_ne_s_attache_pas_a_un_autre_dossier() {
    let autre = Draft::open("dossier-0008", vec![revision(1)])
        .expect("dossier valide")
        .asking("la même question, sur un autre dossier")
        .freeze(ContentHash::parse(&format!("sha256:{}", "cd".repeat(32))).expect("hash"))
        .expect("dossier gelable");
    assert_eq!(
        review(Some(HumanVerdict::Accept), None).attach_to(&autre),
        Err(HumanReviewError::WrongDossier {
            expected: "dossier-0007".to_owned(),
            found: "dossier-0008".to_owned(),
        })
    );
}

// ---------------------------------------------------------------------------------------------
// Opposable, ou non
// ---------------------------------------------------------------------------------------------

/// §17.5 : « un finding sans preuve concrète est un commentaire non bloquant. » La règle porte sur
/// la preuve, pas sur la qualité du relecteur — la vérifier ici est ce qui empêche d'écrire un jour
/// une exception « pour les humains », dans un sens ou dans l'autre.
#[test]
fn sans_preuve_citee_meme_wrong_target_ne_bloque_pas() {
    let sans = review(Some(HumanVerdict::WrongTarget), None)
        .attach_to(&dossier())
        .expect("finding attachable");
    assert!(!sans.is_binding());

    let avec = review(Some(HumanVerdict::WrongTarget), None)
        .citing(vec![revision(2)])
        .attach_to(&dossier())
        .expect("finding attachable");
    assert!(avec.is_binding());
}

/// Et l'inverse : une preuve citée ne rend pas opposable ce qui n'est qu'une remarque. Les deux
/// conditions de §17.5 tiennent ensemble, et un test qui n'éprouverait que l'une laisserait passer
/// une garde qui a perdu l'autre.
#[test]
fn une_preuve_citee_ne_rend_pas_opposable_un_simple_commentaire() {
    let finding = review(None, Some("la marge coupe la réclame"))
        .citing(vec![revision(2)])
        .attach_to(&dossier())
        .expect("finding attachable");
    assert!(!finding.is_binding());
    assert_eq!(finding.evidence(), &[revision(2)]);
}

/// Invariant 12 : ce qui a été dit ne disparaît pas parce que le verdict est favorable.
#[test]
fn le_commentaire_survit_a_un_verdict_favorable() {
    let review = review(
        Some(HumanVerdict::Accept),
        Some("lisible, mais le contraste est faible"),
    );
    assert_eq!(
        review.comment(),
        Some("lisible, mais le contraste est faible")
    );
    assert_eq!(review.verdict(), Some(HumanVerdict::Accept));
}

// ---------------------------------------------------------------------------------------------
// Le lecteur validant — ce que le type engendré ne peut pas dire
// ---------------------------------------------------------------------------------------------

fn wire() -> locus_lep::HumanReviewFinding {
    locus_lep::HumanReviewFinding {
        dossier_id: "dossier-0007".to_owned(),
        target: revision(1).to_string(),
        reviewer: "human:archiviste-12".to_owned(),
        verdict: Some("source-changed".to_owned()),
        comment: None,
        evidence: None,
        recorded_at: None,
    }
}

#[test]
fn un_document_bien_forme_se_relit() {
    let review = HumanReview::from_wire(&wire()).expect("document valide");
    assert_eq!(review.verdict(), Some(HumanVerdict::SourceChanged));
    assert_eq!(review.target(), revision(1));
    assert_eq!(review.dossier_id(), "dossier-0007");
}

/// Le schéma porte `anyOf: [{required: verdict}, {required: comment}]` ; Rust ne sait pas
/// l'exprimer, donc le type engendré offre deux `Option` indépendants. Un document muet le traverse
/// sans bruit, et la règle ne serait tenue que par le validateur JSON — c'est-à-dire nulle part, dès
/// qu'un producteur construit la valeur en mémoire.
#[test]
fn un_document_muet_est_refuse_par_le_domaine() {
    let mut muet = wire();
    muet.verdict = None;
    assert_eq!(
        HumanReview::from_wire(&muet),
        Err(HumanReviewError::SaysNothing)
    );
}

/// Et le refus qui compte le plus : un verdict que §20 ne nomme pas. Le laisser passer comme une
/// chaîne libre le ferait entrer au dossier sous un nom que personne n'a défini — `validated` étant
/// exactement le mot que §20 interdit.
#[test]
fn un_verdict_invente_est_refuse() {
    let mut invente = wire();
    invente.verdict = Some("validated".to_owned());
    assert_eq!(
        HumanReview::from_wire(&invente),
        Err(HumanReviewError::UnknownVerdict {
            value: "validated".to_owned()
        })
    );
}

#[test]
fn une_revision_illisible_est_refusee() {
    let mut cassee = wire();
    cassee.target = "pas-une-revision".to_owned();
    assert_eq!(
        HumanReview::from_wire(&cassee),
        Err(HumanReviewError::MalformedId {
            value: "pas-une-revision".to_owned()
        })
    );

    // Y compris dans les preuves, où une révision illisible passerait pour une preuve de plus.
    let mut preuve = wire();
    preuve.evidence = Some(vec![revision(2).to_string(), "  ".to_owned()]);
    assert!(matches!(
        HumanReview::from_wire(&preuve),
        Err(HumanReviewError::MalformedId { .. })
    ));
}

#[test]
fn les_preuves_du_document_arrivent_dans_le_finding() {
    let mut avec = wire();
    avec.verdict = Some("wrong-target".to_owned());
    avec.evidence = Some(vec![revision(2).to_string()]);
    let finding = HumanReview::from_wire(&avec)
        .expect("document valide")
        .attach_to(&dossier())
        .expect("finding attachable");
    assert_eq!(finding.evidence(), &[revision(2)]);
    assert!(finding.is_binding());
}
