//! Le cadre : un objet JSON par ligne, et une ligne a une fin — ADR 0028 décision 7.
//!
//! # Pourquoi une borne, et pourquoi elle est un fait du protocole
//!
//! Sans borne, tout ce qui écrit dans la socket peut faire allouer sans fin au processus
//! **privilégié** en n'envoyant jamais de saut de ligne. Le danger n'est pas le format, c'est
//! l'absence de fin — la même règle que xiiif s'est donnée pour les corps de réponse et pour la
//! profondeur JSON.
//!
//! La borne n'est donc pas un réglage : les deux côtés la connaissent, et un dépassement est un
//! **refus qui se dit**, jamais un tampon qui grossit.
//!
//! # Pourquoi une ligne, plutôt qu'un préfixe de longueur
//!
//! `serde_json` échappe les sauts de ligne à l'intérieur des chaînes ; un objet sérialisé n'en
//! contient donc jamais, et le séparateur est sans ambiguïté. Ce que la ligne apporte en plus : le
//! tube se lit tel quel avec `socat` pendant une mise en service, ce qui vaut cher pour un lien dont
//! les pannes se diagnostiquent à deux processus.

use std::io::{BufRead, Write};

use serde::Serialize;
use serde::de::DeserializeOwned;

/// La longueur maximale d'une ligne, saut de ligne exclu.
///
/// Un ordre de grandeur au-dessus de ce que la plus grosse réponse connue peut peser — une liste de
/// manques d'hôte avec leurs raisons en clair —, et trois ordres en dessous de ce qui inquiéterait
/// une machine. Le but n'est pas de serrer au plus juste : c'est qu'une fin existe.
pub const MAX_LINE: usize = 256 * 1024;

/// Ce qui peut mal se passer en lisant ou en écrivant un cadre.
///
/// # Trois causes, et elles ne se soignent pas de la même façon
///
/// [`FrameError::Closed`] dit que l'autre bout est parti — c'est un fait de lien. [`FrameError::TooLong`]
/// dit qu'il a envoyé plus que ce que le protocole permet — c'est un fait de protocole, et pour le
/// processus privilégié c'est une défense. [`FrameError::Malformed`] dit qu'il a envoyé quelque chose
/// d'inintelligible — c'est en général un écart de version. Les fondre ferait chercher une panne de
/// service là où il y a un désaccord de vocabulaire.
#[derive(Debug)]
pub enum FrameError {
    /// L'autre bout a fermé sans envoyer de ligne complète.
    Closed,
    /// La ligne dépasse [`MAX_LINE`].
    TooLong {
        /// Ce qui a été lu avant d'abandonner — jamais la ligne, qu'on refuse justement d'accumuler.
        read: usize,
    },
    /// La ligne n'est pas du JSON de la forme attendue.
    Malformed {
        /// Ce que le lecteur en a dit.
        why: String,
    },
    /// L'entrée/sortie a échoué.
    Io {
        /// Ce que le système en a dit.
        why: String,
    },
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed => formatter.write_str("l'autre bout a fermé sans ligne complète"),
            Self::TooLong { read } => write!(
                formatter,
                "ligne trop longue : {read} octets lus sans fin de ligne, la borne est {MAX_LINE}"
            ),
            Self::Malformed { why } => write!(formatter, "cadre illisible — {why}"),
            Self::Io { why } => write!(formatter, "entrée/sortie — {why}"),
        }
    }
}

impl std::error::Error for FrameError {}

/// Écrire une valeur suivie d'un saut de ligne, et vider le tampon.
///
/// Le `flush` n'est pas décoratif : sans lui, l'appelant attendrait une réponse à une requête qui
/// dort dans un tampon, et la panne ressemblerait trait pour trait à un broker qui ne répond pas.
///
/// # Errors
///
/// [`FrameError::TooLong`] si la valeur sérialisée dépasse [`MAX_LINE`] — la borne vaut **aussi** à
/// l'écriture, sans quoi un côté produirait ce que l'autre est tenu de refuser. [`FrameError::Io`]
/// si l'écriture échoue.
pub fn write_frame<T: Serialize, W: Write>(writer: &mut W, value: &T) -> Result<(), FrameError> {
    let line = serde_json::to_string(value).map_err(|error| FrameError::Malformed {
        why: error.to_string(),
    })?;
    if line.len() > MAX_LINE {
        return Err(FrameError::TooLong { read: line.len() });
    }
    writer
        .write_all(line.as_bytes())
        .and_then(|()| writer.write_all(b"\n"))
        .and_then(|()| writer.flush())
        .map_err(|error| FrameError::Io {
            why: error.to_string(),
        })
}

/// Lire une ligne et l'interpréter.
///
/// # La lecture est bornée avant d'être interprétée
///
/// La borne s'applique à ce qui **entre**, octet par octet : un `read_line` classique aurait déjà
/// tout alloué au moment où on aurait pu mesurer. C'est la différence entre une borne qui protège et
/// une borne qui constate.
///
/// # Errors
///
/// [`FrameError::Closed`] si le flux se termine avant une fin de ligne, [`FrameError::TooLong`] si la
/// borne est franchie, [`FrameError::Malformed`] si la ligne n'est pas la forme attendue,
/// [`FrameError::Io`] si la lecture échoue.
pub fn read_frame<T: DeserializeOwned, R: BufRead>(reader: &mut R) -> Result<T, FrameError> {
    let mut line = Vec::new();
    loop {
        let available = match reader.fill_buf() {
            Ok(buffer) => buffer,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => {
                return Err(FrameError::Io {
                    why: error.to_string(),
                });
            }
        };
        if available.is_empty() {
            return Err(FrameError::Closed);
        }
        if let Some(end) = available.iter().position(|byte| *byte == b'\n') {
            if line.len() + end > MAX_LINE {
                return Err(FrameError::TooLong {
                    read: line.len() + end,
                });
            }
            line.extend_from_slice(&available[..end]);
            reader.consume(end + 1);
            break;
        }
        let taken = available.len();
        if line.len() + taken > MAX_LINE {
            // On n'accumule pas ce qu'on refuse : la borne existe pour que ce tampon ne grossisse
            // pas, donc l'abandon est immédiat et ne recopie rien.
            return Err(FrameError::TooLong {
                read: line.len() + taken,
            });
        }
        line.extend_from_slice(available);
        reader.consume(taken);
    }
    serde_json::from_slice(&line).map_err(|error| FrameError::Malformed {
        why: error.to_string(),
    })
}
