//! La fabric d'inférence comme **capacité admise** — `W25.c`, ADR 0026 décisions 6 et 7.
//!
//! # Ce que le dépôt ordonnance, et ce qu'il ne réimplémente pas
//!
//! Le cache de préfixes partagé et la désagrégation prefill/decode sont réels et mesurés — 75 % de
//! requêtes en plus sous SLO, intégrés à deux moteurs de service majeurs. L'ADR 0026 décision 7 en
//! tire la conclusion qui compte : « c'est une **capacité admise** au sens de `W18.d`, derrière un
//! `Published`, pas un sous-système du dépôt. Locus Solus l'ordonnance ; il ne la réimplémente pas,
//! exactement comme `W18.h` a admis le raisonneur d'ontologie. »
//!
//! Ce module suit donc `reasoner.rs` trait pour trait : il ne connaît que des [`Admission`], et une
//! `Admission` ne se fabrique que par [`crate::admit`], qui exige un `Published` de `W5.b`. Le chemin
//! de gouvernance est le seul, **par signature et non par discipline**.
//!
//! # L'absence dégrade la latence, et jamais la correction
//!
//! C'est la clause qui porte l'item, et elle n'est pas tenue par une promesse : elle est tenue par la
//! forme de [`Plan`].
//!
//! Un plan porte ce qui est **demandé** — et c'est cela seul qui détermine la réponse — plus une
//! [`Acceleration`] facultative qui ne dit que **comment** aller plus vite : combien de jetons de
//! préfixe sont réutilisables, et si le prefill se sépare du decode. Retirer la fabric met ce champ à
//! `None` et ne touche rien d'autre.
//!
//! `Acceleration` ne porte donc aucun champ dont dépendrait un résultat — ni modèle, ni gabarit, ni
//! température, ni graine, ni sortie —, et un test le vérifie sur la source. Une accélération qui
//! porterait un modèle ne serait plus une accélération : ce serait un second chemin de décision, et
//! son absence changerait la réponse au lieu de la retarder.
//!
//! # Réutiliser un préfixe n'est pas réutiliser une réponse
//!
//! La distinction vaut d'être dite, parce que c'est là qu'un cache devient faux. [`Acceleration`]
//! compte des **jetons de préfixe déjà calculés**, ce qui est une propriété de la requête présente ;
//! elle ne porte aucune réponse d'une requête passée. Un cache de résultats ferait dépendre ce qu'on
//! rend de ce qu'on a rendu avant, et l'absence de fabric changerait alors les conclusions — ce que
//! la clause interdit exactement.
//!
//! # La résolution se fait par identité, jamais par nom
//!
//! Comme pour le raisonneur, et pour la raison que `W18.h` a écrite : une substitution de capacité
//! par nom ne produit pas d'erreur, elle produit des réponses plausibles fondées sur autre chose. La
//! fabric est donc désignée par le **digest d'image** que son admission porte.

use crate::admission::Admission;

/// Ce qu'une requête d'inférence donne à calculer.
///
/// **C'est ce qui détermine la réponse**, et rien d'autre dans ce module n'y touche.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// Le préfixe partagé — un gabarit de système, un contexte commun à plusieurs requêtes.
    prefix: String,
    /// Ce qui suit, propre à cette requête.
    suffix: String,
}

impl Request {
    /// Poser une requête.
    #[must_use]
    pub fn asking(prefix: &str, suffix: &str) -> Self {
        Self {
            prefix: prefix.to_owned(),
            suffix: suffix.to_owned(),
        }
    }

    /// Le préfixe partagé.
    #[must_use]
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// Ce qui suit.
    #[must_use]
    pub fn suffix(&self) -> &str {
        &self.suffix
    }
}

/// Comment aller plus vite — **et rien de plus**.
///
/// Chaque champ est une propriété de l'exécution, jamais du résultat. Un champ qui influencerait ce
/// qui est rendu ferait de l'absence de fabric un changement de réponse, ce que la clause 3 de
/// `W25.c` interdit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Acceleration {
    /// Combien de jetons de préfixe sont déjà calculés et réutilisables.
    ///
    /// Des **jetons de préfixe**, jamais une réponse : voir l'en-tête du module.
    reusable_prefix_tokens: u64,
    /// Si le prefill se sépare du decode.
    disaggregated: bool,
}

impl Acceleration {
    /// Les jetons de préfixe réutilisables.
    #[must_use]
    pub const fn reusable_prefix_tokens(self) -> u64 {
        self.reusable_prefix_tokens
    }

    /// Vrai quand le prefill est séparé du decode.
    #[must_use]
    pub const fn disaggregated(self) -> bool {
        self.disaggregated
    }
}

/// Une fabric admise.
///
/// Ne se construit que d'une [`Admission`] : il n'y a pas de `Fabric::new`, et `crate::admit` est le
/// seul chemin vers une `Admission`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fabric {
    admission: Admission,
}

impl Fabric {
    /// Prendre une fabric admise.
    #[must_use]
    pub const fn admitted(admission: Admission) -> Self {
        Self { admission }
    }

    /// L'admission qui l'autorise.
    #[must_use]
    pub const fn admission(&self) -> &Admission {
        &self.admission
    }

    /// Le digest d'image qui la désigne — jamais un nom.
    #[must_use]
    pub fn image_digest(&self) -> &str {
        self.admission.image_digest()
    }

    /// Ce que cette fabric peut accélérer sur cette requête.
    ///
    /// Le décompte est une **borne de ce qui est réutilisable**, pas une mesure de ce que le moteur
    /// fera : le dépôt n'exécute aucun moteur, et prétendre chiffrer son gain serait annoncer un
    /// effet qui n'a pas lieu ici.
    #[must_use]
    pub fn accelerating(&self, request: &Request) -> Acceleration {
        Acceleration {
            reusable_prefix_tokens: request.prefix.split_whitespace().count() as u64,
            disaggregated: !request.suffix.is_empty(),
        }
    }
}

/// Ce qui part à l'exécution.
///
/// # Pourquoi les deux champs ne sont pas au même niveau
///
/// `request` **détermine** la réponse ; `acceleration` détermine seulement la vitesse. Les mettre
/// côte à côte dans un même type sans le dire laisserait croire qu'ils pèsent pareil — la
/// documentation le dit, et [`Plan::without_acceleration`] le rend vérifiable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    request: Request,
    acceleration: Option<Acceleration>,
}

impl Plan {
    /// Ce qui est demandé.
    #[must_use]
    pub const fn request(&self) -> &Request {
        &self.request
    }

    /// Comment l'accélérer, quand une fabric est admise.
    #[must_use]
    pub const fn acceleration(&self) -> Option<Acceleration> {
        self.acceleration
    }

    /// Le même plan, sans accélération.
    ///
    /// C'est la façon la plus directe de vérifier la clause 3 : ce que ce plan-ci et celui-là ont en
    /// commun est **tout ce qui détermine la réponse**, et ce qui les sépare ne détermine que la
    /// latence.
    #[must_use]
    pub fn without_acceleration(&self) -> Self {
        Self {
            request: self.request.clone(),
            acceleration: None,
        }
    }
}

/// Planifier une requête, avec la fabric si elle est admise.
///
/// `fabric` est un `Option` **par la signature** : l'absence n'est pas un cas d'erreur qu'on
/// gérerait, c'est le fonctionnement nominal d'un déploiement qui n'a admis aucune fabric.
#[must_use]
pub fn plan(request: Request, fabric: Option<&Fabric>) -> Plan {
    let acceleration = fabric.map(|admitted| admitted.accelerating(&request));
    Plan {
        request,
        acceleration,
    }
}
