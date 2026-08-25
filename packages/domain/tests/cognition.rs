//! Clause 1 du test de sortie de `W25.a` — **une mission déclare une classe, jamais un modèle.**
//!
//! Le test d'absence est l'essentiel : ce qui rend l'affectation gratuite à changer n'est pas une
//! convention qu'on respecterait, c'est que le domaine n'a **aucun** moyen de nommer un modèle.

use locus_domain::CognitionClass;

/// Les deux que l'ADR 0026 décision 6 nomme, et pas une troisième.
///
/// `CLAUDE.md` demande qu'une valeur d'énumération n'entre que lorsqu'un consommateur exécutable et
/// testé existe. Une classe « intermédiaire » s'écrirait sans que rien ne rougisse — c'est
/// exactement la promesse que l'ADR 0022 décision 0 refuse : un type qui annonce une distinction
/// dont personne ne se sert.
#[test]
fn deux_classes_et_pas_trois() {
    assert_eq!(CognitionClass::ALL.len(), 2);
    assert_eq!(
        CognitionClass::ALL,
        [CognitionClass::Frontier, CognitionClass::Economy]
    );
}

/// Relire une classe est l'inverse exact de l'écrire, et rien d'autre ne se relit.
#[test]
fn relire_une_classe_est_l_inverse_de_l_ecrire() {
    for classe in CognitionClass::ALL {
        assert_eq!(CognitionClass::parse(classe.slug()), Some(classe));
        assert_eq!(classe.to_string(), classe.slug());
    }
    assert_eq!(CognitionClass::parse("frontiere"), None);
    assert_eq!(CognitionClass::parse(""), None);
    assert_eq!(CognitionClass::parse("gpt-4"), None);
}

/// **La forme sur le fil est celle de `slug`**, pour chaque valeur.
///
/// `serde(rename_all = "snake_case")` et `slug()` sont deux sources pour un même nom. Ce test les
/// confronte valeur par valeur : deux sources de vérité pour un nom de wire est le genre d'écart qui
/// ne se voit qu'au moment où un journal relu ne se reconnaît plus.
#[test]
fn la_forme_serde_est_celle_du_slug() {
    for classe in CognitionClass::ALL {
        let sur_le_fil = serde_json::to_string(&classe).expect("sérialisable");
        assert_eq!(sur_le_fil, format!("\"{}\"", classe.slug()));
        let relue: CognitionClass =
            serde_json::from_str(&sur_le_fil).expect("relisible depuis sa propre forme");
        assert_eq!(relue, classe);
    }
}

/// **Le domaine ne nomme aucun modèle**, tenu par l'absence dans la source.
///
/// C'est la clause 1 prise au mot. Le module est lu sans ses commentaires — la prose explique
/// pourquoi un modèle ne doit pas y être, et l'exclure du scan est la leçon de `W24.c`, où `Default`
/// contenait « fault » et où la garde a d'abord rougi sur son propre vocabulaire.
#[test]
fn le_domaine_ne_nomme_aucun_modele() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cognition.rs"))
        .expect("le module de production est lisible depuis son propre crate");
    let code: String = source
        .lines()
        .filter(|ligne| !ligne.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    for interdit in [
        "model", "Model", "gpt", "claude", "llama", "provider", "vendor",
    ] {
        assert!(
            !code.contains(interdit),
            "« {interdit} » : le domaine déclare une classe, et n'a aucun moyen de nommer ce qui la \
             sert"
        );
    }
}

/// Aucun constructeur ne prend un identifiant de modèle.
///
/// Le complément du test précédent : même si un nom de modèle passait le scan, il n'y a pas de porte
/// par laquelle le faire entrer. `CognitionClass` est un énuméré sans charge — deux variantes nues.
#[test]
fn aucune_variante_ne_porte_de_charge() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cognition.rs"))
        .expect("le module de production est lisible depuis son propre crate");
    let code: Vec<&str> = source
        .lines()
        .filter(|ligne| !ligne.trim_start().starts_with("//"))
        .collect();

    let variantes: Vec<&&str> = code
        .iter()
        .filter(|ligne| ligne.trim() == "Frontier," || ligne.trim() == "Economy,")
        .collect();
    assert_eq!(
        variantes.len(),
        2,
        "les deux variantes sont nues, sans champ ni tuple : {variantes:?}"
    );
}
