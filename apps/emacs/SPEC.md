# apps/emacs — Spécification du cockpit Emacs Locus Solus V1

**Dépôt cible :** `maribakulj/apps/emacs`  
**Rôle architectural :** client Emacs générique, publiable et testé pour Locus Solus  
**Responsabilité :** cockpit, commandes, événements, inspection, intégrations scientifiques et viewers  
**Statut :** spécification normative V1 du nouveau dépôt

---

## 0. Statut et conventions normatives

Les termes **DOIT**, **NE DOIT PAS**, **DEVRAIT**, **PEUT** sont normatifs.

Ce dépôt contient le **client produit** Emacs de Locus Solus. Il doit être installable indépendamment de toute configuration personnelle et fonctionner contre un daemon local ou distant. `emacs-config` n’est qu’un consommateur/configurateur de ce package.

Le client :

- ne conserve aucun état scientifique canonique ;
- ne dépend pas de Temporal, PostgreSQL ni d’un runtime de containers ;
- communique uniquement par les APIs/events publics de Locus Solus ;
- peut fonctionner en lecture offline sur son cache local ;
- intègre des viewers natifs ou web sans transformer Emacs en moteur graphique universel ;
- ne contient aucun chemin, token, domaine ou préférence personnelle codé en dur.

## 1. Vision d’usage

### 1.1 Emacs comme cockpit expert

Emacs est la surface de travail principale pour :

- explorer un programme de recherche ;
- créer et comparer des branches ;
- inspecter tâches, agents, budgets et décisions ;
- soumettre des commandes au control plane ;
- suivre l’exécution Canterel ;
- examiner preuves, claims, objections et résultats négatifs ;
- lancer, lire et répondre aux revues ;
- ouvrir fichiers, worktrees, notebooks et preuves formelles ;
- capturer des éléments dans Org ;
- naviguer vers une région xiiif ;
- rejouer l’historique ;
- approuver explicitement les opérations sensibles.

### 1.2 Non-substitution

Emacs n’est pas :

- le scheduler ;
- l’event store ;
- le graphe canonique ;
- le budget ledger ;
- le runtime agentique ;
- le stockage d’artefacts ;
- le policy engine ;
- un substitut à la durabilité Temporal.

### 1.3 Expérience cible

L’utilisateur doit pouvoir piloter un programme complet sans navigateur, mais le package reste un client : toute action mutante transite par l’API Locus Solus et respecte `expected_revision`, identité, scopes, approvals et idempotency.

---

## 2. Frontières entre `apps/emacs` et `emacs-config`

### 2.1 `apps/emacs` possède

- client HTTP/stream et authentification abstraite ;
- modèles de buffers et commandes ;
- transients ;
- dashboards textuels ;
- programmes, branches, agents, reviews, budgets et décisions ;
- sandbox inspector ;
- artifact viewer registry ;
- intégrations génériques Org/Magit/Jupyter/xiiif ;
- WebView bridge et visualisations 2D/3D ;
- cache local, cursors, offline read-only ;
- tests ERT et serveur mock.

### 2.2 `emacs-config` possède

- installation du package ;
- URL de Locus Solus par environnement ;
- préférences de layout ;
- raccourcis personnels ;
- chemins locaux ;
- choix du viewer externe ;
- thème et ergonomie ;
- activation optionnelle de xiiif/Jupyter/etc.

### 2.3 Interdictions

Aucune logique produit, aucun schéma LEP, aucun parser d’événement et aucune règle de sécurité essentielle ne doit être uniquement dans `emacs-config`. Le package ne doit jamais accéder directement à PostgreSQL, au runtime de containers ou aux secrets des workers.

## 3. Structure du dépôt

```text
apps/emacs/
├── locusolus.el
├── locusolus-client.el
├── locusolus-events.el
├── locusolus-dashboard.el
├── locusolus-program.el
├── locusolus-branch.el
├── locusolus-review.el
├── locusolus-sandbox.el
├── locusolus-artifact.el
├── locusolus-viewer.el
├── locusolus-3d.el
├── locusolus-org.el
├── locusolus-magit.el
├── locusolus-jupyter.el
├── locusolus-xiiif.el
├── locusolus-transient.el
├── locusolus-cache.el
├── locusolus-doctor.el
├── test/
└── README.md
```

Le package doit pouvoir être installé via `package-vc`/straight/quelpa ou autre méthode standard sans copier des fichiers dans la configuration personnelle.

## 4. Dépendances

### 4.1 Obligatoires

- Emacs 30 ;
- Emacs 30 et bibliothèques intégrées nécessaires au client ;
- bibliothèques intégrées `json`, `auth-source`, `project`, `tabulated-list`, `tab-bar` ;
- `transient` si le client le requiert.

### 4.2 Optionnelles

- Org ;
- Magit ;
- xiiif ;
- Denote ;
- org-roam ;
- Jupyter ;
- Org Babel ;
- eat ;
- Graphviz ;
- SVG ;
- notifications système ;
- consult/embark ;
- perspective ou autre gestionnaire, uniquement si compatible avec tab-bar.

### 4.3 Politique de dégradation

Chaque dépendance optionnelle :

- est détectée ;
- ajoute ses commandes seulement si disponible ;
- produit une erreur actionnable si appelée sans dépendance ;
- ne casse ni le démarrage ni les fonctions Locus Solus de base.

### 4.4 Verrouillage

Le package publie sa propre version. Une incompatibilité d’API/protocole serveur est détectée et affichée explicitement ; aucun gestionnaire de paquets personnel n’est imposé.

---

## 5. Configuration principale

```elisp
(setq locus-endpoint "http://127.0.0.1:7420"
      locus-workspace nil
      locus-auto-connect t
      locus-reconnect t
      locus-event-buffer-size 10000
      locus-cache-directory
      (expand-file-name "locus/" user-emacs-directory)
      locus-notification-level 'important
      locus-confirm-sensitive-actions t
      locus-open-links-in-current-workspace t)
```

Les configurations consommatrices doivent employer les options publiques du client. Les variables internes ne constituent pas une API.

---

## 6. Authentification et secrets

### 6.1 auth-source

Le token ou credential client est récupéré via :

```text
machine locus.local
login marcel
password <credential>
```

Le host exact est configurable.

### 6.2 Exigences

- aucun token dans Git ;
- aucun token dans `custom-file` ;
- aucun token dans les messages de debug ;
- pas de copie dans kill-ring ;
- expiration et reconnexion supportées ;
- scopes visibles dans un buffer d’identité ;
- confirmation si l’endpoint change d’origine.

### 6.3 Identité active

Le mode-line ou dashboard indique :

- endpoint ;
- workspace ;
- principal ;
- rôle/scopes ;
- connecté, offline ou replay ;
- environnement local/distant.

### 6.4 Actions administratives

Les commandes d’administration ne sont pas chargées dans le transient normal si le principal ne possède pas le scope nécessaire.

---

## 7. Démarrage et cycle de vie

### 7.1 Séquence non bloquante

Au démarrage :

1. charger les modules du package ;
2. configurer auth et endpoint ;
3. connecter de manière asynchrone si activé ;
4. afficher un statut discret ;
5. ne pas ralentir l’ouverture de la première frame ;
6. ne lancer aucun stack serveur sans action explicite.

### 7.2 Démarrage local assisté

Une commande personnelle peut exécuter :

```text
M-x locus-start-local-stack
```

Elle doit :

- détecter la CLI `locus` ;
- afficher la commande prévue ;
- utiliser un buffer process dédié ;
- ne pas demander sudo ;
- vérifier health ;
- ne pas relancer une stack déjà active ;
- proposer les diagnostics en cas d’échec.

### 7.3 Emacs daemon

La connexion est partagée entre frames `emacsclient`. Les abonnements aux événements ne doivent pas être multipliés à chaque frame.

### 7.4 Arrêt

À `kill-emacs-hook` :

- fermer proprement les streams ;
- persister seulement le cache autorisé ;
- ne pas arrêter Locus Solus ;
- ne pas annuler de mission ;
- ne pas envoyer de commande mutante implicite.

### 7.5 Reconnexion

- backoff avec jitter ;
- reprise depuis cursor ;
- indicateur `stale` pendant la coupure ;
- aucune action mutante en cache comme si elle avait réussi ;
- resoumission seulement via idempotency key connue.

---

## 8. Workspaces tab-bar

### 8.1 Workspaces canoniques

```text
Locus Solus Dashboard
Program
Branch
Review
Evidence
Writing
Data
Operations
```

### 8.2 Principes de layout

- réutiliser une tab existante pour le même objet ;
- préserver les buffers non Locus Solus ;
- restaurer les dispositions de façon tolérante ;
- ne pas dépendre de tailles de frame fixes ;
- proposer un mode compact laptop ;
- garder un buffer focal actif.

### 8.3 Dashboard

```text
┌────────────────────────────────┬──────────────────────────────┐
│ Programs / portfolio           │ Decisions / approvals        │
├────────────────────────────────┼──────────────────────────────┤
│ Workers / missions             │ Reviews / alerts             │
└────────────────────────────────┴──────────────────────────────┘
```

### 8.4 Program

```text
┌────────────────────────────────┬──────────────────────────────┐
│ Workstreams / branches         │ Orchestrator / decisions     │
├────────────────────────────────┼──────────────────────────────┤
│ Timeline                       │ Portfolio metrics / budget   │
└────────────────────────────────┴──────────────────────────────┘
```

### 8.5 Branch

```text
┌────────────────────────────────┬──────────────────────────────┐
│ Epistemic graph / claims       │ Tasks / agents / workers     │
├────────────────────────────────┼──────────────────────────────┤
│ Project files / Magit          │ Inspector / evidence         │
└────────────────────────────────┴──────────────────────────────┘
```

### 8.6 Review

```text
┌────────────────────────────────┬──────────────────────────────┐
│ Frozen dossier                 │ Findings                     │
├────────────────────────────────┼──────────────────────────────┤
│ Evidence/artifacts             │ Rebuttal / decision          │
└────────────────────────────────┴──────────────────────────────┘
```

---

## 9. Buffers et vues

### 9.1 `*Locus Solus Dashboard*`

Colonnes minimales :

```text
Program  Status  Active branches  Tasks  Agents
Reviews  Decisions  Budget  Last event  Risk
```

Actions : ouvrir, créer, filtrer, portfolio, reviews, approvals, workers, diagnostics.

### 9.2 `*Locus Solus Program: <name>*`

- objective et success contract ;
- état et révision ;
- workstreams ;
- branches ;
- portfolio ;
- open questions ;
- décisions ;
- budget ledger résumé ;
- événements ;
- exports.

### 9.3 `*Locus Solus Branch: <name>*`

- objectif ;
- état ;
- base revision ;
- équipe ;
- tâches ;
- claims et validation ;
- objections ;
- résultats négatifs ;
- dépendances ;
- artifacts/evidence ;
- conflits ;
- merge proposals ;
- mémoire.

### 9.4 `*Locus Solus Agents*`

```text
Agent  Role  Model policy  Branch  Task  Status
Worker  Context  Usage  Calibration  Last event
```

Le modèle affiché distingue policy demandée et modèle effectivement résolu.

### 9.5 `*Locus Solus Workers*`

```text
Worker  Runtime  State  Capabilities  Isolation
Slots  Missions  Heartbeat  Version  Alerts
```

### 9.6 `*Locus Solus Review Queue*`

- target ;
- dossier revision ;
- type ;
- blind status ;
- reviewers ;
- findings ;
- rebuttal ;
- recheck ;
- meta-review ;
- action attendue.

### 9.7 `*Locus Solus Timeline*`

Filtres : workspace, program, branch, task, agent, worker, object, severity, event type, period, correlation.

### 9.8 `*Locus Solus Decisions*`

Affiche proposition, alternatives, justification, policy, coût, risques, approvals et résultat. Les opérations sensibles se font depuis cette vue ou un formulaire équivalent.

### 9.9 `*Locus Solus Evidence*`

Affiche provenance, hashes, sources, droits, transformations, liens claims, validation du bundle et reproductibilité.

### 9.10 `*Locus Solus Diagnostics*`

- connexion ;
- version API/client ;
- cursor ;
- cache ;
- auth/scopes ;
- latence ;
- erreurs ;
- stack locale ;
- workers ;
- projection lag.

---

## 10. Dispatcher Transient

Le point d’entrée personnel :

```text
M-x locus-dispatch
```

### 10.1 Groupes

```text
[P] Program
[B] Branch
[T] Task/Team
[A] Agent/Worker
[R] Review
[E] Evidence/Artifact
[G] Graph
[D] Decision/Approval
[O] Operations
[?] Diagnose/Help
```

### 10.2 Program

- create/open/pause/resume ;
- portfolio ;
- budget ;
- export ;
- timeline.

### 10.3 Branch

- create/fork ;
- suspend/resume ;
- diff ;
- merge propose ;
- rebase/cherry-pick ;
- open worktree ;
- compare claims.

### 10.4 Task/Team

- create mission proposal ;
- spawn team proposal ;
- cancel/retry ;
- inspect ContextView ;
- view usage.

### 10.5 Review

- request ;
- open dossier ;
- submit finding ;
- rebuttal ;
- recheck ;
- meta-review ;
- compare reviewers.

### 10.6 Evidence

- open ;
- stage local file ;
- open xiiif ;
- verify hashes ;
- reproduce ;
- view lineage ;
- export.

### 10.7 Operations

- connect/disconnect ;
- start local stack ;
- worker doctor ;
- projection status ;
- backup status ;
- logs ;
- drain worker.

---

## 11. Commandes mutantes et sécurité UX

### 11.1 Prévisualisation

Une commande complexe affiche avant envoi :

- type ;
- objet cible ;
- expected revision ;
- payload ;
- coût/plafond ;
- policy ;
- effets ;
- approvals ;
- idempotency key.

### 11.2 Confirmation graduée

Niveaux :

- **safe** : création locale ou query, pas de confirmation ;
- **controlled** : commande réversible, confirmation configurable ;
- **sensitive** : coût, publication, merge, secret, données ; confirmation obligatoire ;
- **critical** : suppression, fédération, changement admin ; formulaire explicite et raison.

### 11.3 Conflits

Si `expected_revision` est obsolète :

- afficher état courant ;
- présenter diff ;
- proposer refresh, rebase ou nouvelle commande ;
- ne jamais resoumettre automatiquement avec la nouvelle révision.

### 11.4 Idempotence

Le client conserve temporairement les idempotency keys et affiche le résultat connu si une réponse a été perdue.

### 11.5 Édition structurée

Les payloads complexes peuvent être édités dans un buffer JSON/YAML temporaire avec validation locale et schema, mais les secrets sont exclus.

---

## 12. Dialogue avec l’orchestrateur

### 12.1 Rôle

Le buffer de dialogue est une interface de proposition, non un chat qui contourne les commandes.

### 12.2 Flux

```text
instruction naturelle
→ proposition structurée
→ plan branches/agents/budget/policies
→ inspection et édition
→ commande Locus Solus
→ décisions et événements
```

### 12.3 Affichage

La proposition distingue :

- interprétation de la demande ;
- hypothèses ;
- objets créés ;
- agents/modèles ;
- budget ;
- indépendance/review ;
- risques ;
- approvals ;
- conditions d’arrêt.

### 12.4 Historique

Les instructions et propositions sont liées aux `Decision` et `CommandEnvelope` canoniques. Le buffer local n’est pas la seule trace.

---

## 13. Graphe épistémique dans Emacs

### 13.1 Vue textuelle canonique

Une vue `tabulated-list` ou outline permet :

- navigation clavier ;
- filtres ;
- expansion relations ;
- état de validation ;
- version ;
- branche ;
- conflits ;
- provenance ;
- actions contextuelles.

### 13.2 Vue SVG/Graphviz

La vue visuelle :

- est une projection ;
- conserve sélection avec la vue textuelle ;
- permet zoom/pan ;
- colore selon thème via faces, sans codes figés ;
- limite le nombre de nœuds ;
- expose légende et filtres ;
- reste accessible sans souris.

### 13.3 Vues spécialisées

- argument map ;
- dependency graph ;
- branch diff ;
- inference hypergraph ;
- provenance/run graph ;
- review disagreement ;
- agent/team graph.

### 13.4 Navigation

`RET` ouvre l’objet ; `TAB` développe ; `g` rafraîchit ; `f` filtre ; `d` diff ; `e` evidence ; `r` review ; `t` timeline.

### 13.5 Pas d’édition directe

Une modification du graphe passe par un formulaire de commande avec validation et expected revision.

---

## 13A. Visualisations riches et 3D

### 13A.1 Principe

Emacs affiche des représentations opératoires ; il ne réimplémente pas un moteur graphique 3D en Elisp. Locus Solus produit une projection et une application web de viewer, de référence Three.js. Le package décide entre intégration WebView et navigateur externe.

### 13A.2 Modes

`locus-viewer-mode` :

- `native` pour texte/SVG/images simples ;
- `embedded` pour WebKit/xwidget lorsque le build Emacs le supporte ;
- `external` pour navigateur ;
- `auto` choisit le meilleur mode.

### 13A.3 Vues 3D V1

- espace épistémique : embeddings + relations ;
- paysage de branches/workstreams ;
- temps/provenance ;
- société d’agents et allocation de ressources ;
- artefacts glTF/GLB et scènes scientifiques via viewer registry.

### 13A.4 Interaction bidirectionnelle

Sélection d’un claim dans Emacs → focus de la scène. Clic d’un node dans la scène → ouverture du buffer correspondant. Les messages passent par IDs et événements structurés ; aucun JavaScript n’accède directement au stockage canonique.

### 13A.5 Sécurité

Le WebView charge uniquement l’application Locus autorisée ou un bundle local signé/contrôlé. Pas d’évaluation arbitraire de JS provenant d’un artefact non fiable dans le contexte privilégié d’Emacs. Les contenus non fiables s’ouvrent dans un profil navigateur isolé.

## 14. Événements temps réel

### 14.1 Stream

Le client :

- reprend depuis cursor ;
- déduplique ;
- batch le rendu ;
- conserve les événements critiques ;
- marque les gaps ;
- permet replay ;
- n’effectue pas une query complète à chaque événement.

### 14.2 Notifications

Niveaux :

- `all` ;
- `important` ;
- `decisions` ;
- `critical` ;
- `none`.

Par défaut, notifier :

- approval demandé ;
- mission bloquée ;
- résultat staged majeur ;
- review reçue ;
- conflit ;
- budget proche du plafond ;
- worker perdu ;
- sécurité ;
- reproduction divergente.

### 14.3 Anti-bruit

- regroupement par objet ;
- cooldown ;
- résumé ;
- quiet hours ;
- focus mode ;
- notification persistante pour décisions.

### 14.4 Mode concentration

Pendant l’écriture : uniquement critical/decision, avec résumé différé dans le dashboard.

---

## 15. Intégration Org

### 15.1 Liens

```text
locus:program:<id>
locus:branch:<id>
locus:task:<id>
locus:agent:<id>
locus:claim:<revision-id>
locus:review:<id>
locus:artifact:<id>
locus:evidence:<id>
locus:decision:<id>
```

Les liens utilisent des identifiants stables, pas des noms modifiables.

### 15.2 Capture templates

Templates :

- research question ;
- hypothesis proposal ;
- objection ;
- negative result ;
- decision note ;
- literature source ;
- artefacts IIIF / Content States ;
- review finding ;
- human observation.

### 15.3 Staging

Une capture Org locale reste une note. Une commande explicite la transforme en proposition Locus Solus avec aperçu du mapping.

### 15.4 Propriétés

```text
:CANTEREL_WORKSPACE:
:CANTEREL_PROGRAM:
:CANTEREL_BRANCH:
:CANTEREL_OBJECT:
:CANTEREL_REVISION:
:CANTEREL_STATUS:
:CANTEREL_SYNCED_AT:
```

### 15.5 Export

L’export documentaire peut résoudre les citations et preuves Locus Solus sans injecter de données classifiées dans un document public.

### 15.6 Synchronisation

Pas de synchronisation bidirectionnelle implicite Org ↔ graphe. Les différences sont présentées et les commandes explicites.

---

## 16. Intégration Magit et project.el

### 16.1 Mapping

Une branche Locus Solus peut référencer :

- dépôt ;
- commit de base ;
- worktree ;
- patch/commit produit ;
- artefacts ;
- merge proposal.

### 16.2 Actions

- ouvrir worktree ;
- status Magit ;
- diff avec base ;
- inspecter commit produit par mission ;
- joindre patch à review ;
- stage un artefact ;
- proposer merge Locus Solus.

### 16.3 Séparation Git/Locus Solus

Un merge Git local ne doit pas être présenté comme merge épistémique Locus Solus. Le client affiche les deux états distincts.

### 16.4 Sécurité

Pas de push automatique par une commande de navigation. Les opérations Git distantes restent explicites.

---

## 17. Intégration xiiif

### 17.1 Depuis Locus Solus

Un EvidenceRef xiiif ouvre :

- manifest ;
- canvas ;
- région ;
- OCR fragment ;
- annotation ;
- bundle d’artefacts IIIF.

### 17.2 Depuis xiiif

`xiiif-locus-stage-evidence` ouvre ou cible la branche courante et affiche la proposition avant staging.

### 17.3 Vue combinée

Dans une branch/review view :

- miniature ou lien ;
- source/provider ;
- région ;
- extrait OCR ;
- hash/snapshot ;
- droits ;
- relation au claim ;
- version.

### 17.4 Dégradation

Sans xiiif, afficher metadata et proposer d’ouvrir Content State dans un navigateur ou copier une référence, sans erreur de chargement.

---

## 18. Intégration Jupyter et Org Babel

### 18.1 Distinction des runs

- run local exploratoire ;
- run d’une mission Canterel ;
- reproduction Locus Solus ;
- artefact notebook ;
- résultat canonique staged/validated.

L’UI ne les confond pas.

### 18.2 Ouverture

Depuis un Run/Artifact :

- ouvrir notebook ;
- environnement ;
- inputs/outputs ;
- logs ;
- hash ;
- reproduction status ;
- diff de résultats.

### 18.3 Soumission

Un notebook local peut être proposé comme artefact avec EnvironmentManifest. Il n’est pas validé par le seul fait de s’exécuter dans le buffer utilisateur.

### 18.4 Reproduction

Commande : demander à Locus Solus une reproduction indépendante, plutôt que relancer silencieusement dans le même environnement.

---

## 19. Intégration `eat` et Canterel

### 19.1 Terminal contextuel

Depuis une branche/mission :

- ouvrir shell dans worktree ;
- afficher variables non secrètes de contexte ;
- lancer CLI diagnostics ;
- suivre logs worker ;
- ouvrir Canterel local.

### 19.2 Interdictions

Le terminal ne reçoit pas automatiquement tokens Locus Solus ou secrets mission. Les grants restent gérés par les processus concernés.

### 19.3 Worker diagnostics

Actions :

```text
canterel worker status
canterel worker doctor
canterel worker capabilities
canterel worker inspect-mission
```

### 19.4 Session Canterel

Le client peut ouvrir l’URL locale d’une session associée, mais le statut canonique reste affiché depuis Locus Solus.

---

## 20. Denote et org-roam

### 20.1 Rôle

Denote/org-roam restent la mémoire personnelle et l’environnement de notes. Ils ne deviennent pas le graphe épistémique canonique.

### 20.2 Liens

Les notes peuvent référencer les identifiants Locus Solus. Les backlinks personnels restent distincts des relations scientifiques canoniques.

### 20.3 Import

Une note peut être proposée à Locus Solus via un assistant d’extraction : claims, sources, décisions, objections. Le résultat est `staged` et revu.

### 20.4 Confidentialité

Le client vérifie la classification avant d’insérer du contenu Locus Solus complet dans une note synchronisée ou publiée.

---

## 20A. Sandboxes et environnements

Buffer `*Locus Sandboxes*` : ID, task/attempt, worker/backend, environment, CPU/RAM/disque/GPU, état, durée, réseau, niveau d’isolation et attestation.

Commandes : list, inspect, logs, files, metrics, attestation, terminate, request-extension. `shell` est optionnel, exige confirmation et marque le run `human_modified` lorsque l’utilisateur modifie l’environnement.

Le package ne parle jamais directement à Docker/Podman : toutes les opérations passent par l’API Locus.

## 21. Fichiers, artefacts et cache

### 21.1 Ouverture

Types :

- fichier local ;
- worktree ;
- artefact content-addressed ;
- URL temporaire ;
- bundle ;
- notebook ;
- image ;
- PDF ;
- preuve formelle.

### 21.2 Téléchargement

- URL courte durée ;
- hash vérifié ;
- taille contrôlée ;
- cache local borné ;
- fichier non exécuté automatiquement ;
- classification visible ;
- quarantaine si type douteux.

### 21.3 Cache

Le cache client :

- est supprimable ;
- ne contient pas les secrets ;
- est indexé par hash ;
- respecte TTL/classification ;
- ne sert pas de source canonique ;
- peut être purgé par commande.

### 21.4 Fichiers distants

Pas d’édition TRAMP directe d’un stockage interne Locus Solus. Les modifications passent par les workflows appropriés.

---

## 22. Mode offline et données stale

### 22.1 Lecture

Le cache permet une lecture limitée de :

- derniers programmes/branches ;
- objets ouverts ;
- graphes récents ;
- artefacts locaux ;
- timeline partielle.

### 22.2 Marquage

Toute donnée offline affiche :

- dernière synchronisation ;
- cursor ;
- état `stale` ;
- opérations indisponibles.

### 22.3 Écriture

La V1 ne met pas en file générique des mutations offline. Les notes locales restent locales et peuvent être soumises après reconnexion avec revalidation de révision.

---

## 23. Performance et ergonomie

### 23.1 Objectifs

- connexion asynchrone ;
- dashboard initial progressif ;
- pagination ;
- rendu batch des événements ;
- pas de graphe gigantesque par défaut ;
- annulation des queries ;
- caches bornés ;
- aucun blocage réseau du thread UI.

### 23.2 Gros graphes

Le client demande une projection filtrée et paginée. Il ne télécharge pas 250 000 objets pour afficher une branche.

### 23.3 Mode-line

Information concise : connexion, workspace, programme/branche courante, décisions et alertes. Les détails restent dans dashboard.

### 23.4 Accessibilité

- navigation clavier complète ;
- faces respectant le thème ;
- pas d’information uniquement par couleur ;
- texte alternatif pour SVG ;
- commandes discoverables ;
- densité configurable.

---

## 24. Résilience

### 24.1 Erreurs réseau

- message non intrusif ;
- statut visible ;
- retry borné ;
- diagnostics ;
- conservation du travail local ;
- absence de double commande.

### 24.2 Crash buffer

La fermeture d’un buffer ne change aucun état serveur, sauf action explicitement commandée.

### 24.3 Version incompatible

Le client affiche versions, fonctionnalités indisponibles et chemin de mise à jour. Il ne tente pas des commandes inconnues.

### 24.4 Rebuild de projection

Pendant un rebuild serveur, les vues signalent données partielles ou retardées et évitent les décisions basées sur une projection incohérente.

---

## 25. Sécurité

### 25.1 Menaces

- token dans config/log ;
- endpoint malveillant ;
- artefact hostile ;
- commande destructive accidentelle ;
- contenu source avec propriétés locales ;
- injection Elisp via données ;
- liens `file:` non sûrs ;
- confusion environnement local/distant ;
- publication de données classifiées.

### 25.2 Règles

- données rendues comme texte, jamais évaluées ;
- fichiers temporaires avec permissions restrictives ;
- URLs et paths validés ;
- confirmations graduées ;
- auth-source ;
- endpoint visible ;
- secrets redacted ;
- classification affichée ;
- artefacts non exécutés ;
- logs diagnostics nettoyés.

### 25.3 Commandes Elisp

Aucune réponse serveur ne peut injecter un symbole de fonction à appeler. Les actions sont mappées à une allowlist locale.

---

## 26. Personnalisation

### 26.1 Variables personnelles

- endpoint ;
- workspace préféré ;
- notification ;
- disposition ;
- densité ;
- taille du cache ;
- commandes externes ;
- chemins de projets ;
- intégrations optionnelles.

### 26.2 Faces

Définir des faces sémantiques :

```text
locus-running-face
locus-blocked-face
locus-staged-face
locus-validated-face
locus-rejected-face
locus-warning-face
locus-sensitive-face
```

Elles héritent de faces standards et ne fixent pas un thème complet.

### 26.3 Keybindings

Préfixe recommandé :

```text
C-c c  → Locus Solus
C-c x  → xiiif
```

Les conflits avec bindings existants sont testés. Les commandes essentielles restent accessibles via `M-x`.

---

## 27. Tests et assurance qualité

### 27.1 ERT package

- chargement du module ;
- variables ;
- auth-source mock ;
- intégrations conditionnelles ;
- layouts ;
- templates Org ;
- keybindings ;
- notifications ;
- redaction.

### 27.2 Smoke tests batch

```bash
emacs -Q --batch -l init.el --eval '(message "startup-ok")'
```

Profils :

- toutes dépendances ;
- sans Locus Solus ;
- sans xiiif ;
- sans Magit/Jupyter ;
- daemon ;
- auth absent ;
- endpoint indisponible.

### 27.3 Tests client simulé

Avec serveur factice :

- connexion ;
- cursor replay ;
- conflict ;
- approval ;
- event storm ;
- data stale ;
- version incompatible ;
- artefact hash mismatch.

### 27.4 Parcours manuels

- créer programme ;
- fork branches ;
- lancer équipe ;
- suivre Canterel ;
- examiner commit ;
- revue et rebuttal ;
- artefacts IIIF / Content States ;
- merge proposal ;
- reproduction ;
- reconnexion après redémarrage.

### 27.5 Performance

- 10 000 événements reçus sans gel ;
- timeline paginée ;
- graphe filtré ;
- plusieurs frames daemon ;
- cache purgé ;
- démarrage sans régression notable.

---

## 28. Critères d’acceptation V1

### 28.1 Architecture

- package installable et chargeable dans un Emacs vierge ;
- aucune duplication des schemas/transports ;
- aucune écriture directe au backend ;
- dépendances optionnelles dégradées proprement.

### 28.2 Pilotage

- programme, branche, tâches, agents, workers, graphes, reviews, budgets et décisions accessibles ;
- commandes mutantes prévisualisées ;
- expected revision et conflits gérés ;
- approvals exécutables ;
- replay timeline.

### 28.3 Intégrations

- Org links/capture ;
- Magit/worktrees ;
- artefacts IIIF / Content States ;
- Jupyter/runs ;
- eat/Canterel worker ;
- Denote/org-roam sans confusion de graphes.

### 28.4 Résilience

- Emacs daemon ;
- multi-frame ;
- endpoint absent ;
- reconnexion/cursor ;
- mode offline lecture ;
- version incompatible ;
- aucune double commande.

### 28.5 Sécurité

- auth-source ;
- secrets absents de Git/log/cache ;
- endpoint/scopes visibles ;
- confirmations sensibles ;
- artefacts vérifiés ;
- données serveur jamais évaluées comme Elisp.

### 28.6 UX

- cockpit utilisable sans navigateur ;
- navigation clavier ;
- workspaces cohérents ;
- notifications non envahissantes ;
- état local/canonique/stale distingué ;
- diagnostics actionnables.

### 28.7 Tests

- ERT ;
- batch startup ;
- dépendances absentes ;
- serveur simulé ;
- parcours V1 ;
- performance événementielle.

---

## 29. Non-objectifs

- réimplémenter `locusolus.el` ;
- contenir des préférences personnelles ou chemins propres à un utilisateur ;
- stocker le graphe ;
- orchestrer les agents ;
- remplacer Canterel ;
- devenir un viewer IIIF ;
- synchroniser automatiquement toutes les notes personnelles ;
- exécuter des commandes sensibles sans confirmation ;
- fournir une application mobile ou Web.

---

## 30. Migration depuis `emacs-config`

### 30.1 Extraction

Identifier les fonctions génériques actuellement présentes dans les modules personnels et les déplacer par petits commits dans `apps/emacs`. Conserver temporairement des wrappers de compatibilité si des keybindings personnels les appellent.

### 30.2 Frontière finale

`apps/emacs` contient protocol client, buffers, commands, viewers, 3D bridge, sandbox inspector et intégrations génériques. `emacs-config` contient endpoint, auth-source, keybindings, layout et choix de viewers.

### 30.3 Test de séparation

Installer ce dépôt dans un Emacs vierge, lancer un serveur Locus mock, parcourir un programme et une branche, ouvrir une sandbox et une scène 3D sans charger aucun fichier `marcel-*`.

## 31. Définition finale

`apps/emacs` V1 est un cockpit expert générique et publiable pour Locus Solus. Il permet de piloter la recherche, examiner graphe, branches, agents, reviews, budgets, sandboxes et artefacts, intégrer Org/Magit/Jupyter/xiiif, et afficher les visualisations riches ou 3D via WebView lorsqu’il est disponible avec fallback navigateur, sans devenir lui-même la source de vérité ou un moteur d’exécution.
