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
| Métriques structurelles | Les treize de l'**ADR 0024**, qui les définit une par une — formule, numérateur, dénominateur, et ce que chacune ne prétend pas dire : `mutations_per_run`, `edge_churn`, `applied_edit_length`, `accepted_mutation_rate`, `rollback_rate`, `structural_regret`, `degree_entropy`, `critical_path_length`, `average_parallelism`, `communication_tokens`, `handed_over_attempts`, `agent_lifetime`, `failure_recovery_time`. Toutes calculées depuis le seul journal, et rejouables à l'identique sur le même préfixe ; aucune ne porte de seuil, de note ni de verdict, un seuil étant une décision de politique et non un fait mesuré. Quatre noms ont été arrêtés différemment de la première rédaction, chacun parce qu'il promettait plus que son calcul — voir la décision 2 de l'ADR |
| Gouvernance mesurée | taux de contestation des décisions de coordination, et taux d'annulation humaine des adaptations proposées par les agents |

Les quatre dernières lignes attendaient W15 et W17, qui sont faits. Elles ne sont plus dans le même
état, et les confondre ferait relire cette note comme une excuse valable pour les quatre :

- **Modes d'autorité** et **Mémoire** ont leurs producteurs. Le mode `observed` et le journal du
  changement de mode viennent de W13/W15 ; les sept niveaux, les facteurs de ranking, l'ACL et le
  refus de fusion automatique viennent de W17.k à W17.n, et la détection d'un cycle de citations sans
  ancrage externe de `R1`.
- **Gouvernance mesurée** a la moitié qui la concerne : le taux d'annulation humaine des adaptations
  est livré par W18.e, qui compte les annulations humaines seulement et déclare **hors mesure** ce
  que personne n'a regardé.
- **Métriques structurelles** est la seule qui reste à produire, et c'est la phase W21. Une seule des
  treize existe — `structural_regret`, livrée par `R3`. L'ADR 0024 a arrêté les douze autres avant
  qu'aucune ne s'écrive, parce qu'elles étaient nommées sans être définies, et qu'un nombre publié
  sous un nom qu'on n'a pas défini sera lu, cité et suivi sans que personne sache ce qu'il compte.

Ce que la note d'origine disait reste vrai de cette dernière ligne, et d'elle seule : la matrice
décrit ce qui sera exigé, pas ce qui est déjà mesuré. Elle l'a dit des quatre pendant trois jours de
plus qu'il ne fallait, ce que `W0.17` a constaté un document plus loin.
