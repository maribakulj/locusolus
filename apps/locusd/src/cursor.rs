//! Les cursors de `SPEC_V1.md` §22.6 — opaques, stables, et refusés hors de leur collection.
//!
//! # Ce que « opaque » veut dire ici, et ce que cela ne veut pas dire
//!
//! Opaque est un **contrat**, pas un chiffrement : le client ne doit ni lire ni fabriquer un cursor,
//! et le serveur refuse ceux qu'il n'a pas émis. Le contenu est encodé en hexadécimal pour qu'aucun
//! client ne soit tenté d'y reconnaître un entier et de l'incrémenter — la tentation étant le vrai
//! risque, bien avant l'adversaire.
//!
//! Une somme de contrôle accompagne le contenu. Elle détecte l'accident et la dérive : un cursor
//! tronqué par un journal, recopié à la main, ou forgé par un client qui a deviné le format. **Elle
//! ne résiste pas à un adversaire** — il faudrait un MAC, donc une dépendance cryptographique, donc
//! un ADR (`dependencies.json` refuserait le crate autrement). Le dire est plus utile que de laisser
//! croire à une garantie qui n'est pas là : un cursor n'est pas un jeton d'autorisation, et rien
//! dans §22 ne lui en donne le rôle.
//!
//! # Pourquoi la collection voyage dans le cursor
//!
//! `W20.e` demande qu'« un cursor d'une autre collection soit **refusé** au lieu d'être interprété,
//! parce qu'un cursor mal interprété saute des pages en silence ». C'est le mode d'échec à éviter :
//! une position `47` a un sens dans chaque collection, et la lire dans la mauvaise ne produit ni
//! erreur ni page vide — elle produit une page **plausible**, prise au mauvais endroit. Rien dans la
//! réponse ne permettrait au client de s'en apercevoir.

use std::fmt;
use std::fmt::Write as _;

/// Les collections paginées — §22.4, celles qui ont aujourd'hui de quoi être servies.
///
/// La liste est **close et courte**, et c'est la règle de `CLAUDE.md` sur les énumérations : une
/// sorte n'y entre que lorsqu'un consommateur exécutable et testé existe. §22.4 en nomme dix-neuf ;
/// les autres attendent leurs agrégats, et les inscrire ici les ferait passer pour servies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Collection {
    /// `GET /timeline` — le flux du journal, par position globale.
    Timeline,
    /// `GET /workers` — les workers vus par le graphe d'exécution.
    Workers,
    /// Les conflits ouverts — §9.4, et l'invariant 12 : rien n'y est supprimé.
    Conflicts,
    /// Le fil d'événements clients de §22.1 — `Last-Event-ID` en SSE.
    ///
    /// Distincte de `Timeline` alors que les deux suivent la même position globale, et le choix est
    /// délibéré : la timeline est une **query** paginée, qui pourra recevoir un filtre, tandis que
    /// le fil est une **souscription**. Le jour où la timeline filtre, ses positions ne seront plus
    /// celles du fil — et un cursor qui aurait silencieusement fonctionné des deux côtés se mettrait
    /// à sauter des événements sans que rien ne le dise.
    Events,
}

impl Collection {
    /// Son nom sur le fil, et dans le cursor.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Timeline => "timeline",
            Self::Workers => "workers",
            Self::Conflicts => "conflicts",
            Self::Events => "events",
        }
    }

    /// Toutes, dans l'ordre de déclaration.
    pub const ALL: [Self; 4] = [Self::Timeline, Self::Workers, Self::Conflicts, Self::Events];
}

impl fmt::Display for Collection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

/// Une position dans une collection, que le client rend telle qu'il l'a reçue.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Cursor(String);

impl Cursor {
    /// Émettre un cursor pour cette collection à cette position.
    #[must_use]
    pub fn issue(collection: Collection, position: u64) -> Self {
        let payload = format!("{}:{position}", collection.name());
        let checksum = checksum(&payload);
        Self(encode(&format!("{payload}:{checksum:08x}")))
    }

    /// Le lire, **pour cette collection**.
    ///
    /// # Errors
    ///
    /// - [`CursorError::Malformed`] si ce n'est pas un cursor que ce serveur a pu émettre ;
    /// - [`CursorError::WrongCollection`] s'il vient d'une autre collection. Le refus nomme les
    ///   deux, parce qu'un client qui mélange deux paginations a besoin de savoir laquelle.
    pub fn read(&self, expected: Collection) -> Result<u64, CursorError> {
        let decoded = decode(&self.0).ok_or(CursorError::Malformed)?;
        let mut parts = decoded.rsplitn(2, ':');
        let given = parts.next().ok_or(CursorError::Malformed)?;
        let payload = parts.next().ok_or(CursorError::Malformed)?;
        if given != format!("{:08x}", checksum(payload)) {
            return Err(CursorError::Malformed);
        }

        let (name, position) = payload.split_once(':').ok_or(CursorError::Malformed)?;
        let found = Collection::ALL
            .into_iter()
            .find(|candidate| candidate.name() == name)
            .ok_or(CursorError::Malformed)?;
        if found != expected {
            return Err(CursorError::WrongCollection { expected, found });
        }
        position.parse().map_err(|_| CursorError::Malformed)
    }

    /// Sa forme sur le fil.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Le lire depuis ce qu'un client a envoyé.
    #[must_use]
    pub fn from_wire(text: impl Into<String>) -> Self {
        Self(text.into())
    }
}

impl fmt::Display for Cursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Pourquoi un cursor est refusé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorError {
    /// Ce serveur n'a pas pu émettre ce cursor.
    Malformed,
    /// Il vient d'une autre collection — le mode d'échec silencieux que §22.6 vise.
    WrongCollection {
        /// Celle qu'on interrogeait.
        expected: Collection,
        /// Celle dont il vient.
        found: Collection,
    },
}

impl fmt::Display for CursorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed => formatter.write_str("cursor illisible : il n'a pas été émis ici"),
            Self::WrongCollection { expected, found } => write!(
                formatter,
                "cursor de « {found} » présenté à « {expected} » : une position n'a pas le même sens d'une collection à l'autre"
            ),
        }
    }
}

impl std::error::Error for CursorError {}

/// Un encodage hexadécimal, écrit à la main.
///
/// Base64 aurait été plus court et aurait coûté un crate. `dependencies.json` le refuserait, et il
/// aurait raison : douze lignes valent mieux qu'une dépendance transitive dans un binaire dont
/// l'ADR 0011 fait de la surface auditable un motif.
fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for byte in text.bytes() {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn decode(text: &str) -> Option<String> {
    if !text.len().is_multiple_of(2) || text.is_empty() {
        return None;
    }
    let mut bytes = Vec::with_capacity(text.len() / 2);
    for pair in text.as_bytes().chunks(2) {
        let hex = std::str::from_utf8(pair).ok()?;
        bytes.push(u8::from_str_radix(hex, 16).ok()?);
    }
    String::from_utf8(bytes).ok()
}

/// FNV-1a 32 bits — détection d'accident, pas résistance à un adversaire.
///
/// Le choix est dit plutôt que subi : une somme de contrôle non cryptographique attrape la troncature
/// et la recopie fautive, qui sont les incidents réels. Un MAC attraperait la forge, et demanderait
/// une dépendance cryptographique — donc un ADR. Tant qu'un cursor n'ouvre aucun droit, il n'y a pas
/// de raison de payer ce prix, et la documentation du module dit exactement ce qui n'est pas couvert.
fn checksum(text: &str) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in text.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}
