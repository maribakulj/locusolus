//! Round-trip serde sur toutes les fixtures — le test de sortie de W0.8.
//!
//! Une fixture décodée puis ré-encodée doit rendre le MÊME JSON. C'est ce qui rend le SDK
//! utilisable comme lecture du protocole plutôt que comme approximation : un champ que les types
//! ne modélisent pas disparaîtrait au ré-encodage, et rien d'autre ne le signalerait.
//!
//! Les fixtures `invalid` sont exclues : elles sont mal formées par construction, et exiger
//! qu'elles se décodent reviendrait à demander au SDK d'accepter ce que les schémas refusent.

use locus_lep::{Attempt, CapabilityManifest, Event, Lease, MissionEnvelope, SandboxAttestation};
use serde_json::Value;
use std::{fs, path::PathBuf};

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/examples")
        .canonicalize()
        .expect("le répertoire des fixtures existe")
}

/// Retire le bloc `_fixture` : métadonnée de test, jamais un champ LEP.
fn body(name: &str) -> Value {
    let raw = fs::read_to_string(examples_dir().join(name)).expect("fixture lisible");
    let mut value: Value = serde_json::from_str(&raw).expect("fixture en JSON valide");
    value
        .as_object_mut()
        .expect("une fixture est un objet")
        .remove("_fixture");
    value
}

/// Compare deux JSON par leur SENS, pas par leur écriture.
///
/// JSON ne distingue pas `4` de `4.0` : les deux dénotent le même nombre, et aucun lecteur
/// conforme ne rapporte lequel a été écrit. Le SDK Rust type `cpu` en `f64` parce que le schéma
/// dit `number` — pour les cœurs fractionnaires — donc un `4` reçu ressort en `4.0`.
///
/// Ce test l'accepte, et cette découverte a une conséquence qui dépasse le test : **le
/// `payload_hash` d'un événement ne peut pas être calculé sur la sortie d'un sérialiseur.** Deux
/// pairs conformes émettraient des octets différents pour la même donnée, et leurs hashes
/// divergeraient sur rien. §7.7 exige « une canonicalisation stable » : c'est elle qui doit
/// produire les octets à hasher, pas `serde_json::to_string` ni `JSON.stringify`. Le
/// canonicaliseur appartient à W0.9, et cette note existe pour qu'il ne soit pas oublié.
fn equivalent(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => match (a.as_f64(), b.as_f64()) {
            (Some(a), Some(b)) => (a - b).abs() < f64::EPSILON,
            _ => a == b,
        },
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| equivalent(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(key, a)| b.get(key).is_some_and(|b| equivalent(a, b)))
        }
        _ => left == right,
    }
}

fn round_trip<T>(name: &str)
where
    T: serde::de::DeserializeOwned + serde::Serialize,
{
    let original = body(name);
    let decoded: T = serde_json::from_value(original.clone())
        .unwrap_or_else(|error| panic!("{name} ne se décode pas : {error}"));
    let re_encoded = serde_json::to_value(&decoded).expect("ré-encodage");
    assert!(
        equivalent(&re_encoded, &original),
        "{name} ne fait pas un aller-retour exact\n  reçu   : {original}\n  rendu  : {re_encoded}"
    );
}

#[test]
fn capability_manifests_round_trip() {
    round_trip::<CapabilityManifest>("capability-manifest.json");
    round_trip::<CapabilityManifest>("capability-manifest-vm-linux.json");
}

#[test]
fn mission_envelopes_round_trip() {
    round_trip::<MissionEnvelope>("mission-envelope.json");
    round_trip::<MissionEnvelope>("mission-envelope-nominal.json");
}

#[test]
fn attestations_round_trip() {
    round_trip::<SandboxAttestation>("sandbox-attestation.json");
}

#[test]
fn events_round_trip() {
    for name in [
        "event-reconnection-1-started.json",
        "event-reconnection-2-progress.json",
        "event-reconnection-3-tool-completed.json",
        "event-reconnection-4-replay.json",
    ] {
        round_trip::<Event>(name);
    }
}

#[test]
fn attempts_and_leases_round_trip() {
    round_trip::<Attempt>("attempt-late-result.json");
    round_trip::<Attempt>("attempt-budget-exceeded.json");
    round_trip::<Lease>("lease-expired.json");
}

#[test]
fn an_absent_field_stays_absent() {
    // Le piège que `skip_serializing_if` évite : sans lui, un champ optionnel absent revient
    // en `null` au ré-encodage, et deux pairs qui comparent des hashes de documents divergent
    // sur une donnée que ni l'un ni l'autre n'a écrite.
    let lease = body("lease-expired.json");
    assert!(lease.get("agent_id").is_none());
    let decoded: Lease = serde_json::from_value(lease).expect("décodable");
    let re_encoded = serde_json::to_value(&decoded).expect("ré-encodable");
    assert!(
        re_encoded.get("agent_id").is_none(),
        "un champ absent ne doit pas réapparaître en null"
    );
}
