//! Le travail bloquant sort du fil du runtime asynchrone — `W20.p`, ADR 0030 décision 1.
//!
//! # Ce que `W20.m` a laissé, et pourquoi c'était un item et non un coin
//!
//! Depuis `W20.m`, un profil durable assemble `locusd` sur `PostgresEventStore`. Le port
//! [`locus_event_store::EventStore`] est **synchrone** — l'ADR 0030 l'a voulu ainsi, et `postgres`
//! est le wrapper synchrone de `tokio-postgres` pour cette raison. Les handlers de `W20.k`
//! l'appelaient donc directement depuis un fil `tokio`.
//!
//! Ce n'est pas une faute de correction : le daemon répond juste. C'est une propriété de **latence
//! sous charge**, et elle est brutale — un fil de travail occupé à attendre la base ne sert
//! personne, pas même une requête qui n'aurait pas touché la base. Avec un seul fil de travail, une
//! lecture lente **affame** tout le reste, y compris la sonde de santé qu'un exploitant interroge
//! pour comprendre ce qui se passe.
//!
//! Corriger cela change la **convention d'appel** de toute la couche HTTP, ce qui appartient à un
//! item plutôt qu'au coin d'un autre. C'est celui-ci.
//!
//! # La borne, et pourquoi elle refuse plutôt qu'elle n'attend
//!
//! Le pool bloquant de `tokio` a sa propre borne, et elle est **haute** — plusieurs centaines de
//! fils par défaut. S'en remettre à elle reviendrait à laisser la saturation se manifester par une
//! latence que personne ne sait attribuer.
//!
//! [`Offload`] compte donc ce qui est en vol et **refuse** au-delà de [`MAX_BLOCKING`], avec le
//! `unavailable` de §22.5 qui **nomme la borne**. C'est exactement la forme que
//! [`crate::writes::StreamLocks`] donne déjà à la saturation des écritures, et pour la raison qu'elle
//! écrit : « une attente sans limite est une panne qui ne se déclare pas ». Un client qui reçoit
//! `unavailable` sait qu'il peut retenter ; un client qui attend ne sait rien.
//!
//! # Ce que ce module ne fait pas
//!
//! Il ne rend pas le port asynchrone. Rendre `EventStore` `async` obligerait chaque implémentation à
//! l'être, ferait entrer un runtime dans `packages/event-store`, et transformerait la suite de
//! contract tests de `W1` — qui tourne sans runtime — en suite asynchrone. L'ADR 0030 décision 1 a
//! tranché : le port reste synchrone, et c'est **l'appelant** qui décide où le travail s'exécute.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use locus_event_store::EventStore;

use crate::composition::Runtime;
use crate::error::CommandError;

/// Combien d'appels bloquants peuvent être en vol en même temps.
///
/// # Pourquoi cette valeur, et pourquoi elle voyage dans le refus
///
/// Elle borne le nombre de connexions à la base qu'un daemon peut demander d'un coup. Au-delà, la
/// base ferait la queue elle-même — et la file serait invisible, du mauvais côté du réseau. Refuser
/// ici met la saturation là où on la lit.
///
/// La valeur n'est pas un réglage caché : elle est **dans le message** du refus, et un test
/// l'exerce. C'est la règle que [`crate::writes::MAX_PENDING`] suit déjà.
pub const MAX_BLOCKING: usize = 64;

/// Ce qui arrive à une demande de travail bloquant — la forme de [`crate::writes::Admitted`].
#[derive(Debug, PartialEq, Eq)]
pub enum Offloaded<T> {
    /// Le travail a eu lieu, hors du fil du runtime.
    Done(T),
    /// La borne était franchie : **rien n'a été tenté**.
    ///
    /// Distinct d'un échec du travail lui-même, et la distinction est celle de §22.5 : `unavailable`
    /// dit « retente », `internal` dit « ouvre un ticket ». Les fondre ferait ouvrir des tickets
    /// pour de la charge.
    Saturated {
        /// La borne franchie, pour que le refus la nomme.
        limit: usize,
    },
}

impl<T> Offloaded<T> {
    /// Le résultat, ou le refus de §22.5 qui nomme la borne.
    ///
    /// # Errors
    ///
    /// [`CommandError::Unavailable`] quand la borne était franchie.
    pub fn or_refuse(self) -> Result<T, CommandError> {
        match self {
            Self::Done(value) => Ok(value),
            Self::Saturated { limit } => Err(CommandError::Unavailable {
                detail: format!(
                    "trop d'appels bloquants en vol : la borne est {limit}. Le journal est \
                     synchrone (ADR 0030 décision 1) et ce daemon refuse d'attendre sans le dire — \
                     retentez"
                ),
            }),
        }
    }
}

/// Le compteur de ce qui est en vol, et sa borne.
///
/// Séparé de [`Offload`] pour être exerçable sans runtime asynchrone : la borne est une propriété de
/// comptage, et un test qui devrait monter un serveur pour l'éprouver ne l'éprouverait qu'à travers
/// tout le reste.
#[derive(Debug)]
pub struct Budget {
    in_flight: AtomicUsize,
    limit: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Self::with_limit(MAX_BLOCKING)
    }
}

impl Budget {
    /// Un budget vide, borné par [`MAX_BLOCKING`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Un budget vide, borné à la valeur donnée.
    #[must_use]
    pub const fn with_limit(limit: usize) -> Self {
        Self {
            in_flight: AtomicUsize::new(0),
            limit,
        }
    }

    /// La borne, telle qu'un refus la nomme.
    #[must_use]
    pub const fn limit(&self) -> usize {
        self.limit
    }

    /// Combien d'appels sont en vol à cet instant.
    #[must_use]
    pub fn in_flight(&self) -> usize {
        self.in_flight.load(Ordering::SeqCst)
    }

    /// Prendre une place, ou `None` si la borne est franchie.
    ///
    /// La place se rend **à la destruction** de la garde, panique comprise : un chemin d'erreur qui
    /// oublierait de décompter ferait baisser la capacité du daemon à chaque échec, jusqu'à ce qu'il
    /// refuse tout sans que rien n'ait changé. C'est la même raison qui fait de
    /// [`crate::writes::StreamLocks`] un RAII.
    ///
    /// La garde possède un `Arc` du budget plutôt que de l'emprunter, et ce n'est pas un détail :
    /// elle doit **traverser** le passage au pool bloquant, donc vivre dans une fermeture `'static`.
    /// Une première rédaction empruntait, ce qui obligeait à relâcher la place avant de céder au
    /// pool puis à en reprendre une dedans — c'est-à-dire à ne rien borner entre les deux, et à
    /// laisser la seconde prise échouer sans que le travail s'arrête.
    #[must_use]
    pub fn admit(self: &Arc<Self>) -> Option<Permit> {
        // `fetch_add` puis rendu si l'on dépasse, plutôt que « lire puis incrémenter » : entre la
        // lecture et l'incrément, deux fils passeraient tous les deux sous la borne et la
        // franchiraient ensemble. C'est le « check-then-act » que l'ADR 0029 a rendu inexprimable
        // pour les écritures, et il n'a pas plus sa place ici.
        if self.in_flight.fetch_add(1, Ordering::SeqCst) >= self.limit {
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
            return None;
        }
        Some(Permit {
            budget: Arc::clone(self),
        })
    }
}

/// Une place tenue dans le budget, rendue à la destruction.
#[derive(Debug)]
pub struct Permit {
    budget: Arc<Budget>,
}

impl Drop for Permit {
    fn drop(&mut self) {
        self.budget.in_flight.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Ce que la couche HTTP tient : le daemon, et le droit de l'appeler ailleurs que sur son fil.
///
/// # Pourquoi l'état du routeur n'est plus `Arc<Runtime<S>>`
///
/// Parce qu'un handler qui tient un `Runtime` peut l'appeler, et le fera. La convention ne se tient
/// pas par discipline : elle se tient parce que le type qu'un handler reçoit **n'expose pas** le
/// daemon. `Offload::run` est la seule porte, et une garde de source vérifie qu'aucun handler ne
/// contourne — la même forme que la règle 4 de `boundaries.json` pour les sockets de runtime.
pub struct Offload<S> {
    runtime: Arc<Runtime<S>>,
    budget: Arc<Budget>,
}

// `derive(Clone)` exigerait `S: Clone`, que `Runtime<S>` n'a pas et n'a pas à avoir : les deux
// champs sont des `Arc`, donc le clone est celui des compteurs de références.
impl<S> Clone for Offload<S> {
    fn clone(&self) -> Self {
        Self {
            runtime: Arc::clone(&self.runtime),
            budget: Arc::clone(&self.budget),
        }
    }
}

impl<S> std::fmt::Debug for Offload<S> {
    /// Le budget, et **pas** le daemon.
    ///
    /// `finish_non_exhaustive` plutôt que `finish` : le champ tu est délibéré. Un `Runtime` déroulé
    /// dans une trace y déverserait l'état de quatre projections, et `Desk` prend déjà ce parti pour
    /// la même raison.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Offload")
            .field("in_flight", &self.budget.in_flight())
            .field("limit", &self.budget.limit())
            .finish_non_exhaustive()
    }
}

impl<S: EventStore + Send + Sync + 'static> Offload<S> {
    /// Ce que la couche HTTP reçoit, borné par [`MAX_BLOCKING`].
    #[must_use]
    pub fn new(runtime: Arc<Runtime<S>>) -> Self {
        Self {
            runtime,
            budget: Arc::new(Budget::new()),
        }
    }

    /// Le même, sous la borne donnée — pour qu'un test puisse l'atteindre sans en poser soixante-quatre.
    #[must_use]
    pub fn bounded(runtime: Arc<Runtime<S>>, limit: usize) -> Self {
        Self {
            runtime,
            budget: Arc::new(Budget::with_limit(limit)),
        }
    }

    /// Le budget, en lecture.
    #[must_use]
    pub fn budget(&self) -> &Budget {
        &self.budget
    }

    /// Exécuter ce travail **hors du fil du runtime**, ou refuser en nommant la borne.
    ///
    /// # Ce que la fermeture reçoit, et ce qu'elle ne doit pas faire
    ///
    /// Elle reçoit le daemon et rend une valeur. Elle s'exécute sur un fil du pool bloquant, donc
    /// elle a le droit d'attendre la base — c'est tout l'objet. Elle n'a en revanche aucun moyen
    /// d'attendre une tâche asynchrone, et c'est voulu : mélanger les deux ferait revenir le
    /// problème par l'autre bout.
    ///
    /// # Errors
    ///
    /// [`CommandError::Unavailable`] quand la borne est franchie, ou quand le pool bloquant meurt —
    /// ce qui n'arrive qu'à l'arrêt du runtime, et se lit alors comme une indisponibilité plutôt que
    /// comme un défaut interne.
    pub async fn run<T, F>(&self, work: F) -> Result<T, CommandError>
    where
        F: FnOnce(&Runtime<S>) -> T + Send + 'static,
        T: Send + 'static,
    {
        // La place est prise **avant** de céder au pool, et elle voyage avec la fermeture : la
        // prendre dedans laisserait passer autant de tâches que le pool en accepte, c'est-à-dire ne
        // borner rien.
        let Some(permit) = self.budget.admit() else {
            return Offloaded::<T>::Saturated {
                limit: self.budget.limit(),
            }
            .or_refuse();
        };
        let runtime = Arc::clone(&self.runtime);
        let outcome = tokio::task::spawn_blocking(move || {
            // La garde meurt avec cette fermeture, donc la place est rendue quand le travail
            // s'achève — y compris s'il panique.
            let _permit = permit;
            work(&runtime)
        })
        .await;
        match outcome {
            Ok(value) => Ok(value),
            Err(error) => Err(CommandError::Unavailable {
                detail: format!(
                    "le pool de fils bloquants n'a pas rendu de résultat : {error}. Le daemon \
                     s'arrête, ou le travail a paniqué — dans les deux cas, retenter est la bonne \
                     réponse"
                ),
            }),
        }
    }
}
