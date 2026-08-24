//! Test de sortie de `W17.i` — **le commit d'une version de coordination écrit un fait.**
//!
//! Trois propriétés, et aucune ne se déduit des autres :
//!
//! 1. un commit passe par un `Decide` et rend un événement portant l'opération **et** la
//!    `VersionId` produite, jamais un magasin ;
//! 2. la révision de version **est** le `stream_revision` que le journal attribue, et un commit sur
//!    une base périmée est refusé par `Expected::Exact` **avant** d'écrire ;
//! 3. rejouer le stream depuis la racine rend une `Version` de **même `content_hash`** que celle
//!    commitée, tenu par égalité stricte.
//!
//! La troisième est celle qui compte : c'est elle qui dit que le journal suffit, et donc qu'aucun
//! magasin n'est nécessaire. Les deux premières la rendent possible.

use locus_coordination::version::{ContentDigest, Operation, Version};
use locus_coordination::{CoordinationMode, Relation, RelationKind};
use locus_event_store::{EventStore, MemoryEventStore};
use locus_protocol::id::{Agent, Branch, Command, Event, Project, Workspace};
use locus_protocol::{Id, IdKind, Timestamp};
use locusd::{
    CommandEnvelope, Commit, OrganisationContext, Outcome, ReplayError, Revision, Transaction,
    replay, stream_of,
};

const NOW: Timestamp = Timestamp::from_millis(1_700_000_000_000);

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(NOW, entropy).expect("l'instant de fixture tient sur 48 bits")
}

fn agent(seed: u8) -> Id<Agent> {
    id::<Agent>(seed)
}

fn branche() -> Id<Branch> {
    id::<Branch>(9)
}

fn contexte(seed: u8) -> OrganisationContext {
    OrganisationContext {
        branch_id: branche(),
        project_id: id::<Project>(4),
        event_id: id::<Event>(seed),
        occurred_at: NOW,
        payload_hash: format!("sha256:{}", "ab".repeat(32)),
    }
}

fn commande(seed: u8, revision: u64) -> CommandEnvelope {
    CommandEnvelope::mutating(
        id::<Command>(seed),
        "team.modify",
        id::<Workspace>(2),
        agent(3),
        format!("cle-{seed}"),
        Revision::new(revision),
    )
    .expect("commande bien formée")
}

/// La racine : deux agents, une revue, en tableau noir.
fn racine() -> Version {
    Version::root(
        &[agent(1), agent(2)],
        &[Relation {
            from: agent(1),
            to: agent(2),
            kind: RelationKind::Review,
        }],
        CoordinationMode::Blackboard,
        None,
        &ContentDigest,
    )
    .expect("la fixture est cohérente")
}

/// Les charges écrites sur le stream d'organisation, dans l'ordre.
fn charges(store: &MemoryEventStore) -> Vec<serde_json::Value> {
    store
        .read_stream(&stream_of(branche()), 0)
        .into_iter()
        .map(|recorded| recorded.payload.clone())
        .collect()
}

// ---------------------------------------------------------------------------------------------
// 1. Un commit écrit un fait, et ce fait porte de quoi le vérifier
// ---------------------------------------------------------------------------------------------

/// **L'opération et le résultat, tous les deux.**
///
/// L'opération seule ne se vérifie pas — rien ne dirait qu'on est arrivé où l'on croit. Le résultat
/// seul ne se rejoue pas — on aurait un condensat sans le chemin. Les deux ensemble font un fait
/// qu'un lecteur peut confronter à son propre rejeu, ce qui est la définition de « auditable ».
#[test]
fn un_commit_ecrit_l_operation_et_la_version_produite() {
    let transaction = Transaction::new(MemoryEventStore::new());
    let base = racine();
    let operation = Operation::AddNode(agent(3));
    let attendue = base
        .apply(&operation, &ContentDigest)
        .expect("ajouter un nœud absent est licite");

    let verdict = transaction.submit(
        &Commit {
            base: base.clone(),
            operation: operation.clone(),
            digest: ContentDigest,
        },
        &commande(1, 0),
        &contexte(1),
        NOW,
    );
    assert!(matches!(verdict, Outcome::Accepted(_)), "{verdict:?}");

    let ecrites = charges(transaction.store());
    assert_eq!(ecrites.len(), 1, "un commit écrit un événement, pas deux");

    let charge = &ecrites[0];
    assert_eq!(charge["operation"], operation.canonical());
    assert_eq!(charge["version"], attendue.id().to_string());
    assert_eq!(charge["content"], attendue.content_hash().to_string());
}

/// Une opération que le domaine refuse ne devient **pas** un fait.
///
/// La famille est `policy` et non `validation` : le client a envoyé une requête bien écrite, et
/// c'est l'état de l'organisation qui s'y oppose. L'envoyer relire sa requête serait l'envoyer
/// chercher là où il n'y a rien.
#[test]
fn une_operation_refusee_par_le_domaine_n_ecrit_rien() {
    let transaction = Transaction::new(MemoryEventStore::new());

    let verdict = transaction.submit(
        &Commit {
            base: racine(),
            // `agent(1)` est déjà membre.
            operation: Operation::AddNode(agent(1)),
            digest: ContentDigest,
        },
        &commande(1, 0),
        &contexte(1),
        NOW,
    );

    let refus = verdict.refused().expect("le domaine refuse");
    assert_eq!(refus.family().name(), "policy", "{refus}");
    assert!(charges(transaction.store()).is_empty(), "rien n'est écrit");
}

// ---------------------------------------------------------------------------------------------
// 2. La révision vient du journal, et une base périmée est refusée avant d'écrire
// ---------------------------------------------------------------------------------------------

/// **Aucun compteur** — ADR 0016 décision 5. La révision de la version est celle du stream.
///
/// Le test le tient en enchaînant deux commits et en lisant les révisions rendues par le journal :
/// si ce module tenait son propre compteur, les deux suites divergeraient au premier refus.
#[test]
fn la_revision_de_version_est_celle_du_stream() {
    let transaction = Transaction::new(MemoryEventStore::new());
    let base = racine();

    let premier = transaction.submit(
        &Commit {
            base: base.clone(),
            operation: Operation::AddNode(agent(3)),
            digest: ContentDigest,
        },
        &commande(1, 0),
        &contexte(1),
        NOW,
    );
    let apres_un = premier.accepted().expect("le premier commite").revision;
    assert_eq!(apres_un.get(), 1, "la racine du stream porte la révision 1");

    let intermediaire = base
        .apply(&Operation::AddNode(agent(3)), &ContentDigest)
        .expect("licite");
    let second = transaction.submit(
        &Commit {
            base: intermediaire,
            operation: Operation::AddNode(agent(4)),
            digest: ContentDigest,
        },
        &commande(2, apres_un.get()),
        &contexte(2),
        NOW,
    );
    assert_eq!(
        second.accepted().expect("le second commite").revision.get(),
        2
    );
}

/// **Une base périmée est refusée avant d'écrire**, par le mécanisme de toute autre commande.
///
/// Deux commits sur la même révision attendue : le second a été écrit contre un monde qui n'existe
/// plus. Le refus est celui du journal — `Expected::Exact` —, pas une vérification que ce module
/// aurait ajoutée, et c'est le point : il n'y a rien à ajouter.
#[test]
fn un_commit_sur_une_base_perimee_est_refuse_sans_ecrire() {
    let transaction = Transaction::new(MemoryEventStore::new());
    let base = racine();

    let premier = transaction.submit(
        &Commit {
            base: base.clone(),
            operation: Operation::AddNode(agent(3)),
            digest: ContentDigest,
        },
        &commande(1, 0),
        &contexte(1),
        NOW,
    );
    assert!(premier.accepted().is_some());

    // Le second déclare la même révision attendue que le premier — donc une base périmée.
    let second = transaction.submit(
        &Commit {
            base,
            operation: Operation::AddNode(agent(4)),
            digest: ContentDigest,
        },
        &commande(2, 0),
        &contexte(2),
        NOW,
    );
    assert!(second.refused().is_some(), "{second:?}");
    assert_eq!(
        charges(transaction.store()).len(),
        1,
        "le refus n'a rien écrit : un seul fait sur le stream"
    );
}

// ---------------------------------------------------------------------------------------------
// 3. Le journal suffit — c'est la propriété qui rend le magasin inutile
// ---------------------------------------------------------------------------------------------

/// **Rejouer rend le même `content_hash`**, par égalité stricte.
///
/// C'est la propriété centrale de l'item : si elle tient, aucun magasin de versions n'est nécessaire
/// et `W17.j` peut résoudre une `VersionId` par rejeu. Si elle ne tenait pas, le journal ne serait
/// qu'une trace et il faudrait stocker les versions — ce qu'ADR 0016 décision 5 interdit.
///
/// Cinq opérations, dont une attributaire et une structurelle à partition énoncée : une suite d'un
/// seul `ADD_NODE` prouverait bien moins.
#[test]
fn rejouer_le_stream_rend_la_meme_version() {
    let transaction = Transaction::new(MemoryEventStore::new());
    let racine = racine();

    let suite = [
        Operation::AddNode(agent(3)),
        Operation::AddEdge(Relation {
            from: agent(2),
            to: agent(3),
            kind: RelationKind::Review,
        }),
        Operation::SetRole {
            node: agent(3),
            from: None,
            to: Some("relecteur-en-chef".to_owned()),
        },
        Operation::SetMode {
            from: CoordinationMode::Blackboard,
            to: CoordinationMode::Debate,
        },
        Operation::AddNode(agent(4)),
    ];

    let mut courante = racine.clone();
    for (rang, operation) in suite.iter().enumerate() {
        let seed = u8::try_from(rang).expect("cinq opérations tiennent sur un octet") + 1;
        let revision = u64::try_from(rang).expect("cinq tiennent sur un u64");
        let verdict = transaction.submit(
            &Commit {
                base: courante.clone(),
                operation: operation.clone(),
                digest: ContentDigest,
            },
            &commande(seed, revision),
            &contexte(seed),
            NOW,
        );
        assert!(verdict.accepted().is_some(), "{rang} : {verdict:?}");
        courante = courante
            .apply(operation, &ContentDigest)
            .expect("la suite est licite");
    }

    let rejouee = replay(&racine, &charges(transaction.store()), &ContentDigest)
        .expect("le stream se rejoue");

    // Le contenu **et** l'identité : le second est plus fort, puisqu'il inclut la filiation.
    assert_eq!(rejouee.content_hash(), courante.content_hash());
    assert_eq!(rejouee.id(), courante.id());

    // Et le rejeu est bien passé par les cinq : une version identique obtenue en zéro opération
    // signalerait que la fixture ne change rien.
    assert_ne!(rejouee.content_hash(), racine.content_hash());
}

/// Un stream illisible **dit où**, parce qu'un journal qui ment doit se réparer.
#[test]
fn un_stream_illisible_nomme_la_position() {
    let racine = racine();

    let sans_champ = replay(
        &racine,
        &[serde_json::json!({ "version": "sha256:00" })],
        &ContentDigest,
    )
    .expect_err("aucune opération à lire");
    assert!(
        matches!(sans_champ, ReplayError::Unreadable { position: 0, .. }),
        "{sans_champ}"
    );

    let illisible = replay(
        &racine,
        &[
            serde_json::json!({ "operation": Operation::AddNode(agent(3)).canonical() }),
            serde_json::json!({ "operation": "PEINDRE_LE_MUR\tbleu" }),
        ],
        &ContentDigest,
    )
    .expect_err("la seconde ne se lit pas");
    assert!(
        matches!(illisible, ReplayError::Unreadable { position: 1, .. }),
        "{illisible}"
    );

    // Lisible mais inapplicable : un autre refus, et il ne se confond pas avec le précédent.
    let inapplicable = replay(
        &racine,
        &[serde_json::json!({ "operation": Operation::AddNode(agent(1)).canonical() })],
        &ContentDigest,
    )
    .expect_err("agent(1) est déjà membre");
    assert!(
        matches!(inapplicable, ReplayError::Inapplicable { position: 0, .. }),
        "{inapplicable}"
    );
}
