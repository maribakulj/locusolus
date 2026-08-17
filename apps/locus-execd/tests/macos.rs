//! Test de sortie de W4.e — ADR 0004, `docs/03` « Local macOS », `docs/SPEC_V1.md` §21.6.
//!
//! **Les faits se lisent dans le noyau qui confine, pas dans celui qui appelle ; et une VM
//! partagée n'est pas une micro-VM par mission.**
//!
//! Les deux moitiés sont le même refus de prendre une apparence pour une garantie. Lire
//! `/sys/fs/cgroup` sur macOS répond « rien » pour une machine parfaitement capable — un backend
//! qui s'y fierait refuserait tout. Et une VM qui existe ne fait pas de chaque mission un `S4` —
//! un backend qui s'y fierait accorderait un confinement que personne ne tient.

use std::sync::Mutex;

use locus_execd::linux::{Execution, Runner};
use locus_execd::macos::{MachineFacts, MachineReader, MachineState};
use locus_execd::{Missing, RuntimeError};
use locus_execution::SandboxLevel;

// ---------------------------------------------------------------------------------------------
// Un runtime qui répond selon ce qu'on lui demande
// ---------------------------------------------------------------------------------------------

/// Répond à `machine list` par `listing`, et à `machine ssh <m> cat <path>` par le contenu associé.
struct MacHost {
    listing: String,
    listing_code: i32,
    guest: Vec<(&'static str, &'static str)>,
    calls: Mutex<Vec<Vec<String>>>,
}

impl MacHost {
    fn new(listing: &str, guest: Vec<(&'static str, &'static str)>) -> Self {
        Self {
            listing: listing.to_owned(),
            listing_code: 0,
            guest,
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<Vec<String>> {
        self.calls.lock().expect("verrou").clone()
    }
}

impl Runner for MacHost {
    fn run(&self, arguments: &[String]) -> Result<Execution, RuntimeError> {
        self.calls.lock().expect("verrou").push(arguments.to_vec());
        if arguments.get(1).map(String::as_str) == Some("list") {
            return Ok(Execution {
                code: self.listing_code,
                stdout: self.listing.clone(),
                stderr: if self.listing_code == 0 {
                    String::new()
                } else {
                    "cannot connect to the machine provider".to_owned()
                },
            });
        }
        let path = arguments.last().expect("un chemin");
        let found = self
            .guest
            .iter()
            .find(|(name, _)| name == path)
            .map(|(_, content)| (*content).to_owned());
        Ok(match found {
            Some(content) => Execution {
                code: 0,
                stdout: content,
                stderr: String::new(),
            },
            None => Execution {
                code: 1,
                stdout: String::new(),
                stderr: format!("cat: {path}: No such file or directory"),
            },
        })
    }
}

/// L'invité d'une machine capable : hiérarchie unifiée, trois contrôleurs, userns et seccomp.
fn capable_guest() -> Vec<(&'static str, &'static str)> {
    vec![
        ("/sys/fs/cgroup/cgroup.controllers", "cpu memory pids\n"),
        ("/proc/self/cgroup", "0::/user.slice/session.scope\n"),
        (
            "/sys/fs/cgroup/user.slice/session.scope/cgroup.controllers",
            "cpu io memory pids\n",
        ),
        ("/proc/sys/user/max_user_namespaces", "15000\n"),
        (
            "/proc/sys/kernel/seccomp/actions_avail",
            "kill_process trap errno allow\n",
        ),
    ]
}

// ---------------------------------------------------------------------------------------------
// L'état de la machine
// ---------------------------------------------------------------------------------------------

#[test]
fn sans_machine_il_n_y_a_rien_a_confiner() {
    let host = MacHost::new("", Vec::new());
    let facts = MachineFacts::read(&host);

    assert_eq!(facts.state(), &MachineState::Absent);
    assert_eq!(facts.ceiling(), SandboxLevel::S0);
    assert!(facts.guest().is_none());
}

#[test]
fn une_machine_arretee_existe_et_ne_confine_rien() {
    let host = MacHost::new("podman-machine-default false\n", capable_guest());
    let facts = MachineFacts::read(&host);

    assert_eq!(
        facts.state(),
        &MachineState::Stopped {
            name: "podman-machine-default".to_owned()
        }
    );
    assert_eq!(facts.ceiling(), SandboxLevel::S0);

    let missing = facts.missing_for(SandboxLevel::S2);
    assert!(
        matches!(
            missing.as_slice(),
            [Missing::Unavailable { what, reason }] if *what == "machine" && reason.contains("arrêtée")
        ),
        "une machine arrêtée est un service à démarrer, pas un noyau incapable : {missing:?}"
    );
}

#[test]
fn l_invite_n_est_pas_interroge_quand_la_machine_ne_tourne_pas() {
    let host = MacHost::new("podman-machine-default false\n", capable_guest());
    let _ = MachineFacts::read(&host);

    assert_eq!(
        host.calls().len(),
        1,
        "interroger une machine arrêtée rendrait des lectures vides, donc un diagnostic exact sur \
         une question qui n'avait pas lieu d'être posée"
    );
}

#[test]
fn une_machine_qui_tourne_l_emporte_sur_une_machine_arretee() {
    let host = MacHost::new(
        "ancienne false\npodman-machine-default true\n",
        capable_guest(),
    );
    assert_eq!(
        MachineFacts::read(&host).state(),
        &MachineState::Running {
            name: "podman-machine-default".to_owned()
        }
    );
}

#[test]
fn un_provider_injoignable_est_indetermine_et_pas_un_refus() {
    let mut host = MacHost::new("", Vec::new());
    host.listing_code = 125;
    let facts = MachineFacts::read(&host);

    assert!(matches!(facts.state(), MachineState::Undetermined { .. }));
    assert!(
        matches!(
            facts.missing_for(SandboxLevel::S1).as_slice(),
            [Missing::Undetermined { what, .. }] if *what == "machine"
        ),
        "« je n'ai pas su demander » n'est pas « il n'y a rien »"
    );
    assert_eq!(facts.ceiling(), SandboxLevel::S0, "le doute ne monte pas");
}

// ---------------------------------------------------------------------------------------------
// Les faits viennent du noyau qui confine
// ---------------------------------------------------------------------------------------------

#[test]
fn les_faits_sont_lus_dans_l_invite_a_travers_la_machine() {
    let host = MacHost::new("podman-machine-default true\n", capable_guest());
    let facts = MachineFacts::read(&host);

    assert_eq!(facts.ceiling(), SandboxLevel::S3);
    let guest = facts.guest().expect("la machine tourne, l'invité est lu");
    for controller in ["cpu", "memory", "pids"] {
        assert!(guest.controllers().contains(controller));
    }

    let ssh = host
        .calls()
        .into_iter()
        .find(|call| call.get(1).map(String::as_str) == Some("ssh"))
        .expect("l'invité est interrogé par ssh");
    assert_eq!(ssh[0], "machine");
    assert_eq!(ssh[2], "podman-machine-default");
    assert_eq!(ssh[3], "cat");
}

#[test]
fn un_invite_incapable_fait_tomber_le_plafond_et_le_diagnostic_traverse() {
    let mut guest = capable_guest();
    guest[2] = (
        "/sys/fs/cgroup/user.slice/session.scope/cgroup.controllers",
        "cpu memory\n",
    );
    let host = MacHost::new("podman-machine-default true\n", guest);
    let facts = MachineFacts::read(&host);

    assert_eq!(facts.ceiling(), SandboxLevel::S1);
    assert!(
        facts.missing_for(SandboxLevel::S2).iter().any(|entry| matches!(
            entry,
            Missing::Unavailable { what, reason } if *what == "contrôleur cgroup" && reason.contains("pids")
        )),
        "le diagnostic de l'invité doit remonter tel quel, sinon on cherche sur le mauvais noyau"
    );
}

#[test]
fn le_lecteur_de_machine_rend_le_contenu_du_fichier_de_l_invite() {
    use locus_execd::linux::probe::Reader;

    let host = MacHost::new("podman-machine-default true\n", capable_guest());
    let reader = MachineReader::new(&host, "podman-machine-default");
    assert_eq!(
        reader.read("/proc/sys/user/max_user_namespaces").as_deref(),
        Some("15000\n")
    );
    assert_eq!(reader.read("/proc/sys/kernel/inexistant"), None);
}

// ---------------------------------------------------------------------------------------------
// La règle qui décide de ce commit
// ---------------------------------------------------------------------------------------------

/// `S4` s'appelle `microvm-high-risk` : sa promesse est qu'une mission à haut risque a **son
/// propre** noyau. Une VM partagée entre toutes les missions ne la tient pas, et l'existence d'une
/// VM est exactement l'argument qui donnerait envie de relever le plafond.
#[test]
fn une_vm_partagee_ne_fait_pas_une_microvm_par_mission() {
    let host = MacHost::new("podman-machine-default true\n", capable_guest());
    let facts = MachineFacts::read(&host);

    assert_eq!(facts.ceiling(), SandboxLevel::S3);
    for level in [SandboxLevel::S4, SandboxLevel::S5] {
        assert!(
            facts.missing_for(level).iter().any(|entry| matches!(
                entry,
                Missing::Unavailable { what, reason } if *what == "niveau" && reason.contains("partagée")
            )),
            "{} devrait être refusé en nommant la VM partagée",
            level.code()
        );
    }
}

#[test]
fn la_preuve_dit_la_machine_et_l_invite() {
    let running = MacHost::new("podman-machine-default true\n", capable_guest());
    let evidence = MachineFacts::read(&running).evidence();
    assert!(evidence[0].contains("podman-machine-default"));
    assert!(
        evidence
            .iter()
            .skip(1)
            .all(|line| line.starts_with("invité")),
        "{evidence:?}"
    );

    let stopped = MacHost::new("podman-machine-default false\n", capable_guest());
    let evidence = MachineFacts::read(&stopped).evidence();
    assert_eq!(evidence.len(), 2);
    assert!(evidence[1].contains("ne tourne pas"));
}
