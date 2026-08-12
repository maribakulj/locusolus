# `tooling/`

Automatisation du dépôt : les vérifications que la CI exécute et que l'on peut exécuter à la main.
Rien ici n'est livré au produit.

```text
lib/    utilitaires partagés entre vérifications
repo/   contrat de structure du dépôt (npm run check:repo)
```

## Exécution

```bash
npm run check          # tout ce que la CI exécute, dans le même ordre
npm run check:format   # prettier
npm run check:repo     # structure du dépôt et cohérence du workspace
npm run typecheck      # tsc --noEmit sur tooling/ et tests/
npm test               # tests/**/*.test.ts
```

Chaque vérification est aussi un module importable — `check-repo.ts` n'est qu'une entrée CLI
au-dessus de `layout.ts`. C'est ce qui permet de les tester depuis `tests/` contre des arborescences
fabriquées, plutôt que de tester la CI en la lançant.

## Choix technique, et ce qu'il ne décide pas

Node.js LTS + TypeScript, exécuté directement par Node (« type stripping », d'où
`erasableSyntaxOnly` dans `tsconfig.base.json` : le compilateur refuse la syntaxe que Node ne sait
pas effacer). Pas d'étape de build, pas d'artefact compilé, une seule dépendance de runtime : Node
lui-même.

Ce choix porte sur **l'outillage du dépôt**, pas sur le produit. `docs/SPEC_V1.md` §4.5 donne
TypeScript et Node LTS comme technologies de référence, et un SDK TypeScript existera de toute façon
puisque le worker vit dans un fork TypeScript. Le langage d'implémentation de `locusd` reste une
décision ouverte (`docs/10_V1_ROADMAP.md`, « État de départ ») : si elle tombe sur un autre langage,
cet outillage ne change pas, et les unités concernées apportent le manifeste et la chaîne de build
de leur écosystème.

## Ajouter une vérification

Un module pur qui prend une racine de dépôt et rend des `Finding[]`, une entrée CLI qui l'imprime
via `lib/findings.ts` et fixe le code de sortie, un test dans `tests/` qui la met en défaut sur une
arborescence fabriquée, une ligne dans `package.json` et une étape dans `.github/workflows/ci.yml`.
Une vérification sans cas de violation dans `tests/` n'est pas terminée.
