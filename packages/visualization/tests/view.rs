//! Test de sortie de W9.a — **une vue est un instantané, pas un raccourci vers le graphe.**
//!
//! `docs/SPEC_V1.md` §23.3 : « le graphe canonique n'est jamais envoyé brut à un viewer. Le service
//! produit des projections versionnées et hashées. »
//!
//! `docs/10` : « si une vue devient éditable en place, l'invariant *aucun frontend n'écrit
//! directement dans le graphe* est perdu. »
//!
//! La seconde phrase est celle qui se teste mal si on la lit comme une interdiction — un test ne
//! peut pas vérifier qu'une méthode n'existe pas. Elle se teste bien si on la lit comme une
//! identité : une vue modifiée n'est plus **cette** vue, sa forme canonique change, et elle ne peut
//! donc pas être présentée comme la projection dont elle vient.

use locus_domain::ContentHash;
use locus_visualization::{
    ContentDigest, Digest, Freshness, View, ViewEdge, ViewError, ViewKind, ViewNode,
};

/// Un condensat de fixture : il ne hache rien, il compte. Ce qui est éprouvé ici est la forme
/// canonique, et un vrai algorithme n'ajouterait qu'une couche opaque entre le test et ce qu'il
/// vérifie.
struct Counting(std::cell::RefCell<Vec<String>>);

impl Digest for Counting {
    fn digest(&self, canonical: &str) -> ContentHash {
        self.0.borrow_mut().push(canonical.to_owned());
        let seed = format!("{:02x}", canonical.len() % 256);
        ContentHash::parse(&format!("sha256:{}", seed.repeat(32))).expect("hash bien formé")
    }
}

fn digest() -> Counting {
    Counting(std::cell::RefCell::new(Vec::new()))
}

fn node(id: &str) -> ViewNode {
    ViewNode {
        id: id.to_owned(),
        kind: "claim".to_owned(),
        label: format!("Claim {id}"),
    }
}

fn edge(from: &str, to: &str) -> ViewEdge {
    ViewEdge {
        from: from.to_owned(),
        to: to.to_owned(),
        kind: "supports".to_owned(),
    }
}

fn view(nodes: Vec<ViewNode>, edges: Vec<ViewEdge>) -> View {
    View::render(ViewKind::ArgumentMap, 42, nodes, edges, &digest()).expect("vue valide")
}

// ---------------------------------------------------------------------------------------------
// Deux rendus du même contenu sont la même vue
// ---------------------------------------------------------------------------------------------

/// L'ordre d'insertion est un accident du producteur — un rebuild complet et un rattrapage
/// incrémental ne remplissent pas leurs vecteurs pareil. S'il entrait dans la forme canonique, deux
/// viewers montrant la même chose ne pourraient pas le prouver.
#[test]
fn l_ordre_d_insertion_ne_change_pas_la_vue() {
    let dans_un_sens = view(
        vec![node("a"), node("b"), node("c")],
        vec![edge("a", "b"), edge("b", "c")],
    );
    let dans_l_autre = view(
        vec![node("c"), node("a"), node("b")],
        vec![edge("b", "c"), edge("a", "b")],
    );

    assert_eq!(dans_un_sens.canonical(), dans_l_autre.canonical());
    assert_eq!(dans_un_sens.digest(), dans_l_autre.digest());
    assert_eq!(dans_un_sens.nodes(), dans_l_autre.nodes());
}

/// Le cœur de W9.a. Éditer une vue est possible - c'est de la donnée - mais le résultat cesse
/// d'être cette vue. Un frontend peut donc travailler sur ce qu'il a reçu ; il ne peut pas faire
/// passer le résultat pour la projection.
#[test]
fn une_vue_modifiee_n_est_plus_la_vue() {
    let originale = view(vec![node("a"), node("b")], vec![edge("a", "b")]);

    let mut retouches = originale.nodes().to_vec();
    retouches[1].label = "Claim b, corrigé à la main".to_owned();
    let modifiee = view(retouches, vec![edge("a", "b")]);

    assert_ne!(originale.canonical(), modifiee.canonical());
    assert_ne!(originale, modifiee);
    // Et l'originale n'a pas bougé : ce qui a été rendu reste ce qui a été rendu.
    assert_eq!(originale.nodes()[1].label, "Claim b");
}

/// Une arête ajoutée compte autant qu'un nœud : c'est la relation qui porte la lecture.
#[test]
fn une_arete_ajoutee_change_la_vue_aussi() {
    let sans = view(vec![node("a"), node("b"), node("c")], vec![edge("a", "b")]);
    let avec = view(
        vec![node("a"), node("b"), node("c")],
        vec![edge("a", "b"), edge("b", "c")],
    );
    assert_ne!(sans.canonical(), avec.canonical());
}

/// La forme canonique se lit. Deux implémentations dont les condensats diffèrent doivent pouvoir
/// dire **où** ; sans cela, un désaccord ne se diagnostique pas.
#[test]
fn la_forme_canonique_est_lisible_et_porte_la_version() {
    let rendue = view(vec![node("a")], vec![]);
    let lignes: Vec<&str> = rendue.canonical().lines().collect();
    assert_eq!(lignes[0], "view/1");
    assert_eq!(lignes[1], "argument_map");
    assert_eq!(lignes[2], "42");
    assert_eq!(lignes[3], "n\ta\tclaim\tClaim a");
}

/// Le condensat vient du port, sur la forme canonique et sur rien d'autre.
#[test]
fn le_condensat_passe_par_le_port_et_voit_la_forme_canonique() {
    let digest = digest();
    let rendue = View::render(ViewKind::Provenance, 7, vec![node("a")], vec![], &digest)
        .expect("vue valide");
    let vus = digest.0.borrow();
    assert_eq!(vus.len(), 1);
    assert_eq!(vus[0], rendue.canonical());
}

/// Le genre entre dans la forme canonique : la même matière lue comme provenance et comme carte
/// d'argumentation ne dit pas la même chose, et deux vues qui porteraient le même condensat
/// seraient interchangeables dans un cache.
#[test]
fn deux_genres_de_vue_ne_partagent_pas_un_condensat() {
    let comme_provenance =
        View::render(ViewKind::Provenance, 42, vec![node("a")], vec![], &digest()).expect("valide");
    let comme_argument = View::render(
        ViewKind::ArgumentMap,
        42,
        vec![node("a")],
        vec![],
        &digest(),
    )
    .expect("valide");
    assert_ne!(comme_provenance.canonical(), comme_argument.canonical());
}

/// Et le watermark aussi : la même matière à deux instants du journal est deux vues.
#[test]
fn deux_instants_ne_partagent_pas_un_condensat() {
    let tot =
        View::render(ViewKind::Provenance, 1, vec![node("a")], vec![], &digest()).expect("valide");
    let tard =
        View::render(ViewKind::Provenance, 2, vec![node("a")], vec![], &digest()).expect("valide");
    assert_ne!(tot.canonical(), tard.canonical());
}

// ---------------------------------------------------------------------------------------------
// Le retard se déclare
// ---------------------------------------------------------------------------------------------

#[test]
fn une_vue_en_retard_le_dit() {
    let rendue = view(vec![node("a")], vec![]);
    assert_eq!(rendue.freshness(42), Freshness::Current);
    assert_eq!(rendue.freshness(50), Freshness::Behind { by: 8 });
}

/// Le journal ne recule pas. Comparer une vue à un point antérieur à elle veut dire que le point de
/// comparaison est périmé, et répondre `Current` ferait passer cette méprise pour un accord.
#[test]
fn comparer_a_un_etat_plus_ancien_que_la_vue_est_signale() {
    assert_eq!(
        view(vec![node("a")], vec![]).freshness(1),
        Freshness::Inconsistent
    );
}

// ---------------------------------------------------------------------------------------------
// Ce qu'une vue refuse d'être
// ---------------------------------------------------------------------------------------------

/// Une arête vers un nœud absent invite le lecteur à supposer un objet que le graphe n'a pas — et
/// c'est le genre d'inférence qu'une visualisation rend irrésistible.
#[test]
fn une_arete_sans_extremite_est_refusee() {
    assert_eq!(
        View::render(
            ViewKind::Graph2d,
            1,
            vec![node("a")],
            vec![edge("a", "fantome")],
            &digest()
        ),
        Err(ViewError::DanglingEdge {
            endpoint: "fantome".to_owned()
        })
    );
    // Dans les deux sens : une arête qui *vient* de nulle part se lit aussi mal.
    assert!(matches!(
        View::render(
            ViewKind::Graph2d,
            1,
            vec![node("a")],
            vec![edge("fantome", "a")],
            &digest()
        ),
        Err(ViewError::DanglingEdge { .. })
    ));
}

/// §23 demande des IDs stables parce qu'une sélection doit désigner la même chose d'un rendu à
/// l'autre. Deux nœuds de même identité la rendent ambiguë, et l'ambiguïté se résoudrait
/// différemment selon le viewer.
#[test]
fn deux_noeuds_de_meme_identite_sont_refuses() {
    assert_eq!(
        View::render(
            ViewKind::Graph2d,
            1,
            vec![node("a"), node("a")],
            vec![],
            &digest()
        ),
        Err(ViewError::DuplicateNode)
    );
}

#[test]
fn une_identite_vide_est_refusee() {
    assert_eq!(
        View::render(ViewKind::Graph2d, 1, vec![node("  ")], vec![], &digest()),
        Err(ViewError::EmptyField { field: "node.id" })
    );
}

/// Une arête écrite deux fois est la même relation : la garder deux fois la ferait compter deux
/// fois, et un lecteur qui compte les soutiens d'un claim lirait un appui de plus.
#[test]
fn une_arete_repetee_ne_compte_qu_une_fois() {
    let rendue = view(
        vec![node("a"), node("b")],
        vec![edge("a", "b"), edge("a", "b")],
    );
    assert_eq!(rendue.edges().len(), 1);
}

// ---------------------------------------------------------------------------------------------
// Les huit de §23.3
// ---------------------------------------------------------------------------------------------

#[test]
fn les_huit_projections_de_23_3_existent_et_se_distinguent() {
    let slugs: Vec<&str> = ViewKind::ALL.iter().map(|kind| kind.slug()).collect();
    assert_eq!(
        slugs,
        vec![
            "graph_2d",
            "argument_map",
            "provenance",
            "dependencies",
            "disagreements",
            "semantic_space",
            "branch_landscape",
            "agent_society"
        ]
    );
    let mut uniques = slugs.clone();
    uniques.sort_unstable();
    uniques.dedup();
    assert_eq!(uniques.len(), 8);
}

// ---------------------------------------------------------------------------------------------
// Un champ de texte libre ne forge pas une ligne de la forme canonique
// ---------------------------------------------------------------------------------------------

/// **Deux vues différentes ne partagent pas un condensat**, et ce refus vient d'un balayage.
///
/// Le même défaut a d'abord été trouvé — et une collision construite — dans
/// `coordination::version`, où un rôle contenant une fin de ligne insérait une ligne de rôle dans
/// la forme canonique. Six champs de cette vue-ci sont du texte libre qui entre dans une forme à
/// lignes : `n\t…`, `e\t…`. Une vue d'un nœud dont l'étiquette porte une fin de ligne rendait la
/// même forme canonique qu'une vue de deux nœuds.
///
/// Les étiquettes viennent d'une projection, donc du journal, donc de texte qu'un agent a pu
/// écrire : la portée n'est pas hypothétique. Et le condensat est ce qui dit à un client qu'il
/// regarde la vue qu'il croit — §23 tient la sélection synchronisée dessus.
#[test]
fn un_champ_ne_forge_pas_une_ligne_de_la_forme_canonique() {
    let forge = ViewNode {
        id: "c1".to_owned(),
        kind: "claim".to_owned(),
        label: "Claim c1\nn\tc2\tclaim\tClaim c2".to_owned(),
    };
    assert_eq!(
        View::render(
            ViewKind::ArgumentMap,
            42,
            vec![forge],
            Vec::new(),
            &digest()
        )
        .expect_err("une étiquette qui forge une ligne est refusée"),
        ViewError::ForgedLine {
            field: "node.label"
        }
    );

    // Les six champs sont couverts, pas seulement l'étiquette : chacun entre dans la forme.
    for (mauvais, attendu) in [
        (
            ViewNode {
                id: "c1\tc2".to_owned(),
                kind: "claim".to_owned(),
                label: "x".to_owned(),
            },
            "node.id",
        ),
        (
            ViewNode {
                id: "c1".to_owned(),
                kind: "claim\nautre".to_owned(),
                label: "x".to_owned(),
            },
            "node.kind",
        ),
    ] {
        assert_eq!(
            View::render(
                ViewKind::ArgumentMap,
                42,
                vec![mauvais],
                Vec::new(),
                &digest()
            )
            .expect_err("un champ qui forge une ligne est refusé"),
            ViewError::ForgedLine { field: attendu }
        );
    }

    let arete = ViewEdge {
        from: "c1".to_owned(),
        to: "c2".to_owned(),
        kind: "supports\ne\tc1\tc2\trefutes".to_owned(),
    };
    assert_eq!(
        View::render(
            ViewKind::ArgumentMap,
            42,
            vec![node("c1"), node("c2")],
            vec![arete],
            &digest()
        )
        .expect_err("une arête qui forge une ligne est refusée"),
        ViewError::ForgedLine { field: "edge.kind" }
    );

    // Et une étiquette ordinaire — ponctuation, accents, espaces — reste licite.
    let ordinaire = ViewNode {
        id: "c1".to_owned(),
        kind: "claim".to_owned(),
        label: "Résultat négatif (réplication n° 3) — « à revoir »".to_owned(),
    };
    assert!(
        View::render(
            ViewKind::ArgumentMap,
            42,
            vec![ordinaire],
            Vec::new(),
            &digest()
        )
        .is_ok()
    );
}

/// **Le port a une implémentation de production** — ADR 0020.
///
/// `Digest` existait depuis `W17.e` et rien ne l'implémentait hors des fixtures : le condensat
/// d'une vue n'était calculable nulle part. Le port reste un port — `Counting` sert encore
/// ci-dessus — mais il a désormais une réponse par défaut, qui délègue à `ContentHash::of` plutôt
/// que de choisir un algorithme une seconde fois.
#[test]
fn le_port_de_condensat_a_une_implementation_de_production() {
    let rendue = View::render(
        ViewKind::ArgumentMap,
        42,
        vec![node("c1"), node("c2")],
        vec![edge("c1", "c2")],
        &ContentDigest,
    )
    .expect("vue valide");

    assert_eq!(rendue.digest().algorithm(), "sha256");
    assert_eq!(
        rendue.digest(),
        &ContentHash::of(rendue.canonical().as_bytes())
    );

    // Deux rendus du même contenu sont la même vue, et c'est ce que le condensat doit dire.
    let encore = View::render(
        ViewKind::ArgumentMap,
        42,
        vec![node("c2"), node("c1")],
        vec![edge("c1", "c2")],
        &ContentDigest,
    )
    .expect("vue valide");
    assert_eq!(rendue.digest(), encore.digest());
}
