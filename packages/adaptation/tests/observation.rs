//! Test de sortie de `W18.g` — **le capteur qui manquait entre la mémoire et l'organisation.**
//!
//! Six propriétés, celles du tableau de `docs/10` :
//!
//! 1. une observation se recalcule à l'identique, et deux calculs sur le même préfixe rendent la
//!    même valeur ;
//! 2. elle **cite** les révisions dont elle est tirée, et une observation sans citation n'est pas
//!    constructible ;
//! 3. **aucun chemin de type ne mène d'une `Observation` à un `Trigger`** — test d'absence ;
//! 4. un seuil n'a **nulle part où s'écrire** dans un capteur ;
//! 5. le type s'appelle `Observation` et non `Signal`, que `memory::retrieval` occupe ;
//! 6. les six sources sont exercées, et une source muette produit une observation **absente**.

use locus_adaptation::{Observation, ObservationError, ObservationKind, Sensor, observe_all};
use locus_domain::RevisionId;
use locus_protocol::{Id, Timestamp};

const NOW: Timestamp = Timestamp::from_millis(1_700_000_000_000);

fn revision(seed: u8) -> RevisionId {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(NOW, entropy).expect("l'instant de fixture tient sur 48 bits")
}

/// Un capteur déterministe : il compte les révisions qu'on lui a données.
struct Compteur {
    kind: ObservationKind,
    sources: Vec<RevisionId>,
}

impl Sensor for Compteur {
    fn kind(&self) -> ObservationKind {
        self.kind
    }

    fn observe(&self, watermark: u64) -> Option<Observation> {
        // Un capteur ne lit **que** ce que son préfixe contient : la valeur dépend du watermark, ce
        // qui est la définition d'une mesure prise sur un journal.
        let vues: Vec<RevisionId> = self
            .sources
            .iter()
            .take(usize::try_from(watermark).unwrap_or(usize::MAX))
            .copied()
            .collect();
        if vues.is_empty() {
            return None;
        }
        let valeur = f64::from(u32::try_from(vues.len()).unwrap_or(u32::MAX));
        Observation::measured(self.kind, valeur, vues, watermark).ok()
    }
}

/// Un capteur muet — la source n'a rien à dire.
struct Muet(ObservationKind);

impl Sensor for Muet {
    fn kind(&self) -> ObservationKind {
        self.0
    }

    fn observe(&self, _watermark: u64) -> Option<Observation> {
        None
    }
}

// ---------------------------------------------------------------------------------------------
// 1 et 2 — la mesure, et ce dont elle est tirée
// ---------------------------------------------------------------------------------------------

/// **Deux calculs sur le même préfixe rendent la même valeur.**
///
/// C'est ce qui distingue une observation d'une opinion : elle se recalcule. Et un préfixe plus long
/// rend une autre valeur, ce qui prouve que le watermark n'est pas décoratif — sans ce second
/// assert, un capteur qui ignorerait le préfixe passerait le premier.
#[test]
fn une_observation_se_recalcule_a_l_identique() {
    let capteur = Compteur {
        kind: ObservationKind::OpenConflicts,
        sources: vec![revision(1), revision(2), revision(3)],
    };

    let une = capteur.observe(2).expect("deux révisions vues");
    let autre = capteur.observe(2).expect("deux révisions vues");
    assert_eq!(une, autre);
    assert!((une.value() - 2.0).abs() < f64::EPSILON);
    assert_eq!(une.watermark(), 2);

    // Un préfixe plus long, une autre mesure : deux mondes ne se comparent pas.
    let plus_loin = capteur.observe(3).expect("trois révisions vues");
    assert!((plus_loin.value() - 3.0).abs() < f64::EPSILON);
    assert_ne!(une, plus_loin);
}

/// **Une observation sans citation n'est pas constructible.**
///
/// Sans les révisions dont elle est tirée, elle ne se recalcule pas, donc ne se conteste pas, donc
/// n'est qu'une affirmation de plus — exactement ce que cet item existe pour remplacer.
#[test]
fn une_observation_sans_citation_n_est_pas_constructible() {
    let refus = Observation::measured(ObservationKind::DomainGap, 0.4, Vec::new(), 10)
        .expect_err("aucune révision citée");
    assert_eq!(
        refus,
        ObservationError::Uncited {
            kind: ObservationKind::DomainGap
        }
    );
    assert!(refus.to_string().contains("affirmation"), "{refus}");

    // Et une valeur qui n'est pas un nombre est refusée aussi : une politique comparerait une chose
    // à une non-chose.
    assert!(
        Observation::measured(ObservationKind::DomainGap, f64::NAN, vec![revision(1)], 10).is_err()
    );
}

// ---------------------------------------------------------------------------------------------
// 3 et 4 — la frontière que rien ne franchit
// ---------------------------------------------------------------------------------------------

/// **Aucun chemin de type ne mène d'une `Observation` à un `Trigger`.**
///
/// C'est le test central de l'item. La correspondance « telle mesure déclenche tel trigger » est une
/// **décision**, elle vit dans une politique versionnée avec ses seuils. Un `From` ou une méthode
/// l'aurait figée dans le code, et la changer aurait demandé de recompiler au lieu de commiter une
/// politique.
#[test]
fn aucun_chemin_de_type_ne_mene_d_une_observation_a_un_trigger() {
    // Les motifs visent des **imports et des signatures**, pas le mot « Trigger » : la
    // documentation du module l'emploie pour dire exactement ce que la garde veut obtenir. Une
    // garde qui se déclenche sur sa propre justification est une garde qu'on finit par assouplir —
    // c'est la troisième fois de cette série qu'un motif trop large le rappelle.
    let source = include_str!("../src/observation.rs");
    for interdit in [
        "use crate::Trigger",
        "use crate::spawn::",
        "-> Trigger",
        ": Trigger",
        "Vec<Trigger>",
        "impl From<Observation>",
        "fn trigger",
        "fn to_trigger",
        "fn into_trigger",
        "fn should_spawn",
    ] {
        assert!(
            !source.contains(interdit),
            "« {interdit} » dans observation.rs : la correspondance est une décision, et elle vit \
             dans une politique"
        );
    }
}

/// **Un seuil n'a nulle part où s'écrire.**
///
/// Tenu par l'absence de champ : « à partir de combien de conflits ouverts faut-il agir » n'a pas de
/// réponse dans les données. Un capteur qui porterait ce seuil trancherait la question en silence.
#[test]
fn un_seuil_n_a_nulle_part_ou_s_ecrire_dans_un_capteur() {
    let source = include_str!("../src/observation.rs");
    for interdit in [
        "threshold",
        "seuil:",
        "min_value",
        "max_value",
        "fn exceeds",
        "fn is_above",
    ] {
        assert!(
            !source.contains(interdit),
            "« {interdit} » dans observation.rs : un seuil est une décision, pas une mesure"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 5 et 6 — le nom, et les six sources
// ---------------------------------------------------------------------------------------------

/// **`Observation` et `Signal` cohabitent sans renommage.**
///
/// `memory::retrieval::Signal` désigne un facteur de classement. Si le capteur s'était appelé
/// `Signal`, cette ligne ne compilerait pas — même collision que `Genre` contre `Kind`, résolue de
/// la même façon.
#[test]
fn observation_et_signal_designent_deux_choses_distinctes() {
    use locus_adaptation::Observation as O;
    use locus_memory::Signal;

    assert!(O::measured(ObservationKind::DomainGap, 1.0, vec![revision(1)], 5).is_ok());
    assert_eq!(Signal::ALL.len(), 10);
}

/// **Les six sources se lisent sous leur nom**, et une septième n'existe pas.
#[test]
fn les_six_sources_se_lisent_sous_leur_nom() {
    let noms = [
        "open-conflicts",
        "validation-depth",
        "portfolio-indicator",
        "review-disagreement",
        "reproduction-failure",
        "domain-gap",
    ];
    assert_eq!(ObservationKind::ALL.len(), 6);
    for (kind, nom) in ObservationKind::ALL.into_iter().zip(noms) {
        assert_eq!(kind.slug(), nom);
        assert_eq!(ObservationKind::parse(nom), Some(kind));
    }
    assert_eq!(ObservationKind::parse("consensus-circulaire"), None);
}

/// **Chacune des six est exercée, et une source muette produit une observation absente.**
///
/// `None` et non `Some(0.0)` : « aucun conflit ouvert » et « la source n'a pas répondu » sont deux
/// états qu'une politique traite différemment, et les fondre ferait lire un silence comme une bonne
/// nouvelle.
#[test]
fn une_source_muette_produit_une_observation_absente() {
    // Les six, chacune sur sa fixture.
    let capteurs: Vec<Compteur> = ObservationKind::ALL
        .into_iter()
        .map(|kind| Compteur {
            kind,
            sources: vec![revision(1), revision(2)],
        })
        .collect();
    let refs: Vec<&dyn Sensor> = capteurs.iter().map(|c| c as &dyn Sensor).collect();

    let prises = observe_all(&refs, 2);
    assert_eq!(prises.len(), 6, "les six sources répondent");
    for (prise, kind) in prises.iter().zip(ObservationKind::ALL) {
        assert_eq!(prise.kind(), kind);
        assert_eq!(prise.cites().len(), 2, "chaque mesure cite ce qu'elle a lu");
    }

    // Une muette parmi elles : le compte rendu est **plus court**, pas garni d'un zéro.
    let muet = Muet(ObservationKind::DomainGap);
    let mut melange: Vec<&dyn Sensor> = capteurs.iter().take(5).map(|c| c as &dyn Sensor).collect();
    melange.push(&muet);

    let partielles = observe_all(&melange, 2);
    assert_eq!(
        partielles.len(),
        5,
        "cinq sur six : une politique qui en reçoit cinq sait qu'il lui en manque une"
    );
    assert!(
        !partielles
            .iter()
            .any(|prise| prise.kind() == ObservationKind::DomainGap),
        "la muette est absente, pas présente à zéro"
    );
}
