// Généré depuis schemas/ par tooling/sdk/generate.ts — ne pas éditer à la main.
//
// `npm run check:generated` régénère et compare : une retouche manuelle fait échouer la CI.
// Ce qui doit changer, ce sont les schémas ; ils sont le contrat, ceci n'en est qu'une lecture.

// Deux dérogations, et elles portent sur du code généré, pas écrit.
//
// `missing_docs` : la documentation de ces types EST la description de leur schéma. Un champ dont
// le schéma ne dit rien n'a rien à dire, et inventer une phrase pour satisfaire le lint ajouterait
// du bruit là où le silence est exact. Ce qui manque doit être ajouté au schéma, pas ici.
//
// `doc_markdown` : les descriptions sont de la prose française qui cite des identifiants sans les
// mettre entre accents graves. Les réécrire pour le lint reviendrait à éditer le schéma depuis le
// générateur.
#![allow(missing_docs, clippy::doc_markdown)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Version du protocole portée par chaque enveloppe. Le motif accepte toute la ligne 1.x, pas seulement 1.0 : docs/06 fait du mineur un ajout de champs optionnels compatibles, donc un consommateur 1.0 doit accepter un document 1.1 et ignorer ce qu'il ne connaît pas. Un `const` ici transformerait chaque ajout mineur en rupture.
pub type ProtocolVersion = String;

/// SPEC_V1 §21.6. L'ordre est significatif : S0 < S1 < ... < S5. Un downgrade est interdit sauf approbation explicite et événement de sécurité.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SandboxLevel {
    S0,
    S1,
    S2,
    S3,
    S4,
    S5,
}

/// SPEC_V1 §21.7. `deny` par défaut pour du code non fiable. Voir schemas/README.md pour la graphie de `connector-only`, qui diffère de celle du texte de la spec.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkMode {
    #[serde(rename = "deny")]
    Deny,
    #[serde(rename = "connector-only")]
    ConnectorOnly,
    #[serde(rename = "allowlist")]
    Allowlist,
    #[serde(rename = "full")]
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AcceleratorType {
    #[serde(rename = "cuda")]
    Cuda,
    #[serde(rename = "rocm")]
    Rocm,
    #[serde(rename = "mps")]
    Mps,
    #[serde(rename = "tpu")]
    Tpu,
    #[serde(rename = "none")]
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Os {
    #[serde(rename = "linux")]
    Linux,
    #[serde(rename = "macos")]
    Macos,
    #[serde(rename = "windows")]
    Windows,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Arch {
    #[serde(rename = "x86_64")]
    X8664,
    #[serde(rename = "arm64")]
    Arm64,
}

/// Classification. Les missions ne peuvent pas l'abaisser (SPEC_V1 §21.9) ; l'ordre est croissant en sensibilité.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataClass {
    #[serde(rename = "public")]
    Public,
    #[serde(rename = "internal")]
    Internal,
    #[serde(rename = "confidential")]
    Confidential,
    #[serde(rename = "restricted")]
    Restricted,
}

/// Hash adressant un contenu immuable, préfixé par son algorithme. Le préfixe est obligatoire : un hash nu ne dit pas comment le recalculer, et une vérification d'intégrité qui devine son algorithme n'en est pas une. La longueur est vérifiée par algorithme plutôt que par une borne commode — un digest tronqué est la forme que prend une intégrité cassée.
pub type ContentHash = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainmentResult {
    #[serde(rename = "blocked")]
    Blocked,
    #[serde(rename = "allowed")]
    Allowed,
    #[serde(rename = "not-run")]
    NotRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LimitResult {
    #[serde(rename = "enforced")]
    Enforced,
    #[serde(rename = "unenforced")]
    Unenforced,
    #[serde(rename = "not-run")]
    NotRun,
}

/// Références à des artefacts, par identifiant et hash — la provenance passe par le contenu, pas par le nom.
pub type Refs = Vec<RefsItem>;

pub type Hash = String;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "code")]
pub enum Reason {
    /// L'hôte ne sait pas confiner aussi fort que la mission l'exige. **Distinct de `level_not_attested`** : « l'hôte ne sait pas faire » envoie chercher une autre machine ; « l'hôte l'annonce sans l'avoir prouvé » envoie faire tourner une campagne de self-tests. Les fondre ferait acheter du matériel pour un problème d'attestation.
    #[serde(rename = "level_unavailable")]
    LevelUnavailable {
        required: SandboxLevel,
        best: SandboxLevel,
    },
    /// La réservation dépasse la capacité de l'hôte. Le seul motif sans donnée : ce qui manque est du volume, et la réservation refusée est déjà dans la mission.
    #[serde(rename = "capacity_exceeded")]
    CapacityExceeded,
    /// L'accélérateur demandé n'est pas sur cet hôte.
    #[serde(rename = "accelerator_unavailable")]
    AcceleratorUnavailable { kind: String },
    /// L'hôte ne sait pas **borner** l'espace disque, quel qu'il en reste. Distinct de `capacity_exceeded`, et la distinction n'est pas cosmétique : « la capacité manque » envoie libérer de la place ou réduire la réservation ; « la borne n'est pas applicable ici » envoie changer de système de fichiers, ou de machine. Les fondre ferait réduire une réservation qui aurait échoué de la même façon à un octet. Né avec W5.g et W5.j, après l'écriture d'ADR 0017 §5.2 — qui en nommait six.
    #[serde(rename = "disk_quota_not_enforceable")]
    DiskQuotaNotEnforceable { requested: i64, why: String },
    /// L'hôte ne sait pas appliquer ce mode réseau.
    #[serde(rename = "network_mode_unsupported")]
    NetworkModeUnsupported { mode: NetworkMode },
    /// L'hôte annonce ce niveau mais ne l'a jamais prouvé — §12.2 demande une sandbox « disponible **et attestée** ». `proven` est **absent** quand aucune campagne n'a conclu, et ce n'est pas la même ignorance qu'un niveau prouvé trop bas : l'une envoie lancer les self-tests, l'autre dit que l'hôte a échoué à les passer.
    #[serde(rename = "level_not_attested")]
    LevelNotAttested {
        required: SandboxLevel,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        proven: Option<SandboxLevel>,
    },
    /// Une campagne a conclu pour cet hôte et ce worker, mais sous un mécanisme que le worker n'emploie pas — ADR 0035 décision 3. **Distinct de `level_not_attested`**, et la distinction est tout l'intérêt : « aucune campagne n'a conclu » envoie lancer les self-tests, celui-ci envoie en lancer une **autre**, sous le mécanisme que ce worker emploie réellement. Les fondre ferait relancer indéfiniment une campagne qui conclut déjà, et bien. Les deux noms sont au registre `schemas/lep/1.0/mechanisms.json` et ils désignent deux mécanismes différents : ADR 0035 et ADR 0036 les tiennent pour incomparables faute de les avoir mesurés l'un contre l'autre.
    #[serde(rename = "mechanism_not_employed")]
    MechanismNotEmployed {
        required: SandboxLevel,
        /// Le mécanisme que le manifeste du worker annonce.
        employs: String,
        /// Les mécanismes sous lesquels une campagne a conclu pour ce worker, et qui ont été écartés faute de correspondre. Au pluriel : plusieurs campagnes peuvent avoir déposé, et n'en nommer qu'une ferait chercher la mauvaise.
        attested: Vec<String>,
    },
    /// Le mécanisme attesté et celui du worker n'ont pas pu être **rapprochés**, faute d'un nom que le registre connaisse. Distinct de `mechanism_not_employed` : là, les deux noms sont connus et diffèrent ; ici, on ne sait pas ce qu'un nom désigne, et « ce n'est pas le même » serait une affirmation qu'on n'a pas les moyens de faire. `employs` est **absent** quand le manifeste ne nomme aucun mécanisme — `backend` est facultatif dans `CapabilityManifestSandbox` alors qu'il est obligatoire dans `SandboxAttestation` —, et ce n'est pas la même ignorance qu'un nom présent mais hors registre : l'une envoie faire annoncer son mécanisme au worker, l'autre envoie ajouter le nom au registre ou corriger l'émetteur. `unregistered` peut donc être **vide**, et il l'est exactement quand le défaut est du côté de l'annonce.
    #[serde(rename = "mechanism_unresolved")]
    MechanismUnresolved {
        required: SandboxLevel,
        /// Le mécanisme que le manifeste annonce, quand il en annonce un.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        employs: Option<String>,
        /// Les noms que le registre ne connaît pas, celui du manifeste comme ceux des attestations.
        unregistered: Vec<String>,
    },
    /// L'accélérateur **est** sur cet hôte, mais pas là où la mission veut être confinée. Distinct d'`accelerator_unavailable` : le dire « absent » enverrait chercher du matériel au lieu de choisir entre le conteneur et l'accélérateur.
    #[serde(rename = "accelerator_outside_sandbox")]
    AcceleratorOutsideSandbox {
        kind: String,
        required: SandboxLevel,
        native_level: SandboxLevel,
    },
}

/// Le GPU est une capability, pas une dépendance globale (invariant 8) : absent veut dire « aucun n'est requis », jamais « n'importe lequel fera l'affaire ».

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceSpecAccelerator {
    #[serde(rename = "type")]
    pub r#type: AcceleratorType,
    pub count: i64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub memory_mb: Option<i64>,
}

/// Ce qu'une mission réserve avant de s'exécuter. C'est une demande, pas un inventaire : voir la note dans schemas/README.md sur la raison de ne pas partager cette forme avec les ressources annoncées par un worker. Invariant 6 : les ressources sont réservées avant exécution, elles ne sont pas supposées illimitées — chaque borne est donc obligatoire, aucune n'a de défaut implicite.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceSpec {
    /// Cœurs réservés. Fractionnaire parce qu'un ordonnanceur de conteneurs sait allouer moins d'un cœur.
    pub cpu: f64,
    pub memory_mb: i64,
    pub disk_mb: i64,
    /// Borne de temps réel. Un attempt qui la dépasse est arrêté, pas prolongé.
    pub wall_time_seconds: i64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Le GPU est une capability, pas une dépendance globale (invariant 8) : absent veut dire « aucun n'est requis », jamais « n'importe lequel fera l'affaire ».
    pub accelerator: Option<ResourceSpecAccelerator>,
}

/// L'isolation qu'une mission EXIGE. Ce n'est pas ce qu'un worker offre, et les deux formes restent distinctes exprès : une exigence porte un plancher (`minimum_level`), une offre porte une liste (`levels`). Les confondre est la façon dont un ordonnanceur finit par comparer un plancher à un inventaire et par accorder S1 à une mission qui demandait S3.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SandboxSpec {
    /// Plancher, jamais un souhait. Le worker atteste le niveau réellement appliqué et un downgrade est interdit sauf approbation explicite et événement de sécurité (SPEC_V1 §21.6).
    pub minimum_level: SandboxLevel,
    pub network: NetworkMode,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Domaines joignables quand `network` vaut `allowlist`. Le schéma impose sa présence dans ce cas : une allowlist implicite est une autorisation totale qui n'ose pas dire son nom.
    pub network_allowlist: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Profil V1 (SPEC_V1 §21.6). Il nomme une intention ; c'est `minimum_level` qui engage.
    pub profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Exige du worker une SandboxAttestation (W0.6) plutôt qu'une déclaration.
    pub attestation_required: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextViewTimeRange {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub to: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextViewRedactionsItem {
    pub target: String,
    pub reason: String,
}

/// Ce que l'agent pouvait connaître, arrêté et adressé par hash (SPEC_V1 §16.2). Immuable : `content_hash` et `source_event_watermark` sont obligatoires parce qu'une vue de contexte sans eux ne permet plus de répondre à la question qui la justifie — que savait-on, et à quel instant du journal.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextView {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub root_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub included_types: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub included_relations: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub max_depth: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub time_range: Option<ContextViewTimeRange>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Invariant 11 et §12.4 : l'isolation informationnelle se décide ici. Une vue construite pour la branche A ne doit jamais atteindre une mission de la branche B.
    pub branch_scope: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub validation_levels: Option<Vec<String>>,
    pub confidentiality_ceiling: DataClass,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub artifact_policy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Invariant 12 : les résultats négatifs ne sont jamais supprimés pour rendre le graphe propre. Une vue peut les cadrer, pas les effacer — d'où `include` par défaut et l'absence de toute valeur signifiant « supprimer ».
    pub negative_result_policy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub diversity_policy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub token_budget: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub redactions: Option<Vec<ContextViewRedactionsItem>>,
    /// Position dans le journal jusqu'à laquelle la vue a été construite. C'est ce qui rend « ce que l'agent pouvait connaître » vérifiable après coup.
    pub source_event_watermark: i64,
    pub content_hash: ContentHash,
    pub generated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentBlueprintPlatform {
    pub os: Os,
    pub arch: Arch,
}

/// Par digest, jamais par tag (§21.8). Un tag est mutable, et un environnement dont l'image peut changer sous lui n'est pas verrouillé.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentBlueprintImage {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub reference: Option<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentBlueprintLockfilesItem {
    pub path: String,
    pub hash: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentBlueprintResources {
    pub minimum: ResourceSpec,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub preferred: Option<ResourceSpec>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentBlueprintMountsItem {
    pub source: String,
    pub target: String,
    pub mode: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentBlueprintHealthChecksItem {
    pub name: String,
    pub command: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub timeout_seconds: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentBlueprintAccelerator {
    #[serde(rename = "type")]
    pub r#type: AcceleratorType,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub minimum_memory_mb: Option<i64>,
}

/// Ce qu'un environnement déclare (SPEC_V1 §19.3) : OS/arch, profils de toolchain, images par digest, lockfiles, variables non secrètes, ressources, réseau, mounts, health checks et exigences d'accélérateur. Il vit sous `environments/` et non `lep/` parce qu'une mission le référence par identifiant : c'est un contrat de reproductibilité, pas une trame de fil.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentBlueprint {
    pub environment_id: String,
    /// Deux blueprints qui diffèrent portent des versions différentes. Le niveau R2 de reproductibilité (§19.7) est « environnement verrouillé » : il ne l'est que si l'identifiant l'est aussi.
    pub version: String,
    pub platform: EnvironmentBlueprintPlatform,
    pub toolchains: Vec<String>,
    /// Par digest, jamais par tag (§21.8). Un tag est mutable, et un environnement dont l'image peut changer sous lui n'est pas verrouillé.
    pub image: EnvironmentBlueprintImage,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub lockfiles: Option<Vec<EnvironmentBlueprintLockfilesItem>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Variables NON secrètes. Le schéma ne peut pas empêcher d'y mettre un token, mais il peut refuser de prévoir une place pour en mettre un : il n'y a pas de champ `secrets`, et il n'y en aura pas.
    pub env: Option<BTreeMap<String, String>>,
    pub resources: EnvironmentBlueprintResources,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub network: Option<NetworkMode>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub mounts: Option<Vec<EnvironmentBlueprintMountsItem>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub health_checks: Option<Vec<EnvironmentBlueprintHealthChecksItem>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub accelerator: Option<EnvironmentBlueprintAccelerator>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactManifestProducedBy {
    pub task_id: String,
    pub attempt: i64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub worker_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub run_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactManifestRights {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub holder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactManifestDerivedFromItem {
    pub artifact_id: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub content_hash: Option<ContentHash>,
    /// Sous-ensemble des relations typées de §7.5 qui portent une dérivation d'artefact.
    pub relation: String,
}

/// Indications d'affichage. Facultatives par construction : xiiif n'est pas requis par les agents (invariant 10), et un artefact sans hint reste un artefact complet.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactManifestViewerHints {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub iiif_manifest_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub preview_artifact_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactManifestIntegrity {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub verified_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub verified_hash_matches: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub scanner: Option<String>,
}

/// Ce que porte chaque artefact (SPEC_V1 §19.2) : hash de contenu, media type, taille, créateur/attempt, provenance, classification, droits, relations de dérivation, viewer hints, intégrité et état de quarantaine.
///
/// Invariant 4 : tout résultat scientifique majeur est artifact-first et provenance-first. Le hash, la taille et le créateur sont donc obligatoires — un artefact dont on ne sait ni ce qu'il contient, ni combien il pèse, ni qui l'a produit n'est pas un artefact, c'est un fichier.
///
/// §19.1 : le hash est déclaré AVANT l'upload, et un hash reçu qui diffère du hash déclaré fait rejeter l'envoi. Le même champ sert donc de promesse puis de preuve.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactManifest {
    pub artifact_id: String,
    pub content_hash: ContentHash,
    /// Type MIME. Il décide du viewer et du traitement, donc il n'a pas de valeur par défaut : deviner « application/octet-stream » revient à décider de ne rien afficher.
    pub media_type: String,
    pub size_bytes: i64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub filename: Option<String>,
    pub produced_by: ArtifactManifestProducedBy,
    pub classification: DataClass,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub rights: Option<ArtifactManifestRights>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Relations de dérivation, par hash et non par nom : un chemin change, un contenu non.
    pub derived_from: Option<Vec<ArtifactManifestDerivedFromItem>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Indications d'affichage. Facultatives par construction : xiiif n'est pas requis par les agents (invariant 10), et un artefact sans hint reste un artefact complet.
    pub viewer_hints: Option<ArtifactManifestViewerHints>,
    /// §19 : quarantaine et promotion. Un artefact issu de données non fiables entre en `quarantined` et n'en sort que par une revue (§21.7) — d'où l'absence de toute valeur signifiant « promu automatiquement ».
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub integrity: Option<ArtifactManifestIntegrity>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub declared_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub uploaded_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunManifestEnvironment {
    pub environment_id: String,
    pub image_digest: ContentHash,
    pub toolchains: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunManifestCodeRevision {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub repository: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Vrai quand l'arbre de travail portait des modifications non commitées. Un run dirty ne peut pas prétendre à R1, et cacher le champ ne le rendrait pas reproductible.
    pub dirty: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunManifestInputsItem {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub artifact_id: Option<String>,
    pub content_hash: ContentHash,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub role: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunManifestCommandsItem {
    pub argv: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub exit_code: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub duration_seconds: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunManifestResourcesObserved {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub cpu_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub memory_peak_mb: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub disk_peak_mb: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub wall_time_seconds: Option<f64>,
}

/// Réservé face à observé. C'est l'écart entre les deux que le rapprochement de coûts exploite, et le garder dans un seul document évite d'avoir à le reconstituer.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunManifestResources {
    pub reserved: ResourceSpec,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub observed: Option<RunManifestResourcesObserved>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunManifestOutputsItem {
    pub artifact_id: String,
    pub content_hash: ContentHash,
}

/// Ce qu'un run consigne (SPEC_V1 §19.6) : image digest, toolchains, révision du code, inputs, ressources réservées ET observées, variables pertinentes, commandes, seeds, sorties et attestations de sandbox.
///
/// C'est le document qui décide du niveau de reproductibilité atteint (§19.7) : R1 demande inputs et code identifiés, R2 un environnement verrouillé. Les champs obligatoires ici sont exactement ceux sans lesquels on ne dépasse pas R0 — « narration uniquement ».

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunManifest {
    pub run_id: String,
    pub task_id: String,
    pub attempt: i64,
    pub environment: RunManifestEnvironment,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub code_revision: Option<RunManifestCodeRevision>,
    /// Par hash. Nommer un input par son chemin ne dit pas ce qu'il contenait au moment du run.
    pub inputs: Vec<RunManifestInputsItem>,
    /// Le tableau d'arguments, pas une ligne de shell : une chaîne à réinterpréter par un shell n'est pas reproductible, et c'est aussi par là que passe une injection.
    pub commands: Vec<RunManifestCommandsItem>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Variables pertinentes, NON secrètes. Il n'y a pas de champ pour un secret, et il n'y en aura pas.
    pub env: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Sans les seeds, un run stochastique n'est pas reproductible même avec la même image et les mêmes inputs.
    pub seeds: Option<BTreeMap<String, i64>>,
    /// Réservé face à observé. C'est l'écart entre les deux que le rapprochement de coûts exploite, et le garder dans un seul document évite d'avoir à le reconstituer.
    pub resources: RunManifestResources,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub outputs: Option<Vec<RunManifestOutputsItem>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub sandbox_attestation: Option<SandboxAttestation>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// §19.7. Déclaré par le producteur et vérifiable depuis le reste du manifeste — c'est précisément ce qui le rend contestable.
    pub reproducibility_level: Option<String>,
    pub started_at: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityManifestPlatform {
    pub os: Os,
    pub arch: Arch,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub release: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityManifestModelsItem {
    pub provider: String,
    /// `oauth-local` n'est admissible que sur un worker local de confiance (docs/08) ; le schéma ne peut pas le vérifier, l'admission le peut.
    pub auth: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Vrai quand les prompts quittent la machine. C'est ce qui décide si une classe de données peut être traitée par ce modèle.
    pub remote_inference: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub models: Option<Vec<String>>,
}

/// Inventaire, pas réservation — d'où `cpu_cores` et `disk_free_mb` plutôt que le `cpu`/`disk_mb` de ResourceSpec. Deux noms différents pour deux grandeurs différentes valent mieux qu'un nom commun qui invite à les soustraire l'une de l'autre sans y penser.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityManifestResources {
    pub cpu_cores: i64,
    pub memory_mb: i64,
    pub disk_free_mb: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityManifestAcceleratorsItem {
    #[serde(rename = "type")]
    pub r#type: AcceleratorType,
    pub count: i64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub memory_mb: Option<i64>,
}

/// L'offre d'isolation. `levels` énumère ce qui est réellement applicable ; un worker macOS sous Seatbelt annonce S1/S2 et jamais S3.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityManifestSandbox {
    pub levels: Vec<SandboxLevel>,
    pub network_modes: Vec<NetworkMode>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub backend: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Vrai quand le worker sait produire une SandboxAttestation (W0.6) et non une simple déclaration.
    pub attestation: Option<bool>,
}

/// Ce qu'un worker ANNONCE au handshake (SPEC_V1 §15.3). C'est un inventaire, pas une demande : les formes ne sont volontairement pas partagées avec MissionEnvelope. Un worker annonce les niveaux de sandbox qu'il sait réellement appliquer — jamais ceux qu'il aimerait offrir.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityManifest {
    pub protocol: ProtocolVersion,
    pub worker_id: String,
    /// Ouvert exprès : LEP est un protocole d'exécution générique (docs/00), et un worker qui n'est pas Canterel doit pouvoir parler la même langue.
    pub worker_kind: String,
    pub platform: CapabilityManifestPlatform,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub models: Option<Vec<CapabilityManifestModelsItem>>,
    /// Profils de docs/SPEC_V1 §19.4. Ouvert : la liste des profils évolue plus vite que le protocole.
    pub toolchains: Vec<String>,
    /// Inventaire, pas réservation — d'où `cpu_cores` et `disk_free_mb` plutôt que le `cpu`/`disk_mb` de ResourceSpec. Deux noms différents pour deux grandeurs différentes valent mieux qu'un nom commun qui invite à les soustraire l'une de l'autre sans y penser.
    pub resources: CapabilityManifestResources,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub accelerators: Option<Vec<CapabilityManifestAcceleratorsItem>>,
    /// L'offre d'isolation. `levels` énumère ce qui est réellement applicable ; un worker macOS sous Seatbelt annonce S1/S2 et jamais S3.
    pub sandbox: CapabilityManifestSandbox,
    /// Classes que ce worker est autorisé à traiter. Le plafond d'une mission ne peut pas les dépasser.
    pub data_classes: Vec<DataClass>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub max_concurrency: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub mission_kinds: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub trust_level: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionEnvelopeObjective {
    pub statement: String,
    /// Au moins une, et énoncée avant l'exécution. Un critère de succès écrit après coup ne prouve rien.
    pub success_conditions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub failure_conditions: Option<Vec<String>>,
}

/// Référence, pas contenu : la mission porte l'identité et le hash de la vue, le worker la matérialise. Le hash est obligatoire — sans lui, « ce que l'agent pouvait connaître » n'est plus vérifiable.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionEnvelopeContextView {
    pub id: String,
    pub hash: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionEnvelopeEnvironment {
    pub environment_id: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub image_digest: Option<ContentHash>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub toolchains: Option<Vec<String>>,
}

/// Invariant 6 appliqué au coût du modèle. Les trois bornes sont obligatoires : une seule d'entre elles laissée libre suffit à rendre le dépassement impossible à constater.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionEnvelopeBudget {
    pub max_model_calls: i64,
    pub max_input_tokens: i64,
    pub max_output_tokens: i64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub max_cost_micros: Option<i64>,
}

/// Ce qu'une mission DEMANDE (SPEC_V1 §15.4). Rien ici n'est optionnel par commodité : objectif, contexte, sandbox, ressources, budget et contrat de sortie sont ce qui rend une mission admissible ou refusable, et une mission qui en omet un ne peut pas être jugée — elle serait acceptée par défaut, ce qui est exactement le contraire de l'admission.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionEnvelope {
    pub protocol: ProtocolVersion,
    pub task_id: String,
    /// Une tâche durable connaît plusieurs attempts (§15.5). L'identifiant d'attempt est ce qui distingue un résultat tardif d'un doublon.
    pub attempt_id: String,
    pub branch_id: String,
    pub objective: MissionEnvelopeObjective,
    /// Référence, pas contenu : la mission porte l'identité et le hash de la vue, le worker la matérialise. Le hash est obligatoire — sans lui, « ce que l'agent pouvait connaître » n'est plus vérifiable.
    pub context_view: MissionEnvelopeContextView,
    pub environment: MissionEnvelopeEnvironment,
    pub sandbox: SandboxSpec,
    pub resources: ResourceSpec,
    /// Invariant 6 appliqué au coût du modèle. Les trois bornes sont obligatoires : une seule d'entre elles laissée libre suffit à rendre le dépassement impossible à constater.
    pub budget: MissionEnvelopeBudget,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub required_capabilities: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub confidentiality_ceiling: Option<DataClass>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Invariant 11 : un reviewer indépendant ne reçoit pas le raisonnement privé du générateur. `independent` est ce qui l'engage.
    pub review_policy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Le rôle de l'instance d'agent, au sens de `SPEC_V1.md` §7.1 (`AgentTemplate.role`) et §20 (`- role: logical-reviewer`). Optionnel : un document `1.0` le laisse absent, et absent ne se remplit pas d'un défaut. Chaîne libre et non énumération, parce qu'un mineur ajoute des champs et jamais des valeurs (ADR 0017, interdit 3) — un rôle nouveau dans une énumération fermée ferait échouer la désérialisation chez tout consommateur `1.0`. Ne prend jamais le pas sur `review_policy` : l'invariant 11 décide avant le rôle.
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// La permission de poursuivre **hors ligne** — `SPEC_V1.md` §1.2 (dernier invariant) et §24.3. Distincte de `sandbox.network_mode`, et les deux ne se dérivent jamais l'une de l'autre : `network_mode: deny` est une **contrainte** imposée au worker, qui lui retire le réseau ; cette permission est une **dispense**, qui l'autorise à ne pas échouer quand le réseau manque. Une mission peut exiger `full` et n'avoir aucune dispense — elle échoue si le réseau tombe ; une autre peut être en `deny` sans dispense — elle n'a jamais eu de réseau à perdre. Les confondre ferait d'un confinement une autorisation, ce qu'ADR 0004 sépare partout ailleurs. Absente, elle ne s'accorde pas : un document `1.0` ne demande pas cette dispense, il n'en parle pas.
    pub offline_allowed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Le plafond de travail hors ligne, en millisecondes. Sans effet sans `offline_allowed` : un budget n'est pas une permission. Le lecteur retient de toute façon le plus contraignant de ce budget et du lease restant — un budget plus long que le lease donnerait le droit de travailler après la fin du droit de travailler.
    pub offline_budget_ms: Option<i64>,
    /// Ce que l'attempt doit rendre. `epistemic-commit/1` n'a aucune autorité de validation avant traitement par Locus Solus (§15.7).
    pub output_contract: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub deadline: Option<String>,
}

/// Invariant 6 : les ressources sont réservées, pas supposées illimitées. Une limite absente est une limite absente.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SandboxAttestationLimits {
    pub cpu: f64,
    pub memory_mb: i64,
    pub pids: i64,
    pub disk_mb: i64,
}

/// Les quatre self-tests d'ADR 0004. Tous obligatoires : un self-test qu'on peut omettre est un self-test qu'on omettra le jour où il échoue. `not-run` existe pour que « je ne l'ai pas exécuté » soit dicible — et distinct de « il a réussi ».

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SandboxAttestationSelfTests {
    pub write_outside_workspace: ContainmentResult,
    pub read_host_home: ContainmentResult,
    pub network_egress: ContainmentResult,
    pub memory_limit: LimitResult,
}

/// Ce que le worker atteste avoir RÉELLEMENT appliqué, face au plancher qu'une SandboxSpec exigeait (SPEC_V1 §21.6). ADR 0004 fait des self-tests la définition opérationnelle du mot « sandbox » : l'attestation les porte, sans quoi elle n'est qu'une déclaration d'intention.
///
/// Une attestation doit pouvoir décrire une MAUVAISE sandbox. `host_home_mounted: true` est un document valide — refusé par l'admission, jamais par le schéma. Interdire de l'écrire ne rendrait pas le montage impossible : ça rendrait seulement le worker incapable de l'avouer, et un worker non conforme mais muet est pire qu'un worker non conforme qui le dit.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SandboxAttestation {
    pub sandbox_id: String,
    pub backend: String,
    pub isolation_level: SandboxLevel,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub image_digest: Option<ContentHash>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub rootless: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub read_only_rootfs: Option<bool>,
    pub network_mode: NetworkMode,
    /// Obligatoire, et obligatoire même quand la réponse est `false`. Un champ absent se lit « je n'ai pas regardé » aussi bien que « non » ; seul l'un des deux est une attestation.
    pub host_home_mounted: bool,
    pub runtime_socket_exposed: bool,
    /// Invariant 6 : les ressources sont réservées, pas supposées illimitées. Une limite absente est une limite absente.
    pub limits: SandboxAttestationLimits,
    /// Les quatre self-tests d'ADR 0004. Tous obligatoires : un self-test qu'on peut omettre est un self-test qu'on omettra le jour où il échoue. `not-run` existe pour que « je ne l'ai pas exécuté » soit dicible — et distinct de « il a réussi ».
    pub self_tests: SandboxAttestationSelfTests,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub attested_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub signature: Option<String>,
}

/// L'enveloppe que porte tout événement worker → serveur (SPEC_V1 §15.2 : « toutes les enveloppes portent version de protocole, sequence, correlation IDs et idempotency key »). Les types minimaux sont ceux de §15.6.
///
/// À ne pas confondre avec l'enveloppe du journal institutionnel de §10.1, qui vit sous `schemas/events/` et appartient à W1 : celle-ci traverse le fil, celle-là est écrite dans l'event store. Un worker ne modifie jamais directement la base canonique (invariant 3), donc les deux ne peuvent pas être le même objet.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub protocol: ProtocolVersion,
    /// Fermé exprès, contrairement aux documents. Un type d'événement inconnu n'est pas un champ qu'on peut ignorer : le consommateur ne saura ni quoi en faire ni s'il vient de rater quelque chose. Un nouveau type est un ajout mineur qui met à jour cette liste.
    pub event_type: String,
    /// Monotone par connexion. C'est ce qui permet l'acquittement et la reprise de stream (§12.4) : sans lui, « rien perdu, rien dupliqué » n'est pas vérifiable.
    pub sequence: i64,
    pub occurred_at: String,
    /// Un événement rejoué après reconnexion porte la même clé. C'est ce qui rend la reprise sûre plutôt que dupliquante.
    pub idempotency_key: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub attempt: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub lease_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub worker_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub correlation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub causation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub payload: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub payload_hash: Option<ContentHash>,
}

/// Présente quand l'état est `failed`. La forme suit l'enveloppe d'erreur structurée de `packages/protocol` (W0.4).

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttemptError {
    pub category: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub retryable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub retry_after_seconds: Option<i64>,
}

/// Ce qui a réellement été consommé, face à ce que ResourceSpec avait réservé. L'écart est ce que le rapprochement de coûts (§4) exploite.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttemptResourcesObserved {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub cpu_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub memory_peak_mb: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub disk_peak_mb: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub wall_time_seconds: Option<f64>,
}

/// Ce que ce sous-agent a coûté. Facultatif : un harnais qui ne mesure pas ne déclare pas, et zéro dirait « mesuré à zéro » là où la vérité est « non mesuré » — l'aveu s'appelle ici l'absence, comme pour tout champ `1.1`.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttemptSubagentsItemCost {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub calls: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub wall_time_seconds: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttemptSubagentsItem {
    /// Le nom du sous-agent dans le harnais. Une **désignation**, jamais son contexte : deux sous-agents du même nom sont deux exécutions du même rôle.
    pub name: String,
    /// Sa classe de cognition, au sens de `W25.a` — une **classe**, jamais un identifiant de modèle. Chaîne libre et non énumération : un mineur ajoute des champs et jamais des valeurs (ADR 0017, interdit 3), et une classe nouvelle dans une énumération fermée ferait échouer la désérialisation chez tout consommateur `1.0`.
    pub cognition: String,
    /// Ce qu'il a rendu — le **résultat**, pas le raisonnement qui y mène. Les trois valeurs sont celles qu'un exploitant distingue : abouti, échoué, ou interrompu avant terme. Confondre les deux derniers ferait lire un budget épuisé comme une erreur de sous-agent.
    pub outcome: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Ce que ce sous-agent a coûté. Facultatif : un harnais qui ne mesure pas ne déclare pas, et zéro dirait « mesuré à zéro » là où la vérité est « non mesuré » — l'aveu s'appelle ici l'absence, comme pour tout champ `1.1`.
    pub cost: Option<AttemptSubagentsItemCost>,
}

/// Une tentative d'exécution d'une tâche (SPEC_V1 §15.5). Un attempt ne produit jamais directement un état canonique : il soumet des artefacts et un EpistemicCommit. `succeeded` veut dire que le worker a rempli son contrat technique — jamais que ses claims sont validés, et les deux mots restent séparés partout ici.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attempt {
    pub protocol: ProtocolVersion,
    pub task_id: String,
    pub attempt: i64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub lease_id: Option<String>,
    pub worker_id: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub agent_id: Option<String>,
    /// Sous-ensemble des états de tâche de §5 qu'un attempt peut porter côté worker. `accepted`, `rejected` et `superseded` en sont absents exprès : ce sont des verdicts de Locus Solus sur un attempt terminé, pas des états que le worker s'attribue.
    pub state: String,
    pub started_at: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub completed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Vrai quand le résultat arrive après l'expiration de la lease. §12.3 : un résultat tardif est stocké en quarantaine et ne peut committer sans arbitrage. Le champ existe pour que le worker puisse le dire lui-même plutôt que de le laisser deviner.
    pub late: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Présente quand l'état est `failed`. La forme suit l'enveloppe d'erreur structurée de `packages/protocol` (W0.4).
    pub error: Option<AttemptError>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Ce qui a réellement été consommé, face à ce que ResourceSpec avait réservé. L'écart est ce que le rapprochement de coûts (§4) exploite.
    pub resources_observed: Option<AttemptResourcesObserved>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Ce que l'institution voit des sous-agents internes du harnais — `W16.d`, tranche 4 du mineur `lep/1.1` (ADR 0017 §5.4), tranché par l'ADR 0027 décision 7. **Facultatif** : un harnais qui ne subdivise pas n'a rien à déclarer, et l'obliger à déclarer « aucun » ferait payer la fonctionnalité à ceux qui ne l'utilisent pas. La feature `subagent-visibility` la négocie au handshake ; absente, ce champ ne s'attend pas. **Quatre choses, et pas une cinquième** : qu'un sous-agent a existé, sa classe de cognition, son coût et son résultat. Le contexte et le raisonnement n'y sont pas, et ce n'est pas un oubli — voir qu'un sous-agent existe et voir son contexte sont deux choses, et la seconde traverse l'invariant 11. Un sous-agent reviewer interne au harnais ne doit pas devenir le chemin par lequel le raisonnement privé du générateur remonte. Sa lecture, quand elle est due, passe par les trois classes de lecteurs de l'ADR 0027 décision 2 et jamais par un chemin propre au harnais.
    pub subagents: Option<Vec<AttemptSubagentsItem>>,
}

/// Le droit, borné dans le temps, d'exécuter un attempt (SPEC_V1 §12.3). Une lease expire ; c'est ce qui distingue un worker en panne d'un worker lent, et ce qui permet à `task.orphaned` d'exister sans qu'on interroge personne.
///
/// Une contrainte de §12.3 n'est PAS exprimable ici : « le worker envoie un heartbeat à intervalle inférieur au tiers du TTL » est une relation entre deux champs, que Draft 7 ne sait pas énoncer. Elle est vérifiée par le harnais de conformance (W0.9), et le dire ici vaut mieux que laisser croire que le schéma la couvre.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lease {
    pub protocol: ProtocolVersion,
    pub lease_id: String,
    pub task_id: String,
    /// Rang, pas identifiant : une tâche réattribuée conserve son numéro d'attempt (§12.3). Le compter à partir de 1 rend « attempt 0 » impossible à confondre avec « pas encore tenté ».
    pub attempt: i64,
    pub worker_id: String,
    pub issued_at: String,
    pub expires_at: String,
    /// Courte et renouvelable (§12.3). Aucune borne haute ici : c'est une politique de scheduler, pas une propriété du protocole.
    pub ttl_seconds: i64,
    pub heartbeat_interval_seconds: i64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub renewal_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Les side effects utilisent des clés d'idempotence indépendantes de l'attempt (§12.3) : rejouer un attempt ne doit pas rejouer ses effets.
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefsItem {
    pub artifact_id: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub content_hash: Option<ContentHash>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EpistemicCommitClaimsItem {
    pub statement: String,
    /// Une claim sans confiance déclarée se lit comme certaine. Obligatoire, donc, et bornée.
    pub confidence: f64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub evidence_refs: Option<Refs>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub assumptions: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EpistemicCommitObjectionsItem {
    pub statement: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub targets: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub evidence_refs: Option<Refs>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EpistemicCommitInferencesItem {
    pub rule: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub inference_kind: Option<String>,
    pub premise_refs: Vec<String>,
    pub conclusion_refs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub assumption_refs: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EpistemicCommitLocalDecisionsItem {
    pub decision: String,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EpistemicCommitNegativeResultsItem {
    pub statement: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub attempted: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub evidence_refs: Option<Refs>,
}

/// Ce qu'un attempt PROPOSE (SPEC_V1 §15.7) : claims, objections, inférences, décisions locales, résultats négatifs, limitations, références d'artefacts et prochaines actions.
///
/// Il n'a AUCUNE autorité de validation avant traitement par Locus Solus, et le schéma le rend indéfaisable plutôt que de le répéter : `status` ne peut valoir que `draft` ou `staged`. `validated`, `under_review` et le reste du cycle de vie de §7.4 existent, mais ce sont des verdicts que l'institution prononce — un worker qui les écrirait s'auto-validerait, ce qui est exactement l'invariant 3.
///
/// Un commit ne peut pas contenir de secret (§21.8). Le schéma n'a aucun champ pour en loger un ; le scan, lui, appartient à l'admission.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EpistemicCommit {
    pub protocol: ProtocolVersion,
    pub task_id: String,
    pub attempt: i64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub branch_id: Option<String>,
    /// Jamais au-delà de `staged` (§2.3). C'est la garantie la plus forte de ce schéma, et la seule que sa violation rende un document littéralement invalide.
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub claims: Option<Vec<EpistemicCommitClaimsItem>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Invariant 12 : les conflits ne sont jamais supprimés pour rendre le graphe propre. Une objection est un contenu de premier plan, pas une note de bas de page.
    pub objections: Option<Vec<EpistemicCommitObjectionsItem>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Une inférence est un nœud explicite (§7.6), avec ses prémisses et ses hypothèses — pas une flèche implicite entre deux claims.
    pub inferences: Option<Vec<EpistemicCommitInferencesItem>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub local_decisions: Option<Vec<EpistemicCommitLocalDecisionsItem>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Invariant 12, encore : un résultat négatif est un résultat. Le champ existe pour qu'il ait un endroit où aller, sans quoi il finit dans un commentaire libre et disparaît.
    pub negative_results: Option<Vec<EpistemicCommitNegativeResultsItem>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub limitations: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub artifact_refs: Option<Refs>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub next_actions: Option<Vec<String>>,
    pub produced_at: String,
}

/// Ce que le run a constaté. Le hash du snapshot est ce qui prouve la reproduction ; celui de la ressource live, quand il est connu, ne sert qu'à constater l'évolution.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteArtifactRefExpected {
    pub snapshot_hash: Hash,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub live_hash_at_run: Option<Hash>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub captured_at: Option<String>,
}

/// Comment atteindre la ressource. §19 en nomme cinq et n'en autorise qu'un : deux locators laisseraient au viewer le soin de choisir, donc de choisir différemment d'une fois sur l'autre.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteArtifactRefLocator {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub manifest_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub canvas_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub content_state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub annotation_target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub local_snapshot: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteArtifactRef {
    /// L'identité canonique de l'artefact côté Locus. §19 exige qu'elle s'affiche séparément de la ressource distante : c'est elle qui ne bouge pas.
    pub artifact_id: String,
    pub media_type: String,
    /// Ce que le run a constaté. Le hash du snapshot est ce qui prouve la reproduction ; celui de la ressource live, quand il est connu, ne sert qu'à constater l'évolution.
    pub expected: RemoteArtifactRefExpected,
    /// Comment atteindre la ressource. §19 en nomme cinq et n'en autorise qu'un : deux locators laisseraient au viewer le soin de choisir, donc de choisir différemment d'une fois sur l'autre.
    pub locator: RemoteArtifactRefLocator,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Ce que l'artefact suggère, jamais ce qu'il impose : xiiif n'est pas requis par les agents (invariant 10).
    pub viewer_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeploymentAdaptersItem {
    pub role: String,
    pub implementation: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeploymentSecretRefsItem {
    pub name: String,
    pub reference: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Deployment {
    /// Lequel des cinq profils obligatoires de §27.1.
    pub profile: String,
    /// L'URL Locus à laquelle les clients se connectent. C'est tout ce qu'ils voient de la topologie.
    pub endpoint: String,
    /// Quel rôle est tenu par quelle implémentation. Une liste plutôt qu'un objet : le domaine refuse un rôle déclaré deux fois, ce qu'un objet JSON rendrait indétectable — le second écraserait le premier en silence.
    pub adapters: Vec<DeploymentAdaptersItem>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Les limites du déploiement, déclarées plutôt que contournées (§27.1).
    pub capabilities: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Où trouver un secret, jamais le secret. Le motif refuse une valeur en clair : `hunter2` n'est pas une référence.
    pub secret_refs: Option<Vec<DeploymentSecretRefsItem>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewNodesItem {
    pub id: String,
    pub kind: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewEdgesItem {
    pub from: String,
    pub to: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct View {
    /// Laquelle des huit projections de §23.3.
    pub kind: String,
    /// Le point du journal auquel la vue a été prise. Un viewer qui l'ignore ne peut pas dire s'il montre l'état d'aujourd'hui.
    pub watermark: i64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Le condensat de la vue dont celle-ci est un cadrage ou un filtre. Absent pour une projection ; présent sans exception pour une vue dérivée, y compris quand le filtre n'a rien retiré.
    pub derived_from: Option<String>,
    /// Le condensat de la forme canonique. Le consommateur la reconstruit et compare : ce qui prouve ne peut pas être ce qui est demandé.
    pub digest: String,
    pub nodes: Vec<ViewNodesItem>,
    pub edges: Vec<ViewEdgesItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HumanReviewFinding {
    /// Le ReviewDossier auquel ce finding s'attache. §20 : la revue humaine « produit un finding attachable à un ReviewDossier », donc elle en nomme un.
    pub dossier_id: String,
    /// La révision revue. Le domaine refuse une cible que le dossier ne couvre pas : sans cela une revue humaine élargirait le dossier en silence.
    pub target: String,
    /// Qui a regardé. Une identité humaine, pas un agent : elle ne passe pas par l'attestation d'indépendance de §17.4, parce que ce n'est pas une revue indépendante.
    pub reviewer: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// L'un des quatre de §20, sous son nom. `accept` ne dit pas que la revendication tient : il dit que le relecteur humain n'a pas d'objection, ce qui n'est pas une preuve.
    pub verdict: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Le commentaire libre, cinquième forme d'enregistrement de §20. Un finding qui ne porte ni verdict ni commentaire ne dit rien, et `anyOf` le refuse.
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Les révisions sur lesquelles le relecteur s'appuie. §17.5 : un finding sans preuve concrète est un commentaire non bloquant — la règle vaut pour un humain comme pour un agent, et c'est elle, pas la qualité du relecteur, qui décide si le finding est opposable.
    pub evidence: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub recorded_at: Option<String>,
}

/// Pourquoi une mission n'a pas été admise sur un hôte — SPEC_V1 §10.2, ADR 0017 §5.2, tranche 2 du mineur `lep/1.1`. Document **nouveau** : aucune énumération existante ne gagne un membre, ce que l'interdit 3 de l'ADR refuse. Il porte des données et pas seulement des codes — le niveau exigé, le meilleur niveau prouvé, le genre d'accélérateur — donc un membre de plus sur une énumération n'aurait de toute façon pas suffi.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdmissionRefusal {
    pub protocol: ProtocolVersion,
    pub task_id: String,
    pub attempt_id: String,
    /// **Toutes** les conditions manquantes, jamais la première seule. `admit` les accumule et rend `Refused { reasons }` au pluriel ; un fil qui n'en transmettrait qu'une ferait corriger une condition pour retomber aussitôt sur la suivante, autant de fois qu'il en manque. `minItems: 1` parce qu'un refus sans motif n'est pas un refus.
    pub reasons: Vec<Reason>,
}

/// Les documents qu'un pair peut envoyer ou recevoir, dans l'ordre du registre.
pub const LEP_DOCUMENTS: [&str; 14] = [
    "ArtifactManifest",
    "RunManifest",
    "CapabilityManifest",
    "MissionEnvelope",
    "SandboxAttestation",
    "Event",
    "Attempt",
    "Lease",
    "EpistemicCommit",
    "RemoteArtifactRef",
    "Deployment",
    "View",
    "HumanReviewFinding",
    "AdmissionRefusal",
];

/// Les features négociables au handshake, avec le mineur qui les introduit.
pub const LEP_FEATURES: [(&str, &str); 6] = [
    ("late-results", "1.0"),
    ("human-input", "1.0"),
    ("pull-queue", "1.0"),
    ("artifact-streaming", "1.0"),
    ("signed-events", "1.0"),
    ("subagent-visibility", "1.1"),
];

/// Les mécanismes de confinement dont ce dépôt sait ce qu'ils désignent — registre
/// `schemas/lep/1.0/mechanisms.json`, ADR 0035 décision 3.
///
/// Ce n'est pas une énumération du fil : `backend` reste une chaîne libre dans les deux schémas
/// qui le portent. Un nom absent d'ici n'est pas invalide, il est **non rapproché**, et c'est un
/// verdict différent de « ce n'est pas le même mécanisme ».
pub const LEP_MECHANISMS: [&str; 5] = [
    "bubblewrap",
    "bubblewrap+cgroup",
    "podman-rootless",
    "seatbelt",
    "none",
];
