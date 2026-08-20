//! Le condensat de contenu — ADR 0020.
//!
//! # Un vecteur connu, et pourquoi rien d'autre ne prouverait quoi que ce soit
//!
//! Une fonction de hachage se teste mal par ses propriétés. « Déterministe », « deux entrées
//! différentes donnent deux sorties différentes », « soixante-quatre hexadécimaux » : le `Fnv` jouet
//! des fixtures de ce dépôt satisfait les trois, et il n'est pas SHA-256.
//!
//! Le seul test qui distingue SHA-256 d'une fonction qui lui ressemble est un **vecteur connu** —
//! une entrée dont le condensat est publié et vérifiable ailleurs. C'est aussi ce qui rend le test
//! utile en cas de mise à jour du crate : une régression y serait visible, là où une propriété
//! resterait verte.

use locus_domain::ContentHash;

/// `sha256("")`, le vecteur le plus reproduit du monde. `sha256sum </dev/null` le rend.
const EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// `sha256("abc")`, vecteur de FIPS 180-2, annexe B.1.
const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

#[test]
fn le_condensat_est_bien_du_sha256() {
    assert_eq!(ContentHash::of(b"").to_string(), format!("sha256:{EMPTY}"));
    assert_eq!(ContentHash::of(b"abc").to_string(), format!("sha256:{ABC}"));
}

/// Ce qu'on calcule doit pouvoir se relire — sans quoi la moitié écriture et la moitié lecture du
/// même type auraient deux idées de ce qu'est un condensat bien formé.
#[test]
fn ce_qui_est_calcule_se_relit() {
    let calcule = ContentHash::of(b"une forme canonique quelconque");
    let relu = ContentHash::parse(&calcule.to_string()).expect("ce qu'on écrit se relit");

    assert_eq!(calcule, relu);
    assert_eq!(calcule.algorithm(), "sha256");
    assert_eq!(calcule.digest().len(), 64);
    assert!(
        calcule
            .digest()
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    );
}

/// **Trois réponses, jamais deux.**
///
/// Un condensat `sha512` parfaitement valide n'est pas une intégrité cassée : c'est une intégrité
/// qu'on ne sait pas vérifier. Les fondre rendrait `false` — donc une alerte — pour un document
/// irréprochable, et c'est la faute que ce dépôt refuse partout ailleurs sous le nom
/// « `unverified` n'est pas un `broken` atténué ».
#[test]
fn ne_pas_savoir_verifier_n_est_pas_un_echec_de_verification() {
    let sien = ContentHash::of(b"abc");
    assert_eq!(sien.matches(b"abc"), Some(true));
    assert_eq!(sien.matches(b"abd"), Some(false));

    // Le même contenu, annoncé sous un algorithme qu'on ne calcule pas.
    let ailleurs = ContentHash::parse(&format!("sha512:{}", "ab".repeat(64)))
        .expect("un sha512 bien formé se lit");
    assert_eq!(
        ailleurs.matches(b"abc"),
        None,
        "un algorithme non calculé rend une absence de verdict, pas un verdict négatif"
    );

    // Et l'égalité brute, elle, ne sait pas faire la différence — c'est pour cela que `matches`
    // existe. Le test tient la faute qu'on évite, pas seulement le comportement qu'on veut.
    assert_ne!(ailleurs, ContentHash::of(b"abc"));
}

/// Le condensat ne canonicalise rien : il prend les octets qu'on lui donne.
///
/// La forme canonique appartient à l'appelant — `coordination::version` a la sienne, gelée par une
/// fixture. Si `of` normalisait quoi que ce soit, l'identité d'une version dépendrait d'un détail de
/// cette fonction, et geler une forme canonique ne voudrait plus rien dire.
#[test]
fn aucune_canonicalisation_n_est_faite_ici() {
    assert_ne!(ContentHash::of(b"a\n"), ContentHash::of(b"a"));
    assert_ne!(ContentHash::of(b" a"), ContentHash::of(b"a"));
    assert_ne!(ContentHash::of(b"A"), ContentHash::of(b"a"));
    assert_ne!(ContentHash::of(b"a b"), ContentHash::of(b"a  b"));
}
