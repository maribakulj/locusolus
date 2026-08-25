//! La fiabilité observée — `W24.c`, ADR 0026 décision 4, note d'implémentation.
//!
//! # Le défaut que ce module existe pour rendre impossible
//!
//! L'ADR 0026 le nomme sous « note d'implémentation à ne pas perdre » : le modèle de réputation de la
//! source dont il tire le routage par intention est **inutilisable en l'état**. Chez elle, `s^F = 1`
//! signifie *faute*, donc `E[P]` est une probabilité de **mauvais** comportement et l'algorithme
//! filtre `E[P] < τ` ; mais `T` est de polarité **inverse**, admissible si `E[T] ≥ τ`, sur la même
//! machinerie Beta et la même règle de mise à jour.
//!
//! Deux conventions opposées dans un même mécanisme. Les transcrire produirait un filtre **inversé en
//! silence** : le code compile, les tests d'une moitié passent, et le système retient exactement les
//! pairs qu'il devait écarter.
//!
//! # Une seule polarité, et le nom la porte
//!
//! Ici, **plus c'est grand, plus c'est fiable**. Sans exception, dans tout le module.
//!
//! Le type s'appelle [`Reliability`] et pas `Reputation` : ce qui compte des fautes ne s'appelle pas
//! réputation, et un nom qui ne dit pas son sens est précisément ce qui permet à deux polarités de
//! cohabiter sans que personne ne s'en aperçoive. De même, [`Observation`] a deux variantes nommées
//! `Reliable` et `Unreliable`, jamais `success` et `fault` — les deux dernières se lisent dans les
//! deux sens selon qu'on parle du pair ou du risque.
//!
//! Il n'y a **qu'une** comparaison de seuil dans ce module, [`Reliability::admits`], et elle va dans
//! un seul sens. Un test lit la source et refuse toute comparaison de l'espérance dans l'autre.
//!
//! # Des entiers, pas des flottants
//!
//! L'espérance est rendue en **millièmes**, et la comparaison au seuil est exacte : `(reliable + 1) *
//! 1000 ≥ threshold * (reliable + unreliable + 2)`, en arithmétique entière. Un flottant rendrait
//! deux rejeux du même journal capables de trancher différemment au bord du seuil, et ce dépôt
//! demande partout que le rejeu soit reproductible.
//!
//! Le prior est uniforme — un succès et un échec fictifs, la règle de Laplace. Il est écrit ici plutôt
//! que paramétré : un prior réglable serait une valeur de politique, et §13 n'en a pas encore.
//!
//! # Elle influence le **rang**, jamais la validité
//!
//! Une observation ne mène nulle part vers un `Support` ni vers une prémisse d'`Inference` — ces deux
//! types vivent dans `packages/graph`, dont `packages/review` ne dépend pas, et un test le tient par
//! l'absence dans le manifeste **et** dans la source.
//!
//! C'est la même frontière que l'ADR 0022 décision 2 pose pour `MetaMemory` : « sans une
//! `MetaMemory` séparée, l'utilité passée d'un document finit par entrer dans son score de vérité —
//! le biais de citation reconstruit avec de l'apprentissage automatique ». Un pair peu fiable est
//! moins souvent choisi ; ce qu'il a dit ne devient pas faux pour autant, et l'invariant 12 dit
//! pourquoi les résultats négatifs ne se suppriment pas.

/// Ce qu'une interaction a montré.
///
/// Deux variantes nommées **du côté du pair**, jamais du côté du risque. `success` et `fault` se
/// lisent dans les deux sens selon ce dont on parle, et c'est exactement l'ambiguïté qui a rendu le
/// modèle de la source inutilisable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Observation {
    /// Le pair a tenu ce qu'on attendait de lui.
    Reliable,
    /// Il ne l'a pas tenu.
    Unreliable,
}

impl Observation {
    /// Les deux.
    pub const ALL: [Self; 2] = [Self::Reliable, Self::Unreliable];
}

/// La fiabilité observée d'un pair — **croissante**.
///
/// Zéro observation vaut une fiabilité **neutre**, pas nulle : n'avoir rien vu n'est pas avoir vu du
/// mauvais. C'est la règle 3 du rythme de session transposée au domaine — un compteur qui n'a rien lu
/// ne vaut pas zéro — et c'est le prior uniforme qui la porte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Reliability {
    reliable: u32,
    unreliable: u32,
}

impl Reliability {
    /// Aucune observation.
    #[must_use]
    pub const fn unobserved() -> Self {
        Self {
            reliable: 0,
            unreliable: 0,
        }
    }

    /// Enregistrer une observation.
    ///
    /// Saturante : un compteur qui reboucle ferait chuter une fiabilité établie d'un coup, et le
    /// plafond est une limite de représentation plutôt qu'une décision de politique.
    #[must_use]
    pub const fn observing(self, observation: Observation) -> Self {
        match observation {
            Observation::Reliable => Self {
                reliable: self.reliable.saturating_add(1),
                unreliable: self.unreliable,
            },
            Observation::Unreliable => Self {
                reliable: self.reliable,
                unreliable: self.unreliable.saturating_add(1),
            },
        }
    }

    /// L'espérance de fiabilité, en **millièmes**.
    ///
    /// `(reliable + 1) / (reliable + unreliable + 2)` — prior uniforme. Sans observation, 500 : le
    /// milieu, qui est ce que « je ne sais pas » veut dire ici.
    #[must_use]
    pub const fn expected_per_mille(self) -> u32 {
        let observed = self.reliable as u64 + self.unreliable as u64;
        let numerator = (self.reliable as u64 + 1) * 1_000;
        let denominator = observed + 2;
        // Le quotient est **strictement inférieur à 1 000** : le numérateur vaut
        // `(reliable + 1) * 1000` et le dénominateur `reliable + unreliable + 2 ≥ reliable + 2`,
        // donc le rapport est majoré par `(reliable + 1) / (reliable + 2) < 1`. `#[expect]` plutôt
        // que `#[allow]` : si la démonstration cessait d'être nécessaire, l'attribut rougirait.
        #[expect(
            clippy::cast_possible_truncation,
            reason = "quotient majoré par 1 000, démontré ci-dessus"
        )]
        let per_mille = (numerator / denominator) as u32;
        per_mille
    }

    /// Le pair passe-t-il le seuil ?
    ///
    /// **L'unique comparaison de seuil du module, et elle va dans un seul sens** : est admis ce qui
    /// est *au moins* aussi fiable que demandé. Une seconde comparaison, fût-elle correcte, rouvrirait
    /// la porte par laquelle la source a fait entrer deux polarités.
    ///
    /// Exacte, en entiers : `(reliable + 1) * 1000 ≥ threshold * (reliable + unreliable + 2)`.
    ///
    /// **Et cette forme est aujourd'hui équivalente** à `expected_per_mille() >= threshold`, parce
    /// que `floor(x) ≥ t ⟺ x ≥ t` pour un `t` entier. Une première rédaction de ce commentaire
    /// affirmait le contraire — « passer par les millièmes perdrait la partie fractionnaire » — et
    /// c'était faux ; un mutant l'a montré en survivant, et la phrase a été corrigée plutôt que le
    /// test relâché.
    ///
    /// La forme exacte reste écrite ainsi pour une raison qui, elle, tient : elle ne dépend pas du
    /// mode d'arrondi de [`Reliability::expected_per_mille`]. Si cette fonction passait un jour à
    /// l'arrondi au plus proche, ou changeait d'unité, la sémantique du seuil suivrait sans que
    /// personne l'ait décidé. Un test fige donc la troncature de l'une, et l'autre s'en passe.
    #[must_use]
    pub const fn admits(self, threshold_per_mille: u32) -> bool {
        let observed = self.reliable as u64 + self.unreliable as u64;
        (self.reliable as u64 + 1) * 1_000 >= threshold_per_mille as u64 * (observed + 2)
    }

    /// Combien de fois le pair a tenu.
    #[must_use]
    pub const fn reliable(self) -> u32 {
        self.reliable
    }

    /// Combien de fois il n'a pas tenu.
    #[must_use]
    pub const fn unreliable(self) -> u32 {
        self.unreliable
    }

    /// Combien d'observations en tout.
    #[must_use]
    pub const fn observed(self) -> u64 {
        self.reliable as u64 + self.unreliable as u64
    }
}
