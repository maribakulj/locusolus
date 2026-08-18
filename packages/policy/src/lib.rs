//! Le moteur de politique — `docs/SPEC_V1.md` §20.
//!
//! # Ce que §20.2 exige, et ce que chaque exigence empêche
//!
//! « Le moteur DOIT : employer une DSL déclarative versionnée ; **séparer faits d'entrée et
//! décision** ; produire une **trace d'évaluation** ; supporter `allow`, `deny`, `modify`,
//! `require_approval`, `require_tasks` ; **détecter les conflits** de politiques ; définir une
//! **priorité explicite** ; supporter dry-run et simulation ; être **déterministe à entrées
//! identiques** ; conserver les overrides humains. »
//!
//! Trois de ces exigences se tiennent l'une l'autre, et ce module les traite comme une seule chose.
//!
//! **Faits séparés de la décision.** Les faits entrent, la décision sort, et rien dans l'évaluation
//! ne consulte autre chose. Un moteur qui lirait l'heure, un compteur global ou le résultat d'une
//! évaluation précédente cesserait d'être déterministe sans qu'aucune règle ait changé — et on
//! chercherait le désaccord dans les règles.
//!
//! **Déterminisme à entrées identiques.** C'est ce que la séparation rend vrai, et c'est ce qui rend
//! une décision contestable : sans lui, rejouer une décision pour la comprendre en produit une
//! autre.
//!
//! **Trace d'évaluation.** Une décision sans trace n'est pas une décision, c'est un verdict. §20.5
//! demande qu'on expose « les règles déclenchées » ; la trace est ce qui le permet, et elle est
//! produite par le même passage que la décision, jamais reconstituée après coup — une trace
//! reconstituée raconte ce qu'on croit que le moteur a fait.
//!
//! # La priorité est déclarée, jamais héritée de l'ordre
//!
//! §20.2 demande une « priorité explicite ». Deux règles qui se contredisent doivent être tranchées
//! par un chiffre que quelqu'un a écrit, pas par leur position dans un fichier : sinon réordonner
//! un fichier de politiques change les décisions, et personne ne relit un diff de réordonnancement
//! comme un changement de comportement.
//!
//! À priorité égale, il n'y a pas de gagnant. Le conflit est **rendu**, pas résolu : un moteur qui
//! choisirait tout de même déciderait à la place de qui a écrit les règles.

use std::collections::BTreeSet;
use std::fmt;

/// Les cinq verbes de §20.2.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verb {
    /// Autoriser.
    Allow,
    /// Refuser.
    Deny,
    /// Autoriser en modifiant la demande.
    Modify {
        /// Ce qui est imposé.
        constraint: String,
    },
    /// Exiger une approbation.
    RequireApproval {
        /// De qui.
        approver_role: String,
    },
    /// Exiger que des tâches soient menées d'abord.
    RequireTasks {
        /// Lesquelles.
        tasks: Vec<String>,
    },
}

impl Verb {
    /// Son nom.
    #[must_use]
    pub const fn slug(&self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
            Self::Modify { .. } => "modify",
            Self::RequireApproval { .. } => "require_approval",
            Self::RequireTasks { .. } => "require_tasks",
        }
    }

    /// Vrai quand ce verbe laisse l'action se produire telle quelle.
    ///
    /// Seul `allow` le fait. `modify` laisse passer **autre chose** que ce qui a été demandé, et les
    /// confondre ferait croire qu'une contrainte imposée est une permission simple.
    #[must_use]
    pub const fn permits_as_requested(&self) -> bool {
        matches!(self, Self::Allow)
    }
}

impl fmt::Display for Verb {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Les faits d'entrée — tout ce que l'évaluation a le droit de connaître.
///
/// Ce type est la frontière que §20.2 demande. Ce qui n'est pas ici n'entre pas dans la décision :
/// pas l'heure, pas un compteur, pas le résultat de la fois d'avant.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facts {
    entries: BTreeSet<(String, String)>,
}

impl Facts {
    /// Aucun fait.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Poser un fait.
    #[must_use]
    pub fn with(mut self, key: &str, value: &str) -> Self {
        self.entries.insert((key.to_owned(), value.to_owned()));
        self
    }

    /// Vrai quand ce fait est posé.
    #[must_use]
    pub fn holds(&self, key: &str, value: &str) -> bool {
        self.entries.contains(&(key.to_owned(), value.to_owned()))
    }

    /// Les faits, dans un ordre canonique.
    #[must_use]
    pub fn entries(&self) -> Vec<(&str, &str)> {
        self.entries
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect()
    }
}

/// Une règle de politique.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    id: String,
    version: u32,
    priority: u32,
    when: Vec<(String, String)>,
    verb: Verb,
}

impl Rule {
    /// Déclarer une règle.
    ///
    /// `priority` est explicite parce que §20.2 l'exige : trancher par l'ordre de déclaration ferait
    /// d'un réordonnancement de fichier un changement de comportement que personne ne relit comme
    /// tel.
    ///
    /// # Errors
    ///
    /// [`PolicyError::EmptyField`] pour un identifiant vide ou une condition vide. Une règle sans
    /// condition s'applique à tout, ce qui n'est presque jamais voulu et ne se voit pas.
    pub fn declare(
        id: &str,
        version: u32,
        priority: u32,
        when: &[(&str, &str)],
        verb: Verb,
    ) -> Result<Self, PolicyError> {
        if id.trim().is_empty() {
            return Err(PolicyError::EmptyField { field: "rule.id" });
        }
        if when.is_empty() {
            return Err(PolicyError::EmptyField { field: "rule.when" });
        }
        Ok(Self {
            id: id.to_owned(),
            version,
            priority,
            when: when
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
            verb,
        })
    }

    /// Son identifiant.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Sa version — §20.2 : « DSL déclarative **versionnée** ».
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Sa priorité déclarée.
    #[must_use]
    pub const fn priority(&self) -> u32 {
        self.priority
    }

    /// Ce qu'elle décide.
    #[must_use]
    pub const fn verb(&self) -> &Verb {
        &self.verb
    }

    /// Vrai quand tous ses `when` sont posés dans `facts`.
    #[must_use]
    pub fn matches(&self, facts: &Facts) -> bool {
        self.when.iter().all(|(key, value)| facts.holds(key, value))
    }
}

/// Un jeu de politiques.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Policy {
    rules: Vec<Rule>,
}

impl Policy {
    /// Aucune règle.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Ajouter une règle.
    ///
    /// # Errors
    ///
    /// [`PolicyError::DuplicateRule`] quand l'identifiant est déjà pris : deux règles du même nom
    /// rendraient une trace ambiguë, et c'est la trace qui sert à contester.
    pub fn with(mut self, rule: Rule) -> Result<Self, PolicyError> {
        if self.rules.iter().any(|known| known.id == rule.id) {
            return Err(PolicyError::DuplicateRule { id: rule.id });
        }
        self.rules.push(rule);
        Ok(self)
    }

    /// Évaluer `facts`.
    ///
    /// Rend toujours une [`Evaluation`] : la décision **et** la trace, produites par le même
    /// passage. Reconstituer une trace après coup raconterait ce qu'on croit que le moteur a fait.
    ///
    /// Ne lit rien d'autre que `facts` — c'est ce qui rend le résultat déterministe, donc
    /// contestable.
    #[must_use]
    pub fn evaluate(&self, facts: &Facts) -> Evaluation {
        let mut fired: Vec<&Rule> = self
            .rules
            .iter()
            .filter(|rule| rule.matches(facts))
            .collect();
        // Tri par priorité décroissante, puis par identifiant : deux règles de même priorité
        // doivent apparaître dans la trace dans un ordre stable, sans que cet ordre décide quoi que
        // ce soit — c'est le conflit ci-dessous qui tranche, ou refuse de trancher.
        fired.sort_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| left.id.cmp(&right.id))
        });

        let trace: Vec<Fired> = fired
            .iter()
            .map(|rule| Fired {
                rule: rule.id.clone(),
                version: rule.version,
                priority: rule.priority,
                verb: rule.verb.clone(),
            })
            .collect();

        let Some(first) = fired.first() else {
            return Evaluation {
                outcome: Outcome::NoRule,
                trace,
            };
        };

        let contenders: Vec<&&Rule> = fired
            .iter()
            .filter(|rule| rule.priority == first.priority)
            .collect();
        let disagree = contenders.iter().any(|rule| rule.verb != first.verb);

        if disagree {
            return Evaluation {
                outcome: Outcome::Conflict {
                    priority: first.priority,
                    rules: contenders.iter().map(|rule| rule.id.clone()).collect(),
                },
                trace,
            };
        }

        Evaluation {
            outcome: Outcome::Decided {
                verb: first.verb.clone(),
                by: first.id.clone(),
            },
            trace,
        }
    }
}

/// Une règle qui s'est déclenchée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fired {
    /// Laquelle.
    pub rule: String,
    /// Dans quelle version — §20.5 demande « politique et version ».
    pub version: u32,
    /// À quelle priorité.
    pub priority: u32,
    /// Ce qu'elle décidait.
    pub verb: Verb,
}

/// Ce qu'une évaluation conclut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Une décision, et la règle qui la porte.
    Decided {
        /// Le verbe.
        verb: Verb,
        /// Quelle règle a tranché.
        by: String,
    },
    /// Deux règles de même priorité qui ne disent pas la même chose.
    ///
    /// Le conflit est **rendu**, pas résolu. Un moteur qui choisirait tout de même déciderait à la
    /// place de qui a écrit les règles, et le ferait en silence.
    Conflict {
        /// À quelle priorité.
        priority: u32,
        /// Lesquelles.
        rules: Vec<String>,
    },
    /// Aucune règle ne s'applique.
    ///
    /// Distinct d'`allow` : personne n'a autorisé quoi que ce soit. C'est à l'appelant de décider ce
    /// qu'il fait d'un silence, et le lui dire est le seul moyen qu'il ait le choix.
    NoRule,
}

/// Une décision et la trace qui l'explique.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evaluation {
    outcome: Outcome,
    trace: Vec<Fired>,
}

impl Evaluation {
    /// Ce qui a été décidé.
    #[must_use]
    pub const fn outcome(&self) -> &Outcome {
        &self.outcome
    }

    /// Les règles déclenchées, de la plus prioritaire à la moins.
    ///
    /// §20.5 : « règles déclenchées ». La trace porte **toutes** celles qui ont matché, pas
    /// seulement celle qui a tranché : savoir ce qui a failli s'appliquer est la moitié de ce qui
    /// rend une décision contestable.
    #[must_use]
    pub fn trace(&self) -> &[Fired] {
        &self.trace
    }
}

/// Ce qui empêche une politique d'exister.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyError {
    /// Un champ obligatoire vide.
    EmptyField {
        /// Lequel.
        field: &'static str,
    },
    /// Deux règles du même identifiant.
    DuplicateRule {
        /// Lequel.
        id: String,
    },
}

impl fmt::Display for PolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(
                formatter,
                "« {field} » est vide : une règle sans condition s'applique à tout, ce qui n'est \
                 presque jamais voulu et ne se voit pas"
            ),
            Self::DuplicateRule { id } => write!(
                formatter,
                "« {id} » est déjà déclarée : deux règles du même nom rendraient la trace \
                 ambiguë, et c'est la trace qui sert à contester"
            ),
        }
    }
}

impl std::error::Error for PolicyError {}
