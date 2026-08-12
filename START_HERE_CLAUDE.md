# START HERE

Tu prends en charge un chantier multi-repos V1. Ce package est la spécification normative.
**Ne redéfinis pas le produit depuis zéro.**

## L'étape 0 est faite. Ne la refais pas.

Les dépôts ont été clonés et inspectés le 2026-08-07, et le package a été corrigé en conséquence.
Ce que tu lis intègre déjà ce constat. Ne relance pas d'audit général : vérifie seulement ce dont
ton item de roadmap a besoin.

| Dépôt | État réel constaté | Ce que ça implique |
|---|---|---|
| `maribakulj/locusolus` | **vide** — `LICENSE` + `README`, commit `7dc4dd1` | greenfield intégral. Rien à préserver, aucun audit antérieur à consulter |
| `maribakulj/canterel` | **fork non divergé** de `synthetic-sciences/OpenScience`, HEAD `c3f734c` | le rename est fait ; **le rebrand est interdit** (ADR 0010) |
| `maribakulj/xiiif` | v0.4.0 mûr, 23 `.el`, 33 fichiers de tests, `ROADMAP.md` à jour | l'essentiel de sa V1 existe. Six items sont déverrouillés dès maintenant |
| `maribakulj/emacs-config` | privé, non inventorié | l'inventaire est sa première tâche |

Quatre dépôts, pas cinq. Le client Emacs vit dans `locusolus/apps/emacs/` (ADR 0009).

## Où va ce package

Ce dossier est un paquet de handoff, pas une arborescence de dépôt. Premier commit (W0.1) :

```text
repos/locusolus/CLAUDE.md          → locusolus/CLAUDE.md
repos/locusolus/SPEC_V1.md         → locusolus/docs/SPEC_V1.md
repos/locusolus/apps-emacs/SPEC.md → locusolus/apps/emacs/SPEC.md
docs/ (00–12, adr/, deployment/)   → locusolus/docs/
schemas/examples/                  → locusolus/schemas/examples/
templates/                         → locusolus/templates/

repos/canterel/*                   → canterel/  (CLAUDE.md à la racine,
                                      SPEC_V1.md et migration → canterel/docs/locus/)
repos/xiiif/*                      → xiiif/     (CLAUDE.md à la racine, SPEC_V1.md → xiiif/)
repos/emacs-config/*               → emacs-config/
```

`locusolus/docs/` est la référence documentaire du chantier. Les trois autres dépôts n'en gardent
que ce qui les concerne, plus leur `CLAUDE.md`.

## Ordre de lecture

`CLAUDE.md` du dépôt où tu travailles (il contient les invariants globaux **et** les règles
propres au dépôt) → `docs/00` → `docs/01` → `docs/02` → `docs/03` → `docs/06` → la spec du dépôt →
`docs/10_V1_ROADMAP.md` → `docs/11_ACCEPTANCE_MATRIX.md`.

Les ADR arbitrent. `docs/adr/0009` (client Emacs) et `docs/adr/0010` (fork Canterel) contredisent
délibérément des passages des specs ; ils gagnent.

**Règle de priorité en cas de contradiction :** ADR → `docs/00` → `docs/01` → `docs/DECISIONS.md`
→ spec du dépôt → autres documents. Aucun document utilisant `locus-solus`, `OpenScience Lab`,
l'ancien sens de `Canterel` ou `CWP` n'est normatif.

## Les trois erreurs qu'il ne faut pas commettre

**Ne rebrande pas Canterel.** C'est un fork vivant d'un projet tiers. Renommer `@synsci/*` ou les
import paths toucherait 498 fichiers et détruirait la capacité de merge amont — c'est-à-dire
exactement ce que la spec demande de préserver. Tout ton code local va sous
`backend/cli/src/locus/**`. ADR 0010.

**Ne crée pas de stubs vides.** `repos/canterel/SPEC_V1.md` §4 liste 34 fichiers et
`repos/locusolus/SPEC_V1.md` §5 liste une trentaine de packages. Ce sont des annexes indicatives.
Un item est terminé quand son test de sortie passe, pas quand l'arborescence existe.

**Ne prends pas de raccourci d'architecture.** Pas de pseudo-graphe en Markdown, pas de sandbox
factice, pas de dépendance codée en dur à Temporal, pas d'Emacs utilisé comme moteur 3D, pas de
xiiif utilisé comme robot headless. La V1 peut être livrée par workstreams successifs ; elle ne
peut pas être livrée en trichant sur la structure.

## Stratégie de travail

Préserver ce qui fonctionne. Adapter plutôt que réécrire — sauf dans `locusolus`, où il n'y a rien
à adapter. Écrire les contrats avant les intégrations, et les tests de contrat inter-repos avant
les fonctionnalités cross-repo. Migrations de schéma explicites. Feature flags pour la transition
uniquement, jamais pour masquer une architecture incomplète. Toute dépendance nouvelle a une
justification, une licence compatible, une stratégie de version et un test de santé.

## Avant chaque session de code

Lire `docs/10_V1_ROADMAP.md`, prendre le premier item non terminé dont les dépendances sont
satisfaites, vérifier le code actuel, exécuter les tests de son périmètre, modifier **ce périmètre
seul**, mettre à jour `IMPLEMENTATION_LEDGER.md` à la fin.

Une session suivante doit pouvoir recevoir simplement : `continue la roadmap`.

## Par où commencer maintenant

`W0.1` — placement de la doc et des `CLAUDE.md` dans les quatre dépôts. Puis `W0.2` à `W0.10`,
qui produisent `packages/protocol` et le harness de conformance. Tout le reste en dépend.

En parallèle, sans attendre : les six items xiiif de `docs/10` §W10, et l'inventaire de
`emacs-config`.

Une décision reste ouverte et doit être tranchée avant W1 : **le langage de `locusd`**. Voir
`docs/10_V1_ROADMAP.md`, section « État de départ ».
