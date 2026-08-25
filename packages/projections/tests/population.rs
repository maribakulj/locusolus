//! Test de sortie de `W23.b` — **les trois compteurs de population.**
//!
//! Quatre clauses, celles de la roadmap, et aucune ne se déduit des autres :
//!
//! 1. les trois se recalculent **depuis le journal** et se rejouent à l'identique ;
//! 2. un rapport qui n'en porte qu'un **n'est pas constructible**, tenu par le type ;
//! 3. `generating ≤ active ≤ nominal` est un invariant testé, et une violation **nomme les trois** ;
//! 4. aucun de ces compteurs ne porte de seuil.
//!
//! La première est celle qui dit que le journal suffit — et c'est elle qui n'était pas
//! satisfaisable il y a quatre items, la population n'atteignant pas le journal du tout.

use locus_coordination::agent::InstanceState;
use locus_event_store::{Actor, ActorKind, Envelope, EventType};
use locus_projections::population::{Census, Population};
use locus_projections::projection::Projection;
use locus_protocol::id::{Agent, Command, Event, Project, Workspace};
use locus_protocol::{Id, IdKind, Timestamp};

const NOW: Timestamp = Timestamp::from_millis(1_700_000_000_000);

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(NOW, entropy).expect("l'instant de fixture tient sur 48 bits")
}

fn agent(seed: u8) -> String {
    id::<Agent>(seed).to_string()
}

/// Une enveloppe de §10.1, telle que `locusd` en écrit.
fn fait(seed: u8, event_type: &str, kind: ActorKind, payload: serde_json::Value) -> Envelope {
    Envelope {
        event_id: id::<Event>(seed),
        event_type: EventType::parse(event_type).expect("type de §10.3"),
        schema_version: 1,
        stream_id: format!("stream/{seed}"),
        stream_revision: 0,
        workspace_id: id::<Workspace>(2),
        project_id: id::<Project>(4),
        program_id: None,
        branch_id: None,
        actor: Actor {
            principal_id: id::<Agent>(3),
            kind,
            delegation_id: None,
        },
        occurred_at: NOW,
        recorded_at: NOW,
        causation_id: id::<Command>(seed),
        idempotency_key: None,
        correlation_id: None,
        trace_id: None,
        payload,
        payload_hash: format!("sha256:{}", "ab".repeat(32)),
    }
}

fn cycle_de_vie(seed: u8, verbe: &str, agent_seed: u8, state: InstanceState) -> Envelope {
    fait(
        seed,
        &format!("agent.{verbe}"),
        ActorKind::System,
        serde_json::json!({ "agent_id": agent(agent_seed), "state": state.to_string() }),
    )
}

fn assignation(seed: u8, task_id: &str, agent_seed: u8) -> Envelope {
    fait(
        seed,
        "task.assigned",
        ActorKind::System,
        serde_json::json!({
            "task_id": task_id,
            "agent_id": agent(agent_seed),
            "worker_id": "worker-a",
        }),
    )
}

fn bail(seed: u8, task_id: &str) -> Envelope {
    fait(
        seed,
        "task.leased",
        // Un bail est un fait de **worker** : `lep::fact` pose `Agent` et le documente.
        ActorKind::Agent,
        serde_json::json!({ "task_id": task_id, "worker_id": "worker-a" }),
    )
}

fn acheve(seed: u8, task_id: &str) -> Envelope {
    fait(
        seed,
        "run.completed",
        ActorKind::Agent,
        serde_json::json!({ "task_id": task_id, "worker_id": "worker-a" }),
    )
}

/// Appliquer un flux à une population neuve.
fn recenser(flux: &[Envelope]) -> Population {
    let mut population = Population::new();
    for (rang, event) in flux.iter().enumerate() {
        let position = u64::try_from(rang).expect("un flux de test") + 1;
        population
            .apply(position, event)
            .unwrap_or_else(|refusal| panic!("{refusal}"));
    }
    population
}

/// Le flux de référence : trois identités, dont une tuée, et deux tâches confiées.
fn flux() -> Vec<Envelope> {
    vec![
        cycle_de_vie(1, "spawned", 10, InstanceState::Provisioned),
        cycle_de_vie(2, "spawned", 11, InstanceState::Provisioned),
        cycle_de_vie(3, "spawned", 12, InstanceState::Provisioned),
        // La troisième est arrêtée : connue, plus active.
        cycle_de_vie(4, "killed", 12, InstanceState::Terminated),
        assignation(5, "tache-a", 10),
        assignation(6, "tache-b", 11),
        // Seule la première a un bail ouvert.
        bail(7, "tache-a"),
    ]
}

// ---------------------------------------------------------------------------------------------
// 1. Les trois se recalculent depuis le journal, et se rejouent à l'identique
// ---------------------------------------------------------------------------------------------

/// Les trois nombres, et chacun compte autre chose que les deux autres.
#[test]
fn les_trois_compteurs_se_lisent_du_journal() {
    let census = recenser(&flux()).census();
    assert_eq!(census.nominal(), 3, "trois identités ont été fondées");
    assert_eq!(census.active(), 2, "la troisième est terminée");
    assert_eq!(census.generating(), 1, "une seule a un bail ouvert");
}

/// **Rejouer rend le même état**, résumé compris — §9.5.
///
/// C'est la propriété qui fait de ces compteurs une projection et non une seconde source de vérité.
/// Un `reset` puis une réapplication du même flux doit être indiscernable.
#[test]
fn une_reconstruction_rend_le_meme_etat() {
    let flux = flux();
    let population = recenser(&flux);
    let resume = population.checksum();
    let census = population.census();

    let mut reconstruite = population;
    reconstruite.reset();
    assert_eq!(reconstruite.watermark(), 0, "un reset n'a plus d'histoire");
    assert_eq!(reconstruite.census().nominal(), 0);
    for (rang, event) in flux.iter().enumerate() {
        let position = u64::try_from(rang).expect("un flux de test") + 1;
        reconstruite.apply(position, event).expect("rejeu");
    }

    assert_eq!(reconstruite.checksum(), resume);
    assert_eq!(reconstruite.census(), census);
}

/// Le bail se **referme**, et `generating` redescend.
///
/// `task.leased` l'ouvre, `run.completed` le referme — c'est le couple de `W20.k`. Un compteur qui
/// ne redescendrait pas mesurerait « a déjà raisonné », pas « raisonne ».
#[test]
fn un_bail_referme_fait_redescendre_generating() {
    let mut flux = flux();
    assert_eq!(recenser(&flux).census().generating(), 1);
    flux.push(acheve(8, "tache-a"));
    assert_eq!(recenser(&flux).census().generating(), 0);
}

/// Une identité **terminée** ne raisonne plus, même si son bail est resté ouvert.
///
/// Le cas se produit : tuer une instance abandonne ce qu'elle avait en vol, et `Outcome::Killed`
/// porte le compte de ce qui est perdu. Compter cette identité comme `generating` dirait qu'un mort
/// travaille — et ferait passer `generating` au-dessus d'`active`, c'est-à-dire violer l'invariant
/// par la mesure au lieu de le détecter.
#[test]
fn une_identite_terminee_ne_genere_plus_meme_avec_un_bail_ouvert() {
    let flux = vec![
        cycle_de_vie(1, "spawned", 10, InstanceState::Provisioned),
        assignation(2, "tache-a", 10),
        bail(3, "tache-a"),
        cycle_de_vie(4, "killed", 10, InstanceState::Terminated),
    ];
    let census = recenser(&flux).census();
    assert_eq!(census.nominal(), 1, "l'identité reste connue");
    assert_eq!(census.active(), 0);
    assert_eq!(census.generating(), 0);
}

/// Une tâche **reprise** fait raisonner son nouveau titulaire, pas l'ancien.
///
/// # Pourquoi le compte seul ne suffit pas à le dire, et ce qui le rend observable
///
/// Une première rédaction s'arrêtait à « `generating` vaut 1, pas 2 ». Un mutant faisant gagner la
/// **première** assignation y a survécu : le bail reste tenu par quelqu'un dans les deux cas, donc
/// le compte est le même et rien ne départage l'ancien du nouveau. C'est la même forme que le
/// mutant du stream unique de `W20.ae` — un test qui n'observe pas ce qui distingue les deux
/// hypothèses.
///
/// Arrêter l'ancien titulaire rend la différence visible sans ajouter d'accesseur : si la reprise a
/// bien eu lieu, le nouveau — actif — raisonne toujours ; sinon le bail est tenu par un mort, et
/// `generating` tombe à zéro.
#[test]
fn une_tache_reprise_change_de_generateur() {
    let flux = vec![
        cycle_de_vie(1, "spawned", 10, InstanceState::Provisioned),
        cycle_de_vie(2, "spawned", 11, InstanceState::Provisioned),
        assignation(3, "tache-a", 10),
        bail(4, "tache-a"),
        assignation(5, "tache-a", 11),
    ];
    let population = recenser(&flux);
    assert_eq!(population.census().generating(), 1, "un seul, pas deux");
    assert_eq!(
        population.state_of(&agent(11)),
        Some(InstanceState::Provisioned)
    );

    let mut reprise = flux;
    reprise.push(cycle_de_vie(6, "killed", 10, InstanceState::Terminated));
    let census = recenser(&reprise).census();
    assert_eq!(census.active(), 1, "l'ancien titulaire est arrêté");
    assert_eq!(
        census.generating(),
        1,
        "c'est le nouveau titulaire qui raisonne : un zéro ici voudrait dire que le bail est \
         resté attaché à l'ancien"
    );
}

// ---------------------------------------------------------------------------------------------
// 2. Les deux gardes de provenance
// ---------------------------------------------------------------------------------------------

/// Un fait `agent.*` qui n'est **pas** du système n'entre pas dans la population.
///
/// Invariant 3 : une population que les workers écriraient serait une population qu'ils décident.
/// L'événement est journalisé, et c'est bien ainsi ; il n'est simplement pas une source.
#[test]
fn un_fait_de_cycle_de_vie_qui_n_est_pas_du_systeme_ne_compte_pas() {
    let mut usurpe = cycle_de_vie(1, "spawned", 10, InstanceState::Provisioned);
    usurpe.actor.kind = ActorKind::Agent;
    assert_eq!(recenser(&[usurpe]).census().nominal(), 0);
}

/// Un bail **est** un fait de worker, et l'exiger du système rendrait `generating` nul.
///
/// L'asymétrie avec la garde précédente est voulue : l'appartenance est une décision du plan de
/// contrôle, la réclamation est l'acte d'un worker. `lep::fact` pose `Agent` et le documente ;
/// filtrer sur le système ici ne retiendrait **rien**, et le compteur rendrait zéro sur un système
/// qui tourne — le zéro d'un compteur qui n'a rien lu.
#[test]
fn un_bail_de_worker_compte_bel_et_bien() {
    let flux = vec![
        cycle_de_vie(1, "spawned", 10, InstanceState::Provisioned),
        assignation(2, "tache-a", 10),
        bail(3, "tache-a"),
    ];
    let census = recenser(&flux).census();
    assert_eq!(census.active(), 1);
    assert_eq!(
        census.generating(),
        1,
        "le bail vient d'un worker, et il compte quand même"
    );
}

/// Un état d'instance **inconnu** met la projection en quarantaine.
///
/// Le ranger d'un côté gonflerait ou raboterait `active` sans que rien ne le dise. §9.5 : la
/// quarantaine n'empêche pas l'écriture canonique, elle empêche de croire la projection.
#[test]
fn un_etat_inconnu_met_en_quarantaine_plutot_que_de_choisir() {
    let inconnu = fait(
        1,
        "agent.spawned",
        ActorKind::System,
        serde_json::json!({ "agent_id": agent(10), "state": "hibernating" }),
    );
    let mut population = Population::new();
    let refus = population
        .apply(1, &inconnu)
        .expect_err("un état inconnu ne se range pas");
    assert_eq!(refus.position, 1);
    assert!(refus.reason.contains("hibernating"), "{refus}");
}

// ---------------------------------------------------------------------------------------------
// 3. Le type refuse ce qui ne peut pas être vrai
// ---------------------------------------------------------------------------------------------

/// Un recensement incohérent est **refusé**, et le refus nomme les trois.
///
/// Nommer la seule comparaison qui a échoué obligerait le lecteur à aller chercher les autres pour
/// comprendre, et un recensement incohérent n'a pas de moitié saine.
#[test]
fn un_recensement_incoherent_est_refuse_en_nommant_les_trois() {
    for (nominal, active, generating) in [(3_usize, 4_usize, 0_usize), (3, 2, 3), (0, 0, 1)] {
        let refus = Census::new(nominal, active, generating)
            .expect_err("l'invariant generating ≤ active ≤ nominal est violé");
        assert_eq!(
            (refus.nominal, refus.active, refus.generating),
            (nominal, active, generating)
        );
        let dit = refus.to_string();
        for valeur in [nominal, active, generating] {
            assert!(dit.contains(&valeur.to_string()), "{dit}");
        }
    }
}

/// Les cas limites de l'invariant sont **acceptés** : l'égalité n'est pas une violation.
#[test]
fn l_egalite_n_est_pas_une_violation() {
    for (nominal, active, generating) in [(0_usize, 0_usize, 0_usize), (2, 2, 2), (5, 2, 2)] {
        let census = Census::new(nominal, active, generating).expect("l'invariant tient");
        assert_eq!(census.nominal(), nominal);
        assert_eq!(census.active(), active);
        assert_eq!(census.generating(), generating);
    }
}

// ---------------------------------------------------------------------------------------------
// 4. Aucun seuil
// ---------------------------------------------------------------------------------------------

/// Le module ne porte **aucune constante numérique**.
///
/// L'ADR 0026 décision 3 refuse toute taille décrétée avant que `W23.d` ait mesuré, et un compteur
/// qui saurait dire « c'est trop » aurait décidé à sa place. C'est le même idiome que `W23.d`, dont
/// le test de sortie demande que la taille de cellule soit tenue par **l'absence de constante** — et
/// que `W23.a`, dont le test lit `Cargo.toml` : la propriété n'est pas « personne n'en a écrit », qui
/// se relit à chaque revue, mais « il n'y en a pas », qui se vérifie.
#[test]
fn aucun_seuil_ne_vit_dans_le_module() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/population.rs"))
        .expect("le module de production est lisible depuis son propre crate");
    for (numero, ligne) in source.lines().enumerate() {
        let code = ligne.split("//").next().unwrap_or_default();
        assert!(
            !code.contains("const ") || code.contains("const fn"),
            "ligne {} : une constante dans ce module serait un seuil, et l'ADR 0026 décision 3 \
             refuse toute taille décrétée avant que `W23.d` ait mesuré — {}",
            numero + 1,
            ligne.trim()
        );
    }
}
