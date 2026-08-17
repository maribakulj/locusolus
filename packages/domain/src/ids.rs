//! Les deux identités d'un objet épistémique — `docs/SPEC_V1.md` §7.7.

use locus_protocol::{Id, IdKind};

/// La nature d'un `stable_id`.
///
/// § 7.7 : « `stable_id` identifie le **concept** à travers ses versions ». Le préfixe `obj` est
/// **provisoire** au même titre que ceux de `locus_protocol::id::provisional` : aucun document ne
/// montre d'exemple d'identifiant d'objet épistémique, là où `evt_01…` apparaît littéralement au
/// §10.1. W1.b ou W1.c le confirmeront ou le remplaceront ; ce sera alors une modification de
/// schéma, pas de code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StableKind;

impl IdKind for StableKind {
    const PREFIX: &'static str = "obj";
}

/// La nature d'un `revision_id`.
///
/// §7.7 : « `revision_id` identifie une **version immuable** ». Préfixe provisoire, même raison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RevisionKind;

impl IdKind for RevisionKind {
    const PREFIX: &'static str = "rev";
}

/// L'identité du concept, stable à travers ses versions.
pub type StableId = Id<StableKind>;

/// L'identité d'une version immuable.
///
/// Un type distinct de [`StableId`], et c'est le point : les deux sont des ULID de même forme, et
/// rien à l'exécution ne les distinguerait. Le préfixe les sépare sur le fil, le type les sépare à
/// la compilation. Passer l'un pour l'autre est l'erreur qu'on ne remarque qu'en lisant un
/// historique devenu faux — la même famille que `attempt` contre `attempt_id` en §11.1.
pub type RevisionId = Id<RevisionKind>;
