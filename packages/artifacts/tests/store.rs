//! Test de sortie de W6.c — **aucun octet n'entre sans manifeste déclaré, la taille annoncée
//! borne l'écriture, et un contenu non conforme ne laisse rien derrière lui.**
//!
//! # Une suite de contract tests, pas une suite de tests d'implémentation
//!
//! Tout est écrit contre le trait [`ObjectStore`], jamais contre `MemoryObjectStore`. C'est la
//! forme de W1.c (ADR 0012) : le driver sur système de fichiers ou sur S3 passera cette même
//! suite, et c'est elle qui décidera s'il est conforme — pas sa documentation.

use locus_artifacts::{
    ArtifactManifest, ArtifactState, Digest, IngestError, ManifestError, MemoryObjectStore,
    ObjectStore, ProducedBy, StoreError, UploadId, ingest,
};
use locus_domain::{Confidentiality, ContentHash};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

const CONTENT: &[u8] = b"les mesures brutes, seize octets";
const PROMISED: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const OTHER: &str = "sha256:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

fn hash(value: &str) -> ContentHash {
    ContentHash::parse(value).expect("hash bien formé")
}

/// Un hash de test **déterministe et injectif sur ce que les tests emploient**.
///
/// Ce n'est pas un sha256 : ce paquet ne choisit pas d'algorithme, et l'ingestion n'a besoin que
/// d'un port qui distingue deux contenus différents. Le double rend `PROMISED` pour le contenu
/// attendu et `OTHER` pour tout le reste — assez pour que « ce qui est arrivé n'est pas ce qui
/// avait été promis » soit un cas testable, ce qu'un double qui rendrait toujours la même valeur
/// rendrait impossible.
#[derive(Default)]
struct StubDigest {
    seen: Vec<u8>,
}

impl Digest for StubDigest {
    fn update(&mut self, chunk: &[u8]) {
        self.seen.extend_from_slice(chunk);
    }

    fn finish(&mut self) -> ContentHash {
        let text = if self.seen == CONTENT {
            PROMISED
        } else {
            OTHER
        };
        self.seen.clear();
        hash(text)
    }
}

fn declared() -> ArtifactManifest {
    ArtifactManifest::declare(
        "artifact-0001",
        hash(PROMISED),
        "application/octet-stream",
        CONTENT.len() as u64,
        ProducedBy::new("task-0007", 1),
        Confidentiality::Internal,
    )
    .expect("manifeste valide")
}

// ---------------------------------------------------------------------------------------------
// Aucun octet n'entre sans manifeste déclaré
// ---------------------------------------------------------------------------------------------

#[test]
fn le_chemin_nominal_range_le_contenu_sous_son_hash() {
    let mut store = MemoryObjectStore::new();
    let mut digest = StubDigest::default();
    let uploaded = ingest(&mut store, &mut digest, declared(), &[CONTENT]).expect("contenu promis");

    assert_eq!(uploaded.state(), ArtifactState::Uploaded);
    assert_eq!(store.read(&hash(PROMISED)).as_deref(), Some(CONTENT));
    assert_eq!(store.object_count(), 1);
    assert_eq!(
        store.pending_count(),
        0,
        "un téléversement conclu n'est plus en attente"
    );
}

#[test]
fn un_artefact_deja_televerse_ne_se_reteleverse_pas() {
    let mut store = MemoryObjectStore::new();
    let mut digest = StubDigest::default();
    let uploaded = ingest(&mut store, &mut digest, declared(), &[CONTENT]).expect("contenu promis");

    // Sans cette garde, un second envoi écraserait un contenu que quelqu'un a peut-être déjà cité.
    assert_eq!(
        store.begin(&uploaded),
        Err(StoreError::NotDeclared { state: "uploaded" })
    );

    let promoted = declared()
        .uploaded(&hash(PROMISED))
        .expect("contenu conforme")
        .moved_to(ArtifactState::Verified)
        .expect("vérifié")
        .moved_to(ArtifactState::Promoted)
        .expect("promu");
    assert_eq!(
        store.begin(&promoted),
        Err(StoreError::NotDeclared { state: "promoted" })
    );
}

#[test]
fn un_jeton_inconnu_n_ecrit_nulle_part() {
    let mut store = MemoryObjectStore::new();
    let invented = UploadId::from_raw(4_242);
    assert_eq!(
        store.write(invented, b"x"),
        Err(StoreError::UnknownUpload { upload: invented })
    );
    assert_eq!(store.object_count(), 0);

    // Et un jeton déjà conclu en est un aussi : le rejouer réécrirait sous un hash choisi après
    // coup, ce qui est la même faille par un autre chemin.
    let upload = store.begin(&declared()).expect("artefact déclaré");
    store.write(upload, CONTENT).expect("dans la borne");
    store.commit(upload, &hash(PROMISED)).expect("conclu");
    assert_eq!(
        store.commit(upload, &hash(OTHER)),
        Err(StoreError::UnknownUpload { upload })
    );
    assert!(!store.contains(&hash(OTHER)));
}

// ---------------------------------------------------------------------------------------------
// La taille annoncée borne l'écriture
// ---------------------------------------------------------------------------------------------

#[test]
fn le_fragment_qui_depasse_est_refuse_et_non_tronque() {
    let mut store = MemoryObjectStore::new();
    let upload = store.begin(&declared()).expect("artefact déclaré");
    let too_much = [CONTENT, b" et puis un supplement"].concat();

    assert_eq!(
        store.write(upload, &too_much),
        Err(StoreError::SizeExceeded {
            declared: CONTENT.len() as u64,
            attempted: too_much.len() as u64,
        }),
        "la borne mord au moment du dépassement : un store qui accepterait puis tronquerait aurait \
         déjà lu ce qu'il refuse"
    );

    // Rien n'a été absorbé : ce qui suit la borne peut encore entrer si sa taille le permet.
    store.write(upload, CONTENT).expect("le contenu annoncé");
    store.commit(upload, &hash(PROMISED)).expect("conclu");
    assert_eq!(store.read(&hash(PROMISED)).as_deref(), Some(CONTENT));
}

#[test]
fn la_borne_tient_sur_une_suite_de_fragments() {
    let mut store = MemoryObjectStore::new();
    let upload = store.begin(&declared()).expect("artefact déclaré");
    let (head, tail) = CONTENT.split_at(10);
    store.write(upload, head).expect("premier fragment");

    let overflowing = [tail, b"de trop"].concat();
    assert!(matches!(
        store.write(upload, &overflowing),
        Err(StoreError::SizeExceeded { .. })
    ));

    store.write(upload, tail).expect("le reste exact");
    store.commit(upload, &hash(PROMISED)).expect("conclu");
    assert_eq!(store.read(&hash(PROMISED)).as_deref(), Some(CONTENT));
}

#[test]
fn un_contenu_incomplet_ne_se_conclut_pas() {
    let mut store = MemoryObjectStore::new();
    let upload = store.begin(&declared()).expect("artefact déclaré");
    store.write(upload, &CONTENT[..10]).expect("un début");

    assert_eq!(
        store.commit(upload, &hash(PROMISED)),
        Err(StoreError::SizeMismatch {
            declared: CONTENT.len() as u64,
            written: 10,
        }),
        "un contenu tronqué a un autre hash — mais un backend qui le clôt l'a déjà écrit"
    );
    assert_eq!(store.object_count(), 0);
}

#[test]
fn le_depassement_par_ingestion_ne_laisse_rien() {
    let mut store = MemoryObjectStore::new();
    let mut digest = StubDigest::default();
    let refused = ingest(
        &mut store,
        &mut digest,
        declared(),
        &[CONTENT, b" et un supplement"],
    );

    assert!(matches!(
        refused,
        Err(IngestError::Store(StoreError::SizeExceeded { .. }))
    ));
    assert_eq!(store.object_count(), 0);
    assert_eq!(
        store.pending_count(),
        0,
        "un téléversement laissé ouvert est un contenu partiel qui attend"
    );
}

// ---------------------------------------------------------------------------------------------
// Un contenu non conforme ne laisse rien derrière lui
// ---------------------------------------------------------------------------------------------

/// La garantie la moins évidente et la plus importante : le contenu refusé ne doit pas non plus
/// être rangé **sous son propre hash**. Sans cela, déclarer un faux hash suffirait à faire entrer
/// un contenu arbitraire dans le store, adressable ensuite par qui connaît son hash — la
/// déclaration préalable ne filtrerait plus rien, elle enregistrerait juste un refus.
#[test]
fn un_contenu_refuse_n_est_lisible_sous_aucun_hash() {
    let mut store = MemoryObjectStore::new();
    let mut digest = StubDigest::default();
    let smuggled = b"un contenu qui n'est pas";

    let refused = ingest(&mut store, &mut digest, declared(), &[smuggled]);
    assert!(matches!(
        refused,
        Err(IngestError::Manifest(ManifestError::HashMismatch { .. }))
    ));

    assert!(!store.contains(&hash(PROMISED)), "ni sous le hash promis");
    assert!(!store.contains(&hash(OTHER)), "ni sous le sien");
    assert_eq!(store.object_count(), 0);
    assert_eq!(store.pending_count(), 0);
}

#[test]
fn un_televersement_abandonne_ne_laisse_rien() {
    let mut store = MemoryObjectStore::new();
    let upload = store.begin(&declared()).expect("artefact déclaré");
    store.write(upload, CONTENT).expect("dans la borne");
    store.abort(upload);

    assert_eq!(store.object_count(), 0);
    assert_eq!(store.pending_count(), 0);
    assert_eq!(
        store.write(upload, CONTENT),
        Err(StoreError::UnknownUpload { upload }),
        "un jeton abandonné ne se réouvre pas"
    );
}

#[test]
fn abandonner_un_televersement_n_atteint_pas_les_autres() {
    let mut store = MemoryObjectStore::new();
    let mut digest = StubDigest::default();
    let kept = ingest(&mut store, &mut digest, declared(), &[CONTENT]).expect("contenu promis");
    assert_eq!(kept.state(), ArtifactState::Uploaded);

    let second = ArtifactManifest::declare(
        "artifact-0002",
        hash(OTHER),
        "application/octet-stream",
        4,
        ProducedBy::new("task-0007", 2),
        Confidentiality::Internal,
    )
    .expect("manifeste valide");
    let upload = store.begin(&second).expect("artefact déclaré");
    store.write(upload, b"abcd").expect("dans la borne");
    store.abort(upload);

    assert_eq!(
        store.read(&hash(PROMISED)).as_deref(),
        Some(CONTENT),
        "l'abandon d'un téléversement n'efface pas ce qu'un autre a conclu"
    );
    assert_eq!(store.object_count(), 1);
}

// ---------------------------------------------------------------------------------------------
// L'adressage par contenu
// ---------------------------------------------------------------------------------------------

/// Deux artefacts de même contenu partagent leurs octets. Reconclure sur un hash déjà présent
/// n'est pas une erreur : le contenu adressé est le même par définition, et refuser obligerait à
/// distinguer « déjà là » de « conflit », ce qui n'a pas de sens sur un stockage adressé par hash.
#[test]
fn deux_artefacts_de_meme_contenu_partagent_leurs_octets() {
    let mut store = MemoryObjectStore::new();
    let mut digest = StubDigest::default();
    ingest(&mut store, &mut digest, declared(), &[CONTENT]).expect("premier");

    let twin = ArtifactManifest::declare(
        "artifact-0002",
        hash(PROMISED),
        "application/octet-stream",
        CONTENT.len() as u64,
        ProducedBy::new("task-0008", 1),
        Confidentiality::Public,
    )
    .expect("manifeste valide");
    let uploaded = ingest(&mut store, &mut digest, twin, &[CONTENT]).expect("second");

    assert_eq!(uploaded.artifact_id(), "artifact-0002");
    assert_eq!(
        store.object_count(),
        1,
        "un stockage adressé par hash ne range pas deux fois le même contenu"
    );
    assert_eq!(store.read(&hash(PROMISED)).as_deref(), Some(CONTENT));
}

#[test]
fn un_contenu_absent_se_lit_absent() {
    let store = MemoryObjectStore::new();
    assert!(store.read(&hash(PROMISED)).is_none());
    assert!(!store.contains(&hash(PROMISED)));
}
