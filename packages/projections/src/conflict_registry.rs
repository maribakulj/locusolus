//! La projection « registre des conflits » — `docs/SPEC_V1.md` §9.3, sous l'invariant 12.

use std::collections::BTreeMap;

use locus_event_store::Envelope;

use crate::projection::{Projection, ProjectionError, Watermark};

/// Un conflit enregistré.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictEntry {
    /// Le stream où le conflit a été déclaré.
    pub stream_id: String,
    /// La position à laquelle il a été déclaré.
    pub declared_at: u64,
    /// La position à laquelle il a été résolu, s'il l'a été.
    ///
    /// **Résolu, pas supprimé.** L'entrée reste ; c'est son état qui change.
    pub resolved_at: Option<u64>,
    /// Ce qui a été dit du conflit.
    pub statement: String,
}

impl ConflictEntry {
    /// Vrai tant que le conflit n'a pas été résolu.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.resolved_at.is_none()
    }
}

/// Le registre des conflits — §9.3, et l'invariant 12 par-dessus.
///
/// # Ce que l'invariant 12 impose à une projection
///
/// « Les résultats négatifs et conflits ne sont jamais supprimés pour rendre le graphe *propre*. »
///
/// Le mot « propre » vise exactement ce que ferait une projection ordinaire : ne garder que les
/// conflits ouverts, parce que ce sont les seuls qu'on interroge. Le registre garde donc **tout**,
/// et un conflit résolu porte la position de sa résolution au lieu de disparaître. Il n'existe ici
/// aucune méthode qui retire une entrée, et un test le vérifie par l'absence.
///
/// `reset` en retire toutes, et ce n'est pas une exception : reconstruire n'est pas supprimer. Le
/// registre repart du journal, qui les contient toutes, et l'égalité entre reconstruction et état
/// courant est précisément ce que le test de sortie de W1.d vérifie.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ConflictRegistry {
    entries: BTreeMap<String, ConflictEntry>,
    watermark: Watermark,
}

impl ConflictRegistry {
    /// Un registre vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Tous les conflits, résolus compris.
    #[must_use]
    pub fn all(&self) -> Vec<&ConflictEntry> {
        self.entries.values().collect()
    }

    /// Les conflits non résolus — la requête de §9.4.
    #[must_use]
    pub fn open(&self) -> Vec<&ConflictEntry> {
        self.entries
            .values()
            .filter(|entry| entry.is_open())
            .collect()
    }

    /// Le nombre total de conflits connus, résolus compris.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Vrai quand aucun conflit n'est connu.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Projection for ConflictRegistry {
    fn name(&self) -> &'static str {
        "conflict_registry"
    }

    fn apply(&mut self, position: u64, event: &Envelope) -> Result<(), ProjectionError> {
        self.watermark = position;
        if event.event_type.namespace() != "conflict" {
            return Ok(());
        }
        let payload = event.payload.as_object().ok_or_else(|| ProjectionError {
            position,
            reason: "charge de conflit non objet".to_owned(),
        })?;
        let conflict_id = payload
            .get("conflict_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| ProjectionError {
                position,
                reason: "`conflict_id` absent : un conflit sans identité n'est pas suivable"
                    .to_owned(),
            })?;

        if event.event_type.verb() == "resolved" {
            // Résolu, pas supprimé. Un conflit résolu dont on ne trouve pas la déclaration est un
            // défaut de journal, pas une occasion d'inventer une entrée.
            let entry = self
                .entries
                .get_mut(conflict_id)
                .ok_or_else(|| ProjectionError {
                    position,
                    reason: format!("conflit `{conflict_id}` résolu sans avoir été déclaré"),
                })?;
            entry.resolved_at = Some(position);
            return Ok(());
        }

        let statement = payload
            .get("statement")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_owned();
        self.entries
            .entry(conflict_id.to_owned())
            .or_insert(ConflictEntry {
                stream_id: event.stream_id.clone(),
                declared_at: position,
                resolved_at: None,
                statement,
            });
        Ok(())
    }

    fn watermark(&self) -> Watermark {
        self.watermark
    }

    fn reset(&mut self) {
        self.entries.clear();
        self.watermark = 0;
    }

    fn checksum(&self) -> String {
        self.entries
            .iter()
            .map(|(id, entry)| {
                let state = entry
                    .resolved_at
                    .map_or_else(|| "ouvert".to_owned(), |at| format!("résolu@{at}"));
                format!("{id}={state}")
            })
            .collect::<Vec<_>>()
            .join(";")
    }
}
