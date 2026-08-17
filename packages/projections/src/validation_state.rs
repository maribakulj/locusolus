//! La projection « état de validation » — `docs/SPEC_V1.md` §9.3.

use std::collections::BTreeMap;

use locus_event_store::Envelope;

use crate::projection::{Projection, ProjectionError, Watermark};

/// L'état de validation d'un objet, tel que la projection le voit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectState {
    /// Le statut courant.
    pub status: String,
    /// Le niveau de validation courant.
    pub validation_level: String,
    /// La position à laquelle ce statut a été observé.
    pub at_position: u64,
}

/// « État de validation » — la première des projections obligatoires de §9.3 que ce paquet livre.
///
/// # Les deux champs restent distincts jusqu'ici
///
/// §7.4 : « `validation_level` décrit la force épistémique et ne doit pas être déduit du seul
/// statut ». W1.a l'a rendu vrai dans le domaine ; la projection ne le défait pas. Elle lit les
/// deux champs de la charge et les range côte à côte, sans jamais en calculer un depuis l'autre —
/// une projection qui le ferait rendrait la garantie fausse à la lecture, là où tout le monde
/// regarde.
///
/// # Ce qu'elle refuse
///
/// Un événement `epistemic_object.*` dont la charge ne porte pas les deux champs. Ce n'est pas de
/// la sévérité : une projection qui compléterait les manquants par un défaut inventerait un état
/// de validation, et §8.4 dit qu'« une moyenne de confiance ne constitue jamais une procédure de
/// décision par défaut ». Un défaut inventé est pire qu'une moyenne : il n'a même pas de source.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ValidationState {
    /// L'état par stream. `BTreeMap` et non `HashMap` : le checksum doit être stable d'une
    /// exécution à l'autre, et l'ordre d'itération d'une table de hachage ne l'est pas.
    objects: BTreeMap<String, ObjectState>,
    watermark: Watermark,
}

impl ValidationState {
    /// Une projection vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// L'état d'un objet, s'il est connu.
    #[must_use]
    pub fn get(&self, stream_id: &str) -> Option<&ObjectState> {
        self.objects.get(stream_id)
    }

    /// Le nombre d'objets suivis.
    #[must_use]
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// Vrai quand aucun objet n'est suivi.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }
}

impl Projection for ValidationState {
    fn name(&self) -> &'static str {
        "validation_state"
    }

    fn apply(&mut self, position: u64, event: &Envelope) -> Result<(), ProjectionError> {
        // Le watermark avance même sur un événement qui ne concerne pas cette projection : il dit
        // où elle en est du **journal**, pas de ce qu'elle a retenu. Ne pas l'avancer ferait
        // relire à chaque passage tout ce qu'elle a déjà écarté.
        self.watermark = position;
        if event.event_type.namespace() != "epistemic_object" {
            return Ok(());
        }

        let payload = event.payload.as_object().ok_or_else(|| ProjectionError {
            position,
            reason: "charge d'événement épistémique non objet".to_owned(),
        })?;
        let status = payload
            .get("status")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| ProjectionError {
                position,
                reason: "`status` absent : une projection ne l'invente pas".to_owned(),
            })?;
        let level = payload
            .get("validation_level")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| ProjectionError {
                position,
                reason: "`validation_level` absent, et il ne se déduit pas du statut (§7.4)"
                    .to_owned(),
            })?;

        self.objects.insert(
            event.stream_id.clone(),
            ObjectState {
                status: status.to_owned(),
                validation_level: level.to_owned(),
                at_position: position,
            },
        );
        Ok(())
    }

    fn watermark(&self) -> Watermark {
        self.watermark
    }

    fn reset(&mut self) {
        self.objects.clear();
        self.watermark = 0;
    }

    fn checksum(&self) -> String {
        // Une somme lisible plutôt qu'un hash : le but de §9.5 est de **détecter** une divergence,
        // et une chaîne qu'on peut lire dit aussi où elle est. Un sha256 dirait seulement « ce
        // n'est pas pareil ».
        self.objects
            .iter()
            .map(|(stream, state)| format!("{stream}={}/{}", state.status, state.validation_level))
            .collect::<Vec<_>>()
            .join(";")
    }
}
