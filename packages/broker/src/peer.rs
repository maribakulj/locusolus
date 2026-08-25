//! La **créance de pair** sur le lien du broker — `W4.i`, ADR 0028 décision 2 amendée.
//!
//! # Ce que `W4.h` a découvert en écrivant, et que l'ADR affirmait faux
//!
//! L'ADR 0028 décision 2 annonçait deux barrières, la seconde « derrière `UnixStream::peer_cred` de
//! la bibliothèque standard », et concluait : « les deux sont gratuites ; il n'y a aucune raison de
//! n'en prendre qu'une. » **Les deux moitiés étaient fausses**, et `crate::unix` le consigne depuis
//! `W4.h` :
//!
//! 1. `UnixStream::peer_cred` est **instable** — mesuré sur `rustc 1.94.1`, issue #42839 — et
//!    `unsafe_code = "forbid"` ne se contourne ni par `allow` ni par `expect`. L'obtenir coûte donc
//!    un crate externe **dans le processus privilégié**.
//! 2. La politique envisagée — « le même utilisateur que le broker » — admet **exactement**
//!    l'ensemble que `0600` admet déjà. Deux barrières qui laissent passer les mêmes appelants ne
//!    sont pas une défense en profondeur : c'est une redondance qui coûte une dépendance.
//!
//! # Ce que cet item change, et pourquoi il n'est pas la même chose
//!
//! La créance ne sépare quelque chose qu'à partir du moment où la politique cesse d'être « le
//! même » pour devenir « **celui-là** ». C'est le déploiement à deux utilisateurs : socket en
//! `0660` avec un groupe partagé, et le broker vérifiant que l'appelant est l'uid de `locusd` — pas
//! le sien, pas un autre membre du groupe.
//!
//! Les deux barrières admettent alors des ensembles **différents** :
//!
//! | Barrière | Qui passe |
//! |---|---|
//! | permissions `0660` + groupe | le propriétaire, **et tout membre du groupe** |
//! | [`PeerPolicy`] | **un** uid, nommé |
//!
//! L'écart entre les deux lignes est l'item. En `0600` il est vide, et c'est pour cela que `W4.h`
//! avait raison de ne rien livrer.
//!
//! # Un refus est un verdict, jamais une fermeture
//!
//! ADR 0028 décision 4 : « injoignable » et « refusé » envoient chercher à des endroits opposés. Un
//! appelant que la politique écarte reçoit un [`crate::protocol::Verdict::Refused`] **sur le fil**,
//! avec son motif. Fermer la connexion ferait passer un refus d'identité pour une panne, et la
//! première mise en service se passerait à chercher un problème de réseau qui n'existe pas.
//!
//! # Pourquoi `rustix`, et son arbre mesuré
//!
//! Trois paquets — `rustix`, `bitflags`, `linux-raw-sys` —, mesurés arbre complet en
//! `default-features = false, features = ["net", "std"]`. **Aucun `libc`** : le dos `linux_raw`
//! parle au noyau par syscalls directs, ce qui évite d'ajouter un lien C au processus privilégié.
//!
//! Les concurrents ont été mesurés aussi, pas supposés : `nix` en coûte 5, `uds` en coûte 2. `uds`
//! est le moins cher et n'a pas été retenu — il est spécialisé et peu maintenu, là où `rustix` est
//! l'abstraction que l'écosystème vérifie tous les jours. Deux paquets d'écart ne valent pas cette
//! différence-là dans le processus qui crée des conteneurs.
//!
//! `socket_peercred` est **sûre** : elle prend un descripteur emprunté et rend une structure. Rien
//! ici ne demande `unsafe`, et `forbid` tient.

use std::fmt;
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;

/// L'identité que le noyau attribue à l'autre bout.
///
/// Elle ne se falsifie pas depuis l'espace utilisateur : c'est ce qui en fait une **vérification**
/// plutôt qu'une hypothèse, et toute la raison d'être de cette barrière.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerIdentity {
    /// L'uid effectif de l'appelant.
    pub uid: u32,
    /// Son gid effectif.
    pub gid: u32,
    /// Son pid.
    ///
    /// Porté parce qu'un refus qui nomme le processus se diagnostique, là où un refus anonyme
    /// envoie lire des journaux. **Jamais** utilisé pour décider : un pid se réutilise, et fonder
    /// une autorisation dessus serait une course.
    pub pid: i32,
}

/// Qui le broker accepte de servir.
///
/// # Un uid nommé, et pas « le mien »
///
/// C'est tout l'écart avec ce que `W4.h` aurait livré. `Same` — l'uid du broker lui-même — est
/// exactement ce que `0600` admet, donc n'aurait rien séparé ; il n'existe pas ici. La politique
/// **nomme** l'uid attendu, qui est celui de `locusd`, et qui n'est pas celui du broker dans le
/// déploiement que cet item vise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerPolicy {
    expected_uid: u32,
}

impl PeerPolicy {
    /// N'admettre que cet uid.
    #[must_use]
    pub const fn only(expected_uid: u32) -> Self {
        Self { expected_uid }
    }

    /// L'uid admis.
    #[must_use]
    pub const fn expected_uid(self) -> u32 {
        self.expected_uid
    }

    /// Cet appelant est-il admis ?
    ///
    /// Sur l'**uid** seul. Le gid ne décide pas : c'est précisément la barrière que les permissions
    /// `0660` tiennent déjà, et la refaire ici rendrait les deux ensembles à nouveau identiques —
    /// ce que cet item existe pour éviter.
    #[must_use]
    pub const fn admits(self, identity: PeerIdentity) -> bool {
        identity.uid == self.expected_uid
    }
}

/// Ce que la barrière conclut d'un appelant.
///
/// Trois issues, et la troisième n'est pas un refus : ne pas avoir pu lire la créance n'est pas la
/// même chose que l'avoir lue et refusée. Les confondre ferait chercher une usurpation là où il y a
/// une socket dans un état inattendu — et, dans l'autre sens, laisserait passer un défaut de
/// lecture pour un refus légitime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Admission {
    /// L'appelant est l'uid attendu.
    Admitted(PeerIdentity),
    /// Il ne l'est pas, et le verdict le dit.
    Refused {
        /// Qui a demandé.
        identity: PeerIdentity,
        /// Qui était attendu.
        expected_uid: u32,
    },
    /// La créance n'a **pas pu être lue**.
    ///
    /// Ni admis ni refusé : non mesuré. C'est la règle du dépôt — « pas vérifié n'est jamais
    /// réussi » — et son symétrique, qui compte tout autant ici : pas vérifié n'est pas non plus
    /// un échec attribuable à l'appelant.
    Unreadable {
        /// Ce que le noyau a répondu.
        why: String,
    },
}

impl Admission {
    /// Vrai seulement pour [`Admission::Admitted`].
    ///
    /// `Unreadable` rend `false` : sur ce lien, qui commande la création de conteneurs, une créance
    /// illisible ne se traite pas comme un laissez-passer.
    #[must_use]
    pub const fn is_admitted(&self) -> bool {
        matches!(self, Self::Admitted(_))
    }

    /// Le motif à mettre sur le fil, quand il y en a un.
    ///
    /// `None` pour un appelant admis. Les deux autres rendent un texte, **différent** dans chaque
    /// cas : un exploitant qui lit « créance illisible » ne va pas vérifier les mêmes choses que
    /// celui qui lit « uid inattendu ».
    #[must_use]
    pub fn why(&self) -> Option<String> {
        match self {
            Self::Admitted(_) => None,
            Self::Refused {
                identity,
                expected_uid,
            } => Some(format!(
                "créance de pair refusée : uid {} (pid {}), attendu {expected_uid}",
                identity.uid, identity.pid
            )),
            Self::Unreadable { why } => Some(format!(
                "créance de pair illisible, donc non accordée : {why}"
            )),
        }
    }
}

impl fmt::Display for Admission {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.why() {
            None => formatter.write_str("admis"),
            Some(why) => formatter.write_str(&why),
        }
    }
}

/// Lire la créance de l'autre bout, et la confronter à la politique.
///
/// La lecture vient du **noyau**, pas de ce que l'appelant déclare : c'est la seule propriété qui
/// distingue cette barrière d'un secret partagé, et la raison pour laquelle l'ADR 0028 en écarte un.
#[must_use]
pub fn admit(stream: &UnixStream, policy: PeerPolicy) -> Admission {
    match rustix::net::sockopt::socket_peercred(stream.as_fd()) {
        Err(errno) => Admission::Unreadable {
            why: errno.to_string(),
        },
        Ok(credential) => {
            let identity = PeerIdentity {
                uid: credential.uid.as_raw(),
                gid: credential.gid.as_raw(),
                pid: credential.pid.as_raw_nonzero().get(),
            };
            if policy.admits(identity) {
                Admission::Admitted(identity)
            } else {
                Admission::Refused {
                    identity,
                    expected_uid: policy.expected_uid,
                }
            }
        }
    }
}
