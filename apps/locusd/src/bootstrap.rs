//! Le token d'enrôlement d'amorçage — `W20.v`, §7.2.
//!
//! # Ce que `W20.n` avait laissé, et pourquoi c'est maintenant le moment
//!
//! `EnrollmentTokens` est un port depuis `W20.n`, dont la documentation dit sans détour pourquoi il
//! n'avait pas d'émetteur : « §7.2 veut un token court-terme, à usage unique, portant un scope.
//! Aucune commande de §22.3 n'en émet. Reporter l'item reviendrait à dire "aucun appelant", ce que
//! l'ADR 0022 décision 0 refuse ; **inventer un émetteur en passant serait bâtir une fonctionnalité
//! pour justifier une surface** ».
//!
//! L'appelant existe désormais, et il est nommé : `W12.d` monte la chaîne réelle avec le harnais de
//! `W12.f`, et sa toute première étape est qu'un worker `canterel` s'enrôle. Sans émetteur, un
//! `locusd` fraîchement démarré n'enrôle **personne** — vérifié en sondant la chaîne réelle, pas
//! déduit : `MemoryTokens::issue` existe et rien dans `main.rs` ni `http.rs` ne l'appelle.
//!
//! # Pourquoi l'amorçage passe par l'environnement du daemon, et non par une route
//!
//! La question « qui a le droit d'enrôler un worker dans l'institution » est une question
//! d'autorité, et elle mérite mieux qu'un défaut improvisé. Ce module l'**évite** plutôt que d'y
//! répondre :
//!
//! - une route qui émettrait des tokens demanderait une créance pour être appelée, et la première
//!   créance de l'installation est précisément celle qu'on cherche à obtenir. Le serpent se mord la
//!   queue, et le dénouer par une exception — « cette route-là ne demande rien » — ouvrirait
//!   l'enrôlement à quiconque atteint le port ;
//! - l'environnement du daemon, lui, appartient déjà à qui le démarre. **Aucune autorité nouvelle
//!   n'est accordée** : l'opérateur qui peut poser cette variable peut déjà arrêter le daemon, lire
//!   son journal et le relancer avec la configuration de son choix.
//!
//! C'est un amorçage, pas un mécanisme d'exploitation. Le jour où plusieurs workers s'enrôlent au
//! fil de l'eau, la commande de §22.3 qui émettra des tokens se posera la question d'autorité pour
//! de bon — et elle aura, elle, une créance existante à vérifier.
//!
//! # Le défaut ne change rien
//!
//! Variable absente : aucun token, donc personne ne s'enrôle. C'est **exactement** le comportement
//! d'aujourd'hui, et c'est ce qui rend cet ajout sûr par construction — il n'existe aucune
//! configuration où ce module rend un daemon plus ouvert qu'il ne l'était sans lui.
//!
//! # Un amorçage malformé **refuse le démarrage**
//!
//! Une variable présente mais illisible n'est pas ignorée. Un opérateur qui a écrit
//! `LOCUSD_ENROLLMENT_WORKSPACE` de travers veut enrôler quelqu'un ; démarrer sans token le
//! laisserait chercher pendant que son worker reçoit un refus qui parle de token inconnu. Le refus
//! arrive au démarrage, et il nomme la variable.

use locus_protocol::Id;
use locus_protocol::id::{Agent, Project, Workspace};

use crate::enrollment::{Grant, MemoryTokens};

/// La variable qui porte le token d'amorçage.
pub const TOKEN_ENV: &str = "LOCUSD_ENROLLMENT_TOKEN";

/// Le workspace dans lequel les faits du worker enrôlé s'écriront.
pub const WORKSPACE_ENV: &str = "LOCUSD_ENROLLMENT_WORKSPACE";

/// Le principal sous lequel il agira.
pub const PRINCIPAL_ENV: &str = "LOCUSD_ENROLLMENT_PRINCIPAL";

/// Le projet auquel ses faits appartiendront — `W20.w`.
///
/// Exigé comme les deux autres, et pour la même raison : c'est l'institution qui décide où un
/// worker écrit. Le deviner reviendrait à choisir un projet à la place de l'opérateur.
pub const PROJECT_ENV: &str = "LOCUSD_ENROLLMENT_PROJECT";

/// Le scope accordé à un worker enrôlé par amorçage.
///
/// `worker` et rien d'autre. Un amorçage qui accorderait davantage ferait du chemin le plus
/// commode le chemin le plus puissant, ce qui est la façon habituelle dont une porte de service
/// devient une porte d'entrée.
const BOOTSTRAP_SCOPE: &str = "worker";

/// Pourquoi un amorçage n'a pas pu être lu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapRefusal {
    /// La variable fautive, sous son nom.
    pub variable: &'static str,
    /// Ce qui n'allait pas.
    pub reason: String,
}

impl std::fmt::Display for BootstrapRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "amorçage d'enrôlement — {} : {}",
            self.variable, self.reason
        )
    }
}

/// Lire l'amorçage dans un environnement, et dire ce qu'il faut en faire.
///
/// Trois issues, et elles ne se confondent pas :
///
/// - `Ok(None)` — rien n'est demandé. Le daemon démarre et n'enrôle personne, comme avant.
/// - `Ok(Some(grant))` — un token est demandé, et il est lisible.
/// - `Err(refusal)` — un amorçage est demandé et il est illisible. Le daemon ne démarre pas.
///
/// # Errors
///
/// Rend [`BootstrapRefusal`] quand une variable manque à l'appel de celles que le token exige, ou
/// quand un identifiant ne se relit pas.
pub fn read(
    lookup: impl Fn(&str) -> Option<String>,
) -> Result<Option<(String, Grant)>, BootstrapRefusal> {
    let Some(token) = lookup(TOKEN_ENV).filter(|token| !token.trim().is_empty()) else {
        return Ok(None);
    };

    let workspace_id = required_id::<Workspace>(&lookup, WORKSPACE_ENV, TOKEN_ENV)?;
    let principal_id = required_id::<Agent>(&lookup, PRINCIPAL_ENV, TOKEN_ENV)?;
    let project_id = required_id::<Project>(&lookup, PROJECT_ENV, TOKEN_ENV)?;

    Ok(Some((
        token,
        Grant {
            scope: vec![BOOTSTRAP_SCOPE.to_owned()],
            labels: Vec::new(),
            workspace_id,
            principal_id,
            project_id,
        },
    )))
}

/// Un identifiant qu'un amorçage exige, ou le refus qui nomme sa variable.
///
/// `demandeur` est la variable **à cause de laquelle** celle-ci devient obligatoire — `TOKEN_ENV`
/// ici, `CREDENTIAL_ENV` pour l'amorçage d'administration de `W20.y`. Le passer plutôt que de le
/// coder en dur est ce qui rend cette fonction partageable entre les deux amorçages : le message
/// « absente, alors que `X` demande un amorçage » n'a de valeur que s'il nomme le **bon** `X`, et
/// un helper qui nommerait toujours le token enverrait l'opérateur d'un amorçage d'administration
/// chercher une variable d'enrôlement qu'il n'a pas posée.
pub(crate) fn required_id<K: locus_protocol::IdKind>(
    lookup: &impl Fn(&str) -> Option<String>,
    variable: &'static str,
    demandeur: &'static str,
) -> Result<Id<K>, BootstrapRefusal> {
    let Some(brut) = lookup(variable).filter(|value| !value.trim().is_empty()) else {
        return Err(BootstrapRefusal {
            variable,
            reason: format!("absente, alors que `{demandeur}` demande un amorçage"),
        });
    };
    Id::parse(brut.trim()).map_err(|erreur| BootstrapRefusal {
        variable,
        reason: format!("« {} » ne se relit pas : {erreur}", brut.trim()),
    })
}

/// L'émetteur de tokens du daemon, amorcé si l'environnement le demande.
///
/// # Errors
///
/// Rend [`BootstrapRefusal`] quand l'amorçage est demandé et illisible — voir [`read`].
pub fn tokens(lookup: impl Fn(&str) -> Option<String>) -> Result<MemoryTokens, BootstrapRefusal> {
    let issuer = MemoryTokens::new();
    if let Some((token, grant)) = read(lookup)? {
        issuer.issue(&token, grant);
    }
    Ok(issuer)
}
