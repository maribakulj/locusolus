//! L'enveloppe commune d'un objet épistémique — `docs/SPEC_V1.md` §7.4.

use serde::{Deserialize, Serialize};

use locus_protocol::Timestamp;

use crate::hash::ContentHash;
use crate::ids::{RevisionId, StableId};
use crate::lineage::Lineage;
use crate::status::Status;
use crate::validation::ValidationLevel;

/// La classification d'un contenu — l'ordre est croissant en sensibilité.
///
/// Repris du vocabulaire LEP plutôt que réinventé : le même mot doit désigner la même chose des
/// deux côtés du fil, sans quoi une mission qui refuse `restricted` et un objet qui se dit
/// `restricted` ne parleraient pas de la même restriction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidentiality {
    /// Diffusable.
    Public,
    /// Interne à l'institution.
    Internal,
    /// Confidentiel.
    Confidential,
    /// Restreint : accès nominatif.
    Restricted,
}

/// Une référence à un autre objet, par révision.
///
/// Par **révision** et non par concept : une provenance qui pointerait un `stable_id` désignerait
/// « la dernière version, quelle qu'elle soit », donc une provenance qui change après coup. §7.7
/// fait de `revision_id` l'identité d'une version immuable, et c'est celle-là qu'une preuve cite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ref {
    /// La révision citée.
    pub revision_id: RevisionId,
    /// Une note libre, facultative. Donnée, jamais instruction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// L'enveloppe commune de §7.4, champ pour champ.
///
/// # Ce que ce type garantit
///
/// 1. **`stable_id` et `revision_id` sont deux types différents.** Les confondre est l'erreur
///    qu'on ne remarque qu'en lisant un historique devenu faux.
/// 2. **Au plus un prédécesseur de lignée**, par construction — voir [`Lineage`].
/// 3. **`validation_level` n'est pas dérivable de `status`.** Il n'existe aucune conversion entre
///    les deux dans ce crate, et un test vérifie que toutes les combinaisons sont représentables.
/// 4. **Le `content_hash` est vérifié dans sa forme**, jamais calculé ici : choisir une
///    implémentation de hash est une décision d'infrastructure, et l'invariant 1 l'exclut du
///    domaine.
///
/// # Ce que ce type ne fait pas
///
/// Il ne lit pas l'heure, ne tire pas au sort, n'ouvre aucun fichier. Une révision se fabrique en
/// **fournissant** l'instant et le nouvel identifiant : le domaine reste pur, donc déterministe en
/// test, et l'invariant 1 tient jusque dans les fondations — la même règle que `locus-protocol`
/// s'applique déjà à lui-même.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Envelope {
    /// Identifie le concept à travers ses versions.
    pub stable_id: StableId,
    /// Identifie cette version immuable.
    pub revision_id: RevisionId,
    /// Le type d'objet épistémique — `claim`, `inference`, `source`… Ouvert exprès : §7.3 et les
    /// packs disciplinaires en ajoutent, et une énumération fermée ici bloquerait le pack.
    pub object_type: String,
    /// La version du schéma qui décrit `content`.
    pub schema_version: String,
    /// Le rang de la révision dans la lignée, à partir de 1.
    pub version: u32,
    /// La branche de travail.
    pub branch_id: String,
    /// Le contenu, opaque pour l'enveloppe. C'est `object_type` et `schema_version` qui le
    /// décrivent, et les valider est le travail des packs disciplinaires (§8.2).
    pub content: serde_json::Value,
    /// Le hash du contenu canonicalisé.
    pub content_hash: ContentHash,
    /// Le cycle de vie.
    pub status: Status,
    /// La force épistémique. **Indépendante du statut** — §7.4.
    pub validation_level: ValidationLevel,
    /// Qui a créé cette révision.
    pub created_by: String,
    /// Quand. UTC, à la milliseconde — la présentation locale n'affecte ni signature ni hash.
    pub created_at: Timestamp,
    /// D'où vient cette révision.
    pub lineage: Lineage,
    /// Provenance : ce dont l'objet est issu.
    #[serde(default)]
    pub provenance_refs: Vec<Ref>,
    /// Preuves : ce sur quoi il s'appuie.
    #[serde(default)]
    pub evidence_refs: Vec<Ref>,
    /// La classification.
    pub confidentiality: Confidentiality,
    /// Étiquettes de politique.
    #[serde(default)]
    pub policy_tags: Vec<String>,
}

impl Envelope {
    /// Le `supersedes_revision_id` de §7.4, lu depuis la lignée.
    ///
    /// Exposé en lecture seule : la lignée est ce qui garantit l'unicité du prédécesseur, et un
    /// champ nu se laisserait écrire deux fois.
    #[must_use]
    pub const fn supersedes(&self) -> Option<&RevisionId> {
        self.lineage.supersedes()
    }

    /// Produire la révision suivante.
    ///
    /// §7.7 : « une modification crée une nouvelle révision ». Cette fonction est le seul chemin,
    /// et elle ne modifie rien en place — l'enveloppe reçue est consommée par référence et rendue
    /// intacte, la nouvelle est un autre objet.
    ///
    /// Le nouvel identifiant et l'instant sont **fournis** : ce crate ne lit pas l'heure et ne
    /// tire pas au sort.
    ///
    /// Ce qui change : `revision_id`, `version`, `created_at`, `created_by`, `content`,
    /// `content_hash`, et la lignée qui pointe vers la révision remplacée. Ce qui ne change
    /// **jamais** : `stable_id` — c'est la définition du mot.
    ///
    /// Le statut et le niveau de validation ne sont **pas** reportés : une nouvelle révision
    /// repart en `draft` avec `L0`. Hériter du niveau ferait franchir à un contenu modifié une
    /// validation qui portait sur un autre contenu, ce qui est exactement la manière dont une
    /// preuve se perd sans que personne ne s'en aperçoive.
    #[must_use]
    pub fn revise(&self, next: Revision) -> Self {
        Self {
            stable_id: self.stable_id,
            revision_id: next.revision_id,
            object_type: self.object_type.clone(),
            schema_version: next
                .schema_version
                .unwrap_or_else(|| self.schema_version.clone()),
            version: self.version.saturating_add(1),
            branch_id: next.branch_id.unwrap_or_else(|| self.branch_id.clone()),
            content: next.content,
            content_hash: next.content_hash,
            status: Status::Draft,
            validation_level: ValidationLevel::Unassessed,
            created_by: next.created_by,
            created_at: next.created_at,
            lineage: match next.incorporates {
                incorporates if incorporates.is_empty() => Lineage::Successor {
                    supersedes: self.revision_id,
                },
                incorporates => Lineage::Merge {
                    supersedes: self.revision_id,
                    incorporates,
                },
            },
            provenance_refs: self.provenance_refs.clone(),
            evidence_refs: Vec::new(),
            confidentiality: self.confidentiality,
            policy_tags: self.policy_tags.clone(),
        }
    }
}

/// Ce qu'il faut fournir pour produire la révision suivante.
///
/// Un type nommé plutôt que sept paramètres : l'ordre de sept arguments dont trois sont des
/// chaînes est une invitation à les intervertir, et le compilateur n'y verrait rien.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revision {
    /// L'identifiant de la nouvelle révision. Fourni : ce crate ne tire pas au sort.
    pub revision_id: RevisionId,
    /// Le nouveau contenu.
    pub content: serde_json::Value,
    /// Le hash du nouveau contenu canonicalisé.
    pub content_hash: ContentHash,
    /// Qui produit cette révision.
    pub created_by: String,
    /// Quand. Fourni : ce crate ne lit pas l'heure.
    pub created_at: Timestamp,
    /// Une nouvelle version de schéma, quand la révision en change.
    pub schema_version: Option<String>,
    /// Une autre branche, quand la révision change de branche.
    pub branch_id: Option<String>,
    /// Les parents incorporés, pour une fusion. Vide pour une révision ordinaire.
    pub incorporates: Vec<RevisionId>,
}
