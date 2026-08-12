# Migration cross-repo

## GitHub

1. ~~créer `locusolus`~~ — **fait** : le dépôt existe (commit `7dc4dd1`), vide ;
2. ~~renommer `openscienceDH` → `canterel`~~ — **fait** : les deux noms résolvent vers le même SHA, le redirect GitHub est actif ;
3. ~~créer un dépôt Emacs séparé~~ — annulé par ADR 0009 ; le client vit dans `locusolus/apps/emacs` ;
4. conserver `xiiif` ;
5. conserver `emacs-config`.

**Le rebrand du code de Canterel est interdit** — voir ADR 0010. `canterel` est un fork non divergé de `synthetic-sciences/OpenScience` (Apache-2.0), avec un amont actif. Renommer les packages `@synsci/*` et les import paths toucherait 498 fichiers et ferait de chaque `git merge upstream/main` un conflit de masse, ce qui contredit l'exigence de préserver providers, agents, skills et sandbox (`repos/canterel/SPEC_V1.md` §30.1). « Canterel » nomme le worker, son mode de déploiement et son identité LEP (`worker_kind: "canterel"`), pas le fork. Le rename du dépôt GitHub suffit et il est déjà fait.

## Canterel

Préserver providers, agent registry, TaskTool semantics utiles, sessions, tools/skills, workspace, provenance import et sandbox existante. Ajouter LEP comme adaptateur ; ne pas déplacer tout le runtime dans Locus.

## Locus Solus

Nouveau repo, aucun héritage de code obligatoire. Import Atlas/provenance/research-state via outils de migration, jamais comme runtime dependency.

## Emacs

Extraire le produit générique vers `locusolus/apps/emacs` ; remplacer dans `emacs-config` les fonctions produit par configuration. Garder xiiif autonome. Voir `docs/EMACS_CONFIG_ACCESS.md` pour l'état réel de la source.

## Données

Toute migration produit un rapport : source, hash, mapping, objets staged, erreurs, ambiguïtés. Pas de promotion silencieuse.
