//! Le diff comme objet de première classe — `docs/13` §3, `docs/SPEC_V1.md` §22.4.
//!
//! # La phrase qui décide de la forme
//!
//! `docs/10`, W17 : « diff calculé **une fois** côté serveur, donc identique dans Emacs et dans le
//! web — sinon l'approbation porte sur ce que chaque client a cru voir ».
//!
//! Un diff est donc une valeur qui circule, pas un calcul que chaque client refait. Il porte sa
//! base, ce qu'il produit, la suite d'opérations qui y mène, et une forme canonique par laquelle
//! deux lecteurs prouvent qu'ils regardent la même chose.
//!
//! # Une base est une version, une cible est un contenu
//!
//! L'asymétrie n'est pas un oubli, c'est la conséquence directe des deux hashes de
//! [`crate::version`]. Rejouer se fait sur **une histoire précise** : la base est donc une
//! [`VersionId`]. Ce que le rejeu produira n'a pas encore d'histoire — son identité dépend du rejeu
//! lui-même — donc la cible ne peut être qu'un [`ContentHash`]. Annoncer une cible sous forme
//! d'identité de version obligerait à la deviner avant de l'avoir produite, ce qui est exactement
//! l'erreur que W15.a évite.
//!
//! Ce que le diff garantit reste ce qu'on lui demande : **deux rejeux du même diff sur la même base
//! rendent la même version**, identité comprise. C'est cela, « identique dans Emacs et dans le web ».
//!
//! # Un diff n'invente pas d'intention
//!
//! [`Diff::between`] compare deux états et n'émet que les quatre opérations qui décrivent un écart :
//! retirer des arêtes, retirer des nœuds, ajouter des nœuds, ajouter des arêtes. Il n'infère jamais
//! un `REPLACE_NODE`, un `SPLIT_NODE` ni un `MERGE_NODES` : au niveau des états, un remplacement est
//! indiscernable d'un retrait suivi d'un ajout, et deviner ferait lire à l'approbateur une intention
//! que personne n'a écrite. C'est la même règle que §7.5 pose pour les relations — « ne doivent pas
//! être inférées en sens inverse ».
//!
//! Les opérations riches viennent de celui qui **propose**, par [`Diff::declaring`], et le diff les
//! garde telles quelles.
//!
//! # L'ordre est le diff
//!
//! Une version est un ensemble ; un diff est une **suite**. Sa forme canonique n'est donc pas triée,
//! parce que trier changerait ce qu'il fait : le refus de la cascade (W15.a) impose de retirer les
//! arêtes avant les nœuds, et la même liste dans un autre ordre ne s'applique pas.

use std::fmt;
use std::fmt::Write as _;

use locus_domain::ContentHash;

use crate::version::{Digest, Operation, Version, VersionError, VersionId};

/// La ligne d'en-tête de la forme canonique d'un diff.
const DIFF_MAGIC: &str = "coordination-diff/1";

/// Un écart entre deux versions, sous la forme qui s'applique.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diff {
    base: VersionId,
    target_content: ContentHash,
    operations: Vec<Operation>,
}

impl Diff {
    /// Le diff qui mène de `from` à `to`.
    ///
    /// L'ordre est imposé par le refus de la cascade : on retire les arêtes, puis les nœuds, puis on
    /// ajoute les nœuds, puis les arêtes. Aucun autre ordre ne s'applique — retirer un nœud dont une
    /// arête subsiste est refusé, et ajouter une arête dont une extrémité manque aussi.
    ///
    /// Le diff d'une version vers elle-même est **vide**, jamais absent : rendre `None` obligerait
    /// chaque appelant à écrire un cas particulier, et surtout un approbateur ne verrait *rien* au
    /// lieu de voir que rien ne change.
    #[must_use]
    pub fn between(from: &Version, to: &Version) -> Self {
        let mut operations = Vec::new();
        for relation in from.relations().difference(to.relations()) {
            operations.push(Operation::RemoveEdge(*relation));
        }
        for node in from.members().difference(to.members()) {
            operations.push(Operation::RemoveNode(*node));
        }
        for node in to.members().difference(from.members()) {
            operations.push(Operation::AddNode(*node));
        }
        for relation in to.relations().difference(from.relations()) {
            operations.push(Operation::AddEdge(*relation));
        }
        Self {
            base: from.id().clone(),
            target_content: to.content_hash().clone(),
            operations,
        }
    }

    /// Le diff qu'un proposeur écrit, avec les opérations qu'il a voulues.
    ///
    /// Contrairement à [`Diff::between`], les opérations sont données et non déduites : c'est par ce
    /// chemin qu'un `SPLIT_NODE` ou un `MERGE_NODES` survit jusqu'à l'approbateur. Ce que le
    /// constructeur calcule est ce que la suite **produit**, en la rejouant.
    ///
    /// # Errors
    ///
    /// [`DiffError::Inapplicable`] quand une opération ne s'applique pas sur l'état où la suite la
    /// mène, avec sa **position** : une suite qui échoue au milieu sans dire où oblige à tout
    /// relire.
    pub fn declaring(
        base: &Version,
        operations: Vec<Operation>,
        digest: &impl Digest,
    ) -> Result<Self, DiffError> {
        let produced = replay_onto(base, &operations, digest)?;
        Ok(Self {
            base: base.id().clone(),
            target_content: produced.content_hash().clone(),
            operations,
        })
    }

    /// Lire un diff reçu d'ailleurs — §22.4 le sert sur `/branches/:id/diff`.
    ///
    /// Rien n'est vérifié ici, et ce n'est pas un oubli : sans la base sous la main, il n'y a rien à
    /// confronter. C'est [`Diff::replay`] qui confronte, et il le fait sur ce que la suite produit
    /// réellement plutôt que sur ce que le document annonce.
    #[must_use]
    pub const fn from_wire(
        base: VersionId,
        target_content: ContentHash,
        operations: Vec<Operation>,
    ) -> Self {
        Self {
            base,
            target_content,
            operations,
        }
    }

    /// La version sur laquelle il s'applique.
    #[must_use]
    pub const fn base(&self) -> &VersionId {
        &self.base
    }

    /// Le contenu qu'il annonce produire.
    #[must_use]
    pub const fn target_content(&self) -> &ContentHash {
        &self.target_content
    }

    /// Les opérations, **dans leur ordre**.
    #[must_use]
    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }

    /// Vrai quand il ne change rien.
    ///
    /// « Vide » est un état du diff, pas son absence : un approbateur doit pouvoir lire qu'une
    /// proposition ne change rien, ce qui est une information et souvent une surprise.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    /// Sa forme canonique.
    ///
    /// **Non triée**, contrairement à celle d'une version : un diff est une suite, et la même liste
    /// dans un autre ordre ne s'applique pas. Trier ici ferait signer deux clients sur un document
    /// qui ne décrit pas ce qui sera commité.
    #[must_use]
    pub fn canonical(&self) -> String {
        let mut canonical = format!(
            "{DIFF_MAGIC}\nbase\t{}\ntarget\t{}\n",
            self.base, self.target_content
        );
        for (position, operation) in self.operations.iter().enumerate() {
            let _ = writeln!(canonical, "op\t{position}\t{}", operation.canonical());
        }
        canonical
    }

    /// Rejouer le diff sur sa base.
    ///
    /// # Errors
    ///
    /// [`DiffError::Stale`] quand `base` n'est pas la version sur laquelle le diff a été écrit — le
    /// refus dit alors qu'il faut rebaser, parce qu'un « conflit » sans consigne fait retenter à
    /// l'identique ; [`DiffError::Inapplicable`] quand une opération ne passe pas, avec sa position ;
    /// [`DiffError::ContentMismatch`] quand la suite ne produit pas ce que le diff annonçait.
    ///
    /// # Ce qui prouve n'est pas ce qui est annoncé
    ///
    /// La vérification finale porte sur le contenu **produit**, jamais sur celui que le document
    /// déclare. Un diff venu d'ailleurs qui annonce une cible flatteuse est refusé par le rejeu, pas
    /// cru sur parole.
    pub fn replay(&self, base: &Version, digest: &impl Digest) -> Result<Version, DiffError> {
        if *base.id() != self.base {
            return Err(DiffError::Stale {
                expected: self.base.clone(),
                actual: base.id().clone(),
            });
        }
        let produced = replay_onto(base, &self.operations, digest)?;
        if *produced.content_hash() != self.target_content {
            return Err(DiffError::ContentMismatch {
                announced: self.target_content.clone(),
                produced: produced.content_hash().clone(),
            });
        }
        Ok(produced)
    }
}

/// Appliquer une suite, en nommant la position de celle qui échoue.
///
/// Une suite vide rend la base **elle-même**. Produire une nouvelle version au contenu identique
/// inscrirait qu'il s'est passé quelque chose là où il ne s'est rien passé — le pendant exact de la
/// cascade, qui inscrit moins que ce qui arrive.
fn replay_onto(
    base: &Version,
    operations: &[Operation],
    digest: &impl Digest,
) -> Result<Version, DiffError> {
    let mut current = base.clone();
    for (position, operation) in operations.iter().enumerate() {
        current = current
            .apply(operation, digest)
            .map_err(|because| DiffError::Inapplicable {
                position,
                operation: operation.canonical(),
                because,
            })?;
    }
    Ok(current)
}

/// Ce qui empêche un diff de se rejouer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffError {
    /// Le diff a été écrit sur une autre version.
    Stale {
        /// Celle sur laquelle il a été écrit.
        expected: VersionId,
        /// Celle qu'on lui présente.
        actual: VersionId,
    },
    /// Une opération de la suite ne s'applique pas.
    Inapplicable {
        /// Sa position dans la suite.
        position: usize,
        /// Sa forme canonique.
        operation: String,
        /// Ce que la version en a dit.
        because: VersionError,
    },
    /// La suite ne produit pas ce que le diff annonçait.
    ContentMismatch {
        /// Ce que le document déclarait.
        announced: ContentHash,
        /// Ce que le rejeu a produit.
        produced: ContentHash,
    },
}

impl DiffError {
    /// Vrai quand l'appelant doit rebaser avant de retenter.
    ///
    /// La consigne fait partie du refus, comme pour `ProposalError::Stale` : sans elle, un appelant
    /// retenterait à l'identique jusqu'à ce que quelqu'un lise le code.
    #[must_use]
    pub const fn needs_rebase(&self) -> bool {
        matches!(self, Self::Stale { .. })
    }
}

impl fmt::Display for DiffError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stale { expected, actual } => write!(
                formatter,
                "diff écrit sur {expected}, la version présentée est {actual} : rebaser puis \
                 retenter"
            ),
            Self::Inapplicable {
                position,
                operation,
                because,
            } => write!(
                formatter,
                "opération {position} « {operation} » ne s'applique pas : {because}"
            ),
            Self::ContentMismatch {
                announced,
                produced,
            } => write!(
                formatter,
                "le diff annonçait {announced} et produit {produced} : ce qui prouve est ce que le \
                 rejeu produit, jamais ce que le document déclare"
            ),
        }
    }
}

impl std::error::Error for DiffError {}
