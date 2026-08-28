//! Les deux moitiés du fil canonicalisent pareil — §7.7, `W20.ac`.
//!
//! # Pourquoi ce test est ici
//!
//! Il porte sur `locus_domain::canonical_json`, et il ne peut pas vivre dans `packages/domain` : ce
//! crate ne lit aucun fichier, pas même en test, et `hash.rs` explique pourquoi
//! `packages/artifacts` héberge déjà la vérification du vocabulaire de hash pour la même raison.
//!
//! `packages/lep` est le crate des documents du **fil**, c'est-à-dire de ce dont l'empreinte est
//! calculée d'un côté et recalculée de l'autre. C'est donc là que l'accord entre les deux
//! implémentations doit se constater. `locus-domain` y entre en **dev-dependency** : rien du SDK
//! généré n'en dépend, et l'y faire entrer autrement élargirait le graphe de production pour un
//! test.
//!
//! # Ce que le corpus est, et d'où viennent les valeurs attendues
//!
//! `tests/fixtures/canonical.json` porte, pour chaque cas, la valeur, sa forme canonique et son
//! empreinte. Ces deux dernières sont produites par `packages/testing/src/canonical.ts` — la moitié
//! TypeScript, en service depuis `W0.8`, vendorée dans le worker et donc **celle qui vérifiera
//! réellement**. Le corpus enregistre ce que le worker calculera ; ce test vérifie que Rust en dit
//! autant. `tests/testing/canonical.test.ts` tient l'autre bord : que la moitié TypeScript produise
//! encore ce que le corpus dit.
//!
//! Un corpus dont les attendus seraient produits par l'implémentation Rust ne vérifierait rien
//! d'autre que sa propre stabilité.

use std::path::Path;

use locus_domain::{canonical_hash, canonical_json};
use serde_json::Value;

fn corpus() -> Vec<Value> {
    let racine = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("packages/lep a deux ancêtres jusqu'à la racine du dépôt")
        .to_owned();
    let brut = std::fs::read_to_string(racine.join("tests/fixtures/canonical.json"))
        .expect("le corpus partagé est versionné avec le dépôt");
    let lu: Value = serde_json::from_str(&brut).expect("le corpus est du JSON");
    lu.get("cas")
        .and_then(Value::as_array)
        .cloned()
        .expect("le corpus porte une liste « cas »")
}

#[test]
fn les_deux_moities_du_fil_canonicalisent_pareil() {
    let cas = corpus();
    // Un compteur qui n'a rien lu ne vaut pas zéro : un corpus vide passerait toutes les
    // comparaisons et ne prouverait rien.
    assert!(
        cas.len() >= 8,
        "le corpus s'est vidé : {} cas lus, et un corpus vide passe tout",
        cas.len()
    );
    for entree in cas {
        let nom = entree["nom"].as_str().expect("chaque cas porte un nom");
        let valeur = &entree["valeur"];
        let attendu = entree["canonique"]
            .as_str()
            .expect("chaque cas porte sa forme canonique");
        let empreinte = entree["empreinte"]
            .as_str()
            .expect("chaque cas porte son empreinte");
        assert_eq!(
            canonical_json(valeur).expect("le corpus ne porte que des valeurs canonicalisables"),
            attendu,
            "forme canonique divergente sur « {nom} »"
        );
        assert_eq!(
            canonical_hash(valeur)
                .expect("le corpus ne porte que des valeurs canonicalisables")
                .to_string(),
            empreinte,
            "empreinte divergente sur « {nom} »"
        );
    }
}
