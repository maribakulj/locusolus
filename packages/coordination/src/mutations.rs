//! `mutations_per_run` — ce qu'une exécution a fait à l'organisation. `W21.a`, ADR 0024.
//!
//! # Le compte est un rejeu, et c'est ce qui rend « appliquée » vérifiable
//!
//! La métrique compte les opérations **appliquées**, jamais les proposées. Une fonction qui
//! recevrait une liste d'opérations et les compterait tiendrait cette distinction par la seule
//! bonne foi de l'appelant : c'est lui qui aurait trié, et rien dans le calcul ne le vérifierait.
//! Un test écrit contre une telle fonction ne prouverait rien non plus — il compterait ce qu'il a
//! lui-même trié.
//!
//! [`Mutations::replay`] rejoue donc la suite contre une version de départ, et ne compte que ce que
//! [`Version::apply`] accepte. « Appliquée » cesse d'être une promesse d'appelant et devient une
//! propriété du calcul.
//!
//! C'est aussi ce que la matrice exige — « calculées depuis le seul journal ». La forme canonique
//! d'une opération est sa forme de transport ([`Operation::parse`]) : ce qui est rejoué ici est
//! exactement ce que le journal a écrit, et non une seconde représentation dont il faudrait prouver
//! qu'elle dit la même chose.
//!
//! # Une opération qui ne s'applique pas fait échouer le rejeu, elle ne se saute pas
//!
//! La tentation est de l'ignorer et de continuer. Elle est mauvaise. Une opération que la version ne
//! reçoit pas signifie l'une de deux choses : le rejeu part de la mauvaise racine, ou le journal est
//! corrompu. Dans les deux cas, poursuivre produirait un compte **pour une histoire qui n'a pas eu
//! lieu** — un nombre plausible, du genre exact que la décision 1 de l'ADR 0024 refuse, puisque rien
//! dans son apparence ne le distinguerait d'un nombre juste.
//!
//! Le refus nomme donc le **rang** et la sorte. Un rejeu qui échouerait sans dire où laisserait
//! chercher dans toute la suite, et la première réaction serait de le relancer depuis une autre
//! racine jusqu'à ce qu'il passe — ce qui est la façon la plus discrète de choisir son résultat.
//!
//! # Les dix sortes sont toujours présentes, à zéro s'il le faut
//!
//! Une sorte qui n'est pas survenue vaut **zéro**, et zéro est un fait. Une clé absente ne serait
//! pas un fait : elle ne dirait pas si la sorte n'est jamais survenue ou si le compteur ne la
//! connaît pas, et ces deux-là appellent des suites opposées — regarder l'organisation, ou réparer
//! le compteur.
//!
//! L'ensemble des clés est donc [`Operation::NAMES`], et un test le tient par égalité. Une
//! onzième sorte qui entrerait dans l'énumération sans entrer ici ferait apparaître une clé
//! inconnue au premier rejeu qui la rencontre, et l'égalité échouerait — ce qui est le
//! comportement voulu, puisque le compteur aurait alors une lacune que personne n'a décidée.
//!
//! # Ce que ce module ne fait pas
//!
//! Il ne juge pas. Aucun seuil, aucune note, aucun verdict — décision 9 de l'ADR 0024, qui étend à
//! toute la famille la règle que `R3` avait posée pour ses cinq métriques. Un compte élevé
//! d'opérations n'est pas une faute : une organisation qui se cherche en produit beaucoup, et
//! décider à partir de quand c'est trop est une question de politique et de portefeuille (§13,
//! §20), pas une constante qu'on écrit en Rust.
//!
//! Il ne dit rien non plus de l'**utilité** de ces opérations. Ajouter une arête puis la retirer
//! compte deux, et c'est correct : le chemin parcouru n'est pas la destination atteinte. Ce que ce
//! travail a laissé dans la structure finale est la mesure de `W21.c`, et leur écart est le détour.

use std::collections::BTreeMap;
use std::fmt;

use crate::version::{Digest, Operation, Version, VersionError};

/// Les opérations appliquées, par sorte.
///
/// Les dix clés d'[`Operation::NAMES`] sont toujours présentes : voir la documentation du module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mutations {
    by_sort: BTreeMap<&'static str, usize>,
}

impl Mutations {
    /// Rejouer une suite d'opérations contre une version de départ, et compter ce qui s'applique.
    ///
    /// Rend le compte **et** la version atteinte : un rejeu produit les deux, et ne rendre que le
    /// compte obligerait un appelant qui veut vérifier où il a atterri à rejouer une seconde fois.
    ///
    /// # Errors
    ///
    /// [`MutationsError::NotApplicable`] dès la première opération que la version refuse, en
    /// nommant son rang, sa sorte et la cause. Le rejeu s'arrête là : voir la documentation du
    /// module pour pourquoi il ne saute pas.
    pub fn replay(
        from: &Version,
        operations: &[Operation],
        digest: &impl Digest,
    ) -> Result<Replay, MutationsError> {
        let mut by_sort: BTreeMap<&'static str, usize> =
            Operation::NAMES.iter().map(|name| (*name, 0)).collect();
        let mut landed = from.clone();

        for (index, operation) in operations.iter().enumerate() {
            landed =
                landed
                    .apply(operation, digest)
                    .map_err(|cause| MutationsError::NotApplicable {
                        index,
                        sort: operation.name(),
                        cause,
                    })?;
            // `or_insert` plutôt qu'un accès à une clé supposée présente : une sorte absente de
            // `NAMES` produit ainsi une clé de plus, que le test d'exhaustivité voit. La faire
            // échouer ici la rendrait invisible au test et bruyante en production, ce qui est
            // l'inverse de ce qu'on veut d'une lacune de compteur.
            *by_sort.entry(operation.name()).or_insert(0) += 1;
        }

        Ok(Replay {
            mutations: Self { by_sort },
            landed,
        })
    }

    /// Le compte d'une sorte, nommée comme dans [`Operation::NAMES`].
    ///
    /// Zéro pour une sorte connue qui n'est pas survenue, `None` pour un nom que le compteur ne
    /// connaît pas — un appelant qui interroge `"ADD_NOD"` doit l'apprendre, pas lire zéro.
    #[must_use]
    pub fn of_sort(&self, sort: &str) -> Option<usize> {
        self.by_sort.get(sort).copied()
    }

    /// Toutes les sortes et leur compte.
    #[must_use]
    pub const fn by_sort(&self) -> &BTreeMap<&'static str, usize> {
        &self.by_sort
    }

    /// Le total, toutes sortes confondues — `mutations_per_run` proprement dit.
    #[must_use]
    pub fn total(&self) -> usize {
        self.by_sort.values().sum()
    }
}

/// Ce qu'un rejeu produit : le compte, et la version atteinte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replay {
    mutations: Mutations,
    landed: Version,
}

impl Replay {
    /// Le compte.
    #[must_use]
    pub const fn mutations(&self) -> &Mutations {
        &self.mutations
    }

    /// La version atteinte au bout de la suite.
    #[must_use]
    pub const fn landed(&self) -> &Version {
        &self.landed
    }
}

/// Pourquoi un rejeu s'arrête.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MutationsError {
    /// La version a refusé cette opération.
    NotApplicable {
        /// Son rang dans la suite rejouée, à partir de zéro.
        index: usize,
        /// Sa sorte.
        sort: &'static str,
        /// Ce que la version a répondu.
        cause: VersionError,
    },
}

impl fmt::Display for MutationsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotApplicable { index, sort, cause } => write!(
                formatter,
                "l'opération de rang {index} ({sort}) ne s'applique pas : {cause}. Le rejeu part \
                 d'une racine qui n'est pas la sienne, ou la suite n'est pas celle du journal — \
                 poursuivre compterait une histoire qui n'a pas eu lieu"
            ),
        }
    }
}

impl std::error::Error for MutationsError {}
