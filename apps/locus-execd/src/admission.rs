//! L'admission : décider **avant** d'exécuter — ADR 0004, `docs/SPEC_V1.md` §12.2, §21.6.

use std::fmt;

use locus_execution::{ResourceSpec, SandboxLevel, SandboxSpec};

/// Ce que cet hôte sait offrir.
///
/// Déclaré, et non découvert en essayant. §12.2 place les capabilities et le fit parmi les critères
/// de placement : un broker qui apprendrait ses limites en échouant les découvrirait **après** avoir
/// créé la moitié d'une sandbox, et laisserait derrière lui ce qu'il avait déjà créé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCapabilities {
    best_level: SandboxLevel,
    capacity: ResourceSpec,
    network_modes: Vec<&'static str>,
}

impl HostCapabilities {
    /// Déclarer ce que l'hôte offre.
    ///
    /// L'accélérateur offert, s'il y en a un, vit dans `capacity` : c'est une ressource comme les
    /// autres, et lui donner une seconde déclaration à côté ferait deux endroits à tenir d'accord.
    #[must_use]
    pub fn new(
        best_level: SandboxLevel,
        capacity: ResourceSpec,
        network_modes: Vec<&'static str>,
    ) -> Self {
        Self {
            best_level,
            capacity,
            network_modes,
        }
    }

    /// Le meilleur confinement que cet hôte sait appliquer.
    #[must_use]
    pub const fn best_level(&self) -> SandboxLevel {
        self.best_level
    }

    /// Ce qu'il peut réserver.
    #[must_use]
    pub const fn capacity(&self) -> &ResourceSpec {
        &self.capacity
    }
}

/// Pourquoi une mission est refusée.
///
/// Un code par condition manquante, et jamais une seule à la fois — voir [`admit`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefusalReason {
    /// L'hôte ne sait pas confiner aussi fort que la mission l'exige.
    LevelUnavailable {
        /// Ce que la mission exige.
        required: SandboxLevel,
        /// Le mieux que l'hôte sache faire.
        best: SandboxLevel,
    },
    /// La réservation dépasse la capacité.
    CapacityExceeded,
    /// L'accélérateur demandé n'est pas là.
    AcceleratorUnavailable {
        /// Le genre demandé.
        kind: String,
    },
    /// L'hôte ne sait pas appliquer ce mode réseau.
    NetworkModeUnsupported {
        /// Le mode demandé.
        mode: &'static str,
    },
}

impl fmt::Display for RefusalReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LevelUnavailable { required, best } => write!(
                formatter,
                "mission en {}, hôte au mieux en {}",
                required.code(),
                best.code()
            ),
            Self::CapacityExceeded => {
                formatter.write_str("la réservation demandée dépasse la capacité de l'hôte")
            }
            Self::AcceleratorUnavailable { kind } => {
                write!(formatter, "aucun accélérateur « {kind} » sur cet hôte")
            }
            Self::NetworkModeUnsupported { mode } => {
                write!(
                    formatter,
                    "l'hôte ne sait pas appliquer le mode réseau « {mode} »"
                )
            }
        }
    }
}

/// Le verdict d'admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Admission {
    /// La mission est admise, et voici le niveau qui sera appliqué.
    ///
    /// C'est **le niveau exigé**, jamais le meilleur de l'hôte : appliquer davantage que demandé
    /// serait du sur-confinement, que W4.b nomme et qui fait échouer des missions légitimes.
    Admitted {
        /// Le niveau qui sera appliqué.
        level: SandboxLevel,
    },
    /// La mission est refusée, et voici **toutes** les conditions qui manquent.
    Refused {
        /// Les raisons, dans l'ordre où elles ont été constatées.
        reasons: Vec<RefusalReason>,
    },
}

/// Décider si cet hôte peut honorer cette mission.
///
/// # Toutes les raisons, pas la première
///
/// ADR 0004 : le worker « refuse **proprement** une mission dont il ne peut pas honorer le
/// `SandboxSpec` ». Un refus qui ne nommerait que la première condition manquante ferait corriger
/// une chose, réessayer, découvrir la suivante — et ainsi de suite, un aller-retour par condition.
/// Le refus porte donc la liste complète.
///
/// # Aucun downgrade silencieux
///
/// Quand l'hôte ne sait pas confiner assez fort, la mission est **refusée**. Elle n'est pas admise
/// au niveau que l'hôte sait offrir : ce serait le downgrade que §21.6 interdit, pris au moment où
/// personne ne regarde et sans l'approbation nommée que W4.a exige.
#[must_use]
pub fn admit(spec: &SandboxSpec, host: &HostCapabilities) -> Admission {
    let mut reasons = Vec::new();

    if !host.best_level.satisfies(spec.minimum_level()) {
        reasons.push(RefusalReason::LevelUnavailable {
            required: spec.minimum_level(),
            best: host.best_level,
        });
    }

    // Les quatre quotas et l'accélérateur sont constatés séparément : un GPU manquant faisait
    // autrefois échouer le fit global, et le refus disait deux fois la même chose sous deux noms
    // dont l'un était faux — les quotas, eux, tenaient.
    if !spec.resources().quotas_fit_within(&host.capacity) {
        reasons.push(RefusalReason::CapacityExceeded);
    }

    if let Some(accelerator) = spec.resources().accelerator()
        && !spec.resources().accelerator_fits_within(&host.capacity)
    {
        reasons.push(RefusalReason::AcceleratorUnavailable {
            kind: accelerator.kind.clone(),
        });
    }

    let mode = spec.network().slug();
    if !host.network_modes.contains(&mode) {
        reasons.push(RefusalReason::NetworkModeUnsupported { mode });
    }

    if reasons.is_empty() {
        Admission::Admitted {
            level: spec.minimum_level(),
        }
    } else {
        Admission::Refused { reasons }
    }
}
