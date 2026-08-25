//! La trace de raisonnement comme **artefact** — `W26.a`, ADR 0027.
//!
//! # L'invariant 11 borne des lecteurs, il n'ordonne pas de détruire
//!
//! Le dépôt savait déjà **détecter la fuite** — `Contamination::GeneratorReasoningLeaked`, la
//! première des cinq de §16.6 —, il avait le **rayonnage privé** — [`Level::AgentPrivate`], le plus
//! étroit des sept de §16.1 — et le **genre qui empêche la contamination épistémique** —
//! [`Genre::MetaMemory`]. Et rien n'écrivait le raisonnement nulle part.
//!
//! Retenir et diffuser sont deux actes. L'invariant 11 interdit le second vers un reviewer
//! indépendant ; il ne dit rien du premier. Lu comme un ordre de destruction, il fait disparaître la
//! seule chose qu'aucun audit ne rattrape — et l'invariant 12 interdit déjà cette disparition pour
//! les résultats négatifs, qui sont exactement le même genre de matière gênante.
//!
//! # Le chemin est celui de §9.1, et il n'y en a pas d'autre
//!
//! Une trace entre par [`crate::reasoning::Trace::declaring`], qui produit un
//! [`locus_artifacts::ArtifactManifest`] — donc **déclarée avant dépôt**, hashée, référencée par son
//! condensat. C'est le chemin d'ADR 0005 : « hash déclaré **avant** upload », ce qui fait du
//! condensat une promesse que l'arrivée confronte, au lieu d'un constat que l'arrivée fabrique.
//!
//! **Aucun second stockage n'apparaît.** Ce module ne porte ni carte, ni registre, ni tampon : il
//! rend un manifeste, et le contenu suit le chemin des artefacts. C'est ce que `W16.e` tient pour les
//! messages — « la messagerie demeure un **usage du journal**, aucun second stockage durable » — et
//! un test le vérifie sur la source, parce qu'un stockage de traces serait un endroit de plus où
//! chercher, et un endroit de plus à oublier de purger.
//!
//! # Le couple est fixé, et le type le refuse autrement
//!
//! [`Level::AgentPrivate`] et [`Genre::MetaMemory`], jamais autre chose.
//!
//! - Le **niveau** : `AgentPrivate` est le plus étroit de §16.1. Une trace rangée plus large serait
//!   lisible par ceux que l'invariant 11 exclut, et la fuite ne serait plus une anomalie mais le
//!   fonctionnement.
//! - Le **genre** : `MetaMemory` « influence le rang, jamais la validité » (ADR 0022 décision 1).
//!   Ranger une trace en `Episodic` ou en `Semantic` la ferait entrer dans ce qui fonde des claims,
//!   et le raisonnement d'un générateur deviendrait une source — c'est la contamination épistémique
//!   que le genre existe pour empêcher.
//!
//! Les deux sont **posés par le constructeur**, pas demandés à l'appelant : un paramètre serait un
//! endroit où se tromper, et le refuser ensuite serait vérifier ce qu'on aurait pu rendre
//! inexprimable.
//!
//! # Aucun résumé n'est stocké à la place
//!
//! Un résumé est une **lecture**, et une lecture se refait. Condenser avant écriture décide une fois
//! pour toutes de ce qui méritait d'être gardé, au moment précis où personne ne sait encore quelle
//! question sera posée — et ce qui a été jeté ne se retrouve pas.
//!
//! Ce module n'expose donc aucune signature qui prenne une trace et rende une trace plus courte. Un
//! test le tient par l'absence, sur la source : c'est la seule façon de garantir que personne ne
//! **peut** condenser, plutôt que de constater que personne ne l'a fait.

use locus_artifacts::{ArtifactManifest, ManifestError, ProducedBy};
use locus_domain::{Confidentiality, ContentHash};

use crate::genre::Genre;
use crate::level::Level;

/// Le type MIME d'une trace de raisonnement.
///
/// Du texte brut, et le dire ici plutôt que de le demander à l'appelant : deux traces de types
/// différents seraient deux choses, et le lecteur institutionnel de `W26.b` devrait alors savoir
/// laquelle il lit.
const MEDIA_TYPE: &str = "text/plain";

/// Une trace de raisonnement, sur le point d'être écrite.
///
/// # Ce que le type garantit, et qui n'est donc pas à vérifier
///
/// Le niveau et le genre ne sont pas des champs qu'on renseigne : ils sont **posés**, et
/// [`Trace::level`] comme [`Trace::genre`] les rendent sans qu'aucun chemin ne permette de les
/// choisir. Une trace mal rangée n'est pas refusée — elle est inconstructible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trace {
    manifest: ArtifactManifest,
}

impl Trace {
    /// Déclarer une trace **avant** que son contenu arrive.
    ///
    /// # L'ordre est la garantie
    ///
    /// Comme pour tout artefact de §9.1 : le condensat est déclaré d'abord, donc c'est une promesse
    /// que l'arrivée confronte. Un manifeste bâti après coup sur le contenu reçu ne dirait que « ce
    /// qui est arrivé est ce qui est arrivé ».
    ///
    /// # La classification est `Restricted`, et ce n'est pas le niveau
    ///
    /// [`Confidentiality`] borne **qui peut recevoir l'octet** — c'est le plafond que
    /// `ContextView` compare à l'habilitation d'un worker. [`Level`] borne **quel rayonnage de
    /// mémoire le porte**. Les deux disent des choses différentes, et une trace a besoin des deux au
    /// plus étroit : sans la classification, un worker peu habilité recevrait le contenu ; sans le
    /// niveau, un rayonnage plus large l'indexerait.
    ///
    /// # Errors
    ///
    /// Ce que [`ArtifactManifest::declare`] refuse — identifiant ou tâche vides, taille nulle,
    /// attempt zéro. Les motifs sont les siens et ne sont pas réécrits ici : les redire produirait
    /// deux vocabulaires pour un refus.
    pub fn declaring(
        artifact_id: &str,
        declared_hash: ContentHash,
        size_bytes: u64,
        produced_by: ProducedBy,
    ) -> Result<Self, ManifestError> {
        Ok(Self {
            manifest: ArtifactManifest::declare(
                artifact_id,
                declared_hash,
                MEDIA_TYPE,
                size_bytes,
                produced_by,
                Confidentiality::Restricted,
            )?,
        })
    }

    /// Le manifeste, tel que le chemin des artefacts l'attend.
    ///
    /// C'est la seule sortie de ce module : ce qui suit — le dépôt, la confrontation du condensat —
    /// est le chemin de §9.1, et le rejouer ici ferait le second stockage que l'item refuse.
    #[must_use]
    pub const fn manifest(&self) -> &ArtifactManifest {
        &self.manifest
    }

    /// Le rayonnage qui la porte — **toujours** le plus étroit de §16.1.
    #[must_use]
    pub const fn level(&self) -> Level {
        Level::AgentPrivate
    }

    /// Son genre — **toujours** celui qui influence le rang et jamais la validité.
    #[must_use]
    pub const fn genre(&self) -> Genre {
        Genre::MetaMemory
    }
}
