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

## Utiliser le cockpit

```
M-x locus-cockpit
```

Joint le daemon, relit ce qu'il sert, et affiche : projections, workers, conflits, timeline. Dans le
tampon, `g` rafraîchit, `c` reconnecte, `q` ferme.

La commande **s'ouvre même sans daemon** — c'est le moment où on regarde un cockpit. L'écran nomme
alors la panne et montre le dernier état connu avec son âge, plutôt que de refuser ou de se vider :
un écran vide se lirait comme un laboratoire au repos.

L'endpoint est `locus-endpoint`. Attention au décalage : `SPEC.md` §5 écrit `7420`, et `apps/locusd`
lie `127.0.0.1:8787` (`DEFAULT_BIND`). Tant que l'un des deux ne rejoint pas l'autre, un cockpit
laissé au défaut de la spec ne trouve pas un daemon laissé au sien.

## État

`W8.a` à `W8.k` — la frontière, l'authentification, les événements, le cache, le rendu, les
commandes, les artefacts, les intégrations, le transport, l'auteur, et la **session** qui les
assemble. `W8.k` est le premier item dont le test de sortie est écrit du point de vue de qui s'en
sert, et il existe parce que les dix précédents étaient tenus sans que rien ne s'ouvre.
