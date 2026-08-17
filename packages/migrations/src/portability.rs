//! Les tests de portabilité — `docs/SPEC_V1.md` §4.1 et §27.5.

/// Les marqueurs d'un fournisseur d'infrastructure.
///
/// §4.1 : « aucun objet `Project`, `Branch`, `Claim`, `Review`, `Task` ou `Artifact` ne doit
/// dépendre d'un fournisseur d'infrastructure ».
///
/// `boundaries.json` vérifie déjà les **imports** ; ce qu'il ne voit pas, ce sont les noms de champ
/// et les littéraux. Un `Claim` qui porterait un champ `s3_bucket` ne violerait aucune règle
/// d'import et rendrait pourtant l'objet indéplaçable — c'est le trou que cette liste couvre.
pub const PROVIDER_MARKERS: [&str; 14] = [
    "aws_",
    "s3_",
    "gcs_",
    "gcp_",
    "azure_",
    "k8s_",
    "kubernetes_",
    "eks_",
    "gke_",
    "aks_",
    "lambda_arn",
    "cloudfront",
    "dynamodb",
    "cloudsql",
];

/// Ce qu'un balayage de portabilité a trouvé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortabilityFinding {
    /// Où.
    pub location: String,
    /// Le marqueur trouvé.
    pub marker: &'static str,
    /// La ligne fautive.
    pub line: String,
}

/// Chercher les marqueurs de fournisseur dans un texte source.
///
/// Rend des constats plutôt que de lever : le public est un test, et une liste complète se corrige
/// mieux qu'une erreur à la fois.
///
/// Les lignes de commentaire sont ignorées — nommer un fournisseur pour dire qu'on n'en dépend pas
/// n'est pas une dépendance, et l'inverse ferait échouer la garde sur sa propre documentation.
#[must_use]
pub fn provider_findings(location: &str, source: &str) -> Vec<PortabilityFinding> {
    let mut findings = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") || trimmed.starts_with('*') || trimmed.starts_with('#') {
            continue;
        }
        let lowered = line.to_lowercase();
        for marker in PROVIDER_MARKERS {
            if lowered.contains(marker) {
                findings.push(PortabilityFinding {
                    location: location.to_owned(),
                    marker,
                    line: line.trim().to_owned(),
                });
            }
        }
    }
    findings
}
