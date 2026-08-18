# apps/emacs — cockpit Emacs de Locus Solus

Client Emacs produit, dans le monorepo (ADR 0009). `emacs-config` n'en est qu'un consommateur : ce
paquet doit s'installer et fonctionner sans aucune configuration personnelle.

## Installation

Le paquet ne dépend que d'Emacs. Il s'installe par `package-vc`, `straight`, `quelpa` ou en ajoutant
ce répertoire à `load-path` — aucun gestionnaire n'est imposé.

```elisp
(add-to-list 'load-path "/chemin/vers/locusolus/apps/emacs")
(require 'locus)
(locus-describe)
```

## Ce que charger le paquet ne fait pas

Charger `locus` n'ouvre aucune connexion, ne lance aucun processus et n'arme aucun timer. Le
démarrage d'Emacs reste donc fonctionnel sans réseau et sans daemon Locus, ce que `SPEC.md` §7.1
exige et que la suite ERT vérifie.

## Tests

```sh
npm run check:emacs          # depuis la racine du dépôt
```

La suite tourne sous `emacs -Q` avec la seule `load-path` du paquet : une suite lancée sous la
configuration de son auteur prouverait que le paquet marche là où ce n'était pas en doute.

La frontière est gardée deux fois, sans code partagé : par cette suite, depuis l'intérieur du
paquet, et par la règle 5 de `tooling/boundaries/`, depuis l'extérieur.

## État

`W8.a` — la frontière. Le reste du cockpit décrit par `SPEC.md` arrive avec `W8.b` et suivants.
