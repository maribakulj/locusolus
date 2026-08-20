//! Le hash de contenu — `docs/SPEC_V1.md` §7.7.

use std::fmt;
use std::fmt::Write as _;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

/// Les algorithmes acceptés, avec la longueur de digest qui les identifie.
///
/// **Exactement ceux du vocabulaire LEP**, et `packages/artifacts/tests/hash_vocabulary.rs` le
/// vérifie contre le schéma lui-même plutôt que contre une copie — depuis là-bas, parce que le
/// domaine ne lit pas de fichiers, pas même en test. Le domaine avait deux entrées et le vocabulaire trois : un manifeste
/// parfaitement conforme, hashé en `blake3`, était refusé à la lecture par le seul pair qui devait
/// le comprendre. Une table plus étroite que le contrat ne se voit pas — elle ressemble à un
/// document invalide.
const ALGORITHMS: [(&str, usize); 3] = [("sha256", 64), ("sha512", 128), ("blake3", 64)];

/// Un hash de contenu, préfixé par son algorithme.
///
/// §7.7 : « les hashes portent sur une canonicalisation stable ». Ce type **vérifie la forme** de ce
/// qu'on lui donne, et depuis l'ADR 0020 il sait aussi en **calculer** un.
///
/// # Ce que l'ADR 0020 a changé, et pourquoi la phrase d'avant était fausse
///
/// Ce commentaire disait « ce type ne calcule rien — le domaine ne choisit pas d'implémentation de
/// hash, ce serait une décision d'infrastructure ». La prudence était juste, la conclusion non : le
/// résultat était que **rien** ne calculait de condensat, nulle part. `Digest` était un trait déclaré
/// deux fois sans implémentation de production, et les deux seuls écrivains d'événements du dépôt
/// recevaient leur `payload_hash` de l'appelant. §10.1 exige ce champ ; le système ne savait pas le
/// produire, et une chaîne de la bonne forme passait sans que rien ne s'en aperçoive.
///
/// Choisir un algorithme **est** une décision, et c'est pourquoi elle a un ADR. Ne pas la prendre
/// n'était pas la neutralité, c'était l'absence.
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

    /// Le condensat SHA-256 de ces octets — ADR 0020.
    ///
    /// Elle prend des **octets** et ne canonicalise rien. La forme canonique appartient à l'appelant,
    /// et `coordination::version` en a déjà une, écrite et gelée par un test de fixture ; la refaire
    /// ici ferait dépendre l'identité d'une version d'un détail de cette fonction.
    ///
    /// SHA-256 et pas un autre : c'est ce qu'un registre OCI rend, donc ce qu'il faut savoir
    /// recalculer pour vérifier un digest d'image. [`ContentHash::parse`] en accepte trois — lire
    /// plus large qu'on n'écrit est la bonne dissymétrie, elle laisse entrer les condensats des
    /// autres.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        use sha2::{Digest as _, Sha256};

        let mut digest = String::with_capacity(64);
        for byte in Sha256::digest(bytes) {
            let _ = write!(digest, "{byte:02x}");
        }
        Self {
            algorithm: "sha256".to_owned(),
            digest,
        }
    }

    /// Ces octets ont-ils **ce** condensat ?
    ///
    /// Trois réponses, jamais deux — la discipline que ce dépôt applique déjà aux verdicts :
    ///
    /// - `Some(true)` — ils correspondent ;
    /// - `Some(false)` — ils ne correspondent pas, et c'est une intégrité cassée ;
    /// - `None` — **on ne sait pas vérifier** cet algorithme. Ce n'est pas un échec de
    ///   vérification, c'est une absence de vérification, et les confondre transformerait un
    ///   condensat `sha512` parfaitement valide en alerte d'intégrité.
    ///
    /// Sans cette fonction, un appelant écrirait `*self == ContentHash::of(bytes)`, qui rend
    /// silencieusement `false` pour tout condensat qui n'est pas en sha256 — la faute exacte que la
    /// troisième réponse existe pour empêcher.
    #[must_use]
    pub fn matches(&self, bytes: &[u8]) -> Option<bool> {
        (self.algorithm == "sha256").then(|| self.digest == Self::of(bytes).digest)
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
