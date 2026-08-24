//! La source d'identifiants du daemon — `W20.x`, ADR 0034.
//!
//! # Ce que `NoIdentities` disait, et qui était vrai
//!
//! « Aucune source d'identifiants n'est câblée : `locusd` ne tire pas d'entropie — cela demande un
//! crate, donc un ADR et une entrée dans `dependencies.json` — et il refuse d'inventer un
//! identifiant de commande plutôt que d'en réattribuer un au redémarrage suivant. »
//!
//! Le refus **nommait son propre remède**. Ce module l'applique : l'ADR 0034 mesure le coût de
//! `getrandom` — zéro paquet ajouté, il est déjà dans l'arbre par `rand` — et
//! [`SystemIdentities`] tire les dix octets d'un ULID de l'entropie du système.
//!
//! # Le défaut continue de refuser, et ce n'est pas une timidité
//!
//! `SystemIdentities` **ne remplace pas** `NoIdentities` : le composition root garde le refus par
//! défaut, et c'est le binaire qui câble la source réelle. Si la source système devenait le défaut,
//! plus personne ne rencontrerait jamais ce refus — et le jour où une plateforme sans entropie
//! apparaîtrait, un conteneur durci ou un environnement embarqué, le message qui explique quoi faire
//! aurait disparu du chemin. Un refus qu'on ne peut plus atteindre est un refus qu'on ne peut plus
//! maintenir.
//!
//! # Une panne d'entropie est une panne, jamais un identifiant de secours
//!
//! `getrandom` peut échouer — c'est rare, et c'est précisément pourquoi il faut décider maintenant
//! ce qu'on en fait. Retomber sur un compteur, sur l'horloge, ou sur des zéros produirait des
//! identifiants **prévisibles ou colisionnants**, donc deux actes institutionnels distincts sous le
//! même nom. Le refus, lui, ne coûte qu'une requête. C'est la même asymétrie que partout ailleurs
//! dans ce dépôt : se tromper d'un côté coûte un appel, de l'autre coûte la vérité du journal.

use locus_protocol::id::{Command as CommandId, Event as EventId};
use locus_protocol::{Id, IdKind, Timestamp};

use crate::error::CommandError;
use crate::lep::Identities;

/// La source réelle : l'entropie du système, par `getrandom`.
///
/// Sans état — chaque appel redemande à l'OS. Un générateur ensemencé une fois au démarrage serait
/// plus rapide et rendrait la suite d'identifiants d'un daemon **rejouable** par qui connaîtrait la
/// graine ; l'ADR 0034 écarte `rand` pour exactement cette raison.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemIdentities;

impl SystemIdentities {
    /// Un identifiant neuf, horodaté à l'instant et complété par dix octets d'aléa.
    ///
    /// # Errors
    ///
    /// [`CommandError::Unavailable`] quand l'OS ne rend pas d'entropie. Le même code que le refus
    /// par défaut, et pour la même raison : le service ne peut pas répondre **maintenant**, et cela
    /// se répare hors du code. Un `Internal` enverrait chercher un défaut là où il n'y en a pas.
    fn fresh<K: IdKind>(now: Timestamp) -> Result<Id<K>, CommandError> {
        let mut entropy = [0_u8; 10];
        getrandom::fill(&mut entropy).map_err(|erreur| CommandError::Unavailable {
            detail: format!(
                "l'entropie du système est indisponible ({erreur}) : `locusd` refuse d'inventer un \
                 identifiant plutôt que d'en produire un prévisible"
            ),
        })?;
        Id::from_parts(now, entropy).map_err(|erreur| CommandError::Unavailable {
            detail: format!("l'horodatage ne tient pas dans un ULID : {erreur}"),
        })
    }

    /// L'instant courant, tel que le système le donne.
    ///
    /// Un ULID porte 48 bits d'horodatage, donc l'ordre lexicographique des identifiants suit
    /// l'ordre des instants. C'est une commodité de lecture, **pas** une garantie : `recorded_at`
    /// du journal reste ce qui date une écriture (§10.1).
    ///
    /// Calculé comme `http::maintenant`, et non par un `Timestamp::now` qui n'existe pas —
    /// `packages/protocol` n'offre que `from_millis` et `parse`. Vérifié plutôt que supposé : la
    /// première rédaction de ce module appelait une méthode inventée.
    fn now() -> Timestamp {
        Timestamp::from_millis(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| i64::try_from(since.as_millis()).unwrap_or(0)),
        )
    }
}

impl Identities for SystemIdentities {
    fn events(&self, count: usize) -> Result<Vec<Id<EventId>>, CommandError> {
        // Le même instant pour tout le lot : les événements d'une commande ont lieu ensemble, et
        // les horodater un par un les ferait paraître étalés dans le temps par le seul effet de la
        // durée de la boucle.
        let now = Self::now();
        (0..count).map(|_| Self::fresh(now)).collect()
    }

    fn command(&self) -> Result<Id<CommandId>, CommandError> {
        Self::fresh(Self::now())
    }

    fn lease(&self) -> Result<Id<CommandId>, CommandError> {
        Self::fresh(Self::now())
    }
}
