//! La version canonique immuable et le jeu d'opérations — `docs/13` §3, ADR 0016 décisions 4 et 5.
//!
//! # Deux hashes, et c'est le sujet du module
//!
//! Une version porte **un hash de contenu** et **un hash de version**. Le premier ne dépend que de
//! ce que la version contient : deux organisations identiques le partagent, quelle que soit
//! l'histoire qui y a mené. Le second ajoute le parent, et c'est lui l'identité.
//!
//! La séparation n'est pas décorative, elle rend testable la phrase de l'ADR 0016 décision 5 :
//! « une annulation est le commit d'un changement inverse […] aucune version antérieure n'est
//! supprimée ». Défaire une opération rend **le même contenu** et **une autre version** — l'état
//! revient, l'histoire non. Avec un seul hash il aurait fallu choisir : ou bien défaire ramène
//! littéralement à la version d'avant, et l'histoire est fausse ; ou bien défaire produit un état
//! qu'on ne peut pas reconnaître comme celui d'avant, et personne ne peut vérifier qu'une
//! annulation a bien annulé.
//!
//! # Un IR déclaratif, jamais un script
//!
//! `docs/10` : « la représentation détermine ce qui est vérifiable ». Une opération énonce son
//! résultat ; elle ne le calcule pas. La conséquence visible est le refus de la **cascade** :
//! retirer un nœud qui porte encore des arêtes est refusé au lieu d'emporter les arêtes avec lui.
//! Une cascade est un script — elle fait au commit des choses que le diff ne montrait pas, et
//! l'approbation aurait porté sur autre chose que ce qui s'applique.
//!
//! # Dix opérations : huit des onze de `docs/13`, et deux de §14.3
//!
//! `docs/13` nomme onze opérations cibles, dont **huit** sont ici. Les deux dernières —
//! [`Operation::SetMode`] et [`Operation::SetCoordinator`] — ne viennent pas de cette liste mais de
//! §14.3, par l'ADR 0021, et elles sont entrées **ensemble** : §14.3 fait du coordinateur la
//! définition du mode `coordinator`, et les séparer aurait permis d'atteindre entre les deux un état
//! que §14.3 déclare impossible. La règle « aucune sémantique inerte » (ADR 0016,
//! décision 4) vaut pour une opération comme pour une sorte de relation, et elle trace ici une
//! frontière nette : une opération **structurelle** — nœuds et arêtes — a son effet entièrement
//! défini par l'état que ce crate détient, donc un consommateur exécutable et testé, qui est
//! [`Version::apply`]. Une opération **attributaire** écrit sur un nœud un champ dont le lecteur
//! vit ailleurs ; l'écrire ici produirait un attribut que le système sait versionner, différencier,
//! approuver et afficher, et que rien n'honore.
//!
//! `SET_ROLE` est entrée par W15.f, et **uniquement** parce que ce lecteur existe désormais :
//! `selectOverlay`, dans le worker Canterel, choisit l'agent par rôle depuis le champ que la
//! tranche 1 du mineur `lep/1.1` ajoute (ADR 0017 §5.1). La décision 4 n'interdit pas les
//! opérations attributaires, elle interdit celles que rien n'honore — c'est une condition, pas un
//! verdict, et elle se lève quand la condition tombe.
//!
//! Elle porte une conséquence que les structurelles n'avaient pas : un rôle est une information
//! qu'un nœud emporte. Retirer, scinder ou fusionner un nœud qui en porte un est donc **refusé**,
//! comme pour une arête et pour la même raison — l'opération inverse ne saurait pas le rendre.
//! Remplacer, en revanche, l'emporte avec l'identité : un remplacement est un isomorphisme, rien ne
//! s'y perd.
//!
//! Les trois absentes, et ce que chacune attend :
//!
//! - `SET_VISIBILITY` : la construction de `ContextView` (décision 11) ;
//! - `SET_VALIDATOR` : qu'un validateur soit un nœud — `docs/13` le range dans les nœuds « plus
//!   tard », et il n'y en a pas aujourd'hui ;
//! - `SET_EXECUTION_ORDER` : qu'une chose ordonne des attempts **entre instances d'agent**. La
//!   décision 4 a déjà fait cette vérification en instruisant `dependency` : `steps` ordonne à
//!   l'intérieur d'un workflow, et le scheduler de §12 place sans ordonner.
//!
//! # Fusionner se compense, se défait pas
//!
//! Neuf des dix opérations ont un inverse exact. La fusion n'en a pas, et pour une raison qui se
//! lit dans sa définition : elle perd la partition. Deux arêtes `X → premier` et `X → second`
//! deviennent une seule `X → fusionné`, et rien dans le résultat ne dit qu'elles étaient deux. La
//! scission, elle, énonce sa partition, donc sa fusion inverse la restitue.
//!
//! [`Undo::Compensating`] nomme cette asymétrie plutôt que de la cacher derrière une fonction qui
//! rendrait une scission approximative. ADR 0016, décision 5 : « une modification non inversible ne
//! peut être que compensée, et elle le déclare à la proposition ».

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use locus_domain::ContentHash;
use locus_protocol::{Id, id::Agent};

use crate::proposal::{Relation, RelationKind};
use crate::team::CoordinationMode;

/// La ligne d'en-tête de la forme canonique d'un contenu.
const CONTENT_MAGIC: &str = "coordination-content/1";

/// Ce qu'une forme canonique écrit à la place d'un rôle absent.
///
/// Nommée plutôt que répétée, parce qu'un rôle **égal à cette chaîne** est refusé : sans quoi
/// « retirer le rôle » et « poser le rôle `-` » auraient la même forme canonique, donc la même
/// signature d'approbation, pour deux opérations qui ne font pas la même chose.
const ABSENT: &str = "-";

/// La ligne d'en-tête de la forme canonique d'une identité de version.
const VERSION_MAGIC: &str = "coordination-version/1";

/// Ce qui calcule le condensat d'une forme canonique — un port.
///
/// Ce crate ne choisit aucun algorithme, pour la raison que `locus_domain::ContentHash` énonce
/// déjà : ce serait une décision d'infrastructure. Ce qu'il garantit est plus fort et plus utile —
/// deux contenus égaux produisent la **même** forme canonique, octet pour octet, et un test la fige
/// en fixture.
pub trait Digest {
    /// Le condensat de `canonical`.
    fn digest(&self, canonical: &str) -> ContentHash;
}

/// L'implémentation de production du port — ADR 0020.
///
/// Le port existait depuis `W15.a`, et **rien ne l'implémentait** hors des fixtures de test. La
/// prudence qui l'avait laissé vide se lit encore au-dessus : « ce crate ne choisit aucun
/// algorithme, ce serait une décision d'infrastructure ». Elle était juste sur le principe et
/// fausse dans son effet — le résultat n'était pas la neutralité, c'était qu'aucun condensat n'était
/// calculable nulle part.
///
/// Le port reste un port : il n'a pas disparu, et un appelant qui veut un condensat jouet en fournit
/// toujours un. Ce qui change est qu'il existe désormais une **réponse par défaut**, et qu'elle
/// délègue à `locus_domain::ContentHash::of` — un seul endroit choisit l'algorithme, et
/// `dependencies.json` l'y tient.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContentDigest;

impl Digest for ContentDigest {
    fn digest(&self, canonical: &str) -> ContentHash {
        ContentHash::of(canonical.as_bytes())
    }
}

/// L'identité d'une version : son contenu **et** d'où elle vient.
///
/// Deux versions de même contenu et de parents différents sont deux versions. C'est ce qui empêche
/// une annulation de se faire passer pour un retour en arrière.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VersionId(ContentHash);

impl VersionId {
    /// Relire une identité de version reçue d'ailleurs — un chemin d'URL, un cursor, un événement.
    ///
    /// `None` plutôt qu'un défaut : une identité illisible rabattue sur une racine plausible est
    /// exactement le mode d'échec que `W17.j` interdit, et celui que `W20.e` a nommé pour les
    /// cursors — une réponse prise au mauvais endroit, que rien dans la réponse ne signale.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        ContentHash::parse(text).ok().map(Self)
    }

    /// Le hash sous-jacent.
    #[must_use]
    pub const fn hash(&self) -> &ContentHash {
        &self.0
    }
}

impl fmt::Display for VersionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// Les dix opérations qui ont un consommateur exécutable : sept structurelles, et trois
/// attributaires — `SET_ROLE`, entrée par W15.f, puis `SET_MODE` et `SET_COORDINATOR` par l'ADR 0021.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    /// `ADD_NODE` — une instance d'agent entre.
    AddNode(Id<Agent>),
    /// `REMOVE_NODE` — elle sort. **Sans cascade** : ses arêtes se retirent avant, une par une.
    RemoveNode(Id<Agent>),
    /// `REPLACE_NODE` — une identité en remplace une autre, arêtes comprises.
    ReplaceNode {
        /// Celle qui sort.
        from: Id<Agent>,
        /// Celle qui entre.
        to: Id<Agent>,
    },
    /// `ADD_EDGE` — une relation de coordination.
    AddEdge(Relation),
    /// `REMOVE_EDGE` — elle disparaît.
    RemoveEdge(Relation),
    /// `SPLIT_NODE` — un nœud devient deux, et la partition de ses arêtes est **énoncée**.
    SplitNode {
        /// Celui qui disparaît.
        node: Id<Agent>,
        /// Les deux qui le remplacent, dans cet ordre.
        into: (Id<Agent>, Id<Agent>),
        /// Les arêtes incidentes qui suivent le premier ; les autres suivent le second.
        follows_first: BTreeSet<Relation>,
    },
    /// `MERGE_NODES` — deux nœuds deviennent un, et la partition est perdue.
    MergeNodes {
        /// Le premier.
        first: Id<Agent>,
        /// Le second.
        second: Id<Agent>,
        /// L'identité produite, neuve.
        into: Id<Agent>,
    },
    /// `SET_ROLE` — l'instance reçoit, change ou perd son rôle (`SPEC_V1.md` §7.1, §20).
    ///
    /// Elle **énonce ce qu'elle remplace**, comme la scission énonce sa partition, et pour la même
    /// raison : sans `from`, son inverse devrait deviner le rôle d'avant, et une annulation
    /// rendrait un contenu que personne n'a approuvé. [`Version::apply`] vérifie que `from` est
    /// bien ce que le nœud porte — un diff calculé sur un état périmé s'applique alors à autre
    /// chose que ce qu'il montrait.
    SetRole {
        /// L'instance.
        node: Id<Agent>,
        /// Ce qu'elle porte aujourd'hui — `None` si elle n'a pas de rôle.
        from: Option<String>,
        /// Ce qu'elle portera — `None` la lui retire.
        to: Option<String>,
    },
    /// `SET_MODE` — changer le mode de coordination de §14.3.
    ///
    /// Attributaire comme `SET_ROLE`, et entrée par l'ADR 0021 **avec** [`Operation::SetCoordinator`],
    /// jamais seule : §14.3 fait du coordinateur la définition du mode `coordinator`, et deux
    /// opérations entrées séparément auraient permis d'atteindre entre les deux un état que §14.3
    /// déclare impossible.
    ///
    /// Elle énonce ce qu'elle remplace, pour la raison de `SET_ROLE` — sans `from`, son inverse
    /// devrait deviner.
    SetMode {
        /// Celui d'avant.
        from: CoordinationMode,
        /// Celui d'après.
        to: CoordinationMode,
    },
    /// `SET_COORDINATOR` — désigner, changer ou retirer l'instance qui coordonne (§14.3).
    SetCoordinator {
        /// Celle d'avant — `None` s'il n'y en avait pas.
        from: Option<Id<Agent>>,
        /// Celle d'après — `None` la retire.
        to: Option<Id<Agent>>,
    },
}

impl Operation {
    /// Les dix noms — les huit de `docs/13`, et les deux que l'ADR 0021 y ajoute.
    ///
    /// Les trois absents — `SET_VISIBILITY`, `SET_VALIDATOR`, `SET_EXECUTION_ORDER` — le sont
    /// parce qu'aucun lecteur n'existe pour l'attribut qu'ils écriraient, et un test le tient par
    /// l'absence. La liste est ici pour que ce test ait quelque chose à lire.
    ///
    /// `SET_ROLE` était le quatrième et ne l'est plus : `selectOverlay`, dans le worker Canterel,
    /// lit le rôle sur le fil — la tranche 1 du mineur `lep/1.1`, ADR 0017 §5.1, qui nomme le
    /// document et le champ. C'est la règle de la décision 4 appliquée dans le sens où elle ouvre :
    /// une opération attributaire entre **quand** son consommateur existe, pas avant.
    ///
    /// Le document en question ne se nomme pas ici, et ce n'est pas une pudeur : un test de
    /// `proposal.rs` refuse ce nom dans tout le crate, commentaires compris, parce que ce crate ne
    /// doit avoir aucun moyen de toucher à une mission émise. Sa version stricte — le nom interdit
    /// jusque dans la prose — coûte cette périphrase et évite d'avoir à décider, à chaque relecture,
    /// si une occurrence est un usage ou une explication.
    pub const NAMES: [&'static str; 10] = [
        "ADD_NODE",
        "REMOVE_NODE",
        "REPLACE_NODE",
        "ADD_EDGE",
        "REMOVE_EDGE",
        "SPLIT_NODE",
        "MERGE_NODES",
        "SET_ROLE",
        // Deux ajouts locaux, et signalés comme tels : `docs/13` ne les liste pas. Ils entrent par
        // l'ADR 0021, qui fait vivre dans la version la structure que `Team` stockait en double, et
        // sous la règle de la décision 4 de l'ADR 0016 — §14.3 est leur consommateur. Les fondre
        // dans la liste sans le dire ferait passer un ajout pour une lecture de `docs/13`.
        "SET_MODE",
        "SET_COORDINATOR",
    ];

    /// Son nom, celui de `docs/13`.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::AddNode(_) => "ADD_NODE",
            Self::RemoveNode(_) => "REMOVE_NODE",
            Self::ReplaceNode { .. } => "REPLACE_NODE",
            Self::AddEdge(_) => "ADD_EDGE",
            Self::RemoveEdge(_) => "REMOVE_EDGE",
            Self::SplitNode { .. } => "SPLIT_NODE",
            Self::MergeNodes { .. } => "MERGE_NODES",
            Self::SetRole { .. } => "SET_ROLE",
            Self::SetMode { .. } => "SET_MODE",
            Self::SetCoordinator { .. } => "SET_COORDINATOR",
        }
    }

    /// Sa forme canonique — ce que deux clients comparent pour prouver qu'ils lisent la même
    /// opération.
    ///
    /// Elle porte **tout** ce qui décide de l'effet, la partition d'une scission comprise. Une
    /// forme qui n'écrirait que le nom et les identités laisserait deux scissions de partitions
    /// opposées se ressembler, et l'approbation aurait porté sur celle qu'on n'applique pas.
    #[must_use]
    pub fn canonical(&self) -> String {
        match self {
            Self::AddNode(node) | Self::RemoveNode(node) => {
                format!("{}\t{node}", self.name())
            }
            Self::ReplaceNode { from, to } => format!("{}\t{from}\t{to}", self.name()),
            Self::AddEdge(relation) | Self::RemoveEdge(relation) => {
                format!("{}\t{}", self.name(), edge(relation))
            }
            Self::SplitNode {
                node,
                into,
                follows_first,
            } => {
                let mut shares: Vec<String> = follows_first.iter().map(edge).collect();
                shares.sort_unstable();
                format!(
                    "{}\t{node}\t{}\t{}\t{}",
                    self.name(),
                    into.0,
                    into.1,
                    shares.join(" ")
                )
            }
            Self::MergeNodes {
                first,
                second,
                into,
            } => format!("{}\t{first}\t{second}\t{into}", self.name()),
            Self::SetRole { node, from, to } => {
                format!(
                    "{}\t{node}\t{}\t{}",
                    self.name(),
                    slot(from.as_ref()),
                    slot(to.as_ref())
                )
            }
            Self::SetMode { from, to } => {
                format!("{}\t{}\t{}", self.name(), from.slug(), to.slug())
            }
            Self::SetCoordinator { from, to } => format!(
                "{}\t{}\t{}",
                self.name(),
                node_slot(from.as_ref()),
                node_slot(to.as_ref())
            ),
        }
    }

    /// Relire une opération depuis sa forme canonique.
    ///
    /// # Pourquoi la forme canonique est aussi la forme de transport
    ///
    /// `W17.i` doit écrire une opération dans le journal, et le journal est la vérité
    /// institutionnelle : ce qu'on y met se relit dans dix ans. Deux formes étaient possibles — une
    /// sérialisation dérivée, ou celle-ci. Celle-ci gagne pour une raison qui n'est pas
    /// l'économie : les octets écrits sont **exactement** ceux sur lesquels le condensat a été
    /// calculé. Un lecteur qui relit un événement relit ce qui a été signé, et non une seconde
    /// représentation dont il faudrait prouver qu'elle dit la même chose.
    ///
    /// Ce n'était pas vrai avant le durcissement contre l'injection. Un rôle portant une tabulation
    /// forgeait un champ, et un rôle nommé `-` était indistinguable d'une absence : la forme
    /// canonique n'était pas analysable sans ambiguïté. Elle l'est devenue en refusant ces deux
    /// entrées, et cette fonction en est le bénéficiaire direct.
    ///
    /// # Errors
    ///
    /// [`ParseOperationError`] — le nom, l'arité ou un champ. Le refus dit **lequel**, parce qu'un
    /// événement illisible dont on ne sait pas quelle moitié est fautive ne se répare pas.
    pub fn parse(canonical: &str) -> Result<Self, ParseOperationError> {
        let mut fields = canonical.split('\t');
        let name = fields.next().ok_or(ParseOperationError::Empty)?;
        let rest: Vec<&str> = fields.collect();
        let arity = |expected: usize| {
            if rest.len() == expected {
                Ok(())
            } else {
                Err(ParseOperationError::Arity {
                    operation: name.to_owned(),
                    expected,
                    found: rest.len(),
                })
            }
        };
        match name {
            "ADD_NODE" | "REMOVE_NODE" => {
                arity(1)?;
                let node = node(rest[0])?;
                Ok(if name == "ADD_NODE" {
                    Self::AddNode(node)
                } else {
                    Self::RemoveNode(node)
                })
            }
            "REPLACE_NODE" => {
                arity(2)?;
                Ok(Self::ReplaceNode {
                    from: node(rest[0])?,
                    to: node(rest[1])?,
                })
            }
            "ADD_EDGE" | "REMOVE_EDGE" => {
                arity(1)?;
                let relation = relation(rest[0])?;
                Ok(if name == "ADD_EDGE" {
                    Self::AddEdge(relation)
                } else {
                    Self::RemoveEdge(relation)
                })
            }
            "SPLIT_NODE" => {
                arity(4)?;
                let mut follows_first = BTreeSet::new();
                for edge in rest[3].split(' ').filter(|edge| !edge.is_empty()) {
                    follows_first.insert(relation(edge)?);
                }
                Ok(Self::SplitNode {
                    node: node(rest[0])?,
                    into: (node(rest[1])?, node(rest[2])?),
                    follows_first,
                })
            }
            "MERGE_NODES" => {
                arity(3)?;
                Ok(Self::MergeNodes {
                    first: node(rest[0])?,
                    second: node(rest[1])?,
                    into: node(rest[2])?,
                })
            }
            "SET_ROLE" => {
                arity(3)?;
                Ok(Self::SetRole {
                    node: node(rest[0])?,
                    from: role_slot(rest[1]),
                    to: role_slot(rest[2]),
                })
            }
            "SET_MODE" => {
                arity(2)?;
                Ok(Self::SetMode {
                    from: mode(rest[0])?,
                    to: mode(rest[1])?,
                })
            }
            "SET_COORDINATOR" => {
                arity(2)?;
                Ok(Self::SetCoordinator {
                    from: node_option(rest[0])?,
                    to: node_option(rest[1])?,
                })
            }
            other => Err(ParseOperationError::UnknownOperation {
                operation: other.to_owned(),
            }),
        }
    }

    /// Ce qui défait cette opération, quand quelque chose la défait.
    ///
    /// « Défaire » veut dire : appliqué à la version que celle-ci a produite, rendre le **contenu**
    /// d'avant. Pas la version d'avant — l'histoire ne se défait pas, et c'est [`Version`] qui le
    /// tient.
    ///
    /// La fusion rend [`Undo::Compensating`] parce qu'elle perd la partition : deux arêtes
    /// `X → premier` et `X → second` deviennent une seule, et aucune scission ne saurait dire
    /// laquelle était laquelle. Rendre ici une scission plausible ferait passer une compensation
    /// pour une annulation.
    #[must_use]
    pub fn undo(&self) -> Undo {
        match self {
            Self::AddNode(node) => Undo::Exact(Self::RemoveNode(*node)),
            Self::RemoveNode(node) => Undo::Exact(Self::AddNode(*node)),
            Self::ReplaceNode { from, to } => Undo::Exact(Self::ReplaceNode {
                from: *to,
                to: *from,
            }),
            Self::AddEdge(relation) => Undo::Exact(Self::RemoveEdge(*relation)),
            Self::RemoveEdge(relation) => Undo::Exact(Self::AddEdge(*relation)),
            Self::SplitNode { node, into, .. } => Undo::Exact(Self::MergeNodes {
                first: into.0,
                second: into.1,
                into: *node,
            }),
            Self::MergeNodes { .. } => Undo::Compensating,
            Self::SetRole { node, from, to } => Undo::Exact(Self::SetRole {
                node: *node,
                from: to.clone(),
                to: from.clone(),
            }),
            Self::SetMode { from, to } => Undo::Exact(Self::SetMode {
                from: *to,
                to: *from,
            }),
            Self::SetCoordinator { from, to } => Undo::Exact(Self::SetCoordinator {
                from: *to,
                to: *from,
            }),
        }
    }
}

impl fmt::Display for Operation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

/// Ce qui défait une opération, ou le fait qu'aucune ne la défasse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Undo {
    /// L'opération qui, appliquée ensuite, rend le contenu d'avant.
    Exact(Operation),
    /// Rien ne la défait : elle se **compense**, et la proposition le déclare.
    ///
    /// ADR 0016, décision 5. Ce n'est pas un manque : c'est le refus de rendre une opération
    /// plausible là où l'information nécessaire n'existe plus.
    Compensating,
}

impl Undo {
    /// L'opération inverse, quand il y en a une.
    ///
    /// Rend `None` pour une compensation, et c'est **le point** : il n'existe dans ce module aucune
    /// fonction qui rende une opération pour une fusion.
    #[must_use]
    pub const fn exact(&self) -> Option<&Operation> {
        match self {
            Self::Exact(operation) => Some(operation),
            Self::Compensating => None,
        }
    }
}

/// Une version canonique immuable.
///
/// Elle ne s'édite pas : [`Version::apply`] en rend une autre. Ce n'est pas une discipline d'appel,
/// il n'y a aucun accesseur mutable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    id: VersionId,
    parent: Option<VersionId>,
    members: BTreeSet<Id<Agent>>,
    relations: BTreeSet<Relation>,
    roles: BTreeMap<Id<Agent>, String>,
    mode: CoordinationMode,
    coordinator: Option<Id<Agent>>,
    content: ContentHash,
}

impl Version {
    /// La première version, sans parent.
    ///
    /// # Errors
    ///
    /// [`VersionError::DanglingEdge`] pour une relation dont une extrémité n'est pas membre, et
    /// [`VersionError::SelfRelation`] pour une relation d'un agent vers lui-même : sous la seule
    /// sorte qui existe, ce serait un agent qui se relit, ce que §14.4 et l'invariant 11 refusent.
    ///
    /// [`VersionError::NoMembers`], [`VersionError::CoordinatorNotAMember`] et
    /// [`VersionError::CoordinatorRequired`] sont les trois règles de §14.3, déménagées ici depuis
    /// `Team::new` par l'ADR 0021 — elles portent sur la structure, et la structure vit ici.
    pub fn root(
        members: &[Id<Agent>],
        relations: &[Relation],
        mode: CoordinationMode,
        coordinator: Option<Id<Agent>>,
        digest: &impl Digest,
    ) -> Result<Self, VersionError> {
        let members: BTreeSet<Id<Agent>> = members.iter().copied().collect();
        let relations: BTreeSet<Relation> = relations.iter().copied().collect();
        for relation in &relations {
            check_relation(relation, &members)?;
        }
        check_composition(&members, mode, coordinator.as_ref())?;
        Ok(Self::seal(
            None,
            members,
            relations,
            BTreeMap::new(),
            mode,
            coordinator,
            digest,
        ))
    }

    /// Le mode de coordination — §14.3.
    #[must_use]
    pub const fn mode(&self) -> CoordinationMode {
        self.mode
    }

    /// L'instance qui coordonne, quand le mode en nomme une.
    #[must_use]
    pub const fn coordinator(&self) -> Option<&Id<Agent>> {
        self.coordinator.as_ref()
    }

    /// Son identité.
    #[must_use]
    pub const fn id(&self) -> &VersionId {
        &self.id
    }

    /// La version dont elle descend.
    #[must_use]
    pub const fn parent(&self) -> Option<&VersionId> {
        self.parent.as_ref()
    }

    /// Le hash de ce qu'elle contient, indépendant de l'histoire.
    #[must_use]
    pub const fn content_hash(&self) -> &ContentHash {
        &self.content
    }

    /// Les instances d'agent.
    #[must_use]
    pub const fn members(&self) -> &BTreeSet<Id<Agent>> {
        &self.members
    }

    /// Les relations de coordination.
    #[must_use]
    pub const fn relations(&self) -> &BTreeSet<Relation> {
        &self.relations
    }

    /// Les rôles portés, par instance.
    ///
    /// La racine n'en porte aucun : un rôle arrive par `SET_ROLE`, qui est une ligne de diff comme
    /// une autre. Le poser à la racine le ferait entrer sans approbation.
    #[must_use]
    pub const fn roles(&self) -> &BTreeMap<Id<Agent>, String> {
        &self.roles
    }

    /// Le rôle d'une instance, s'il y en a un.
    #[must_use]
    pub fn role(&self, node: &Id<Agent>) -> Option<&str> {
        self.roles.get(node).map(String::as_str)
    }

    /// La forme canonique du contenu.
    ///
    /// Les lignes sont triées **en tant que chaînes**, pas dans l'ordre d'un conteneur : deux
    /// producteurs qui rempliraient leurs collections différemment doivent produire les mêmes
    /// octets, sinon le hash cesse de dire ce qu'il prétend dire.
    #[must_use]
    pub fn canonical(&self) -> String {
        content_canonical(
            &self.members,
            &self.relations,
            &self.roles,
            self.mode,
            self.coordinator.as_ref(),
        )
    }

    /// Appliquer une opération, produisant la version suivante.
    ///
    /// # Errors
    ///
    /// Toutes les variantes de [`VersionError`]. Aucune n'est un détail : chacune nomme une chose
    /// que l'appelant doit écrire explicitement dans son diff plutôt que de la laisser arriver.
    pub fn apply(&self, operation: &Operation, digest: &impl Digest) -> Result<Self, VersionError> {
        let mut members = self.members.clone();
        let mut relations = self.relations.clone();
        let mut roles = self.roles.clone();
        let mut mode = self.mode;
        let mut coordinator = self.coordinator;
        match operation {
            Operation::AddNode(node) => add_node(&mut members, *node)?,
            Operation::RemoveNode(node) => remove_node(
                &mut members,
                &relations,
                &roles,
                coordinator.as_ref(),
                *node,
            )?,
            Operation::ReplaceNode { from, to } => {
                replace_node(&mut members, &mut relations, &mut roles, *from, *to)?;
                // Un remplacement est un isomorphisme : il emporte l'identité, donc la charge de
                // coordinateur avec elle. Rien ne s'y perd, et c'est ce qui le distingue d'un
                // retrait — refusé sur le coordinateur, parce que l'inverse ne saurait pas le rendre.
                if coordinator == Some(*from) {
                    coordinator = Some(*to);
                }
            }
            Operation::AddEdge(relation) => add_edge(&members, &mut relations, *relation)?,
            Operation::RemoveEdge(relation) => remove_edge(&mut relations, *relation)?,
            Operation::SplitNode {
                node,
                into,
                follows_first,
            } => split_node(
                &mut members,
                &mut relations,
                &roles,
                *node,
                *into,
                follows_first,
            )?,
            Operation::MergeNodes {
                first,
                second,
                into,
            } => merge_nodes(&mut members, &mut relations, &roles, *first, *second, *into)?,
            Operation::SetRole { node, from, to } => {
                set_role(&mut roles, &members, *node, from.as_deref(), to.as_deref())?;
            }
            Operation::SetMode { from, to } => set_mode(&mut mode, *from, *to)?,
            Operation::SetCoordinator { from, to } => {
                set_coordinator(&mut coordinator, &members, *from, *to)?;
            }
        }
        // Vérifié après, et pas seulement dans les deux opérations attributaires : `REMOVE_NODE`
        // et `SPLIT_NODE` changent l'appartenance, et §14.3 lie le mode au coordinateur. Une règle
        // vérifiée seulement là où on pense qu'elle peut casser est une règle qu'on croit tenir.
        check_composition(&members, mode, coordinator.as_ref())?;
        Ok(Self::seal(
            Some(self.id.clone()),
            members,
            relations,
            roles,
            mode,
            coordinator,
            digest,
        ))
    }

    fn seal(
        parent: Option<VersionId>,
        members: BTreeSet<Id<Agent>>,
        relations: BTreeSet<Relation>,
        roles: BTreeMap<Id<Agent>, String>,
        mode: CoordinationMode,
        coordinator: Option<Id<Agent>>,
        digest: &impl Digest,
    ) -> Self {
        let content = digest.digest(&content_canonical(
            &members,
            &relations,
            &roles,
            mode,
            coordinator.as_ref(),
        ));
        let id = VersionId(digest.digest(&identity_canonical(parent.as_ref(), &content)));
        Self {
            id,
            parent,
            members,
            relations,
            roles,
            mode,
            coordinator,
            content,
        }
    }
}

fn add_node(members: &mut BTreeSet<Id<Agent>>, node: Id<Agent>) -> Result<(), VersionError> {
    if members.insert(node) {
        return Ok(());
    }
    Err(VersionError::NodeAlreadyPresent {
        node: node.to_string(),
    })
}

/// Retirer un nœud, **sans cascade**.
///
/// Un nœud qui porte encore des arêtes n'est pas retiré : l'appelant les retire d'abord, une par
/// une, dans le diff. Emporter les arêtes avec le nœud ferait au commit ce que le diff ne montrait
/// pas, et l'approbation aurait porté sur autre chose que ce qui s'applique.
fn remove_node(
    members: &mut BTreeSet<Id<Agent>>,
    relations: &BTreeSet<Relation>,
    roles: &BTreeMap<Id<Agent>, String>,
    coordinator: Option<&Id<Agent>>,
    node: Id<Agent>,
) -> Result<(), VersionError> {
    if !members.contains(&node) {
        return Err(VersionError::NoSuchNode {
            node: node.to_string(),
        });
    }
    check_roleless(roles, &node)?;
    // Même raison que pour un rôle : la charge de coordinateur est une information que le nœud
    // emporte, et l'opération inverse — `ADD_NODE` — ne saurait pas la rendre. Le diff doit donc
    // écrire `SET_COORDINATOR` d'abord, ce qui le rend lisible par un approbateur.
    if coordinator == Some(&node) {
        return Err(VersionError::NodeIsCoordinator {
            node: node.to_string(),
        });
    }
    let attached = incident(relations, &node).count();
    if attached > 0 {
        return Err(VersionError::NodeStillConnected {
            node: node.to_string(),
            edges: attached,
        });
    }
    members.remove(&node);
    Ok(())
}

/// Le rôle **suit** l'identité, comme les arêtes.
///
/// C'est le seul des quatre à ne pas refuser un nœud qui porte un rôle, et l'asymétrie n'est pas un
/// oubli : un remplacement est un isomorphisme, son inverse est le remplacement opposé, et il rend
/// le rôle à `from` exactement. Retrait, scission et fusion perdent de l'information — le rôle
/// disparaîtrait sans que l'inverse sache le rendre.
fn replace_node(
    members: &mut BTreeSet<Id<Agent>>,
    relations: &mut BTreeSet<Relation>,
    roles: &mut BTreeMap<Id<Agent>, String>,
    from: Id<Agent>,
    to: Id<Agent>,
) -> Result<(), VersionError> {
    if from == to {
        return Err(VersionError::SameNode {
            node: from.to_string(),
        });
    }
    if !members.contains(&from) {
        return Err(VersionError::NoSuchNode {
            node: from.to_string(),
        });
    }
    if members.contains(&to) {
        return Err(VersionError::NodeAlreadyPresent {
            node: to.to_string(),
        });
    }
    members.remove(&from);
    members.insert(to);
    *relations = substitute(relations, &[(from, to)]);
    if let Some(role) = roles.remove(&from) {
        roles.insert(to, role);
    }
    Ok(())
}

fn add_edge(
    members: &BTreeSet<Id<Agent>>,
    relations: &mut BTreeSet<Relation>,
    relation: Relation,
) -> Result<(), VersionError> {
    check_relation(&relation, members)?;
    if relations.insert(relation) {
        return Ok(());
    }
    Err(VersionError::EdgeAlreadyPresent {
        edge: render(&relation),
    })
}

fn remove_edge(relations: &mut BTreeSet<Relation>, relation: Relation) -> Result<(), VersionError> {
    if relations.remove(&relation) {
        return Ok(());
    }
    Err(VersionError::NoSuchEdge {
        edge: render(&relation),
    })
}

/// Scinder un nœud en deux, selon la partition que l'opération **énonce**.
///
/// Les deux identités produites sont neuves. Laisser l'une d'elles reprendre celle du nœud scindé
/// ferait dire à la version qu'un agent a été scindé et qu'il a survécu inchangé, et rien en aval
/// ne saurait dire laquelle des deux moitiés est l'originale.
fn split_node(
    members: &mut BTreeSet<Id<Agent>>,
    relations: &mut BTreeSet<Relation>,
    roles: &BTreeMap<Id<Agent>, String>,
    node: Id<Agent>,
    into: (Id<Agent>, Id<Agent>),
    follows_first: &BTreeSet<Relation>,
) -> Result<(), VersionError> {
    let (first, second) = into;
    if first == second {
        return Err(VersionError::SameNode {
            node: first.to_string(),
        });
    }
    if !members.contains(&node) {
        return Err(VersionError::NoSuchNode {
            node: node.to_string(),
        });
    }
    // L'opération énonce la partition des **arêtes**, pas celle du rôle : laquelle des deux
    // moitiés le garde n'est écrit nulle part, et le dupliquer inventerait un second agent qui
    // porte le même rôle sans que personne l'ait demandé. L'appelant le retire d'abord, dans le
    // diff, comme il retire les arêtes.
    check_roleless(roles, &node)?;
    for produced in [first, second] {
        if members.contains(&produced) {
            return Err(VersionError::NodeAlreadyPresent {
                node: produced.to_string(),
            });
        }
    }
    for declared in follows_first {
        if declared.from != node && declared.to != node {
            return Err(VersionError::NotIncident {
                edge: render(declared),
                node: node.to_string(),
            });
        }
        if !relations.contains(declared) {
            return Err(VersionError::NoSuchEdge {
                edge: render(declared),
            });
        }
    }
    members.remove(&node);
    members.insert(first);
    members.insert(second);
    *relations = relations
        .iter()
        .map(|relation| {
            if relation.from != node && relation.to != node {
                return *relation;
            }
            let side = if follows_first.contains(relation) {
                first
            } else {
                second
            };
            rewrite(relation, &[(node, side)])
        })
        .collect();
    Ok(())
}

/// Fusionner deux nœuds en une identité neuve.
///
/// `into` doit être absent, symétriquement à la scission : si le fusionné reprenait l'identité de
/// l'un des deux, l'histoire ne distinguerait plus « ils ont fusionné » de « l'autre a été retiré ».
fn merge_nodes(
    members: &mut BTreeSet<Id<Agent>>,
    relations: &mut BTreeSet<Relation>,
    roles: &BTreeMap<Id<Agent>, String>,
    first: Id<Agent>,
    second: Id<Agent>,
    into: Id<Agent>,
) -> Result<(), VersionError> {
    if first == second {
        return Err(VersionError::SameNode {
            node: first.to_string(),
        });
    }
    for absorbed in [first, second] {
        if !members.contains(&absorbed) {
            return Err(VersionError::NoSuchNode {
                node: absorbed.to_string(),
            });
        }
        // Deux rôles pour une identité produite : en garder un serait choisir sans le dire, et le
        // refus est déjà la règle pour ce qu'une fusion perd.
        check_roleless(roles, &absorbed)?;
    }
    if members.contains(&into) {
        return Err(VersionError::NodeAlreadyPresent {
            node: into.to_string(),
        });
    }
    // Une relation qui joignait les deux fusionnés deviendrait une relation d'un agent vers
    // lui-même. Sous « review », ce serait un agent qui se relit, obtenu sans qu'aucune opération
    // ne l'ait demandé : la fusion est refusée, et l'appelant retire l'arête d'abord — dans le
    // diff, où l'approbateur la voit.
    let joins = |relation: &Relation, left: Id<Agent>, right: Id<Agent>| {
        relation.from == left && relation.to == right
    };
    for relation in relations.iter() {
        if joins(relation, first, second) || joins(relation, second, first) {
            return Err(VersionError::SelfRelation {
                node: into.to_string(),
            });
        }
    }
    members.remove(&first);
    members.remove(&second);
    members.insert(into);
    *relations = substitute(relations, &[(first, into), (second, into)]);
    Ok(())
}

/// La forme canonique d'un contenu — voir [`Version::canonical`].
/// Une version sans rôle produit les **mêmes octets** qu'avant `SET_ROLE` : la table vide
/// n'ajoute aucune ligne. C'est ce qui rend la migration `[M]` sans effet sur l'existant — seules
/// les versions qui usent de l'opération nouvelle ont une forme nouvelle, et elles ne pouvaient pas
/// exister avant.
fn content_canonical(
    members: &BTreeSet<Id<Agent>>,
    relations: &BTreeSet<Relation>,
    roles: &BTreeMap<Id<Agent>, String>,
    mode: CoordinationMode,
    coordinator: Option<&Id<Agent>>,
) -> String {
    let mut lines: Vec<String> =
        members
            .iter()
            .map(|member| format!("n\t{member}"))
            .chain(relations.iter().map(|relation| {
                format!("e\t{}\t{}\t{}", relation.from, relation.kind, relation.to)
            }))
            .chain(
                roles
                    .iter()
                    .map(|(node, role)| format!("r\t{node}\t{role}")),
            )
            .chain([
                format!("m\t{}", mode.slug()),
                format!(
                    "c\t{}",
                    coordinator.map_or(ABSENT.to_owned(), ToString::to_string)
                ),
            ])
            .collect();
    lines.sort_unstable();
    let mut canonical = String::from(CONTENT_MAGIC);
    for line in lines {
        canonical.push('\n');
        canonical.push_str(&line);
    }
    canonical.push('\n');
    canonical
}

/// La forme canonique de l'identité : le contenu **et** le parent.
fn identity_canonical(parent: Option<&VersionId>, content: &ContentHash) -> String {
    let parent = parent.map_or_else(|| "-".to_owned(), ToString::to_string);
    format!("{VERSION_MAGIC}\nparent\t{parent}\ncontent\t{content}\n")
}

/// Poser, changer ou retirer le rôle d'une instance.
///
/// `from` est vérifié contre ce que le nœud porte, et non ignoré : un diff calculé sur un état
/// périmé s'appliquerait sinon à autre chose que ce qu'il montrait à l'approbateur. C'est la même
/// exigence que la partition d'une scission, écrite au moment de l'application plutôt qu'à celui de
/// la lecture.
/// §14.3 : le mode est enregistré et comparé dans les benchmarks. Un changement sans effet est
/// refusé comme pour un rôle — une ligne de diff sans effet se lit comme un changement approuvé.
fn set_mode(
    mode: &mut CoordinationMode,
    from: CoordinationMode,
    to: CoordinationMode,
) -> Result<(), VersionError> {
    if *mode != from {
        return Err(VersionError::ModeMismatch {
            held: mode.slug().to_owned(),
            declared: from.slug().to_owned(),
        });
    }
    if from == to {
        return Err(VersionError::ModeUnchanged {
            mode: mode.slug().to_owned(),
        });
    }
    *mode = to;
    Ok(())
}

fn set_coordinator(
    coordinator: &mut Option<Id<Agent>>,
    members: &BTreeSet<Id<Agent>>,
    from: Option<Id<Agent>>,
    to: Option<Id<Agent>>,
) -> Result<(), VersionError> {
    if *coordinator != from {
        return Err(VersionError::CoordinatorMismatch {
            held: node_slot(coordinator.as_ref()),
            declared: node_slot(from.as_ref()),
        });
    }
    if from == to {
        return Err(VersionError::CoordinatorUnchanged {
            coordinator: node_slot(coordinator.as_ref()),
        });
    }
    if let Some(node) = to
        && !members.contains(&node)
    {
        return Err(VersionError::CoordinatorNotAMember {
            coordinator: node.to_string(),
        });
    }
    *coordinator = to;
    Ok(())
}

/// Les trois règles de §14.3, qu'aucune opération ne détient à elle seule.
///
/// Vérifiées après **chaque** application, et pas seulement dans les deux opérations attributaires :
/// `REMOVE_NODE`, `SPLIT_NODE` et `MERGE_NODES` changent l'appartenance, donc peuvent défaire « le
/// coordinateur est membre » sans rien savoir du coordinateur. Une règle vérifiée seulement là où
/// l'on pense qu'elle peut casser est une règle qu'on croit tenir.
fn check_composition(
    members: &BTreeSet<Id<Agent>>,
    mode: CoordinationMode,
    coordinator: Option<&Id<Agent>>,
) -> Result<(), VersionError> {
    if members.is_empty() {
        return Err(VersionError::NoMembers);
    }
    if let Some(node) = coordinator
        && !members.contains(node)
    {
        return Err(VersionError::CoordinatorNotAMember {
            coordinator: node.to_string(),
        });
    }
    if mode == CoordinationMode::Coordinator && coordinator.is_none() {
        return Err(VersionError::CoordinatorRequired);
    }
    Ok(())
}

fn set_role(
    roles: &mut BTreeMap<Id<Agent>, String>,
    members: &BTreeSet<Id<Agent>>,
    node: Id<Agent>,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<(), VersionError> {
    if !members.contains(&node) {
        return Err(VersionError::NoSuchNode {
            node: node.to_string(),
        });
    }
    if to.is_some_and(|role| role.trim().is_empty()) {
        return Err(VersionError::EmptyRole {
            node: node.to_string(),
        });
    }
    // Un rôle est le **seul champ de texte libre** qui entre dans une forme canonique. Les deux
    // refus qui suivent ferment les deux façons dont il peut en forger une — voir `RoleForgesALine`
    // et `RoleLooksLikeAbsence`, où le raisonnement est écrit.
    if to.is_some_and(|role| role.chars().any(char::is_control)) {
        return Err(VersionError::RoleForgesALine {
            node: node.to_string(),
        });
    }
    if to == Some(ABSENT) {
        return Err(VersionError::RoleLooksLikeAbsence {
            node: node.to_string(),
        });
    }
    let held = roles.get(&node).map(String::as_str);
    if held != from {
        return Err(VersionError::RoleMismatch {
            node: node.to_string(),
            held: held.map(ToOwned::to_owned),
            declared: from.map(ToOwned::to_owned),
        });
    }
    if held == to {
        return Err(VersionError::RoleUnchanged {
            node: node.to_string(),
        });
    }
    match to {
        Some(role) => roles.insert(node, role.to_owned()),
        None => roles.remove(&node),
    };
    Ok(())
}

/// Un rôle, ou son absence, dans un message d'erreur.
fn role_or_none(role: Option<&str>) -> String {
    role.map_or_else(|| "aucun rôle".to_owned(), |role| format!("« {role} »"))
}

/// Refuser une opération qui perdrait un rôle sans que son inverse sache le rendre.
fn check_roleless(
    roles: &BTreeMap<Id<Agent>, String>,
    node: &Id<Agent>,
) -> Result<(), VersionError> {
    match roles.get(node) {
        Some(role) => Err(VersionError::NodeStillHasRole {
            node: node.to_string(),
            role: role.clone(),
        }),
        None => Ok(()),
    }
}

/// Un champ facultatif dans la forme canonique d'une opération : `-` pour l'absence.
///
/// Un rôle vide est refusé par [`set_role`], donc `-` ne peut pas se confondre avec un rôle réel.
fn slot(value: Option<&String>) -> &str {
    value.map_or(ABSENT, String::as_str)
}

/// La même marque d'absence, pour un identifiant.
///
/// Un `Id` n'a pas la faille que `slot` doit surveiller pour un rôle : sa forme textuelle est
/// contrainte, ne vaut jamais `-` et ne porte aucun caractère de contrôle. Il n'y a donc rien à
/// refuser ici, et la dissymétrie tient au texte libre, pas à la fonction.
fn node(text: &str) -> Result<Id<Agent>, ParseOperationError> {
    Id::parse(text).map_err(|_| ParseOperationError::Field {
        field: "nœud".to_owned(),
        value: text.to_owned(),
    })
}

/// `-` est l'absence, et **jamais** un identifiant : `set_role` et `set_coordinator` refusent une
/// valeur égale à la sentinelle, ce qui rend cette lecture non ambiguë plutôt que probable.
fn node_option(text: &str) -> Result<Option<Id<Agent>>, ParseOperationError> {
    if text == ABSENT {
        return Ok(None);
    }
    node(text).map(Some)
}

fn role_slot(text: &str) -> Option<String> {
    if text == ABSENT {
        return None;
    }
    Some(text.to_owned())
}

fn mode(text: &str) -> Result<CoordinationMode, ParseOperationError> {
    CoordinationMode::parse(text).ok_or_else(|| ParseOperationError::Field {
        field: "mode".to_owned(),
        value: text.to_owned(),
    })
}

/// Une arête s'écrit `<from>><kind>><to>` — sans tabulation, pour tenir dans un champ.
fn relation(text: &str) -> Result<Relation, ParseOperationError> {
    let parts: Vec<&str> = text.split('>').collect();
    let [from, kind, to] = parts.as_slice() else {
        return Err(ParseOperationError::Field {
            field: "arête".to_owned(),
            value: text.to_owned(),
        });
    };
    Ok(Relation {
        from: node(from)?,
        to: node(to)?,
        kind: RelationKind::parse(kind).ok_or_else(|| ParseOperationError::Field {
            field: "sorte de relation".to_owned(),
            value: (*kind).to_owned(),
        })?,
    })
}

/// Pourquoi une opération ne se relit pas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseOperationError {
    /// Une chaîne vide.
    Empty,
    /// Un nom d'opération que ce jeu ne contient pas.
    UnknownOperation {
        /// Ce qui a été lu.
        operation: String,
    },
    /// Le bon nom, le mauvais nombre de champs.
    Arity {
        /// L'opération.
        operation: String,
        /// Ce qu'elle attend.
        expected: usize,
        /// Ce qui a été trouvé.
        found: usize,
    },
    /// Un champ illisible, nommé.
    Field {
        /// Lequel.
        field: String,
        /// Sa valeur.
        value: String,
    },
}

impl fmt::Display for ParseOperationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("opération vide"),
            Self::UnknownOperation { operation } => {
                write!(
                    formatter,
                    "« {operation} » n'est pas une opération de ce jeu"
                )
            }
            Self::Arity {
                operation,
                expected,
                found,
            } => write!(
                formatter,
                "« {operation} » attend {expected} champ(s) et en a {found}"
            ),
            Self::Field { field, value } => {
                write!(formatter, "{field} illisible : « {value} »")
            }
        }
    }
}

impl std::error::Error for ParseOperationError {}

fn node_slot(value: Option<&Id<Agent>>) -> String {
    value.map_or_else(|| ABSENT.to_owned(), ToString::to_string)
}

fn render(relation: &Relation) -> String {
    format!("{} -{}-> {}", relation.from, relation.kind, relation.to)
}

/// Une arête dans une forme canonique — sans tabulation, pour tenir dans un champ.
fn edge(relation: &Relation) -> String {
    format!("{}>{}>{}", relation.from, relation.kind, relation.to)
}

fn incident<'a>(
    relations: &'a BTreeSet<Relation>,
    node: &'a Id<Agent>,
) -> impl Iterator<Item = &'a Relation> {
    relations
        .iter()
        .filter(move |relation| relation.from == *node || relation.to == *node)
}

fn check_relation(relation: &Relation, members: &BTreeSet<Id<Agent>>) -> Result<(), VersionError> {
    if relation.from == relation.to {
        return Err(VersionError::SelfRelation {
            node: relation.from.to_string(),
        });
    }
    for endpoint in [relation.from, relation.to] {
        if !members.contains(&endpoint) {
            return Err(VersionError::DanglingEdge {
                edge: render(relation),
                endpoint: endpoint.to_string(),
            });
        }
    }
    Ok(())
}

fn rewrite(relation: &Relation, substitutions: &[(Id<Agent>, Id<Agent>)]) -> Relation {
    let map = |node: Id<Agent>| {
        substitutions
            .iter()
            .find(|(from, _)| *from == node)
            .map_or(node, |(_, to)| *to)
    };
    Relation {
        from: map(relation.from),
        to: map(relation.to),
        kind: relation.kind,
    }
}

/// Réécrire toutes les arêtes. Le résultat est un ensemble : deux arêtes qui deviennent égales
/// n'en font qu'une, et c'est exactement l'information qu'une fusion perd.
fn substitute(
    relations: &BTreeSet<Relation>,
    substitutions: &[(Id<Agent>, Id<Agent>)],
) -> BTreeSet<Relation> {
    relations
        .iter()
        .map(|relation| rewrite(relation, substitutions))
        .collect()
}

/// Ce qui empêche une opération de s'appliquer.
///
/// Chaque variante nomme une chose que l'appelant doit écrire dans son diff. Un refus générique
/// laisserait croire que l'opération était mal formée alors qu'elle était incomplète.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionError {
    /// Ce nœud est déjà là.
    NodeAlreadyPresent {
        /// Lequel.
        node: String,
    },
    /// Ce nœud n'y est pas.
    NoSuchNode {
        /// Lequel.
        node: String,
    },
    /// Ce nœud porte encore des arêtes. **Aucune cascade** : elles se retirent avant.
    NodeStillConnected {
        /// Lequel.
        node: String,
        /// Combien d'arêtes.
        edges: usize,
    },
    /// Ce nœud porte encore un rôle. Le retirer d'abord, dans le diff.
    NodeStillHasRole {
        /// Lequel.
        node: String,
        /// Le rôle qu'il porte.
        role: String,
    },
    /// Le rôle déclaré comme « celui d'aujourd'hui » n'est pas celui que le nœud porte.
    RoleMismatch {
        /// Lequel.
        node: String,
        /// Ce qu'il porte réellement.
        held: Option<String>,
        /// Ce que l'opération déclarait qu'il portait.
        declared: Option<String>,
    },
    /// L'opération ne change rien : `from` et `to` sont le même rôle.
    RoleUnchanged {
        /// Lequel.
        node: String,
    },
    /// Ce nœud coordonne — le retirer perdrait une information que son inverse ne rendrait pas.
    NodeIsCoordinator {
        /// Lequel.
        node: String,
    },
    /// Une version sans membre — §14.3 : une équipe sans membre ne coordonne rien.
    ///
    /// Déménagée depuis `Team::new` par l'ADR 0021. Vérifiée sur **toute** version, pas seulement
    /// la racine : retirer le dernier membre est refusé, et c'est voulu — une équipe qu'on vide
    /// s'archive, elle ne se réduit pas silencieusement à un graphe vide qui se dit encore une
    /// équipe.
    NoMembers,
    /// Le coordinateur désigné n'est pas membre — §14.3.
    CoordinatorNotAMember {
        /// Lequel.
        coordinator: String,
    },
    /// Le mode `coordinator` sans personne pour coordonner — §14.3 en fait la définition du mode.
    CoordinatorRequired,
    /// Le mode d'avant déclaré ne correspond pas à celui que la version porte.
    ModeMismatch {
        /// Celui qu'elle porte.
        held: String,
        /// Celui que l'opération déclare.
        declared: String,
    },
    /// L'opération ne change rien : `from` et `to` sont le même mode.
    ModeUnchanged {
        /// Lequel.
        mode: String,
    },
    /// Le coordinateur d'avant déclaré ne correspond pas à celui que la version porte.
    CoordinatorMismatch {
        /// Celui qu'elle porte.
        held: String,
        /// Celui que l'opération déclare.
        declared: String,
    },
    /// L'opération ne change rien : `from` et `to` sont le même coordinateur.
    CoordinatorUnchanged {
        /// Lequel.
        coordinator: String,
    },
    /// Un rôle vide ou blanc — indistinguable d'une absence pour tout lecteur.
    EmptyRole {
        /// Lequel.
        node: String,
    },
    /// Un rôle qui contient un caractère de contrôle — il **forge une ligne** de la forme canonique.
    ///
    /// La forme canonique d'un contenu est un texte à lignes, `n\t…`, `e\t…`, `r\t…`, trié. Un rôle
    /// est le seul champ de texte libre qui y entre, et une tabulation ou une fin de ligne dedans y
    /// insère une ligne que personne n'a écrite. Vérifié plutôt que supposé : deux contenus
    /// réellement différents — un agent portant un rôle forgé, et deux agents portant deux rôles —
    /// produisaient les **mêmes octets**, donc le même `content_hash`. Le même défaut valait pour la
    /// forme canonique d'un diff, sur laquelle porte une approbation.
    ///
    /// Refusé plutôt qu'échappé, et le motif décide : échapper changerait la forme canonique de
    /// **toutes** les versions, donc tous les condensats déjà calculés, que §10.2 rend immuables.
    /// Refuser n'invalide rien de ce qui a été légitimement écrit.
    RoleForgesALine {
        /// Lequel.
        node: String,
    },
    /// Un rôle qui est exactement la marque d'absence.
    ///
    /// `SET_ROLE\t<nœud>\t-\t-` est ce qu'écrit « retirer le rôle ». Un rôle nommé `-` produirait la
    /// même ligne pour une opération qui **pose** un rôle, et une approbation ne saurait plus
    /// laquelle des deux elle couvre.
    RoleLooksLikeAbsence {
        /// Lequel.
        node: String,
    },
    /// Cette arête est déjà là.
    EdgeAlreadyPresent {
        /// Laquelle.
        edge: String,
    },
    /// Cette arête n'y est pas.
    NoSuchEdge {
        /// Laquelle.
        edge: String,
    },
    /// Une arête dont une extrémité n'est pas membre.
    DanglingEdge {
        /// Laquelle.
        edge: String,
        /// L'extrémité absente.
        endpoint: String,
    },
    /// Une relation d'un agent vers lui-même.
    SelfRelation {
        /// Lequel.
        node: String,
    },
    /// Une arête déclarée dans une scission sans toucher le nœud scindé.
    NotIncident {
        /// Laquelle.
        edge: String,
        /// Le nœud scindé.
        node: String,
    },
    /// Deux identités qui devaient différer.
    SameNode {
        /// Laquelle.
        node: String,
    },
}

impl fmt::Display for VersionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NodeAlreadyPresent { node } => write!(formatter, "{node} est déjà membre"),
            Self::NoSuchNode { node } => write!(formatter, "{node} n'est pas membre"),
            Self::NodeStillConnected { node, edges } => write!(
                formatter,
                "{node} porte encore {edges} arête(s) : les retirer d'abord, une par une — une \
                 cascade ferait au commit ce que le diff ne montrait pas"
            ),
            Self::NodeStillHasRole { node, role } => write!(
                formatter,
                "{node} porte encore le rôle « {role} » : le retirer d'abord, dans le diff — un \
                 retrait, une scission ou une fusion le perdrait, et l'opération inverse ne \
                 saurait pas le rendre"
            ),
            Self::RoleMismatch {
                node,
                held,
                declared,
            } => write!(
                formatter,
                "{node} porte {} et l'opération déclarait {} : le diff a été calculé sur un état \
                 périmé, et s'appliquerait à autre chose que ce qu'il montrait",
                role_or_none(held.as_deref()),
                role_or_none(declared.as_deref())
            ),
            Self::RoleUnchanged { node } => write!(
                formatter,
                "l'opération ne changerait rien au rôle de {node} : une ligne de diff sans effet \
                 se lit comme un changement approuvé"
            ),
            Self::NodeIsCoordinator { node } => write!(
                formatter,
                "{node} coordonne : le retirer perdrait une charge que son inverse ne rendrait \
                 pas — écrire SET_COORDINATOR d'abord rend le diff lisible"
            ),
            Self::NoMembers => formatter.write_str(
                "une version sans membre : §14.3 dit qu'une équipe sans membre ne coordonne rien",
            ),
            Self::CoordinatorNotAMember { coordinator } => write!(
                formatter,
                "{coordinator} coordonnerait une équipe dont il n'est pas membre"
            ),
            Self::CoordinatorRequired => formatter.write_str(
                "le mode « coordinator » sans coordinateur : §14.3 en fait la définition du mode, \
                 et l'omettre laisse une équipe qui se dit coordonnée sans que personne ne coordonne",
            ),
            Self::ModeMismatch { held, declared } => write!(
                formatter,
                "le mode est « {held} », l'opération en déclare « {declared} » : elle a été écrite \
                 sur un état périmé"
            ),
            Self::ModeUnchanged { mode } => write!(
                formatter,
                "l'opération ne changerait rien au mode « {mode} » : une ligne de diff sans effet \
                 se lit comme un changement approuvé"
            ),
            Self::CoordinatorMismatch { held, declared } => write!(
                formatter,
                "le coordinateur est « {held} », l'opération en déclare « {declared} » : elle a été \
                 écrite sur un état périmé"
            ),
            Self::CoordinatorUnchanged { coordinator } => write!(
                formatter,
                "l'opération ne changerait rien au coordinateur « {coordinator} »"
            ),
            Self::EmptyRole { node } => write!(
                formatter,
                "le rôle donné à {node} est vide : aucun lecteur ne le distinguerait d'une absence"
            ),
            Self::RoleForgesALine { node } => write!(
                formatter,
                "le rôle donné à {node} contient un caractère de contrôle : il forgerait une ligne \
                 de la forme canonique, et deux contenus différents auraient le même condensat"
            ),
            Self::RoleLooksLikeAbsence { node } => write!(
                formatter,
                "le rôle donné à {node} est « {ABSENT} », la marque d'un rôle absent : poser ce \
                 rôle et le retirer s'écriraient pareil"
            ),
            Self::EdgeAlreadyPresent { edge } => write!(formatter, "« {edge} » existe déjà"),
            Self::NoSuchEdge { edge } => write!(formatter, "« {edge} » n'existe pas"),
            Self::DanglingEdge { edge, endpoint } => {
                write!(formatter, "« {edge} » pend : {endpoint} n'est pas membre")
            }
            Self::SelfRelation { node } => write!(
                formatter,
                "{node} serait en relation avec lui-même : sous « review » un agent qui se relit, \
                 ce que §14.4 et l'invariant 11 refusent ; sous « visibility » une redondance, \
                 puisqu'un agent voit toujours ce qu'il a produit"
            ),
            Self::NotIncident { edge, node } => write!(
                formatter,
                "« {edge} » ne touche pas {node} : une scission ne partage que ses propres arêtes"
            ),
            Self::SameNode { node } => {
                write!(formatter, "{node} est donné deux fois pour deux identités")
            }
        }
    }
}

impl std::error::Error for VersionError {}
