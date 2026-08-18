//! Premier test de sortie de W7.f — **les sept formes de §13.6, chacune par une stratégie qui la
//! met en œuvre.**
//!
//! # Pourquoi « qui la met en œuvre »
//!
//! La roadmap le demande mot pour mot : « une stratégie qui optimise l'indicateur sans produire de
//! connaissance est détectée par un test **qui la met en œuvre** ». La différence est celle de W7.b
//! entre « je ne vois pas comment on tricherait » et « voici comment on triche ».
//!
//! Chaque cas construit donc la manœuvre, puis exige qu'elle soit vue — et chacun est doublé d'un
//! cas honnête qui lui ressemble, sans quoi le détecteur pourrait n'être qu'un refus général.

use locus_portfolio::{
    ArtifactRecord, BranchActivity, ClaimRecord, Gaming, GamingFinding, LexicalSimilarity,
    ReviewRecord, Similarity, Thresholds, screen,
};
use locus_protocol::{Id, IdKind, Timestamp, id::Agent};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn claim(statement: &str, evidence_count: usize) -> ClaimRecord {
    ClaimRecord {
        statement: statement.to_owned(),
        evidence_count,
        declared_confidence: 50,
        held_up: None,
    }
}

fn kinds(findings: &[GamingFinding]) -> Vec<Gaming> {
    findings.iter().map(|finding| finding.kind).collect()
}

fn crible(activity: &BranchActivity) -> Vec<Gaming> {
    kinds(screen(activity, &LexicalSimilarity, Thresholds::default()).findings())
}

// ---------------------------------------------------------------------------------------------
// Le cas honnête, pour que les sept autres veuillent dire quelque chose
// ---------------------------------------------------------------------------------------------

/// Une branche qui travaille : peu de revendications, toutes étayées, des tâches qui aboutissent,
/// des métriques tenues. Sans ce cas, un détecteur qui signalerait tout passerait pour vigilant.
#[test]
fn une_branche_honnete_ne_produit_aucun_constat() {
    let activity = BranchActivity {
        claims: vec![
            claim("le lemme 2 borne la variance", 3),
            claim("la borne est atteinte au point critique", 2),
            claim("le contre-exemple de Weil ne s'applique pas ici", 4),
            claim("la constante vaut 1/4 en dimension paire", 2),
            claim("la méthode échoue en dimension impaire", 3),
        ],
        tasks_created: 8,
        tasks_accepted: 6,
        preregistered_metrics: vec!["rmse".to_owned(), "coverage".to_owned()],
        reported_metrics: vec!["rmse".to_owned(), "coverage".to_owned()],
        ..BranchActivity::default()
    };

    assert!(crible(&activity).is_empty(), "{:?}", crible(&activity));
}

// ---------------------------------------------------------------------------------------------
// 1 — Multiplication artificielle de claims triviaux
// ---------------------------------------------------------------------------------------------

/// La manœuvre : au lieu d'une revendication étayée, en produire dix qu'aucune preuve ne soutient.
/// Le compte monte, la connaissance non — et c'est le compte que l'indicateur regarde.
#[test]
fn adverse_dix_revendications_sans_preuve_valent_mieux_qu_une_etayee() {
    let activity = BranchActivity {
        claims: (0..10)
            .map(|index| claim(&format!("observation numéro {index}"), 0))
            .collect(),
        ..BranchActivity::default()
    };

    assert_eq!(crible(&activity), vec![Gaming::TrivialClaimInflation]);
}

/// Et le cas voisin qui n'est pas la manœuvre : peu de revendications, dont une sans preuve. Le
/// volume ne dit rien en dessous du seuil, sans quoi toute branche naissante serait suspecte.
#[test]
fn deux_revendications_dont_une_sans_preuve_ne_disent_rien() {
    let activity = BranchActivity {
        claims: vec![claim("une piste", 0), claim("un résultat", 2)],
        ..BranchActivity::default()
    };
    assert!(crible(&activity).is_empty());
}

// ---------------------------------------------------------------------------------------------
// 2 — Inflation de confiance
// ---------------------------------------------------------------------------------------------

/// La manœuvre : déclarer 95 % de confiance partout. L'indicateur de « qualité des preuves » monte
/// sans qu'aucune preuve ne change — il suffit d'affirmer plus fort.
#[test]
fn adverse_declarer_quatre_vingt_quinze_pour_cent_partout() {
    let activity = BranchActivity {
        claims: (0..6)
            .map(|index| ClaimRecord {
                statement: format!("affirmation {index} sur un sujet distinct"),
                evidence_count: 2,
                declared_confidence: 95,
                held_up: Some(index % 4 == 0),
            })
            .collect(),
        ..BranchActivity::default()
    };

    let findings = screen(&activity, &LexicalSimilarity, Thresholds::default());
    assert!(
        kinds(findings.findings()).contains(&Gaming::ConfidenceInflation),
        "{:?}",
        kinds(findings.findings())
    );
}

/// Une confiance **calibrée** ne déclenche rien, même haute : c'est l'écart qui compte, pas
/// l'assurance. Punir la confiance justifiée découragerait exactement le bon comportement.
#[test]
fn une_confiance_haute_mais_calibree_ne_declenche_rien() {
    let activity = BranchActivity {
        claims: (0..6)
            .map(|index| ClaimRecord {
                statement: format!("affirmation {index} sur un sujet distinct"),
                evidence_count: 2,
                declared_confidence: 90,
                held_up: Some(index != 0),
            })
            .collect(),
        ..BranchActivity::default()
    };

    assert!(!crible(&activity).contains(&Gaming::ConfidenceInflation));
}

/// Sans revendication tranchée, il n'y a pas de calibration à mesurer — et supposer la pire serait
/// punir l'absence de recul.
#[test]
fn sans_revendication_tranchee_la_calibration_ne_se_prononce_pas() {
    let activity = BranchActivity {
        claims: vec![ClaimRecord {
            statement: "une conjecture récente".to_owned(),
            evidence_count: 2,
            declared_confidence: 99,
            held_up: None,
        }],
        ..BranchActivity::default()
    };
    assert!(!crible(&activity).contains(&Gaming::ConfidenceInflation));
}

// ---------------------------------------------------------------------------------------------
// 3 — Duplications paraphrastiques
// ---------------------------------------------------------------------------------------------

/// La manœuvre : reformuler la même revendication. Deux énoncés différents à la lettre, un seul
/// contenu — et deux fois le crédit.
#[test]
fn adverse_la_meme_revendication_reformulee() {
    let activity = BranchActivity {
        claims: vec![
            claim("la borne est atteinte au point critique", 2),
            claim("au point critique la borne est atteinte", 2),
        ],
        ..BranchActivity::default()
    };

    assert_eq!(crible(&activity), vec![Gaming::ParaphraseDuplication]);
}

/// Deux revendications qui partagent le sujet sans dire la même chose ne sont pas des doublons.
#[test]
fn deux_revendications_sur_le_meme_sujet_ne_sont_pas_des_doublons() {
    let activity = BranchActivity {
        claims: vec![
            claim("la borne est atteinte au point critique", 2),
            claim("la variance explose loin du régime stationnaire", 2),
        ],
        ..BranchActivity::default()
    };
    assert!(!crible(&activity).contains(&Gaming::ParaphraseDuplication));
}

/// La similarité est un **port**, et ce test le montre : un index qui déclare tout identique fait
/// tout signaler, sans que le détecteur change. C'est ce qui permettra de brancher une mesure
/// sémantique sans réécrire §13.6 — et ce qui dit que `LexicalSimilarity` n'est qu'un plancher.
#[test]
fn la_similarite_est_un_port_et_le_detecteur_n_en_depend_pas() {
    struct ToutEstPareil;
    impl Similarity for ToutEstPareil {
        fn score(&self, _: &str, _: &str) -> u8 {
            100
        }
    }

    let activity = BranchActivity {
        claims: vec![
            claim("la borne est atteinte au point critique", 2),
            claim("la variance explose loin du régime stationnaire", 2),
        ],
        ..BranchActivity::default()
    };

    let lexical = screen(&activity, &LexicalSimilarity, Thresholds::default());
    let semantic = screen(&activity, &ToutEstPareil, Thresholds::default());

    assert!(!kinds(lexical.findings()).contains(&Gaming::ParaphraseDuplication));
    assert!(kinds(semantic.findings()).contains(&Gaming::ParaphraseDuplication));
}

// ---------------------------------------------------------------------------------------------
// 4 — Production de tâches pour maximiser l'activité
// ---------------------------------------------------------------------------------------------

/// La manœuvre : ouvrir quarante tâches pour que la vélocité monte. Deux aboutissent.
#[test]
fn adverse_quarante_taches_ouvertes_deux_abouties() {
    let activity = BranchActivity {
        tasks_created: 40,
        tasks_accepted: 2,
        ..BranchActivity::default()
    };
    assert_eq!(crible(&activity), vec![Gaming::ActivityInflation]);
}

/// Une branche qui échoue honnêtement crée aussi des tâches qui n'aboutissent pas — mais peu, et le
/// seuil de volume est ce qui distingue l'échec de la manœuvre.
#[test]
fn trois_taches_dont_une_seule_aboutit_ne_disent_rien() {
    let activity = BranchActivity {
        tasks_created: 3,
        tasks_accepted: 1,
        ..BranchActivity::default()
    };
    assert!(crible(&activity).is_empty());
}

// ---------------------------------------------------------------------------------------------
// 5 — Collusion de reviewers
// ---------------------------------------------------------------------------------------------

/// La manœuvre : deux relecteurs qui s'approuvent l'un l'autre sans faute. Chaque revue est
/// régulière ; c'est la réciprocité parfaite qui ne l'est pas.
#[test]
fn adverse_deux_relecteurs_ne_se_refusent_jamais_rien() {
    let reviews = (0..3)
        .flat_map(|_| {
            [
                ReviewRecord {
                    reviewer: id::<Agent>(1),
                    author: id::<Agent>(2),
                    approves: true,
                },
                ReviewRecord {
                    reviewer: id::<Agent>(2),
                    author: id::<Agent>(1),
                    approves: true,
                },
            ]
        })
        .collect();

    let activity = BranchActivity {
        reviews,
        ..BranchActivity::default()
    };
    assert_eq!(crible(&activity), vec![Gaming::ReviewerCollusion]);
}

/// Un seul refus suffit à défaire l'entente : ce que le détecteur cherche est l'**absence** de
/// désaccord, pas l'accord fréquent. Deux relecteurs qui convergent souvent restent deux relecteurs.
#[test]
fn un_seul_refus_defait_l_entente() {
    let mut reviews: Vec<ReviewRecord> = (0..3)
        .flat_map(|_| {
            [
                ReviewRecord {
                    reviewer: id::<Agent>(1),
                    author: id::<Agent>(2),
                    approves: true,
                },
                ReviewRecord {
                    reviewer: id::<Agent>(2),
                    author: id::<Agent>(1),
                    approves: true,
                },
            ]
        })
        .collect();
    reviews.push(ReviewRecord {
        reviewer: id::<Agent>(2),
        author: id::<Agent>(1),
        approves: false,
    });

    let activity = BranchActivity {
        reviews,
        ..BranchActivity::default()
    };
    assert!(!crible(&activity).contains(&Gaming::ReviewerCollusion));
}

/// Et l'approbation systématique **dans un seul sens** n'est pas une entente : approuver quelqu'un
/// qui vous refuse parfois n'a rien de réciproque.
#[test]
fn l_approbation_a_sens_unique_n_est_pas_une_entente() {
    let mut reviews: Vec<ReviewRecord> = (0..4)
        .map(|_| ReviewRecord {
            reviewer: id::<Agent>(1),
            author: id::<Agent>(2),
            approves: true,
        })
        .collect();
    reviews.extend((0..4).map(|index| ReviewRecord {
        reviewer: id::<Agent>(2),
        author: id::<Agent>(1),
        approves: index != 0,
    }));

    let activity = BranchActivity {
        reviews,
        ..BranchActivity::default()
    };
    assert!(!crible(&activity).contains(&Gaming::ReviewerCollusion));
}

// ---------------------------------------------------------------------------------------------
// 6 — Fragmentation artificielle d'artefacts
// ---------------------------------------------------------------------------------------------

/// La manœuvre : un jeu de données livré en huit fichiers minuscules, pour que le compte
/// d'artefacts monte.
#[test]
fn adverse_un_jeu_de_donnees_livre_en_huit_miettes() {
    let activity = BranchActivity {
        artifacts: (0..8)
            .map(|_| ArtifactRecord {
                logical_unit: "dataset-alpha".to_owned(),
                size_bytes: 200,
            })
            .collect(),
        ..BranchActivity::default()
    };
    assert_eq!(crible(&activity), vec![Gaming::ArtifactFragmentation]);
}

/// Un jeu de données réellement gros, livré en huit parts **substantielles**, n'est pas une
/// fragmentation : c'est un découpage. Sans la taille, le détecteur compterait les fichiers et
/// punirait tout ensemble volumineux — exactement ce qu'un projet sérieux produit.
#[test]
fn un_ensemble_livre_en_huit_parts_substantielles_n_est_pas_un_decoupage() {
    let activity = BranchActivity {
        artifacts: (0..8)
            .map(|_| ArtifactRecord {
                logical_unit: "dataset-alpha".to_owned(),
                size_bytes: 40_000_000,
            })
            .collect(),
        ..BranchActivity::default()
    };
    assert!(crible(&activity).is_empty());
}

/// Des petits fichiers **d'ensembles différents** ne sont pas une fragmentation : c'est un projet
/// qui produit des choses petites, et l'interdire punirait la granularité légitime.
#[test]
fn des_petits_artefacts_d_ensembles_differents_ne_sont_pas_un_decoupage() {
    let activity = BranchActivity {
        artifacts: (0..8)
            .map(|index| ArtifactRecord {
                logical_unit: format!("figure-{index}"),
                size_bytes: 200,
            })
            .collect(),
        ..BranchActivity::default()
    };
    assert!(crible(&activity).is_empty());
}

// ---------------------------------------------------------------------------------------------
// 7 — Sélection opportuniste de métriques
// ---------------------------------------------------------------------------------------------

/// La manœuvre : pré-enregistrer trois métriques, n'en rapporter qu'une — celle qui est bonne.
#[test]
fn adverse_ne_rapporter_que_la_metrique_qui_arrange() {
    let activity = BranchActivity {
        preregistered_metrics: vec!["rmse".to_owned(), "mae".to_owned(), "coverage".to_owned()],
        reported_metrics: vec!["rmse".to_owned()],
        ..BranchActivity::default()
    };
    assert_eq!(crible(&activity), vec![Gaming::MetricCherryPicking]);
}

/// L'autre sens de la même manœuvre : rapporter une métrique qu'on n'avait pas annoncée, trouvée
/// après coup parce qu'elle est flatteuse.
#[test]
fn adverse_rapporter_une_metrique_trouvee_apres_coup() {
    let activity = BranchActivity {
        preregistered_metrics: vec!["rmse".to_owned()],
        reported_metrics: vec!["rmse".to_owned(), "r2_sur_le_sous_echantillon".to_owned()],
        ..BranchActivity::default()
    };
    assert_eq!(crible(&activity), vec![Gaming::MetricCherryPicking]);
}

/// Sans pré-enregistrement, il n'y a rien à comparer. Le silence n'est pas un aveu — et traiter
/// toute branche non pré-enregistrée comme tricheuse rendrait le détecteur inutilisable.
#[test]
fn sans_pre_enregistrement_la_selection_ne_se_constate_pas() {
    let activity = BranchActivity {
        reported_metrics: vec!["rmse".to_owned()],
        ..BranchActivity::default()
    };
    assert!(crible(&activity).is_empty());
}

// ---------------------------------------------------------------------------------------------
// Le criblage lui-même
// ---------------------------------------------------------------------------------------------

#[test]
fn les_sept_formes_de_13_6_sont_nommees() {
    let slugs: Vec<&str> = Gaming::ALL.into_iter().map(Gaming::slug).collect();
    assert_eq!(
        slugs,
        vec![
            "trivial_claim_inflation",
            "confidence_inflation",
            "paraphrase_duplication",
            "activity_inflation",
            "reviewer_collusion",
            "artifact_fragmentation",
            "metric_cherry_picking"
        ]
    );
}

/// Une manœuvre trouvée n'exclut pas les autres. S'arrêter au premier constat ferait réparer une
/// stratégie en laissant les six suivantes, et le rapport donnerait l'impression du contraire.
#[test]
fn une_branche_qui_triche_de_trois_facons_produit_trois_constats() {
    let activity = BranchActivity {
        claims: (0..10)
            .map(|index| claim(&format!("observation numéro {index}"), 0))
            .collect(),
        tasks_created: 40,
        tasks_accepted: 2,
        preregistered_metrics: vec!["rmse".to_owned(), "mae".to_owned()],
        reported_metrics: vec!["rmse".to_owned()],
        ..BranchActivity::default()
    };

    assert_eq!(
        crible(&activity),
        vec![
            Gaming::TrivialClaimInflation,
            Gaming::ActivityInflation,
            Gaming::MetricCherryPicking
        ]
    );
}

/// La pression est bornée. Une pénalité sans plafond permettrait à un seul détecteur mal réglé
/// d'annuler toute valeur — et un anti-gaming qui annule tout ne discrimine plus rien.
#[test]
fn la_pression_est_bornee() {
    let activity = BranchActivity {
        claims: (0..10)
            .map(|index| claim(&format!("observation numéro {index}"), 0))
            .collect(),
        tasks_created: 40,
        tasks_accepted: 0,
        preregistered_metrics: vec!["a".to_owned(), "b".to_owned(), "c".to_owned()],
        reported_metrics: vec!["z".to_owned()],
        artifacts: (0..20)
            .map(|_| ArtifactRecord {
                logical_unit: "dataset".to_owned(),
                size_bytes: 10,
            })
            .collect(),
        ..BranchActivity::default()
    };

    let screening = screen(&activity, &LexicalSimilarity, Thresholds::default());
    assert!(screening.findings().len() >= 4);
    assert_eq!(screening.pressure(), 100);
}

/// Un criblage sans constat n'est pas l'absence de criblage : il dit que la branche a été regardée,
/// et avec quels seuils. §13.4 exige que les paramètres soient enregistrés — un seuil qu'on ne
/// retrouve pas est une décision que personne n'a prise.
#[test]
fn un_criblage_propre_dit_avec_quels_seuils_il_a_regarde() {
    let severe = Thresholds {
        max_unsupported_percent: 10,
        ..Thresholds::default()
    };

    let activity = BranchActivity {
        claims: (0..6)
            .map(|index| {
                claim(
                    &format!("sujet distinct numéro {index}"),
                    usize::from(index < 4),
                )
            })
            .collect(),
        ..BranchActivity::default()
    };

    let indulgent = screen(&activity, &LexicalSimilarity, Thresholds::default());
    let strict = screen(&activity, &LexicalSimilarity, severe);

    assert!(indulgent.is_clean());
    assert!(!strict.is_clean());
    assert_eq!(indulgent.thresholds().max_unsupported_percent, 50);
    assert_eq!(strict.thresholds().max_unsupported_percent, 10);
}
