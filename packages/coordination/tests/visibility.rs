//! Test de sortie de W15.e — **les trois garanties de l'item.**
//!
//! 1. Deux `ContextView` construites sous deux versions de coordination différentes diffèrent
//!    **exactement** des révisions que `visibility` retire.
//! 2. Aucune relation `visibility` n'élargit ce qu'une ACL refuse.
//! 3. Le constat de la clause de falsification (ADR 0016, décision 10) est écrit — ici en tests, au
//!    ledger en prose.
//!
//! # Ce que ce fichier est
//!
//! Le **consommateur exécutable** que la décision 4 exige avant qu'une sorte entre dans
//! l'énumération. `locus-review` est une dev-dependency : le crate de production ne dépend pas de
//! la revue, c'est un port qui les relie, et c'est ce test qui le branche — donc la sorte est
//! éprouvée contre la vraie `ContextView`, pas contre une imitation.

use std::fmt::Write as _;

use locus_coordination::{
    ApprovalMode, CoordinationMode, Diff, Digest, Operation, Region, Relation, RelationKind,
    Verdict, Version, Visibility, threatens,
};
use locus_domain::{Confidentiality, ContentHash, RevisionId, ids::RevisionKind};
use locus_protocol::{Id, IdKind, Timestamp, id::Agent};
use locus_review::{ContextItem, ContextView, Recipient, Visible};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn agent(seed: u8) -> Id<Agent> {
    id::<Agent>(seed)
}

fn revision(seed: u8) -> RevisionId {
    id::<RevisionKind>(seed)
}

fn relation(from: Id<Agent>, to: Id<Agent>, kind: RelationKind) -> Relation {
    Relation { from, to, kind }
}

const PRIME: u64 = 0x0000_0100_0000_01b3;
const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

struct Fnv;

impl Digest for Fnv {
    fn digest(&self, canonical: &str) -> ContentHash {
        let mut digest = String::with_capacity(64);
        for salt in 0_u64..4 {
            let mut hash = OFFSET ^ salt.wrapping_mul(PRIME);
            for byte in canonical.bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(PRIME);
            }
            write!(digest, "{hash:016x}").expect("écrire dans une String n'échoue pas");
        }
        ContentHash::parse(&format!("sha256:{digest}")).expect("64 hexadécimaux minuscules")
    }
}

/// L'adaptateur de port : c'est **la** ligne qui branche la sorte sur le consommateur.
struct Seen(Visibility);

impl Visible for Seen {
    fn sees(&self, viewer: Id<Agent>, producer: Id<Agent>) -> bool {
        self.0.sees(viewer, producer)
    }
}

/// Un élément de contexte propre, produit par `producer`.
fn item(seed: u8, producer: Id<Agent>) -> (ContextItem, u64) {
    (
        ContextItem {
            revision: revision(seed),
            is_generator_reasoning: false,
            is_refuted: false,
            classification: Confidentiality::Internal,
            cites: Vec::new(),
            is_external_source: true,
            produced_by: Some(producer),
            disclosed: None,
        },
        1,
    )
}

/// Le destinataire : `agent(1)`, habilité `Internal`, non aveugle.
fn recipient() -> Recipient {
    Recipient {
        agent_id: agent(1),
        worker_id: "wrk-1".to_owned(),
        blind_to_generator: false,
        clearance: Confidentiality::Internal,
    }
}

fn view_under(version: &Version, candidates: &[(ContextItem, u64)]) -> ContextView {
    ContextView::build_under(
        candidates,
        &recipient(),
        10,
        Fnv.digest("vue"),
        &Seen(Visibility::of(version)),
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect("les éléments sont en deçà du watermark")
}

/// Quatre agents ; `agent(1)` est le destinataire, `2`, `3` et `4` produisent.
fn organisation(seen: &[Id<Agent>]) -> Version {
    let relations: Vec<Relation> = seen
        .iter()
        .map(|producer| relation(agent(1), *producer, RelationKind::Visibility))
        .collect();
    Version::root(
        &[agent(1), agent(2), agent(3), agent(4)],
        &relations,
        CoordinationMode::Blackboard,
        None,
        &Fnv,
    )
    .expect("organisation cohérente")
}

// ---------------------------------------------------------------------------------------------
// 1. Deux versions, deux vues, et l'écart est exactement ce que `visibility` retire
// ---------------------------------------------------------------------------------------------

#[test]
fn two_versions_give_two_views_differing_exactly_by_what_visibility_removes() {
    let candidates = [item(2, agent(2)), item(3, agent(3)), item(4, agent(4))];

    let wide = view_under(&organisation(&[agent(2), agent(3), agent(4)]), &candidates);
    let narrow = view_under(&organisation(&[agent(2), agent(3)]), &candidates);

    assert_eq!(
        wide.included(),
        [revision(2), revision(3), revision(4)],
        "sous la version large, les trois entrent"
    );
    assert_eq!(
        narrow.included(),
        [revision(2), revision(3)],
        "sous la version étroite, ce qu'agent(4) a produit sort"
    );
    assert_eq!(
        narrow.redactions().len(),
        1,
        "et il sort **en le disant** : une exclusion silencieuse est indistinguable d'un oubli"
    );
    assert_eq!(narrow.redactions()[0].revision, revision(4));
}

/// Recâbler la visibilité change ce qu'un destinataire peut lire, et **seulement** cela.
///
/// C'est la phrase de la décision 11 rendue exécutable : « recâbler une relation change qui peut
/// lire quoi ». Le watermark, le plafond de confidentialité et le hash ne bougent pas.
#[test]
fn rewiring_visibility_changes_what_is_read_and_nothing_else() {
    let candidates = [item(2, agent(2)), item(4, agent(4))];
    let before = view_under(&organisation(&[agent(2), agent(4)]), &candidates);
    let after = view_under(&organisation(&[agent(2)]), &candidates);

    assert_ne!(before.included(), after.included());
    assert_eq!(
        before.source_event_watermark(),
        after.source_event_watermark()
    );
    assert_eq!(
        before.confidentiality_ceiling(),
        after.confidentiality_ceiling()
    );
    assert_eq!(before.content_hash(), after.content_hash());
}

/// Un agent voit ce qu'il a produit, **sans arête**.
///
/// Il ne peut pas en avoir : `Version` refuse les auto-relations. Ce n'est pas une exception
/// arrangeante, c'est la conséquence directe d'une règle de W15.a — et l'oublier ferait disparaître
/// d'une vue le travail de celui qui la reçoit.
#[test]
fn an_agent_sees_its_own_work_without_an_edge() {
    let candidates = [item(1, agent(1)), item(2, agent(2))];
    let view = view_under(&organisation(&[]), &candidates);
    assert_eq!(view.included(), [revision(1)]);

    let refused = Version::root(
        &[agent(1)],
        &[relation(agent(1), agent(1), RelationKind::Visibility)],
        CoordinationMode::Blackboard,
        None,
        &Fnv,
    );
    assert!(
        refused.is_err(),
        "une auto-visibilité serait une redondance que la version refuse"
    );
}

/// Ce qu'aucun agent n'a produit n'est pas concerné.
///
/// Couper une vue de ses sources externes sous couvert d'organisation serait une autre faute que
/// celle qu'on prévient.
#[test]
fn what_no_agent_produced_is_not_governed_by_visibility() {
    let mut anonymous = item(9, agent(2));
    anonymous.0.produced_by = None;
    let view = view_under(&organisation(&[]), &[anonymous]);
    assert_eq!(view.included(), [revision(9)]);
}

/// Ce qui n'est pas déclaré n'est pas vu.
///
/// Le défaut permissif ferait qu'ajouter un agent lui donnerait accès à tout, et qu'il faudrait
/// penser à l'en priver. Personne n'y pense.
#[test]
fn what_is_not_declared_is_not_seen() {
    let view = view_under(&organisation(&[]), &[item(2, agent(2))]);
    assert!(view.included().is_empty());
    assert_eq!(view.redactions().len(), 1);
}

// ---------------------------------------------------------------------------------------------
// 2. Elle retire, elle n'élargit jamais
// ---------------------------------------------------------------------------------------------

/// §16.3 : les embeddings « ne contournent pas les ACL ». Une relation de coordination non plus.
///
/// Le destinataire voit `agent(2)` par une relation déclarée, et l'élément reste écarté parce que
/// la contamination le refuse. Les deux filtres se composent par un **et**, et la vue le dit **deux
/// fois** plutôt qu'une : les deux motifs sont consignés, parce que réparer la contamination sans
/// savoir que la visibilité l'écartait aussi ferait croire le problème résolu.
#[test]
fn a_declared_visibility_never_widens_what_an_acl_refuses() {
    let mut refuted = item(2, agent(2));
    refuted.0.is_refuted = true;

    let view = view_under(&organisation(&[agent(2)]), &[refuted]);
    assert!(
        view.included().is_empty(),
        "une visibilité déclarée ne fait pas entrer une revendication réfutée"
    );
    assert!(
        view.redactions()[0]
            .reason
            .contains("refuted_claim_propagated")
    );
}

/// Et le cumul : refusé par la contamination **et** invisible.
#[test]
fn both_refusals_are_named_when_both_apply() {
    let mut refuted = item(4, agent(4));
    refuted.0.is_refuted = true;

    let view = view_under(&organisation(&[agent(2)]), &[refuted]);
    let reason = &view.redactions()[0].reason;
    assert!(reason.contains("refuted_claim_propagated"));
    assert!(reason.contains("not_visible_to_recipient"));
}

/// Une relation de **revue** ne donne aucune visibilité.
///
/// Les confondre donnerait à tout relecteur la vue de son relu — exactement ce que §12.4 et
/// l'invariant 11 refusent. C'est la faute que deux sortes rendent possible, et qu'une seule
/// rendait invisible.
#[test]
fn a_review_relation_grants_no_visibility() {
    let version = Version::root(
        &[agent(1), agent(2)],
        &[relation(agent(1), agent(2), RelationKind::Review)],
        CoordinationMode::Blackboard,
        None,
        &Fnv,
    )
    .expect("organisation cohérente");

    let view = view_under(&version, &[item(2, agent(2))]);
    assert!(
        view.included().is_empty(),
        "relire n'est pas voir : un relecteur n'obtient pas le contexte de son relu"
    );
    assert!(Visibility::of(&version).is_empty());
}

// ---------------------------------------------------------------------------------------------
// 3. Ce que la deuxième sorte a révélé — la clause de falsification
// ---------------------------------------------------------------------------------------------

/// **Un cycle de visibilité est normal ; un cycle de revue ne l'est pas.**
///
/// Tant que l'énumération n'avait qu'une valeur, le veto d'acyclicité de `region.rs` parlait de
/// « relations » en pensant « revues », et l'implicite était juste par accident. Deux agents qui
/// voient le travail l'un de l'autre coopèrent — les vetoer aurait interdit la collaboration au nom
/// de l'indépendance.
///
/// Le test porte sur le **verdict de la région**, pas sur l'accesseur : c'est là que la faute
/// aurait mordu, et c'est là qu'il faut la chercher.
#[test]
fn a_cycle_of_visibility_is_not_vetoed_but_a_cycle_of_review_is() {
    let region = Region::declare(
        "duo",
        &[agent(1), agent(2)],
        &Operation::NAMES,
        1,
        4,
        4,
        ApprovalMode::Peer,
        false,
    )
    .expect("région valide");

    for (kind, vetoed) in [
        (RelationKind::Visibility, false),
        (RelationKind::Review, true),
    ] {
        let base = Version::root(
            &[agent(1), agent(2)],
            &[relation(agent(1), agent(2), kind)],
            CoordinationMode::Blackboard,
            None,
            &Fnv,
        )
        .expect("organisation cohérente");
        let closing = Diff::declaring(
            &base,
            vec![Operation::AddEdge(relation(agent(2), agent(1), kind))],
            &Fnv,
        )
        .expect("l'arête s'applique");

        let verdict = region
            .admits(&base, &closing, &Fnv)
            .expect("le diff s'applique");
        assert_eq!(
            matches!(verdict, Verdict::Vetoed { .. }),
            vetoed,
            "un cycle de « {kind} » : {verdict:?}"
        );
    }
}

/// L'accesseur, lui aussi, ne compte que la visibilité.
#[test]
fn mutual_visibility_is_read_in_both_directions() {
    let mutual = Version::root(
        &[agent(1), agent(2)],
        &[
            relation(agent(1), agent(2), RelationKind::Visibility),
            relation(agent(2), agent(1), RelationKind::Visibility),
        ],
        CoordinationMode::Blackboard,
        None,
        &Fnv,
    )
    .expect("organisation cohérente");
    let visibility = Visibility::of(&mutual);
    assert_eq!(visibility.len(), 2);
    assert!(visibility.sees(agent(1), agent(2)));
    assert!(visibility.sees(agent(2), agent(1)));
}

/// `seen_by` rend les **observés**, pas les observateurs.
///
/// Les confondre donnerait une liste plausible et fausse : celle de qui regarde un agent, là où on
/// demandait ce que cet agent regarde. Une relation à sens unique se lit dans son sens.
#[test]
fn seen_by_lists_the_observed_never_the_observers() {
    let version = Version::root(
        &[agent(1), agent(2), agent(3)],
        &[
            relation(agent(1), agent(2), RelationKind::Visibility),
            relation(agent(3), agent(1), RelationKind::Visibility),
        ],
        CoordinationMode::Blackboard,
        None,
        &Fnv,
    )
    .expect("organisation cohérente");
    let visibility = Visibility::of(&version);

    assert_eq!(
        visibility.seen_by(agent(1)).into_iter().collect::<Vec<_>>(),
        vec![agent(2)],
        "agent(1) voit agent(2) — pas agent(3), qui le voit"
    );
    assert_eq!(
        visibility.seen_by(agent(2)).into_iter().collect::<Vec<_>>(),
        Vec::new(),
        "agent(2) est vu et ne voit personne"
    );
}

/// Le risque dérivé distingue désormais les sortes.
///
/// Ajouter une arête de visibilité ne peut pas fermer un cycle de revue, donc ne menace rien : une
/// région à `risk_ceiling` nul peut recâbler la visibilité sans rien relâcher. C'est le premier
/// endroit où la deuxième sorte **gagne** quelque chose au lieu de coûter.
#[test]
fn adding_a_visibility_edge_threatens_nothing() {
    let visibility = Operation::AddEdge(relation(agent(1), agent(2), RelationKind::Visibility));
    let review = Operation::AddEdge(relation(agent(1), agent(2), RelationKind::Review));
    assert!(threatens(&visibility).is_empty());
    assert_eq!(threatens(&review).len(), 1);
}

/// L'énumération porte deux sortes, et `role` n'en est pas.
///
/// ADR 0016, amendement du 2026-08-18 : `role` est un champ d'`AgentTemplate` (§7.1), une
/// classification dans une exigence de reviewers (§20), un attribut d'appartenance (§6.3) — jamais
/// une forme *A → B*. Il reste dû comme `SET_ROLE`, opération attributaire.
#[test]
fn the_enumeration_carries_two_kinds_and_role_is_not_one() {
    let slugs: Vec<&str> = RelationKind::ALL.iter().map(|kind| kind.slug()).collect();
    assert_eq!(slugs, ["review", "visibility"]);
    for absent in ["role", "mentors", "delegates_to", "supervises"] {
        assert!(
            RelationKind::parse(absent).is_none(),
            "« {absent} » n'a pas de consommateur exécutable"
        );
    }
}
