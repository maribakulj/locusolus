//! Le pilote d'une projection — `docs/SPEC_V1.md` §9.5.

use locus_event_store::EventStore;

use crate::projection::{Projection, ProjectionError};

/// L'état d'une projection vis-à-vis du journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Health {
    /// À jour, ou en retard sans erreur.
    Healthy,
    /// En quarantaine — §9.5. Elle a cessé d'avancer, et elle dit où et pourquoi.
    Quarantined {
        /// La faute qui l'a arrêtée.
        error: ProjectionError,
    },
}

/// Ce qu'un passage du pilote a produit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    /// Le nombre d'événements appliqués pendant ce passage.
    pub applied: usize,
    /// Le watermark après le passage.
    pub watermark: u64,
    /// L'état de santé.
    pub health: Health,
}

/// Le pilote : il pousse le flux dans une projection et tient la promesse de quarantaine.
///
/// # Ce que « sans bloquer l'écriture canonique » veut dire
///
/// §9.5 : « les erreurs de projection sont mises en quarantaine **sans bloquer l'écriture
/// canonique**, sauf si elles concernent une projection synchrone nécessaire à un invariant ».
///
/// Le pilote ne touche jamais au journal. Il le **lit**, et c'est tout : il n'a aucune méthode qui
/// écrive, et le [`EventStore`] lui est passé par référence partagée. Une projection en défaut ne
/// peut donc pas empêcher une écriture, parce qu'il n'existe aucun chemin par lequel elle
/// l'atteindrait. La promesse tient par la forme, pas par la discipline.
///
/// Le cas réservé — projection synchrone nécessaire à un invariant — n'est pas implémenté ici :
/// aucune projection de ce paquet n'est synchrone, et écrire le mécanisme avant d'avoir le cas
/// produirait une abstraction que rien ne teste.
///
/// # Pourquoi la projection n'avance pas au-delà d'une faute
///
/// Une projection qui sauterait l'événement fautif pour continuer aurait un état que la
/// reconstruction ne reproduirait pas — la reconstruction, elle, rencontrerait la même faute au
/// même endroit. « Reconstruction depuis zéro = état courant » deviendrait faux, et c'est
/// précisément la propriété que W1.d livre.
#[derive(Debug)]
pub struct ProjectionRunner<P: Projection> {
    projection: P,
    health: Health,
}

impl<P: Projection> ProjectionRunner<P> {
    /// Piloter cette projection.
    pub const fn new(projection: P) -> Self {
        Self {
            projection,
            health: Health::Healthy,
        }
    }

    /// Consommer ce que le journal a de neuf depuis le watermark.
    ///
    /// Ne rend jamais d'erreur : une faute met en quarantaine et se lit dans [`Progress::health`].
    /// Faire remonter l'erreur inviterait un appelant à la propager jusqu'à un chemin d'écriture,
    /// ce que §9.5 interdit.
    pub fn catch_up<S: EventStore>(&mut self, store: &S) -> Progress {
        if let Health::Quarantined { error } = &self.health {
            return Progress {
                applied: 0,
                watermark: self.projection.watermark(),
                health: Health::Quarantined {
                    error: error.clone(),
                },
            };
        }

        let mut applied = 0;
        for entry in store.feed(self.projection.watermark()) {
            match self.projection.apply(entry.position, &entry.event) {
                Ok(()) => applied += 1,
                Err(error) => {
                    self.health = Health::Quarantined {
                        error: error.clone(),
                    };
                    return Progress {
                        applied,
                        watermark: self.projection.watermark(),
                        health: Health::Quarantined { error },
                    };
                }
            }
        }
        Progress {
            applied,
            watermark: self.projection.watermark(),
            health: Health::Healthy,
        }
    }

    /// Détruire et reconstruire depuis zéro — §9.5.
    ///
    /// Lève aussi la quarantaine : une reconstruction est une seconde chance, et une projection
    /// qui resterait en quarantaine après avoir été reconstruite ne pourrait jamais s'en sortir,
    /// même une fois la cause corrigée dans son code.
    pub fn rebuild<S: EventStore>(&mut self, store: &S) -> Progress {
        self.projection.reset();
        self.health = Health::Healthy;
        self.catch_up(store)
    }

    /// La projection pilotée, en lecture.
    pub const fn projection(&self) -> &P {
        &self.projection
    }

    /// L'état de santé.
    pub const fn health(&self) -> &Health {
        &self.health
    }
}
