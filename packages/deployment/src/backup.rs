//! Sauvegarde cohérente et restauration ailleurs — `docs/SPEC_V1.md` §27.4 et §27.5.
//!
//! # Ce que « cohérente » veut dire, et pourquoi ça ne se déclare pas
//!
//! §27.4 : « une sauvegarde cohérente comprend PostgreSQL/event store, artefacts promus, refs Git,
//! configuration non secrète, métadonnées de version et clés selon procédure. »
//!
//! Cinq parties obligatoires, donc, et les clés à part — le texte les subordonne à une procédure
//! plutôt qu'à la liste. [`Backup::coherence`] les confronte à ce que la sauvegarde contient
//! **réellement** ; il n'existe aucun champ « cohérente » qu'un producteur pourrait cocher. Une
//! sauvegarde qui se déclarerait complète est exactement ce qu'on découvre le jour où on la
//! restaure, c'est-à-dire le seul jour où c'est trop tard.
//!
//! # Les clés : incluses ou non, mais jamais sans le dire
//!
//! « Selon procédure » n'autorise pas le silence. Une sauvegarde d'où les clés sont absentes **sans
//! qu'on sache pourquoi** est indiscernable d'une sauvegarde où on les a oubliées, et les deux se
//! restaurent pareil : mal. [`KeyHandling`] force donc à nommer la procédure dans les deux sens.
//!
//! # Les sandboxes ne sont pas des sauvegardes
//!
//! §27.4, dernière phrase : « les sandboxes temporaires ne sont pas des sauvegardes canoniques. »
//! Le refus est explicite plutôt qu'implicite, parce que quelqu'un essaiera de les inclure en
//! croyant bien faire — et une sauvegarde qui porte l'état d'une sandbox invite à la restaurer,
//! donc à traiter un état jetable comme une source.
//!
//! # Restaurer ailleurs : déclarer, pas rejouer
//!
//! §27.5 : « une campagne exportée doit pouvoir être restaurée sur un backend différent, **sous
//! réserve des capabilities requises par ses runs historiques**. » La réserve est la moitié qui
//! compte. Restaurer sur un hôte qui n'a pas ce que les runs exigeaient produirait une campagne
//! qu'on croit intacte et qui ne se rejoue pas — et l'écart ne se verrait qu'à la première
//! reproduction.

use std::collections::BTreeSet;
use std::fmt;

/// Les cinq parties qu'une sauvegarde cohérente doit contenir — §27.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BackupPart {
    /// Le journal d'événements et la base transactionnelle.
    EventStore,
    /// Les artefacts promus — les autres se reconstruisent.
    PromotedArtifacts,
    /// Les refs Git.
    GitRefs,
    /// La configuration, sans les secrets.
    NonSecretConfig,
    /// De quelle version vient cette sauvegarde.
    VersionMetadata,
}

impl BackupPart {
    /// Les cinq, dans l'ordre où §27.4 les nomme.
    pub const ALL: [Self; 5] = [
        Self::EventStore,
        Self::PromotedArtifacts,
        Self::GitRefs,
        Self::NonSecretConfig,
        Self::VersionMetadata,
    ];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::EventStore => "event-store",
            Self::PromotedArtifacts => "promoted-artifacts",
            Self::GitRefs => "git-refs",
            Self::NonSecretConfig => "non-secret-config",
            Self::VersionMetadata => "version-metadata",
        }
    }

    /// Le relire.
    #[must_use]
    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|part| part.slug() == slug)
    }
}

impl fmt::Display for BackupPart {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Ce que la sauvegarde a fait des clés — §27.4, « selon procédure ».
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyHandling {
    /// Incluses, selon la procédure nommée.
    Included {
        /// Laquelle.
        procedure: String,
    },
    /// Délibérément exclues, selon la procédure nommée.
    Excluded {
        /// Laquelle.
        procedure: String,
    },
}

impl KeyHandling {
    /// La procédure suivie, dans les deux cas.
    #[must_use]
    pub fn procedure(&self) -> &str {
        match self {
            Self::Included { procedure } | Self::Excluded { procedure } => procedure,
        }
    }
}

/// Une sauvegarde, telle qu'elle a été prise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backup {
    parts: BTreeSet<BackupPart>,
    keys: KeyHandling,
    required_capabilities: Option<BTreeSet<String>>,
}

impl Backup {
    /// Décrire une sauvegarde à partir des noms de ce qu'elle contient.
    ///
    /// # Errors
    ///
    /// [`BackupError::NotCanonical`] pour une sandbox — §27.4 : « les sandboxes temporaires ne sont
    /// pas des sauvegardes canoniques », et le refus est explicite parce que quelqu'un essaiera de
    /// les inclure en croyant bien faire. [`BackupError::UnknownPart`] pour un nom que §27.4 ne
    /// donne pas, et [`BackupError::EmptyField`] pour une procédure de clés non nommée : « selon
    /// procédure » n'autorise pas le silence.
    pub fn taken(parts: &[&str], keys: KeyHandling) -> Result<Self, BackupError> {
        if keys.procedure().trim().is_empty() {
            return Err(BackupError::EmptyField {
                field: "keys.procedure",
            });
        }
        let mut contained = BTreeSet::new();
        for part in parts {
            if part.starts_with("sandbox") {
                return Err(BackupError::NotCanonical {
                    part: (*part).to_owned(),
                });
            }
            let known = BackupPart::from_slug(part).ok_or_else(|| BackupError::UnknownPart {
                part: (*part).to_owned(),
            })?;
            contained.insert(known);
        }
        Ok(Self {
            parts: contained,
            keys,
            required_capabilities: None,
        })
    }

    /// Consigner ce que les runs historiques exigeaient.
    ///
    /// Sans cet appel, l'exigence reste **inconnue** — pas « aucune ». Une sauvegarde qui n'a pas
    /// relevé ce dont ses runs avaient besoin ne permet pas de dire qu'elle se restaure quelque
    /// part.
    #[must_use]
    pub fn requiring(mut self, capabilities: &[&str]) -> Self {
        self.required_capabilities = Some(
            capabilities
                .iter()
                .map(|capability| (*capability).to_owned())
                .collect(),
        );
        self
    }

    /// Ce que la sauvegarde contient.
    #[must_use]
    pub const fn parts(&self) -> &BTreeSet<BackupPart> {
        &self.parts
    }

    /// Ce qu'elle a fait des clés.
    #[must_use]
    pub const fn keys(&self) -> &KeyHandling {
        &self.keys
    }

    /// Est-elle cohérente au sens de §27.4 ?
    ///
    /// Calculé, jamais déclaré : il n'existe aucun champ qu'un producteur pourrait cocher. Une
    /// sauvegarde qui se dirait complète le serait jusqu'au jour où on la restaure.
    #[must_use]
    pub fn coherence(&self) -> Coherence {
        let missing: BTreeSet<BackupPart> = BackupPart::ALL
            .into_iter()
            .filter(|part| !self.parts.contains(part))
            .collect();
        if missing.is_empty() {
            Coherence::Coherent
        } else {
            Coherence::Incomplete { missing }
        }
    }

    /// Peut-elle être restaurée sur un hôte offrant `available` ?
    ///
    /// §27.5 : « sous réserve des capabilities requises par ses runs historiques ». La réserve est
    /// la moitié qui compte : restaurer sur un hôte qui n'a pas ce que les runs exigeaient
    /// produirait une campagne qu'on croit intacte et qui ne se rejoue pas, et l'écart ne se verrait
    /// qu'à la première reproduction.
    #[must_use]
    pub fn restorable_on(&self, available: &BTreeSet<String>) -> Restorability {
        if let Coherence::Incomplete { missing } = self.coherence() {
            return Restorability::Incoherent { missing };
        }
        let Some(required) = &self.required_capabilities else {
            return Restorability::RequirementsUnknown;
        };
        let missing: BTreeSet<String> = required.difference(available).cloned().collect();
        if missing.is_empty() {
            Restorability::Ready
        } else {
            Restorability::MissingCapabilities { missing }
        }
    }
}

/// Ce que §27.4 dit de cette sauvegarde.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Coherence {
    /// Les cinq parties y sont.
    Coherent,
    /// Il en manque.
    Incomplete {
        /// Lesquelles.
        missing: BTreeSet<BackupPart>,
    },
}

/// Ce que §27.5 dit d'une restauration ailleurs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Restorability {
    /// L'hôte offre tout ce que les runs historiques exigeaient.
    Ready,
    /// Il manque des capabilities, nommées.
    MissingCapabilities {
        /// Lesquelles.
        missing: BTreeSet<String>,
    },
    /// La sauvegarde n'a pas relevé ce que ses runs exigeaient.
    ///
    /// Distinct de « rien n'est requis » : personne n'a regardé. Répondre `Ready` ferait passer
    /// cette ignorance pour un feu vert.
    RequirementsUnknown,
    /// La sauvegarde n'est pas cohérente : la question de l'hôte ne se pose pas encore.
    Incoherent {
        /// Ce qui manque à la sauvegarde elle-même.
        missing: BTreeSet<BackupPart>,
    },
}

impl fmt::Display for Restorability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready => formatter.write_str("restaurable"),
            Self::MissingCapabilities { missing } => write!(
                formatter,
                "non restaurable ; l'hôte n'offre pas : {}",
                missing
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::RequirementsUnknown => formatter.write_str(
                "exigences inconnues : la sauvegarde n'a pas relevé ce que ses runs demandaient",
            ),
            Self::Incoherent { missing } => write!(
                formatter,
                "sauvegarde incohérente ; il manque : {}",
                missing
                    .iter()
                    .map(|part| part.slug())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

/// Ce qui empêche une sauvegarde d'être décrite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackupError {
    /// Un champ obligatoire vide.
    EmptyField {
        /// Lequel.
        field: &'static str,
    },
    /// Un nom que §27.4 ne donne pas.
    UnknownPart {
        /// Lequel.
        part: String,
    },
    /// Une sandbox, que §27.4 exclut nommément.
    NotCanonical {
        /// Ce qu'on tentait d'inclure.
        part: String,
    },
}

impl fmt::Display for BackupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(
                formatter,
                "« {field} » est vide : « selon procédure » n'autorise pas le silence"
            ),
            Self::UnknownPart { part } => {
                write!(
                    formatter,
                    "« {part} » n'est pas une partie nommée par §27.4"
                )
            }
            Self::NotCanonical { part } => write!(
                formatter,
                "« {part} » est une sandbox temporaire : §27.4 dit qu'elles ne sont pas des \
                 sauvegardes canoniques, et une sauvegarde qui en porte l'état invite à traiter \
                 du jetable comme une source"
            ),
        }
    }
}

impl std::error::Error for BackupError {}
