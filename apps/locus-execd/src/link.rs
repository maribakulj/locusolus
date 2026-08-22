//! Le côté qui répond — `W4.h`, ADR 0028.
//!
//! # Ce module est la moitié broker du couloir
//!
//! `W22.c` a découvert que les deux binaires existaient sans se parler. `packages/broker` porte le
//! tube ; ce module porte ce que le broker **met dedans**, c'est-à-dire la traduction de son
//! [`Readiness`] vers la forme de fil.
//!
//! # Pourquoi une traduction, et pas le type du domaine sur le fil
//!
//! [`crate::linux::Missing`] porte `what: &'static str` : il se sérialise, il ne se **désérialise**
//! pas — une chaîne lue sur un fil n'est pas statique. Et faire voyager le type du domaine
//! obligerait `locusd` à dépendre du crate qui contient la seule fonction du dépôt exécutant
//! `podman`, ce qui tiendrait la règle 4 de `boundaries.json` à la lettre contre son objet.
//!
//! C'est la même forme que [`crate::wire`], qui traduit déjà les refus d'admission vers
//! `packages/lep`, et le niveau de confinement passe d'ailleurs par la **même** fonction —
//! [`crate::wire::level`] — plutôt que par une seconde copie qui divergerait d'un cran.
//!
//! # Le broker ne rappelle personne
//!
//! [`serve`] lit une requête et répond ; il n'ouvre aucune connexion sortante. ADR 0028 décision 3 :
//! un programme qui répond n'a besoin que d'écouter, et doubler la surface du processus privilégié
//! pour lui donner l'initiative irait contre la raison d'être de la séparation.

use std::os::unix::net::UnixListener;

use locus_broker::protocol::{Missing as WireMissing, Verdict};
use locus_broker::unix::answer;

use crate::linux::{HostFacts, Missing};
use crate::readiness::Readiness;
use crate::wire::level;

/// Traduire un manque vers sa forme de fil.
///
/// Les deux variantes ne se fondent pas, ici comme dans le domaine : « l'hôte ne l'offre pas »
/// envoie changer de machine, « on n'a pas pu l'établir » envoie regarder pourquoi la lecture a
/// échoué. La conversion est **exhaustive** — une variante nouvelle sans forme de fil ne compile
/// pas, exactement comme [`crate::wire::reason`] le tient pour les refus d'admission.
#[must_use]
pub fn missing(item: &Missing) -> WireMissing {
    match item {
        Missing::Unavailable { what, reason } => WireMissing::Unavailable {
            what: (*what).to_owned(),
            reason: reason.clone(),
        },
        Missing::Undetermined { what, reason } => WireMissing::Undetermined {
            what: (*what).to_owned(),
            reason: reason.clone(),
        },
    }
}

/// Ce que le broker répond à une question de disponibilité.
///
/// Le verdict est calculé depuis les faits d'hôte à **chaque** demande, et non figé au démarrage :
/// un hôte peut perdre une capacité — un système de fichiers démonté, un cgroup remanié — et une
/// réponse mise en cache affirmerait alors quelque chose que plus rien ne vérifie.
#[must_use]
pub fn verdict(facts: &HostFacts) -> Verdict {
    match Readiness::assess(facts) {
        Readiness::Provable { ceiling } => Verdict::Provable {
            ceiling: level(ceiling),
        },
        Readiness::HostShort {
            ceiling,
            missing: absent,
        } => Verdict::HostShort {
            ceiling: level(ceiling),
            missing: absent.iter().map(missing).collect(),
        },
    }
}

/// Servir, une connexion à la fois, jusqu'à ce que l'écoute cesse.
///
/// # Une connexion en échec n'arrête pas le broker
///
/// Un client qui coupe au milieu, qui envoie une ligne sans fin ou qui parle un autre protocole ne
/// doit pas emporter le service : ce serait un déni de service ouvert à quiconque peut se connecter.
/// L'échec est rendu à l'appelant, qui décide d'en faire une trace ; le module, lui, reprend la
/// boucle.
pub fn serve<F>(listener: &UnixListener, facts: &HostFacts, mut report: F)
where
    F: FnMut(&str),
{
    for incoming in listener.incoming() {
        match incoming {
            Ok(stream) => {
                if let Err(error) = answer(&stream, |_| verdict(facts)) {
                    report(&format!("connexion abandonnée — {error}"));
                }
            }
            Err(error) => report(&format!("connexion non acceptée — {error}")),
        }
    }
}
