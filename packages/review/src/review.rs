//! La `Review`, le `Finding`, et l'attestation d'indépendance — `docs/SPEC_V1.md` §17.1, §17.4,
//! §17.5, §14.4.
//!
//! # « Une revue sans attestation n'est pas une revue indépendante »
//!
//! La clause est tenue plus fort que son énoncé : une revue **sans** attestation n'existe pas.
//! [`Review::render`] calcule la sienne en confrontant le relecteur au générateur, exigence par
//! exigence, et aucun appelant ne peut en fournir une — les champs sont privés et il n'y a pas de
//! constructeur littéral. Un relecteur ne peut donc pas se déclarer indépendant, seulement l'être.
//!
//! Sans le bloc ci-dessous, la garantie ne tiendrait qu'à la discipline de qui ajoute un champ
//! `pub` ou un `with_attestation` un jour. `cargo test --doc` l'exécute, et il doit **ne pas**
//! compiler :
//!
//! ```compile_fail
//! use locus_protocol::{Id, id::Agent};
//! use locus_review::{IndependenceAttestation, Review};
//! fn se_declarer(reviewer: Id<Agent>, attestation: IndependenceAttestation) -> Review {
//!     Review {
//!         dossier_id: "dossier-0001".to_owned(),
//!         reviewer,
//!         attestation,
//!         findings: Vec::new(),
//!         coverage: "relecture".to_owned(),
//!         limitations: Vec::new(),
//!     }
//! }
//! ```
//!
//! Il énumère **tous** les champs : un bloc auquel il en manquerait un échouerait à compiler pour
//! cette raison-là, et pinerait la faute de frappe au lieu de la garantie.

use std::collections::BTreeSet;
use std::fmt;

use locus_domain::RevisionId;
use locus_protocol::{Id, id::Agent};

use crate::dossier::{Frozen, IndependenceRequirement};

/// La gravité d'un finding — §17.5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Pour information.
    Info,
    /// Mineur.
    Minor,
    /// Majeur.
    Major,
    /// Bloquant.
    Blocking,
}

impl Severity {
    /// Les quatre, de la moindre à la plus grave.
    pub const ALL: [Self; 4] = [Self::Info, Self::Minor, Self::Major, Self::Blocking];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Minor => "minor",
            Self::Major => "major",
            Self::Blocking => "blocking",
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Ce qu'un finding conclut — §17.5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Verdict {
    /// L'élément soutient la revendication.
    Supports,
    /// L'élément la réfute.
    Refutes,
    /// Il n'y a pas de quoi conclure.
    ///
    /// Distinct de `refutes`, et c'est §17.7 qui l'exige : la méta-revue « distingue absence de
    /// preuve, contradiction et réfutation ». Les confondre transformerait un manque en résultat.
    Insufficient,
    /// La question ne s'applique pas.
    NotApplicable,
}

impl Verdict {
    /// Les quatre de §17.5.
    pub const ALL: [Self; 4] = [
        Self::Supports,
        Self::Refutes,
        Self::Insufficient,
        Self::NotApplicable,
    ];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Supports => "supports",
            Self::Refutes => "refutes",
            Self::Insufficient => "insufficient",
            Self::NotApplicable => "not_applicable",
        }
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Un constat de revue — §17.5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    target: RevisionId,
    issue_type: String,
    severity: Severity,
    verdict: Verdict,
    evidence: Vec<RevisionId>,
}

impl Finding {
    /// Consigner un constat.
    ///
    /// # Errors
    ///
    /// [`ReviewError::EmptyField`] pour un type de problème vide.
    pub fn new(
        target: RevisionId,
        issue_type: &str,
        severity: Severity,
        verdict: Verdict,
        evidence: Vec<RevisionId>,
    ) -> Result<Self, ReviewError> {
        if issue_type.trim().is_empty() {
            return Err(ReviewError::EmptyField {
                field: "issue_type",
            });
        }
        Ok(Self {
            target,
            issue_type: issue_type.to_owned(),
            severity,
            verdict,
            evidence,
        })
    }

    /// La révision visée.
    #[must_use]
    pub const fn target(&self) -> &RevisionId {
        &self.target
    }

    /// Le type de problème.
    #[must_use]
    pub fn issue_type(&self) -> &str {
        &self.issue_type
    }

    /// Sa gravité déclarée.
    #[must_use]
    pub const fn severity(&self) -> Severity {
        self.severity
    }

    /// Ce qu'il conclut.
    #[must_use]
    pub const fn verdict(&self) -> Verdict {
        self.verdict
    }

    /// Les preuves citées, par révision.
    #[must_use]
    pub fn evidence(&self) -> &[RevisionId] {
        &self.evidence
    }

    /// Vrai quand ce finding peut à lui seul changer un niveau de validation.
    ///
    /// §17.5 : « un finding **sans preuve concrète** est un commentaire non bloquant et ne peut à
    /// lui seul changer un niveau de validation. » La gravité déclarée ne suffit donc pas — un
    /// `blocking` sans preuve reste un avis, et c'est ce que cette fonction dit d'une seule voix
    /// plutôt que de laisser chaque appelant recomposer la règle.
    #[must_use]
    pub fn is_binding(&self) -> bool {
        !self.evidence.is_empty() && matches!(self.severity, Severity::Blocking | Severity::Major)
    }
}

/// Ce qu'un relecteur atteste de son indépendance — §17.4, `independence_attestation`.
///
/// # Elle est constatée, pas déclarée
///
/// L'attestation n'est pas une case que le relecteur coche : elle est **calculée** en confrontant
/// le relecteur au générateur, exigence par exigence. C'est la quatrième occurrence de la même
/// forme dans ce chantier — attestation de sandbox (W4.d.2), digest de build (W5.e), niveau de
/// reproductibilité (W6.d) — et toujours pour la même raison : ce qui prouve ne peut pas être ce
/// qui est demandé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndependenceAttestation {
    satisfied: BTreeSet<IndependenceRequirement>,
    violated: BTreeSet<IndependenceRequirement>,
}

impl IndependenceAttestation {
    /// Vrai quand toutes les exigences du dossier sont satisfaites.
    #[must_use]
    pub fn holds(&self) -> bool {
        self.violated.is_empty()
    }

    /// Ce qui est satisfait.
    #[must_use]
    pub const fn satisfied(&self) -> &BTreeSet<IndependenceRequirement> {
        &self.satisfied
    }

    /// Ce qui ne l'est pas.
    #[must_use]
    pub const fn violated(&self) -> &BTreeSet<IndependenceRequirement> {
        &self.violated
    }
}

/// Qui a produit, et qui relit.
///
/// Les deux mêmes identités qu'en W13.d : l'agent est un rôle situé, le worker une machine. Le
/// groupe d'indépendance vient du template (W13.c) et descend à l'instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Party {
    /// L'instance d'agent.
    pub agent_id: Id<Agent>,
    /// Le worker où elle tourne.
    pub worker_id: String,
    /// Son groupe d'indépendance, quand le template en porte un.
    pub independence_group: Option<String>,
    /// Vrai quand cette partie a reçu le transcript de génération.
    pub holds_generator_transcript: bool,
}

/// Constater l'indépendance d'un relecteur vis-à-vis d'un générateur.
///
/// # Ce que chaque exigence regarde
///
/// - **Groupe distinct** : deux relecteurs du même groupe ne comptent pas comme indépendants
///   (§14.4). Deux `None` ne sont **pas** distincts — un groupe inconnu n'est pas un groupe
///   différent, et conclure l'inverse ferait de l'absence d'information une preuve.
/// - **Worker distinct** : deux identités, deux questions (W13.d).
/// - **Pas de transcript** : invariant 11, littéralement.
#[must_use]
pub fn attest(dossier: &Frozen, generator: &Party, reviewer: &Party) -> IndependenceAttestation {
    let mut satisfied = BTreeSet::new();
    let mut violated = BTreeSet::new();

    for requirement in dossier.independence() {
        let met = match requirement {
            IndependenceRequirement::DistinctIndependenceGroup => {
                match (&generator.independence_group, &reviewer.independence_group) {
                    (Some(left), Some(right)) => left != right,
                    // Un groupe inconnu n'est pas un groupe différent.
                    _ => false,
                }
            }
            IndependenceRequirement::DistinctWorker => generator.worker_id != reviewer.worker_id,
            IndependenceRequirement::NoGeneratorTranscript => !reviewer.holds_generator_transcript,
        };
        if met {
            satisfied.insert(*requirement);
        } else {
            violated.insert(*requirement);
        }
    }

    IndependenceAttestation {
        satisfied,
        violated,
    }
}

/// Une revue rendue — §17.4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Review {
    dossier_id: String,
    reviewer: Id<Agent>,
    attestation: IndependenceAttestation,
    findings: Vec<Finding>,
    coverage: String,
    limitations: Vec<String>,
}

impl Review {
    /// Rendre une revue.
    ///
    /// # Errors
    ///
    /// [`ReviewError::EmptyField`] pour une couverture vide — §17.1 exige que la revue rende
    /// explicite « le dossier consulté » et sa couverture ; une revue qui ne dit pas ce qu'elle a
    /// examiné ne peut pas être contestée. [`ReviewError::ReviewerIsAuthor`] quand le relecteur est
    /// le générateur : ce n'est pas une revue, c'est une relecture.
    pub fn render(
        dossier: &Frozen,
        generator: &Party,
        reviewer: &Party,
        findings: Vec<Finding>,
        coverage: &str,
    ) -> Result<Self, ReviewError> {
        if coverage.trim().is_empty() {
            return Err(ReviewError::EmptyField { field: "coverage" });
        }
        if generator.agent_id == reviewer.agent_id {
            return Err(ReviewError::ReviewerIsAuthor);
        }
        Ok(Self {
            dossier_id: dossier.id().to_owned(),
            reviewer: reviewer.agent_id,
            attestation: attest(dossier, generator, reviewer),
            findings,
            coverage: coverage.to_owned(),
            limitations: Vec::new(),
        })
    }

    /// Déclarer une limite de la revue — §17.4, `limitations`.
    #[must_use]
    pub fn limited_by(mut self, limitation: &str) -> Self {
        self.limitations.push(limitation.to_owned());
        self
    }

    /// Le dossier consulté.
    #[must_use]
    pub fn dossier_id(&self) -> &str {
        &self.dossier_id
    }

    /// Le relecteur.
    #[must_use]
    pub const fn reviewer(&self) -> Id<Agent> {
        self.reviewer
    }

    /// Ce que l'indépendance a donné.
    #[must_use]
    pub const fn attestation(&self) -> &IndependenceAttestation {
        &self.attestation
    }

    /// Les constats.
    #[must_use]
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    /// Ce qui a été couvert.
    #[must_use]
    pub fn coverage(&self) -> &str {
        &self.coverage
    }

    /// Ce que la revue déclare ne pas avoir pu faire.
    #[must_use]
    pub fn limitations(&self) -> &[String] {
        &self.limitations
    }

    /// Vrai quand cette revue est **indépendante** au sens du dossier.
    ///
    /// Une revue non indépendante n'est pas nulle : elle est une revue, et elle reste au dossier.
    /// Ce qu'elle ne peut pas faire est compter comme la revue indépendante que la politique
    /// exigeait — et c'est cette phrase-là qui doit être une fonction plutôt qu'une convention.
    #[must_use]
    pub fn is_independent(&self) -> bool {
        self.attestation.holds()
    }

    /// Les constats qui peuvent à eux seuls changer un niveau de validation.
    #[must_use]
    pub fn binding_findings(&self) -> Vec<&Finding> {
        self.findings
            .iter()
            .filter(|finding| finding.is_binding())
            .collect()
    }
}

/// Ce qui empêche une revue ou un constat d'exister.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewError {
    /// Un champ obligatoire vide.
    EmptyField {
        /// Lequel.
        field: &'static str,
    },
    /// Le relecteur est l'auteur.
    ReviewerIsAuthor,
}

impl fmt::Display for ReviewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(formatter, "le champ « {field} » est vide"),
            Self::ReviewerIsAuthor => {
                formatter.write_str("relire son propre travail n'est pas une revue")
            }
        }
    }
}

impl std::error::Error for ReviewError {}
