//! Le backend macOS — W4.e.
//!
//! # Ce que macOS change, et ce qu'il ne change pas
//!
//! Il ne change **rien** au confinement lui-même. `docs/03` fixe le profil : « host macOS + VM
//! Linux légère + containers rootless par mission ». Le conteneur tourne dans un noyau Linux, donc
//! le plan de confinement est celui de [`crate::linux::plan`], au namespace près et au cgroup près.
//! Écrire un second traducteur aurait produit deux façons de dire la même chose, et le jour où
//! elles auraient divergé, rien n'aurait dit laquelle était appliquée.
//!
//! Il change **où on regarde**. Le noyau qui confine n'est pas celui du processus : lire
//! `/sys/fs/cgroup` sur macOS répond « rien » pour une machine parfaitement capable. Les faits se
//! lisent donc dans l'invité, à travers la machine — c'est ce que [`machine`] fournit.
//!
//! # La règle qui vaut d'être écrite
//!
//! Une VM partagée entre les missions n'est **pas** une micro-VM par mission. `S4` s'appelle
//! `microvm-high-risk` : sa promesse est qu'une mission à haut risque a son propre noyau. Un
//! déploiement macOS ordinaire fait tourner toutes ses missions dans la même VM, où le voisin est
//! un conteneur et non une machine. Le plafond reste donc `S3`, et le dire ici évite qu'on le
//! relève parce qu'« il y a bien une VM ».

pub mod machine;

pub use machine::{MachineFacts, MachineReader, MachineState};
