//! Test de sortie de `R3`, seconde moitié — **les métriques structurelles d'une version.**
//!
//! 1. Chacune des quatre mesure ce qu'aucun invariant ne force — sinon elle serait morte.
//! 2. Aucune ne juge : pas de seuil, pas de verdict.
//! 3. Les cinq sont rendues **ensemble**, en un passage.

use std::fmt::Write as _;

use locus_coordination::{Digest, Metrics, Relation, RelationKind, Version};
use locus_domain::ContentHash;
use locus_protocol::{Id, IdKind, Timestamp, id::Agent};

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

fn reviews(from: u8, to: u8) -> Relation {
    Relation {
        from: agent(from),
        to: agent(to),
        kind: RelationKind::Review,
    }
}

fn sees(from: u8, to: u8) -> Relation {
    Relation {
        from: agent(from),
        to: agent(to),
        kind: RelationKind::Visibility,
    }
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

fn version(members: &[u8], relations: &[Relation]) -> Version {
    let members: Vec<Id<Agent>> = members.iter().map(|seed| agent(*seed)).collect();
    Version::root(&members, relations, &Fnv).expect("la fixture est licite")
}

// ---------------------------------------------------------------------------------------------
// 1. Chacune mesure ce qu'aucun invariant ne force
// ---------------------------------------------------------------------------------------------

/// Couverture de revue : rien n'impose qu'un membre soit relu.
///
/// Une version parfaitement valide peut ne relire personne. C'est ce que l'invariant 11 et §14.4
/// supposent acquis, et ce qu'aucun type ne vérifie.
#[test]
fn review_coverage_measures_what_nothing_enforces() {
    let nobody = version(&[1, 2, 3], &[]);
    assert_eq!(Metrics::of(&nobody).reviewed_members(), 0);
    assert_eq!(Metrics::of(&nobody).members(), 3);

    let partial = version(&[1, 2, 3], &[reviews(1, 2)]);
    assert_eq!(Metrics::of(&partial).reviewed_members(), 1);

    let complete = version(&[1, 2, 3], &[reviews(1, 2), reviews(2, 3), reviews(3, 1)]);
    assert_eq!(
        Metrics::of(&complete).reviewed_members(),
        3,
        "un anneau de revue à trois est acyclique et couvre tout le monde"
    );
}

/// Profondeur de revue : la plus longue chaîne, en arêtes.
#[test]
fn review_depth_counts_the_longest_chain() {
    assert_eq!(Metrics::of(&version(&[1, 2], &[])).review_depth(), 0);
    assert_eq!(
        Metrics::of(&version(&[1, 2], &[reviews(1, 2)])).review_depth(),
        1
    );
    let chain = version(
        &[1, 2, 3, 4],
        &[reviews(1, 2), reviews(2, 3), reviews(3, 4)],
    );
    assert_eq!(Metrics::of(&chain).review_depth(), 3);
}

/// Une étoile est large et **plate** : profondeur un, concentration trois.
///
/// Les deux métriques répondent à deux questions ; une seule ferait passer l'étoile pour une chaîne.
#[test]
fn a_star_is_wide_and_flat() {
    let star = version(
        &[1, 2, 3, 4],
        &[reviews(1, 2), reviews(1, 3), reviews(1, 4)],
    );
    let measured = Metrics::of(&star);
    assert_eq!(measured.review_depth(), 1);
    assert_eq!(measured.busiest_reviewer_load(), 3);
    assert_eq!(measured.review_edges(), 3);
    assert_eq!(measured.reviewed_members(), 3);
}

/// Concentration de revue : un relecteur qui relit tout le monde fait de son jugement le seul.
///
/// §13.3 demande « une limite de concentration par famille de modèle et méthode ». La métrique la
/// rend lisible ; elle ne la fixe pas.
#[test]
fn review_concentration_reads_who_reviews_the_most() {
    let spread = version(
        &[1, 2, 3, 4],
        &[reviews(1, 2), reviews(2, 3), reviews(3, 4)],
    );
    assert_eq!(Metrics::of(&spread).busiest_reviewer_load(), 1);

    let concentrated = version(
        &[1, 2, 3, 4],
        &[reviews(1, 2), reviews(1, 3), reviews(1, 4)],
    );
    assert_eq!(Metrics::of(&concentrated).busiest_reviewer_load(), 3);
}

/// Isolement de visibilité : un membre qui ne voit le travail de personne.
///
/// W15.e : la visibilité restreint, elle n'élargit jamais. N'avoir aucune relation sortante est
/// licite, et vaut d'être compté.
#[test]
fn visibility_isolation_counts_members_who_see_nobody() {
    let blind = version(&[1, 2, 3], &[]);
    assert_eq!(Metrics::of(&blind).blind_members(), 3);

    let partial = version(&[1, 2, 3], &[sees(1, 2)]);
    assert_eq!(Metrics::of(&partial).blind_members(), 2);

    // Une relation de **revue** ne fait voir personne : ce sont deux sortes distinctes.
    let reviewing = version(&[1, 2, 3], &[reviews(1, 2)]);
    assert_eq!(
        Metrics::of(&reviewing).blind_members(),
        3,
        "relire n'est pas voir"
    );
}

/// L'isolement se lit sur les arêtes **sortantes**, jamais sur les entrantes.
///
/// « Ne voir personne » et « n'être vu de personne » sont deux situations opposées, et une fixture
/// symétrique les confond : avec la seule arête `1 → 2`, deux membres sont aveugles et deux membres
/// sont invisibles. Il faut un cas asymétrique pour que la différence se voie.
#[test]
fn isolation_reads_outgoing_edges_never_incoming() {
    let fan_out = version(&[1, 2, 3], &[sees(1, 2), sees(1, 3)]);
    assert_eq!(
        Metrics::of(&fan_out).blind_members(),
        2,
        "`2` et `3` ne voient personne ; `1` voit les deux autres"
    );

    let fan_in = version(&[1, 2, 3], &[sees(2, 1), sees(3, 1)]);
    assert_eq!(
        Metrics::of(&fan_in).blind_members(),
        1,
        "seul `1` ne voit personne, bien qu'il soit le seul que tout le monde voie"
    );
}

/// Les deux sortes ne se mélangent pas dans l'autre sens non plus.
#[test]
fn a_visibility_edge_is_not_a_review() {
    let seeing = version(&[1, 2, 3], &[sees(1, 2), sees(2, 3)]);
    let measured = Metrics::of(&seeing);
    assert_eq!(measured.review_edges(), 0);
    assert_eq!(measured.reviewed_members(), 0);
    assert_eq!(measured.review_depth(), 0);
    assert_eq!(measured.blind_members(), 1);
}

// ---------------------------------------------------------------------------------------------
// 2. Aucune ne juge
// ---------------------------------------------------------------------------------------------

/// Il n'existe ni seuil, ni verdict, ni note.
///
/// Un seuil écrit ici deviendrait la définition d'une bonne organisation, alors que c'est une
/// question de politique et de portefeuille — et qu'un chiffre écrit en Rust a l'air d'une décision
/// prise.
///
/// Le test lit le **code**, commentaires retirés : expliquer pourquoi un seuil est absent le fait
/// apparaître dans la prose, et un test qui confondrait les deux interdirait d'écrire la raison.
#[test]
fn no_metric_carries_a_threshold_or_a_verdict() {
    let source = include_str!("../src/metrics.rs")
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "const MIN",
        "const MAX",
        "const THRESHOLD",
        "fn is_healthy",
        "fn score",
        "fn grade",
        "enum Verdict",
        "fn is_acceptable",
    ] {
        assert!(
            !source.contains(forbidden),
            "« {forbidden} » ferait de ce module la définition d'une bonne organisation"
        );
    }
}

/// **La métrique qui a failli être écartée à tort.**
///
/// `A relit B` et `B relit A` est un cycle de longueur deux, et `region` veto déjà
/// `ReviewAcyclicity` : la réciprocité avait donc l'air d'une métrique morte. Elle ne l'est pas. Le
/// veto s'applique à un **diff** ; `Version::root` ne refuse que les arêtes pendantes et les
/// auto-relations. Une version racine porte parfaitement l'aller-retour, et c'est exactement l'état
/// qu'aucune transition n'a gardé.
#[test]
fn a_root_version_can_carry_the_mutual_review_the_veto_forbids() {
    let mutual = Version::root(&[agent(1), agent(2)], &[reviews(1, 2), reviews(2, 1)], &Fnv)
        .expect(
            "`root` ne vérifie pas l'acyclicité — seul le veto de `region` le fait, sur un diff",
        );
    assert_eq!(Metrics::of(&mutual).mutual_review_pairs(), 1);
}

/// Elle est comptée par **paire**, jamais par arête.
#[test]
fn mutual_review_is_counted_by_pair_not_by_edge() {
    let one = version(&[1, 2], &[reviews(1, 2), reviews(2, 1)]);
    assert_eq!(Metrics::of(&one).mutual_review_pairs(), 1);
    assert_eq!(
        Metrics::of(&one).review_edges(),
        2,
        "deux arêtes, un aller-retour"
    );

    let two = version(
        &[1, 2, 3, 4],
        &[reviews(1, 2), reviews(2, 1), reviews(3, 4), reviews(4, 3)],
    );
    assert_eq!(Metrics::of(&two).mutual_review_pairs(), 2);
}

/// Une chaîne à sens unique n'a aucune réciprocité, et un anneau à trois non plus.
///
/// Un anneau `1 → 2 → 3 → 1` est un cycle — que `citation_cycles` de `R1` verrait dans son domaine —
/// mais aucune de ses arêtes n'a de retour direct. Les deux notions ne se confondent pas.
#[test]
fn a_chain_and_a_three_ring_have_no_mutual_pair() {
    let chain = version(&[1, 2, 3], &[reviews(1, 2), reviews(2, 3)]);
    assert_eq!(Metrics::of(&chain).mutual_review_pairs(), 0);

    let ring = version(&[1, 2, 3], &[reviews(1, 2), reviews(2, 3), reviews(3, 1)]);
    assert_eq!(Metrics::of(&ring).mutual_review_pairs(), 0);
}

/// Et la profondeur termine même sur la version qui porte le cycle.
///
/// Une métrique qui ne termine pas est pire qu'une métrique absente : elle emporte l'appelant.
#[test]
fn the_depth_terminates_on_a_cyclic_version() {
    let ring = version(&[1, 2, 3], &[reviews(1, 2), reviews(2, 3), reviews(3, 1)]);
    assert_eq!(Metrics::of(&ring).review_depth(), 3);

    let mutual = Version::root(&[agent(1), agent(2)], &[reviews(1, 2), reviews(2, 1)], &Fnv)
        .expect("`root` accepte l'aller-retour");
    assert_eq!(Metrics::of(&mutual).review_depth(), 2);
}

// ---------------------------------------------------------------------------------------------
// 3. Les quatre ensemble, en un passage
// ---------------------------------------------------------------------------------------------

/// `Metrics::of` est le seul constructeur, et il rend tout.
///
/// Les calculer à la demande ferait lire la même version cinq fois et permettrait d'en rapporter
/// quatre — la façon la plus discrète de choisir ce qu'on montre.
#[test]
fn the_five_are_rendered_together() {
    let source = include_str!("../src/metrics.rs")
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in ["Metrics::new", "impl Default for Metrics", "pub members:"] {
        assert!(
            !source.contains(forbidden),
            "« {forbidden} » laisserait fabriquer des métriques sans version"
        );
    }
    let constructions = source
        .match_indices("Self {")
        .filter(|(offset, _)| source[..*offset].contains("pub fn of"))
        .count();
    assert!(constructions >= 1);
}

/// Une version vide se mesure sans cas particulier.
#[test]
fn an_empty_organisation_measures_as_zero() {
    let empty = Version::root(&[], &[], &Fnv).expect("une organisation sans membre est licite");
    let measured = Metrics::of(&empty);
    assert_eq!(measured.members(), 0);
    assert_eq!(measured.reviewed_members(), 0);
    assert_eq!(measured.review_depth(), 0);
    assert_eq!(measured.busiest_reviewer_load(), 0);
    assert_eq!(measured.review_edges(), 0);
    assert_eq!(measured.mutual_review_pairs(), 0);
    assert_eq!(measured.blind_members(), 0);
}

/// Deux mesures de la même version sont identiques.
///
/// C'est ce que « calculable en rejeu sur fixtures identiques » exige de la moitié structurelle : une
/// métrique qui varierait d'un passage à l'autre rendrait tout regret incalculable.
#[test]
fn measuring_twice_gives_the_same_thing() {
    let organisation = version(
        &[1, 2, 3, 4],
        &[reviews(1, 2), reviews(2, 3), sees(4, 1), sees(1, 4)],
    );
    assert_eq!(Metrics::of(&organisation), Metrics::of(&organisation));
}
