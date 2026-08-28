//! La forme canonique d'un document JSON — §7.7, `W20.ac`.
//!
//! Ces tests-ci sont **purs** : aucun ne lit de fichier, parce que le domaine n'en lit pas, pas même
//! en test. L'accord avec la moitié TypeScript du fil se vérifie ailleurs, sur un corpus partagé —
//! `packages/lep/tests/canonical_corpus.rs`, depuis le crate des documents du fil.

use locus_domain::{CanonicalError, canonical_hash, canonical_json};
use serde_json::json;

#[test]
fn les_cles_sont_triees_et_rien_ne_separe() {
    let document = json!({ "b": 1, "a": 2, "A": 3 });
    assert_eq!(
        canonical_json(&document).expect("un document d'entiers a une forme canonique"),
        r#"{"A":3,"a":2,"b":1}"#
    );
}

/// Le tri est sur les **unités de code UTF-16**, pas sur les octets UTF-8.
///
/// Les deux ordres coïncident sur l'ASCII, et divergent au-delà du plan multilingue de base : un
/// caractère hors BMP s'écrit en UTF-16 par une paire de substituts qui commence en `0xD800`, donc
/// **avant** `\u{FFFD}`, alors qu'en UTF-8 il commence par `0xF0`, donc **après** le `0xEF` de ce
/// même `\u{FFFD}`. Un tri par octets inverserait ces deux clés, et l'empreinte de tout document qui
/// les porte toutes les deux — sans que rien ne le signale, puisque les deux formes sont du JSON
/// parfaitement valide.
#[test]
fn le_tri_suit_utf16_et_non_les_octets() {
    let document = json!({ "\u{FFFD}": 1, "\u{1F600}": 2 });
    let canonique = canonical_json(&document).expect("deux clés et deux entiers");
    let hors_bmp = canonique.find('\u{1F600}').expect("la clé hors BMP est là");
    let remplacement = canonique.find('\u{FFFD}').expect("la clé BMP est là");
    assert!(
        hors_bmp < remplacement,
        "en UTF-16 la paire de substituts passe avant U+FFFD ; ici : {canonique}"
    );
}

#[test]
fn un_flottant_est_refuse_plutot_que_rendu_de_travers() {
    let refus = canonical_json(&json!({ "t": 1.5 })).expect_err("1.5 n'a pas d'écriture commune");
    assert_eq!(
        refus,
        CanonicalError::NonIntegerNumber {
            rendered: "1.5".to_owned()
        }
    );
}

/// `4.0` **est** un flottant pour `serde_json`, et JavaScript l'écrirait `4`.
///
/// C'est le cas qui a motivé la canonicalisation en `W0.8` : la fixture écrit `4`, le SDK Rust
/// réémet `4.0`, et deux pairs conformes produisent deux empreintes. Le refuser est la seule réponse
/// que ce module peut donner sans reproduire la sérialisation d'ECMAScript ; l'accepter en écrivant
/// `4` supposerait que tout flottant entier vaut son entier, ce qui est vrai ici et faux dès
/// `1e21`.
#[test]
fn un_entier_ecrit_en_flottant_est_refuse_aussi() {
    let refus = canonical_json(&json!({ "cpu": 4.0 })).expect_err("4.0 n'est pas un entier lu");
    assert!(matches!(refus, CanonicalError::NonIntegerNumber { .. }));
}

#[test]
fn un_entier_hors_de_la_plage_exacte_est_refuse() {
    let brut = format!(r#"{{"n":{}}}"#, i64::MAX);
    let document: serde_json::Value = serde_json::from_str(&brut).expect("du JSON valide");
    let refus = canonical_json(&document).expect_err("au-delà de 2^53-1, le pair d'en face dérive");
    assert_eq!(
        refus,
        CanonicalError::UnsafeInteger {
            rendered: i64::MAX.to_string()
        }
    );
}

/// `i64::MIN` n'a pas d'opposé représentable : une garde écrite avec `abs` paniquerait dessus.
#[test]
fn la_borne_negative_refuse_sans_paniquer() {
    let brut = format!(r#"{{"n":{}}}"#, i64::MIN);
    let document: serde_json::Value = serde_json::from_str(&brut).expect("du JSON valide");
    assert!(matches!(
        canonical_json(&document),
        Err(CanonicalError::UnsafeInteger { .. })
    ));
}

#[test]
fn les_bornes_exactes_passent() {
    let brut = format!(
        r#"{{"haut":{},"bas":{}}}"#,
        9_007_199_254_740_991_i64, -9_007_199_254_740_991_i64
    );
    let document: serde_json::Value = serde_json::from_str(&brut).expect("du JSON valide");
    assert_eq!(
        canonical_json(&document).expect("2^53-1 survit à un aller-retour en double"),
        r#"{"bas":-9007199254740991,"haut":9007199254740991}"#
    );
}

#[test]
fn les_tableaux_gardent_leur_ordre() {
    let document = json!(["b", "a", { "z": 1, "y": 2 }]);
    assert_eq!(
        canonical_json(&document).expect("des chaînes et des entiers"),
        r#"["b","a",{"y":2,"z":1}]"#
    );
}

/// L'empreinte porte sur la forme canonique, et le préfixe dit comment la recalculer.
#[test]
fn l_empreinte_porte_sur_la_forme_canonique() {
    let document = json!({ "b": 1, "a": 2 });
    let empreinte = canonical_hash(&document).expect("un document canonicalisable");
    let attendu = locus_domain::ContentHash::of(r#"{"a":2,"b":1}"#.as_bytes());
    assert_eq!(empreinte, attendu);
    assert_eq!(empreinte.algorithm(), "sha256");
}

/// Deux documents que seul l'ordre d'écriture sépare ont la même empreinte.
#[test]
fn l_ordre_d_ecriture_ne_change_pas_l_empreinte() {
    let gauche: serde_json::Value = serde_json::from_str(r#"{"a":1,"b":2}"#).expect("JSON valide");
    let droite: serde_json::Value = serde_json::from_str(r#"{"b":2,"a":1}"#).expect("JSON valide");
    assert_eq!(
        canonical_hash(&gauche).expect("canonicalisable"),
        canonical_hash(&droite).expect("canonicalisable")
    );
}

#[test]
fn les_chaines_sont_echappees_comme_json_les_echappe() {
    let document = json!({ "t": "un \"guillemet\", une \\ et un \n" });
    assert_eq!(
        canonical_json(&document).expect("des chaînes seulement"),
        r#"{"t":"un \"guillemet\", une \\ et un \n"}"#
    );
}

#[test]
fn null_et_les_booleens_ont_leur_forme() {
    let document = json!({ "n": null, "v": true, "f": false });
    assert_eq!(
        canonical_json(&document).expect("ni nombre ni chaîne"),
        r#"{"f":false,"n":null,"v":true}"#
    );
}
