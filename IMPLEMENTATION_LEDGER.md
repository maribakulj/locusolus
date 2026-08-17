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

## 2026-08-12 — W0.4 — `packages/protocol`

**Périmètre.** Premier code produit du dépôt. Workspace Cargo (`Cargo.toml`, `rust-toolchain.toml`,
`target/` ignoré par git et exclu de la garde de frontières), crate `locus-protocol` — `id.rs`,
`time.rs`, `error.rs`, `version.rs` — et sa suite `tests/canonical_forms.rs`. Débordements déclarés,
tous induits par l'arrivée d'un second écosystème : job CI `rust`, script `check:rust` appelé par
`npm run check`, convention de nommage des crates ajoutée à `check:repo` avec ses deux tests, et les
`README.md` que le tout change.

**Tests exécutés.** Test de sortie de l'item — « unitaires » — : `cargo test` → 27 tests, exit 0.
Plus les gardes qui les encadrent : `cargo fmt --all --check` et
`cargo clippy --all-targets --all-features -- -D warnings`, pedantic comprise, sans aucune
dérogation posée. `npm run check` → 47 tests JS puis la chaîne Rust, exit 0. La garde de frontières
couvre désormais les cinq fichiers Rust : les règles 2 et 3 passent de 21 à 29 fichiers examinés.

**Décisions prises.** Quatre, toutes contraignantes pour la suite.

Le corps des identifiants est un **ULID**, là où §7.7 laissait le choix entre UUIDv7 et ULID : son
encodage textuel se trie lexicographiquement dans l'ordre chronologique, propriété dont l'event
store se servira directement, et c'est la forme que montrent les exemples du §10.1.

**Une seule écriture par valeur.** Horodatage, identifiant et version n'acceptent au décodage que
leur forme canonique exacte : `2026-07-26T12:00:00Z` est refusé, pas normalisé. §7.7 dit que « les
hashes portent sur une canonicalisation stable » ; deux pairs qui écriraient différemment le même
instant calculeraient deux hashes différents, et c'est précisément ce que les fixtures inter-SDK de
`docs/06` existeront pour attraper.

**Le crate ne lit ni l'heure ni l'aléa.** Composer un identifiant demande un instant et dix octets
d'entropie, tous deux fournis par l'appelant. Le crate reste pur, donc déterministe en test, et
l'invariant 1 tient jusque dans les fondations.

**Deux règles de la spec sont rendues indéfaisables plutôt que documentées.** « Une erreur
`retryable` doit préciser les conditions de retry » : il n'existe aucune façon de construire une
erreur réessayable sans énoncer sa condition, et un `"retryable": true` nu est refusé au décodage
JSON. « Ne logge ni jeton, ni clé, ni contenu classifié » : une erreur marquée `security_sensitive`
expurge message et détails à travers `Display`, qui est le chemin par lequel une erreur finit dans
un log — l'accès reste ouvert, c'est l'écriture accidentelle qui est fermée.

**Écart avec la spec.** Un seul, isolé dans un module `provisional`. Les specs fixent exactement dix
préfixes d'identifiant, tous vus littéralement sous la forme `evt_01…`. `error_id` et `mission_id`
sont nommés sans qu'aucun exemple n'en donne le préfixe ; l'enveloppe d'erreur de cet item en a
besoin. `err` et `msn` sont donc déclarés provisoires, dans un module à part, et W0.6 — qui définit
`Attempt` et les événements — les confirmera ou les remplacera. `attempt` est modélisé comme un rang
numérique et non comme un identifiant, parce que la spec Canterel §11.1 et son §26 l'écrivent sans
le suffixe `_id` que portent tous ses voisins ; à confirmer au même moment.

Par ailleurs, la taxonomie d'erreurs retenue est celle de la spec Canterel §26 (dix-sept
catégories), pas celle du `SPEC_V1.md` §22.5 (huit types) : la première décrit LEP, la seconde l'API
HTTP. Ce sont deux surfaces, pas deux versions d'une même liste, et la seconde viendra avec son API.

**Prochain item.** **W0.5** — JSON Schemas LEP : `CapabilityManifest`, `MissionEnvelope`,
`ContextView`, `EnvironmentBlueprint`, `SandboxSpec`, `ResourceSpec`. `[M]`, donc plan de rollback
exigé dans son entrée. Dépendances satisfaites : `packages/protocol` existe, `schemas/examples/` est
en place depuis W0.1, et le dialecte est fixé à Draft 7 par la condition 1 d'ADR 0011 — c'est l'item
qui doit l'appliquer.

## 2026-08-13 — W0.5 `[M]` — JSON Schemas LEP, première moitié

Item repris après un arrêt de ma part qui n'avait pas lieu d'être : j'avais traité `[M]` comme une
porte à l'ouvrage alors que c'est une porte à la **fusion**. La PR est donc ouverte et **non
fusionnée** — elle attend l'arbitrage, comme la règle le demande.

Deuxième erreur corrigée en chemin : je croyais W0.5 bloqué par ADR 0011. `schemas/README.md`, sur
`main` depuis W0.1, dit le contraire en toutes lettres — « les schémas sont communs aux deux options
: ils fixent le protocole avant qu'un choix de langage puisse l'infléchir ». La réponse était dans
le dépôt.

**Périmètre.** `schemas/lep/1.0/` (six schémas dont le vocabulaire partagé),
`schemas/environments/1.0/environment-blueprint.schema.json`, `schemas/registry.json`,
`schemas/README.md`, `tooling/schemas/` (validateur et CLI), `tests/schemas/lep.test.ts`,
`package.json` (`check:schemas` dans la chaîne, `ajv` et `ajv-formats` en devDependencies), et deux
fixtures reçues — débordement déclaré ci-dessous.

**Tests exécutés.** Test de sortie de l'item : « les exemples de `schemas/examples/` valident ».
`npm run check:schemas` → ok, quatre exemples validés, un déclaré `pending` (W0.6). `npm run check`
complet → 0, 47 tests, 0 échec.

Les seize tests neufs ne vérifient pas que les schémas acceptent — les fixtures s'en chargent — mais
qu'ils **refusent** : chaque champ obligatoire retiré un par un, `lep/2.0`, un hash tronqué, une
réservation à zéro, un niveau `S6`, une allowlist sans liste, un `deny` qui en porte une, une classe
de données inventée, une date mal formée. Un schéma qui accepte tout passe toutes les fixtures.

**Décisions prises.** Les six qui contraignent la suite sont écrites dans `schemas/README.md` plutôt
qu'ici, parce que c'est là qu'on les lira. En résumé : Draft 7 (le plus outillé hors JavaScript, et
le langage de `locusd` n'est pas tranché), identifiants en URN (stables, versionnés, sans promesse
d'être récupérables), documents **ouverts** (`docs/06` fait du mineur un ajout compatible : fermer
les documents transformerait chaque ajout mineur en rupture), et une demande n'est pas une offre —
`MissionEnvelope.resources` et `CapabilityManifest.resources` gardent des noms de champs différents
pour que personne ne les soustraie l'un de l'autre sans y penser.

**Une ambiguïté de spec tranchée, et signalée.** `SPEC_V1.md` §21.7 écrit `connector_only` ; les
fixtures reçues et toutes les autres valeurs d'énumération du protocole sont en kebab-case
(`oauth-local`, `rootless-oci`, `service-credential`). J'ai retenu `connector-only` — les noms de
champs en `snake_case`, les valeurs en kebab — et un test fixe la graphie pour que la renverser soit
un changement visible. La PR étant de toute façon soumise à arbitrage, c'est là qu'il faut me
contredire si le texte de la spec doit primer.

**Débordement de périmètre, déclaré.** Deux fixtures reçues portaient `"hash": "sha256:..."`, un
placeholder. Trois issues : accepter les points dans le motif — un schéma qui valide `...` comme
digest ment ; laisser l'exemple échouer — le test de sortie de l'item n'est plus tenu ; ou réparer
la fixture. J'ai réparé, avec `sha256("ctx-example")`, reproductible. Un fichier dont la valeur est
`...` est une esquisse, pas une fixture.

**Écart avec la spec.** Aucun sur le périmètre de l'item. W0.6 porte la seconde moitié (`Lease`,
`Attempt`, événements, `ArtifactManifest`, `RunManifest`, `SandboxAttestation`, `EpistemicCommit`) ;
`sandbox-attestation.json` est déclaré `pending` dans le registre plutôt qu'ignoré, et le validateur
échoue sur tout exemple qui ne serait ni validé ni déclaré — un exemple que plus personne ne vérifie
ressemble en tout point à un exemple qui passe.

**Plan de rollback (`[M]`).** La migration est additive : aucun schéma n'existait avant, aucun
consommateur n'en dépend, `packages/protocol` (W0.4) n'est pas fusionné. Revenir en arrière est un
`git revert` du merge commit, sans étape de données ni de migration inverse. Les deux points à
vérifier après un revert : `package.json` retrouve sa chaîne `check` sans `check:schemas`, et les
deux fixtures retrouvent leur placeholder — ce dernier point étant le seul qu'un revert restaure
alors qu'il valait mieux le garder. Si le revert vise seulement une décision (la graphie de
`connector-only`, le choix du Draft), il ne demande pas de revert du tout : un seul fichier de
vocabulaire et un test à changer.

**Prochain item.** W0.6, `[M]` lui aussi, qui complète les schémas et débloque W0.7 (corpus de
fixtures) puis W0.8 (SDK généré). Sa dépendance — les schémas de W0.5 — est satisfaite par cette PR,
donc par sa fusion.

## 2026-08-13 — W0.6 `[M]` — JSON Schemas LEP, seconde moitié

Premier sprint sous la règle révisée : CI verte → item suivant, sans attendre d'arbitrage pour ce
qui ne dévie pas du cadre. La porte `[M]` de W0.5 et W0.6 tombe ; ce qui la remplace est ce plan de
rollback, écrit et vérifié.

**Périmètre.** Sept schémas — `lep/1.0/{lease,attempt,event,sandbox-attestation,epistemic-commit}`,
`artifacts/1.0/{artifact-manifest,run-manifest}` —, `schemas/registry.json`, `schemas/README.md`,
onze tests dans `tests/schemas/lep.test.ts`, et deux retouches à la fixture
`sandbox-attestation.json` : débordement déclaré ci-dessous.

**Tests exécutés.** Test de sortie de l'item : « validation ». `npm run check:schemas` → ok, cinq
exemples validés, **plus aucun `pending`** — le registre est désormais exhaustif. `npm run check`
complet → 0 : 74 tests JS, 27 tests Rust, 0 échec.

**Décisions prises.** Trois contraignent la suite.

**Un commit ne peut pas se valider lui-même.** `EpistemicCommit.status` n'accepte que `draft` ou
`staged`. Le reste du cycle de vie de §7.4 existe, mais ce sont des verdicts que l'institution
prononce. §2.3 l'écrivait en prose ; c'est maintenant un document littéralement invalide, ce qui est
la seule forme d'interdiction qu'un worker ne peut pas contourner par inattention.

**Une attestation a le droit de décrire une mauvaise sandbox.** `host_home_mounted: true` est un
document **valide**. Refuser au niveau du schéma ne rendrait pas le montage impossible : ça rendrait
le worker non conforme incapable de l'avouer, et un worker qui ment par construction est pire qu'un
worker qui déclare une mauvaise isolation. Le refus appartient à l'admission. En revanche une
attestation **muette** est invalide — le champ est obligatoire même quand la réponse est `false`,
parce qu'un champ absent se lit « je n'ai pas regardé » aussi bien que « non ».

**Les types d'événements sont fermés, contrairement aux documents.** W0.5 avait laissé les documents
ouverts pour que le mineur reste compatible. L'inverse vaut pour `event_type` : un champ inconnu
s'ignore, un **type** inconnu ne s'ignore pas — le consommateur ne saura ni quoi en faire ni s'il
vient de rater quelque chose. Un nouveau type est un ajout mineur qui met cette liste à jour.

**Ce qu'un schéma ne peut pas dire, écrit dans le schéma.** §12.3 exige un heartbeat inférieur au
tiers du TTL. C'est une relation entre deux champs, que Draft 7 n'exprime pas. La contrainte est
donc renvoyée au harnais de conformance (W0.9) et la limite est écrite dans la description de
`Lease` : croire qu'une garantie existe est pire que savoir qu'elle manque.

**Débordement déclaré.** `sandbox-attestation.json` portait le même placeholder `sha256:...` que les
deux fixtures de W0.5, réparé de la même façon (`sha256("sbx-example")`), et ne portait **aucun**
bloc `_fixture` alors que `schemas/examples/README.md` déclarait son `expect` en prose. Le bloc est
ajouté pour que la machine lise ce que l'œil lisait déjà — les quatre autres fixtures le portent.

**Écart avec la spec.** Un, nommé : l'enveloppe du journal institutionnel (§10.1) n'est pas écrite
ici. Elle vit sous `schemas/events/` et appartient à W1. Un worker ne modifie jamais directement la
base canonique (invariant 3) : ce qui traverse le fil et ce qui est écrit dans l'event store ne
peuvent pas être le même objet.

**Plan de rollback (`[M]`).** Additif comme W0.5, et cette fois sans même de dépendance nouvelle :
aucun paquet ajouté, aucun consommateur des schémas. `git revert` du merge commit, sans étape de
données. Le seul effet non neutre du revert est que les trois placeholders réparés reviendraient —
`schemas/examples/` retrouverait des digests `...` que les schémas de W0.5, eux, resteraient à
refuser, donc un revert de W0.6 seul laisserait `check:schemas` **rouge** sur
`sandbox-attestation.json`. Le revert correct est celui de W0.5 et W0.6 ensemble, ou bien conserver
les fixtures réparées.

**Prochain item.** W0.7 `[R]` — corpus de fixtures : nominal, refus d'admission, reconnexion,
résultat tardif, dépassement de budget. Ses dépendances sont satisfaites : les schémas des deux
moitiés existent, et `expect: invalid` est déjà géré par le validateur, posé en W0.5 pour que
l'arrivée du corpus ne soit pas une surprise.

## 2026-08-13 — W0.7 `[R]` — corpus de fixtures

**Périmètre.** Neuf fixtures neuves dans `schemas/examples/`, `schemas/registry.json` (vocabulaire
des résultats), `tooling/schemas/validate.ts` (le vocabulaire quitte le code pour la donnée),
`schemas/examples/README.md`, sept tests dans `tests/schemas/lep.test.ts`.

**Tests exécutés.** Test de sortie : « chaque fixture valide **ou invalide intentionnellement**,
selon son `expect` déclaré ». `check:schemas` → ok sur les quinze fixtures, dont trois qui doivent
échouer et échouent. `npm run check` → 0 : 81 tests JS, 27 tests Rust.

Vérifié par mutation, dans le sens qui compte ici : en rendant `invalid-commit-self-validated.json`
valide, le validateur lève `example-unexpectedly-valid`. Une fixture invalide qui cesserait de
l'être serait autrement le genre de régression que rien ne signale.

**Décisions prises.** Trois.

**Le vocabulaire des `expect` passe du code à la donnée.** Il était une constante dans `validate.ts`
; il est maintenant une table de `schemas/registry.json`, chaque valeur portant une note qui dit ce
qu'elle signifie. W0.7 en ajoute trois — `replayed`, `quarantined`, `budget-exceeded` — et il
fallait choisir entre allonger une constante que personne n'explique et tenir une table qui
s'explique. Un `expect` absent de la table fait échouer la validation.

**Une fixture est un document, pas un scénario enrobé.** La reconnexion a d'abord été écrite comme
un fichier unique portant `acknowledged_through` et un tableau d'événements — un objet qui n'existe
nulle part sur le fil et qu'aucun schéma ne décrit. Elle est devenue quatre fichiers, chacun un
véritable événement LEP. Le quatrième est **byte-identique** au troisième hors bloc `_fixture`, et
c'est exactement la propriété à démontrer : un rejeu est un envoi, pas une note de bas de page. Un
test compare les deux corps.

**Le corpus exerce enfin `expect: invalid`.** Le chemin existait depuis W0.5 sans qu'aucune fixture
ne l'emprunte. Les trois documents invalides ne sont pas des erreurs qu'on aurait oublié de corriger
: ils portent les garanties les plus fortes de W0.5 et W0.6 — un worker ne valide pas son propre
commit, une attestation muette n'est pas une attestation, une borne de budget libre rend le
dépassement inconstatable. Ce sont eux que le harnais de W0.9 rejouera contre une implémentation
tierce.

Deux fixtures ne se contentent pas de s'étiqueter, elles se prouvent : un test vérifie que le
résultat tardif se termine réellement après l'expiration de `lease-expired.json`, et un autre que la
paire de refus est bien inadmissible — S3 exigé, S1/S2 offerts. Une fixture qui affirmerait « tardif
» sans que ses dates le montrent ne serait qu'une étiquette.

**Écart avec la spec.** Aucun. Les cinq scénarios nommés par `docs/10` sont écrits.

**Prochain item.** W0.8 `[R]` — SDK généré depuis les schémas plus `schema-registry` avec
négociation de features au handshake. Test de sortie : round-trip sur toutes les fixtures.
Dépendances satisfaites : les schémas des deux moitiés et le corpus existent. ADR 0011 condition 4
prévient que W0.8 génère **deux** SDK, TypeScript et Rust, depuis les mêmes schémas — c'est un coût
budgété, pas à découvrir.

## 2026-08-13 — W0.8 `[R]` — SDK généré et négociation de features

**Périmètre.** `tooling/sdk/` (IR partagée, deux émetteurs, CLI avec `--check`), `packages/lep/`
(unité neuve : `@locus/lep` et le crate `locus-lep`, code généré plus la négociation),
`schemas/lep/1.0/features.json`, `Cargo.toml` (membre), `package.json` (`sdk` et `check:generated`
dans la chaîne), `tests/sdk/lep.test.ts` et `packages/lep/tests/round_trip.rs`.

**Tests exécutés.** Test de sortie : « round-trip sur toutes les fixtures ». Côté Rust, six tests
décodent puis ré-encodent les onze fixtures valides et comparent ; côté TypeScript, dix tests dont
un qui vérifie que les types **couvrent tous les champs** de chaque fixture. `npm run check` → 0 :
91 tests JS, 33 tests Rust.

Vérification par mutation de la garde anti-dérive : une ligne ajoutée à la main dans `generated.ts`
fait sortir `generated-stale`.

**Décisions prises.** Quatre.

**Une seule lecture des schémas, deux émetteurs.** L'IR existe pour ça. Deux lecteurs divergeraient,
et la divergence se manifesterait comme un client TypeScript et un serveur Rust en désaccord sur le
fil — précisément le défaut pour lequel `docs/06` invente des fixtures inter-SDK.

**Le code généré est committé, pas construit.** `packages/protocol` est un crate Rust et `tooling/`
est du Node : une étape de génération à exécuter avant que l'un ou l'autre compile ferait attendre
chaque écosystème sur l'autre. Le prix est la dérive, et `--check` est ce qui la rend impossible
plutôt que seulement improbable. Le générateur passe sa sortie TypeScript par prettier, sans quoi
`check:format` et `check:generated` se contrediraient — l'un exigeant un reformatage que l'autre
signalerait comme une dérive.

**Ce que le générateur ne sait pas modéliser est un `finding`, jamais un silence.** Un générateur
qui saute ce qu'il ne comprend pas produit des types qui ont l'air complets et ne le sont pas, et le
premier à s'en apercevoir est celui qui débogue un champ disparu. Six règles couvrent les cas :
`oneOf` non réductible à un motif, énumération non textuelle, tableau sans `items`, objet imbriqué
sans nom dérivable, document sans `properties`, et définitions homonymes venues de deux fichiers.

**`schema-registry` est un module de `packages/lep`, pas une unité à part.** Le registre est celui
des schémas que le SDK porte ; l'en séparer créerait un paquet dont tout le contenu serait une table
de ce que contient l'autre. La liste des features vit dans `schemas/lep/1.0/features.json` et se
génère dans les deux SDK, comme le reste. Les cinq features sont **sourcées** — chacune est une
capacité que la spec décrit comme facultative ou conditionnelle, avec sa référence. Une feature
qu'aucun document ne nomme n'aurait personne pour l'implémenter.

La négociation distingue **trois** issues et non deux : accordée, refusée, inconnue. Les fondre en
un seul « non » rendrait un pair venu d'un mineur ultérieur indiscernable d'un pair qui a mal
orthographié son besoin — le premier appelle un repli, le second un rapport d'erreur.

**Ce que le round-trip a trouvé, et qui dépasse W0.8.** La fixture écrit `"cpu": 4` ; le schéma dit
`number` — pour les cœurs fractionnaires — donc le SDK Rust type `f64` et ré-encode `4.0`. JSON ne
distingue pas les deux, et aucun lecteur conforme ne rapporte lequel a été écrit. La conséquence
n'est pas dans le test : **le `payload_hash` d'un événement ne peut pas être calculé sur la sortie
d'un sérialiseur.** Deux pairs conformes émettraient des octets différents pour la même donnée et
leurs hashes divergeraient sur rien. §7.7 exige « une canonicalisation stable » : c'est elle qui
doit produire les octets à hasher, ni `serde_json::to_string` ni `JSON.stringify`. Le canonicaliseur
appartient à W0.9 ; la note est dans le test pour qu'il ne soit pas oublié.

**Écart avec la spec.** Aucun. La dérogation `missing_docs` du fichier Rust généré est écrite dans
son en-tête : la documentation de ces types EST la description de leur schéma, et inventer une
phrase pour satisfaire un lint ajouterait du bruit là où le silence est exact. Ce qui manque doit
être ajouté au schéma, pas au générateur.

**Prochain item.** W0.9 `[R]` — `packages/testing` : harnais de conformance LEP côté serveur
(handshake, offre, lease, heartbeat, expiration, acquittements). Test de sortie : le harnais se
teste contre un worker factice. Dépendances satisfaites : schémas, corpus et SDK existent. Il hérite
de deux dettes nommées ici — le canonicaliseur, et la règle « heartbeat < TTL/3 » que Draft 7 ne
sait pas exprimer.

## 2026-08-13 — W0.9 `[R]` — harnais de conformance LEP

**Périmètre.** `packages/testing/` (unité neuve `@locus/testing` : canonicaliseur, port du worker
sous test, huit vérifications), `tests/testing/harness.test.ts`, `package-lock.json`.

**Tests exécutés.** Test de sortie : « le harnais se teste contre un worker factice ». Dix-huit
tests, dont un worker conforme qui ne produit aucun constat et **sept workers délibérément fautifs
que le harnais attrape**. `npm run check` → 0 : 109 tests JS, 33 tests Rust, après un `npm ci`
depuis un `node_modules` vide.

**Décisions prises.** Quatre.

**Le harnais joue le serveur, et rend un rapport plutôt qu'un verdict.** `docs/10` §W2 en donne la
raison : écrire le worker contre un faux serveur oblige le protocole à être suffisant avant que
`locusd` puisse compenser ses lacunes. Chaque vérification rend des `Finding` et jamais une
exception : un harnais qui s'arrête à la première faute ne dit pas si les suivantes existent. Le
rapport porte aussi la liste des vérifications exécutées — « rien à signaler » et « rien vérifié »
ne doivent pas se ressembler, c'est la même règle qu'en W0.3.

**Le port du worker est sans transport.** LEP nomme WebSocket comme référence et autorise un mode
pull/queue (§15.2) ; un harnais qui imposerait l'un des deux ne testerait pas le protocole mais son
enrobage. Et les événements sont **consommés** plutôt qu'attendus en temps réel : une conformance
qui dépendrait d'horloges serait intermittente, et un test intermittent finit désactivé.

**Les deux dettes héritées sont honorées.** Le canonicaliseur (RFC 8785 sur les points qui comptent
: clés triées, nombres écrits comme ECMAScript les écrit, aucun espace) rend `4` et `4.0` identiques
— c'est ce que W0.8 avait établi comme nécessaire pour que `payload_hash` ne diverge pas entre pairs
conformes. Et la règle « heartbeat < TTL/3 », que Draft 7 ne savait pas exprimer, est vérifiée ici.

Le canonicaliseur **s'arrête** sur ce qu'il ne sait pas représenter — `NaN`, l'infini, un entier
hors de la plage exacte d'un `double`. Rendre quelque chose produirait un hash, et un hash faux
ressemble en tout point à un hash juste.

**Refuser une mission n'est pas une faute.** La politique locale d'un worker peut être plus
restrictive que son manifeste (§10.2) ; un worker qui accepte tout est le vrai défaut. Le harnais ne
signale que l'inverse : accepter au-dessus de ses moyens, ce que la paire de refus du corpus existe
précisément pour attraper.

**Deux corrections que les tests ont provoquées.** Le harnais lisait `>` là où §12.3 écrit «
intervalle **inférieur** au tiers du TTL » : un tiers pile n'est pas inférieur à un tiers, et un
worker qui bat exactement trois fois par TTL n'a aucune marge — le premier battement en retard fait
expirer la lease. Le test encodait la spec plus strictement que l'implémentation, et c'est lui qui
avait raison.

Et deux de mes propres fixtures de test omettaient le heartbeat, ce que la vérification a signalé au
premier passage. Le harnais a donc attrapé son auteur avant d'attraper qui que ce soit d'autre.

**Écart avec la spec.** Aucun. Le rejeu est explicitement toléré — même séquence **et** même clé
d'idempotence — sans quoi le harnais interdirait la reprise de stream que §12.4 exige. Une même
séquence avec une autre clé reste une faute : ce sont deux événements qui se disputent une place.

**Prochain item.** W0 est terminé. La suite est W2, « exécutable dès la fin de W0 » et explicitement
indépendant de W1 : le harnais livré ici joue le serveur contre lequel le worker Canterel s'écrit.
W2.1 et W2.2 vivent dans `maribakulj/canterel` et ne dépendent que de ce dépôt-ci pour le SDK,
désormais publié. En parallèle, W1 (domaine et event store) est ouvert côté `locusolus` et n'attend
rien.

## 2026-08-17 — W1.a — enveloppe commune d'objet épistémique (§7.4)

**Périmètre.** Un crate neuf : `packages/domain` — `envelope.rs`, `status.rs`, `validation.rs`,
`lineage.rs`, `ids.rs`, `hash.rs`, plus `tests/envelope_invariants.rs`. Ajouté aux membres du
workspace Cargo. Aucun fichier existant modifié en dehors de cette ligne.

**Tests exécutés.** `cargo test --all-features` : 10 property tests sur le domaine, 43 au total sur
le workspace, 0 échec. `npm run check` : les neuf portes vertes — format, repo, naming, boundaries,
schemas, generated, typecheck, tests Node, Rust (fmt + clippy `-D warnings` + tests).

Le test de sortie de W1.a — « property tests sur les invariants » — passe. Vérifié par mutation,
trois fois : faire hériter le niveau de validation d'une révision fait rougir §7.4 ; réattribuer le
`stable_id` à chaque révision fait rougir §7.7 ; faire perdre à un merge son prédécesseur unique
fait rougir deux tests de lignée.

**La garde de frontières a été vérifiée sur ce crate, pas supposée.** `packages/domain` est le
premier code que la règle 1 de `boundaries.json` avait à surveiller — jusqu'ici elle tournait sur
zéro fichier. Un `use std::fs` glissé dans `status.rs` la fait échouer avec
`domain-imports-no-infrastructure`, et le retirer la fait repasser. La règle scanne désormais neuf
fichiers réels.

**Décisions prises.** Cinq.

_`validation_level` n'est dérivable d'aucun statut._ §7.4 : « `validation_level` décrit la force
épistémique et ne doit pas être déduit du seul statut ». Il n'existe donc aucune conversion entre
les deux, et le property test montre que **les soixante-dix combinaisons sont représentables** — y
compris `validated` avec `L0`, qui décrit un objet ayant traversé le processus sans qu'aucune preuve
n'ait été produite. Un type qui interdirait cette combinaison aurait déjà déduit le niveau.

_`ValidationLevel` n'est ni `Ord` ni `PartialOrd`._ §8.1 : « ces niveaux ne forment pas toujours une
chaîne totale. Une interprétation historique peut atteindre L3 et L6 sans être “reproduite” ».
Dériver `Ord` écrirait dans le type une affirmation que la spec dément — et ce ne serait pas un
défaut théorique : `if level >= Reproduced` compilerait, se lirait bien, et refuserait une
interprétation historique parfaitement validée parce qu'aucune expérience ne l'a répliquée. `rank()`
reste accessible comme **étiquette**, avec la note qui dit ce qu'elle n'est pas.

_La lignée est une énumération, pas un `Vec`._ §7.7 dit deux choses qui semblent se contredire : «
au plus un prédécesseur direct » et « un merge peut créer une révision avec plusieurs parents
déclarés ». Elles ne parlent pas de la même chose. La lignée est une chaîne — c'est elle qui donne
un sens à « la version précédente » ; les parents d'un merge sont de la provenance. Les fondre dans
un même `Vec<RevisionId>` ferait de « la version précédente » une question sans réponse dès le
premier merge. `Lineage::{Root, Successor, Merge}` rend l'unicité vraie **par construction**, merge
compris.

_Une révision repart en `draft` / `L0`._ Hériter du statut et du niveau ferait franchir à un contenu
modifié une validation qui portait sur un autre contenu — la manière exacte dont une preuve se perd
sans que personne ne s'en aperçoive. Les `evidence_refs` ne suivent pas non plus, pour la même
raison ; la provenance, elle, suit, parce qu'elle dit d'où vient l'objet et non ce qu'il vaut.

_Le domaine ne calcule aucun hash._ `ContentHash` vérifie la **forme** — préfixe obligatoire,
longueur par algorithme, hexadécimal minuscule non normalisé — et rien de plus. Choisir une
implémentation de hash est une décision d'infrastructure, et l'invariant 1 l'exclut d'ici. Même
raison pour l'horloge et l'aléa : `revise()` reçoit l'instant et l'identifiant, elle ne les fabrique
pas. Le crate reste pur, donc rejouable.

**Écart avec la spec.** Trois notes.

_Les préfixes `obj_` et `rev_` sont provisoires._ Aucun document ne montre d'exemple d'identifiant
d'objet épistémique, là où `evt_01…` apparaît littéralement au §10.1. Ils sont marqués comme tels,
au même titre que `msn` et `err` de `locus_protocol::id::provisional`. W1.b ou W1.c les confirmeront
ou les remplaceront : ce sera une modification de schéma, pas de code.

_`supersedes_revision_id` de §7.4 est exposé en lecture, pas en champ._ C'est la lignée qui porte la
garantie d'unicité ; un champ nu se laisserait écrire deux fois. `Envelope::supersedes()` rend
exactement ce que le YAML de §7.4 nomme.

_Pas de `proptest` ni de `quickcheck`._ Le workspace ne dépend que de `serde` et `serde_json`, et
ajouter une bibliothèque de génération pour dix propriétés paierait une dépendance permanente pour
un confort ponctuel. Le générateur congruentiel du fichier de test tient en vingt lignes et il est
**déterministe** — un échec se rejoue en relançant, sans graine à recopier depuis une sortie CI. Ce
qu'on perd est la réduction des contre-exemples, et c'est tout. Si W1.b ou W1.f demandent des
espaces qu'un LCG ne balaie pas honnêtement, la dépendance se justifiera à ce moment-là.

**Prochain item.** W1.b `[R]` — agrégats organisationnels (§7.1) et objets épistémiques (§7.3), test
de sortie « property tests ». Ses dépendances sont satisfaites : l'enveloppe livrée ici est ce que
§7.3 enveloppe, et §7.1 s'écrit sur les mêmes identifiants typés.

## 2026-08-17 — W1.b — agrégats organisationnels (§7.1) et objets épistémiques (§7.3)

**Périmètre.** Trois modules ajoutés à `packages/domain` : `branch.rs`, `task.rs`, `objects.rs`,
plus `tests/aggregate_invariants.rs`. Aucun autre fichier touché.

**Tests exécutés.** `cargo test --all-features` : 15 property tests neufs, 25 sur le domaine, 58 au
total sur le workspace, 0 échec. `npm run check` : les neuf portes vertes.

Le test de sortie de W1.b — « property tests » — passe. Vérifié par mutation, quatre fois : rendre
`merged` non terminal, faire valoir un témoin vide pour une validation, laisser une extension
masquer un type core, ou faire sauter une attente au résultat font rougir cinq tests au total.

**Décisions prises.** Cinq.

_Le graphe d'états de `Branch` n'est pas inventé._ §7.1 liste les dix états d'une branche mais **ne
dessine aucune flèche**, contrairement à `Task`. `transition` refuse donc exactement deux choses —
sortir de `merged`, atteindre `validated` — parce que ce sont les deux seules que le texte interdit.
Écrire une table de transitions complète ici interdirait des passages que personne n'a interdits, et
une table inventée est plus difficile à corriger qu'une table absente : elle a l'air d'une décision.

_`validated` demande un témoin, pas un drapeau._ Les conditions viennent d'une politique
(`review_policy_id`) que ce crate ne connaît pas, et §8.2 en fait le travail des packs
disciplinaires. Ce que le domaine peut garantir, c'est qu'**on ne passe pas à `validated` sans avoir
répondu à la question** : `ValidationWitness` porte les conditions et leur verdict, et le refus les
nomme. Un témoin vide est refusé — « aucune condition » n'est pas « toutes satisfaites », c'est une
politique qu'on n'a pas lue.

_`reopen` porte son nom._ §7.1 : « `merged` est terminal **sauf opération explicite `reopen`** ».
Permettre une transition ordinaire depuis `merged` en aurait fait une réouverture qui ne dit pas son
nom.

_L'origine d'une branche est une énumération._ Invariant 2 : « un fork référence **exactement** la
révision d'origine ». `forked_from_branch_id` et `fork_revision` ne se remplissent donc jamais l'un
sans l'autre : une branche qui saurait de quelle branche elle est issue sans savoir à quelle
révision aurait un point de départ qui bouge quand l'origine avance. Même geste que `Lineage` en
W1.a — un couplage rendu vrai par construction plutôt que par un contrôle à réécrire dans chaque
constructeur.

_Une extension ne peut pas porter le nom d'un type core._ §7.3 : « les extensions ne doivent pas
modifier la signification des types core ». Un pack qui déclarerait son propre `Claim` ne
modifierait pas le type core — il le **remplacerait**, silencieusement, et le graphe contiendrait
deux notions de `Claim` qu'aucune lecture ultérieure ne saurait séparer. `ObjectType::parse` refuse
donc l'homonymie, ce qui est la seule interprétation de la phrase qui reste vraie une fois le pack
installé. Le test la vérifie sur les quarante noms.

**Une fonction qui rend toujours `false`.** `implies_validated_claims` existe pour que la phrase de
§7.1 — « une tâche `succeeded` signifie que le worker a rempli son contrat technique ; elle ne
signifie pas que ses claims sont validés » — soit **écrite quelque part** plutôt que sous-entendue.
Le jour où quelqu'un voudra la faire rendre `true` pour un cas particulier, il faudra qu'il
l'écrive, et le diff le montrera. C'est le même geste que l'absence de `setBalance` en W2.13 ou de
`promote` en W2.15, pris par l'autre bout.

**Écart avec la spec.** Une note. §7.1 décrit six agrégats organisationnels — `Project`,
`ResearchProgram`, `Workstream`, `Branch`, `Task`, `AgentTemplate` — et ce sprint en livre deux :
ceux qui portent des **invariants énoncés** et une machine à états. Les quatre autres sont des
listes de champs sans contrainte propre à ce stade ; les écrire maintenant produirait des structures
que rien ne teste, ce que `docs/10` interdit explicitement (« ne crée pas 34 stubs vides : chaque
commit livre une garantie testée »). Ils viendront avec W1.c, qui leur donnera un event store et
donc des invariants de persistance.

**Correction de méthode.** L'entrée de W1.a a fait rougir `check:format` en CI : j'avais lancé
`npm run check` **avant** d'écrire au ledger, donc la porte n'avait rien à voir localement. Corrigé
en une passe, sans changement de contenu. Sur ce dépôt, la vérification finale vient après
l'écriture du ledger.

**Prochain item.** W1.c `[M]` — `packages/event-store` : enveloppe de §10.1, append-only logique,
concurrence optimiste. Test de sortie : « replay complet + conflit de concurrence détecté ». Premier
item `[M]` de W1 : il demande donc un ADR et un plan de rollback.

## 2026-08-17 — W1.c `[M]` — `packages/event-store` : enveloppe, append-only, concurrence optimiste

**Périmètre.** Un crate neuf : `packages/event-store` — `envelope.rs`, `store.rs`, `memory.rs`, plus
`tests/contract.rs`. Un ADR : `docs/adr/0012-port-event-store-avant-driver.md`. Ajouté aux membres
du workspace Cargo.

**Tests exécutés.** `cargo test --all-features` : 15 contract tests neufs, 73 au total sur le
workspace, 0 échec. `npm run check` : les neuf portes vertes.

Le test de sortie de W1.c — « replay complet + conflit de concurrence détecté » — passe. Vérifié par
mutation, trois fois : désactiver le contrôle de révision fait rougir les deux tests de concurrence
; vérifier l'idempotence **après** la concurrence fait rougir les deux tests de rejeu ; laisser
passer un lot mal formé fait rougir l'atomicité.

**Quatre décisions, toutes dans l'ADR 0012.** Résumées ici, argumentées là-bas.

_Le port et sa suite de contract tests avant tout driver._ `CLAUDE.md` demande des ports purs avant
tout branchement. La suite est écrite **contre le trait**, jamais contre l'implémentation en mémoire
: le jour où le driver PostgreSQL existe, c'est elle qui décidera s'il est conforme — pas sa
documentation, pas sa relecture. Écrite après lui, elle documenterait ce qu'il fait ; écrite avant,
elle dit ce qu'il doit faire. Même geste que les self-tests de sandbox d'ADR 0004.

_`Expected` n'a pas de variante « peu importe »._ §10.2 dit « optimistic concurrency **par**
`expected_stream_revision` ». Un écrivain qui ne sait pas sur quelle révision il construit n'a rien
vérifié : ce qu'il produit n'est pas un append concurrent réussi, c'est un conflit qu'on n'a pas
regardé. La plupart des journaux offrent un `Any` par commodité, et c'est par cette commodité que
les invariants d'agrégat se perdent.

_L'idempotence est vérifiée avant la concurrence._ Une commande rejouée a fait avancer le stream,
donc son `expected` est périmé **par sa propre faute**. Vérifier la concurrence d'abord lui
opposerait sa propre écriture, et l'appelant obtiendrait un doublon en relisant puis en retentant.
Un contract test verrouille l'ordre.

_`Draft` et `Envelope` sont deux types._ « Ordre total par stream » n'est pas une propriété à
vérifier après coup, c'est une propriété à rendre non violable : le producteur ne peut pas poser un
rang parce que le champ n'existe pas chez lui. Idem pour `recorded_at`, qui est un fait du journal —
et sa distinction d'avec `occurred_at` n'est pas décorative, un worker hors ligne (§24.3) produit
des actes dont l'écriture suit de plusieurs heures.

**Écart avec la spec.** Deux notes.

_Le namespace d'un type d'événement est vérifié, le verbe ne l'est pas._ §10.3 donne les familles
avec un `*` et n'énumère aucun verbe. Fermer la liste des verbes interdirait un événement que la
spec autorise ; fermer celle des namespaces attrape la faute qui compte — un événement rangé dans
une famille inexistante est un événement qu'aucune projection n'ira chercher. `EVENT_NAMESPACES`
porte les vingt-huit familles du texte plus deux ajouts locaux, `projection` et `migration`,
**signalés comme tels** en commentaire plutôt que fondus dans la liste normative.

_Trois garanties de §10.2 ne sont pas portées par ce commit_ : signature de fédération, upcasters de
migration (W1.h), snapshots reconstruisibles (W1.d). Nommées dans le module et dans l'ADR pour qu'on
ne les croie pas oubliées.

**Plan de rollback.** Dans l'ADR, section dédiée. En résumé : avant W1.d, revenir coûte la
suppression du crate et d'une ligne de `Cargo.toml` — rien d'autre n'en dépend, `packages/domain` ne
le connaît pas. Après W1.d, seule la décision 4 a un rollback coûteux, et c'est pourquoi elle est
prise maintenant. Aucune donnée n'est en jeu : l'implémentation de référence est en mémoire, et
aucun journal persistant n'existe.

**Prochain item.** W1.d `[M]` — projections reconstructibles. Test de sortie : « reconstruction
depuis zéro = état courant ». Ses dépendances sont satisfaites : le replay total et ordonné livré
ici est exactement ce dont une reconstruction a besoin, et l'export brut lui donne l'ordre global.

## 2026-08-17 — W1.d `[M]` — projections reconstructibles (§9.3, §9.5)

**Périmètre.** Un crate neuf : `packages/projections` — `projection.rs`, `runner.rs`, `verify.rs`,
`validation_state.rs`, `conflict_registry.rs`, plus `tests/rebuild.rs`. Une addition au port de W1.c
: `EventStore::feed` et le type `Sequenced`. Un ADR :
`docs/adr/0013-projections-reconstructibles.md`.

**Tests exécutés.** `cargo test --all-features` : 10 tests neufs, 83 au total sur le workspace, 0
échec. `npm run check` : les neuf portes vertes.

Le test de sortie de W1.d — « reconstruction depuis zéro = état courant » — passe, sur les deux
projections livrées et sur huit graines de journal. Vérifié par mutation, trois fois : laisser
`reset` garder le watermark, faire sauter l'événement fautif au lieu de s'arrêter, ou faire oublier
au registre les conflits résolus font rougir quatre tests au total.

**Cinq décisions, toutes dans l'ADR 0013.** Résumées ici.

_Le journal gagne une position globale, hors de l'enveloppe._ §9.5 demande que chaque projection
expose son dernier `event_sequence` — encore faut-il qu'il y en ait un. §10.1 n'en met pas dans
l'enveloppe, et `stream_revision` est un rang **dans un stream** : deux streams portent tous deux
une révision 1. La position vit donc à côté de l'enveloppe, dans `Sequenced`, parce que l'enveloppe
est le document normatif que deux SDK doivent produire à l'identique.

_`reset` et `checksum` sont dans le port._ `reset` n'est pas une facilité d'opérateur : c'est la
propriété qui rend une projection **secondaire**. Une projection qu'on ne saurait pas reconstruire
serait une seconde source de vérité, ce que §9.1 réserve au journal. La mettre dans le trait oblige
chaque projection à répondre à la question au moment où elle est écrite, plutôt qu'à la découvrir le
jour d'un incident.

_Une projection en défaut s'arrête, elle ne saute pas._ Sauter l'événement fautif donnerait un état
que la reconstruction ne reproduirait pas — elle rencontrerait la même faute au même endroit — et «
reconstruction depuis zéro = état courant » deviendrait faux. C'est aussi décider unilatéralement
qu'un événement canonique n'a pas d'importance, ce qu'une projection n'a pas autorité pour faire.

_La quarantaine ne bloque pas l'écriture, et cela tient par la forme._ Le pilote reçoit le journal
par référence partagée et n'a **aucun chemin d'écriture** : une projection en défaut ne peut pas
empêcher un append parce qu'il n'existe pas de moyen par lequel elle l'atteindrait. `catch_up` ne
rend jamais d'erreur — la faire remonter inviterait un appelant à la propager jusqu'à un chemin
d'écriture, ce que §9.5 interdit.

_Deux projections, pas douze._ §9.3 en liste douze ; celles que le domaine de W1.a et W1.b permet
d'écrire honnêtement sont deux. Elles suffisent à éprouver le port : la propriété de reconstruction
est vérifiée sur les deux, donc elle porte sur le trait et non sur une implémentation.

**Un détail qui aurait pu passer.** `verify` reconstruit sur une projection **neuve**, jamais sur
celle qu'elle vérifie. Une vérification qui détruirait son sujet réparerait la divergence en même
temps qu'elle la découvrirait, ce qui est la définition d'une réparation silencieuse — celle que
§24.5 interdit ailleurs pour la même raison. Le test le vérifie : après un `verify`, le watermark de
la projection vivante n'a pas bougé.

**Écart avec la spec.** Une note. Le cas réservé de §9.5 — « sauf si elles concernent une projection
synchrone nécessaire à un invariant » — n'est pas implémenté : aucune projection de ce paquet n'est
synchrone, et écrire le mécanisme avant d'avoir le cas produirait une abstraction que rien ne teste.
Nommé dans le module et dans l'ADR.

**Plan de rollback.** Dans l'ADR. En résumé : la décision 1 est **additive** — `feed` s'ajoute au
trait sans toucher aux quatre méthodes existantes. Les décisions 2 à 5 sont contenues dans un crate
que rien ne consomme encore ; avant W1.e, revenir coûte une suppression et une ligne de
`Cargo.toml`. Après W1.e, seul le retrait de `reset` du port coûte une garantie plutôt qu'un diff.
Aucune donnée n'est en jeu : les projections sont reconstructibles par construction.

**Prochain item.** W1.e `[R]` — `packages/graph` : relations typées de §7.5 et **hyperarêtes** pour
les inférences multi-prémisses (§7.6). Test de sortie : « une inférence à 3 prémisses n'est pas 3
liens ». Ses dépendances sont satisfaites : les objets de §7.3 sont typés depuis W1.b, et le port de
projection livré ici est ce sur quoi les graphes de §9.3 se construiront.

## 2026-08-17 — W1.e — `packages/graph` : relations typées et hyperarêtes (§7.5, §7.6)

**Périmètre.** Un crate neuf : `packages/graph` — `relation.rs`, `inference.rs`, `graph.rs`, plus
`tests/hyperedges.rs`. Ajouté aux membres du workspace. Aucun fichier existant modifié en dehors de
cette ligne.

**Tests exécutés.** `cargo test --all-features` : 12 tests neufs, 95 au total sur le workspace, 0
échec. `npm run check` : les neuf portes vertes.

Le test de sortie de W1.e — « une inférence à 3 prémisses n'est pas 3 liens » — passe. Vérifié par
mutation, trois fois : aplatir les prémisses en ensembles d'une, laisser une relation à sens unique
se retourner, ou retirer à la règle et au scope leur cible d'objection font rougir six tests au
total.

**Comment le test de sortie s'y prend.** Il construit **les deux graphes** — l'hyperarête et la
réduction interdite en trois `supports` — et montre en cinq points ce que le second perd :

1. le compte : une hyperarête contre trois arêtes ;
2. ce qui étaye la conclusion : **un** support portant les trois prémisses, contre trois soutiens ;
3. les prémisses minimales (§9.4) : un ensemble de trois, contre trois ensembles d'un — c'est-à-dire
   « il faut ces trois faits » contre « il suffit d'un des trois », qui ne sont pas la même
   affirmation scientifique ;
4. réfuter **une** prémisse casse l'inférence entière, là où le graphe aplati laisse deux `supports`
   debout et la conclusion « encore soutenue aux deux tiers » ;
5. la règle et le scope ont un endroit où être contestés — sur trois arêtes, ils n'existent pas, et
   l'objection « le raisonnement ne tient pas même si tous les faits sont vrais » n'aurait aucune
   cible.

**Décisions prises.** Quatre.

_Le graphe range relations et inférences séparément, sans passerelle._ Ni `flatten`, ni `decompose`,
ni `as_edges`, ni `impl From<Inference>`. Un test verrouille ces absences, parce que c'est
exactement la fonction de commodité que quelqu'un finira par vouloir écrire — et §7.6 dit « NE DOIT
PAS » en majuscules.

_`Support` est une énumération, pas une liste d'arêtes._ Un appelant qui ne traiterait que le cas
binaire ne compile pas. Le type interdit de confondre une inférence avec une relation, là où une
liste homogène les aurait fondues à la première itération.

_Chaque relation déclare sa direction._ §7.5 : « les relations non symétriques ne doivent pas être
inférées en sens inverse ». `traversable_backwards` refuse **vingt-deux relations sur vingt-huit** :
deux sont symétriques (`contradicts`, `analogous_to`), quatre forment deux paires de réciproques
nommées (`generalizes`/`specializes`, `forked_from`/`merged_into`). `supports` n'est pas symétrique
— deux thèses qui s'étayent mutuellement sont deux relations écrites, pas une relation lue deux
fois. Lu à l'envers, `A supports B` ferait de la preuve une thèse ; `cites` ferait citer un article
de 2026 par un article de 1890.

_`None` ne veut pas dire « la réciproque est fausse »._ Il veut dire qu'elle **n'est pas
déductible**, et qu'affirmer quoi que ce soit dans ce sens demanderait de l'écrire comme une
relation à part entière. `incoming()` reste disponible : lire les arêtes entrantes est une lecture,
pas une déduction.

**Une distinction que §7.6 fait et qu'on aurait pu manquer.** Les hypothèses (`assumption_ids`) ne
sont pas des prémisses. Une prémisse est **affirmée**, une hypothèse est **admise** ; les confondre
ferait passer pour établi ce qui a seulement été supposé. Les prémisses minimales ne contiennent
donc pas les hypothèses, et réfuter une hypothèse ne casse pas l'inférence par le même chemin.

**Écart avec la spec.** Une note. `inference_kind` et `review_status` restent des chaînes ouvertes :
§7.6 ne donne aucune liste fermée pour l'un ni l'autre, et en fermer une interdirait un genre
d'inférence que les packs disciplinaires ont le droit d'ajouter. La `formalization_status`, elle,
est fermée à quatre valeurs — c'est une échelle de vérification, pas un vocabulaire disciplinaire.

**Une erreur d'arithmétique attrapée par le test.** J'avais écrit « vingt-quatre relations refusent
l'inversion » dans trois commentaires et une assertion ; c'est vingt-deux — 28 moins 2 symétriques
moins 4 en paires. Le test a rougi au premier passage et le compte est corrigé partout.

**Prochain item.** W1.f `[R]` — validation épistémique (§8) et propagation de l'invalidation (§8.3).
Test de sortie : « invalider une prémisse propage correctement ». Ses dépendances sont satisfaites :
`inferences_broken_by` livré ici est exactement le premier pas de la propagation, et les sept
niveaux de §8.1 sont typés depuis W1.a.

## 2026-08-17 — W1.f — validation épistémique et propagation de l'invalidation (§8)

**Périmètre.** Un crate neuf : `packages/validation` — `policy.rs`, `propagation.rs`, plus
`tests/propagation.rs`. Ajouté aux membres du workspace. Aucun fichier existant modifié en dehors de
cette ligne.

**Tests exécutés.** `cargo test --all-features` : 10 tests neufs, 105 au total sur le workspace, 0
échec. `npm run check` : les neuf portes vertes.

Le test de sortie de W1.f — « invalider une prémisse propage correctement » — passe. Vérifié par
mutation, quatre fois : arrêter la propagation au premier étage, cesser de conserver le niveau
antérieur, faire de `cites` une relation de dépendance, ou faire propager une politique tolérante
font rougir six tests au total.

**Le mot « correctement » du test de sortie.** Il porte dans les deux sens, et le test le vérifie
dans les deux : la propagation atteint la conclusion directe, le claim du second étage et l'objet
dérivé — **et** elle n'atteint pas l'autre prémisse de la même inférence, ni l'objet qui se contente
de citer. Une propagation qui marquerait tout serait aussi fausse qu'une qui ne marquerait rien.

**Les cinq points de §8.3, et où chacun est vérifié.**

_1. « Identifie les objets transitivement dépendants »._ Parcours en largeur, par les hyperarêtes de
W1.e et par cinq relations de dépendance. Le second étage est atteint à la distance 2, et la
mutation qui supprime l'enfilement de la file le fait rougir.

_2. « Ne les réfute pas automatiquement sans règle disciplinaire »._ C'est une contrainte sur ce que
le code a le droit de **rendre**, pas seulement sur ce qu'il fait : `Propagation` n'a aucun champ
portant un niveau de validation, et un test verrouille l'absence de `refute`, `demote`, `downgrade`,
`new_level`, `resulting_level`. Une propagation qui rendrait un niveau révisé aurait déjà pris la
décision qu'elle a interdiction de prendre — et l'appelant l'appliquerait, parce qu'un champ rendu
par une fonction a l'air d'un résultat.

_3. « Les marque `needs_reassessment` »._ Une marque par dépendant, avec sa distance et sa raison.

_4. « Ouvre des tâches de réévaluation selon la politique »._ Et **dit** quand il n'y a pas de
politique : une liste de tâches vide se lirait « rien à réévaluer », alors qu'elle veut dire « la
question est posée mais personne n'a de règle pour y répondre ».

_5. « Conserve le niveau et la justification antérieurs dans l'historique »._ Sans cette trace, une
réévaluation repartirait de zéro et le travail de validation qui avait mené à L3 serait **perdu** au
lieu d'être remis en question. Un dépendant dont on ne savait rien porte `None` et non L0 : « je ne
sais pas ce qu'il valait » et « il ne valait rien » ne sont pas la même information.

**Décisions prises.** Trois.

_La liste des relations de dépendance est courte, et `cites` n'y est pas._ Citer un article réfuté
ne rend pas l'article citant faux : ça le rend discutable. Marquer tout le corpus citant à chaque
rétractation noierait les vrais dépendants, et §8.3 vise « une définition, une source, un dataset ou
une prémisse » — ce dont un objet **dépend**, pas ce qu'il mentionne. Cinq relations retenues :
`depends_on`, `derived_from`, `instantiates`, `formalizes`, `anchored_in`, chacune avec sa raison en
commentaire.

_C'est la discipline qui déclare ce qui invalide._ §8.2, dernière puce. Une révision peut laisser
une conclusion debout dans un domaine et la faire tomber dans un autre ; une politique qui ne fait
pas de l'événement un invalidant arrête la propagation — et le **dit**, sans quoi « aucun dépendant
» et « la politique a refusé de propager » se ressembleraient.

_Le parcours tient un ensemble de visités._ Un graphe épistémique **contient** des cycles — deux
claims qui se soutiennent mutuellement, une définition qui s'appuie sur un cas qui l'instancie — et
une propagation qui ne les supporterait pas boucherait au premier corpus réel. Un test construit un
cycle à trois et vérifie que l'objet invalidé ne se remarque pas lui-même en repassant par la
boucle.

**Ce que §8.4 interdit, et qui n'existe pas ici.** « Les scores de confiance des agents […] ne
remplacent ni les preuves, ni les revues, ni les niveaux de validation. Une moyenne de confiance ne
constitue jamais une procédure de décision par défaut. » Aucune fonction du crate ne prend une
confiance en entrée ni n'en calcule la moyenne, et un test verrouille l'absence des mots
`confidence`, `mean_`, `average`, `fn score` dans les deux modules.

**Écart avec la spec.** Une note. §8.2 dit qu'un schéma disciplinaire « DOIT déclarer » six choses ;
`TypePolicy` les rend donc toutes obligatoires, y compris quand la réponse est une liste vide. Une
liste vide reste une décision — « aucune revue obligatoire » — mais deux d'entre elles la rendent
suspecte, et `findings()` le signale : sans preuve minimale, une discipline ne valide rien ; sans
événement invalidant, la propagation de §8.3 ne se déclenche jamais.

**Prochain item.** W1.g `[R]` — résultats négatifs et conflits (§18.7). Test de sortie : « aucun
chemin de code ne supprime un conflit ». Ses dépendances sont satisfaites : le registre des conflits
de W1.d porte déjà l'invariant 12 côté lecture, et les types `NegativeResult` et `Conflict` sont
dans les quarante de W1.b. C'est aussi le dernier item `[R]` avant W1.h, qui clôt W1.
