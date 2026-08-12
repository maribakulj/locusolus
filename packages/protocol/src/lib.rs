//! LEP — primitives de protocole.
//!
//! Ce crate ne porte pas encore le protocole : il porte ce sur quoi tout le reste sera écrit —
//! les identifiants, l'horodatage, l'enveloppe d'erreur et le versionnement. Les schémas JSON
//! (W0.5, W0.6) et le SDK généré (W0.8) viendront s'appuyer dessus.
//!
//! # Une seule forme canonique
//!
//! Trois de ces quatre primitives ont une représentation textuelle, et chacune n'en accepte
//! qu'**une**. `docs/SPEC_V1.md` §7.7 dit que « les hashes portent sur une canonicalisation
//! stable » et que « la présentation locale des dates n'affecte jamais les signatures ni les
//! hashes ». Une forme alternative mais valide — un horodatage sans millisecondes, un
//! identifiant en minuscules — est donc **refusée** plutôt que normalisée en silence : deux
//! pairs qui l'écriraient différemment calculeraient deux hashes différents sur la même donnée,
//! et c'est précisément le genre de divergence que les fixtures inter-SDK de `docs/06` existent
//! pour attraper.
//!
//! # Aucune horloge, aucune source d'aléa
//!
//! Rien ici ne lit l'heure ni ne tire au sort. Générer un identifiant demande un instant et de
//! l'entropie, tous deux fournis par l'appelant. Le crate reste pur, donc déterministe en test,
//! et l'invariant 1 — « le domaine ne dépend pas du backend de déploiement » — tient jusque
//! dans les fondations.

pub mod error;
pub mod id;
pub mod time;
pub mod version;

pub use error::{Category, Retry, RetryCondition, StructuredError};
pub use id::{Id, IdKind, ParseIdError};
pub use time::{ParseTimestampError, Timestamp};
pub use version::{ParseVersionError, ProtocolVersion};
