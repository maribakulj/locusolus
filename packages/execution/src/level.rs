//! Les niveaux d'isolation — `docs/SPEC_V1.md` §21.6.

use std::fmt;

/// Un niveau de sandbox.
///
/// # L'ordre est significatif, et c'est délibéré
///
/// `locus_domain::ValidationLevel` refuse `Ord` : rien ne dit qu'une preuve formelle vaut « plus »
/// qu'une reproduction indépendante, et les ranger sur une échelle ferait dire au type ce que la
/// spécification ne dit pas. Ici c'est l'inverse : §21.6 énumère les niveaux comme une **échelle de
/// confinement**, et la règle « un downgrade est interdit » n'a de sens que si l'on peut comparer.
///
/// La distinction a été reprise plutôt que copiée. Si `S5` se révélait ne pas dominer `S4` — un
/// enclave distant n'est pas une micro-VM locale sous tous les aspects — la comparaison devrait
/// devenir un ordre partiel, et le test qui transcrit l'échelle serait le premier à le dire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SandboxLevel {
    /// `S0 unsandboxed-explicit` — aucun confinement, et il faut le demander.
    S0,
    /// `S1 os-write-contained` — les écritures sont contenues par l'OS.
    S1,
    /// `S2 container-rootless` — conteneur sans privilèges.
    S2,
    /// `S3 container-isolated-network` — conteneur au réseau isolé.
    S3,
    /// `S4 microvm-high-risk` — micro-VM pour ce qui est à haut risque.
    S4,
    /// `S5 remote-trusted-enclave-or-equivalent` — enclave distante de confiance ou équivalent.
    S5,
}

impl SandboxLevel {
    /// Les six, du moins confiné au plus confiné, dans l'ordre de §21.6.
    pub const ALL: [Self; 6] = [Self::S0, Self::S1, Self::S2, Self::S3, Self::S4, Self::S5];

    /// Le nom du niveau, tel que §21.6 l'écrit.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::S0 => "unsandboxed-explicit",
            Self::S1 => "os-write-contained",
            Self::S2 => "container-rootless",
            Self::S3 => "container-isolated-network",
            Self::S4 => "microvm-high-risk",
            Self::S5 => "remote-trusted-enclave-or-equivalent",
        }
    }

    /// Le code court, `S0` à `S5`.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::S0 => "S0",
            Self::S1 => "S1",
            Self::S2 => "S2",
            Self::S3 => "S3",
            Self::S4 => "S4",
            Self::S5 => "S5",
        }
    }

    /// Relire un code ou un nom.
    ///
    /// Rend `None` plutôt qu'un niveau par défaut : un niveau inconnu traité comme `S0` ouvrirait
    /// la sandbox, et traité comme `S5` masquerait une configuration fausse en la rendant
    /// inoffensive. Les deux sont pires que l'aveu.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|level| level.code() == value || level.slug() == value)
    }

    /// Vrai quand ce niveau confine au moins autant que celui exigé.
    #[must_use]
    pub fn satisfies(self, required: Self) -> bool {
        self >= required
    }
}

impl fmt::Display for SandboxLevel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} {}", self.code(), self.slug())
    }
}

/// Un profil de sandbox de la V1 — §21.6.
///
/// # Pourquoi les profils ne portent pas de niveau
///
/// §21.6 énumère les sept profils **sans** dire à quel niveau chacun s'exécute. Leur en attribuer
/// un ici serait inventer une politique de sécurité dans un type, à l'endroit exact où personne ne
/// viendrait la relire. La correspondance profil → niveau appartient à la politique (§20) et se
/// décidera en W4.g ; ce type existe pour que le vocabulaire soit fixe et ne dérive pas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SandboxProfile {
    /// `interactive-local`.
    InteractiveLocal,
    /// `readonly-review`.
    ReadonlyReview,
    /// `network-allowlisted`.
    NetworkAllowlisted,
    /// `math-compute`.
    MathCompute,
    /// `dh-corpus`.
    DhCorpus,
    /// `untrusted-repository`.
    UntrustedRepository,
    /// `microvm-high-risk`.
    MicrovmHighRisk,
}

impl SandboxProfile {
    /// Les sept, dans l'ordre de §21.6.
    pub const ALL: [Self; 7] = [
        Self::InteractiveLocal,
        Self::ReadonlyReview,
        Self::NetworkAllowlisted,
        Self::MathCompute,
        Self::DhCorpus,
        Self::UntrustedRepository,
        Self::MicrovmHighRisk,
    ];

    /// Le nom du profil, tel que §21.6 l'écrit.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::InteractiveLocal => "interactive-local",
            Self::ReadonlyReview => "readonly-review",
            Self::NetworkAllowlisted => "network-allowlisted",
            Self::MathCompute => "math-compute",
            Self::DhCorpus => "dh-corpus",
            Self::UntrustedRepository => "untrusted-repository",
            Self::MicrovmHighRisk => "microvm-high-risk",
        }
    }

    /// Relire un nom.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|profile| profile.slug() == value)
    }
}

impl fmt::Display for SandboxProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}
