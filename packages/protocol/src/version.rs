//! Versionnement du protocole et négociation au handshake.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

/// Le nom du protocole, tel qu'il apparaît dans sa forme textuelle.
const NAME: &str = "lep";

/// Une version de LEP, `lep/<majeur>.<mineur>`.
///
/// `docs/06_LEP_PROTOCOL.md` : « Major = rupture ; minor = champs optionnels compatibles ;
/// feature negotiation au handshake ». D'où deux opérations, et deux seulement : savoir si deux
/// pairs peuvent se parler, et savoir à quelle version ils se parleront.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProtocolVersion {
    /// Incrémenté par toute rupture de compatibilité.
    pub major: u16,
    /// Incrémenté par l'ajout de champs optionnels compatibles.
    pub minor: u16,
}

impl ProtocolVersion {
    /// La version que W0 gèle à son terme.
    pub const V1_0: Self = Self::new(1, 0);

    /// Construit une version.
    #[must_use]
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    /// Deux pairs peuvent-ils se parler ?
    ///
    /// Le majeur seul décide : un mineur plus élevé n'apporte que des champs optionnels, qu'un
    /// pair plus ancien ignore sans dommage.
    #[must_use]
    pub const fn speaks_with(self, other: Self) -> bool {
        self.major == other.major
    }

    /// La version à laquelle deux pairs se parleront, ou `None` s'ils ne le peuvent pas.
    ///
    /// C'est le mineur le plus bas : chacun doit comprendre tout ce que l'autre émet, donc
    /// aucun ne peut se permettre les champs que l'autre ignore.
    #[must_use]
    pub const fn negotiate(self, other: Self) -> Option<Self> {
        if !self.speaks_with(other) {
            return None;
        }
        let minor = if self.minor < other.minor {
            self.minor
        } else {
            other.minor
        };
        Some(Self::new(self.major, minor))
    }

    /// Lit la forme canonique `lep/<majeur>.<mineur>`.
    ///
    /// # Errors
    ///
    /// Rend [`ParseVersionError`] si le nom n'est pas `lep`, si la forme n'est pas
    /// `nom/majeur.mineur`, ou si un nombre est absent, non décimal, ou trop grand pour un `u16`.
    pub fn parse(text: &str) -> Result<Self, ParseVersionError> {
        let (name, numbers) = text
            .split_once('/')
            .ok_or(ParseVersionError::NotCanonical)?;
        if name != NAME {
            return Err(ParseVersionError::UnknownProtocol);
        }
        let (major, minor) = numbers
            .split_once('.')
            .ok_or(ParseVersionError::NotCanonical)?;
        Ok(Self::new(component(major)?, component(minor)?))
    }
}

impl fmt::Display for ProtocolVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{NAME}/{}.{}", self.major, self.minor)
    }
}

impl Serialize for ProtocolVersion {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ProtocolVersion {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = <&str>::deserialize(deserializer)?;
        Self::parse(text).map_err(D::Error::custom)
    }
}

/// Ce qui peut empêcher de lire une version canonique.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseVersionError {
    /// L'entrée n'a pas la forme `lep/<majeur>.<mineur>`.
    NotCanonical,
    /// Le nom de protocole n'est pas `lep`.
    UnknownProtocol,
}

impl fmt::Display for ParseVersionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotCanonical => formatter.write_str("version non canonique : attendu lep/M.m"),
            Self::UnknownProtocol => write!(formatter, "protocole inconnu : attendu {NAME}"),
        }
    }
}

impl std::error::Error for ParseVersionError {}

fn component(text: &str) -> Result<u16, ParseVersionError> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ParseVersionError::NotCanonical);
    }
    text.parse().map_err(|_| ParseVersionError::NotCanonical)
}
