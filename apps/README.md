# `apps/`

Unités déployables. Une par répertoire, chacune étant une racine de composition : elle assemble des
`packages/`, elle n'est importée par personne.

Cibles prévues par `docs/SPEC_V1.md` §5 : `locusd` (daemon et composition root), `locus-execd`
(broker d'exécution privilégié, séparé), `cli` (la commande `locus`), `web` (cockpit et viewers
riches), `worker-control` (workers de workflows du control plane), `emacs` (cockpit Emacs publiable,
ADR 0009).

Cette liste est une annexe indicative. **Aucun de ces répertoires n'existe tant qu'il ne porte pas
de comportement testé** : un répertoire vide sous `apps/` fait échouer `npm run check:repo`.

## Ce que ce répertoire ne présume pas

Le langage d'implémentation de `locusd` est une décision ouverte (`docs/10_V1_ROADMAP.md`, « État de
départ »). Le squelette ne la tranche pas : l'outillage de dépôt est en Node/TypeScript parce qu'un
SDK TypeScript existera de toute façon, mais aucune unité `apps/*` n'est supposée être en
TypeScript. Les unités qui le sont déclarent un `package.json` et rejoignent le workspace npm ; les
autres apportent le manifeste de leur écosystème.

## Frontières

`apps/locusd` n'ouvre jamais de socket de runtime de containers — c'est le rôle de
`apps/locus-execd`. `apps/emacs` doit démarrer sous `emacs -Q` avec sa seule `load-path` — cette
dernière est vérifiée en démarrant réellement un Emacs, pas en lisant le code.

Les deux règles sont opposables : `npm run check:boundaries`, exécuté en CI. Leur forme exécutable
est `boundaries.json`, leur énoncé est dans `CLAUDE.md`, section « Frontières vérifiées par la CI ».
