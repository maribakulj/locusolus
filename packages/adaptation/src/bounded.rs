//! `bounded` et `operator` — ADR 0016, décision 8, les deux barreaux qui manquaient.
//!
//! # Ce que `bounded` retire, et ce qu'il ne retire pas
//!
//! Sous `bounded`, un agent commite dans une région sans qu'un humain approuve. Il ne s'approuve
//! pas lui-même pour autant : le mode **retire l'approbation**, il ne la lui confie pas. Rien ici ne
//! produit d'`Approved` — cette valeur enregistre le jugement d'une personne, et en fabriquer une au
//! nom d'un agent mettrait un nom sur un jugement que personne n'a porté. `forbid_self_approval`
//! (§20.3, et l'ADR le range parmi les trois bornes qui ne se relâchent dans aucun mode) reste donc
//! vrai sans qu'on ait à le vérifier ici : il n'y a pas d'approbation à détourner.
//!
//! # La classe de risque est dérivée, jamais déclarée
//!
//! [`RiskClass`] n'a qu'un constructeur, [`RiskClass::of`], qui lit un `Diff` et unit les invariants
//! que `region::threatens` attribue à chaque opération. Il n'existe ni `new`, ni champ public, ni
//! `From` : un proposeur **n'a nulle part où écrire** sa classe de risque. C'est la seule forme qui
//! tienne, parce que la classe décide de ce que l'agent peut committer sans humain — la déclarer
//! reviendrait à lui laisser choisir son propre plafond.
//!
//! # Le refus nomme l'invariant
//!
//! `Region` a déjà un `risk_ceiling`, et c'est un **nombre** : une borne sur la quantité
//! d'invariants qu'une opération peut menacer, comme `docs/13` la définit. Elle reste ce qu'elle
//! est. Mais son refus dit « risque 1 pour un plafond de 0 », ce qui ne renseigne personne, et sous
//! `bounded` ce refus est la seule chose qu'un humain lira jamais de la décision. Le plafond de ce
//! module est donc un **ensemble** — les invariants qu'un agent a le droit de menacer — et le refus
//! les nomme. C'est la règle de W16.b appliquée un cran plus loin : le refus nomme l'invariant, pas
//! le lieu, et pas davantage le compte.
//!
//! # `operator` n'est pas un barreau de plus sur la même échelle
//!
//! C'est le mode le plus privilégié et celui qui permet à un agent le moins : rien. Il décrit la
//! session d'un humain nommé qui répare, force un rollback, exécute une opération privilégiée.
//! [`Operator::taking`] n'accepte donc qu'un `Author::Human`, et aucun `Author::Agent` n'a de chemin
//! vers lui — pas parce qu'une règle l'interdit, mais parce que la fonction ne sait pas en faire un.

use std::collections::BTreeSet;
use std::fmt;

use locus_coordination::{
    ApprovalMode, Author, Diff, Invariant, Mode, Refusal, Verdict, threatens,
};
use locus_policy::{Outcome, Verb};

// ---------------------------------------------------------------------------------------------
// La classe de risque
// ---------------------------------------------------------------------------------------------

/// Les invariants qu'un lot d'opérations menace — **calculés**, jamais annoncés.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskClass {
    threatened: BTreeSet<Invariant>,
}

impl RiskClass {
    /// Dériver la classe de risque d'un diff.
    ///
    /// L'union des menaces de chaque opération, et non leur maximum : deux opérations qui menacent
    /// chacune un invariant différent en menacent deux ensemble, et un maximum en cacherait un.
    #[must_use]
    pub fn of(diff: &Diff) -> Self {
        Self {
            threatened: diff.operations().iter().flat_map(threatens).collect(),
        }
    }

    /// Les invariants menacés.
    #[must_use]
    pub const fn threatened(&self) -> &BTreeSet<Invariant> {
        &self.threatened
    }

    /// Vrai quand ce lot ne menace rien.
    ///
    /// Distinct de « le lot est vide » : un diff peut porter des opérations qui ne menacent aucun
    /// invariant, et c'est le cas ordinaire.
    #[must_use]
    pub fn threatens_nothing(&self) -> bool {
        self.threatened.is_empty()
    }
}

/// Les invariants qu'un agent a le droit de menacer sous `bounded`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ceiling {
    tolerated: BTreeSet<Invariant>,
}

impl Ceiling {
    /// Le plafond le plus bas : aucun invariant menaçable.
    ///
    /// C'est le défaut, et il vaut mieux qu'il soit celui qu'on obtient sans réfléchir. Élargir un
    /// plafond est une décision qui se lit dans un diff ; ne pas l'avoir resserré ne se lit nulle
    /// part.
    #[must_use]
    pub fn untouchable() -> Self {
        Self::default()
    }

    /// Tolérer que ces invariants soient menacés.
    #[must_use]
    pub fn tolerating(invariants: &[Invariant]) -> Self {
        Self {
            tolerated: invariants.iter().copied().collect(),
        }
    }

    /// Les invariants tolérés.
    #[must_use]
    pub const fn tolerated(&self) -> &BTreeSet<Invariant> {
        &self.tolerated
    }

    /// Le premier invariant menacé que ce plafond ne tolère pas.
    ///
    /// « Le premier » suit l'ordre de l'énumération, qui est stable : deux exécutions sur le même
    /// lot nomment le même invariant, sinon on corrigerait au hasard.
    #[must_use]
    pub fn exceeded_by(&self, class: &RiskClass) -> Option<Invariant> {
        class
            .threatened()
            .iter()
            .find(|invariant| !self.tolerated.contains(invariant))
            .copied()
    }
}

// ---------------------------------------------------------------------------------------------
// L'autonomie sous `bounded`
// ---------------------------------------------------------------------------------------------

/// Le droit de committer un lot **sans qu'un humain approuve**.
///
/// Aucun constructeur : [`autonomously`] est le seul producteur, et il exige les six conditions
/// écrites là-bas. Ce n'est pas un `Approved` et ça ne s'y convertit pas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Autonomy {
    region: String,
    by: String,
    class: RiskClass,
}

impl Autonomy {
    /// La région dans laquelle le lot a été accepté.
    #[must_use]
    pub fn region(&self) -> &str {
        &self.region
    }

    /// La règle de politique qui a rendu `allow`.
    ///
    /// §20.5 : une décision dit **par quoi** elle est portée. Un commit autonome n'a pas
    /// d'approbateur à nommer ; il a une règle, et la taire ferait d'un commit sans humain un commit
    /// sans origine.
    #[must_use]
    pub fn by(&self) -> &str {
        &self.by
    }

    /// La classe de risque dérivée qui a été tolérée.
    ///
    /// Elle est conservée parce qu'un commit autonome doit pouvoir se relire : savoir *qu'*il est
    /// passé ne dit pas *ce qu'*il menaçait, et c'est la seconde question qu'on pose après coup.
    #[must_use]
    pub const fn class(&self) -> &RiskClass {
        &self.class
    }
}

/// Décider si ce lot peut être commité sans qu'un humain approuve.
///
/// Six conditions, dans cet ordre. Le mode d'abord — inutile de calculer quoi que ce soit pour un
/// déploiement qui n'autorise pas l'autonomie —, puis le verdict de la politique, puis celui de la
/// région (W15.c) et ses deux bornes qui **obligent**, puis la classe de risque dérivée.
///
/// # Qui approuve, quand personne n'approuve
///
/// L'ADR 0016 décision 8 donne le mécanisme de `bounded` : « `allow` dans les bornes de `scope` et
/// `budget_ceiling` ». Le `allow` du moteur de politique tient donc la place que `ApprovalMode::Peer`
/// réserve à « n'importe qui d'autre que l'auteur » — et il la tient sans effort, n'étant l'auteur
/// de rien. Ce module ne réévalue aucune règle : il lit l'`Outcome` que le moteur a rendu, avec sa
/// trace. Une politique de `bounded` écrite ici serait une deuxième politique, invisible du moteur et
/// non tracée.
///
/// Une région qui déclare `ApprovalMode::Human` a dit, **dans son propre périmètre**, qu'une
/// personne doit regarder. Un mode ne surclasse pas un périmètre.
///
/// # Errors
///
/// [`NotBounded`](Denial::NotBounded) quand le mode n'est pas `bounded` ;
/// [`PolicyDidNotAllow`](Denial::PolicyDidNotAllow) quand le moteur n'a pas rendu `allow` — un
/// silence, un conflit et un `require_approval` y tombent ensemble, parce qu'aucun n'est une
/// autorisation ; [`RegionRefused`](Denial::RegionRefused) et [`RegionVetoed`](Denial::RegionVetoed)
/// quand la région ou la cohérence globale ont déjà dit non ;
/// [`HumanApprovalRequired`](Denial::HumanApprovalRequired) et
/// [`ShadowRequired`](Denial::ShadowRequired) quand la région exige l'une ou l'autre ;
/// [`ThreatensInvariant`](Denial::ThreatensInvariant) quand le lot menace un invariant que le
/// plafond ne tolère pas.
pub fn autonomously(
    mode: Mode,
    allowed: &Outcome,
    verdict: &Verdict,
    diff: &Diff,
    ceiling: &Ceiling,
) -> Result<Autonomy, Denial> {
    if !mode.dispenses_with_approval() {
        return Err(Denial::NotBounded { mode });
    }
    let by = match allowed {
        Outcome::Decided {
            verb: Verb::Allow,
            by,
        } => by.clone(),
        _ => return Err(Denial::PolicyDidNotAllow),
    };
    let accepted = match verdict {
        Verdict::Admissible(acceptance) => acceptance,
        Verdict::Refused(refusal) => return Err(Denial::RegionRefused(refusal.clone())),
        Verdict::Vetoed { invariant, .. } => {
            return Err(Denial::RegionVetoed {
                invariant: *invariant,
            });
        }
    };
    if accepted.requires_approval() == ApprovalMode::Human {
        return Err(Denial::HumanApprovalRequired {
            region: accepted.region().to_owned(),
        });
    }
    if accepted.requires_shadow() {
        return Err(Denial::ShadowRequired {
            region: accepted.region().to_owned(),
        });
    }
    let class = RiskClass::of(diff);
    if let Some(invariant) = ceiling.exceeded_by(&class) {
        return Err(Denial::ThreatensInvariant { invariant });
    }
    Ok(Autonomy {
        region: accepted.region().to_owned(),
        by,
        class,
    })
}

/// Ce qui empêche un commit autonome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Denial {
    /// Le déploiement n'est pas en `bounded`.
    NotBounded {
        /// Ce qu'il est.
        mode: Mode,
    },
    /// Le moteur de politique n'a pas rendu `allow`.
    PolicyDidNotAllow,
    /// Une borne de la région a déjà refusé le lot.
    ///
    /// Le refus de W15.c est transporté tel quel : il dit **laquelle** des quatre bornes a mordu, et
    /// le résumer en « refusé » ferait relire la région à la main.
    RegionRefused(Refusal),
    /// La cohérence globale a vetoé le lot.
    RegionVetoed {
        /// L'invariant rompu.
        invariant: Invariant,
    },
    /// La région exige qu'un **humain** approuve, et un mode ne surclasse pas un périmètre.
    HumanApprovalRequired {
        /// Laquelle.
        region: String,
    },
    /// La région exige une ombre, que `bounded` ne lève pas.
    ShadowRequired {
        /// Laquelle.
        region: String,
    },
    /// Le lot menace un invariant que le plafond ne tolère pas.
    ThreatensInvariant {
        /// Lequel. **Pas** un compte, pas un plafond : le nom.
        invariant: Invariant,
    },
}

impl fmt::Display for Denial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotBounded { mode } => write!(
                formatter,
                "le déploiement est en `{mode}` : seul `bounded` dispense de l'approbation"
            ),
            Self::RegionRefused(refusal) => write!(
                formatter,
                "la région a refusé le lot ({}), et `bounded` ne rattrape pas cela",
                refusal.bound()
            ),
            Self::RegionVetoed { invariant } => write!(
                formatter,
                "le lot rompt `{invariant}` par un chemin passant hors de la région"
            ),
            Self::PolicyDidNotAllow => formatter.write_str(
                "le moteur de politique n'a pas rendu `allow`, et rien d'autre n'est une autorisation",
            ),
            Self::HumanApprovalRequired { region } => write!(
                formatter,
                "la région `{region}` exige qu'un humain approuve, et un mode ne surclasse pas un périmètre"
            ),
            Self::ShadowRequired { region } => write!(
                formatter,
                "la région `{region}` exige une ombre avant tout commit"
            ),
            Self::ThreatensInvariant { invariant } => write!(
                formatter,
                "le lot menace `{invariant}`, que le plafond de ce déploiement ne tolère pas"
            ),
        }
    }
}

impl std::error::Error for Denial {}

// ---------------------------------------------------------------------------------------------
// `operator`
// ---------------------------------------------------------------------------------------------

/// Un opérateur : un humain **nommé**.
///
/// Il n'y a pas de constructeur depuis une chaîne. Le seul chemin passe par un [`Author`], et
/// `Author::Agent` y est refusé — non par une règle qu'on pourrait déplacer, mais parce qu'aucune
/// branche ne sait en fabriquer un.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operator {
    principal: String,
}

impl Operator {
    /// Prendre la main sur un déploiement en `operator`.
    ///
    /// # Errors
    ///
    /// [`NotOperatorMode`](OperatorError::NotOperatorMode) quand le déploiement n'est pas en
    /// `operator`, et [`NotAHuman`](OperatorError::NotAHuman) pour un agent. Les deux refus sont
    /// distincts : le premier se corrige en changeant le mode, ce qui est « un acte gouverné et
    /// journalisé » (ADR 0016) ; le second ne se corrige pas du tout.
    pub fn taking(author: &Author, mode: Mode) -> Result<Self, OperatorError> {
        if mode != Mode::Operator {
            return Err(OperatorError::NotOperatorMode { mode });
        }
        match author {
            Author::Human(principal) => Ok(Self {
                principal: principal.clone(),
            }),
            Author::Agent(instance) => Err(OperatorError::NotAHuman {
                agent: instance.to_string(),
            }),
        }
    }

    /// Son principal.
    #[must_use]
    pub fn principal(&self) -> &str {
        &self.principal
    }
}

/// Ce qui empêche de prendre la main.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorError {
    /// Le déploiement n'est pas en `operator`.
    NotOperatorMode {
        /// Ce qu'il est.
        mode: Mode,
    },
    /// L'auteur est un agent.
    NotAHuman {
        /// Lequel.
        agent: String,
    },
}

impl fmt::Display for OperatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotOperatorMode { mode } => write!(
                formatter,
                "le déploiement est en `{mode}` : les opérations privilégiées demandent `operator`"
            ),
            Self::NotAHuman { agent } => write!(
                formatter,
                "`{agent}` est un agent, et `operator` est tenu par un humain nommé, jamais un agent"
            ),
        }
    }
}

impl std::error::Error for OperatorError {}
