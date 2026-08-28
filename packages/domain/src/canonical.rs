//! La forme canonique d'un document JSON — `docs/SPEC_V1.md` §7.7.
//!
//! # Pourquoi ce module existe, et pourquoi il est ici
//!
//! §7.7 : « les hashes portent sur une canonicalisation stable ». [`crate::ContentHash::of`] prend
//! des **octets** et ne canonicalise rien — c'est écrit dans sa documentation, et c'était exact tant
//! que chaque appelant avait sa propre forme canonique et la gelait par une fixture, comme
//! `coordination::version` le fait pour une `Version`.
//!
//! Un document du **fil**, lui, n'a pas ce luxe : il est hashé ici et vérifié ailleurs, dans un
//! autre langage. `packages/testing/src/canonical.ts` tient la moitié TypeScript depuis `W0.8`, et
//! le worker s'en sert pour recalculer l'empreinte d'une `ContextView` avant de démarrer (§12.3).
//! Rien ne tenait la moitié Rust : le seul pair capable de produire une vue ne savait pas produire
//! l'empreinte que le seul pair capable de la vérifier allait recalculer.
//!
//! # Ce qui est implémenté, et ce qui est **refusé**
//!
//! La forme suit RFC 8785 (JCS) sur les trois points qui décident du résultat :
//!
//! - les clés d'objet sont triées par leurs **unités de code UTF-16**, pas par leurs octets UTF-8 —
//!   les deux ordres coïncident sur l'ASCII et divergent au-delà du plan multilingue de base, et
//!   c'est l'ordre UTF-16 que JCS impose et que l'implémentation TypeScript applique ;
//! - aucun espace insignifiant ;
//! - les chaînes sont échappées comme JSON les échappe.
//!
//! Les nombres non entiers sont **refusés** plutôt que rendus de travers, et c'est la différence
//! assumée avec la moitié TypeScript, qui les accepte. Écrire un flottant à l'identique des deux
//! côtés demanderait de reproduire en Rust la sérialisation d'ECMAScript, qui rend `1e+21` là où
//! `{}` rend `1000000000000000000000` : deux pairs conformes, deux empreintes, et un refus
//! d'intégrité sur une vue parfaitement valide. Un canonicaliseur qui rend quelque chose pour une
//! valeur qu'il ne sait pas représenter à l'identique est pire que celui qui s'arrête — le premier
//! produit un hash, et un hash faux ressemble en tout point à un hash juste.
//!
//! Le refus est **bruyant et nommé** : [`CanonicalError::NonIntegerNumber`] dit ce qui n'est pas
//! représentable, et l'appelant décide. Aucun champ numérique d'un document `lep/1.0` n'est
//! flottant ; le jour où l'un le deviendra, c'est cette erreur qui le fera savoir, et non une
//! divergence d'empreinte constatée trois mois plus tard.

use std::fmt;

use serde_json::Value;

use crate::hash::ContentHash;

/// Ce qui empêche un document d'avoir une forme canonique.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalError {
    /// Un nombre que les deux implémentations n'écriraient pas pareil.
    NonIntegerNumber {
        /// Le nombre tel que `serde_json` l'a lu.
        rendered: String,
    },
    /// Un entier hors de la plage où il survit à un aller-retour en `double`.
    ///
    /// La même borne que côté TypeScript, où un entier au-delà de 2^53−1 cesse d'être exact : deux
    /// pairs qui liraient le même document y verraient deux valeurs différentes, donc deux
    /// empreintes, sans que rien n'ait été altéré.
    UnsafeInteger {
        /// Le nombre tel que `serde_json` l'a lu.
        rendered: String,
    },
}

impl fmt::Display for CanonicalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonIntegerNumber { rendered } => write!(
                formatter,
                "« {rendered} » n'est pas un entier : les deux moitiés du fil ne l'écriraient pas \
                 pareil, et deux écritures d'un même nombre font deux empreintes"
            ),
            Self::UnsafeInteger { rendered } => write!(
                formatter,
                "« {rendered} » est hors de la plage où un entier survit à un aller-retour en \
                 double : le pair d'en face n'y lirait pas la même valeur"
            ),
        }
    }
}

impl std::error::Error for CanonicalError {}

/// La borne au-delà de laquelle un entier cesse d'être exact en `double` — 2^53 − 1.
const SAFE_INTEGER: i64 = 9_007_199_254_740_991;

/// La forme canonique de ce document, telle que le pair d'en face la recalculera.
///
/// # Errors
///
/// [`CanonicalError`] pour un nombre dont les deux implémentations ne produiraient pas la même
/// écriture — voir l'en-tête du module.
pub fn canonical_json(value: &Value) -> Result<String, CanonicalError> {
    let mut out = String::new();
    write(value, &mut out)?;
    Ok(out)
}

/// L'empreinte d'un document, sur sa forme canonique.
///
/// C'est la composition que tout appelant écrirait sinon, et la centraliser est ce qui garantit
/// qu'aucun ne hashe la sortie d'un sérialiseur ordinaire par distraction.
///
/// # Errors
///
/// Les mêmes que [`canonical_json`].
pub fn canonical_hash(value: &Value) -> Result<ContentHash, CanonicalError> {
    Ok(ContentHash::of(canonical_json(value)?.as_bytes()))
}

fn write(value: &Value, out: &mut String) -> Result<(), CanonicalError> {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(number) => out.push_str(&integer(number)?),
        Value::String(text) => out.push_str(&escaped(text)),
        Value::Array(items) => {
            out.push('[');
            for (rank, item) in items.iter().enumerate() {
                if rank > 0 {
                    out.push(',');
                }
                write(item, out)?;
            }
            out.push(']');
        }
        Value::Object(members) => {
            // Le tri est **sur les clés lues**, jamais sur l'ordre d'insertion : `serde_json` peut
            // être compilé avec `preserve_order`, et une forme canonique qui dépendrait d'une
            // feature de compilation ne serait pas canonique.
            let mut keys: Vec<&String> = members.keys().collect();
            keys.sort_unstable_by_key(|key| utf16(key));
            out.push('{');
            for (rank, key) in keys.into_iter().enumerate() {
                if rank > 0 {
                    out.push(',');
                }
                out.push_str(&escaped(key));
                out.push(':');
                write(&members[key], out)?;
            }
            out.push('}');
        }
    }
    Ok(())
}

fn integer(number: &serde_json::Number) -> Result<String, CanonicalError> {
    if let Some(signed) = number.as_i64() {
        // `unsigned_abs` et non `abs` : `i64::MIN` n'a pas d'opposé représentable, et une garde qui
        // panique sur la valeur qu'elle est censée refuser ne garde rien.
        if signed.unsigned_abs() > SAFE_INTEGER.unsigned_abs() {
            return Err(CanonicalError::UnsafeInteger {
                rendered: number.to_string(),
            });
        }
        return Ok(signed.to_string());
    }
    if let Some(unsigned) = number.as_u64() {
        if unsigned > SAFE_INTEGER.unsigned_abs() {
            return Err(CanonicalError::UnsafeInteger {
                rendered: number.to_string(),
            });
        }
        return Ok(unsigned.to_string());
    }
    Err(CanonicalError::NonIntegerNumber {
        rendered: number.to_string(),
    })
}

/// Une chaîne JSON échappée.
///
/// Déléguée à `serde_json`, qui échappe déjà comme la RFC le demande — les guillemets, la barre
/// oblique inverse, les commandes en dessous de l'espace en `\u00XX`, et rien d'autre. Réécrire cet
/// échappement ici en donnerait une seconde définition, et c'est exactement le genre de doublon qui
/// diverge sur un caractère que personne n'a en tête.
fn escaped(text: &str) -> String {
    Value::String(text.to_owned()).to_string()
}

/// La suite d'unités de code UTF-16 d'une clé — l'ordre que JCS impose.
fn utf16(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}
