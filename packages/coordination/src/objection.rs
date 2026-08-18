//! La contestabilité d'une décision de coordination — ADR 0016, décision 9 ; `docs/SPEC_V1.md`
//! §7.1 et §14.5.
//!
//! # Ce que ce module ajoute, et à quoi
//!
//! `Decision` porte déjà `rationale`, `evidence_refs`, `policy_evaluation_id` et `overrides` — la
//! moitié du chemin. Ce qui manquait est la famille d'objection : de quoi contester le
//! **déclencheur**, la **politique** et le **périmètre** d'une décision d'organisation, comme on
//! conteste une prémisse, une règle et un domaine de validité dans le graphe épistémique.
//!
//! Avec elle, l'histoire de l'organisation devient réfutable comme un claim. C'est la contribution
//! originale du projet, et le geste le plus facile à faire de travers.
//!
//! # La duplication est le choix correct, et voici pourquoi
//!
//! `locus_graph::ObjectionTarget` a **la même forme logique** : un tout, une pièce nommée, la règle
//! appliquée, le domaine où elle vaut. La tentation est immédiate — un type partagé, ou un trait
//! générique « ce qui peut être objecté, relu, réfuté, remplacé ».
//!
//! Il ne faut pas. Les deux familles portent sur des **domaines disjoints** : l'une dit qu'un fait
//! du monde est faux, l'autre qu'une décision d'organisation était mal prise. Une conversion, même
//! correcte à l'écriture, ferait circuler une objection organisationnelle dans la machinerie
//! épistémique — où `packages/validation` propage l'invalidation sur les niveaux de §8.1. Une
//! objection au périmètre d'un recâblage n'a rien à propager sur un claim ; l'y faire entrer
//! affaiblirait un résultat scientifique au motif qu'une équipe a été mal composée.
//!
//! Et un trait générique par-dessus les deux **serait la conversion reconstruite** : dès qu'un
//! appelant peut écrire une fonction sur `impl Objectionable`, les deux domaines se traversent à
//! nouveau, sans qu'aucune ligne ne s'appelle « convertir ». C'est pourquoi il n'y a ici aucun
//! trait, et pourquoi la septième frontière vérifiée par la CI interdit qu'un fichier voie les deux
//! familles à la fois.
//!
//! La duplication assumée est dans l'esprit de la double liste de coalescence du worker, gardée
//! redondante exprès pour qu'un test vérifie qu'elle ne se recoupe pas.
//!
//! # Pourquoi quatre cibles et pas une
//!
//! §7.6 donne l'argument dans l'autre domaine : « sur trois arêtes indépendantes, *la règle est
//! fausse* n'a aucun endroit où s'accrocher ». Ici de même. Objecter au **déclencheur**, c'est dire
//! que le fait observé n'a pas eu lieu ; objecter à la **politique**, c'est dire que même si le fait
//! est réel, elle ne justifiait pas cette décision ; objecter au **périmètre**, c'est dire que la
//! politique valait, mais pas sur ces agents-là. Les fondre en une seule « objection à la décision »
//! ferait perdre ce qu'il faut corriger — et une décision de coordination se corrige de trois façons
//! qui n'ont rien à voir.

use std::collections::BTreeSet;
use std::fmt;

use locus_protocol::{Id, id::provisional::Decision as DecisionKind};

/// Ce qu'une objection de coordination peut viser.
///
/// **Famille parallèle à `locus_graph::ObjectionTarget`, jamais convertible en elle.** Même forme
/// logique, domaines disjoints : voir l'en-tête du module, et ADR 0016 décision 9.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ObjectedTo {
    /// La décision dans son ensemble.
    Decision,
    /// Le déclencheur invoqué — un des onze de §14.5, ou un autre que la politique reconnaît.
    ///
    /// Objecter ici, c'est dire que le fait observé n'a pas eu lieu, ou pas comme décrit.
    Trigger {
        /// Lequel.
        trigger: String,
    },
    /// La politique appliquée.
    ///
    /// Objecter ici, c'est dire que même si le déclencheur est réel, la politique ne justifiait pas
    /// cette décision. C'est l'analogue exact de l'objection à la règle de §7.6, dans l'autre
    /// domaine.
    Policy,
    /// Le périmètre sur lequel la décision porte.
    ///
    /// Objecter ici, c'est dire que la politique valait, mais pas sur ces agents-là.
    Perimeter,
}

impl ObjectedTo {
    /// Son nom.
    #[must_use]
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Decision => "decision",
            Self::Trigger { .. } => "trigger",
            Self::Policy => "policy",
            Self::Perimeter => "perimeter",
        }
    }
}

impl fmt::Display for ObjectedTo {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Trigger { trigger } => write!(formatter, "trigger({trigger})"),
            other => formatter.write_str(other.slug()),
        }
    }
}

/// Une décision de coordination, telle qu'on peut la contester.
///
/// Ce n'est pas l'agrégat `Decision` de §7.1 : c'est ce que la décision **offre à l'objection**. La
/// séparation évite qu'un champ ajouté à l'agrégat devienne silencieusement contestable, ou
/// l'inverse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contestable {
    decision: Id<DecisionKind>,
    trigger: String,
    perimeter: BTreeSet<String>,
}

impl Contestable {
    /// Déclarer ce qu'une décision offre à l'objection.
    ///
    /// # Errors
    ///
    /// [`ObjectionError::EmptyField`] pour un déclencheur vide — une décision dont le déclencheur
    /// n'est pas nommé ne se conteste pas sur son déclencheur, et l'objection irait se poser
    /// ailleurs, sur ce qu'elle trouve ; [`ObjectionError::EmptyPerimeter`] pour une décision qui
    /// ne dit sur qui elle porte, ce qui rendrait l'objection de périmètre sans cible.
    pub fn declare(
        decision: Id<DecisionKind>,
        trigger: &str,
        perimeter: &[&str],
    ) -> Result<Self, ObjectionError> {
        if trigger.trim().is_empty() {
            return Err(ObjectionError::EmptyField { field: "trigger" });
        }
        if perimeter.is_empty() {
            return Err(ObjectionError::EmptyPerimeter);
        }
        Ok(Self {
            decision,
            trigger: trigger.to_owned(),
            perimeter: perimeter.iter().map(|who| (*who).to_owned()).collect(),
        })
    }

    /// Son identifiant.
    #[must_use]
    pub const fn decision(&self) -> Id<DecisionKind> {
        self.decision
    }

    /// Le déclencheur invoqué.
    #[must_use]
    pub fn trigger(&self) -> &str {
        &self.trigger
    }

    /// Sur qui la décision porte.
    #[must_use]
    pub const fn perimeter(&self) -> &BTreeSet<String> {
        &self.perimeter
    }

    /// Les cibles que cette décision offre.
    ///
    /// Quatre, et le déclencheur est nommé plutôt que générique : une objection qui dirait « le
    /// déclencheur est faux » sans dire lequel obligerait à retrouver dans le dossier ce qui avait
    /// été invoqué, et personne ne le fait.
    #[must_use]
    pub fn targets(&self) -> Vec<ObjectedTo> {
        vec![
            ObjectedTo::Decision,
            ObjectedTo::Trigger {
                trigger: self.trigger.clone(),
            },
            ObjectedTo::Policy,
            ObjectedTo::Perimeter,
        ]
    }
}

/// Une objection à une décision de coordination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Objection {
    decision: Id<DecisionKind>,
    target: ObjectedTo,
    because: String,
    raised_by: String,
}

impl Objection {
    /// Soulever une objection.
    ///
    /// # Errors
    ///
    /// [`ObjectionError::NoSuchTarget`] quand la cible n'est pas une de celles que la décision
    /// offre — objecter au déclencheur `x` d'une décision déclenchée par `y` viserait quelque chose
    /// qui n'a pas été invoqué, et le dossier porterait une contestation sans objet ;
    /// [`ObjectionError::EmptyField`] pour un motif ou un auteur vide, parce qu'une objection sans
    /// motif ne se répond pas et qu'une objection anonyme ne se discute avec personne.
    pub fn raise(
        contestable: &Contestable,
        target: ObjectedTo,
        because: &str,
        raised_by: &str,
    ) -> Result<Self, ObjectionError> {
        if !contestable.targets().contains(&target) {
            return Err(ObjectionError::NoSuchTarget {
                target: target.to_string(),
            });
        }
        for (field, value) in [("because", because), ("raised_by", raised_by)] {
            if value.trim().is_empty() {
                return Err(ObjectionError::EmptyField { field });
            }
        }
        Ok(Self {
            decision: contestable.decision(),
            target,
            because: because.to_owned(),
            raised_by: raised_by.to_owned(),
        })
    }

    /// La décision visée.
    #[must_use]
    pub const fn decision(&self) -> Id<DecisionKind> {
        self.decision
    }

    /// Ce qu'elle vise.
    #[must_use]
    pub const fn target(&self) -> &ObjectedTo {
        &self.target
    }

    /// Pourquoi.
    #[must_use]
    pub fn because(&self) -> &str {
        &self.because
    }

    /// Qui la soulève.
    #[must_use]
    pub fn raised_by(&self) -> &str {
        &self.raised_by
    }

    /// Ce qu'il faut corriger pour y répondre.
    ///
    /// Trois corrections qui n'ont rien à voir, et c'est la raison d'être des quatre cibles : les
    /// fondre en une seule « objection à la décision » rendrait la réponse indéterminée.
    #[must_use]
    pub const fn remedy(&self) -> Remedy {
        match self.target {
            ObjectedTo::Decision => Remedy::ReopenTheDecision,
            ObjectedTo::Trigger { .. } => Remedy::EstablishTheTrigger,
            ObjectedTo::Policy => Remedy::RevisitThePolicy,
            ObjectedTo::Perimeter => Remedy::NarrowThePerimeter,
        }
    }
}

/// Ce qu'une objection demande de reprendre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Remedy {
    /// Rouvrir la décision entière.
    ReopenTheDecision,
    /// Établir que le déclencheur a bien eu lieu.
    EstablishTheTrigger,
    /// Reprendre la politique, le déclencheur étant admis.
    RevisitThePolicy,
    /// Restreindre le périmètre, la politique étant admise.
    NarrowThePerimeter,
}

impl fmt::Display for Remedy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::ReopenTheDecision => "rouvrir la décision",
            Self::EstablishTheTrigger => "établir que le déclencheur a eu lieu",
            Self::RevisitThePolicy => "reprendre la politique, le déclencheur étant admis",
            Self::NarrowThePerimeter => "restreindre le périmètre, la politique étant admise",
        };
        formatter.write_str(message)
    }
}

/// Ce qui empêche une objection d'exister.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectionError {
    /// Un champ obligatoire vide.
    EmptyField {
        /// Lequel.
        field: &'static str,
    },
    /// Une décision qui ne dit pas sur qui elle porte.
    EmptyPerimeter,
    /// Une cible que cette décision n'offre pas.
    NoSuchTarget {
        /// Laquelle.
        target: String,
    },
}

impl fmt::Display for ObjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(
                formatter,
                "« {field} » est vide : une objection sans motif ne se répond pas, et une objection \
                 anonyme ne se discute avec personne"
            ),
            Self::EmptyPerimeter => formatter.write_str(
                "une décision qui ne dit pas sur qui elle porte rend l'objection de périmètre sans \
                 cible",
            ),
            Self::NoSuchTarget { target } => write!(
                formatter,
                "« {target} » n'est pas une cible de cette décision : le dossier porterait une \
                 contestation sans objet"
            ),
        }
    }
}

impl std::error::Error for ObjectionError {}
