//! Ce qu'un worker **annonce**, traduit en candidat — et ce que ça ne devient jamais. `W20.q`.
//!
//! # Pourquoi cette traduction existe
//!
//! `W4.g` a livré [`crate::placement::place`] : il choisit un hôte parmi des candidats, ou nomme ce
//! qui manquait à chacun. Il n'avait **aucun appelant** — la file de missions de `W20.k` sert la
//! première offre à qui la demande, quel que soit son manifeste, et sa propre documentation le dit.
//! `W20.q` branche l'appel, et il passe par le fil : `locusd` transmet le `CapabilityManifest` du
//! worker, ce module le relit, et le broker répond.
//!
//! # Une annonce n'est pas une preuve, et rien ici ne la promeut
//!
//! §15.3 : un `CapabilityManifest` est un **inventaire déclaré**. `sandbox.attestation` vaut
//! « ce worker sait produire une `SandboxAttestation` », pas « il en a produit une ». Le lire comme
//! un [`Standing::Trusted`] serait exactement la faute que [`crate::placement`] existe pour
//! refuser — « la confiance ne se déclare pas, elle se prouve » —, et elle serait invisible :
//! le placement marcherait, sur un hôte dont personne n'a rien vérifié.
//!
//! Ce que le worker a **prouvé** vient donc d'ailleurs — du port [`Proven`], que la campagne de
//! self-tests de `W4.d.3` remplit. Son implémentation par défaut ne connaît personne, donc un
//! daemon sans campagne ne place rien au-dessus de `S0`, et le refus le dit sous le nom
//! `level_not_attested`. C'est exact, et c'est la seule chose exacte.
//!
//! # Trois grandeurs que §15.3 n'annonce pas, et qui ne sont donc pas des critères
//!
//! Le manifeste n'inventorie ni quota PID, ni horizon de temps, ni applicabilité d'un quota disque.
//! Les inventer donnerait à un refus une cause que personne n'a déclarée. Chacune est donc
//! **neutralisée** ici, nommément, et un test épingle sa neutralité : le jour où l'une d'elles
//! commence à peser, le test rougit et le dit à qui l'a fait peser.
//!
//! # Ce que ce module ne fait pas, et pourquoi ce n'est pas un oubli
//!
//! Il ne construit jamais [`AcceleratorReach::NativeOnly`]. Cette variante porte un `native_level`
//! — le confinement qu'une exécution **hors conteneur** obtient sur l'hôte — et §15.3 n'a pas de
//! champ pour lui. Le déduire de la plateforme (« macOS, donc MPS est natif, donc `S1` ») serait
//! une politique de sécurité écrite dans une traduction. Elle garde son consommateur : les
//! `HostCapabilities` **lues** sur l'hôte local la posent, et [`crate::admission::admit`] la lit.

use locus_broker::protocol::Shortfall;
use locus_execution::{
    Accelerator, NetworkMode, ResourceSpec, SandboxLevel, SandboxProfile, SandboxSpec, Standing,
};
use locus_lep::{
    AcceleratorType, CapabilityManifest, NetworkMode as WireNetwork, ResourceSpec as WireResources,
    SandboxLevel as WireLevel, SandboxSpec as WireSandbox,
};

use crate::admission::HostCapabilities;
use crate::placement::{Candidate, Placement, place};
use crate::wire::reason;

/// Le quota PID prêté aux deux côtés, faute que §15.3 ou §15.4 en parlent.
///
/// La même valeur de part et d'autre rend la comparaison **neutre** : la demande tient toujours dans
/// l'offre, donc le PID ne décide jamais rien ici. C'est le seul choix qui n'invente pas de plafond
/// d'hôte, et [`ResourceSpec::new`] refuse `0` — à juste titre : une exécution sans processus ne
/// démarre pas.
const PIDS_UNANNOUNCED: u32 = 1;

/// L'horizon prêté à l'**offre** d'un hôte, faute que §15.3 en parle.
///
/// Jamais comparé : [`ResourceSpec::quotas_fit_within`] regarde le CPU, la mémoire, les PID et le
/// disque, et pas le temps. Un test tient cette indistinction, parce qu'une valeur qui ne sert à
/// rien est exactement celle qui se met à servir sans que personne le remarque.
const HORIZON_UNANNOUNCED: u32 = 1;

/// Le profil prêté à une mission qui n'en nomme aucun.
///
/// §21.6 : un profil « nomme une intention ; c'est `minimum_level` qui engage », et `level.rs` note
/// que les profils ne portent pas de niveau. Il est donc **inert** pour le placement, et un test le
/// tient en vérifiant que les sept rendent le même verdict.
///
/// Le choix va au plus confiné pour une seule raison : si un profil se met un jour à engager
/// quelque chose, une mission qui n'en nomme aucun aura été traitée trop sévèrement plutôt que trop
/// légèrement. Il ne porte aucune autre intention, et surtout pas celle de la mission.
const PROFILE_UNNAMED: SandboxProfile = SandboxProfile::MicrovmHighRisk;

/// Pourquoi une demande de placement ne se lit pas.
///
/// Distinct d'un refus de placement : ici on n'a pas pu **poser** la question, là on y a répondu
/// non. Les fondre ferait chercher une machine à qui a envoyé un document incomplet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unreadable {
    /// Le manifeste n'annonce aucun niveau de confinement.
    ///
    /// Ce n'est pas « il annonce `S0` » : un worker qui n'annonce rien n'a pas dit qu'il exécute à
    /// nu, il n'a rien dit. Les confondre placerait une mission sur un silence.
    NoLevelAnnounced,
    /// La mission nomme un profil que §21.6 n'a pas.
    ///
    /// Refusé plutôt que ignoré : un nom que personne n'a défini serait quand même écrit dans le
    /// journal de ce qui a été appliqué.
    UnknownProfile {
        /// Ce qui a été annoncé.
        given: String,
    },
    /// Une grandeur ne se lit pas comme une réservation.
    Quantity {
        /// Laquelle.
        field: String,
        /// Ce qui l'empêche.
        why: String,
    },
}

impl std::fmt::Display for Unreadable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoLevelAnnounced => formatter.write_str(
                "le manifeste n'annonce aucun niveau de confinement : §15.3 veut un inventaire, et \
                 une liste vide n'en est pas un — elle ne dit pas « S0 », elle ne dit rien",
            ),
            Self::UnknownProfile { given } => write!(
                formatter,
                "profil « {given} » inconnu : §21.6 en nomme sept — {}",
                SandboxProfile::ALL.map(SandboxProfile::slug).join(", ")
            ),
            Self::Quantity { field, why } => {
                write!(
                    formatter,
                    "« {field} » ne se lit pas comme une réservation : {why}"
                )
            }
        }
    }
}

/// Ce qu'un worker a **prouvé**, par opposition à ce qu'il annonce.
///
/// # Un port, parce que la campagne qui le remplit ne tourne pas ici
///
/// [`Standing`] est le verdict d'une suite de self-tests (`W4.d.3`). Personne, dans ce dépôt, ne
/// conserve encore les verdicts d'une campagne passée : c'est l'affaire de `W12.e`, qui porte
/// l'attestation, et ce module ne la simule pas.
///
/// Le défaut est donc [`NothingProven`], et il n'est pas un pis-aller : un broker qui ne sait rien
/// d'un worker doit refuser de le placer au-dessus de `S0`, en le disant. C'est ce que
/// [`crate::placement::Candidate::shortfall`] fait déjà, sous le nom `level_not_attested`.
pub trait Proven: Send + Sync {
    /// Les verdicts de campagne connus pour ce worker — vide quand aucun n'a conclu.
    ///
    /// Vide veut dire « aucune campagne n'a conclu », jamais « la campagne a conclu que non » :
    /// [`Standing::NotTrusted`] existe pour la seconde, et `denies_trust` a déjà tranché que
    /// l'absence de preuve n'est pas une preuve.
    fn standing(&self, worker_id: &str) -> Vec<Attested>;
}

/// Ce qu'une campagne a conclu, **et sous quel mécanisme** — ADR 0035 décision 3.
///
/// # Pourquoi les deux voyagent ensemble
///
/// Le port rendait un [`Standing`] nu, qui ne porte que le niveau et ce qui le bloque. La décision 3
/// demande de vérifier « un mécanisme que ce worker emploie » ; avec un verdict nu, le site de
/// placement n'avait **rien à comparer**, quelle que soit la table de vocabulaire qu'on lui aurait
/// donnée. `W5.ae` a rendu le champ obligatoire dans l'enregistrement sur disque, et la traduction
/// vers le port le perdait aussitôt.
///
/// Les tenir dans une seule valeur plutôt qu'en deux listes appariées rend le demi-état
/// inexprimable : personne ne peut prendre le verdict et oublier le mécanisme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attested {
    /// Le mécanisme, sous le nom que `SandboxAttestation.backend` lui donne.
    pub backend: String,
    /// Ce que la campagne a conclu sous ce mécanisme.
    pub standing: Standing,
}

/// La source par défaut : aucune campagne n'a conclu, sur personne.
#[derive(Debug, Default, Clone, Copy)]
pub struct NothingProven;

impl Proven for NothingProven {
    fn standing(&self, _worker_id: &str) -> Vec<Attested> {
        Vec::new()
    }
}

/// Le niveau de confinement, relu depuis le fil.
///
/// L'inverse exact de [`crate::wire::level`], et exhaustif pour la même raison : un niveau nouveau
/// sans relecture ne compile pas.
#[must_use]
pub const fn level_of(wire: WireLevel) -> SandboxLevel {
    match wire {
        WireLevel::S0 => SandboxLevel::S0,
        WireLevel::S1 => SandboxLevel::S1,
        WireLevel::S2 => SandboxLevel::S2,
        WireLevel::S3 => SandboxLevel::S3,
        WireLevel::S4 => SandboxLevel::S4,
        WireLevel::S5 => SandboxLevel::S5,
    }
}

/// Le nom d'un mode réseau **tel que [`crate::admission::admit`] le compare**.
///
/// Ce n'est pas l'orthographe du fil : `locus_execution` écrit `connector_only` et LEP écrit
/// `connector-only`. Comparer l'une à l'autre ferait qu'un hôte annonçant `connector-only` ne
/// satisferait jamais une mission qui l'exige — un refus permanent, silencieux, et sur une
/// capacité que l'hôte a bel et bien.
#[must_use]
const fn mode_of(wire: WireNetwork) -> &'static str {
    match wire {
        WireNetwork::Deny => NetworkMode::Deny.slug(),
        WireNetwork::ConnectorOnly => NetworkMode::ConnectorOnly.slug(),
        WireNetwork::Allowlist => "allowlist",
        WireNetwork::Full => NetworkMode::Full.slug(),
    }
}

/// Le genre d'accélérateur, tel que la taxonomie de §19.4 l'écrit.
#[must_use]
const fn accelerator_kind(kind: AcceleratorType) -> &'static str {
    match kind {
        AcceleratorType::Cuda => "cuda",
        AcceleratorType::Rocm => "rocm",
        AcceleratorType::Mps => "mps",
        AcceleratorType::Tpu => "tpu",
        AcceleratorType::None => "none",
    }
}

/// Lire un entier de fil comme une quantité d'octets, mégaoctet par mégaoctet.
fn megabytes(field: &str, value: i64) -> Result<u64, Unreadable> {
    u64::try_from(value)
        .ok()
        .and_then(|mb| mb.checked_mul(1024 * 1024))
        .ok_or_else(|| Unreadable::Quantity {
            field: field.to_owned(),
            why: format!("{value} Mo ne tient pas dans un compte d'octets positif"),
        })
}

/// Lire un compte de cœurs comme un quota CPU en millièmes.
///
/// Arrondi **vers le haut** pour une offre et pour une demande : côté offre, arrondir vers le bas
/// perdrait une fraction de cœur réellement disponible ; côté demande, arrondir vers le bas
/// accorderait moins que ce qui a été réservé, ce que l'invariant 6 refuse.
fn millis(field: &str, cores: f64) -> Result<u32, Unreadable> {
    let refusal = |why: &str| Unreadable::Quantity {
        field: field.to_owned(),
        why: why.to_owned(),
    };
    if !cores.is_finite() {
        return Err(refusal("un compte de cœurs qui n'est pas un nombre fini"));
    }
    if cores <= 0.0 {
        return Err(refusal(
            "un compte de cœurs nul ou négatif : une exécution sans CPU ne démarre pas",
        ));
    }
    let scaled = (cores * 1000.0).ceil();
    if scaled > f64::from(u32::MAX) {
        return Err(refusal("un compte de cœurs plus grand que tout hôte réel"));
    }
    // `as` est borné juste au-dessus, des deux côtés : la valeur est finie, strictement positive et
    // sous `u32::MAX`. Sans ces trois gardes, un flottant hors bornes saturerait en silence.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Ok(scaled as u32)
}

/// Ce que l'hôte annoncé sait offrir, tel que l'admission le compare.
///
/// `wanted` est le genre d'accélérateur que la mission exige, s'il y en a un. Il entre ici parce
/// qu'un manifeste inventorie **plusieurs** accélérateurs et qu'une [`ResourceSpec`] n'en porte
/// qu'un — c'est une réservation, pas un inventaire. En élire un sans savoir ce qui est demandé
/// refuserait une mission `cuda` sur un hôte offrant `cuda` et `mps`, au motif que `mps` était
/// écrit en premier.
///
/// # Errors
///
/// [`Unreadable`] quand le manifeste n'annonce aucun niveau, ou qu'une grandeur ne se lit pas.
pub fn capabilities(
    manifest: &CapabilityManifest,
    wanted: Option<&str>,
) -> Result<HostCapabilities, Unreadable> {
    let best = manifest
        .sandbox
        .levels
        .iter()
        .copied()
        .map(level_of)
        .max()
        .ok_or(Unreadable::NoLevelAnnounced)?;

    let capacity = ResourceSpec::new(
        millis("resources.cpu_cores", cores(manifest.resources.cpu_cores)?)?,
        megabytes("resources.memory_mb", manifest.resources.memory_mb)?,
        PIDS_UNANNOUNCED,
        megabytes("resources.disk_free_mb", manifest.resources.disk_free_mb)?,
        HORIZON_UNANNOUNCED,
    )
    .map_err(|error| Unreadable::Quantity {
        field: "resources".to_owned(),
        why: error.to_string(),
    })?;

    let capacity = match offered(manifest, wanted)? {
        Some(accelerator) => {
            capacity
                .with_accelerator(accelerator)
                .map_err(|error| Unreadable::Quantity {
                    field: "accelerators".to_owned(),
                    why: error.to_string(),
                })?
        }
        None => capacity,
    };

    let modes = manifest
        .sandbox
        .network_modes
        .iter()
        .copied()
        .map(mode_of)
        .collect();

    // `DiskQuota::Enforceable`, le défaut de `HostCapabilities::new`, est conservé : un manifeste
    // n'annonce pas si le stockage de l'hôte sait porter un quota de projet — c'est un fait lu dans
    // `/proc/mounts` par `W5.g`, sur la machine, et pas une chose qu'on déclare. Ne pas refuser sur
    // ce motif ici est donc exact ; le poser à `NotEnforceable` refuserait tous les workers pour une
    // raison qu'on n'a pas constatée.
    // Le mécanisme annoncé voyage avec le reste : c'est le troisième terme de l'ADR 0035 décision 3,
    // et le manifeste est le seul endroit où il se lise. `None` quand le manifeste ne le nomme pas —
    // `backend` est facultatif ici alors qu'il est obligatoire dans l'attestation, et confondre
    // « absent » avec une chaîne vide ferait comparer un nom que personne n'a écrit.
    let announced = HostCapabilities::new(best, capacity, modes);
    Ok(match manifest.sandbox.backend.as_deref() {
        Some(mechanism) => announced.employing(mechanism),
        None => announced,
    })
}

/// L'accélérateur du manifeste qui répond au genre demandé, s'il y en a un.
///
/// # Le genre rendu est celui que l'**hôte** annonce, jamais celui qu'on cherchait
///
/// Les deux sont égaux par construction, puisque c'est ainsi qu'on l'a trouvé — et une première
/// rédaction en profitait pour recopier le genre demandé. Une passe de mutation a montré ce que ça
/// coûte : en remplaçant la recherche par « prends le premier », l'accélérateur rendu portait
/// toujours le genre **demandé**, donc la comparaison en aval réussissait quel que soit le matériel
/// réel de l'hôte. Le mutant survivait, et il aurait survécu à n'importe quel test.
///
/// Écrire ce que la donnée dit plutôt que ce qu'on espère y trouver ne change rien au comportement
/// correct, et rend le comportement faux **exprimable** — donc constatable.
fn offered(
    manifest: &CapabilityManifest,
    wanted: Option<&str>,
) -> Result<Option<Accelerator>, Unreadable> {
    let Some(kind) = wanted else { return Ok(None) };
    let Some(item) = manifest
        .accelerators
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|item| accelerator_kind(item.r#type) == kind)
    else {
        return Ok(None);
    };
    Ok(Some(Accelerator {
        kind: accelerator_kind(item.r#type).to_owned(),
        count: u32::try_from(item.count).map_err(|_| Unreadable::Quantity {
            field: "accelerators[].count".to_owned(),
            why: format!("{} n'est pas un compte d'accélérateurs", item.count),
        })?,
        memory_bytes: megabytes("accelerators[].memory_mb", item.memory_mb.unwrap_or(0))?,
    }))
}

/// Lire un compte de cœurs annoncé en entier.
fn cores(value: i64) -> Result<f64, Unreadable> {
    u32::try_from(value)
        .map(f64::from)
        .map_err(|_| Unreadable::Quantity {
            field: "resources.cpu_cores".to_owned(),
            why: format!("{value} n'est pas un compte de cœurs"),
        })
}

/// Ce que la mission **exige**, relu du fil.
///
/// # Errors
///
/// [`Unreadable`] quand un profil n'est pas de §21.6, ou qu'une grandeur ne se lit pas.
pub fn requirement(
    sandbox: &WireSandbox,
    resources: &WireResources,
) -> Result<SandboxSpec, Unreadable> {
    let profile = match sandbox.profile.as_deref() {
        None => PROFILE_UNNAMED,
        Some(named) => SandboxProfile::parse(named).ok_or_else(|| Unreadable::UnknownProfile {
            given: named.to_owned(),
        })?,
    };

    let network = match sandbox.network {
        WireNetwork::Deny => NetworkMode::Deny,
        WireNetwork::ConnectorOnly => NetworkMode::ConnectorOnly,
        WireNetwork::Full => NetworkMode::Full,
        WireNetwork::Allowlist => NetworkMode::allowlist(
            sandbox.network_allowlist.clone().unwrap_or_default(),
        )
        .map_err(|error| Unreadable::Quantity {
            field: "sandbox.network_allowlist".to_owned(),
            why: error.to_string(),
        })?,
    };

    let mut reservation = ResourceSpec::new(
        millis("resources.cpu", resources.cpu)?,
        megabytes("resources.memory_mb", resources.memory_mb)?,
        PIDS_UNANNOUNCED,
        megabytes("resources.disk_mb", resources.disk_mb)?,
        u32::try_from(resources.wall_time_seconds).map_err(|_| Unreadable::Quantity {
            field: "resources.wall_time_seconds".to_owned(),
            why: format!("{} n'est pas un horizon", resources.wall_time_seconds),
        })?,
    )
    .map_err(|error| Unreadable::Quantity {
        field: "resources".to_owned(),
        why: error.to_string(),
    })?;

    if let Some(accelerator) = &resources.accelerator {
        reservation = reservation
            .with_accelerator(Accelerator {
                kind: accelerator_kind(accelerator.r#type).to_owned(),
                count: u32::try_from(accelerator.count).map_err(|_| Unreadable::Quantity {
                    field: "resources.accelerator.count".to_owned(),
                    why: format!("{} n'est pas un compte d'accélérateurs", accelerator.count),
                })?,
                memory_bytes: megabytes(
                    "resources.accelerator.memory_mb",
                    accelerator.memory_mb.unwrap_or(0),
                )?,
            })
            .map_err(|error| Unreadable::Quantity {
                field: "resources.accelerator".to_owned(),
                why: error.to_string(),
            })?;
    }

    // Aucun montage : la mission n'en déclare pas sur le fil, et en inventer un serait monter
    // quelque chose que personne n'a demandé — ce que `CLAUDE.md` interdit précisément.
    SandboxSpec::new(
        level_of(sandbox.minimum_level),
        profile,
        network,
        Vec::new(),
        reservation,
    )
    .map_err(|error| Unreadable::Quantity {
        field: "sandbox".to_owned(),
        why: error.to_string(),
    })
}

/// Le candidat que ce manifeste décrit, avec ce que le worker a **prouvé**.
///
/// # Errors
///
/// [`Unreadable`] quand le manifeste ne se lit pas.
pub fn candidate(
    manifest: &CapabilityManifest,
    spec: &SandboxSpec,
    proven: &dyn Proven,
) -> Result<Candidate, Unreadable> {
    let wanted = spec
        .resources()
        .accelerator()
        .map(|accelerator| accelerator.kind.as_str());
    let capabilities = capabilities(manifest, wanted)?;
    let mut candidate = Candidate::new(&manifest.worker_id, capabilities);
    for standing in proven.standing(&manifest.worker_id) {
        candidate = candidate.attested(standing);
    }
    Ok(candidate)
}

/// Placer cette mission sur ce worker, ou dire tout ce qui lui manquait.
///
/// # Un seul candidat, et le pluriel reste
///
/// `locusd` soumet le worker qui réclame, donc un seul. La réponse garde néanmoins la forme
/// plurielle de [`Placement::Refused`] : le jour où un ordonnanceur en soumettra plusieurs, rien du
/// fil ne changera, et d'ici là le pluriel ne coûte rien qu'un élément.
///
/// # Errors
///
/// [`Unreadable`] quand la demande ne se lit pas — ce qui n'est **pas** un refus de placement.
pub fn placement(
    manifest: &CapabilityManifest,
    sandbox: &WireSandbox,
    resources: &WireResources,
    proven: &dyn Proven,
) -> Result<Placement, Unreadable> {
    let spec = requirement(sandbox, resources)?;
    let candidate = candidate(manifest, &spec, proven)?;
    Ok(place(&spec, &[candidate]))
}

/// Ce qu'un refus de placement devient sur le fil.
///
/// Les motifs passent par [`crate::wire::reason`] — la **même** fonction que le refus d'admission de
/// §10.2. Une seconde traduction aurait divergé au premier motif ajouté, et il s'en est ajouté un.
#[must_use]
pub fn shortfalls(shortfalls: &[(String, Vec<crate::admission::RefusalReason>)]) -> Vec<Shortfall> {
    shortfalls
        .iter()
        .map(|(worker, reasons)| Shortfall {
            worker: worker.clone(),
            reasons: reasons.iter().map(reason).collect(),
        })
        .collect()
}
