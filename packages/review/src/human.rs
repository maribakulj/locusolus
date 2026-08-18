//! La revue humaine depuis un viewer — `xiiif/SPEC_V1.md` §20.
//!
//! # La phrase qui contraint tout ce fichier
//!
//! « Cette revue n'est pas une validation scientifique complète. Elle produit un finding humain
//! attachable à un `ReviewDossier`. »
//!
//! Deux exigences, donc, et elles tirent en sens contraire. Le finding doit être **réel** — il
//! s'attache au dossier, il se compte, il ne se perd pas (invariant 12). Et il ne doit **jamais**
//! pouvoir tenir lieu de validation. La façon la plus courte de tenir les deux est de fermer une
//! seule porte : aucun verdict humain ne rend [`Verdict::Supports`].
//!
//! C'est la traduction exacte de §20 dans le vocabulaire de §17.5. Un relecteur humain qui accepte
//! ce qu'il voit dit qu'il n'a pas d'objection — ce qui n'est pas la même chose que dire que la
//! revendication tient. Les confondre ferait d'un coup d'œil dans une visionneuse une preuve, et
//! c'est précisément ce que la phrase interdit.
//!
//! # `source-changed` ne réfute rien
//!
//! Le quatrième verdict est celui de §19, vu par un humain : la ressource distante n'est plus celle
//! qui a été lue. C'est un constat sur la **source**, et le rendre `refutes` ferait douter d'un run
//! correct chaque fois qu'une bibliothèque remanie son site — la confusion que §19 nomme et
//! interdit. Il rend donc [`Verdict::NotApplicable`] : le relecteur répond à une autre question que
//! celle du dossier, et il faut que cela se voie.
//!
//! # Ce que ce module ne fait pas
//!
//! Il ne construit aucune [`crate::Review`]. Une revue au sens de §17 porte une attestation
//! d'indépendance calculée entre deux agents ; un humain devant une visionneuse n'entre pas dans ce
//! calcul, et lui fabriquer une attestation serait déclarer ce que W7.a s'applique à constater.

use std::fmt;

use locus_domain::RevisionId;
use locus_lep::HumanReviewFinding as Wire;

use crate::dossier::Frozen;
use crate::review::{Finding, Severity, Verdict};

/// Ce qu'un humain peut enregistrer depuis un viewer — les quatre de §20.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HumanVerdict {
    /// Rien à redire.
    Accept,
    /// La production demande une correction.
    NeedsCorrection,
    /// Ce n'est pas la région, ou pas la ressource, que la revendication désigne.
    WrongTarget,
    /// La ressource distante a changé depuis le run.
    SourceChanged,
}

impl HumanVerdict {
    /// Les quatre de §20, sous leur nom.
    pub const ALL: [Self; 4] = [
        Self::Accept,
        Self::NeedsCorrection,
        Self::WrongTarget,
        Self::SourceChanged,
    ];

    /// Son nom, tel qu'il part sur le fil.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Accept => "accept",
            Self::NeedsCorrection => "needs-correction",
            Self::WrongTarget => "wrong-target",
            Self::SourceChanged => "source-changed",
        }
    }

    /// Le relire depuis le fil.
    #[must_use]
    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.slug() == slug)
    }

    /// Ce que ce verdict conclut, dans le vocabulaire de §17.5.
    ///
    /// **Aucune branche ne rend [`Verdict::Supports`]**, et c'est §20 qui l'exige : « cette revue
    /// n'est pas une validation scientifique complète ». `accept` rend donc
    /// [`Verdict::Insufficient`] — le relecteur n'a pas d'objection, ce qui n'est pas une preuve —
    /// et `source-changed` rend [`Verdict::NotApplicable`], parce qu'il parle de la source et non
    /// de la revendication.
    #[must_use]
    pub const fn verdict(self) -> Verdict {
        match self {
            Self::Accept => Verdict::Insufficient,
            Self::NeedsCorrection | Self::WrongTarget => Verdict::Refutes,
            Self::SourceChanged => Verdict::NotApplicable,
        }
    }

    /// La gravité que ce verdict porte.
    ///
    /// `wrong-target` est bloquant parce qu'il ne conteste pas la conclusion : il dit que rien de
    /// ce qui suit ne porte sur le bon objet.
    #[must_use]
    pub const fn severity(self) -> Severity {
        match self {
            Self::Accept | Self::SourceChanged => Severity::Info,
            Self::NeedsCorrection => Severity::Major,
            Self::WrongTarget => Severity::Blocking,
        }
    }
}

impl fmt::Display for HumanVerdict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Ce qu'un humain a enregistré depuis un viewer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanReview {
    dossier_id: String,
    target: RevisionId,
    reviewer: String,
    verdict: Option<HumanVerdict>,
    comment: Option<String>,
    evidence: Vec<RevisionId>,
}

impl HumanReview {
    /// Enregistrer une revue humaine.
    ///
    /// # Errors
    ///
    /// [`HumanReviewError::EmptyField`] pour un dossier ou un relecteur vide, et
    /// [`HumanReviewError::SaysNothing`] quand il n'y a ni verdict ni commentaire : §20 offre cinq
    /// façons de s'exprimer et aucune n'est le silence. Un dossier peuplé de findings vides se
    /// compterait ensuite comme un dossier relu.
    pub fn record(
        dossier_id: &str,
        target: RevisionId,
        reviewer: &str,
        verdict: Option<HumanVerdict>,
        comment: Option<&str>,
    ) -> Result<Self, HumanReviewError> {
        if dossier_id.trim().is_empty() {
            return Err(HumanReviewError::EmptyField {
                field: "dossier_id",
            });
        }
        if reviewer.trim().is_empty() {
            return Err(HumanReviewError::EmptyField { field: "reviewer" });
        }
        let comment = comment.map(str::trim).filter(|text| !text.is_empty());
        if verdict.is_none() && comment.is_none() {
            return Err(HumanReviewError::SaysNothing);
        }
        Ok(Self {
            dossier_id: dossier_id.to_owned(),
            target,
            reviewer: reviewer.to_owned(),
            verdict,
            comment: comment.map(ToOwned::to_owned),
            evidence: Vec::new(),
        })
    }

    /// Adosser la revue à des révisions concrètes.
    ///
    /// §17.5 vaut ici comme ailleurs : sans preuve, le finding est un commentaire non bloquant. La
    /// règle porte sur la preuve, pas sur la qualité du relecteur — un humain qui montre ce sur
    /// quoi il s'appuie est opposable, un agent qui ne montre rien ne l'est pas.
    #[must_use]
    pub fn citing(mut self, evidence: Vec<RevisionId>) -> Self {
        self.evidence = evidence;
        self
    }

    /// Le dossier visé.
    #[must_use]
    pub fn dossier_id(&self) -> &str {
        &self.dossier_id
    }

    /// La révision revue.
    #[must_use]
    pub const fn target(&self) -> RevisionId {
        self.target
    }

    /// Qui a regardé.
    #[must_use]
    pub fn reviewer(&self) -> &str {
        &self.reviewer
    }

    /// Le verdict, s'il y en a un.
    #[must_use]
    pub const fn verdict(&self) -> Option<HumanVerdict> {
        self.verdict
    }

    /// Le commentaire libre, s'il y en a un.
    #[must_use]
    pub fn comment(&self) -> Option<&str> {
        self.comment.as_deref()
    }

    /// Le type de problème que ce finding portera.
    #[must_use]
    pub fn issue_type(&self) -> String {
        match self.verdict {
            Some(verdict) => format!("human-review:{verdict}"),
            None => "human-review:comment".to_owned(),
        }
    }

    /// Attacher la revue au dossier `dossier`, sous la forme d'un [`Finding`].
    ///
    /// # Errors
    ///
    /// [`HumanReviewError::WrongDossier`] quand la revue ne vise pas ce dossier, et
    /// [`HumanReviewError::TargetNotInDossier`] quand la révision revue n'est pas une de ses
    /// cibles. Sans ce second refus, une revue humaine élargirait le dossier en silence : le
    /// dossier est figé avant attribution (§17.3), et un finding qui porte sur autre chose que ce
    /// qu'il couvre défait cette garantie sans jamais la contredire ouvertement.
    pub fn attach_to(&self, dossier: &Frozen) -> Result<Finding, HumanReviewError> {
        if dossier.id() != self.dossier_id {
            return Err(HumanReviewError::WrongDossier {
                expected: self.dossier_id.clone(),
                found: dossier.id().to_owned(),
            });
        }
        if !dossier.targets().contains(&self.target) {
            return Err(HumanReviewError::TargetNotInDossier);
        }
        Finding::new(
            self.target,
            &self.issue_type(),
            self.severity(),
            self.verdict_as_finding(),
            self.evidence.clone(),
        )
        .map_err(|_| HumanReviewError::SaysNothing)
    }

    /// Ce que ce finding conclut, dans le vocabulaire de §17.5.
    ///
    /// Un commentaire sans verdict rend [`Verdict::Insufficient`] : quelqu'un a écrit quelque
    /// chose, et rien n'en découle tant que personne ne l'a instruit.
    #[must_use]
    pub fn verdict_as_finding(&self) -> Verdict {
        self.verdict
            .map_or(Verdict::Insufficient, HumanVerdict::verdict)
    }

    /// La gravité de ce finding.
    #[must_use]
    pub fn severity(&self) -> Severity {
        self.verdict.map_or(Severity::Info, HumanVerdict::severity)
    }
}

impl HumanReview {
    /// Relire un document — le lecteur validant de W6.b, appliqué ici.
    ///
    /// # Ce que le type engendré ne peut pas dire
    ///
    /// Le schéma porte `anyOf: [{required: verdict}, {required: comment}]` et une énumération de
    /// quatre valeurs sur `verdict`. Rust n'exprime ni l'un ni l'autre : le type engendré offre
    /// deux `Option<String>` indépendants, donc un document muet le traverse sans bruit, et
    /// `verdict: "validated"` aussi. L'exclusivité ne serait alors tenue que par le validateur
    /// JSON — c'est-à-dire nulle part, dès qu'un producteur construit la valeur en mémoire.
    ///
    /// Un verdict inventé est particulièrement à surveiller ici : c'est **précisément** le mot que
    /// §20 interdit, et le laisser passer comme un commentaire libre le ferait entrer dans le
    /// dossier sous un nom que personne n'a défini.
    ///
    /// # Errors
    ///
    /// [`HumanReviewError::UnknownVerdict`] pour un verdict hors des quatre de §20,
    /// [`HumanReviewError::MalformedId`] pour une révision illisible, et ce que
    /// [`HumanReview::record`] refuse.
    pub fn from_wire(wire: &Wire) -> Result<Self, HumanReviewError> {
        let verdict = match &wire.verdict {
            None => None,
            Some(slug) => Some(HumanVerdict::from_slug(slug).ok_or_else(|| {
                HumanReviewError::UnknownVerdict {
                    value: slug.clone(),
                }
            })?),
        };
        let target = parse_revision(&wire.target)?;
        let evidence = wire
            .evidence
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|text| parse_revision(text))
            .collect::<Result<Vec<_>, _>>()?;
        let review = Self::record(
            &wire.dossier_id,
            target,
            &wire.reviewer,
            verdict,
            wire.comment.as_deref(),
        )?;
        Ok(review.citing(evidence))
    }
}

fn parse_revision(text: &str) -> Result<RevisionId, HumanReviewError> {
    RevisionId::parse(text).map_err(|_| HumanReviewError::MalformedId {
        value: text.to_owned(),
    })
}

/// Ce qui empêche une revue humaine d'exister ou de s'attacher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HumanReviewError {
    /// Un champ obligatoire vide.
    EmptyField {
        /// Lequel.
        field: &'static str,
    },
    /// Ni verdict ni commentaire.
    SaysNothing,
    /// La revue vise un autre dossier.
    WrongDossier {
        /// Celui que la revue nomme.
        expected: String,
        /// Celui auquel on tente de l'attacher.
        found: String,
    },
    /// La révision revue n'est pas couverte par le dossier.
    TargetNotInDossier,
    /// Un verdict hors des quatre de §20.
    UnknownVerdict {
        /// La valeur reçue.
        value: String,
    },
    /// Une révision illisible.
    MalformedId {
        /// La valeur reçue.
        value: String,
    },
}

impl fmt::Display for HumanReviewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(formatter, "« {field} » est vide"),
            Self::SaysNothing => formatter.write_str(
                "ni verdict ni commentaire : §20 offre cinq façons de s'exprimer, et aucune \
                 n'est le silence",
            ),
            Self::WrongDossier { expected, found } => {
                write!(formatter, "la revue vise « {expected} », pas « {found} »")
            }
            Self::TargetNotInDossier => formatter.write_str(
                "la révision revue n'est pas une cible du dossier : l'y attacher élargirait un \
                 dossier figé avant attribution (§17.3)",
            ),
            Self::UnknownVerdict { value } => write!(
                formatter,
                "« {value} » n'est pas un des quatre verdicts de §20, et le type engendré ne peut \
                 pas le dire — un verdict inventé entrerait au dossier sous un nom que personne \
                 n'a défini"
            ),
            Self::MalformedId { value } => {
                write!(formatter, "« {value} » n'est pas une révision lisible")
            }
        }
    }
}

impl std::error::Error for HumanReviewError {}
