//! La conversion que la règle 6 ne pouvait pas attraper.
//!
//! Ni `packages/graph` ni `packages/coordination` ne peut voir l'autre. Un tiers, lui, le peut —
//! et c'est exactement ici que la conversion s'écrit, avec les meilleures intentions : « unifier
//! l'affichage des objections dans le cockpit ». Une fois écrite, une objection au périmètre d'un
//! recâblage circule dans la machinerie qui propage l'invalidation sur les claims.

use locus_coordination::ObjectedTo;
use locus_graph::ObjectionTarget;

pub fn unify(target: ObjectedTo) -> ObjectionTarget {
    match target {
        ObjectedTo::Policy => ObjectionTarget::Rule,
        ObjectedTo::Perimeter => ObjectionTarget::Scope,
        _ => ObjectionTarget::Inference,
    }
}
