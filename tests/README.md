# `tests/`

Suites transverses : celles qui portent sur le dépôt entier ou sur la frontière entre plusieurs
unités. Les tests unitaires d'un package vivent dans ce package, pas ici.

`npm test` exécute `tests/**/*.test.ts` avec le lanceur intégré de Node.

## Suites présentes

- `repo/` — le contrat de structure du dépôt lui-même : racines normatives, cohérence du workspace,
  absence de répertoires-stubs.
- `boundaries/` — les cinq frontières architecturales. Chaque règle est mise en défaut par une
  arborescence de `fixtures/` qui la franchit délibérément et déclare le verdict attendu ; un test
  refuse qu'une règle du contrat n'en ait aucune. Les fixtures Emacs demandent un `emacs` sur la
  machine : elles se sautent en local quand il manque, et la CI l'exige (`--require-emacs`).

## Suites prévues

`docs/SPEC_V1.md` §5 prévoit `contract/`, `integration/`, `replay/`, `portability/`, `sandbox/`,
`endurance/`, `adversarial/`, `benchmarks/`. Chacune apparaît avec la première garantie qu'elle
vérifie.

## Règle

Un item de `docs/10_V1_ROADMAP.md` est terminé quand son test de sortie passe en CI, pas quand le
code est écrit. Un test qui ne peut pas échouer ne prouve rien : les gardes de ce dépôt sont
accompagnées de cas de violation délibérée qui démontrent qu'elles mordent.
