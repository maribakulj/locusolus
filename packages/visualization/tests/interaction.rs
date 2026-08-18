//! Test de sortie de W9.c — **rien de ce qui revient d'un viewer n'écrit.**
//!
//! §23 / `docs/07` : « Emacs peut envoyer `focus`, `filter`, `select` ; le viewer renvoie
//! `node_selected`, `artifact_opened`, etc. Toute mutation passe ensuite par command API et
//! confirmation appropriée. »
//!
//! Le canal de retour est l'endroit où « la vue devient éditable en place » reviendrait si on la
//! chassait de la vue : non pas en modifiant la projection, mais en laissant le viewer **dire** au
//! control plane ce qu'un nœud vaut désormais. Un `node_selected` qui porterait un label
//! remplacerait une lecture par une écriture sans toucher au graphe.
//!
//! D'où la propriété testée : un événement de viewer porte une identité, et il n'existe aucun champ
//! où mettre autre chose.

use locus_domain::ContentHash;
use locus_visualization::{
    Digest, InteractionError, View, ViewEdge, ViewKind, ViewNode, ViewerCommand, ViewerEvent,
};

struct Fixture;

impl Digest for Fixture {
    fn digest(&self, canonical: &str) -> ContentHash {
        let seed = format!("{:02x}", canonical.len() % 256);
        ContentHash::parse(&format!("sha256:{}", seed.repeat(32))).expect("hash bien formé")
    }
}

fn node(id: &str, kind: &str) -> ViewNode {
    ViewNode {
        id: id.to_owned(),
        kind: kind.to_owned(),
        label: format!("{kind} {id}"),
    }
}

fn edge(from: &str, to: &str) -> ViewEdge {
    ViewEdge {
        from: from.to_owned(),
        to: to.to_owned(),
        kind: "supports".to_owned(),
    }
}

fn objection(from: &str, to: &str) -> ViewEdge {
    ViewEdge {
        from: from.to_owned(),
        to: to.to_owned(),
        kind: "refutes".to_owned(),
    }
}

/// `a → b → c`, une objection `x → a`, et un `d` isolé d'une autre sorte.
///
/// L'objection **entre** dans `a` : c'est le sens qu'un cadrage naïf oublie, et l'oubli est grave
/// — une objection pointe toujours vers ce qu'elle conteste, donc un `focus` qui ne suivrait que
/// le sens sortant montrerait un claim débarrassé de tout ce qui lui est opposé.
fn chaine() -> View {
    View::render(
        ViewKind::ArgumentMap,
        7,
        vec![
            node("a", "claim"),
            node("b", "claim"),
            node("c", "claim"),
            node("d", "artifact"),
            node("x", "claim"),
        ],
        vec![edge("a", "b"), edge("b", "c"), objection("x", "a")],
        &Fixture,
    )
    .expect("vue valide")
}

// ---------------------------------------------------------------------------------------------
// Le canal de retour ne transporte que des identités
// ---------------------------------------------------------------------------------------------

/// Le cœur de W9.c. Chaque événement a un sujet, et c'est tout ce qu'il a. Il n'existe pas
/// d'accesseur au « contenu » d'un événement parce qu'il n'y a pas de contenu : un consommateur qui
/// voudrait faire écrire un viewer devrait d'abord changer le type.
#[test]
fn un_evenement_de_viewer_porte_une_identite_et_rien_d_autre() {
    let selection = ViewerEvent::from_wire("node_selected", "claim-42").expect("événement valide");
    assert_eq!(selection.subject(), "claim-42");
    assert_eq!(selection.slug(), "node_selected");

    let ouverture =
        ViewerEvent::from_wire("artifact_opened", "art-0f3c").expect("événement valide");
    assert_eq!(ouverture.subject(), "art-0f3c");
    assert_eq!(ouverture.slug(), "artifact_opened");
}

/// Deux événements, pas « etc. ». Une sorte n'entre dans l'énumération que lorsqu'un consommateur
/// exécutable existe ; un `node_edited` accepté aujourd'hui serait le canal d'écriture que §23
/// interdit, ouvert par avance et sans que personne l'ait décidé.
#[test]
fn un_evenement_que_le_texte_ne_nomme_pas_est_refuse() {
    assert_eq!(
        ViewerEvent::from_wire("node_edited", "claim-42"),
        Err(InteractionError::UnknownEvent {
            name: "node_edited".to_owned()
        })
    );
    assert_eq!(
        ViewerEvent::from_wire("graph_committed", "claim-42"),
        Err(InteractionError::UnknownEvent {
            name: "graph_committed".to_owned()
        })
    );
}

#[test]
fn un_evenement_sans_sujet_est_refuse() {
    assert_eq!(
        ViewerEvent::from_wire("node_selected", "  "),
        Err(InteractionError::EmptyField { field: "subject" })
    );
}

#[test]
fn les_trois_commandes_de_23_existent_sous_leur_nom() {
    let commandes = [
        ViewerCommand::Focus {
            node: "a".to_owned(),
            depth: 1,
        },
        ViewerCommand::Filter {
            node_kinds: vec!["claim".to_owned()],
        },
        ViewerCommand::Select {
            nodes: vec!["a".to_owned()],
        },
    ];
    let slugs: Vec<&str> = commandes.iter().map(ViewerCommand::slug).collect();
    assert_eq!(slugs, vec!["focus", "filter", "select"]);
}

// ---------------------------------------------------------------------------------------------
// Une vue dérivée dit toujours d'où elle vient
// ---------------------------------------------------------------------------------------------

#[test]
fn une_vue_cadree_declare_son_parent() {
    let complete = chaine();
    let cadree = complete.focused("a", 1, &Fixture).expect("cadrage valide");

    assert_eq!(cadree.derived_from(), Some(complete.digest()));
    assert!(
        cadree
            .canonical()
            .contains(&format!("derived-from\t{}", complete.digest()))
    );
    assert_eq!(complete.derived_from(), None);
}

/// Sans exception — y compris quand le filtre ne retire rien. Une vue filtrée « à tout » qui aurait
/// la forme canonique de la projection deviendrait indiscernable d'elle dans un cache, et le jour
/// où le filtre change quelque chose, personne ne saurait dire lequel des deux on regarde.
#[test]
fn un_filtre_qui_ne_retire_rien_declare_quand_meme_son_parent() {
    let complete = View::render(
        ViewKind::ArgumentMap,
        7,
        vec![node("a", "claim"), node("b", "claim")],
        vec![edge("a", "b")],
        &Fixture,
    )
    .expect("vue valide");

    let filtree = complete
        .filtered(&["claim".to_owned()], &Fixture)
        .expect("filtre valide");

    assert_eq!(filtree.nodes(), complete.nodes());
    assert_eq!(filtree.edges(), complete.edges());
    assert_ne!(filtree.canonical(), complete.canonical());
    assert_eq!(filtree.derived_from(), Some(complete.digest()));
}

/// Le cadrage garde le voisinage demandé, et pas au-delà : `focus a` à un saut voit `b`, pas `c`.
#[test]
fn le_cadrage_s_arrete_a_la_profondeur_demandee() {
    let complete = chaine();

    let un_saut = complete.focused("a", 1, &Fixture).expect("cadrage");
    let vus: Vec<&str> = un_saut.nodes().iter().map(|n| n.id.as_str()).collect();
    assert_eq!(vus, vec!["a", "b", "x"]);

    let deux_sauts = complete.focused("a", 2, &Fixture).expect("cadrage");
    let vus: Vec<&str> = deux_sauts.nodes().iter().map(|n| n.id.as_str()).collect();
    assert_eq!(vus, vec!["a", "b", "c", "x"]);
}

/// Le voisinage se suit dans les **deux** sens. Une objection pointe vers ce qu'elle conteste ;
/// un cadrage qui ne suivrait que le sens sortant montrerait `a` sans rien de ce qui lui est
/// opposé — et l'invariant 12 dit que les conflits ne disparaissent pas pour faire propre.
#[test]
fn le_cadrage_suit_les_aretes_dans_les_deux_sens() {
    let cadree = chaine().focused("a", 1, &Fixture).expect("cadrage");
    let vus: Vec<&str> = cadree.nodes().iter().map(|n| n.id.as_str()).collect();
    assert!(
        vus.contains(&"x"),
        "l'objection x → a doit rester : {vus:?}"
    );
    assert!(
        cadree.edges().contains(&objection("x", "a")),
        "et l'arête qui la porte aussi"
    );
}

/// Ce qui sort du cadrage emporte ses arêtes. En garder une ferait supposer un nœud absent — et un
/// trait qui mène hors de l'écran est l'invitation la plus forte qu'une visualisation puisse faire.
#[test]
fn une_arete_qui_sort_du_cadrage_est_retiree_pas_laissee_pendante() {
    let cadree = chaine().focused("a", 1, &Fixture).expect("cadrage");
    assert_eq!(cadree.edges().len(), 2);
    assert!(cadree.edges().contains(&edge("a", "b")));
    // Rien ne mène à `c`, qui n'est pas là.
    assert!(
        !cadree
            .edges()
            .iter()
            .any(|edge| edge.to == "c" || edge.from == "c")
    );
}

#[test]
fn le_filtre_garde_les_sortes_demandees_et_leurs_aretes_internes() {
    let filtree = chaine()
        .filtered(&["claim".to_owned()], &Fixture)
        .expect("filtre");
    let vus: Vec<&str> = filtree.nodes().iter().map(|n| n.id.as_str()).collect();
    assert_eq!(vus, vec!["a", "b", "c", "x"]);
    assert_eq!(filtree.edges().len(), 3);

    let artefacts = chaine()
        .filtered(&["artifact".to_owned()], &Fixture)
        .expect("filtre");
    let vus: Vec<&str> = artefacts.nodes().iter().map(|n| n.id.as_str()).collect();
    assert_eq!(vus, vec!["d"]);
    assert!(artefacts.edges().is_empty());
}

/// Cadrer sur un nœud absent ne rend pas une vue « presque bonne » : elle est vide, et elle déclare
/// toujours son parent. Un cadrage silencieusement élargi à tout serait la pire réponse — il aurait
/// l'air de marcher.
#[test]
fn cadrer_sur_un_noeud_absent_rend_une_vue_vide_pas_la_vue_entiere() {
    let cadree = chaine()
        .focused("inexistant", 2, &Fixture)
        .expect("cadrage");
    assert!(cadree.nodes().is_empty());
    assert!(cadree.edges().is_empty());
    assert!(cadree.derived_from().is_some());
}

/// Une vue dérivée garde le genre et le watermark de son parent : elle montre moins de la **même**
/// chose au **même** instant. En changer un ferait d'un cadrage une autre projection.
#[test]
fn une_vue_derivee_garde_le_genre_et_l_instant_de_son_parent() {
    let complete = chaine();
    let cadree = complete.focused("a", 1, &Fixture).expect("cadrage");
    assert_eq!(cadree.kind(), complete.kind());
    assert_eq!(cadree.watermark(), complete.watermark());
}

/// `select` désigne, il ne réduit pas. Confondre les deux ferait disparaître de l'écran ce qu'on
/// voulait seulement montrer du doigt — et la vue elle-même n'est pas concernée.
#[test]
fn selectionner_n_est_pas_filtrer() {
    let commande = ViewerCommand::Select {
        nodes: vec!["a".to_owned()],
    };
    assert_eq!(commande.slug(), "select");
    // Il n'existe aucune opération de vue qui corresponde à `select` : la sélection vit dans le
    // canal d'interaction, pas dans la projection.
    let complete = chaine();
    assert_eq!(complete.nodes().len(), 5);
}
