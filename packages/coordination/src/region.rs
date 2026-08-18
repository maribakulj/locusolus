//! Les régions mutables bornées — `docs/13` §3, d'après GRAFT (`arXiv:2608.02353`).
//!
//! # Pourquoi une région existe
//!
//! `docs/13` insiste sur le motif, et il n'est pas esthétique : « région déclarée, critère
//! d'acceptation local, veto de cohérence globale, **parce que la ré-optimisation globale à chaque
//! incident est prohibitive** ». Une région est une réponse de coût. Elle permet de décider vite sur
//! un petit périmètre, et c'est précisément pour cela qu'elle ne peut pas décider seule.
//!
//! # Le veto n'est pas un second critère local
//!
//! C'est le point du dispositif, et le seul endroit où on peut le rater. Un critère local ne voit
//! que la région ; une incohérence peut se refermer par un chemin qui en sort. L'exemple que le test
//! exerce : la région contient `A` et `B`, l'organisation porte déjà `B → D → A` avec `D` dehors, et
//! quelqu'un ajoute `A → B`. L'opération ne touche que des nœuds de la région, donc le critère local
//! l'accepte — et le cycle `A → B → D → A` vient de se fermer. Trois agents qui se relisent en rond
//! ne sont relus par personne.
//!
//! Un veto qui ne regarderait que la région serait un critère local écrit deux fois, et coûterait
//! exactement la garantie qu'on croyait avoir.
//!
//! # Quatre bornes refusent, deux obligent
//!
//! `docs/13` donne six bornes : `allowed_ops`, `risk_ceiling`, `max_nodes_delta`, `max_edges_delta`,
//! `approval_mode`, `require_shadow`. Les quatre premières **interdisent** ; les deux dernières
//! **exigent**. Les mélanger ferait croire qu'une région à `require_shadow` bloque une proposition,
//! alors qu'elle demande une étape de plus — et un opérateur qui attend un refus qui ne vient jamais
//! finit par croire que la borne n'existe pas.
//!
//! [`Acceptance`] porte donc les deux obligations et **n'expose rien qui commite**. Ce n'est pas une
//! discipline d'appel : il n'y a pas de méthode à ne pas appeler, comme pour la `Simulation` de
//! `packages/policy`.
//!
//! # « Delta » se mesure en différence symétrique, pas en solde
//!
//! Un solde net est jouable : ajouter cinq agents et en retirer cinq passe sous un plafond de zéro
//! alors que dix identités ont changé. Ce que les bornes de GRAFT veulent borner est le rayon
//! d'explosion, donc on compte les nœuds et les arêtes dont l'appartenance **a changé**.
//!
//! # Le risque est dérivé, jamais déclaré
//!
//! `docs/10` W18 demande une classe de risque « **dérivée** des invariants menacés ». Un risque que
//! le proposeur déclarerait serait auto-évalué sous un plafond, c'est-à-dire la définition d'une
//! borne qu'on contourne. Ici, le risque d'une opération est le **nombre d'invariants globaux
//! qu'elle peut menacer** — aujourd'hui zéro ou un, et l'échelle s'élargira d'elle-même quand un
//! deuxième invariant entrera. C'est peu, et c'est vrai.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use locus_protocol::{Id, id::Agent};

use crate::diff::{Diff, DiffError};
use crate::version::{Digest, Operation, Version};

/// Les invariants globaux qu'un veto protège.
///
/// **Un seul**, et pour la même raison que `RelationKind` n'en a qu'une : un invariant n'entre ici
/// que lorsqu'un vérificateur exécutable et testé existe. En nommer d'autres produirait un veto qui
/// aurait l'air de protéger ce que rien ne regarde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Invariant {
    /// Aucun cycle de revue.
    ///
    /// `A` relit `B` qui relit `C` qui relit `A` : chacun est relu, le groupe ne l'est par
    /// personne. C'est le consensus circulaire que §16.6 nomme dans le domaine épistémique, ici
    /// dans celui de la coordination — et l'invariant 11 est ce qu'il vide de son sens.
    ReviewAcyclicity,
}

impl Invariant {
    /// Le seul, aujourd'hui.
    pub const ALL: [Self; 1] = [Self::ReviewAcyclicity];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::ReviewAcyclicity => "review-acyclicity",
        }
    }
}

impl fmt::Display for Invariant {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Ce qu'une opération peut menacer.
///
/// « Peut », pas « menace » : la question est posée sur la forme de l'opération, avant de savoir sur
/// quelle organisation elle tombera. Seules celles qui **créent** un chemin de revue peuvent fermer
/// un cycle ; retirer n'en crée jamais, et remplacer ou scinder réécrit sans en ajouter.
#[must_use]
pub fn threatens(operation: &Operation) -> BTreeSet<Invariant> {
    match operation {
        // Une arête neuve peut refermer un chemin existant ; fusionner rapproche deux extrémités,
        // et `A → B → C` fusionné sur `A` et `C` devient un cycle de longueur deux sans qu'aucune
        // arête n'ait été ajoutée.
        Operation::AddEdge(_) | Operation::MergeNodes { .. } => {
            [Invariant::ReviewAcyclicity].into_iter().collect()
        }
        // Ajouter un nœud isolé, retirer, remplacer — un remplacement est un isomorphisme — et
        // scindre, qui ne fait que répartir des arêtes existantes : aucun ne crée de chemin.
        Operation::AddNode(_)
        | Operation::RemoveNode(_)
        | Operation::ReplaceNode { .. }
        | Operation::RemoveEdge(_)
        | Operation::SplitNode { .. } => BTreeSet::new(),
    }
}

/// Ce qu'une région exige avant qu'un lot puisse être commité.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalMode {
    /// Un humain approuve.
    Human,
    /// N'importe qui d'autre que l'auteur approuve.
    ///
    /// `forbid_self_approval` ne se relâche dans aucun mode (ADR 0016, décision 8) : « n'importe qui
    /// d'autre » exclut l'auteur, ici comme ailleurs.
    Peer,
}

impl ApprovalMode {
    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Peer => "peer",
        }
    }
}

impl fmt::Display for ApprovalMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Une région mutable bornée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    name: String,
    nodes: BTreeSet<Id<Agent>>,
    allowed_ops: BTreeSet<String>,
    risk_ceiling: usize,
    max_nodes_delta: usize,
    max_edges_delta: usize,
    approval_mode: ApprovalMode,
    require_shadow: bool,
}

impl Region {
    /// Déclarer une région, avec ses six bornes.
    ///
    /// # Errors
    ///
    /// [`RegionError::EmptyName`] pour une région anonyme — un veto qui ne dit pas de quelle région
    /// il vient ne se corrige pas ; [`RegionError::EmptyRegion`] pour une région sans nœud, qui ne
    /// borne rien tout en ayant l'air d'une borne ; [`RegionError::UnknownOperation`] pour une
    /// opération que `docs/13` nomme mais qui n'existe pas encore, comme `SET_ROLE` — l'autoriser
    /// ne permettrait rien pendant que son auteur croirait le contraire.
    #[expect(
        clippy::too_many_arguments,
        reason = "les six bornes de docs/13, plus l'identité"
    )]
    pub fn declare(
        name: &str,
        nodes: &[Id<Agent>],
        allowed_ops: &[&str],
        risk_ceiling: usize,
        max_nodes_delta: usize,
        max_edges_delta: usize,
        approval_mode: ApprovalMode,
        require_shadow: bool,
    ) -> Result<Self, RegionError> {
        if name.trim().is_empty() {
            return Err(RegionError::EmptyName);
        }
        if nodes.is_empty() {
            return Err(RegionError::EmptyRegion);
        }
        for operation in allowed_ops {
            if !Operation::NAMES.contains(operation) {
                return Err(RegionError::UnknownOperation {
                    operation: (*operation).to_owned(),
                });
            }
        }
        Ok(Self {
            name: name.to_owned(),
            nodes: nodes.iter().copied().collect(),
            allowed_ops: allowed_ops
                .iter()
                .map(|operation| (*operation).to_owned())
                .collect(),
            risk_ceiling,
            max_nodes_delta,
            max_edges_delta,
            approval_mode,
            require_shadow,
        })
    }

    /// Son nom.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Les nœuds qu'elle déclare.
    #[must_use]
    pub const fn nodes(&self) -> &BTreeSet<Id<Agent>> {
        &self.nodes
    }

    /// Confronter un diff à cette région, puis à la cohérence globale.
    ///
    /// Les deux étapes sont menées dans cet ordre et **toutes les deux** : un lot refusé localement
    /// ne va pas plus loin, un lot accepté localement passe encore par le veto. C'est la structure
    /// que GRAFT décrit, et l'ordre inverse coûterait le bénéfice de coût qui la justifie.
    ///
    /// # Errors
    ///
    /// Ce que `Diff::replay` refuse : une base qui a bougé, une opération inapplicable, une cible
    /// annoncée que le rejeu ne produit pas. Une région ne se prononce pas sur un lot qui ne
    /// s'applique pas — elle dirait quelque chose d'un état qui n'existera jamais.
    pub fn admits(
        &self,
        base: &Version,
        diff: &Diff,
        digest: &impl Digest,
    ) -> Result<Verdict, DiffError> {
        let produced = diff.replay(base, digest)?;
        let accepted = match self.accepts(base, diff, &produced) {
            Ok(acceptance) => acceptance,
            Err(refusal) => return Ok(Verdict::Refused(refusal)),
        };
        Ok(match coherence(&produced) {
            Coherence::Coherent => Verdict::Admissible(accepted),
            Coherence::Broken { invariant, witness } => Verdict::Vetoed {
                accepted,
                invariant,
                witness,
            },
        })
    }

    /// Le critère d'acceptation **local** — les quatre bornes qui interdisent.
    fn accepts(
        &self,
        base: &Version,
        diff: &Diff,
        produced: &Version,
    ) -> Result<Acceptance, Refusal> {
        for operation in diff.operations() {
            if !self.allowed_ops.contains(operation.name()) {
                return Err(Refusal::OperationNotAllowed {
                    region: self.name.clone(),
                    operation: operation.canonical(),
                });
            }
            for node in touched(operation) {
                if !self.nodes.contains(&node) {
                    return Err(Refusal::OutOfRegion {
                        region: self.name.clone(),
                        operation: operation.canonical(),
                        node: node.to_string(),
                    });
                }
            }
            let risk = threatens(operation).len();
            if risk > self.risk_ceiling {
                return Err(Refusal::OverRisk {
                    region: self.name.clone(),
                    operation: operation.canonical(),
                    risk,
                    ceiling: self.risk_ceiling,
                });
            }
        }

        let nodes_delta = base
            .members()
            .symmetric_difference(produced.members())
            .count();
        if nodes_delta > self.max_nodes_delta {
            return Err(Refusal::TooManyNodes {
                region: self.name.clone(),
                changed: nodes_delta,
                ceiling: self.max_nodes_delta,
            });
        }
        let edges_delta = base
            .relations()
            .symmetric_difference(produced.relations())
            .count();
        if edges_delta > self.max_edges_delta {
            return Err(Refusal::TooManyEdges {
                region: self.name.clone(),
                changed: edges_delta,
                ceiling: self.max_edges_delta,
            });
        }

        Ok(Acceptance {
            region: self.name.clone(),
            requires_approval: self.approval_mode,
            requires_shadow: self.require_shadow,
        })
    }
}

/// Les nœuds dont une opération change l'appartenance, ou qu'elle nomme aux bouts d'une arête.
///
/// La partition d'une scission n'y entre **pas**, et c'est une décision : exiger que tout le
/// voisinage d'un agent soit dans la région rendrait toute région inutile dès qu'un agent est relu
/// depuis l'extérieur, ce qui est le cas courant. Les conséquences d'une scission sur des nœuds
/// externes sont ce que le veto global attrape — c'est exactement le partage du travail que GRAFT
/// décrit.
fn touched(operation: &Operation) -> Vec<Id<Agent>> {
    match operation {
        Operation::AddNode(node) | Operation::RemoveNode(node) => vec![*node],
        Operation::ReplaceNode { from, to } => vec![*from, *to],
        Operation::AddEdge(relation) | Operation::RemoveEdge(relation) => {
            vec![relation.from, relation.to]
        }
        Operation::SplitNode { node, into, .. } => vec![*node, into.0, into.1],
        Operation::MergeNodes {
            first,
            second,
            into,
        } => vec![*first, *second, *into],
    }
}

/// Ce que la cohérence globale dit d'un état.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Coherence {
    Coherent,
    Broken {
        invariant: Invariant,
        witness: Vec<String>,
    },
}

/// Le veto de cohérence globale, sur l'organisation **entière**.
///
/// Le cycle se cherche par élimination itérative des nœuds sans arête entrante — Kahn. Ce qui reste
/// est exactement l'ensemble des nœuds pris dans un cycle, et il est rendu comme témoin : un veto
/// qui dirait « il y a un cycle » sans dire lequel obligerait à le chercher à la main.
fn coherence(version: &Version) -> Coherence {
    let mut incoming: BTreeMap<Id<Agent>, usize> =
        version.members().iter().map(|node| (*node, 0)).collect();
    for relation in version.relations() {
        *incoming.entry(relation.to).or_insert(0) += 1;
    }

    let mut ready: VecDeque<Id<Agent>> = incoming
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(node, _)| *node)
        .collect();
    let mut settled = 0_usize;
    while let Some(node) = ready.pop_front() {
        settled += 1;
        for relation in version.relations().iter().filter(|edge| edge.from == node) {
            if let Some(count) = incoming.get_mut(&relation.to) {
                *count -= 1;
                if *count == 0 {
                    ready.push_back(relation.to);
                }
            }
        }
        incoming.remove(&node);
    }

    if settled == version.members().len() {
        return Coherence::Coherent;
    }
    Coherence::Broken {
        invariant: Invariant::ReviewAcyclicity,
        witness: incoming.keys().map(ToString::to_string).collect(),
    }
}

/// Ce qu'une région a accepté, et ce qu'elle exige encore.
///
/// Elle n'expose **rien** qui commite. Une acceptation locale n'est pas une permission d'écrire :
/// c'est l'énoncé de ce qui reste à faire — approuver, et ombrer si la région le demande.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Acceptance {
    region: String,
    requires_approval: ApprovalMode,
    requires_shadow: bool,
}

impl Acceptance {
    /// Quelle région a accepté.
    #[must_use]
    pub fn region(&self) -> &str {
        &self.region
    }

    /// L'approbation que la région exige.
    #[must_use]
    pub const fn requires_approval(&self) -> ApprovalMode {
        self.requires_approval
    }

    /// Vrai quand la région exige une exécution en ombre avant le commit.
    #[must_use]
    pub const fn requires_shadow(&self) -> bool {
        self.requires_shadow
    }
}

/// Ce qu'une région conclut d'un lot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Accepté localement **et** globalement cohérent. Restent l'approbation et, s'il est exigé,
    /// l'ombre.
    Admissible(Acceptance),
    /// Refusé par une borne de la région.
    Refused(Refusal),
    /// Accepté localement, **vetoé** globalement.
    ///
    /// L'acceptation locale est conservée exprès : elle montre que le critère de la région a bien
    /// passé, ce qui est tout le sujet du veto. La fondre dans le refus ferait croire à une borne de
    /// région trop lâche, alors que la région a fait son travail et qu'une chose qu'elle ne pouvait
    /// pas voir a mordu.
    Vetoed {
        /// Ce que la région avait accepté.
        accepted: Acceptance,
        /// L'invariant rompu.
        invariant: Invariant,
        /// Les agents pris dedans.
        witness: Vec<String>,
    },
}

impl Verdict {
    /// Vrai quand rien n'empêche plus la suite du chemin.
    ///
    /// Ce n'est **pas** une permission de commiter : l'[`Acceptance`] dit ce qui reste à obtenir.
    #[must_use]
    pub const fn is_admissible(&self) -> bool {
        matches!(self, Self::Admissible(_))
    }
}

/// Laquelle des bornes de la région a mordu.
///
/// Quatre, parce que quatre bornes interdisent. `approval_mode` et `require_shadow` n'apparaissent
/// pas ici : elles exigent, elles n'interdisent pas, et leur donner un refus ferait attendre un
/// blocage qui ne vient jamais.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// L'opération touche un nœud hors de la région déclarée.
    OutOfRegion {
        /// Laquelle.
        region: String,
        /// L'opération.
        operation: String,
        /// Le nœud fautif.
        node: String,
    },
    /// L'opération n'est pas dans `allowed_ops`.
    OperationNotAllowed {
        /// Laquelle.
        region: String,
        /// L'opération.
        operation: String,
    },
    /// L'opération menace plus d'invariants que la région ne l'admet.
    OverRisk {
        /// Laquelle.
        region: String,
        /// L'opération.
        operation: String,
        /// Ce qu'elle menace.
        risk: usize,
        /// Le plafond.
        ceiling: usize,
    },
    /// Trop de nœuds changent d'appartenance.
    TooManyNodes {
        /// Laquelle.
        region: String,
        /// Combien changent.
        changed: usize,
        /// Le plafond.
        ceiling: usize,
    },
    /// Trop d'arêtes changent d'appartenance.
    TooManyEdges {
        /// Laquelle.
        region: String,
        /// Combien changent.
        changed: usize,
        /// Le plafond.
        ceiling: usize,
    },
}

impl Refusal {
    /// Le nom de la borne de `docs/13` qui a mordu.
    #[must_use]
    pub const fn bound(&self) -> &'static str {
        match self {
            Self::OutOfRegion { .. } => "region",
            Self::OperationNotAllowed { .. } => "allowed_ops",
            Self::OverRisk { .. } => "risk_ceiling",
            Self::TooManyNodes { .. } => "max_nodes_delta",
            Self::TooManyEdges { .. } => "max_edges_delta",
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfRegion {
                region,
                operation,
                node,
            } => write!(
                formatter,
                "« {operation} » touche {node}, hors de la région « {region} »"
            ),
            Self::OperationNotAllowed { region, operation } => write!(
                formatter,
                "« {operation} » n'est pas dans les `allowed_ops` de « {region} »"
            ),
            Self::OverRisk {
                region,
                operation,
                risk,
                ceiling,
            } => write!(
                formatter,
                "« {operation} » menace {risk} invariant(s), la région « {region} » en admet \
                 {ceiling}"
            ),
            Self::TooManyNodes {
                region,
                changed,
                ceiling,
            } => write!(
                formatter,
                "{changed} nœuds changent d'appartenance, « {region} » en admet {ceiling}"
            ),
            Self::TooManyEdges {
                region,
                changed,
                ceiling,
            } => write!(
                formatter,
                "{changed} arêtes changent d'appartenance, « {region} » en admet {ceiling}"
            ),
        }
    }
}

impl std::error::Error for Refusal {}

/// Ce qui empêche une région d'être déclarée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegionError {
    /// Une région anonyme.
    EmptyName,
    /// Une région sans nœud.
    EmptyRegion,
    /// Une opération que `docs/13` nomme mais qui n'existe pas encore.
    UnknownOperation {
        /// Laquelle.
        operation: String,
    },
}

impl fmt::Display for RegionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => formatter.write_str(
                "une région anonyme produit un veto qui ne dit pas d'où il vient, donc qui ne se \
                 corrige pas",
            ),
            Self::EmptyRegion => formatter
                .write_str("une région sans nœud ne borne rien tout en ayant l'air d'une borne"),
            Self::UnknownOperation { operation } => write!(
                formatter,
                "« {operation} » n'existe pas : l'autoriser ne permettrait rien pendant que son \
                 auteur croirait le contraire"
            ),
        }
    }
}

impl std::error::Error for RegionError {}
