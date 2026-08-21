//! `edge_churn` — le renouvellement des arêtes entre deux versions. `W21.b`, ADR 0024.
//!
//! # Le solde n'est pas le churn, et c'est tout l'objet de la métrique
//!
//! Deux arêtes qui disparaissent pendant que deux autres apparaissent laissent le compte d'arêtes
//! **inchangé**. Une organisation lue de loin paraît alors stable, alors qu'elle s'est recomposée
//! entièrement en dessous. Le churn vaut quatre ; le solde vaut zéro ; et c'est le solde qu'on lit
//! par défaut, parce que c'est celui qui se calcule sans y penser.
//!
//! Le piège n'est pas théorique : il vit déjà dans ce dépôt sous ce nom, dans les tests de
//! [`crate::region`], où un plafond de deux refuse quatre identités changées à solde nul.
//!
//! Ce module ne rend donc **aucun** solde. Ce n'est pas une omission : un accesseur qui rendrait la
//! différence de cardinalité serait à un caractère de distance de celui qu'il faut lire, porterait
//! un nom tout aussi plausible, et rendrait un nombre plus petit — donc plus rassurant. Un test
//! d'absence refuse ces signatures dans la source.
//!
//! # Pourquoi entre deux versions, et non sur une suite d'opérations
//!
//! Compter les `ADD_EDGE` et les `REMOVE_EDGE` d'un rejeu serait plus direct, et **manquerait des
//! arêtes**. Trois opérations changent les arêtes sans être l'une des deux : `REPLACE_NODE` emporte
//! les arêtes de l'identité remplacée, `SPLIT_NODE` partage celles du nœud scindé, `MERGE_NODES`
//! les réunit. Un churn tiré des seules opérations d'arête rendrait zéro sur un remplacement qui a
//! réécrit toute la voisinage d'un nœud.
//!
//! La différence symétrique des ensembles d'arêtes les voit toutes, quelle que soit l'opération qui
//! les a produites. Un test le tient en comparant, sur un remplacement, ce que ce module rend et ce
//! que le compteur de [`crate::mutations`] rend : deux contre zéro.
//!
//! # Ce que le churn ne dit pas
//!
//! Il ne dit pas si le renouvellement était **utile**. Une organisation qui se cherche produit
//! beaucoup de churn, et une organisation qui se dégrade aussi ; les distinguer demande de savoir ce
//! que le travail a produit, ce qu'aucune mesure de structure ne contient.
//!
//! Il ne dit rien non plus du **chemin** : deux versions qui se ressemblent peuvent avoir été
//! séparées par cent opérations qui se sont annulées. C'est [`crate::mutations`] qui mesure le
//! chemin, et leur écart est le détour — décision 3 de l'ADR 0024.
//!
//! Et il ne juge pas : aucun seuil, aucune note, aucun verdict (décision 9).

use std::collections::BTreeSet;

use crate::proposal::Relation;
use crate::version::Version;

/// Les arêtes qui sont entrées et celles qui sont sorties, entre deux versions.
///
/// Les deux ensembles sont rendus, et non leurs seules tailles : savoir **quelles** arêtes ont
/// changé est ce qui rend un churn actionnable. Un nombre seul envoie relire les deux versions à la
/// main pour retrouver ce que la mesure vient déjà de calculer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeChurn {
    entered: BTreeSet<Relation>,
    left: BTreeSet<Relation>,
}

impl EdgeChurn {
    /// Mesurer le renouvellement entre deux versions.
    ///
    /// L'ordre des arguments porte le sens : `before` puis `after`. Les inverser échange
    /// [`Self::entered`] et [`Self::left`] sans changer [`Self::total`].
    #[must_use]
    pub fn between(before: &Version, after: &Version) -> Self {
        let (was, is) = (before.relations(), after.relations());
        Self {
            entered: is.difference(was).copied().collect(),
            left: was.difference(is).copied().collect(),
        }
    }

    /// Les arêtes présentes dans `after` et absentes de `before`.
    #[must_use]
    pub const fn entered(&self) -> &BTreeSet<Relation> {
        &self.entered
    }

    /// Les arêtes présentes dans `before` et absentes de `after`.
    #[must_use]
    pub const fn left(&self) -> &BTreeSet<Relation> {
        &self.left
    }

    /// Le churn : entrées **plus** sorties — `edge_churn` proprement dit.
    #[must_use]
    pub fn total(&self) -> usize {
        self.entered.len() + self.left.len()
    }

    /// Vrai quand aucune arête n'a changé.
    ///
    /// Distinct d'un solde nul, et c'est la distinction que porte tout ce module : un churn nul
    /// signifie que les deux ensembles d'arêtes sont **le même**, ce qu'un solde nul ne dit pas.
    #[must_use]
    pub fn is_still(&self) -> bool {
        self.entered.is_empty() && self.left.is_empty()
    }
}
