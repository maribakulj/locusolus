//! Le refus d'admission sur le fil — le test de sortie de `W19.a`.
//!
//! Les deux propriétés qu'ADR 0017 §5.2 dit « à ne pas perdre en traduisant » sont ici, et elles
//! sont testées sur ce qui voyage — le JSON — plutôt que sur les types Rust. Un test qui
//! comparerait deux `Reason` prouverait que la traduction est injective en mémoire ; ce qu'on veut
//! savoir est ce qu'un pair lit.

use locus_execd::admission::RefusalReason;
use locus_execd::wire;
use locus_execution::SandboxLevel;
use serde_json::Value;

fn code(reason: &RefusalReason) -> String {
    let value = serde_json::to_value(wire::reason(reason)).expect("un motif s'encode");
    value["code"]
        .as_str()
        .expect("un motif porte son code")
        .to_owned()
}

/// **Un refus voyage avec tous ses motifs, jamais le premier seul.**
///
/// C'est la première des deux propriétés, et la plus coûteuse à perdre : un fil qui n'en
/// transmettrait qu'une ferait corriger une condition, relancer, découvrir la suivante — autant
/// d'allers-retours qu'il manque de conditions. Le test porte sur quatre motifs simultanés, et
/// vérifie **l'ordre** en plus du nombre : un `HashSet` quelque part dans la chaîne le perdrait
/// sans rien casser d'autre.
#[test]
fn un_refus_voyage_avec_tous_ses_motifs() {
    let reasons = vec![
        RefusalReason::LevelUnavailable {
            required: SandboxLevel::S4,
            best: SandboxLevel::S2,
        },
        RefusalReason::CapacityExceeded,
        RefusalReason::AcceleratorUnavailable {
            kind: "cuda".to_owned(),
        },
        RefusalReason::DiskQuotaNotEnforceable {
            requested: 12_000_000_000,
            why: "overlayfs sur ext4".to_owned(),
        },
    ];

    let document = wire::refusal("task-1", "attempt-1", &reasons);
    let encoded = serde_json::to_value(&document).expect("le document s'encode");
    let carried = encoded["reasons"].as_array().expect("un tableau");

    assert_eq!(carried.len(), 4, "aucun motif ne tombe en route");
    assert_eq!(
        carried
            .iter()
            .map(|reason| reason["code"].as_str().expect("un code"))
            .collect::<Vec<_>>(),
        [
            "level_unavailable",
            "capacity_exceeded",
            "accelerator_unavailable",
            "disk_quota_not_enforceable"
        ],
        "et l'ordre est celui que l'admission a trouvé"
    );
}

/// **`level_not_attested` et `level_unavailable` restent deux refus distincts.**
///
/// « L'hôte ne sait pas faire » envoie chercher une autre machine ; « l'hôte l'annonce sans l'avoir
/// prouvé » envoie faire tourner une campagne de self-tests. Les fondre ferait acheter du matériel
/// pour un problème d'attestation.
///
/// Tenu par **égalité stricte** sur les codes, pas par lecture de phrase : une assertion qui
/// chercherait « attested » dans un message passerait encore le jour où les deux motifs
/// partageraient un texte.
#[test]
fn les_deux_paires_qui_se_ressemblent_ne_se_fondent_pas() {
    let indisponible = code(&RefusalReason::LevelUnavailable {
        required: SandboxLevel::S4,
        best: SandboxLevel::S2,
    });
    let non_atteste = code(&RefusalReason::LevelNotAttested {
        required: SandboxLevel::S2,
        proven: None,
    });
    assert_eq!(indisponible, "level_unavailable");
    assert_eq!(non_atteste, "level_not_attested");
    assert_ne!(indisponible, non_atteste);

    // La seconde paire, que l'ADR nomme au même titre.
    let absent = code(&RefusalReason::AcceleratorUnavailable {
        kind: "cuda".to_owned(),
    });
    let ailleurs = code(&RefusalReason::AcceleratorOutsideSandbox {
        kind: "cuda".to_owned(),
        required: SandboxLevel::S3,
        native_level: SandboxLevel::S1,
    });
    assert_eq!(absent, "accelerator_unavailable");
    assert_eq!(ailleurs, "accelerator_outside_sandbox");
    assert_ne!(absent, ailleurs);
}

/// Les sept motifs ont chacun leur code, et aucun ne se répète.
///
/// L'exhaustivité de la traduction est tenue par le compilateur — le `match` de `wire::reason` n'a
/// pas de branche fourre-tout. Ce que ce test ajoute est que les sept codes sont **distincts** :
/// un copier-coller qui donnerait deux fois le même compilerait très bien.
#[test]
fn les_sept_motifs_ont_sept_codes_distincts() {
    let tous = [
        RefusalReason::LevelUnavailable {
            required: SandboxLevel::S4,
            best: SandboxLevel::S2,
        },
        RefusalReason::CapacityExceeded,
        RefusalReason::AcceleratorUnavailable {
            kind: "cuda".to_owned(),
        },
        RefusalReason::DiskQuotaNotEnforceable {
            requested: 1,
            why: "ext4".to_owned(),
        },
        RefusalReason::NetworkModeUnsupported { mode: "allowlist" },
        RefusalReason::LevelNotAttested {
            required: SandboxLevel::S2,
            proven: Some(SandboxLevel::S1),
        },
        RefusalReason::AcceleratorOutsideSandbox {
            kind: "mps".to_owned(),
            required: SandboxLevel::S3,
            native_level: SandboxLevel::S1,
        },
    ];

    let codes: Vec<String> = tous.iter().map(code).collect();
    let mut uniques = codes.clone();
    uniques.sort();
    uniques.dedup();
    assert_eq!(codes.len(), 7);
    assert_eq!(
        uniques.len(),
        7,
        "deux motifs partageraient un code : {codes:?}"
    );
}

/// **Chaque niveau se traduit par le sien**, et pas un seulement par accident.
///
/// La traduction est un `match` de six lignes qui se copient-collent, et un mutant qui envoyait
/// `S4` sur `S3` a **survécu** à tout le reste de cette suite : les autres tests portent sur les
/// codes de motif, jamais sur les niveaux qu'ils transportent. Un refus qui dirait « hôte au mieux
/// en S3 » d'un hôte en S4 enverrait chercher une machine qu'on a déjà.
#[test]
fn chaque_niveau_se_traduit_par_le_sien() {
    let attendus = [
        (SandboxLevel::S0, "S0"),
        (SandboxLevel::S1, "S1"),
        (SandboxLevel::S2, "S2"),
        (SandboxLevel::S3, "S3"),
        (SandboxLevel::S4, "S4"),
        (SandboxLevel::S5, "S5"),
    ];
    for (niveau, code) in attendus {
        let value = serde_json::to_value(wire::reason(&RefusalReason::LevelUnavailable {
            required: niveau,
            best: SandboxLevel::S0,
        }))
        .expect("encodage");
        assert_eq!(value["required"], Value::String(code.to_owned()), "{code}");
    }

    // Et les deux positions ne se confondent pas : `required` et `best` viennent du même
    // traducteur, et une inversion passerait les six assertions ci-dessus.
    let value = serde_json::to_value(wire::reason(&RefusalReason::LevelUnavailable {
        required: SandboxLevel::S4,
        best: SandboxLevel::S1,
    }))
    .expect("encodage");
    assert_eq!(value["required"], Value::String("S4".to_owned()));
    assert_eq!(value["best"], Value::String("S1".to_owned()));
}

/// **`proven` absent n'est pas `proven` bas**, et le fil le dit en n'écrivant pas la clé.
///
/// « Aucune campagne n'a conclu » et « la campagne a conclu plus bas » envoient chercher deux
/// choses différentes. Un `null` ou un niveau par défaut les confondrait.
#[test]
fn une_attestation_absente_ne_s_ecrit_pas() {
    let sans = serde_json::to_value(wire::reason(&RefusalReason::LevelNotAttested {
        required: SandboxLevel::S3,
        proven: None,
    }))
    .expect("encodage");
    assert_eq!(sans.get("proven"), None, "absent, pas null");

    let avec = serde_json::to_value(wire::reason(&RefusalReason::LevelNotAttested {
        required: SandboxLevel::S3,
        proven: Some(SandboxLevel::S1),
    }))
    .expect("encodage");
    assert_eq!(avec["proven"], Value::String("S1".to_owned()));
}

/// Le document se relit tel qu'il a été écrit.
#[test]
fn le_document_fait_l_aller_retour() {
    let document = wire::refusal(
        "task-1",
        "attempt-1",
        &[RefusalReason::LevelNotAttested {
            required: SandboxLevel::S2,
            proven: None,
        }],
    );
    let encoded = serde_json::to_value(&document).expect("encodage");
    let decoded: locus_lep::AdmissionRefusal =
        serde_json::from_value(encoded.clone()).expect("décodage");
    assert_eq!(
        serde_json::to_value(&decoded).expect("ré-encodage"),
        encoded
    );
    assert_eq!(document.protocol, "lep/1.1");
}
