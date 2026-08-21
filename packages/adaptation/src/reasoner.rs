//! Le raisonneur d'ontologie — `W18.h`, ADR 0023 décisions 2, 3, 4 et 7.
//!
//! # Une capacité admise, jamais une dépendance
//!
//! Rien ici ne construit un raisonneur : ce module ne connaît que des [`Admission`], et une
//! `Admission` ne se fabrique que par [`crate::admit`], qui exige un `Published` de `W5.b`. Le
//! chemin de gouvernance est donc le seul, par signature et non par discipline — et c'est ce que
//! `W18.d` avait construit sans qu'aucun artefact réel ne l'éprouve.
//!
//! # Trois verdicts, et le troisième refuse la confiance
//!
//! Un échec à dériver une contradiction **n'est pas** une cohérence. Un raisonneur qui rendrait
//! « cohérent » faute d'avoir trouvé de contradiction convertirait une limite de calcul en
//! affirmation — et l'hypothèse de monde ouvert rend cette conversion systématiquement fausse.
//!
//! C'est la discipline de `W4.b` : « une sonde non exécutée est un troisième verdict », qui refuse
//! la confiance parce que c'est la preuve qui manque.
//!
//! # Une sortie est un claim **proposé**
//!
//! Avec sa provenance — quel raisonneur, quelle version d'ontologie, quel profil — et soumise au
//! pipeline de validation normal de §8.1. Jamais un fait. Ce module ne connaît donc ni `Inference`
//! ni `Support` : l'absence de chemin est ce qui tient la règle, et un test la vérifie sur la source.
//!
//! # La résolution se fait par identité, jamais par nom
//!
//! Le motif vient d'un harnais tiers, et il est bon : ses providers de mémoire sont découverts dans
//! l'ordre « la source la plus précoce l'emporte », l'inverse de son système de plugins général,
//! parce qu'un provider activé **par nom** qu'on masque « redirigerait silencieusement la mémoire de
//! l'agent au lieu de simplement remplacer un outil ».
//!
//! Une substitution de source de connaissance ne produit pas d'erreur : elle produit des réponses
//! plausibles fondées sur autre chose. Le registre est donc clé par le **digest d'image** que
//! l'admission porte, et il n'existe aucune résolution par nom.

use std::collections::BTreeMap;
use std::fmt;

use crate::admission::Admission;

/// Ce qu'un raisonneur conclut — **trois** valeurs, ADR 0023 décision 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Aucune contradiction, **et** le résidu a été déchargé.
    Consistent,
    /// Une contradiction a été dérivée.
    Rejected,
    /// Le raisonneur n'a pas tranché.
    ///
    /// Distinct de `Consistent`, et c'est tout l'objet du troisième verdict : un échec à dériver
    /// une contradiction n'est pas une cohérence. Les fondre convertirait une limite de calcul —
    /// un budget épuisé, un profil trop faible, un fragment indécidable — en affirmation.
    Undetermined,
}

impl Verdict {
    /// Les trois.
    pub const ALL: [Self; 3] = [Self::Consistent, Self::Rejected, Self::Undetermined];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Consistent => "consistent",
            Self::Rejected => "rejected",
            Self::Undetermined => "undetermined",
        }
    }

    /// Vrai quand ce verdict **soutient** quelque chose.
    ///
    /// Faux pour `Undetermined`, et c'est ce que « refuse la confiance » veut dire : un appelant qui
    /// interroge ce prédicat n'a pas à se souvenir lequel des trois est l'ignorance.
    #[must_use]
    pub const fn supports_a_claim(self) -> bool {
        matches!(self, Self::Consistent | Self::Rejected)
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// D'où vient une conclusion de raisonneur.
///
/// Les trois champs sont obligatoires. Une conclusion sans version d'ontologie ne se rejoue pas :
/// la même question posée à la même ontologie révisée peut rendre l'inverse, et rien dans la
/// conclusion ne le dirait.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    reasoner: String,
    ontology_version: String,
    profile: String,
}

impl Provenance {
    /// Nommer la provenance d'une conclusion.
    ///
    /// # Errors
    ///
    /// [`ReasonerError::MissingProvenance`] pour un champ vide.
    pub fn of(
        reasoner: impl Into<String>,
        ontology_version: impl Into<String>,
        profile: impl Into<String>,
    ) -> Result<Self, ReasonerError> {
        let (reasoner, ontology_version, profile) =
            (reasoner.into(), ontology_version.into(), profile.into());
        for (field, value) in [
            ("reasoner", &reasoner),
            ("ontology_version", &ontology_version),
            ("profile", &profile),
        ] {
            if value.trim().is_empty() {
                return Err(ReasonerError::MissingProvenance { field });
            }
        }
        Ok(Self {
            reasoner,
            ontology_version,
            profile,
        })
    }

    /// Quel raisonneur.
    #[must_use]
    pub fn reasoner(&self) -> &str {
        &self.reasoner
    }

    /// Quelle version d'ontologie.
    #[must_use]
    pub fn ontology_version(&self) -> &str {
        &self.ontology_version
    }

    /// Quel profil de raisonnement.
    #[must_use]
    pub fn profile(&self) -> &str {
        &self.profile
    }
}

/// Ce qu'un raisonneur rend : un claim **proposé**, jamais un fait.
///
/// Le nom du type porte la règle. Il n'existe aucune conversion vers un objet validé, et ce module
/// ne connaît ni `Inference` ni `Support` — c'est l'absence qui tient, pas une convention d'appel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposedClaim {
    subject: String,
    verdict: Verdict,
    provenance: Provenance,
}

impl ProposedClaim {
    /// Proposer une conclusion.
    ///
    /// # Errors
    ///
    /// [`ReasonerError::EmptySubject`] pour un sujet vide.
    pub fn proposed(
        subject: impl Into<String>,
        verdict: Verdict,
        provenance: Provenance,
    ) -> Result<Self, ReasonerError> {
        let subject = subject.into();
        if subject.trim().is_empty() {
            return Err(ReasonerError::EmptySubject);
        }
        Ok(Self {
            subject,
            verdict,
            provenance,
        })
    }

    /// Sur quoi elle porte.
    #[must_use]
    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// Le verdict.
    #[must_use]
    pub const fn verdict(&self) -> Verdict {
        self.verdict
    }

    /// D'où elle vient.
    #[must_use]
    pub const fn provenance(&self) -> &Provenance {
        &self.provenance
    }
}

/// Les raisonneurs admis, résolus **par identité**.
///
/// La clé est le digest d'image que l'admission porte. Deux capacités homonymes sont donc deux
/// entrées distinctes, et enregistrer la seconde ne masque pas la première : il n'existe aucune
/// résolution par nom qui puisse en préférer une.
#[derive(Debug, Clone, Default)]
pub struct Reasoners {
    by_identity: BTreeMap<String, Admission>,
}

impl Reasoners {
    /// Un registre vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Inscrire une capacité admise.
    ///
    /// # Errors
    ///
    /// [`ReasonerError::AlreadyRegistered`] quand la **même identité** est inscrite deux fois — un
    /// remplacement silencieux redirigerait les questions vers autre chose sans erreur.
    pub fn register(&mut self, admission: Admission) -> Result<&Admission, ReasonerError> {
        let identity = admission.image_digest().to_owned();
        if self.by_identity.contains_key(&identity) {
            return Err(ReasonerError::AlreadyRegistered { identity });
        }
        Ok(self.by_identity.entry(identity).or_insert(admission))
    }

    /// Résoudre **par identité**.
    #[must_use]
    pub fn resolve(&self, identity: &str) -> Option<&Admission> {
        self.by_identity.get(identity)
    }

    /// Combien de raisonneurs sont inscrits.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_identity.len()
    }

    /// Vrai quand aucun n'est inscrit.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_identity.is_empty()
    }
}

/// Pourquoi une conclusion ou une inscription est refusée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReasonerError {
    /// Un champ de provenance vide.
    MissingProvenance {
        /// Lequel.
        field: &'static str,
    },
    /// Un sujet vide.
    EmptySubject,
    /// Cette identité est déjà inscrite.
    AlreadyRegistered {
        /// Laquelle.
        identity: String,
    },
}

impl fmt::Display for ReasonerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingProvenance { field } => write!(
                formatter,
                "« {field} » manque : une conclusion sans provenance complète ne se rejoue pas, et \
                 la même question posée à la même ontologie révisée peut rendre l'inverse"
            ),
            Self::EmptySubject => {
                formatter.write_str("une conclusion sans sujet ne porte sur rien")
            }
            Self::AlreadyRegistered { identity } => write!(
                formatter,
                "« {identity} » est déjà inscrite : un remplacement silencieux redirigerait les \
                 questions vers autre chose sans produire d'erreur"
            ),
        }
    }
}

impl std::error::Error for ReasonerError {}
