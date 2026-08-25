//! La classe de cognition d'une mission — `W25.a`, ADR 0026 décision 6.
//!
//! # Le levier n'est pas le modèle, c'est l'affectation
//!
//! L'ADR 0026 appelle cette mesure « la plus actionnable du dossier, et la seule industrielle » : à
//! qualité identique vérifiée par test caché, un facteur 7,9 sur le coût total et 22 sur la flotte de
//! workers seule. Et il en donne la forme en une phrase — « frontière pour **planifier**, bon marché
//! pour **exécuter** ».
//!
//! Une mission déclare donc une **classe**, jamais un modèle. L'affectation classe → modèle est une
//! valeur de politique, versionnée et visible, jamais une constante de code.
//!
//! # Ce que ce fichier ne contient pas, et c'est l'essentiel
//!
//! **Aucun identifiant de modèle.** Pas de nom de fournisseur, pas de numéro de version, pas de
//! table. Le domaine ne sait pas quel modèle sert une classe, et il n'a aucun moyen de l'apprendre :
//! l'affectation vit dans la politique, sous forme de **données** indexées par [`CognitionClass::slug`].
//!
//! C'est ce qui rend vraie la clause qui porte l'item — *changer l'affectation ne change aucun type*.
//! Elle n'est pas tenue par une convention qu'on respecterait : il n'y a rien à changer, parce que
//! rien ici ne nomme un modèle.
//!
//! # Deux valeurs, et pas trois
//!
//! L'ADR nomme deux pôles, et deux seulement. `CLAUDE.md` demande qu'une valeur d'énumération n'entre
//! que lorsqu'un consommateur exécutable et testé existe — une troisième classe « intermédiaire »
//! s'écrirait sans que rien ne rougisse, et ce serait exactement la promesse que l'ADR 0022
//! décision 0 refuse : un type qui annonce une distinction dont personne ne se sert.
//!
//! Le jour où une politique a besoin d'un troisième barreau, il entrera avec elle.
//!
//! # Les noms disent le coût, pas l'usage
//!
//! `Frontier` et `Economy` nomment ce que la classe **est** — le haut et le bas de la gamme —, pas ce
//! à quoi elle sert. Les appeler `Planning` et `Execution` figerait dans le type l'usage que l'ADR en
//! donne aujourd'hui, et une mission d'exécution qui aurait besoin de la frontière devrait alors
//! demander une classe qui dit le contraire de ce qu'elle fait.

use std::fmt;

use serde::{Deserialize, Serialize};

/// La classe de cognition qu'une mission déclare.
///
/// **Une classe, jamais un modèle.** Le type ne porte aucun identifiant de modèle et n'en accepte
/// aucun ; il n'y a pas de variante « ce modèle-là ».
///
/// # La forme sur le fil et [`Self::slug`] ne peuvent pas diverger
///
/// `serde(rename_all = "snake_case")` produit exactement les slugs ci-dessous, et un test l'exige
/// pour **chaque** valeur. Deux sources de vérité pour un nom de wire est le genre d'écart qui ne se
/// voit qu'au moment où un journal relu ne se reconnaît plus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CognitionClass {
    /// Le haut de la gamme, celui qu'on réserve à ce qui décide.
    Frontier,
    /// Le bon marché, celui qui exécute.
    Economy,
}

impl CognitionClass {
    /// Les deux que l'ADR 0026 décision 6 nomme.
    pub const ALL: [Self; 2] = [Self::Frontier, Self::Economy];

    /// Son nom.
    ///
    /// C'est par lui que la politique indexe son affectation : le domaine expose une **clé**, et la
    /// politique y attache une valeur qu'elle est seule à connaître.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Frontier => "frontier",
            Self::Economy => "economy",
        }
    }

    /// Relire une classe.
    ///
    /// Dérivée de [`Self::ALL`] et de [`Self::slug`], comme `InstanceState::parse` : une seconde
    /// table divergerait de la première au premier barreau ajouté.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|class| class.slug() == value)
    }
}

impl fmt::Display for CognitionClass {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}
