//! Le test de sortie de `W17.f` — les six capacités de branche.

use locus_coordination::barrier::{Barriers, Passage};
use locus_coordination::simulation::{Fidelity, Recorded};
use locus_coordination::version::{Digest, Operation, Version};
use locus_domain::ContentHash;
use locus_domain::branch::{Branch, BranchState, Condition, Origin, ValidationWitness};
use locus_event_store::EventStore;
use locus_protocol::id::provisional::Decision as DecisionKind;
use locus_protocol::id::{Agent, Command, Event, Project, Workspace};
use locus_protocol::{Id, IdKind, Timestamp};
use locusd::branch::{Approve, BranchContext, Rollback};
use locusd::{CommandEnvelope, Family, Revision, Runtime};

const NOW: Timestamp = Timestamp::from_millis(1_700_000_000_000);

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

/// Le crate de coordination ne choisit pas d'algorithme ; le test en fournit un jouet.
struct Fnv;

impl Digest for Fnv {
    fn digest(&self, canonical: &str) -> ContentHash {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for byte in canonical.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        ContentHash::parse(&format!("sha256:{hash:064x}")).expect("forme de condensat valide")
    }
}

fn commande(seed: u8) -> CommandEnvelope {
    CommandEnvelope::mutating(
        id::<Command>(seed),
        "branch.approve",
        id::<Workspace>(2),
        id::<Agent>(3),
        format!("idem-{seed}"),
        Revision::INITIAL,
    )
    .expect("commande bien formée")
}

fn contexte() -> BranchContext {
    BranchContext {
        project_id: id::<Project>(4),
        event_id: id::<Event>(9),
        occurred_at: NOW,
        payload_hash: format!("sha256:{}", "ab".repeat(32)),
    }
}

fn branche(state: BranchState) -> Branch {
    Branch {
        id: "br_01".to_owned(),
        workstream_id: "wst_01".to_owned(),
        title: "une branche".to_owned(),
        objective: "un objectif".to_owned(),
        origin: Origin::Root,
        head_revision: id::<locus_domain::ids::RevisionKind>(5),
        state,
        revision: 1,
    }
}

fn temoin(satisfaite: bool) -> ValidationWitness {
    ValidationWitness {
        policy_id: "pol_01".to_owned(),
        conditions: vec![Condition {
            statement: "deux revues indépendantes".to_owned(),
            satisfied: satisfaite,
        }],
    }
}

// ---------------------------------------------------------------------------------------------
// 1. Un diff se lit entre deux révisions nommées
// ---------------------------------------------------------------------------------------------

/// **Les deux bornes sont exigées par la signature**, comme `expected_revision` pour une commande.
///
/// Une comparaison sans borne n'est pas une comparaison : elle rend l'état, pas l'écart. Un
/// approbateur à qui l'on montrerait « tout ce qui existe » croirait relire un changement alors
/// qu'il relit un monde. Le test lit donc la **source** : aucune façade ne prend une seule version.
#[test]
fn un_diff_se_lit_entre_deux_revisions_nommees() {
    let runtime = Runtime::in_memory();
    let depart = Version::root(&[id::<Agent>(1)], &[], &Fnv).expect("version racine");
    let arrivee = depart
        .apply(&Operation::AddNode(id::<Agent>(2)), &Fnv)
        .expect("un membre de plus");

    let vue = runtime.branch_diff(&depart, &arrivee);
    assert_eq!(vue.from, depart.id().to_string());
    assert_eq!(vue.to, arrivee.id().to_string());
    assert!(!vue.is_empty(), "un membre a été ajouté");
    assert_eq!(vue.operations.len(), 1);

    // **La nature de l'opération, pas seulement son compte.** Deux mutants sont passés au travers
    // d'un test qui comptait : inverser les deux bornes rend un `RemoveNode` au lieu d'un
    // `AddNode` — toujours une opération — et rendre chaque opération sous forme de chaîne vide
    // en rend une aussi. Or c'est exactement ce que la documentation de `DiffView` promet : un
    // approbateur qui lirait « 47 changements » sans savoir lesquels n'approuverait rien, il
    // signerait.
    let ajout = &vue.operations[0];
    assert!(
        ajout.contains("AddNode"),
        "« {ajout} » : le sens de l'opération doit se lire, et l'inverse serait un retrait"
    );
    assert!(
        ajout.contains(&id::<Agent>(2).to_string()),
        "« {ajout} » : sur quoi elle porte doit se lire aussi"
    );

    // Le diff d'une version vers elle-même est **rendu**, jamais absent : un approbateur doit voir
    // que rien ne change, et non ne rien voir.
    let immobile = runtime.branch_diff(&depart, &depart);
    assert!(immobile.is_empty());
    assert_eq!(immobile.from, immobile.to);

    // Et il n'existe aucun chemin qui compare depuis le début.
    let source = include_str!("../src/branch.rs");
    assert!(
        !source.contains("fn branch_diff(&self, to: &Version)"),
        "une façade à une seule borne rendrait l'état, pas l'écart"
    );
}

// ---------------------------------------------------------------------------------------------
// 2. L'ombre et la preview ne produisent aucun événement
// ---------------------------------------------------------------------------------------------

/// **Prévisualiser ne doit pas agir**, et la faute serait silencieuse.
///
/// Personne ne relit le journal après une preview. Le test le relit — avant et après — parce que
/// c'est la seule façon de distinguer « n'a rien écrit » de « a écrit sans qu'on regarde ».
#[test]
fn la_preview_et_l_ombre_n_ecrivent_rien() {
    // `mut` seulement pour **relire** le journal : `transaction()` rend un `&mut` dont ce test
    // n'use que le `store()`, en lecture. Que la preview, elle, ne puisse rien écrire est tenu par
    // le test suivant, qui lit sa signature.
    let mut runtime = Runtime::in_memory();
    let avant = runtime.transaction().store().feed(0).len();

    let depart = Version::root(&[id::<Agent>(1)], &[], &Fnv).expect("version racine");
    let arrivee = depart
        .apply(&Operation::AddNode(id::<Agent>(2)), &Fnv)
        .expect("un membre de plus");

    let passage = runtime.branch_preview(&Barriers::new(), &depart, &arrivee);
    assert_eq!(passage, Passage::Clear, "aucune barrière tenue");

    let ombre = runtime.branch_shadow(id::<DecisionKind>(7), &["la question"], &Recorded::new());
    assert_eq!(
        ombre.reached(),
        Fidelity::Shadow,
        "le degré atteint voyage : un rejeu ne dit pas ce qu'un canari dirait"
    );

    assert_eq!(
        runtime.transaction().store().feed(0).len(),
        avant,
        "prévisualiser a écrit dans le journal : prévisualiser est devenu agir"
    );
}

/// Et la garantie tient par les types, pas seulement par le journal.
///
/// Les deux façades prennent `&self` : elles n'ont aucun chemin vers la transaction, qui exige
/// `&mut`. Deux vérifications indépendantes valent mieux qu'une — celle-ci attrape le cas que le
/// journal ne verrait pas, une écriture dans un stream qu'on n'a pas pensé à relire.
#[test]
fn la_preview_et_l_ombre_ne_tiennent_pas_de_quoi_ecrire() {
    let source = include_str!("../src/branch.rs");
    let code: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    for facade in [
        "fn branch_preview(&self",
        "fn branch_shadow(\n        &self",
    ] {
        assert!(
            code.contains(facade) || code.contains(&facade.replace('\n', "")),
            "« {facade} » : la façade doit prendre `&self`"
        );
    }
    assert!(
        !code.contains("&mut self") && !code.contains("submit("),
        "aucune façade de lecture ne touche la transaction"
    );
}

// ---------------------------------------------------------------------------------------------
// 3. L'approbation nomme ce qui manque
// ---------------------------------------------------------------------------------------------

/// Une approbation dont les conditions sont satisfaites écrit un fait.
#[test]
fn une_approbation_complete_ecrit_un_fait() {
    let mut runtime = Runtime::in_memory();
    let handler = Approve {
        branch: branche(BranchState::Formalizing),
        witness: temoin(true),
    };

    let verdict = runtime
        .transaction()
        .submit(&handler, &commande(1), &contexte(), NOW);

    assert!(verdict.accepted().is_some(), "{verdict:?}");
    let ecrits = runtime.transaction().store().read_stream("branch/br_01", 0);
    assert_eq!(ecrits.len(), 1);
    assert_eq!(ecrits[0].event_type.to_string(), "branch.validated");
}

/// **Un refus d'approbation est une politique, pas une requête mal formée.**
///
/// Le client a demandé quelque chose de bien écrit ; c'est l'état de la branche qui l'interdit. Lui
/// rendre `validation` l'enverrait relire sa requête, où il ne trouverait rien.
#[test]
fn une_approbation_incomplete_est_refusee_sous_la_famille_politique() {
    let mut runtime = Runtime::in_memory();
    let handler = Approve {
        branch: branche(BranchState::Formalizing),
        witness: temoin(false),
    };

    let verdict = runtime
        .transaction()
        .submit(&handler, &commande(1), &contexte(), NOW);

    assert_eq!(
        verdict.refused().map(locusd::CommandError::family),
        Some(Family::Policy)
    );
    assert_eq!(
        runtime.transaction().store().stream_count(),
        0,
        "un refus n'écrit rien"
    );
}

// ---------------------------------------------------------------------------------------------
// 4. Le rollback est une commande, pas une suppression
// ---------------------------------------------------------------------------------------------

/// **Le journal est plus long après qu'avant.**
///
/// C'est l'invariant 12 vu du bon côté : ce qui a eu lieu ne cesse pas d'avoir eu lieu parce qu'on
/// est revenu dessus. Un rollback qui effacerait rendrait l'histoire cohérente et fausse.
#[test]
fn le_rollback_allonge_le_journal_au_lieu_de_le_raccourcir() {
    let mut runtime = Runtime::in_memory();

    runtime
        .transaction()
        .submit(
            &Approve {
                branch: branche(BranchState::Formalizing),
                witness: temoin(true),
            },
            &commande(1),
            &contexte(),
            NOW,
        )
        .accepted()
        .expect("l'approbation passe");
    let apres_approbation = runtime
        .transaction()
        .store()
        .read_stream("branch/br_01", 0)
        .len();

    let revision = runtime
        .transaction()
        .store()
        .revision("branch/br_01")
        .expect("le stream existe");
    runtime
        .transaction()
        .submit(
            &Rollback {
                branch: branche(BranchState::Merged),
                into: BranchState::Exploring,
            },
            &CommandEnvelope::mutating(
                id::<Command>(2),
                "branch.rollback",
                id::<Workspace>(2),
                id::<Agent>(3),
                "idem-2",
                Revision::new(revision),
            )
            .expect("commande bien formée"),
            &contexte(),
            NOW,
        )
        .accepted()
        .expect("le rollback passe");

    let apres_rollback = runtime.transaction().store().read_stream("branch/br_01", 0);
    assert_eq!(
        apres_rollback.len(),
        apres_approbation + 1,
        "le rollback a retiré un fait au lieu d'en ajouter un"
    );
    assert_eq!(
        apres_rollback[0].event_type.to_string(),
        "branch.validated",
        "le fait d'origine est toujours là, à sa place"
    );
    assert_eq!(apres_rollback[1].event_type.to_string(), "branch.reopened");
}

// ---------------------------------------------------------------------------------------------
// 5. La navigation dans le temps
// ---------------------------------------------------------------------------------------------

/// L'histoire d'un stream se relit par révision, et son cursor n'est pas celui d'une timeline.
///
/// Deux streams ont tous deux une révision 1 : un cursor d'histoire lu comme une timeline
/// désignerait un tout autre événement, et rien dans la réponse ne le dirait.
#[test]
fn l_histoire_se_relit_par_revision_avec_son_propre_cursor() {
    let mut runtime = Runtime::in_memory();
    runtime
        .transaction()
        .submit(
            &Approve {
                branch: branche(BranchState::Formalizing),
                witness: temoin(true),
            },
            &commande(1),
            &contexte(),
            NOW,
        )
        .accepted()
        .expect("écrite");

    let page = runtime
        .branch_history("branch/br_01", None, None)
        .expect("sans cursor");
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].revision, 1);
    assert_eq!(page.items[0].event_type, "branch.validated");

    // Un cursor d'une autre collection est refusé, pas interprété.
    let etranger = locusd::Cursor::issue(locusd::Collection::Timeline, 1);
    assert!(
        runtime
            .branch_history("branch/br_01", Some(&etranger), None)
            .is_err(),
        "une révision de stream et une position globale ne sont pas la même chose"
    );
}
