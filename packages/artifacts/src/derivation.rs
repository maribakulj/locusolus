//! Les relations de dérivation d'un artefact — `docs/SPEC_V1.md` §19.2, sur le vocabulaire de §7.5.

use std::fmt;

use locus_domain::ContentHash;

/// Ce qu'un artefact doit à un autre.
///
/// # Pourquoi la relation est typée
///
/// W6.a ne gardait qu'une liste de hashes de parents, et croyait dire « §19.2 : par hash et non par
/// nom ». C'était vrai et insuffisant : `artifact-manifest.schema.json` porte, pour chaque parent,
/// une **relation** prise dans un sous-ensemble de §7.5. La différence n'est pas décorative —
/// `reproduces` est ce qu'une reproduction indépendante (§19.7, R4) inscrit, `supersedes` est ce
/// qu'une correction inscrit, et une liste de hashes nus les rend indistinguables. Un graphe qui ne
/// sait plus qui reproduit qui ne peut plus dire ce qui est reproduit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DerivationRelation {
    /// Produit à partir de.
    DerivedFrom,
    /// Produit par.
    ProducedBy,
    /// Consomme.
    Consumes,
    /// Remplace.
    Supersedes,
    /// Reproduit — §19.7, R4.
    Reproduces,
}

impl DerivationRelation {
    /// Les cinq, dans l'ordre du schéma.
    pub const ALL: [Self; 5] = [
        Self::DerivedFrom,
        Self::ProducedBy,
        Self::Consumes,
        Self::Supersedes,
        Self::Reproduces,
    ];

    /// Le nom que le schéma emploie.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::DerivedFrom => "derived_from",
            Self::ProducedBy => "produced_by",
            Self::Consumes => "consumes",
            Self::Supersedes => "supersedes",
            Self::Reproduces => "reproduces",
        }
    }

    /// Relire un nom de relation.
    ///
    /// `None` plutôt qu'un défaut : la relation généraliste `derived_from` avalerait un
    /// `reproduces` qu'on ne saurait plus retrouver, et une provenance qui se devine n'en est pas
    /// une.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.slug() == value)
    }
}

impl fmt::Display for DerivationRelation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Un parent, et ce qu'on lui doit.
///
/// L'identifiant **et** le hash : le schéma exige le premier et rend le second facultatif, et les
/// deux disent des choses différentes. L'identifiant désigne l'artefact comme objet du graphe, le
/// hash désigne le contenu exact dont on a dérivé. Un artefact peut être re-téléversé après
/// correction ; seul le hash dit lequel des deux contenus a servi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Derivation {
    artifact_id: String,
    content_hash: Option<ContentHash>,
    relation: DerivationRelation,
}

impl Derivation {
    /// Déclarer une dérivation.
    ///
    /// # Errors
    ///
    /// [`DerivationError::EmptyArtifactId`] : un parent sans identité ne désigne rien.
    pub fn new(
        artifact_id: &str,
        relation: DerivationRelation,
        content_hash: Option<ContentHash>,
    ) -> Result<Self, DerivationError> {
        if artifact_id.trim().is_empty() {
            return Err(DerivationError::EmptyArtifactId);
        }
        Ok(Self {
            artifact_id: artifact_id.to_owned(),
            content_hash,
            relation,
        })
    }

    /// L'identifiant du parent.
    #[must_use]
    pub fn artifact_id(&self) -> &str {
        &self.artifact_id
    }

    /// Le contenu exact dont on a dérivé, quand il est connu.
    #[must_use]
    pub const fn content_hash(&self) -> Option<&ContentHash> {
        self.content_hash.as_ref()
    }

    /// Ce qu'on lui doit.
    #[must_use]
    pub const fn relation(&self) -> DerivationRelation {
        self.relation
    }
}

/// Ce qui empêche une dérivation d'exister.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DerivationError {
    /// Un parent sans identifiant.
    EmptyArtifactId,
}

impl fmt::Display for DerivationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyArtifactId => {
                formatter.write_str("une dérivation sans identifiant de parent ne désigne rien")
            }
        }
    }
}

impl std::error::Error for DerivationError {}
