# Roadmap V1 multi-repos

Ce n'est pas une roadmap de MVP. Tous les workstreams appartiennent à la V1 finale ; l'ordre
reflète les dépendances de construction.

Granularité décroissante assumée. W0 et W2 sont découpés au commit près, parce qu'ils sont
immédiats et que leurs dépendances sont connues. W3 à W7 sont au groupe de commits. W8 à W12
restent au workstream : les découper finement aujourd'hui produirait un plan faux. **Chaque
workstream est redécoupé au commit près quand il devient le prochain**, dans son
`IMPLEMENTATION_STATE.md`.

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

## Règle de session

Lire ce fichier, prendre le premier item non terminé dont les dépendances sont satisfaites, lire
le code concerné, exécuter les tests de son périmètre, modifier **ce périmètre seul**, mettre à
jour `IMPLEMENTATION_LEDGER.md`.
