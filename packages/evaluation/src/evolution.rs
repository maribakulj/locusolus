//! L'évolution inter-exécutions — `docs/10_V1_ROADMAP.md`, item `R6`.
//!
//! « Une adaptation **récurrente** et **gagnante en validation appariée** propose une amélioration de
//! template. »
//!
//! # Trois mots, trois bornes
//!
//! **Récurrente** : vue dans plusieurs exécutions distinctes. Une victoire unique n'est pas un motif,
//! et un système qui promeut sur une observation promeut le tirage de cette observation-là.
//!
//! **Gagnante en validation appariée** : chaque occurrence est un [`Credit::Attributed`] de `R2`,
//! avec un gain positif. Ce module ne rejuge rien et ne calcule aucun écart : il **compte** des
//! verdicts déjà rendus. Refaire l'attribution ici serait une seconde attribution, avec sa propre
//! bande de bruit, qui divergerait de la première.
//!
//! **Propose** : le résultat est une [`Improvement`], et il n'existe aucun chemin qui l'applique.
//! Même forme que la boucle lente de W18.b — une adaptation de structure est une proposition qui
//! suit son chemin entier, jamais une écriture.
//!
//! # Ce qui ne se moyenne pas
//!
//! Deux exécutions qui gagnent et une qui régresse ne font pas « globalement positif ».
//! [`Evolution::Contradictory`] les rend telles quelles. Moyenner reviendrait à supprimer un
//! résultat négatif pour rendre le dossier lisible, ce que l'invariant 12 interdit — et à promouvoir
//! un template dont on sait qu'il a déjà nui une fois, sans savoir pourquoi.
//!
//! # Un seul facteur à la fois
//!
//! Si une exécution crédite la relation et une autre le budget, ce ne sont pas deux occurrences
//! d'une même adaptation : ce sont deux adaptations vues une fois chacune. Le facteur est donc donné
//! en argument, et les occurrences qui ne le concernent pas ne sont pas comptées — ni au numérateur,
//! ni au dénominateur.

use std::collections::BTreeSet;
use std::fmt;

use crate::credit::{Credit, Factor};

/// Ce qu'une exécution a conclu sur une adaptation.
#[derive(Debug, Clone, PartialEq)]
pub struct Occurrence {
    run: String,
    credit: Credit,
}

impl Occurrence {
    /// Consigner ce qu'une exécution a conclu.
    ///
    /// # Errors
    ///
    /// [`EvolutionError::UnnamedRun`] pour une exécution sans nom : « récurrente » se compte en
    /// exécutions **distinctes**, et deux exécutions anonymes ne se distinguent pas.
    pub fn in_run(run: &str, credit: Credit) -> Result<Self, EvolutionError> {
        if run.trim().is_empty() {
            return Err(EvolutionError::UnnamedRun);
        }
        Ok(Self {
            run: run.to_owned(),
            credit,
        })
    }

    /// De quelle exécution elle vient.
    #[must_use]
    pub fn run(&self) -> &str {
        &self.run
    }

    /// Ce que `R2` a conclu.
    #[must_use]
    pub const fn credit(&self) -> &Credit {
        &self.credit
    }
}

/// Une amélioration de template proposée.
///
/// Aucun constructeur : [`consider`] est le seul producteur. Et aucune méthode ne l'applique — c'est
/// une proposition, qui suivra le chemin des propositions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Improvement {
    factor: Factor,
    runs: BTreeSet<String>,
}

impl Improvement {
    /// Le facteur dont l'ajustement est proposé.
    #[must_use]
    pub const fn factor(&self) -> Factor {
        self.factor
    }

    /// Les exécutions distinctes qui l'ont vu gagner.
    ///
    /// Nommées, pas comptées : une proposition de template se conteste, et la contester demande de
    /// pouvoir aller relire les exécutions citées.
    #[must_use]
    pub const fn runs(&self) -> &BTreeSet<String> {
        &self.runs
    }

    /// Combien elles sont.
    #[must_use]
    pub fn recurrence(&self) -> usize {
        self.runs.len()
    }
}

impl fmt::Display for Improvement {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "`{}` a gagné dans {} exécutions distinctes — amélioration de template **proposée**",
            self.factor,
            self.runs.len()
        )
    }
}

/// Ce qu'un ensemble d'exécutions conclut sur une adaptation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evolution {
    /// Récurrente et gagnante : une amélioration est proposée.
    Proposed(Improvement),
    /// Elle gagne, mais dans trop peu d'exécutions distinctes.
    NotRecurrent {
        /// Combien l'ont vue gagner.
        runs: usize,
        /// Combien il en fallait.
        required: usize,
    },
    /// Elle gagne ici et nuit là. **Rien ne se moyenne.**
    Contradictory {
        /// Les exécutions où elle a gagné.
        wins: BTreeSet<String>,
        /// Celles où elle a nui.
        regressions: BTreeSet<String>,
    },
    /// Aucune exécution ne lui attribue quoi que ce soit.
    ///
    /// Distinct de `NotRecurrent` : là, le facteur gagnait sans assez d'exécutions ; ici il n'a
    /// jamais rien gagné. Les confondre ferait attendre d'autres exécutions d'un facteur que
    /// personne n'a vu marcher.
    NothingAttributed {
        /// Combien d'occurrences ont été examinées.
        examined: usize,
    },
}

/// Examiner ce que plusieurs exécutions disent d'un facteur.
///
/// # Errors
///
/// [`EvolutionError::NoRecurrenceRequired`] pour un seuil de récurrence inférieur à deux : « une
/// adaptation **récurrente** » ne se constate pas sur une exécution, et accepter un seuil de un
/// ferait promouvoir le tirage d'une observation unique.
pub fn consider(
    factor: Factor,
    occurrences: &[Occurrence],
    required: usize,
) -> Result<Evolution, EvolutionError> {
    if required < 2 {
        return Err(EvolutionError::NoRecurrenceRequired { required });
    }

    let mut wins = BTreeSet::new();
    let mut regressions = BTreeSet::new();
    let mut examined = 0;
    for occurrence in occurrences {
        let Credit::Attributed {
            factor: credited,
            gain,
        } = occurrence.credit()
        else {
            continue;
        };
        if *credited != factor {
            continue;
        }
        examined += 1;
        if *gain > 0.0 {
            wins.insert(occurrence.run().to_owned());
        } else {
            regressions.insert(occurrence.run().to_owned());
        }
    }

    if !regressions.is_empty() && !wins.is_empty() {
        return Ok(Evolution::Contradictory { wins, regressions });
    }
    if wins.is_empty() {
        return Ok(Evolution::NothingAttributed { examined });
    }
    if wins.len() < required {
        return Ok(Evolution::NotRecurrent {
            runs: wins.len(),
            required,
        });
    }
    Ok(Evolution::Proposed(Improvement { factor, runs: wins }))
}

/// Ce qui empêche d'examiner une évolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvolutionError {
    /// Une exécution sans nom.
    UnnamedRun,
    /// Un seuil de récurrence inférieur à deux.
    NoRecurrenceRequired {
        /// Ce qui a été demandé.
        required: usize,
    },
}

impl fmt::Display for EvolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnnamedRun => formatter.write_str(
                "une exécution sans nom ne se distingue pas d'une autre, et « récurrente » se compte en exécutions distinctes",
            ),
            Self::NoRecurrenceRequired { required } => write!(
                formatter,
                "un seuil de {required} ne constate aucune récurrence : il en faut au moins deux"
            ),
        }
    }
}

impl std::error::Error for EvolutionError {}
