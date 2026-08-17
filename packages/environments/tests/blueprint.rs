//! Test de sortie de W5.a — `docs/SPEC_V1.md` §19.3, §19.4, §19.7, §21.8.
//!
//! **Le blueprint refuse ce que le schéma de W0.5 ne peut pas refuser : un profil répété, un
//! préféré inférieur au minimum, une variable qui porte un secret — et il refuse une image par
//! tag, que le schéma refuse aussi, parce que le type doit tenir sans lui.**
//!
//! Un schéma JSON ne sait pas exprimer les invariants **entre** champs. Ce qui suit est la part
//! que le schéma laisse passer, plus la part qu'il refuse déjà et que le type ne doit pas se
//! contenter de supposer vérifiée en amont : rien ne garantit qu'un blueprint construit en Rust
//! soit passé par le schéma.

use std::fs;
use std::path::Path;

use locus_environments::{
    BlueprintError, EnvironmentBlueprint, Image, Requirements, ToolchainProfile,
};
use locus_execution::ResourceSpec;

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

const DIGEST: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn image() -> Image {
    Image::new(DIGEST, Some("ghcr.io/locus/ml-cpu:v1")).expect("digest bien formé")
}

fn modest() -> ResourceSpec {
    ResourceSpec::new(4_000, 8 << 30, 512, 20 << 30, 3_600).expect("quotas non nuls")
}

fn blueprint(toolchains: Vec<ToolchainProfile>) -> Result<EnvironmentBlueprint, BlueprintError> {
    EnvironmentBlueprint::new(
        "ml-cpu-v1",
        "1.0.0",
        toolchains,
        image(),
        Requirements::minimum(modest()),
    )
}

// ---------------------------------------------------------------------------------------------
// Les profils
// ---------------------------------------------------------------------------------------------

#[test]
fn les_treize_profils_de_la_spec_sont_transcrits() {
    let slugs: Vec<&str> = ToolchainProfile::ALL
        .into_iter()
        .map(ToolchainProfile::slug)
        .collect();
    assert_eq!(
        slugs,
        vec![
            "base",
            "python-science",
            "ml-cpu",
            "ml-mps",
            "ml-cuda",
            "math-formal",
            "math-compute",
            "browser",
            "dh",
            "r",
            "julia",
            "gis",
            "security",
        ],
        "§19.4 en nomme neuf puis quatre complémentaires ; en retirer un est un acte, pas un nettoyage"
    );
}

#[test]
fn un_profil_inconnu_ne_devient_pas_un_profil_par_defaut() {
    assert_eq!(ToolchainProfile::parse("pyhton-science"), None);
    assert_eq!(
        ToolchainProfile::parse("python-science"),
        Some(ToolchainProfile::PythonScience)
    );
}

#[test]
fn un_profil_compose_deux_fois_est_refuse() {
    let repeated = blueprint(vec![
        ToolchainProfile::Base,
        ToolchainProfile::PythonScience,
        ToolchainProfile::Base,
    ]);
    assert_eq!(
        repeated,
        Err(BlueprintError::DuplicateToolchain {
            profile: ToolchainProfile::Base
        }),
        "l'ordre de composition décide de ce qui écrase quoi"
    );
}

#[test]
fn l_ordre_de_composition_est_conserve() {
    let composed = blueprint(vec![
        ToolchainProfile::Base,
        ToolchainProfile::PythonScience,
        ToolchainProfile::MlCpu,
    ])
    .expect("blueprint valide");
    assert_eq!(
        composed.toolchains(),
        [
            ToolchainProfile::Base,
            ToolchainProfile::PythonScience,
            ToolchainProfile::MlCpu
        ]
    );
}

#[test]
fn un_environnement_sans_profil_ne_decrit_aucune_image() {
    assert_eq!(blueprint(Vec::new()), Err(BlueprintError::NoToolchain));
}

/// `ml-mps` est le seul profil que §19.4 déclare « non image Linux portable ». C'est le pendant,
/// côté environnement, de la portée d'accélérateur de W4.f : un profil natif décrit une machine,
/// pas une image.
#[test]
fn le_profil_natif_se_signale_et_il_est_seul() {
    let native: Vec<&str> = ToolchainProfile::ALL
        .into_iter()
        .filter(|profile| profile.is_native_only())
        .map(ToolchainProfile::slug)
        .collect();
    assert_eq!(native, vec!["ml-mps"]);

    assert!(
        blueprint(vec![
            ToolchainProfile::PythonScience,
            ToolchainProfile::MlMps
        ])
        .expect("blueprint valide")
        .is_native_only()
    );
    assert!(
        !blueprint(vec![ToolchainProfile::Base, ToolchainProfile::MlCpu])
            .expect("blueprint valide")
            .is_native_only()
    );
}

// ---------------------------------------------------------------------------------------------
// L'image
// ---------------------------------------------------------------------------------------------

#[test]
fn une_image_par_tag_est_refusee() {
    for rejected in [
        "ghcr.io/locus/base:latest",
        "sha256:trop-court",
        "sha1:0123456789abcdef0123456789abcdef01234567",
        "",
    ] {
        assert!(
            matches!(
                Image::new(rejected, None),
                Err(BlueprintError::MalformedDigest { .. })
            ),
            "« {rejected} » ne verrouille pas l'image"
        );
    }
}

#[test]
fn la_reference_documente_et_ne_designe_pas() {
    let designated = image();
    assert_eq!(designated.digest(), DIGEST);
    assert_eq!(designated.reference(), Some("ghcr.io/locus/ml-cpu:v1"));
    assert_eq!(
        Image::new(DIGEST, None).expect("digest seul").reference(),
        None,
        "un environnement se désigne par son digest, la référence est un confort de lecture"
    );
}

// ---------------------------------------------------------------------------------------------
// L'identité et les ressources
// ---------------------------------------------------------------------------------------------

#[test]
fn une_version_vide_ne_verrouille_rien() {
    for (identity, version) in [("", "1.0.0"), ("ml-cpu-v1", ""), ("  ", "  ")] {
        assert_eq!(
            EnvironmentBlueprint::new(
                identity,
                version,
                vec![ToolchainProfile::Base],
                image(),
                Requirements::minimum(modest()),
            ),
            Err(BlueprintError::EmptyIdentity),
            "§19.7 fait de la version la condition du niveau R2"
        );
    }
}

#[test]
fn un_prefere_inferieur_au_minimum_est_refuse() {
    let smaller = ResourceSpec::new(1_000, 1 << 30, 64, 1 << 30, 60).expect("quotas non nuls");
    assert_eq!(
        Requirements::minimum(modest()).preferring(smaller),
        Err(BlueprintError::PreferredBelowMinimum),
        "un environnement qui préfère moins qu'il n'exige laisse le placement choisir lequel lire"
    );
}

#[test]
fn un_prefere_qui_contient_le_minimum_est_accepte() {
    let larger =
        ResourceSpec::new(8_000, 16 << 30, 1_024, 40 << 30, 7_200).expect("quotas non nuls");
    let requirements = Requirements::minimum(modest())
        .preferring(larger.clone())
        .expect("le préféré contient le minimum");
    assert_eq!(requirements.required(), &modest());
    assert_eq!(requirements.preferred(), Some(&larger));
}

#[test]
fn le_minimum_seul_est_une_declaration_complete() {
    assert_eq!(Requirements::minimum(modest()).preferred(), None);
}

// ---------------------------------------------------------------------------------------------
// Les variables non secrètes
// ---------------------------------------------------------------------------------------------

#[test]
fn une_variable_qui_porte_un_secret_est_refusee() {
    let composed = blueprint(vec![ToolchainProfile::Base]).expect("blueprint valide");
    let refused = composed.clone().with_variable("HF_TOKEN", "hf_abcdef");
    assert!(
        matches!(refused, Err(BlueprintError::SecretInEnvironment { .. })),
        "le schéma ne peut que refuser de prévoir une place ; le type peut refuser la valeur"
    );

    let leaked = composed.with_variable("SETTINGS", "api_key=abcdef");
    assert!(
        matches!(leaked, Err(BlueprintError::SecretInEnvironment { .. })),
        "le marqueur peut être dans la valeur autant que dans le nom"
    );
}

#[test]
fn une_variable_ordinaire_passe() {
    let composed = blueprint(vec![ToolchainProfile::Base])
        .expect("blueprint valide")
        .with_variable("PYTHONUNBUFFERED", "1")
        .expect("variable non secrète")
        .with_variable("TZ", "UTC")
        .expect("variable non secrète");
    assert_eq!(composed.variables().len(), 2);
    assert_eq!(composed.variables()[0].0, "PYTHONUNBUFFERED");
}

// ---------------------------------------------------------------------------------------------
// Ce que les templates reçus ne sont pas
// ---------------------------------------------------------------------------------------------

/// `docs/10` §W5 : « les fichiers de `templates/environment/` sont le point de départ ». Ce test
/// dit ce qui les sépare d'un blueprint, pour que l'écart soit consigné plutôt que découvert au
/// moment de les charger.
///
/// Aucun des quatre ne porte `version:` ni `image:`, tous deux **obligatoires** dans le schéma de
/// W0.5. Et `ml-mps.yaml` porte un champ `trust:` que le schéma ne définit pas — un vocabulaire
/// reçu du handoff qui rejoint la portée d'accélérateur de W4.f, et qui demandera sa propre
/// décision.
#[test]
fn les_templates_recus_sont_des_points_de_depart_pas_des_blueprints() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("racine du dépôt")
        .join("templates/environment");

    let mut examined = 0;
    for name in [
        "base.yaml",
        "dh.yaml",
        "math-formal.yaml",
        "ml-cpu.yaml",
        "ml-mps.yaml",
    ] {
        let Ok(source) = fs::read_to_string(root.join(name)) else {
            continue;
        };
        examined += 1;
        assert!(
            !source.contains("version:"),
            "{name} porte une version : le charger comme blueprint devient possible, ce test doit être revu"
        );
        assert!(!source.contains("image:"), "{name} porte une image : idem");
        assert!(
            source.contains("toolchains:"),
            "{name} devrait au moins composer des profils"
        );
    }
    assert_eq!(
        examined, 4,
        "quatre templates étaient attendus ; en trouver un autre nombre change ce que ce test affirme"
    );
}

/// Tous les profils nommés par les templates existent dans l'énumération. C'est ce qui aurait
/// attrapé une divergence entre `docs/10` §W5 — qui liste six profils — et §19.4, qui en liste
/// treize.
#[test]
fn les_profils_nommes_par_les_templates_existent_tous() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("racine du dépôt")
        .join("templates/environment");

    let mut named = 0;
    for entry in fs::read_dir(&root).expect("le répertoire des templates existe") {
        let path = entry.expect("entrée lisible").path();
        let source = fs::read_to_string(&path).expect("template lisible");
        let Some(line) = source.lines().find(|line| line.starts_with("toolchains:")) else {
            continue;
        };
        let list = line
            .trim_start_matches("toolchains:")
            .trim()
            .trim_matches(['[', ']']);
        for slug in list
            .split(',')
            .map(str::trim)
            .filter(|slug| !slug.is_empty())
        {
            named += 1;
            assert!(
                ToolchainProfile::parse(slug).is_some(),
                "« {slug} », nommé par {}, n'est pas un profil de §19.4",
                path.display()
            );
        }
    }
    assert!(
        named >= 10,
        "seulement {named} profils lus : le test ne regarde presque rien"
    );
}

/// Les deux tables répondent à deux questions, et un test le fixe : ce que l'une attrape, l'autre
/// laisse passer, et c'est voulu. Les fondre obligerait la table des preuves à refuser un événement
/// de sécurité qui dit « le token de session a expiré ».
#[test]
fn les_deux_tables_de_secrets_ne_se_recouvrent_pas() {
    use locus_environments::secret_name_marker;
    use locus_execution::secret_marker;

    assert!(secret_name_marker("HF_TOKEN").is_some());
    assert!(
        secret_marker("HF_TOKEN").is_none(),
        "la table des preuves n'a pas à refuser le mot « token »"
    );

    assert!(secret_marker("Bearer abcdef").is_some());
    assert!(
        secret_name_marker("Bearer abcdef").is_none(),
        "la table des noms ne vise pas les valeurs"
    );
}

#[test]
fn le_nom_est_reconnu_quelle_que_soit_son_orthographe() {
    use locus_environments::secret_name_marker;

    for name in ["API_KEY", "api-key", "apikey", "ApiKey", "OPENAI_API_KEY"] {
        assert!(
            secret_name_marker(name).is_some(),
            "« {name} » annonce un secret"
        );
    }
    for name in [
        "PYTHONUNBUFFERED",
        "TZ",
        "LOCUS_BRANCH",
        "TOKENIZERS_PARALLELISM",
    ] {
        assert_eq!(
            secret_name_marker(name),
            None,
            "« {name} » est une variable ordinaire"
        );
    }
}
