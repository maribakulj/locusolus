//! L'amorçage d'administration — `W20.y`, §22.3.
//!
//! # Ce que ces tests protègent
//!
//! Les mêmes trois propriétés qu'en `W20.ab`, parce que c'est le même genre de porte : le **défaut ne
//! change rien**, un amorçage illisible **refuse en nommant sa variable**, et ce qui est accordé
//! l'est **à cette créance-là et à aucune autre**.
//!
//! S'y ajoute une propriété que l'enrôlement n'avait pas à tenir : **la créance ne s'imprime
//! jamais**. Un token d'enrôlement se consomme ; une créance d'administration ouvre §22.3 aussi
//! longtemps que le daemon tourne, et l'annonce de démarrage est le fichier de log le plus lu de
//! l'installation.

use locusd::administration::{
    CREDENTIAL_ENV, PRINCIPAL_ENV, WORKSPACE_ENV, administrators, annonce, read,
};

/// Un environnement de fixture, sans toucher au processus.
///
/// Même raison qu'en `W20.ab` : les tests Rust partagent un processus, et deux d'entre eux qui
/// muteraient la même variable se verraient l'un l'autre selon l'ordre d'ordonnancement.
fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |name| {
        pairs
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| (*value).to_owned())
    }
}

/// Un identifiant de fixture, **du bon genre** — les identifiants de `packages/protocol` sont typés,
/// et une fixture générique est ce qui empêche d'en fabriquer un du mauvais.
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

fn workspace(seed: u8) -> String {
    identifiant::<locus_protocol::id::Workspace>(seed)
}

fn principal(seed: u8) -> String {
    identifiant::<locus_protocol::id::Agent>(seed)
}

/// La créance de fixture. Une chaîne reconnaissable : si elle apparaissait dans une sortie, on la
/// verrait.
const CREANCE: &str = "creance-d-administration-de-fixture";

/// L'amorçage complet.
fn complet() -> [(&'static str, String); 3] {
    [
        (CREDENTIAL_ENV, CREANCE.to_owned()),
        (WORKSPACE_ENV, workspace(2)),
        (PRINCIPAL_ENV, principal(3)),
    ]
}

/// Le même, sous la forme que `env` accepte.
fn paires<'a>(valeurs: &'a [(&'static str, String)]) -> Vec<(&'static str, &'a str)> {
    valeurs
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect()
}

// ---------------------------------------------------------------------------------------------
// 1. Le défaut ne change rien.
// ---------------------------------------------------------------------------------------------

/// **Sans variable, personne n'administre.**
///
/// C'est le comportement d'avant cet item — vérifié contre un daemon réel, qui rendait `403` sur
/// `/commands/task/queue` pour toute créance. Il n'existe aucune configuration où ce module rend un
/// daemon plus ouvert qu'il ne l'était sans lui.
#[test]
fn sans_variable_personne_n_administre() {
    let (registry, admise) = administrators(env(&[])).expect("rien à lire n'est pas une faute");

    assert!(admise.is_none());
    assert!(
        locusd::mission::Administrators::authority(&registry, "n-importe-quoi").is_none(),
        "un registre non amorcé n'admet rien"
    );
    assert_eq!(read(env(&[])), Ok(None));
}

/// **Une créance vide vaut une créance absente.**
///
/// Un `LOCUSD_ADMIN_CREDENTIAL=""` traîne dans tous les orchestrateurs qui posent leurs variables
/// sans les remplir. La lire comme « un amorçage est demandé » ferait refuser le démarrage d'un
/// daemon dont personne n'a rien demandé.
#[test]
fn une_creance_vide_ne_demande_aucun_amorcage() {
    for vide in ["", "   "] {
        assert_eq!(read(env(&[(CREDENTIAL_ENV, vide)])), Ok(None));
    }
}

// ---------------------------------------------------------------------------------------------
// 2. Un amorçage demandé et illisible refuse, en nommant sa variable.
// ---------------------------------------------------------------------------------------------

/// **Une créance sans workspace refuse, et nomme le workspace.**
///
/// Le refus cite `LOCUSD_ADMIN_CREDENTIAL` comme demandeur, et **pas** le token d'enrôlement : le
/// helper partagé avec `W20.ab` prend le nom du demandeur en paramètre pour exactement cette raison.
/// Un message qui nommerait toujours le token enverrait l'opérateur chercher une variable
/// d'enrôlement qu'il n'a pas posée.
#[test]
fn une_creance_sans_workspace_refuse_en_nommant_la_variable() {
    let refus = read(env(&[(CREDENTIAL_ENV, CREANCE)])).expect_err("l'amorçage est incomplet");

    assert_eq!(refus.variable, WORKSPACE_ENV);
    assert!(refus.reason.contains(CREDENTIAL_ENV), "{}", refus.reason);
    assert!(
        !refus.reason.contains("ENROLLMENT"),
        "le refus ne renvoie pas vers l'amorçage d'enrôlement : {}",
        refus.reason
    );
}

/// **Une créance sans principal refuse, et nomme le principal.**
///
/// Séparé du test précédent plutôt que cumulé : deux variables absentes doivent produire deux refus
/// différents, et un test unique passerait encore si les deux messages se confondaient.
#[test]
fn une_creance_sans_principal_refuse_en_nommant_la_variable() {
    let refus = read(env(&[
        (CREDENTIAL_ENV, CREANCE),
        (WORKSPACE_ENV, &workspace(2)),
    ]))
    .expect_err("l'amorçage est incomplet");

    assert_eq!(refus.variable, PRINCIPAL_ENV);
}

/// **Un identifiant illisible refuse, et cite ce qui a été écrit.**
#[test]
fn un_identifiant_illisible_refuse_en_citant_la_valeur() {
    let refus = read(env(&[
        (CREDENTIAL_ENV, CREANCE),
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

/// **Un identifiant du mauvais genre refuse.**
///
/// Le principal est un `Id<Agent>` ; y écrire un identifiant de workspace est l'erreur exacte que
/// commet un opérateur qui copie la mauvaise ligne de son presse-papiers. Les identifiants typés de
/// `packages/protocol` la refusent, et ce test le constate au lieu de le supposer — la fixture de
/// `W20.ab` avait commis précisément cette erreur, et c'est le type qui l'a rendue visible.
#[test]
fn un_identifiant_du_mauvais_genre_refuse() {
    let refus = read(env(&[
        (CREDENTIAL_ENV, CREANCE),
        (WORKSPACE_ENV, &workspace(2)),
        (PRINCIPAL_ENV, &workspace(3)),
    ]))
    .expect_err("un workspace n'est pas un agent");

    assert_eq!(refus.variable, PRINCIPAL_ENV);
}

// ---------------------------------------------------------------------------------------------
// 3. Ce que l'amorçage accorde, et à qui.
// ---------------------------------------------------------------------------------------------

/// **Un amorçage complet rend la créance utilisable, et l'autorité est celle qui a été demandée.**
#[test]
fn un_amorcage_complet_admet_la_creance_avec_son_autorite() {
    let valeurs = complet();
    let (registry, admise) =
        administrators(env(&paires(&valeurs))).expect("l'amorçage est complet");

    let autorite = locusd::mission::Administrators::authority(&registry, CREANCE)
        .expect("la créance est admise");
    assert_eq!(autorite.workspace_id.to_string(), workspace(2));
    assert_eq!(autorite.principal_id.to_string(), principal(3));
    assert_eq!(admise, Some(autorite));
}

/// **Une autre créance que celle qui a été posée n'ouvre rien.**
///
/// Le pendant du test précédent. Une garde qui refuserait tout serait exacte et inutile ; celle qui
/// accepterait tout serait la porte d'entrée que cet amorçage existe pour ne pas être.
#[test]
fn une_autre_creance_n_ouvre_rien() {
    let valeurs = complet();
    let (registry, _) = administrators(env(&paires(&valeurs))).expect("l'amorçage est complet");

    assert!(locusd::mission::Administrators::authority(&registry, "une-autre").is_none());
    assert!(locusd::mission::Administrators::authority(&registry, CREANCE).is_some());
}

/// **La créance d'administration n'est pas un token d'enrôlement.**
///
/// Deux variables, deux registres, deux résolutions qui ne se croisent jamais. Les confondre ferait
/// d'un token d'enrôlement fuité une autorité d'administration — et l'inverse donnerait à une
/// créance d'administration le droit d'enrôler des workers, que personne ne lui a accordé.
///
/// Tenu par les **noms**, ici, parce que c'est là que la confusion commencerait : deux constantes
/// qui coïncideraient feraient lire une seule variable pour deux amorçages.
#[test]
fn les_deux_amorcages_ne_partagent_aucune_variable() {
    for administration in [CREDENTIAL_ENV, WORKSPACE_ENV, PRINCIPAL_ENV] {
        for enrolement in [
            locusd::bootstrap::TOKEN_ENV,
            locusd::bootstrap::WORKSPACE_ENV,
            locusd::bootstrap::PRINCIPAL_ENV,
            locusd::bootstrap::PROJECT_ENV,
        ] {
            assert_ne!(
                administration, enrolement,
                "un amorçage d'administration ne lit pas une variable d'enrôlement"
            );
        }
    }
}

/// **Un amorçage d'administration n'émet aucun token d'enrôlement.**
///
/// La moitié comportementale du test précédent : les noms peuvent différer et les deux amorçages se
/// contaminer quand même si l'un remplissait le registre de l'autre.
#[test]
fn un_amorcage_d_administration_n_enrole_personne() {
    let valeurs = complet();
    let issuer = locusd::bootstrap::tokens(env(&paires(&valeurs)))
        .expect("aucune variable d'enrôlement n'est posée");

    assert!(
        locusd::enrollment::EnrollmentTokens::redeem(&issuer, CREANCE).is_none(),
        "la créance d'administration n'est pas un token d'enrôlement"
    );
}

// ---------------------------------------------------------------------------------------------
// 4. La créance ne s'imprime jamais.
// ---------------------------------------------------------------------------------------------

/// **L'annonce de démarrage dit sur quoi l'amorçage porte, et ne dit pas la créance.**
///
/// Un token d'enrôlement se consomme ; celle-ci ouvre §22.3 aussi longtemps que le daemon tourne.
/// La règle du dépôt — « ne logge ni OAuth token, API key, cookie » — vise exactement ceci, et un
/// amorçage bavard la violerait au démarrage, dans le fichier de log le plus lu de l'installation.
///
/// Le test tient les deux moitiés : le secret **absent**, et le renseignement **présent**. Une
/// annonce vide tiendrait la première et serait inutile.
#[test]
fn l_annonce_ne_porte_pas_la_creance() {
    let valeurs = complet();
    let (_, admise) = administrators(env(&paires(&valeurs))).expect("l'amorçage est complet");
    let autorite = admise.expect("une autorité est admise");

    let phrase = annonce(&autorite);
    assert!(
        !phrase.contains(CREANCE),
        "la créance ne s'imprime pas : {phrase}"
    );
    assert!(phrase.contains(&workspace(2)), "{phrase}");
    assert!(phrase.contains(&principal(3)), "{phrase}");
}
