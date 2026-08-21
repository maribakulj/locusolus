//! Ce que le broker peut prouver au démarrage. `W22.c`, ADR 0025.
//!
//! # Pourquoi ce module existe plutôt qu'un `eprintln!` dans `main.rs`
//!
//! Le point d'entrée de ce binaire a imprimé pendant des mois « aucun driver de runtime n'est
//! encore branché », alors que [`crate::linux::SystemRunner`] — la seule fonction du dépôt qui
//! exécute `podman` — était exportée par le crate depuis `W4.d.2`. Un exploitant qui lançait le
//! binaire en concluait que la fabric d'exécution n'existait pas.
//!
//! La cause n'est pas l'inattention : c'est qu'**aucun test ne traversait `main.rs`**. Une
//! affirmation qui n'est vérifiée par rien vieillit sans que rien ne le dise, et l'ADR 0025 a fait
//! d'une telle affirmation une promesse au sens de l'ADR 0022 décision 0 — une capacité **niée**
//! est une promesse négative, qui induit en erreur dans l'autre sens.
//!
//! D'où la forme : le constat est une **valeur**, `main.rs` n'est qu'une coquille qui l'imprime, et
//! des tests exercent la valeur. C'est le même remède que `locusd`, dont `main.rs` ne décide de
//! rien et se contente de rendre compte d'un `Readiness` calculé ailleurs.
//!
//! # Ce que le binaire dit, et ce qu'il ne dit pas
//!
//! Il dit ce qu'il a **vérifié** : que le driver se construit, ce que l'hôte prouve, et à quel
//! niveau de confinement cet hôte-là plafonne. Il ne dit rien de ce qui viendra — pas de « en
//! attendant tel item », pas de « bientôt ». Une phrase sur l'avenir dans un point d'entrée est
//! exactement la forme de prose que cette phase supprime, et l'y remettre pour expliquer une
//! absence reviendrait à réintroduire la faute en la commentant.
//!
//! # Le plancher, et ce que sa valeur exacte ne décide pas aujourd'hui
//!
//! Le backend Linux plafonne à [`crate::linux::BACKEND_CEILING`], `S3`. Son **plancher** utile est
//! `S2` : en deçà, l'hôte ne sait pas tenir un conteneur sans privilèges, donc le driver est
//! construit mais n'a rien à quoi parler ici. La distinction qui compte et qui **s'exerce** est
//! celle entre `S1` et `S2` — un hôte qui plafonne à `S1` n'est pas utilisable, et les deux se
//! ressembleraient dans un rapport qui ne donnerait qu'un chiffre.
//!
//! **Ce que la valeur exacte ne décide pas.** Une passe de mutation l'a établi : remonter le
//! plancher à `S3` ne change rien, parce que [`crate::linux::HostFacts`] n'exige rien de plus pour
//! `S3` que pour `S2`. Ce qui sépare ces deux niveaux est l'isolation réseau, et elle ne se lit pas
//! dans `/proc` — elle se vérifie sur une sandbox **vivante**, par la suite de sondes de `W5.k`. Un
//! hôte qui prouverait `S2` sans `S3` est donc inexprimable pour ce lecteur de faits.
//!
//! Le plancher garde le nom de ce que le backend exige plutôt que celui du plafond, et un test
//! épingle l'indistinction : si une sonde réseau entre un jour dans `HostFacts`, il rougira et dira
//! à qui l'ajoute que le plancher vient de prendre un sens.

use std::fmt;

use locus_execution::SandboxLevel;

use crate::linux::{HostFacts, Missing};

/// Ce que le broker a pu établir sur cet hôte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Readiness {
    /// L'hôte prouve au moins le plancher : le driver est construit et utilisable ici.
    Provable {
        /// Le niveau le plus confiné que cet hôte prouve.
        ceiling: SandboxLevel,
    },
    /// L'hôte ne prouve pas le plancher, et voici ce qui manque — **fait par fait**.
    ///
    /// Jamais « l'hôte ne convient pas » seul : un exploitant à qui l'on dit quel contrôleur
    /// cgroup n'est pas délégué corrige en une commande, un exploitant à qui l'on dit « non »
    /// change de machine. C'est la règle que [`crate::admission::admit`] applique déjà en
    /// accumulant ses refus au lieu de rendre le premier.
    HostShort {
        /// Le niveau le plus confiné que cet hôte prouve — sous le plancher, par construction.
        ceiling: SandboxLevel,
        /// Ce qui manque pour atteindre le plancher.
        missing: Vec<Missing>,
    },
}

impl Readiness {
    /// Le niveau sous lequel le driver n'a rien à quoi parler sur cet hôte.
    pub const FLOOR: SandboxLevel = SandboxLevel::S2;

    /// Lire ce que l'hôte prouve.
    #[must_use]
    pub fn assess(facts: &HostFacts) -> Self {
        let ceiling = facts.ceiling();
        let missing = facts.missing_for(Self::FLOOR);
        if missing.is_empty() {
            Self::Provable { ceiling }
        } else {
            Self::HostShort { ceiling, missing }
        }
    }

    /// Vrai quand cet hôte peut porter une sandbox.
    #[must_use]
    pub const fn is_provable(&self) -> bool {
        matches!(self, Self::Provable { .. })
    }

    /// Le niveau le plus confiné que cet hôte prouve, dans les deux cas.
    ///
    /// Rendu même quand l'hôte est court : « `S1`, et voici pourquoi pas `S2` » est une réponse,
    /// « rien » n'en est pas une.
    #[must_use]
    pub const fn ceiling(&self) -> SandboxLevel {
        match self {
            Self::Provable { ceiling } | Self::HostShort { ceiling, .. } => *ceiling,
        }
    }

    /// Ce qui manque pour atteindre le plancher — vide quand rien ne manque.
    #[must_use]
    pub fn missing(&self) -> &[Missing] {
        match self {
            Self::Provable { .. } => &[],
            Self::HostShort { missing, .. } => missing,
        }
    }
}

impl fmt::Display for Readiness {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provable { ceiling } => write!(
                formatter,
                "driver construit ; cet hôte prouve {ceiling} et peut porter une sandbox"
            ),
            Self::HostShort { ceiling, missing } => {
                write!(
                    formatter,
                    "driver construit ; cet hôte plafonne à {ceiling}, sous {}, et il y manque :",
                    Self::FLOOR
                )?;
                for item in missing {
                    write!(formatter, "\n  - {item}")?;
                }
                Ok(())
            }
        }
    }
}
