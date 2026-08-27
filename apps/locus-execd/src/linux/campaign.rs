//! Ce qu'une campagne de sondes demande à un mécanisme de confinement — `W5.af.3`.
//!
//! # Pourquoi une couture, et pourquoi celle-ci
//!
//! `selftest.rs` sait conduire une campagne : ouvrir une sandbox par sonde, l'éprouver, la démonter,
//! et ne jamais confondre « contenue » avec « pas lancée ». Ce savoir-là a coûté sept sprints —
//! `W5.n` à `W5.r` — et il ne dépend d'aucun mécanisme.
//!
//! Ce qui dépend du mécanisme est **une seule opération** : éprouver une commande *dans* la sandbox
//! déjà ouverte. `podman` la fait avec `podman exec`, qui entre dans un conteneur qui tourne.
//! `bubblewrap` n'a pas d'`exec` — mesuré, il n'a ni `exec`, ni `enter`, ni `attach` — parce qu'il
//! n'a pas de conteneur où entrer : la sonde **est** la commande qu'il enveloppe.
//!
//! La couture est donc posée là, à la primitive, et pas plus haut. Un trait qui aurait porté « passe
//! la campagne » aurait fait recopier à chaque mécanisme la discipline de retrait et la reprise sur
//! lancement raté, c'est-à-dire précisément ce que ces sept sprints ont appris.
//!
//! # Les trois accesseurs qui accompagnent la primitive
//!
//! [`ProbeHost::is_probeable`] mérite son nom, et ce n'est pas de la cosmétique. Le premier jet
//! demandait « la sandbox tourne-t-elle », ce que `podman inspect` sait dire — mais pour un
//! mécanisme où **rien ne tourne entre deux sondes**, la question n'a pas de réponse honnête : `bwrap`
//! sort à chaque invocation, et répondre `false` ferait déclarer morte une sandbox parfaitement
//! utilisable. La question que la campagne pose réellement est « y a-t-il encore une sandbox à
//! éprouver ? », et celle-là, les deux mécanismes savent y répondre.
//!
//! Le `Option` reste, et pour la raison d'origine : `None` veut dire **on n'a pas pu demander**, ce
//! qu'un booléen forcerait dans l'une des deux autres réponses.

use std::time::Duration;

use super::process::Execution;
use super::selftest::ProbeContext;
use crate::runtime::{RuntimeError, RuntimePort, SandboxId};

/// Un mécanisme de confinement, vu par la campagne de sondes.
///
/// Il est d'abord un [`RuntimePort`] : la campagne crée, démarre, arrête et retire par le port, sans
/// rien savoir du mécanisme. Ce trait n'ajoute que ce que le port ne peut pas porter — éprouver une
/// commande **à l'intérieur**, ce qui n'a de sens que pour une campagne.
pub trait ProbeHost: RuntimePort {
    /// Éprouver cette commande dans la sandbox ouverte, et rendre ce que l'hôte en a dit.
    ///
    /// Le contexte voyage en **variables d'environnement** : c'est ce que les seize commandes de
    /// `PROBE_COMMANDS` lisent, et le porter autrement obligerait à les réécrire par mécanisme.
    ///
    /// # Errors
    ///
    /// [`RuntimeError`] quand le mécanisme n'a pas pu être interrogé. Un code de sortie non nul
    /// n'est **pas** une erreur : c'est un verdict, et c'est la campagne qui le lit.
    fn probe(
        &self,
        id: &SandboxId,
        command: &[&str],
        context: &ProbeContext,
    ) -> Result<Execution, RuntimeError>;

    /// Reste-t-il une sandbox à éprouver sous cet identifiant ?
    ///
    /// `Some(true)` : oui. `Some(false)` : le mécanisme a répondu, et il n'y en a plus.
    /// `None` : **on n'a pas pu demander**, ce qui n'est aucune des deux autres réponses.
    ///
    /// La campagne s'en sert pour une seule décision : après un lancement que le mécanisme a refusé,
    /// faut-il retenter ? Contre une sandbox disparue, retenter n'apprend rien et coûte le budget —
    /// c'est ce que `W5.p` a mesuré après que `W5.o` eut supposé le contraire.
    fn is_probeable(&self, id: &SandboxId) -> Option<bool>;

    /// La pause avant la première reprise d'un lancement que le mécanisme n'a pas pu faire.
    ///
    /// Elle double ensuite. Un test la met à zéro : contre un double, la durée ne mesure rien et
    /// coûte tout, et c'est le **nombre** de tentatives qui décide si une sonde a été mesurée.
    fn launch_pause(&self) -> Duration;

    /// Le `boot_id` de l'hôte, quand le harnais a su le lire.
    ///
    /// `None` est un fait que la sonde `reach_host_kernel_interfaces` sait dire : sans lui, elle ne
    /// conclut pas, au lieu de conclure sur rien.
    fn host_boot_id(&self) -> Option<&str>;
}
