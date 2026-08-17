//! La machine : son état, ce qu'on lit dedans, et le plafond qui en découle.

use std::fmt;

use locus_execution::SandboxLevel;

use crate::linux::driver::Runner;
use crate::linux::plan::BACKEND_CEILING;
use crate::linux::probe::{HostFacts, Missing, Reader};

/// Où en est la VM Linux qui porte les conteneurs.
///
/// Trois états, et le troisième est celui qu'on oublie. Une machine **arrêtée** existe : elle
/// apparaît dans les listes, elle a une configuration, un opérateur la croit là. Elle ne confine
/// rien. Les confondre ferait annoncer `S3` à un hôte où aucune mission ne peut tourner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineState {
    /// Aucune machine n'est définie.
    Absent,
    /// Une machine est définie mais ne tourne pas.
    Stopped {
        /// Son nom.
        name: String,
    },
    /// Une machine tourne.
    Running {
        /// Son nom.
        name: String,
    },
    /// L'état n'a pas pu être établi.
    Undetermined {
        /// Ce qui a empêché de savoir.
        reason: String,
    },
}

impl MachineState {
    /// Le nom de la machine, quand il y en a une.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Stopped { name } | Self::Running { name } => Some(name),
            Self::Absent | Self::Undetermined { .. } => None,
        }
    }

    /// Vrai seulement quand une machine tourne.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        matches!(self, Self::Running { .. })
    }
}

impl fmt::Display for MachineState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Absent => formatter.write_str("aucune machine définie"),
            Self::Stopped { name } => write!(formatter, "« {name} » est définie mais arrêtée"),
            Self::Running { name } => write!(formatter, "« {name} » tourne"),
            Self::Undetermined { reason } => write!(formatter, "état indéterminé : {reason}"),
        }
    }
}

/// Les arguments qui demandent l'état des machines.
///
/// Le gabarit rend une ligne `nom état` par machine : une liste explicite, comme pour
/// l'inspection des conteneurs, et non tout ce que Podman voudra bien dire.
#[must_use]
pub fn list_arguments() -> Vec<String> {
    vec![
        "machine".to_owned(),
        "list".to_owned(),
        "--format".to_owned(),
        "{{.Name}} {{.Running}}".to_owned(),
    ]
}

/// Les arguments qui lisent un fichier dans l'invité.
#[must_use]
pub fn read_arguments(machine: &str, path: &str) -> Vec<String> {
    vec![
        "machine".to_owned(),
        "ssh".to_owned(),
        machine.to_owned(),
        "cat".to_owned(),
        path.to_owned(),
    ]
}

/// Lire l'état des machines.
pub fn state<R: Runner + ?Sized>(runner: &R) -> MachineState {
    let Ok(execution) = runner.run(&list_arguments()) else {
        return MachineState::Undetermined {
            reason: "podman machine list n'a pas répondu".to_owned(),
        };
    };
    if execution.code != 0 {
        return MachineState::Undetermined {
            reason: format!(
                "podman machine list a rendu {} : {}",
                execution.code,
                execution.stderr.trim()
            ),
        };
    }
    let mut stopped = None;
    for line in execution.stdout.lines() {
        let mut parts = line.split_whitespace();
        let (Some(name), Some(running)) = (parts.next(), parts.next()) else {
            continue;
        };
        if running.eq_ignore_ascii_case("true") {
            return MachineState::Running {
                name: name.to_owned(),
            };
        }
        stopped.get_or_insert_with(|| name.to_owned());
    }
    stopped.map_or(MachineState::Absent, |name| MachineState::Stopped { name })
}

/// Lire un fichier de l'invité à travers la machine.
///
/// C'est l'implémentation de [`Reader`] qui permet à [`HostFacts::probe`] d'établir, sans une ligne
/// de logique dupliquée, les faits du noyau **qui confine** plutôt que ceux du noyau qui appelle.
pub struct MachineReader<'a, R: Runner + ?Sized> {
    runner: &'a R,
    machine: String,
}

impl<'a, R: Runner + ?Sized> MachineReader<'a, R> {
    /// Lire à travers cette machine.
    pub fn new(runner: &'a R, machine: &str) -> Self {
        Self {
            runner,
            machine: machine.to_owned(),
        }
    }
}

impl<R: Runner + ?Sized> Reader for MachineReader<'_, R> {
    fn read(&self, path: &str) -> Option<String> {
        let execution = self
            .runner
            .run(&read_arguments(&self.machine, path))
            .ok()
            .filter(|execution| execution.code == 0)?;
        Some(execution.stdout)
    }
}

/// Ce qu'un hôte macOS peut offrir : l'état de la machine, et ce que son invité permet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineFacts {
    state: MachineState,
    guest: Option<HostFacts>,
}

impl MachineFacts {
    /// Interroger la machine, puis son invité si elle tourne.
    ///
    /// L'invité n'est lu que quand la machine tourne : interroger une machine arrêtée rendrait des
    /// lectures vides, que [`HostFacts`] lirait comme des indéterminations — un diagnostic exact sur
    /// une question qui n'avait pas lieu d'être posée, et qui ferait chercher un problème de noyau
    /// là où il suffit de démarrer la machine.
    pub fn read<R: Runner + ?Sized>(runner: &R) -> Self {
        let state = state(runner);
        let guest = state
            .name()
            .filter(|_| state.is_running())
            .map(|machine| HostFacts::probe(&MachineReader::new(runner, machine)));
        Self { state, guest }
    }

    /// L'état de la machine.
    #[must_use]
    pub const fn state(&self) -> &MachineState {
        &self.state
    }

    /// Les faits de l'invité, quand la machine tourne.
    #[must_use]
    pub const fn guest(&self) -> Option<&HostFacts> {
        self.guest.as_ref()
    }

    /// Ce qui manque pour honorer ce niveau sur cet hôte.
    ///
    /// Deux familles de manques, et elles ne se confondent pas : la machine, et l'invité. Une
    /// machine arrêtée n'est pas un noyau incapable, c'est un service à démarrer, et le refus doit
    /// permettre de faire la différence sans lire le code.
    #[must_use]
    pub fn missing_for(&self, level: SandboxLevel) -> Vec<Missing> {
        if level == SandboxLevel::S0 {
            return Vec::new();
        }
        let Some(guest) = self.guest.as_ref() else {
            return vec![match &self.state {
                MachineState::Undetermined { reason } => Missing::Undetermined {
                    what: "machine",
                    reason: reason.clone(),
                },
                other => Missing::Unavailable {
                    what: "machine",
                    reason: other.to_string(),
                },
            }];
        };
        let mut missing = guest.missing_for(level);
        if level > BACKEND_CEILING {
            missing.push(Missing::Unavailable {
                what: "niveau",
                reason: format!(
                    "{} exige une VM par mission ; celle-ci est partagée",
                    level.code()
                ),
            });
        }
        missing
    }

    /// Le niveau le plus élevé que cet hôte peut soutenir.
    ///
    /// # Pourquoi une VM ne fait pas un `S4`
    ///
    /// `S4` s'appelle `microvm-high-risk` : sa promesse est qu'une mission à haut risque a **son
    /// propre** noyau. Un déploiement macOS ordinaire fait tourner toutes ses missions dans la même
    /// VM, où le voisin d'une mission est un conteneur et non une machine. Le plafond reste donc
    /// celui du backend rootless. Le jour où un déploiement créera une VM par mission, ce sera un
    /// autre backend, avec son propre plafond et sa propre suite de self-tests.
    #[must_use]
    pub fn ceiling(&self) -> SandboxLevel {
        SandboxLevel::ALL
            .into_iter()
            .filter(|level| *level <= BACKEND_CEILING)
            .rfind(|level| self.missing_for(*level).is_empty())
            .unwrap_or(SandboxLevel::S0)
    }

    /// Ce que ces faits valent comme preuve.
    #[must_use]
    pub fn evidence(&self) -> Vec<String> {
        let mut lines = vec![format!("machine : {}", self.state)];
        match self.guest.as_ref() {
            Some(guest) => lines.extend(
                guest
                    .evidence()
                    .into_iter()
                    .map(|line| format!("invité — {line}")),
            ),
            None => lines.push("invité : non interrogé, la machine ne tourne pas".to_owned()),
        }
        lines
    }
}
