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

// ---------------------------------------------------------------------------------------------
// W15.f — les deux tests qui **définissent** ce que « mineur » veut dire ici
// ---------------------------------------------------------------------------------------------
//
// ADR 0017 décide `lep/1.1` une fois pour quatre ajouts, et `role` est la tranche 1. Les deux
// tests ci-dessous ne portent pas sur `role` : ils portent sur ce qu'un **mineur** promet, et
// `role` est le premier champ qui permet de les écrire. Ils sont donc écrits sur le champ réel
// plutôt que sur un champ fictif — un `some_field_added_in_1_1` prouve que le SDK tolère
// l'inconnu, pas qu'un ajout réel se comporte comme un mineur.

/// Un document `1.1` reste lisible par un consommateur `1.0`.
///
/// Le sens du mineur, et la moitié la plus visible : un émetteur qui a mis à jour ne casse pas un
/// lecteur qui ne l'a pas fait. Ici le lecteur est le SDK d'avant `role` — approché par ce qu'il
/// **ferait** : lire le document, en tirer les champs qu'il connaît, et ne trébucher sur aucun
/// autre.
#[test]
fn un_document_1_1_se_lit_chez_un_consommateur_1_0() {
    let mut document = body("mission-envelope-nominal.json");
    let objet = document.as_object_mut().expect("un objet");
    objet.insert("protocol".to_owned(), Value::String("lep/1.1".to_owned()));
    objet.insert(
        "role".to_owned(),
        Value::String("logical-reviewer".to_owned()),
    );

    let mission: MissionEnvelope =
        serde_json::from_value(document).expect("un consommateur ne rejette pas un mineur");
    assert_eq!(mission.role.as_deref(), Some("logical-reviewer"));
    assert_eq!(mission.protocol, "lep/1.1");
}

/// Un document `1.0` laisse le champ **absent**, jamais rempli par un défaut.
///
/// L'autre moitié, et la plus facile à rater : un consommateur `1.1` qui remplirait `role` d'un
/// défaut ferait croire qu'un émetteur ancien a demandé quelque chose. « Absent » et « demandé
/// explicitement » sont deux faits différents, et c'est `Option` qui les sépare — un `String`
/// vide, ou un rôle par défaut, les confondrait.
///
/// Le ré-encodage est vérifié aussi : `skip_serializing_if` fait qu'un `role` absent ne réapparaît
/// pas en `null`. Un lecteur `1.0` qui recevrait `"role": null` verrait un champ qu'il ne connaît
/// pas, là où le document d'origine n'en avait aucun.
#[test]
fn un_document_1_0_laisse_le_champ_absent() {
    let document = body("mission-envelope-nominal.json");
    assert!(
        document.get("role").is_none(),
        "la fixture est bien un document d'avant le mineur"
    );

    let mission: MissionEnvelope = serde_json::from_value(document.clone()).expect("document 1.0");
    assert_eq!(mission.role, None, "absent ne se remplit pas d'un défaut");

    let reencode = serde_json::to_value(&mission).expect("ré-encodage");
    assert!(
        reencode.get("role").is_none(),
        "un champ absent ne revient pas en null"
    );
    assert!(
        equivalent(&reencode, &document),
        "et le reste est identique — par `equivalent`, parce que JSON ne distingue pas `4` de `4.0`"
    );
}

/// **Une dispense absente ne s'accorde pas** — W19.b, tranche 3 du mineur.
///
/// C'est la moitié de `un_document_1_0_laisse_le_champ_absent` appliquée à un champ dont l'enjeu
/// est différent : un rôle absent fait choisir un agent par capacité, une **permission** absente
/// qu'on remplirait d'un défaut ferait travailler hors ligne sur la mission la plus sensible du
/// lot, celle dont l'auteur n'a jamais imaginé qu'on le lui demanderait. `Option<bool>` distingue
/// les trois états qu'un `bool` confondrait : absente, refusée, accordée.
#[test]
fn une_permission_hors_ligne_absente_ne_s_accorde_pas() {
    let document = body("mission-envelope-nominal.json");
    assert!(document.get("offline_allowed").is_none(), "un document 1.0");

    let mission: MissionEnvelope = serde_json::from_value(document).expect("document 1.0");
    assert_eq!(mission.offline_allowed, None, "absente, pas `false`");
    assert_eq!(mission.offline_budget_ms, None);

    let reencode = serde_json::to_value(&mission).expect("ré-encodage");
    assert!(
        reencode.get("offline_allowed").is_none(),
        "et elle ne revient pas en null, ce qu'un lecteur 1.0 verrait comme un champ inconnu"
    );
}

/// Refusée explicitement n'est pas absente, et le fil garde les deux.
#[test]
fn une_permission_refusee_voyage_comme_telle() {
    let mut document = body("mission-envelope-nominal.json");
    document
        .as_object_mut()
        .expect("un objet")
        .insert("offline_allowed".to_owned(), Value::Bool(false));

    let mission: MissionEnvelope = serde_json::from_value(document).expect("document 1.1");
    assert_eq!(mission.offline_allowed, Some(false));

    let reencode = serde_json::to_value(&mission).expect("ré-encodage");
    assert_eq!(reencode["offline_allowed"], Value::Bool(false));
}
