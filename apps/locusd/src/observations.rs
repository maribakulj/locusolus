//! Ce que le daemon **dit** d'une décision qu'aucune réponse ne porte — `W20.aa`, §10.2.
//!
//! # Le défaut, tel que la chaîne réelle l'a rendu
//!
//! Un worker `canterel` enrôlé réclame ; `locusd` rend `204`. Le `204` couvre **deux états** que le
//! daemon distingue parfaitement et que personne d'autre ne peut distinguer :
//!
//! - la file n'avait rien pour ce worker — `queue.take()` a rendu `None`, et aucun broker n'a été
//!   consulté ;
//! - une mission **était** là, le broker a répondu `NotPlaced`, et la mission est retournée en file.
//!
//! Dans le second cas, `Placement::NotPlaced` porte un [`Shortfall`] par worker examiné, chacun avec
//! ses [`Reason`] — les sept motifs de §10.2, que `packages/lep` distingue avec soin parce que « cet
//! hôte ne sait pas faire » et « cet hôte ne l'a jamais prouvé » envoient à des endroits opposés.
//!
//! `Runtime::placed` écrivait `Ok(Placement::NotPlaced { .. }) => Ok(false)`. **Tout était jeté.**
//! Une sonde de session s'est arrêtée là : une chaîne réelle rendait `204` sur une mission alignée
//! sur ce que le worker annonçait, et rien dans le système ne permettait de dire pourquoi.
//!
//! # Pourquoi le `204` reste vide, lui
//!
//! ADR 0028 décision 4 : « rien pour toi » n'est pas une erreur. Y mettre un corps ferait d'une
//! réponse normale un diagnostic, et donnerait à un worker le détail des manques d'un hôte — ce que
//! sa créance ne lui donne aucun droit de connaître. Ce qui manquait n'est pas une réponse plus
//! bavarde : c'est un **exploitant** qui puisse lire ce que son daemon a décidé.
//!
//! # Pourquoi un port et non un `eprintln!`
//!
//! Rien sous `apps/locusd/src/` n'imprime — **vérifié, pas supposé** : `println!` n'apparaît que
//! dans `main.rs`, quatorze fois. La bibliothèque décide, le binaire rend compte. Un `eprintln!`
//! glissé dans `lep_claim` serait la première exception à cette frontière, et une frontière qui a
//! une exception en a bientôt trois.
//!
//! # Le défaut de ce port **parle**, et l'asymétrie est délibérée
//!
//! Les ports d'autorité de ce dépôt refusent par défaut — `NoIdentities`, `NoAdministrators`,
//! `NoBlobs` —, parce que le danger y est la permissivité silencieuse. Ici le danger est
//! l'**inverse** : le défaut à corriger *est* le silence. Un puits de diagnostic dont le défaut
//! serait muet reproduirait exactement ce que cet item retire, sous un nom plus rassurant.
//!
//! Le défaut écrit donc sur la sortie d'erreur du daemon, et un test double capture les lignes.

use std::sync::{Mutex, PoisonError};

use locus_broker::port::Placement;
use locus_broker::protocol::Shortfall;
use locus_lep::Reason;

/// Ce que le daemon a décidé sans que sa réponse puisse le porter.
///
/// Une seule méthode, et volontairement pauvre : ce port n'est pas un système de journalisation. Il
/// existe pour qu'une décision prise en silence cesse de l'être, et l'élargir en avance ferait
/// entrer une infrastructure là où une phrase suffit.
pub trait Observations: Send + Sync {
    /// Une mission a été retirée de la file puis rendue, faute de placement.
    ///
    /// Appelé **seulement** dans ce cas. Une file vide ne dit rien, et ce silence-là est un
    /// renseignement : un worker qui sonde toutes les secondes remplirait n'importe quel journal, et
    /// l'absence de ligne veut alors dire « la file n'avait rien » — ce qui lève l'ambiguïté du
    /// `204` sans écrire une ligne par sondage.
    fn unplaced(&self, note: &str);
}

/// Le défaut : la sortie d'erreur du daemon.
#[derive(Debug, Default, Clone, Copy)]
pub struct StderrObservations;

impl Observations for StderrObservations {
    fn unplaced(&self, note: &str) {
        eprintln!("locusd : {note}");
    }
}

/// Un puits qui garde ce qu'on lui donne — pour les tests, et pour eux seuls.
#[derive(Debug, Default)]
pub struct MemoryObservations {
    notes: Mutex<Vec<String>>,
}

impl MemoryObservations {
    /// Un puits vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Ce qui a été dit, dans l'ordre.
    #[must_use]
    pub fn notes(&self) -> Vec<String> {
        self.notes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Observations for MemoryObservations {
    fn unplaced(&self, note: &str) {
        self.notes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(note.to_owned());
    }
}

/// La note à dire d'un verdict de placement, ou `None` s'il a placé — `W20.aa`.
///
/// # Pourquoi la lecture du verdict vit **ici** et non dans `lep.rs`
///
/// Un test d'absence de `W20.o` interdit à `apps/locusd/src/lep.rs` de contenir les mots de la
/// **décision** — `Candidate`, `shortfall`, `admit(`, `proven_level` —, parce que la surface §15.2
/// demande un placement et n'en décide jamais. La première rédaction de cet item faisait remonter
/// les `Shortfall` jusque dans `lep.rs` pour les rendre en phrase, et **le test a rougi**.
///
/// Il avait raison, et c'est le code qui a changé. Manipuler le vocabulaire des manques, même pour
/// n'en faire qu'une phrase, c'est en connaître la forme — et une surface qui connaît la forme d'une
/// décision finit par en prendre une. `lep.rs` lit donc « placé / pas placé, et voici la phrase à
/// dire » ; tout le reste est ici.
///
/// La garde tenait par le **nom**, ce qui semble grossier et se révèle exact : le mot n'apparaît que
/// là où quelqu'un regarde ce qu'il y a dedans.
#[must_use]
pub fn refusal_note(task_id: &str, placement: &Placement) -> Option<String> {
    match placement {
        Placement::Placed { .. } | Placement::Refused { .. } => None,
        Placement::NotPlaced { shortfalls } => Some(unplaced_note(task_id, shortfalls)),
    }
}

/// La phrase que lit l'exploitant quand une mission n'a pas trouvé d'hôte.
///
/// # Ce qu'elle ne fond pas
///
/// Les sept motifs de §10.2 sont distincts parce qu'ils envoient à des endroits différents, et les
/// résumer en « l'hôte ne convient pas » annulerait le travail que `packages/lep` a fait pour les
/// séparer. `level_unavailable` envoie changer de machine ; `level_not_attested` envoie lancer une
/// campagne de self-tests ; `capacity_exceeded` envoie libérer de la place ;
/// `disk_quota_not_enforceable` envoie changer de système de fichiers. Confondre les deux derniers
/// ferait réduire une réservation qui aurait échoué de la même façon à un octet.
///
/// # Aucune créance, aucun secret
///
/// La phrase porte la tâche, les workers examinés et leurs manques. Elle ne porte **pas** la créance
/// qui a réclamé : c'est un secret durable, et un diagnostic n'a jamais besoin de le citer pour être
/// utile — le `worker_id` suffit à savoir de qui on parle.
#[must_use]
pub fn unplaced_note(task_id: &str, shortfalls: &[Shortfall]) -> String {
    if shortfalls.is_empty() {
        // Inatteignable par le chemin normal : `lep_claim` soumet exactement un worker. Si cela
        // arrivait, c'est le daemon qui aurait un défaut, et le dire vaut mieux que rendre une
        // phrase qui laisserait croire à un hôte insuffisant.
        return format!(
            "« {task_id} » n'a pas été placée et aucun worker n'a été soumis au broker : \
             c'est un défaut du daemon, pas un manque d'hôte"
        );
    }

    let details = shortfalls
        .iter()
        .map(|shortfall| {
            let motifs = shortfall
                .reasons
                .iter()
                .map(motif)
                .collect::<Vec<_>>()
                .join(" ; ");
            // Un worker sans motif est un répondant qui dit non sans dire pourquoi. C'est une
            // information, et l'écrire « aucun manque » serait un contresens.
            let motifs = if motifs.is_empty() {
                "sans motif — le répondant a refusé sans dire quoi".to_owned()
            } else {
                motifs
            };
            format!("« {} » : {motifs}", shortfall.worker)
        })
        .collect::<Vec<_>>()
        .join(" | ");

    format!(
        "« {task_id} » retourne en file : aucun des {} worker(s) soumis ne convient — {details}",
        shortfalls.len()
    )
}

/// Un motif de §10.2, en une phrase qui dit **où aller**.
fn motif(reason: &Reason) -> String {
    match reason {
        Reason::LevelUnavailable { required, best } => format!(
            "confinement {required:?} exigé, l'hôte ne sait pas dépasser {best:?} — changer de machine"
        ),
        Reason::LevelNotAttested { required, proven } => match proven {
            // `None` et « prouvé trop bas » sont deux ignorances différentes, et `packages/lep` le
            // dit dans sa propre documentation : l'une envoie lancer les self-tests, l'autre dit
            // que l'hôte a échoué à les passer.
            None => format!(
                "confinement {required:?} annoncé mais jamais prouvé, aucune campagne n'a conclu — \
                 lancer les self-tests"
            ),
            Some(niveau) => format!(
                "confinement {required:?} exigé, prouvé seulement jusqu'à {niveau:?} — l'hôte a \
                 échoué à passer les self-tests au-dessus"
            ),
        },
        Reason::CapacityExceeded => {
            "la réservation dépasse la capacité de l'hôte — libérer de la place ou réduire la \
             réservation"
                .to_owned()
        }
        Reason::DiskQuotaNotEnforceable { requested, why } => format!(
            "l'hôte ne sait pas **borner** {requested} Mo de disque ({why}) — changer de système de \
             fichiers ou de machine, pas réduire la réservation"
        ),
        Reason::AcceleratorUnavailable { kind } => {
            format!("aucun accélérateur « {kind} » sur cet hôte")
        }
        Reason::AcceleratorOutsideSandbox {
            kind,
            required,
            native_level,
        } => format!(
            "l'accélérateur « {kind} » est sur cet hôte mais pas en {required:?} — il n'est \
             atteignable qu'en {native_level:?}, donc choisir entre le confinement et \
             l'accélérateur"
        ),
        Reason::NetworkModeUnsupported { mode } => {
            format!("l'hôte ne sait pas appliquer le mode réseau {mode:?}")
        }
    }
}
