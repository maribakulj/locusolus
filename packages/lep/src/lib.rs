//! SDK LEP : les types générés depuis `schemas/`, et la négociation de features du handshake.
//!
//! Le module `generated` est produit par `tooling/sdk/generate.ts` et n'est jamais édité à la
//! main ; `npm run check:generated` le vérifie. Ce qui suit — la négociation — est écrit à la
//! main, parce que c'est de la logique et non une lecture des schémas.

mod generated;

pub use generated::*;

/// Ce sur quoi deux pairs se sont mis d'accord au handshake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Negotiated {
    /// Les features que les deux pairs annoncent, triées, sans doublon.
    pub features: Vec<String>,
    /// Ce qu'un pair a demandé et que l'autre ne tient pas.
    pub declined: Vec<String>,
    /// Ce qu'un pair a demandé et que ce protocole ne connaît pas du tout.
    pub unknown: Vec<String>,
}

/// Négocier les features à partir de ce que chaque pair annonce.
///
/// Trois issues, et les distinguer est tout l'intérêt : une feature que les deux tiennent est
/// **accordée** ; une que ce protocole connaît mais que l'autre ne tient pas est **refusée**, ce
/// qui est une information exploitable — le demandeur sait qu'il doit se replier ; une que le
/// protocole ne connaît pas est **inconnue**, et c'est un signal différent, celui d'un pair plus
/// récent ou mal configuré.
///
/// Les fondre en un seul « non » ferait qu'un client venu d'un mineur ultérieur serait
/// indiscernable d'un client qui a mal orthographié son besoin.
#[must_use]
pub fn negotiate(local: &[&str], remote: &[&str]) -> Negotiated {
    let known: Vec<&str> = LEP_FEATURES.iter().map(|(name, _)| *name).collect();
    let mut features = Vec::new();
    let mut declined = Vec::new();
    let mut unknown = Vec::new();
    for name in local {
        if !known.contains(name) {
            unknown.push((*name).to_owned());
        } else if remote.contains(name) {
            features.push((*name).to_owned());
        } else {
            declined.push((*name).to_owned());
        }
    }
    features.sort_unstable();
    features.dedup();
    declined.sort_unstable();
    declined.dedup();
    unknown.sort_unstable();
    unknown.dedup();
    Negotiated {
        features,
        declined,
        unknown,
    }
}

/// Le mineur qui introduit `feature`, ou `None` si ce protocole ne la connaît pas.
#[must_use]
pub fn feature_since(feature: &str) -> Option<&'static str> {
    LEP_FEATURES
        .iter()
        .find(|(name, _)| *name == feature)
        .map(|(_, since)| *since)
}
