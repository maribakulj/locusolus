//! La mémoire — `docs/SPEC_V1.md` §16.
//!
//! # Ce que ce crate porte, et ce qu'il ne reprend pas
//!
//! Les niveaux de §16.1, le retrieval hybride de §16.3, la déduplication de §16.4 et la compaction
//! de §16.5. **Les cinq préventions de contamination de §16.6 ne sont pas ici** : elles vivent dans
//! `packages/review/src/contamination.rs` depuis W7.b, écrites par cas adverses. Les réécrire
//! produirait deux listes de cinq qui divergeraient, et la seconde aurait l'air aussi vraie que la
//! première.
//!
//! # La phrase qui décide de la forme
//!
//! §16.1, dernière ligne : « le graphe, les événements et les artefacts sont **canoniques**. Les
//! résumés et embeddings sont des **projections régénérables**. »
//!
//! C'est une frontière, pas une nuance de vocabulaire. Perdre une projection coûte un recalcul ;
//! perdre un canonique coûte la vérité institutionnelle. [`Substance`] la porte dans le type, de
//! sorte qu'un appelant ne puisse pas déclarer régénérable ce qui ne l'est pas — une compaction qui
//! se croirait canonique deviendrait la source, et l'invariant 2 tomberait sans que rien n'échoue.

pub mod level;
pub mod retrieval;

pub use level::{Entry, Level, MemoryError, Shelf, Substance};
pub use retrieval::{Candidate, Excluded, Ranking, Results, RetrievalError, Signal, retrieve};
