//! Ce qui traverse le tube — ADR 0028 décision 5, ADR 0032.
//!
//! # Deux questions, et la seconde est arrivée comme l'ADR 0028 l'annonçait
//!
//! Le lien porte [`Ask::Readiness`] : *broker, sais-tu confiner, et sinon que te manque-t-il ?*
//! C'est la question que `locusd` doit poser avant toute autre — sans elle, il placerait une mission
//! sur un hôte dont il ne sait rien.
//!
//! [`Ask::Place`] est la seconde, et `W20.q` en est le consommateur : *ce worker, tel qu'il
//! s'annonce, peut-il porter cette mission ?* L'ADR 0028 décision 5 écrivait qu'« l'admission […]
//! s'ajouter[ait] comme des variantes de requête sur un tube qui marche » ; c'est exactement ce qui
//! se passe ici, et rien du transport ne change.
//!
//! Au sens de l'ADR 0022 décision 0, chacune est une **capacité finie**. Ce qui serait une promesse
//! serait l'inverse — déclarer ici des variantes que personne ne sait honorer, ce qui est la raison
//! pour laquelle [`Ask::Place`] n'est entrée qu'avec le code qui la pose et celui qui y répond.
//!
//! # Le vocabulaire de fil ne duplique rien
//!
//! Les six niveaux de confinement s'écrivent avec [`locus_lep::SandboxLevel`], qui existe déjà. Une
//! troisième orthographe de `S0`–`S5` serait le « vocabulaire parallèle » que `CLAUDE.md` interdit,
//! et deux orthographes qui divergent d'un cran sont pires qu'une seule mal choisie.
//!
//! La même règle vaut pour [`Ask::Place`], et elle décide tout ce que cette variante porte : le
//! manifeste est [`locus_lep::CapabilityManifest`], l'exigence est [`locus_lep::SandboxSpec`] et
//! [`locus_lep::ResourceSpec`], et un manque s'écrit [`locus_lep::Reason`] — les sept motifs que
//! `apps/locus-execd/src/wire.rs` produit déjà pour un refus d'admission. Une seconde écriture des
//! motifs aurait divergé au premier motif ajouté, et `W5.g` a montré qu'il s'en ajoute.
//!
//! # Pourquoi `Missing` est recopié ici plutôt qu'importé
//!
//! `locus_execd::linux::Missing` porte `what: &'static str` : il se sérialise, il ne se
//! **désérialise** pas — une chaîne lue sur un fil n'est pas statique. La traduction est donc
//! nécessaire, et elle vit du côté qui répond, comme `wire.rs` traduit déjà les refus d'admission
//! vers `packages/lep`. L'importer aurait par ailleurs fait dépendre `locusd` du crate qui contient
//! la seule fonction du dépôt exécutant `podman`, ce qui aurait tenu la règle 4 contre son objet.

use std::fmt;

use locus_lep::{CapabilityManifest, Reason, ResourceSpec, SandboxLevel, SandboxSpec};
use serde::{Deserialize, Serialize};

/// La version du protocole de lien, portée par chaque requête et chaque réponse.
///
/// Elle n'est pas celle de LEP : LEP est le protocole des **workers**, figé en `lep/1.0` et amendé
/// par un mineur. Ce lien-ci est interne au control plane, et confondre les deux ferait qu'un
/// changement de l'un obligerait à versionner l'autre.
pub const PROTOCOL: &str = "broker/1.0";

/// Ce que `locusd` demande.
///
/// Le champ `protocol` est en premier et n'a pas de valeur par défaut : une requête d'une autre
/// version se lit et se refuse, au lieu d'être interprétée avec le vocabulaire courant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    /// La version du protocole de l'appelant.
    pub protocol: String,
    /// Ce qui est demandé.
    pub ask: Ask,
}

impl Request {
    /// La requête de disponibilité, dans la version courante.
    #[must_use]
    pub fn readiness() -> Self {
        Self {
            protocol: PROTOCOL.to_owned(),
            ask: Ask::Readiness,
        }
    }

    /// La requête de placement, dans la version courante — `W20.q`.
    #[must_use]
    pub fn place(
        manifest: CapabilityManifest,
        sandbox: SandboxSpec,
        resources: ResourceSpec,
    ) -> Self {
        Self {
            protocol: PROTOCOL.to_owned(),
            ask: Ask::Place {
                manifest: Box::new(manifest),
                sandbox,
                resources,
            },
        }
    }
}

/// La question posée, sous une forme close.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "ask", rename_all = "snake_case")]
pub enum Ask {
    /// Sais-tu confiner, et sinon que te manque-t-il ?
    Readiness,
    /// Ce worker, tel qu'il s'annonce, peut-il porter cette mission ?
    ///
    /// # Le manifeste voyage entier, et il reste une **annonce**
    ///
    /// §15.3 : un `CapabilityManifest` est ce qu'un worker **annonce**, jamais ce qu'il a prouvé.
    /// Le transmettre tel quel est donc exact, et c'est le répondant — qui, lui, sait ce qui a été
    /// prouvé — qui décide. Le réduire ici à « le niveau qu'il dit tenir » aurait fait passer une
    /// déclaration pour un fait à mi-chemin, à l'endroit où plus personne ne le relit.
    Place {
        /// Ce que le worker annonce. Encadré parce que `clippy` refuse à juste titre qu'une
        /// variante d'énumération pèse dix fois les autres.
        manifest: Box<CapabilityManifest>,
        /// L'isolation que la mission **exige** — un plancher, jamais un inventaire.
        sandbox: SandboxSpec,
        /// Ce que la mission réserve — invariant 6.
        resources: ResourceSpec,
    },
}

/// Ce que `locus-execd` répond.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    /// La version du protocole du répondant.
    pub protocol: String,
    /// Le verdict.
    pub verdict: Verdict,
}

impl Response {
    /// Une réponse dans la version courante.
    #[must_use]
    pub fn new(verdict: Verdict) -> Self {
        Self {
            protocol: PROTOCOL.to_owned(),
            verdict,
        }
    }
}

/// Ce que le broker rend.
///
/// # `Refused` n'est pas une erreur, et le type ne permet pas de les confondre
///
/// `Refused` n'est **pas** une erreur de transport : le broker a parlé, et il a dit non. Une panne
/// de lien ne s'exprime pas ici du tout — elle vit dans [`crate::BrokerError`], du côté de
/// l'appelant, parce qu'un broker qui ne répond pas ne peut par définition rien mettre sur le fil.
/// C'est la décision 4 de l'ADR 0028 rendue structurelle.
///
/// # Un verdict de placement en réponse à une question de disponibilité est un désaccord
///
/// Les cinq variantes vivent dans une seule énumération parce qu'il n'y a qu'une [`Response`] et
/// qu'un [`crate::unix::answer`]. Elles ne répondent pas toutes à la même question, et ce n'est
/// **pas** au lecteur de s'en arranger : [`crate::port::BrokerPort::place`] refuse un
/// `Provable`/`HostShort` comme une réponse hors sujet, et `Standing::probe` côté `locusd` refuse
/// symétriquement un `Placed`/`NotPlaced`. Les deux le disent, plutôt que de l'interpréter — c'est
/// la règle de [`crate::unix::answer`] pour un désaccord de version, appliquée à un désaccord de
/// question.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum Verdict {
    /// L'hôte prouve ce qu'il faut, et voici jusqu'où il plafonne.
    Provable {
        /// Le niveau le plus fort que cet hôte sait tenir.
        ceiling: SandboxLevel,
    },
    /// L'hôte ne prouve pas assez, et voici **tout** ce qui manque.
    HostShort {
        /// Le niveau le plus fort que cet hôte sait tenir malgré tout.
        ceiling: SandboxLevel,
        /// Les manques, dans l'ordre où ils ont été constatés.
        missing: Vec<Missing>,
    },
    /// Le broker refuse de répondre à cet appelant.
    ///
    /// Il porte son propre nom sur le fil parce que, sans lui, la première mise en service se
    /// passerait à chercher un problème de réseau qui n'existe pas — ADR 0028 décision 2.
    Refused {
        /// Pourquoi, en clair.
        why: String,
    },
    /// La mission peut aller sur ce worker, à ce niveau — `W20.q`.
    Placed {
        /// Le worker retenu, sous le nom que **le répondant** a examiné.
        worker: String,
        /// Le niveau qui sera appliqué : celui qu'exige la mission, jamais le plafond de l'hôte.
        level: SandboxLevel,
    },
    /// Aucun worker examiné ne convient, et voici ce qui manquait à **chacun** — `W20.q`.
    ///
    /// La liste est plurielle alors que `W20.q` n'en soumet qu'un, et c'est la forme de
    /// `Placement::Refused` chez le répondant : ne garder que « le plus proche » ferait corriger un
    /// hôte pour découvrir ensuite que les autres manquaient d'autre chose.
    NotPlaced {
        /// Un manque par worker examiné, dans l'ordre où ils ont été soumis.
        shortfalls: Vec<Shortfall>,
    },
}

/// Ce qui manquait à un worker, sur le fil.
///
/// Les motifs sont ceux de §10.2 — [`Reason`], que `apps/locus-execd/src/wire.rs` produit déjà pour
/// un `AdmissionRefusal`. Les réécrire ici aurait fait deux vocabulaires de refus dans le même
/// binaire, dont l'un aurait manqué le motif suivant : c'est arrivé une fois, avec
/// `disk_quota_not_enforceable`, né après l'ADR qui en nommait six.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Shortfall {
    /// Le worker concerné.
    pub worker: String,
    /// **Tout** ce qui lui manquait, dans l'ordre où le répondant l'a constaté.
    pub reasons: Vec<Reason>,
}

/// Un manque, sous une forme qui traverse un fil.
///
/// Les deux variantes ne se fondent pas : « l'hôte ne l'offre pas » envoie changer de machine,
/// « on n'a pas pu l'établir » envoie regarder pourquoi la lecture a échoué. C'est la règle de
/// `W5.h`, et elle vaut ici comme là-bas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "missing", rename_all = "snake_case")]
pub enum Missing {
    /// L'hôte ne l'offre pas.
    Unavailable {
        /// La capacité concernée.
        what: String,
        /// Ce qui le dit.
        reason: String,
    },
    /// On n'a pas pu l'établir.
    Undetermined {
        /// La capacité concernée.
        what: String,
        /// Ce qui a empêché de savoir.
        reason: String,
    },
}

impl fmt::Display for Missing {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable { what, reason } => write!(formatter, "{what} : {reason}"),
            Self::Undetermined { what, reason } => {
                write!(formatter, "{what} : indéterminé — {reason}")
            }
        }
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provable { ceiling } => {
                write!(formatter, "prouvé jusqu'à {ceiling:?}")
            }
            Self::HostShort { ceiling, missing } => {
                write!(
                    formatter,
                    "hôte insuffisant — plafond {ceiling:?}, {} manque(s)",
                    missing.len()
                )
            }
            Self::Refused { why } => write!(formatter, "refusé — {why}"),
            Self::Placed { worker, level } => {
                write!(formatter, "« {worker} » en {level:?}")
            }
            Self::NotPlaced { shortfalls } if shortfalls.is_empty() => {
                formatter.write_str("aucun worker n'a été soumis")
            }
            Self::NotPlaced { shortfalls } => {
                write!(formatter, "aucun des {} workers soumis", shortfalls.len())
            }
        }
    }
}
