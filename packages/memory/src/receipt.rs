//! Le reçu de retrieval — ADR 0022 décision 6.
//!
//! # Ce qu'un reçu rend possible, et que rien d'autre ne rend possible
//!
//! Deux choses, et elles sont structurelles plutôt que décoratives.
//!
//! Il rend le déclencheur `DomainGapDetected` **auditable** : une lacune cesse d'être affirmée par
//! un agent pour devenir lisible dans un document. `W18.g` en dépend — sans reçu, le capteur de
//! lacune n'aurait qu'une parole d'agent à lire.
//!
//! Il rend le retrieval **réfutable** : on peut objecter à l'exclusion d'un résultat négatif comme
//! on objecte à une prémisse. Les systèmes de mémoire de 2026 rendent le retrieval auditable ;
//! aucun ne le rend contestable.
//!
//! # La contestation vise le reçu, jamais la vue
//!
//! §16.2 : « une `ContextView` est immuable, adressée par hash ». Contester une vue n'a pas de sens
//! — elle est ce qui a été vu, et c'est un fait. Ce qui se conteste est **la manière dont elle a été
//! constituée** : ce plan-là, ces canaux-là, cette exclusion-là. D'où deux objets, et une seule
//! cible d'objection.
//!
//! # Ce que le reçu ne détient pas
//!
//! Rien que le journal n'ait écrit, ou que le plan n'ait déclaré. Il ne garde aucun contenu : les
//! clés retenues sont des identités, pas des documents. Un reçu qui embarquerait ce qu'il a servi
//! serait un second stockage du même fait, et c'est l'argument qui a écarté le courtier de messages
//! dans l'ADR 0019.

use std::fmt;
use std::fmt::Write as _;

use locus_domain::ContentHash;

use crate::plan::{Channel, Escalation, Intent, Plan, RankingIdentity};

/// La ligne d'en-tête de la forme canonique d'un reçu.
const RECEIPT_MAGIC: &str = "retrieval-receipt/1";

/// Une couverture mesurée, entre 0 et 1.
///
/// # `None` et `Some(0.0)` ne se confondent pas
///
/// « Non mesurée » et « mesurée et nulle » sont deux états différents, et le second est une
/// information — il dit qu'on a cherché une contre-preuve et qu'il n'y en avait pas. Les fondre
/// ferait lire « aucune contre-preuve » là où personne n'a regardé, ce qui est la faute que ce
/// dépôt refuse partout : `unverified` n'est pas un `broken` atténué.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coverage(f64);

impl Coverage {
    /// Une couverture mesurée.
    ///
    /// # Errors
    ///
    /// [`ReceiptError::CoverageOutOfRange`] hors de `[0, 1]`, et pour un flottant non fini : une
    /// couverture qui n'est pas une proportion ne se lit pas comme une proportion.
    pub fn measured(value: f64) -> Result<Self, ReceiptError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(ReceiptError::CoverageOutOfRange { value });
        }
        Ok(Self(value))
    }

    /// Sa valeur.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

impl fmt::Display for Coverage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:.4}", self.0)
    }
}

/// Une exclusion, **avec son motif**.
///
/// Un motif vide n'est pas constructible : une exclusion sans motif est indistinguable d'un oubli,
/// et c'est précisément ce qu'un reçu existe pour rendre impossible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exclusion {
    key: String,
    reason: String,
}

impl Exclusion {
    /// Une exclusion motivée.
    ///
    /// # Errors
    ///
    /// [`ReceiptError::UnmotivatedExclusion`] pour un motif vide ou blanc ;
    /// [`ReceiptError::ForgesALine`] pour un caractère de contrôle — la forme canonique d'un reçu
    /// est un texte à lignes, et un motif qui en forge une insérerait une exclusion que personne
    /// n'a écrite. Même durcissement que les quatre formes de `W17.h`, et pour la même raison.
    pub fn motivated(
        key: impl Into<String>,
        reason: impl Into<String>,
    ) -> Result<Self, ReceiptError> {
        let key = key.into();
        let reason = reason.into();
        if reason.trim().is_empty() {
            return Err(ReceiptError::UnmotivatedExclusion { key });
        }
        for (field, value) in [("clé", &key), ("motif", &reason)] {
            if value.chars().any(char::is_control) {
                return Err(ReceiptError::ForgesALine {
                    field: field.to_owned(),
                });
            }
        }
        Ok(Self { key, reason })
    }

    /// Ce qui a été écarté.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Pourquoi.
    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

/// Ce qu'un retrieval a fait, écrit pour être relu et contesté.
#[derive(Debug, Clone, PartialEq)]
pub struct RetrievalReceipt {
    intent: Intent,
    channels: Vec<Channel>,
    ranking: RankingIdentity,
    watermark: u64,
    budget: usize,
    negative_reserve: usize,
    considered: usize,
    included: Vec<String>,
    exclusions: Vec<Exclusion>,
    escalations: Vec<Escalation>,
    evidence: Option<Coverage>,
    counter_evidence: Option<Coverage>,
    gaps: Vec<String>,
}

impl RetrievalReceipt {
    /// Écrire un reçu.
    ///
    /// # Errors
    ///
    /// [`ReceiptError::ForgesALine`] pour un caractère de contrôle dans une clé retenue ou une
    /// lacune déclarée.
    pub fn write(
        plan: &Plan,
        watermark: u64,
        considered: usize,
        included: Vec<String>,
        exclusions: Vec<Exclusion>,
        escalations: Vec<Escalation>,
    ) -> Result<Self, ReceiptError> {
        for key in &included {
            if key.chars().any(char::is_control) {
                return Err(ReceiptError::ForgesALine {
                    field: "clé retenue".to_owned(),
                });
            }
        }
        Ok(Self {
            intent: plan.intent(),
            channels: plan.channels().to_vec(),
            ranking: plan.ranking().clone(),
            watermark,
            budget: plan.budget(),
            negative_reserve: plan.negative_reserve(),
            considered,
            included,
            exclusions,
            escalations,
            evidence: None,
            counter_evidence: None,
            gaps: Vec::new(),
        })
    }

    /// Y inscrire les couvertures mesurées.
    ///
    /// **Rendues même à zéro** : `None` dit « non mesurée », `Some(0.0)` dit « mesurée et nulle ».
    #[must_use]
    pub const fn with_coverage(
        mut self,
        evidence: Option<Coverage>,
        counter_evidence: Option<Coverage>,
    ) -> Self {
        self.evidence = evidence;
        self.counter_evidence = counter_evidence;
        self
    }

    /// Y inscrire les lacunes connues.
    ///
    /// # Errors
    ///
    /// [`ReceiptError::ForgesALine`] pour un caractère de contrôle.
    pub fn with_gaps(mut self, gaps: Vec<String>) -> Result<Self, ReceiptError> {
        for gap in &gaps {
            if gap.chars().any(char::is_control) {
                return Err(ReceiptError::ForgesALine {
                    field: "lacune".to_owned(),
                });
            }
        }
        self.gaps = gaps;
        Ok(self)
    }

    /// Ce que la question cherchait.
    #[must_use]
    pub const fn intent(&self) -> Intent {
        self.intent
    }

    /// Les canaux interrogés, dans l'ordre.
    #[must_use]
    pub fn channels(&self) -> &[Channel] {
        &self.channels
    }

    /// Ce qui a produit les scores.
    #[must_use]
    pub const fn ranking(&self) -> &RankingIdentity {
        &self.ranking
    }

    /// Le watermark de source.
    #[must_use]
    pub const fn watermark(&self) -> u64 {
        self.watermark
    }

    /// La réserve de négatifs du plan — **écrite même quand elle vaut zéro**.
    #[must_use]
    pub const fn negative_reserve(&self) -> usize {
        self.negative_reserve
    }

    /// Combien de candidats ont été considérés.
    #[must_use]
    pub const fn considered(&self) -> usize {
        self.considered
    }

    /// Les identités retenues.
    #[must_use]
    pub fn included(&self) -> &[String] {
        &self.included
    }

    /// Les exclusions, avec leurs motifs.
    #[must_use]
    pub fn exclusions(&self) -> &[Exclusion] {
        &self.exclusions
    }

    /// Les escalades, s'il y en a eu.
    #[must_use]
    pub fn escalations(&self) -> &[Escalation] {
        &self.escalations
    }

    /// La couverture en preuve — `None` quand elle n'a pas été mesurée.
    #[must_use]
    pub const fn evidence_coverage(&self) -> Option<Coverage> {
        self.evidence
    }

    /// La couverture en **contre-preuve** — `None` quand elle n'a pas été mesurée.
    #[must_use]
    pub const fn counter_evidence_coverage(&self) -> Option<Coverage> {
        self.counter_evidence
    }

    /// Les lacunes connues.
    #[must_use]
    pub fn gaps(&self) -> &[String] {
        &self.gaps
    }

    /// Vrai quand ce reçu promet un rejeu — c'est-à-dire quand sa fonction de classement est nommée.
    ///
    /// Un reçu dont les scores viennent de l'appelant **ne promet rien**, et le dire vaut mieux que
    /// de laisser croire à une garantie qui n'est pas là.
    #[must_use]
    pub fn promises_replay(&self) -> bool {
        self.ranking.is_replayable()
    }

    /// Sa forme canonique — celle sur laquelle porte le condensat.
    ///
    /// Un texte à lignes, **non trié** pour les canaux et les inclusions : l'ordre des canaux est le
    /// plan, et l'ordre des inclusions est le classement. Trier effacerait précisément ce qui se
    /// conteste.
    #[must_use]
    pub fn canonical(&self) -> String {
        let mut canonical = format!("{RECEIPT_MAGIC}\nintent\t{}\n", self.intent);
        let _ = writeln!(canonical, "ranking\t{}", self.ranking);
        let _ = writeln!(canonical, "watermark\t{}", self.watermark);
        let _ = writeln!(canonical, "budget\t{}", self.budget);
        let _ = writeln!(canonical, "reserve\t{}", self.negative_reserve);
        let _ = writeln!(canonical, "considered\t{}", self.considered);
        for channel in &self.channels {
            let _ = writeln!(canonical, "channel\t{channel}");
        }
        for key in &self.included {
            let _ = writeln!(canonical, "in\t{key}");
        }
        for exclusion in &self.exclusions {
            let _ = writeln!(canonical, "out\t{}\t{}", exclusion.key, exclusion.reason);
        }
        for escalation in &self.escalations {
            let _ = writeln!(canonical, "up\t{}", render_escalation(escalation));
        }
        // Les deux couvertures, **toujours écrites** : une ligne absente et une ligne à zéro ne se
        // lisent pas pareil, et le reçu est le lieu où cette distinction compte le plus.
        let _ = writeln!(canonical, "evidence\t{}", slot(self.evidence));
        let _ = writeln!(canonical, "counter\t{}", slot(self.counter_evidence));
        for gap in &self.gaps {
            let _ = writeln!(canonical, "gap\t{gap}");
        }
        canonical
    }

    /// Le condensat de ce reçu.
    #[must_use]
    pub fn digest(&self) -> ContentHash {
        ContentHash::of(self.canonical().as_bytes())
    }
}

/// Ce qu'écrit une couverture non mesurée.
///
/// `-`, et une couverture mesurée s'écrit avec quatre décimales : `0.0000` n'est donc jamais `-`.
fn slot(coverage: Option<Coverage>) -> String {
    coverage.map_or_else(|| "-".to_owned(), |value| value.to_string())
}

fn render_escalation(escalation: &Escalation) -> String {
    match escalation {
        Escalation::DeeperGraph {
            from_depth,
            to_depth,
        } => format!("deeper-graph\t{from_depth}\t{to_depth}"),
        Escalation::BroaderScope {
            requested,
            granted_by,
        } => format!("broader-scope\t{requested}\t{granted_by}"),
        Escalation::Coprocessor { capability_id } => format!("coprocessor\t{capability_id}"),
    }
}

/// Pourquoi un reçu ne s'écrit pas.
#[derive(Debug, Clone, PartialEq)]
pub enum ReceiptError {
    /// Une exclusion sans motif.
    UnmotivatedExclusion {
        /// Ce qui a été écarté.
        key: String,
    },
    /// Un champ qui forgerait une ligne de la forme canonique.
    ForgesALine {
        /// Lequel.
        field: String,
    },
    /// Une couverture qui n'est pas une proportion.
    CoverageOutOfRange {
        /// Ce qui a été donné.
        value: f64,
    },
}

impl fmt::Display for ReceiptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnmotivatedExclusion { key } => write!(
                formatter,
                "« {key} » est écartée sans motif : une exclusion sans motif est indistinguable \
                 d'un oubli, et c'est ce qu'un reçu existe pour rendre impossible"
            ),
            Self::ForgesALine { field } => write!(
                formatter,
                "{field} contient un caractère de contrôle : il forgerait une ligne de la forme \
                 canonique du reçu"
            ),
            Self::CoverageOutOfRange { value } => write!(
                formatter,
                "une couverture de {value} n'est pas une proportion : elle ne se lit pas comme telle"
            ),
        }
    }
}

impl std::error::Error for ReceiptError {}
