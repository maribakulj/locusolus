//! `projections verify` — `docs/SPEC_V1.md` §9.5.

use locus_event_store::EventStore;

use crate::projection::Projection;
use crate::runner::ProjectionRunner;

/// Ce qu'une vérification a constaté.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyReport {
    /// La projection vérifiée.
    pub projection: &'static str,
    /// Le watermark de l'état courant.
    pub live_watermark: u64,
    /// Le watermark de la reconstruction.
    pub rebuilt_watermark: u64,
    /// Le checksum de l'état courant.
    pub live_checksum: String,
    /// Le checksum de la reconstruction.
    pub rebuilt_checksum: String,
}

impl VerifyReport {
    /// Vrai quand la reconstruction reproduit exactement l'état courant.
    #[must_use]
    pub fn agrees(&self) -> bool {
        self.live_watermark == self.rebuilt_watermark && self.live_checksum == self.rebuilt_checksum
    }

    /// Ce qui diverge, en toutes lettres. Vide quand tout concorde.
    #[must_use]
    pub fn findings(&self) -> Vec<String> {
        let mut findings = Vec::new();
        if self.live_watermark != self.rebuilt_watermark {
            findings.push(format!(
                "watermark : courant {}, reconstruit {}",
                self.live_watermark, self.rebuilt_watermark
            ));
        }
        if self.live_checksum != self.rebuilt_checksum {
            findings.push(format!(
                "checksum : courant `{}`, reconstruit `{}`",
                self.live_checksum, self.rebuilt_checksum
            ));
        }
        findings
    }
}

/// Comparer une projection à sa reconstruction — §9.5, « un outil compare événements et
/// projections ».
///
/// La reconstruction se fait sur une **copie**, jamais sur la projection vérifiée : une
/// vérification qui détruirait ce qu'elle vérifie ne pourrait rien constater, et le premier
/// appel réparerait la divergence en même temps qu'il la découvrirait — ce qui est la définition
/// d'une réparation silencieuse, que §24.5 interdit ailleurs pour la même raison.
///
/// Le paramètre `fresh` fournit la projection vide dans laquelle rejouer. Un `Clone` de la
/// projection vérifiée ne conviendrait pas : `reset` sur une copie testerait que `reset` efface,
/// pas qu'une reconstruction depuis zéro donne le même résultat.
pub fn verify<P, F, S>(live: &ProjectionRunner<P>, fresh: F, store: &S) -> VerifyReport
where
    P: Projection,
    F: FnOnce() -> P,
    S: EventStore,
{
    let mut rebuilt = ProjectionRunner::new(fresh());
    rebuilt.rebuild(store);
    VerifyReport {
        projection: live.projection().name(),
        live_watermark: live.projection().watermark(),
        rebuilt_watermark: rebuilt.projection().watermark(),
        live_checksum: live.projection().checksum(),
        rebuilt_checksum: rebuilt.projection().checksum(),
    }
}
