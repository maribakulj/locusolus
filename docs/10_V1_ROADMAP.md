# Roadmap V1 multi-repos

Ce n'est pas une roadmap de MVP. Tous les workstreams appartiennent à la V1 finale ; l'ordre
reflète les dépendances de construction.

Granularité décroissante assumée. W0 et W2 sont découpés au commit près, parce qu'ils sont
immédiats et que leurs dépendances sont connues. W3 à W7 sont au groupe de commits. W8 à W12
restent au workstream : les découper finement aujourd'hui produirait un plan faux. **Chaque
workstream est redécoupé au commit près quand il devient le prochain**, dans son
`IMPLEMENTATION_STATE.md`.

La cible que cette V1 doit atteindre est énoncée dans `docs/13`, sur une échelle de dynamisme à cinq
niveaux : chaque workstream de W13 à W18 déclare le niveau qu'il fait franchir et sur quelle sorte de
relation de coordination. Un workstream qui ne saurait pas le dire n'aurait pas de critère de fin.

Un item est terminé quand son **test de sortie passe en CI**, pas quand le code est écrit.

Convention : `[R]` = réversible par `git revert` seul ; `[M]` = comporte une migration de schéma
ou de données et exige un plan de rollback dans le ledger.

---

## État de départ, vérifié

Ne pas repartir des suppositions du package d'origine — elles ont été confrontées aux dépôts
réels le 2026-08-07.

| Dépôt | État | Conséquence |
|---|---|---|
| `locusolus` | vide (LICENSE + README, commit `7dc4dd1`) | greenfield intégral ; rien à préserver, aucun audit antérieur |
| `canterel` | fork non divergé de `synthetic-sciences/OpenScience`, HEAD `c3f734c` | rename fait, rebrand interdit (ADR 0010), `src/locus/` absent |
| `xiiif` | v0.4.0 mûr, 23 `.el`, 33 fichiers de tests | l'essentiel de sa V1 existe ; six items déverrouillés |
| `emacs-config` | privé, non inventorié | l'inventaire est la première tâche de ce dépôt |

Décisions déjà tranchées : le projet s'appelle `locusolus` ; quatre dépôts, client Emacs dans
`apps/emacs/` (ADR 0009) ; fork suiveur pour Canterel (ADR 0010).

Un second constat, établi le 2026-08-17 : **§7.1 (agrégats d'agents et d'équipes), §13 (orchestrateur
de portefeuille), §16 (mémoire) et §20 (policy engine) ne sont couverts par aucun workstream.** W7
attrape le portefeuille de §13 par ses seuls indicateurs et la revue de §17 ; rien n'assigne
`AgentTemplate`, `AgentInstance`, `Team`, `Decision`, `ApprovalRequest`, les actions de §13.5,
l'anti-gaming de §13.6, les sept niveaux de mémoire, ni la DSL de politique. C'est un trou de
couverture, pas un choix : le dépôt sait décrire ce qui est cru et pourquoi, et ne sait rien dire de
qui travaille. W13 comble le socle, W14 à W18 portent la cible, et ADR 0016 en fixe les bornes.

Une décision reste ouverte : **le langage de `locusd`**. `SPEC_V1.md` §4.5 fixe TypeScript comme
référence en précisant que le choix est remplaçable. Il l'est moins qu'il n'y paraît, puisque le
worker vit dans un fork TypeScript et qu'un SDK TS existera donc de toute façon. Une autre langue
côté serveur est architecturalement propre — c'est le cas prévu par `docs/06` (« schemas JSON
versionnés et fixtures inter-SDK ») — mais double la suite de contract tests, qui devient alors le
vrai garde-fou du projet plutôt qu'un filet. W0 est agnostique : il produit d'abord les JSON
Schemas, communs aux deux options. La décision peut donc attendre la fin de W0, pas au-delà — W1
commence par du code de domaine.

---

## W0 — Baseline et contrats

Le seul workstream dont tout le reste dépend. Objectif : que `packages/protocol` existe, soit figé
en `lep/1.0`, et soit consommable par deux implémentations indépendantes.

| # | Commit | Dépôt | Test de sortie |
|---|---|---|---|
| W0.1 `[R]` | placement de la doc, `CLAUDE.md` par repo, ADR 0001–0010 | les 4 | `grep -r "locus-solus"` ne renvoie rien hors historique Git |
| W0.2 `[R]` | squelette monorepo : `apps/`, `packages/`, `schemas/`, `tests/`, tooling, CI qui passe à vide | locusolus | CI verte sur un dépôt sans code |
| W0.3 `[R]` | garde de frontières architecturales (les 5 règles du `CLAUDE.md`) branchée sur le squelette vide | locusolus | une violation délibérée fait échouer la CI |
| W0.4 `[R]` | `packages/protocol` : IDs, enveloppe d'erreur structurée, politique de versionnement, horodatage | locusolus | unitaires |
| W0.5 `[M]` | JSON Schemas LEP : `CapabilityManifest`, `MissionEnvelope`, `ContextView`, `EnvironmentBlueprint`, `SandboxSpec`, `ResourceSpec` | locusolus | les exemples de `schemas/examples/` valident |
| W0.6 `[M]` | JSON Schemas LEP, suite : `Lease`, `Attempt`, événements, `ArtifactManifest`, `RunManifest`, `SandboxAttestation`, `EpistemicCommit` | locusolus | validation |
| W0.7 `[R]` | corpus de fixtures : nominal, refus d'admission, reconnexion, résultat tardif, dépassement de budget | locusolus | chaque fixture valide **ou invalide intentionnellement**, selon son `expect` déclaré |
| W0.8 `[R]` | SDK généré depuis les schémas + `schema-registry` avec négociation de features au handshake | locusolus | round-trip sur toutes les fixtures |
| W0.9 `[R]` | `packages/testing` : **harness de conformance LEP côté serveur** — handshake, offre, lease, heartbeat, expiration, acquittements | locusolus | le harness se teste contre un worker factice |
| W0.10 `[R]` | `IMPLEMENTATION_LEDGER.md` dans les quatre dépôts | les 4 | présent, avec l'entrée d'étape 0 |

Fin de W0 : `lep/1.0` est gelé. Toute évolution suit `docs/06` — majeur = rupture, mineur = champs
optionnels compatibles.

---

## W2 — LEP et worker Canterel — **exécutable dès la fin de W0**

Numéroté W2 par dépendance logique, mais **il ne dépend pas de W1**. Le harness de W0.9 joue le
rôle de serveur. C'est la parallélisation principale du chantier : `locusd` part de zéro et
représente l'essentiel du travail, alors que le worker a un socle existant. Écrire le worker
contre un faux serveur oblige en outre le protocole à être suffisant avant que le vrai serveur
puisse compenser ses lacunes.

| # | Commit | Test de sortie |
|---|---|---|
| W2.1 `[R]` | remote `upstream` + `docs/locus/upstream.md` + politique de sync | un merge amont à blanc ne touche aucun fichier local |
| W2.2 `[R]` | non-régression standalone en CI (§28.8) — **avant tout code `locus/`** | passe sur le HEAD actuel |
| W2.3 `[R]` | `src/locus/{index,config,errors}.ts` + `canterel worker --locus` qui ne fait rien | `bun run check` vert ; standalone intact |
| W2.4 `[R]` | `identity.ts`, `auth.ts`, enrôlement, révocation (§7) | identité persistante après redémarrage |
| W2.5 `[R]` | `protocol.ts`, `schema-registry.ts`, `connection.ts` sur le SDK de W0.8 | contract tests contre le harness |
| W2.6 `[R]` | `capability-manifest.ts` + `capability-watch.ts` — détection réelle des toolchains, modèles, accélérateurs **et du niveau de sandbox effectif** | sur macOS : annonce `["S1","S2"]` et `mps`, jamais plus |
| W2.7 `[R]` | `registration.ts`, handshake complet | conformance §8.2 |
| W2.8 `[R]` | `admission.ts` — validation, refus structuré (§10.2), politique locale plus restrictive | la fixture de refus de W0.7 produit le bon code d'erreur |
| W2.9 `[R]` | `lease.ts`, `attempt.ts`, heartbeats, perte de lease (§11) | expiration et reprise contre le harness |
| W2.10 `[R]` | `context-materializer.ts` + isolation informationnelle (§12.4) | un contexte de branche A n'atteint jamais une mission de branche B |
| W2.11 `[R]` | `session-map.ts`, `agent-overlay.ts`, `model-policy.ts`, `tool-policy.ts` — couche d'adaptation vers l'amont, à garder mince | mission → session **sans modifier `src/session/`** |
| W2.12 `[R]` | `event-bridge.ts`, `event-spool.ts`, coalescence (§18) | perte de connexion : rien perdu, rien dupliqué |
| W2.13 `[R]` | `usage-meter.ts`, budget local, dépassement (§17) | arrêt propre au dépassement |
| W2.14 `[R]` | `artifact-client.ts`, `artifact-scanner.ts`, déclaration avant upload (§19.1) | hash déclaré ≠ hash reçu → rejet |
| W2.15 `[R]` | `epistemic-commit.ts` — jamais au-delà de `staged` (§2.3) | tentative de promotion → erreur structurée |
| W2.16 `[R]` | `recovery.ts`, `resume-store.ts`, offline, résultats partiels (§24) | redémarrage du worker en cours de mission |
| W2.17 `[R]` | `human-input.ts` (§22) | suspension sans processus coûteux maintenu |
| W2.18 `[R]` | `ui/worker-status.ts`, `mission-view.ts`, `security-view.ts` | rendu |
| W2.19 `[R]` | suite de conformance complète + consumer-driven contracts (§28.2/28.3) | verte contre le harness |

La liste de fichiers de `repos/canterel/SPEC_V1.md` §4 est une **annexe indicative**, pas un
gabarit. Ne crée pas 34 stubs vides : chaque commit ci-dessus livre une garantie testée, et les
fichiers apparaissent quand ils portent du comportement.

---

## W1 — Locus domain / event store — parallèle à W2

| Groupe | Contenu | Test de sortie |
|---|---|---|
| W1.a `[R]` | enveloppe commune d'objet épistémique (§7.4) : identité, version, statut, niveau de validation, portée de branche, provenance, supersession | property tests sur les invariants |
| W1.b `[R]` | agrégats organisationnels (§7.1) et objets épistémiques (§7.3) | property tests |
| W1.c `[M]` | `packages/event-store` : enveloppe (§10.1), append-only logique, concurrence optimiste | replay complet + conflit de concurrence détecté |
| W1.d `[M]` | projections reconstructibles | reconstruction depuis zéro = état courant |
| W1.e `[R]` | `packages/graph` : relations typées, **hyperarêtes** pour les inférences multi-prémisses (§7.6) | une inférence à 3 prémisses n'est pas 3 liens |
| W1.f `[R]` | validation épistémique (§8) et propagation de l'invalidation (§8.3) | invalider une prémisse propage correctement |
| W1.g `[R]` | résultats négatifs et conflits (§18.7) | aucun chemin de code ne supprime un conflit |
| W1.h `[M]` | migrations de schéma + tests de portabilité | migration aller-retour |

---

## W3 — Workflow abstraction

Backend déterministe de test **avant** Temporal (ADR 0003). Si Temporal vient en premier, le
domaine s'y adapte silencieusement et l'ADR devient une intention.

`W3.a` définitions indépendantes du backend + règles de déterminisme (§11.3) · `W3.b` port
`WorkflowBackend` + backend déterministe de test · `W3.c` workflows obligatoires (§11.2) sur le
backend de test · `W3.d` backend Temporal, sous `packages/workflow-backends` uniquement ·
`W3.e` crash/restart/replay sur les deux backends ; compensation qui n'efface aucun fait observé.

---

## W4 — Execution Fabric

`W4.a` `SandboxSpec`, `ResourceSpec`, `SandboxAttestation` · `W4.b` **suite de self-tests indexée
par niveau S0–S4**, chaque test déclarant le niveau auquel il doit échouer côté sandbox ·
`W4.c` `locus-execd`, seul détenteur du socket runtime · `W4.d` backend Linux rootless, cgroups v2,
seccomp · `W4.e` backend macOS : VM Linux légère + containers rootless par mission ·
`W4.f` worker macOS de confiance annonçant `mps` · `W4.g` scheduler : placement par capability +
trust + localité + fit + budget, admission, refus, reroutage.

W4.b avant W4.c : la suite de tests définit ce que « sandbox » veut dire dans ce projet, et un
backend qui échoue un test critique n'est pas `trusted`.

**W4.d se découpe en deux commits**, sur la forme d'ADR 0012 (le port avant le driver) et d'ADR 0015
(la traduction avant le fil). Elle vaut ici plus qu'ailleurs : le plan de rollback d'ADR 0004 dit
qu'il n'y a « aucun chemin de repli acceptable », et un driver écrit avant sa traduction confinerait
de travers en silence.

| # | Commit | Test de sortie |
|---|---|---|
| W4.d.1 `[R]` | la traduction `SandboxSpec` → plan de confinement rootless, et la lecture de ce que l'hôte permet | le plan ne concède jamais plus que le niveau exigé — monter d'un niveau ne relâche rien et change quelque chose — il refuse par leur nom la micro-VM, l'enclave et un mode réseau sans namespace pour le porter, et la lecture de l'hôte nomme ce qui manque au lieu de le supposer |
| W4.d.2 `[R]` | le driver rootless : `RuntimePort` implémenté sur Podman rootless (`docs/03`), la sandbox créée, l'attestation lue de ce qui tourne | le driver demande au runtime exactement ce que le plan a décidé, et il atteste de ce qu'il **observe** — un confinement plus faible que demandé apparaît dans l'attestation et `conformance` le refuse |
| W4.d.3 `[R]` | la suite de W4.b passée contre le backend : une commande par sonde, et le `Standing` qui en sort | la suite rend un `Standing` pour ce backend ; une sonde non exécutée est `NotRun` avec sa raison, jamais un succès, et un hôte sans runtime n'obtient jamais `Trusted` |
| W4.d.4 `[R]` | la vérification du profil seccomp restreint apporté par le déploiement | un profil qui ne refuse pas ce que la posture promet est refusé, et le refus nomme **tous** les appels manquants |

Le plafond de ce backend est `S3` et c'est une constante, pas une ambition : `S4` est une micro-VM
et `S5` une enclave distante.

**W4.e n'ouvre pas `S4`.** `docs/03` recommande sur macOS « host macOS + VM Linux légère + containers
rootless par mission » : le conteneur tourne dans un noyau Linux, donc le plan de confinement est
celui de W4.d.1, et ce qui change est **où on lit les faits** — dans l'invité, pas sur l'hôte. Une VM
partagée entre les missions ne tient pas la promesse de `S4 microvm-high-risk`, qui est qu'une mission
à haut risque ait **son propre** noyau. Le jour où un déploiement créera une VM par mission, ce sera
un autre backend, avec son propre plafond et sa propre suite.

| # | Commit | Test de sortie |
|---|---|---|
| W4.e.1 `[R]` | la machine macOS : son état, la lecture des faits **dans l'invité**, et le plafond qui en découle | les faits viennent du noyau qui confine et non de celui qui appelle ; une machine arrêtée se distingue d'un noyau incapable ; une VM partagée ne fait jamais franchir `S3` |
| W4.f.1 `[R]` | la portée de l'accélérateur : `mps` n'existe qu'en natif, donc hors du conteneur | sur un hôte natif, une mission a le conteneur **ou** l'accélérateur, jamais les deux, et le refus dit lequel des deux il faut lâcher — distinct de « accélérateur absent », qui appelle une autre action |
| W4.g.1 `[R]` | le placement : choisir parmi plusieurs candidats, sur ce que chacun a **prouvé** | un hôte ne reçoit que le niveau qu'il a prouvé tenir ; le refus dit ce qui manquait à **chaque** candidat ; deux placements du même journal placent au même endroit |
| W4.g.2 `[R]` | le reroutage : une mission dont l'hôte tombe, une mission refusée partout | à écrire quand W4.g.1 est mergé |

La posture seccomp **restreinte** promet le refus, depuis l'intérieur, de la création de namespaces
et du chargement de code noyau. Ce dépôt ne **fournit pas** ce profil : un profil par défaut-refus
est une liste de plusieurs centaines d'appels autorisés dont l'exactitude ne se démontre qu'en
l'exécutant contre des charges réelles, et en écrire un sans hôte pour l'éprouver produirait soit une
sandbox qui casse tout, soit une sandbox qui autorise ce qu'elle prétend refuser. Le déploiement
l'apporte, W4.d.4 le **vérifie**, et tant qu'aucun profil vérifié n'est fourni le backend refuse les
niveaux qui en dépendent — c'est la règle du plafond `S3`, appliquée à une capacité que l'opérateur
apporte.

---

## W5 — Toolchains

`EnvironmentBlueprint`/Builder ; chaîne lockfile → build OCI → SBOM → scan → health checks →
signature → digest ; profils `base`, `python-science`, `math-formal`, `ml-cpu`, `browser`, `dh` ;
capabilities MPS/CUDA de worker. Les fichiers de `templates/environment/` sont le point de départ.

## W6 — Artifact / reproductibilité

Object store, manifests, quarantaine/promotion, `RunManifest`, workflows de reproduction.

## W7 — Memory / review / portfolio

`ContextView`, retrieval hybride, revue indépendante, budgets, scheduler qualité-diversité.

Deux points faciles à rater et coûteux à réparer : la prévention de contamination (§16.6) doit
être testée par un cas adverse explicite et pas seulement par construction ; l'anti-gaming du
portefeuille (§13.6) doit exister avant que la fonction de valeur pilote des décisions
automatiques.

**Dépend de W13.c et W13.d** (ADR 0016, décision 13) : la revue indépendante suppose des instances
d'agent distinctes et une assignation, sans quoi « qui relit qui » n'a pas d'objet où s'écrire.

## W8 — Clients

Web workspace + `apps/emacs` (monorepo) ; sandbox inspector ; decisions ; artifacts.

**Premier commit : le test de séparation** (`emacs -Q` avec la seule `load-path` du package). Il
fixe la frontière avant qu'il y ait quoi que ce soit à séparer — le seul moment où c'est gratuit.

Ordre : client/événements → dashboard et buffers → commandes et transient → artefacts et
inspecteur de sandbox → intégrations Org/Magit/Jupyter/xiiif → 3D et WebView. Si l'inventaire de
`emacs-config` révèle du code client réutilisable, il s'insère ici en tête sans réordonner l'amont.

## W9 — Visualization

Service de projection, 2D, Three.js 3D, viewer registry, pont xwidget/navigateur.

Contrainte à retenir dès maintenant : le service produit une **projection**, jamais une copie
mutable du graphe. Si une vue devient éditable en place, l'invariant « aucun frontend n'écrit
directement dans le graphe » est perdu.

## W10 — xiiif — **déverrouillé aujourd'hui**

Six items ne dépendent d'aucun autre dépôt : dispatcher `xiiif-open`, alias d'API §15, sélection
numérique de région, politique d'URL, limites de taille et de redirections, bridge OpenSeadragon.
Bon travail de repli quand une décision bloque ailleurs.

Bloqué sur W0.6 : `RemoteArtifactRef`, `xiiif-open-locus-artifact`, affichage séparé identité /
live / snapshot / intégrité / divergences (§19), revue humaine (§20).

## W11 — Deployment profiles

Local, personal-node, VM, adapter cloud, hybride distribué ; backup/restore/migration.

## W12 — Evaluation / release

Tests de sécurité, injection de fautes, endurance, benchmarks, ablations, docs, release candidate.

---

## W13 — Socle de coordination agentique — **repli, jamais prioritaire** — niveau 3 sur `review`

Couvre le socle de §7.1 et les deux seuls manques de §14 (ADR 0016, décision 3). Ne prend jamais la
priorité sur W4 ; les périmètres ne se recoupent pas. Aucun item ne modifie `canterel`. W13.c et
W13.d sont néanmoins des **dépendances de W7** : « repli » ordonne, il ne déprogramme pas.

Les modes `observed` et `assisted` (ADR 0016, décision 8) existent dès W13.e. Le mode fermé est une
exigence de §33, pas une précaution.

| # | Commit | Test de sortie |
|---|---|---|
| W13.a `[R]` | ADR 0016, sixième frontière (`CLAUDE.md` + `boundaries.json` + garde), ce workstream, `docs/11`, `docs/13` | une violation délibérée **dans chacun des deux sens** fait échouer la CI, et la garde déclare le nombre de fichiers réellement examinés |
| W13.b `[R]` | pli des fixtures `lep/1.0` en graphe d'exécution, sous `tests/` — aucun champ ajouté au protocole | le pli rend un graphe attempt/outil/artefact **sans arête orpheline**, et un test affirme **par l'absence** qu'aucun champ d'agent n'existe dans l'événement LEP |
| W13.c `[R]` | `packages/coordination` : `AgentTemplate`, `AgentInstance`, `Team`, `Decision`, `ApprovalRequest` selon §7.1 ; `Id<Team>`, `Id<Task>`, `Id<Decision>`, `Id<Approval>` dans `packages/protocol` | property test : la capacité effective est l'**intersection** des quatre sources de §14.2, jamais leur union ; ignorer une source fait rougir. Round-trip des quatre nouveaux identifiants |
| W13.d `[R]` | complétion de l'agrégat `Task` de §7.1 — dont `assigned_agent_id` et `assigned_worker_id` — sans toucher la machine à états existante | l'assignation est un événement ; la machine à états de `task.rs` est inchangée et ses tests passent |
| W13.e `[R]` | relation de coordination (`kind` fermé à `review`), payload de `team.modify`, CAS par `expected_revision`, annulation par commit inverse, autorité de proposition agentique | quatre : deux propositions concurrentes sur la même base ne committent pas toutes deux et le refus dit s'il faut rebaser ; une proposition sans justification citant un objet épistémique existant est refusée ; aucun chemin de code ne modifie une `MissionEnvelope` émise ni le hash de sa `ContextView` ; une proposition d'origine agentique suit le même chemin qu'une proposition humaine et son proposeur ne peut pas l'approuver |
| W13.f `[R]` | `packages/projections` : projection du graphe d'exécution | reconstruction depuis zéro = état courant ; quarantaine conforme à ADR 0013 |
| W13.g `[R]` | projection du graphe organisationnel réalisé, par jointure `assigned_agent_id` × événements | **dépend de W13.b et W13.d.** Le graphe se reconstruit depuis le journal seul ; aucun instantané n'est reçu du worker |

W13.b avant W13.f : le pli décide si la projection s'écrit contre `lep/1.0` inchangé, et le découvrir
après aurait coûté la projection. W13.c avant W13.e : une relation entre deux agents suppose que
l'agent soit un objet. W13.d avant W13.g : c'est l'assignation qui rend le graphe réalisé dérivable,
l'événement LEP ne portant aucun champ d'agent.

§18 est à lire avant W13.e : la coordination d'une branche interagit avec le fork, le merge et le
rebase, et §18.4 pourrait imposer des règles sur ce qu'une fusion fait d'une équipe.

Deux pièges qui coûteraient une garantie chacun : ajouter une variante à `ObjectionTarget` ou à toute
énumération de `packages/graph` ; ouvrir l'énumération des sortes de relation à une valeur qu'aucun
consommateur exécutable n'honore.

## W14 — Moteur de politique et orchestrateur de portefeuille

§20 en entier et §13 en entier — le plus large trou du dépôt, et déjà normatif. DSL déclarative
versionnée ; séparation des faits et de la décision ; trace d'évaluation ; cinq verbes ; détection des
conflits de politiques ; priorité explicite ; dry-run ; déterminisme à entrées identiques ;
conservation des overrides. `Delegation` et sa révocation. Explicabilité de §20.5, y compris les
**alternatives rejetées**. Côté portefeuille : les quinze indicateurs de §13.2, la qualité-diversité de
§13.3, `V(b)` de §13.4, les actions de §13.5, et l'**anti-gaming de §13.6**.

Attend W13.c et W13.e. Débloque les modes `bounded` et `operator`, la classe de risque dérivée, et la
moitié de W7. §W7 avertit déjà que l'anti-gaming doit exister avant que la fonction de valeur pilote
des décisions automatiques.

## W15 — Cœur du graphe agentique et contestabilité — **niveau 3 → 4, structure**

Généralisation de la relation unique de W13.e : version canonique immuable avec hash et parent, diff
comme objet de première classe, régions mutables bornées à la façon de GRAFT — région déclarée,
acceptation locale, veto de cohérence globale. L'énumération des sortes s'ouvre **une valeur à la
fois** (ADR 0016 décision 4) ; `role` exerce la clause de falsification de la décision 10, puis
`visibility` dont le consommateur est la construction de `ContextView`. Jeu d'opérations cible :
`ADD_NODE`, `REMOVE_NODE`, `REPLACE_NODE`, `ADD_EDGE`, `REMOVE_EDGE`, `SPLIT_NODE`, `MERGE_NODES`,
`SET_ROLE`, `SET_VISIBILITY`, `SET_VALIDATOR`, `SET_EXECUTION_ORDER`.

Contestabilité des décisions de coordination : famille d'objection parallèle à celle du domaine
épistémique, même forme logique, domaines disjoints, avec un test vérifiant l'**absence** de
conversion. C'est la contribution originale du projet et le geste le plus facile à faire de travers.

Attend W14. Un IR déclaratif contraint, jamais un script : la représentation détermine ce qui est
vérifiable.

## W16 — Reconfiguration vivante et scheduler dynamique — **niveau 4**

Le scheduler doit savoir spawn, suspend, drain, kill, replace, split, merge, connect, disconnect,
rerouter l'état, rejouer, migrer le contexte, et livrer les messages **en connaissance de la version**.
Barrières par invariant menacé plutôt que par lieu ; quiescence locale d'un nœud plutôt que drain
global. Epochs, messages tardifs et transfert d'état : ils n'ont un problème réel à résoudre qu'une
fois une messagerie inter-agents existante. Visibilité institutionnelle facultative des sous-agents
internes du harnais — le cas de W16 justifiant un mineur LEP, avec son ADR.

Plan de simulation : rejeu déterministe, substitut d'environnement enregistré, ombre en sandbox réelle,
canari facultatif. Un objet simulé n'existe pas comme type dans le domaine épistémique.

Attend W15, W4.e et W4.g.

## W17 — Cockpit et orchestration de la mémoire

Cockpit à quatre vues — plan, vivant, trace, épistémique — avec sélection synchronisée par `Id<Agent>`.
Le canvas produit une commande, jamais une écriture. Diff calculé une fois côté serveur, donc identique
dans Emacs et dans le web. Preview statique, ombre, approbation, rollback, navigation dans le temps
comme propriété du pli. Se branche sur le Visualization Projection Service de §23.3, dont la vue
« société d'agents » est déjà nommée, et consomme `/branches/:id/diff` de §22.4.

Mémoire : les sept niveaux de §16.1 ; le retrieval hybride de §16.3 avec ranking dont les facteurs sont
exposés et embeddings ne contournant pas les ACL ; l'index mémoire hybride comme projection obligatoire
de §9.3 ; la déduplication non automatique de §16.4 ; la compaction de §16.5 ; les cinq préventions de
§16.6. Deux retrievals séparés, épistémique et organisationnel, sans conversion.

Attend W16, W9 et `locusd`. Aucun outil existant ne combine canvas, graphe comme état mutable de
première classe, mutations proposées par les agents, commit atomique et invariants.

## W18 — Adaptation automatique et admission de capacité

Boucle rapide sur la capacité — routage de modèle, choix d'outil, sélection de skill, retry, routes
éphémères ; boucle lente sur la structure. Les onze déclencheurs de §14.5 et les indicateurs de §13.2.
`bounded` sur les seules opérations dont la classe de risque est **dérivée** des invariants menacés.

Admission de capacité : proposition, politique et approbation faisant entrer une capacité nouvelle sous
forme d'`EnvironmentBlueprint` construit, scanné, signé et attesté (§19.3), jamais sous forme de code
injecté. Attend W5, W6 et S3/S4 attesté.

Métrique d'acceptation propre : le taux d'annulation humaine des adaptations agentiques.

## Recherche — sans dépendance de chemin critique, abandonnable sans coût

`R1` **consensus circulaire** — cycle de `Cites` sans `AnchoredIn` externe, exigé par §16.6, calculable
sur les types existants de `packages/graph`. Le moins cher des items de recherche, ne dépend ni de W4
ni de `locusd`, publiable seul. · `R2` **crédit structurel** — attribuer une amélioration à une
relation, un rôle, un budget ou au hasard d'échantillonnage ; sans cela un système évolutionnaire
accumule des changements inutiles. · `R3` **évaluation structurelle** — métriques de graphe et regret
structurel `R_s = U(meilleur candidat disponible) − U(graphe choisi)`, calculable en **rejeu** sur
fixtures identiques. · `R4` **substitut d'environnement puis world model** — générateur de trajectoires
contrefactuelles, comparatif à graine et préfixe identiques, unilatéral en rejet, jamais un juge,
jamais une preuve ; et la fidélité sur les environnements du domaine — IIIF, SPARQL, ALTO/PAGE,
notebooks, prouveurs — est inconnue. · `R5` **prototype externe de harnais tiers** — dépôt jetable, une
seule question : un `SessionPlan` peut-il produire un flux conforme au harnais de conformance de W0.9 ?
Si oui, un worker LEP séparé ; si non, on supprime le dépôt. Aucune ligne dans
`backend/cli/src/locus/` avant la réponse. · `R6` **évolution inter-exécutions** — une adaptation
récurrente et gagnante en validation appariée propose une amélioration de template.

---

## Règle de session

Lire ce fichier, prendre le premier item non terminé dont les dépendances sont satisfaites, lire
le code concerné, exécuter les tests de son périmètre, modifier **ce périmètre seul**, mettre à
jour `IMPLEMENTATION_LEDGER.md`.
