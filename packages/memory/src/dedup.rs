//! Déduplication et résolution d'entités — `docs/SPEC_V1.md` §16.4.
//!
//! # Les deux sortes de doublon ne se traitent pas pareil
//!
//! « Détection de duplicatas exacts par hash ; candidats sémantiques **non fusionnés
//! automatiquement**. »
//!
//! Un duplicata exact est un **constat** : deux contenus de même hash sont le même contenu, et le
//! dire n'engage personne. Un candidat sémantique est une **ressemblance**, et la ressemblance n'est
//! pas l'identité. Les fondre coûterait la chose que §16.4 protège en dernière ligne : « possibilité
//! de *mêmes mots, concepts différents* ».
//!
//! Ce module tient la distinction par le type. [`Candidate`] n'expose **aucune** méthode qui
//! fusionne ; le seul chemin est [`Resolution::decide`], qui exige une confiance, une provenance et
//! un décideur. Ce n'est pas une discipline d'appel : il n'y a pas de `merge()` à ne pas appeler.
//!
//! # « Distinct » est une réponse, pas une absence
//!
//! Une résolution peut conclure que deux candidats sont **différents**, et c'est un résultat qui se
//! consigne au même titre qu'une fusion. Sans cette variante, un candidat non fusionné serait
//! indiscernable d'un candidat jamais examiné — et quelqu'un le réexaminerait, puis un autre, jusqu'à
//! ce que l'un d'eux tranche dans l'autre sens.
//!
//! # Une fusion se défait par une **nouvelle** décision
//!
//! « Fusion réversible par nouvelle décision. » Pas par suppression : la résolution d'origine reste,
//! et celle qui la renverse la cite. C'est la forme de l'ADR 0016 décision 5 — retirer la première
//! rendrait l'histoire fausse, puisqu'on ne pourrait plus dire que des travaux ont été menés sous une
//! identification qui, désormais, n'aurait jamais existé.

use std::collections::BTreeMap;
use std::fmt;

use locus_domain::ContentHash;

/// Une entité connue de la mémoire, avec ses noms d'ailleurs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entity {
    key: String,
    hash: ContentHash,
    aliases: Vec<String>,
    external_ids: Vec<String>,
}

impl Entity {
    /// Déclarer une entité.
    ///
    /// # Errors
    ///
    /// [`DedupError::EmptyKey`] pour une entité sans clé : elle ne se retrouve pas, donc elle ne se
    /// déduplique pas non plus.
    pub fn new(key: &str, hash: ContentHash) -> Result<Self, DedupError> {
        if key.trim().is_empty() {
            return Err(DedupError::EmptyKey);
        }
        Ok(Self {
            key: key.to_owned(),
            hash,
            aliases: Vec::new(),
            external_ids: Vec::new(),
        })
    }

    /// Lui connaître un alias.
    #[must_use]
    pub fn also_known_as(mut self, alias: &str) -> Self {
        self.aliases.push(alias.to_owned());
        self
    }

    /// Lui connaître un identifiant externe — DOI, ORCID, ce que le monde lui donne.
    #[must_use]
    pub fn identified_elsewhere_as(mut self, external: &str) -> Self {
        self.external_ids.push(external.to_owned());
        self
    }

    /// Sa clé.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Le hash de son contenu.
    #[must_use]
    pub const fn hash(&self) -> &ContentHash {
        &self.hash
    }

    /// Ses alias.
    #[must_use]
    pub fn aliases(&self) -> &[String] {
        &self.aliases
    }

    /// Ses identifiants externes.
    #[must_use]
    pub fn external_ids(&self) -> &[String] {
        &self.external_ids
    }
}

/// Un groupe d'entités de même hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactDuplicates {
    /// Le hash commun.
    pub hash: ContentHash,
    /// Les clés qui le partagent, triées.
    pub keys: Vec<String>,
}

/// Les duplicatas **exacts**, par hash.
///
/// Un constat, pas une décision : deux contenus de même hash sont le même contenu. C'est la seule
/// forme de doublon qu'on puisse affirmer sans juger.
#[must_use]
pub fn exact_duplicates(entities: &[Entity]) -> Vec<ExactDuplicates> {
    let mut by_hash: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entity in entities {
        by_hash
            .entry(entity.hash.to_string())
            .or_default()
            .push(entity.key.clone());
    }
    by_hash
        .into_iter()
        .filter(|(_, keys)| keys.len() > 1)
        .filter_map(|(hash, mut keys)| {
            keys.sort_unstable();
            ContentHash::parse(&hash)
                .ok()
                .map(|hash| ExactDuplicates { hash, keys })
        })
        .collect()
}

/// Une **ressemblance** entre deux entités. Jamais une fusion.
///
/// Il n'existe sur ce type aucune méthode qui fusionne : le seul chemin est [`Resolution::decide`],
/// et il exige ce qu'une fusion doit porter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    left: String,
    right: String,
}

impl Candidate {
    /// Signaler une ressemblance.
    ///
    /// # Errors
    ///
    /// [`DedupError::SameEntity`] quand les deux clés sont la même : une entité ne ressemble pas à
    /// elle-même, et le prétendre ferait un doublon là où il n'y en a pas.
    pub fn between(left: &str, right: &str) -> Result<Self, DedupError> {
        if left == right {
            return Err(DedupError::SameEntity {
                key: left.to_owned(),
            });
        }
        let (left, right) = if left < right {
            (left, right)
        } else {
            (right, left)
        };
        Ok(Self {
            left: left.to_owned(),
            right: right.to_owned(),
        })
    }

    /// Les deux entités, dans l'ordre canonique.
    #[must_use]
    pub fn pair(&self) -> (&str, &str) {
        (&self.left, &self.right)
    }
}

/// Ce qu'une résolution conclut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Ce sont la même entité.
    Same,
    /// Ce sont deux entités différentes.
    ///
    /// **Une réponse, pas une absence.** Sans elle, un candidat non fusionné serait indiscernable
    /// d'un candidat jamais examiné, et quelqu'un le réexaminerait — jusqu'à ce que l'un d'eux
    /// tranche dans l'autre sens. C'est la « possibilité de *mêmes mots, concepts différents* » que
    /// §16.4 nomme en dernière ligne.
    Distinct,
}

/// Une résolution explicite d'entités.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolution {
    candidate: Candidate,
    verdict: Verdict,
    confidence: f64,
    provenance: String,
    decided_by: String,
    reverses: Option<Box<Resolution>>,
}

impl Resolution {
    /// Trancher une ressemblance.
    ///
    /// # Errors
    ///
    /// [`DedupError::ConfidenceOutOfRange`] hors de `[0, 1]` ou pour `NaN` — une confiance dont
    /// personne ne sait ce qu'elle mesure n'est pas une confiance ; [`DedupError::EmptyField`] pour
    /// une provenance ou un décideur vide, parce qu'une fusion anonyme ne se conteste auprès de
    /// personne et qu'une fusion sans provenance ne se rejoue pas.
    pub fn decide(
        candidate: Candidate,
        verdict: Verdict,
        confidence: f64,
        provenance: &str,
        decided_by: &str,
    ) -> Result<Self, DedupError> {
        // Le test de bornes écarte `NaN` de lui-même : toute comparaison avec `NaN` est fausse,
        // donc `contains` l'est aussi. Ajouter un `is_finite()` ferait une garde que rien
        // n'atteint — et une garde morte finit par être lue comme la seule qui protège.
        if !(0.0..=1.0).contains(&confidence) {
            return Err(DedupError::ConfidenceOutOfRange { confidence });
        }
        for (field, value) in [("provenance", provenance), ("decided_by", decided_by)] {
            if value.trim().is_empty() {
                return Err(DedupError::EmptyField { field });
            }
        }
        Ok(Self {
            candidate,
            verdict,
            confidence,
            provenance: provenance.to_owned(),
            decided_by: decided_by.to_owned(),
            reverses: None,
        })
    }

    /// Ce qui a été tranché.
    #[must_use]
    pub const fn verdict(&self) -> Verdict {
        self.verdict
    }

    /// Sur quel candidat.
    #[must_use]
    pub const fn candidate(&self) -> &Candidate {
        &self.candidate
    }

    /// Avec quelle confiance.
    #[must_use]
    pub const fn confidence(&self) -> f64 {
        self.confidence
    }

    /// D'où elle vient.
    #[must_use]
    pub fn provenance(&self) -> &str {
        &self.provenance
    }

    /// Qui a tranché.
    #[must_use]
    pub fn decided_by(&self) -> &str {
        &self.decided_by
    }

    /// La résolution qu'elle renverse, s'il y en a une.
    #[must_use]
    pub fn reverses(&self) -> Option<&Self> {
        self.reverses.as_deref()
    }

    /// Renverser cette résolution par une **nouvelle** décision.
    ///
    /// La résolution d'origine n'est pas retirée : elle est **citée**. La retirer rendrait
    /// l'histoire fausse — on ne pourrait plus dire que des travaux ont été menés sous une
    /// identification qui, désormais, n'aurait jamais existé.
    ///
    /// # Errors
    ///
    /// [`DedupError::SameVerdict`] quand la nouvelle décision conclut comme l'ancienne : elle ne
    /// renverse alors rien, et la consigner comme un renversement ferait croire à un changement.
    /// Les mêmes refus que [`Resolution::decide`] pour les autres champs.
    pub fn reversed_by(
        self,
        verdict: Verdict,
        confidence: f64,
        provenance: &str,
        decided_by: &str,
    ) -> Result<Self, DedupError> {
        if verdict == self.verdict {
            return Err(DedupError::SameVerdict { verdict });
        }
        let mut next = Self::decide(
            self.candidate.clone(),
            verdict,
            confidence,
            provenance,
            decided_by,
        )?;
        next.reverses = Some(Box::new(self));
        Ok(next)
    }
}

/// Ce qui empêche une déduplication.
#[derive(Debug, Clone, PartialEq)]
pub enum DedupError {
    /// Une entité sans clé.
    EmptyKey,
    /// Une ressemblance d'une entité avec elle-même.
    SameEntity {
        /// Laquelle.
        key: String,
    },
    /// Une confiance hors de `[0, 1]`.
    ConfidenceOutOfRange {
        /// Ce qui a été donné.
        confidence: f64,
    },
    /// Un champ obligatoire vide.
    EmptyField {
        /// Lequel.
        field: &'static str,
    },
    /// Un renversement qui conclut comme l'original.
    SameVerdict {
        /// Le verdict inchangé.
        verdict: Verdict,
    },
}

impl fmt::Display for DedupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyKey => formatter
                .write_str("une entité sans clé ne se retrouve pas, donc ne se déduplique pas"),
            Self::SameEntity { key } => write!(
                formatter,
                "« {key} » ne ressemble pas à elle-même : le prétendre ferait un doublon là où il \
                 n'y en a pas"
            ),
            Self::ConfidenceOutOfRange { confidence } => write!(
                formatter,
                "confiance {confidence} hors de [0, 1] : un chiffre dont personne ne sait ce qu'il \
                 mesure n'est pas une confiance"
            ),
            Self::EmptyField { field } => write!(
                formatter,
                "« {field} » est vide : une fusion anonyme ne se conteste auprès de personne, et \
                 une fusion sans provenance ne se rejoue pas"
            ),
            Self::SameVerdict { verdict } => write!(
                formatter,
                "la nouvelle décision conclut comme l'ancienne ({verdict:?}) : elle ne renverse \
                 rien, et la consigner comme un renversement ferait croire à un changement"
            ),
        }
    }
}

impl std::error::Error for DedupError {}
