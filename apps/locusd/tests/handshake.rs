//! Test de sortie de `W19.e` — `docs/06`, ADR 0037.
//!
//! **Le plan de contrôle annonce ce qu'il tient, et la négociation en tire un accord.**
//!
//! # Ce que ces tests protègent, et pourquoi la moitié la plus utile est négative
//!
//! `W2.7` a livré la moitié cliente du handshake ; celle du serveur n'existait pas. Le worker
//! posait donc une question à laquelle rien ne répondait, et son défaut — liste serveur vide, tout
//! en `declined` — était correct et **indistinguable** d'un plan de contrôle sans features.
//!
//! Le piège de cet item est d'annoncer le registre en bloc. Ce serait exact du **protocole** et faux
//! de ce daemon, et la faute serait pire qu'une promesse ordinaire : le pair négocie dessus et tient
//! l'accord pour acquis. Les tests tiennent donc les deux sens — ce qui est accordé, et ce qui est
//! **refusé et non ignoré**.
//!
//! # La négociation est jouée par le vrai `negotiate`
//!
//! Pas par une réimplémentation locale : ce qui est éprouvé est l'accord entre deux moitiés, et
//! recopier la logique de l'une d'elles ferait un test qui se répond à lui-même.

use locus_lep::{LEP_FEATURES, negotiate};
use locusd::handshake::{
    HELD, HelloRefused, MAJOR, ServerHello, answer, spoken, unknown_to_protocol,
};

/// Ce qu'un worker qui parle `lep/1.1` annonce savoir parler.
fn worker_moderne() -> Vec<String> {
    vec!["lep/1.0".to_owned(), "lep/1.1".to_owned()]
}

fn servi() -> ServerHello {
    answer(&worker_moderne()).expect("un pair qui parle notre majeur est servi")
}

// ---------------------------------------------------------------------------------------------
// 1. Ce qui est annoncé est vrai
// ---------------------------------------------------------------------------------------------

/// **Chaque feature annoncée est une feature que le protocole définit.**
///
/// Un nom hors registre ne serait négociable par personne — `negotiate` le rangerait dans `unknown`
/// chez le pair —, donc l'annoncer reviendrait à annoncer une capacité que le protocole ne sait pas
/// nommer. C'est la garde qui attrape une faute de frappe, celle qu'aucune relecture ne voit.
#[test]
fn chaque_feature_annoncee_est_connue_du_protocole() {
    assert_eq!(unknown_to_protocol(), Vec::<&str>::new());
}

/// **Le daemon n'annonce pas le registre en bloc, et l'écart est le sujet de l'item.**
///
/// La comparaison est faite par **différence nommée** et non par un compte : « trois sur six »
/// deviendrait faux le jour où une septième feature entre au registre, alors que la propriété — ces
/// trois-là et pas les autres — reste vraie. Et les trois écartées sont nommées ici parce que c'est
/// le seul endroit où l'on peut lire, sans ouvrir le module, ce que ce daemon ne tient pas.
#[test]
fn les_features_non_tenues_ne_sont_pas_annoncees() {
    let annoncees = servi().features;
    for absente in ["late-results", "human-input", "signed-events"] {
        assert!(
            !annoncees.contains(&absente.to_owned()),
            "« {absente} » est annoncée alors que ce daemon ne la tient pas : {annoncees:?}"
        );
        assert!(
            LEP_FEATURES.iter().any(|(name, _)| *name == absente),
            "« {absente} » doit rester une feature du protocole, sans quoi ce test n'éprouve rien"
        );
    }
}

/// **Les versions annoncées se dérivent des features tenues, elles ne sont pas écrites.**
///
/// `lep/1.0` est le socle, et chaque feature tenue ajoute le mineur qui l'introduit. Le test le
/// vérifie **contre le registre** plutôt que contre une liste recopiée : une constante écrite à la
/// main dériverait sans que rien ne le dise.
#[test]
fn les_versions_annoncees_se_derivent_des_features_tenues() {
    let versions = spoken();
    assert!(versions.contains(&format!("lep/{MAJOR}.0")), "{versions:?}");

    for held in HELD {
        let since = LEP_FEATURES
            .iter()
            .find(|(name, _)| *name == held)
            .map(|(_, since)| *since)
            .expect("une feature tenue est au registre — le test précédent le tient");
        assert!(
            versions.contains(&format!("lep/{since}")),
            "« {held} » est tenue depuis {since}, et cette version n'est pas annoncée : {versions:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 2. La négociation, dans les deux sens
// ---------------------------------------------------------------------------------------------

/// **Une feature que les deux annoncent est accordée.**
///
/// Le pendant positif, et il n'allait pas de soi : c'est précisément ce qui était impossible avant
/// cet item, la liste serveur étant vide.
#[test]
fn une_feature_que_les_deux_annoncent_est_accordee() {
    let servi = servi();
    let accord = negotiate(&["pull-queue"], &refs(&servi.features));

    assert_eq!(accord.features, vec!["pull-queue".to_owned()]);
    assert!(accord.declined.is_empty(), "{accord:?}");
    assert!(accord.unknown.is_empty(), "{accord:?}");
}

/// **Une feature que seul le worker annonce est refusée — et non ignorée.**
///
/// C'est le sens qui compte. `declined` dit au demandeur qu'il doit se replier ; un silence le
/// laisserait croire à un accord, et il compterait sur une capacité que personne ne tient. La
/// distinction est celle que `negotiate` documente : « refusée » et « inconnue » ne sont pas le même
/// non, et aucun des deux n'est un oui.
#[test]
fn une_feature_que_seul_le_worker_annonce_est_refusee() {
    let servi = servi();
    let accord = negotiate(&["signed-events"], &refs(&servi.features));

    assert!(accord.features.is_empty(), "{accord:?}");
    assert_eq!(accord.declined, vec!["signed-events".to_owned()]);
    assert!(
        accord.unknown.is_empty(),
        "« signed-events » est au registre : la refuser n'est pas l'ignorer — {accord:?}"
    );
}

/// **Un nom que le protocole ne connaît pas reste `unknown`, même face à cette liste.**
///
/// Troisième issue, et elle ne se confond avec aucune des deux autres : un pair plus récent, ou mal
/// configuré. Sans ce cas, un test qui ne verrait que `features` et `declined` laisserait croire que
/// la négociation n'a que deux issues.
#[test]
fn un_nom_hors_registre_reste_inconnu() {
    let servi = servi();
    let accord = negotiate(&["telepathie"], &refs(&servi.features));

    assert_eq!(accord.unknown, vec!["telepathie".to_owned()]);
    assert!(accord.features.is_empty(), "{accord:?}");
    assert!(accord.declined.is_empty(), "{accord:?}");
}

// ---------------------------------------------------------------------------------------------
// 3. Le majeur décide qui est servi
// ---------------------------------------------------------------------------------------------

/// **Un pair qui ne parle pas notre majeur est refusé, et le refus nomme ce qu'il a annoncé.**
///
/// Servir une liste de features à un `lep/2.0` serait négocier dans le vide : il ne saurait pas lire
/// les documents qui vont avec. Le refus porte ce que le pair a écrit, parce que c'est la seule
/// chose qu'il puisse corriger.
#[test]
fn un_pair_d_un_autre_majeur_est_refuse() {
    let refus = answer(&["lep/2.0".to_owned()]).expect_err("un autre majeur ne se parle pas");

    assert_eq!(
        refus,
        HelloRefused::NoCommonMajor {
            offered: vec!["lep/2.0".to_owned()]
        }
    );
    assert!(refus.to_string().contains("lep/2.0"), "{refus}");
}

/// **Un pair qui n'annonce rien de lisible est refusé, et pas sous le même motif.**
///
/// « Je parle autre chose » et « je n'ai pas dit ce que je parle » se réparent différemment : l'un
/// change de pair, l'autre corrige son hello. Les fondre enverrait la moitié des cas au mauvais
/// endroit — la règle que ce dépôt applique déjà aux motifs de refus de §12.2.
#[test]
fn un_pair_sans_version_lisible_est_refuse_sous_son_propre_motif() {
    for illisible in [vec![], vec!["1.0".to_owned()], vec!["autre/1.0".to_owned()]] {
        assert_eq!(
            answer(&illisible),
            Err(HelloRefused::NoVersion),
            "{illisible:?}"
        );
    }
}

/// **Un pair qui n'annonce que son `protocol` est servi.**
///
/// Les deux champs sont facultatifs séparément, et le refus vient de leur réunion vide. Exiger
/// `supported_versions` refuserait un worker parfaitement lisible, et c'est le genre de rigidité
/// qu'un handshake ne peut pas se permettre.
#[test]
fn un_pair_qui_n_annonce_que_son_protocole_est_servi() {
    let servi =
        answer(&["lep/1.0".to_owned()]).expect("un seul champ suffit à dire ce qu'on parle");
    assert!(!servi.features.is_empty());
}

fn refs(features: &[String]) -> Vec<&str> {
    features.iter().map(String::as_str).collect()
}
