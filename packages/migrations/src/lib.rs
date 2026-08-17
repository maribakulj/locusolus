//! Migrations de schéma et portabilité — `docs/SPEC_V1.md` §10.4, §4.1.
//!
//! # La décision de ce paquet
//!
//! Une migration qui monte sait rarement redescendre, et **le prétendre est pire que l'admettre**.
//! Une chaîne qui redescendrait à travers une étape destructive rendrait un document ancien qui
//! n'a jamais existé — et il aurait l'air authentique, ce qui est exactement le genre de faux dont
//! un journal canonique ne se remet pas.
//!
//! D'où deux constructeurs et un seul type : [`migration::Migration::reversible`] pour celles qui
//! savent redescendre, [`migration::Migration::lossy`] pour les autres — et la seconde **exige** de
//! déclarer ce qu'elle perd. L'irréversibilité devient alors exécutoire plutôt que documentée :
//! [`chain::MigrationChain::downcast`] refuse, et le refus porte la liste des champs perdus.
//!
//! # Le test de sortie
//!
//! « Migration aller-retour » : sur une chaîne réversible, monter puis redescendre rend
//! **exactement** le document d'origine. C'est une propriété vérifiée, pas supposée — et là où
//! elle ne peut pas tenir, le code le dit au lieu de faire semblant.
//!
//! # §4.1, ce que `boundaries.json` ne voit pas
//!
//! La garde de frontières vérifie les **imports**. Elle ne voit pas les noms de champ ni les
//! littéraux : un `Claim` qui porterait `s3_bucket` ne violerait aucune règle d'import et rendrait
//! pourtant l'objet indéplaçable. [`portability::provider_findings`] couvre ce trou.

pub mod chain;
pub mod migration;
pub mod portability;

pub use chain::{MINIMUM_SUPPORTED_VERSIONS, MigrationChain};
pub use migration::{Loss, Migration, MigrationError, SchemaVersion};
pub use portability::{PROVIDER_MARKERS, PortabilityFinding, provider_findings};
