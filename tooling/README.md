# `tooling/`

Automatisation du dépôt : les vérifications que la CI exécute et que l'on peut exécuter à la main.
Rien ici n'est livré au produit.

```text
lib/          utilitaires partagés entre vérifications
repo/         structure du dépôt et nom retiré (check:repo, check:naming)
boundaries/   frontières architecturales (check:boundaries)
```

## Exécution

```bash
npm run check            # tout ce que la CI exécute, dans le même ordre
npm run check:format     # prettier
npm run check:repo       # structure du dépôt et cohérence du workspace
npm run check:naming     # aucune occurrence non justifiée du nom retiré
npm run check:boundaries # les cinq frontières de CLAUDE.md
npm run typecheck        # tsc --noEmit sur tooling/ et tests/
npm test                 # tests/**/*.test.ts
```

Chaque vérification est aussi un module importable — `check-repo.ts` n'est qu'une entrée CLI
au-dessus de `layout.ts`. C'est ce qui permet de les tester depuis `tests/` contre des arborescences
fabriquées, plutôt que de tester la CI en la lançant.

## `repo/naming.ts` — le nom retiré

`docs/10_V1_ROADMAP.md` énonce le test de sortie de W0.1 : « `grep -r "locus-solus"` ne renvoie rien
hors historique Git ». Pris à la lettre, il ne peut jamais passer — les documents qui **consignent**
le renommage citent forcément l'ancien nom : la roadmap cite son propre test, l'ADR 0009 nomme ce à
quoi il se substitue, le ledger consigne le renommage.

La garde interdit donc toute occurrence et exige que chacune des survivantes soit nommée dans
`historicalMentions` avec sa raison. Une mention que personne n'a justifiée est un résidu ; une
justification à laquelle plus aucune mention ne correspond est caduque, et signalée aussi.

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
  dont l'extension n'est ni analysable ni ignorée fait échouer la CI. La propriété a servi : les
  premiers fichiers Rust ont fait échouer la CI avant que leur extracteur existe, au lieu de passer
  en silence.
- **Aucune règle n'est admise sans une violation délibérée qui la démontre.** Les fixtures de
  `tests/boundaries/fixtures/` sont des arborescences miniatures qui franchissent une frontière et
  déclarent le verdict attendu ; un test refuse qu'une règle du contrat n'en ait aucune.

Ajouter un langage : un extracteur dans `imports.ts`, son extension dans `boundaries.json` →
`extensions.analysable`, une fixture qui le met en défaut. Si le langage a un manifeste de
dépendances, un lecteur dans `manifests.ts` — une dépendance déclarée compte comme un import, parce
qu'elle est le moment où quelqu'un a décidé.

Langages couverts : TypeScript/JavaScript (via le scanner TypeScript), Rust (`use`, `extern crate`,
`Cargo.toml`), Emacs Lisp (`require`). **Go reste un angle mort signalé** — du code `.go` fait
échouer la CI tant que personne n'a écrit son extracteur.

Les chemins Rust sont normalisés `::` → `/`, pour que les motifs de `boundaries.json` s'écrivent
dans une seule syntaxe quel que soit le langage : `std::fs::File` devient `std/fs/File`, que le
motif `std/fs` attrape par la même règle de sous-chemin qui fait que `pg` attrape `pg/lib/pool`. Un
crate dont le nom porte un tiret est émis sous ses deux orthographes, Cargo l'écrivant
`tokio-postgres` et le code Rust `tokio_postgres`.

## Choix technique, et ce qu'il ne décide pas

Node.js LTS + TypeScript, exécuté directement par Node (« type stripping », d'où
`erasableSyntaxOnly` dans `tsconfig.base.json` : le compilateur refuse la syntaxe que Node ne sait
pas effacer). Pas d'étape de build, pas d'artefact compilé, une seule dépendance de runtime : Node
lui-même.

Ce choix porte sur **l'outillage du dépôt**, pas sur le produit — et le produit a tranché autrement
: ADR 0011 met `locusd`, `locus-execd` et la CLI en Rust. L'outillage reste en TypeScript parce
qu'il n'a aucune raison de changer : il lit des fichiers et rend des `Finding[]`, il n'est livré à
personne, et il doit savoir lire tous les langages du dépôt sans appartenir à aucun.

## Ajouter une vérification

Un module pur qui prend une racine de dépôt et rend des `Finding[]`, une entrée CLI qui l'imprime
via `lib/findings.ts` et fixe le code de sortie, un test dans `tests/` qui la met en défaut sur une
arborescence fabriquée, une ligne dans `package.json` et une étape dans `.github/workflows/ci.yml`.
Une vérification sans cas de violation dans `tests/` n'est pas terminée.
