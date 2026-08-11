# Carte des dépôts et frontières

## 1. `locusolus` — nouveau

Autorité architecturale. Contient domaine, LEP, orchestration, graphe, mémoire, reviews, artifact registry, budgets, Execution Fabric, toolchain registry, déploiements, web workspace et visualization service.

## 2. `canterel` — rename de `openscienceDH`

Runtime scientifique. Conserve providers, sessions, agents, tools, skills, sandbox locale existante, workspace web de session. Ajoute LEP worker, environment awareness, artifact manifests et telemetry. Ne possède pas le programme de recherche global.

## 3. `xiiif` — existant

Viewer/éditeur IIIF Emacs humain. Ne devient pas un service requis par les agents.

## 4. `locusolus/apps/emacs` — nouveau, dans le monorepo

Client Emacs produit, versionnable et publiable. Cockpit, command/query client, events, artifact viewers et 3D embedded/external.

## 5. `emacs-config` — existant

Config personnelle uniquement.

## Dépendances autorisées

```text
locusolus/apps/emacs ──HTTP/Events──> locusolus/apps/locusd
canterel ─────────────LEP─────────> locusolus
xiiif ──optional adapter──────────> locusolus/apps/emacs
emacs-config ──configures─────────> locusolus/apps/emacs + xiiif

locusolus --DOES NOT IMPORT--> canterel internals
locusolus --DOES NOT REQUIRE--> xiiif
```
