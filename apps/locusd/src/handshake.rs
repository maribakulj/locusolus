//! Ce que le plan de contrôle annonce de lui-même — `docs/06`, `W19.e`.
//!
//! # Une boucle ouverte depuis `W2.7`, et son autre bout
//!
//! `docs/06` fait de la négociation de features un acte du handshake, et `W2.7` en a livré la
//! moitié **cliente** : `canterel` envoie un hello, lit un [`ServerHello`], et appelle `negotiate` avec
//! ce qu'il en tire. La moitié serveur n'existait pas. Le worker posait donc une question à laquelle
//! rien ne répondait, et son défaut — liste serveur vide, donc **tout** en `declined` — était
//! correct et **indistinguable** d'un plan de contrôle qui ne tiendrait aucune feature.
//!
//! Ce module ferme la boucle. Il ne débloque pas seulement `W19.c` : sans lui, la garde que l'ADR
//! 0037 exige d'un membre d'énumération serait fermée pour toujours, et une valeur gardée ne
//! pourrait jamais être émise.
//!
//! # Ce que ce module n'annonce pas, et c'est le travail
//!
//! **Pas `LEP_FEATURES`.** Le registre porte les features que le *protocole* définit ; ce daemon
//! n'en tient qu'une partie, et les recopier en bloc les annoncerait toutes. La faute serait pire qu'une
//! promesse ordinaire : le pair d'en face **négocie** dessus, et tient l'accord pour acquis. Un
//! worker qui se replie parce qu'une feature est refusée fonctionne ; un worker qui compte sur une
//! feature accordée à tort casse.
//!
//! Les trois retenues l'ont été parce qu'on a trouvé leur mécanisme, et les trois écartées parce
//! qu'on a **lu** leur absence — non parce qu'une recherche est restée muette :
//!
//! - `late-results` demande que le serveur conserve un résultat rendu après expiration du bail,
//!   comme late candidate. Ce daemon ne suit aucune expiration : il ne garde pas la mission après
//!   l'avoir servie et ne tient aucun registre de baux. Il ne peut donc pas distinguer un résultat
//!   tardif d'un résultat ordinaire, et encore moins le ranger à part ;
//! - `human-input` demande qu'un attempt **se suspende**. `TaskState::WaitingForHuman`
//!   existe et ses transitions sont éprouvées — mais rien ici ne les emprunte :
//!   `human.input.requested` traverse le chemin générique de `Report`, écrit un fait, et la tâche
//!   reste `Running` pour l'institution ;
//! - `signed-events` : `packages/event-store` le dit de lui-même — la signature de fédération
//!   « appartient aux items suivants » — et l'événement LEP n'a de toute façon aucune propriété de
//!   signature parmi ses treize champs.
//!
//! # Ce module ne vérifie pas la signature du hello, et ce n'est pas un oubli
//!
//! `canterel` la pose, et son commentaire annonce que `locusd` la vérifiera. Deux choses s'y
//! opposent aujourd'hui : le registre ne conserve **aucune clé publique** — `WorkerIdentity` porte
//! le worker, le workspace et le principal —, et la réponse est **identique pour tout le monde**.
//! Elle n'accorde rien, ne porte aucun secret, et ne consulte ni le journal ni le registre.
//! Authentifier une annonce publique ne protégerait rien ; le faire demanderait la moitié serveur de
//! `W2.4`. C'est une abstention motivée, et elle se rouvre le jour où le handshake liera un état de
//! session.
//!
//! Ce qui est vérifié, en revanche, est le **majeur** : un pair qui ne parle pas notre majeur est
//! refusé et non servi d'une liste qu'il ne saurait pas lire. C'est ce à quoi `supported_versions`
//! sert, et un handshake qui répondrait « voici mes features » à un `lep/2.0` aurait négocié dans le
//! vide.

use locus_lep::{LEP_FEATURES, feature_since};
use locus_protocol::ProtocolVersion;
use serde::{Deserialize, Serialize};

/// Les features que **ce daemon** tient, par opposition à celles que le protocole définit.
///
/// Quatre sur sept. La liste est courte parce qu'elle est vraie ; voir le module pour ce que chacune
/// des trois autres exigerait et que ce daemon ne fait pas.
///
/// - `pull-queue` — `POST /lep/v1/claim` : le worker **tire**, et ce daemon n'a pas d'autre mode ;
/// - `artifact-streaming` — `POST /lep/v1/artifacts` déclare et `PUT` porte les octets, qui ne
///   passent donc pas par le canal de contrôle ;
/// - `subagent-visibility` — [`crate::subagents`] lit les sous-agents qu'un attempt déclare ;
/// - `refusal-events` — [`crate::refusal`] lit `task.refused` et **remet la mission en file**, au
///   lieu de la laisser sous un bail que plus personne n'honore.
///
/// La dernière est celle que l'ADR 0037 rend obligatoire plutôt que facultative au sens ordinaire :
/// `task.refused` est un membre neuf d'une énumération fermée, et sans la garde il n'aurait pas eu le
/// droit d'entrer. L'annoncer ici n'est donc pas une commodité — c'est la moitié qui rend la valeur
/// émissible.
pub const HELD: [&str; 4] = [
    "pull-queue",
    "artifact-streaming",
    "subagent-visibility",
    "refusal-events",
];

/// Le majeur que ce daemon parle.
///
/// Un seul, et c'est le sujet de la vérification : `speaks_with` ne regarde que lui, parce qu'un
/// mineur plus élevé n'apporte que des champs optionnels qu'un pair plus ancien ignore.
pub const MAJOR: u16 = 1;

/// Ce que le daemon répond à un `worker.hello` — les champs que la moitié cliente lit déjà.
///
/// `server_sequence` est **absent**, et son absence est l'information : ce daemon n'acquitte aucune
/// séquence de worker, et le client lit `-1` — « rien acquitté » — quand le champ manque. Écrire un
/// zéro y ferait lire « j'ai acquitté jusqu'à 0 », ce qui est autre chose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerHello {
    /// La version que le daemon emploie pour cette réponse.
    pub protocol: String,
    /// Tout ce qu'il sait parler, du plus ancien au plus récent.
    pub supported_versions: Vec<String>,
    /// Les features qu'il **tient**, triées.
    pub features: Vec<String>,
}

/// Pourquoi un hello n'a pas reçu de réponse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HelloRefused {
    /// Le pair n'annonce aucune version lisible.
    NoVersion,
    /// Aucun majeur commun : les deux ne peuvent pas se parler.
    NoCommonMajor {
        /// Ce que le pair annonce, tel qu'il l'a écrit.
        offered: Vec<String>,
    },
}

impl std::fmt::Display for HelloRefused {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoVersion => formatter.write_str(
                "aucune version de protocole lisible : un hello qui ne dit pas ce qu'il parle ne \
                 peut pas être servi",
            ),
            Self::NoCommonMajor { offered } => write!(
                formatter,
                "aucun majeur commun — ce pair annonce « {} », ce daemon parle lep/{MAJOR}.x",
                offered.join(" », « ")
            ),
        }
    }
}

/// Les versions annoncées, **dérivées** des features tenues.
///
/// # Pourquoi dériver plutôt qu'écrire la liste
///
/// Une constante écrite à la main dériverait sans que rien ne le dise : le jour où une feature `1.1`
/// quitte [`HELD`], l'annonce continuerait de promettre `lep/1.1`. Ici elle retombe seule à `1.0`.
///
/// `lep/1.0` est toujours présent — c'est le socle que `W0.5` a figé —, et chaque feature tenue
/// ajoute le mineur qui l'introduit. Une feature absente du registre n'ajoute rien : elle serait un
/// nom que le protocole ne définit pas, ce qu'un test refuse par ailleurs.
#[must_use]
pub fn spoken() -> Vec<String> {
    let mut minors: Vec<u16> = vec![0];
    for held in HELD {
        if let Some(since) = feature_since(held)
            && let Ok(version) = ProtocolVersion::parse(&format!("lep/{since}"))
            && version.major == MAJOR
        {
            minors.push(version.minor);
        }
    }
    minors.sort_unstable();
    minors.dedup();
    minors
        .into_iter()
        .map(|minor| format!("lep/{MAJOR}.{minor}"))
        .collect()
}

/// Répondre à un hello, ou refuser en disant pourquoi.
///
/// `offered` est ce que le pair annonce savoir parler — son `protocol` et ses
/// `supported_versions`, indifféremment : ce qui compte est qu'un majeur commun existe.
///
/// # Errors
///
/// [`HelloRefused`] quand rien de lisible n'est annoncé, ou quand aucun majeur n'est commun.
pub fn answer(offered: &[String]) -> Result<ServerHello, HelloRefused> {
    let lisibles: Vec<ProtocolVersion> = offered
        .iter()
        .filter_map(|text| ProtocolVersion::parse(text).ok())
        .collect();
    if lisibles.is_empty() {
        return Err(HelloRefused::NoVersion);
    }
    if !lisibles.iter().any(|version| version.major == MAJOR) {
        return Err(HelloRefused::NoCommonMajor {
            offered: offered.to_vec(),
        });
    }

    let versions = spoken();
    let mut features: Vec<String> = HELD.iter().map(|held| (*held).to_owned()).collect();
    features.sort_unstable();
    Ok(ServerHello {
        // La plus récente que ce daemon parle : le pair rabat au minimum des deux, ce que
        // `ProtocolVersion::negotiate` fait déjà de son côté.
        protocol: versions
            .last()
            .cloned()
            .unwrap_or_else(|| format!("lep/{MAJOR}.0")),
        supported_versions: versions,
        features,
    })
}

/// Chaque feature annoncée est-elle une feature que le protocole définit ?
///
/// Rendue plutôt que testée sur place : c'est une propriété du couple registre/daemon, et un test
/// d'intégration la vérifie. Un nom qui ne serait pas au registre ne serait négociable par personne
/// — `negotiate` le rangerait dans `unknown` chez le pair —, donc l'annoncer reviendrait à annoncer
/// une capacité que le protocole ne sait pas nommer.
#[must_use]
pub fn unknown_to_protocol() -> Vec<&'static str> {
    HELD.into_iter()
        .filter(|held| !LEP_FEATURES.iter().any(|(name, _)| name == held))
        .collect()
}
