//! Le plan de retrieval — ADR 0022 décision 4.
//!
//! # Ce que le plan ajoute, et ce qu'il ne remplace pas
//!
//! `retrieve` appliquait les dix signaux de §16.3 en une passe : sans intention de requête, sans
//! ordre de canaux, sans critère d'arrêt. Le plan donne les trois, et **ne touche pas aux dix
//! signaux** — un test tient leur nombre et leurs noms.
//!
//! # Deux axes, deux types
//!
//! Un [`Channel`] est une **route qui produit** des candidats ; un [`crate::Signal`] est un facteur
//! qui les **classe**. Les dix signaux de §16.3 mélangent des routes (`GraphTraversal`, `Lexical`,
//! `Vector`, `ExactIdentifiers`), des filtres (`ValidationLevel`, `BranchAndConfidentiality`,
//! `ContextBudget`) et des objectifs (`SourceDiversity`, `NegativeResults`). C'est fidèle à la spec
//! et il ne faut pas y toucher ; mais ajouter les canaux à cette énumération perpétuerait la
//! confusion **et** modifierait une liste normative.
//!
//! # Pourquoi l'intention ordonne
//!
//! Trois questions différentes ne se paient pas au prix de toutes les routes.
//! `arXiv:2603.15658`, atelier ICLR 2026, formalise le retrieval comme un problème de routage entre
//! magasins et montre qu'un routeur oracle atteint une meilleure exactitude avec substantiellement
//! moins de tokens. L'intention est ce routeur, sous forme déclarative — donc lisible, donc
//! contestable, ce qu'un orchestrateur appris qui choisit sans laisser de plan n'est pas.

use std::fmt;

/// Ce qu'une question **cherche** — six, liste close (ADR 0022 décision 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Intent {
    /// Pourquoi X contredit-il H ?
    Explanatory,
    /// Avons-nous déjà essayé ceci ?
    Episodic,
    /// Ce lemme est-il démontré ?
    Formal,
    /// D'où vient cette affirmation ?
    Bibliographic,
    /// Quelles conclusions reposent sur ce type d'argument ?
    Structural,
    /// Que dit l'ensemble du dossier ?
    Global,
}

impl Intent {
    /// Les six, dans l'ordre de l'ADR.
    pub const ALL: [Self; 6] = [
        Self::Explanatory,
        Self::Episodic,
        Self::Formal,
        Self::Bibliographic,
        Self::Structural,
        Self::Global,
    ];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Explanatory => "explanatory",
            Self::Episodic => "episodic",
            Self::Formal => "formal",
            Self::Bibliographic => "bibliographic",
            Self::Structural => "structural",
            Self::Global => "global",
        }
    }

    /// La relire.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|intent| intent.slug() == value)
    }

    /// Les canaux qu'elle interroge, **dans l'ordre**.
    ///
    /// L'ordre est le sujet : c'est lui qui fait qu'une question ne se paie pas au prix de toutes
    /// les routes. Une intention explicative part du graphe, parce que « pourquoi X contredit-il
    /// H » est une question de chemins ; une intention bibliographique part des identifiants
    /// exacts, parce qu'une citation se retrouve par son identité avant de se retrouver par
    /// ressemblance ; une intention formelle ne part **jamais** du vectoriel, ce qui n'est pas une
    /// préférence mais la décision 2 — l'autorité d'un objet formel est un vérificateur.
    #[must_use]
    pub fn channels(self) -> Vec<Channel> {
        match self {
            Self::Explanatory => vec![
                Channel::GraphTraversal,
                Channel::Lexical,
                Channel::Vector,
                Channel::ExactIdentifiers,
            ],
            Self::Episodic => vec![
                Channel::ExactIdentifiers,
                Channel::Lexical,
                Channel::GraphTraversal,
            ],
            Self::Formal => vec![Channel::ExactIdentifiers, Channel::GraphTraversal],
            Self::Bibliographic => {
                vec![Channel::ExactIdentifiers, Channel::Lexical, Channel::Vector]
            }
            Self::Structural => vec![Channel::GraphTraversal, Channel::ExactIdentifiers],
            // Une intention globale ne part **pas** d'un nœud : « que dit l'ensemble du dossier »
            // n'a pas de point de départ dans le graphe, donc elle balaie d'abord largement. C'est
            // ce qui la distingue de l'explicative, qui part d'une contradiction précise. `W17.m`
            // lui ajoutera `Community`, qu'aucune autre intention ne sélectionne.
            Self::Global => vec![
                Channel::Lexical,
                Channel::Vector,
                Channel::GraphTraversal,
                Channel::ExactIdentifiers,
            ],
        }
    }
}

impl fmt::Display for Intent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Une **route qui produit** des candidats.
///
/// Quatre pour l'instant — celles que §16.3 nomme et que le dépôt sait déjà emprunter. `W17.m` en
/// ajoute quatre : `Formal`, `Structural`, `Regional` et `Community`. Elles n'entrent pas ici,
/// faute de ce qui les rendrait exécutables : un oracle de types de prémisses pour `Structural`,
/// des identités de région pour `Regional`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Channel {
    /// Parcours du graphe épistémique.
    GraphTraversal,
    /// Recherche lexicale.
    Lexical,
    /// Similarité vectorielle.
    Vector,
    /// Résolution par identifiant exact.
    ExactIdentifiers,
}

impl Channel {
    /// Les quatre, dans l'ordre de §16.3.
    pub const ALL: [Self; 4] = [
        Self::GraphTraversal,
        Self::Lexical,
        Self::Vector,
        Self::ExactIdentifiers,
    ];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::GraphTraversal => "graph-traversal",
            Self::Lexical => "lexical",
            Self::Vector => "vector",
            Self::ExactIdentifiers => "exact-identifiers",
        }
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Ce qui fait qu'on s'arrête de chercher.
///
/// **Obligatoire** : un plan sans critère d'arrêt cherche jusqu'à épuisement, ce qui est une
/// politique — et une politique qu'on n'a pas écrite est une politique que personne ne peut
/// contester.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// S'arrêter dès que le budget est rempli.
    BudgetFilled,
    /// S'arrêter après ce nombre de canaux, même si le budget reste ouvert.
    ChannelsTried {
        /// Combien.
        after: usize,
    },
}

/// Ce qui a produit les scores, nommé.
///
/// # Pourquoi le plan porte cela
///
/// `Ranking::of` reçoit des `(Signal, f64)` **calculés par l'appelant** : ce crate ne produit aucun
/// score. Un reçu qui n'enregistrerait pas comment les scores ont été produits promettrait un rejeu
/// déterministe sur une entrée qu'il ne connaît pas — rejoué sous une autre fonction de classement,
/// il rendrait d'autres inclusions et un autre condensat. Et le test passerait quand même, une
/// fixture étant déterministe par construction, c'est-à-dire qu'il ne testerait rien.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankingIdentity(String);

impl RankingIdentity {
    /// Une fonction de classement nommée — son identifiant et sa version.
    ///
    /// # Errors
    ///
    /// [`PlanError::UnnamedRanking`] pour un nom vide : « non nommée » et « nommée par la chaîne
    /// vide » se liraient pareil dans un reçu, et l'une des deux est une faute.
    pub fn named(identity: impl Into<String>) -> Result<Self, PlanError> {
        let identity = identity.into();
        if identity.trim().is_empty() {
            return Err(PlanError::UnnamedRanking);
        }
        Ok(Self(identity))
    }

    /// Les scores viennent de l'appelant, qui n'a pas nommé sa fonction.
    ///
    /// **Honnête plutôt que confortable** : c'est l'état réel du dépôt aujourd'hui, et l'écrire
    /// permet à un reçu de dire « le rejeu de ce retrieval n'est pas garanti » au lieu de le
    /// promettre. Une identité absente et une identité qui déclare son absence ne se lisent pas
    /// pareil, exactement comme `None` et `Some(0.0)` pour une couverture.
    #[must_use]
    pub fn caller_supplied() -> Self {
        Self("caller-supplied".to_owned())
    }

    /// Vrai quand un rejeu peut être garanti.
    #[must_use]
    pub fn is_replayable(&self) -> bool {
        self.0 != "caller-supplied"
    }

    /// Son nom.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RankingIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Un élargissement de la recherche, **enregistré**.
///
/// Un type et non un booléen : ce qui rend « ce résultat n'a pas été obtenu sous les mêmes
/// contraintes » lisible sans convention. L'escalade change la nature de la preuve — un résultat
/// trouvé après élargissement du périmètre de branche n'a pas été obtenu sous les mêmes contraintes
/// d'isolation, et §12.4 dépend de cette distinction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Escalation {
    /// Le graphe a été parcouru plus profond.
    DeeperGraph {
        /// La profondeur d'avant.
        from_depth: usize,
        /// Celle qui a été atteinte.
        to_depth: usize,
    },
    /// Le périmètre a été élargi, et quelqu'un l'a autorisé.
    BroaderScope {
        /// Ce qui a été demandé.
        requested: String,
        /// Qui l'a accordé — une escalade de périmètre sans autorité nommée serait un contournement.
        granted_by: String,
    },
    /// Un coprocesseur a été interrogé — ADR 0023.
    Coprocessor {
        /// Quelle capacité, par identité et jamais par nom.
        capability_id: String,
    },
}

/// D'où vient un candidat — directement, ou après une escalade.
///
/// C'est ce qui rend la distinction lisible **par le type**. Une convention — un préfixe de clé, un
/// drapeau — se perdrait à la première sérialisation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Provenance {
    /// Obtenu par les canaux du plan, sans élargissement.
    #[default]
    Direct,
    /// Obtenu après une escalade, laquelle est nommée.
    AfterEscalation(Escalation),
}

impl Provenance {
    /// L'escalade qui l'a produit, s'il y en a une.
    #[must_use]
    pub const fn escalation(&self) -> Option<&Escalation> {
        match self {
            Self::Direct => None,
            Self::AfterEscalation(escalation) => Some(escalation),
        }
    }
}

/// Ce qu'un retrieval fait, déclaré avant de le faire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    intent: Intent,
    channels: Vec<Channel>,
    budget: usize,
    negative_reserve: usize,
    stop: Stop,
    ranking: RankingIdentity,
}

impl Plan {
    /// Écrire un plan.
    ///
    /// L'ordre des canaux vient de l'intention ; le fournir séparément aurait permis de déclarer une
    /// intention et d'en exécuter une autre, ce qui est exactement ce que le plan existe pour
    /// empêcher.
    ///
    /// # Errors
    ///
    /// [`PlanError::EmptyBudget`] pour un budget nul — un retrieval qui ne peut rien inclure n'est
    /// pas un retrieval ; [`PlanError::ReserveExceedsBudget`] quand la réserve de négatifs dépasse
    /// le budget, ce qui n'exclurait plus « ailleurs d'abord » mais partout.
    pub fn new(
        intent: Intent,
        budget: usize,
        negative_reserve: usize,
        stop: Stop,
        ranking: RankingIdentity,
    ) -> Result<Self, PlanError> {
        if budget == 0 {
            return Err(PlanError::EmptyBudget);
        }
        if negative_reserve > budget {
            return Err(PlanError::ReserveExceedsBudget {
                reserve: negative_reserve,
                budget,
            });
        }
        Ok(Self {
            intent,
            channels: intent.channels(),
            budget,
            negative_reserve,
            stop,
            ranking,
        })
    }

    /// Le plan qui reproduit le comportement d'avant `W17.l`, à ce budget.
    ///
    /// # Ce que « reproduire » veut dire, et pourquoi c'est un test
    ///
    /// `retrieve` filtrait par habilitation, triait par score décroissant puis par clé croissante,
    /// et coupait au budget. Aucune réserve de négatifs — `is_negative` n'était **jamais lu** dans
    /// ce chemin. Ce plan déclare donc une réserve **nulle**, et c'est ce qui rend l'item additif
    /// plutôt qu'un changement de comportement déguisé.
    ///
    /// L'identité de classement est `caller_supplied`, parce que c'est la vérité : les scores
    /// viennent de l'appelant et personne ne les a nommés.
    ///
    /// # Errors
    ///
    /// [`PlanError::EmptyBudget`] pour un budget nul.
    pub fn compatible(budget: usize) -> Result<Self, PlanError> {
        Self::new(
            Intent::Global,
            budget,
            0,
            Stop::BudgetFilled,
            RankingIdentity::caller_supplied(),
        )
    }

    /// Ce que la question cherche.
    #[must_use]
    pub const fn intent(&self) -> Intent {
        self.intent
    }

    /// Les canaux, **dans l'ordre**.
    #[must_use]
    pub fn channels(&self) -> &[Channel] {
        &self.channels
    }

    /// Combien de résultats au plus.
    #[must_use]
    pub const fn budget(&self) -> usize {
        self.budget
    }

    /// Combien de places sont réservées aux résultats négatifs.
    ///
    /// **Zéro par défaut**, et le reçu l'écrit quand même : une garantie absente et une garantie
    /// nulle ne se lisent pas pareil. Sa valeur vient de la `negative_result_policy` de §16.2 quand
    /// une politique est attachée.
    #[must_use]
    pub const fn negative_reserve(&self) -> usize {
        self.negative_reserve
    }

    /// Quand s'arrêter.
    #[must_use]
    pub const fn stop(&self) -> Stop {
        self.stop
    }

    /// Ce qui a produit les scores.
    #[must_use]
    pub const fn ranking(&self) -> &RankingIdentity {
        &self.ranking
    }
}

/// Pourquoi un plan ne s'écrit pas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    /// Un budget nul.
    EmptyBudget,
    /// Une réserve plus large que le budget.
    ReserveExceedsBudget {
        /// La réserve demandée.
        reserve: usize,
        /// Le budget.
        budget: usize,
    },
    /// Une fonction de classement sans nom.
    UnnamedRanking,
}

impl fmt::Display for PlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyBudget => formatter.write_str(
                "un budget nul : un retrieval qui ne peut rien inclure n'est pas un retrieval",
            ),
            Self::ReserveExceedsBudget { reserve, budget } => write!(
                formatter,
                "une réserve de {reserve} pour un budget de {budget} : la réserve exclut « ailleurs \
                 d'abord », et au-delà du budget elle exclurait partout"
            ),
            Self::UnnamedRanking => formatter.write_str(
                "une fonction de classement sans nom : « non nommée » et « nommée par la chaîne \
                 vide » se liraient pareil dans un reçu, et l'une des deux est une faute",
            ),
        }
    }
}

impl std::error::Error for PlanError {}
