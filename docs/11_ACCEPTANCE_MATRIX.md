# Matrice d’acceptation V1

| Domaine | Test de sortie |
|---|---|
| Domain | invariants/property tests + migrations |
| Event store | replay complet et concurrence optimiste |
| Graph | relations/inférences/versioning/conflits à échelle cible |
| Organisation | capacité effective = intersection des quatre sources de §14.2, jamais leur union ; ignorer une source fait rougir un test |
| Coordination | modification versionnée : deux propositions concurrentes sur la même base ne committent pas toutes deux ; une proposition sans justification citant un objet épistémique existant est refusée ; aucune `MissionEnvelope` émise n'est modifiée ; une sorte de relation sans consommateur exécutable ne peut pas entrer dans l'énumération |
| Trace | graphe d'exécution reconstruit depuis les seuls événements `lep/1.0`, sans arête orpheline ; reconstruction depuis zéro = état courant |
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
| Modes d'autorité | en `observed`, aucun chemin de code ne permet à un agent de produire une proposition de coordination ; le changement de mode est journalisé comme un acte, avec son auteur ; le proposeur ne peut jamais approuver sa propre proposition |
| Mémoire | sept niveaux distincts ; le ranking du retrieval expose ses facteurs ; aucun embedding ne contourne une ACL ; aucune fusion automatique de quasi-duplicat ; un cycle de citations sans ancrage externe est détecté |
| Métriques structurelles | `mutations_per_run`, `accepted_mutation_rate`, `rollback_rate`, `graph_edit_distance`, `edge_churn`, `agent_lifetime`, `topology_entropy`, `critical_path_length`, `parallelism`, `communication_tokens`, `state_transfer_volume`, `failure_recovery_time`, `structural_regret` — calculées depuis le seul journal |
| Gouvernance mesurée | taux de contestation des décisions de coordination, et taux d'annulation humaine des adaptations proposées par les agents |

Les quatre dernières lignes n'ont de producteur qu'après W15 et W17. Elles s'écrivent maintenant comme
la ligne `Sandbox` l'a fait avant W4.b : la matrice décrit ce qui sera exigé, pas ce qui est mesuré.
