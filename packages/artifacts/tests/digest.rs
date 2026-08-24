//! L'implémentation de production du port de hash — `W20.t`.
//!
//! `dependencies.json` notait, en faisant entrer `sha2` : « `Digest` n'avait aucune implémentation
//! de production ». `ContentHash::of` a comblé la moitié tenue en mémoire ; [`Sha256Digest`] comble
//! celle qui hashe pendant que le contenu arrive, ce dont un artefact de plusieurs gigaoctets a
//! besoin pour être refusé sans avoir été retenu.

use locus_artifacts::Sha256Digest;
use locus_artifacts::store::Digest;
use locus_domain::ContentHash;

/// **Hasher en fragments donne le condensat du tout.**
///
/// C'est la seule propriété qui compte, et elle n'est pas gratuite : un calcul incrémental qui
/// réinitialiserait entre deux fragments, ou qui en oublierait un, rendrait un condensat
/// parfaitement bien formé — et il ne ressemblerait à rien tant que personne ne le compare.
#[test]
fn hasher_en_fragments_donne_le_condensat_du_tout() {
    let entier = b"le contenu entier, en un seul morceau";
    let mut digest = Sha256Digest::new();
    digest.update(b"le contenu entier, ");
    digest.update(b"en un seul ");
    digest.update(b"morceau");

    assert_eq!(digest.finish(), ContentHash::of(entier));
}

/// **Un calcul vierge rend le condensat du vide.**
///
/// Et pas une valeur inventée : `finish` sans `update` est un contenu vide, ce qui est un fait, pas
/// une absence de réponse.
#[test]
fn un_calcul_vierge_rend_le_condensat_du_vide() {
    assert_eq!(Sha256Digest::new().finish(), ContentHash::of(b""));
}

/// **Un second `finish` rend le condensat du vide, jamais le précédent.**
///
/// La documentation du type l'annonce, donc un test le tient — une propriété décrite sans être
/// testée est une propriété qu'on croit tenir.
///
/// Le choix mérite d'être défendu, parce que rendre à nouveau le condensat précédent serait plus
/// « pratique ». Ce serait précisément la faute que [`locus_domain::Hasher::finish`] rend
/// impossible en amont en consommant son calcul : un hasher qu'on croit vierge et qui ne l'est pas
/// hashe la concaténation de deux contenus en croyant hasher le second, et **rien dans le résultat
/// ne le montre**. Ici, un condensat de contenu vide ne ressemble à aucun contenu réel : la faute
/// se voit au premier essai au lieu de se découvrir six mois plus tard sur une intégrité cassée.
#[test]
fn un_second_finish_ne_rend_pas_le_condensat_precedent() {
    let mut digest = Sha256Digest::new();
    digest.update(b"quelque chose");
    let premier = digest.finish();

    assert_eq!(premier, ContentHash::of(b"quelque chose"));
    assert_eq!(digest.finish(), ContentHash::of(b""));
}

/// **Ce qui est absorbé après `finish` n'est pas silencieusement compté.**
///
/// Un `update` sur un calcul clos est une faute d'appelant. Il ne panique pas — un port de hash qui
/// fait tomber le daemon sur un ordre d'appels rendrait la panne pire que le défaut — mais il
/// n'entre pas non plus dans un calcul suivant, ce qui contaminerait le condensat d'un autre
/// contenu.
#[test]
fn absorber_apres_finish_ne_contamine_aucun_calcul() {
    let mut digest = Sha256Digest::new();
    digest.update(b"le vrai contenu");
    let condensat = digest.finish();
    digest.update(b"ceci arrive trop tard");

    assert_eq!(condensat, ContentHash::of(b"le vrai contenu"));
    assert_eq!(
        digest.finish(),
        ContentHash::of(b""),
        "le fragment tardif n'a rejoint aucun calcul"
    );
}
