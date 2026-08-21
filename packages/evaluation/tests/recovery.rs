//! Test de sortie de `W21.k` — **`failure_recovery_time`**, ADR 0024.
//!
//! 1. Une panne sans reprise est **nommée**, jamais omise ni comptée comme instantanée.
//! 2. Les deux absences de durée ne se ressemblent pas — et le compte de pannes les sépare.
//! 3. La mesure se branche sur le cadre d'endurance **sans le modifier**.
//! 4. Aucune horloge, aucun jugement.

use locus_evaluation::{
    Campaign, Endurance, Outage, Recoveries, Recovery, RecoveryError, Requirement,
};

/// Le code d'un fichier, c'est sa source moins ses commentaires — voir `W21.j`.
fn code_seul(source: &str) -> String {
    source
        .lines()
        .filter(|ligne| !ligne.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------------------------
// 1. Une panne sans reprise est nommée
// ---------------------------------------------------------------------------------------------

/// **Une panne reprise porte sa durée ; une panne en cours n'en porte aucune.**
#[test]
fn une_panne_en_cours_ne_porte_aucune_duree() {
    let reprise = Outage::recovered(1_000, 3_500).expect("ordre licite");
    let en_cours = Outage::ongoing(1_000);

    assert_eq!(reprise.recovery(), Recovery::Recovered { millis: 2_500 });
    assert_eq!(en_cours.recovery(), Recovery::Unrecovered);
    assert_eq!(en_cours.recovery().millis(), None);
}

/// **Une panne non reprise ne fait pas baisser la plus longue durée.**
///
/// Le test qui porte l'item. La compter comme une reprise instantanée retournerait le pire fait
/// d'une campagne — le système est encore à terre — en son contraire, et ferait **baisser** la
/// mesure. L'omettre le ferait disparaître du relevé.
///
/// Ici la panne en cours est ni l'un ni l'autre : elle est comptée dans les pannes, comptée dans les
/// non reprises, et n'entre pas dans la durée.
#[test]
fn une_panne_non_reprise_ne_fait_pas_baisser_la_duree() {
    let releve = Recoveries::over(&[
        Outage::recovered(0, 500).expect("licite"),
        Outage::ongoing(1_000),
    ]);

    assert_eq!(releve.failures(), 2, "les deux pannes sont comptées");
    assert_eq!(releve.unrecovered(), 1, "et l'une est nommée non reprise");
    assert_eq!(
        releve.longest(),
        Some(500),
        "la panne en cours n'a pas été comptée comme instantanée"
    );
}

/// **Une reprise antérieure à sa panne est refusée.**
#[test]
fn une_reprise_avant_la_panne_est_refusee() {
    let refus = Outage::recovered(900, 400).expect_err("ordre impossible");

    assert_eq!(
        refus,
        RecoveryError::RecoveredBeforeFailing {
            failed_at: 900,
            recovered_at: 400
        }
    );
}

/// **Une reprise instantanée est une durée, pas une absence de durée.**
#[test]
fn une_reprise_instantanee_reste_une_reprise() {
    let eclair = Outage::recovered(42, 42).expect("licite");

    assert_eq!(eclair.recovery(), Recovery::Recovered { millis: 0 });
    assert_ne!(eclair.recovery(), Recovery::Unrecovered);
}

// ---------------------------------------------------------------------------------------------
// 2. Deux absences qui ne se ressemblent pas
// ---------------------------------------------------------------------------------------------

/// **Aucune panne et aucune reprise rendent la même absence de durée — et sont opposées.**
///
/// La première est une bonne nouvelle, la seconde la pire possible. C'est le compte de pannes qui
/// les sépare, et c'est pour cela qu'il est rendu **à côté** de la durée plutôt que résumé dedans —
/// même forme que les relais et les tentatives de `W21.i`.
#[test]
fn aucune_panne_et_aucune_reprise_ne_se_confondent_pas() {
    let rien = Recoveries::over(&[]);
    let toujours_a_terre = Recoveries::over(&[Outage::ongoing(10), Outage::ongoing(20)]);

    assert_eq!(rien.longest(), None);
    assert_eq!(toujours_a_terre.longest(), None);
    assert_eq!(rien.failures(), 0, "aucune panne : la bonne nouvelle");
    assert_eq!(
        toujours_a_terre.failures(),
        2,
        "deux pannes, aucune reprise : la pire"
    );
    assert_eq!(toujours_a_terre.unrecovered(), 2);
    assert_ne!(rien, toujours_a_terre);
}

/// **La plus longue est bien la plus longue, quel que soit l'ordre.**
#[test]
fn la_plus_longue_l_emporte_dans_les_deux_ordres() {
    let croissant = Recoveries::over(&[
        Outage::recovered(0, 100).expect("licite"),
        Outage::recovered(0, 900).expect("licite"),
    ]);
    let decroissant = Recoveries::over(&[
        Outage::recovered(0, 900).expect("licite"),
        Outage::recovered(0, 100).expect("licite"),
    ]);

    assert_eq!(croissant.longest(), Some(900));
    assert_eq!(decroissant.longest(), Some(900));
}

// ---------------------------------------------------------------------------------------------
// 3. Le branchement sur le cadre d'endurance
// ---------------------------------------------------------------------------------------------

/// **Le compte de pannes alimente le cadre d'endurance tel quel, sans le modifier.**
///
/// L'item demande que la mesure « se branche sur le cadre de `endurance.rs` sans le modifier ». Ce
/// test le fait **pour de vrai** plutôt que de l'annoncer : il relève des pannes, verse leur compte
/// dans une `Campaign` pour `WorkerLosses`, et vérifie que le cadre l'accepte.
#[test]
fn le_compte_de_pannes_alimente_une_campagne_d_endurance() {
    let releve = Recoveries::over(&[
        Outage::recovered(0, 500).expect("licite"),
        Outage::recovered(600, 900).expect("licite"),
        Outage::ongoing(1_000),
    ]);

    let campagne = Campaign::new()
        .counted(Requirement::WorkerLosses, releve.failures())
        .expect("le cadre accepte le compte tel quel");

    assert_eq!(releve.failures(), 3, "les trois pannes, reprises ou non");
    assert!(
        campagne.measure(Requirement::WorkerLosses).is_some(),
        "la campagne porte désormais le relevé"
    );
    // Une campagne à qui il manque tout le reste n'est évidemment pas tenue : ce test vérifie le
    // branchement, pas la conformité de la campagne.
    assert_ne!(campagne.endurance(), Endurance::Held);
}

// ---------------------------------------------------------------------------------------------
// 4. Aucune horloge, aucun jugement
// ---------------------------------------------------------------------------------------------

/// **Le module ne lit aucune horloge et ne juge pas.**
///
/// Les motifs lisent le **code seul** — la source privée de ses commentaires — pour la raison
/// établie en `W21.j` : une anti-garde qui lit la prose mord sur la phrase qui explique l'absence
/// qu'elle surveille, ce qui est arrivé huit fois dans ce dépôt.
#[test]
fn le_code_ne_lit_aucune_horloge_et_ne_juge_pas() {
    let code = code_seul(include_str!("../src/recovery.rs"));
    assert!(
        code.contains("pub fn"),
        "le nettoyage a trop enlevé : ce test ne lit plus ce qu'il croit lire"
    );

    for interdit in [
        "std::time",
        "SystemTime",
        "Instant",
        "::now()",
        "fn now",
        "const MIN",
        "const MAX",
        "fn is_healthy",
        "fn score",
        "enum Verdict",
        "fn acceptable",
    ] {
        assert!(
            !code.contains(interdit),
            "« {interdit} » ferait de cette durée un jugement, ou la ferait dépendre de l'instant \
             de lecture"
        );
    }
}
