//! Le domaine accepte exactement les hashes que le vocabulaire LEP accepte.
//!
//! **Ce test vit ici et non dans `packages/domain`** : il lit un fichier, et la première frontière
//! vérifiée par la CI interdit au domaine de toucher au système de fichiers — jusque dans ses
//! tests, ce qui est correct et l'a rappelé à mi-sprint. Le paquet des artefacts, lui, dépend
//! déjà du domaine et lit déjà les fixtures.
//!
//! Découvert en W6.b : `ContentHash` connaissait `sha256` et `sha512`, le vocabulaire en déclare
//! **trois**. Un manifeste conforme hashé en `blake3` était donc refusé par le seul pair censé le
//! comprendre — et un refus de lecture ressemble en tout point à un document invalide, donc
//! personne n'aurait cherché du côté de la table.
//!
//! Ce test lit le schéma, pas une copie du schéma. Une liste recopiée dans un test vérifie que le
//! code est d'accord avec le test, ce qui est vrai par construction et ne dit rien.

use std::{fs, path::PathBuf};

use locus_domain::{ContentHash, ParseHashError};
use serde_json::Value;

/// Les patrons `^algorithme:[0-9a-f]{N}$` que le vocabulaire déclare, lus tels quels.
fn vocabulary_algorithms() -> Vec<(String, usize)> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/lep/1.0/vocabulary.schema.json");
    let raw = fs::read_to_string(path).expect("le vocabulaire LEP est lisible");
    let document: Value = serde_json::from_str(&raw).expect("vocabulaire en JSON valide");
    let variants = document["definitions"]["content_hash"]["oneOf"]
        .as_array()
        .expect("content_hash est une alternative de patrons");
    variants
        .iter()
        .map(|variant| {
            let pattern = variant["pattern"].as_str().expect("un patron par variante");
            let body = pattern
                .trim_start_matches('^')
                .trim_end_matches('$')
                .to_owned();
            let (algorithm, rest) = body.split_once(':').expect("patron `algorithme:digest`");
            let length: usize = rest
                .rsplit_once('{')
                .and_then(|(_, count)| count.trim_end_matches('}').parse().ok())
                .expect("une longueur explicite");
            (algorithm.to_owned(), length)
        })
        .collect()
}

#[test]
fn tout_hash_que_le_vocabulaire_accepte_se_lit() {
    let algorithms = vocabulary_algorithms();
    assert!(
        algorithms.len() >= 3,
        "le vocabulaire déclare au moins sha256, sha512 et blake3 : {algorithms:?}"
    );
    for (algorithm, length) in algorithms {
        let text = format!("{algorithm}:{}", "a".repeat(length));
        let parsed = ContentHash::parse(&text);
        assert!(
            parsed.is_ok(),
            "« {algorithm} » est dans le vocabulaire et le domaine le refuse : {parsed:?}"
        );
        let parsed = parsed.expect("vérifié juste au-dessus");
        assert_eq!(parsed.algorithm(), algorithm);
        assert_eq!(parsed.digest().len(), length);
    }
}

#[test]
fn un_algorithme_hors_vocabulaire_reste_refuse() {
    let known: Vec<String> = vocabulary_algorithms()
        .into_iter()
        .map(|(algorithm, _)| algorithm)
        .collect();
    for invented in ["md5", "sha1", "crc32"] {
        assert!(
            !known.iter().any(|algorithm| algorithm == invented),
            "« {invented} » ne devrait pas être dans le vocabulaire"
        );
        assert_eq!(
            ContentHash::parse(&format!("{invented}:{}", "a".repeat(64))),
            Err(ParseHashError::UnknownAlgorithm),
            "élargir la table ne doit pas la vider"
        );
    }
}

#[test]
fn la_longueur_reste_verifiee_par_algorithme() {
    // Un `blake3` de 128 caractères a la longueur d'un `sha512` : sans vérification par
    // algorithme, la table élargie accepterait n'importe quel digest de n'importe quelle longueur
    // connue, ce qui est précisément l'inverse de ce qu'elle sert à garantir.
    assert_eq!(
        ContentHash::parse(&format!("blake3:{}", "a".repeat(128))),
        Err(ParseHashError::WrongDigestLength)
    );
    assert_eq!(
        ContentHash::parse(&format!("sha512:{}", "a".repeat(64))),
        Err(ParseHashError::WrongDigestLength)
    );
}
