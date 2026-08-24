//! La sérialisation des écritures — `W20.h`, ADR 0029 décisions 2, 3 et 6.
//!
//! # Ce qui s'exclut, et ce qui ne s'exclut pas
//!
//! `main.rs` nommait le blocage : « `Transaction::submit` prend `&mut self`, et la couche HTTP ne
//! tient qu'un `&Runtime`. » Rust refusait de compiler tant que personne n'avait dit comment
//! plusieurs requêtes concurrentes obtiennent l'accès exclusif qu'une écriture demande.
//!
//! Mesuré sur le code plutôt que supposé, `submit` fait quatre choses : lire l'enveloppe, consulter
//! le registre d'idempotence, **décider**, écrire. `Decide::decide` prend `&self` et `&State` : elle
//! est **pure**. Le travail de domaine — le plus coûteux des quatre — n'a donc jamais eu besoin
//! d'être sérialisé, et c'est ce constat qui rend cette forme possible.
//!
//! Ce qui s'exclut est le couple `(consultation du registre, écriture)` **pour un stream donné**, et
//! rien d'autre. Une commande lente sur une mission ne retarde pas une commande sur une autre
//! mission, et un test le **mesure** au lieu de le décrire.
//!
//! # Pourquoi une fermeture, et pas un permis rendu à l'appelant
//!
//! Un permis qui traverserait la frontière de la fonction demanderait de faire vivre un
//! `MutexGuard` dans une structure, c'est-à-dire un type auto-référent ou un `unsafe` —
//! `unsafe_code = "forbid"` ferme d'ailleurs la seconde porte. Avec une fermeture, le garde vit sur
//! la pile de [`StreamLocks::with`] et se relâche à la sortie, y compris si le travail panique.
//!
//! # Pourquoi le verrou est pris après la décision
//!
//! Le stream n'est connu qu'une fois les événements décidés : c'est le premier `Draft` qui le nomme.
//! Verrouiller plus tôt aurait demandé que l'enveloppe déclare son agrégat, ce qu'elle ne fait pas —
//! et le lui faire déclarer aurait créé un champ que rien d'autre n'utilise, dont la fausseté serait
//! **indétectable** : ni erreur, ni conflit, seulement deux commandes qui cessent de s'exclure.
//!
//! # La sérialisation n'est pas la correction
//!
//! Elle ordonne l'accès ; c'est `Expected` qui garde la correction. Deux commandes sur le même
//! stream font la queue, **puis** la seconde découvre que sa révision attendue est périmée et reçoit
//! un `Conflict` portant l'état courant. Retirer le contrôle de révision au motif que l'accès est
//! sérialisé serait faux dès qu'il y a plus d'un processus — un second daemon, une migration, un
//! outil d'exploitation.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};

/// Combien d'écritures peuvent être admises en même temps, tous streams confondus.
///
/// # Une attente sans limite est une panne qui ne se déclare pas
///
/// Sous charge, tout le monde attendrait et personne ne saurait pourquoi. La borne existe pour que
/// la saturation soit un **fait déclaré** — un refus typé `unavailable`, que le client sait lire
/// comme « retente plus tard » — et non une lenteur qu'un exploitant cherche ailleurs.
///
/// La valeur n'est pas un réglage caché : elle voyage dans le refus, et un test l'exerce.
pub const MAX_PENDING: usize = 1024;

/// Combien d'entrées la table tolère avant de balayer.
///
/// Le balayage ne retire que ce que **plus personne ne tient**, donc il ne peut jamais séparer deux
/// écritures qui devaient s'exclure. Le seuil n'est qu'un compromis entre le coût du balayage et la
/// taille de la table ; il ne porte aucune garantie.
pub const PRUNE_AT: usize = 256;

/// Les verrous d'écriture, un par stream.
///
/// # La table ne tient que des références **faibles**, et c'est une garantie et non une économie
///
/// La première version gardait des `Arc` et retirait une entrée quand son compte de références
/// tombait sous un seuil. Elle était correcte — cloner depuis la table et retirer passent par le
/// même verrou de table —, mais sa correction reposait sur un **nombre**, et une passe de mutants
/// l'a montré : porter le seuil de deux à quatre-vingt-dix-neuf laissait retirer une entrée qu'un
/// fil attendait, sans qu'aucun test bronche. Un troisième fil aurait alors créé un second verrou
/// pour le même stream, et deux écritures se seraient recouvertes.
///
/// Avec des `Weak`, ce nombre disparaît. Un fil qui attend tient un `Arc`, donc la référence faible
/// s'élève et il obtient **le même** verrou ; quand plus personne ne tient, elle ne s'élève plus et
/// un fil neuf crée un verrou neuf — mais à cet instant, personne ne tenait l'ancien, donc rien ne
/// se recouvre. **La faute cesse d'être exprimable au lieu d'être cherchée**, comme pour le
/// `check(&mut self)` du journal.
#[derive(Debug)]
pub struct StreamLocks {
    locks: Mutex<HashMap<String, Weak<Mutex<()>>>>,
    admitted: AtomicUsize,
    limit: usize,
}

impl Default for StreamLocks {
    fn default() -> Self {
        Self::with_limit(MAX_PENDING)
    }
}

/// Ce qui arrive à une demande d'écriture.
#[derive(Debug, PartialEq, Eq)]
pub enum Admitted<T> {
    /// Le travail a eu lieu, sous le verrou du stream.
    Done(T),
    /// La borne était franchie : rien n'a été tenté.
    ///
    /// Distinct d'un échec du travail lui-même. L'appelant en fait un refus `unavailable` de §22.5,
    /// et le client sait qu'il peut retenter — là où `internal` l'enverrait ouvrir un ticket.
    Saturated {
        /// La borne franchie, pour que le refus la nomme.
        limit: usize,
    },
}

impl StreamLocks {
    /// Une table vide, bornée par [`MAX_PENDING`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Une table vide, bornée autrement.
    ///
    /// La borne est une valeur du service et non une constante cachée : un profil de déploiement
    /// peut la choisir, et un test peut l'exercer sans fabriquer mille écritures. Ce que l'ADR 0029
    /// décision 6 exige est qu'elle **existe** et qu'elle se dise dans le refus, pas qu'elle soit la
    /// même partout.
    #[must_use]
    pub fn with_limit(limit: usize) -> Self {
        Self {
            locks: Mutex::new(HashMap::new()),
            admitted: AtomicUsize::new(0),
            limit,
        }
    }

    /// La borne de cette table.
    #[must_use]
    pub const fn limit(&self) -> usize {
        self.limit
    }

    /// Combien d'écritures sont admises en ce moment — en attente ou en cours.
    #[must_use]
    pub fn admitted(&self) -> usize {
        self.admitted.load(Ordering::Acquire)
    }

    /// Combien de streams sont **réellement** verrouillés ou attendus en ce moment.
    ///
    /// Les entrées mortes ne comptent pas : elles ne verrouillent rien, et les compter ferait lire
    /// une fuite là où il n'y a qu'un balayage en retard.
    #[must_use]
    pub fn tracked(&self) -> usize {
        self.locks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .filter(|weak| weak.strong_count() > 0)
            .count()
    }

    /// La taille brute de la table, entrées mortes comprises — pour vérifier qu'elle ne fuit pas.
    #[must_use]
    pub fn slots(&self) -> usize {
        self.locks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// Exécuter ce travail sous le verrou de ce stream.
    ///
    /// # Le compteur est incrémenté **avant** l'attente, pas après
    ///
    /// Un compteur incrémenté une fois le verrou obtenu ne compterait que les écritures en cours,
    /// jamais celles qui patientent — c'est-à-dire exactement ce que la borne doit voir. Il compte
    /// donc les demandes **admises**, et la borne se lit avant de faire attendre qui que ce soit.
    pub fn with<T>(&self, stream_id: &str, work: impl FnOnce() -> T) -> Admitted<T> {
        if !self.admit() {
            return Admitted::Saturated { limit: self.limit };
        }

        let lock = {
            let mut locks = self.locks.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(existing) = locks.get(stream_id).and_then(Weak::upgrade) {
                existing
            } else {
                let fresh = Arc::new(Mutex::new(()));
                locks.insert(stream_id.to_owned(), Arc::downgrade(&fresh));
                if locks.len() > PRUNE_AT {
                    // Ne retire que ce que plus personne ne tient : sûr par construction.
                    locks.retain(|_, weak| weak.strong_count() > 0);
                }
                fresh
            }
        };

        // Le verrou de stream est pris **hors** de la table : le tenir pendant l'attente
        // sérialiserait tous les streams à travers la table, c'est-à-dire reconstruirait le goulot
        // global que cette structure existe pour éviter.
        let outcome = {
            let _held = lock.lock().unwrap_or_else(PoisonError::into_inner);
            work()
        };

        self.release(lock);
        Admitted::Done(outcome)
    }

    /// Réserver une place sous la borne, ou refuser.
    ///
    /// `fetch_update` et non « lire puis écrire » : entre les deux, une autre demande passerait, et
    /// la borne se franchirait d'exactement autant de demandes qu'il y a de fils.
    fn admit(&self) -> bool {
        self.admitted
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                (current < self.limit).then_some(current + 1)
            })
            .is_ok()
    }

    /// Relâcher la place.
    ///
    /// Il n'y a rien à retirer de la table : l'entrée est une référence faible, et elle cesse d'elle
    /// même de désigner un verrou dès que le dernier `Arc` disparaît. Le balayage a lieu ailleurs,
    /// et il ne peut retirer que du mort.
    fn release(&self, lock: Arc<Mutex<()>>) {
        drop(lock);
        self.admitted.fetch_sub(1, Ordering::AcqRel);
    }
}
