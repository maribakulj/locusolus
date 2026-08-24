//! L'amorçage d'**administration** — `W20.y`, §22.3.
//!
//! # Ce que la chaîne réelle a rendu, et qui n'était pas déduit
//!
//! `main.rs` câble le broker (`W20.q`), l'émetteur de tokens (`W20.v`) et la source d'entropie
//! (`W20.x`). Il ne câble **pas** `Administrators`, donc `Desk` garde `NoAdministrators`, dont le
//! contrat est de n'admettre personne. Vérifié contre un daemon réel, pas lu :
//!
//! ```text
//! POST /commands/task/queue  →  403
//! { "family": "authorization",
//!   "detail": "« commander §22.3 sans autorité d'administration reconnue » n'est pas permis" }
//! ```
//!
//! Conséquence exacte : **la première clause de `W12.d` — « une question produit une mission » — ne
//! peut pas s'exécuter contre un `locusd` réel**, quelle que soit la créance présentée. Les deux
//! routes de §22.3 existent depuis `W20.s` et personne ne peut les appeler.
//!
//! C'est la troisième fois que la même forme se présente — un port dont le défaut refuse, correct
//! séparément, et qu'aucun assemblage de production ne remplace. `W20.v` pour les tokens, `W20.x`
//! pour l'entropie, celui-ci pour l'autorité. La forme se reconnaît maintenant à l'œil, et ce
//! module la traite comme les deux précédents plutôt que d'improviser une troisième réponse.
//!
//! # Pourquoi l'environnement du daemon, et non une route
//!
//! Le même argument qu'en `W20.v`, et il tient pour la même raison :
//!
//! - une route qui accorderait l'autorité d'administration demanderait une créance pour être
//!   appelée, et la première créance de l'installation est précisément celle qu'on cherche à
//!   obtenir. La dénouer par une exception — « cette route-là ne demande rien » — donnerait
//!   l'administration à quiconque atteint le port ;
//! - l'environnement du daemon appartient déjà à qui le démarre. **Aucune autorité nouvelle n'est
//!   accordée** : l'opérateur qui pose cette variable peut déjà arrêter le daemon, lire son journal
//!   et le relancer autrement.
//!
//! # Ce qui **diffère** de `W20.v`, et qui n'est pas un détail
//!
//! Un token d'enrôlement est **à usage unique** : il est consommé, et ce qu'il laisse derrière lui
//! est une créance de worker. Une créance d'administration, elle, est un **secret durable** — elle
//! ouvre §22.3 aussi longtemps que le daemon tourne.
//!
//! Deux conséquences tenues ici plutôt que supposées :
//!
//! 1. **elle ne s'imprime jamais**. `main.rs` annonce que l'administration est amorcée et pour quel
//!    workspace ; il n'écrit pas la créance. La règle du dépôt — « ne logge ni OAuth token, API key,
//!    cookie » — vise exactement ceci, et un amorçage bavard la violerait au démarrage, dans le
//!    fichier de log le plus lu de l'installation ;
//! 2. **elle n'est pas le token d'enrôlement**. Deux variables, deux registres, deux résolutions qui
//!    ne se croisent jamais (`crate::mission::Administrators` le dit dans son propre contrat). Les
//!    confondre ferait d'un token d'enrôlement fuité une autorité d'administration.
//!
//! # Le défaut ne change rien
//!
//! Variable absente : aucune créance admise, donc `403` sur §22.3 — **exactement** le comportement
//! d'aujourd'hui. Il n'existe aucune configuration où ce module rend un daemon plus ouvert qu'il ne
//! l'était sans lui.
//!
//! Variable présente et illisible : le démarrage **refuse**, en nommant la variable. Un opérateur
//! qui a écrit `LOCUSD_ADMIN_WORKSPACE` de travers veut administrer ; démarrer sans autorité le
//! laisserait chercher pendant que ses commandes reçoivent un `403` qui parle d'autorité inconnue.

use locus_protocol::id::{Agent, Workspace};

use crate::bootstrap::{BootstrapRefusal, required_id};
use crate::mission::{Authority, MemoryAdministrators};

/// La variable qui porte la créance d'administration.
///
/// Un **secret durable**, contrairement au token de `crate::bootstrap` : elle n'est pas consommée,
/// et elle ne s'imprime nulle part.
pub const CREDENTIAL_ENV: &str = "LOCUSD_ADMIN_CREDENTIAL";

/// Le workspace que cette autorité vise.
pub const WORKSPACE_ENV: &str = "LOCUSD_ADMIN_WORKSPACE";

/// Le principal qui agit sous cette autorité.
pub const PRINCIPAL_ENV: &str = "LOCUSD_ADMIN_PRINCIPAL";

/// Lire l'amorçage d'administration, et dire ce qu'il faut en faire.
///
/// Trois issues, et elles ne se confondent pas :
///
/// - `Ok(None)` — rien n'est demandé. Le daemon démarre et n'admet personne sur §22.3, comme avant.
/// - `Ok(Some((credential, authority)))` — une autorité est demandée, et elle est lisible.
/// - `Err(refusal)` — une autorité est demandée et elle est illisible. Le daemon ne démarre pas.
///
/// # Errors
///
/// Rend [`BootstrapRefusal`] quand une variable manque à l'appel de celles que la créance exige, ou
/// quand un identifiant ne se relit pas.
pub fn read(
    lookup: impl Fn(&str) -> Option<String>,
) -> Result<Option<(String, Authority)>, BootstrapRefusal> {
    let Some(credential) = lookup(CREDENTIAL_ENV).filter(|value| !value.trim().is_empty()) else {
        return Ok(None);
    };

    let workspace_id = required_id::<Workspace>(&lookup, WORKSPACE_ENV, CREDENTIAL_ENV)?;
    let principal_id = required_id::<Agent>(&lookup, PRINCIPAL_ENV, CREDENTIAL_ENV)?;

    Ok(Some((
        credential,
        Authority {
            workspace_id,
            principal_id,
        },
    )))
}

/// Le registre d'administration du daemon, amorcé si l'environnement le demande.
///
/// Rend aussi l'autorité admise, **sans la créance** : le binaire en a besoin pour annoncer sur quoi
/// l'amorçage porte, et la lui redemander par un second [`read`] ferait lire l'environnement deux
/// fois pour une seule décision — deux lectures qui pourraient, en principe, ne pas dire la même
/// chose. Une lecture, une décision.
///
/// La créance, elle, ne ressort pas : rien en dehors du registre n'a de raison de la tenir, et un
/// appelant qui l'aurait sous la main finirait par l'imprimer.
///
/// # Errors
///
/// Rend [`BootstrapRefusal`] quand l'amorçage est demandé et illisible — voir [`read`].
pub fn administrators(
    lookup: impl Fn(&str) -> Option<String>,
) -> Result<(MemoryAdministrators, Option<Authority>), BootstrapRefusal> {
    let registry = MemoryAdministrators::new();
    let Some((credential, authority)) = read(lookup)? else {
        return Ok((registry, None));
    };
    registry.admit(&credential, authority);
    Ok((registry, Some(authority)))
}

/// Ce que `main.rs` a le droit d'imprimer d'un amorçage réussi.
///
/// Le workspace et le principal, **jamais la créance**. Un opérateur a besoin de savoir que son
/// amorçage a pris et sur quoi il porte ; personne n'a besoin de relire le secret dans un journal.
///
/// Fonction plutôt que `format!` à l'appel : un jour où quelqu'un ajoutera un champ à
/// [`Authority`], il le trouvera ici, avec la phrase qui dit ce qui ne doit pas y entrer.
#[must_use]
pub fn annonce(authority: &Authority) -> String {
    format!(
        "administration amorcée — workspace {}, principal {}",
        authority.workspace_id, authority.principal_id
    )
}
