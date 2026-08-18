//! La métrique d'acceptation — `docs/10_V1_ROADMAP.md`, W18 : « le taux d'annulation humaine des
//! adaptations agentiques ».
//!
//! # Pourquoi cette métrique-là et pas une autre
//!
//! Un système qui s'adapte tout seul se juge mal de l'intérieur. Le nombre d'adaptations produites
//! mesure l'activité, pas l'utilité — et §13.6 range précisément « la production de tâches pour
//! maximiser l'activité » parmi les sept formes de gaming. Le taux d'annulation **humaine** est le
//! contraire : il ne peut monter que si quelqu'un a regardé et n'a pas voulu, et aucun agent ne peut
//! l'améliorer en travaillant davantage.
//!
//! # Le silence n'est pas un accord
//!
//! C'est toute la difficulté du calcul. `annulées / total` compte au dénominateur les adaptations
//! que **personne n'a regardées**, et les compte donc comme acceptées. Un déploiement que plus
//! personne ne surveille verrait alors son taux d'annulation tomber vers zéro, et lirait cette chute
//! comme une réussite au moment exact où il perd son seuil humain.
//!
//! Ici une adaptation non regardée est **hors mesure** : ni au numérateur, ni au dénominateur. Elle
//! est comptée à part, et [`CancellationRate::out_of_measure`] la rend visible — parce que la
//! proportion d'adaptations que personne ne regarde est elle-même ce qu'il faut savoir.
//!
//! # Trois choses qui ne sont pas une annulation humaine
//!
//! Une annulation **par le système** — une fenêtre expirée, un budget épuisé, un rollback
//! automatique — ne dit rien de ce qu'un humain aurait voulu ; la compter ferait monter le taux sans
//! qu'aucun jugement ait eu lieu. Une adaptation **d'auteur humain** n'est pas agentique, et la
//! mesurer ferait varier le score du système avec ce que ses opérateurs font eux-mêmes. Et une
//! adaptation **non regardée** est le cas ci-dessus.
//!
//! # Le taux garde son numérateur et son dénominateur
//!
//! `1/2` et `500/1000` valent le même nombre et ne sont pas la même preuve. [`Ratio`] porte les deux
//! entiers ; le flottant est calculé à la demande et n'est jamais stocké.

use std::collections::BTreeSet;
use std::fmt;

use locus_coordination::Author;

/// De quelle boucle l'adaptation vient — W18.b.
///
/// Les deux sont mesurées séparément parce qu'elles n'ont pas le même profil : une adaptation rapide
/// expire d'elle-même et se corrige en attendant, une adaptation lente entre dans l'histoire de
/// l'organisation. Les additionner ferait disparaître le second signal dans le premier, qui est bien
/// plus nombreux.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Loop {
    /// La boucle rapide : capacité, bornée par une fenêtre.
    Fast,
    /// La boucle lente : structure, par proposition.
    Slow,
}

impl Loop {
    /// Les deux.
    pub const ALL: [Self; 2] = [Self::Fast, Self::Slow];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Slow => "slow",
        }
    }
}

impl fmt::Display for Loop {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Ce qu'une adaptation est devenue.
///
/// Quatre issues, et une seule fait monter le taux. Les quatre sont distinctes parce qu'elles se
/// lisent différemment : la première est un désaccord, la deuxième un accord, la troisième une
/// absence de jugement, la quatrième un événement de machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fate {
    /// Un humain l'a annulée. **La seule qui compte au numérateur.**
    CancelledByHuman {
        /// Qui.
        by: String,
    },
    /// Un humain l'a vue et l'a laissée.
    ReviewedAndKept {
        /// Qui.
        by: String,
    },
    /// Personne ne l'a regardée. **Hors mesure**, pas acceptée.
    Unreviewed,
    /// Le système l'a annulée — fenêtre expirée, budget épuisé, rollback automatique.
    ///
    /// Hors mesure aussi : la machine n'a pas de préférence, et compter son annulation ferait monter
    /// le taux sans qu'aucun jugement ait eu lieu.
    CancelledBySystem {
        /// Pourquoi.
        reason: String,
    },
}

impl Fate {
    /// Les quatre noms.
    pub const NAMES: [&'static str; 4] = [
        "cancelled_by_human",
        "reviewed_and_kept",
        "unreviewed",
        "cancelled_by_system",
    ];

    /// Son nom.
    #[must_use]
    pub const fn slug(&self) -> &'static str {
        match self {
            Self::CancelledByHuman { .. } => "cancelled_by_human",
            Self::ReviewedAndKept { .. } => "reviewed_and_kept",
            Self::Unreviewed => "unreviewed",
            Self::CancelledBySystem { .. } => "cancelled_by_system",
        }
    }

    /// Vrai quand un humain s'est prononcé, dans un sens ou dans l'autre.
    #[must_use]
    pub const fn is_a_human_judgement(&self) -> bool {
        matches!(
            self,
            Self::CancelledByHuman { .. } | Self::ReviewedAndKept { .. }
        )
    }
}

impl fmt::Display for Fate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Ce qu'on retient d'une adaptation, une fois qu'elle a eu son sort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    author: Author,
    origin: Loop,
    fate: Fate,
}

impl Record {
    /// Consigner le sort d'une adaptation.
    #[must_use]
    pub const fn of(author: Author, origin: Loop, fate: Fate) -> Self {
        Self {
            author,
            origin,
            fate,
        }
    }

    /// Qui l'a écrite.
    #[must_use]
    pub const fn author(&self) -> &Author {
        &self.author
    }

    /// De quelle boucle elle vient.
    #[must_use]
    pub const fn origin(&self) -> Loop {
        self.origin
    }

    /// Ce qu'elle est devenue.
    #[must_use]
    pub const fn fate(&self) -> &Fate {
        &self.fate
    }

    /// Vrai quand elle a été écrite par un agent.
    ///
    /// Une adaptation d'auteur humain n'est pas agentique : la mesurer ferait varier le score du
    /// système avec ce que ses opérateurs font eux-mêmes.
    #[must_use]
    pub const fn is_agentic(&self) -> bool {
        matches!(self.author, Author::Agent(_))
    }
}

/// Un taux, avec ce dont il est fait.
///
/// `1/2` et `500/1000` valent le même nombre et ne sont pas la même preuve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ratio {
    cancelled: usize,
    measured: usize,
}

impl Ratio {
    /// Les annulations humaines.
    #[must_use]
    pub const fn cancelled(self) -> usize {
        self.cancelled
    }

    /// Les adaptations sur lesquelles un humain s'est prononcé.
    #[must_use]
    pub const fn measured(self) -> usize {
        self.measured
    }

    /// Le taux, entre 0 et 1.
    ///
    /// Calculé à la demande, jamais stocké : un flottant conservé se recopierait dans un rapport
    /// sans ses deux entiers, et on ne saurait plus sur combien il porte.
    #[must_use]
    #[expect(
        clippy::cast_precision_loss,
        reason = "un compte d'adaptations n'atteint pas 2^53"
    )]
    pub fn value(self) -> f64 {
        self.cancelled as f64 / self.measured as f64
    }
}

impl fmt::Display for Ratio {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}", self.cancelled, self.measured)
    }
}

/// Le taux d'annulation humaine des adaptations agentiques, et ce qu'il laisse dehors.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CancellationRate {
    cancelled: usize,
    kept: usize,
    unreviewed: usize,
    cancelled_by_system: usize,
    human_authored: usize,
}

impl CancellationRate {
    /// Mesurer sur un ensemble de consignations.
    #[must_use]
    pub fn over<'a>(records: impl IntoIterator<Item = &'a Record>) -> Self {
        let mut rate = Self::default();
        for record in records {
            if !record.is_agentic() {
                rate.human_authored += 1;
                continue;
            }
            match record.fate() {
                Fate::CancelledByHuman { .. } => rate.cancelled += 1,
                Fate::ReviewedAndKept { .. } => rate.kept += 1,
                Fate::Unreviewed => rate.unreviewed += 1,
                Fate::CancelledBySystem { .. } => rate.cancelled_by_system += 1,
            }
        }
        rate
    }

    /// Mesurer sur une seule boucle.
    #[must_use]
    pub fn over_loop<'a>(records: impl IntoIterator<Item = &'a Record>, origin: Loop) -> Self {
        Self::over(
            records
                .into_iter()
                .filter(|record| record.origin() == origin),
        )
    }

    /// Le taux, **quand il en existe un**.
    ///
    /// `None` quand personne ne s'est prononcé. Rendre `0.0` se lirait « aucun humain n'a jamais
    /// annulé », donc une acceptation parfaite — tirée de zéro observation, et au moment précis où
    /// le déploiement a perdu son seuil humain. « Pas vérifié » n'est jamais « réussi ».
    #[must_use]
    pub const fn ratio(&self) -> Option<Ratio> {
        let measured = self.measured();
        if measured == 0 {
            return None;
        }
        Some(Ratio {
            cancelled: self.cancelled,
            measured,
        })
    }

    /// Le nombre d'adaptations agentiques sur lesquelles un humain s'est prononcé.
    #[must_use]
    pub const fn measured(&self) -> usize {
        self.cancelled + self.kept
    }

    /// Le nombre d'adaptations agentiques **hors mesure** : non regardées, ou annulées par le
    /// système.
    ///
    /// Ce nombre n'est pas un déchet de calcul. La proportion d'adaptations que personne ne regarde
    /// est elle-même ce qu'il faut savoir, et la taire ferait d'un taux fondé sur trois observations
    /// la même chose qu'un taux fondé sur trois mille.
    #[must_use]
    pub const fn out_of_measure(&self) -> usize {
        self.unreviewed + self.cancelled_by_system
    }

    /// Les adaptations que personne n'a regardées.
    #[must_use]
    pub const fn unreviewed(&self) -> usize {
        self.unreviewed
    }

    /// Les adaptations que le système a annulées.
    #[must_use]
    pub const fn cancelled_by_system(&self) -> usize {
        self.cancelled_by_system
    }

    /// Les adaptations d'auteur humain, écartées de la mesure.
    #[must_use]
    pub const fn human_authored(&self) -> usize {
        self.human_authored
    }
}

/// Les humains qui se sont prononcés sur ces adaptations.
///
/// §14.6 : la réputation « ne doit pas devenir un score social unique ». Cette liste sert à savoir
/// **combien de personnes** portent la mesure — un taux sur cent adaptations toutes jugées par la
/// même personne n'est pas cent observations —, pas à noter qui que ce soit.
#[must_use]
pub fn reviewers<'a>(records: impl IntoIterator<Item = &'a Record>) -> BTreeSet<String> {
    records
        .into_iter()
        .filter_map(|record| match record.fate() {
            Fate::CancelledByHuman { by } | Fate::ReviewedAndKept { by } => Some(by.clone()),
            Fate::Unreviewed | Fate::CancelledBySystem { .. } => None,
        })
        .collect()
}
