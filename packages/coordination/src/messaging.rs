//! La messagerie inter-agents — ADR 0019, `W16.e`, `docs/13` §3.
//!
//! # Ce module ne transporte rien
//!
//! ADR 0019, décision 1 : **un message est un événement**. La messagerie n'est pas un transport
//! parallèle au journal, c'est un usage du journal. Émettre, c'est écrire un fait ; recevoir, c'est
//! lire par cursor — le mécanisme que `W20.e` et `W20.f` ont livré.
//!
//! Ce qui vit ici est donc ce qu'un journal ne sait pas décider tout seul : **sous quel epoch un
//! émetteur a agi**, et ce qu'un destinataire doit conclure quand ce n'est pas le sien.
//!
//! # Le contenu d'un message n'est pas modélisé, et c'est délibéré
//!
//! [`Message`] porte l'émetteur, le destinataire, l'epoch et le sujet. Pas le contenu. Le contenu
//! est la **charge de l'événement**, et le typer ici en produirait une seconde représentation du
//! même fait — deux représentations d'une même chose divergent le jour où l'une est corrigée, ce
//! qui est l'argument même par lequel l'ADR 0019 a écarté le courtier dédié. Le journal tient la
//! charge et son condensat ; ce module tient ce qui l'entoure.
//!
//! Les noms des champs dont il est question ici ne s'écrivent pas littéralement, et c'est la même
//! discipline que `version.rs` applique déjà : un test lit cette source et refuse ces noms, parce
//! qu'une garde qui doit décider, à chaque relecture, si une occurrence est un usage ou une
//! explication est une garde qu'on finit par assouplir. La périphrase est le prix de n'avoir jamais
//! à en juger.
//!
//! Pour la même raison, **un message n'a pas d'identifiant à lui**. Son identité est celle de
//! l'événement qui le porte. Un `Id<Message>` serait une seconde identité du même fait, et la
//! question « lequel des deux est le bon ? » n'aurait pas de réponse.
//!
//! # Un epoch est une `Version`, pas un compteur
//!
//! ADR 0019, décision 2. Ce qui change la configuration d'un ensemble d'agents produit déjà une
//! [`Version`] (ADR 0016) ; c'est elle l'epoch. Ouvrir un compteur `epoch: u64` à côté aurait donné
//! deux vocabulaires de version pour une seule chose, ce que `CLAUDE.md` interdit — et le second
//! aurait dérivé du premier au premier oubli.
//!
//! La conséquence est qu'un epoch **ne se compare pas** par `<`. Une `VersionId` est un hash. Ce
//! qui ordonne deux epochs est la **filiation** : [`Epochs`] tient la suite que le destinataire a
//! réellement traversée, et [`Epochs::advanced_to`] refuse un maillon qui n'est pas l'enfant du
//! dernier. Une suite qu'on ne peut pas fabriquer est ce qui rend le verdict opposable.
//!
//! # Trois verdicts, parce que deviner et ignorer sont deux fautes distinctes
//!
//! ADR 0019, décision 3 et condition 2. Un message tardif n'est ni appliqué en silence, ni jeté en
//! silence : il est **rapporté**.
//!
//! - [`Reception::Delivered`] — l'émetteur a agi sous l'epoch courant du destinataire ;
//! - [`Reception::Late`] — sous un epoch que le destinataire a traversé **puis quitté**. Le verdict
//!   nomme les deux, parce qu'un destinataire qui lirait « tardif » sans savoir de combien ne
//!   pourrait rien en faire ;
//! - [`Reception::Unknown`] — sous un epoch que le destinataire n'a jamais vu. Ce n'est **pas** un
//!   `Late` atténué : un epoch inconnu peut venir d'une reconfiguration plus récente, ou d'une
//!   lignée divergente, et les deux appellent des suites opposées. Les fondre rendrait un verdict
//!   plausible là où il n'y a pas d'information.
//!
//! Ce que ce module ne décide **pas** : à qui un message est adressé. Un destinataire qui n'est pas
//! le bon n'est pas une question d'epoch, et [`Epochs::receive`] ne s'en mêle pas — c'est le routage
//! qui lit [`Message::to`], et lui rendre un quatrième verdict mélangerait deux refus qui n'ont ni
//! la même cause ni la même réparation.
//!
//! # Le transfert d'état est un passage de témoin, pas une copie de contexte
//!
//! ADR 0019, condition 3. `docs/13` fixe pour la V1 : « nouvel attempt, nouvelle vue, nouveau
//! hash ». Un message qui transporterait un contexte de mission contournerait cette immuabilité
//! sans la nommer — la vue de contexte porte un hash obligatoire, et une copie qui voyage n'en a
//! plus l'usage.
//!
//! [`Handover`] porte donc ce que le nœud sortant **tenait** — combien de tentatives sont encore en
//! vol — et rien de ce qu'il **savait**. Il ne se construit que depuis un [`Outcome::Draining`] :
//! un `kill` abandonne, il ne passe pas la main, et un nœud posé n'a rien à transmettre.
//!
//! Le nom du type qu'il ne porte pas ne s'écrit pas ici, et ce n'est pas une pudeur : un test de
//! `proposal.rs` refuse ce nom dans tout le crate, commentaires compris. La périphrase est le prix
//! de n'avoir jamais à décider si une occurrence est un usage ou une explication.

use std::fmt;

use locus_protocol::{Id, id::Agent};

use crate::lifecycle::Outcome;
use crate::version::{Version, VersionId};

/// La suite des epochs qu'un destinataire a traversés, du plus ancien au courant.
///
/// Elle ne se fabrique pas : [`Epochs::advanced_to`] vérifie la filiation. C'est ce qui permet à
/// [`Reception::Late`] de dire quelque chose — sans la filiation, « antérieur » n'aurait pas de
/// sens entre deux hashes.
/// La racine est un champ à part, et non le premier élément d'un `Vec`.
///
/// Un vecteur peut être vide, donc `current()` aurait eu à choisir entre panique et `Option` — et
/// les deux auraient été fausses, puisqu'une suite d'epochs a **toujours** un epoch courant. Séparer
/// la racine rend l'invariant structurel : il n'y a pas d'état où le type existe sans courant, donc
/// rien à documenter ni à vérifier à l'exécution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Epochs {
    root: VersionId,
    since: Vec<VersionId>,
}

impl Epochs {
    /// Le destinataire ouvre sa suite sur cette version.
    #[must_use]
    pub fn rooted(at: &Version) -> Self {
        Self {
            root: at.id().clone(),
            since: Vec::new(),
        }
    }

    /// Avancer d'un epoch.
    ///
    /// # Errors
    ///
    /// [`EpochError::NotAChild`] si `next` ne descend pas directement de l'epoch courant. Accepter
    /// un saut ferait de la suite une collection d'epochs plutôt qu'une lignée, et « antérieur »
    /// redeviendrait indécidable — un epoch présent dans la suite ne prouverait plus qu'on l'a
    /// traversé.
    pub fn advanced_to(mut self, next: &Version) -> Result<Self, EpochError> {
        let current = self.current().clone();
        if next.parent() != Some(&current) {
            return Err(EpochError::NotAChild {
                current,
                parent: next.parent().cloned(),
            });
        }
        self.since.push(next.id().clone());
        Ok(self)
    }

    /// L'epoch courant. Il y en a toujours un — voir la note sur la racine.
    #[must_use]
    pub fn current(&self) -> &VersionId {
        self.since.last().unwrap_or(&self.root)
    }

    /// Combien d'epochs ont été traversés, racine comprise.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.since.len() + 1
    }

    /// Cet epoch a-t-il été traversé ?
    #[must_use]
    pub fn knows(&self, epoch: &VersionId) -> bool {
        &self.root == epoch || self.since.contains(epoch)
    }

    /// Le verdict de réception d'un message.
    ///
    /// Rien n'est appliqué ni jeté ici : la fonction **rapporte**. C'est la condition 2 de l'ADR
    /// 0019, et elle se lit dans la signature — un `Reception` rendu est un fait que l'appelant doit
    /// traiter, là où un `Option` l'aurait laissé l'ignorer d'un `?`.
    #[must_use]
    pub fn receive(&self, message: &Message) -> Reception {
        let current = self.current().clone();
        if message.epoch() == &current {
            return Reception::Delivered;
        }
        let sent_under = message.epoch().clone();
        if self.knows(&sent_under) {
            Reception::Late {
                sent_under,
                current,
            }
        } else {
            Reception::Unknown {
                sent_under,
                current,
            }
        }
    }
}

/// Pourquoi une suite d'epochs refuse un maillon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EpochError {
    /// La version proposée ne descend pas de l'epoch courant.
    NotAChild {
        /// L'epoch courant du destinataire.
        current: VersionId,
        /// Le parent que la version proposée déclare, s'il y en a un.
        parent: Option<VersionId>,
    },
}

impl fmt::Display for EpochError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAChild {
                current,
                parent: Some(parent),
            } => write!(
                formatter,
                "epoch « {parent} » n'est pas l'epoch courant « {current} » : une suite d'epochs est une lignée, pas une collection"
            ),
            Self::NotAChild {
                current,
                parent: None,
            } => write!(
                formatter,
                "une racine ne succède à rien, et l'epoch courant est « {current} »"
            ),
        }
    }
}

impl std::error::Error for EpochError {}

/// Ce qu'un agent adresse à un autre, sous l'epoch où il a agi.
///
/// Sans contenu : voir la documentation du module. La charge est celle de l'événement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    from: Id<Agent>,
    to: Id<Agent>,
    epoch: VersionId,
    subject: String,
}

impl Message {
    /// Composer un message sous l'epoch de l'émetteur.
    #[must_use]
    pub fn sent(
        from: Id<Agent>,
        to: Id<Agent>,
        under: &Version,
        subject: impl Into<String>,
    ) -> Self {
        Self {
            from,
            to,
            epoch: under.id().clone(),
            subject: subject.into(),
        }
    }

    /// L'émetteur.
    #[must_use]
    pub const fn from(&self) -> Id<Agent> {
        self.from
    }

    /// Le destinataire.
    #[must_use]
    pub const fn to(&self) -> Id<Agent> {
        self.to
    }

    /// L'epoch **sous lequel l'émetteur a agi** — jamais celui du destinataire.
    #[must_use]
    pub const fn epoch(&self) -> &VersionId {
        &self.epoch
    }

    /// Ce dont il est question.
    #[must_use]
    pub fn subject(&self) -> &str {
        &self.subject
    }
}

/// Ce qu'un destinataire conclut d'un message reçu.
///
/// Trois verdicts, jamais deux : voir la documentation du module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reception {
    /// L'émetteur a agi sous l'epoch courant du destinataire.
    Delivered,
    /// Sous un epoch traversé puis quitté. Le verdict nomme les deux.
    Late {
        /// Celui de l'émetteur.
        sent_under: VersionId,
        /// Celui du destinataire, maintenant.
        current: VersionId,
    },
    /// Sous un epoch que le destinataire n'a jamais traversé.
    ///
    /// Distinct de [`Reception::Late`] : ignorer et deviner sont deux fautes, et les fondre rendrait
    /// un verdict plausible là où il n'y a pas d'information.
    Unknown {
        /// Celui de l'émetteur.
        sent_under: VersionId,
        /// Celui du destinataire, maintenant.
        current: VersionId,
    },
}

impl fmt::Display for Reception {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Delivered => formatter.write_str("délivré sous l'epoch courant"),
            Self::Late {
                sent_under,
                current,
            } => write!(
                formatter,
                "message tardif : émis sous « {sent_under} », l'epoch courant est « {current} »"
            ),
            Self::Unknown {
                sent_under,
                current,
            } => write!(
                formatter,
                "epoch inconnu « {sent_under} » : jamais traversé depuis « {current} », rien n'en est déduit"
            ),
        }
    }
}

/// Le passage de témoin d'un nœud qui se draine.
///
/// Ce que le sortant **tenait**, jamais ce qu'il **savait** : voir la documentation du module et la
/// condition 3 de l'ADR 0019.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handover {
    from: Id<Agent>,
    to: Id<Agent>,
    in_flight: usize,
}

impl Handover {
    /// Le construire depuis le constat d'un drain.
    ///
    /// # Errors
    ///
    /// [`HandoverError::NotDraining`] si la commande n'était pas un drain. Un `kill` abandonne ce
    /// qu'il tenait et le **dit** — lui laisser passer la main ferait croire qu'un successeur
    /// reprend un travail que personne ne reprend. Un nœud posé, lui, n'a rien à transmettre.
    /// [`HandoverError::ToItself`] si le sortant est aussi l'entrant : un passage de témoin vers
    /// soi-même est un drain qui n'en est pas un, et le laisser passer produirait un successeur qui
    /// attend sa propre quiescence.
    pub fn after_drain(
        from: Id<Agent>,
        to: Id<Agent>,
        outcome: Outcome,
    ) -> Result<Self, HandoverError> {
        let Outcome::Draining { remaining } = outcome else {
            return Err(HandoverError::NotDraining);
        };
        if from == to {
            return Err(HandoverError::ToItself);
        }
        Ok(Self {
            from,
            to,
            in_flight: remaining,
        })
    }

    /// Le nœud sortant.
    #[must_use]
    pub const fn from(&self) -> Id<Agent> {
        self.from
    }

    /// Le nœud entrant.
    #[must_use]
    pub const fn to(&self) -> Id<Agent> {
        self.to
    }

    /// Combien de tentatives le sortant tient encore.
    #[must_use]
    pub const fn in_flight(&self) -> usize {
        self.in_flight
    }
}

/// Pourquoi un passage de témoin est refusé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandoverError {
    /// La commande n'était pas un drain.
    NotDraining,
    /// Le sortant et l'entrant sont le même nœud.
    ToItself,
}

impl fmt::Display for HandoverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotDraining => formatter.write_str(
                "seul un drain passe la main : un kill abandonne, et un nœud posé n'a rien à transmettre",
            ),
            Self::ToItself => {
                formatter.write_str("un nœud ne se passe pas le témoin à lui-même")
            }
        }
    }
}

impl std::error::Error for HandoverError {}
