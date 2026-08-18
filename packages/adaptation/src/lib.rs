//! L'adaptation automatique — `docs/SPEC_V1.md` §14.5, ADR 0016 décisions 7 et 8.
//!
//! # Un crate séparé, pour la raison qui sépare les deux boucles
//!
//! W18 tient deux boucles : une **rapide**, qui change la capacité d'un agent — routage de modèle,
//! choix d'outil, sélection de skill, retry, routes éphémères — et une **lente**, qui change la
//! structure de l'organisation. Elles n'ont ni la même latence, ni le même degré d'autorité, ni les
//! mêmes conséquences quand elles se trompent. Les loger dans `packages/coordination` reviendrait à
//! donner à la boucle rapide le vocabulaire de la lente, et rien n'empêcherait ensuite un routage de
//! modèle de s'écrire comme une opération de graphe.
//!
//! # Ce que la borne de §14.5 dit, mot pour mot
//!
//! « Le moteur de politique peut accepter, refuser, modifier ou soumettre à approbation. **Aucun
//! agent ne crée librement une flotte non bornée.** »
//!
//! La deuxième phrase est celle qui coûte. Un agent qui observe un déclencheur — `high_uncertainty`,
//! `barrier_encountered` — a par construction une raison de vouloir un agent de plus, et rien dans
//! sa situation ne le pousse à s'arrêter. La borne ne peut donc pas être une discipline d'appel :
//! elle est un chemin de types. Une [`spawn::SpawnProposal`] ne sait pas fabriquer d'agent ; seul
//! un [`spawn::Admitted`] le sait, et le seul producteur d'`Admitted` est [`spawn::dispose`], qui
//! exige un verdict de moteur de politique.

pub mod spawn;

pub use spawn::{
    Admitted, Disposition, Draft, SpawnError, SpawnProposal, Trigger, Undecided, dispose,
};
