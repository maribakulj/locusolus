//! Le lien entre `locusd` et `locus-execd` — `W4.h`, ADR 0028.
//!
//! # Le maillon qui manquait, et pourquoi il ne se voyait pas
//!
//! `W22.c` a découvert le quatrième maillon manquant de la fermeture verticale : **aucun code du
//! dépôt ne construisait de client vers `locus-execd`, et le broker n'écoutait rien.** Les deux
//! binaires existaient, chacun cohérent séparément, et il n'y avait pas de couloir entre eux. C'est
//! exactement pourquoi l'absence n'apparaissait dans aucun décompte d'items faits : rien n'était
//! faux nulle part, il manquait la chose entre les deux.
//!
//! # Ce que ce crate coûte, et c'est le point
//!
//! Rien. `serde` et `serde_json` sont autorisés en portée `*` depuis l'ADR 0011, `packages/lep`
//! est du dépôt, et la socket vient de la bibliothèque standard. Mesuré le 2026-08-22, `locusd`
//! portait **53** paquets externes et `locus-execd` **11** : le processus privilégié en a cinq fois
//! moins que celui qui parle au monde, et c'est la propriété que la séparation de l'ADR 0004
//! achète. Un transport HTTP l'aurait inversée en portant `locus-execd` autour de 53 à son tour —
//! quintupler la surface de code tiers du **seul** processus privilégié pour un confort de format.
//!
//! # Les quatre modules
//!
//! - [`protocol`] : ce qui traverse — deux questions depuis `W20.q`, la disponibilité et le
//!   placement ;
//! - [`frame`] : un objet JSON par ligne, et une ligne a une fin ;
//! - [`port`] : le trait, ses issues, et l'implémentation de référence ;
//! - [`unix`] : la socket, ses deux barrières à l'entrée, et l'écoute.
//!
//! # Le sens unique
//!
//! `locusd` demande, `locus-execd` répond. Le broker n'initie jamais de connexion et n'a pas de
//! client : un programme qui répond n'a besoin que d'écouter, tandis qu'un programme qui appelle a
//! besoin en plus de résoudre, de se connecter, de réessayer et de tenir des échéances. Les
//! nouvelles d'une exécution en cours remontent par le canal que le worker tient **déjà** vers
//! `locusd` — la passerelle d'événements de `W2.12` —, et les faire remonter aussi par ici créerait
//! deux chemins pour le même fait, donc deux versions de la vérité.
//!
//! # Ce que ce crate ne fera jamais
//!
//! Il ne parle pas au réseau. Une socket de domaine Unix n'a pas d'adresse routable : ce n'est pas
//! une option qu'un exploitant peut mal régler ni qu'une régression peut rouvrir, elle n'existe pas.
//! Le lien distant du profil `distributed-hybrid` est un **second backend** derrière le même port, à
//! la condition nommée par l'ADR 0028 décision 6 — le jour où un profil place `locus-execd` sur une
//! autre machine que `locusd`.

pub mod frame;
pub mod port;
pub mod protocol;
#[cfg(unix)]
pub mod unix;

pub use frame::{FrameError, MAX_LINE, read_frame, write_frame};
pub use port::{BrokerError, BrokerPort, Loopback, Placement, as_placement};
pub use protocol::{Ask, Missing, PROTOCOL, Request, Response, Shortfall, Verdict};
#[cfg(unix)]
pub use unix::{DIRECTORY_MODE, ListenError, SOCKET_MODE, UnixSocketBroker, answer, listen};
