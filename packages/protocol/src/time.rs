//! Horodatage UTC, à la milliseconde.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

/// Longueur de la forme canonique `YYYY-MM-DDTHH:MM:SS.sssZ`.
const CANONICAL_LEN: usize = 24;

/// Jours écoulés entre le 1er mars de l'an 0 et le 1er janvier 1970, dans l'algorithme civil.
const EPOCH_SHIFT: i64 = 719_468;

const MILLIS_PER_DAY: i64 = 86_400_000;

/// Un instant UTC, à la milliseconde.
///
/// `docs/SPEC_V1.md` §7.7 : « les timestamps sont en UTC ISO 8601 », et « la présentation locale
/// des dates n'affecte jamais les signatures ni les hashes ». La forme canonique est donc
/// `YYYY-MM-DDTHH:MM:SS.sssZ` — exactement trois décimales, suffixe `Z`, rien d'autre — et
/// [`Timestamp::parse`] refuse toute autre écriture, même valide au sens d'ISO 8601.
///
/// L'ordre naturel est l'ordre chronologique.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(i64);

impl Timestamp {
    /// L'époque Unix, `1970-01-01T00:00:00.000Z`.
    pub const UNIX_EPOCH: Self = Self(0);

    /// Construit un instant depuis un nombre de millisecondes depuis l'époque Unix.
    #[must_use]
    pub const fn from_millis(millis: i64) -> Self {
        Self(millis)
    }

    /// Millisecondes depuis l'époque Unix.
    #[must_use]
    pub const fn millis(self) -> i64 {
        self.0
    }

    /// Lit la forme canonique.
    ///
    /// # Errors
    ///
    /// Rend [`ParseTimestampError`] si l'entrée n'est pas exactement
    /// `YYYY-MM-DDTHH:MM:SS.sssZ`, ou si les champs sont hors bornes. Une seconde intercalaire
    /// (`:60`) est refusée : elle n'a pas de représentation en millisecondes depuis l'époque.
    pub fn parse(text: &str) -> Result<Self, ParseTimestampError> {
        let bytes = text.as_bytes();
        if bytes.len() != CANONICAL_LEN {
            return Err(ParseTimestampError::NotCanonical);
        }
        for (index, expected) in [
            (4, b'-'),
            (7, b'-'),
            (10, b'T'),
            (13, b':'),
            (16, b':'),
            (19, b'.'),
            (23, b'Z'),
        ] {
            if bytes[index] != expected {
                return Err(ParseTimestampError::NotCanonical);
            }
        }
        let year = number(bytes, 0, 4)?;
        let month = number(bytes, 5, 2)?;
        let day = number(bytes, 8, 2)?;
        let hour = number(bytes, 11, 2)?;
        let minute = number(bytes, 14, 2)?;
        let second = number(bytes, 17, 2)?;
        let milli = number(bytes, 20, 3)?;

        if !(1..=12).contains(&month) {
            return Err(ParseTimestampError::OutOfRange("month"));
        }
        if day < 1 || day > days_in_month(year, month) {
            return Err(ParseTimestampError::OutOfRange("day"));
        }
        if hour > 23 {
            return Err(ParseTimestampError::OutOfRange("hour"));
        }
        if minute > 59 {
            return Err(ParseTimestampError::OutOfRange("minute"));
        }
        if second > 59 {
            return Err(ParseTimestampError::OutOfRange("second"));
        }

        let days = days_from_civil(year, month, day);
        let seconds = days * 86_400 + hour * 3_600 + minute * 60 + second;
        Ok(Self(seconds * 1_000 + milli))
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let days = self.0.div_euclid(MILLIS_PER_DAY);
        let rest = self.0.rem_euclid(MILLIS_PER_DAY);
        let (year, month, day) = civil_from_days(days);
        let milli = rest % 1_000;
        let second = (rest / 1_000) % 60;
        let minute = (rest / 60_000) % 60;
        let hour = rest / 3_600_000;
        write!(
            formatter,
            "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{milli:03}Z"
        )
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = <&str>::deserialize(deserializer)?;
        Self::parse(text).map_err(D::Error::custom)
    }
}

/// Ce qui peut empêcher de lire un horodatage canonique.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseTimestampError {
    /// L'entrée n'a pas la forme `YYYY-MM-DDTHH:MM:SS.sssZ`.
    NotCanonical,
    /// Un champ est hors de ses bornes.
    OutOfRange(&'static str),
}

impl fmt::Display for ParseTimestampError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotCanonical => {
                formatter.write_str("horodatage non canonique : attendu YYYY-MM-DDTHH:MM:SS.sssZ")
            }
            Self::OutOfRange(field) => write!(formatter, "champ hors bornes : {field}"),
        }
    }
}

impl std::error::Error for ParseTimestampError {}

fn number(bytes: &[u8], start: usize, width: usize) -> Result<i64, ParseTimestampError> {
    let mut value = 0_i64;
    for &byte in &bytes[start..start + width] {
        if !byte.is_ascii_digit() {
            return Err(ParseTimestampError::NotCanonical);
        }
        value = value * 10 + i64::from(byte - b'0');
    }
    Ok(value)
}

const fn is_leap(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

const fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Jours depuis l'époque Unix pour une date civile. Algorithme de Howard Hinnant.
const fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_shifted = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_shifted + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - EPOCH_SHIFT
}

/// Réciproque de [`days_from_civil`].
const fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + EPOCH_SHIFT;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_shifted = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_shifted + 2) / 5 + 1;
    let month = if month_shifted < 10 {
        month_shifted + 3
    } else {
        month_shifted - 9
    };
    (if month <= 2 { year + 1 } else { year }, month, day)
}
