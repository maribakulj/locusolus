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

Un seul existe à ce jour : `protocol` (crate `locus-protocol`), qui porte les primitives de LEP —
identifiants typés, horodatage, enveloppe d'erreur structurée, versionnement. Les crates Rust sont
nommés `locus-<répertoire>`, les packages npm `@locus/<répertoire>` ; les deux conventions sont
vérifiées par `check:repo`.

## Ordre de construction

`domain`, `protocol` et `event-store` d'abord, avec des ports purs. Temporal, les containers et le
cloud ne se branchent qu'après les interfaces et les contract tests. `packages/protocol` est le
goulot du projet entier : il se fige en `lep/1.0` avant que deux consommateurs en dépendent.

## Frontières

`packages/domain` n'importe aucun package d'infrastructure. `@temporalio/*` ne vit que sous
`packages/workflow-backends`. Aucun client PostgreSQL hors `packages/event-store` et des
projections.

Les trois règles sont opposables : `npm run check:boundaries`, exécuté en CI. Leur forme exécutable
est `boundaries.json`, leur énoncé est dans `CLAUDE.md`, section « Frontières vérifiées par la CI ».
Un chemin relatif qui sort du package est ramené au nom du package visé avant d'être confronté aux
règles : `../../event-store/src/client.ts` ne contourne rien.
