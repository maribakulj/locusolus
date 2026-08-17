//! Le hash de contenu — `docs/SPEC_V1.md` §7.7.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

/// Les algorithmes acceptés, avec la longueur de digest qui les identifie.
const ALGORITHMS: [(&str, usize); 2] = [("sha256", 64), ("sha512", 128)];

/// Un hash de contenu, préfixé par son algorithme.
///
/// §7.7 : « les hashes portent sur une canonicalisation stable ». Ce type ne calcule rien — le
/// domaine ne choisit pas d'implémentation de hash, ce serait une décision d'infrastructure — il
/// **vérifie la forme** de ce qu'on lui donne.
///
/// Le préfixe est obligatoire pour la raison que le vocabulaire des schémas énonce déjà : un hash
/// nu ne dit pas comment le recalculer, et une vérification d'intégrité qui devine son algorithme
/// n'en est pas une. La longueur est vérifiée **par algorithme** plutôt que par une borne commode :
/// un digest tronqué est la forme que prend une intégrité cassée, et il ressemble en tout point à
/// un digest valide tant que personne ne compte.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ContentHash {
    algorithm: String,
    digest: String,
}

impl ContentHash {
    /// Lit un hash canonique `algorithme:digest`.
    ///
    /// # Errors
    ///
    /// Rend [`ParseHashError`] si le préfixe manque, si l'algorithme est inconnu, si la longueur
    /// ne correspond pas à celle de l'algorithme, ou si le digest n'est pas en hexadécimal
    /// minuscule. La casse **n'est pas normalisée** : deux écritures d'un même hash produiraient
    /// deux formes canoniques, donc deux signatures, et §7.7 dit qu'il n'y en a qu'une.
    pub fn parse(text: &str) -> Result<Self, ParseHashError> {
        let Some((algorithm, digest)) = text.split_once(':') else {
            return Err(ParseHashError::MissingAlgorithm);
        };
        let Some((_, expected)) = ALGORITHMS.iter().find(|(name, _)| *name == algorithm) else {
            return Err(ParseHashError::UnknownAlgorithm);
        };
        if digest.len() != *expected {
            return Err(ParseHashError::WrongDigestLength);
        }
        if !digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(ParseHashError::NotLowercaseHex);
        }
        Ok(Self {
            algorithm: algorithm.to_owned(),
            digest: digest.to_owned(),
        })
    }

    /// L'algorithme, sans le séparateur.
    #[must_use]
    pub fn algorithm(&self) -> &str {
        &self.algorithm
    }

    /// Le digest, sans le préfixe.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.algorithm, self.digest)
    }
}

impl fmt::Debug for ContentHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self}")
    }
}

impl Serialize for ContentHash {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ContentHash {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = <&str>::deserialize(deserializer)?;
        Self::parse(text).map_err(D::Error::custom)
    }
}

/// Ce qui peut empêcher de lire un hash canonique.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseHashError {
    /// Aucun `:` : le hash ne dit pas comment le recalculer.
    MissingAlgorithm,
    /// Un algorithme que ce crate ne connaît pas.
    UnknownAlgorithm,
    /// La longueur ne correspond pas à celle de l'algorithme annoncé.
    WrongDigestLength,
    /// Le digest n'est pas en hexadécimal minuscule.
    NotLowercaseHex,
}

impl fmt::Display for ParseHashError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::MissingAlgorithm => "hash sans préfixe d'algorithme",
            Self::UnknownAlgorithm => "algorithme de hash inconnu",
            Self::WrongDigestLength => "digest de longueur incorrecte pour cet algorithme",
            Self::NotLowercaseHex => "digest non hexadécimal minuscule",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ParseHashError {}
