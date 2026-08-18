//! La compaction — `docs/SPEC_V1.md` §16.5.
//!
//! # Les six exigences, et celle qui décide de la forme
//!
//! Une compaction « conserve les identifiants et pointeurs de preuve ; distingue faits, hypothèses,
//! décisions et questions ; **signale ce qui a été omis** ; possède une provenance et un watermark ;
//! peut être régénérée ; **ne transforme pas un objet non validé en connaissance établie** ».
//!
//! Les deux en gras sont celles qu'on perd en résumant, et elles se perdent de la même façon : en
//! silence. Un résumé qui ne dit pas ce qu'il a laissé se lit comme complet, et un résumé qui
//! promeut une hypothèse en fait se lit comme un fait.
//!
//! # Une compaction est une projection, toujours
//!
//! « Peut être régénérée » n'est pas une faculté optionnelle : c'est ce qui la range du côté des
//! projections de §16.1, et donc du côté de ce qu'une purge peut détruire sans rien coûter. Elle ne
//! peut pas se déclarer canonique, parce que [`Compaction::substance`] ne rend qu'une chose — une
//! compaction qui deviendrait la source serait la fin de l'invariant 2, et elle le deviendrait sans
//! qu'aucune ligne ne l'annonce.
//!
//! # Quatre sortes, parce que les confondre change ce qu'on croit savoir
//!
//! Un fait, une hypothèse, une décision et une question ne s'utilisent pas de la même façon en aval.
//! Un résumé qui les aplatit en « points » rend une liste où plus rien ne distingue ce qui est établi
//! de ce qui est demandé.

use std::fmt;

use locus_domain::{RevisionId, ValidationLevel};

use crate::level::Substance;

/// Ce qu'une entrée de compaction est — les quatre de §16.5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// Un fait.
    Fact,
    /// Une hypothèse.
    Hypothesis,
    /// Une décision.
    Decision,
    /// Une question.
    Question,
}

impl Kind {
    /// Les quatre, dans l'ordre de §16.5.
    pub const ALL: [Self; 4] = [Self::Fact, Self::Hypothesis, Self::Decision, Self::Question];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Fact => "fact",
            Self::Hypothesis => "hypothesis",
            Self::Decision => "decision",
            Self::Question => "question",
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Ce qu'une compaction retient d'un objet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    /// L'identifiant conservé — « conserve les identifiants et pointeurs de preuve ».
    pub revision: RevisionId,
    /// Ce que c'est.
    pub kind: Kind,
    /// Le niveau de validation de l'objet, tel qu'il était.
    ///
    /// Conservé **à côté** de la sorte, jamais fondu dedans : c'est ce qui permet de constater après
    /// coup qu'une compaction n'a rien promu.
    pub level: ValidationLevel,
}

/// Un résumé, avec ce qu'il a laissé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compaction {
    provenance: String,
    watermark: u64,
    kept: Vec<Kept>,
    omitted: Vec<RevisionId>,
}

impl Compaction {
    /// Composer une compaction.
    ///
    /// # Errors
    ///
    /// [`CompactionError::EmptyProvenance`] pour une compaction dont on ne sait pas d'où elle vient
    /// — elle ne se régénère alors pas, ce que §16.5 exige ;
    /// [`CompactionError::UnvalidatedPresentedAsFact`] pour un objet **non évalué** consigné comme
    /// fait, ce qui est exactement « transformer un objet non validé en connaissance établie ».
    ///
    /// # Où s'arrête ce refus
    ///
    /// Il porte sur le seul cas que §16.5 rend indiscutable : un objet que **personne n'a évalué**
    /// (`L0`) ne peut pas être un fait. Exiger davantage — une revue indépendante, une reproduction
    /// — serait fixer un seuil que la section ne fixe pas ; c'est une question de politique (§20),
    /// pas une question de mémoire, et l'inventer ici la mettrait hors de portée de la politique.
    pub fn of(
        provenance: &str,
        watermark: u64,
        kept: Vec<Kept>,
        omitted: Vec<RevisionId>,
    ) -> Result<Self, CompactionError> {
        if provenance.trim().is_empty() {
            return Err(CompactionError::EmptyProvenance);
        }
        for entry in &kept {
            if entry.kind == Kind::Fact && entry.level == ValidationLevel::Unassessed {
                return Err(CompactionError::UnvalidatedPresentedAsFact {
                    revision: entry.revision,
                });
            }
        }
        Ok(Self {
            provenance: provenance.to_owned(),
            watermark,
            kept,
            omitted,
        })
    }

    /// D'où elle vient.
    #[must_use]
    pub fn provenance(&self) -> &str {
        &self.provenance
    }

    /// L'instant du journal auquel elle est arrêtée.
    #[must_use]
    pub const fn watermark(&self) -> u64 {
        self.watermark
    }

    /// Ce qu'elle retient.
    #[must_use]
    pub fn kept(&self) -> &[Kept] {
        &self.kept
    }

    /// Ce qu'elle a **omis**, nommé.
    ///
    /// §16.5 l'exige, et c'est la moitié de ce qu'une compaction dit : un résumé qui ne signale pas
    /// ses omissions se lit comme complet, et personne ne va chercher ce qu'il ignore avoir perdu.
    #[must_use]
    pub fn omitted(&self) -> &[RevisionId] {
        &self.omitted
    }

    /// Ce qu'elle retient d'une sorte donnée.
    pub fn of_kind(&self, kind: Kind) -> impl Iterator<Item = &Kept> {
        self.kept.iter().filter(move |entry| entry.kind == kind)
    }

    /// **Toujours une projection.**
    ///
    /// « Peut être régénérée » la range du côté des projections de §16.1. Il n'existe aucun chemin
    /// par lequel une compaction se déclare canonique : elle deviendrait la source, et l'invariant 2
    /// tomberait sans qu'aucune ligne ne l'annonce.
    #[must_use]
    pub const fn substance(&self) -> Substance {
        Substance::Projection
    }
}

/// Ce qui empêche une compaction d'exister.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompactionError {
    /// Une compaction sans provenance.
    EmptyProvenance,
    /// Un objet non évalué consigné comme fait.
    UnvalidatedPresentedAsFact {
        /// Lequel.
        revision: RevisionId,
    },
}

impl fmt::Display for CompactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyProvenance => formatter.write_str(
                "une compaction sans provenance ne se régénère pas, et §16.5 l'exige régénérable",
            ),
            Self::UnvalidatedPresentedAsFact { revision } => write!(
                formatter,
                "« {revision} » n'a été évalué par personne et serait consigné comme fait : c'est \
                 exactement transformer un objet non validé en connaissance établie"
            ),
        }
    }
}

impl std::error::Error for CompactionError {}
