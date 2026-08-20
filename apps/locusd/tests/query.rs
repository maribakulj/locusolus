//! Le test de sortie de `W20.e` — queries de §22.4, cursors de §22.6.

use locus_event_store::{Actor, ActorKind, Draft as EventDraft, EventType};
use locus_protocol::id::{Agent, Command, Event, Project, Workspace};
use locus_protocol::{Id, IdKind, Timestamp};
use locusd::cursor::{Collection, Cursor, CursorError};
use locusd::{CommandEnvelope, CommandError, Decide, Revision, Runtime};

const NOW: Timestamp = Timestamp::from_millis(1_700_000_000_000);

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

/// Un décideur qui produit un fait par worker nommé.
struct Demarre(&'static str);

impl Decide for Demarre {
    type State = ();

    fn decide(
        &self,
        command: &CommandEnvelope,
        (): &Self::State,
    ) -> Result<Vec<EventDraft>, CommandError> {
        Ok(vec![EventDraft {
            event_id: id::<Event>(9),
            event_type: EventType::parse("task.started").expect("type valide"),
            schema_version: 1,
            stream_id: format!("task/{}", self.0),
            workspace_id: id::<Workspace>(2),
            project_id: id::<Project>(4),
            program_id: None,
            branch_id: None,
            actor: Actor {
                principal_id: id::<Agent>(3),
                kind: ActorKind::Agent,
                delegation_id: None,
            },
            occurred_at: NOW,
            causation_id: *command.command_id(),
            correlation_id: None,
            trace_id: None,
            payload: serde_json::json!({ "attempt_id": self.0, "worker_id": self.0 }),
            payload_hash: format!("sha256:{}", "ab".repeat(32)),
        }])
    }
}

fn commande(seed: u8, key: &str, revision: u64) -> CommandEnvelope {
    CommandEnvelope::mutating(
        id::<Command>(seed),
        "task.start",
        id::<Workspace>(2),
        id::<Agent>(3),
        key,
        Revision::new(revision),
    )
    .expect("commande bien formée")
}

/// Un runtime avec `count` faits écrits, chacun sur son propre stream et son propre worker.
fn runtime_avec(count: u8) -> Runtime<locus_event_store::MemoryEventStore> {
    const NOMS: [&str; 7] = [
        "wrk_a", "wrk_b", "wrk_c", "wrk_d", "wrk_e", "wrk_f", "wrk_g",
    ];
    let mut runtime = Runtime::in_memory();
    for index in 0..count {
        let nom = NOMS[usize::from(index) % NOMS.len()];
        runtime
            .transaction()
            .submit(
                &Demarre(nom),
                &commande(index + 1, &format!("idem-{index}"), 0),
                &(),
                NOW,
            )
            .accepted()
            .unwrap_or_else(|| panic!("l'écriture {index} passe"));
    }
    runtime.catch_up();
    runtime
}

// ---------------------------------------------------------------------------------------------
// 1. Un cursor est opaque
// ---------------------------------------------------------------------------------------------

/// **Opaque veut dire qu'aucun entier ne s'y lit**, donc qu'aucun client ne sera tenté de
/// l'incrémenter.
///
/// La tentation est le vrai risque, bien avant l'adversaire : un cursor qui ressemble à `47` sera
/// incrémenté par quelqu'un, un jour, et sa pagination sautera des pages sans que rien ne le dise.
#[test]
fn un_cursor_ne_laisse_lire_ni_position_ni_collection() {
    let cursor = Cursor::issue(Collection::Timeline, 47);
    let texte = cursor.as_str();

    assert!(!texte.contains("47"), "« {texte} » laisse lire sa position");
    assert!(
        !texte.contains("timeline"),
        "« {texte} » laisse lire sa collection"
    );
    assert!(
        texte.chars().all(|c| c.is_ascii_hexdigit()),
        "« {texte} » n'est pas une forme opaque uniforme"
    );

    // Et il reste lisible par celui qui l'a émis : opaque n'est pas perdu.
    assert_eq!(cursor.read(Collection::Timeline), Ok(47));
}

/// **Chaque collection fait l'aller-retour**, et `ALL` ne peut pas dériver de l'énumération.
///
/// Le défaut que ce test aurait attrapé : `History` a été ajoutée à l'énumération sans entrer dans
/// `ALL`. `Cursor::read` cherche la collection **dans `ALL`** — tout cursor d'histoire était donc
/// illisible, et rendait `Malformed` comme s'il avait été forgé. Rien ne l'a vu, parce qu'aucun test
/// n'exerçait l'aller-retour de cette collection-là.
///
/// La fonction `rang` ci-dessous attache les variantes à `ALL` par un `match` exhaustif : une
/// collection nouvelle rend le crate de test non compilable tant qu'elle n'est pas rangée. C'est le
/// même geste que `Family::rang` en `W20.a`, pour la même raison — une liste écrite à la main ne se
/// contraint pas toute seule.
#[test]
fn chaque_collection_fait_l_aller_retour_et_aucune_ne_manque_a_la_liste() {
    fn rang(collection: Collection) -> usize {
        match collection {
            Collection::Timeline => 0,
            Collection::Workers => 1,
            Collection::Conflicts => 2,
            Collection::Events => 3,
            Collection::History => 4,
        }
    }

    for (position, collection) in Collection::ALL.iter().enumerate() {
        assert_eq!(
            rang(*collection),
            position,
            "{collection} n'est pas à son rang"
        );
        assert_eq!(
            Cursor::issue(*collection, 7).read(*collection),
            Ok(7),
            "« {collection} » n'est pas relisable : elle manque probablement à ALL"
        );
    }
}

/// Un cursor abîmé est refusé, plutôt que lu de travers.
#[test]
fn un_cursor_abime_est_refuse() {
    let cursor = Cursor::issue(Collection::Workers, 3);
    let texte = cursor.as_str().to_owned();

    for abime in [
        texte[..texte.len() - 2].to_owned(),
        format!("{texte}ff"),
        "pas du tout un cursor".to_owned(),
        String::new(),
    ] {
        assert_eq!(
            Cursor::from_wire(abime.clone()).read(Collection::Workers),
            Err(CursorError::Malformed),
            "« {abime} »"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 2. Un cursor d'une autre collection est refusé, pas interprété
// ---------------------------------------------------------------------------------------------

/// **Le refus est nominatif, et c'est tout l'objet de la clause.**
///
/// Une position `2` existe dans les trois collections. La lire dans la mauvaise ne produirait ni
/// erreur ni page vide : elle produirait une page **plausible**, prise au mauvais endroit, que rien
/// dans la réponse ne permettrait de distinguer d'une bonne.
#[test]
fn un_cursor_d_une_autre_collection_est_refuse() {
    for emettrice in Collection::ALL {
        for lectrice in Collection::ALL {
            let verdict = Cursor::issue(emettrice, 2).read(lectrice);
            if emettrice == lectrice {
                assert_eq!(verdict, Ok(2));
            } else {
                assert_eq!(
                    verdict,
                    Err(CursorError::WrongCollection {
                        expected: lectrice,
                        found: emettrice,
                    }),
                    "{emettrice} lu comme {lectrice}"
                );
            }
        }
    }
}

/// Et la query le refuse aussi, pas seulement le type.
#[test]
fn une_query_refuse_le_cursor_d_une_autre_collection() {
    let runtime = runtime_avec(3);
    let etranger = Cursor::issue(Collection::Workers, 1);

    let verdict = runtime.timeline(Some(&etranger), None);
    assert_eq!(
        verdict.err(),
        Some(CursorError::WrongCollection {
            expected: Collection::Timeline,
            found: Collection::Workers,
        })
    );
}

// ---------------------------------------------------------------------------------------------
// 3. Une reprise rend exactement la suite — sans trou ni doublon
// ---------------------------------------------------------------------------------------------

/// **Le parcours page par page rend exactement le tout, une fois chaque.**
///
/// Vérifier que la page 2 « a l'air correcte » ne dirait rien : les deux fautes qu'on cherche — un
/// trou et un doublon — se voient sur l'**ensemble** parcouru, pas sur une page. Le test compare
/// donc la concaténation des pages à ce qu'une lecture non paginée rend.
#[test]
fn une_reprise_rend_exactement_la_suite_sans_trou_ni_doublon() {
    let runtime = runtime_avec(7);

    let tout: Vec<u64> = runtime
        .timeline(None, Some(1000))
        .expect("sans cursor")
        .items
        .iter()
        .map(|entry| entry.position)
        .collect();
    assert_eq!(tout.len(), 7, "sept faits écrits");

    let mut parcouru = Vec::new();
    let mut cursor = None;
    let mut pages = 0;
    loop {
        let page = runtime
            .timeline(cursor.as_ref(), Some(2))
            .expect("cursor émis ici");
        pages += 1;
        parcouru.extend(page.items.iter().map(|entry| entry.position));
        match page.next {
            Some(suivant) => cursor = Some(suivant),
            None => break,
        }
        assert!(pages < 20, "la pagination ne termine pas");
    }

    assert_eq!(parcouru, tout, "ni trou, ni doublon, ni réordonnancement");
    assert_eq!(pages, 4, "7 éléments par pages de 2");
}

/// La même chose sur une collection de projection, qui n'a pas d'ordre naturel.
#[test]
fn une_projection_se_pagine_aussi_sans_trou_ni_doublon() {
    let runtime = runtime_avec(7);

    let tout = runtime
        .workers(None, Some(1000))
        .expect("sans cursor")
        .items;
    assert!(tout.len() >= 2, "plusieurs workers distincts : {tout:?}");

    // **L'ordre est canonique**, et non seulement cohérent avec lui-même. Comparer le parcours
    // paginé à une lecture complète ne dit rien de l'ordre : les deux passent par le même code, et
    // un tri inversé les inverserait toutes les deux — un mutant l'a montré. Une projection est un
    // ensemble sans ordre naturel ; c'est le tri lexicographique qui rend sa pagination
    // reproductible d'un appel à l'autre, et c'est donc lui qu'il faut affirmer.
    assert!(
        tout.windows(2).all(|paire| paire[0] < paire[1]),
        "l'ordre n'est pas lexicographique strict : {tout:?}"
    );

    let mut parcouru = Vec::new();
    let mut cursor = None;
    loop {
        let page = runtime.workers(cursor.as_ref(), Some(1)).expect("cursor");
        parcouru.extend(page.items);
        match page.next {
            Some(suivant) => cursor = Some(suivant),
            None => break,
        }
    }
    assert_eq!(parcouru, tout);
}

/// **La dernière page dit qu'elle est la dernière**, et ne rend pas un cursor de plus.
///
/// Un cursor rendu jusqu'à l'infini ferait boucler tout client qui suit le contrat — c'est une
/// panne du client causée par une politesse du serveur.
#[test]
fn la_derniere_page_ne_rend_pas_de_cursor() {
    let runtime = runtime_avec(3);

    let page = runtime.timeline(None, Some(10)).expect("sans cursor");
    assert_eq!(page.items.len(), 3);
    assert!(
        page.is_last(),
        "tout tient dans la page : il n'y a pas de suite"
    );

    let exacte = runtime.timeline(None, Some(3)).expect("sans cursor");
    assert!(
        exacte.is_last(),
        "une page pleine qui épuise la collection est la dernière — sinon le client redemande pour rien"
    );
}

// ---------------------------------------------------------------------------------------------
// 4. Stable dans une fenêtre cohérente
// ---------------------------------------------------------------------------------------------

/// **Une écriture concurrente ne déplace pas ce qui a déjà été lu.**
///
/// C'est ce que « stable dans une fenêtre cohérente » veut dire, et c'est vérifiable : on lit une
/// page, on écrit, on reprend — la reprise ne doit ni répéter ni sauter ce que la première page
/// avait rendu.
#[test]
fn une_ecriture_pendant_la_pagination_ne_deplace_pas_le_deja_lu() {
    let mut runtime = runtime_avec(4);

    let premiere = runtime.timeline(None, Some(2)).expect("première page");
    let lues: Vec<u64> = premiere.items.iter().map(|entry| entry.position).collect();
    let cursor = premiere.next.expect("il en reste");

    // Une écriture arrive entre les deux appels.
    runtime
        .transaction()
        .submit(
            &Demarre("wrk_tardif"),
            &commande(99, "idem-tardif", 0),
            &(),
            NOW,
        )
        .accepted()
        .expect("l'écriture tardive passe");

    let suite: Vec<u64> = runtime
        .timeline(Some(&cursor), Some(50))
        .expect("reprise")
        .items
        .iter()
        .map(|entry| entry.position)
        .collect();

    for position in &lues {
        assert!(
            !suite.contains(position),
            "la position {position} revient : la fenêtre n'est pas stable"
        );
    }
    assert!(
        suite.windows(2).all(|pair| pair[0] < pair[1]),
        "la suite reste ordonnée"
    );
    assert_eq!(
        lues.len() + suite.len(),
        5,
        "les quatre d'origine plus le tardif, chacun une fois"
    );
}

// ---------------------------------------------------------------------------------------------
// 5. Les bornes
// ---------------------------------------------------------------------------------------------

/// Une limite absurde est ramenée dans ses bornes plutôt que refusée.
///
/// Un client qui demande un million d'éléments veut des éléments, pas une erreur — et il aura la
/// suite par son cursor. Un client qui en demande zéro en reçoit un, faute de quoi la pagination
/// n'avancerait jamais et boucler serait le comportement correct du client.
#[test]
fn une_limite_absurde_est_ramenee_dans_ses_bornes() {
    let runtime = runtime_avec(3);

    assert_eq!(
        runtime.timeline(None, Some(0)).expect("zéro").items.len(),
        1,
        "zéro élément par page ne progresse jamais"
    );
    assert_eq!(
        runtime
            .timeline(None, Some(usize::MAX))
            .expect("énorme")
            .items
            .len(),
        3,
        "le plafond ne retire rien de ce qui existe"
    );
}
