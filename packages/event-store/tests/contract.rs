//! La suite de contract tests du journal — le test de sortie de W1.c.
//!
//! Elle définit ce que « journal » veut dire ici, et c'est elle qui décidera si le driver
//! `PostgreSQL` de W1.d est conforme — pas sa documentation. Elle est donc écrite contre le **port**
//! [`EventStore`], jamais contre l'implémentation en mémoire : le jour où un second journal
//! existe, cette suite tourne sur lui sans être modifiée.

use locus_event_store::{
    Actor, ActorKind, Append, AppendError, Draft, EVENT_NAMESPACES, EventStore, EventType,
    Expected, MemoryEventStore, ParseEventTypeError,
};
use locus_protocol::{
    Id, Timestamp,
    id::{Agent, Command, Event, Project, Workspace},
};

const RECORDED: i64 = 1_700_000_100_000;

/// Un générateur congruentiel linéaire. Même choix qu'en W1.a et W1.b, mêmes raisons.
struct Rng(u64);

impl Rng {
    const fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    fn entropy(&mut self) -> [u8; 10] {
        let mut bytes = [0u8; 10];
        for byte in &mut bytes {
            *byte = u8::try_from(self.next() >> 56).unwrap_or(0);
        }
        bytes
    }

    fn id<K: locus_protocol::IdKind>(&mut self) -> Id<K> {
        Id::from_parts(Timestamp::from_millis(1_700_000_000_000), self.entropy())
            .expect("instant dans les bornes")
    }
}

fn draft(rng: &mut Rng, stream_id: &str, verb: &str, at: i64) -> Draft {
    Draft {
        event_id: rng.id::<Event>(),
        event_type: EventType::parse(&format!("epistemic_object.{verb}")).expect("type valide"),
        schema_version: 1,
        stream_id: stream_id.to_owned(),
        workspace_id: rng.id::<Workspace>(),
        project_id: rng.id::<Project>(),
        program_id: None,
        branch_id: None,
        actor: Actor {
            principal_id: rng.id::<Agent>(),
            kind: ActorKind::Agent,
            delegation_id: None,
        },
        occurred_at: Timestamp::from_millis(at),
        causation_id: rng.id::<Command>(),
        correlation_id: None,
        trace_id: None,
        payload: serde_json::json!({ "verb": verb }),
        payload_hash: format!("sha256:{}", "ab".repeat(32)),
    }
}

fn recorded() -> Timestamp {
    Timestamp::from_millis(RECORDED)
}

// ————————————————————————— Le test de sortie de W1.c —————————————————————————

#[test]
fn a_full_replay_yields_exactly_what_was_written() {
    // « Replay complet » : relire un stream depuis zéro rend tous ses événements, dans l'ordre des
    // révisions, sans trou ni doublon. C'est ce qui rend une projection reconstructible (W1.d) :
    // si le replay n'est pas total et ordonné, la reconstruction ne l'est pas non plus.
    let mut rng = Rng::new(21);
    let store = MemoryEventStore::new();
    let stream = "claim_01";

    let mut expected = Expected::NoStream;
    let mut written = Vec::new();
    for step in 0..25u64 {
        let event = draft(
            &mut rng,
            stream,
            "staged",
            1_700_000_000_000 + i64::try_from(step).unwrap_or(0),
        );
        let appended = store
            .append(
                Append {
                    stream_id: stream.to_owned(),
                    expected,
                    command_id: rng.id::<Command>(),
                    events: vec![event],
                },
                recorded(),
            )
            .expect("écriture permise");
        assert_eq!(appended.revision, step + 1);
        assert!(!appended.replayed);
        written.extend(appended.events);
        expected = Expected::Exact(step + 1);
    }

    let replayed = store.read_stream(stream, 0);
    assert_eq!(
        replayed, written,
        "le replay ne rend pas ce qui a été écrit"
    );

    // Les révisions sont contiguës à partir de 1 : ni trou, ni doublon, ni rang zéro.
    let revisions: Vec<u64> = replayed.iter().map(|event| event.stream_revision).collect();
    assert_eq!(revisions, (1..=25).collect::<Vec<u64>>());

    // Et un replay partiel reprend exactement après la révision demandée.
    let tail = store.read_stream(stream, 20);
    assert_eq!(tail.len(), 5);
    assert_eq!(tail.first().map(|event| event.stream_revision), Some(21));
}

#[test]
fn a_concurrent_write_is_detected_and_named() {
    // « Conflit de concurrence détecté ». Deux écrivains lisent la même révision, le premier écrit,
    // le second est refusé — et le refus dit ce qu'il attendait et ce qu'il a trouvé, parce que
    // c'est de cet écart que l'appelant a besoin pour relire et retenter.
    let mut rng = Rng::new(22);
    let store = MemoryEventStore::new();
    let stream = "claim_02";

    store
        .append(
            Append {
                stream_id: stream.to_owned(),
                expected: Expected::NoStream,
                command_id: rng.id::<Command>(),
                events: vec![draft(&mut rng, stream, "staged", 1_700_000_000_000)],
            },
            recorded(),
        )
        .expect("premier écrivain");

    // Les deux écrivains ont lu la révision 1. Le premier passe.
    store
        .append(
            Append {
                stream_id: stream.to_owned(),
                expected: Expected::Exact(1),
                command_id: rng.id::<Command>(),
                events: vec![draft(&mut rng, stream, "revised", 1_700_000_001_000)],
            },
            recorded(),
        )
        .expect("écrivain rapide");

    // Le second arrive avec la même attente, sur un stream qui a bougé.
    let refusal = store
        .append(
            Append {
                stream_id: stream.to_owned(),
                expected: Expected::Exact(1),
                command_id: rng.id::<Command>(),
                events: vec![draft(&mut rng, stream, "revised", 1_700_000_002_000)],
            },
            recorded(),
        )
        .unwrap_err();

    assert_eq!(
        refusal,
        AppendError::Conflict {
            expected: Expected::Exact(1),
            actual: 2,
        }
    );

    // Et le stream n'a pas bougé : un conflit n'écrit rien, même partiellement.
    assert_eq!(store.revision(stream), Some(2));
    assert_eq!(store.read_stream(stream, 0).len(), 2);
}

#[test]
fn a_second_creation_of_the_same_stream_is_a_conflict() {
    // L'autre moitié de la concurrence optimiste : `NoStream` sur un stream qui existe déjà. Sans
    // ce refus, deux créateurs concurrents produiraient deux histoires du même objet.
    let mut rng = Rng::new(23);
    let store = MemoryEventStore::new();
    let stream = "claim_03";

    let first = Append {
        stream_id: stream.to_owned(),
        expected: Expected::NoStream,
        command_id: rng.id::<Command>(),
        events: vec![draft(&mut rng, stream, "staged", 1_700_000_000_000)],
    };
    store.append(first, recorded()).expect("création");

    let second = Append {
        stream_id: stream.to_owned(),
        expected: Expected::NoStream,
        command_id: rng.id::<Command>(),
        events: vec![draft(&mut rng, stream, "staged", 1_700_000_001_000)],
    };
    assert_eq!(
        store.append(second, recorded()).unwrap_err(),
        AppendError::Conflict {
            expected: Expected::NoStream,
            actual: 1,
        }
    );
}

// ————————————————————————— Les garanties de §10.2 —————————————————————————

#[test]
fn the_same_command_replayed_yields_the_original_result() {
    // §10.2 : « idempotence par commande ». Une commande réémise après une coupure réseau a déjà
    // eu son effet ; lui rendre son résultat d'origine est plus utile que de faire échouer un
    // appelant qui referait la même chose.
    let mut rng = Rng::new(24);
    let store = MemoryEventStore::new();
    let stream = "claim_04";
    let command_id = rng.id::<Command>();
    let event = draft(&mut rng, stream, "staged", 1_700_000_000_000);

    let batch = |events: Vec<Draft>| Append {
        stream_id: stream.to_owned(),
        expected: Expected::NoStream,
        command_id,
        events,
    };

    let first = store
        .append(batch(vec![event.clone()]), recorded())
        .expect("première application");
    assert!(!first.replayed);

    let again = store.append(batch(vec![event]), recorded()).expect("rejeu");
    assert!(again.replayed, "le rejeu n'est pas signalé");
    assert_eq!(again.events, first.events, "le rejeu rend autre chose");
    assert_eq!(again.revision, first.revision);

    // Et rien n'a été écrit deux fois : c'est le point de l'idempotence.
    assert_eq!(store.read_stream(stream, 0).len(), 1);
    assert_eq!(store.revision(stream), Some(1));
}

#[test]
fn a_reused_command_id_with_other_content_is_refused() {
    // Distinct du rejeu : deux lots différents sous un même identifiant veulent dire que
    // l'identifiant a été réutilisé, et l'accepter écrirait l'un des deux en croyant écrire l'autre.
    let mut rng = Rng::new(25);
    let store = MemoryEventStore::new();
    let stream = "claim_05";
    let command_id = rng.id::<Command>();

    store
        .append(
            Append {
                stream_id: stream.to_owned(),
                expected: Expected::NoStream,
                command_id,
                events: vec![draft(&mut rng, stream, "staged", 1_700_000_000_000)],
            },
            recorded(),
        )
        .expect("première application");

    let refusal = store
        .append(
            Append {
                stream_id: stream.to_owned(),
                expected: Expected::Exact(1),
                command_id,
                events: vec![draft(&mut rng, stream, "refuted", 1_700_000_001_000)],
            },
            recorded(),
        )
        .unwrap_err();
    assert_eq!(refusal, AppendError::CommandReused { command_id });
    assert_eq!(store.revision(stream), Some(1));
}

#[test]
fn a_replayed_command_is_not_a_conflict_with_itself() {
    // Le piège que l'ordre des contrôles évite : la commande a fait avancer le stream, donc son
    // `expected` est périmé au rejeu. Vérifier la concurrence avant l'idempotence lui opposerait
    // sa propre écriture — le comble de la concurrence optimiste.
    let mut rng = Rng::new(26);
    let store = MemoryEventStore::new();
    let stream = "claim_06";
    let command_id = rng.id::<Command>();
    let event = draft(&mut rng, stream, "staged", 1_700_000_000_000);
    let batch = Append {
        stream_id: stream.to_owned(),
        expected: Expected::NoStream,
        command_id,
        events: vec![event],
    };

    store.append(batch.clone(), recorded()).expect("première");
    let again = store.append(batch, recorded()).expect("rejeu, pas conflit");
    assert!(again.replayed);
}

#[test]
fn a_batch_is_written_whole_or_not_at_all() {
    // §9.2 exige l'atomicité entre l'ajout des événements et la révision de l'agrégat. Un lot dont
    // un événement vise un autre stream est refusé **avant** toute écriture : un lot à moitié
    // écrit laisserait un agrégat dont l'état ne correspond à aucune décision prise.
    let mut rng = Rng::new(27);
    let store = MemoryEventStore::new();
    let stream = "claim_07";

    let refusal = store
        .append(
            Append {
                stream_id: stream.to_owned(),
                expected: Expected::NoStream,
                command_id: rng.id::<Command>(),
                events: vec![
                    draft(&mut rng, stream, "staged", 1_700_000_000_000),
                    draft(&mut rng, "claim_autre", "staged", 1_700_000_001_000),
                ],
            },
            recorded(),
        )
        .unwrap_err();

    assert!(matches!(refusal, AppendError::StreamMismatch { .. }));
    // Rien n'a été écrit, pas même le premier événement du lot.
    assert_eq!(store.revision(stream), None);
    assert_eq!(store.stream_count(), 0);
}

#[test]
fn an_empty_batch_is_refused() {
    // Une commande sans effet n'a rien à journaliser ; l'écrire produirait une entrée dont aucune
    // projection ne saurait quoi faire.
    let mut rng = Rng::new(28);
    let store = MemoryEventStore::new();
    assert_eq!(
        store
            .append(
                Append {
                    stream_id: "claim_08".to_owned(),
                    expected: Expected::NoStream,
                    command_id: rng.id::<Command>(),
                    events: Vec::new(),
                },
                recorded(),
            )
            .unwrap_err(),
        AppendError::EmptyBatch
    );
}

#[test]
fn a_stream_that_does_not_exist_is_none_not_zero() {
    // « Ce stream n'existe pas » et « ce stream est vide » sont deux faits différents, et le second
    // n'arrive jamais : un stream naît de son premier événement.
    let store = MemoryEventStore::new();
    assert_eq!(store.revision("claim_inconnu"), None);
    assert!(store.read_stream("claim_inconnu", 0).is_empty());
}

#[test]
fn a_multi_event_batch_numbers_them_consecutively() {
    // L'ordre total par stream vaut aussi à l'intérieur d'un lot : les événements d'une même
    // commande se suivent, dans l'ordre où l'appelant les a rangés.
    let mut rng = Rng::new(29);
    let store = MemoryEventStore::new();
    let stream = "claim_09";

    let appended = store
        .append(
            Append {
                stream_id: stream.to_owned(),
                expected: Expected::NoStream,
                command_id: rng.id::<Command>(),
                events: vec![
                    draft(&mut rng, stream, "staged", 1_700_000_000_000),
                    draft(&mut rng, stream, "reviewed", 1_700_000_001_000),
                    draft(&mut rng, stream, "validated", 1_700_000_002_000),
                ],
            },
            recorded(),
        )
        .expect("écriture");

    assert_eq!(appended.revision, 3);
    assert_eq!(
        appended
            .events
            .iter()
            .map(|event| event.stream_revision)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(
        appended
            .events
            .iter()
            .map(|event| event.event_type.verb())
            .collect::<Vec<_>>(),
        vec!["staged", "reviewed", "validated"]
    );
}

#[test]
fn the_raw_export_follows_write_order_across_streams() {
    // §10.2, « export brut ». L'ordre global est celui des écritures, pas celui des `occurred_at` :
    // un worker hors ligne produit des actes anciens écrits tard, et trier par `occurred_at` ferait
    // apparaître ses événements avant ceux qui les ont provoqués.
    let mut rng = Rng::new(30);
    let store = MemoryEventStore::new();

    for (stream, at) in [
        ("claim_10", 1_700_000_900_000_i64),
        ("claim_11", 1_700_000_100_000),
        ("claim_10", 1_700_000_500_000),
    ] {
        let expected = store
            .revision(stream)
            .map_or(Expected::NoStream, Expected::Exact);
        store
            .append(
                Append {
                    stream_id: stream.to_owned(),
                    expected,
                    command_id: rng.id::<Command>(),
                    events: vec![draft(&mut rng, stream, "staged", at)],
                },
                recorded(),
            )
            .expect("écriture");
    }

    let export = store.export();
    assert_eq!(export.len(), 3);
    assert_eq!(
        export
            .iter()
            .map(|event| (event.stream_id.as_str(), event.stream_revision))
            .collect::<Vec<_>>(),
        vec![("claim_10", 1), ("claim_11", 1), ("claim_10", 2)]
    );
    // Le premier écrit porte l'`occurred_at` le plus tardif : l'export ne trie pas par acte.
    assert!(export[0].occurred_at > export[1].occurred_at);
}

#[test]
fn the_journal_offers_no_way_to_rewrite_history() {
    // §10.2 : « immutabilité logique ». La garantie se tient par l'absence, et c'est la seule façon
    // de garder vraie une propriété qui se violerait d'une ligne. Le jour où quelqu'un ajoutera une
    // méthode d'écrasement, ce test le lui rappellera avant la revue.
    let source = include_str!("../src/memory.rs");
    for forbidden in [
        "fn update",
        "fn delete",
        "fn remove",
        "fn truncate",
        "fn compact",
        "fn rewrite",
    ] {
        assert!(!source.contains(forbidden), "`{forbidden}` existe");
    }
    let port = include_str!("../src/store.rs");
    for forbidden in ["fn update", "fn delete", "fn truncate"] {
        assert!(!port.contains(forbidden), "`{forbidden}` dans le port");
    }
}

#[test]
fn the_revision_is_assigned_by_the_journal_and_by_nothing_else() {
    // Le rôle de `Draft` : le producteur ne peut pas poser un rang, parce que le champ n'existe pas
    // chez lui. Deux événements de même rang dans un stream ne sont donc pas représentables.
    let source = include_str!("../src/envelope.rs");
    let draft_block = source
        .split("pub struct Draft {")
        .nth(1)
        .and_then(|rest| rest.split('}').next())
        .expect("le type Draft existe");
    assert!(!draft_block.contains("stream_revision"));
    assert!(!draft_block.contains("recorded_at"));
}

// ————————————————————————— La taxonomie de §10.3 —————————————————————————

#[test]
fn an_event_outside_the_taxonomy_is_refused() {
    // Un événement rangé dans une famille qui n'existe pas est un événement qu'aucune projection
    // n'ira chercher. Le verbe, lui, reste ouvert : §10.3 donne les familles avec un `*` et
    // n'énumère aucun verbe.
    assert_eq!(
        EventType::parse("inexistant.staged").unwrap_err(),
        ParseEventTypeError::UnknownNamespace
    );
    assert_eq!(
        EventType::parse("epistemic_object").unwrap_err(),
        ParseEventTypeError::NotNamespaced
    );
    assert_eq!(
        EventType::parse(".staged").unwrap_err(),
        ParseEventTypeError::Empty
    );

    // Chaque famille du texte accepte un verbe quelconque.
    for namespace in EVENT_NAMESPACES {
        let kind = EventType::parse(&format!("{namespace}.un_verbe_quelconque"))
            .expect("famille du texte");
        assert_eq!(kind.namespace(), namespace);
    }
}

#[test]
fn an_envelope_round_trips_through_json() {
    // §10.2 : « export brut ». Un export qui perdrait ou reformaterait un champ ne serait pas brut.
    let mut rng = Rng::new(31);
    let store = MemoryEventStore::new();
    let stream = "claim_12";
    let appended = store
        .append(
            Append {
                stream_id: stream.to_owned(),
                expected: Expected::NoStream,
                command_id: rng.id::<Command>(),
                events: vec![draft(&mut rng, stream, "staged", 1_700_000_000_000)],
            },
            recorded(),
        )
        .expect("écriture");

    let original = appended.events.first().expect("un événement");
    let text = serde_json::to_string(original).expect("sérialisable");
    let back: locus_event_store::Envelope = serde_json::from_str(&text).expect("relisible");
    assert_eq!(*original, back);
    // Les deux horodatages sont distincts et tous deux présents : §10.1 les montre ainsi, et un
    // acte hors ligne les sépare de plusieurs heures.
    assert_ne!(back.occurred_at, back.recorded_at);
    assert_eq!(back.recorded_at, recorded());
}

// ——————————————— La concurrence, depuis que `append` prend `&self` — ADR 0029 ———————————————

/// **Rien ne se perd quand plusieurs fils écrivent sur des streams distincts.**
///
/// Depuis l'ADR 0029 décision 1, `append` prend `&self` : la garantie d'exclusion n'est plus donnée
/// par le type, elle est **exigée de l'implémenteur et vérifiée ici**. Ce test est celui que le
/// driver `PostgreSQL` de `W20.i` rejouera à l'identique — un backend qui ne le passe pas n'est pas
/// un journal, quoi que dise sa documentation.
#[test]
fn des_ecritures_concurrentes_sur_des_streams_distincts_ne_perdent_rien() {
    const FILS: u64 = 8;
    const PAR_FIL: u64 = 25;

    let store = std::sync::Arc::new(MemoryEventStore::new());
    // **Un rendez-vous, parce qu'un test de concurrence qui espère l'entrelacement n'en éprouve
    // aucun.** Sans lui, les fils sont lancés l'un après l'autre, chacun finit avant que le suivant
    // démarre, et le test passe sur une course qu'il n'a jamais provoquée — vérifié en injectant un
    // « check-then-act » qui a survécu.
    let depart = std::sync::Arc::new(std::sync::Barrier::new(
        usize::try_from(FILS).expect("compte raisonnable"),
    ));
    let mut fils = Vec::new();
    for numero in 0..FILS {
        let store = std::sync::Arc::clone(&store);
        let depart = std::sync::Arc::clone(&depart);
        fils.push(std::thread::spawn(move || {
            let mut rng = Rng::new(9_000 + numero);
            let stream = format!("epistemic_object:{numero}");
            depart.wait();
            for rang in 0..PAR_FIL {
                let expected = if rang == 0 {
                    Expected::NoStream
                } else {
                    Expected::Exact(rang)
                };
                let draft = draft(&mut rng, &stream, "created", RECORDED);
                store
                    .append(
                        Append {
                            stream_id: stream.clone(),
                            expected,
                            command_id: rng.id::<Command>(),
                            events: vec![draft],
                        },
                        recorded(),
                    )
                    .expect("un stream n'est écrit que par son fil");
            }
        }));
    }
    for fil in fils {
        fil.join().expect("aucun fil ne panique");
    }

    // Chaque stream porte exactement ce que son fil a écrit, sans trou.
    for numero in 0..FILS {
        let stream = format!("epistemic_object:{numero}");
        assert_eq!(
            store.revision(&stream),
            Some(PAR_FIL),
            "le stream {stream} a perdu des écritures"
        );
        let events = store.read_stream(&stream, 0);
        let revisions: Vec<u64> = events.iter().map(|event| event.stream_revision).collect();
        assert_eq!(
            revisions,
            (1..=PAR_FIL).collect::<Vec<_>>(),
            "les révisions de {stream} doivent être contiguës et ordonnées"
        );
    }

    // Et le flux global les porte tous : un événement écrit sans entrer dans l'ordre global serait
    // invisible aux projections, ce qui est pire qu'une écriture refusée.
    assert_eq!(
        store.feed(0).len(),
        usize::try_from(FILS * PAR_FIL).expect("compte raisonnable"),
        "le flux global doit porter chaque événement écrit"
    );
}

/// **Sur un même stream, il y a exactement un gagnant par révision.**
///
/// C'est la contrepartie du test précédent, et la plus importante des deux : le contrôle optimiste
/// de `Expected` est ce qui garde la correction quand plusieurs écrivains visent le même agrégat.
/// Un backend qui laisserait passer deux écritures sur la même révision attendue produirait deux
/// histoires pour un même objet.
///
/// # Ce que ce test attrape, et ce qu'il n'attrape pas
///
/// Il vérifie les propriétés **positives** d'un backend conforme : un gagnant par tour, des
/// révisions contiguës, un perdant qui reçoit un conflit et non autre chose. C'est à ce titre que
/// `W20.i` le rejouera contre `PostgreSQL`.
///
/// Il n'est **pas** un détecteur de course, et le dire évite de s'y fier. Mesuré contre une
/// implémentation délibérément fautive — vérification sous un verrou de lecture relâché avant
/// l'écriture —, il a rendu : vert à tous les coups sans rendez-vous, deux fois sur trois avec un
/// rendez-vous, quatre fois sur dix en portant la contention à trente-deux fils. La détection
/// dépend de l'ordonnanceur, donc de la machine, et une garde qui n'attrape qu'à moitié n'est pas
/// une garde.
///
/// C'est pourquoi la course est rendue **inexprimable** plutôt que cherchée : `Journal::check` exige
/// un accès exclusif, donc vérifier sous un verrou de lecture puis écrire sous un verrou d'écriture
/// ne compile pas. Ce test garde ce qu'un test peut garder ; le compilateur garde le reste.
#[test]
fn sur_un_meme_stream_une_seule_ecriture_gagne_par_revision() {
    const FILS: u64 = 8;
    const TOURS: u64 = 20;

    let store = std::sync::Arc::new(MemoryEventStore::new());
    let stream = "epistemic_object:contesté".to_owned();
    let depart = std::sync::Arc::new(std::sync::Barrier::new(
        usize::try_from(FILS).expect("compte raisonnable"),
    ));

    let mut fils = Vec::new();
    for numero in 0..FILS {
        let store = std::sync::Arc::clone(&store);
        let stream = stream.clone();
        let depart = std::sync::Arc::clone(&depart);
        fils.push(std::thread::spawn(move || {
            let mut rng = Rng::new(7_000 + numero);
            let mut verdicts = Vec::new();
            for tour in 0..TOURS {
                let expected = if tour == 0 {
                    Expected::NoStream
                } else {
                    Expected::Exact(tour)
                };
                let draft = draft(&mut rng, &stream, "created", RECORDED);
                depart.wait();
                verdicts.push(store.append(
                    Append {
                        stream_id: stream.clone(),
                        expected,
                        command_id: rng.id::<Command>(),
                        events: vec![draft],
                    },
                    recorded(),
                ));
            }
            verdicts
        }));
    }

    let verdicts: Vec<_> = fils
        .into_iter()
        .flat_map(|fil| fil.join().expect("aucun fil ne panique"))
        .collect();

    let gagnants = verdicts.iter().filter(|verdict| verdict.is_ok()).count();
    assert_eq!(
        gagnants,
        usize::try_from(TOURS).expect("compte raisonnable"),
        "exactement une écriture gagne par tour : deux gagnantes sur une même révision attendue \
         feraient deux histoires pour un même objet"
    );

    // Un perdant reçoit un **conflit**, jamais autre chose : `internal` l'enverrait ouvrir un
    // ticket, `validation` chercher une faute dans sa requête. Seul le conflit dit « relis et
    // retente ».
    for verdict in &verdicts {
        if let Err(other) = verdict {
            assert!(
                matches!(other, AppendError::Conflict { .. }),
                "une écriture concurrente perdante est un conflit, pas {other:?}"
            );
        }
    }

    assert_eq!(
        store.revision(&stream),
        Some(TOURS),
        "le stream porte un événement par tour, ni plus ni moins"
    );
    let revisions: Vec<u64> = store
        .read_stream(&stream, 0)
        .iter()
        .map(|event| event.stream_revision)
        .collect();
    assert_eq!(
        revisions,
        (1..=TOURS).collect::<Vec<_>>(),
        "les révisions restent contiguës sous contention"
    );
}
