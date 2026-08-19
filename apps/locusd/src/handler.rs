//! Le handler transactionnel comme **port** — `CLAUDE.md`, « toute mutation passe par un command
//! handler transactionnel », et `SPEC_V1.md` §22.5.
//!
//! # Ce qui rend la règle opposable, plutôt que recommandée
//!
//! Une règle écrite dans un document se contourne par distraction. Celle-ci tient par la **forme de
//! la signature** : [`Decide::decide`] reçoit l'état et rend des brouillons d'événements, et ne
//! reçoit **jamais** le journal. Un décideur n'a donc rien en main qui sache écrire — non parce
//! qu'on le lui interdit, mais parce qu'on ne le lui donne pas.
//!
//! C'est plus fort qu'une convention et plus fort qu'un `grep` : on peut oublier une convention, et
//! un `grep` ne voit pas ce qui passe par un alias. Ici il n'existe pas d'expression, dans le corps
//! d'un `decide`, qui atteigne le journal — il faudrait que l'auteur du décideur s'en procure un
//! lui-même, ce qui est un autre geste, visible dans le diff.
//!
//! Le `grep` existe quand même, dans le test de sortie : deux vérifications indépendantes valent
//! mieux qu'une, et celle-ci attrape le cas que le type ne couvre pas — un module de `locusd` qui
//! écrirait sans être un décideur du tout.
//!
//! # Ce que ce module ne décide pas
//!
//! Aucune commande de §22.3 n'est implémentée ici. Le port est ce que `W20.b` demande ; les
//! quarante commandes viendront avec leurs agrégats, et chacune sera un `impl Decide`.

use std::collections::BTreeMap;
use std::fmt;

use locus_event_store::{Draft as EventDraft, Expected};
use locus_protocol::Id;
use locus_protocol::id::{Agent, Workspace};

use crate::command::CommandEnvelope;
use crate::error::{CommandError, Revision};

/// Ce qu'un handler sait faire : **décider**, jamais écrire.
///
/// # La signature est la garantie
///
/// `decide` reçoit `&Self::State` et rend `Vec<EventDraft>`. Aucun paramètre n'est un journal,
/// aucune méthode du trait n'en prend, et le trait n'a pas de méthode par défaut qui en fabrique
/// un. Un décideur qui voudrait écrire devrait se procurer un journal par ses propres moyens —
/// c'est-à-dire ajouter une dépendance et un champ, ce qui se voit.
///
/// # Pourquoi `Vec` et non un flux
///
/// Un lot est écrit d'un bloc, par un seul `append` : c'est ce qui rend l'échec sans trace. Un flux
/// laisserait croire qu'on peut émettre au fil de l'eau, donc qu'un échec à mi-parcours est
/// concevable — et c'est exactement ce que §9.2 interdit.
pub trait Decide {
    /// L'état sur lequel la décision se prend.
    type State;

    /// Décider, à partir de l'état, ce que la commande produit.
    ///
    /// # Errors
    ///
    /// Le refus porte sa famille de §22.5. Un refus **n'écrit rien** : c'est la transaction qui
    /// écrit, et elle n'écrit que ce qu'un `Ok` lui rend.
    fn decide(
        &self,
        command: &CommandEnvelope,
        state: &Self::State,
    ) -> Result<Vec<EventDraft>, CommandError>;
}

/// La portée d'une clé d'idempotence — §22.5, « les idempotency keys sont **scoped** ».
///
/// # Pourquoi elle se dérive de l'enveloppe, et ne se passe pas à côté
///
/// Deux clients qui choisissent `retry-1` ne se concertent pas. Si la clé était globale, la
/// resoumission de l'un rendrait à l'autre le résultat d'une commande qu'il n'a jamais émise — un
/// succès pour une commande jamais exécutée, ce qui est pire qu'une erreur.
///
/// La portée est donc `(workspace, principal)`, **lue sur l'enveloppe**. La passer en paramètre
/// séparé ouvrirait la possibilité de la passer fausse, et une portée fausse est indétectable :
/// elle ne produit ni erreur ni conflit, seulement deux commandes qui se confondent.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IdempotencyScope {
    workspace: Id<Workspace>,
    principal: Id<Agent>,
}

impl IdempotencyScope {
    /// La portée de cette commande, telle que l'enveloppe la porte.
    #[must_use]
    pub fn of(command: &CommandEnvelope) -> Self {
        Self {
            workspace: *command.workspace_id(),
            principal: *command.actor_principal_id(),
        }
    }

    /// Le workspace de la portée.
    #[must_use]
    pub const fn workspace(&self) -> &Id<Workspace> {
        &self.workspace
    }

    /// Le principal de la portée.
    #[must_use]
    pub const fn principal(&self) -> &Id<Agent> {
        &self.principal
    }
}

impl fmt::Display for IdempotencyScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}", self.workspace, self.principal)
    }
}

/// Ce qui identifie une soumission : sa portée **et** sa clé.
///
/// Le type existe pour que la paire ne se dissocie pas. Une `String` nue comme clé de registre
/// aurait laissé écrire `registre.get(&key)`, qui compile et qui est faux.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Submission {
    scope: IdempotencyScope,
    key: String,
}

impl Submission {
    /// La soumission que cette enveloppe représente.
    #[must_use]
    pub fn of(command: &CommandEnvelope) -> Self {
        Self {
            scope: IdempotencyScope::of(command),
            key: command.idempotency_key().to_owned(),
        }
    }

    /// Sa portée.
    #[must_use]
    pub const fn scope(&self) -> &IdempotencyScope {
        &self.scope
    }

    /// Sa clé, telle que le client l'a choisie.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }
}

/// Le registre des soumissions déjà vues, par portée et par clé.
///
/// `BTreeMap` et non `HashMap` : l'ordre d'itération est stable, ce qui rend un diagnostic
/// reproductible. Le registre ne connaît pas l'expiration — §22.5 dit que les clés « expirent selon
/// la catégorie », et la catégorie n'existe pas encore ; l'inventer ici serait du vocabulaire
/// parallèle, ce que `CLAUDE.md` interdit.
#[derive(Debug, Clone, Default)]
pub(crate) struct Ledger {
    seen: BTreeMap<Submission, Revision>,
}

impl Ledger {
    pub(crate) fn recall(&self, submission: &Submission) -> Option<Revision> {
        self.seen.get(submission).copied()
    }

    pub(crate) fn remember(&mut self, submission: Submission, revision: Revision) {
        self.seen.insert(submission, revision);
    }
}

/// Un lot de commandes, et **la déclaration de ce qu'il garantit**.
///
/// # §22.5 : « les batch commands sont atomiques uniquement si explicitement déclarées »
///
/// La phrase se lit comme une permission ; c'est une **interdiction du défaut**. Un `Vec` de
/// commandes soumis tel quel serait atomique ou non selon ce que l'implémentation se trouve faire,
/// et l'appelant n'aurait aucun moyen de savoir lequel. Il n'existe donc pas de constructeur qui
/// prenne un `Vec` sans dire lequel des deux il est : c'est le choix qui est obligatoire, pas
/// l'atomicité.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Batch {
    /// Tout ou rien. Les commandes visent **un seul** stream — sans quoi l'atomicité serait une
    /// promesse que le journal ne peut pas tenir, et une promesse invérifiable est un mensonge.
    Atomic(Vec<CommandEnvelope>),
    /// Une par une, dans l'ordre. Un refus arrête le lot ; ce qui précède reste écrit, et c'est
    /// précisément ce que « non atomique » veut dire.
    Sequential(Vec<CommandEnvelope>),
}

impl Batch {
    /// Les commandes du lot, quelle que soit sa nature.
    #[must_use]
    pub fn commands(&self) -> &[CommandEnvelope] {
        match self {
            Self::Atomic(commands) | Self::Sequential(commands) => commands,
        }
    }

    /// Vrai si le lot s'est déclaré atomique.
    #[must_use]
    pub const fn is_atomic(&self) -> bool {
        matches!(self, Self::Atomic(_))
    }
}

/// Ce sur quoi la commande croit écrire, traduit pour le journal.
///
/// [`Revision::INITIAL`] — zéro — veut dire « ce stream n'existe pas encore », et non « le stream
/// est à la révision 0 » : un stream naît de son premier événement, et sa première révision est 1.
/// La traduction est ici, en un seul endroit, parce qu'un décalage d'un rang entre le client et le
/// journal produirait des conflits que personne ne saurait expliquer.
pub(crate) const fn expected_from(revision: Revision) -> Expected {
    if revision.get() == 0 {
        Expected::NoStream
    } else {
        Expected::Exact(revision.get())
    }
}
