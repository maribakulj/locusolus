//! L'admission de capacité — `docs/SPEC_V1.md` §19.3 et §19.5, ADR 0016 décision 8.
//!
//! # Ce qui manquait, et ce qui ne manquait pas
//!
//! L'ADR le dit en une phrase : « Locusolus possède déjà le blueprint, l'artefact, l'attestation et
//! le refus nommant toutes ses conditions. Ce qui manque est **la proposition, la politique et
//! l'approbation** : du travail de gouvernance. » Ce module est ce travail-là, et rien de plus. Il
//! ne construit aucune image, ne scanne rien, ne signe rien — `packages/environments` le fait
//! depuis W5.b, et refaire un maillon de sa chaîne ici en ferait un deuxième chemin, plus court.
//!
//! # Une capacité nouvelle n'entre que par un `Published`
//!
//! [`admit`] exige un `environments::Published`, et cette valeur « est la preuve que les six étapes
//! ont eu lieu, dans l'ordre : elle ne se construit pas autrement ». Il n'existe donc pas d'argument
//! par lequel une capacité entrerait sans lockfile, sans SBOM, sans scan, sans tests et sans
//! signature — non parce qu'on les vérifie ici, mais parce que la valeur qu'on exige ne peut pas
//! exister sans eux.
//!
//! # Du code injecté n'est pas une valeur exprimable
//!
//! Ce que la littérature et les harnais tiers appellent « système de plugins » est ici une admission
//! de capacité. La différence tient à ce qui **circule** : un plugin fait circuler du code qu'un
//! processus charge ; une admission fait circuler un digest d'image que `locus-execd` fait tourner
//! sous sandbox. Aucun type de ce module ne porte de source, de script, de chemin de bibliothèque ni
//! d'expression à évaluer, et un test lit le module pour le tenir. Ce n'est pas une garantie
//! partielle : il n'y a pas de champ à remplir.
//!
//! # L'extension est un axe **orthogonal**, pas un cinquième barreau
//!
//! ADR 0016 décision 8 : « Un déploiement peut être en `bounded` sur la coordination et interdire
//! toute capacité nouvelle, ou l'inverse. » [`Extension`] est donc son propre interrupteur, et non
//! une valeur de `Mode`. Le ranger dans `Mode` aurait fait de l'extension de capacité une
//! conséquence de l'autonomie de coordination, alors que les deux se décident séparément et n'ont
//! pas les mêmes conséquences quand elles se trompent.

use std::fmt;

use locus_coordination::{Author, Capability};
use locus_environments::{HealthResult, Published};
use locus_policy::{Outcome, Verb};

/// L'axe orthogonal : ce déploiement admet-il des capacités nouvelles ?
///
/// Le défaut est `Forbidden`, pour la raison de §33 — l'autonomie sans seuil est un non-objectif —
/// et parce qu'ouvrir se lit dans un diff quand ne pas avoir fermé ne se lit nulle part.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Extension {
    /// Aucune capacité nouvelle. **Le défaut.**
    #[default]
    Forbidden,
    /// Une capacité nouvelle entre par la porte de ce module, et par elle seule.
    Governed,
}

impl Extension {
    /// Les deux.
    pub const ALL: [Self; 2] = [Self::Forbidden, Self::Governed];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Forbidden => "forbidden",
            Self::Governed => "governed",
        }
    }
}

impl fmt::Display for Extension {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Une capacité admise — la valeur qu'aucun autre chemin ne produit.
///
/// Elle porte le digest et la clé de signature plutôt que le `Published` entier : ce qui suit
/// l'admission est une **mission**, et une mission a besoin de savoir quelle image lancer, pas de
/// relire le SBOM. Le `Published` reste chez celui qui l'a construit, et l'admission ne le duplique
/// pas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Admission {
    capability: Capability,
    image_digest: String,
    signing_key: String,
    approved_by: Author,
    rule: String,
}

impl Admission {
    /// La capacité admise.
    #[must_use]
    pub const fn capability(&self) -> &Capability {
        &self.capability
    }

    /// Le digest de l'image qui la porte.
    ///
    /// Un digest, jamais un tag : W5.a refuse déjà l'image par tag, et une capacité admise sur un
    /// tag serait admise sur ce que le tag désignera demain.
    #[must_use]
    pub fn image_digest(&self) -> &str {
        &self.image_digest
    }

    /// La clé qui a signé l'image.
    #[must_use]
    pub fn signing_key(&self) -> &str {
        &self.signing_key
    }

    /// Qui a approuvé.
    #[must_use]
    pub const fn approved_by(&self) -> &Author {
        &self.approved_by
    }

    /// La règle de politique qui a rendu `allow`.
    #[must_use]
    pub fn rule(&self) -> &str {
        &self.rule
    }
}

/// Faire entrer une capacité nouvelle.
///
/// Quatre conditions, dans cet ordre : l'extension est-elle gouvernée, la politique a-t-elle rendu
/// `allow`, l'approbateur est-il quelqu'un d'autre que le demandeur, et l'image **a-t-elle
/// démontré** la capacité.
///
/// # Ce que « démontré » veut dire
///
/// `Published` garantit que **toutes** les vérifications de santé sont passées : `Tested::tested`
/// refuse un échec, et refuse séparément une vérification qu'on n'a pas su lancer. La question qui
/// reste est donc seulement *laquelle* a été faite, et la capacité doit être **nommée** par l'une
/// d'elles. Une capacité qu'aucune vérification ne nomme n'a pas été démontrée par cette image ; la
/// laisser passer parce que l'image est signée confondrait la provenance avec l'aptitude.
///
/// La comparaison est exacte. Un rapprochement par préfixe admettrait `sparql-write` sur la foi
/// d'une vérification nommée `sparql`.
///
/// # Errors
///
/// [`ExtensionForbidden`](AdmissionError::ExtensionForbidden),
/// [`PolicyDidNotAllow`](AdmissionError::PolicyDidNotAllow),
/// [`SelfApproval`](AdmissionError::SelfApproval) et
/// [`NotDemonstrated`](AdmissionError::NotDemonstrated). Chacune nomme **laquelle** des conditions
/// manque : un refus qui dirait « non » enverrait relire quatre politiques à la main, et la réponse
/// « l'image ne l'a pas démontrée » n'appelle pas du tout la même suite que « le déploiement
/// n'admet aucune capacité nouvelle ».
pub fn admit(
    extension: Extension,
    allowed: &Outcome,
    requester: &Author,
    approver: &Author,
    capability: &Capability,
    published: &Published,
) -> Result<Admission, AdmissionError> {
    if extension != Extension::Governed {
        return Err(AdmissionError::ExtensionForbidden);
    }
    let rule = match allowed {
        Outcome::Decided {
            verb: Verb::Allow,
            by,
        } => by.clone(),
        _ => return Err(AdmissionError::PolicyDidNotAllow),
    };
    if requester.is(approver) {
        return Err(AdmissionError::SelfApproval {
            author: approver.to_string(),
        });
    }
    if !demonstrates(published.health(), capability) {
        return Err(AdmissionError::NotDemonstrated {
            capability: capability.to_string(),
            checks: published
                .health()
                .iter()
                .map(|result| result.name.clone())
                .collect(),
        });
    }
    Ok(Admission {
        capability: capability.clone(),
        image_digest: published.image().digest().to_owned(),
        signing_key: published.signature().key_id.clone(),
        approved_by: approver.clone(),
        rule,
    })
}

/// Vrai quand une vérification de santé porte exactement le nom de cette capacité.
fn demonstrates(health: &[HealthResult], capability: &Capability) -> bool {
    health
        .iter()
        .any(|result| result.name == capability.as_str())
}

/// Ce qui empêche une capacité d'entrer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionError {
    /// Ce déploiement n'admet aucune capacité nouvelle.
    ExtensionForbidden,
    /// Le moteur de politique n'a pas rendu `allow`.
    PolicyDidNotAllow,
    /// Le demandeur est l'approbateur.
    ///
    /// §20.3 `forbid_self_approval`, que l'ADR 0016 range parmi les trois bornes qui ne se relâchent
    /// dans aucun mode. Ici elle empêche un agent d'élargir seul ce qu'il a le droit de faire, ce
    /// qui est la forme la plus directe du problème que la littérature nomme « agent
    /// auto-modifiant ».
    SelfApproval {
        /// Qui.
        author: String,
    },
    /// Aucune vérification de santé de l'image ne porte le nom de cette capacité.
    NotDemonstrated {
        /// Laquelle.
        capability: String,
        /// Ce que l'image a effectivement démontré — pour que le refus dise quoi corriger.
        checks: Vec<String>,
    },
}

impl fmt::Display for AdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExtensionForbidden => formatter.write_str(
                "ce déploiement n'admet aucune capacité nouvelle : l'extension est `forbidden`",
            ),
            Self::PolicyDidNotAllow => formatter.write_str(
                "le moteur de politique n'a pas rendu `allow`, et rien d'autre n'est une autorisation",
            ),
            Self::SelfApproval { author } => write!(
                formatter,
                "{author} demande et approuve : `forbid_self_approval` ne se relâche dans aucun mode"
            ),
            Self::NotDemonstrated { capability, checks } => write!(
                formatter,
                "aucune vérification de l'image ne porte le nom `{capability}` ; elle a démontré : {}",
                checks.join(", ")
            ),
        }
    }
}

impl std::error::Error for AdmissionError {}
