//! Test de sortie de `W18.h` — **le raisonneur d'ontologie, première capacité réellement admise.**
//!
//! Cinq propriétés, celles du tableau de `docs/10` :
//!
//! 1. il n'entre que par un `Published` de `W5.b`, et aucun constructeur ne le fabrique autrement ;
//! 2. sa sortie entre comme claim **proposé** avec sa provenance, jamais comme fait — tenu par
//!    l'absence de chemin vers un `Inference` validé ;
//! 3. le verdict a **trois** valeurs, et `Undetermined` refuse la confiance ;
//! 4. un moteur de règles ne peut alimenter aucun chemin de décision — tenu par l'absence ;
//! 5. la résolution se fait **par identité**, et masquer une capacité par un homonyme échoue.

use locus_adaptation::{
    Admission, Extension, ProposedClaim, Provenance, ReasonerError, Reasoners, Verdict, admit,
};
use locus_coordination::{Author, Capability};
use locus_environments::{
    BuildError, EnvironmentBlueprint, HealthOutcome, HealthResult, Image, Locked, Lockfile,
    Published, Requirements, Sbom, Severity, Signature, ToolchainProfile,
};
use locus_execution::ResourceSpec;
use locus_policy::{Outcome, Verb};

// ---------------------------------------------------------------------------------------------
// Fixtures — les mêmes que `tests/admission.rs`, parce que c'est le même chemin
// ---------------------------------------------------------------------------------------------

const DIGEST: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const AUTRE_DIGEST: &str =
    "sha256:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

fn blueprint(digest: &str) -> EnvironmentBlueprint {
    EnvironmentBlueprint::new(
        "dh-v1",
        "1.0.0",
        vec![ToolchainProfile::Base, ToolchainProfile::Dh],
        Image::new(digest, None).expect("digest bien formé"),
        Requirements::minimum(
            ResourceSpec::new(4_000, 8 << 30, 512, 20 << 30, 3_600).expect("quotas non nuls"),
        ),
    )
    .expect("blueprint valide")
}

fn build(digest: &str, checks: &[&str]) -> Result<Published, BuildError> {
    Locked::new(
        blueprint(digest),
        vec![Lockfile {
            path: "uv.lock".to_owned(),
            hash: digest.to_owned(),
        }],
    )?
    .built(digest)
    .inventoried(Sbom {
        components: 412,
        document_hash: digest.to_owned(),
    })?
    .scanned(Vec::new(), Severity::High)?
    .tested(
        checks
            .iter()
            .map(|name| HealthResult {
                name: (*name).to_owned(),
                outcome: HealthOutcome::Passed,
            })
            .collect(),
    )?
    .published(Signature {
        key_id: "locus-release".to_owned(),
        value: "3045…".to_owned(),
    })
}

fn admise(digest: &str, nom: &str) -> Admission {
    admit(
        Extension::Governed,
        &Outcome::Decided {
            verb: Verb::Allow,
            by: "capability/allow-dh".to_owned(),
        },
        &Author::Human("agent-dh".to_owned()),
        &Author::Human("usr-marie".to_owned()),
        &Capability::new(nom).expect("un nom non vide"),
        &build(digest, &[nom]).expect("la chaîne de W5.b va au bout"),
    )
    .expect("les quatre conditions sont réunies")
}

fn provenance() -> Provenance {
    Provenance::of("oxigraph-mcp/0.3", "cidoc-crm/7.1.3", "OWL-2-RL").expect("trois champs")
}

// ---------------------------------------------------------------------------------------------
// 1 — le raisonneur n'entre que par la porte de W5.b
// ---------------------------------------------------------------------------------------------

/// **Le registre n'accepte qu'une `Admission`**, et une `Admission` n'existe que par `admit`.
///
/// C'est la première capacité qui éprouve réellement le chemin construit par `W18.d` : jusqu'ici il
/// n'avait admis aucun artefact. Rien dans ce module ne construit un raisonneur — il ne connaît que
/// des admissions.
#[test]
fn un_raisonneur_n_entre_que_par_une_admission() {
    let mut registre = Reasoners::new();
    assert!(registre.is_empty());

    let inscrit = registre
        .register(admise(DIGEST, "sparql"))
        .expect("une admission fraîche");
    assert_eq!(inscrit.image_digest(), DIGEST);
    assert_eq!(registre.len(), 1);

    // Le module ne sait rien fabriquer : il n'a ni constructeur d'admission, ni chemin qui
    // contournerait `admit`.
    let source = include_str!("../src/reasoner.rs");
    for interdit in ["fn admit", "Admission {", "-> Admission"] {
        assert!(
            !source.contains(interdit),
            "« {interdit} » dans reasoner.rs : la porte est `admit`, et elle est ailleurs"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 2 et 4 — ce que la sortie est, et ce qu'elle ne peut pas devenir
// ---------------------------------------------------------------------------------------------

/// **Une sortie est un claim proposé, avec sa provenance complète.**
#[test]
fn une_sortie_de_raisonneur_est_un_claim_propose() {
    let propose = ProposedClaim::proposed("cidoc:E22 ⊑ bf:Work", Verdict::Consistent, provenance())
        .expect("un sujet non vide");

    assert_eq!(propose.verdict(), Verdict::Consistent);
    assert_eq!(propose.provenance().reasoner(), "oxigraph-mcp/0.3");
    assert_eq!(propose.provenance().ontology_version(), "cidoc-crm/7.1.3");
    assert_eq!(propose.provenance().profile(), "OWL-2-RL");
}

/// **Une provenance incomplète est refusée**, champ par champ.
///
/// Une conclusion sans version d'ontologie ne se rejoue pas : la même question posée à la même
/// ontologie révisée peut rendre l'inverse, et rien dans la conclusion ne le dirait.
#[test]
fn une_provenance_incomplete_est_refusee_en_nommant_le_champ() {
    assert_eq!(
        Provenance::of("", "cidoc-crm/7.1.3", "OWL-2-RL"),
        Err(ReasonerError::MissingProvenance { field: "reasoner" })
    );
    assert_eq!(
        Provenance::of("oxigraph-mcp/0.3", "  ", "OWL-2-RL"),
        Err(ReasonerError::MissingProvenance {
            field: "ontology_version"
        })
    );
    assert_eq!(
        Provenance::of("oxigraph-mcp/0.3", "cidoc-crm/7.1.3", ""),
        Err(ReasonerError::MissingProvenance { field: "profile" })
    );
}

/// **Aucun chemin ne mène d'une sortie de raisonneur à un fait validé**, ni d'une règle à une
/// décision.
///
/// Tenu par l'absence : le module ne connaît ni `Inference`, ni `Support`, ni aucun moteur de
/// règles. §20.2 exige que le moteur de politique soit déterministe à entrées identiques, et la
/// spécification de SHACL 1.2 Rules reconnaît elle-même que la négation par l'échec « pourrait
/// conduire à des graphes inférés différents selon l'ordre d'exécution » — incompatible avec une
/// décision, acceptable pour une proposition.
///
/// Les motifs visent des **types en position d'usage** — `: Inference`, `-> Support`, `use
/// locus_graph` — et non les mots nus. Le premier essai les interdisait nus, et il s'est déclenché
/// sur la documentation du module, qui les emploie pour dire exactement ce que la garde veut
/// obtenir. C'est la sixième fois de cette série, et cette fois la règle était écrite **dans ce
/// commentaire même** avant d'être enfreinte deux lignes plus bas.
#[test]
fn aucun_chemin_ne_mene_d_une_sortie_a_un_fait_ni_d_une_regle_a_une_decision() {
    let source = include_str!("../src/reasoner.rs");
    for interdit in [
        "use locus_graph",
        ": Inference",
        "-> Inference",
        "Vec<Inference>",
        ": Support",
        "-> Support",
        "fn validate",
        "fn decide",
        "-> Outcome",
        "impl From<ProposedClaim>",
        "fn as_fact",
    ] {
        assert!(
            !source.contains(interdit),
            "« {interdit} » dans reasoner.rs : un raisonneur propose, il ne valide rien"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 3 — trois verdicts, et le troisième refuse la confiance
// ---------------------------------------------------------------------------------------------

/// **`Undetermined` n'est pas un `Consistent` atténué.**
///
/// Un échec à dériver une contradiction n'est pas une cohérence : c'est la discipline de `W4.b`,
/// « une sonde non exécutée est un troisième verdict », qui refuse la confiance parce que c'est la
/// preuve qui manque. L'hypothèse de monde ouvert rend la conversion inverse systématiquement
/// fausse.
#[test]
fn undetermined_refuse_la_confiance() {
    assert_eq!(Verdict::ALL.len(), 3);
    assert!(!Verdict::Undetermined.supports_a_claim());
    assert!(Verdict::Consistent.supports_a_claim());
    assert!(Verdict::Rejected.supports_a_claim());

    // Les trois sont distincts, y compris par leur nom sur le fil.
    let noms: Vec<&str> = Verdict::ALL.iter().map(|v| v.slug()).collect();
    assert_eq!(noms, vec!["consistent", "rejected", "undetermined"]);

    // Exercé sur une entrée que le raisonneur ne sait pas trancher : le claim existe, il porte sa
    // provenance, et il **ne soutient rien**.
    let indecidable =
        ProposedClaim::proposed("fragment indécidable", Verdict::Undetermined, provenance())
            .expect("un sujet non vide");
    assert!(!indecidable.verdict().supports_a_claim());
    assert_eq!(indecidable.provenance().profile(), "OWL-2-RL");
}

// ---------------------------------------------------------------------------------------------
// 5 — la résolution se fait par identité
// ---------------------------------------------------------------------------------------------

/// **Un homonyme ne masque pas une capacité déjà inscrite.**
///
/// Le motif vient d'un harnais tiers : un provider activé **par nom** qu'on masque « redirigerait
/// silencieusement la mémoire de l'agent au lieu de simplement remplacer un outil ». Une
/// substitution de source de connaissance ne produit pas d'erreur — elle produit des réponses
/// plausibles fondées sur autre chose.
///
/// Le registre est donc clé par le **digest d'image**, et il n'existe aucune résolution par nom.
#[test]
fn un_homonyme_ne_masque_pas_une_capacite_inscrite() {
    let mut registre = Reasoners::new();
    registre
        .register(admise(DIGEST, "sparql"))
        .expect("la première");

    // La même capacité **par le nom**, une autre image. Elle s'inscrit sans rien masquer.
    registre
        .register(admise(AUTRE_DIGEST, "sparql"))
        .expect("un digest distinct est une autre capacité");

    assert_eq!(registre.len(), 2, "deux entrées, pas un remplacement");
    assert_eq!(
        registre.resolve(DIGEST).map(Admission::image_digest),
        Some(DIGEST),
        "la première reste atteignable"
    );
    assert_eq!(
        registre.resolve(AUTRE_DIGEST).map(Admission::image_digest),
        Some(AUTRE_DIGEST)
    );

    // Et il n'existe **aucune** résolution par nom, qui aurait à préférer l'une des deux.
    let source = include_str!("../src/reasoner.rs");
    for interdit in ["fn resolve_by_name", "fn by_capability", "fn find_by_name"] {
        assert!(
            !source.contains(interdit),
            "« {interdit} » dans reasoner.rs"
        );
    }
}

/// La **même identité** deux fois est refusée : un remplacement silencieux redirige sans erreur.
#[test]
fn la_meme_identite_ne_s_inscrit_pas_deux_fois() {
    let mut registre = Reasoners::new();
    registre
        .register(admise(DIGEST, "sparql"))
        .expect("la première");

    let refus = registre
        .register(admise(DIGEST, "sparql"))
        .expect_err("la même image");
    assert_eq!(
        refus,
        ReasonerError::AlreadyRegistered {
            identity: DIGEST.to_owned()
        }
    );
    assert!(
        refus.to_string().contains("sans produire d'erreur"),
        "{refus}"
    );
    assert_eq!(registre.len(), 1);
}
