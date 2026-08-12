# `schemas/`

JSON Schemas versionnés. Ce sont les contrats de fil : ce qui est ici prime sur toute représentation
en mémoire, dans n'importe quel langage.

`docs/SPEC_V1.md` §5 prévoit la partition `commands/`, `events/`, `lep/`, `artifacts/`,
`environments/`, `federation/`. Les répertoires apparaissent avec les schémas qu'ils portent (W0.5
et W0.6 de `docs/10_V1_ROADMAP.md`), pas avant.

## Pourquoi les schémas d'abord

Les schémas sont la source depuis laquelle les SDK sont générés (W0.8), et il y en aura deux :
TypeScript pour le worker, Rust pour le serveur (ADR 0011). Ils fixent le protocole avant qu'un
choix de langage puisse l'infléchir. Une implémentation qui diverge du schéma a tort.

## Dialecte : JSON Schema Draft 7

Contrainte d'outillage, pas de goût. `typify` — la voie de référence pour JSON Schema → Rust —
supporte réellement Draft 7 ; sur 2020-12 il fonctionne parfois et casse souvent, et sa refonte est
en cours. C'est la condition 1 d'ADR 0011.

Le dialecte est fixé ici parce que ce répertoire est vide : le choix ne coûte rien aujourd'hui et se
migre mal une fois `lep/1.0` gelé. Un prototype `typify` qui passerait sur 2020-12 lève la condition
— l'expérience se fait en W0.5, avant le premier schéma, pas après.

## Règles

Un schéma publié ne change pas de sens sous le même identifiant. L'évolution suit `docs/06` : majeur
= rupture, mineur = champs optionnels compatibles. `lep/1.0` gèle à la fin de W0.

Les exemples et les fixtures ne sont pas de la documentation : chaque fichier de `schemas/examples/`
doit valider — ou invalider intentionnellement, selon son `expect` déclaré — contre le schéma qu'il
illustre.
