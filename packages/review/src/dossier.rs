//! Le `ReviewDossier` — `docs/SPEC_V1.md` §17.3.
//!
//! # Ce que « figé » veut dire, et pourquoi c'est un type
//!
//! §17.3 : « le dossier est figé **avant attribution**. Toute modification entraîne une nouvelle
//! version ou un addendum explicitement visible. »
//!
//! Un dossier qu'on pourrait retoucher après attribution rendrait toute revue incontestable : on
//! ne saurait jamais si le relecteur a vu ce que le dossier dit aujourd'hui. La phrase est donc
//! portée par une **suite de types** — `Draft` puis `Frozen` — plutôt que par un booléen : un
//! `Frozen` n'a aucune méthode qui modifie ce que le relecteur consultera, et ce n'est pas une
//! discipline à tenir mais ce que le compilateur permet.
//!
//! C'est la forme de la chaîne de build de W5.b, employée ici pour la même raison : il s'agit d'un
//! **processus** qui se déroule une fois, dans un ordre qui est la garantie.

use std::collections::BTreeSet;
use std::fmt;

use locus_domain::{ContentHash, RevisionId};

/// Ce qu'un relecteur n'a pas le droit de voir.
///
/// §17.1 exige que la revue rende explicite « ce qui a été exclu ». Une exclusion qui ne serait
/// pas nommée serait indistinguable d'un oubli, et c'est exactement la différence entre un
/// aveuglement méthodique et un dossier incomplet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Blindness {
    /// Le relecteur ne voit pas le raisonnement du générateur.
    ///
    /// Invariant 11 : « les reviewers indépendants ne reçoivent pas le raisonnement privé ou le
    /// contexte non autorisé du générateur ».
    GeneratorTranscript,
    /// Le relecteur ne voit pas l'identité de l'auteur.
    AuthorIdentity,
    /// Le relecteur ne voit pas les revues déjà rendues.
    OtherReviews,
}

impl Blindness {
    /// Les trois, dans l'ordre où elles se décident.
    pub const ALL: [Self; 3] = [
        Self::GeneratorTranscript,
        Self::AuthorIdentity,
        Self::OtherReviews,
    ];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::GeneratorTranscript => "generator_transcript",
            Self::AuthorIdentity => "author_identity",
            Self::OtherReviews => "other_reviews",
        }
    }
}

impl fmt::Display for Blindness {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Ce que la politique d'indépendance exige — §14.4.
///
/// La liste de §14.4 en compte dix ; trois entrent ici, celles dont un consommateur exécutable
/// existe : le groupe d'indépendance vient de `packages/coordination` (W13.c), le worker distinct
/// de l'assignation (W13.d), et l'absence de transcript de l'aveuglement ci-dessus. Les sept
/// autres — familles de modèles, fournisseurs, corpus, outils, randomisation, anonymisation,
/// mémoire partagée — n'ont encore rien qui les vérifie, et les écrire en ferait du vocabulaire
/// inerte (ADR 0016, décision 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IndependenceRequirement {
    /// Le relecteur n'appartient pas au groupe d'indépendance du générateur.
    DistinctIndependenceGroup,
    /// Le relecteur tourne sur un worker distinct.
    DistinctWorker,
    /// Le relecteur n'a pas reçu le transcript de génération.
    NoGeneratorTranscript,
}

impl IndependenceRequirement {
    /// Les trois qui ont un consommateur.
    pub const ALL: [Self; 3] = [
        Self::DistinctIndependenceGroup,
        Self::DistinctWorker,
        Self::NoGeneratorTranscript,
    ];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::DistinctIndependenceGroup => "distinct_independence_group",
            Self::DistinctWorker => "distinct_worker",
            Self::NoGeneratorTranscript => "no_generator_transcript",
        }
    }
}

impl fmt::Display for IndependenceRequirement {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Un dossier en cours de constitution.
///
/// Il se remplit, il ne s'attribue pas. Le seul chemin vers l'attribution passe par
/// [`Draft::freeze`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    id: String,
    targets: Vec<RevisionId>,
    questions: Vec<String>,
    excluded: Vec<String>,
    blindness: BTreeSet<Blindness>,
    independence: BTreeSet<IndependenceRequirement>,
}

impl Draft {
    /// Ouvrir un dossier.
    ///
    /// # Errors
    ///
    /// [`DossierError::EmptyId`] pour un identifiant vide, [`DossierError::NoTarget`] pour un
    /// dossier qui ne vise aucune révision — relire « en général » n'est pas relire.
    pub fn open(id: &str, targets: Vec<RevisionId>) -> Result<Self, DossierError> {
        if id.trim().is_empty() {
            return Err(DossierError::EmptyId);
        }
        if targets.is_empty() {
            return Err(DossierError::NoTarget);
        }
        Ok(Self {
            id: id.to_owned(),
            targets,
            questions: Vec::new(),
            excluded: Vec::new(),
            blindness: BTreeSet::new(),
            independence: BTreeSet::new(),
        })
    }

    /// Poser une question de revue.
    #[must_use]
    pub fn asking(mut self, question: &str) -> Self {
        self.questions.push(question.to_owned());
        self
    }

    /// Nommer ce qui est exclu du dossier, et pourquoi.
    #[must_use]
    pub fn excluding(mut self, what: &str) -> Self {
        self.excluded.push(what.to_owned());
        self
    }

    /// Décider d'un aveuglement.
    #[must_use]
    pub fn blind_to(mut self, blindness: Blindness) -> Self {
        self.blindness.insert(blindness);
        self
    }

    /// Exiger une condition d'indépendance.
    #[must_use]
    pub fn requiring(mut self, requirement: IndependenceRequirement) -> Self {
        self.independence.insert(requirement);
        self
    }

    /// Figer le dossier.
    ///
    /// Le hash est **fourni**, pas calculé : choisir une implémentation de hash est une décision
    /// d'infrastructure, et ce paquet ne la prend pas — même règle qu'en W6.c pour le store.
    ///
    /// # Errors
    ///
    /// [`DossierError::NoQuestion`] : §17.1 exige que la revue rende explicites « les questions
    /// posées ». Un dossier sans question laisse le relecteur décider seul de ce qu'il examine, ce
    /// qui rend sa couverture inopposable.
    pub fn freeze(self, content_hash: ContentHash) -> Result<Frozen, DossierError> {
        if self.questions.is_empty() {
            return Err(DossierError::NoQuestion);
        }
        Ok(Frozen {
            draft: self,
            content_hash,
            addenda: Vec::new(),
        })
    }
}

/// Un dossier figé — celui qu'un relecteur reçoit.
///
/// # Ce qu'on ne peut pas en faire
///
/// Le modifier. Il n'existe aucune méthode qui change ses cibles, ses questions, ses exclusions,
/// son aveuglement ou ses exigences d'indépendance. La seule évolution possible est
/// [`Frozen::with_addendum`], qui **ajoute** une note visible sans toucher au reste, et
/// [`Frozen::revise`], qui rend un nouveau brouillon — donc un nouveau dossier, à re-figer et à
/// ré-attribuer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frozen {
    draft: Draft,
    content_hash: ContentHash,
    addenda: Vec<String>,
}

impl Frozen {
    /// Ajouter un addendum **visible**.
    ///
    /// §17.3 : « toute modification entraîne une nouvelle version ou un addendum explicitement
    /// visible ». Un addendum ne réécrit rien : il s'ajoute, et le hash du contenu figé ne bouge
    /// pas — c'est ce qui permet de dire, après coup, ce que le relecteur avait sous les yeux au
    /// moment de l'attribution et ce qui est arrivé ensuite.
    #[must_use]
    pub fn with_addendum(mut self, note: &str) -> Self {
        self.addenda.push(note.to_owned());
        self
    }

    /// Repartir d'un brouillon pour produire une **nouvelle version**.
    ///
    /// L'autre issue de §17.3. Le dossier figé reste ce qu'il est ; celui-ci en est un autre, qui
    /// devra être figé et attribué à son tour.
    #[must_use]
    pub fn revise(&self) -> Draft {
        self.draft.clone()
    }

    /// Son identifiant.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.draft.id
    }

    /// Les révisions visées.
    #[must_use]
    pub fn targets(&self) -> &[RevisionId] {
        &self.draft.targets
    }

    /// Les questions posées.
    #[must_use]
    pub fn questions(&self) -> &[String] {
        &self.draft.questions
    }

    /// Ce qui a été exclu, nommé.
    #[must_use]
    pub fn excluded(&self) -> &[String] {
        &self.draft.excluded
    }

    /// Les aveuglements décidés.
    #[must_use]
    pub const fn blindness(&self) -> &BTreeSet<Blindness> {
        &self.draft.blindness
    }

    /// Les exigences d'indépendance.
    #[must_use]
    pub const fn independence(&self) -> &BTreeSet<IndependenceRequirement> {
        &self.draft.independence
    }

    /// Le hash du contenu figé.
    #[must_use]
    pub const fn content_hash(&self) -> &ContentHash {
        &self.content_hash
    }

    /// Les addenda, dans l'ordre.
    #[must_use]
    pub fn addenda(&self) -> &[String] {
        &self.addenda
    }
}

/// Ce qui empêche un dossier d'exister ou d'être figé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DossierError {
    /// Un identifiant vide.
    EmptyId,
    /// Aucune révision visée.
    NoTarget,
    /// Aucune question posée.
    NoQuestion,
}

impl fmt::Display for DossierError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::EmptyId => "un dossier sans identifiant ne s'attribue pas",
            Self::NoTarget => "relire « en général » n'est pas relire",
            Self::NoQuestion => {
                "sans question, le relecteur décide seul de ce qu'il examine et sa couverture \
                 devient inopposable"
            }
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for DossierError {}
