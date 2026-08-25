//! Test de sortie de `W24.c` — **la fiabilité observée, une seule polarité.**
//!
//! Trois clauses, celles de la roadmap :
//!
//! 1. une seule convention dans tout le module — un test refuse qu'un seuil se compare dans un sens
//!    à un endroit et dans l'autre ailleurs ;
//! 2. le nom du type dit son sens, et ce qui compte des fautes ne s'appelle pas réputation ;
//! 3. la fiabilité influence le **rang**, jamais la validité — aucun chemin ne mène d'une
//!    observation vers un `Support` ou une prémisse d'`Inference`, tenu par l'absence.
//!
//! # Le défaut que cet item rend impossible
//!
//! L'ADR 0026 le nomme : chez la source, `s^F = 1` signifie *faute*, donc `E[P]` est une probabilité
//! de **mauvais** comportement filtrée par `E[P] < τ`, tandis que `T` est admissible si `E[T] ≥ τ` —
//! deux polarités opposées sur la même machinerie Beta. Les transcrire produirait un filtre inversé
//! en silence : le code compile, la moitié des tests passe, et le système retient exactement les pairs
//! qu'il devait écarter.

use locus_domain::{Confidentiality, ContentHash, RevisionId};
use locus_protocol::{Id, IdKind, Timestamp, id::Agent};
use locus_review::{Audience, Observation, Peer, Recipient, Reliability, Subscription};
use locus_review::{ContextItem, ContextView};

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn revision(seed: u8) -> RevisionId {
    id::<locus_domain::ids::RevisionKind>(seed)
}

fn hash(byte: &str) -> ContentHash {
    ContentHash::parse(&format!("sha256:{}", byte.repeat(32))).expect("hash bien formé")
}

fn destinataire(seed: u8) -> Recipient {
    Recipient {
        agent_id: id::<Agent>(seed),
        worker_id: format!("vm-{seed:02}"),
        blind_to_generator: true,
        clearance: Confidentiality::Internal,
    }
}

fn pair(seed: u8) -> Peer {
    let item = ContextItem {
        revision: revision(1),
        is_generator_reasoning: false,
        is_refuted: false,
        classification: Confidentiality::Internal,
        cites: Vec::new(),
        is_external_source: true,
        produced_by: Some(id::<Agent>(1)),
        disclosed: None,
    };
    let vue = ContextView::build(
        &[(item, 1)],
        &destinataire(seed),
        10,
        hash("ab"),
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect("la fixture est cohérente");
    Peer::authorised(destinataire(seed), Subscription::derived_from(&vue))
}

/// Observer `reliable` fois bien et `unreliable` fois mal.
fn observee(reliable: u32, unreliable: u32) -> Reliability {
    let mut fiabilite = Reliability::unobserved();
    for _ in 0..reliable {
        fiabilite = fiabilite.observing(Observation::Reliable);
    }
    for _ in 0..unreliable {
        fiabilite = fiabilite.observing(Observation::Unreliable);
    }
    fiabilite
}

// ---------------------------------------------------------------------------------------------
// 1. Une seule polarité
// ---------------------------------------------------------------------------------------------

/// **Plus c'est grand, plus c'est fiable** — sans exception.
///
/// C'est la convention unique, et ce test la vérifie sur la monotonie plutôt que sur un cas : une
/// observation fiable ne fait jamais baisser l'espérance, une observation non fiable ne la fait
/// jamais monter.
#[test]
fn observer_du_bon_ne_fait_jamais_baisser_et_reciproquement() {
    let mut fiabilite = Reliability::unobserved();
    for _ in 0..20 {
        let avant = fiabilite.expected_per_mille();
        fiabilite = fiabilite.observing(Observation::Reliable);
        assert!(
            fiabilite.expected_per_mille() >= avant,
            "une observation fiable ne fait pas baisser : {avant} → {}",
            fiabilite.expected_per_mille()
        );
    }
    for _ in 0..20 {
        let avant = fiabilite.expected_per_mille();
        fiabilite = fiabilite.observing(Observation::Unreliable);
        assert!(
            fiabilite.expected_per_mille() <= avant,
            "une observation non fiable ne fait pas monter : {avant} → {}",
            fiabilite.expected_per_mille()
        );
    }
}

/// **Rien vu n'est pas mauvais vu.** Sans observation, l'espérance est au milieu.
///
/// C'est la règle 3 du rythme de session transposée au domaine : un compteur qui n'a rien lu ne vaut
/// pas zéro. Un pair jamais observé qui partirait de `0` serait écarté par tout seuil, donc jamais
/// observé — un piège dont on ne sort pas.
#[test]
fn rien_observe_vaut_le_milieu_et_pas_zero() {
    let vierge = Reliability::unobserved();
    assert_eq!(vierge.observed(), 0);
    assert_eq!(vierge.expected_per_mille(), 500);
    assert!(vierge.admits(500));
    assert!(!vierge.admits(501));
}

/// Le seuil se compare **dans un seul sens**, et le module n'en porte pas d'autre.
///
/// La clause dit « un test refuse qu'un seuil se compare dans un sens à un endroit et dans l'autre
/// ailleurs ». La vérification est dans la **source** : c'est une propriété du module, pas d'une
/// exécution, et aucun jeu de valeurs ne la montrerait — un second `admits` de polarité inverse
/// passerait tous les tests de comportement du premier.
#[test]
fn le_module_ne_porte_qu_une_comparaison_de_seuil() {
    let source =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/reliability.rs"))
            .expect("le module de production est lisible depuis son propre crate");
    let code: Vec<&str> = source
        .lines()
        .filter(|ligne| !ligne.trim_start().starts_with("//"))
        .collect();

    // `->` porte un `>` qui n'est pas une comparaison : la première rédaction de ce test comptait
    // la signature d'`admits` comme une seconde comparaison. Le retirer d'abord.
    let sans_fleche: Vec<String> = code.iter().map(|ligne| ligne.replace("->", "")).collect();
    let comparaisons: Vec<&String> = sans_fleche
        .iter()
        .filter(|ligne| ligne.contains("threshold"))
        .filter(|ligne| {
            ligne.contains(">=")
                || ligne.contains("<=")
                || ligne.contains('<')
                || ligne.contains('>')
        })
        .collect();
    assert_eq!(
        comparaisons.len(),
        1,
        "une seule comparaison de seuil, et ce sont celles-ci : {comparaisons:?}"
    );
    assert!(
        comparaisons[0].contains(">="),
        "et elle admet ce qui est *au moins* aussi fiable : {}",
        comparaisons[0].trim()
    );
}

/// L'admission est **exacte** au bord, et elle ne passe pas par les millièmes arrondis.
///
/// Deux pairs séparés par moins d'un millième doivent rester discernables : arrondir avant de
/// comparer rendrait le seuil dépendant de la troncature, donc deux rejeux du même journal pourraient
/// trancher différemment.
#[test]
fn l_admission_est_exacte_au_bord() {
    // 2 fiables, 1 non : (2+1)/(3+2) = 0,600 exactement.
    let fiabilite = observee(2, 1);
    assert_eq!(fiabilite.expected_per_mille(), 600);
    assert!(fiabilite.admits(600));
    assert!(!fiabilite.admits(601));

    // 1 fiable, 1 non : (1+1)/(2+2) = 0,500.
    let mediane = observee(1, 1);
    assert_eq!(mediane.expected_per_mille(), 500);
    assert!(mediane.admits(500));
    assert!(!mediane.admits(501));
}

/// `expected_per_mille` **tronque**, et c'est ce qui rend `admits` équivalent à une comparaison sur
/// les millièmes — aujourd'hui.
///
/// # Pourquoi ce test existe
///
/// Un mutant remplaçant la comparaison exacte d'`admits` par `expected_per_mille() >= threshold` a
/// **survécu**, et il avait raison de survivre : `floor(x) ≥ t ⟺ x ≥ t` pour un `t` entier. Le
/// commentaire du module affirmait le contraire ; c'est le commentaire qui était faux.
///
/// Ce qui reste vrai est plus étroit : l'équivalence dépend de la **troncature**. Si
/// `expected_per_mille` passait à l'arrondi au plus proche, `admits` deviendrait plus permissif d'un
/// demi-millième sans que personne l'ait décidé. Ce test fige donc la troncature, pour que ce soit
/// ce fichier-ci qui rougisse le jour où elle change.
#[test]
fn l_esperance_tronque_et_c_est_de_cela_que_depend_l_equivalence() {
    // 1 fiable, 0 non : (1+1)/(0+2)… non, prenons un rationnel non décimal.
    // 1 fiable, 1 non fiable, 1 fiable → 2 fiables sur 3 : (2+1)/(3+2) = 0,6 exact. Pas celui-là.
    // 1 fiable seul : (1+1)/(1+2) = 0,666… → tronqué à 666, pas arrondi à 667.
    let un_seul = observee(1, 0);
    assert_eq!(un_seul.expected_per_mille(), 666, "troncature, pas arrondi");
    assert!(un_seul.admits(666));
    assert!(!un_seul.admits(667));
}

/// Un seuil de zéro admet tout le monde, un seuil au-delà de mille n'admet personne.
///
/// Les deux bouts, parce qu'une garde qui ne dirait que « refusé » serait exacte et inutile.
#[test]
fn les_deux_bouts_du_seuil_se_comportent_comme_on_croit() {
    for (reliable, unreliable) in [(0_u32, 0_u32), (10, 0), (0, 10), (5, 5)] {
        let fiabilite = observee(reliable, unreliable);
        assert!(fiabilite.admits(0), "tout le monde passe un seuil nul");
        assert!(!fiabilite.admits(1_001), "personne ne dépasse la certitude");
    }
}

// ---------------------------------------------------------------------------------------------
// 2. Le nom dit le sens
// ---------------------------------------------------------------------------------------------

/// Le module ne parle ni de **réputation**, ni de **faute**, ni de **succès**.
///
/// « Ce qui compte des fautes ne s'appelle pas réputation », dit la clause. Et `success` / `fault` se
/// lisent dans les deux sens selon qu'on parle du pair ou du risque — c'est cette ambiguïté-là qui a
/// laissé deux polarités cohabiter dans le modèle de la source.
#[test]
fn le_vocabulaire_ne_laisse_pas_place_a_deux_lectures() {
    let source =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/reliability.rs"))
            .expect("le module de production est lisible depuis son propre crate");
    // `Default` contient « fault », et la première rédaction de ce test l'a compté comme une faute
    // de vocabulaire. Une recherche de sous-chaîne est un outil grossier : elle se corrige en
    // retirant d'abord ce qu'on sait innocent, pas en relâchant la règle.
    let code: String = source
        .lines()
        .filter(|ligne| !ligne.trim_start().starts_with("//"))
        .map(|ligne| ligne.replace("Default", "").replace("default", ""))
        .collect::<Vec<_>>()
        .join("\n");

    for interdit in [
        "Reputation",
        "reputation",
        "fault",
        "Fault",
        "success",
        "Success",
    ] {
        assert!(
            !code.contains(interdit),
            "« {interdit} » se lit dans les deux sens, et c'est ce qui a rendu le modèle de la \
             source inutilisable"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 3. Le rang, jamais la validité
// ---------------------------------------------------------------------------------------------

/// La fiabilité **classe** les pairs, et c'est tout ce qu'elle fait.
///
/// Elle se branche comme n'importe quelle affinité sur `Audience::best` de `W24.b` : un pair moins
/// fiable est moins souvent choisi. Rien de ce qu'il a dit ne change de statut.
#[test]
fn la_fiabilite_classe_les_pairs_via_l_appariement() {
    let audience = Audience::of(vec![pair(2), pair(3)]);
    let fiable = id::<Agent>(3);

    let choisi = audience
        .best(|peer| {
            let fiabilite = if peer.recipient().agent_id == fiable {
                observee(9, 1)
            } else {
                observee(1, 9)
            };
            i64::from(fiabilite.expected_per_mille())
        })
        .expect("l'audience n'est pas vide");

    assert_eq!(choisi.recipient().agent_id, fiable);
    // Et le pair le moins fiable reste dans l'audience : il est moins choisi, pas exclu.
    assert_eq!(audience.len(), 2);
}

/// **Aucun chemin ne mène d'une observation vers la validité**, tenu par l'absence.
///
/// `Support` et `Inference` vivent dans `packages/graph`, dont `packages/review` ne dépend pas. La
/// vérification porte donc sur les deux niveaux : le manifeste, qui rend le chemin **impossible**, et
/// la source, qui n'en nomme aucun.
///
/// C'est la frontière que l'ADR 0022 décision 2 pose pour `MetaMemory` : « sans une `MetaMemory`
/// séparée, l'utilité passée d'un document finit par entrer dans son score de vérité — le biais de
/// citation reconstruit avec de l'apprentissage automatique ».
#[test]
fn aucun_chemin_ne_mene_d_une_observation_vers_la_validite() {
    let manifeste = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
        .expect("le crate lit son propre manifeste");
    assert!(
        !manifeste.contains("locus-graph"),
        "le crate ne peut pas atteindre `Support` ni `Inference`"
    );

    let source =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/reliability.rs"))
            .expect("le module de production est lisible depuis son propre crate");
    let code: String = source
        .lines()
        .filter(|ligne| !ligne.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for interdit in ["Support", "Inference", "premise", "Validity", "validity"] {
        assert!(
            !code.contains(interdit),
            "« {interdit} » ferait entrer l'utilité passée dans le score de vérité"
        );
    }
}
