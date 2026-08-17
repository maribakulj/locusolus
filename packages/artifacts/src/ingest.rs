//! L'ingestion : ce qui tient ensemble le manifeste, le hash et le store.
//!
//! # Pourquoi cette fonction existe
//!
//! Les trois pièces savent chacune une chose et aucune ne sait l'essentiel. Le manifeste sait
//! quel hash avait été promis ([`ArtifactManifest::uploaded`] le confronte) ; le [`Digest`] sait
//! calculer celui du contenu qui arrive ; le store sait ranger des octets. **L'ordre** dans lequel
//! on les appelle est la garantie, et il n'appartient à aucune des trois.
//!
//! Cet ordre est : ouvrir, écrire en hashant, confronter, **puis seulement** conclure. Confronter
//! après avoir conclu rangerait d'abord et vérifierait ensuite, ce qui revient à faire entrer le
//! contenu puis à espérer pouvoir l'oublier.

use std::fmt;

use crate::manifest::{ArtifactManifest, ManifestError};
use crate::store::{Digest, ObjectStore, StoreError};

/// Téléverser un contenu pour un artefact déclaré.
///
/// Rend le manifeste avancé en `uploaded`. Sur refus, **rien n'est lisible** : ni sous le hash
/// déclaré, ni sous celui du contenu écrit.
///
/// # Errors
///
/// [`IngestError::Store`] pour ce que le store refuse — au premier chef la taille annoncée
/// dépassée — et [`IngestError::Manifest`] quand le contenu reçu n'est pas celui qui avait été
/// promis.
pub fn ingest<S, D>(
    store: &mut S,
    digest: &mut D,
    manifest: ArtifactManifest,
    chunks: &[&[u8]],
) -> Result<ArtifactManifest, IngestError>
where
    S: ObjectStore,
    D: Digest,
{
    let upload = store.begin(&manifest)?;
    for chunk in chunks {
        if let Err(error) = store.write(upload, chunk) {
            // Le refus laisse le téléversement ouvert si personne ne l'abandonne, et un
            // téléversement ouvert est un contenu partiel qui attend. C'est ici qu'il disparaît.
            store.abort(upload);
            return Err(IngestError::Store(error));
        }
        digest.update(chunk);
    }
    let observed = digest.finish();

    let uploaded = match manifest.uploaded(&observed) {
        Ok(uploaded) => uploaded,
        Err(error) => {
            store.abort(upload);
            return Err(IngestError::Manifest(error));
        }
    };
    if let Err(error) = store.commit(upload, &observed) {
        store.abort(upload);
        return Err(IngestError::Store(error));
    }
    Ok(uploaded)
}

/// Ce qui fait échouer une ingestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestError {
    /// Le store a refusé.
    Store(StoreError),
    /// Le manifeste a refusé — le contenu n'est pas celui qui avait été promis.
    Manifest(ManifestError),
}

impl From<StoreError> for IngestError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl fmt::Display for IngestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => write!(formatter, "{error}"),
            Self::Manifest(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for IngestError {}
