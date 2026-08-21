//! `handed_over_attempts` — ce qu'une reconfiguration coûte réellement. `W21.i`, ADR 0024.
//!
//! # Pourquoi ce n'est pas un volume d'octets
//!
//! La matrice d'acceptation demandait `state_transfer_volume`. Le mot appelle des octets, et il n'y
//! en a pas — non par omission, mais par décision. L'ADR 0019 condition 3 tranche que le passage de
//! témoin porte ce que le nœud sortant **tenait**, jamais ce qu'il **savait** : `docs/13` fixe
//! « nouvel attempt, nouvelle vue, nouveau hash », et un contexte de mission qui voyagerait
//! contournerait cette immuabilité sans la nommer.
//!
//! Une métrique de volume aurait donc deux issues, toutes deux mauvaises. Ou bien elle vaudrait zéro
//! en permanence, puisque rien n'est copié — un cadran qui n'a jamais bougé et qu'on finit par
//! croire cassé plutôt que juste. Ou bien on ajouterait la copie pour avoir quelque chose à mesurer,
//! et la métrique aurait **créé le coût qu'elle prétend observer**.
//!
//! Ce que le passage de témoin coûte réellement dans cette architecture est le nombre de tentatives
//! qu'un successeur doit reprendre. C'est cela qu'on mesure, et c'est pourquoi l'ADR 0024 décision 2
//! a renommé la métrique.
//!
//! # Transmis n'est pas abandonné, et c'est la confusion à éviter
//!
//! Un `kill` porte lui aussi un compte : [`crate::lifecycle::Outcome::Killed`] dit combien de
//! tentatives ont été **abandonnées**, et le porte même quand il vaut zéro, « ce qui distingue un
//! arrêt propre d'un arrêt coûteux ».
//!
//! Une mesure qui additionnerait les deux dirait qu'une reconfiguration a transmis cinq tentatives
//! là où cinq ont été **perdues** — deux issues opposées sous un même nombre, et celle qui coûte le
//! plus cher rendue invisible.
//!
//! La confusion est ici **inexprimable** plutôt qu'évitée par discipline : [`HandedOver::over`] ne
//! reçoit que des [`Handover`], et [`Handover::after_drain`] refuse tout ce qui n'est pas un drain.
//! Un `kill` ne produit donc aucune valeur que cette mesure sache lire.
//!
//! # Un passage de témoin à zéro tentative n'est pas une absence de passage
//!
//! Deux nœuds peuvent se relayer sans que rien ne soit en vol : c'est un fait, et un bon — la
//! reconfiguration n'a rien coûté. Le confondre avec « aucun relais n'a eu lieu » ferait lire une
//! organisation qui se recompose sans frais comme une organisation qui ne se recompose pas.
//!
//! D'où deux comptes, jamais un : les **relais** et les **tentatives**.

use std::fmt;

use crate::messaging::Handover;

/// Ce que des passages de témoin ont transmis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HandedOver {
    transfers: usize,
    attempts: usize,
}

impl HandedOver {
    /// Compter des passages de témoin.
    ///
    /// Ne reçoit que des [`Handover`], et c'est ce qui rend l'abandon inexprimable : voir la
    /// documentation du module.
    #[must_use]
    pub fn over<'a>(handovers: impl IntoIterator<Item = &'a Handover>) -> Self {
        let mut counted = Self::default();
        for handover in handovers {
            counted.transfers += 1;
            counted.attempts += handover.in_flight();
        }
        counted
    }

    /// Combien de relais ont eu lieu.
    ///
    /// Distinct de [`Self::attempts`] : un relais qui ne transmet rien est un relais, et une
    /// organisation qui se recompose sans frais n'est pas une organisation qui ne se recompose pas.
    #[must_use]
    pub const fn transfers(self) -> usize {
        self.transfers
    }

    /// Combien de tentatives ont été transmises — `handed_over_attempts` proprement dit.
    #[must_use]
    pub const fn attempts(self) -> usize {
        self.attempts
    }
}

impl fmt::Display for HandedOver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} tentatives sur {} relais",
            self.attempts, self.transfers
        )
    }
}
