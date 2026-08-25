//! Test de sortie de `W26.b` — **les trois classes de lecteurs, et la lecture institutionnelle
//! journalisée.**
//!
//! Trois clauses, celles de la roadmap :
//!
//! 1. générateur, institution, pair : **trois et pas quatre**, tenu par une énumération close qu'un
//!    test lit sous ses noms ;
//! 2. l'institution lit **sans condition d'autorisation** et **la lecture produit un fait**, vérifié
//!    en comptant les événements émis ;
//! 3. un lecteur qui n'est **aucune des trois** n'a pas de chemin, et un test d'absence le tient.
//!
//! C'est ce lecteur qui débloque `W16.d` : ADR 0027 décision 7 tranche ce que l'institution voit
//! d'un sous-agent, et il ne restait à `W16.d` qu'un lecteur pour le lui montrer.

use locus_artifacts::ProducedBy;
use locus_domain::ContentHash;
use locus_memory::{Genre, InstitutionalRead, Reader, Reading, Refusal, Trace, read};
use locus_protocol::Timestamp;

fn hash(byte: &str) -> ContentHash {
    ContentHash::parse(&format!("sha256:{}", byte.repeat(32))).expect("hash bien formé")
}

fn instant() -> Timestamp {
    Timestamp::parse("2026-08-25T12:00:00.000Z").expect("instant bien formé")
}

/// Une trace dont le générateur est **nommé**.
fn trace_de(agent: &str) -> Trace {
    let mut produite = ProducedBy::new("tsk_catalyseur", 3);
    produite.agent_id = Some(agent.to_owned());
    Trace::declaring("art_raisonnement", hash("ab"), 4_096, produite)
        .expect("la déclaration est bien formée")
}

/// Une trace dont le générateur n'est **pas** enregistré — le schéma d'artefact l'autorise.
fn trace_anonyme() -> Trace {
    Trace::declaring(
        "art_anonyme",
        hash("cd"),
        512,
        ProducedBy::new("tsk_catalyseur", 1),
    )
    .expect("la déclaration est bien formée")
}

fn source() -> String {
    let brut = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/readers.rs"))
        .expect("le module de production est lisible depuis son propre crate");
    brut.lines()
        .filter(|ligne| !ligne.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------------------------
// 1. Trois classes, et pas quatre
// ---------------------------------------------------------------------------------------------

/// **Trois variantes, sous leurs noms.**
///
/// Le test lit l'énumération dans la source plutôt que de compter des valeurs construites : les
/// variantes portent des données, donc un `ALL` n'existe pas, et compter ce qu'on a soi-même
/// fabriqué ne dirait rien de ce que le type permet.
#[test]
fn le_lecteur_a_trois_classes_et_pas_une_quatrieme() {
    let code = source();
    let debut = code
        .find("pub enum Reader {")
        .expect("l'énumération existe");
    let fin = code[debut..].find("\n}").expect("elle se ferme") + debut;
    let corps = &code[debut..fin];
    assert!(
        corps.len() > 80,
        "extraction vide : un test d'absence qui n'a rien lu passerait sans rien vérifier"
    );

    for nom in ["Generator {", "Institution {", "Peer {"] {
        assert!(corps.contains(nom), "« {nom} » manque à l'énumération");
    }

    // Le décompte, et pas seulement la présence : une quatrième s'ajouterait sans que les trois
    // assertions du dessus bronchent.
    let variantes = corps
        .lines()
        .filter(|ligne| {
            let nu = ligne.trim();
            nu.ends_with('{') && !nu.starts_with("pub enum") && !nu.starts_with("pub ")
        })
        .count();
    assert_eq!(
        variantes, 3,
        "trois classes, et une quatrième est un amendement de l'ADR : {corps}"
    );
}

/// Chacune des trois a bien un chemin **distinct**, et aucune ne retombe sur une autre.
///
/// Le pendant exécutable du test précédent : une énumération à trois noms dont deux branches
/// rendraient la même chose n'aurait que deux classes.
#[test]
fn les_trois_classes_ont_trois_issues_distinctes() {
    let trace = trace_de("agt_kepler");

    let propre = read(
        &Reader::Generator {
            agent_id: "agt_kepler".to_owned(),
        },
        &trace,
        instant(),
        None,
    );
    let institutionnelle = read(
        &Reader::Institution {
            operator: "usr-marie".to_owned(),
        },
        &trace,
        instant(),
        None,
    );
    let pair = read(
        &Reader::Peer {
            agent_id: "agt_brahe".to_owned(),
        },
        &trace,
        instant(),
        None,
    );

    assert!(matches!(propre, Reading::Own(_)));
    assert!(matches!(institutionnelle, Reading::Institutional(_, _)));
    assert!(matches!(
        pair,
        Reading::Refused(Refusal::NeedsDisclosure { .. })
    ));

    assert_ne!(propre, institutionnelle);
    assert_ne!(institutionnelle, pair);
    assert_ne!(propre, pair);
}

// ---------------------------------------------------------------------------------------------
// 2. L'institution lit sans condition, et la lecture produit un fait
// ---------------------------------------------------------------------------------------------

/// **La clause qui porte l'item**, comptée sur les événements émis.
///
/// Trois lectures institutionnelles produisent trois faits ; trois lectures par le générateur n'en
/// produisent aucun. Le décompte est sur les **deux** polarités, parce qu'un compteur qui ne
/// vérifierait que la première serait content d'une implémentation qui journalise tout — et
/// journaliser la lecture qu'un agent fait de sa propre trace n'est pas ce que l'ADR demande.
#[test]
fn la_lecture_institutionnelle_produit_un_fait_et_l_autre_non() {
    let trace = trace_de("agt_kepler");
    let institution = Reader::Institution {
        operator: "usr-marie".to_owned(),
    };
    let generateur = Reader::Generator {
        agent_id: "agt_kepler".to_owned(),
    };

    let faits_institution = (0..3)
        .filter(|_| read(&institution, &trace, instant(), None).fact().is_some())
        .count();
    assert_eq!(faits_institution, 3, "une lecture, un fait");

    let faits_generateur = (0..3)
        .filter(|_| read(&generateur, &trace, instant(), None).fact().is_some())
        .count();
    assert_eq!(
        faits_generateur, 0,
        "lire la sienne n'implique aucun tiers, et n'écrit donc rien"
    );
}

/// Le fait **nomme** ce qu'il faut pour être utile : quoi, qui, quand.
///
/// Un fait de journal qui dirait seulement « une lecture a eu lieu » ne permettrait ni de savoir
/// quelle trace est sortie, ni à qui la demander.
#[test]
fn le_fait_nomme_la_trace_le_lecteur_et_l_instant() {
    let trace = trace_de("agt_kepler");
    let lecture = read(
        &Reader::Institution {
            operator: "usr-marie".to_owned(),
        },
        &trace,
        instant(),
        None,
    );

    let fait = lecture
        .fact()
        .expect("une lecture institutionnelle est un fait");
    assert_eq!(fait.artifact_id(), "art_raisonnement");
    assert_eq!(fait.operator(), "usr-marie");
    assert_eq!(fait.at(), instant());
}

/// **Sans condition d'autorisation** : l'institution lit la trace d'un agent quelconque, y compris
/// une trace **anonyme**, que le générateur lui-même ne peut pas réclamer.
///
/// C'est le sens exact de la ligne de l'ADR. Poser ici une quelconque habilitation transformerait
/// la journalisation — qui ne restreint personne — en contrôle d'accès, ce que la décision 2 refuse.
#[test]
fn l_institution_lit_sans_condition_d_autorisation() {
    for trace in [trace_de("agt_kepler"), trace_anonyme()] {
        let lecture = read(
            &Reader::Institution {
                operator: "usr-marie".to_owned(),
            },
            &trace,
            instant(),
            None,
        );
        assert!(matches!(lecture, Reading::Institutional(_, _)));
    }
}

/// Le fait est **dans** la variante, pas à côté d'elle.
///
/// Tenu par l'absence : aucune signature ne rend un grant institutionnel sans son fait. Un
/// `Option<InstitutionalRead>` rendu séparément se laisserait ignorer d'un `?`, et c'est l'argument
/// que `messaging.rs` a déjà écrit pour `Reception`.
#[test]
fn aucun_chemin_ne_rend_le_contenu_institutionnel_sans_le_fait() {
    let code = source();
    let debut = code
        .find("pub enum Reading {")
        .expect("l'énumération existe");
    let fin = code[debut..].find("\n}").expect("elle se ferme") + debut;
    let corps = &code[debut..fin];
    assert!(
        corps.len() > 80,
        "extraction vide : voir la note de la règle 3"
    );

    assert!(
        corps.contains("Institutional(Granted, InstitutionalRead)"),
        "le grant et le fait voyagent ensemble : {corps}"
    );

    for interdit in [
        "fn granted_for",
        "fn content_of",
        "fn read_without_journal",
        "fn read_silently",
        "fn peek",
    ] {
        assert!(
            !code.contains(interdit),
            "« {interdit} » rendrait le contenu sans le fait"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 3. Un lecteur qui n'est aucune des trois n'a pas de chemin
// ---------------------------------------------------------------------------------------------

/// **Aucun joker dans le `match` sur le lecteur.**
///
/// C'est ce qui donne son sens à l'énumération close. Un `_ =>` absorberait en silence une
/// quatrième classe, et le compilateur — qui se plaint d'un `match` incomplet, jamais d'un `match`
/// trop tolérant — n'aurait plus rien à dire. `W23.c` tient déjà la même propriété pour les départs
/// d'ordonnancement.
#[test]
fn le_match_sur_le_lecteur_n_a_pas_de_joker() {
    let code = source();
    let debut = code.find("match reader {").expect("le `match` existe");
    let fin = code[debut..].find("\n    }").expect("il se ferme") + debut;
    let corps = &code[debut..fin];
    assert!(
        corps.contains("Reader::Generator") && corps.contains("Reader::Peer"),
        "extraction tronquée : le `match` lu doit être le vrai, sinon l'absence de joker ne dit rien"
    );

    for joker in ["_ =>", "_ if", "..=>"] {
        assert!(
            !corps.contains(joker),
            "« {joker} » absorberait une quatrième classe en silence : {corps}"
        );
    }
}

/// **Le vocabulaire d'une quatrième classe est refusé**, dans toute la source.
///
/// « Un lecteur système ou un outil d'analyse qui lirait sans être ni le générateur, ni
/// l'institution, ni un pair autorisé serait la porte dérobée de ce mécanisme » — ADR 0027 décision
/// 2. La propriété tenue n'est pas « personne n'en a ajouté », qui se relit à chaque revue, mais
/// « personne ne **peut** sans que ce test rougisse ».
#[test]
fn aucune_quatrieme_classe_n_est_nommee() {
    let code = source();
    for interdit in [
        "System {",
        "Tool {",
        "Service {",
        "Analytics {",
        "Admin {",
        "Other {",
        "Any {",
        "Internal {",
    ] {
        assert!(
            !code.contains(interdit),
            "« {interdit} » serait la quatrième classe, et l'ADR en fait un amendement"
        );
    }
    for interdit in ["fn read_as", "fn read_unchecked", "fn grant", "fn bypass"] {
        assert!(
            !code.contains(interdit),
            "« {interdit} » serait le chemin qui contourne les trois"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Les refus, qui ne sont pas un booléen
// ---------------------------------------------------------------------------------------------

/// Un agent ne lit **pas** la trace d'un autre en se réclamant du générateur.
#[test]
fn un_generateur_ne_lit_pas_la_trace_d_un_autre() {
    let lecture = read(
        &Reader::Generator {
            agent_id: "agt_brahe".to_owned(),
        },
        &trace_de("agt_kepler"),
        instant(),
        None,
    );

    assert_eq!(
        lecture,
        Reading::Refused(Refusal::NotYourTrace {
            asked_by: "agt_brahe".to_owned(),
            produced_by: "agt_kepler".to_owned(),
        })
    );
    assert!(lecture.fact().is_none(), "un refus n'est pas une lecture");
}

/// **Non vérifié n'est jamais accordé.**
///
/// `ProducedBy::agent_id` est facultatif dans le schéma d'artefact, donc une trace peut arriver sans
/// générateur nommé. L'accorder à qui l'affirme ferait de l'affirmation la preuve — et le refus a
/// son **propre** motif, distinct de `NotYourTrace` : l'un se répare en demandant la bonne trace,
/// l'autre en enregistrant le générateur à la déclaration.
#[test]
fn une_trace_sans_generateur_nomme_n_est_reclamee_par_personne() {
    let lecture = read(
        &Reader::Generator {
            agent_id: "agt_kepler".to_owned(),
        },
        &trace_anonyme(),
        instant(),
        None,
    );
    assert_eq!(lecture, Reading::Refused(Refusal::UnknownGenerator));

    assert_ne!(
        lecture,
        Reading::Refused(Refusal::NotYourTrace {
            asked_by: "agt_kepler".to_owned(),
            produced_by: String::new(),
        }),
        "deux ignorances différentes ne se résument pas l'une à l'autre"
    );
}

/// Un pair n'obtient **rien**, et le motif nomme ce qui manque.
///
/// L'énumération des motifs de dévoilement commence vide (ADR 0027 décision 3) : aucun dévoilement
/// n'existe, donc aucun pair ne lit. Ce n'est pas un moignon en attente de `W26.c` — c'est la
/// réponse exacte à l'état présent, et `W26.c` la **conditionnera** au lieu de la corriger.
#[test]
fn un_pair_n_obtient_rien_sans_devoilement() {
    let lecture = read(
        &Reader::Peer {
            agent_id: "agt_brahe".to_owned(),
        },
        &trace_de("agt_kepler"),
        instant(),
        None,
    );

    assert_eq!(
        lecture,
        Reading::Refused(Refusal::NeedsDisclosure {
            asked_by: "agt_brahe".to_owned(),
        })
    );
}

// ---------------------------------------------------------------------------------------------
// Ce qu'un grant rend, et ce qu'il ne rend pas
// ---------------------------------------------------------------------------------------------

/// Un grant rend une **référence**, jamais des octets.
///
/// `W26.a` tient par l'absence que ce crate ne stocke aucun contenu de trace ; un grant qui rendrait
/// des octets serait le même second stockage par l'autre bout. Le grant porte donc l'identifiant et
/// le condensat — la façon dont §9.1 désigne un artefact — et le genre sous lequel la trace entre
/// chez le lecteur.
#[test]
fn un_grant_rend_une_reference_et_le_genre_jamais_des_octets() {
    let trace = trace_de("agt_kepler");
    let Reading::Own(grant) = read(
        &Reader::Generator {
            agent_id: "agt_kepler".to_owned(),
        },
        &trace,
        instant(),
        None,
    ) else {
        panic!("le générateur lit la sienne");
    };

    assert_eq!(grant.artifact_id(), "art_raisonnement");
    assert_eq!(grant.declared_hash(), &hash("ab"));
    assert_eq!(
        grant.genre(),
        Genre::MetaMemory,
        "elle entre en meta-mémoire : elle change ce qu'on cherche, jamais ce qu'on tient pour vrai"
    );

    let code = source();
    for interdit in ["bytes", "content:", "body", "-> Vec<u8>", "payload"] {
        assert!(
            !code.contains(interdit),
            "« {interdit} » ferait du grant le second stockage que `W26.a` refuse"
        );
    }
}

/// Le module **ne lit pas l'heure**.
///
/// L'instant du fait est fourni, comme `domain::Envelope` le fait pour une révision. Un journal dont
/// les instants viennent de la montre de celui qui écrit n'est pas rejouable, et l'invariant 1
/// exclut du domaine le choix d'une horloge. Tenu par l'absence, et par l'égalité : deux lectures au
/// même instant fourni portent le même instant.
#[test]
fn le_module_ne_lit_pas_l_heure() {
    let code = source();
    for interdit in ["SystemTime", "Instant::now", "Utc::now", "now()"] {
        assert!(
            !code.contains(interdit),
            "« {interdit} » : l'instant est fourni, jamais lu"
        );
    }

    let autre = Timestamp::parse("2027-01-01T00:00:00.000Z").expect("instant bien formé");
    let trace = trace_de("agt_kepler");
    let institution = Reader::Institution {
        operator: "usr-marie".to_owned(),
    };

    assert_eq!(
        read(&institution, &trace, instant(), None)
            .fact()
            .map(InstitutionalRead::at),
        Some(instant())
    );
    assert_eq!(
        read(&institution, &trace, autre, None)
            .fact()
            .map(InstitutionalRead::at),
        Some(autre),
        "le fait porte l'instant qu'on lui donne, et pas un autre"
    );
}
