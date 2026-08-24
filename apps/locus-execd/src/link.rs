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
//!
//! # Deux questions depuis `W20.q`, et le dispatch est **exhaustif**
//!
//! [`answer_ask`] n'a pas de bras fourre-tout : une variante nouvelle d'[`Ask`] sans réponse ne
//! compile pas. C'est la même garantie structurelle que [`crate::wire::reason`] obtient pour les
//! motifs de refus, et c'est ce qui empêche une question nouvelle de recevoir en silence la réponse
//! d'une autre.

use std::os::unix::net::UnixListener;

use locus_broker::protocol::{Ask, Missing as WireMissing, Verdict};
use locus_broker::unix::answer;

use crate::announced::{Proven, placement, shortfalls};
use crate::linux::{HostFacts, Missing};
use crate::placement::Placement;
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

/// Ce que le broker répond à une demande de placement — `W20.q`.
///
/// # Une demande illisible n'est pas un refus de placement
///
/// Elle rend [`Verdict::Refused`], qui veut dire « le broker a parlé et il ne répond pas à ça ».
/// Lui rendre un `NotPlaced` vide enverrait chercher une machine plus grosse à qui a envoyé un
/// document incomplet — et un `NotPlaced` sans manque serait exactement le refus muet que l'ADR 0028
/// décision 2 refuse.
#[must_use]
pub fn verdict_of_placement(
    manifest: &locus_lep::CapabilityManifest,
    sandbox: &locus_lep::SandboxSpec,
    resources: &locus_lep::ResourceSpec,
    proven: &dyn Proven,
) -> Verdict {
    match placement(manifest, sandbox, resources, proven) {
        Ok(Placement::Placed { worker, level: at }) => Verdict::Placed {
            worker,
            level: level(at),
        },
        Ok(Placement::Refused {
            shortfalls: missing,
        }) => Verdict::NotPlaced {
            shortfalls: shortfalls(&missing),
        },
        Err(unreadable) => Verdict::Refused {
            why: unreadable.to_string(),
        },
    }
}

/// La réponse à une question, quelle qu'elle soit.
///
/// Exhaustif par construction : ajouter une variante à [`Ask`] sans lui donner de réponse **ne
/// compile pas**. Un bras fourre-tout aurait fait recevoir à une question nouvelle la réponse d'une
/// autre, ce que `packages/broker` refuse de l'autre côté du fil.
#[must_use]
pub fn answer_ask(ask: &Ask, facts: &HostFacts, proven: &dyn Proven) -> Verdict {
    match ask {
        Ask::Readiness => verdict(facts),
        Ask::Place {
            manifest,
            sandbox,
            resources,
        } => verdict_of_placement(manifest, sandbox, resources, proven),
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
pub fn serve<F>(listener: &UnixListener, facts: &HostFacts, proven: &dyn Proven, mut report: F)
where
    F: FnMut(&str),
{
    for incoming in listener.incoming() {
        match incoming {
            Ok(stream) => {
                if let Err(error) =
                    answer(&stream, |request| answer_ask(&request.ask, facts, proven))
                {
                    report(&format!("connexion abandonnée — {error}"));
                }
            }
            Err(error) => report(&format!("connexion non acceptée — {error}")),
        }
    }
}
