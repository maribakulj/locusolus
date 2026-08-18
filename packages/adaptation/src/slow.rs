//! La boucle lente — celle qui change la **structure**.
//!
//! `docs/10_V1_ROADMAP.md`, W18 : « boucle lente sur la structure ».
//!
//! # Ce module ne déclare aucun type et n'enveloppe aucun chemin
//!
//! Une adaptation lente **est** une `coordination::Proposal` de W13, écrite par `Proposal::write`,
//! approuvée par `approve`, commitée par `commit`. Il n'y a pas de `StructuralAdaptation`, pas
//! d'`adapt()` qui envelopperait les quatre étapes, pas de réexport. Deux tests lisent ce source :
//! l'un vérifie qu'il ne déclare ni `struct`, ni `enum`, ni `trait` publics ; l'autre qu'il n'y a
//! qu'une seule fonction publique, celle ci-dessous.
//!
//! Trois raisons, et la troisième est la vraie.
//!
//! `CLAUDE.md` interdit le vocabulaire parallèle : « les objets d'organisation, de coordination et
//! de gouvernance sont ceux de `SPEC_V1.md`, **sous leur nom** ». Un type de plus serait un nom que
//! la spec ne porte pas.
//!
//! Un `adapt()` qui prendrait les sept arguments de `Proposal::write` plus le déclencheur serait une
//! deuxième signature à maintenir, qui divergerait de la première au premier champ ajouté — et qui
//! divergerait **en silence**, puisqu'elle compilerait encore.
//!
//! Surtout : une porte à soi est une porte qu'on peut franchir **sans** vérifier le mode du
//! déploiement, sans citer une révision existante, sans base de révision — puis qu'on convertit « au
//! moment de committer », c'est-à-dire trop tard pour refuser. Le chemin entier de W13 — mode,
//! citation, approbation par un autre, base à jour — ne tient que s'il n'existe aucune autre porte.
//!
//! # Ce que ce module ajoute, et c'est tout
//!
//! Le déclencheur d'une adaptation **automatique** vient de la liste close de §14.5, alors que
//! `Justification` porte un `&str` ouvert. Le champ est ouvert à dessein — son commentaire le dit,
//! « la liste n'est pas fermée ici parce qu'elle relève de la politique » — et un humain peut
//! justifier une proposition par ce qu'il veut. Un agent, non : ce qu'il observe est l'un des onze,
//! et [`justify`] est la porte par laquelle il écrit ce champ.
//!
//! ```
//! # use locus_adaptation::{Trigger, slow};
//! # fn demo(cites: locus_domain::RevisionId) -> Result<(), locus_coordination::ProposalError> {
//! let justification = slow::justify(Trigger::ReviewDisagreement, cites)?;
//! assert_eq!(justification.trigger(), "review_disagreement");
//! // Puis `Proposal::write(id, author, mode, base, change, justification, index)`, tel quel.
//! # Ok(())
//! # }
//! ```

use locus_coordination::{Justification, ProposalError};
use locus_domain::RevisionId;

use crate::spawn::Trigger;

/// Justifier une adaptation automatique par l'un des onze déclencheurs de §14.5.
///
/// # Errors
///
/// [`ProposalError::EmptyTrigger`] ne peut pas se produire — un `slug()` n'est jamais vide — mais
/// l'erreur est propagée plutôt qu'écartée par un `expect`. Un `expect` ici serait juste aujourd'hui
/// et faux le jour où l'énumération gagne un membre mal nommé, et il paniquerait dans `locusd` au
/// lieu de refuser.
pub fn justify(trigger: Trigger, cites: RevisionId) -> Result<Justification, ProposalError> {
    Justification::new(trigger.slug(), cites)
}
