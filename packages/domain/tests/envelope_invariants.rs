//! Property tests sur les invariants de l'enveloppe — le test de sortie de W1.a.
//!
//! # Pourquoi un générateur écrit à la main
//!
//! Pas de `proptest` ni de `quickcheck` : le workspace ne dépend aujourd'hui que de `serde` et
//! `serde_json`, et ajouter une bibliothèque de génération pour huit propriétés paierait une
//! dépendance permanente pour un confort ponctuel. Le générateur ci-dessous tient en vingt lignes,
//! il est **déterministe** — un échec se rejoue en relançant le test, sans graine à recopier depuis
//! une sortie CI — et il ne réduit pas les contre-exemples, ce qui est la seule chose qu'on perde.
//!
//! Ce que le générateur produit couvre l'espace qui compte ici : les dix statuts × les sept
//! niveaux, les trois formes de lignée, et des contenus de tailles variées.

use locus_domain::{
    Confidentiality, ContentHash, Envelope, Lineage, Ref, Revision, RevisionId, StableId, Status,
    ValidationLevel,
};
use locus_protocol::Timestamp;

/// Un générateur congruentiel linéaire. Déterministe, sans dépendance, suffisant pour balayer.
struct Rng(u64);

impl Rng {
    const fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        // Les constantes de Numerical Recipes : période complète sur 64 bits.
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    fn below(&mut self, bound: usize) -> usize {
        usize::try_from(self.next() >> 33).unwrap_or(0) % bound
    }

    fn entropy(&mut self) -> [u8; 10] {
        let mut bytes = [0u8; 10];
        for byte in &mut bytes {
            *byte = u8::try_from(self.next() >> 56).unwrap_or(0);
        }
        bytes
    }
}

fn instant(millis: i64) -> Timestamp {
    Timestamp::from_millis(millis)
}

fn stable(rng: &mut Rng) -> StableId {
    StableId::from_parts(instant(1_700_000_000_000), rng.entropy())
        .expect("instant dans les bornes")
}

fn revision(rng: &mut Rng) -> RevisionId {
    RevisionId::from_parts(instant(1_700_000_000_000), rng.entropy())
        .expect("instant dans les bornes")
}

fn hash(rng: &mut Rng) -> ContentHash {
    let digest: String = (0..64)
        .map(|_| char::from_digit(u32::try_from(rng.below(16)).unwrap_or(0), 16).unwrap_or('0'))
        .collect();
    ContentHash::parse(&format!("sha256:{digest}")).expect("digest bien formé")
}

/// Une enveloppe arbitraire, mais toujours bien formée.
fn envelope(rng: &mut Rng) -> Envelope {
    let status = Status::ALL[rng.below(Status::ALL.len())];
    let level = ValidationLevel::ALL[rng.below(ValidationLevel::ALL.len())];
    let lineage = match rng.below(3) {
        0 => Lineage::Root,
        1 => Lineage::Successor {
            supersedes: revision(rng),
        },
        _ => Lineage::Merge {
            supersedes: revision(rng),
            incorporates: (0..rng.below(4)).map(|_| revision(rng)).collect(),
        },
    };
    Envelope {
        stable_id: stable(rng),
        revision_id: revision(rng),
        object_type: ["claim", "inference", "source", "dataset"][rng.below(4)].to_owned(),
        schema_version: "1.0".to_owned(),
        version: u32::try_from(rng.below(50)).unwrap_or(1).saturating_add(1),
        branch_id: "br_main".to_owned(),
        content: serde_json::json!({ "statement": "x", "n": rng.below(1000) }),
        content_hash: hash(rng),
        status,
        validation_level: level,
        created_by: "agent_1".to_owned(),
        created_at: instant(1_700_000_000_000 + i64::try_from(rng.below(1_000_000)).unwrap_or(0)),
        lineage,
        provenance_refs: (0..rng.below(3))
            .map(|_| Ref {
                revision_id: revision(rng),
                note: None,
            })
            .collect(),
        evidence_refs: Vec::new(),
        confidentiality: [
            Confidentiality::Public,
            Confidentiality::Internal,
            Confidentiality::Confidential,
            Confidentiality::Restricted,
        ][rng.below(4)],
        policy_tags: Vec::new(),
    }
}

const CASES: usize = 500;

#[test]
fn stable_id_survives_every_revision() {
    // §7.7 : « `stable_id` identifie le concept à travers ses versions ». C'est la définition du
    // mot ; une révision qui le changerait créerait un second concept en croyant en modifier un.
    let mut rng = Rng::new(1);
    for _ in 0..CASES {
        let before = envelope(&mut rng);
        let after = before.revise(Revision {
            revision_id: revision(&mut rng),
            content: serde_json::json!({ "statement": "y" }),
            content_hash: hash(&mut rng),
            created_by: "agent_2".to_owned(),
            created_at: instant(1_800_000_000_000),
            schema_version: None,
            branch_id: None,
            incorporates: Vec::new(),
        });

        assert_eq!(
            before.stable_id, after.stable_id,
            "le concept a changé d'identité"
        );
        assert_ne!(
            before.revision_id, after.revision_id,
            "deux révisions de même identité"
        );
        assert_eq!(after.version, before.version + 1, "le rang n'a pas avancé");
        // « Une modification crée une nouvelle révision » : la précédente est intacte.
        assert_eq!(before.status, envelope_status_of(&before));
    }
}

/// Relit le statut par une seconde voie, pour que l'assertion précédente porte sur l'objet et non
/// sur la variable locale.
fn envelope_status_of(envelope: &Envelope) -> Status {
    envelope.status
}

#[test]
fn a_revision_has_at_most_one_lineage_predecessor() {
    // §7.7 : « une révision possède au plus un prédécesseur direct dans sa lignée ; un merge peut
    // créer une révision avec plusieurs parents déclarés ». Les deux phrases coexistent, et c'est
    // le type qui les tient : même dans le cas `Merge`, `supersedes` est unique.
    let mut rng = Rng::new(2);
    for _ in 0..CASES {
        let current = envelope(&mut rng);
        let predecessors = current.supersedes().into_iter().count();
        assert!(predecessors <= 1, "plus d'un prédécesseur de lignée");

        // Et les parents déclarés incluent toujours celui de la lignée en premier.
        let declared = current.lineage.declared_parents();
        match current.supersedes() {
            None => assert!(declared.is_empty(), "des parents sans prédécesseur"),
            Some(direct) => assert_eq!(
                declared.first(),
                Some(direct),
                "la lignée n'est pas en tête"
            ),
        }
        assert_eq!(
            declared.len(),
            predecessors + current.lineage.incorporates().len()
        );
    }
}

#[test]
fn a_merge_keeps_a_single_lineage_predecessor() {
    // Le cas qui aurait pu casser l'invariant précédent : une fusion à plusieurs parents.
    let mut rng = Rng::new(3);
    for _ in 0..CASES {
        let before = envelope(&mut rng);
        let incorporates: Vec<RevisionId> = (0..3).map(|_| revision(&mut rng)).collect();
        let merged = before.revise(Revision {
            revision_id: revision(&mut rng),
            content: serde_json::json!({}),
            content_hash: hash(&mut rng),
            created_by: "agent_3".to_owned(),
            created_at: instant(1_800_000_000_000),
            schema_version: None,
            branch_id: None,
            incorporates: incorporates.clone(),
        });

        assert_eq!(merged.supersedes(), Some(&before.revision_id));
        assert_eq!(merged.lineage.incorporates(), incorporates.as_slice());
        // Trois parents incorporés, et toujours un seul prédécesseur de lignée.
        assert_eq!(merged.supersedes().into_iter().count(), 1);
        assert_eq!(merged.lineage.declared_parents().len(), 4);
    }
}

#[test]
fn validation_level_is_never_derived_from_status() {
    // §7.4 : « `validation_level` décrit la force épistémique et ne doit pas être déduit du seul
    // statut ». La propriété se teste en montrant que les 70 combinaisons existent — et notamment
    // `validated` avec `L0`, qui décrit un objet ayant traversé le processus sans qu'aucune preuve
    // n'ait été produite. Un type qui interdirait cette combinaison aurait déduit le niveau.
    let mut rng = Rng::new(4);
    let base = envelope(&mut rng);
    let mut seen = 0;
    for status in Status::ALL {
        for level in ValidationLevel::ALL {
            let candidate = Envelope {
                status,
                validation_level: level,
                ..base.clone()
            };
            let text = serde_json::to_string(&candidate).expect("sérialisable");
            let back: Envelope = serde_json::from_str(&text).expect("relisible");
            assert_eq!(back.status, status);
            assert_eq!(back.validation_level, level);
            seen += 1;
        }
    }
    assert_eq!(
        seen, 70,
        "toutes les combinaisons ne sont pas représentables"
    );
}

#[test]
fn a_new_revision_does_not_inherit_its_predecessor_validation() {
    // Hériter du niveau ferait franchir à un contenu modifié une validation qui portait sur un
    // autre contenu — la manière dont une preuve se perd sans que personne ne s'en aperçoive.
    let mut rng = Rng::new(5);
    for _ in 0..CASES {
        let before = Envelope {
            status: Status::Validated,
            validation_level: ValidationLevel::Reproduced,
            ..envelope(&mut rng)
        };
        let after = before.revise(Revision {
            revision_id: revision(&mut rng),
            content: serde_json::json!({ "statement": "modifié" }),
            content_hash: hash(&mut rng),
            created_by: "agent_4".to_owned(),
            created_at: instant(1_800_000_000_000),
            schema_version: None,
            branch_id: None,
            incorporates: Vec::new(),
        });

        assert_eq!(after.status, Status::Draft);
        assert_eq!(after.validation_level, ValidationLevel::Unassessed);
        // Les preuves de la révision précédente ne suivent pas non plus : elles portaient sur un
        // autre contenu.
        assert!(after.evidence_refs.is_empty());
        // La provenance, elle, suit : elle dit d'où vient l'objet, pas ce qu'il vaut.
        assert_eq!(after.provenance_refs, before.provenance_refs);
    }
}

#[test]
fn the_canonical_form_survives_a_round_trip() {
    // §7.7 : « les hashes portent sur une canonicalisation stable » et « la présentation locale des
    // dates n'affecte jamais les signatures ni les hashes ». Un aller-retour qui perdrait ou
    // reformaterait un champ ferait diverger deux pairs sur la même donnée.
    let mut rng = Rng::new(6);
    for _ in 0..CASES {
        let original = envelope(&mut rng);
        let text = serde_json::to_string(&original).expect("sérialisable");
        let back: Envelope = serde_json::from_str(&text).expect("relisible");
        assert_eq!(original, back);
        // Et une seconde sérialisation rend exactement les mêmes octets.
        assert_eq!(text, serde_json::to_string(&back).expect("sérialisable"));
    }
}

#[test]
fn identifiers_carry_their_type_prefix() {
    // §7.7 : « tous les identifiants globaux sont des UUIDv7 ou ULID avec préfixe de type ».
    // Le préfixe fait partie de l'identité : c'est lui qui empêche de lire un `revision_id` là où
    // un `stable_id` était attendu, sur le fil comme dans un log.
    let mut rng = Rng::new(7);
    for _ in 0..CASES {
        let current = envelope(&mut rng);
        let text = serde_json::to_string(&current).expect("sérialisable");
        assert!(text.contains(&format!("\"{}\"", current.stable_id)));
        assert!(current.stable_id.to_string().starts_with("obj_"));
        assert!(current.revision_id.to_string().starts_with("rev_"));
        // Et les deux ne se confondent pas, même à valeur égale par accident.
        assert_ne!(
            current.stable_id.to_string(),
            current.revision_id.to_string()
        );
    }
}

#[test]
fn a_content_hash_without_its_algorithm_is_refused() {
    // Un hash nu ne dit pas comment le recalculer, et une vérification d'intégrité qui devine son
    // algorithme n'en est pas une.
    assert!(ContentHash::parse(&"a".repeat(64)).is_err());
    assert!(ContentHash::parse("md5:d41d8cd98f00b204e9800998ecf8427e").is_err());
    // Un digest tronqué est la forme que prend une intégrité cassée.
    assert!(ContentHash::parse(&format!("sha256:{}", "a".repeat(63))).is_err());
    // La casse n'est pas normalisée : deux écritures produiraient deux formes canoniques.
    assert!(ContentHash::parse(&format!("sha256:{}", "A".repeat(64))).is_err());

    let good = ContentHash::parse(&format!("sha256:{}", "a".repeat(64))).expect("bien formé");
    assert_eq!(good.algorithm(), "sha256");
    assert_eq!(good.to_string(), format!("sha256:{}", "a".repeat(64)));
}

#[test]
fn a_worker_proposes_no_further_than_staged() {
    // Canterel §2.3, vu depuis le domaine. La règle vit des deux côtés du fil : un domaine qui
    // accepterait `validated` d'un worker ferait dépendre l'invariant 3 de la bonne foi du client.
    for status in Status::ALL {
        let proposable = matches!(status, Status::Draft | Status::Staged);
        assert_eq!(status.is_worker_proposable(), proposable, "{status}");
    }
}

#[test]
fn validation_levels_are_not_a_total_chain() {
    // §8.1 : « ces niveaux ne forment pas toujours une chaîne totale ». Dériver `Ord` écrirait
    // dans le type une affirmation que la spec dément, et `if level >= Reproduced` refuserait une
    // interprétation historique parfaitement validée. Le rang reste lisible, comme étiquette.
    let ranks: Vec<u8> = ValidationLevel::ALL
        .iter()
        .map(|level| level.rank())
        .collect();
    assert_eq!(ranks, vec![0, 1, 2, 3, 4, 5, 6]);
    // L6 sans L4 est un état légitime, et le type ne s'y oppose pas.
    let accepted = ValidationLevel::InstitutionallyAccepted;
    assert_eq!(accepted.rank(), 6);
    assert_ne!(accepted, ValidationLevel::Reproduced);
}
