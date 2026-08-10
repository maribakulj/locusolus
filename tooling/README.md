# `tooling/`

Automatisation du dépôt : les vérifications que la CI exécute et que l'on peut exécuter à la main.
Rien ici n'est livré au produit.

```text
lib/          utilitaires partagés entre vérifications
repo/         contrat de structure du dépôt (npm run check:repo)
boundaries/   frontières architecturales (npm run check:boundaries)
```

## Exécution

```bash
npm run check            # tout ce que la CI exécute, dans le même ordre
npm run check:format     # prettier
npm run check:repo       # structure du dépôt et cohérence du workspace
npm run check:boundaries # les cinq frontières de CLAUDE.md
npm run typecheck        # tsc --noEmit sur tooling/ et tests/
npm test                 # tests/**/*.test.ts
```

Chaque vérification est aussi un module importable — `check-repo.ts` n'est qu'une entrée CLI
au-dessus de `layout.ts`. C'est ce qui permet de les tester depuis `tests/` contre des arborescences
fabriquées, plutôt que de tester la CI en la lançant.

## `boundaries/` — la garde de frontières

Les cinq règles de `CLAUDE.md`, section « Frontières vérifiées par la CI », sous leur forme
opposable : `boundaries.json` à la racine du dépôt. Le fichier reprend le texte de chaque règle mot
pour mot dans `statement` ; s'il en diverge, `CLAUDE.md` fait foi.

Quatre règles se lisent dans les imports (`analyze.ts`, extraction par `imports.ts`), la cinquième
démarre un vrai Emacs (`emacs.ts`) parce qu'un paquet qui dépend en douce de l'`init.el` de son
auteur a l'air parfaitement autonome dans un diff.

Trois propriétés font la différence entre une garde et une décoration :

- **Une règle sans objet le dit.** `check:boundaries` imprime une ligne par règle, avec le nombre de
  fichiers réellement examinés. Sur un dépôt vide, la plupart annoncent zéro : c'est la différence
  entre « vérifiée » et « il n'y avait rien à vérifier ».
- **Un langage sans extracteur est un angle mort signalé, pas une dérogation.** Un fichier source
  dont l'extension n'est ni analysable ni ignorée fait échouer la CI. Le langage de `locusd` n'est
  pas tranché ; le jour où du code arrive dans un langage sans extracteur, il faut le savoir
  immédiatement.
- **Aucune règle n'est admise sans une violation délibérée qui la démontre.** Les fixtures de
  `tests/boundaries/fixtures/` sont des arborescences miniatures qui franchissent une frontière et
  déclarent le verdict attendu ; un test refuse qu'une règle du contrat n'en ait aucune.

Ajouter un langage : un extracteur dans `imports.ts`, son extension dans `boundaries.json` →
`extensions.analysable`, une fixture qui le met en défaut.

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
