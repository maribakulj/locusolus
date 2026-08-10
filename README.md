# locusolus

Locus Solus — laboratoire et control plane pour le travail scientifique agentique. Monorepo :
domaine, event store, graphe épistémique, Execution Fabric, cockpit web et cockpit Emacs.

Le runtime scientifique qui exécute les missions est un dépôt séparé (`canterel`) ; il parle à ce
control plane via LEP, le protocole d'exécution défini ici.

## État

Squelette. Le dépôt porte son outillage, sa configuration de workspace et sa CI ; il ne porte pas
encore de code produit. La feuille de route est `docs/10_V1_ROADMAP.md`, et un item n'y est terminé
que quand son test de sortie passe en CI.

## Structure

```text
apps/       unités déployables (locusd, locus-execd, cli, web, emacs…)
packages/   bibliothèques (domain, protocol, event-store, graph…)
schemas/    JSON Schemas versionnés — les contrats de fil
tests/      suites transverses au dépôt
tooling/    automatisation du dépôt exécutée par la CI
```

`docs/SPEC_V1.md` §5 donne la structure normative complète. Les listes de packages et d'apps y sont
des **annexes indicatives** : un répertoire apparaît quand il porte une garantie testée, et un
répertoire vide sous `apps/` ou `packages/` fait échouer la CI.

## Développement

Node.js LTS, épinglé dans `.nvmrc`. Aucune autre dépendance système.

```bash
npm ci
npm run check   # format, structure du dépôt, typecheck, tests — ce que la CI exécute
```

Les vérifications sont détaillées dans `tooling/README.md`.

## Licence

Apache-2.0. Voir `LICENSE`.
