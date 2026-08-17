//! L'implémentation de référence de [`crate::store::ObjectStore`], en mémoire.

use std::collections::HashMap;

use locus_domain::ContentHash;

use crate::manifest::ArtifactManifest;
use crate::state::ArtifactState;
use crate::store::{ObjectStore, StoreError, UploadId};

/// Un téléversement en cours : ce qui a été écrit, et ce qui avait été annoncé.
#[derive(Debug)]
struct Pending {
    declared_size: u64,
    written: Vec<u8>,
}

/// Un object store en mémoire.
///
/// # Ce qu'il est
///
/// L'implémentation **de référence** des trois garanties du port, et le sujet de la suite de
/// contract tests. Un driver sur système de fichiers ou sur S3 passera la même suite ; c'est elle
/// qui décidera s'il est conforme, pas sa documentation.
///
/// # Ce qu'il n'est pas
///
/// Un stockage durable. Il ne survit pas au processus, et c'est assumé — une implémentation qui
/// ouvrirait un fichier ici ferait entrer une décision d'infrastructure dans le paquet qui définit
/// le contrat.
///
/// # Adressage par contenu
///
/// Les objets sont rangés **par hash**, donc deux artefacts de même contenu partagent leurs
/// octets. Reconclure un téléversement sur un hash déjà présent n'est pas une erreur : c'est de la
/// déduplication, et le contenu adressé est le même par définition.
#[derive(Debug, Default)]
pub struct MemoryObjectStore {
    objects: HashMap<String, Vec<u8>>,
    pending: HashMap<u64, Pending>,
    next: u64,
}

impl MemoryObjectStore {
    /// Un store vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Le nombre d'objets stockés. Lecture, pour les diagnostics et les tests.
    #[must_use]
    pub fn object_count(&self) -> usize {
        self.objects.len()
    }

    /// Le nombre de téléversements ouverts.
    ///
    /// Un téléversement abandonné ou conclu ne compte plus : c'est ce qui rend visible qu'il ne
    /// reste rien derrière un refus.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

impl ObjectStore for MemoryObjectStore {
    fn begin(&mut self, manifest: &ArtifactManifest) -> Result<UploadId, StoreError> {
        // §19.1, et la première garantie du port : le manifeste précède les octets. L'état
        // `Declared` est le seul d'où un contenu peut arriver — `Uploaded` signifierait qu'il est
        // déjà là, et tout ce qui suit qu'on écrase un contenu que quelqu'un a peut-être cité.
        if manifest.state() != ArtifactState::Declared {
            return Err(StoreError::NotDeclared {
                state: manifest.state().slug(),
            });
        }
        self.next += 1;
        self.pending.insert(
            self.next,
            Pending {
                declared_size: manifest.size_bytes(),
                written: Vec::new(),
            },
        );
        Ok(UploadId::from_raw(self.next))
    }

    fn write(&mut self, upload: UploadId, chunk: &[u8]) -> Result<(), StoreError> {
        let pending = self
            .pending
            .get_mut(&upload.raw())
            .ok_or(StoreError::UnknownUpload { upload })?;
        let attempted = pending.written.len() as u64 + chunk.len() as u64;
        if attempted > pending.declared_size {
            // Le fragment n'est pas absorbé : la borne mord au moment du dépassement, pas après.
            // Un store qui accepterait puis tronquerait aurait déjà lu ce qu'il refuse.
            return Err(StoreError::SizeExceeded {
                declared: pending.declared_size,
                attempted,
            });
        }
        pending.written.extend_from_slice(chunk);
        Ok(())
    }

    fn commit(&mut self, upload: UploadId, observed: &ContentHash) -> Result<(), StoreError> {
        let pending = self
            .pending
            .get(&upload.raw())
            .ok_or(StoreError::UnknownUpload { upload })?;
        let written = pending.written.len() as u64;
        if written != pending.declared_size {
            return Err(StoreError::SizeMismatch {
                declared: pending.declared_size,
                written,
            });
        }
        let pending = self
            .pending
            .remove(&upload.raw())
            .expect("présent : lu juste au-dessus");
        self.objects.insert(observed.to_string(), pending.written);
        Ok(())
    }

    fn abort(&mut self, upload: UploadId) {
        // Les octets partent avec l'entrée. Rien n'est rangé sous le hash de ce qui a été écrit —
        // sinon déclarer un faux hash suffirait à faire entrer un contenu arbitraire dans le
        // store, adressable ensuite par qui connaît son hash.
        self.pending.remove(&upload.raw());
    }

    fn read(&self, hash: &ContentHash) -> Option<Vec<u8>> {
        self.objects.get(&hash.to_string()).cloned()
    }
}
