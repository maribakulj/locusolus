//! La forme canonique traverse la frontière — W9.d.
//!
//! `apps/web` reconstruit la même forme canonique en TypeScript, et refuse un document dont le
//! condensat ne correspond pas. Les deux implémentations ne se lisent pas : elles se rencontrent
//! sur la fixture de ce répertoire, que chacune reproduit depuis le document.
//!
//! Une bibliothèque partagée serait d'accord avec elle-même même en ayant tort. Deux
//! implémentations et une fixture commune sont la configuration où l'accord dit quelque chose.

use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;

use locus_domain::ContentHash;
use locus_visualization::{Digest, View, ViewEdge, ViewKind, ViewNode};

struct Capturing(RefCell<String>);

impl Digest for Capturing {
    fn digest(&self, canonical: &str) -> ContentHash {
        self.0.replace(canonical.to_owned());
        ContentHash::parse(&format!("sha256:{}", "ab".repeat(32))).expect("hash bien formé")
    }
}

fn fixture(name: &str) -> String {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", name]
        .iter()
        .collect();
    fs::read_to_string(&path).unwrap_or_else(|_| panic!("fixture lisible : {}", path.display()))
}

fn node(id: &str, kind: &str, label: &str) -> ViewNode {
    ViewNode {
        id: id.to_owned(),
        kind: kind.to_owned(),
        label: label.to_owned(),
    }
}

fn edge(from: &str, to: &str, kind: &str) -> ViewEdge {
    ViewEdge {
        from: from.to_owned(),
        to: to.to_owned(),
        kind: kind.to_owned(),
    }
}

/// La fixture que `tests/web/view.test.ts` reproduit de son côté. Si l'une des deux formes change,
/// ce test tombe ici et l'autre tombe là-bas — jamais l'un sans l'autre en silence.
#[test]
fn la_forme_canonique_est_celle_de_la_fixture_partagee() {
    let capturing = Capturing(RefCell::new(String::new()));
    View::render(
        ViewKind::ArgumentMap,
        128,
        vec![
            node("claim-a", "claim", "Le lemme 3 tient sans compacité"),
            node("claim-b", "claim", "La borne est atteinte en dimension 2"),
            node("art-1", "artifact", "Preuve Lean, révision 4"),
        ],
        vec![
            edge("art-1", "claim-a", "supports"),
            edge("claim-b", "claim-a", "refutes"),
        ],
        &capturing,
    )
    .expect("vue valide");

    assert_eq!(
        capturing.0.borrow().as_str(),
        fixture("argument-map.canonical.txt"),
        "la forme canonique de Rust a changé sans que la fixture partagée bouge"
    );
}
