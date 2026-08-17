//! Les conflits — `docs/SPEC_V1.md` §18.4, sous l'invariant 12.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ids::RevisionId;

/// Ce qui a produit le conflit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictOrigin {
    /// Deux claims incompatibles, découverts à la fusion — §18.4, point 3.
    Merge,
    /// Une contradiction relevée dans une branche.
    Contradiction,
    /// Deux reproductions divergentes du même run.
    Reproduction,
    /// Une revue qui contredit une autre.
    Review,
}

/// Comment un conflit a été tranché.
///
/// # Ce que ce type ne contient pas
///
/// Aucune variante ne veut dire « le conflit n'a jamais existé ». Trancher un conflit, c'est
/// décider **lequel des deux camps l'emporte, et pourquoi** ; ce n'est pas effacer le camp perdant.
/// §18.4, point 3 : une fusion « conserve les claims incompatibles ».
///
/// La variante qui compte le plus est [`Verdict::Unresolved`] : un conflit sans verdict reste
/// ouvert indéfiniment, et c'est bien. Un graphe qui ne peut pas porter de désaccord durable est un
/// graphe qui force une réponse avant qu'elle existe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum Verdict {
    /// Un des deux côtés l'emporte — l'autre **reste** au graphe.
    Prevails {
        /// Le côté retenu.
        side: RevisionId,
        /// Pourquoi.
        rationale: String,
    },
    /// Les deux tiennent, dans des domaines différents.
    ScopeSplit {
        /// Le domaine où le premier vaut.
        first_scope: String,
        /// Celui où le second vaut.
        second_scope: String,
    },
    /// Les deux sont écartés par un troisième résultat.
    BothSuperseded {
        /// Ce qui remplace.
        by: RevisionId,
    },
    /// Non tranché. L'état par défaut, et un état légitime.
    Unresolved,
}

/// Un conflit explicite — §18.4, point 7.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conflict {
    /// L'identité du conflit.
    pub id: String,
    /// Le premier camp.
    pub first: RevisionId,
    /// Le second.
    pub second: RevisionId,
    /// Ce qui l'a produit.
    pub origin: ConflictOrigin,
    /// L'énoncé du désaccord.
    pub statement: String,
    /// Où il a été relevé.
    pub branch_id: String,
    /// Quand — horodatage fourni, ce crate ne lit pas l'heure.
    pub declared_at: String,
    /// Comment il a été tranché, s'il l'a été.
    pub verdict: Verdict,
}

impl Conflict {
    /// Vrai tant qu'aucun verdict n'a été rendu.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        matches!(self.verdict, Verdict::Unresolved)
    }

    /// Les deux camps, quel que soit le verdict.
    ///
    /// **Y compris le camp perdant.** C'est la fonction qui rend l'invariant 12 utilisable : après
    /// un verdict, on peut toujours demander qui avait dit quoi.
    #[must_use]
    pub const fn sides(&self) -> [RevisionId; 2] {
        [self.first, self.second]
    }

    /// Trancher.
    ///
    /// Rend un **nouveau** conflit portant le verdict ; l'original n'est pas modifié, et surtout
    /// pas retiré. Un verdict est un fait qui s'ajoute à l'histoire du désaccord, pas un effacement
    /// de ce désaccord.
    #[must_use]
    pub fn with_verdict(&self, verdict: Verdict) -> Self {
        Self {
            verdict,
            ..self.clone()
        }
    }
}

/// Le journal des conflits — additif seulement.
///
/// # L'invariant 12, rendu opposable
///
/// « Les résultats négatifs et conflits ne sont jamais supprimés pour rendre le graphe *propre*. »
///
/// Ce type n'offre **aucune** méthode de retrait : ni `remove`, ni `prune`, ni `clear`, ni
/// `retain`, ni `drain`. Trancher un conflit passe par [`ConflictLog::record_verdict`], qui
/// remplace l'entrée par une entrée **portant le verdict** — jamais par rien.
///
/// L'absence est vérifiée par le test de sortie de W1.g, et elle l'est **sur tout le workspace** :
/// une garantie qui ne tiendrait que dans le module qui la déclare ne serait pas une garantie.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictLog {
    entries: BTreeMap<String, Conflict>,
}

impl ConflictLog {
    /// Un journal vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Consigner un conflit.
    ///
    /// Un conflit déjà connu n'est pas réécrit : le premier enregistrement fait foi, et une seconde
    /// déclaration du même désaccord ne doit pas effacer le verdict qu'il porte peut-être déjà.
    pub fn declare(&mut self, conflict: Conflict) {
        self.entries.entry(conflict.id.clone()).or_insert(conflict);
    }

    /// Consigner un verdict.
    ///
    /// Rend `false` quand le conflit est inconnu — inventer une entrée pour porter un verdict
    /// ferait exister un désaccord que personne n'a déclaré.
    pub fn record_verdict(&mut self, id: &str, verdict: Verdict) -> bool {
        match self.entries.get(id) {
            None => false,
            Some(existing) => {
                let updated = existing.with_verdict(verdict);
                self.entries.insert(id.to_owned(), updated);
                true
            }
        }
    }

    /// Tous les conflits, tranchés compris.
    #[must_use]
    pub fn all(&self) -> Vec<&Conflict> {
        self.entries.values().collect()
    }

    /// Les conflits non tranchés — la requête de §9.4.
    #[must_use]
    pub fn open(&self) -> Vec<&Conflict> {
        self.entries
            .values()
            .filter(|conflict| conflict.is_open())
            .collect()
    }

    /// Un conflit par son identité.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Conflict> {
        self.entries.get(id)
    }

    /// Le nombre total, tranchés compris.
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

/// Ce qu'une fusion refuse de faire — §18.4, point 3.
///
/// « Conserve les claims incompatibles. » Une fusion qui trancherait d'elle-même en écartant l'un
/// des deux camps produirait un graphe propre et faux. La fonction rend donc les conflits à
/// **déclarer**, et jamais une liste d'objets à retirer.
#[must_use]
pub fn conflicts_from_merge(
    incompatible: &[(RevisionId, RevisionId)],
    branch_id: &str,
    declared_at: &str,
) -> Vec<Conflict> {
    incompatible
        .iter()
        .enumerate()
        .map(|(index, (first, second))| Conflict {
            id: format!("cfl_{branch_id}_{index}"),
            first: *first,
            second: *second,
            origin: ConflictOrigin::Merge,
            statement: "claims incompatibles rencontrés à la fusion (§18.4)".to_owned(),
            branch_id: branch_id.to_owned(),
            declared_at: declared_at.to_owned(),
            verdict: Verdict::Unresolved,
        })
        .collect()
}
