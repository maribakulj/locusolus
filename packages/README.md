# `packages/`

Bibliothèques. Une par répertoire, sans point d'entrée exécutable : ce qui se déploie vit dans
`apps/`.

Le découpage visé par `docs/SPEC_V1.md` §5 sépare le domaine (`domain`, `application`), les contrats
(`protocol`), l'infrastructure (`event-store`, `graph`, `workflow-backends`, `execution`,
`telemetry`) et les capacités métier (`scheduler`, `portfolio`, `policies`, `reviews`, `memory`,
`artifacts`, `budgets`, `identity`, `environments`, `toolchains`, `visualization`, `federation`,
`testing`).

C'est une annexe indicative, pas un gabarit à instancier. **Un package apparaît quand il porte une
garantie testée** ; un répertoire vide fait échouer `npm run check:repo`.

## Ordre de construction

`domain`, `protocol` et `event-store` d'abord, avec des ports purs. Temporal, les containers et le
cloud ne se branchent qu'après les interfaces et les contract tests. `packages/protocol` est le
goulot du projet entier : il se fige en `lep/1.0` avant que deux consommateurs en dépendent.

## Frontières

`packages/domain` n'importe aucun package d'infrastructure. `@temporalio/*` ne vit que sous
`packages/workflow-backends`. Aucun client PostgreSQL hors `packages/event-store` et des
projections. Les règles de frontière opposables sont listées dans `CLAUDE.md`, section « Frontières
vérifiées par la CI ».
