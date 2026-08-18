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
    reach: AcceleratorReach,
    disk_quota: DiskQuota,
}

/// D'où l'accélérateur de l'hôte est atteignable.
///
/// # Pourquoi cette distinction existe
///
/// `docs/05` : « les capacités macOS natives telles que MPS/MLX sont exposées par un worker de
/// confiance **séparé** ». Le mot « séparé » n'est pas une préférence d'organisation, c'est une
/// contrainte de la plateforme : Metal est une API de macOS, et un invité Linux dans une VM n'y a
/// pas accès. Sur un tel hôte, une mission peut avoir le conteneur **ou** l'accélérateur, jamais les
/// deux — et c'est exactement le genre de chose qu'on fusionne par optimisme, parce que « la machine
/// a bien un GPU ».
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceleratorReach {
    /// L'accélérateur est visible depuis la sandbox : un GPU passé au conteneur, par exemple.
    InsideSandbox,
    /// L'accélérateur n'existe qu'en exécution native, hors du conteneur.
    NativeOnly {
        /// Le meilleur confinement qu'une exécution **native** obtient sur cet hôte.
        ///
        /// C'est ce qui rend le mot « confiance » mesurable : un worker qui offre `mps` tourne hors
        /// conteneur, donc bas dans l'échelle, et ne peut recevoir que ce qu'on lui confie.
        native_level: SandboxLevel,
    },
}

impl HostCapabilities {
    /// Déclarer ce que l'hôte offre.
    ///
    /// L'accélérateur offert, s'il y en a un, vit dans `capacity` : c'est une ressource comme les
    /// autres, et lui donner une seconde déclaration à côté ferait deux endroits à tenir d'accord.
    /// Par défaut il est réputé atteignable depuis la sandbox — voir [`HostCapabilities::native_only`].
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
            reach: AcceleratorReach::InsideSandbox,
            disk_quota: DiskQuota::Enforceable,
        }
    }

    /// Déclarer que cet hôte **ne peut pas** tenir de quota disque, et pourquoi.
    ///
    /// Le paramètre est la raison lue, pas un booléen : un refus qui dirait seulement « pas de
    /// quota disque » enverrait chercher une option de configuration là où c'est le système de
    /// fichiers qui décide. `HostFacts::unenforceable_disk_quota` la produit — c'est le pont entre
    /// ce qui a été **lu** sur l'hôte et ce sur quoi l'admission **décide**.
    #[must_use]
    pub fn without_disk_quota(mut self, why: &str) -> Self {
        self.disk_quota = DiskQuota::NotEnforceable {
            why: why.to_owned(),
        };
        self
    }

    /// Si un quota disque est applicable ici.
    #[must_use]
    pub const fn disk_quota(&self) -> &DiskQuota {
        &self.disk_quota
    }

    /// Déclarer que l'accélérateur n'est atteignable qu'en exécution native.
    #[must_use]
    pub fn native_only(mut self, native_level: SandboxLevel) -> Self {
        self.reach = AcceleratorReach::NativeOnly { native_level };
        self
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

    /// D'où son accélérateur est atteignable.
    #[must_use]
    pub const fn reach(&self) -> &AcceleratorReach {
        &self.reach
    }

    /// Le meilleur confinement disponible **pour cette mission**.
    ///
    /// Il vaut [`HostCapabilities::best_level`], sauf quand la mission exige un accélérateur que
    /// seule l'exécution native atteint : le plafond devient alors celui de l'exécution native.
    /// Décider cela ici plutôt que dans [`admit`] évite qu'un second appelant l'oublie.
    #[must_use]
    pub fn level_for(&self, spec: &SandboxSpec) -> SandboxLevel {
        match &self.reach {
            AcceleratorReach::NativeOnly { native_level }
                if spec.resources().accelerator().is_some() =>
            {
                *native_level
            }
            _ => self.best_level,
        }
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
    /// L'hôte ne sait pas **borner** l'espace disque, quel que soit ce qu'il en reste.
    ///
    /// Distinct de [`RefusalReason::CapacityExceeded`], et la distinction n'est pas cosmétique :
    /// « la capacité manque » envoie libérer de la place ou réduire la réservation ; « la borne
    /// n'est pas applicable ici » envoie changer de système de fichiers, ou de machine. Les fondre
    /// ferait réduire une réservation qui aurait échoué de la même façon à un octet.
    DiskQuotaNotEnforceable {
        /// Ce que la mission réserve.
        requested: u64,
        /// Ce qui a été lu de l'hôte, et qui nomme le système de fichiers.
        why: String,
    },
    /// L'hôte ne sait pas appliquer ce mode réseau.
    NetworkModeUnsupported {
        /// Le mode demandé.
        mode: &'static str,
    },
    /// L'hôte annonce ce niveau mais ne l'a jamais prouvé.
    ///
    /// §12.2 demande une sandbox « disponible **et attestée** ». Une annonce n'est pas une preuve,
    /// et W4.d.3 a établi qu'une suite non passée ne vaut pas une suite passée — c'est la même
    /// règle, appliquée au placement plutôt qu'à une sonde.
    LevelNotAttested {
        /// Ce que la mission exige.
        required: SandboxLevel,
        /// Le meilleur niveau que l'hôte a prouvé tenir, s'il en a prouvé un.
        proven: Option<SandboxLevel>,
    },
    /// L'accélérateur existe, mais pas là où la mission veut être confinée.
    ///
    /// Le refus est distinct de [`RefusalReason::AcceleratorUnavailable`] : l'accélérateur **est**
    /// sur cet hôte, et le dire « absent » enverrait chercher du matériel au lieu de choisir entre
    /// le conteneur et l'accélérateur.
    AcceleratorOutsideSandbox {
        /// Le genre demandé.
        kind: String,
        /// Le niveau que la mission exige.
        required: SandboxLevel,
        /// Le niveau qu'une exécution native obtient sur cet hôte.
        native_level: SandboxLevel,
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
            Self::DiskQuotaNotEnforceable { requested, why } => write!(
                formatter,
                "la mission réserve {requested} octets de disque, que cet hôte ne sait pas borner : {why}"
            ),
            Self::NetworkModeUnsupported { mode } => {
                write!(
                    formatter,
                    "l'hôte ne sait pas appliquer le mode réseau « {mode} »"
                )
            }
            Self::LevelNotAttested { required, proven } => match proven {
                Some(level) => write!(
                    formatter,
                    "l'hôte exige {} et n'a prouvé que {}",
                    required.code(),
                    level.code()
                ),
                None => write!(
                    formatter,
                    "aucune campagne de self-tests n'a conclu sur cet hôte : {} n'est pas prouvé",
                    required.code()
                ),
            },
            Self::AcceleratorOutsideSandbox {
                kind,
                required,
                native_level,
            } => write!(
                formatter,
                "« {kind} » n'existe qu'en exécution native sur cet hôte, donc au mieux en {} ; la mission exige {}",
                native_level.code(),
                required.code()
            ),
        }
    }
}

/// Si une réservation de disque est applicable sur cet hôte.
///
/// # Pourquoi ce n'est pas un quota de plus dans `ResourceSpec`
///
/// Les trois autres quotas — CPU, mémoire, PID — se bornent par cgroup v2, et un hôte qui porte les
/// contrôleurs les porte tous. Le disque ne se borne pas par cgroup : `cgroup.controllers` ne dit
/// rien de lui, et `REQUIRED_CONTROLLERS` le note sans en tirer la conséquence. Il dépend du
/// **système de fichiers**, ce qui en fait une capacité d'une autre nature — présente ou absente
/// indépendamment du niveau de confinement atteignable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiskQuota {
    /// Le stockage de cet hôte sait porter un quota de projet.
    Enforceable,
    /// Il ne sait pas, et voici ce qui le dit.
    NotEnforceable {
        /// Ce qui a été lu, mot pour mot, et qui nomme le système de fichiers.
        why: String,
    },
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
    let available = host.level_for(spec);

    if !available.satisfies(spec.minimum_level()) {
        reasons.push(match (&host.reach, spec.resources().accelerator()) {
            (AcceleratorReach::NativeOnly { native_level }, Some(accelerator)) => {
                RefusalReason::AcceleratorOutsideSandbox {
                    kind: accelerator.kind.clone(),
                    required: spec.minimum_level(),
                    native_level: *native_level,
                }
            }
            _ => RefusalReason::LevelUnavailable {
                required: spec.minimum_level(),
                best: available,
            },
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

    // Constaté **avant** toute création, et c'est tout l'objet de W5.g. Ce refus-là s'apprenait
    // autrefois de `podman create`, qui rendait 125 après avoir commencé à configurer le stockage —
    // exactement ce que l'en-tête de `HostCapabilities` dit qu'un broker ne doit pas faire.
    if spec.resources().disk_bytes() > 0
        && let DiskQuota::NotEnforceable { why } = &host.disk_quota
    {
        reasons.push(RefusalReason::DiskQuotaNotEnforceable {
            requested: spec.resources().disk_bytes(),
            why: why.clone(),
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
