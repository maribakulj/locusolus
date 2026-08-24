//! L'implémentation de production de [`crate::store::Digest`] — `W20.t`.
//!
//! # Pourquoi elle est ici et pas ailleurs
//!
//! Le trait appartient à ce paquet, le calcul appartient au domaine. Les règles d'orphelin
//! laissent exactement une place à l'implémentation : ici, chez le propriétaire du trait. Écrire le
//! calcul ici l'aurait dupliqué — `locus_domain::ContentHash::of` en fait déjà un, et deux
//! implémentations de SHA-256 dans un même workspace finissent par diverger sur un détail que
//! personne ne remarque avant qu'un digest cesse de correspondre.
//!
//! # Ce que ce module ne fait pas
//!
//! Il ne choisit pas l'algorithme : [`locus_domain::Hasher`] le fait, sous l'ADR 0020, et ce
//! module ne fait que présenter ce choix sous la forme que le port attend. `store` documente
//! pourquoi le port existe — « ce paquet ne choisit pas d'algorithme » — et cette phrase reste
//! vraie : elle dit que le paquet n'en impose pas un à ses appelants, pas qu'aucun n'existe.

use locus_domain::{ContentHash, Hasher};

use crate::store::Digest;

/// Le calcul SHA-256 incrémental, sous la forme du port.
///
/// # Pourquoi une enveloppe et non `impl Digest for Hasher`
///
/// [`Hasher::finish`] **consomme** le calcul, délibérément : un `Hasher` réutilisé hasherait la
/// concaténation de deux contenus en croyant hasher le second. [`Digest::finish`] prend un
/// `&mut self`, parce que [`crate::ingest`] tient le calcul pendant toute l'écriture et ne peut
/// pas le rendre. L'enveloppe est ce qui réconcilie les deux sans affaiblir ni l'un ni l'autre :
/// elle **retire** le calcul plutôt que de le cloner, et un second `finish` sur la même enveloppe
/// rend le condensat du vide plutôt que celui du contenu.
///
/// Ce dernier point mérite d'être dit plutôt que laissé à la surprise : c'est exactement la faute
/// que la consommation de `Hasher` rend impossible en amont, et l'enveloppe la rend simplement
/// visible — un condensat de contenu vide ne ressemble à aucun contenu réel, alors qu'un condensat
/// de concaténation ressemble à un condensat correct.
#[derive(Debug, Default)]
pub struct Sha256Digest {
    hasher: Option<Hasher>,
}

impl Sha256Digest {
    /// Un calcul vierge.
    #[must_use]
    pub fn new() -> Self {
        Self {
            hasher: Some(Hasher::new()),
        }
    }
}

impl Digest for Sha256Digest {
    fn update(&mut self, chunk: &[u8]) {
        if let Some(hasher) = self.hasher.as_mut() {
            hasher.update(chunk);
        }
    }

    fn finish(&mut self) -> ContentHash {
        self.hasher.take().unwrap_or_default().finish()
    }
}
