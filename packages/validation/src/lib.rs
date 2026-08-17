//! Validation épistémique et propagation de l'invalidation — `docs/SPEC_V1.md` §8.
//!
//! # La phrase qui gouverne ce paquet
//!
//! §8.3, deuxième point : lorsqu'une prémisse est réfutée, Locus Solus « **ne les réfute pas
//! automatiquement** sans règle disciplinaire ».
//!
//! C'est une contrainte sur ce que ce code a le droit de **rendre**, pas seulement sur ce qu'il
//! fait : [`propagation::Propagation`] n'a aucun champ portant un niveau de validation. Une
//! propagation qui rendrait un niveau révisé aurait déjà pris la décision qu'elle a interdiction
//! de prendre — et l'appelant l'appliquerait, parce qu'un champ rendu par une fonction a l'air
//! d'un résultat.
//!
//! Ce que la propagation rend est une **question posée** : des objets marqués `needs_reassessment`
//! et des tâches ouvertes. La réponse appartient à une revue.
//!
//! # Les cinq points, et où chacun vit
//!
//! 1. « identifie les objets transitivement dépendants » — parcours en largeur, par les
//!    hyperarêtes de W1.e et par [`propagation::DEPENDENCY_RELATIONS`] ;
//! 2. « ne les réfute pas automatiquement » — aucun niveau dans le résultat, et un test le vérifie
//!    par l'absence ;
//! 3. « les marque `needs_reassessment` » — [`propagation::ReassessmentMark`] ;
//! 4. « ouvre des tâches de réévaluation selon la politique » — et **dit** quand il n'y a pas de
//!    politique, plutôt que de rendre une liste vide qui se lirait « rien à réévaluer » ;
//! 5. « conserve le niveau et la justification antérieurs » — [`propagation::PriorAssessment`].
//!    Sans cette trace, une réévaluation repartirait de zéro, et le travail qui avait mené à L3
//!    serait perdu au lieu d'être remis en question.
//!
//! # Ce que §8.4 interdit, et qui n'existe pas ici
//!
//! « Les scores de confiance des agents sont des métadonnées de calibration. Ils ne remplacent ni
//! les preuves, ni les revues, ni les niveaux de validation. Une moyenne de confiance ne constitue
//! jamais une procédure de décision par défaut. »
//!
//! Aucune fonction de ce crate ne prend une confiance en entrée, et aucune n'en calcule la moyenne.

pub mod policy;
pub mod propagation;

pub use policy::{Condition, InvalidatingEvent, TypePolicy};
pub use propagation::{
    DEPENDENCY_RELATIONS, PriorAssessment, PriorAssessments, Propagation, ReassessmentMark,
    ReassessmentTask, Trigger, propagate,
};
