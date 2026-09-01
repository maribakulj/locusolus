//! Rapprocher le mécanisme d'une attestation de celui qu'un worker emploie — ADR 0035 décision 3.
//!
//! # La question, et pourquoi elle n'était pas décidable
//!
//! Une attestation vaut pour un worker quand les trois tiennent ensemble : même hôte (l'empreinte,
//! vérifiée par [`crate::attestation`]), même worker (la clé, décision 2), **et** un mécanisme que
//! ce worker emploie. Les deux premiers termes sont vérifiés depuis `W5.z` ; le troisième ne
//! l'était pas, et `W5.ae` a trouvé pourquoi en lisant les schémas : `backend` est une chaîne libre
//! `minLength: 1` dans `capability-manifest.schema.json` **et** dans
//! `sandbox-attestation.schema.json`. Comparer deux chaînes libres n'est pas une comparaison de
//! mécanismes — c'est une comparaison d'orthographes.
//!
//! # Ce qui rend la comparaison décidable, et ce que ça coûte
//!
//! Le registre `schemas/lep/1.0/mechanisms.json` dit quels noms ce dépôt sait interpréter. Il
//! **n'entre pas dans le fil** : `lep/1.0` est gelé, `backend` y reste une chaîne libre, et rien
//! ici ne rend invalide un document qui était valide. Ce qu'il permet est de séparer deux « non »
//! que l'égalité de chaînes confond :
//!
//! - **`Foreign`** — les deux noms sont au registre et désignent deux mécanismes différents. La
//!   preuve existe, elle porte sur autre chose. On répare en lançant une **autre** campagne.
//! - **`Unresolved`** — les noms diffèrent et l'un au moins manque au registre, ou le manifeste
//!   n'en annonce aucun. On ne sait pas ce qu'un des deux désigne, donc « ils diffèrent » serait
//!   une affirmation qu'on n'a pas les moyens de faire. On répare en ajoutant le nom au registre,
//!   ou en le faisant annoncer par le worker.
//!
//! C'est la forme de [`locus_lep::negotiate`], et pour la même raison : « refusé » et « inconnu »
//! fondus en un seul non rendraient un pair mal orthographié indiscernable d'un pair légitime.
//!
//! # Ce que ce module ne fait pas
//!
//! Il ne normalise rien — ni casse repliée, ni espaces rognés, ni famille déduite d'un préfixe. Un
//! nom désigne lui-même. Deux noms ne se rapprochent que si une **mesure** établit qu'ils désignent
//! le même mécanisme ; l'ADR 0035 laisse `podman-rootless` et `bubblewrap` incomparables faute de
//! les avoir mesurés l'un contre l'autre, et l'ADR 0036 applique le même critère à `bubblewrap` et
//! `bubblewrap+cgroup`. Une équivalence, le jour où elle sera mesurée, s'écrira dans le registre —
//! pas ici, et surtout pas dans une heuristique de comparaison.

use locus_lep::mechanism_registered;

/// Ce que vaut une attestation pour le worker qui réclame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Employment {
    /// Le mécanisme attesté est celui que le worker annonce employer.
    Employed,
    /// Les deux noms sont au registre, et ils ne désignent pas le même mécanisme.
    ///
    /// L'attestation est écartée. Ce n'est pas une lacune de la campagne : elle a conclu, et bien,
    /// sur autre chose.
    Foreign,
    /// Les deux noms diffèrent et l'un au moins est hors registre, ou le manifeste n'en annonce
    /// aucun.
    ///
    /// `unregistered` porte les noms que le registre ne connaît pas — il est **vide** quand le
    /// défaut est du côté de l'annonce, c'est-à-dire quand le manifeste ne nomme aucun mécanisme.
    /// Les deux se réparent différemment, et les fondre ferait chercher au registre un nom qui
    /// manque au manifeste.
    Unresolved {
        /// Les noms absents du registre, dans l'ordre où ils ont été examinés.
        unregistered: Vec<String>,
    },
}

/// Rapprocher le mécanisme d'une attestation de celui qu'un manifeste annonce.
///
/// # Deux noms **égaux** rapprochent, même hors registre
///
/// Le registre sert à distinguer les deux façons de dire non, pas à autoriser le oui : un nom
/// désigne un mécanisme, et deux émetteurs qui écrivent le même jeton parlent de la même chose.
/// Refuser un `firecracker` attesté à un worker qui annonce `firecracker` obligerait chaque
/// déploiement tiers à modifier un fichier de **ce** dépôt pour placer quoi que ce soit, et ne
/// protégerait de rien qu'un nom enregistré ne risque déjà.
///
/// L'ignorance reste du bon côté là où elle compte : un nom que le registre ne connaît pas ne se
/// rapproche **jamais** d'un nom différent. C'est le rapprochement silencieux de deux mécanismes
/// distincts que l'ADR 0035 interdit, pas l'identité d'un nom avec lui-même.
///
/// # `announced` absent
///
/// `backend` est facultatif dans `CapabilityManifestSandbox` alors qu'il est obligatoire dans
/// `SandboxAttestation` — une asymétrie des schémas gelés qu'on ne peut pas corriger et qu'il faut
/// donc savoir traiter. Sans nom annoncé, le troisième terme de la décision 3 ne se vérifie pas, et
/// `unregistered` reste **vide** : le nom manque au manifeste, et l'ajouter au registre ne
/// réparerait rien.
#[must_use]
pub fn employment(attested: &str, announced: Option<&str>) -> Employment {
    let Some(announced) = announced else {
        return Employment::Unresolved {
            unregistered: Vec::new(),
        };
    };
    if attested == announced {
        return Employment::Employed;
    }
    let unregistered: Vec<String> = [attested, announced]
        .into_iter()
        .filter(|name| !mechanism_registered(name))
        .map(str::to_owned)
        .collect();
    if unregistered.is_empty() {
        Employment::Foreign
    } else {
        Employment::Unresolved { unregistered }
    }
}
