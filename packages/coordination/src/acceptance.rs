//! `accepted_mutation_rate` — la part des propositions qui est approuvée. `W21.d`, ADR 0024.
//!
//! # Le dénominateur ne contient pas l'indécis
//!
//! Une proposition encore `proposed` n'est ni acceptée ni refusée. La compter au dénominateur fait
//! baisser le taux pour une raison qui n'a rien à voir avec la qualité des propositions : **la
//! lenteur des décideurs**. Un système dont la gouvernance prend du retard verrait son « taux
//! d'acceptation » chuter sans qu'aucun agent n'ait changé de comportement, et la lecture naturelle
//! — « les agents proposent n'importe quoi » — serait fausse, et coûteuse : on irait corriger les
//! agents.
//!
//! Les indécises sont donc comptées **à part**, et rendues avec le taux. C'est la règle que `W18.e`
//! a posée pour l'acceptation des adaptations — une adaptation que personne n'a regardée est
//! déclarée hors mesure, jamais comptée comme acceptée, parce que le silence n'est pas un accord —
//! transposée aux mutations de coordination.
//!
//! # Une révocation ne défait pas une acceptation
//!
//! `revoked` désigne une décision qui **a été approuvée**, puis annulée après coup. Elle compte donc
//! au numérateur comme au dénominateur : au moment de la décision, elle a bien été acceptée.
//!
//! L'exclure ferait baisser le taux d'acceptation **rétroactivement**, à chaque révocation, ce qui
//! fondrait deux signaux distincts en un seul nombre : « les propositions passent-elles ? » et
//! « celles qui passent tiennent-elles ? ». La seconde question est celle de `rollback_rate`
//! (`W21.e`), et un taux d'acceptation qui bougerait en même temps rendrait les deux illisibles.
//!
//! Un test le tient : révoquer une décision approuvée ne change pas le taux.
//!
//! # Aucune décision terminale ne rend pas zéro
//!
//! [`MutationAcceptance::rate`] rend `None` quand rien n'a été décidé, jamais `0.0`. Les deux se
//! lisent « rien n'est accepté », et l'un des deux est faux : zéro signifie que tout ce qui a été
//! décidé a été refusé, `None` que rien n'a encore été décidé. Ils appellent des suites opposées —
//! regarder les propositions, ou attendre les décideurs.
//!
//! # Ce que ce module ne fait pas
//!
//! Il ne juge pas — décision 9 de l'ADR 0024. Un taux d'acceptation bas n'est pas une faute : une
//! gouvernance exigeante en produit, et c'est ce qu'on lui demande. Un taux haut n'est pas une
//! réussite non plus : il peut signifier que personne ne regarde. Ce qui distingue les deux est le
//! taux d'annulation, et c'est une autre métrique.

use std::fmt;

use crate::decision::DecisionState;

/// Ce que sont devenues des propositions de mutation.
///
/// Trois comptes, jamais deux : accepté, refusé, et **en attente**. Le troisième n'entre dans aucun
/// des deux premiers — voir la documentation du module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MutationAcceptance {
    accepted: usize,
    refused: usize,
    pending: usize,
}

impl MutationAcceptance {
    /// Compter une suite d'états de décision.
    #[must_use]
    pub fn over<'a>(states: impl IntoIterator<Item = &'a DecisionState>) -> Self {
        let mut counted = Self::default();
        for state in states {
            match state {
                // `Revoked` a été approuvée avant d'être annulée : au moment de la décision, elle a
                // bien été acceptée. Voir la documentation du module.
                DecisionState::Approved | DecisionState::Revoked => counted.accepted += 1,
                DecisionState::Rejected => counted.refused += 1,
                DecisionState::Proposed => counted.pending += 1,
            }
        }
        counted
    }

    /// Les propositions approuvées — le numérateur.
    #[must_use]
    pub const fn accepted(self) -> usize {
        self.accepted
    }

    /// Les propositions refusées.
    #[must_use]
    pub const fn refused(self) -> usize {
        self.refused
    }

    /// Les propositions encore en attente d'une décision.
    ///
    /// Rendues **avec** le taux, et jamais fondues dedans : un taux dont on ignore combien de
    /// propositions attendent encore ne se lit pas.
    #[must_use]
    pub const fn pending(self) -> usize {
        self.pending
    }

    /// Les propositions parvenues à une décision — le dénominateur.
    #[must_use]
    pub const fn decided(self) -> usize {
        self.accepted + self.refused
    }

    /// Le taux — `accepted_mutation_rate` proprement dit.
    ///
    /// `None` quand rien n'a été décidé. Voir la documentation du module : `0.0` dirait que tout ce
    /// qui a été décidé a été refusé, ce qui est un fait, et non l'absence de fait.
    #[must_use]
    pub fn rate(self) -> Option<f64> {
        let decided = self.decided();
        if decided == 0 {
            return None;
        }
        // Les deux conversions sont exactes tant que les comptes tiennent sur 53 bits, ce qu'un
        // journal de propositions ne franchit pas.
        #[expect(
            clippy::cast_precision_loss,
            reason = "un compte de propositions ne franchit pas 2^53"
        )]
        Some(self.accepted as f64 / decided as f64)
    }
}

impl fmt::Display for MutationAcceptance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.rate() {
            // Le compte des indécises accompagne toujours le taux : c'est la moitié de ce qu'il faut
            // pour le lire.
            Some(rate) => write!(
                formatter,
                "{}/{} approuvées ({rate:.2}), {} en attente",
                self.accepted,
                self.decided(),
                self.pending
            ),
            None => write!(
                formatter,
                "aucune décision, {} en attente — le taux n'est pas nul, il n'existe pas",
                self.pending
            ),
        }
    }
}
