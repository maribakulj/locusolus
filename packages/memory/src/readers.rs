//! Les trois classes de lecteurs d'une trace, et la lecture institutionnelle journalisée —
//! `W26.b`, ADR 0027 décision 2.
//!
//! # Trois, et il n'y en a pas de quatrième
//!
//! | Lecteur | Ce qu'il obtient | À quelle condition |
//! |---|---|---|
//! | Le **générateur** | sa propre trace | sans condition |
//! | L'**institution** — un humain, par le cockpit de §20 | toute trace | sans condition d'autorisation, **mais la lecture est journalisée** |
//! | Un **pair** — un autre agent | une trace nommée | **seulement** par un dévoilement valide |
//!
//! C'est le partage que la question posée à l'ADR demandait : l'humain voit, les agents non, sauf
//! règle. Ce qui s'y ajoute est la journalisation de la lecture institutionnelle — elle ne restreint
//! personne, elle empêche qu'un accès sans trace devienne le chemin par lequel un contexte non
//! autorisé remonte, et elle coûte une ligne de journal.
//!
//! **Un « lecteur système » ou un « outil d'analyse » serait la porte dérobée de ce mécanisme.**
//! L'ADR le dit et ce module le tient : [`Reader`] a trois variantes, [`read`] les épuise sans
//! joker, et un test d'absence refuse le vocabulaire d'une quatrième. Une classe nouvelle demande un
//! amendement de l'ADR, pas une ligne de plus dans un `match`.
//!
//! # Le fait n'est pas à côté du contenu, il est dedans
//!
//! [`Reading::Institutional`] porte le grant **et** le fait dans la même variante. Un
//! `Option<InstitutionalRead>` rendu à côté se laisserait ignorer d'un `?` ; ici, l'appelant ne peut
//! pas déstructurer la lecture sans que le fait lui tombe dans la main.
//!
//! C'est l'argument que `messaging.rs` a déjà écrit pour `Reception` — « un fait que l'appelant doit
//! traiter, là où un `Option` l'aurait laissé l'ignorer d'un `?` ». Ce module ne prétend pas plus :
//! rien n'empêche un appelant de lier le fait à `_`, et aucun type de valeur pure ne le pourrait. Ce
//! qu'on tient est qu'il faut le faire **exprès**.
//!
//! # Ce module ne lit pas l'heure
//!
//! L'instant du fait est **fourni**, comme `domain::Envelope` le fait pour une révision. Un journal
//! dont les instants viennent de la montre de celui qui écrit n'est pas rejouable, et l'invariant 1
//! exclut du domaine le choix d'une horloge.
//!
//! # Ce qu'un pair obtient aujourd'hui, et pourquoi ce n'est pas un moignon
//!
//! Rien. Un pair lit **seulement** par un dévoilement valide (ADR 0027 décision 3), et l'énumération
//! des motifs de dévoilement **commence vide** — chaque motif arrive avec le mécanisme qui le
//! déclenche, et `W26.c` livre le premier. Aucun dévoilement n'existe donc, et
//! [`Refusal::NeedsDisclosure`] est la réponse **exacte** à l'état présent du système, pas un
//! `todo!()` déguisé.
//!
//! La différence se voit à ceci : quand `W26.c` livrera `Disclosure`, ce refus ne sera pas
//! *corrigé*, il sera *conditionné*. Rien ici n'annonce un effet qui n'a pas lieu.

use locus_domain::ContentHash;
use locus_protocol::Timestamp;

use crate::genre::Genre;
use crate::reasoning::Trace;

/// Qui lit — les trois classes de l'ADR 0027 décision 2, **et pas une quatrième**.
///
/// Fermée exprès. Un lecteur qui n'est aucune des trois n'a pas de chemin dans [`read`], et c'est
/// une propriété du type plutôt qu'une discipline d'appelant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reader {
    /// L'agent qui a produit la trace, lisant la sienne.
    Generator {
        /// Son identifiant, confronté à celui que la trace enregistre.
        agent_id: String,
    },
    /// Un humain, par le cockpit de §20.
    Institution {
        /// Qui, nommément — un fait de journal sans nom ne dit pas grand-chose.
        operator: String,
    },
    /// Un autre agent.
    Peer {
        /// Son identifiant.
        agent_id: String,
    },
}

/// Ce qu'une lecture accordée rend.
///
/// # Une référence, jamais des octets
///
/// L'identifiant et le condensat, c'est-à-dire exactement la façon dont §9.1 désigne un artefact.
/// Ce crate n'a jamais stocké le contenu d'une trace — `W26.a` le tient par l'absence — et un grant
/// qui rendrait des octets serait le second stockage par un autre bout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Granted {
    artifact_id: String,
    declared_hash: ContentHash,
}

impl Granted {
    /// L'artefact accordé.
    #[must_use]
    pub fn artifact_id(&self) -> &str {
        &self.artifact_id
    }

    /// Son condensat déclaré — la référence de §9.1.
    #[must_use]
    pub const fn declared_hash(&self) -> &ContentHash {
        &self.declared_hash
    }

    /// Le genre sous lequel elle entre chez le lecteur — **toujours** celui qui influence le rang
    /// et jamais la validité.
    ///
    /// Une constante, pas un champ : ce n'est pas une propriété de ce grant-ci mais de toute trace
    /// lue, et un champ en aurait fait une seconde source pour ce que `W26.a` pose déjà.
    ///
    /// ADR 0027 décision 4 : une trace lue peut changer ce que quelqu'un va **chercher** ; elle ne
    /// peut pas changer ce qui est **tenu pour vrai**.
    #[must_use]
    pub const fn genre(&self) -> Genre {
        Genre::MetaMemory
    }
}

/// Le fait qu'une lecture institutionnelle écrit.
///
/// Ce n'est pas une autorisation : l'institution lit **sans condition d'autorisation**. C'est la
/// trace de l'accès, et c'est tout ce qu'elle coûte.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use = "une lecture institutionnelle est journalisée ; ce fait est ce qu'il y a à écrire"]
pub struct InstitutionalRead {
    artifact_id: String,
    operator: String,
    at: Timestamp,
}

impl InstitutionalRead {
    /// Quelle trace a été lue.
    #[must_use]
    pub fn artifact_id(&self) -> &str {
        &self.artifact_id
    }

    /// Par qui.
    #[must_use]
    pub fn operator(&self) -> &str {
        &self.operator
    }

    /// Quand — l'instant **fourni**, jamais lu d'une horloge par ce module.
    #[must_use]
    pub const fn at(&self) -> Timestamp {
        self.at
    }
}

/// Pourquoi une lecture est refusée.
///
/// Trois motifs, et pas un booléen : ils appellent des gestes différents. Un `false` unique ferait
/// chercher un dévoilement à qui s'est simplement trompé de trace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// Un agent demande la trace d'un autre en se réclamant du générateur.
    NotYourTrace {
        /// Qui demande.
        asked_by: String,
        /// Qui l'a produite.
        produced_by: String,
    },
    /// La trace n'enregistre aucun agent : « c'est la mienne » n'est pas vérifiable.
    ///
    /// **Non vérifié n'est jamais accordé.** `ProducedBy::agent_id` est facultatif dans le schéma
    /// d'artefact, donc une trace peut arriver sans générateur nommé ; l'accorder à qui l'affirme
    /// ferait de l'affirmation la preuve.
    UnknownGenerator,
    /// Un pair, faute de dévoilement valide.
    NeedsDisclosure {
        /// Qui demande.
        asked_by: String,
    },
}

/// Ce qu'une lecture rend.
///
/// # Pourquoi le fait est dans la variante
///
/// `Institutional` porte le grant **et** le fait. Les séparer — un grant d'un côté, un
/// `Option<InstitutionalRead>` de l'autre — laisserait la journalisation à la discipline de
/// l'appelant, et c'est exactement ce que l'ADR ne veut pas : « un accès sans trace devient le
/// chemin par lequel un contexte non autorisé remonte ».
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reading {
    /// Le générateur lit la sienne. **Rien à journaliser** : il n'y a pas de tiers dans l'affaire.
    Own(Granted),
    /// L'institution lit, et la lecture est un fait.
    Institutional(Granted, InstitutionalRead),
    /// Refusé, avec le motif.
    Refused(Refusal),
}

impl Reading {
    /// Le fait à journaliser, s'il y en a un.
    ///
    /// Un accesseur de commodité pour compter et pour écrire ; il ne remplace pas le `match`, qui
    /// reste le seul chemin vers le contenu accordé.
    #[must_use]
    pub const fn fact(&self) -> Option<&InstitutionalRead> {
        match self {
            Self::Institutional(_, fact) => Some(fact),
            Self::Own(_) | Self::Refused(_) => None,
        }
    }
}

/// Lire une trace, en tant que l'une des trois classes.
///
/// # L'exhaustivité est la garantie
///
/// Le `match` sur [`Reader`] épuise les trois variantes **sans joker**. Un `_ =>` absorberait en
/// silence une quatrième classe le jour où quelqu'un l'ajouterait, et le compilateur n'aurait plus
/// rien à dire — c'est le contraire de ce qu'une énumération close sert à faire. Un test lit cette
/// source et refuse le joker, parce que le compilateur, lui, ne se plaint jamais d'un `match` trop
/// tolérant.
///
/// `at` est l'instant du fait, **fourni** : voir l'en-tête du module.
#[must_use]
pub fn read(reader: &Reader, trace: &Trace, at: Timestamp) -> Reading {
    let manifest = trace.manifest();
    let granted = Granted {
        artifact_id: manifest.artifact_id().to_owned(),
        declared_hash: manifest.declared_hash().clone(),
    };

    match reader {
        Reader::Generator { agent_id } => match manifest.produced_by().agent_id.as_deref() {
            None => Reading::Refused(Refusal::UnknownGenerator),
            Some(produced_by) if produced_by == agent_id => Reading::Own(granted),
            Some(produced_by) => Reading::Refused(Refusal::NotYourTrace {
                asked_by: agent_id.clone(),
                produced_by: produced_by.to_owned(),
            }),
        },
        Reader::Institution { operator } => {
            let fact = InstitutionalRead {
                artifact_id: granted.artifact_id.clone(),
                operator: operator.clone(),
                at,
            };
            Reading::Institutional(granted, fact)
        }
        Reader::Peer { agent_id } => Reading::Refused(Refusal::NeedsDisclosure {
            asked_by: agent_id.clone(),
        }),
    }
}
