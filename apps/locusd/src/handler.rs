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

use locus_event_store::{Draft as EventDraft, Envelope, Expected};
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
    /// Reconstruire le registre depuis le journal — `W20.j`, ADR 0029 décision 4.
    ///
    /// # Ce que cette fonction répare
    ///
    /// Le registre vivait en mémoire vive, et un redémarrage l'oubliait — or un redémarrage est
    /// précisément ce qui coupe les connexions et déclenche les retentes. La garantie de §22.5 était
    /// donc fausse **au moment exact où elle sert**. C'est une promesse au sens de l'ADR 0022
    /// décision 0 : un mécanisme qui annonce un effet qui n'a pas toujours lieu.
    ///
    /// # Le rang retenu est le **plus grand**, et ce n'est pas un détail
    ///
    /// Une commande peut écrire plusieurs événements en une fois, et ce que la transaction a rendu
    /// au client est le rang du stream **après** l'écriture entière — donc celui du dernier
    /// événement. Retenir le premier rendrait au client, à la retente, un rang antérieur à ce qu'il
    /// avait reçu ; il le passerait en `expected_revision` et son écriture suivante serait refusée
    /// pour conflit, sur un journal parfaitement sain.
    ///
    /// # Un événement sans clé n'entre pas
    ///
    /// Ceux d'avant la migration n'en portent pas, et `None` n'est pas `""` : une commande dont la
    /// clé est inconnue n'est pas une commande dont la clé est vide. Les ranger sous une clé vide
    /// les ferait tous se confondre, et la première retente d'un client recevrait le rang d'une
    /// commande étrangère.
    pub(crate) fn rebuild<'a>(events: impl IntoIterator<Item = &'a Envelope>) -> Self {
        let mut ledger = Self::default();
        for event in events {
            let Some(key) = event.idempotency_key.as_deref() else {
                continue;
            };
            let submission = Submission {
                scope: IdempotencyScope {
                    workspace: event.workspace_id,
                    principal: event.actor.principal_id,
                },
                key: key.to_owned(),
            };
            let revision = Revision::new(event.stream_revision);
            let retenu = ledger
                .recall(&submission)
                .map_or(revision, |connu| connu.max(revision));
            ledger.remember(submission, retenu);
        }
        ledger
    }

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

#[cfg(test)]
mod tests {
    use locus_event_store::{ActorKind, EventType};
    use locus_protocol::Timestamp;
    use locus_protocol::id::{Command, Event, Project};

    use super::{Envelope, IdempotencyScope, Ledger, Submission};
    use crate::error::Revision;
    use locus_event_store::Actor;
    use locus_protocol::id::{Agent, Workspace};
    use locus_protocol::{Id, IdKind};

    fn id<K: IdKind>(seed: u8) -> Id<K> {
        let mut entropy = [0_u8; 10];
        entropy[9] = seed;
        Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
            .expect("l'instant de fixture tient sur 48 bits")
    }

    /// Un événement au journal, avec ou sans clé d'idempotence.
    fn evenement(revision: u64, principal: u8, key: Option<&str>) -> Envelope {
        Envelope {
            event_id: id::<Event>(u8::try_from(revision).unwrap_or(u8::MAX)),
            event_type: EventType::parse("branch.forked").expect("type valide"),
            schema_version: 1,
            stream_id: "br_1".to_owned(),
            stream_revision: revision,
            workspace_id: id::<Workspace>(2),
            project_id: id::<Project>(4),
            program_id: None,
            branch_id: None,
            actor: Actor {
                principal_id: id::<Agent>(principal),
                kind: ActorKind::Agent,
                delegation_id: None,
            },
            occurred_at: Timestamp::from_millis(1_700_000_000_000),
            recorded_at: Timestamp::from_millis(1_700_000_000_000),
            causation_id: id::<Command>(1),
            idempotency_key: key.map(str::to_owned),
            correlation_id: None,
            trace_id: None,
            payload: serde_json::json!({}),
            payload_hash: format!("sha256:{}", "ab".repeat(32)),
        }
    }

    fn soumission(principal: u8, key: &str) -> Submission {
        Submission {
            scope: IdempotencyScope {
                workspace: id::<Workspace>(2),
                principal: id::<Agent>(principal),
            },
            key: key.to_owned(),
        }
    }

    /// **Un événement sans clé n'entre pas au registre, et surtout pas sous une clé vide.**
    ///
    /// C'est la règle d'ADR 0029 décision 4 — « une absence n'est pas une valeur » — et elle ne
    /// s'observe qu'ici : `CommandEnvelope::mutating` refuse une clé vide, donc aucune soumission ne
    /// peut venir la réclamer, et une passe de mutants l'a montré en laissant survivre
    /// `unwrap_or("")`.
    ///
    /// Ce que le mutant produisait : **tous** les événements d'avant la migration rangés sous la
    /// même entrée `("", portée)`, à se recouvrir les uns les autres. Rien ne l'aurait lu, et c'est
    /// précisément ce qui le rendait invisible — jusqu'au jour où une clé vide devient licite.
    #[test]
    fn un_evenement_sans_cle_n_entre_pas_au_registre() {
        let journal = [
            evenement(1, 3, None),
            evenement(2, 3, None),
            evenement(3, 3, Some("apres-migration")),
        ];

        let ledger = Ledger::rebuild(journal.iter());

        assert_eq!(
            ledger.recall(&soumission(3, "")),
            None,
            "les événements sans clé se sont rangés sous une clé vide"
        );
        assert_eq!(
            ledger.recall(&soumission(3, "apres-migration")),
            Some(Revision::new(3))
        );
    }

    /// **Le rang retenu est le plus grand des événements d'une même clé.**
    ///
    /// Une commande écrit parfois plusieurs événements, et ce que la transaction a rendu au client
    /// est le rang du stream **après** l'écriture entière.
    #[test]
    fn le_rang_retenu_est_celui_du_dernier_evenement() {
        let journal = [
            evenement(7, 3, Some("une-ecriture")),
            evenement(8, 3, Some("une-ecriture")),
        ];

        assert_eq!(
            Ledger::rebuild(journal.iter()).recall(&soumission(3, "une-ecriture")),
            Some(Revision::new(8))
        );
    }

    /// **L'ordre de lecture ne change pas le rang retenu.**
    ///
    /// `max` est commutatif, et c'est ce qui rend la reconstruction indépendante de l'ordre dans
    /// lequel le flux rend les événements. Le vérifier coûte trois lignes et retire une supposition.
    #[test]
    fn l_ordre_de_lecture_ne_change_pas_le_rang() {
        let journal = [
            evenement(8, 3, Some("une-ecriture")),
            evenement(7, 3, Some("une-ecriture")),
        ];

        assert_eq!(
            Ledger::rebuild(journal.iter()).recall(&soumission(3, "une-ecriture")),
            Some(Revision::new(8))
        );
    }

    /// **Deux principaux ne se confondent pas, même sous la même clé.**
    #[test]
    fn la_portee_separe_deux_principaux() {
        let journal = [
            evenement(1, 3, Some("cle-partagee")),
            evenement(2, 99, Some("cle-partagee")),
        ];
        let ledger = Ledger::rebuild(journal.iter());

        assert_eq!(
            ledger.recall(&soumission(3, "cle-partagee")),
            Some(Revision::new(1))
        );
        assert_eq!(
            ledger.recall(&soumission(99, "cle-partagee")),
            Some(Revision::new(2))
        );
    }
}
