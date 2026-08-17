//! Conduire une future qui n'attend rien.

use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

/// Faire tourner une future jusqu'à son résultat, sans exécuteur.
///
/// # Panics
///
/// Panique si la future rend `Pending`.
///
/// Ce n'est pas une limitation qu'on subit, c'est **l'assertion centrale** du backend déterministe :
/// rendre `Pending` voudrait dire attendre quelque chose — un réseau, un fichier, un timer — et il
/// n'y a rien à attendre ici. Un moteur de test qui se mettrait à attendre aurait cessé d'être
/// déterministe sans que rien d'autre ne le dise, et un exécuteur complet l'aurait patiemment
/// laissé faire.
///
/// Le port est asynchrone parce que §11.1 l'écrit ainsi et que Temporal l'exigera ; ce qui est
/// derrière, ici, se résout au premier `poll`.
pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!(
            "le backend déterministe a rendu Pending : il attend quelque chose, et il ne devrait \
             rien y avoir à attendre"
        ),
    }
}
