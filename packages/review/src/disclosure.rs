//! Le dévoilement d'une trace de raisonnement — `W26.c`, ADR 0027 décision 3.
//!
//! # Les quatre, exigés par le type
//!
//! Un dévoilement n'est pas constructible sans **un motif**, **une portée**, **une échéance** et
//! **sa journalisation**. Il n'y a pas de constructeur qui en laisse un de côté, et pas de valeur
//! par défaut pour un quelconque des quatre : un défaut serait le choix que personne n'a fait, et
//! c'est précisément ce qu'un dévoilement ne peut pas être.
//!
//! # Pourquoi le motif vit ici, et le port dans `packages/memory`
//!
//! Ce qui compte les tours de contestation est la **revue**. `memory::read` n'a donc pas à connaître
//! les motifs : il lui suffit de savoir si un dévoilement couvre cette trace, ce lecteur, cet
//! instant — c'est le port [`locus_memory::Disclosed`], et [`Disclosure`] en est le seul
//! implémenteur du workspace, ce qu'un test vérifie en parcourant les sources.
//!
//! L'inverse — faire connaître les motifs à la mémoire — aurait inversé la dépendance, `review`
//! dépendant déjà de `memory`.
//!
//! # L'énumération des motifs commence vide, et reçoit ici son premier
//!
//! C'est la règle du dépôt : « une sorte de relation n'entre dans son énumération que lorsqu'un
//! consommateur exécutable et testé existe. » [`Reason`] a donc **un** barreau aujourd'hui —
//! l'objection non résolue après un nombre borné de tours de contestation — et il arrive **avec le
//! mécanisme qui le déclenche**, [`Contestation`], jamais sans.
//!
//! Ce que cela veut dire dans le type : [`Motive`] n'a **aucun** constructeur public. Un motif ne
//! s'écrit pas, il se **constate**, et le seul chemin est [`Contestation::unresolved_after`], qui
//! compte de vrais tours. Un test confronte le nombre de barreaux de `Reason` au nombre de chemins
//! qui en produisent un : un motif sans mécanisme ferait rougir le décompte.
//!
//! # « Toutes les traces de cette branche » n'est pas une portée
//!
//! ADR 0027 décision 3 : « un dévoilement ne s'accorde jamais par défaut, et jamais globalement.
//! *Toutes les traces de cette branche* n'est pas une portée : c'est une politique de diffusion
//! déguisée en autorisation ponctuelle. »
//!
//! [`Scope`] nomme donc **une** trace et **un** lecteur. Ce n'est pas un filtre qu'on aurait
//! restreint : il n'y a pas de forme plus large à écrire. Un test d'absence refuse le vocabulaire
//! qui en ouvrirait une.
//!
//! # Un dévoilement expiré ne donne plus rien
//!
//! L'échéance n'est pas un indicateur qu'on consulte : elle est dans [`Disclosure::covers`], qui est
//! le seul chemin par lequel `memory::read` interroge un dévoilement. Passée l'heure, la réponse est
//! `false`, et le pair retombe sur le refus ordinaire.

use std::fmt;

use locus_domain::RevisionId;
use locus_memory::Disclosed;
use locus_protocol::{Id, Timestamp, id::Agent};

use crate::rebuttal::Rebuttal;
use crate::review::Review;

/// Les motifs de dévoilement — **l'énumération close**, un seul barreau aujourd'hui.
///
/// Un barreau de plus est une décision, pas une commodité : il arrive avec le mécanisme qui le
/// déclenche, et le test de décompte le refuse tant que ce mécanisme n'existe pas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reason {
    /// Une objection restée non résolue après un nombre borné de tours de contestation.
    ///
    /// C'est le conflit prolongé que la question posée à l'ADR 0027 nommait : deux agents qui ne se
    /// départagent pas, et dont l'un pourrait comprendre l'autre en voyant son raisonnement.
    UnresolvedObjection,
}

impl Reason {
    /// Les motifs existants. **Un**, et le jour où il y en aura deux, le second aura son mécanisme.
    pub const ALL: [Self; 1] = [Self::UnresolvedObjection];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::UnresolvedObjection => "unresolved_objection",
        }
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Un motif **constaté** — jamais écrit.
///
/// Aucun constructeur public. Le seul chemin est [`Contestation::unresolved_after`], et c'est ce qui
/// rend vraie la phrase « chaque motif arrive avec le mécanisme qui le déclenche ». Un `Motive::new`
/// aurait permis d'affirmer un conflit prolongé sans qu'aucun tour n'ait eu lieu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Motive {
    reason: Reason,
    finding: RevisionId,
    rounds: u32,
}

impl Motive {
    /// Le motif, dans l'énumération close.
    #[must_use]
    pub const fn reason(&self) -> Reason {
        self.reason
    }

    /// Le constat sur lequel la contestation s'est enlisée.
    #[must_use]
    pub const fn finding(&self) -> &RevisionId {
        &self.finding
    }

    /// Combien de tours ont eu lieu.
    #[must_use]
    pub const fn rounds(&self) -> u32 {
        self.rounds
    }
}

/// La suite des tours de contestation sur un même constat — **le mécanisme**.
///
/// # Ce qu'est un tour, et ce qui n'en est pas un
///
/// Un tour est un [`Rebuttal`] qui **conteste** et **demande un recheck**. Une réponse qui accepte
/// n'est pas une contestation ; une contestation qui ne demande pas de recheck ne relance rien, donc
/// n'ouvre pas de tour. Compter toutes les réponses aurait fait du dialogue ordinaire un conflit
/// prolongé, et un dévoilement se serait déclenché sur une revue qui se passait bien.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contestation {
    finding: RevisionId,
    rounds: u32,
}

impl Contestation {
    /// Ouvrir la suite des tours sur un constat. **Zéro tour**, et zéro est un fait.
    #[must_use]
    pub const fn on(finding: RevisionId) -> Self {
        Self { finding, rounds: 0 }
    }

    /// Verser une réponse. Elle ne compte comme tour que si elle conteste **et** relance.
    ///
    /// Rendue par valeur : une contestation se construit, elle ne se modifie pas sous les pieds de
    /// qui la lit.
    #[must_use]
    pub fn then(mut self, rebuttal: &Rebuttal) -> Self {
        if !rebuttal.contested().is_empty() && rebuttal.requests_recheck() {
            self.rounds += 1;
        }
        self
    }

    /// Combien de tours ont eu lieu.
    #[must_use]
    pub const fn rounds(&self) -> u32 {
        self.rounds
    }

    /// Le motif, **si** la borne est dépassée.
    ///
    /// # Strictement au-delà, et pas « à partir de »
    ///
    /// `bound` est le nombre de tours qu'on accepte **sans** dévoiler. À `bound` tours exactement, la
    /// contestation est encore dans ce qui était prévu ; c'est le tour suivant qui la fait sortir. Un
    /// `>=` aurait dévoilé au dernier tour admis, c'est-à-dire un tour trop tôt, et la borne aurait
    /// dit autre chose que ce que son nom annonce.
    #[must_use]
    pub fn unresolved_after(&self, bound: u32) -> Option<Motive> {
        (self.rounds > bound).then_some(Motive {
            reason: Reason::UnresolvedObjection,
            finding: self.finding,
            rounds: self.rounds,
        })
    }
}

/// Quelle trace, vers quel lecteur — **et rien de plus large**.
///
/// Il n'existe pas de portée qui vise une branche, un ensemble ou un motif générique. Ce n'est pas
/// un filtre restreint : c'est la seule forme qui s'écrive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    artifact_id: String,
    reader: Id<Agent>,
}

impl Scope {
    /// Viser une trace, pour un lecteur.
    #[must_use]
    pub fn one(artifact_id: &str, reader: Id<Agent>) -> Self {
        Self {
            artifact_id: artifact_id.to_owned(),
            reader,
        }
    }

    /// La trace visée.
    #[must_use]
    pub fn artifact_id(&self) -> &str {
        &self.artifact_id
    }

    /// Le lecteur visé.
    #[must_use]
    pub const fn reader(&self) -> &Id<Agent> {
        &self.reader
    }
}

/// Le fait qu'un dévoilement écrit à sa construction.
///
/// ADR 0027 décision 3 point 4 : « le dévoilement est un fait, comme la lecture ». Il est rendu
/// **avec** le dévoilement, dans le même couple, pour la raison que `W26.b` a écrite pour la lecture
/// institutionnelle — un fait rendu à côté se laisse ignorer d'un `?`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use = "un dévoilement est un fait ; ceci est ce qu'il y a à écrire au journal"]
pub struct DisclosureGranted {
    artifact_id: String,
    reader: Id<Agent>,
    reason: Reason,
    granted_at: Timestamp,
    until: Timestamp,
}

impl DisclosureGranted {
    /// Quelle trace.
    #[must_use]
    pub fn artifact_id(&self) -> &str {
        &self.artifact_id
    }

    /// Vers qui.
    #[must_use]
    pub const fn reader(&self) -> &Id<Agent> {
        &self.reader
    }

    /// Pourquoi.
    #[must_use]
    pub const fn reason(&self) -> Reason {
        self.reason
    }

    /// Quand il a été accordé.
    #[must_use]
    pub const fn granted_at(&self) -> Timestamp {
        self.granted_at
    }

    /// Jusqu'à quand il vaut.
    #[must_use]
    pub const fn until(&self) -> Timestamp {
        self.until
    }
}

/// L'état de revue du lecteur visé — `W26.d`, ADR 0027 décision 5.
///
/// # Ce que l'énumération ne porte pas, et c'est là qu'est la garantie
///
/// **Il n'y a pas de variante « revue ouverte ».** Ce n'est pas un oubli : une revue ouverte est
/// exactement l'**absence** d'une [`Review`] rendue, et `Standing::recorded` en exige une. Qui n'a
/// pas le verdict n'a pas la valeur, donc n'a pas de `Standing`, donc n'obtient pas de dévoilement.
///
/// L'invariant 11 est ainsi une borne sur le **mécanisme**, et non un défaut qu'un motif
/// surclasserait : il n'existe aucune signature qui prenne une revue ouverte et rende un
/// dévoilement, et un test le tient par l'absence.
///
/// # Ce que `OutsideReview` est, dit franchement
///
/// Une **affirmation de l'appelant**, pas une preuve : ce crate ne tient pas le registre de qui
/// relit quoi, et prétendre le vérifier ici serait faux. Ce qui la rend supportable est que la
/// garde de contamination ne s'y fie pas — `contamination::inspect` reteste l'aveuglement de son
/// côté et **retombe sur la fuite** en l'absence d'un dévoilement valide attaché. Les deux
/// mécanismes ne se font pas confiance l'un l'autre, et c'est le seul agencement qui tienne.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Standing {
    /// Le lecteur ne relit pas le dossier en cause.
    OutsideReview,
    /// Son verdict est **enregistré**, et la revue rendue en est la preuve.
    Recorded(Id<Agent>),
}

impl Standing {
    /// Constater qu'un verdict est enregistré.
    ///
    /// Posséder une [`Review`] **est** la preuve : elle ne se rend qu'une fois, et une revue en
    /// cours n'a pas d'existence sous cette forme. Une fois le verdict rendu, la revue est un fait
    /// figé que rien ne peut contaminer rétroactivement — c'est la phrase exacte de l'ADR.
    #[must_use]
    pub const fn recorded(review: &Review) -> Self {
        Self::Recorded(review.reviewer())
    }
}

/// Un dévoilement : motif, portée, échéance — et le fait qu'il écrit.
///
/// # Les quatre à la fois, ou rien
///
/// [`Disclosure::granting`] exige les trois premiers et rend le quatrième. Il n'y a pas de
/// `Disclosure::new` qui prendrait moins, pas de `with_deadline` qui l'ajouterait après coup : un
/// dévoilement sans échéance aurait existé, ne serait-ce qu'un instant, et c'est un instant de trop
/// pour une valeur qui peut être clonée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disclosure {
    motive: Motive,
    scope: Scope,
    until: Timestamp,
}

impl Disclosure {
    /// Accorder un dévoilement, et **écrire le fait**.
    ///
    /// # Errors
    ///
    /// [`DisclosureError::DeadlineNotAfterGrant`] quand l'échéance ne suit pas l'instant d'octroi.
    /// Un dévoilement déjà expiré à sa naissance ne serait pas une autorisation prudente : ce serait
    /// une ligne de journal qui dit qu'on a autorisé, sans que rien ne l'ait jamais été.
    pub fn granting(
        motive: Motive,
        scope: Scope,
        standing: &Standing,
        granted_at: Timestamp,
        until: Timestamp,
    ) -> Result<(Self, DisclosureGranted), DisclosureError> {
        if until <= granted_at {
            return Err(DisclosureError::DeadlineNotAfterGrant { granted_at, until });
        }
        if let Standing::Recorded(reviewer) = standing
            && *reviewer != scope.reader
        {
            return Err(DisclosureError::SettledSomeoneElse {
                settled: *reviewer,
                reader: scope.reader,
            });
        }
        let fact = DisclosureGranted {
            artifact_id: scope.artifact_id.clone(),
            reader: scope.reader,
            reason: motive.reason,
            granted_at,
            until,
        };
        Ok((
            Self {
                motive,
                scope,
                until,
            },
            fact,
        ))
    }

    /// Le motif.
    #[must_use]
    pub const fn motive(&self) -> &Motive {
        &self.motive
    }

    /// La portée.
    #[must_use]
    pub const fn scope(&self) -> &Scope {
        &self.scope
    }

    /// L'échéance.
    #[must_use]
    pub const fn until(&self) -> Timestamp {
        self.until
    }
}

impl Disclosed for Disclosure {
    /// Les trois questions ensemble, et l'échéance en fait partie.
    ///
    /// Un dévoilement expiré ne donne plus rien : ce n'est pas un état qu'on consulte à côté, c'est
    /// la même réponse que « ce n'est pas la bonne trace ». `memory::read` n'a donc aucun cas
    /// particulier à écrire pour l'expiration, et n'aurait aucun moyen de l'oublier.
    fn covers(&self, artifact_id: &str, reader: &str, at: Timestamp) -> bool {
        self.scope.artifact_id == artifact_id
            && self.scope.reader.to_string() == reader
            && at <= self.until
    }
}

/// Pourquoi un dévoilement ne s'accorde pas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisclosureError {
    /// L'échéance ne suit pas l'octroi.
    DeadlineNotAfterGrant {
        /// L'instant d'octroi.
        granted_at: Timestamp,
        /// L'échéance demandée.
        until: Timestamp,
    },
    /// Le verdict présenté est celui d'un **autre** relecteur que le lecteur visé.
    ///
    /// Sans ce refus, la revue close de l'un blanchirait la revue ouverte de l'autre : il aurait
    /// suffi de présenter n'importe quel verdict enregistré pour dévoiler vers n'importe qui.
    SettledSomeoneElse {
        /// Le relecteur dont le verdict est enregistré.
        settled: Id<Agent>,
        /// Le lecteur que la portée vise.
        reader: Id<Agent>,
    },
}

impl fmt::Display for DisclosureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeadlineNotAfterGrant { granted_at, until } => write!(
                formatter,
                "échéance « {until} » au plus tard que l'octroi « {granted_at} » : le dévoilement n'aurait jamais rien autorisé"
            ),
            Self::SettledSomeoneElse { settled, reader } => write!(
                formatter,
                "le verdict enregistré est celui de « {settled} », et la portée vise « {reader} » : la revue close de l'un ne referme pas celle de l'autre"
            ),
        }
    }
}

impl std::error::Error for DisclosureError {}

/// Les deux verdicts d'un dévoilement — `W26.d`, ADR 0027 décision 5.
///
/// # Pourquoi deux, et pourquoi le premier reste
///
/// Dévoiler pendant la revue casse l'invariant 11 ; ne jamais dévoiler laisse le conflit sans autre
/// issue que l'autorité. Séparer les deux verdicts fait payer le dévoilement en **traçabilité**
/// plutôt qu'en crédibilité.
///
/// Le premier reste **lisible** : l'invariant 12 interdit de faire disparaître un résultat gênant, et
/// un verdict rendu aveugle puis révisé après lecture du raisonnement adverse en est exactement un.
/// **L'écart entre les deux est l'information que le conflit prolongé cherchait** — l'effacer
/// reviendrait à jeter la réponse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reconsidered {
    blind: Review,
    informed: Review,
    disclosure: Disclosure,
}

impl Reconsidered {
    /// Rendre un second verdict, après un dévoilement.
    ///
    /// # Errors
    ///
    /// [`DisclosureError::SettledSomeoneElse`] quand le dévoilement ne vise pas le relecteur du
    /// premier verdict : un second verdict ne se rend qu'au nom de qui a rendu le premier, sans quoi
    /// ce serait une revue de plus et non une reconsidération.
    pub fn after(
        blind: Review,
        informed: Review,
        disclosure: Disclosure,
    ) -> Result<Self, DisclosureError> {
        if blind.reviewer() != *disclosure.scope().reader() {
            return Err(DisclosureError::SettledSomeoneElse {
                settled: blind.reviewer(),
                reader: *disclosure.scope().reader(),
            });
        }
        if informed.reviewer() != blind.reviewer() {
            return Err(DisclosureError::SettledSomeoneElse {
                settled: blind.reviewer(),
                reader: informed.reviewer(),
            });
        }
        Ok(Self {
            blind,
            informed,
            disclosure,
        })
    }

    /// Le **premier** verdict, rendu aveugle. Il ne disparaît pas.
    #[must_use]
    pub const fn blind(&self) -> &Review {
        &self.blind
    }

    /// Le **second**, rendu après lecture.
    #[must_use]
    pub const fn informed(&self) -> &Review {
        &self.informed
    }

    /// Le dévoilement que le second verdict porte dans sa provenance.
    ///
    /// C'est ce qui distingue une reconsidération d'un changement d'avis : un lecteur du dossier
    /// peut nommer **ce qui** a été montré, et à quel titre.
    #[must_use]
    pub const fn disclosure(&self) -> &Disclosure {
        &self.disclosure
    }
}
