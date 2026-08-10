# `schemas/`

JSON Schemas versionnés. Ce sont les contrats de fil : ce qui est ici prime sur toute représentation
en mémoire, dans n'importe quel langage.

`docs/SPEC_V1.md` §5 prévoit la partition `commands/`, `events/`, `lep/`, `artifacts/`,
`environments/`, `federation/`. Les répertoires apparaissent avec les schémas qu'ils portent (W0.5
et W0.6 de `docs/10_V1_ROADMAP.md`), pas avant.

## Pourquoi les schémas d'abord

Le langage d'implémentation de `locusd` reste une décision ouverte. Les schémas sont communs aux
deux options : ils fixent le protocole avant qu'un choix de langage puisse l'infléchir, et ils sont
la source depuis laquelle les SDK sont générés (W0.8). Une implémentation qui diverge du schéma a
tort.

## Règles

Un schéma publié ne change pas de sens sous le même identifiant. L'évolution suit `docs/06` : majeur
= rupture, mineur = champs optionnels compatibles. `lep/1.0` gèle à la fin de W0.

Les exemples et les fixtures ne sont pas de la documentation : chaque fichier de `schemas/examples/`
doit valider — ou invalider intentionnellement, selon son `expect` déclaré — contre le schéma qu'il
illustre.
