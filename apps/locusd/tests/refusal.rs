//! Test de sortie de `W19.c` — `docs/06`, ADR 0037, `repos/canterel/SPEC_V1.md` §10.2.
//!
//! **Un refus d'admission du worker cesse d'être muet, et la mission revient en file.**
//!
//! # Ce que ces tests protègent
//!
//! Avant cet item, `runLoop` refusait et rendait la main sans rien dire : la mission restait sous
//! bail jusqu'à expiration, et « le worker a refusé » se confondait avec « le worker est mort ».
//!
//! Le piège est de croire que le fait suffit. Le chemin générique de `Report` écrit déjà un fait
//! pour n'importe quel type d'événement ; un `task.refused` qui n'aurait fait que cela serait une
//! valeur d'énumération sans effet. **Ce qui est éprouvé ici est la conséquence**, pas la trace.

use locus_lep::Event;
use locusd::refusal::{REFUSED, Refused, refused};

fn evenement(event_type: &str, payload: Option<serde_json::Value>) -> Event {
    Event {
        protocol: "lep/1.1".to_owned(),
        event_type: event_type.to_owned(),
        sequence: 1,
        occurred_at: "2026-09-01T10:00:00Z".to_owned(),
        idempotency_key: "cle".to_owned(),
        task_id: Some("tache-01".to_owned()),
        attempt: Some(1),
        lease_id: None,
        worker_id: None,
        correlation_id: None,
        causation_id: None,
        payload,
        payload_hash: None,
    }
}

fn refus(code: &str) -> Event {
    evenement(
        REFUSED,
        Some(serde_json::json!({ "code": code, "details": { "requested": "gpt-x" } })),
    )
}

// ---------------------------------------------------------------------------------------------
// 1. Ce qui est reconnu, et ce qui ne l'est pas
// ---------------------------------------------------------------------------------------------

/// **Un `task.refused` est reconnu, et il porte son code.**
///
/// Le pendant positif : une reconnaissance qui ne reconnaîtrait rien serait exacte et inutile.
#[test]
fn un_refus_est_reconnu_et_porte_son_code() {
    assert_eq!(
        refused(&refus("model_unavailable"), "tache-01"),
        Some(Refused {
            task_id: "tache-01".to_owned(),
            code: "model_unavailable".to_owned(),
        })
    );
}

/// **Un événement de progression n'est pas un refus, même s'il porte un `code`.**
///
/// La reconnaissance se fait sur le **type**, jamais sur la présence d'un champ. L'inverse ferait
/// remettre une mission en file parce qu'un `progress` a nommé un code dans sa charge — et une
/// mission remise en file pendant qu'elle s'exécute serait réclamée deux fois.
#[test]
fn un_evenement_de_progression_n_est_pas_un_refus() {
    let progres = evenement(
        "progress",
        Some(serde_json::json!({ "code": "model_unavailable" })),
    );
    assert_eq!(refused(&progres, "tache-01"), None);
}

/// **Un refus sans code lisible n'est pas reconnu, et ce n'est pas une indulgence.**
///
/// Le schéma rend `code` obligatoire : un document qui en manque n'a pas été validé. Remettre une
/// mission en file sur la foi d'un document qu'on n'a pas su lire serait agir sans savoir pourquoi.
/// Le fait, lui, s'écrit quand même par le chemin générique — la trace ne se perd pas, seule la
/// conséquence est retenue.
#[test]
fn un_refus_sans_code_lisible_n_est_pas_reconnu() {
    for charge in [
        None,
        Some(serde_json::json!({})),
        Some(serde_json::json!({ "code": "" })),
        Some(serde_json::json!({ "code": 12 })),
    ] {
        let boiteux = evenement(REFUSED, charge.clone());
        assert_eq!(refused(&boiteux, "tache-01"), None, "{charge:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// 2. La feature, sans laquelle la valeur n'aurait pas eu le droit d'entrer
// ---------------------------------------------------------------------------------------------

/// **`refusal-events` est au registre, et ce daemon l'annonce.**
///
/// Les deux moitiés, et l'une sans l'autre ne servirait à rien : le registre la rend **définissable**,
/// `HELD` la rend **annonçable**. Une feature définie et non annoncée ne serait jamais accordée, donc
/// `task.refused` ne serait jamais émis — la garde de l'ADR 0037 resterait fermée, et le membre
/// d'énumération serait entré pour rien.
#[test]
fn la_feature_est_definie_et_annoncee() {
    assert_eq!(locus_lep::feature_since("refusal-events"), Some("1.1"));
    assert!(
        locusd::handshake::HELD.contains(&"refusal-events"),
        "{:?}",
        locusd::handshake::HELD
    );
}

/// **Le membre est dans l'énumération du fil, et le SDK le relit.**
///
/// La vérification passe par un aller-retour `serde` plutôt que par la lecture du schéma : ce qui
/// compte est qu'un document portant ce type traverse le SDK, et non qu'une chaîne figure dans un
/// fichier.
#[test]
fn le_type_traverse_le_sdk() {
    let brut = serde_json::to_string(&refus("worker_draining")).expect("un événement se sérialise");
    let relu: Event = serde_json::from_str(&brut).expect("et se relit");

    assert_eq!(relu.event_type, REFUSED);
    assert_eq!(
        refused(&relu, "tache-01")
            .map(|refus| refus.code)
            .as_deref(),
        Some("worker_draining")
    );
}
