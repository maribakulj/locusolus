//! `applied_edit_length` — la longueur du diff entre deux versions. `W21.c`, ADR 0024.
//!
//! # Ce n'est pas une distance, et le nom le dit
//!
//! La **distance d'édition** entre deux graphes est le nombre *minimal* d'opérations pour passer de
//! l'un à l'autre. Ce module ne la calcule pas. Il mesure la longueur du diff que
//! [`crate::diff::Diff::between`] produit — *une* suite qui mène de `a` à `b`, celle que le système
//! montre effectivement à un approbateur.
//!
//! Le nom d'origine dans la matrice d'acceptation était `graph_edit_distance`, et l'ADR 0024
//! décision 2 l'a écarté : publier cette longueur sous le nom de « distance » affirmerait une
//! minimalité qu'aucun code ici ne calcule. Aucune signature de ce module ne porte donc ce nom, et
//! un test d'absence le tient.
//!
//! # Pourquoi elle n'est pas minimale, et ce n'est pas une affaire de complexité
//!
//! On peut invoquer la NP-difficulté du problème général. Ce serait vrai et hors sujet : la raison
//! pour laquelle *cette* longueur n'est pas minimale est écrite dans `diff.rs`, et elle est
//! délibérée.
//!
//! `Diff::between` n'émet que **quatre** sortes d'opérations — retirer des arêtes, retirer des
//! nœuds, ajouter des nœuds, ajouter des arêtes. Il n'infère jamais un `REPLACE_NODE`, un
//! `SPLIT_NODE` ni un `MERGE_NODES`, parce qu'au niveau des états un remplacement est indiscernable
//! d'un retrait suivi d'un ajout, et que deviner ferait lire à un approbateur une intention que
//! personne n'a écrite.
//!
//! Conséquence directe : un remplacement de nœud qui a coûté **une** opération au chemin réel se
//! relit comme **quatre** dans le diff. La longueur mesurée est donc une borne supérieure de la
//! distance véritable, et l'écart peut être large. Un test l'exhibe sur ce cas précis, plutôt que de
//! le déduire d'un théorème.
//!
//! # Le détour, et pourquoi il ne se calcule pas par une soustraction
//!
//! La décision 3 de l'ADR 0024 énonce que l'écart entre le chemin ([`crate::mutations`]) et la
//! destination (ce module) est le **détour** : le travail de coordination qui n'a laissé aucune
//! trace. C'est vrai lorsque les deux comptent dans le même vocabulaire — un aller-retour d'arête
//! coûte deux au chemin et zéro au diff.
//!
//! Ce n'est **pas** vrai en général, et l'implémentation l'a montré : quand le chemin emploie une
//! opération que le diff ne sait pas exprimer, le diff est plus long que le chemin, et la
//! soustraction change de signe. Un détour lu comme un nombre négatif n'a pas de sens ; il faut
//! comparer ce qui se compare. [`AppliedEdit::detour_from`] rend donc `None` dans ce cas plutôt
//! qu'un entier signé, parce qu'une valeur négative serait affichée, et lue comme « moins que
//! rien ».

use crate::diff::Diff;
use crate::version::{Operation, Version};

/// La longueur du diff entre deux versions, et les opérations comptées.
///
/// Les opérations sont rendues avec la longueur : un nombre seul envoie relire le diff pour savoir
/// ce qu'il contenait, et c'est ce diff-là qui explique pourquoi la longueur vaut ce qu'elle vaut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedEdit {
    operations: Vec<Operation>,
}

impl AppliedEdit {
    /// Mesurer le diff qui mène de `from` à `to`.
    #[must_use]
    pub fn between(from: &Version, to: &Version) -> Self {
        Self {
            operations: Diff::between(from, to).operations().to_vec(),
        }
    }

    /// La longueur — `applied_edit_length` proprement dit.
    ///
    /// Borne **supérieure** de la distance d'édition véritable : voir la documentation du module.
    #[must_use]
    pub fn length(&self) -> usize {
        self.operations.len()
    }

    /// Les opérations que le diff porte.
    #[must_use]
    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }

    /// Vrai quand les deux versions ne diffèrent par aucune opération.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    /// Le détour : ce que le chemin a coûté en plus de la destination.
    ///
    /// `None` quand le chemin est **plus court** que le diff, ce qui arrive dès qu'il emploie une
    /// opération que le diff ne sait pas exprimer. Rendre un entier signé dans ce cas produirait un
    /// détour négatif, qui serait affiché et lu comme « moins que rien » alors qu'il signifie « ces
    /// deux mesures ne se comparent pas ici ».
    #[must_use]
    pub fn detour_from(&self, path: usize) -> Option<usize> {
        path.checked_sub(self.length())
    }
}
