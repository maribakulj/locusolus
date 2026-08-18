//! Le spawn dynamique — `docs/SPEC_V1.md` §14.5.
//!
//! # Les onze déclencheurs sont une liste close
//!
//! §14.5 les donne dans un bloc, sous leur nom. Ils sont ici une énumération, et le champ `reason`
//! d'une proposition est **l'un d'eux**, pas une phrase. La tentation inverse est forte — une prose
//! dit mieux pourquoi *maintenant*, sur *cette* branche — et elle est exactement ce qu'il ne faut
//! pas : un `reason` libre laisse entrer un douzième déclencheur que personne n'a déclaré, et la
//! liste de §14.5 ne dit plus rien de ce que le système fait. Le *maintenant* et le *cette branche*
//! ont leur place ailleurs : dans les faits que le moteur de politique évalue, et dans la trace
//! qu'il produit.
//!
//! # Les neuf champs sont obligatoires, et l'absence n'est pas un champ vide
//!
//! §14.5 énumère neuf clés. Une proposition à qui il en manque une n'est pas construite : il n'y a
//! pas de `Default`, pas de constructeur partiel, pas de `with_*` qui compléterait après coup. Une
//! proposition incomplète circulerait, serait évaluée, et le moteur trancherait sur ce qu'elle ne
//! dit pas.
//!
//! # Ce que le moteur a le droit de savoir, et ce qu'il ne doit pas savoir
//!
//! [`SpawnProposal::facts`] ne livre pas les neuf champs. Deux en sont retirés :
//! `expected_information_gain` et `diversity_contribution`. Ce sont des **prétentions de valeur**,
//! et §13.4 en fait les termes `G` et `D` d'une fonction que le portefeuille calcule lui-même. Une
//! règle de politique qui s'y accrocherait laisserait le proposeur choisir son propre verdict en
//! choisissant son propre chiffre — la même faute que `forbid_self_approval` (§20.3) interdit sur
//! l'approbation, commise sur l'admission.
//!
//! `cost_estimate` reste un fait, et ce n'est pas une incohérence : un coût est une **borne**, pas
//! une prétention. Un proposeur qui le sous-estime ne gagne rien, parce que l'invariant 6 réserve
//! les ressources avant l'exécution et que la réservation, elle, ne croit personne.

use std::fmt;
use std::time::Duration;

use locus_budget::{Dimension, Limits};
use locus_coordination::{Capability, Command};
use locus_policy::{Facts, Outcome, Verb};

// ---------------------------------------------------------------------------------------------
// Les onze déclencheurs
// ---------------------------------------------------------------------------------------------

/// Ce qui peut faire proposer un agent de plus — les onze de §14.5.
///
/// Ce sont des faits que l'agent **observe**, non des intentions qu'il forme : ADR 0016 décision 7
/// le note pour justifier qu'un agent soit auteur de proposition dès W13.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Trigger {
    /// Un domaine manque à l'équipe.
    DomainGapDetected,
    /// Deux revues ne disent pas la même chose.
    ReviewDisagreement,
    /// Une barrière a été rencontrée.
    BarrierEncountered,
    /// Une branche n'avance plus.
    BranchStagnation,
    /// La formalisation est bloquée.
    FormalizationBlocked,
    /// Il faudrait un contre-exemple.
    CounterexampleNeeded,
    /// Une méthode nouvelle est apparue.
    NewMethodFound,
    /// Un pont entre deux branches est envisageable.
    BridgeCandidate,
    /// L'incertitude est haute.
    HighUncertainty,
    /// Une reproduction a échoué.
    ReproductionFailure,
    /// Deux sources se contredisent.
    SourceConflict,
}

impl Trigger {
    /// Les onze, dans l'ordre du bloc de §14.5.
    pub const ALL: [Self; 11] = [
        Self::DomainGapDetected,
        Self::ReviewDisagreement,
        Self::BarrierEncountered,
        Self::BranchStagnation,
        Self::FormalizationBlocked,
        Self::CounterexampleNeeded,
        Self::NewMethodFound,
        Self::BridgeCandidate,
        Self::HighUncertainty,
        Self::ReproductionFailure,
        Self::SourceConflict,
    ];

    /// Son nom, celui du bloc de §14.5.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::DomainGapDetected => "domain_gap_detected",
            Self::ReviewDisagreement => "review_disagreement",
            Self::BarrierEncountered => "barrier_encountered",
            Self::BranchStagnation => "branch_stagnation",
            Self::FormalizationBlocked => "formalization_blocked",
            Self::CounterexampleNeeded => "counterexample_needed",
            Self::NewMethodFound => "new_method_found",
            Self::BridgeCandidate => "bridge_candidate",
            Self::HighUncertainty => "high_uncertainty",
            Self::ReproductionFailure => "reproduction_failure",
            Self::SourceConflict => "source_conflict",
        }
    }

    /// Le déclencheur de ce nom, s'il y en a un.
    ///
    /// Rend `None` plutôt qu'un déclencheur par défaut. Un nom inconnu lu comme
    /// `domain_gap_detected` ferait proposer un spawn pour une raison que personne n'a observée.
    #[must_use]
    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|trigger| trigger.slug() == slug)
    }
}

impl fmt::Display for Trigger {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

// ---------------------------------------------------------------------------------------------
// La proposition
// ---------------------------------------------------------------------------------------------

/// Les neuf clés de §14.5, telles qu'un proposeur les écrit.
///
/// Ce type est le formulaire, pas la proposition. Ses champs sont publics et Rust exige qu'ils
/// soient **tous** donnés d'un coup : il n'existe pas de `Draft` partiel qu'on compléterait ensuite,
/// pas de `Default`, pas de `with_*`. « Une proposition à qui il manque un champ n'existe pas » est
/// donc vrai avant même que [`SpawnProposal::declare`] regarde quoi que ce soit — la validation ne
/// rattrape que ce qui a été écrit, jamais ce qui a été omis.
///
/// Les champs portent les noms du bloc YAML de §14.5, sans traduction ni abréviation.
#[derive(Debug, Clone, PartialEq)]
pub struct Draft {
    /// Le déclencheur observé — l'un des onze, jamais une phrase.
    pub reason: Trigger,
    /// La capacité qui manque à l'équipe.
    pub missing_capability: Capability,
    /// Le gain d'information attendu, entre 0 et 1.
    pub expected_information_gain: f64,
    /// La contribution de diversité annoncée, entre 0 et 1.
    pub diversity_contribution: f64,
    /// Le coût estimé, comme jeu de bornes.
    pub cost_estimate: Limits,
    /// La durée de vie demandée.
    pub time_to_live: Duration,
    /// À quoi l'agent s'arrête.
    pub termination_condition: String,
    /// La politique de contexte, **par référence**.
    pub context_policy: String,
    /// La politique de revue, **par référence**.
    pub review_policy: String,
}

/// Une proposition de spawn — les neuf clés de §14.5, vérifiées.
#[derive(Debug, Clone, PartialEq)]
pub struct SpawnProposal {
    reason: Trigger,
    missing_capability: Capability,
    expected_information_gain: f64,
    diversity_contribution: f64,
    cost_estimate: Limits,
    time_to_live: Duration,
    termination_condition: String,
    context_policy: String,
    review_policy: String,
}

impl SpawnProposal {
    /// Déclarer une proposition complète.
    ///
    /// Les deux politiques sont des **références** — un identifiant que §8.2 laisse au moteur, pas
    /// un jeu de règles écrit ici. Les laisser s'inliner ferait écrire au proposeur les règles qui
    /// jugeront sa descendance ; ce serait `forbid_self_approval` contourné d'une génération.
    ///
    /// # Errors
    ///
    /// [`SpawnError::EmptyField`] pour une condition de terminaison ou une politique sans nom :
    /// une terminaison vide est une flotte sans fin, et c'est la phrase de §14.5 qu'elle contredit.
    ///
    /// [`SpawnError::NotAProportion`] pour un gain attendu ou une contribution de diversité hors de
    /// `0..=1`. Le test de plage refuse aussi `NaN`, parce qu'une comparaison avec `NaN` est fausse
    /// et que la plage est donc réputée non contenue — un `is_finite` de plus serait mort.
    ///
    /// [`SpawnError::ZeroTimeToLive`] pour une durée de vie nulle. Zéro n'est pas « pas de limite »,
    /// et l'écrire ainsi serait le seul champ du type dont la valeur la plus permissive est aussi
    /// celle qu'on obtient en ne réfléchissant pas.
    pub fn declare(draft: Draft) -> Result<Self, SpawnError> {
        proportion("expected_information_gain", draft.expected_information_gain)?;
        proportion("diversity_contribution", draft.diversity_contribution)?;
        if draft.time_to_live.is_zero() {
            return Err(SpawnError::ZeroTimeToLive);
        }
        for (field, value) in [
            ("termination_condition", &draft.termination_condition),
            ("context_policy", &draft.context_policy),
            ("review_policy", &draft.review_policy),
        ] {
            if value.trim().is_empty() {
                return Err(SpawnError::EmptyField { field });
            }
        }
        Ok(Self {
            reason: draft.reason,
            missing_capability: draft.missing_capability,
            expected_information_gain: draft.expected_information_gain,
            diversity_contribution: draft.diversity_contribution,
            cost_estimate: draft.cost_estimate,
            time_to_live: draft.time_to_live,
            termination_condition: draft.termination_condition,
            context_policy: draft.context_policy,
            review_policy: draft.review_policy,
        })
    }

    /// Le déclencheur observé.
    #[must_use]
    pub const fn reason(&self) -> Trigger {
        self.reason
    }

    /// La capacité qui manque.
    #[must_use]
    pub const fn missing_capability(&self) -> &Capability {
        &self.missing_capability
    }

    /// Le gain d'information attendu, entre 0 et 1.
    #[must_use]
    pub const fn expected_information_gain(&self) -> f64 {
        self.expected_information_gain
    }

    /// La contribution de diversité annoncée, entre 0 et 1.
    #[must_use]
    pub const fn diversity_contribution(&self) -> f64 {
        self.diversity_contribution
    }

    /// Le coût estimé, comme jeu de bornes.
    #[must_use]
    pub const fn cost_estimate(&self) -> &Limits {
        &self.cost_estimate
    }

    /// La durée de vie demandée.
    #[must_use]
    pub const fn time_to_live(&self) -> Duration {
        self.time_to_live
    }

    /// À quoi l'agent s'arrête.
    #[must_use]
    pub fn termination_condition(&self) -> &str {
        &self.termination_condition
    }

    /// La politique de contexte, par référence.
    #[must_use]
    pub fn context_policy(&self) -> &str {
        &self.context_policy
    }

    /// La politique de revue, par référence.
    #[must_use]
    pub fn review_policy(&self) -> &str {
        &self.review_policy
    }

    /// Ce que le moteur de politique a le droit de connaître de cette proposition.
    ///
    /// §20.2 : « séparer faits d'entrée et décision ». Ce qui n'est pas ici n'entre pas dans la
    /// décision — et deux champs n'y sont volontairement pas, pour la raison écrite en tête de
    /// module.
    #[must_use]
    pub fn facts(&self) -> Facts {
        let mut facts = Facts::new()
            .with("spawn.reason", self.reason.slug())
            .with("spawn.missing_capability", self.missing_capability.as_str())
            .with("spawn.context_policy", &self.context_policy)
            .with("spawn.review_policy", &self.review_policy)
            .with(
                "spawn.time_to_live_seconds",
                &self.time_to_live.as_secs().to_string(),
            );
        for dimension in Dimension::ALL {
            if let Some(ceiling) = self.cost_estimate.ceiling(dimension) {
                facts = facts.with(
                    &format!("spawn.cost.{}", dimension.slug()),
                    &ceiling.to_string(),
                );
            }
        }
        facts
    }
}

/// Refuser une valeur hors de `0..=1`, `NaN` compris.
fn proportion(field: &'static str, value: f64) -> Result<(), SpawnError> {
    if (0.0..=1.0).contains(&value) {
        return Ok(());
    }
    Err(SpawnError::NotAProportion { field, value })
}

// ---------------------------------------------------------------------------------------------
// L'admission
// ---------------------------------------------------------------------------------------------

/// Une proposition que le moteur de politique a acceptée **telle quelle**.
///
/// C'est le seul objet de ce module d'où une commande de spawn se déduit, et il n'a pas de
/// constructeur : [`dispose`] le produit, et rien d'autre. C'est la forme exécutable de « aucun
/// agent ne crée librement une flotte non bornée » — non pas une règle qu'on applique, mais une
/// valeur qu'on ne sait pas fabriquer sans verdict.
#[derive(Debug, Clone, PartialEq)]
pub struct Admitted {
    proposal: SpawnProposal,
    by: String,
}

impl Admitted {
    /// La proposition admise.
    #[must_use]
    pub const fn proposal(&self) -> &SpawnProposal {
        &self.proposal
    }

    /// La règle qui a tranché — §20.5 : une admission dit **par quoi** elle est portée.
    #[must_use]
    pub fn by(&self) -> &str {
        &self.by
    }

    /// La commande de cycle de vie que cette admission autorise.
    ///
    /// [`Command::Spawn`] et rien d'autre : une admission de spawn n'autorise pas à suspendre, à
    /// drainer ni à tuer quoi que ce soit. W16.a a déjà réglé ce que devient l'instance ensuite —
    /// elle naît `provisioned`, jamais active.
    #[must_use]
    pub const fn command(&self) -> Command {
        Command::Spawn
    }
}

/// Les façons dont le moteur **n'a pas** rendu une des quatre réponses de §14.5.
///
/// §14.5 en nomme quatre ; §20.2 donne cinq verbes au moteur, et le cinquième — `require_tasks` —
/// n'est pas une réponse à un spawn. S'y ajoutent les deux issues qui ne sont pas des verbes du
/// tout : le conflit, que §20.2 exige de **rendre** plutôt que de résoudre, et le silence, dont
/// `policy::Outcome::NoRule` dit qu'il est « distinct d'`allow` ».
///
/// Les trois sont ici, distinctes, parce qu'elles se réparent différemment : un conflit se tranche
/// en écrivant une priorité, un silence en écrivant une règle, des tâches en les menant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Undecided {
    /// Deux règles de même priorité qui ne disent pas la même chose.
    Conflict {
        /// À quelle priorité.
        priority: u32,
        /// Lesquelles.
        rules: Vec<String>,
    },
    /// Aucune règle ne s'applique. Ce n'est pas une permission.
    Silent,
    /// Le moteur exige des tâches préalables — le verbe de §20.2 que §14.5 ne prévoit pas.
    TasksFirst {
        /// Lesquelles.
        tasks: Vec<String>,
    },
}

impl fmt::Display for Undecided {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict { priority, rules } => write!(
                formatter,
                "{} règles de priorité {priority} se contredisent : la politique tranche, pas le spawn",
                rules.len()
            ),
            Self::Silent => formatter.write_str(
                "aucune règle n'a statué sur ce spawn, et un silence n'est pas une autorisation",
            ),
            Self::TasksFirst { tasks } => write!(
                formatter,
                "{} tâches sont exigées d'abord ; §14.5 ne compte pas cette réponse parmi les quatre",
                tasks.len()
            ),
        }
    }
}

/// Ce que le moteur de politique répond à une proposition de spawn.
///
/// Les quatre premières variantes sont les quatre réponses de §14.5, sous leur nom.
#[derive(Debug, Clone, PartialEq)]
pub enum Disposition {
    /// Accepter.
    Accepted(Admitted),
    /// Refuser.
    Refused {
        /// Quelle règle a refusé.
        by: String,
    },
    /// Modifier : ce qui passerait n'est pas ce qui a été demandé.
    ///
    /// La proposition modifiée n'est **pas** portée ici. Le moteur impose une contrainte, il ne
    /// réécrit pas la proposition à la place de son auteur : la rendre déjà réécrite ferait
    /// disparaître la différence entre ce qui a été demandé et ce qui a été concédé, et c'est
    /// précisément cette différence qui se conteste (§7.6 côté épistémique, W15.d côté
    /// coordination).
    Modified {
        /// Ce qui est imposé.
        constraint: String,
        /// Quelle règle l'impose.
        by: String,
    },
    /// Soumettre à approbation.
    ApprovalRequired {
        /// De quel rôle.
        approver_role: String,
        /// Quelle règle l'exige.
        by: String,
    },
    /// Le moteur n'a rendu aucune des quatre.
    Undecided(Undecided),
}

impl Disposition {
    /// L'admission, quand il y en a une.
    ///
    /// Le seul accesseur qui rende un [`Admitted`], et il rend une `Option`. Un accesseur qui
    /// dépliait la variante en supposant l'acceptation ferait de chaque autre issue un `panic` —
    /// donc, en production, un chemin qu'on prend soin de ne pas emprunter plutôt qu'un chemin qui
    /// n'existe pas.
    #[must_use]
    pub const fn admitted(&self) -> Option<&Admitted> {
        match self {
            Self::Accepted(admitted) => Some(admitted),
            _ => None,
        }
    }
}

/// Confronter une proposition au verdict d'un moteur de politique.
///
/// L'`Outcome` vient de `locus_policy`, avec sa trace : ce module ne réévalue rien et n'a aucune
/// règle à lui. Une politique de spawn qui vivrait ici serait une deuxième politique, invisible du
/// moteur, non tracée, non contestable — et §20.2 demande justement une trace produite par le même
/// passage que la décision.
#[must_use]
pub fn dispose(proposal: SpawnProposal, outcome: &Outcome) -> Disposition {
    match outcome {
        Outcome::Decided { verb, by } => match verb {
            Verb::Allow => Disposition::Accepted(Admitted {
                proposal,
                by: by.clone(),
            }),
            Verb::Deny => Disposition::Refused { by: by.clone() },
            Verb::Modify { constraint } => Disposition::Modified {
                constraint: constraint.clone(),
                by: by.clone(),
            },
            Verb::RequireApproval { approver_role } => Disposition::ApprovalRequired {
                approver_role: approver_role.clone(),
                by: by.clone(),
            },
            Verb::RequireTasks { tasks } => Disposition::Undecided(Undecided::TasksFirst {
                tasks: tasks.clone(),
            }),
        },
        Outcome::Conflict { priority, rules } => Disposition::Undecided(Undecided::Conflict {
            priority: *priority,
            rules: rules.clone(),
        }),
        Outcome::NoRule => Disposition::Undecided(Undecided::Silent),
    }
}

// ---------------------------------------------------------------------------------------------
// Les refus
// ---------------------------------------------------------------------------------------------

/// Ce qui empêche une proposition de spawn d'exister.
#[derive(Debug, Clone, PartialEq)]
pub enum SpawnError {
    /// Un champ obligatoire vide.
    EmptyField {
        /// Lequel.
        field: &'static str,
    },
    /// Un champ qui doit être une proportion et n'en est pas une.
    NotAProportion {
        /// Lequel.
        field: &'static str,
        /// Ce qui a été donné.
        value: f64,
    },
    /// Une durée de vie nulle.
    ZeroTimeToLive,
}

impl fmt::Display for SpawnError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(
                formatter,
                "`{field}` est vide : §14.5 en fait une des neuf clés d'une proposition de spawn"
            ),
            Self::NotAProportion { field, value } => write!(
                formatter,
                "`{field}` vaut {value} et doit tenir entre 0 et 1"
            ),
            Self::ZeroTimeToLive => formatter.write_str(
                "une durée de vie nulle n'est pas une absence de limite : §14.5 exige un `time_to_live`",
            ),
        }
    }
}

impl std::error::Error for SpawnError {}
