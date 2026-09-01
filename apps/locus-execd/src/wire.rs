//! Ce qu'un refus d'admission devient sur le fil — `SPEC_V1.md` §10.2, ADR 0017 §5.2.
//!
//! # Deux propriétés, et elles sont la raison d'être du document
//!
//! **Jamais un seul motif à la fois.** [`crate::admission::admit`] accumule les conditions
//! manquantes et rend `Refused { reasons }` au pluriel. Un fil qui n'en transmettrait que la
//! première ferait corriger une condition, relancer, découvrir la suivante, corriger, relancer —
//! autant d'allers-retours qu'il manque de conditions. La traduction ci-dessous conserve l'ordre
//! **et** le nombre, et un test le tient sur un refus qui en porte quatre.
//!
//! **Deux refus qui se ressemblent ne se fondent pas.** `LevelNotAttested` n'est pas
//! `LevelUnavailable` : « l'hôte ne sait pas faire » envoie chercher une autre machine, « l'hôte
//! l'annonce sans l'avoir prouvé » envoie faire tourner une campagne de self-tests. Les fondre
//! ferait acheter du matériel pour un problème d'attestation. Même règle pour
//! `AcceleratorOutsideSandbox` face à `AcceleratorUnavailable`. Un test tient les paires par
//! **égalité stricte** sur les codes de fil, pas par lecture de phrase.
//!
//! # Sept motifs, là où l'ADR en nommait six
//!
//! `DiskQuotaNotEnforceable` est né avec `W5.g` et `W5.j`, après l'écriture d'ADR 0017 §5.2. Le
//! laisser hors du fil aurait violé la première propriété au premier hôte sans quota de projet :
//! le refus serait parti amputé d'une de ses raisons, et l'ADR aurait été respecté à la lettre
//! contre son propre objet.
//!
//! # Une conversion exhaustive, sans branche fourre-tout
//!
//! Le `match` ci-dessous n'a pas de `_ =>`. Ajouter une variante à [`RefusalReason`] sans lui
//! donner sa forme de fil **ne compile pas** — c'est la même garantie structurelle que
//! `packages/environments` obtient de sa chaîne de types, appliquée ici à une traduction. Une
//! branche fourre-tout aurait laissé un motif nouveau voyager en silence sous le code d'un autre.

use locus_execution::SandboxLevel;
use locus_lep::{AdmissionRefusal, NetworkMode, Reason, SandboxLevel as WireLevel};

use crate::admission::RefusalReason;

/// La version de protocole que ce document porte : le refus est la tranche 2 du mineur.
const PROTOCOL: &str = "lep/1.1";

/// Traduire un niveau de confinement vers son écriture de fil.
///
/// Exhaustif lui aussi : un niveau nouveau dans `locus_execution` n'a pas de code de fil tant que
/// personne ne le lui donne, et le compilateur le dit.
#[must_use]
pub fn level(level: SandboxLevel) -> WireLevel {
    match level {
        SandboxLevel::S0 => WireLevel::S0,
        SandboxLevel::S1 => WireLevel::S1,
        SandboxLevel::S2 => WireLevel::S2,
        SandboxLevel::S3 => WireLevel::S3,
        SandboxLevel::S4 => WireLevel::S4,
        SandboxLevel::S5 => WireLevel::S5,
    }
}

/// Traduire un mode réseau vers son écriture de fil.
fn network(mode: &str) -> Option<NetworkMode> {
    match mode {
        "deny" => Some(NetworkMode::Deny),
        "connector-only" => Some(NetworkMode::ConnectorOnly),
        "allowlist" => Some(NetworkMode::Allowlist),
        "full" => Some(NetworkMode::Full),
        _ => None,
    }
}

/// Un motif, sur le fil.
///
/// # Panics
///
/// Jamais : `network` ne rend `None` que pour un mode que le vocabulaire ne connaît pas, et
/// [`RefusalReason::NetworkModeUnsupported`] ne porte que des modes lus de la `SandboxSpec`, donc
/// du vocabulaire. L'`expect` est là pour que l'hypothèse soit **écrite** plutôt que supposée : si
/// un mode hors vocabulaire arrivait, mieux vaut un arrêt net qu'un refus qui ment sur sa cause.
#[must_use]
pub fn reason(refusal: &RefusalReason) -> Reason {
    match refusal {
        RefusalReason::LevelUnavailable { required, best } => Reason::LevelUnavailable {
            required: level(*required),
            best: level(*best),
        },
        RefusalReason::CapacityExceeded => Reason::CapacityExceeded,
        RefusalReason::AcceleratorUnavailable { kind } => {
            Reason::AcceleratorUnavailable { kind: kind.clone() }
        }
        RefusalReason::DiskQuotaNotEnforceable { requested, why } => {
            Reason::DiskQuotaNotEnforceable {
                requested: i64::try_from(*requested).unwrap_or(i64::MAX),
                why: why.clone(),
            }
        }
        RefusalReason::NetworkModeUnsupported { mode } => Reason::NetworkModeUnsupported {
            mode: network(mode).expect("un mode réseau vient du vocabulaire"),
        },
        RefusalReason::LevelNotAttested { required, proven } => Reason::LevelNotAttested {
            required: level(*required),
            proven: proven.map(level),
        },
        RefusalReason::AcceleratorOutsideSandbox {
            kind,
            required,
            native_level,
        } => Reason::AcceleratorOutsideSandbox {
            kind: kind.clone(),
            required: level(*required),
            native_level: level(*native_level),
        },
        RefusalReason::MechanismNotEmployed {
            required,
            employs,
            attested,
        } => Reason::MechanismNotEmployed {
            required: level(*required),
            employs: employs.clone(),
            attested: attested.clone(),
        },
        RefusalReason::MechanismUnresolved {
            required,
            employs,
            unregistered,
        } => Reason::MechanismUnresolved {
            required: level(*required),
            employs: employs.clone(),
            unregistered: unregistered.clone(),
        },
    }
}

/// Le document entier — **toutes** les raisons, dans l'ordre où l'admission les a trouvées.
#[must_use]
pub fn refusal(task_id: &str, attempt_id: &str, reasons: &[RefusalReason]) -> AdmissionRefusal {
    AdmissionRefusal {
        protocol: PROTOCOL.to_owned(),
        task_id: task_id.to_owned(),
        attempt_id: attempt_id.to_owned(),
        reasons: reasons.iter().map(reason).collect(),
    }
}
