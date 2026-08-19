//! Ce que les formes canoniques garantissent, et ce qu'elles refusent.
//!
//! Le test de sortie de W0.4. Il porte moins sur « le code marche » que sur les trois propriétés
//! dont tout le reste dépendra : une seule écriture par valeur, un typage des identifiants qui
//! survit à la sérialisation, et une enveloppe d'erreur qui ne peut pas mentir.

use std::collections::BTreeMap;

use locus_protocol::error::{
    Category, EmptyRetryCondition, Retry, RetryCondition, StructuredError,
};
use locus_protocol::id::provisional::{Error as ErrorKind, Mission};
use locus_protocol::id::{Agent, Command, Event, Id, ParseIdError, Workspace};
use locus_protocol::time::{ParseTimestampError, Timestamp};
use locus_protocol::version::{ParseVersionError, ProtocolVersion};

// ---------------------------------------------------------------- horodatage

#[test]
fn l_horodatage_fait_l_aller_retour_sur_l_exemple_de_la_spec() {
    // §10.1, `occurred_at`.
    let text = "2026-07-26T12:00:00.000Z";
    assert_eq!(Timestamp::parse(text).unwrap().to_string(), text);
}

#[test]
fn l_epoque_unix_s_ecrit_comme_prevu() {
    assert_eq!(
        Timestamp::UNIX_EPOCH.to_string(),
        "1970-01-01T00:00:00.000Z"
    );
    assert_eq!(
        Timestamp::parse("1970-01-01T00:00:00.000Z").unwrap(),
        Timestamp::UNIX_EPOCH
    );
}

#[test]
fn une_forme_iso_8601_valide_mais_non_canonique_est_refusee() {
    // Toutes désignent le même instant. Une seule est acceptée, parce que deux pairs qui les
    // écriraient différemment calculeraient deux hashes différents sur la même donnée.
    for text in [
        "2026-07-26T12:00:00Z",          // pas de millisecondes
        "2026-07-26T12:00:00.000000Z",   // microsecondes
        "2026-07-26T12:00:00.000+00:00", // décalage explicite
        "2026-07-26 12:00:00.000Z",      // espace au lieu de T
        "2026-07-26T12:00:00.000z",      // suffixe minuscule
    ] {
        assert_eq!(
            Timestamp::parse(text),
            Err(ParseTimestampError::NotCanonical),
            "{text} aurait dû être refusé"
        );
    }
}

#[test]
fn les_champs_hors_bornes_sont_nommes() {
    assert_eq!(
        Timestamp::parse("2026-13-01T00:00:00.000Z"),
        Err(ParseTimestampError::OutOfRange("month"))
    );
    assert_eq!(
        Timestamp::parse("2026-02-30T00:00:00.000Z"),
        Err(ParseTimestampError::OutOfRange("day"))
    );
    assert_eq!(
        Timestamp::parse("2026-07-26T24:00:00.000Z"),
        Err(ParseTimestampError::OutOfRange("hour"))
    );
    // Seconde intercalaire : pas de représentation en millisecondes depuis l'époque.
    assert_eq!(
        Timestamp::parse("2016-12-31T23:59:60.000Z"),
        Err(ParseTimestampError::OutOfRange("second"))
    );
}

#[test]
fn les_annees_bissextiles_sont_traitees_par_la_regle_complete() {
    // 2000 est bissextile (divisible par 400), 1900 ne l'est pas (divisible par 100).
    assert!(Timestamp::parse("2000-02-29T00:00:00.000Z").is_ok());
    assert_eq!(
        Timestamp::parse("1900-02-29T00:00:00.000Z"),
        Err(ParseTimestampError::OutOfRange("day"))
    );
    assert!(Timestamp::parse("2024-02-29T00:00:00.000Z").is_ok());
}

#[test]
fn l_aller_retour_tient_sur_toute_la_plage_utile() {
    // Un jour sur sept, de 1970 à 2100 : couvre tous les mois, toutes les règles bissextiles.
    let mut millis = 0_i64;
    let end = Timestamp::parse("2100-01-01T00:00:00.000Z")
        .unwrap()
        .millis();
    while millis < end {
        let instant = Timestamp::from_millis(millis);
        let rendered = instant.to_string();
        assert_eq!(
            Timestamp::parse(&rendered),
            Ok(instant),
            "aller-retour cassé sur {rendered}"
        );
        millis += 7 * 86_400_000 + 3_600_123;
    }
}

#[test]
fn les_dates_anterieures_a_l_epoque_s_ecrivent_aussi() {
    let instant = Timestamp::parse("1969-12-31T23:59:59.999Z").unwrap();
    assert_eq!(instant.millis(), -1);
    assert_eq!(instant.to_string(), "1969-12-31T23:59:59.999Z");
}

#[test]
fn l_ordre_naturel_est_chronologique() {
    let early = Timestamp::parse("2026-07-26T12:00:00.000Z").unwrap();
    let late = Timestamp::parse("2026-07-26T12:00:00.112Z").unwrap();
    assert!(early < late);
}

// ---------------------------------------------------------------- identifiants

const ENTROPY: [u8; 10] = [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0x12, 0x34];

#[test]
fn un_identifiant_fait_l_aller_retour_et_porte_son_prefixe() {
    let instant = Timestamp::parse("2026-07-26T12:00:00.000Z").unwrap();
    let id = Id::<Event>::from_parts(instant, ENTROPY).unwrap();
    let rendered = id.to_string();

    assert!(rendered.starts_with("evt_"), "{rendered}");
    assert_eq!(rendered.len(), "evt_".len() + 26);
    assert_eq!(Id::<Event>::parse(&rendered), Ok(id));
    assert_eq!(id.timestamp(), instant);
}

#[test]
fn le_prefixe_fait_partie_de_l_identite() {
    let instant = Timestamp::parse("2026-07-26T12:00:00.000Z").unwrap();
    let event = Id::<Event>::from_parts(instant, ENTROPY).unwrap();
    let body = event.to_string();
    let body = body.strip_prefix("evt_").unwrap();

    // Le même corps, lu comme une commande : refusé. C'est ce que le typage achète.
    assert_eq!(
        Id::<Command>::parse(&format!("evt_{body}")),
        Err(ParseIdError::WrongPrefix { expected: "cmd" })
    );
    assert!(Id::<Command>::parse(&format!("cmd_{body}")).is_ok());
}

#[test]
fn la_forme_canonique_des_identifiants_est_stricte() {
    let instant = Timestamp::parse("2026-07-26T12:00:00.000Z").unwrap();
    let body = Id::<Event>::from_parts(instant, ENTROPY)
        .unwrap()
        .to_string();
    let body = body.strip_prefix("evt_").unwrap().to_owned();

    assert_eq!(Id::<Event>::parse(&body), Err(ParseIdError::MissingPrefix));
    assert_eq!(
        Id::<Event>::parse(&format!("evt_{}", &body[1..])),
        Err(ParseIdError::BodyLength)
    );
    assert_eq!(
        Id::<Event>::parse(&format!("evt_{}", body.to_lowercase())),
        Err(ParseIdError::InvalidCharacter),
        "la forme canonique est en majuscules"
    );
    // I, L, O et U sont hors de l'alphabet Crockford, précisément pour éviter les confusions.
    for confusable in ['I', 'L', 'O', 'U'] {
        let mutated = format!("evt_{confusable}{}", &body[1..]);
        assert_eq!(
            Id::<Event>::parse(&mutated),
            Err(ParseIdError::InvalidCharacter)
        );
    }
    // 26 × 5 bits valent 130 : le premier caractère ne peut pas dépasser 7.
    assert_eq!(
        Id::<Event>::parse(&format!("evt_8{}", &body[1..])),
        Err(ParseIdError::Overflow)
    );
}

#[test]
fn l_ordre_textuel_des_identifiants_suit_l_ordre_chronologique() {
    // La propriété dont l'event store se sert : trier les identifiants comme des chaînes suffit.
    let early = Id::<Event>::from_parts(Timestamp::from_millis(1), ENTROPY).unwrap();
    let late = Id::<Event>::from_parts(Timestamp::from_millis(2), ENTROPY).unwrap();

    assert!(early < late);
    assert!(
        early.to_string() < late.to_string(),
        "l'ordre lexicographique doit suivre"
    );
}

#[test]
fn un_instant_hors_des_48_bits_est_refuse() {
    assert_eq!(
        Id::<Event>::from_parts(Timestamp::from_millis(-1), ENTROPY),
        Err(ParseIdError::TimestampOutOfRange)
    );
    assert_eq!(
        Id::<Event>::from_parts(Timestamp::from_millis(1 << 48), ENTROPY),
        Err(ParseIdError::TimestampOutOfRange)
    );
}

// ---------------------------------------------------------------- versionnement

#[test]
fn une_version_fait_l_aller_retour() {
    assert_eq!(ProtocolVersion::V1_0.to_string(), "lep/1.0");
    assert_eq!(ProtocolVersion::parse("lep/1.0"), Ok(ProtocolVersion::V1_0));
}

#[test]
fn le_majeur_seul_decide_de_la_compatibilite() {
    let v1_0 = ProtocolVersion::new(1, 0);
    let v1_7 = ProtocolVersion::new(1, 7);
    let v2_0 = ProtocolVersion::new(2, 0);

    assert!(v1_0.speaks_with(v1_7));
    assert!(!v1_0.speaks_with(v2_0));
}

#[test]
fn la_negociation_retient_le_mineur_le_plus_bas() {
    let v1_0 = ProtocolVersion::new(1, 0);
    let v1_7 = ProtocolVersion::new(1, 7);

    assert_eq!(v1_7.negotiate(v1_0), Some(v1_0));
    assert_eq!(v1_0.negotiate(v1_7), Some(v1_0));
    assert_eq!(v1_0.negotiate(ProtocolVersion::new(2, 0)), None);
}

#[test]
fn une_version_non_canonique_est_refusee() {
    assert_eq!(
        ProtocolVersion::parse("1.0"),
        Err(ParseVersionError::NotCanonical)
    );
    assert_eq!(
        ProtocolVersion::parse("lep/1"),
        Err(ParseVersionError::NotCanonical)
    );
    assert_eq!(
        ProtocolVersion::parse("lep/1.x"),
        Err(ParseVersionError::NotCanonical)
    );
    assert_eq!(
        ProtocolVersion::parse("http/1.1"),
        Err(ParseVersionError::UnknownProtocol)
    );
}

// ---------------------------------------------------------------- erreurs

fn sample(security_sensitive: bool, retryable: Retry) -> StructuredError {
    let instant = Timestamp::parse("2026-07-26T12:00:00.000Z").unwrap();
    let mut details = BTreeMap::new();
    details.insert("token".to_owned(), "sk-live-42".to_owned());
    StructuredError {
        error_id: Id::<ErrorKind>::from_parts(instant, ENTROPY).unwrap(),
        code: "secret.rejected".to_owned(),
        category: Category::Secret,
        retryable,
        mission_id: Some(Id::<Mission>::from_parts(instant, ENTROPY).unwrap()),
        attempt: Some(2),
        component: "locus-execd".to_owned(),
        message: "le jeton sk-live-42 a été refusé".to_owned(),
        details,
        caused_by: None,
        security_sensitive,
        occurred_at: instant,
    }
}

#[test]
fn une_erreur_sensible_ne_fuit_ni_par_display_ni_par_ses_details() {
    let error = sample(true, Retry::Never);
    let rendered = error.to_string();

    assert!(
        !rendered.contains("sk-live-42"),
        "le secret a fui : {rendered}"
    );
    assert!(
        !rendered.contains("token"),
        "la clé de détail a fui : {rendered}"
    );
    assert!(
        rendered.contains("secret.rejected"),
        "le code doit rester lisible : {rendered}"
    );
    // L'accès reste ouvert à qui traite l'erreur légitimement ; c'est l'écriture qui est fermée.
    assert!(error.message.contains("sk-live-42"));
}

#[test]
fn une_erreur_non_sensible_reste_lisible() {
    let rendered = sample(false, Retry::Never).to_string();
    assert!(rendered.contains("sk-live-42"));
    assert!(rendered.contains("token=sk-live-42"));
}

#[test]
fn la_redaction_suit_la_chaine_des_causes() {
    let mut outer = sample(false, Retry::Never);
    outer.security_sensitive = false;
    outer.message = "échec d'admission".to_owned();
    outer.details = BTreeMap::new();
    outer.caused_by = Some(Box::new(sample(true, Retry::Never)));

    let rendered = outer.to_string();
    assert!(rendered.contains("échec d'admission"));
    assert!(
        !rendered.contains("sk-live-42"),
        "la cause sensible a fui : {rendered}"
    );
}

#[test]
fn une_condition_de_retry_vide_est_refusee() {
    assert_eq!(RetryCondition::new(""), Err(EmptyRetryCondition));
    assert_eq!(RetryCondition::new("   "), Err(EmptyRetryCondition));
    assert!(RetryCondition::new("le quota se réarme à minuit").is_ok());
}

#[test]
fn une_erreur_reessayable_porte_toujours_sa_condition() {
    let condition = RetryCondition::new("le quota se réarme à minuit").unwrap();
    let error = sample(false, Retry::When(condition));

    let retry = error
        .retry_condition()
        .expect("une erreur réessayable a une condition");
    assert_eq!(retry.condition(), "le quota se réarme à minuit");
    assert_eq!(sample(false, Retry::Never).retry_condition(), None);
}

#[test]
fn la_chaine_des_causes_se_parcourt() {
    let mut outer = sample(false, Retry::Never);
    outer.caused_by = Some(Box::new(sample(false, Retry::Never)));
    assert_eq!(outer.chain().count(), 2);
}

// ---------------------------------------------------------------- sur le fil

#[test]
fn l_enveloppe_fait_l_aller_retour_en_json() {
    let condition = RetryCondition::new("la lease est reprise")
        .unwrap()
        .not_before(Timestamp::from_millis(1));
    let error = sample(false, Retry::When(condition));

    let json = serde_json::to_string(&error).unwrap();
    let back: StructuredError = serde_json::from_str(&json).unwrap();
    assert_eq!(back, error);
}

#[test]
fn les_identifiants_et_instants_voyagent_sous_leur_forme_canonique() {
    let error = sample(false, Retry::Never);
    let json: serde_json::Value = serde_json::to_value(&error).unwrap();

    assert_eq!(json["occurred_at"], "2026-07-26T12:00:00.000Z");
    assert!(json["error_id"].as_str().unwrap().starts_with("err_"));
    assert!(json["mission_id"].as_str().unwrap().starts_with("msn_"));
    assert_eq!(json["category"], "secret");
}

#[test]
fn un_retryable_nu_est_refuse_sur_le_fil() {
    // La règle « une erreur retryable doit préciser ses conditions » tient aussi au décodage :
    // un pair qui enverrait `"retryable": true` se fait refuser, pas normaliser.
    let mut json: serde_json::Value = serde_json::to_value(sample(false, Retry::Never)).unwrap();
    assert_eq!(json["retryable"], serde_json::Value::Bool(false));

    json["retryable"] = serde_json::Value::Bool(true);
    let decoded = serde_json::from_value::<StructuredError>(json);
    assert!(decoded.is_err(), "un retryable nu aurait dû être refusé");
}

#[test]
fn un_horodatage_non_canonique_est_refuse_au_decodage() {
    let mut json: serde_json::Value = serde_json::to_value(sample(false, Retry::Never)).unwrap();
    json["occurred_at"] = serde_json::Value::String("2026-07-26T12:00:00Z".to_owned());
    assert!(serde_json::from_value::<StructuredError>(json).is_err());
}

/// **Un identifiant se relit depuis un désérialiseur qui possède ses données.**
///
/// `<&str>::deserialize` exige que le lecteur **prête** ses octets : `serde_json::from_str` le
/// fait, `from_value` et `from_reader` ne le peuvent pas. Le défaut dormait depuis W0.4 parce
/// qu'aucun document portant un identifiant n'avait été relu autrement que depuis une chaîne — le
/// `CommandEnvelope` de W20.a l'a réveillé, avec un message qui accusait la donnée : « invalid
/// type: string, expected a borrowed string ».
///
/// Les trois chemins sont exercés ici, parce que celui qui manquait était précisément celui que
/// personne n'avait essayé.
#[test]
fn un_identifiant_se_relit_par_les_trois_chemins() {
    let id = Id::<Agent>::from_parts(Timestamp::from_millis(1_700_000_000_000), [0_u8; 10])
        .expect("l'instant tient sur 48 bits");
    let json = serde_json::to_string(&id).expect("encodage");

    let depuis_chaine: Id<Agent> = serde_json::from_str(&json).expect("emprunté");
    let depuis_valeur: Id<Agent> =
        serde_json::from_value(serde_json::to_value(id).expect("valeur")).expect("possédé");
    let depuis_lecteur: Id<Agent> =
        serde_json::from_reader(json.as_bytes()).expect("lecteur possédant");

    assert_eq!(depuis_chaine, id);
    assert_eq!(depuis_valeur, id);
    assert_eq!(depuis_lecteur, id);
}

/// Un préfixe étranger reste refusé, quel que soit le chemin.
///
/// `Cow` élargit ce qui est **lisible**, pas ce qui est **accepté** : la vérification de préfixe
/// tient toujours, et un `agent_…` lu comme `Id<Workspace>` échoue par les trois chemins.
#[test]
fn un_prefixe_etranger_est_refuse_par_les_trois_chemins() {
    let id = Id::<Agent>::from_parts(Timestamp::from_millis(1_700_000_000_000), [0_u8; 10])
        .expect("l'instant tient sur 48 bits");
    let json = serde_json::to_string(&id).expect("encodage");

    assert!(serde_json::from_str::<Id<Workspace>>(&json).is_err());
    assert!(
        serde_json::from_value::<Id<Workspace>>(serde_json::to_value(id).expect("valeur")).is_err()
    );
    assert!(serde_json::from_reader::<_, Id<Workspace>>(json.as_bytes()).is_err());
}
