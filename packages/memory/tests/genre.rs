//! Test de sortie de `W17.k` — **le genre, seconde dimension de la mémoire.**
//!
//! Sept propriétés, celles du tableau de `docs/10` :
//!
//! 1. les dix se lisent sous leur nom, et un onzième n'existe pas ;
//! 2. le type s'appelle `Genre` et non `Kind`, que `compaction` occupe déjà ;
//! 3. genre **et** niveau sont obligatoires — pas d'`Entry` sans genre ;
//! 4. aucune conversion entre genres n'est écrivable, et aucun trait générique ne les factorise ;
//! 5. une promotion change le niveau et laisse le genre, sur les dix ;
//! 6. un genre qui **contredit** un type résolu est refusé en nommant les deux, tandis qu'une clé
//!    qu'aucun oracle ne résout est **acceptée** ;
//! 7. le couple `(Formal, Vector)` est refusé à la construction du candidat, et `Ranking::of` est
//!    inchangé ; un objet `MetaMemory` ne soutient aucune conclusion.

use std::collections::BTreeMap;

use locus_domain::Confidentiality;
use locus_memory::{
    Candidate, Genre, GenreOracle, Level, MemoryError, Ranking, RetrievalError, Shelf, Signal,
    Substance, Unknowing,
};

// ---------------------------------------------------------------------------------------------
// 1 et 2 — la liste, et le nom
// ---------------------------------------------------------------------------------------------

/// Les dix, sous leur nom, et rien d'autre.
#[test]
fn les_dix_genres_se_lisent_sous_leur_nom() {
    let noms = [
        "episodic",
        "semantic",
        "formal",
        "negative",
        "procedural",
        "strategic",
        "literature",
        "computational",
        "coordination",
        "meta-memory",
    ];
    assert_eq!(Genre::ALL.len(), 10);
    for (genre, nom) in Genre::ALL.into_iter().zip(noms) {
        assert_eq!(genre.slug(), nom);
        assert_eq!(Genre::parse(nom), Some(genre));
        assert_eq!(genre.to_string(), nom);
    }

    // Un onzième n'existe pas, et un genre inconnu ne se rabat sur rien : le rabattre sur
    // `Semantic` ferait passer pour un claim validé quelque chose que personne n'a validé.
    assert_eq!(Genre::parse("épistémique"), None);
    assert_eq!(Genre::parse(""), None);
    assert_eq!(Genre::parse("Semantic"), None, "la casse compte");
}

/// **`Genre` et `Kind` cohabitent sans renommage à l'import.**
///
/// C'est la raison du nom, et le test la rend mécanique : `compaction::Kind` est exporté par ce
/// crate depuis longtemps. Si le genre s'était appelé `Kind`, cette ligne ne compilerait pas, et
/// chaque appelant aurait dû renommer — la duplication de vocabulaire sous une autre forme.
#[test]
fn genre_et_kind_cohabitent_dans_un_meme_use() {
    use locus_memory::{Genre as G, Kind};

    assert_eq!(G::ALL.len(), 10);
    // `Kind` existe toujours, et il désigne autre chose.
    let _: fn() -> Option<Kind> = || None;
}

// ---------------------------------------------------------------------------------------------
// 3 — les deux dimensions sont obligatoires
// ---------------------------------------------------------------------------------------------

/// **Une `Entry` porte les deux, et il n'y a pas de chemin pour n'en porter qu'une.**
///
/// `Entry` n'a aucun champ public et aucun constructeur : elle ne s'obtient que par `Shelf::store`,
/// qui exige le genre. C'est la même discipline que `W17.a` a posée pour le niveau.
#[test]
fn genre_et_niveau_sont_tous_deux_obligatoires() {
    let mut etagere = Shelf::new();
    let rangee = etagere
        .store(
            "clm-1",
            Level::Branch,
            Genre::Semantic,
            Substance::Canonical,
        )
        .expect("rangement licite");

    assert_eq!(rangee.level(), Level::Branch);
    assert_eq!(rangee.genre(), Genre::Semantic);
    assert_eq!(rangee.substance(), Substance::Canonical);

    // Le test d'absence : **une seule** construction d'`Entry` dans tout le module.
    //
    // La forme visée est celle qui construit — `= Entry {` — et non le jeton `Entry {`, qui
    // attrape aussi la déclaration et le bloc `impl`. Un compte qui ramasse trois choses
    // différentes ne dit rien de ce qu'on voulait savoir, et c'est ce qu'un premier essai a fait.
    let source = include_str!("../src/level.rs");
    assert_eq!(
        source.matches("= Entry {").count(),
        1,
        "une seconde construction serait un chemin qui contourne le genre"
    );
}

// ---------------------------------------------------------------------------------------------
// 4 et 5 — aucune conversion, et la promotion laisse le genre
// ---------------------------------------------------------------------------------------------

/// **Aucune conversion entre genres n'est écrivable**, et le test le tient par l'absence.
///
/// Une conversion de genre serait une conversion d'**autorité** : l'affirmation qu'un objet est vrai
/// pour une raison qui ne l'a jamais établi. Un objet formel ne devient pas sémantique parce qu'il
/// est beaucoup cité.
///
/// La factorisation par le haut est refusée dans le même mouvement : un trait générique « ce qui
/// peut être rangé, retrouvé, promu » reconstruirait la conversion en la rendant invisible.
#[test]
fn aucune_conversion_entre_genres_n_est_ecrivable() {
    let source = include_str!("../src/genre.rs");
    for interdit in [
        "impl From<Genre>",
        "fn promote",
        "fn convert",
        "fn as_semantic",
        "fn into_genre",
        "trait Memorable",
        "trait Storable",
    ] {
        assert!(
            !source.contains(interdit),
            "« {interdit} » dans genre.rs : une conversion de genre est une conversion d'autorité"
        );
    }
}

/// **Une promotion change le niveau et laisse le genre** — sur les dix, pas sur un échantillon.
#[test]
fn une_promotion_change_le_niveau_et_laisse_le_genre() {
    for genre in Genre::ALL {
        let mut etroite = Shelf::new();
        let range = etroite
            .store("k", Level::Branch, genre, Substance::Canonical)
            .expect("rangement licite");
        assert_eq!(range.genre(), genre);

        // Promouvoir, c'est ranger la même chose plus large. Le genre voyage tel quel.
        let mut large = Shelf::new();
        let promu = large
            .store("k", Level::Program, genre, Substance::Canonical)
            .expect("rangement licite");
        assert_eq!(promu.genre(), genre, "{genre} a changé en étant promu");
        assert!(promu.level().is_at_least_as_wide_as(Level::Branch));
    }
}

// ---------------------------------------------------------------------------------------------
// 6 — le désaccord est un refus, l'ignorance ne l'est pas
// ---------------------------------------------------------------------------------------------

/// Un oracle qui connaît quelques clés, et pas les autres.
struct Connu(BTreeMap<String, Genre>);

impl GenreOracle for Connu {
    fn genre_of(&self, key: &str) -> Option<Genre> {
        self.0.get(key).copied()
    }
}

/// **Le désaccord nomme les deux côtés**, parce qu'on doit savoir qui a tort.
///
/// Sans les deux, un opérateur ne peut pas trancher entre « la déclaration est fausse » et « le
/// résolveur est en retard », qui appellent des suites opposées.
#[test]
fn un_genre_qui_contredit_un_type_resolu_est_refuse_en_nommant_les_deux() {
    let oracle = Connu(
        [("neg-1".to_owned(), Genre::Negative)]
            .into_iter()
            .collect(),
    );
    let mut etagere = Shelf::new();

    let refus = etagere
        .store_checked(
            "neg-1",
            Level::Branch,
            Genre::Semantic,
            Substance::Canonical,
            &oracle,
        )
        .expect_err("le reste du système la tient pour un résultat négatif");

    assert_eq!(
        refus,
        MemoryError::GenreContradicted {
            key: "neg-1".to_owned(),
            declared: Genre::Semantic,
            known: Genre::Negative,
        }
    );
    let dit = refus.to_string();
    assert!(dit.contains("semantic"), "{dit}");
    assert!(dit.contains("negative"), "{dit}");

    // Et le refus n'a rien rangé.
    assert!(etagere.get("neg-1").is_none());
}

/// **L'ignorance n'est pas un démenti** — une clé qu'aucun oracle ne résout est acceptée.
///
/// La faute symétrique serait de refuser tout ce qu'on ne sait pas confirmer, ce qui rendrait la
/// mémoire inutilisable partout où le résolveur n'a rien à dire. C'est la règle que `xiiif` applique
/// déjà en ne collapsant pas `unverified` sur `broken`.
#[test]
fn une_cle_qu_aucun_oracle_ne_resout_est_acceptee() {
    let oracle = Connu(BTreeMap::new());
    let mut etagere = Shelf::new();

    assert!(
        etagere
            .store_checked(
                "inconnue",
                Level::Branch,
                Genre::Strategic,
                Substance::Projection,
                &oracle,
            )
            .is_ok()
    );

    // Et l'oracle par défaut, qui ne sait rien, accepte de la même façon.
    let mut autre = Shelf::new();
    assert!(
        autre
            .store_checked(
                "inconnue",
                Level::Branch,
                Genre::Strategic,
                Substance::Projection,
                &Unknowing,
            )
            .is_ok()
    );
    // Un accord, lui, passe aussi : le refus vise le désaccord, pas la vérification.
    let accord = Connu(
        [("sem-1".to_owned(), Genre::Semantic)]
            .into_iter()
            .collect(),
    );
    let mut troisieme = Shelf::new();
    assert!(
        troisieme
            .store_checked(
                "sem-1",
                Level::Branch,
                Genre::Semantic,
                Substance::Canonical,
                &accord,
            )
            .is_ok()
    );
}

// ---------------------------------------------------------------------------------------------
// 7 — les deux interdits d'autorité
// ---------------------------------------------------------------------------------------------

/// **`(Formal, Vector)` est refusé à la construction du candidat, et `Ranking::of` est inchangé.**
///
/// L'ordre compte : le `Ranking` se construit sans erreur — il ne connaît pas le candidat —, et
/// c'est l'attachement qui refuse. Poser le refus dans `Ranking::of` aurait demandé de lui passer le
/// genre ; le poser après aurait laissé exister un état intermédiaire invalide représentable.
#[test]
fn un_objet_formel_ne_se_classe_pas_par_similarite_vectorielle() {
    let score = Ranking::of(&[(Signal::Vector, 0.9)]).expect("un score vectoriel est un score");

    let refus = Candidate::new(
        "lemme-1",
        Confidentiality::Internal,
        Genre::Formal,
        score.clone(),
    )
    .expect_err("l'autorité d'un lemme est un vérificateur");
    assert_eq!(
        refus,
        RetrievalError::VectorOnFormal {
            key: "lemme-1".to_owned(),
        }
    );

    // Le même score sur un objet sémantique passe : c'est le **couple** qui est refusé.
    assert!(Candidate::new("claim-1", Confidentiality::Internal, Genre::Semantic, score).is_ok());

    // Et un objet formel classé autrement passe : l'interdit ne bannit pas le genre du retrieval.
    let par_graphe =
        Ranking::of(&[(Signal::GraphTraversal, 0.7)]).expect("un score de graphe est un score");
    assert!(
        Candidate::new(
            "lemme-1",
            Confidentiality::Internal,
            Genre::Formal,
            par_graphe
        )
        .is_ok()
    );
}

/// **Un objet `MetaMemory` influence le rang, jamais la validité.**
///
/// Ce crate ne connaît ni `Support` ni `Inference` — c'est `packages/graph` qui les tient —, et
/// l'interdit s'y applique par l'absence de conversion. Ce que ce crate peut tenir, et qu'il tient,
/// est le prédicat qu'un appelant interroge, et le fait qu'il ne dit « non » que pour un genre.
#[test]
fn un_objet_meta_memory_ne_soutient_aucune_conclusion() {
    assert!(!Genre::MetaMemory.may_support_a_conclusion());
    for genre in Genre::ALL {
        if genre != Genre::MetaMemory {
            assert!(
                genre.may_support_a_conclusion(),
                "{genre} devrait pouvoir soutenir une conclusion"
            );
        }
    }

    // Le pendant, pour l'autre interdit : `Formal` est le seul à refuser la similarité.
    assert!(!Genre::Formal.admits_vector_similarity());
    for genre in Genre::ALL {
        if genre != Genre::Formal {
            assert!(genre.admits_vector_similarity(), "{genre}");
        }
    }
}
