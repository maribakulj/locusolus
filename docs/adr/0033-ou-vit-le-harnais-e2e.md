# ADR 0033 — Où vit le harnais `e2e/minimal_science`, et qui y joue le worker

**Statut :** accepté. Ouvre `W12.f`, et rend `W12.d` exécutable.

**Contexte.** La première tentative de `W12.d` s'est arrêtée avant sa première ligne de code :
`e2e/minimal_science` n'est nommé **nulle part** hors de `docs/10` — pas de répertoire, pas de job de
CI, pas d'ADR. Et le seul worker du chantier vit dans un autre dépôt, dans un autre langage.
`packages/testing` ne peut pas le remplacer : il joue le **serveur** par construction (`W0.9`), et
`W20.k` s'y est heurté.

La ligne `W12.f` posait la question sous forme de dilemme — faire entrer `canterel` dans la CI de
`locusolus`, ou l'inverse — et demandait un ADR qui tranche « en **mesurant** les deux coûts plutôt
qu'en les estimant ». Ce document mesure. Deux des trois arguments qu'il attendait n'ont pas
survécu à la mesure.

---

## Ce qui a été mesuré, et comment

Toutes les valeurs ci-dessous viennent d'exécutions réelles, dans le conteneur de session pour les
temps locaux et dans GitHub Actions pour les durées de CI. Aucune n'est estimée.

| Grandeur | Mesure | Comment |
| --- | --- | --- |
| `locusd` + `locus-execd`, build **à froid** | **26 s**, 529 Mo de `target` | `cargo clean` puis `cargo build --bin locusd --bin locus-execd` |
| Les deux mêmes, dépendances déjà compilées | 10 s | `cargo clean -p locusd -p locus-execd` puis rebuild |
| Le worker `canterel`, démarrage depuis les sources | **2 s** | `bun run src/index.ts worker status`, `node_modules` présent |
| `node_modules` de `canterel` | 871 Mo | `du -sh` |
| Job `rust` de `locusolus` en CI | 117 s | run 32778810002, clippy + tests du workspace entier |
| Job `Test` de `canterel` en CI | 161 s | run 32780982164, 2 323 tests |

**Le coût de construction ne décide rien.** Vingt-six secondes pour les deux binaires Rust, deux
secondes pour démarrer le worker : les deux directions sont bon marché, et l'argument de « la chaîne
à installer » — celui que la ligne de roadmap mettait en avant — ne sépare pas les options. Il fallait
le mesurer pour le retirer du débat plutôt que de le trancher au jugé.

---

## Décision 1 — Le harnais vit dans `locusolus`, et `canterel` y est cloné à une révision épinglée

Trois raisons, dans l'ordre où elles ont résisté à la vérification.

**a. La CI de `locusolus` a déjà une sandbox réelle ; celle de `canterel` n'en a pas.** Le job
`sandbox` de `.github/workflows/ci.yml` construit une image, la fait tourner sous `podman`, et
constate ce que le conteneur voit du réseau. L'invariant 5 veut que l'exécution non fiable se fasse
dans une sandbox réelle, avec limites et attestation ; un `e2e/minimal_science` qui tournerait sans
sandbox n'exercerait pas la clause qu'il prétend tenir. Refaire ce job dans `canterel` serait le
dupliquer, et une seconde copie diverge.

**b. Le verdict appartient à l'endroit où la clause est écrite.** `e2e/minimal_science` est le test
de sortie d'un item de `docs/10`, sur une spécification qui vit dans `docs/SPEC_V1.md`. Un verdict
rendu ailleurs oblige à traverser un dépôt pour savoir si la clause tient.

**c. La flakiness, mesurée pendant ce chantier.** Voir décision 3 — c'est l'argument qui s'est
révélé décisif, et il ne figurait pas dans la question posée.

---

## Décision 2 — L'asymétrie de coût de synchronisation que la roadmap invoquait **n'existe pas**

La ligne `W12.f` opposait « coût ponctuel » (faire entrer `canterel` dans la CI de `locusolus`) à
« coût **payé à chaque synchronisation amont** » (l'inverse), en s'appuyant sur l'ADR 0010.

C'est faux, et il suffisait d'ouvrir le fichier : `canterel` porte déjà
`.github/workflows/locus.yml`, un fichier **neuf**, dont l'en-tête dit exactement pourquoi il
existe — « l'amont n'en a aucun de ce nom, donc aucune synchronisation ne peut le conflicter. C'est
la raison pour laquelle les jobs vivent ici plutôt qu'ajoutés à `ci.yml` ». Un job ajouté là ne coûte
rien au merge amont.

L'argument est donc **retiré**, et la décision 1 ne s'appuie pas dessus. C'est la même faute que
`W2.22` a trouvée dans sa propre ligne de roadmap : déduire un état du code à partir d'une règle
générale, sans lire le fichier qui la contredit. Une règle vraie — l'ADR 0010 l'est — ne dit rien
d'un fichier qu'elle n'a pas regardé.

---

## Décision 3 — L'argument décisif est la **stabilité de la suite hôte**, et il a été mesuré malgré nous

Pendant `W2.23`, trois exécutions de CI sur un **arbre identique** (deux commits vides) ont donné :

| Tour | `Migration (windows-latest)` | `Test` |
| --- | --- | --- |
| 1 — `1b110f0` | **rouge** — `EBUSY` sur une course d'écriture à 1 ms | vert |
| 2 — arbre identique | vert | **rouge** — `ComputeJobs Modal`, 1 échec sur 2 323 |
| 3 — arbre identique | vert | vert |

Deux tests amont différents, instables, dans des fichiers qu'aucun diff local n'atteint —
vérifié en lisant leurs imports, pas en le supposant. Il a fallu **trois** exécutions pour obtenir un
tour entièrement vert.

Sur la même période, la CI de `locusolus` a rendu vert au premier coup sur chacune des sept PR de la
session.

Un `e2e/minimal_science` hébergé dans `canterel` hériterait de cette instabilité : son rouge
signifierait tantôt « la chaîne scientifique est cassée », tantôt « un test de migration Windows a
perdu une course de fichiers ». C'est exactement ce que `W20.i` a coûté cher à apprendre sous une
autre forme — **un verdict qui peut être rouge pour une raison étrangère cesse d'être lu**.

Cette mesure n'a pas été cherchée ; elle est tombée pendant un autre item. Elle est consignée ici
parce qu'une décision d'architecture appuyée sur une observation fortuite mais **datée et
reproductible** vaut mieux qu'une décision appuyée sur une intuition de stabilité.

---

## Décision 4 — Le worker est le **vrai**, et son absence est une panne, jamais un saut

Le harnais démarre `locusd`, `locus-execd` et le worker `canterel` réel. Aucun des trois n'est
simulé : `packages/testing` joue le serveur (`W0.9`) et ne peut pas jouer le client, et un faux
worker prouverait que `locusd` parle à quelque chose, pas que la chaîne tient.

Où `canterel` se trouve est dit par `LOCUS_E2E_WORKER`. **Son absence fait échouer le harnais**, avec
un message qui nomme la variable — elle ne le fait pas se déclarer non applicable. C'est la règle du
dépôt, et `W20.i` a montré ce qu'elle coûte quand on l'oublie : un test qui se saute lui-même rend
vert un dossier que personne n'a exercé, et le rouge attendu n'arrive jamais.

Même chose pour les trois processus : si l'un ne démarre pas, le harnais **échoue bruyamment** en
nommant lequel et en rendant ce qu'il a écrit sur sa sortie d'erreur. Un harnais qui se rabattrait
sur « deux processus sur trois, c'est déjà ça » rendrait un vert dont personne ne pourrait dire ce
qu'il couvre.

---

## Décision 5 — Un job séparé, qui n'est pas dans le chemin de `npm run check`

`npm run check` est la porte que chaque item franchit ; il tourne en quelques minutes et ne demande
rien de plus qu'un checkout. Le harnais e2e demande un second dépôt, une chaîne Bun et `podman`. Le
faire entrer dans `check` rendrait rouge, sur la machine d'un contributeur sans `podman`, un contrôle
qui n'a rien à voir avec ce qu'il vient d'écrire.

Il vit donc dans un job de CI distinct, et se lance à la main par `npm run e2e`. Conséquence assumée
et nommée : **une session qui ne lance que `npm run check` n'exerce pas la chaîne e2e**, et c'est la
CI qui la tient.

---

## Conséquences

- `tests/e2e/` accueille le harnais ; `canterel` y est cloné à une révision épinglée, comme le SDK
  l'est déjà en sens inverse.
- La CI de `locusolus` gagne un job `e2e`, qui checkout les deux dépôts. Les deux étant privés sous
  le même propriétaire, il lui faut un jeton — c'est le seul coût opérationnel réel de la décision 1,
  et il est ponctuel.
- `W12.d` devient exécutable : il écrit le scénario, le harnais lui donne les trois processus.
- Rien n'est ajouté à `canterel` par cet ADR. Le dépôt worker reste le worker.
