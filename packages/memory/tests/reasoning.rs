//! Test de sortie de `W26.a` — **la trace de raisonnement comme artefact.**
//!
//! Trois clauses, celles de la roadmap :
//!
//! 1. elle entre par le chemin de §9.1 — déclarée **avant** dépôt, hashée, référencée par son
//!    condensat — et **aucun second stockage n'apparaît**, tenu par l'absence comme `W16.e` le tient
//!    pour les messages ;
//! 2. elle est rangée en `Level::AgentPrivate` et `Genre::MetaMemory`, et un test refuse tout autre
//!    couple ;
//! 3. **aucun résumé n'est stocké à la place**, et un test d'absence refuse toute signature qui
//!    condenserait avant écriture — un résumé est une lecture, il se refait.
//!
//! # Ce que l'item corrige, et qui n'était pas un oubli
//!
//! Le dépôt savait détecter la fuite, avait le rayonnage privé et le genre qui empêche la
//! contamination épistémique — et **rien n'écrivait le raisonnement nulle part**. L'invariant 11
//! borne un ensemble de lecteurs ; lu comme un ordre de destruction, il fait disparaître la seule
//! chose qu'aucun audit ne rattrape.

use locus_artifacts::{ArtifactState, ProducedBy};
use locus_domain::{Confidentiality, ContentHash};
use locus_memory::{Genre, Level, Trace};

fn hash(byte: &str) -> ContentHash {
    ContentHash::parse(&format!("sha256:{}", byte.repeat(32))).expect("hash bien formé")
}

fn produite() -> ProducedBy {
    ProducedBy::new("tsk_catalyseur", 3)
}

fn trace() -> Trace {
    Trace::declaring("art_raisonnement", hash("ab"), 4_096, produite())
        .expect("la déclaration est bien formée")
}

// ---------------------------------------------------------------------------------------------
// 1. Le chemin de §9.1, et pas de second stockage
// ---------------------------------------------------------------------------------------------

/// **Déclarée avant dépôt**, hashée, référencée par son condensat.
///
/// ADR 0005 : le condensat déclaré d'abord est une **promesse** que l'arrivée confronte. Un
/// manifeste bâti après coup sur le contenu reçu ne dirait que « ce qui est arrivé est ce qui est
/// arrivé ».
#[test]
fn une_trace_est_declaree_avant_son_contenu() {
    let trace = trace();
    let manifeste = trace.manifest();

    assert_eq!(manifeste.state(), ArtifactState::Declared);
    assert_eq!(manifeste.declared_hash(), &hash("ab"));
    assert_eq!(manifeste.artifact_id(), "art_raisonnement");
    assert_eq!(manifeste.size_bytes(), 4_096);
}

/// Le module **ne stocke rien** : il rend un manifeste, et le contenu suit le chemin des artefacts.
///
/// C'est ce que `W16.e` tient pour les messages — « la messagerie demeure un usage du journal, aucun
/// second stockage durable ». Un stockage de traces serait un endroit de plus où chercher, et un
/// endroit de plus à oublier de purger.
///
/// Tenu par l'absence dans la source : ni carte, ni registre, ni vecteur de traces.
#[test]
fn aucun_second_stockage_n_apparait() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/reasoning.rs"))
        .expect("le module de production est lisible depuis son propre crate");
    let code: String = source
        .lines()
        .filter(|ligne| !ligne.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    for interdit in [
        "BTreeMap", "HashMap", "Vec<", "BTreeSet", "HashSet", "store", "insert",
    ] {
        assert!(
            !code.contains(interdit),
            "« {interdit} » : la trace voyage par le chemin des artefacts, ce module n'en retient \
             aucune"
        );
    }
}

/// La classification borne **qui reçoit l'octet**, et elle est au plus étroit.
///
/// Distincte du niveau, qui borne **quel rayonnage l'indexe**. Une trace a besoin des deux : sans la
/// classification, un worker peu habilité recevrait le contenu ; sans le niveau, un rayonnage plus
/// large la référencerait.
#[test]
fn la_classification_est_au_plus_etroit_et_n_est_pas_le_niveau() {
    let trace = trace();
    assert_eq!(
        trace.manifest().classification(),
        Confidentiality::Restricted
    );
    assert_eq!(trace.level(), Level::AgentPrivate);
}

// ---------------------------------------------------------------------------------------------
// 2. Le couple est fixé, et tout autre est inconstructible
// ---------------------------------------------------------------------------------------------

/// **`AgentPrivate` et `MetaMemory`, jamais autre chose.**
///
/// Le niveau : le plus étroit des sept de §16.1. Rangée plus large, une trace serait lisible par ceux
/// que l'invariant 11 exclut, et la fuite cesserait d'être une anomalie pour devenir le
/// fonctionnement.
///
/// Le genre : `MetaMemory` « influence le rang, jamais la validité ». En `Episodic` ou `Semantic`,
/// une trace entrerait dans ce qui fonde des claims, et le raisonnement d'un générateur deviendrait
/// une source — la contamination épistémique que le genre existe pour empêcher.
#[test]
fn le_couple_est_agent_private_et_meta_memory() {
    let trace = trace();
    assert_eq!(trace.level(), Level::AgentPrivate);
    assert_eq!(trace.genre(), Genre::MetaMemory);
}

/// **Aucun autre couple n'est exprimable**, et c'est plus fort que « refusé ».
///
/// Le niveau et le genre ne sont pas des paramètres : ils sont posés par le constructeur. Un test qui
/// vérifierait un refus supposerait qu'on puisse les demander — ce qui serait déjà un endroit où se
/// tromper. La vérification porte donc sur la **signature**, lue dans la source.
///
/// C'est le même arbitrage que `W24.a`, dont le test compte les portes d'entrée plutôt que d'essayer
/// d'en franchir une qui n'existe pas.
#[test]
fn aucun_autre_couple_n_est_exprimable() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/reasoning.rs"))
        .expect("le module de production est lisible depuis son propre crate");
    let code: Vec<&str> = source
        .lines()
        .filter(|ligne| !ligne.trim_start().starts_with("//"))
        .collect();

    // Aucune signature ne prend un niveau ou un genre : ils ne s'offrent pas au choix.
    for interdit in ["level: Level", "genre: Genre", "Level,", "Genre,"] {
        assert!(
            !code.join("\n").contains(interdit),
            "« {interdit} » offrirait le rangement au choix de l'appelant"
        );
    }

    // Et les deux accesseurs rendent des constantes, pas des champs.
    let constants: Vec<&&str> = code
        .iter()
        .filter(|ligne| {
            ligne.contains("Level::AgentPrivate") || ligne.contains("Genre::MetaMemory")
        })
        .collect();
    assert_eq!(
        constants.len(),
        2,
        "un couple posé une fois chacun : {constants:?}"
    );
}

/// Le genre choisi est bien celui que l'ADR 0022 décrit, et pas un homonyme.
///
/// `Genre::ALL` porte dix valeurs, et `MetaMemory` en est une : le test le confronte à l'énumération
/// plutôt qu'à sa seule valeur, pour que le retrait du genre du domaine fasse rougir ici.
#[test]
fn le_genre_choisi_appartient_bien_a_l_enumeration_close() {
    assert_eq!(Genre::ALL.len(), 10);
    assert!(Genre::ALL.contains(&trace().genre()));
    assert!(Level::ALL.contains(&trace().level()));
    assert_eq!(
        Level::ALL[0],
        Level::AgentPrivate,
        "le plus étroit de §16.1 est bien le premier"
    );
}

// ---------------------------------------------------------------------------------------------
// 3. Aucun résumé stocké à la place
// ---------------------------------------------------------------------------------------------

/// **Aucune signature ne condense avant écriture.**
///
/// Un résumé est une **lecture**, et une lecture se refait. Condenser avant écriture décide une fois
/// pour toutes de ce qui méritait d'être gardé, au moment précis où personne ne sait encore quelle
/// question sera posée — et ce qui a été jeté ne se retrouve pas.
///
/// Tenu par l'absence : la propriété n'est pas « personne n'a résumé », qui se relit à chaque revue,
/// mais « personne ne **peut** ».
#[test]
fn aucune_signature_ne_condense_avant_ecriture() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/reasoning.rs"))
        .expect("le module de production est lisible depuis son propre crate");
    let code: String = source
        .lines()
        .filter(|ligne| !ligne.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    for interdit in [
        "fn summar",
        "fn condense",
        "fn abridge",
        "fn compact",
        "fn truncate",
        "fn digest_of",
        "Summary",
    ] {
        assert!(
            !code.contains(interdit),
            "« {interdit} » : un résumé est une lecture, et il se refait — le stocker à la place \
             jetterait ce que personne n'a encore su vouloir"
        );
    }
}

/// Le type MIME est **posé**, identique pour toutes, et c'est du texte.
///
/// Trouvé par un mutant survivant : `text/plain` → `application/octet-stream` ne faisait rougir
/// aucun test, alors que l'en-tête du module s'appuie sur la propriété — « deux traces de types
/// différents seraient deux choses, et le lecteur institutionnel de `W26.b` devrait alors savoir
/// laquelle il lit ». C'était une prose qui affirmait ce que le code ne tenait pas.
///
/// Deux choses, et la seconde n'est pas la première : **le même** pour deux traces déclarées
/// différemment — ce qui est ce dont `W26.b` a besoin —, et **du texte** — ce sans quoi un lecteur
/// aurait des octets et pas un raisonnement.
#[test]
fn le_type_mime_est_pose_le_meme_et_lisible() {
    let une = trace();
    let autre = Trace::declaring("art_autre", hash("ef"), 12, ProducedBy::new("tsk_autre", 1))
        .expect("la déclaration est bien formée");

    assert_eq!(
        une.manifest().media_type(),
        autre.manifest().media_type(),
        "deux traces de types différents seraient deux choses à lire"
    );
    assert_eq!(une.manifest().media_type(), "text/plain");

    // Et il ne s'offre pas au choix, pour la même raison que le niveau et le genre.
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/reasoning.rs"))
        .expect("le module de production est lisible depuis son propre crate");
    let code: String = source
        .lines()
        .filter(|ligne| !ligne.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for interdit in ["media_type:", "mime:", "content_type:"] {
        assert!(
            !code.contains(interdit),
            "« {interdit} » offrirait le type au choix de l'appelant"
        );
    }
}

/// La taille déclarée est celle de la **trace entière**, et une taille nulle est refusée.
///
/// Le pendant exécutable de la clause 3 : une trace de taille zéro serait la façon la plus discrète
/// de n'écrire qu'un résumé — c'est-à-dire rien. `ArtifactManifest::declare` la refuse déjà, et ce
/// test vérifie que la trace hérite bien du refus plutôt que de le contourner.
#[test]
fn une_trace_vide_est_refusee() {
    assert!(Trace::declaring("art_vide", hash("cd"), 0, produite()).is_err());
    assert!(Trace::declaring("", hash("cd"), 10, produite()).is_err());
    assert!(Trace::declaring("art_ok", hash("cd"), 10, produite()).is_ok());
}
