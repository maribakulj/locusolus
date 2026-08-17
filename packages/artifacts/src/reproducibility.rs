//! Les niveaux de reproductibilité — `docs/SPEC_V1.md` §19.7.
//!
//! # La règle qui gouverne ce module
//!
//! §19.7 déclare cinq niveaux, et le schéma du `RunManifest` dit du champ qui les porte qu'il est
//! « déclaré par le producteur et **vérifiable depuis le reste du manifeste** — c'est précisément
//! ce qui le rend contestable ». Ce module est cette vérification.
//!
//! C'est la troisième fois que la même forme revient dans ce dépôt : l'attestation de sandbox
//! (W4.d.2) vient de l'observation et non de la demande, le digest de build (W5.e) se lit sur la
//! sortie du runtime et ne se compose pas depuis le blueprint, et un niveau de reproductibilité se
//! **calcule** depuis ce que le manifeste consigne. Un champ qui s'auto-atteste n'atteste rien.

use std::fmt;

/// Les cinq niveaux de §19.7, dans l'ordre croissant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    /// Narration uniquement.
    R0,
    /// Inputs et code identifiés.
    R1,
    /// Environnement verrouillé.
    R2,
    /// Reproduction automatisée sur backend compatible.
    R3,
    /// Reproduction indépendante sur worker distinct, avec comparaison structurée.
    R4,
}

impl Level {
    /// Les cinq, dans l'ordre.
    pub const ALL: [Self; 5] = [Self::R0, Self::R1, Self::R2, Self::R3, Self::R4];

    /// Le plus haut niveau qu'un manifeste seul puisse porter.
    ///
    /// `R3` et `R4` ne sont pas des propriétés d'un document : ce sont des **événements**. « Une
    /// reproduction automatisée a eu lieu » et « un worker distinct a retrouvé les mêmes sorties »
    /// se constatent en rejouant, pas en lisant. Un manifeste qui les déclare décrit quelque chose
    /// qui n'a pas encore de trace — W6.e produira cette trace, et c'est elle qui les portera.
    pub const FROM_A_MANIFEST_ALONE: Self = Self::R2;

    /// Le nom que le schéma emploie.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::R0 => "R0",
            Self::R1 => "R1",
            Self::R2 => "R2",
            Self::R3 => "R3",
            Self::R4 => "R4",
        }
    }

    /// Relire un niveau.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|level| level.slug() == value)
    }
}

impl fmt::Display for Level {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Ce qui manque à un manifeste pour monter d'un cran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Missing {
    /// Aucun input identifié par hash.
    ///
    /// Nommer un input par son chemin ne dit pas ce qu'il contenait au moment du run.
    Inputs,
    /// Aucune révision de code.
    CodeRevision,
    /// L'arbre de travail portait des modifications non commitées.
    ///
    /// Le schéma le dit déjà : « un run dirty ne peut pas prétendre à R1, et cacher le champ ne le
    /// rendrait pas reproductible ».
    DirtyTree,
    /// Une reproduction, qui ne se lit pas dans un manifeste.
    ReproductionNotEvidenced,
}

impl fmt::Display for Missing {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Inputs => "aucun input identifié par hash",
            Self::CodeRevision => "aucune révision de code identifiée",
            Self::DirtyTree => "l'arbre de travail portait des modifications non commitées",
            Self::ReproductionNotEvidenced => {
                "aucune reproduction n'est attestée : R3 et R4 se constatent en rejouant"
            }
        };
        formatter.write_str(message)
    }
}

/// Ce qui empêche un run de monter, sans l'empêcher d'exister.
///
/// Ni un refus ni un silence. Un caveat est ce qu'on sait ne pas savoir, gardé auprès du verdict
/// plutôt que perdu — la même forme que `Support::Undetermined` de W4.d.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Caveat {
    /// Aucun seed consigné.
    ///
    /// Rien dans le manifeste ne dit si le run est stochastique, et rien ne peut le dire : ce
    /// n'est pas une propriété du document. Si le run l'était, le niveau atteint est plus bas que
    /// ce qui est calculé ici, et personne ne le saura en relisant. Le caveat existe pour que la
    /// question soit posée à qui peut y répondre.
    NoSeeds,
}

impl fmt::Display for Caveat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSeeds => formatter.write_str(
                "aucun seed consigné : si le run est stochastique, le niveau calculé est optimiste",
            ),
        }
    }
}

/// Le verdict rendu sur un manifeste.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assessment {
    /// Le niveau que le manifeste soutient.
    pub attained: Level,
    /// Ce qui manque pour aller plus haut.
    pub missing: Vec<Missing>,
    /// Ce qui reste incertain.
    pub caveats: Vec<Caveat>,
}

impl Assessment {
    /// Vrai quand le niveau demandé est soutenu.
    #[must_use]
    pub fn supports(&self, claimed: Level) -> bool {
        claimed <= self.attained
    }
}

impl fmt::Display for Assessment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.attained)?;
        for missing in &self.missing {
            write!(formatter, " ; {missing}")?;
        }
        for caveat in &self.caveats {
            write!(formatter, " ; {caveat}")?;
        }
        Ok(())
    }
}
