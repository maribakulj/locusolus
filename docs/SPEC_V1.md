# Locus Solus — Spécification fonctionnelle, épistémique et architecturale V1

**Dépôt cible :** `maribakulj/locusolus`  
**Nom du produit :** Locus Solus  
**Daemon :** `locusd`  
**CLI :** `locus`  
**Client Emacs :** `locusolus.el`  
**Protocole worker :** Locus Execution Protocol, LEP v1  
**Statut :** spécification normative de la V1  
**Positionnement :** système d’exploitation local-first pour la recherche cumulative menée par des collectifs humains et agentiques

---

## 0. Statut et conventions normatives

Cette spécification définit le produit complet attendu pour Locus Solus V1. Elle ne décrit ni une démonstration, ni une version d’essai, ni une succession de prototypes jetables. L’implémentation pourra être organisée en chantiers parallèles et livraisons internes, mais la version publique `1.0.0` ne devra être déclarée achevée que lorsque les critères d’acceptation de la section 32 seront satisfaits.

Les termes **DOIT**, **NE DOIT PAS**, **DEVRAIT**, **NE DEVRAIT PAS** et **PEUT** sont normatifs.

Principes de rédaction :

- les objets scientifiques et organisationnels sont décrits indépendamment des technologies employées ;
- les exemples de schémas sont illustratifs, mais les invariants associés sont obligatoires ;
- aucune règle décisive ne doit exister uniquement dans un prompt ;
- tout comportement automatisé ayant un effet sur l’état canonique doit être explicable, versionné et rejouable ;
- toute sortie textuelle d’un agent est non canonique tant qu’elle n’a pas été convertie en objets structurés et soumise aux politiques appropriées.

---

## 1. Vision du produit

Locus Solus est le noyau institutionnel d’un laboratoire de recherche agentique. Il transforme des modèles, agents, outils, notebooks et services de calcul en une organisation persistante capable de :

- conduire plusieurs programmes de recherche de longue durée ;
- explorer simultanément des attaques intellectuellement différentes ;
- créer des équipes spécialisées en fonction des obstacles rencontrés ;
- conserver hypothèses, arguments, objections, contre-exemples, résultats négatifs, décisions et artefacts ;
- organiser des revues aveugles, rebuttals, reproductions et formalisation ;
- arbitrer un portefeuille de branches selon leur valeur scientifique, leur coût et leur diversité ;
- reprendre exactement une campagne après interruption ;
- rendre l’intégralité du processus inspectable depuis Emacs, le Web et la CLI ;
- fonctionner sans dépendance obligatoire à un service propriétaire ;
- échanger des objets de recherche avec d’autres installations sans céder la maîtrise du stockage local.

Locus Solus n’est pas un « chat multi-agent ». Le produit doit se comporter comme une **institution de recherche programmable** : il possède un droit interne, une mémoire, des procédures de preuve, des rôles, des budgets, des archives, des mécanismes de contestation et une temporalité propre.

### 1.1 Proposition de valeur

Locus Solus répond à cinq déficits des systèmes agentiques ordinaires :

1. **Volatilité** — les conversations se terminent, alors qu’une recherche doit survivre aux sessions et aux modèles.
2. **Fragmentation** — les agents produisent des textes isolés, alors que la connaissance doit former un réseau explicite de dépendances et de conflits.
3. **Absence d’institution** — plusieurs agents ne constituent pas une organisation tant qu’il n’existe ni allocation, ni responsabilité, ni procédure de revue.
4. **Confusion entre génération et validation** — la plausibilité verbale n’est pas une preuve.
5. **Perte des échecs** — une attaque réfutée ou un calcul négatif doit réduire l’espace de recherche futur au lieu de disparaître dans un transcript.

### 1.2 Invariants non négociables

Locus Solus V1 DOIT respecter les invariants suivants :

- un programme de recherche n’est jamais identifié à une session de chat ;
- une branche n’est jamais identifiée à un seul agent ;
- un agent n’est jamais la source de vérité de son propre résultat ;
- une revendication scientifique majeure ne peut être validée par simple vote de modèles ;
- une décision automatisée est toujours rattachée à une politique versionnée ;
- un artefact promu est immuable et adressé par son contenu ;
- une révision ne détruit jamais la version antérieure ;
- une contradiction non résolue est un état légitime ;
- un workflow durable ne contient pas l’état scientifique canonique ;
- aucun worker n’accède directement à la base canonique ;
- aucune perte d’état n’est acceptable après acquittement d’une commande ;
- les coûts et ressources sont réservés, consommés et rapprochés dans un registre auditable ;
- une installation locale complète doit rester utilisable hors ligne, à l’exception des outils ou modèles explicitement distants.

---

## 2. Périmètre et frontières de responsabilité

### 2.1 Locus Solus possède

Locus Solus est l’autorité canonique sur :

- workspaces, programmes, workstreams, branches et tâches durables ;
- équipes et instances d’agents comme objets institutionnels ;
- politiques, décisions, approbations, délégations et budgets ;
- graphe épistémique versionné, inférences et conflits ;
- états de validation, dossiers de revue, findings, rebuttals et méta-revues ;
- journal d’événements, projections, mémoire collective et `ContextView` ;
- registre d’artefacts, manifestes d’environnement et de reproduction ;
- planification durable et placement des missions ;
- contrats d’exécution, capacités des workers et attestations ;
- profils de déploiement et ports d’infrastructure ;
- export, import et fédération des objets canoniques.

Locus Solus ne doit pas contenir un second runtime LLM complet. Il gouverne le laboratoire ; il ne remplace pas ses chercheurs ni ses instruments.

### 2.2 Canterel possède

Canterel, anciennement `openscienceDH`, est le principal runtime scientifique agentique de l’écosystème. Il reste responsable de :

- sessions LLM, streaming, compaction et handoffs ;
- routage des modèles et fournisseurs, y compris OAuth local lorsque permis ;
- agents scientifiques, reviewers locaux, tools, skills, MCP et connecteurs ;
- interaction avec shell, notebooks, navigateurs et toolchains exposés ;
- exécution concrète d’une mission dans un environnement accordé par Locus Solus ;
- production de fichiers, calculs, rapports, artefacts et résultats structurés ;
- remontée des événements d’exécution et de consommation ;
- construction d’un `EpistemicCommit` conforme à LEP.

Canterel peut continuer à fonctionner en mode standalone. En mode Locus, il ne devient jamais la source de vérité globale.

### 2.3 `apps/emacs` possède (dans ce dépôt, ADR 0009)

Le répertoire `apps/emacs/` contient le client Emacs générique et publiable — package Emacs propre (`locusolus-pkg.el`, `README.md`, `CHANGELOG.md`, `tests/`), installable par `:load-path` seul et extractible ultérieurement sans changer son architecture :

- cockpit textuel ;
- commandes et approbations ;
- navigation programmes/branches/reviews/artefacts ;
- stream d’événements ;
- intégration Org/Magit/Jupyter ;
- registre de viewers ;
- WebView intégré pour les visualisations riches et 3D lorsque disponible ;
- fallback navigateur externe.

Il ne contient aucune préférence personnelle et ne réimplémente aucun état canonique.

### 2.4 `xiiif` possède

`xiiif` est un instrument humain spécialisé, principalement Emacs, pour :

- ouvrir et naviguer dans des ressources IIIF ;
- examiner manifests, canvases, annotations, Content States et OCR ;
- sélectionner des régions et créer/corriger des annotations ;
- inspecter des preuves IIIF produites par des agents ;
- ouvrir au besoin Mirador/OpenSeadragon ou une vue web spécialisée.

`xiiif` n’est pas un composant obligatoire de Locus Solus et n’est pas le moteur IIIF des agents. Les workers utilisent des clients IIIF/headless adaptés à leur environnement et produisent des artefacts standards lisibles ensuite par xiiif ou d’autres viewers.

### 2.5 `emacs-config` possède

`emacs-config` reste une configuration personnelle. Il installe et configure `apps/emacs`, `xiiif` et les autres outils de l’utilisateur. Aucun comportement produit essentiel ne doit vivre uniquement dans ce dépôt.

### 2.6 Règle de commande

> Locus Solus décide quoi poursuivre, pourquoi, selon quelle branche, quelle politique, quel budget, quel environnement et quel niveau de validation. Canterel décide comment accomplir localement une mission scientifique dans les capacités qui lui sont accordées.

Les viewers visualisent des projections et artefacts ; ils ne modifient jamais directement la vérité canonique.

## 3. Acteurs et scénarios d’usage

### 3.1 Acteurs humains

- **Owner** : possède une installation ou un espace de travail.
- **Research Director** : crée les programmes, définit les objectifs et délègue les budgets.
- **Researcher** : explore, annote, conteste et propose des objets.
- **Reviewer** : produit une évaluation indépendante.
- **Operator** : administre les services, workers, sauvegardes et secrets.
- **Observer** : consulte les objets autorisés sans mutation.

Une même personne peut cumuler plusieurs rôles. Les responsabilités restent néanmoins distinctes dans les journaux.

### 3.2 Acteurs logiciels

- `locusd` : API et autorité transactionnelle ;
- worker de workflow : exécute les workflows Temporal ;
- worker Canterel : exécute les missions cognitives et instrumentales ;
- worker formel : Lean, SAT, SMT, calcul symbolique ;
- worker de calcul : CPU, GPU ou cluster ;
- worker de conservation : indexation, déduplication, export ;
- clients Emacs, Web et CLI ;
- pairs fédérés ;
- connecteurs externes.

### 3.3 Scénario canonique V1 — mathématiques

Un utilisateur lance depuis Emacs un programme sur une conjecture ouverte. Locus Solus :

1. crée une carte initiale des définitions, résultats connus et barrières ;
2. ouvre plusieurs branches indépendantes ;
3. compose des équipes avec modèles et spécialités distinctes ;
4. commande des missions aux workers Canterel ;
5. conserve les claims, contre-exemples, scripts et formalisations ;
6. déclenche revues aveugles et reproductions ;
7. réalloue le budget selon le gain d’information ;
8. suspend les branches dominées sans effacer leurs résultats négatifs ;
9. fusionne les résultats compatibles ;
10. produit un dossier audit-able et reprend après redémarrage complet.

### 3.4 Scénario canonique V1 — humanités numériques

Un utilisateur étudie un corpus patrimonial :

1. le corpus et ses règles de sélection sont enregistrés ;
2. des agents extraient passages OCR, entités et régions IIIF ;
3. chaque interprétation est ancrée dans les sources ;
4. plusieurs lectures incompatibles peuvent coexister ;
5. les reviewers vérifient la représentativité du corpus, les citations et les ancrages ;
6. le système conserve les controverses plutôt que de forcer un consensus ;
7. l’ensemble est exportable en JSON-LD/RDF avec provenance.

### 3.5 Scénario canonique V1 — recherche empirique

Le système doit également démontrer : pré-enregistrement d’une hypothèse, plan expérimental, réservation de calcul, exécution sandboxée, suivi des données, analyse, figures, critique statistique, reproduction et rapprochement des coûts.

---

## 4. Architecture générale

### 4.1 Principe de portabilité

Locus Solus DOIT conserver le même modèle de domaine et les mêmes contrats qu’il soit déployé :

- entièrement sur un MacBook ;
- sur un Mac mini ou autre nœud personnel ;
- dans une VM Linux ;
- dans un cluster Kubernetes ;
- sur une plateforme cloud disposant de containers/sandboxes/workflows ;
- dans une topologie hybride combinant control plane cloud, workers locaux et workers GPU distants.

Le placement physique est une propriété de configuration. Aucun objet `Project`, `Branch`, `Claim`, `Review`, `Task` ou `Artifact` ne doit dépendre d’un fournisseur d’infrastructure.

### 4.2 Séparation control plane / execution plane / evidence plane / presentation plane

```text
┌──────────────────────────────────────────────────────────────────────┐
│ CONTROL PLANE                                                        │
│ locusd · policies · portfolio · workflows · scheduler · decisions   │
└───────────────────────────────┬──────────────────────────────────────┘
                                │ LEP / commands / events
┌───────────────────────────────▼──────────────────────────────────────┐
│ EXECUTION PLANE                                                      │
│ Canterel · sandbox workers · Lean · browser · CPU/GPU · connectors  │
└───────────────────────────────┬──────────────────────────────────────┘
                                │ artifacts + manifests + attestations
┌───────────────────────────────▼──────────────────────────────────────┐
│ EVIDENCE PLANE                                                       │
│ event store · epistemic graph · CAS/object store · Git · memory     │
└───────────────────────────────┬──────────────────────────────────────┘
                                │ projections + artifacts
┌───────────────────────────────▼──────────────────────────────────────┐
│ PRESENTATION PLANE                                                   │
│ Emacs cockpit · Web workspace · 2D/3D viewers · xiiif · notebooks   │
└──────────────────────────────────────────────────────────────────────┘
```

### 4.3 Ports d’infrastructure obligatoires

Le noyau applicatif dépend d’interfaces, jamais d’un fournisseur concret :

```ts
interface WorkflowBackend {}
interface DomainStore {}
interface EventStore {}
interface ArtifactStore {}
interface ExecutionBackend {}
interface EnvironmentBuilder {}
interface SecretBackend {}
interface IdentityBackend {}
interface RealtimeBackend {}
interface ObservabilityBackend {}
```

Implémentations V1 attendues :

- `TemporalWorkflowBackend` pour local/VM/cluster ;
- `CloudWorkflowBackend` lorsque la plateforme fournit des workflows durables compatibles ;
- PostgreSQL comme stockage canonique de référence ;
- filesystem/S3-compatible/R2 comme backends d’artefacts ;
- Lima/Podman ou équivalent sur macOS ;
- OCI/rootless container sur Linux ;
- backend de sandbox cloud lorsqu’il offre isolation et quotas vérifiables ;
- workers externes LEP pour Modal/RunPod/Kubernetes ou infrastructures similaires.

### 4.4 Composants obligatoires

```text
Locus Solus
├── locusd                     # command/query API et composition root
├── domain kernel
├── event store + projections
├── epistemic graph
├── workflow abstraction
├── scheduler + portfolio
├── policy engine
├── review system
├── memory/context service
├── artifact registry
├── budget ledger
├── worker/LEP gateway
├── execution broker API
├── toolchain/environment registry
├── visualization projection service
├── federation gateway
├── CLI `locus`
└── Web workspace
```

Le broker privilégié d’exécution est un service séparé `locus-execd` lorsque des privilèges élevés ou un socket de runtime sont nécessaires. `locusd` ne doit pas posséder directement un socket Docker/Podman root.

### 4.5 Technologies de référence, non constitutives

- TypeScript pour domaine, services et SDK ;
- Node.js LTS comme runtime serveur de référence ;
- PostgreSQL + `pgvector` ;
- Temporal OSS pour le profil local/VM de référence ;
- S3-compatible object storage ;
- OCI images et signatures par digest ;
- OpenTelemetry ;
- JSON Schema/Zod ;
- HTTP + WebSocket/SSE ;
- JSON-LD/PROV-O/RDF/GraphML pour interopérabilité ;
- Three.js/WebGL/WebGPU pour projections 3D riches, sans dépendance au client Emacs.

Ces choix sont remplaçables sans changer les invariants métier.

## 5. Structure normative du dépôt

```text
locusolus/
├── apps/
│   ├── locusd/                 # daemon et composition root
│   ├── locus-execd/            # execution broker privilégié, séparé
│   ├── cli/                    # CLI `locus`
│   ├── web/                    # cockpit web et viewers riches
│   └── worker-control/         # workers de workflows du control plane
├── packages/
│   ├── domain/
│   ├── protocol/               # LEP, commands, events, schemas, SDK
│   ├── application/
│   ├── event-store/
│   ├── graph/
│   ├── workflows/              # définitions indépendantes du backend
│   ├── workflow-backends/
│   ├── scheduler/
│   ├── portfolio/
│   ├── policies/
│   ├── reviews/
│   ├── memory/
│   ├── artifacts/
│   ├── budgets/
│   ├── identity/
│   ├── execution/              # SandboxSpec, ResourceSpec, attestations
│   ├── environments/           # blueprints, builder, lock manifests
│   ├── toolchains/             # registry et capability taxonomy
│   ├── visualization/          # projections 2D/3D et viewer registry
│   ├── federation/
│   ├── telemetry/
│   └── testing/
├── disciplines/
│   ├── core/
│   ├── mathematics/
│   ├── digital-humanities/
│   ├── biology/
│   ├── physics/
│   └── machine-learning/
├── environments/
│   ├── base/
│   ├── python-science/
│   ├── ml-cpu/
│   ├── math-formal/
│   ├── math-compute/
│   ├── browser/
│   └── dh/
├── schemas/
│   ├── commands/
│   ├── events/
│   ├── lep/
│   ├── artifacts/
│   ├── environments/
│   └── federation/
├── deploy/
│   ├── local/
│   ├── vm/
│   ├── cloudflare/
│   ├── kubernetes/
│   └── observability/
├── docs/
│   ├── architecture/
│   ├── adr/
│   ├── protocols/
│   ├── governance/
│   └── threat-model/
└── tests/
    ├── contract/
    ├── integration/
    ├── replay/
    ├── portability/
    ├── sandbox/
    ├── endurance/
    ├── adversarial/
    └── benchmarks/
```

Le client Emacs publiable vit dans `apps/emacs/` (ADR 0009, révise ce point) ; la configuration personnelle reste dans `emacs-config`. Sa spécification détaillée est `repos/locusolus/apps-emacs/SPEC.md`.

Chaque package DOIT déclarer ses dépendances autorisées et la CI doit vérifier les frontières architecturales.

## 6. Identité, espace de travail et multi-utilisateur

### 6.1 Workspace

Le `Workspace` représente une frontière de données et d’administration.

```yaml
id:
slug:
title:
mode: personal | team | federated
owner_principal_id:
default_visibility:
default_policy_set_id:
artifact_namespace:
retention_policy_id:
created_at:
```

Une installation personnelle possède au moins un workspace. Le mode équipe n’introduit pas un second modèle de données.

### 6.2 Principal

```yaml
id:
kind: human | service | worker | federated_peer
subject:
display_name:
status:
authentication_methods:
created_at:
```

### 6.3 Membership et rôles

Les autorisations combinent :

- rôles héritables au niveau workspace/projet/programme ;
- capabilities ponctuelles et bornées ;
- propriété d’une ressource ;
- politiques de séparation des fonctions.

Une même identité NE DOIT PAS pouvoir générer, revoir et approuver seule un objet classé `major` lorsque la politique exige une séparation.

---

## 7. Modèle de domaine canonique

### 7.1 Agrégats organisationnels

#### Project

Conteneur stable d’un dépôt, corpus, infrastructure ou domaine de travail.

```yaml
id:
workspace_id:
slug:
title:
description:
source_bindings: []
default_policy_set_id:
visibility:
state: active | suspended | archived
revision:
created_at:
updated_at:
```

#### ResearchProgram

```yaml
id:
project_id:
title:
objective:
scope:
root_question_ids: []
success_criteria: []
failure_criteria: []
portfolio_policy_id:
budget_account_id:
authority_policy_id:
state: proposed | active | paused | closing | closed | archived
revision:
created_at:
closed_at:
```

#### Workstream

```yaml
id:
program_id:
title:
objective:
priority:
dependencies: []
team_id:
budget_envelope_id:
state: proposed | active | blocked | paused | completed | abandoned | archived
revision:
```

#### Branch

```yaml
id:
workstream_id:
title:
objective:
forked_from_branch_id:
fork_revision:
head_revision:
team_id:
budget_envelope_id:
review_policy_id:
termination_policy_id:
information_sharing_policy_id:
state: seed | exploring | substantiated | contested | blocked | formalizing | validated | merged | suspended | archived
revision:
```

Invariants :

- une branche possède un seul head canonique ;
- un fork référence exactement la révision d’origine ;
- `merged` est terminal sauf opération explicite `reopen` ;
- une branche ne peut être `validated` si ses conditions de validation ne sont pas satisfaites ;
- `archived` ne supprime aucun objet.

#### Task

```yaml
id:
branch_id:
kind:
objective:
success_contract:
failure_contract:
context_view_id:
agent_requirement:
capability_requirements:
sandbox_profile_id:
review_policy_id:
budget_reservation_id:
priority:
state:
attempt:
idempotency_key:
assigned_worker_id:
assigned_agent_id:
lease_id:
revision:
created_at:
started_at:
completed_at:
```

États :

```text
proposed → queued → leased → running
running → waiting_for_tool | waiting_for_human | waiting_for_review
running → succeeded | failed | cancelled | timed_out
leased/running → orphaned → queued
succeeded → accepted | rejected | superseded
```

Une tâche `succeeded` signifie que le worker a rempli son contrat technique. Elle ne signifie pas que ses claims sont validés.

#### AgentTemplate

```yaml
id:
name:
role:
description:
base_runtime:
prompt_overlay_ref:
required_capabilities:
preferred_model_policy:
tool_policy_id:
sandbox_profile_id:
memory_policy_id:
review_independence_group:
termination_policy_id:
version:
status: active | deprecated | disabled
```

#### AgentInstance

```yaml
id:
template_id:
program_id:
branch_id:
team_id:
worker_id:
model_identity:
context_view_id:
private_memory_id:
budget_envelope_id:
independence_group:
state: provisioned | active | waiting | completed | failed | terminated
created_at:
last_heartbeat_at:
```

L’identité d’un agent comprend le template, sa version, le modèle exact, le fournisseur, les overlays, les outils, le contexte, le worker et les paramètres d’exécution.

#### Team

```yaml
id:
branch_id:
title:
coordination_mode: coordinator | blackboard | debate | independent_pool | pipeline
member_ids: []
coordinator_id:
information_sharing_policy_id:
review_independence_policy_id:
state:
revision:
```

#### Decision

```yaml
id:
target_ids: []
decision_type:
outcome:
rationale:
evidence_refs: []
policy_evaluation_id:
made_by:
overrides: []
state: proposed | approved | rejected | revoked
created_at:
```

#### ApprovalRequest

Objet explicite permettant de suspendre durablement un workflow en attente d’une décision humaine.

```yaml
id:
action:
impact:
requested_by:
required_roles:
expires_at:
state: pending | approved | rejected | expired | cancelled
responses: []
```

### 7.2 Agrégats financiers et ressources

#### BudgetAccount

Le budget est un registre, pas un compteur mutable isolé.

```yaml
id:
currency:
limit_amount:
limit_model_calls:
limit_tokens:
limit_compute_seconds:
limit_wall_time_seconds:
limit_parallelism:
policy_id:
state:
```

Écritures obligatoires : `allocation`, `reservation`, `release`, `consumption`, `adjustment`, `refund`.

Invariants :

- `spent + reserved` ne dépasse pas la limite dure ;
- toute exécution coûteuse possède une réservation ;
- la consommation finale est rapprochée avec les métriques du worker ;
- une correction ne réécrit pas une écriture antérieure ; elle crée un ajustement compensatoire.

### 7.3 Objets épistémiques

Types core obligatoires :

```text
ResearchQuestion  Definition       Assumption        Hypothesis
Conjecture        Claim            Lemma             Theorem
Interpretation    Method           Strategy          Analogy
TransferCertificate Barrier        OpenQuestion      Objection
Counterexample    NegativeResult   Failure           Conflict
Inference         Experiment       Run               Source
Citation          Dataset          Artifact          Figure
Notebook          CodeRevision     FormalStatement   FormalProof
Review            Rebuttal         Reproduction      Decision
CorpusSelection   Measurement      Evaluation        Synthesis
```

Chaque type est défini par un schéma disciplinaire versionné. Les extensions ne doivent pas modifier la signification des types core.

### 7.4 Enveloppe commune d’un objet épistémique

```yaml
stable_id:
revision_id:
object_type:
schema_version:
version:
branch_id:
content:
content_hash:
status:
validation_level:
created_by:
created_at:
supersedes_revision_id:
provenance_refs: []
evidence_refs: []
confidentiality:
policy_tags: []
```

Statuts :

```text
draft | staged | under_review | contested | validated
refuted | superseded | withdrawn | quarantined | archived
```

`status` décrit le cycle de vie. `validation_level` décrit la force épistémique et ne doit pas être déduit du seul statut.

### 7.5 Relations typées

Relations core :

```text
depends_on       implies           supports           refutes
contradicts      generalizes       specializes        formalizes
cites            derived_from      produced_by        consumes
tests            reproduces       fails_because      blocked_by
analogous_to     transfers_to      reviewed_by        responds_to
assigned_to      forked_from       merged_into        supersedes
instantiates     measures          interprets         anchored_in
```

Une relation est un objet versionné avec auteur, scope, force, justification et evidence refs. Les relations non symétriques ne doivent pas être inférées en sens inverse.

### 7.6 Inférences et hyperarêtes

Une inférence est un nœud explicite :

```yaml
id:
inference_kind:
premise_ids: []
conclusion_ids: []
assumption_ids: []
rule:
scope:
formalization_status:
evidence_refs: []
author:
review_status:
```

Le système NE DOIT PAS réduire un raisonnement multi-prémisses à plusieurs arêtes indépendantes. Une objection peut cibler l’inférence, une prémisse, la règle ou son domaine de validité.

### 7.7 Versionnement et identité

- `stable_id` identifie le concept à travers ses versions.
- `revision_id` identifie une version immuable.
- une modification crée une nouvelle révision ;
- une révision possède au plus un prédécesseur direct dans sa lignée ;
- un merge peut créer une révision avec plusieurs parents déclarés ;
- les hashes portent sur une canonicalisation stable ;
- tous les identifiants globaux sont des UUIDv7 ou ULID avec préfixe de type ;
- les timestamps sont en UTC ISO 8601 ;
- la présentation locale des dates n’affecte jamais les signatures ni les hashes.

---
## 8. Modèle de validation épistémique

Locus Solus doit distinguer explicitement existence, plausibilité, soutien empirique, reproduction et preuve formelle.

### 8.1 Niveaux de validation core

```text
L0 UNASSESSED       objet enregistré, non évalué
L1 TRACEABLE        auteur, provenance et sources identifiables
L2 INTERNALLY_CHECKED contrôles de cohérence et de forme passés
L3 INDEPENDENTLY_REVIEWED au moins une revue indépendante satisfaite
L4 REPRODUCED       résultat reproduit depuis les artefacts
L5 FORMALLY_VERIFIED obligation formelle vérifiée, lorsque applicable
L6 INSTITUTIONALLY_ACCEPTED critères du programme et approbations satisfaits
```

Ces niveaux ne forment pas toujours une chaîne totale. Une interprétation historique peut atteindre L3 et L6 sans être « reproduite » au sens expérimental. Les packs disciplinaires définissent les chemins admissibles et la signification de chaque niveau.

### 8.2 Validation par type

Chaque schéma disciplinaire DOIT déclarer :

- les preuves minimales ;
- les revues obligatoires ;
- les contrôles automatisables ;
- les niveaux inapplicables ;
- les conditions de promotion et de rétrogradation ;
- les événements qui invalident les dépendants.

### 8.3 Propagation de l’invalidation

Lorsqu’une définition, une source, un dataset ou une prémisse est réfuté, retiré ou révisé, Locus Solus :

1. identifie les objets transitivement dépendants ;
2. ne les réfute pas automatiquement sans règle disciplinaire ;
3. les marque `needs_reassessment` dans une projection dédiée ;
4. ouvre des tâches de réévaluation selon la politique ;
5. conserve le niveau et la justification antérieurs dans l’historique.

### 8.4 Confiance et vérité

Les scores de confiance des agents sont des métadonnées de calibration. Ils ne remplacent ni les preuves, ni les revues, ni les niveaux de validation. Une moyenne de confiance ne constitue jamais une procédure de décision par défaut.

---

## 9. Graphe épistémique et projections

### 9.1 Source de vérité

PostgreSQL est la source de vérité transactionnelle pour :

- événements de domaine ;
- révisions d’objets et relations ;
- têtes d’agrégats ;
- registres de budget ;
- permissions et décisions ;
- métadonnées d’artefacts ;
- état des imports et exports.

Les vecteurs, index plein texte, vues matérialisées, graph databases et caches sont des projections reconstruisibles.

### 9.2 Modèle d’écriture

L’écriture suit une architecture command/event :

```text
Command
  → authorization
  → load aggregate revision
  → validate invariants
  → append domain events
  → update aggregate head/snapshot
  → transactional outbox
  → async projections
```

Le système DOIT garantir l’atomicité entre l’ajout des événements, la révision de l’agrégat et l’outbox. Les projections secondaires peuvent être légèrement retardées, mais leur watermark est visible.

### 9.3 Modèle de lecture

Projections obligatoires :

- graphe argumentatif ;
- graphe de provenance ;
- graphe des tâches et workflows ;
- graphe des équipes et agents ;
- graphe des citations ;
- graphe des transferts interdisciplinaires ;
- graphe des dépendances formelles ;
- registre des conflits ;
- timeline des décisions ;
- état de validation ;
- consommation de budget ;
- index mémoire hybride.

### 9.4 Requêtes minimales

Locus Solus DOIT supporter :

- sous-graphe d’une branche à une révision donnée ;
- chemin de dépendance entre deux objets ;
- prémisses minimales d’un claim ;
- claims contestés ou sans revue ;
- descendants affectés par une révision ;
- résultats négatifs par méthode, paramètres et scope ;
- branches bloquées par une même barrière ;
- objets équivalents ou quasi-dupliqués ;
- preuves et sources d’une phrase donnée ;
- artefacts et runs d’une conclusion ;
- conflits non résolus ;
- objets formalisables ;
- vue temporelle « telle qu’elle était connue à la date T » ;
- comparaison structurelle de deux branches.

### 9.5 Cohérence et réparation

- Chaque projection expose son dernier `event_sequence` appliqué.
- Un outil `canterel projections verify` compare événements et projections.
- Une projection peut être détruite et reconstruite.
- Les erreurs de projection sont mises en quarantaine sans bloquer l’écriture canonique, sauf si elles concernent une projection synchrone nécessaire à un invariant.
- Des checksums de segments détectent la corruption silencieuse.

### 9.6 Performance cible

Sur une machine de développement raisonnable, la V1 vise :

- 250 000 objets et relations core sans changement d’architecture ;
- ouverture d’une branche de 5 000 objets en moins de 2 secondes pour la requête serveur, hors rendu graphique ;
- recherche textuelle P95 inférieure à 750 ms sur le corpus de référence ;
- traversée bornée à trois niveaux P95 inférieure à 1 seconde ;
- ingestion soutenue d’au moins 100 événements/s sans perte ;
- reconstruction complète mesurée et documentée.

Ces valeurs sont des SLO de référence, non des garanties universelles pour les corpus massifs.

---

## 10. Journal d’événements

### 10.1 Enveloppe normative

```json
{
  "event_id": "evt_01...",
  "event_type": "epistemic_object.staged",
  "schema_version": 1,
  "stream_id": "claim_01...",
  "stream_revision": 7,
  "workspace_id": "ws_01...",
  "project_id": "prj_01...",
  "program_id": "pgm_01...",
  "branch_id": "br_01...",
  "actor": {
    "principal_id": "agent_01...",
    "kind": "agent",
    "delegation_id": "del_01..."
  },
  "occurred_at": "2026-07-26T12:00:00.000Z",
  "recorded_at": "2026-07-26T12:00:00.112Z",
  "causation_id": "cmd_01...",
  "correlation_id": "wf_01...",
  "trace_id": "...",
  "payload": {},
  "payload_hash": "sha256:..."
}
```

### 10.2 Garanties

- ordre total par stream ;
- optimistic concurrency par `expected_stream_revision` ;
- idempotence par commande ;
- immutabilité logique ;
- signature facultative locale et obligatoire en fédération ;
- schéma versionné ;
- migration upcaster pour les consommateurs ;
- export brut ;
- snapshots reconstruisibles ;
- rétention illimitée par défaut pour les événements canoniques, sauf politique légale explicite.

### 10.3 Taxonomie d’événements

```text
workspace.* project.* program.* workstream.* branch.*
task.* agent.* team.* worker.* lease.*
budget.* policy.* approval.* decision.*
epistemic_object.* relation.* inference.* conflict.*
artifact.* run.* reproduction.* review.* rebuttal.*
memory.* context_view.* workflow.* federation.* security.*
```

### 10.4 Évolution de schéma

- Aucun consommateur ne doit dépendre d’un champ non documenté.
- Les changements incompatibles créent une nouvelle version de message.
- Les producteurs supportent au minimum la version courante et la version précédente de LEP pendant une fenêtre de migration.
- Les tests de replay utilisent des historiques réels anonymisés.

---

## 11. Orchestration durable

### 11.1 Abstraction du moteur de workflow

Locus Solus définit un `WorkflowBackend` et ne code aucun invariant métier directement contre Temporal, Cloudflare Workflows ou un autre fournisseur.

```ts
interface WorkflowBackend {
  start(definition: WorkflowDefinition): Promise<WorkflowHandle>
  signal(id: string, signal: WorkflowSignal): Promise<void>
  suspend(id: string): Promise<void>
  resume(id: string): Promise<void>
  terminate(id: string, reason: string): Promise<void>
  inspect(id: string): Promise<WorkflowState>
}
```

Implémentations V1 :

- `TemporalWorkflowBackend` : profil local, Mac mini, VM Linux, cluster ;
- `CloudWorkflowBackend` : profil cloud lorsque la plateforme offre durabilité, timers, retries et signaux ;
- `DeterministicTestWorkflowBackend` : tests et simulation.

Le stockage scientifique canonique demeure PostgreSQL/event store. Le moteur de workflow ne remplace ni le graphe ni l’histoire scientifique.

### 11.2 Workflows obligatoires

Les workflows V1 sont : `ProgramWorkflow`, `WorkstreamWorkflow`, `BranchWorkflow`, `TaskWorkflow`, `ReviewWorkflow`, `ReproductionWorkflow`, `MemoryCurationWorkflow`, `PortfolioWorkflow`, `EnvironmentBuildWorkflow`, `SandboxLifecycleWorkflow` et `FederationWorkflow`.

Chaque workflow métier doit pouvoir être rejoué ou repris avec un autre backend sans changer l’identité des objets scientifiques.

### 11.3 Règles de déterminisme et side effects

- aucun appel LLM, réseau, filesystem ou horloge non encapsulé dans une activity/step ;
- IDs métier créés avant l’entrée dans le backend de workflow ;
- side effects idempotents ;
- versions de workflow explicites ;
- tests de replay pour les versions supportées ;
- migrations contrôlées des workflows longue durée.

### 11.4 Compensation

Les compensations annulent les réservations techniques, leases, fichiers temporaires et ressources cloud. Elles ne réécrivent jamais l’histoire épistémique : un résultat produit puis invalidé reste traçable.

### 11.5 Dégradation

Si aucun moteur durable n’est disponible, Locus Solus peut offrir un mode `single-process-dev` explicitement non conforme aux garanties de campagne durable. Ce mode ne doit jamais être présenté comme équivalent au profil V1 de production.

## 12. Scheduler, queues et placement

### 12.1 Classes de tâches

```text
interactive          latence faible, humain présent
research-general     missions Canterel généralistes
review-isolated      contexte aveugle et lecture seule
formal               Lean/SAT/SMT
compute-cpu          calcul CPU borné
compute-gpu          calcul GPU
untrusted            micro-VM ou conteneur renforcé
curation             index, déduplication, export
```

### 12.2 Placement

Le scheduler prend en compte :

- capabilities requises ;
- OS et architecture ;
- sandbox disponible et attestée ;
- modèles et credentials ;
- mémoire, CPU, GPU et espace disque ;
- localisation et confidentialité des données ;
- coût estimé ;
- limites de parallélisme ;
- indépendance requise ;
- affinité ou anti-affinité ;
- santé et historique du worker.

### 12.3 Lease et reprise

- une lease possède une expiration courte et renouvelable ;
- le worker envoie un heartbeat à intervalle inférieur au tiers du TTL ;
- l’expiration produit `task.orphaned` ;
- une tâche réattribuée conserve le numéro d’attempt ;
- un résultat tardif est stocké en quarantaine et ne peut committer sans arbitrage ;
- les side effects utilisent des clés d’idempotence indépendantes de l’attempt ;
- la cancellation est coopérative puis forcée après un délai.

### 12.4 Backpressure

Le worker gateway et le scheduler DOIVENT :

- limiter les événements de progression ;
- supporter acknowledgement et reprise de stream ;
- refuser les nouvelles leases si le worker dépasse ses ressources ;
- distinguer saturation temporaire et incapacité structurelle ;
- protéger `locusd` contre un worker bavard ou malveillant.

---

## 13. Orchestrateur de portefeuille

### 13.1 Objectif

Le portefeuille cherche à maximiser le progrès scientifique attendu sous contraintes, sans réduire la recherche à une exploitation myope d’une seule piste.

### 13.2 Indicateurs par branche

- progression épistémique ;
- gain d’information ;
- qualité et indépendance des preuves ;
- nombre de dépendances fragiles ;
- contradictions résolues ;
- nouveauté ;
- réutilisabilité ;
- valeur des résultats négatifs ;
- coût consommé et coût marginal ;
- vélocité ;
- redondance sémantique et méthodologique ;
- corrélation d’erreur attendue ;
- calibration des agents ;
- risque de verrouillage conceptuel ;
- couverture de l’espace des stratégies.

### 13.3 Qualité-diversité

La V1 NE DOIT PAS sélectionner uniquement les branches au score moyen le plus élevé. Elle maintient :

- une part d’exploitation ;
- une réserve exploratoire ;
- des niches méthodologiques ;
- au moins une branche de falsification pour toute hypothèse majeure ;
- une pénalité de corrélation ;
- une prime aux résultats négatifs informatifs ;
- une limite de concentration par famille de modèle et méthode.

### 13.4 Fonction de valeur de référence

\[
V(b)=p_s I+\lambda G+\mu R+\nu D+\xi N-\alpha C-\beta S-\gamma\rho-\delta F
\]

- \(p_s\) : probabilité calibrée de progrès ;
- \(I\) : impact ;
- \(G\) : gain d’information ;
- \(R\) : réutilisabilité ;
- \(D\) : diversité ;
- \(N\) : valeur du négatif attendu ;
- \(C\) : coût marginal ;
- \(S\) : similarité avec le portefeuille ;
- \(\rho\) : corrélation d’erreur ;
- \(F\) : fragilité des dépendances.

Cette formule est une politique par défaut, non une vérité scientifique. Tous ses paramètres, entrées, incertitudes et overrides sont enregistrés.

### 13.5 Actions

- créer/forker une branche ;
- composer ou renforcer une équipe ;
- imposer une attaque contradictoire ;
- créer un agent-pont ;
- ouvrir une méta-branche ;
- réduire ou augmenter un budget ;
- suspendre ou archiver ;
- commander une reproduction ;
- réviser une définition ;
- escalader vers un humain.

Les actions dépassant les seuils de coût, de confidentialité ou d’impact exigent une `ApprovalRequest`.

### 13.6 Anti-gaming

Le système doit détecter et pénaliser :

- multiplication artificielle de claims triviaux ;
- inflation de confiance ;
- duplications paraphrastiques ;
- production de tâches pour maximiser l’activité ;
- collusion de reviewers ;
- fragmentation artificielle d’artefacts ;
- sélection opportuniste de métriques.

---

## 14. Société d’agents

### 14.1 Rôles V1

**Gouvernance** : Governor, PortfolioOrchestrator, BranchCoordinator, BudgetManager, GraphCurator, HumanLiaison.  
**Exploration** : DomainCartographer, HypothesisGenerator, StrategyExplorer, CrossDisciplinaryBridge, DefinitionReviser, AnalogyAuditor.  
**Spécialisation** : DomainSpecialist, MethodSpecialist, BibliographicVerifier, ComputationAgent, ExperimentAgent, Formalizer, CounterexampleHunter, Reproducer.  
**Adversarialité** : LogicalReviewer, MethodologicalReviewer, StatisticalReviewer, CitationReviewer, BarrierReviewer, ReproducibilityAuditor, RedTeam, MetaReviewer.  
**Mémoire** : Deduplicator, EntityResolver, NegativeResultArchivist, SynthesisCompiler, TemporalCurator.

Ces noms désignent des rôles et contrats. Ils ne supposent pas un modèle ou fournisseur particulier.

### 14.2 Template, instance, équipe

- `AgentTemplate` définit le rôle et les contraintes.
- `AgentInstance` est une exécution située, traçable et temporaire.
- `Team` définit coordination et partage d’information.

Une instance n’hérite jamais tacitement des permissions du modèle ou du worker. Les capacités effectives sont l’intersection de la mission, du template, de la politique locale et de l’attestation du worker.

### 14.3 Coordination

Modes obligatoires :

- `coordinator` : un agent distribue et synthétise ;
- `blackboard` : contributions sur une mémoire de branche partagée ;
- `debate` : positions et objections structurées ;
- `independent_pool` : aucun partage avant remise ;
- `pipeline` : sorties typées enchaînées.

Le mode est enregistré et peut être comparé dans les benchmarks.

### 14.4 Indépendance

Une politique d’indépendance peut imposer :

- familles de modèles distinctes ;
- fournisseurs distincts ;
- contextes séparés ;
- absence du transcript de génération ;
- corpus ou ordres de recherche différents ;
- outils différents ;
- randomisation ;
- anonymisation ;
- workers distincts ;
- interdiction de mémoire partagée.

### 14.5 Spawn dynamique

Déclencheurs :

```text
domain_gap_detected review_disagreement barrier_encountered
branch_stagnation formalization_blocked counterexample_needed
new_method_found bridge_candidate high_uncertainty
reproduction_failure source_conflict
```

Une proposition de spawn comprend :

```yaml
reason:
missing_capability:
expected_information_gain:
diversity_contribution:
cost_estimate:
time_to_live:
termination_condition:
context_policy:
review_policy:
```

Le moteur de politique peut accepter, refuser, modifier ou soumettre à approbation. Aucun agent ne crée librement une flotte non bornée.

### 14.6 Réputation et calibration

La réputation est multidimensionnelle, temporelle et contextuelle :

- exactitude confirmée ;
- taux de claims réfutés ;
- qualité des citations ;
- reproductibilité ;
- détection de défauts ;
- calibration de confiance ;
- coût par contribution validée ;
- spécialité ;
- corrélation avec d’autres agents.

Elle ne donne jamais un droit à la vérité et ne doit pas devenir un score social unique.

---

## 15. Locus Execution Protocol — LEP v1

### 15.1 Objet

LEP est le contrat générique entre Locus Solus et tout exécuteur : Canterel, worker Lean, browser worker, sandbox CPU, GPU worker ou service externe. Le protocole ne présume ni LLM ni langage de programmation.

### 15.2 Transport

Transport de référence : WebSocket authentifié pour contrôle/événements et HTTP/object storage pour artefacts volumineux. Un mode pull/queue peut être fourni pour les plateformes serverless. Toutes les enveloppes portent version de protocole, sequence, correlation IDs et idempotency key.

### 15.3 Handshake et capacités

Un worker annonce un `CapabilityManifest` signé ou attesté comprenant :

- types de mission ;
- OS/architecture ;
- toolchains disponibles et versions ;
- modèles/fournisseurs autorisés ;
- CPU, RAM, disque et accélérateurs ;
- backends de sandbox ;
- politiques réseau ;
- classes de données admissibles ;
- niveau de confiance et attestations ;
- concurrence maximale.

### 15.4 MissionEnvelope

Une mission spécifie au minimum : objectif, critères de succès/échec, ContextView immuable, outils/capacités requis, `EnvironmentBlueprint`, `SandboxSpec`, `ResourceSpec`, budget, politique de revue et contrat de sortie.

### 15.5 Leases et attempts

Une tâche durable peut connaître plusieurs attempts. Un attempt ne produit jamais directement un état canonique : il soumet artefacts et `EpistemicCommit`. Les résultats tardifs sont conservés, comparés et explicitement acceptés ou rejetés.

### 15.6 Événements

Événements minimaux : `worker.registered`, `task.offered`, `task.accepted`, `attempt.started`, `heartbeat`, `progress`, `tool.started`, `tool.completed`, `artifact.declared`, `artifact.uploaded`, `resource.sampled`, `human.input.requested`, `attempt.completed`, `attempt.failed`, `attempt.orphaned`, `epistemic_commit.submitted`.

### 15.7 EpistemicCommit

Le commit peut proposer claims, objections, inférences, décisions locales, résultats négatifs, limitations, références d’artefacts et prochaines actions. Il n’a aucune autorité de validation avant traitement par Locus Solus.

### 15.8 SDK et conformance

Le dépôt fournit SDK TypeScript et schémas JSON. Tout autre SDK doit passer le même suite de contract tests. LEP est versionné indépendamment du daemon et de Canterel.

## 16. Contexte, mémoire et retrieval

### 16.1 Niveaux de mémoire

- mémoire privée d’agent ;
- mémoire d’équipe ;
- mémoire de branche ;
- mémoire de workstream ;
- mémoire de programme ;
- mémoire inter-programmes ;
- mémoire disciplinaire.

Le graphe, les événements et les artefacts sont canoniques. Les résumés et embeddings sont des projections régénérables.

### 16.2 ContextView

```yaml
id:
query:
root_ids: []
included_types: []
included_relations: []
max_depth:
time_range:
branch_scope:
validation_levels: []
confidentiality_ceiling:
artifact_policy:
negative_result_policy:
diversity_policy:
token_budget:
redactions: []
source_event_watermark:
content_hash:
generated_at:
```

Une `ContextView` est immuable, adressée par hash et rattachée à l’exécution. Elle permet de savoir exactement ce que l’agent pouvait connaître.

### 16.3 Retrieval hybride

Le moteur combine :

- traversée de graphe ;
- recherche lexicale ;
- recherche vectorielle ;
- identifiants exacts, citations et formules ;
- temporalité ;
- niveau de validation ;
- branche et confidentialité ;
- diversité des sources ;
- résultats négatifs ;
- budget de contexte.

Le ranking DOIT exposer ses facteurs. Les embeddings ne peuvent pas contourner les ACL.

### 16.4 Déduplication et résolution d’entités

- détection de duplicatas exacts par hash ;
- candidats sémantiques non fusionnés automatiquement ;
- résolution explicite avec confiance et provenance ;
- alias et identifiants externes ;
- possibilité de « mêmes mots, concepts différents » ;
- fusion réversible par nouvelle décision.

### 16.5 Compaction

Une compaction :

- conserve les identifiants et pointeurs de preuve ;
- distingue faits, hypothèses, décisions et questions ;
- signale ce qui a été omis ;
- possède une provenance et un watermark ;
- peut être régénérée ;
- ne transforme pas un objet non validé en connaissance établie.

### 16.6 Prévention de contamination

Locus Solus doit pouvoir empêcher :

- partage du raisonnement du générateur avec un reviewer aveugle ;
- propagation d’un claim réfuté comme contexte par défaut ;
- réutilisation d’une donnée confidentielle dans un modèle ou worker non autorisé ;
- consensus circulaire où des agents se citent mutuellement sans source externe ;
- oubli des contradictions lors de la synthèse.

---
## 17. Système de revue, rebuttal et méta-revue

### 17.1 Principe

La revue est un protocole d’évaluation, non un agent unique. Elle doit rendre explicites : le dossier consulté, ce qui a été exclu, les questions posées, l’identité ou l’indépendance du reviewer, les findings, les réponses et la décision finale.

### 17.2 Types de revue core

- logique ;
- bibliographique ;
- méthodologique ;
- statistique ;
- provenance ;
- reproductibilité ;
- formalisation ;
- alignement informel/formel ;
- analogie et transfert ;
- barrière ;
- sécurité ;
- éthique et conformité ;
- représentativité du corpus.

### 17.3 ReviewDossier

```yaml
id:
target_revision_ids: []
statements: []
artifacts: []
citations: []
provenance_view_id:
formal_objects: []
review_questions: []
excluded_context: []
blindness_policy:
independence_requirements:
severity_schema:
content_hash:
frozen_at:
```

Le dossier est figé avant attribution. Toute modification entraîne une nouvelle version ou un addendum explicitement visible.

### 17.4 Review

```yaml
id:
dossier_id:
reviewer_agent_id:
reviewer_principal_id:
reviewer_context_view_id:
independence_attestation:
findings: []
verified_items: []
coverage:
limitations:
overall_recommendation:
confidence:
created_at:
```

### 17.5 Finding

```yaml
id:
target_revision_id:
claim_or_item:
issue_type:
severity: blocking | major | minor | info
verdict: supports | refutes | insufficient | not_applicable
evidence_refs: []
reproduction_refs: []
recommended_action:
confidence:
```

Un finding sans preuve concrète est un commentaire non bloquant et ne peut à lui seul changer un niveau de validation.

### 17.6 Rebuttal

```yaml
id:
finding_id:
response:
accepted_parts: []
contested_parts: []
correction_revision_ids: []
counter_evidence_refs: []
requested_recheck:
created_by:
```

Le reviewer initial peut effectuer un recheck. La politique peut imposer un nouveau reviewer pour éviter l’auto-justification.

### 17.7 Méta-revue

La méta-revue :

- compare les prémisses et scopes des reviewers ;
- distingue absence de preuve, contradiction et réfutation ;
- mesure l’indépendance effective ;
- détecte les findings corrélés ou copiés ;
- hiérarchise les blocages ;
- recommande `validate`, `revise`, `contest`, `reject`, `reproduce` ou `human_escalation` ;
- ne masque jamais les opinions minoritaires.

### 17.8 Calibration

Pour chaque rôle/reviewer :

- précision et rappel sur défauts confirmés ;
- faux positifs et faux négatifs ;
- accord avec vérificateurs formels ou reproductions ;
- qualité des preuves citées ;
- couverture réelle du dossier ;
- coût et délai ;
- corrélation avec autres reviewers ;
- dérive dans le temps et selon le domaine.

Les métriques de calibration sont conservées avec leurs intervalles d’incertitude et leur population de référence.

---

## 18. Branches, forks et fusions

### 18.1 Opérations obligatoires

```text
fork merge rebase cherry_pick compare suspend resume
archive reopen abandon
```

Ces opérations portent sur le graphe et les responsabilités institutionnelles, pas uniquement sur Git.

### 18.2 Fork

Un fork :

1. capture la révision source ;
2. définit les objets visibles ou copiés logiquement ;
3. crée une nouvelle politique de partage ;
4. réserve éventuellement un budget ;
5. assigne une équipe ;
6. enregistre la motivation et les critères de divergence.

### 18.3 MergeProposal

```yaml
id:
source_branch_ids: []
target_branch_id:
base_revisions: []
object_changes: []
relation_changes: []
conflicts: []
review_policy_id:
proposed_by:
state: proposed | reviewing | approved | rejected | applied
```

### 18.4 Merge

Une fusion DOIT :

1. identifier objets identiques et lignées communes ;
2. détecter les modifications concurrentes ;
3. conserver les claims incompatibles ;
4. ne jamais fusionner automatiquement deux concepts sur seule similarité vectorielle ;
5. fusionner les artefacts par hash ;
6. recalculer les dépendances et impacts ;
7. créer des `Conflict` explicites ;
8. déclencher la revue de fusion ;
9. créer une `Decision` ;
10. publier une nouvelle révision de branche.

### 18.5 Rebase

Le rebase réévalue une branche sur de nouvelles prémisses. Il ne réécrit pas l’histoire. Les objets inchangés peuvent être réutilisés par référence ; les conclusions affectées sont marquées pour réévaluation.

### 18.6 Cherry-pick

Permet d’importer une méthode, une définition, un résultat négatif ou un claim validé avec sa provenance. Les politiques de licence, confidentialité et dépendance sont vérifiées avant application.

### 18.7 Résultats négatifs

Un `NegativeResult` contient :

```yaml
question_or_hypothesis:
method:
parameters:
search_space:
conditions:
outcome:
statistical_or_formal_power:
known_limitations:
failure_mode:
artifacts: []
applicability_scope:
```

Le système doit pouvoir répondre : « cette attaque a-t-elle déjà été tentée, dans quelles conditions, et qu’est-ce que son échec exclut réellement ? »

---

## 19. Artefacts, environnements, toolchains et reproductibilité

### 19.1 Artefact-first

Tout résultat important est externalisé comme artefact portable plutôt que prisonnier d’un transcript ou d’un viewer. Texte, tables, figures, notebooks, graphes, Content States IIIF, modèles, preuves Lean et scènes 3D doivent être adressables hors de leur outil d’origine.

### 19.2 ArtifactManifest

Chaque artefact possède : hash de contenu, media type, taille, créateur/attempt, provenance, classification, licence/droits si connus, relations de dérivation, viewer hints, intégrité et état de quarantaine/promotion.

### 19.3 EnvironmentBlueprint

Un environnement déclare : OS/arch, toolchain profiles, images par digest, lockfiles, variables non secrètes, ressources minimales/préférées, réseau, mounts, health checks et exigences d’accélérateur.

### 19.4 Toolchain Registry

La V1 fournit des profils composables :

- `base` : git, curl, jq, ripgrep, build tools, ffmpeg, pandoc ;
- `python-science` : Python/uv, NumPy, SciPy, pandas/Polars, DuckDB, PyArrow, SymPy, scikit-learn, statsmodels, matplotlib, Jupyter ;
- `ml-cpu` : PyTorch CPU, torchvision, transformers, datasets, sentence-transformers, ONNX Runtime, safetensors, llama.cpp ;
- `ml-mps` : capability macOS native avec PyTorch MPS/MLX/Metal, non image Linux portable ;
- `ml-cuda` : PyTorch CUDA, CUDA/cuDNN, vLLM/ONNX GPU selon image ;
- `math-formal` : Lean 4 via elan, lake, mathlib, Z3, cvc5 et theorem provers sélectionnés ;
- `math-compute` : SageMath, GAP, PARI/GP, Singular/Macaulay2 selon disponibilité ;
- `browser` : Chromium/Firefox, Playwright, PDF/screenshot tooling ;
- `dh` : IIIF clients, ALTO/PageXML/TEI, XML sûr, RDF/SPARQL, Tesseract/OpenCV ;
- `r`, `julia`, `gis`, `security` comme profils complémentaires.

Les versions sont verrouillées. On n’installe pas tout dans une image universelle.

### 19.5 Environment Builder

Les extensions de dépendances demandées par un agent déclenchent un build séparé avec réseau autorisé, lockfile, SBOM, scan, tests, signature et publication par digest. Une mission standard ne peut pas `curl | bash` ni installer arbitrairement des packages avec privilèges.

### 19.6 RunManifest

Un run consigne image digest, toolchains, code revision, inputs, ressources réservées et observées, variables pertinentes, commandes, seeds, sorties et attestations de sandbox.

### 19.7 Niveaux de reproductibilité

- R0 : narration uniquement ;
- R1 : inputs et code identifiés ;
- R2 : environnement verrouillé ;
- R3 : reproduction automatisée sur backend compatible ;
- R4 : reproduction indépendante sur worker distinct avec comparaison structurée.

Les claims majeurs ne peuvent atteindre un niveau de validation élevé si la politique exige une reproduction supérieure à celle disponible.

## 20. Policy engine et gouvernance humaine

### 20.1 Catégories de politiques

- spawn ;
- model routing ;
- coordination d’équipe ;
- information sharing ;
- budget ;
- scheduling ;
- sandbox et réseau ;
- secrets ;
- revue ;
- validation ;
- branche et terminaison ;
- publication ;
- rétention ;
- fédération ;
- conformité disciplinaire ;
- escalade humaine.

### 20.2 Propriétés du moteur

Le moteur DOIT :

- employer une DSL déclarative versionnée ;
- séparer faits d’entrée et décision ;
- produire une trace d’évaluation ;
- supporter `allow`, `deny`, `modify`, `require_approval`, `require_tasks` ;
- détecter les conflits de politiques ;
- définir une priorité explicite ;
- supporter dry-run et simulation ;
- être déterministe à entrées identiques ;
- conserver les overrides humains.

### 20.3 Exemple

```yaml
id: policy_major_claim_review_v3
when:
  object_type: [Claim, Theorem, Interpretation]
  impact: high
require:
  reviewers:
    - role: logical-reviewer
      count: 2
      distinct_model_family: true
    - role: provenance-reviewer
      count: 1
  reproduction:
    minimum_level: R3
decision:
  human_required_on_blocking_or_disagreement: true
  forbid_self_approval: true
```

### 20.4 Autorité et délégation

Une `Delegation` contient :

```yaml
delegator:
delegate:
actions: []
scope:
budget_ceiling:
confidentiality_ceiling:
valid_from:
expires_at:
revocable:
```

Les actions d’un agent sont attribuées au principal agentique et à la délégation humaine ou institutionnelle qui les autorise.

### 20.5 Explicabilité

Toute décision automatisée expose :

- politique et version ;
- données d’entrée ;
- règles déclenchées ;
- scores et incertitudes ;
- alternatives rejetées ;
- approbations ;
- overrides ;
- lien avec les événements produits.

---

## 21. Sécurité, confidentialité et confiance zéro

### 21.1 Modèle de menace

Locus Solus doit considérer comme potentiellement hostiles :

- code généré ;
- dépôts et dépendances externes ;
- documents et pages Web contenant des injections de prompt ;
- workers compromis ;
- modèles distants ;
- plugins ;
- pairs fédérés ;
- artefacts malveillants ;
- utilisateurs disposant de droits partiels ;
- erreurs de configuration ;
- fuites inter-branches et inter-projets.

### 21.2 Classification des données

```text
public | internal | confidential | restricted
```

Chaque objet, artefact et ContextView possède une classification. Les missions ne peuvent abaisser la classification. Les modèles et outils déclarent le plafond qu’ils peuvent traiter.

### 21.3 Authentification

- identité locale sécurisée pour mode personnel ;
- OIDC/WebAuthn possible en équipe ;
- identité de service pour daemons ;
- certificats ou tokens courts pour workers ;
- mTLS obligatoire hors loopback selon le profil ;
- rotation et révocation ;
- aucune clé permanente dans une mission ou un transcript.

### 21.4 Autorisation

RBAC + ABAC + capability tokens. Exemples :

```text
workspace.admin project.read branch.write task.execute
artifact.upload review.submit decision.approve policy.edit
secret.request federation.import security.override
```

Les contrôles s’appliquent au command side, query side, stockage d’artefacts, retrieval et événements temps réel.

### 21.5 Secret broker

- secrets stockés hors base de domaine ;
- délivrance de credentials courts ;
- liaison à task, worker, capability, domaine, quota et durée ;
- injection sans exposition au modèle lorsque possible ;
- redaction des logs ;
- journal d’accès ;
- révocation à la fin de la lease ;
- interdiction d’inclure un secret dans un `EpistemicCommit`.

### 21.6 Sandbox et isolation

Niveaux :

```text
S0 unsandboxed-explicit
S1 os-write-contained
S2 container-rootless
S3 container-isolated-network
S4 microvm-high-risk
S5 remote-trusted-enclave-or-equivalent
```

Profils V1 :

```text
interactive-local readonly-review network-allowlisted
math-compute dh-corpus untrusted-repository microvm-high-risk
```

La mission impose un niveau minimal. Le worker atteste le niveau réellement appliqué. Un downgrade est interdit sauf approbation explicite et événement de sécurité.

### 21.7 Egress et prompt injection

- modes réseau : `deny`, `connector_only`, `allowlist`, `full` ;
- proxy d’egress pour allowlist et audit ;
- séparation entre contenu récupéré et instructions système ;
- marquage des données non fiables ;
- outils Web limités par domaine et méthode ;
- les instructions contenues dans une source ne modifient jamais la politique de mission ;
- revue obligatoire avant promotion d’un résultat provenant de données non fiables à fort impact.

### 21.8 Supply chain

- lockfiles obligatoires ;
- images par digest ;
- SBOM ;
- signatures lorsque disponibles ;
- scan de dépendances et artefacts ;
- allowlist des plugins de confiance ;
- provenance de build ;
- CI séparée pour code non fiable ;
- politique de mise à jour et de révocation.

### 21.9 Audit de sécurité

Les événements de sécurité sont append-only et séparés des logs applicatifs ordinaires. Ils contiennent l’acteur, le scope, la décision de politique et les preuves techniques, sans enregistrer les secrets.

---

## 22. API et contrats de `locusd`

### 22.1 Styles d’interface

- commandes : JSON-RPC 2.0 ou endpoints command typés ;
- queries : REST typé ;
- événements clients : WebSocket/SSE avec cursor ;
- workers : LEP ;
- artefacts : HTTP signé ;
- administration : API distincte et scopes renforcés ;
- export : JSON, JSON-LD, RDF, PROV-O, GraphML et bundle Locus Solus.

### 22.2 CommandEnvelope

```json
{
  "command_id": "cmd_...",
  "command_type": "branch.fork",
  "schema_version": 1,
  "workspace_id": "ws_...",
  "actor_principal_id": "usr_...",
  "delegation_id": null,
  "idempotency_key": "...",
  "expected_revision": 18,
  "correlation_id": "...",
  "payload": {}
}
```

### 22.3 Commandes essentielles

```text
workspace.create project.create program.create program.pause
workstream.create branch.create branch.fork branch.merge.propose
branch.merge.apply branch.rebase branch.suspend branch.resume
task.create task.cancel task.retry agent.spawn agent.terminate
team.create team.modify review.request review.submit rebuttal.submit
decision.propose decision.approve approval.respond
policy.create policy.apply budget.allocate budget.reserve
artifact.register reproduction.request graph.object.stage
graph.object.revise graph.relation.stage conflict.resolve
federation.peer.add federation.bundle.import federation.bundle.export
```

### 22.4 Queries essentielles

```text
GET /workspaces
GET /projects/:id
GET /programs/:id
GET /programs/:id/portfolio
GET /branches/:id
GET /branches/:id/graph
GET /branches/:id/diff
GET /tasks/:id
GET /agents/:id
GET /teams/:id
GET /reviews/:id
GET /epistemic-objects/:revision_id
POST /graph/query
POST /memory/retrieve
GET /timeline
GET /budgets/:id/ledger
GET /workers
GET /workflows/:domain_object_id
GET /projections/status
```

### 22.5 Concurrence et idempotence

- toute commande mutante accepte `expected_revision` ;
- un conflit retourne l’état courant et un code de conflit structuré ;
- les idempotency keys sont scoped et expirent selon la catégorie ;
- les clients peuvent resoumettre sans dupliquer l’effet ;
- les batch commands sont atomiques uniquement si explicitement déclarées ;
- les erreurs sont typées : validation, authorization, conflict, unavailable, budget, policy, security, internal.

### 22.6 Pagination et cursors

Toutes les collections utilisent des cursors opaques, stables dans une fenêtre cohérente. Les événements et timelines supportent reprise depuis une séquence connue.

### 22.7 Compatibilité

- version de protocole dans chaque message ;
- OpenAPI/JSON Schema publiés ;
- SDK générés ;
- tests consumer-driven ;
- politique de dépréciation documentée ;
- aucune rupture silencieuse entre Locus Solus et Canterel.

---

## 23. Interfaces utilisateur et visualisation

### 23.1 Emacs

Le client `apps/emacs` (dans ce dépôt) est le cockpit expert : commandes, navigation, décisions, reviews, graphes opératoires, artefacts, budgets, sandbox inspection et intégration Org/Magit/Jupyter. Emacs n’est pas le moteur de rendu universel.

### 23.2 Web workspace

Le workspace Web fournit les surfaces où un navigateur est supérieur : graphes massifs, dashboards, timelines interactives, notebooks rendus, IIIF via Mirador/OpenSeadragon, viewers scientifiques et scènes 3D.

### 23.3 Visualization Projection Service

Le graphe canonique n’est jamais envoyé brut à un viewer. Le service produit des projections versionnées et hashées : 2D, argument map, provenance, dépendances, désaccords, espace sémantique, paysage de branches et société d’agents.

### 23.4 3D

La V1 supporte une scène web portable, de référence Three.js/WebGL avec WebGPU lorsqu’il est disponible. Vues attendues :

- espace épistémique spatial (embeddings + relations) ;
- paysage de branches et workstreams ;
- graphe temporel ;
- société d’agents et allocation de ressources ;
- artefacts glTF/GLB, maillages et autres formats via viewer registry.

La même scène peut être ouverte dans le navigateur ou intégrée dans Emacs via WebKit/xwidget lorsqu’un build compatible existe. En l’absence de WebView, le client ouvre l’URL externe sans perte fonctionnelle.

### 23.5 ArtifactViewerRegistry

Exemples : image native, SVG, Markdown/Org, HTML, IIIF/xiiif/Mirador, graph viewer, glTF/Three.js, point cloud/Potree, molecule/Mol*, scientific volume/vtk.js, notebook/Jupyter. Un artefact déclare des hints mais le client choisit le meilleur viewer disponible.

### 23.6 CLI

La CLI offre sortie humaine et JSON stable pour scripts, administration, diagnostic, sandboxes, workers et restauration.

## 24. Packs disciplinaires, outils et SDK d’extension

### 24.1 Principe

Un discipline pack ajoute ontologies, politiques de revue, templates de mission, viewers hints et requirements de toolchain ; il ne contourne ni LEP ni le graphe canonique.

### 24.2 Mathematics

Lean/mathlib, theorem search, Z3/cvc5, calcul formel, recherche de contre-exemples, graphes de dépendance des lemmes, export/reproduction de preuves.

### 24.3 Digital Humanities / GLAM

IIIF headless, ALTO/PageXML/TEI, OCR, RDF/SPARQL/Wikidata, alignement d’entités, GIS et analyse de corpus. `xiiif` reste un viewer/éditeur humain optionnel, pas le moteur agentique obligatoire.

### 24.4 Machine Learning

PyTorch CPU/MPS/CUDA selon worker, ONNX, transformers, datasets, métriques, entraînement et inférence reproductibles. Les GPU sont des capacités de worker, jamais une hypothèse du control plane.

### 24.5 Biology, Physics et autres domaines

Chaque pack peut déclarer ses propres outils et formats, mais toute dépendance doit passer par `ToolchainRegistry`/`EnvironmentBlueprint`.

### 24.6 SDK

SDK séparés pour workers LEP, tool adapters, discipline packs, viewer adapters et import/export. Une extension ne reçoit que les capacités dont elle a besoin.

## 25. Fédération et interopérabilité

### 25.1 Objectif V1

La fédération V1 doit permettre l’échange contrôlé de programmes, sous-graphes, reviews et artefacts entre installations indépendantes, sans exiger une base commune ni une identité centrale.

### 25.2 Bundle Locus Solus

Un bundle contient :

```text
manifest.json
objects.ndjson
relations.ndjson
events-or-attestations.ndjson
artifacts/ or external-references.json
signatures/
licenses.json
redactions.json
```

Il est content-addressed, signé et peut être chiffré pour un destinataire.

### 25.3 Import

1. vérifier format, signatures, hashes et licences ;
2. scanner les artefacts ;
3. placer les objets dans une zone de quarantaine ;
4. résoudre identités et schémas ;
5. conserver les IDs d’origine et créer des IDs locaux si nécessaire ;
6. appliquer les politiques locales ;
7. créer conflits et doublons candidats ;
8. accepter par décision explicite.

### 25.4 Review exchange

Une installation peut demander une revue externe en transmettant un dossier figé et recevoir une review signée. L’installation locale reste seule responsable du niveau de validation qu’elle attribue.

### 25.5 Standards d’export

- JSON/NDJSON Locus Solus ;
- JSON-LD ;
- RDF ;
- PROV-O ;
- GraphML ;
- RO-Crate lorsque pertinent ;
- BibTeX/CSL JSON ;
- IIIF Content State pour les ancrages ;
- exports disciplinaires définis par les packs.

### 25.6 Non-confusion des identités

Une source distante, une copie locale et une révision locale sont distinctes. Les signatures et provenance doivent permettre de déterminer qui affirme quoi, où et à quelle version.

---

## 26. Migration et intégration avec l’écosystème existant

### 26.1 Renommage de l’écosystème

- nouveau dépôt : `maribakulj/locusolus` ;
- `maribakulj/openscienceDH` → `maribakulj/canterel` : **rename effectué**. Le rebrand du code est **interdit** (ADR 0010) : aucun renommage de `@synsci/*`, d'import path ou de fichier amont ;
- ~~nouveau dépôt Emacs séparé~~ — annulé par ADR 0009 ;
- `maribakulj/xiiif` conservé et recentré ;
- `maribakulj/emacs-config` conservé comme configuration personnelle.

### 26.2 Canterel

La migration préserve le runtime existant. Elle ajoute le mode worker LEP, l’admission de missions, la remontée d’événements/artefacts, les manifests d’environnement et le respect des sandboxes accordées. Les sessions standalone continuent de fonctionner.

### 26.3 Atlas et provenance existante

Atlas devient connecteur/import facultatif. Le graphe local de provenance de l’ancien OpenScience est importable comme Runs/Artifacts/Sources/Claims et relations typées, sans devenir seconde vérité canonique.

### 26.4 `research-state.md`

Les documents existants sont importés comme sources/staging ; aucune extraction automatique n’est promue sans revue.

### 26.5 xiiif

Aucune dépendance d’exécution de Locus Solus vers xiiif. Locus stocke des artefacts IIIF standards ; xiiif sait les ouvrir, corriger ou annoter depuis Emacs. Les agents utilisent des outils IIIF headless appartenant à leurs toolchains.

### 26.6 Emacs

Le code produit du cockpit est extrait de `emacs-config` vers `apps/emacs/`. `emacs-config` ne conserve que l’installation, les touches, chemins et préférences utilisateur.

### 26.7 Compatibilité progressive

Canterel peut fonctionner sans Locus Solus ; Locus Solus peut fonctionner avec fake workers ; xiiif peut fonctionner indépendamment ; le cockpit peut se connecter à un daemon local ou distant sans connaître son profil de déploiement.

## 27. Déploiement et exploitation

### 27.1 Profils obligatoires

#### `personal-local`

Tout sur un MacBook : `locusd`, PostgreSQL, workflow backend, object store local et `locus-execd`. Les sandboxes passent par une VM Linux légère (Lima/Podman Machine/équivalent) et containers rootless. Les capacités macOS natives telles que MPS/MLX sont exposées par un worker de confiance séparé.

#### `personal-node`

Control plane et/ou Canterel sur Mac mini ou autre machine dédiée ; cockpit sur un autre poste. Même contrats que local.

#### `single-node-vm`

VM Linux hébergeant services et workers CPU, sandboxes OCI/rootless. GPU externe facultatif.

#### `cloud-platform`

Ports d’infrastructure implémentés avec les services de la plateforme : durable workflows, object store, PostgreSQL ou connexion PostgreSQL, containers/sandboxes et identité. Les limites CPU/RAM/disque/absence de GPU sont déclarées comme capabilities, non contournées.

#### `distributed-hybrid`

Control plane local ou cloud + plusieurs workers LEP : Canterel OAuth local, CPU distant, GPU CUDA, browser, Lean, données sensibles on-premises.

### 27.2 Commandes

`locus up --profile <name>` matérialise une topologie déclarative ; `locus doctor` vérifie dépendances, ports, versions, ressources, attestations et accès ; `locus deployment explain` affiche exactement quels backends sont actifs.

### 27.3 Configuration portable

Un `DeploymentProfile` sélectionne les adaptateurs sans modifier le domaine. Tous les profils exposent la même API publique et passent une suite de conformance commune.

### 27.4 Sauvegarde/restauration

Une sauvegarde cohérente comprend PostgreSQL/event store, artefacts promus, refs Git, configuration non secrète, métadonnées de version et clés selon procédure. Les sandboxes temporaires ne sont pas sauvegardes canoniques.

### 27.5 Migrations et portability tests

Chaque release majeure doit être testée sur au moins macOS local et Linux VM ; le profil cloud est validé par sa propre conformance suite. Une campagne exportée doit pouvoir être restaurée sur un backend différent, sous réserve des capabilities requises par ses runs historiques.

## 28. Observabilité et auditabilité

### 28.1 Corrélation

Toutes les traces, métriques et logs utilisent selon disponibilité :

```text
workspace_id project_id program_id workstream_id branch_id
task_id attempt workflow_id agent_id worker_id review_id
command_id event_id trace_id
```

### 28.2 Métriques système

- commandes et erreurs par type ;
- lag des projections ;
- profondeur des queues ;
- leases expirées ;
- retries et orphaned tasks ;
- disponibilité des workers ;
- uploads et échecs de hash ;
- latence API ;
- stockage ;
- état Temporal et PostgreSQL ;
- décisions en attente.

### 28.3 Métriques scientifiques

- objets produits, validés, contestés et réfutés ;
- taux de reproduction ;
- temps vers résultat validé ;
- coût par résultat validé ;
- diversité et concentration du portefeuille ;
- réutilisation des résultats négatifs ;
- taux de findings confirmés ;
- valeur ajoutée de la formalisation ;
- calibration par rôle et domaine ;
- dépendances fragiles ;
- taux de synthèses contenant des contradictions omises.

### 28.4 Logs et données sensibles

Les logs ne contiennent par défaut ni prompts complets, ni secrets, ni corpus confidentiels. Les traces de contenu sont opt-in, classifiées, chiffrées et soumises à rétention.

### 28.5 Audit trail

L’utilisateur peut reconstruire :

- qui a demandé une action ;
- quelle délégation l’autorisait ;
- quelle politique a décidé ;
- quel worker, modèle et environnement ont exécuté ;
- quels artefacts ont été produits ;
- quelles revues ont conduit à la promotion ;
- quels coûts ont été consommés.

---

## 29. Tests et assurance qualité

### 29.1 Tests du domaine

- invariants d’agrégats ;
- transitions d’état ;
- versionnement ;
- fusions et conflits ;
- niveaux de validation ;
- budget ledger ;
- délégations ;
- propagation de réévaluation.

Property-based testing est requis pour identifiants, graphes, budgets, merges et commandes idempotentes.

### 29.2 Tests de contrats

- clients ↔ `locusd` ;
- LEP worker ↔ gateway ;
- Canterel adapter ↔ LEP ;
- artefact upload ;
- federation bundles ;
- schémas disciplinaires ;
- rétrocompatibilité N/N-1.

### 29.3 Tests de workflows

- replay ;
- version upgrade ;
- timeout ;
- cancellation ;
- signal humain ;
- compensation ;
- continue-as-new ;
- activity retry ;
- worker loss ;
- late result ;
- double delivery.

### 29.4 Fault injection

- PostgreSQL indisponible ;
- Temporal redémarré ;
- stockage objet indisponible ;
- réseau partitionné ;
- worker tué ;
- heartbeat retardé ;
- événement dupliqué ;
- projection corrompue ;
- upload partiel ;
- horloge décalée ;
- disque plein ;
- secret révoqué ;
- pair fédéré malveillant.

### 29.5 Tests de sécurité

- écriture hors workspace ;
- lecture interdite ;
- exfiltration réseau ;
- prompt injection ;
- fuite de secret ;
- path traversal ;
- archive bomb ;
- SSRF ;
- confusion d’identité ;
- replay de token ;
- bypass ACL par recherche vectorielle ;
- downgrade de sandbox ;
- dépendance ou plugin malveillant ;
- cross-tenant leakage.

### 29.6 Endurance

Campagne de sept jours minimum avec :

- 10 workstreams simultanés ;
- 30 branches au total ;
- 100 instances d’agents successives ;
- 5 000 tâches ;
- 250 000 événements ;
- redémarrages réguliers ;
- pertes de workers ;
- reprise sans perte ni double application.

### 29.7 Benchmarks scientifiques

Comparer :

1. agent unique ;
2. agents parallèles sans mémoire commune ;
3. hiérarchie simple de sous-agents ;
4. Canterel seul ;
5. Locus Solus sans orchestrateur de portefeuille ;
6. Locus Solus complet.

Mesures : exactitude, utilité, nouveauté, faux positifs, diversité, coût, reproductibilité, taux de rejet en revue, temps vers validation, réutilisation des négatifs et capacité à détecter une impasse.

### 29.8 Ablations obligatoires

- sans graphe ;
- sans résultats négatifs ;
- sans reviewers aveugles ;
- sans diversité de modèles ;
- sans allocation adaptative ;
- sans formalisation ;
- sans ContextView bornée ;
- sans mémoire inter-programmes.

L’objectif est de démontrer quelles parties améliorent réellement la recherche, pas seulement la complexité du produit.

---

## 30. SLO et contraintes opérationnelles

### 30.1 Durabilité

- aucune commande acquittée perdue ;
- reprise des tâches après crash ;
- RPO cible de 0 pour les événements validés en mode local avec stockage sain ;
- restauration testée régulièrement ;
- détection des divergences projection/event store.

### 30.2 API

Pour les opérations non lourdes sur le dataset de référence :

- lecture simple P95 < 300 ms ;
- commande simple P95 < 500 ms hors workflows ;
- publication d’événement client < 1 s ;
- disponibilité personnelle mesurée, sans prétention d’HA ;
- mode équipe : objectifs publiés selon l’infrastructure réellement testée.

### 30.3 Resource footprint personnel

Le profil local doit être exploitable sur une machine de 16 Go de RAM en limitant les services optionnels, avec configuration documentée. Le produit ne doit pas obliger à lancer tous les workers ni l’observabilité lourde pour consulter et piloter une campagne.

### 30.4 Dégradation contrôlée

- Temporal indisponible : lecture et commandes non workflow possibles selon politique, nouvelles missions suspendues ;
- worker absent : tâches en queue ;
- stockage objet absent : aucun commit nécessitant un artefact ne peut être promu ;
- index vectoriel indisponible : fallback lexical/graphe ;
- Web indisponible : Emacs et CLI restent fonctionnels ;
- Emacs fermé : workflows continuent.

---

## 31. Gouvernance du développement et release engineering

### 31.1 ADR obligatoires

Au minimum :

```text
0001-canterel-as-canonical-control-plane.md
0002-postgres-event-store-and-projections.md
0003-temporal-as-replaceable-durability-engine.md
0004-canterel-as-worker.md
0005-cwp-v1.md
0006-epistemic-versioning.md
0007-budget-ledger.md
0008-context-views-and-information-isolation.md
0009-artifact-cas.md
0010-federation-trust-model.md
0011-emacs-as-first-class-client.md
0012-atlas-migration.md
```

### 31.2 Qualité du code

- TypeScript strict ;
- aucun `any` non justifié dans le domaine/protocole ;
- schémas runtime ;
- migrations et compatibilité testées ;
- couverture orientée invariants, non score artificiel ;
- lint des dépendances de packages ;
- documentation des erreurs et états ;
- changelog et versionnement sémantique ;
- builds reproductibles autant que possible.

### 31.3 Release V1

La release comprend :

- images et binaires signés lorsque l’infrastructure le permet ;
- SBOM ;
- migrations ;
- guide d’installation locale ;
- guide opérateur ;
- guide worker Canterel ;
- paquet Emacs ;
- console Web ;
- CLI ;
- scénarios de démonstration reproductibles ;
- rapport de benchmarks et limites connues ;
- procédure de sauvegarde/restauration ;
- threat model.

---

## 32. Critères d’acceptation V1

Locus Solus V1 n’est accepté que si les scénarios suivants sont automatisés ou documentés reproductiblement.

### 32.1 Portabilité

- même programme créé et repris sur `personal-local` puis `single-node-vm` ;
- API, IDs et graphe inchangés ;
- workflow backend remplaçable sans migration des objets scientifiques ;
- artefacts restaurables entre filesystem/S3-compatible ;
- worker local et worker distant visibles simultanément.

### 32.2 Durabilité et graphe

- campagne survivant aux redémarrages complets ;
- event replay et projections reconstruisibles ;
- graphes typés/versionnés, conflits et inférences multi-prémisses ;
- au moins 250 000 objets/relations testés.

### 32.3 Execution Fabric

- `EnvironmentBlueprint`, `SandboxSpec`, `ResourceSpec` et attestations ;
- local macOS capable de lancer une sandbox Linux isolée ;
- quotas CPU/RAM/PID/disque vérifiés par self-tests ;
- aucun home, socket de runtime ou secret hôte accessible par défaut ;
- scheduler refuse une mission lorsque capacité réservée insuffisante ;
- reroutage vers worker compatible possible.

### 32.4 Toolchains

- profils Python science, Lean/mathlib, SMT, browser, DH et ML CPU construits/testés ;
- PyTorch MPS détectable sur worker macOS compatible ;
- profil CUDA exécutable sur worker GPU dédié ;
- lockfiles, SBOM, scan et image digest enregistrés ;
- EnvironmentBuildWorkflow pour dépendance supplémentaire.

### 32.5 Société d’agents et revue

- dix workstreams possibles sans perte de cohérence ;
- équipes multi-modèles ;
- reviewers aveugles, rebuttal, recheck et méta-revue ;
- ContextViews isolées ;
- résultats tardifs et contradictions conservés.

### 32.6 Artefacts et visualisation

- artefacts content-addressed ;
- Markdown/HTML/SVG/PNG/notebook/IIIF/graph/3D ouvrables par viewer registry ;
- scène 3D web portable ;
- intégration Emacs via WebView lorsque disponible et fallback navigateur ;
- xiiif capable d’ouvrir un artefact IIIF produit par un agent sans être requis pendant son exécution.

### 32.7 Interfaces

- programme pilotable depuis `apps/emacs` ;
- console web et CLI ;
- sandbox list/inspect/logs/files/resource usage ;
- approbations et décisions ;
- reconnexion/replay.

### 32.8 Scénarios scientifiques

- math : conjecture → recherche → contre-exemple ou preuve → Lean/SMT → review → reproduction ;
- DH : corpus IIIF → extraction headless → artefacts/Content States → interprétation → xiiif/Mirador review humaine ;
- ML : tâche PyTorch CPU puis reroutage GPU/MPS selon capacité ;
- campagne hybride avec Canterel local et worker distant.

## 33. Non-objectifs de la V1

- garantir la résolution d’un problème ouvert ;
- remplacer Canterel comme runtime de modèles et d’outils ;
- développer un réseau social public ;
- créer un marketplace commercial ;
- héberger un entraînement massif de modèles fondation ;
- imposer une ontologie disciplinaire unique ;
- considérer le consensus majoritaire comme vérité ;
- rendre toute action autonome sans seuil humain ;
- prétendre sécuriser un worker non attesté ;
- dépendre obligatoirement d’Atlas, d’un cloud ou d’un fournisseur LLM ;
- optimiser prématurément pour des milliards de nœuds au détriment des invariants.

---

## 34. Documentation obligatoire

- vision et architecture ;
- modèle de domaine et glossaire ;
- schémas de commande/événement ;
- LEP v1 ;
- sécurité et confidentialité ;
- politiques et gouvernance ;
- workflows ;
- installation et exploitation ;
- backup/restore ;
- migration Canterel/Atlas ;
- création d’un agent, worker et pack disciplinaire ;
- client Emacs ;
- API Web/CLI ;
- formats d’export/fédération ;
- benchmarks, ablations et limites connues ;
- procédures d’incident et de révocation.

---

## 35. Consolidation architecturale finale

Cette version intègre les décisions prises après les audits précédents :

- **Locus Solus** remplace le nom du laboratoire/orchestrateur ;
- **Canterel** est le nouveau nom du runtime auparavant `openscienceDH` ;
- LEP remplace le protocole nommé d’après un worker particulier ;
- le client Emacs produit vit dans `apps/emacs/` du monorepo ;
- xiiif est recentré sur l’usage humain IIIF et ne constitue plus un service obligatoire de preuve ;
- Emacs est un cockpit, pas un moteur universel de visualisation ou d’exécution ;
- la visualisation riche/3D est web-native et intégrable dans Emacs ;
- Temporal devient un backend de workflow, non une dépendance du domaine ;
- l’exécution est portée par une Execution Fabric avec sandbox, quotas, attestations et placement ;
- Lean, PyTorch et les autres dépendances deviennent des toolchains versionnées et testées ;
- local, Mac mini, VM, cloud et hybride sont des profils équivalents au niveau du domaine ;
- le GPU est une capability de worker, jamais une hypothèse du control plane ;
- les résultats restent artifact-first et lisibles indépendamment des outils qui les ont produits.

## 36. Définition finale

Locus Solus V1 est un système d’exploitation de recherche local-first et déployable partout, capable d’orchestrer durablement des sociétés de chercheurs humains et artificiels au-dessus d’un graphe épistémique versionné. Il coordonne Canterel et d’autres workers spécialisés, fabrique ou sélectionne des environnements reproductibles, isole l’exécution dans des sandboxes attestées, alloue explicitement CPU/RAM/disque/GPU, conserve les artefacts et résultats négatifs, organise revue et reproduction, puis présente l’ensemble dans un cockpit Emacs et des viewers web 2D/3D sans dépendre d’un fournisseur de modèle, d’un moteur de workflow, d’un cloud ou d’un poste précis.
