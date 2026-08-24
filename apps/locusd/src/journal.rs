//! Quel journal ce daemon ouvre, et ce qu'un profil ne peut pas promettre à vide — `W20.m`.
//!
//! # Ce que `W20.i` a livré, et ce qui manquait
//!
//! Le driver `PostgreSQL` existe et passe la suite de contract tests contre le port. Il n'était
//! **câblé nulle part** : `Runtime::in_memory()` restait le seul assemblage, donc « `locusd`
//! redémarre et tout est encore là » n'était vérifié par personne. C'était le périmètre de `W20.i`
//! et c'est le sujet de celui-ci.
//!
//! # Un profil qui promet la durabilité ne démarre pas sur un journal volatile
//!
//! §27.1 nomme cinq profils. `personal-local` met tout sur un poste et un journal en mémoire y est
//! un choix défendable — on perd un laboratoire local, pas une institution. Les quatre autres
//! hébergent un control plane que d'autres interrogent, et un `single-node-vm` qui repartirait vide
//! à chaque redémarrage mentirait à tout ce qui s'y connecte.
//!
//! Le refus est donc **au démarrage**, avant d'ouvrir le port, et pour la même raison que
//! `main.rs` refuse de servir avec une projection en quarantaine : un daemon qui a l'air d'aller
//! bien et qui perd tout au premier redémarrage est pire qu'un daemon qui ne démarre pas, parce
//! qu'un refus se voit.
//!
//! # `composition.rs` n'est pas touché, et c'est la seule chose que `S` garantissait
//!
//! `Runtime<S>` est générique depuis `W20.d`, dont la documentation dit que le driver « se substitue
//! **sans toucher à ce fichier** ». La substitution a lieu ici, dans le binaire, et un test lit
//! `composition.rs` pour vérifier qu'il ne nomme aucun backend concret. C'est la première fois que
//! cette affirmation est éprouvée plutôt qu'annoncée.

use locus_deployment::ProfileKind;

/// Le journal que ce démarrage ouvrira.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    /// En mémoire — volatile, et seul `personal-local` a le droit de s'en contenter.
    Volatile,
    /// Durable, à cette adresse.
    Durable(String),
}

/// Pourquoi un démarrage est refusé avant d'avoir ouvert quoi que ce soit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// Le profil annoncé n'est pas l'un des cinq de §27.1.
    UnknownProfile {
        /// Ce qui a été annoncé.
        given: String,
    },
    /// Le profil promet la durabilité et aucun journal durable n'est configuré.
    VolatileUnderDurableProfile {
        /// Lequel.
        profile: ProfileKind,
    },
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownProfile { given } => write!(
                formatter,
                "profil « {given} » inconnu : §27.1 en nomme cinq — {}",
                ProfileKind::ALL.map(ProfileKind::slug).join(", ")
            ),
            Self::VolatileUnderDurableProfile { profile } => write!(
                formatter,
                "le profil « {profile} » héberge un control plane que d'autres interrogent, et \
                 aucun journal durable n'est configuré. Démarrer en mémoire ferait repartir de \
                 zéro au premier redémarrage, sans que rien ne le dise. Renseignez \
                 `LOCUSD_JOURNAL`, ou déclarez le profil « {} » si ce laboratoire est local.",
                ProfileKind::PersonalLocal.slug()
            ),
        }
    }
}

/// Vrai quand ce profil promet à ses clients de survivre à un redémarrage.
///
/// Écrit comme un `match` exhaustif et non comme une liste : ajouter un sixième profil à
/// [`ProfileKind`] casse cette ligne, et la casser oblige à décider si le nouveau promet la
/// durabilité. Une liste littérale n'oblige à rien — c'est la dérive que `served()` a connue quatre
/// fois dans ce chantier.
#[must_use]
pub const fn promises_durability(profile: ProfileKind) -> bool {
    match profile {
        ProfileKind::PersonalLocal => false,
        ProfileKind::PersonalNode
        | ProfileKind::SingleNodeVm
        | ProfileKind::CloudPlatform
        | ProfileKind::DistributedHybrid => true,
    }
}

impl Choice {
    /// Décider, depuis le profil annoncé et l'adresse du journal.
    ///
    /// # Errors
    ///
    /// [`Refusal::UnknownProfile`] si le profil n'est pas l'un des cinq de §27.1,
    /// [`Refusal::VolatileUnderDurableProfile`] si un profil qui promet la durabilité n'a pas de
    /// journal durable.
    pub fn decide(profile: &str, journal: Option<String>) -> Result<Self, Refusal> {
        let kind = ProfileKind::from_slug(profile).ok_or_else(|| Refusal::UnknownProfile {
            given: profile.to_owned(),
        })?;
        match journal {
            // Une adresse vide n'est pas une adresse. La traiter comme telle ferait échouer la
            // connexion plus tard, avec un message de driver au lieu d'un refus de configuration.
            Some(url) if !url.trim().is_empty() => Ok(Self::Durable(url)),
            _ if promises_durability(kind) => {
                Err(Refusal::VolatileUnderDurableProfile { profile: kind })
            }
            _ => Ok(Self::Volatile),
        }
    }

    /// Ce que ce choix annonce au démarrage — sans jamais citer l'adresse.
    ///
    /// Une chaîne de connexion porte un mot de passe : `CLAUDE.md` interdit de journaliser une
    /// créance, et l'imprimer au démarrage la mettrait dans tous les journaux de supervision.
    #[must_use]
    pub const fn describe(&self) -> &'static str {
        match self {
            Self::Volatile => "journal : en mémoire — volatile, rien ne survit au redémarrage",
            Self::Durable(_) => "journal : durable (PostgreSQL)",
        }
    }
}
