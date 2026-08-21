//! L'**objet** d'une écriture de budget — `W21.m`, ADR 0024.
//!
//! # Ce que `EntryKind` dit, et ce qu'il ne dit pas
//!
//! [`crate::EntryKind`] distingue le **mouvement** : allouer, retenir, rendre, constater, ajuster,
//! rembourser. Il ne dit jamais **pour quoi** l'écriture paie. Une consommation de jetons est une
//! consommation de jetons, qu'elle ait servi à faire le travail ou à se mettre d'accord sur qui le
//! ferait.
//!
//! Ce module ajoute l'objet manquant, et rien d'autre. C'est la dépendance technique nommée que
//! l'ADR 0024 invoque pour reporter `communication_tokens` — le dépôt savait **compter** les jetons,
//! pas les **classer**.
//!
//! # Une écriture non classée n'est pas une écriture de travail
//!
//! C'est la décision de cet item, et elle décide de la forme du type. Toute la difficulté d'ajouter
//! un champ à un journal existant tient dans ce que valent les écritures d'avant. Le défaut
//! serviable — « on suppose du travail » — est le pire des deux :
//!
//! - il est **majoritairement juste**, donc il ne se voit pas ;
//! - et il fausse le rapport dans le sens qui rassure, en faisant paraître la coordination moins
//!   chère qu'elle n'est, exactement là où `communication_tokens` sert à la mesurer.
//!
//! [`Classification::Unclassified`] nomme donc l'ignorance, et [`Spend`] ne la contient pas : un
//! classificateur ne peut pas *choisir* « non classé », il ne peut que constater qu'il n'y a rien à
//! lire. C'est la même séparation que les deux verdicts de `xiiif` §19 — une absence de mesure n'est
//! pas une mesure atténuée.
//!
//! # Aucune classification ne se déduit de `reason`
//!
//! Chaque écriture porte un motif en texte libre, et il serait facile d'y chercher « handoff » ou
//! « négociation ». Une justesse qui dépendrait de la rédaction de chaque appelant se dégraderait au
//! premier qui écrit autrement — et se dégraderait **en silence**, puisqu'un motif non reconnu
//! retomberait sur le défaut.
//!
//! La règle tient ici par la **forme** avant de tenir par un test : [`inherited`] ne reçoit pas les
//! écritures, il reçoit des paires « retenue, classification ». Le motif ne lui est pas passé, donc
//! il ne peut pas le lire, même par erreur. Un test d'absence garde la porte, mais il garde une
//! porte que le type a déjà fermée.

use std::fmt;

use locus_protocol::{Id, id::provisional::Reservation};

/// Ce qu'une écriture paie.
///
/// Deux valeurs, et pas de troisième pour l'ignorance : voir [`Classification`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Spend {
    /// Se mettre d'accord — négocier, transmettre, réviser, arbitrer.
    Coordination,
    /// Faire la chose elle-même.
    Work,
}

impl Spend {
    /// Les deux.
    pub const ALL: [Self; 2] = [Self::Coordination, Self::Work];

    /// Son nom.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Coordination => "coordination",
            Self::Work => "work",
        }
    }
}

impl fmt::Display for Spend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Ce qu'on sait de l'objet d'une écriture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Classification {
    /// L'appelant l'a dit.
    Classified(
        /// Ce qu'elle paie.
        Spend,
    ),
    /// Personne ne l'a dit — et ce n'est **pas** du travail par défaut : voir la documentation du
    /// module.
    #[default]
    Unclassified,
}

impl Classification {
    /// Ce que l'écriture paie, si on le sait.
    #[must_use]
    pub const fn spend(self) -> Option<Spend> {
        match self {
            Self::Classified(spend) => Some(spend),
            Self::Unclassified => None,
        }
    }

    /// Vrai quand quelqu'un l'a dit.
    #[must_use]
    pub const fn is_classified(self) -> bool {
        matches!(self, Self::Classified(_))
    }
}

impl From<Spend> for Classification {
    fn from(spend: Spend) -> Self {
        Self::Classified(spend)
    }
}

impl fmt::Display for Classification {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Classified(spend) => formatter.write_str(spend.slug()),
            Self::Unclassified => formatter.write_str("non classée"),
        }
    }
}

/// **Le classificateur** — ce qu'une écriture dérivée hérite de la retenue qu'elle solde.
///
/// Rendre, constater et rapprocher ne redéclarent rien : rembourser de la coordination reste de la
/// coordination. Redemander l'objet à chaque solde ouvrirait la porte à deux réponses différentes
/// pour la même retenue, et le journal porterait alors une contradiction que personne n'aurait
/// voulue.
///
/// La première écriture qui nomme la retenue est celle qui l'a créée — [`crate::EntryKind::Reservation`]
/// est le seul mouvement qui en produit une —, donc la première correspondance décide, et les
/// suivantes, qui héritent déjà, redonnent la même réponse.
///
/// Ce que cette fonction **ne reçoit pas** est aussi important que ce qu'elle reçoit : ni le motif,
/// ni les montants, ni le mouvement. Voir la documentation du module.
pub(crate) fn inherited<'a>(
    written: impl IntoIterator<Item = (Option<&'a Id<Reservation>>, Classification)>,
    reservation: &Id<Reservation>,
) -> Classification {
    for (against, classification) in written {
        if against == Some(reservation) {
            return classification;
        }
    }
    Classification::Unclassified
}
