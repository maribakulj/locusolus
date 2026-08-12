# IMPLEMENTATION_LEDGER

Un exemplaire par dépôt, à la racine. Ajout en fin de fichier, jamais de réécriture d'une entrée
passée : c'est un journal, pas un état.

Une session de code se termine en ajoutant une entrée. Une session qui n'en produit pas n'a rien
livré, quoi qu'elle ait écrit.

## Format

<!-- prettier-ignore -->
```markdown
## AAAA-MM-JJ — <id roadmap> — <titre>

**Périmètre.** Fichiers touchés, une ligne. Si le périmètre a débordé de l'item, dire pourquoi.
**Tests exécutés.** Commande et résultat. Le test de sortie de l'item, nommément.
**Décisions prises.** Seulement celles qui contraignent la suite. Une décision qui mérite un ADR
reçoit un ADR et est référencée ici, pas décrite ici.
**Écart avec la spec.** Ce qui a été fait autrement, et pourquoi. « Aucun » est fréquent et valide.
**Prochain item.** Identifiant + vérification que ses dépendances sont satisfaites.
```

## Règles

Le périmètre déclaré doit correspondre au diff. Un débordement signale soit un découpage trop fin,
soit un couplage non anticipé — les deux méritent une ligne.

Sur `canterel` : toute modification hors de `backend/cli/src/locus/**` est justifiée, parce qu'elle
sera payée à chaque synchronisation amont (ADR 0010).

Une migration `[M]` inscrit son plan de rollback dans l'entrée.

Un test de sortie qui ne passe pas laisse l'item ouvert. On peut committer du code incomplet ; on
n'écrit pas « terminé ».

## Entrées

## 2026-08-07 — Étape 0 — constat et correction du package

**Périmètre.** Aucun code. Package de handoff révisé : renommage `locus-solus` → `locusolus`, fusion
du client Emacs dans `apps/emacs`, ADR 0001–0008 complétés, ADR 0009 et 0010 ajoutés, `CLAUDE.md`
par dépôt, `docs/10` découpé au commit près pour W0/W2, `docs/11` paramétrée par niveau de sandbox,
fixtures étiquetées.

**Tests exécutés.** Aucun test de code. Vérifications factuelles : accessibilité des dépôts, égalité
des SHA `canterel`/`openscienceDH`, distribution des auteurs, présence des modules cités par les
specs, surface du rebrand amont (498 fichiers), couverture réelle de l'API xiiif.

**Décisions prises.** Nom du projet : `locusolus`. ADR 0009 (client Emacs dans le monorepo, quatre
dépôts). ADR 0010 (fork suiveur pour Canterel, rebrand interdit).

**Écart avec la spec.** Cinq contradictions matérielles corrigées : nom du dépôt, rebrand vs suivi
amont, dépôt Emacs séparé, self-tests de sandbox non paramétrés par niveau, fixtures d'admission
ambiguës. Le statut de `emacs-config` reste à établir depuis Claude Code.

**Prochain item.** W0.1 — placement de la doc dans les quatre dépôts. Dépendances satisfaites.

## 2026-08-10 — W0.2 — squelette du monorepo, workspace et CI

**Périmètre.** Racines normatives et leurs `README.md` (`apps/`, `packages/`, `schemas/`, `tests/`,
`tooling/`), configuration de workspace (`package.json`, `package-lock.json`, `tsconfig.json`,
`tsconfig.base.json`, `.prettierrc.json`, `.prettierignore`, `.editorconfig`, `.nvmrc`,
`.gitignore`), `README.md` racine, `.github/workflows/ci.yml`, la vérification de structure
`tooling/lib/findings.ts` + `tooling/repo/{layout,check-repo}.ts` et son test
`tests/repo/layout.test.ts`. Aucun débordement. Aucun stub : pas un répertoire de `apps/` ou
`packages/` n'a été créé.

**Tests exécutés.** `npm run check` → `check:format` + `check:repo` + `typecheck` + 11 tests,
exit 0. Test de sortie de l'item — « CI verte sur un dépôt sans code » — vérifié en CI : run #1,
conclusion `success`. Vérifié aussi que la garde mord : `packages/domain/` réduit à un `.gitkeep`
produit `unit-placeholder` et exit 1.

**Décisions prises.** Outillage de dépôt en Node.js LTS + TypeScript exécuté par le type stripping
de Node, sans étape de build, avec `erasableSyntaxOnly` pour que le compilateur refuse la syntaxe
que Node ne sait pas effacer. Cette décision porte sur l'outillage, **pas** sur le langage de
`locusd`, qui reste ouvert : aucun répertoire `apps/*` n'existe, et une unité dans un autre langage
apportera le manifeste de son écosystème. Convention de nommage des unités de workspace :
`@locus/<répertoire>`, apps comprises — le scope npm n'était fixé par aucun document, il est
réversible et vérifié par `check:repo`.

**Écart avec la spec.** Les listes de packages, d'apps et de sous-répertoires de `docs/SPEC_V1.md`
§5 ne sont pas instanciées : ce sont des annexes indicatives, et un répertoire vide est un stub.
`disciplines/`, `environments/` et `deploy/`, également prévus au §5, appartiennent à W5 et W11 et
ne sont pas créés. La règle « pas de stub » est rendue opposable plutôt que déclarative :
`check:repo` refuse un répertoire d'unité vide.

**Prochain item.** W0.3 — garde de frontières architecturales. Dépendances satisfaites : le
squelette et la CI existent, ce qui est tout ce dont la garde a besoin.

## 2026-08-10 — W0.3 — garde de frontières architecturales

**Périmètre.** `boundaries.json` à la racine (forme opposable des cinq règles),
`tooling/boundaries/{rules,imports,analyze,emacs,check-boundaries}.ts`, `tooling/lib/glob.ts`,
`tests/boundaries/` (fixtures + `contract`, `imports`, `emacs`), l'étape CI « Architectural
boundaries » et l'installation d'Emacs qu'elle exige, le script `check:boundaries`, l'exclusion des
fixtures de `tsconfig.json`, et la mise à jour des `README.md` que la garde change. Débordement
assumé et minime : les `README.md` de W0.2 sont amendés parce qu'ils annonçaient des frontières
encore non opposables.

**Tests exécutés.** Test de sortie de l'item — « une violation délibérée fait échouer la CI » —
vérifié dans les deux sens, dans cet ordre. (1) Le contrat et les douze violations délibérées ont
été committés **sans la garde** : run CI #2 sur `9593420`, conclusion `failure`, étape «
Architectural boundaries », `MODULE_NOT_FOUND`. (2) La garde implémentée : `npm run check` → 27
tests, exit 0. (3) Une violation de **chacune** des cinq règles plantée dans l'arborescence réelle :
`npm run check` → 6 findings, exit 1, et la règle 5 passe de « sans objet » à « vérifiée sur 1
fichier ». Retrait des violations → exit 0.

**Décisions prises.** Trois, qui contraignent la suite. Le contrat de frontières est un fichier de
données à la racine, pas du code : ajouter un package d'infrastructure oblige à amender
`boundaries.json`, donc à faire apparaître l'acte d'architecture dans le diff. Un fichier source
dont le langage n'a pas d'extracteur d'imports est un **angle mort qui fait échouer la CI**, pas une
dérogation silencieuse — c'est la contrepartie de ne pas trancher le langage de `locusd` : le jour
où du code arrive dans un langage que la garde ne sait pas lire, la CI le dit tout de suite. Enfin,
une règle sans objet est rapportée comme telle et jamais comptée comme vérifiée ; la CI passe
`--require-emacs` pour qu'une règle ne puisse pas être sautée en silence.

**Écart avec la spec.** Trois précisions, aucune contradiction. La règle 1 dit « package
d'infrastructure » sans définir le terme : `boundaries.json` en donne une définition opposable
(catalogue `infrastructure`), à laquelle est ajouté un catalogue `host-io` — un domaine qui ouvre un
fichier ou une socket a cessé d'être un domaine (invariant 1). La règle 3 dit « et projections »
sans que leur emplacement soit fixé : `packages/projections/**` tient la place jusqu'à W1.d. La
règle 5 n'a aujourd'hui aucun objet — `apps/emacs` n'existe pas — et est démontrée sur fixtures ; le
test de séparation du premier commit de W8 reste dû.

**Prochain item.** W0.4 — `packages/protocol` : IDs, enveloppe d'erreur structurée, politique de
versionnement, horodatage. Dépendances satisfaites : W0.2 et W0.3 sont terminés, et `docs/06` plus
`docs/10` placent W0.4 immédiatement après. Réserve à lever d'abord : **W0.1 n'a pas été exécuté** —
`CLAUDE.md`, `docs/` et `schemas/examples/` ne sont pas encore dans le dépôt, alors que W0.5 et W0.7
en dépendent directement.

## 2026-08-10 — W0.1 — placement de la doc dans les quatre dépôts

Réserve de l'entrée précédente levée : W0.1 est exécuté, hors séquence.

**Périmètre.** Quatre dépôts. Sur `locusolus`, 45 fichiers reçus : `CLAUDE.md`,
`START_HERE_CLAUDE.md`, `docs/` (00–12, `DECISIONS.md`, `adr/0001`–`0010`, `deployment/`),
`docs/SPEC_V1.md`, `apps/emacs/{SPEC,CLAUDE}.md`, `schemas/examples/`, `templates/` ; plus la garde
`tooling/repo/{naming,check-naming}.ts` et son test, l'extraction de `tooling/lib/walk.ts`,
l'exclusion du corpus dans `.prettierignore`, et les `README.md` que le corpus change. Sur
`canterel` : `docs/locus/` plus deux fichiers amont, justifiés dans son propre ledger. Sur `xiiif`
et `emacs-config` : trois documents chacun, aucun fichier existant modifié sur `xiiif`.

**Tests exécutés.** Intégrité du paquet d'abord : `sha256sum -c CHECKSUMS.sha256` → 59/59. Puis
chaque fichier placé comparé à son original : 45/45 identiques sur `locusolus`, 3/3 sur `canterel`,
3/3 sur `xiiif`, 2/3 sur `emacs-config` (voir écart). Test de sortie de l'item — « l'ancien nom du
projet ne subsiste nulle part » — vérifié sur les quatre dépôts, et rendu permanent sur `locusolus`
par `npm run check:naming`, exécuté en CI. `npm run check` → 31 tests, exit 0.

**Décisions prises.** Le corpus reçu est exclu du formatage, sur `locusolus` comme sur `canterel` :
reflower un tableau ou un bloc de spec est une mutation silencieuse d'un document normatif. Vérifié
que l'exclusion est porteuse et non décorative — sans elle, `prettier --list-different` signale
`SPEC_V1.md`. Sur `canterel`, le `CLAUDE.md` amont est **conservé et complété**, pas remplacé : ADR
0010 prescrit ce motif pour le `NOTICE`, et le document amont porte l'architecture de prompts et le
guide de RCA dont une session travaillant sur le code amont a besoin.

**Écart avec la spec.** Quatre, tous consignés. (1) Le test de sortie tel qu'écrit —
`grep -r "locus-solus"` ne renvoie rien — ne peut pas passer : les documents qui consignent le
renommage citent forcément l'ancien nom. `check:naming` implémente l'intention : toute occurrence
est interdite et chaque survivante est nommée avec sa raison, une dérogation devenue caduque étant
signalée à son tour. (2) `emacs-config/CLAUDE.md` corrigé sur deux lignes :
`modules/marcel-locus-solus.el` était la dernière occurrence **vivante** de l'ancien nom, et le
répertoire était faux — les modules de ce dépôt vivent dans `lisp/`. Seul contenu reçu modifié de
tout W0.1. (3) Deux placements que la table de `START_HERE_CLAUDE.md` ne prévoit pas :
`START_HERE_CLAUDE.md` lui-même à la racine, et `apps-emacs/CLAUDE-notes.md` →
`apps/emacs/CLAUDE.md`. (4) `canterel/IMPLEMENTATION_LEDGER.md` créé en avance sur W0.10, ADR 0010
exigeant que la modification de fichiers amont y soit justifiée.

**Prochain item.** Inchangé : **W0.4**, `packages/protocol`. Ses dépendances sont désormais
pleinement satisfaites, la réserve de l'entrée précédente étant levée — `docs/06` (politique de
versionnement) et `docs/SPEC_V1.md` sont dans le dépôt, et `schemas/examples/` y est pour W0.5 et
W0.7. Déverrouillé en parallèle et sans dépendance : les six items `xiiif` de `docs/10` §W10, et
l'inventaire de `emacs-config`.

## 2026-08-11 — ADR 0011 — langage du control plane, et sa mise en application

Hors roadmap : `docs/10` ne numérote pas cette décision, il la déclare ouverte et exige qu'elle
tombe avant W1.

**Périmètre.** Deux PR. La décision : `docs/adr/0011-langage-de-locusd.md`, règle 2 de `CLAUDE.md`
reformulée, `statement` correspondant dans `boundaries.json`, `docs/DECISIONS.md` (D016), dialecte
fixé dans `schemas/README.md`. La mise en application : extracteur d'imports Rust dans
`tooling/boundaries/imports.ts`, lecture des manifestes dans `tooling/boundaries/manifests.ts`
(extrait de `analyze.ts` et étendu à `Cargo.toml`), motifs Rust dans les cinq catalogues, `.rs`
déclaré analysable, quatre fixtures Rust, `tests/boundaries/extractors.test.ts`,
`tooling/README.md`. Débordement déclaré : cette entrée de ledger est dans la seconde PR plutôt que
dans une troisième.

**Tests exécutés.** Même discipline que W0.3, fixtures d'abord. Les quatre fixtures Rust et la
fixture `clean` augmentée ont d'abord échoué en rendant `boundary-blind-spot` au lieu du verdict
attendu — c'est-à-dire que la garde disait « je ne sais pas lire ce fichier » et non « rien à
signaler ». Puis, extracteur écrit : `npm run check` → 45 tests, exit 0. Enfin, quatre violations
Rust plantées dans l'arborescence réelle : `std::{fs::File, …}` attrapé par la règle 1, `bollard`
déclaré dans un `Cargo.toml` sans aucun import attrapé par la règle 4, `youki::container` par la
règle 4, `temporalio_sdk::Worker` par la règle 2 après normalisation en `temporalio-sdk`. Retrait →
exit 0.

**Décisions prises.** ADR 0011 : Rust pour `locusd`, `locus-execd` et la CLI ; TypeScript pour
`apps/web`, le SDK client et le worker ; Emacs Lisp pour `apps/emacs`. Le motif décisif n'est pas la
performance mais l'exhaustivité vérifiée des types somme sur une machine à états épistémique, et le
constat que le chantier est multi-langage quel que soit le choix — `locus-execd` ne peut pas être en
TypeScript. Quatre conditions dans l'ADR, dont deux ont un effet immédiat : les JSON Schemas
s'écrivent en Draft 7 (`typify` ne tient pas 2020-12), et W0.8 est budgété pour deux SDK.

Deux décisions d'outillage suivent. Un manifeste de dépendances est lu comme une source d'imports,
pour Cargo comme pour npm : une dépendance déclarée est le moment où quelqu'un a décidé, avant même
la première ligne qui l'utilise. Et les chemins Rust sont normalisés `::` → `/`, pour que
`boundaries.json` s'écrive dans une seule syntaxe quel que soit le langage.

**Écart avec la spec.** `SPEC_V1.md` §4.5 donnait TypeScript comme technologie de référence ; ADR
0011 l'amende, selon la convention déjà employée par ADR 0009 et 0010, qui amendent sans réécrire le
document amendé. La règle 2 de `CLAUDE.md` nommait `@temporalio/*`, un package npm : elle
présupposait TypeScript dans sa formulation même et désigne maintenant le SDK Temporal
indépendamment de son écosystème. Go n'a toujours pas d'extracteur et reste un angle mort signalé —
assumé, puisque le langage retenu n'est pas Go.

**Prochain item.** Inchangé : **W0.4**, `packages/protocol`. Dépendances satisfaites, et la décision
de langage qui devait tomber avant W1 est désormais prise, donc W0.4 peut être écrit directement
dans le langage définitif au lieu d'être réécrit.
