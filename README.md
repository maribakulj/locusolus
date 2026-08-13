# locusolus

Locus Solus — laboratoire et control plane pour le travail scientifique agentique. Monorepo :
domaine, event store, graphe épistémique, Execution Fabric, cockpit web et cockpit Emacs.

Le runtime scientifique qui exécute les missions est un dépôt séparé (`canterel`) ; il parle à ce
control plane via LEP, le protocole d'exécution défini ici.

## État

Le corpus normatif du chantier, l'outillage, la CI, et le premier package produit :
`packages/protocol`, qui porte les primitives sur lesquelles LEP sera écrit — identifiants,
horodatage, enveloppe d'erreur, versionnement.

## Par où commencer

`CLAUDE.md` — invariants et règles de ce dépôt. `START_HERE_CLAUDE.md` — ordre de lecture, règle de
priorité en cas de contradiction, et les trois erreurs à ne pas commettre. `docs/10_V1_ROADMAP.md`
est la feuille de route : un item n'y est terminé que quand son test de sortie passe en CI, et
`IMPLEMENTATION_LEDGER.md` dit lesquels le sont.

Les ADR arbitrent. En cas de contradiction : ADR → `docs/00` → `docs/01` → `docs/DECISIONS.md` →
`docs/SPEC_V1.md` → le reste.

## Structure

```text
apps/       unités déployables (locusd, locus-execd, cli, web, emacs…)
packages/   bibliothèques (domain, protocol, event-store, graph…)
schemas/    JSON Schemas versionnés — les contrats de fil
docs/       corpus normatif : spec, ADR, roadmap, matrice d'acceptation
templates/  gabarits d'environnement et de profil de déploiement
tests/      suites transverses au dépôt
tooling/    automatisation du dépôt exécutée par la CI
```

`docs/SPEC_V1.md` §5 donne la structure normative complète. Les listes de packages et d'apps y sont
des **annexes indicatives** : un répertoire apparaît quand il porte une garantie testée, et un
répertoire vide sous `apps/` ou `packages/` fait échouer la CI.

## Développement

Rust, épinglé dans `rust-toolchain.toml` — rustup l'installe seul. Node.js LTS pour l'outillage du
dépôt, épinglé dans `.nvmrc`. Emacs pour la cinquième frontière.

```bash
npm ci
npm run check   # format, structure, frontières, typecheck, tests JS puis Rust — ce que la CI exécute
```

Les cinq frontières architecturales de `CLAUDE.md` sont opposables, pas déclaratives : leur forme
exécutable est `boundaries.json` et `npm run check:boundaries` les fait échouer. Le corpus de
`docs/`, `templates/` et `schemas/examples/` est reçu du paquet de handoff et placé à l'octet près ;
il est exclu du formatage pour cette raison.

Les vérifications sont détaillées dans `tooling/README.md`.

## Licence

Apache-2.0. Voir `LICENSE`.
