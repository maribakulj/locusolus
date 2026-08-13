# `apps/`

Unités déployables. Une par répertoire, chacune étant une racine de composition : elle assemble des
`packages/`, elle n'est importée par personne.

Cibles prévues par `docs/SPEC_V1.md` §5 : `locusd` (daemon et composition root), `locus-execd`
(broker d'exécution privilégié, séparé), `cli` (la commande `locus`), `web` (cockpit et viewers
riches), `worker-control` (workers de workflows du control plane), `emacs` (cockpit Emacs publiable,
ADR 0009).

Cette liste est une annexe indicative. **Aucun de ces répertoires n'existe tant qu'il ne porte pas
de comportement testé** : un répertoire vide sous `apps/` fait échouer `npm run check:repo`.

## Langages

`locusd`, `locus-execd` et `cli` sont en **Rust** (ADR 0011). `web` est en TypeScript, `emacs` en
Emacs Lisp.

Les unités TypeScript déclarent un `package.json` et rejoignent le workspace npm ; les unités Rust
apportent leur `Cargo.toml`. La garde de frontières lit les deux — une dépendance déclarée compte
comme un import, avant même la première ligne qui l'utilise.

L'outillage du dépôt reste en Node/TypeScript : il n'est livré à personne et doit savoir lire tous
les langages du dépôt sans appartenir à aucun.

## Frontières

`apps/locusd` n'ouvre jamais de socket de runtime de containers — c'est le rôle de
`apps/locus-execd`. `apps/emacs` doit démarrer sous `emacs -Q` avec sa seule `load-path` — cette
dernière est vérifiée en démarrant réellement un Emacs, pas en lisant le code.

Les deux règles sont opposables : `npm run check:boundaries`, exécuté en CI. Leur forme exécutable
est `boundaries.json`, leur énoncé est dans `CLAUDE.md`, section « Frontières vérifiées par la CI ».
