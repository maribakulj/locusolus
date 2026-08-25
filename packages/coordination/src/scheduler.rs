//! L'ordonnanceur d'instances — `W23.c`, `docs/13` §3, ADR 0026 décision 5.
//!
//! # Ce que cet item ferme, et qui attendait depuis `W13`
//!
//! [`crate::lifecycle::may_leave_the_version`] existe depuis que le module de cycle de vie existe,
//! avec cette docstring : « `REMOVE_NODE` retire un nœud de l'organisation, et le retirer pendant
//! que son instance tourne ferait dire à la version qu'un agent est parti alors qu'il travaille
//! encore. La version ne peut pas le savoir seule — elle ne détient que des identités — donc c'est
//! **ici** que la question se pose, et le scheduler compose les deux. »
//!
//! Le scheduler n'existait pas. La fonction n'a donc **jamais eu d'appelant** hors de ses propres
//! tests, et `Version::apply` retire un nœud sans que personne demande si son instance tourne. Ce
//! n'est pas un lecteur sans producteur comme `W20.ad` et `W20.ae` : c'est l'inverse, une **règle
//! sans applicateur**. La conséquence est la même — les deux moitiés sont correctes et testées
//! séparément, et rien ne les met bout à bout.
//!
//! # Le scheduler n'a aucun verbe à lui
//!
//! C'est la propriété centrale, et [`SchedulerDecision`] la porte dans sa forme : **deux** variantes, l'une
//! déléguant à [`crate::lifecycle::Command`], l'autre à [`crate::version::Operation`]. Aucune
//! troisième, aucun verbe propre, aucun synonyme.
//!
//! `docs/13` énumère treize choses que « le scheduler doit savoir faire », et l'en-tête de
//! [`crate::lifecycle`] a déjà fait le tri : quatre portent sur l'instance qui tourne, cinq sont des
//! opérations de version, les quatre dernières supposaient une messagerie qui vit ailleurs. Les
//! réécrire ici produirait un second chemin qui divergerait du premier le jour où l'un des deux est
//! corrigé — et personne ne saurait lequel décrit ce qui sera commité.
//!
//! # Il ne choisit **jamais** d'hôte
//!
//! `place` de `W4.g` vit chez `locus-execd` et choisit sur ce qu'un hôte a *prouvé*. L'ordonnanceur
//! d'instances est un cran au-dessus : il décide **au sujet** d'instances, il ne les exécute pas et
//! ne dit pas où. La propriété est tenue par l'absence — `packages/coordination` ne dépend d'aucun
//! crate d'exécution, et un test lit `Cargo.toml` plutôt que les sources : chercher un `use` laissé
//! passerait une dépendance ajoutée sans import encore écrit, et la propriété voulue est « personne
//! ne **peut** », pas « personne n'a encore ».
//!
//! # Ce qu'il refuse, et pourquoi il ne refuse que ça
//!
//! Une décision de cycle de vie est validée par le cycle de vie ; une opération de version est
//! validée par la version. L'ordonnanceur n'ajoute **qu'une** règle, celle qu'aucun des deux ne peut
//! poser seul : un nœud ne quitte pas la version tant que son instance n'est pas terminée. Quatre
//! opérations font sortir un nœud, et [`SchedulerDecision::departures`] les énumère par un `match`
//! exhaustif — une opération de plus ne compilera pas sans qu'on ait répondu à la question.

use locus_protocol::Id;
use locus_protocol::id::Agent;

use crate::lifecycle::{Command, Lifecycle, LifecycleError, may_leave_the_version};
use crate::version::Operation;

/// Ce qu'un ordonnanceur d'instances décide.
///
/// # Pourquoi le nom est long
///
/// `Decision` est déjà pris dans ce crate, par [`crate::decision`], et il y désigne tout autre
/// chose — une demande d'approbation de §20. Deux types du même nom dans un même crate se
/// confondent à la lecture même quand le compilateur les sépare, et `W0.18` a montré ce que coûte
/// un identifiant qui en désigne deux.
///
/// **Deux variantes, et c'est la propriété.** Le vocabulaire de l'ordonnanceur est exactement
/// l'union des deux familles qu'il compose ; une troisième variante voudrait dire qu'il a inventé un
/// verbe, ce que `CLAUDE.md` refuse sous le nom de « vocabulaire parallèle ».
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchedulerDecision {
    /// Piloter une instance — les quatre verbes de [`crate::lifecycle`].
    Lifecycle {
        /// L'instance visée.
        node: Id<Agent>,
        /// Ce qui lui est demandé.
        command: Command,
    },
    /// Changer la structure — les opérations de [`crate::version`].
    Structural(Operation),
}

impl SchedulerDecision {
    /// Les nœuds que cette décision fait **quitter** la version.
    ///
    /// # Le `match` est exhaustif, et c'est ce qui rend la règle durable
    ///
    /// Quatre opérations font sortir un nœud : `REMOVE_NODE`, `REPLACE_NODE` par son `from`,
    /// `SPLIT_NODE` par le nœud scindé — les trois réunies en un motif, leurs corps étant
    /// identiques — et `MERGE_NODES`, à part parce qu'elle en fait sortir **deux**. Les autres n'en
    /// font sortir aucun. Une opération ajoutée à [`Operation`] sans réponse à cette question **ne
    /// compilera pas** — c'est la seule façon de garantir que la règle suive le domaine plutôt que
    /// de prendre du retard sur lui en silence.
    #[must_use]
    pub fn departures(&self) -> Vec<Id<Agent>> {
        match self {
            Self::Lifecycle { .. } => Vec::new(),
            Self::Structural(operation) => match operation {
                // Trois opérations font sortir **un** nœud, et le motif les réunit parce que leurs
                // corps sont identiques — les séparer ferait rougir `clippy::match_same_arms`, et
                // la distinction perdue serait purement décorative : ce qui compte est *quel* nœud
                // part, pas par quelle opération.
                Operation::RemoveNode(node)
                | Operation::ReplaceNode { from: node, .. }
                | Operation::SplitNode { node, .. } => vec![*node],
                // La fusion est à part parce qu'elle en fait sortir **deux**.
                Operation::MergeNodes { first, second, .. } => vec![*first, *second],
                Operation::AddNode(_)
                | Operation::AddEdge(_)
                | Operation::RemoveEdge(_)
                | Operation::SetRole { .. }
                | Operation::SetMode { .. }
                | Operation::SetCoordinator { .. } => Vec::new(),
            },
        }
    }
}

/// Admettre une décision, ou dire ce qui s'y oppose.
///
/// # La seule règle que l'ordonnanceur ajoute
///
/// Le cycle de vie valide ses propres commandes, la version valide les siennes ; les appeler ici
/// referait leur travail et les deux copies divergeraient. Ce qu'aucun des deux ne peut faire seul
/// est de confronter une sortie de version à l'état d'une instance : la version ne détient que des
/// identités, le cycle de vie ignore la structure. C'est donc **tout** ce qui se décide ici.
///
/// Un nœud que le cycle de vie ne connaît pas n'a pas d'instance, et rien ne s'oppose à ce qu'il
/// quitte la version — un membre déclaré qu'on n'a jamais réveillé se retire sans cérémonie.
///
/// # Errors
///
/// [`LifecycleError::StillRunning`], tel que [`may_leave_the_version`] le rend, en nommant le nœud
/// et son état. L'erreur n'est pas réemballée : un type de refus propre à ce module serait un
/// vocabulaire de plus pour dire ce que le cycle de vie dit déjà.
pub fn admit(instances: &Lifecycle, decision: &SchedulerDecision) -> Result<(), LifecycleError> {
    for node in decision.departures() {
        if let Some(state) = instances.state(node) {
            may_leave_the_version(node, state)?;
        }
    }
    Ok(())
}
