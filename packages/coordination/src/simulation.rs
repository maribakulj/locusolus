//! Le plan de simulation — `docs/10` W16, ADR 0016 décision 9.
//!
//! # Quatre degrés, et le dernier est facultatif
//!
//! « Rejeu déterministe, substitut d'environnement enregistré, ombre en sandbox réelle, canari
//! facultatif. » Ils vont du moins fidèle au plus fidèle, et un plan peut s'arrêter avant le
//! dernier. Ce qui ne doit jamais arriver est qu'un résultat obtenu à un degré soit lu comme s'il
//! venait d'un degré supérieur : un rejeu ne dit pas ce qu'un canari dirait, et [`Outcome`] porte
//! donc **le degré réellement atteint**, jamais celui qui était visé.
//!
//! # Un substitut qui n'a pas la réponse le dit
//!
//! C'est la faute que ce module existe pour empêcher, et elle est silencieuse. Un substitut
//! d'environnement qui rendrait une valeur par défaut — chaîne vide, zéro, « inconnu » — ferait
//! réussir une simulation là où le run réel aurait échoué. Or prédire est **la seule chose** qu'on
//! demande à une simulation : celle qui se trompe dans ce sens-là est pire qu'absente, puisqu'on
//! s'appuie dessus.
//!
//! [`Answer::NotRecorded`] est donc un résultat, pas une erreur de plomberie, et une simulation qui
//! en rencontre un ne rend **aucun verdict** : elle rend ce qui manque. « Pas vérifié » n'est jamais
//! « réussi ».
//!
//! # Un objet simulé n'existe pas comme type dans le domaine épistémique
//!
//! ADR 0016, décision 9 : « la garantie est une **absence de type**, pas un champ de
//! classification ». Un champ `evidence_class` reposerait sur le fait que *chaque* consommateur le
//! vérifie, et c'est le genre d'invariant qui tient six mois. Pis : `packages/validation` propage
//! l'invalidation sur les niveaux de §8.1, et un niveau `simulated` y ferait circuler la simulation.
//!
//! La garantie est donc structurelle. Un [`Outcome`] désigne une **proposition** par son
//! identifiant de décision, et **rien d'autre** : il ne peut pas nommer une `RevisionId`, donc il ne
//! peut pas être cité comme preuve à propos d'un objet épistémique. Il n'existe par ailleurs aucune
//! fonction qui rende un `ValidationLevel`. Deux tests le tiennent par l'absence.

use std::collections::BTreeMap;
use std::fmt;
use std::fmt::Write as _;

use locus_protocol::{Id, id::provisional::Decision as DecisionKind};

/// Les quatre degrés du plan, du moins fidèle au plus fidèle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Fidelity {
    /// Rejouer une trace enregistrée. Rien ne s'exécute.
    Replay,
    /// Exécuter contre un substitut d'environnement enregistré.
    RecordedEnvironment,
    /// Exécuter en ombre, dans une sandbox réelle, sans effet institutionnel.
    Shadow,
    /// Exposer une fraction du trafic réel. **Facultatif** : un plan peut s'arrêter avant.
    Canary,
}

impl Fidelity {
    /// Les quatre, dans l'ordre de fidélité croissante.
    pub const ALL: [Self; 4] = [
        Self::Replay,
        Self::RecordedEnvironment,
        Self::Shadow,
        Self::Canary,
    ];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Replay => "replay",
            Self::RecordedEnvironment => "recorded-environment",
            Self::Shadow => "shadow",
            Self::Canary => "canary",
        }
    }
}

impl fmt::Display for Fidelity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Ce qu'un substitut d'environnement répond.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// Ce qui a été enregistré.
    Recorded(String),
    /// Rien n'a été enregistré pour cette question.
    ///
    /// **Un résultat, pas un incident.** Rendre une valeur par défaut ferait réussir la simulation
    /// là où le run réel aurait échoué, et prédire est la seule chose qu'on lui demande.
    NotRecorded {
        /// Laquelle.
        question: String,
    },
}

/// Un substitut d'environnement enregistré.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Recorded {
    answers: BTreeMap<String, String>,
}

impl Recorded {
    /// Un substitut qui ne sait rien.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enregistrer une réponse.
    #[must_use]
    pub fn answering(mut self, question: &str, answer: &str) -> Self {
        self.answers.insert(question.to_owned(), answer.to_owned());
        self
    }

    /// Interroger le substitut.
    ///
    /// Il n'existe **aucune** variante de cette méthode qui prenne un défaut : `ask_or` serait la
    /// porte par laquelle une valeur inventée entrerait, et personne ne la verrait passer.
    #[must_use]
    pub fn ask(&self, question: &str) -> Answer {
        self.answers.get(question).map_or_else(
            || Answer::NotRecorded {
                question: question.to_owned(),
            },
            |answer| Answer::Recorded(answer.clone()),
        )
    }
}

/// Ce qu'une simulation a produit.
///
/// Elle désigne une **proposition**, et rien d'autre. Aucun champ ne peut nommer une révision
/// épistémique, donc aucun résultat de simulation ne peut être cité comme preuve à propos d'un
/// claim — ADR 0016, décision 9.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    proposal: Id<DecisionKind>,
    reached: Fidelity,
    verdict: Verdict,
}

impl Outcome {
    /// La proposition simulée.
    #[must_use]
    pub const fn proposal(&self) -> Id<DecisionKind> {
        self.proposal
    }

    /// Le degré **réellement atteint**, jamais celui qui était visé.
    ///
    /// Un rejeu ne dit pas ce qu'un canari dirait. Rendre le degré visé laisserait citer une
    /// simulation pour ce qu'elle n'a pas fait.
    #[must_use]
    pub const fn reached(&self) -> Fidelity {
        self.reached
    }

    /// Ce qu'elle conclut.
    #[must_use]
    pub const fn verdict(&self) -> &Verdict {
        &self.verdict
    }
}

/// Ce qu'une simulation conclut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Toutes les questions du plan ont eu une réponse enregistrée.
    Complete {
        /// La forme canonique de ce qui a été observé — deux rejeux la produisent identique.
        observed: String,
    },
    /// Le substitut n'avait pas tout, et **rien n'est conclu**.
    ///
    /// Les questions sans réponse sont nommées. Conclure quand même reviendrait à dire « pas
    /// vérifié » et à l'écrire « réussi ».
    Incomplete {
        /// Ce qui n'a jamais été enregistré.
        unanswered: Vec<String>,
    },
}

impl Verdict {
    /// Vrai quand la simulation a pu conclure.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        matches!(self, Self::Complete { .. })
    }
}

/// Mener une simulation.
///
/// # Le déterminisme, et d'où il vient
///
/// Le rejeu ne consulte **rien** d'autre que le substitut : ni horloge, ni ordre d'itération d'un
/// conteneur non ordonné, ni environnement. C'est ce qui fait que deux rejeux de la même trace
/// rendent le même résultat — pas une promesse, une conséquence de ce que la fonction peut voir.
#[must_use]
pub fn run(
    proposal: Id<DecisionKind>,
    reached: Fidelity,
    plan: &[&str],
    environment: &Recorded,
) -> Outcome {
    let mut observed = format!("simulation/1\n{reached}\n");
    let mut unanswered = Vec::new();
    for question in plan {
        match environment.ask(question) {
            Answer::Recorded(answer) => {
                let _ = writeln!(observed, "{question}\t{answer}");
            }
            Answer::NotRecorded { question } => unanswered.push(question),
        }
    }
    let verdict = if unanswered.is_empty() {
        Verdict::Complete { observed }
    } else {
        Verdict::Incomplete { unanswered }
    };
    Outcome {
        proposal,
        reached,
        verdict,
    }
}
