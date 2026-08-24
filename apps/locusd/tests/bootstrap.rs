//! L'amorçage d'enrôlement — `W20.v`, §7.2.
//!
//! # Ce que ces tests protègent
//!
//! Un chemin d'amorçage est la façon habituelle dont une porte de service devient une porte
//! d'entrée. Les tests ci-dessous tiennent les trois propriétés qui l'en empêchent : le **défaut ne
//! change rien**, le scope accordé est **le plus petit**, et le token est **à usage unique**.
//!
//! Le quatrième — un amorçage illisible refuse le démarrage — est tenu ici au grain de la lecture ;
//! le binaire s'en sert pour sortir en `FAILURE`, et c'est `main.rs` qui le montre.

use locusd::bootstrap::{PRINCIPAL_ENV, TOKEN_ENV, WORKSPACE_ENV, read, tokens};
use locusd::enrollment::EnrollmentTokens;

/// Un environnement de fixture, sans toucher au processus.
///
/// `std::env::set_var` aurait été plus court et aurait fait de ces tests des voisins hostiles : les
/// tests Rust partagent un processus, et deux d'entre eux qui muteraient la même variable se
/// verraient l'un l'autre selon l'ordre d'ordonnancement. C'est la même raison qui fait que
/// `loadConfig` prend son environnement en paramètre côté worker.
fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |name| {
        pairs
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| (*value).to_owned())
    }
}

/// Un identifiant de fixture, **du bon genre**.
///
/// Générique sur la sorte, et le premier jet ne l'était pas : il fabriquait un identifiant de
/// workspace pour les deux variables, et le refus qui a suivi — « préfixe attendu : agent » — est
/// exactement ce que les identifiants typés de `packages/protocol` existent pour produire. La
/// fixture était fausse, pas le code ; le noter ici plutôt que de la corriger en silence, parce
/// qu'un test qui aurait pris l'autre pente aurait relâché le type pour se faire passer.
fn identifiant<K: locus_protocol::IdKind>(seed: u8) -> String {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    locus_protocol::Id::<K>::from_parts(
        locus_protocol::Timestamp::from_millis(1_700_000_000_000),
        entropy,
    )
    .expect("l'instant de fixture tient sur 48 bits")
    .to_string()
}

/// Un identifiant de workspace.
fn workspace(seed: u8) -> String {
    identifiant::<locus_protocol::id::Workspace>(seed)
}

/// Un identifiant d'agent — le principal sous lequel le worker agira.
fn principal(seed: u8) -> String {
    identifiant::<locus_protocol::id::Agent>(seed)
}

// ---------------------------------------------------------------------------------------------
// 1. Le défaut ne change rien.
// ---------------------------------------------------------------------------------------------

/// **Sans variable, aucun token — donc personne ne s'enrôle.**
///
/// C'est le comportement d'avant cet item, et c'est ce qui le rend sûr par construction : il
/// n'existe aucune configuration où ce module rend un daemon plus ouvert qu'il ne l'était sans lui.
#[test]
fn sans_variable_le_daemon_n_enrole_personne() {
    let issuer = tokens(env(&[])).expect("rien à lire n'est pas une faute");

    assert!(issuer.redeem("n-importe-quoi").is_none());
    assert_eq!(read(env(&[])), Ok(None));
}

/// **Une variable vide vaut une variable absente.**
///
/// Un `LOCUSD_ENROLLMENT_TOKEN=""` traîne dans tous les orchestrateurs qui posent leurs variables
/// sans les remplir. La lire comme « un amorçage est demandé » ferait refuser le démarrage d'un
/// daemon dont personne n'a rien demandé — un refus au visage d'un exploitant innocent.
#[test]
fn un_token_vide_ne_demande_aucun_amorcage() {
    for vide in ["", "   "] {
        assert_eq!(read(env(&[(TOKEN_ENV, vide)])), Ok(None));
    }
}

// ---------------------------------------------------------------------------------------------
// 2. Un amorçage demandé et illisible refuse, en nommant sa variable.
// ---------------------------------------------------------------------------------------------

/// **Un token sans workspace refuse, et nomme le workspace.**
#[test]
fn un_token_sans_workspace_refuse_en_nommant_la_variable() {
    let refus = read(env(&[(TOKEN_ENV, "jeton-1")])).expect_err("l'amorçage est incomplet");

    assert_eq!(refus.variable, WORKSPACE_ENV);
    assert!(refus.reason.contains(TOKEN_ENV), "{}", refus.reason);
}

/// **Un token sans principal refuse, et nomme le principal.**
///
/// Séparé du test précédent plutôt que cumulé : deux variables absentes doivent produire deux
/// refus différents, et un test unique passerait encore si les deux messages se confondaient.
#[test]
fn un_token_sans_principal_refuse_en_nommant_la_variable() {
    let refus = read(env(&[
        (TOKEN_ENV, "jeton-1"),
        (WORKSPACE_ENV, &workspace(2)),
    ]))
    .expect_err("l'amorçage est incomplet");

    assert_eq!(refus.variable, PRINCIPAL_ENV);
}

/// **Un identifiant illisible refuse, et cite ce qui a été écrit.**
///
/// Le refus porte la valeur fautive : un message qui dirait seulement « illisible » laisserait
/// comparer à l'œil une chaîne de vingt-six caractères avec celle du presse-papiers.
#[test]
fn un_identifiant_illisible_refuse_en_citant_la_valeur() {
    let refus = read(env(&[
        (TOKEN_ENV, "jeton-1"),
        (WORKSPACE_ENV, "pas-un-identifiant"),
    ]))
    .expect_err("l'identifiant ne se relit pas");

    assert_eq!(refus.variable, WORKSPACE_ENV);
    assert!(
        refus.reason.contains("pas-un-identifiant"),
        "{}",
        refus.reason
    );
}

/// **Le refus se lit sans ouvrir le code.**
#[test]
fn le_refus_se_lit_en_une_ligne() {
    let refus = read(env(&[(TOKEN_ENV, "jeton-1")])).expect_err("incomplet");

    let phrase = refus.to_string();
    assert!(phrase.contains("amorçage d'enrôlement"), "{phrase}");
    assert!(phrase.contains(WORKSPACE_ENV), "{phrase}");
}

// ---------------------------------------------------------------------------------------------
// 3. Ce que l'amorçage accorde, et ce qu'il n'accorde pas.
// ---------------------------------------------------------------------------------------------

/// **Un amorçage complet rend un token utilisable, et le scope est le plus petit.**
///
/// `worker` et rien d'autre. Un amorçage qui accorderait davantage ferait du chemin le plus commode
/// le chemin le plus puissant — la façon habituelle dont une porte de service devient une porte
/// d'entrée.
#[test]
fn un_amorcage_complet_accorde_le_scope_le_plus_petit() {
    let issuer = tokens(env(&[
        (TOKEN_ENV, "jeton-1"),
        (WORKSPACE_ENV, &workspace(2)),
        (PRINCIPAL_ENV, &principal(3)),
    ]))
    .expect("l'amorçage est complet");

    let grant = issuer.redeem("jeton-1").expect("le token est déposé");
    assert_eq!(grant.scope, vec!["worker".to_owned()]);
    assert_eq!(grant.labels, Vec::<String>::new());
}

/// **Le token est à usage unique.**
///
/// §7.2 le veut, et `redeem` le tient déjà — mais rien ne l'attestait sur le chemin d'amorçage.
/// Un token réutilisable laisserait un second worker prendre l'identité que le premier a obtenue.
#[test]
fn le_token_d_amorcage_ne_sert_qu_une_fois() {
    let issuer = tokens(env(&[
        (TOKEN_ENV, "jeton-1"),
        (WORKSPACE_ENV, &workspace(2)),
        (PRINCIPAL_ENV, &principal(3)),
    ]))
    .expect("l'amorçage est complet");

    assert!(issuer.redeem("jeton-1").is_some());
    assert!(
        issuer.redeem("jeton-1").is_none(),
        "un token consommé ne l'est plus"
    );
}

/// **Un autre token que celui qui a été déposé n'ouvre rien.**
///
/// Le pendant positif du test précédent : une garde qui refuserait tout serait exacte et inutile.
#[test]
fn un_token_inconnu_n_ouvre_rien() {
    let issuer = tokens(env(&[
        (TOKEN_ENV, "jeton-1"),
        (WORKSPACE_ENV, &workspace(2)),
        (PRINCIPAL_ENV, &principal(3)),
    ]))
    .expect("l'amorçage est complet");

    assert!(issuer.redeem("jeton-2").is_none());
    assert!(issuer.redeem("jeton-1").is_some());
}
