//! Test de sortie de `W21.f` — **`degree_entropy`**, normalisée. ADR 0024.
//!
//! 1. Deux organisations de même forme et de tailles différentes rendent la **même** valeur.
//! 2. La métrique ne mesure **pas** l'équité de charge — une fixture où `R3` répond et pas elle.
//! 3. Les cas limites sont rendus explicitement, et le cas « aucun membre » est **inexprimable**.
//! 4. Rien ne juge.

use std::fmt::Write as _;

use locus_coordination::{
    CoordinationMode, DegreeEntropy, Digest, Metrics, Operation, Relation, RelationKind, Version,
    VersionError,
};
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
    Version::root(
        &members,
        relations,
        CoordinationMode::Blackboard,
        None,
        &Fnv,
    )
    .expect("la fixture est licite")
}

/// Un cycle de `n` nœuds : chaque nœud a exactement deux arêtes incidentes.
fn cycle(n: u8) -> Version {
    let members: Vec<u8> = (1..=n).collect();
    let relations: Vec<Relation> = (1..=n).map(|i| reviews(i, i % n + 1)).collect();
    version(&members, &relations)
}

// ---------------------------------------------------------------------------------------------
// 1. La normalisation, exercée
// ---------------------------------------------------------------------------------------------

/// **Même forme, tailles différentes, même valeur.**
///
/// Le test qui porte l'item. Sans normalisation, l'entropie brute croîtrait avec `n` et comparer
/// deux organisations reviendrait à comparer leurs tailles en croyant comparer leurs structures.
///
/// L'égalité est **stricte** : c'est ce que l'arrondi à `1e-9` achète. Personne ne compare deux
/// organisations à epsilon près, et un lecteur qui verrait `0.999999999999` et `1.0` conclurait à
/// une différence.
#[test]
fn deux_tailles_de_la_meme_forme_rendent_la_meme_valeur() {
    let petit = DegreeEntropy::of(&cycle(3));
    let moyen = DegreeEntropy::of(&cycle(6));
    let grand = DegreeEntropy::of(&cycle(12));

    assert_eq!(petit, moyen);
    assert_eq!(moyen, grand);
    assert_eq!(
        petit.value(),
        Some(1.0),
        "un graphe régulier est la dispersion parfaite"
    );
}

/// **Une forme concentrée rend moins qu'une forme régulière de même taille.**
///
/// À nombre de nœuds égal, l'étoile disperse moins que le cycle. C'est le sens de la mesure, et il
/// faut le vérifier : une métrique normalisée qui rendrait la même valeur partout serait morte.
#[expect(
    clippy::float_cmp,
    reason = "l'égalité stricte est la propriété testée : la valeur est arrondie à 1e-9 \
précisément pour que deux mesures de la même forme soient égales et non presque égales"
)]
#[test]
fn une_etoile_disperse_moins_qu_un_cycle_de_meme_taille() {
    let etoile = version(
        &[1, 2, 3, 4, 5],
        &[reviews(1, 2), reviews(1, 3), reviews(1, 4), reviews(1, 5)],
    );

    let dispersee = DegreeEntropy::of(&cycle(5)).value().expect("mesurable");
    let concentree = DegreeEntropy::of(&etoile).value().expect("mesurable");

    assert!(
        concentree < dispersee,
        "l'étoile ({concentree}) devrait disperser moins que le cycle ({dispersee})"
    );
    assert_eq!(dispersee, 1.0);
}

/// **Une organisation où seuls certains agents participent se mesure, et vaut moins.**
///
/// Trouvé par un mutant survivant : rien n'exerçait le mélange de nœuds connectés et de nœuds
/// isolés, alors que c'est un cas parfaitement ordinaire. Un degré nul donne `0 × ln 0`, soit
/// **`NaN`** — et un `NaN` ne se compare à rien, pas même à lui-même : il se serait propagé dans
/// tout tableau de bord sans qu'aucune assertion ne le retienne.
///
/// La valeur attendue vérifie aussi le sens : quatre membres dont deux isolés dispersent **moitié
/// moins** qu'un graphe régulier, parce que les isolés font partie de l'organisation et n'y
/// apportent rien.
#[expect(
    clippy::float_cmp,
    reason = "l'égalité stricte est la propriété testée : la valeur est arrondie à 1e-9 \
précisément pour que deux mesures de la même forme soient égales et non presque égales"
)]
#[test]
fn des_noeuds_isoles_parmi_des_noeuds_relies_se_mesurent() {
    let partiel = version(&[1, 2, 3, 4], &[reviews(1, 2)]);

    let mesure = DegreeEntropy::of(&partiel).value().expect("mesurable");

    assert!(
        mesure.is_finite(),
        "un degré nul a produit un NaN : {mesure}"
    );
    assert_eq!(mesure, 0.5);
}

// ---------------------------------------------------------------------------------------------
// 2. Ce que la métrique ne mesure pas
// ---------------------------------------------------------------------------------------------

/// **Entropie élevée et charge totalement concentrée, sur la même version.**
///
/// L'étoile a une entropie de `0.861` — proche du `1.0` d'un cycle — pendant que
/// `busiest_reviewer_load` passe de `1` à `4`. Les deux nombres regardent **les mêmes arêtes** et
/// répondent à deux questions différentes.
///
/// Un lecteur qui chercherait un goulot avec l'entropie ne le trouverait pas ; c'est la
/// concentration de `R3` qui répond. La phrase de la documentation est donc exercée plutôt que crue
/// sur parole.
#[test]
fn l_entropie_ne_mesure_pas_l_equite_de_charge() {
    let etoile = version(
        &[1, 2, 3, 4, 5],
        &[reviews(1, 2), reviews(1, 3), reviews(1, 4), reviews(1, 5)],
    );
    let regulier = cycle(5);

    let entropie_etoile = DegreeEntropy::of(&etoile).value().expect("mesurable");
    let entropie_cycle = DegreeEntropy::of(&regulier).value().expect("mesurable");
    let charge_etoile = Metrics::of(&etoile).busiest_reviewer_load();
    let charge_cycle = Metrics::of(&regulier).busiest_reviewer_load();

    assert!(
        entropie_etoile > 0.8,
        "l'entropie de l'étoile reste élevée : {entropie_etoile}"
    );
    assert!(
        entropie_cycle - entropie_etoile < 0.2,
        "les deux entropies sont proches"
    );

    assert_eq!(charge_cycle, 1);
    assert_eq!(charge_etoile, 4);
    assert!(
        charge_etoile > charge_cycle * 3,
        "la concentration, elle, sépare nettement les deux"
    );
}

// ---------------------------------------------------------------------------------------------
// 3. Les cas limites, et celui qui n'existe pas
// ---------------------------------------------------------------------------------------------

/// **Un seul membre : `SingleMember`, jamais une division par `ln 1 = 0`.**
#[test]
fn un_seul_membre_est_rendu_explicitement() {
    let seul = version(&[1], &[]);

    assert_eq!(DegreeEntropy::of(&seul), DegreeEntropy::SingleMember);
    assert_eq!(DegreeEntropy::of(&seul).value(), None);
}

/// **Des membres sans arête : `NoEdges`, et c'est autre chose que `SingleMember`.**
///
/// Ici la structure **pourrait** exister, elle est simplement vide. Fondre les deux ferait lire
/// « rien à mesurer » sur deux organisations dont l'une a des agents qui ne se parlent pas — ce qui
/// est précisément un constat.
#[test]
fn des_membres_sans_arete_sont_un_cas_distinct() {
    let isoles = version(&[1, 2, 3], &[]);

    assert_eq!(DegreeEntropy::of(&isoles), DegreeEntropy::NoEdges);
    assert_ne!(DegreeEntropy::of(&isoles), DegreeEntropy::SingleMember);
    assert_eq!(DegreeEntropy::of(&isoles).value(), None);
}

/// **Le cas « aucun membre » est inexprimable, et les deux chemins qui y mèneraient sont fermés.**
///
/// Une première rédaction portait une variante pour lui. Elle annonçait un cas que rien ne peut
/// produire — une promesse, au sens de la décision 0 de l'ADR 0022. Ce test tient l'absence par les
/// deux chemins plutôt que par l'énumération : construire une version vide, et vider une version
/// existante.
#[test]
fn une_version_sans_membre_est_inexprimable() {
    let vide = Version::root(&[], &[], CoordinationMode::Blackboard, None, &Fnv);
    assert!(
        matches!(vide, Err(VersionError::NoMembers)),
        "une racine sans membre doit être refusée"
    );

    let seul = version(&[1], &[]);
    let vidage = seul.apply(&Operation::RemoveNode(agent(1)), &Fnv);
    assert!(
        matches!(vidage, Err(VersionError::NoMembers)),
        "retirer le dernier nœud doit être refusé"
    );
}

// ---------------------------------------------------------------------------------------------
// 4. Rien ne juge
// ---------------------------------------------------------------------------------------------

/// **Aucun seuil, aucune note, aucun verdict.**
///
/// Une entropie basse n'est pas une faute : une organisation en étoile est parfois exactement ce
/// qu'on veut. Les motifs visent des signatures.
#[test]
fn la_source_ne_porte_aucun_jugement() {
    let source = include_str!("../src/entropy.rs");

    for interdit in [
        "const MIN",
        "const MAX",
        "fn is_healthy",
        "fn score",
        "enum Verdict",
        "fn verdict",
        "fn is_balanced",
    ] {
        assert!(
            !source.contains(interdit),
            "« {interdit} » ferait de cette dispersion un jugement"
        );
    }
}
