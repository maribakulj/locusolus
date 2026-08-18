//! Test de sortie de W7.g — **deux propositions de valeur égale et de diversité inégale ne sont
//! pas départagées au hasard ; le choix est reproductible.**
//!
//! # Ce que « reproductible » demande de plus que « déterministe »
//!
//! Un scheduler qui garde le premier arrivé en cas d'égalité est déterministe : la même liste donne
//! le même résultat. Il n'est pas reproductible pour autant — c'est l'**ordre d'arrivée** qui
//! décide, en silence, et deux appels sur le même ensemble de candidats peuvent différer.
//!
//! Le test central mélange donc la liste d'entrée et exige le même portefeuille.

use std::collections::BTreeSet;

use locus_portfolio::{
    BranchActivity, Candidate, Indicators, LexicalSimilarity, Policy, Reason, Thresholds,
    Valuation, Weights, schedule, screen, value,
};
use locus_protocol::{Id, IdKind, Timestamp, id::Branch};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

/// Une valorisation d'un montant donné, passée par un criblage propre.
///
/// Le chemin complet est emprunté exprès : `Valuation` ne se fabrique pas, elle se calcule, et
/// `value` exige un `Screening`. Le scheduler hérite donc de la garantie de W7.f sans avoir à la
/// redire — une branche non criblée ne peut pas entrer dans un portefeuille.
fn valued(amount: f64) -> Valuation {
    let indicators = Indicators {
        calibrated_progress: 1.0,
        impact: amount,
        information_gain: 0.0,
        reusability: 0.0,
        diversity: 0.0,
        negative_value: 0.0,
        marginal_cost: 0.0,
        portfolio_similarity: 0.0,
        error_correlation: 0.0,
        dependency_fragility: 0.0,
    };
    let screening = screen(
        &BranchActivity::default(),
        &LexicalSimilarity,
        Thresholds::default(),
    );
    value(&indicators, &Weights::default(), &screening).expect("valorisation valide")
}

fn niches(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

fn candidate(seed: u8, amount: f64, method: &str, model: &str) -> Candidate {
    Candidate {
        branch: id::<Branch>(seed),
        valuation: valued(amount),
        method_family: method.to_owned(),
        model_family: model.to_owned(),
        hypothesis: None,
        falsifies: None,
        informative_negative: false,
        niches: BTreeSet::new(),
    }
}

// ---------------------------------------------------------------------------------------------
// L'égalité se tranche par la diversité, pas au hasard
// ---------------------------------------------------------------------------------------------

/// Le refus qui porte le sprint. Deux branches de **même valeur** : celle qui occupe plus de niches
/// passe devant. Départager au hasard serait plus facile à écrire et impossible à expliquer.
#[test]
fn deux_propositions_de_valeur_egale_sont_departagees_par_la_diversite() {
    let mut narrow = candidate(1, 10.0, "algebrique", "modele-a");
    narrow.niches = niches(&["formalisation"]);
    let mut broad = candidate(2, 10.0, "analytique", "modele-b");
    broad.niches = niches(&["formalisation", "numerique", "combinatoire"]);

    let selection = schedule(&[narrow.clone(), broad.clone()], 1, &Policy::default());

    assert_eq!(selection.slots().len(), 1);
    assert!(
        selection.holds(&broad.branch),
        "à valeur égale, c'est la diversité qui tranche"
    );
    assert!(!selection.holds(&narrow.branch));
}

/// Et la diversité ne renverse pas la valeur : elle ne départage que ce que la valeur laisse à
/// égalité. Sans ce cas, « qualité-diversité » voudrait dire « diversité ».
#[test]
fn la_diversite_ne_renverse_pas_un_ecart_de_valeur() {
    let mut strong = candidate(1, 100.0, "algebrique", "modele-a");
    strong.niches = niches(&["formalisation"]);
    let mut broad = candidate(2, 1.0, "analytique", "modele-b");
    broad.niches = niches(&["a", "b", "c", "d", "e"]);

    let selection = schedule(&[broad, strong.clone()], 1, &Policy::default());
    assert!(selection.holds(&strong.branch));
}

/// « Reproductible » veut dire davantage que « déterministe » : c'est l'ordre d'arrivée qui ne doit
/// **pas** décider. Le troisième barreau de l'ordre — l'identifiant — ne dit rien de scientifique,
/// et c'est exactement son rôle.
#[test]
fn melanger_la_liste_d_entree_ne_change_pas_le_portefeuille() {
    let candidates = vec![
        candidate(1, 10.0, "algebrique", "modele-a"),
        candidate(2, 10.0, "analytique", "modele-b"),
        candidate(3, 10.0, "combinatoire", "modele-c"),
        candidate(4, 10.0, "geometrique", "modele-d"),
    ];

    let reference = schedule(&candidates, 2, &Policy::default());

    // Toutes les rotations : aucune ne change le résultat.
    for rotation in 1..candidates.len() {
        let mut shuffled = candidates.clone();
        shuffled.rotate_left(rotation);
        let other = schedule(&shuffled, 2, &Policy::default());
        assert_eq!(
            reference.slots(),
            other.slots(),
            "rotation de {rotation} : l'ordre d'arrivée a décidé"
        );
    }

    // Et l'ordre inverse non plus.
    let mut reversed = candidates;
    reversed.reverse();
    assert_eq!(
        reference.slots(),
        schedule(&reversed, 2, &Policy::default()).slots()
    );
}

#[test]
fn deux_appels_identiques_donnent_le_meme_portefeuille() {
    let candidates = vec![
        candidate(1, 10.0, "algebrique", "modele-a"),
        candidate(2, 9.0, "analytique", "modele-b"),
        candidate(3, 8.0, "combinatoire", "modele-c"),
    ];
    let first = schedule(&candidates, 2, &Policy::default());
    let second = schedule(&candidates, 2, &Policy::default());
    assert_eq!(first, second);
}

// ---------------------------------------------------------------------------------------------
// « NE DOIT PAS sélectionner uniquement les branches au score le plus élevé »
// ---------------------------------------------------------------------------------------------

/// §13.3 l'écrit en capitales. Ici, les trois meilleures valeurs sont trois variantes de la même
/// méthode ; le portefeuille en prend une part et garde une place pour ailleurs. Un tri par `V(b)`
/// coupé à N passerait §13.4 et violerait §13.3 — et c'est le code qu'on écrit sans y penser.
#[test]
fn le_portefeuille_ne_prend_pas_seulement_les_meilleurs_scores() {
    let candidates = vec![
        candidate(1, 100.0, "algebrique", "modele-a"),
        candidate(2, 99.0, "algebrique", "modele-a"),
        candidate(3, 98.0, "algebrique", "modele-a"),
        candidate(4, 1.0, "analytique", "modele-b"),
    ];

    let selection = schedule(&candidates, 2, &Policy::default());

    assert_eq!(selection.slots().len(), 2);
    assert!(
        selection.holds(&id::<Branch>(4)),
        "la branche la moins bien notée entre, parce qu'elle est ailleurs : {:?}",
        selection.slots()
    );
    assert!(
        !selection.holds(&id::<Branch>(2)),
        "et la deuxième variante de la même méthode n'entre pas"
    );
}

/// La réserve exploratoire ne regarde **que** la distance. Y glisser la valeur en ferait une
/// seconde part d'exploitation un peu plus indulgente, et §13.3 ne serait tenue qu'en apparence.
///
/// Les trois familles sont distinctes exprès : sans cela, c'est la limite de concentration qui
/// écarterait le second, et le test ne dirait rien du critère de la réserve.
#[test]
fn la_reserve_exploratoire_choisit_le_plus_loin_pas_le_deuxieme_meilleur() {
    let mut leader = candidate(1, 100.0, "algebrique", "modele-a");
    leader.niches = niches(&["formalisation"]);
    let mut close = candidate(2, 50.0, "analytique", "modele-b");
    close.niches = niches(&["formalisation"]);
    let mut far = candidate(3, 2.0, "combinatoire", "modele-c");
    far.niches = niches(&["experimental"]);

    let selection = schedule(&[leader.clone(), close, far.clone()], 2, &Policy::default());

    assert!(selection.holds(&leader.branch));
    assert!(
        selection.holds(&far.branch),
        "la réserve prend le plus loin, pas le deuxième meilleur : {:?}",
        selection.slots()
    );
    assert_eq!(selection.slots()[1].reason, Reason::ExploratoryReserve);
}

/// La limite de concentration par famille — §13.3, dernier point. Six branches d'une seule famille
/// ne peuvent pas occuper tout le portefeuille, même si elles sont les six meilleures.
#[test]
fn une_seule_famille_ne_prend_pas_tout_le_portefeuille() {
    let mut candidates: Vec<Candidate> = (1..=6)
        .map(|seed| {
            candidate(
                seed,
                f64::from(100 - i32::from(seed)),
                "algebrique",
                "modele-a",
            )
        })
        .collect();
    candidates.push(candidate(9, 1.0, "analytique", "modele-b"));

    let selection = schedule(&candidates, 4, &Policy::default());
    let from_family = selection
        .slots()
        .iter()
        .filter(|slot| {
            candidates
                .iter()
                .any(|c| c.branch == slot.branch && c.method_family == "algebrique")
        })
        .count();

    assert!(
        from_family <= 2,
        "quatre places, 50 % au plus par famille : {from_family} places prises"
    );
}

// ---------------------------------------------------------------------------------------------
// « Au moins une branche de falsification pour toute hypothèse majeure »
// ---------------------------------------------------------------------------------------------

/// L'exigence la plus facile à perdre : la branche de falsification a par construction une valeur
/// plus basse — elle cherche à démolir, pas à construire — donc un tri par score l'écarte toujours.
#[test]
fn une_hypothese_majeure_retenue_entraine_sa_branche_de_falsification() {
    let mut carrier = candidate(1, 100.0, "algebrique", "modele-a");
    carrier.hypothesis = Some("H1".to_owned());
    let mut falsifier = candidate(2, 1.0, "adversarial", "modele-b");
    falsifier.falsifies = Some("H1".to_owned());
    let filler = candidate(3, 50.0, "analytique", "modele-c");

    let selection = schedule(
        &[carrier.clone(), falsifier.clone(), filler],
        2,
        &Policy::default(),
    );

    assert!(selection.holds(&carrier.branch));
    assert!(
        selection.holds(&falsifier.branch),
        "la contradiction entre, même mal notée : {:?}",
        selection.slots()
    );
    assert!(
        selection
            .slots()
            .iter()
            .any(|slot| slot.reason == Reason::FalsificationDuty)
    );
}

/// Et ce qui manque est **rendu**, pas comblé en silence. Quand aucun candidat ne falsifie une
/// hypothèse retenue, l'exigence ne peut pas être tenue — la taire ferait passer un portefeuille
/// incomplet pour un portefeuille conforme.
#[test]
fn une_hypothese_que_personne_ne_falsifie_est_signalee() {
    let mut carrier = candidate(1, 100.0, "algebrique", "modele-a");
    carrier.hypothesis = Some("H2".to_owned());

    let selection = schedule(&[carrier.clone()], 1, &Policy::default());

    assert!(selection.holds(&carrier.branch));
    assert!(selection.unfalsified_hypotheses().contains("H2"));
}

/// Le devoir de falsification déplace la place la **moins bien classée**, et l'ordre de classement
/// est celui de la valeur — pas celui de la diversité. Déplacer au hasard, ou déplacer le mieux
/// classé, ferait sortir la branche porteuse de l'hypothèse : le portefeuille garderait alors la
/// contradiction sans ce qu'elle contredit.
#[test]
fn le_devoir_de_falsification_deplace_la_place_la_moins_bien_classee() {
    let mut carrier = candidate(1, 100.0, "algebrique", "modele-a");
    carrier.hypothesis = Some("H4".to_owned());
    carrier.niches = niches(&["formalisation"]);

    let mut filler = candidate(2, 50.0, "analytique", "modele-b");
    filler.niches = niches(&["numerique", "combinatoire", "experimental"]);

    let mut falsifier = candidate(3, 1.0, "adversarial", "modele-c");
    falsifier.falsifies = Some("H4".to_owned());
    falsifier.niches = niches(&["adversarial"]);

    let selection = schedule(
        &[carrier.clone(), filler.clone(), falsifier.clone()],
        2,
        &Policy::default(),
    );

    assert!(
        selection.holds(&carrier.branch),
        "c'est le remplissage qui cède la place, pas la branche porteuse : {:?}",
        selection.slots()
    );
    assert!(selection.holds(&falsifier.branch));
    assert!(!selection.holds(&filler.branch));
}

/// Une hypothèse **non retenue** n'entraîne aucun devoir : le portefeuille ne doit pas de
/// contradiction à une piste qu'il n'a pas prise.
#[test]
fn une_hypothese_non_retenue_n_entraine_aucun_devoir() {
    let mut ignored = candidate(2, 0.5, "algebrique", "modele-a");
    ignored.hypothesis = Some("H3".to_owned());
    let leader = candidate(1, 100.0, "analytique", "modele-b");

    let selection = schedule(&[leader.clone(), ignored], 1, &Policy::default());

    assert!(selection.holds(&leader.branch));
    assert!(selection.unfalsified_hypotheses().is_empty());
}

// ---------------------------------------------------------------------------------------------
// Pénalité de corrélation, prime au négatif
// ---------------------------------------------------------------------------------------------

/// La pénalité est **marginale** : elle dépend de ce qui est déjà retenu, pas d'une propriété
/// intrinsèque. La même branche est un bon choix dans un portefeuille et un doublon dans un autre.
///
/// Les familles sont distinctes et les places assez nombreuses pour que la limite de concentration
/// n'ait rien à dire : ce qui écarte le jumeau est la corrélation, et elle seule.
#[test]
fn la_penalite_de_correlation_depend_de_ce_qui_est_deja_retenu() {
    let policy = Policy {
        exploitation_percent: 100,
        correlation_penalty: 0.05,
        ..Policy::default()
    };

    let mut leader = candidate(1, 100.0, "algebrique", "modele-a");
    leader.niches = niches(&["formalisation", "numerique"]);
    let mut twin = candidate(2, 60.0, "analytique", "modele-b");
    twin.niches = niches(&["formalisation", "numerique"]);
    let mut stranger = candidate(3, 59.0, "combinatoire", "modele-c");
    stranger.niches = niches(&["experimental"]);

    // Seul, le jumeau passe devant l'étranger : sa valeur est plus haute.
    let alone = schedule(&[twin.clone(), stranger.clone()], 1, &policy);
    assert!(alone.holds(&twin.branch));

    // Avec le leader, c'est l'étranger qui entre.
    let together = schedule(
        &[leader.clone(), twin.clone(), stranger.clone()],
        2,
        &policy,
    );
    assert!(together.holds(&leader.branch));
    assert!(
        together.holds(&stranger.branch),
        "le jumeau du leader n'apporte rien de plus : {:?}",
        together.slots()
    );
}

/// La corrélation compte **deux** choses, et il faut les deux : la famille partagée et les niches
/// partagées. Un seul des deux termes laisserait passer la moitié des doublons — deux branches de
/// méthodes différentes sur exactement la même niche, ou deux variantes d'une même méthode sur des
/// niches nommées autrement.
#[test]
fn la_correlation_compte_la_famille_de_methode_et_les_niches() {
    let policy = Policy {
        correlation_penalty: 0.05,
        ..Policy::default()
    };

    // Ne partagent que la famille de méthode.
    let mut leader = candidate(1, 100.0, "algebrique", "modele-a");
    leader.niches = niches(&["formalisation"]);
    let mut same_method = candidate(2, 60.0, "algebrique", "modele-b");
    same_method.niches = niches(&["numerique"]);
    let mut neutral = candidate(3, 59.0, "combinatoire", "modele-c");
    neutral.niches = niches(&["experimental"]);

    let by_method = schedule(
        &[leader.clone(), same_method.clone(), neutral.clone()],
        4,
        &policy,
    );
    assert_eq!(
        by_method.slots()[1].branch,
        neutral.branch,
        "la famille de méthode partagée coûte plus que l'écart de valeur : {:?}",
        by_method.slots()
    );

    // Ne partagent que les niches.
    let mut same_niche = candidate(4, 60.0, "analytique", "modele-d");
    same_niche.niches = niches(&["formalisation"]);

    let by_niche = schedule(&[leader, same_niche, neutral.clone()], 4, &policy);
    assert_eq!(
        by_niche.slots()[1].branch,
        neutral.branch,
        "la niche partagée coûte, elle aussi : {:?}",
        by_niche.slots()
    );
}

/// « Une prime aux résultats négatifs informatifs » — §13.3. Un résultat négatif a une valeur plus
/// basse par nature ; sans prime, il ne serait jamais choisi, et l'invariant 12 n'aurait personne
/// pour le servir.
#[test]
fn un_resultat_negatif_informatif_est_prime() {
    let mut negative = candidate(2, 9.8, "analytique", "modele-b");
    negative.informative_negative = true;
    let ordinary = candidate(3, 10.0, "combinatoire", "modele-c");

    let selection = schedule(&[negative.clone(), ordinary], 1, &Policy::default());
    assert!(
        selection.holds(&negative.branch),
        "la prime doit suffire à renverser un écart faible"
    );
}

// ---------------------------------------------------------------------------------------------
// Ce que la sélection dit d'elle-même
// ---------------------------------------------------------------------------------------------

/// Comme la valorisation de §13.4, la sélection transporte sa politique : un portefeuille dont on ne
/// peut plus retrouver la politique est une liste, pas une décision.
#[test]
fn une_selection_transporte_sa_politique() {
    let policy = Policy {
        exploitation_percent: 25,
        ..Policy::default()
    };
    let selection = schedule(&[candidate(1, 10.0, "a", "m")], 1, &policy);
    assert_eq!(selection.policy(), &policy);
}

/// Chaque place dit pourquoi elle a été attribuée. Sans cela, on ne pourrait pas distinguer une
/// réserve exploratoire d'un mauvais choix d'exploitation — et §13.3 serait invérifiable de
/// l'extérieur.
#[test]
fn chaque_place_dit_pourquoi_elle_a_ete_attribuee() {
    let candidates = vec![
        candidate(1, 100.0, "algebrique", "modele-a"),
        candidate(2, 1.0, "combinatoire", "modele-c"),
    ];
    let selection = schedule(&candidates, 2, &Policy::default());

    assert_eq!(selection.slots()[0].reason, Reason::Exploitation);
    assert_eq!(selection.slots()[1].reason, Reason::ExploratoryReserve);
    assert_eq!(
        selection
            .slots()
            .iter()
            .map(|slot| slot.reason.slug())
            .collect::<Vec<_>>(),
        vec!["exploitation", "exploratory_reserve"]
    );
}

/// Moins de candidats que de places : le portefeuille ne s'invente pas de branches, et ne boucle
/// pas en cherchant à remplir ce qui ne peut pas l'être.
#[test]
fn moins_de_candidats_que_de_places_ne_boucle_pas() {
    let selection = schedule(&[candidate(1, 10.0, "a", "m")], 5, &Policy::default());
    assert_eq!(selection.slots().len(), 1);
}

#[test]
fn un_portefeuille_sans_candidat_est_vide() {
    let selection = schedule(&[], 3, &Policy::default());
    assert!(selection.slots().is_empty());
    assert!(selection.unfalsified_hypotheses().is_empty());
}
