//! Ce qu'une exécution réserve — `docs/SPEC_V1.md` §12.2, §32.3, invariants 6 et 8.

use std::fmt;

/// Un accélérateur, quand la mission en demande un.
///
/// # Invariant 8 : le GPU est une capability, pas une dépendance globale
///
/// D'où l'`Option` dans [`ResourceSpec`] plutôt qu'un champ toujours présent avec un « aucun » : un
/// champ obligatoire ferait de l'accélérateur une dimension de **toute** exécution, et le premier
/// scheduler écrit dessus supposerait qu'il y en a partout un, fût-il vide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accelerator {
    /// Le genre demandé — `cuda`, `mps`, `rocm`… La liste appartient à la taxonomie des
    /// capabilities (§19.4), pas à ce type.
    pub kind: String,
    /// Combien.
    pub count: u32,
    /// La mémoire dédiée exigée, en octets.
    pub memory_bytes: u64,
}

/// Ce qu'une exécution réserve avant de commencer.
///
/// # Invariant 6 : rien n'est supposé illimité
///
/// « Les ressources sont réservées avant exécution ; elles ne sont pas supposées illimitées. » Il
/// n'existe donc **aucune** façon de construire un `ResourceSpec` qui laisse une borne non dite :
/// pas de `Default`, pas d'`Option` sur les quatre quotas, pas de variante `Unlimited`. Une borne
/// absente n'est pas une borne large — c'est une borne que personne n'a choisie, et c'est ce que le
/// premier dépassement révèle.
///
/// Les quatre quotas sont ceux que §32.3 exige de vérifier par self-tests : « quotas
/// CPU/RAM/PID/disque vérifiés par self-tests ». Le cinquième, le temps, est ici parce qu'une
/// exécution sans horizon consomme les quatre autres indéfiniment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceSpec {
    cpu_millis: u32,
    memory_bytes: u64,
    pids: u32,
    disk_bytes: u64,
    wall_clock_seconds: u32,
    accelerator: Option<Accelerator>,
}

impl ResourceSpec {
    /// Déclarer une réservation.
    ///
    /// # Errors
    ///
    /// [`ResourceError::Zero`] pour un quota nul : une exécution à zéro CPU, zéro processus ou zéro
    /// seconde ne démarre pas, et l'écrire est plus probablement un champ oublié qu'une intention.
    pub fn new(
        cpu_millis: u32,
        memory_bytes: u64,
        pids: u32,
        disk_bytes: u64,
        wall_clock_seconds: u32,
    ) -> Result<Self, ResourceError> {
        for (quota, value) in [
            ("cpu_millis", u64::from(cpu_millis)),
            ("memory_bytes", memory_bytes),
            ("pids", u64::from(pids)),
            ("wall_clock_seconds", u64::from(wall_clock_seconds)),
        ] {
            if value == 0 {
                return Err(ResourceError::Zero { quota });
            }
        }
        Ok(Self {
            cpu_millis,
            memory_bytes,
            pids,
            disk_bytes,
            wall_clock_seconds,
            accelerator: None,
        })
    }

    /// Exiger un accélérateur.
    ///
    /// # Errors
    ///
    /// [`ResourceError::Zero`] si le nombre demandé est nul, [`ResourceError::EmptyKind`] si le
    /// genre est vide — un accélérateur sans genre ne se place sur aucun worker.
    pub fn with_accelerator(mut self, accelerator: Accelerator) -> Result<Self, ResourceError> {
        if accelerator.kind.trim().is_empty() {
            return Err(ResourceError::EmptyKind);
        }
        if accelerator.count == 0 {
            return Err(ResourceError::Zero {
                quota: "accelerator.count",
            });
        }
        self.accelerator = Some(accelerator);
        Ok(self)
    }

    /// Le quota CPU, en millicœurs.
    #[must_use]
    pub const fn cpu_millis(&self) -> u32 {
        self.cpu_millis
    }

    /// Le quota mémoire, en octets.
    #[must_use]
    pub const fn memory_bytes(&self) -> u64 {
        self.memory_bytes
    }

    /// Le nombre maximal de processus.
    #[must_use]
    pub const fn pids(&self) -> u32 {
        self.pids
    }

    /// Le quota disque, en octets. Zéro est licite : une exécution peut n'avoir aucun droit
    /// d'écriture, ce qui est un choix et non un oubli.
    #[must_use]
    pub const fn disk_bytes(&self) -> u64 {
        self.disk_bytes
    }

    /// L'horizon, en secondes.
    #[must_use]
    pub const fn wall_clock_seconds(&self) -> u32 {
        self.wall_clock_seconds
    }

    /// L'accélérateur exigé, s'il y en a un.
    #[must_use]
    pub const fn accelerator(&self) -> Option<&Accelerator> {
        self.accelerator.as_ref()
    }

    /// Vrai quand cette réservation tient dans une capacité offerte.
    ///
    /// Sert au placement (§12.2). La comparaison est **par quota** et non globale : un worker qui
    /// offrirait beaucoup de mémoire et trop peu de PID ne convient pas, et un score agrégé le
    /// laisserait passer.
    #[must_use]
    pub fn fits_within(&self, capacity: &Self) -> bool {
        self.quotas_fit_within(capacity) && self.accelerator_fits_within(capacity)
    }

    /// Vrai quand les quatre quotas de §32.3 tiennent, **sans** regarder l'accélérateur.
    ///
    /// # Pourquoi la séparation
    ///
    /// Un accélérateur manquant faisait échouer `fits_within`, et un appelant qui listait les
    /// causes d'un refus le nommait deux fois : une fois « capacité dépassée », une fois
    /// « accélérateur absent ». Deux noms pour un seul fait, dont l'un est faux — les quatre quotas
    /// tenaient parfaitement. Le refus est plus utile quand chaque cause est dite une fois et sous
    /// le bon nom.
    #[must_use]
    pub const fn quotas_fit_within(&self, capacity: &Self) -> bool {
        self.cpu_millis <= capacity.cpu_millis
            && self.memory_bytes <= capacity.memory_bytes
            && self.pids <= capacity.pids
            && self.disk_bytes <= capacity.disk_bytes
    }

    /// Vrai quand l'accélérateur demandé, s'il y en a un, est offert en quantité suffisante.
    #[must_use]
    pub fn accelerator_fits_within(&self, capacity: &Self) -> bool {
        match (&self.accelerator, &capacity.accelerator) {
            (None, _) => true,
            (Some(_), None) => false,
            (Some(wanted), Some(offered)) => {
                wanted.kind == offered.kind
                    && wanted.count <= offered.count
                    && wanted.memory_bytes <= offered.memory_bytes
            }
        }
    }
}

/// Ce qui empêche une réservation d'exister.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceError {
    /// Un quota nul là où zéro n'a pas de sens.
    Zero {
        /// Lequel.
        quota: &'static str,
    },
    /// Un accélérateur sans genre.
    EmptyKind,
}

impl fmt::Display for ResourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero { quota } => write!(
                formatter,
                "le quota « {quota} » est nul : une exécution qui n'en a pas ne démarre pas"
            ),
            Self::EmptyKind => {
                formatter.write_str("un accélérateur sans genre ne se place sur aucun worker")
            }
        }
    }
}

impl std::error::Error for ResourceError {}
