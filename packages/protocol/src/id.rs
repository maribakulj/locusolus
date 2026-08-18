//! Identifiants typés, préfixés par leur nature.

use std::fmt;
use std::marker::PhantomData;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

use crate::time::Timestamp;

/// Alphabet Crockford base32, sans I, L, O ni U.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Longueur du corps : 26 caractères, soit 130 bits pour une valeur de 128.
const BODY_LEN: usize = 26;

/// Le format réserve 48 bits à l'instant : de 1970 à l'an 10889.
const TIMESTAMP_LIMIT: u128 = 1 << 48;

/// La nature d'un identifiant, et le préfixe qui la désigne sur le fil.
pub trait IdKind {
    /// Le préfixe, sans le tiret bas.
    const PREFIX: &'static str;
}

/// Un identifiant global : `<préfixe>_<26 caractères>`.
///
/// `docs/SPEC_V1.md` §7.7 : « tous les identifiants globaux sont des `UUIDv7` ou ULID avec préfixe
/// de type ». Le corps retenu est un ULID, parce que son encodage textuel est trié
/// lexicographiquement dans l'ordre chronologique — propriété dont l'event store se sert
/// directement — et parce que c'est la forme que montrent les exemples du §10.1 (`evt_01…`).
///
/// Le préfixe fait partie de l'identité : `evt_01ARZ…` et `cmd_01ARZ…` ne sont pas le même
/// identifiant, et le type les empêche d'être confondus à la compilation.
pub struct Id<K: IdKind> {
    value: u128,
    kind: PhantomData<fn() -> K>,
}

impl<K: IdKind> Id<K> {
    /// Compose un identifiant depuis un instant et 80 bits d'entropie.
    ///
    /// Ni l'un ni l'autre n'est produit ici : ce crate ne lit pas l'heure et ne tire pas au sort.
    ///
    /// # Errors
    ///
    /// Rend [`ParseIdError::TimestampOutOfRange`] si l'instant ne tient pas sur les 48 bits que
    /// le format réserve — c'est-à-dire avant 1970 ou après l'an 10889.
    pub fn from_parts(instant: Timestamp, entropy: [u8; 10]) -> Result<Self, ParseIdError> {
        let millis =
            u128::try_from(instant.millis()).map_err(|_| ParseIdError::TimestampOutOfRange)?;
        if millis >= TIMESTAMP_LIMIT {
            return Err(ParseIdError::TimestampOutOfRange);
        }
        let mut value = millis << 80;
        for (index, byte) in entropy.iter().enumerate() {
            value |= u128::from(*byte) << (72 - index * 8);
        }
        Ok(Self {
            value,
            kind: PhantomData,
        })
    }

    /// L'instant encodé dans les 48 bits de tête.
    #[must_use]
    pub fn timestamp(self) -> Timestamp {
        // Le masque borne la valeur à 48 bits ; reprendre les huit octets de poids faible est
        // donc une conversion totale, sans troncature possible et sans chemin de panique.
        let millis = (self.value >> 80) & (TIMESTAMP_LIMIT - 1);
        let mut low = [0_u8; 8];
        low.copy_from_slice(&millis.to_be_bytes()[8..]);
        Timestamp::from_millis(i64::from_be_bytes(low))
    }

    /// Lit la forme canonique `<préfixe>_<26 caractères>`.
    ///
    /// # Errors
    ///
    /// Rend [`ParseIdError`] si le préfixe manque ou ne correspond pas, si le corps n'a pas
    /// exactement 26 caractères, s'il porte un caractère hors alphabet — les minuscules
    /// comprises, la forme canonique étant en majuscules — ou s'il déborde de 128 bits.
    pub fn parse(text: &str) -> Result<Self, ParseIdError> {
        let (prefix, body) = text.split_once('_').ok_or(ParseIdError::MissingPrefix)?;
        if prefix != K::PREFIX {
            return Err(ParseIdError::WrongPrefix {
                expected: K::PREFIX,
            });
        }
        let bytes = body.as_bytes();
        if bytes.len() != BODY_LEN {
            return Err(ParseIdError::BodyLength);
        }
        let mut value: u128 = 0;
        for (index, &byte) in bytes.iter().enumerate() {
            let digit = ALPHABET
                .iter()
                .position(|&candidate| candidate == byte)
                .ok_or(ParseIdError::InvalidCharacter)?;
            // 26 × 5 bits valent 130 : le premier caractère n'a droit qu'à trois bits.
            if index == 0 && digit > 7 {
                return Err(ParseIdError::Overflow);
            }
            value = (value << 5) | digit as u128;
        }
        Ok(Self {
            value,
            kind: PhantomData,
        })
    }
}

impl<K: IdKind> fmt::Display for Id<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(K::PREFIX)?;
        formatter.write_str("_")?;
        for index in 0..BODY_LEN {
            let shift = 5 * (BODY_LEN - 1 - index);
            let digit = ((self.value >> shift) & 0x1F) as usize;
            formatter.write_str(
                std::str::from_utf8(&ALPHABET[digit..=digit]).map_err(|_| fmt::Error)?,
            )?;
        }
        Ok(())
    }
}

impl<K: IdKind> fmt::Debug for Id<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self}")
    }
}

impl<K: IdKind> Clone for Id<K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K: IdKind> Copy for Id<K> {}

impl<K: IdKind> PartialEq for Id<K> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<K: IdKind> Eq for Id<K> {}

impl<K: IdKind> PartialOrd for Id<K> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// L'ordre est celui du ULID, donc chronologique puis arbitraire mais stable.
impl<K: IdKind> Ord for Id<K> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.value.cmp(&other.value)
    }
}

impl<K: IdKind> std::hash::Hash for Id<K> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl<K: IdKind> Serialize for Id<K> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de, K: IdKind> Deserialize<'de> for Id<K> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = <&str>::deserialize(deserializer)?;
        Self::parse(text).map_err(D::Error::custom)
    }
}

/// Ce qui peut empêcher de lire un identifiant canonique.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseIdError {
    /// Aucun tiret bas séparateur.
    MissingPrefix,
    /// Le préfixe ne désigne pas cette nature d'identifiant.
    WrongPrefix {
        /// Le préfixe qu'attendait le type visé.
        expected: &'static str,
    },
    /// Le corps ne fait pas 26 caractères.
    BodyLength,
    /// Le corps porte un caractère hors de l'alphabet Crockford majuscule.
    InvalidCharacter,
    /// Le corps déborde des 128 bits du format.
    Overflow,
    /// L'instant ne tient pas sur les 48 bits de tête.
    TimestampOutOfRange,
}

impl fmt::Display for ParseIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPrefix => formatter.write_str("identifiant sans préfixe"),
            Self::WrongPrefix { expected } => write!(formatter, "préfixe attendu : {expected}"),
            Self::BodyLength => write!(formatter, "corps attendu de {BODY_LEN} caractères"),
            Self::InvalidCharacter => {
                formatter.write_str("caractère hors alphabet Crockford majuscule")
            }
            Self::Overflow => formatter.write_str("corps hors des 128 bits du format"),
            Self::TimestampOutOfRange => formatter.write_str("instant hors des 48 bits du format"),
        }
    }
}

impl std::error::Error for ParseIdError {}

macro_rules! id_kinds {
    ($( $(#[$doc:meta])* $name:ident => $prefix:literal ),* $(,)?) => {
        $(
            $(#[$doc])*
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub struct $name;

            impl IdKind for $name {
                const PREFIX: &'static str = $prefix;
            }
        )*
    };
}

id_kinds! {
    /// Un événement du journal (§10.1).
    Event => "evt",
    /// Une commande (§10.1, `causation_id`).
    Command => "cmd",
    /// Un workspace (§6.1).
    Workspace => "ws",
    /// Un projet (§7.1).
    Project => "prj",
    /// Un programme de recherche (§7.1).
    Program => "pgm",
    /// Une branche (§7.1).
    Branch => "br",
    /// Un agent, en tant que principal (§10.1, `principal_id`).
    Agent => "agent",
    /// Une délégation (§10.1, `delegation_id`).
    Delegation => "del",
    /// Un workflow, porteur de la corrélation (§10.1, `correlation_id`).
    Workflow => "wf",
    /// Une assertion épistémique (§10.1, `stream_id`).
    Claim => "claim",
}

/// Natures d'identifiant dont **aucun document ne fixe le préfixe**.
///
/// `docs/SPEC_V1.md` §10.1 et la spec Canterel §11.1 nomment `mission_id` et `error_id` sans
/// jamais en montrer d'exemple, là où les dix natures ci-dessus apparaissent littéralement sous
/// la forme `evt_01…`. Les préfixes retenus ici sont donc **provisoires** : l'enveloppe d'erreur
/// de W0.4 en a besoin, et W0.6 — qui définit `Attempt` et les événements — les confirmera ou
/// les remplacera. Un changement à ce moment-là est une modification de schéma, pas de code.
///
/// W13.c y ajoute `Task`, `Team`, `Decision` et `Approval`, pour la même raison exactement : §7.1
/// nomme les quatre agrégats et §10.1 ne montre aucun exemple de leurs identifiants. Les mettre
/// ici plutôt que parmi les dix fixés est ce qui empêche de croire qu'un document les a arbitrés —
/// c'est W13.e, qui écrira les événements de coordination, qui le fera.
pub mod provisional {
    use super::IdKind;

    id_kinds! {
        /// Une mission, l'enveloppe distribuée (Canterel §11.1). Préfixe provisoire.
        Mission => "msn",
        /// Une erreur structurée (Canterel §26). Préfixe provisoire.
        Error => "err",
        /// Une tâche (§7.1). Préfixe provisoire — W13.c.
        Task => "task",
        /// Une équipe (§7.1, §14.2). Préfixe provisoire — W13.c.
        Team => "team",
        /// Une décision (§7.1, §20). Préfixe provisoire — W13.c.
        Decision => "dec",
        /// Une demande d'approbation humaine (§7.1, §20). Préfixe provisoire — W13.c.
        Approval => "apr",
        /// Un compte de budget (§7.2). Préfixe provisoire — W7.e.
        BudgetAccount => "budg",
        /// Une réservation de budget (§7.1 `budget_reservation_id`, §7.2). Préfixe provisoire — W7.e.
        Reservation => "resv",
    }
}
