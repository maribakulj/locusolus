# Matrice d’acceptation V1

| Domaine | Test de sortie |
|---|---|
| Domain | invariants/property tests + migrations |
| Event store | replay complet et concurrence optimiste |
| Graph | relations/inférences/versioning/conflits à échelle cible |
| Workflow | crash/restart/replay/backend abstraction |
| LEP | contract suite + reconnect + late result |
| Sandbox | suite de self-tests **indexée par niveau S0–S4** : chaque test déclare le niveau minimal auquel il doit échouer côté sandbox. Un backend annonce le niveau le plus élevé pour lequel il passe. Seatbelt/bubblewrap = S1/S2 au mieux (allow-by-default, lectures ouvertes, ni cgroups ni quota disque) ; S3/S4 exigent `locus-execd` + VM/container avec quotas |
| Resources | admission/refusal/reroute corrects |
| Toolchains | health checks, lockfiles, SBOM, digests |
| Lean | projet mathlib compile et reproduction indépendante |
| PyTorch CPU | run reproductible |
| MPS | capability détectée seulement sur macOS compatible |
| CUDA | capability déclarée seulement sur worker GPU testé |
| Artifacts | hash, quarantine, promotion, restore |
| Review | blind dossier + rebuttal + meta-review |
| Tokens/budgets | réservation, mesure, dépassement et arrêt |
| Emacs | programme pilotable sans browser pour opérations courantes |
| 3D | scène web + xwidget si disponible + browser fallback |
| xiiif | artifact agentique IIIF vérifiable humainement |
| Local | MacBook deployment conformance |
| VM | Linux deployment conformance |
| Cloud | adapter conformance et limites déclarées |
| Hybrid | local Canterel + remote worker simultanés |
| Security | prompt injection/SSRF/secrets/supply-chain tests |
| Endurance | campagne longue avec redémarrages et workers perdus |
