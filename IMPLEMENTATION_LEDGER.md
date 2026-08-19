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

## 2026-08-17 — W1.g — résultats négatifs et conflits (§18.7, §18.4, invariant 12)

**Périmètre.** Deux modules ajoutés à `packages/domain` : `negative_result.rs` et `conflict.rs`,
plus `tests/negative_and_conflict.rs`. Aucun autre fichier touché.

**Tests exécutés.** `cargo test --all-features` : 12 tests neufs, 117 au total sur le workspace, 0
échec. `npm run check` : les neuf portes vertes.

Le test de sortie de W1.g — « aucun chemin de code ne supprime un conflit » — passe. Vérifié par
mutation, quatre fois.

**Le test de sortie balaie tout le workspace, pas seulement son module.** C'est le point de ce
sprint. Une garantie qui ne tiendrait que dans le fichier qui la déclare n'en est pas une : le
retrait viendra d'ailleurs, d'un paquet voisin écrit six mois plus tard par quelqu'un qui n'aura pas
lu l'invariant 12. Le test lit donc **tous** les `.rs` du dépôt et signale toute ligne mentionnant
un conflit et portant `remove`, `retain`, `drain`, `clear`, `prune`, `forget`, `purge`, `delete`,
`truncate` ou `pop`.

La mutation qui compte l'a confirmé : un `self.entries.remove(conflict_id)` ajouté dans
**`packages/projections`** — un autre crate — fait rougir le test, avec le chemin et le numéro de
ligne dans le message. Les commentaires sont exclus du balayage, sinon le test échouerait sur sa
propre justification.

**Un trou trouvé par une mutation qui n'a pas rougi.** La troisième mutation — faire dire à
`Power::Unstated` qu'elle est concluante — est passée au vert. Cause : `excludes()` teste `Unstated`
en premier et sort avant d'appeler `is_conclusive`, donc le test couvrait la garde de `excludes`
mais pas celle de `is_conclusive`. Les deux gardes existaient, une seule était vérifiée. Assertion
ajoutée sur `is_conclusive` directement ; la mutation rougit maintenant, et la mutation de la garde
de `excludes` aussi. Les deux tiennent séparément, et c'est vérifié séparément.

**Décisions prises.** Quatre.

_Trancher un conflit est un fait qui s'ajoute, pas un effacement._ `record_verdict` remplace
l'entrée par une entrée **portant le verdict** ; `sides()` rend les deux camps après coup, y compris
le perdant. §18.4, point 3 : une fusion « conserve les claims incompatibles ». Un test vérifie qu'on
peut toujours demander qui avait dit quoi après la décision.

_`Verdict::Unresolved` est un état légitime, pas un défaut._ Un graphe qui ne peut pas porter de
désaccord durable est un graphe qui force une réponse avant qu'elle existe. Rien dans l'API ne
permet de faire taire un conflit ouvert.

_Une fusion rend des conflits à déclarer, jamais des objets à retirer._ §18.4, point 7. Une fusion
qui trancherait d'elle-même produirait un graphe propre et faux.

_Une puissance non déclarée n'exclut rien._ C'est la troisième question de §18.7 — « qu'est-ce que
son échec exclut **réellement** ? » — et le mot « réellement » est le sujet du type `Exclusion`.
Trois refus, chacun correspondant à une manière dont une piste se ferme à tort : puissance non
déclarée (personne ne sait si l'échec est informatif), puissance insuffisante (l'échec est
compatible avec l'existence de ce qu'on cherchait), espace de recherche vide (on n'a pas cherché).
Quand il exclut, l'énoncé est **borné** par le scope : « nous n'avons pas trouvé X ici » devient « X
n'existe pas » exactement quand l'énoncé perd ses bornes.

**Une nuance qui aurait pu se perdre.** Un résultat négatif sans puissance déclarée n'exclut rien,
et il est **conservé quand même** — `findings()` le signale sans le rejeter. Savoir qu'une tentative
n'a rien prouvé évite de la refaire en croyant qu'elle avait prouvé quelque chose ; c'est aussi ce
que `attempt_signature()` rend trouvable, en réponse à la première question de §18.7.

**Écart avec la spec.** Une note. Le seuil `CONCLUSIVE_POWER = 0.8` est une **politique**, pas une
lecture : §18.7 demande que le champ de puissance existe sans chiffrer ce qui suffit. Il vit en
constante pour être discuté d'un seul endroit, comme `STAGE_THRESHOLDS` en W2.13 et
`MAX_ARCHIVE_EXPANSION_RATIO` en W2.14.

**Prochain item.** W1.h `[M]` — migrations de schéma et tests de portabilité. Test de sortie : «
migration aller-retour ». **Dernier item de W1**, et le troisième `[M]` : il demande donc un ADR et
un plan de rollback. Ses dépendances sont satisfaites : §10.4 pose déjà les règles d'évolution, et
l'enveloppe versionnée de W1.c porte le `schema_version` sur lequel un upcaster s'accroche.

## 2026-08-17 — W1.h `[M]` — migrations de schéma et tests de portabilité (§10.4, §4.1)

**Périmètre.** Un crate neuf : `packages/migrations` — `migration.rs`, `chain.rs`, `portability.rs`,
plus `tests/round_trip.rs`. Un ADR : `docs/adr/0014-migrations-reversibilite-declaree.md`. Ajouté
aux membres du workspace.

**Tests exécutés.** `cargo test --all-features` : 10 tests neufs, 127 au total sur le workspace, 0
échec. `npm run check` : les neuf portes vertes.

Le test de sortie de W1.h — « migration aller-retour » — passe : sur une chaîne réversible, monter
puis redescendre rend **exactement** le document d'origine, et à chaque palier intermédiaire, pas
seulement d'un bout à l'autre. Vérifié par mutation, trois fois : faire rendre le document tel quel
à une étape irréversible, faire sauter une étape en descente, ou faire taire le filet de portabilité
font rougir trois tests.

**La décision de ce sprint.** Une migration qui monte sait rarement redescendre, et **le prétendre
est pire que l'admettre**. Une chaîne qui redescendrait à travers une étape destructive rendrait un
document ancien **qui n'a jamais existé**, et il aurait l'air authentique — sur un journal dont la
raison d'être est la provenance, c'est le faux dont on ne se remet pas, parce qu'aucune inspection
ne le distingue d'un vrai.

L'alternative habituelle — une descente « au mieux » qui remplit les manquants par des valeurs par
défaut — produit exactement ce document. Elle est aussi ce que tout le monde écrit quand rien ne
l'en empêche, parce qu'elle fait passer les tests de round-trip qu'on avait sous la main.

D'où deux constructeurs et pas de troisième : `reversible` prend une montée **et** une descente,
`lossy` prend une montée **et** la liste de ce qu'elle perd. Une migration sans descente et sans
perte déclarée ne se construit pas. L'irréversibilité devient exécutoire plutôt que documentée : un
commentaire « attention, destructif » n'aurait arrêté personne.

**Décisions prises.** Quatre, argumentées dans l'ADR 0014.

_La chaîne refuse, elle ne saute pas._ Un résultat partiel serait un document d'une version que
l'appelant n'a pas demandée, sans que rien dans sa forme ne le dise. Le refus se traite :
`irreversible_between` permet de **demander avant de tenter**. Une migration destructive n'est pas
une faute — c'est parfois la seule façon d'avancer ; la faute est de le découvrir au moment où l'on
avait besoin de redescendre.

_Les étapes sont contiguës, et la chaîne panique sinon._ Une chaîne trouée est une erreur de
programmation, pas une entrée : elle se construit en dur, au démarrage. La rendre récupérable
inviterait à la rattraper à l'exécution — c'est-à-dire à migrer un document en sautant une forme
qu'il a réellement eue.

_La montée et la descente ne se confondent pas._ `upcast` avec `to < from` est refusé plutôt que
réinterprété : appliquer un `up` là où un `down` était voulu produirait un document deux fois monté.

_La portabilité de §4.1 se vérifie sur les noms, pas seulement sur les imports._ `boundaries.json`
vérifie les dépendances ; il ne voit ni les noms de champ ni les littéraux. Un `Claim` qui porterait
`s3_bucket` ne violerait aucune règle d'import et rendrait pourtant l'objet indéplaçable, ce que
§4.1 interdit en toutes lettres. Les commentaires sont exclus du balayage, et un test vérifie que le
filet **attrape** — même précaution qu'en W2.18 et W1.g, et pour la même raison.

**Plan de rollback.** Dans l'ADR. En résumé : avant qu'un journal persistant existe, revenir coûte
une suppression et une ligne de `Cargo.toml`. Après, la décision 1 a le rollback le plus
**dangereux** des quatre — ajouter un constructeur « descente au mieux » est additif, ne casse rien
visiblement, et fait perdre la garantie sans que personne ne s'en aperçoive. C'est la raison pour
laquelle elle est prise maintenant.

---

## W1 est terminé

Les huit items de W1 sont faits. Le workspace Rust compte **sept crates** — `protocol`, `lep`,
`domain`, `event-store`, `projections`, `graph`, `validation`, `migrations` — et **127 tests**, tous
verts, plus les neuf portes de `npm run check`.

**Trois ADR produits par W1** : 0012 (port event-store avant driver), 0013 (projections
reconstructibles), 0014 (migrations à réversibilité déclarée). Chacun porte son plan de rollback,
comme le demande un item `[M]`.

**Une constante de méthode, sur les huit items.** Chaque garantie que la spec énonce en négatif — ne
pas déduire un niveau d'un statut, ne pas réduire une hyperarête en arêtes, ne pas réfuter
automatiquement, ne pas supprimer un conflit, ne pas prétendre redescendre — est vérifiée **par
l'absence de la fonction qui la violerait**, et la vérification est mutée pour prouver qu'elle
rougit. Deux fois, la mutation a révélé un trou plutôt que de confirmer une garantie :
`is_conclusive` en W1.g, et le compte des relations à sens unique en W1.e.

**Ce qui reste ouvert pour la suite.** Les trois arbitrages cross-repo relevés pendant W2 —
`message_id` de §18.2, la permission offline de §24.3, le producteur manquant de
`data_locality_violation` — attendent toujours une décision côté schémas. Aucun ne bloque W3.

**Prochain chantier.** W3 — abstraction de workflow. ADR 0003 en fixe l'ordre à la lettre : le
backend déterministe de test s'écrit **avant** le backend Temporal.

---

## W3 — abstraction de workflow

### W3.a `[R]` — définitions indépendantes du backend et règles de déterminisme (§11.1, §11.2, §11.3)

**Livré.** `packages/workflow` (crate `locus-workflow`) : `kind.rs` (les onze workflows de §11.2),
`definition.rs` (`WorkflowDefinition`, `Step`, `Activity`, `Effect`, `Idempotency`,
`WorkflowVersion`), `determinism.rs` (les six règles de §11.3, le filet des noms, le balayage de
frappe d'identifiants), `versions.rs` (`VersionRegistry`, couverture de replay, retraits refusés).
Dix-huit tests dans `tests/determinism.rs`.

**Test de sortie, arbitré ici.** La roadmap tient W3 au groupe de commits et ne donne pas de test
par item. Celui de W3.a est : **« un effet non encapsulé ne se déclare pas, et la liste de §11.2 ne
se réduit pas en silence »**. Il porte sur les deux seules choses qu'un paquet de définitions puisse
garantir avant qu'un moteur existe — la **forme** de ce qui est déclaré, et le **décompte** de ce
qui manque. Ce que fait le corps d'un pas ne se vérifie qu'en l'exécutant, donc en W3.b, et le dire
vaut mieux que de laisser croire le contraire.

**Pourquoi les définitions avant les deux backends.** ADR 0003 exige le backend déterministe avant
Temporal. W3.a va un cran plus loin, pour la même raison : §11.1 dit que Locus Solus « ne code aucun
invariant métier directement contre Temporal », et cette phrase n'est vérifiable que si un paquet
existe qui ne connaît **aucun** backend. Si les définitions venaient après, elles porteraient la
forme du premier moteur branché et l'indépendance serait une intention rétrospective.

**Décisions prises.**

_Les six règles de §11.3 n'ont pas la même force, et le code le dit._ `Rule::enforcement` rend
`ByConstruction`, `ByNet` ou `ByCoverage`. Une règle tenue par le type ne peut pas être violée ; une
règle tenue par un filet peut l'être sans que le filet le voie ; une règle tenue par un décompte ne
dit rien tant que personne ne lit le décompte. Les confondre reviendrait à croire que les six sont
également garanties — la cinquième, « tests de replay pour les versions supportées », est la seule
qui ne casse rien quand on la laisse pourrir, et c'est précisément pour ça qu'elle est nommée.

_Un pas déterministe n'a pas de champ pour un effet._ La première règle de §11.3 est portée par la
forme de `Step`, pas par une convention : `Step::Deterministic` n'a qu'un nom. La faute ne s'exprime
pas dans le type. Ce que le type ne voit pas — ce que le pas **fera** —, un filet le cherche dans le
nom : `fetch_reviewer_context` déclaré déterministe est un aveu écrit, et c'est la seule trace
qu'une définition, qui est de la donnée et non du code, puisse en garder. Le filet compare des
**jetons** et non des sous-chaînes : `known` contient `now`, et un filet qui crierait sur
`known_inputs` serait désarmé au premier agacement.

_`Effect::Random` est une addition à §11.3, et elle s'assume._ Le texte énumère quatre appels
sortants — LLM, réseau, filesystem, horloge. L'aléa est le seul non-déterminisme qui ne sorte de
nulle part, donc le plus discret des cinq : un pas qui tire au sort rejoue autrement, ce qui est
exactement la panne que la règle existe pour empêcher. L'omettre par fidélité littérale aurait
laissé passer le cas le plus difficile à voir après coup.

_L'idempotence se déclare, en deux formes et pas trois._ `Idempotency::key` ou
`Idempotency::natural`, cette dernière exigeant une raison écrite — même forme que
`Migration::reversible` / `lossy` en W1.h, et pour le même motif : « naturellement idempotent » sans
justification a exactement le même air que la même phrase vérifiée. Ce n'est pas une preuve
d'idempotence, aucun type ne la donnerait ; c'est l'endroit où quelqu'un y réfléchit une fois, au
moment où c'est encore bon marché.

_Les identifiants métier ne se frappent pas ici._ §11.3 les veut créés **avant** l'entrée dans le
backend ; §11.2 exige qu'un workflow soit « rejoué ou repris avec un autre backend sans changer
l'identité des objets scientifiques ». Un workflow qui frapperait un identifiant en chemin en
produirait un neuf à chaque replay, et l'objet changerait d'identité en étant simplement rejoué.
Garantie tenue deux fois : `WorkflowDefinition::new` exige son sujet, et `minting_findings` balaie
les sources du paquet.

_La table des marqueurs de frappe est assemblée par `concat!`._ Le balayage passe aussi sur le
fichier qui porte la table. Écrite d'un bloc, elle se signalerait elle-même ; la sortie habituelle —
exclure ce fichier du balayage — ouvrirait dans la garde exactement le trou qu'elle ferme. Les
marqueurs sont donc produits à la compilation sans qu'aucune ligne source ne les contienne.

_Deux retraits de version sont refusés, et le refus est structurel, pas informé._ `retire` refuse la
version courante et la dernière restante : les deux laisseraient une exécution en cours pointer vers
une forme que plus personne ne revendique. Le registre ne sait pas ce qui tourne — W3.b, qui aura un
moteur, saura le lui demander. En attendant, refuser les deux retraits certainement dangereux vaut
mieux que de tous les autoriser.

**Six mutations vérifiées rouges.** Renommer l'un des onze workflows ; museler le filet des noms ;
museler le balayage de frappe ; rendre une couverture de replay toujours vide ; autoriser le retrait
de la version courante ; **et ajouter une vraie frappe d'identifiant dans les sources du paquet** —
cette dernière parce que prouver qu'une fonction de balayage attrape un appât n'est pas la même
chose que prouver que le balayage des sources réelles est branché.

**Ce que ce paquet ne garantit pas.** Aucune de ses gardes ne voit le corps d'un pas. Une définition
est de la donnée ; le déterminisme d'une exécution se vérifie en la rejouant. C'est le travail de
W3.b, et `Rule::enforcement` est là pour que cette limite reste lisible au lieu d'être oubliée.

### W3.b `[R]` — port `WorkflowBackend` et backend déterministe de test (§11.1, ADR 0003)

**Livré.** `packages/workflow/src/backend.rs` : le port des six opérations de §11.1, ses types
(`WorkflowId`, `WorkflowHandle`, `WorkflowState`, `WorkflowSignal`, `BackendError`).
`packages/workflow-backends` (crate `locus-workflow-backends`) : `deterministic.rs` (le moteur en
mémoire), `history.rs` (l'historique et le rejeu), `immediate.rs` (`block_on`). Seize tests dans
`tests/replay.rs`.

**Test de sortie, arbitré ici.** **« Rejouer l'historique rend exactement l'état, sans qu'un seul
effet soit réexécuté »** — et pas seulement à l'arrivée : le rejeu est confronté à l'état vivant **à
chaque pas**. Un rejeu qui ne tomberait juste qu'à la fin serait un rejeu qui devine bien, et la
différence ne se verrait qu'au premier redémarrage au milieu de quelque chose. C'est la même
exigence qu'en W1.h, où la migration aller-retour est vérifiée à chaque barreau intermédiaire.

**Décisions prises.**

_Le rejeu est une fonction libre, pas une méthode du moteur._ `replay(definition, history)` ne prend
rien de vivant : ni le moteur, ni son registre d'activities. Une méthode sur le backend pourrait
regarder l'état courant au lieu de le reconstruire, et le rejeu tomberait juste **pour la mauvaise
raison** — en lisant la réponse au lieu de la retrouver. La panne ne se verrait qu'au premier
redémarrage réel, quand il n'y aurait plus rien à lire. Le test le rend visible en **détruisant le
moteur** avant de rejouer.

_Le port est asynchrone, et ses futures sont boxées._ §11.1 écrit les six opérations en `Promise`.
Un port synchrone s'écrirait sans peine tant que le seul backend est en mémoire — puis Temporal
arriverait, dont le SDK est asynchrone, et l'adaptateur devrait bloquer un thread au milieu d'un
exécuteur : le domaine s'adaptant au premier backend branché, c'est-à-dire la panne que l'ADR 0003
nomme. Les futures sont **boxées** plutôt qu'écrites en `async fn` parce qu'un `async fn` de trait
n'est pas compatible `dyn`, et que §11.1 énumère trois implémentations choisies par profil, donc à
l'exécution. Aucun exécuteur n'entre pour autant : définir une future n'en demande pas, et
`packages/workflow` n'a toujours aucune dépendance.

_`block_on` panique sur `Pending`, et c'est l'assertion centrale._ Rendre `Pending` voudrait dire
attendre **quelque chose** — un réseau, un fichier, un timer — et il n'y a rien à attendre dans un
moteur déterministe. Un exécuteur complet l'aurait patiemment laissé faire ; celui-ci refuse. Un
test `should_panic` vérifie que la garde se déclenche, faute de quoi elle serait vide de sens.

_`advance` n'est pas dans le port._ Un moteur durable avance seul : personne ne lui demande le pas
suivant. Mettre `advance` dans `WorkflowBackend` obligerait Temporal à porter une méthode que rien
n'appellerait — le port se pliant au backend de test, exactement l'inversion que l'ADR 0003 cherche
à empêcher.

_Le résultat d'une activity est cherché avant que quoi que ce soit ne soit écrit._ Un refus qui
aurait déjà poussé `StepEntered` laisserait un historique décrivant un pas abordé et jamais fini :
un historique **faux produit par une erreur bénigne**. Le test vérifie l'historique octet pour octet
après un refus, puis qu'il reste rejouable.

_Le rejeu refuse plutôt que de rendre un état plausible._ Version différente, pas renommé, résultat
sans pas abordé, activity abordée sans résultat, événement après la fin : chacun est une erreur
nommée. Le cas du **pas renommé** est celui qui compte à l'usage — renommer un pas sans changer de
version casse le rejeu des exécutions en cours, et rien d'autre ne le dirait.

_Les identifiants viennent d'un compteur._ Ni horloge ni tirage : deux moteurs neufs à qui l'on
demande les mêmes démarrages rendent les mêmes identifiants. Un test le vérifie en construisant deux
moteurs. C'est ce qui permet de rejouer un test ligne à ligne, et c'est §11.3 appliqué au moteur
lui-même.

**Six mutations vérifiées rouges.** Le rejeu qui n'avance pas le curseur sur un pas déterministe ;
le rejeu qui ignore la suspension ; le rejeu qui accepte un pas renommé ; `advance` qui écrit
`StepEntered` avant de chercher l'exécutant ; `advance` qui n'enregistre pas le résultat de
l'activity ; `terminate` qui perd le motif.

**Ce qui reste pour W3.c et W3.e.** Les onze workflows de §11.2 n'ont pas encore de définition
concrète — c'est W3.c, sur ce moteur. Le crash/restart et la compensation de §11.4 sont W3.e ; le
`Progress`/`HistoryEvent` d'ici en porte déjà la matière, mais rien n'a été écrit qui les suppose.

### W3.c `[R]` — les onze workflows de §11.2 sur le backend de test

**Livré.** `packages/workflow/src/catalog.rs` : les onze définitions, cinquante-quatre pas,
vingt-neuf activities. `packages/workflow-backends/tests/catalog.rs` : sept tests.

**Test de sortie, arbitré ici.** **« Les onze workflows de §11.2 s'exécutent sur le backend de test
et se rejouent à l'identique »** — chacun démarré, mené jusqu'à `Completed`, puis rejoué après
destruction du moteur, avec vérification que les résultats d'activity rejoués sont exactement ceux
qui ont eu lieu.

**Ce que ce sprint change vraiment.** Les gardes de W3.a cessent de tourner sur des fixtures écrites
par celui-là même qui les avait écrites. Le filet des noms voyait trois exemples choisis ; il voit
maintenant cinquante-quatre pas de contenu. C'est exactement ce qui s'était produit en W1.a, quand
la règle 1 de `boundaries.json` — qui tournait sur zéro fichier depuis W0.3 — a enfin eu quelque
chose à examiner.

**Décisions prises.**

_Les suites de pas sont arbitrées, et ancrées._ §11.2 énumère les onze et ne décrit leurs pas nulle
part. Chaque définition est donc dérivée de la section qui décrit le processus correspondant : §13
pour le portefeuille et la campagne, §7.1 et §18 pour la branche, §11 et §12 pour la tâche, §17 pour
la revue, §16 pour la curation, §19 pour la reproduction et la construction d'environnement, §21
pour la sandbox, §25 pour la fédération. Exactes sur la **forme** — où un effet a le droit d'avoir
lieu, ce que chaque activity dédoublonne — et provisoires sur le détail métier, que W4 à W8
rempliront.

_Une seule fonction, un `match` exhaustif._ Un douzième workflow ajouté à `WorkflowKind` ne
compilera pas tant qu'il n'aura pas de définition. La liste de §11.2 et le catalogue ne peuvent pas
diverger en silence — la garantie est au compilateur, le test n'en dit que la conséquence.

_La clé d'idempotence est ancrée sur l'objet, pas sur l'exécution._ Deux tentatives du même workflow
sur le même objet doivent se dédoublonner, sinon la reprise après incident refait l'effet une
seconde fois. Un test vérifie que **deux activities d'un même workflow ne partagent jamais une clé**
: elles se dédoublonneraient l'une contre l'autre, le second effet ne se produirait jamais, et rien
ne le dirait.

_Un seul pas est naturellement idempotent, et c'est écrit._ `record_image_digest` : le digest est
l'identité de l'image, le réenregistrer ne change rien. Les vingt-huit autres activities portent une
clé. Écrire « naturel » là où une clé suffisait aurait été la facilité que les deux constructeurs de
`Idempotency` existent pour rendre visible.

_Le décompte de replay est fait à partir de ce que le test a réellement rejoué._ La liste des
versions rejouées est **accumulée pendant l'exécution du test**, pas écrite à côté : une liste
écrite resterait verte le jour où l'un des onze cesserait d'être rejoué. C'est §11.3, cinquième
règle, tenue par un décompte qui ne peut pas se désynchroniser de ce qu'il décompte.

_Quatre des cinq effets apparaissent, et `Random` n'apparaît pas._ Un catalogue monoculture ne
prouverait rien du filet ; un test vérifie donc que `Llm`, `Network`, `Filesystem` et `Clock` sont
tous déclarés quelque part. `Random` est absent parce qu'aucun des onze n'a besoin de tirer au sort,
et en déclarer un pour remplir le tableau irait dans le mauvais sens. `collect_attestation` déclare
`Clock` : §11.3 ne demande pas que le temps ne soit jamais lu, mais qu'il ne le soit **que** dans
une activity.

**Quatre mutations vérifiées rouges.** Retirer un effet d'une activity dont le nom l'annonce ;
donner la même clé à deux activities d'un même workflow ; transformer un pas qui touche au monde en
pas déterministe ; retirer l'un des onze du décompte de replay.

**Ce qui reste.** Le détail métier de chaque pas, qui viendra avec les couches qu'il commande. Le
crash/restart et la compensation de §11.4 sont W3.e ; le backend Temporal, W3.d.

### W3.d `[R]` — adaptateur Temporal : la traduction, pas le fil (§11.1, ADR 0015)

**Livré.** `packages/workflow-backends/src/temporal.rs` : la traduction complète des six opérations
de §11.1 vers les concepts de Temporal, `TemporalGateway` (une méthode par RPC de
`WorkflowService`), `state_from` (la carte des statuts). `tests/temporal.rs` : dix tests contre un
faux cluster. ADR 0015.

**Ce qui n'est pas livré, et pourquoi.** La liaison au fil. Constat vérifié pendant le sprint : le
SDK Rust officiel est en `0.1.0-alpha.1`, et `temporal-sdk-core-api` **ne se construit pas** ici —
son script de build panique sur la génération protobuf. L'ajouter ajouterait `protoc` à toute
machine qui construit le projet, ce que « aucune dépendance implicite à une machine de développeur »
interdit. Et il n'y a aucun cluster en CI pour valider la liaison même si elle compilait. Les trois
voies possibles — SDK alpha, fork tiers `squads-temporal-*` en `0.3.x`, ou gRPC direct — sont dans
l'ADR 0015 avec leur coût. **Aucune ne se tranche dans un sprint** : elles engagent ce que la V1
peut promettre en durabilité. La couture livrée est commune aux trois, donc rien n'est perdu quelle
que soit l'issue.

**Le nommer autrement aurait été le mensonge facile.** Un `TemporalWorkflowBackend` qui n'a jamais
vu de cluster n'est pas un backend Temporal ; c'est une traduction testée, ce qui est utile et n'est
pas la même chose. Le module le dit dans sa première phrase, le test dans la sienne, l'ADR dans la
sienne.

**Test de sortie, arbitré ici.** **« Les six opérations tiennent le même contrat sur les deux
backends, et la traduction ne perd aucune distinction que le cluster faisait »** — une suite
d'opérations identique jouée sur le backend déterministe et sur l'adaptateur, comparée sur les
**étiquettes** d'état. Les étiquettes et non les états : le moteur en mémoire connaît son indice de
pas, Temporal ne le connaît pas, et exiger l'égalité stricte reviendrait à demander au second de
savoir ce que seul le premier peut savoir.

**Ce que le second moteur a appris au port.** Trois amendements à `WorkflowState`, tous imposés par
la traduction, tous invisibles tant que le seul moteur était en mémoire :

1. `step` devient `Option<usize>` — Temporal rend un statut, pas une position ;
2. `Failed` apparaît à côté de `Terminated` — « on l'a arrêtée » et « elle a cassé » n'appellent pas
   la même compensation (§11.4), et `TimedOut` est une casse, pas une décision ;
3. `Unknown` apparaît — pour la suspension inobservable et pour `ContinuedAsNew`.

C'est la panne exacte que l'ADR 0003 nomme : le port avait pris la forme de son unique
implémentation sans que personne ne le décide. Elle a été trouvée en écrivant le second moteur, ce
qui est la raison pour laquelle l'ADR en demande deux. **Première fois de ce chantier qu'une
interface est corrigée par l'usage plutôt que par relecture.** Coût : trois lignes dans les tests de
W3.b et W3.c, parce que la correction est arrivée avant le premier consommateur.

**Décisions prises.**

_`suspend` et `resume` sont des signaux réservés._ Temporal n'a pas de pause côté serveur pour une
exécution de workflow : `PauseActivity` existe, `PauseWorkflow` non. Suspendre est de la logique de
workflow, et la seule façon de la demander de l'extérieur est un signal. Conséquence : le serveur
dira « running » d'un workflow en pause, et la suspension se lit par une **query** réservée. Un
workflow qui n'y répond pas rend `inspect` incapable de conclure — et c'est `Unknown` qui est rendu,
avec le détail de ce qui manquait. Rendre `Running` aurait été un défaut plausible : la réponse la
plus probable, indiscernable d'une observation, et fausse exactement quand la question compte. Même
décision qu'en W2.18, même raison.

_L'adaptateur ne garde aucun état d'exécution._ Seule la correspondance identifiant du port →
référence cluster (`workflow_id` **et** `run_id`). Un cache local serait une seconde vérité, qui
diverge au premier redémarrage du control plane et qui diverge en silence. Un test le vérifie en
changeant le statut du faux cluster derrière l'adaptateur.

_L'identifiant Temporal est composé, pas tiré._ `kind/version/subject`. Temporal se sert du
`workflow_id` pour dédoublonner les démarrages : un identifiant aléatoire ferait de chaque reprise
un second workflow.

**Quatre mutations vérifiées rouges.** Replier `TimedOut` sur `Terminated` ; faire passer un
workflow muet pour `Running` ; traduire `suspend` par un `terminate` ; mettre en cache l'état au
lieu de le demander au cluster.

**Ce qui reste.** W3.e — crash/restart/replay sur les deux backends, compensation qui n'efface aucun
fait observé.

### W3.e `[R]` — crash, redémarrage, rejeu sur les deux backends ; compensation (§11.3, §11.4)

**Livré.** `Activity::compensating` / `compensated_by` et leur validation dans
`WorkflowDefinition::new` ; `packages/workflow-backends/src/compensation.rs` ;
`HistoryEvent::Compensated` ; `DeterministicBackend::{resume_from, compensate}` ;
`TemporalWorkflowBackend::{workflow_id, reattach}` et `TemporalGateway::resolve_execution`. Treize
tests dans `tests/recovery.rs`.

**Test de sortie, arbitré ici.** **« Un redémarrage au milieu ne change pas l'histoire, et compenser
n'en efface rien. »** La première moitié est vérifiée en coupant l'exécution **à chaque pas
possible**, en détruisant le moteur, en reprenant depuis le seul historique, et en comparant
l'histoire finale à celle d'une exécution ininterrompue. La seconde est une propriété de préfixe :
l'historique d'avant compensation doit être un préfixe exact de celui d'après.

**Décisions prises.**

_« Crash » ne veut pas dire la même chose des deux côtés, et c'est le point._ Le moteur déterministe
**est** la vérité : le perdre perd tout sauf l'historique. Le cluster Temporal est la vérité :
perdre l'adaptateur ne perd qu'une table de correspondance. Les deux disent pourtant la même chose —
ce qui survit à un crash est ce qui n'était pas dans le processus qui a crashé. C'est pour cela que
`reattach` marche : le `workflow_id` est composé, donc reconstructible sans mémoire, et le `run_id`
appartient au cluster, à qui on le redemande.

_Compenser ajoute un événement, jamais une rature._ §11.4 : les compensations « annulent les
réservations techniques […] elles ne réécrivent jamais l'histoire épistémique ». Un historique d'où
l'on retirerait ce qui a été compensé décrirait une exécution où la réservation n'a jamais eu lieu —
et une réservation qui n'a jamais eu lieu n'a pas consommé de capacité, ce qui est faux. Un test
balaie les sources à la recherche d'un retrait appliqué à un historique : garantie tenue par
l'absence de la fonction qui la violerait, comme en W1.g pour les conflits.

_Le plan se lit dans l'historique, pas dans la définition._ On ne compense que ce qui a réellement
eu lieu. Une définition dit ce qui était prévu ; l'historique dit ce qui s'est passé, et les deux
diffèrent exactement quand la compensation devient nécessaire, c'est-à-dire au milieu.

_L'ordre inverse n'est pas une élégance._ La sandbox se ferme avant que ses ressources soient
rendues : rendre des ressources qu'un processus vivant occupe encore ne les rend pas.

_Une compensation peut arriver après la fin._ Le rejeu l'accepte explicitement. Une exécution
terminée tient encore ses leases et ses fichiers temporaires jusqu'à ce qu'on les rende ; refuser
l'événement après `Completed` obligerait à compenser avant de finir, c'est-à-dire à défaire une
réservation dont on a encore besoin.

_Aucun pas qui enregistre un fait n'a de compensatrice._ Trois compensations dans tout le catalogue,
toutes techniques : `reserve_resources` → `release_resources`, `reserve_sandbox_resources` →
`release_sandbox_resources`, `start_sandbox` → `stop_sandbox`. Un test vérifie que les pas
d'enregistrement n'en déclarent aucune — défaire un fait observé n'est pas une compensation, c'est
une falsification.

**La mutation qui a trouvé un trou — troisième fois du chantier.** Remplacer `ActivityCompleted` par
`StepEntered` dans la lecture du plan **est passé au vert**. Raison : le moteur déterministe finit
ses activities dans le même appel qu'il les aborde, donc les deux événements sont indissociables
dans tout historique qu'il produit. Le cas où ils diffèrent — un worker qui meurt entre les deux —
n'était testé nulle part, et c'est précisément le cas ambigu : **on ne sait pas si l'effet a eu
lieu**.

La correction ne devine pas. `plan` rend désormais deux listes : `steps`, ce qui a eu lieu et se
défait, et `uncertain`, ce qui a été abordé sans qu'on sache. `compensate` rend les deux ensemble,
parce qu'un appelant qui ne verrait que la première croirait le ménage fini. Compenser ce qui n'a
pas eu lieu peut casser un état sain ; ne pas compenser ce qui a eu lieu laisse une réservation que
personne ne rendra — le moteur n'a aucun moyen de trancher, donc il nomme. Même décision qu'en W2.18
pour `UNKNOWN` et qu'en W3.d pour `WorkflowState::Unknown`. W4, qui tiendra les leases, saura poser
la question au bon endroit.

**Six mutations vérifiées rouges.** La compensation qui rature au lieu d'ajouter ; le plan qui lit
`StepEntered` au lieu d'`ActivityCompleted` (rouge **après** la correction, verte avant — c'est elle
qui a servi) ; l'ordre de compensation non inversé ; `resume_from` qui repart de zéro au lieu du
curseur rejoué ; l'incertain rangé sous le certain ; le rattachement Temporal qui invente le
`run_id`.

---

## W3 est terminé

Les cinq items de W3 sont faits. Le workspace compte **neuf crates** — les sept de W1 plus
`workflow` et `workflow-backends` — et **191 tests**, tous verts, plus les neuf portes de
`npm run check`.

**Un ADR produit par W3** : 0015 (Temporal, la traduction avant le fil), avec son plan de rollback
et un arbitrage explicitement laissé ouvert.

**Ce que W3 a appris.** L'ADR 0003 demande deux backends avant de croire une abstraction ; W3.d a
montré pourquoi, en corrigeant trois fois `WorkflowState` — un port qui n'a qu'une implémentation
prend la forme de cette implémentation sans que personne ne le décide. Le coût de la correction a
été de trois lignes de test, parce qu'elle est arrivée avant le premier consommateur.

**Ce qui reste ouvert, et qui bloque une promesse de la V1.** La liaison au fil de Temporal,
ADR 0015. Le SDK Rust officiel est en alpha et ne se construit pas sans `protoc` ; les trois voies
possibles engagent chacune ce que la V1 peut promettre en durabilité. **C'est un arbitrage à
prendre, pas un détail d'implémentation.**

**Prochain chantier.** W4 — Execution Fabric. ADR 0004 en fixe l'ordre : la suite de self-tests de
sandbox (W4.b) s'écrit **avant** le premier backend d'exécution (W4.c), parce que c'est elle qui
définit ce que « sandbox » veut dire dans ce projet.

### Correction — ADR 0015 amendé : deux constats de W3.d étaient faux

Le ledger est append-only ; cette entrée corrige l'entrée W3.d ci-dessus sans la modifier.

**Ce qui était écrit :** que `temporal-sdk-core-api` « ne se construit pas ici », et que le SDK Rust
est « en `0.1.0-alpha.1` ». Les deux ont été réexaminés après la clôture de W3.

**Ce qui est vrai.** L'échec de build venait de `prost-wkt-types 0.6.1`, dont le script de build ne
trouvait pas `protoc` ; avec `protobuf-compiler` installé, le crate compile en une vingtaine de
secondes. Et « le SDK » n'est pas un bloc : `temporal-sdk-core-protos` est en **0.1.0**, non alpha,
contient les protos vendorés et les clients gRPC `tonic` générés — dont
`workflow_service_client::WorkflowServiceClient` et les cinq requêtes exactes de `TemporalGateway`.
Seul `temporal-sdk-core`, qui est le runtime de **worker**, est en alpha. Vérifié en construisant et
en exécutant un binaire qui les instancie. Le crate `temporal-client` cité en W3.d n'existe pas sur
crates.io.

**Pourquoi ça change la décision.** Ce que `locusd` consomme de Temporal, c'est le **client**, et il
est officiel et stable. Ce qui est en alpha est le runtime de worker — un rôle que le SDK TypeScript
tient en GA et que l'ADR 0011 place déjà dans le périmètre TypeScript du dépôt. La question n'était
donc pas « quel crate » mais **qui exécute les corps de workflow**.

**Décision prise** (ADR 0015, décision 5) : client gRPC officiel `temporal-sdk-core-protos` +
`tonic`, worker en TypeScript, `protoc` ajouté à la CI et vérifié par `locus doctor`. **Différé à
W7**, avec `locusd` : la liaison exige un runtime asynchrone dans un binaire qui n'existe pas
encore, et l'écrire maintenant ferait choisir ce runtime depuis une dépendance au lieu du besoin.

**Ce que la V1 ne peut pas promettre en attendant** : le profil `cloud-platform` de §27.1 exige des
« durable workflows », et §32 en fait un critère d'acceptation. Jusqu'à W7, seul le mode dégradé de
§11.5 est tenable — et §11.5 exige qu'il ne soit jamais présenté comme équivalent à la production.
Dette datée, pas zone grise.

**Ce que cette correction dit de la méthode.** Un `cargo build` rouge avait été lu comme un verdict
sur un écosystème. Il disait seulement qu'un outil manquait. La leçon est la même que pour les
mutations : un signal rouge doit être **ouvert** avant d'être conclu, et les trois trous trouvés
cette étape l'ont été en refusant de s'arrêter au premier constat plausible.

---

## W4 — Execution Fabric

### W4.a `[R]` — `SandboxSpec`, `ResourceSpec`, `SandboxAttestation` (§21.6, §21.7, §21.9, invariants 5, 6, 8)

**Livré.** `packages/execution` (crate `locus-execution`) : `level.rs` (les six niveaux et les sept
profils de §21.6), `spec.rs` (`SandboxSpec`, `Mount`, `NetworkMode`, la liste des montages
interdits), `resources.rs` (`ResourceSpec`, `Accelerator`), `approval.rs` (`Approval`,
`SecurityEvent`), `attestation.rs` (`SandboxAttestation`, `conformance`). Vingt-deux tests.

**Test de sortie, arbitré ici.** **« Un niveau appliqué sous le niveau exigé est refusé, et l'écart
qui est autorisé produit son événement de sécurité — sans que personne ait à y penser. »**

**Décisions prises.**

_L'événement de sécurité est dans la valeur de retour, pas à la charge de l'appelant._ §21.6 : un
downgrade est interdit « sauf approbation explicite **et** événement de sécurité ». Les deux
conditions sont conjointes, et la seconde est celle qu'on oublie — approuver est un geste que
quelqu'un pose, consigner est un geste que personne ne réclame. `conformance` **produit** donc
l'événement et le met dans `Conformance::ApprovedDeviation`. Il n'existe que deux variantes :
conforme, ou écart portant ses événements. Pas de troisième forme, donc pas de « accepté, à
journaliser plus tard » — le trou exact que §21.6 nomme.

_L'ordre des niveaux est significatif, et la question a été posée._ `SandboxLevel` implémente `Ord`,
là où `ValidationLevel` (W1.a) le refuse délibérément. La différence n'est pas un oubli de cohérence
: rien ne dit qu'une preuve formelle vaut « plus » qu'une reproduction, tandis que §21.6 énumère les
niveaux comme une échelle de confinement et que « un downgrade est interdit » n'a de sens que si
l'on peut comparer. Si `S5` se révélait ne pas dominer `S4`, la comparaison devrait devenir un ordre
partiel — et le test qui transcrit l'échelle serait le premier à le dire.

_Les profils ne portent pas de niveau._ §21.6 énumère les sept profils sans dire à quel niveau
chacun s'exécute. Leur en attribuer un ici serait écrire une politique de sécurité dans un type, à
l'endroit exact où personne ne viendrait la relire. La correspondance appartient à §20 et se
décidera en W4.g ; le type existe pour que le vocabulaire ne dérive pas.

_Les montages interdits ont la même forme que le downgrade._ CLAUDE.md : « ne monte jamais le home
utilisateur, le socket Docker/Podman ou un répertoire de secrets dans une sandbox **par défaut** ».
Le « par défaut » est rendu par `Mount::approved`, qui exige une approbation nommée et produit son
propre événement — **même quand le niveau d'isolation est tenu**. Un socket de runtime monté dans
une micro-VM reste un socket de runtime monté : le confinement du niveau ne rachète pas le trou
qu'on y a percé. Approuver un montage qui n'en avait pas besoin est refusé : banaliser l'approbation
la vide.

_Rien n'est supposé illimité._ Invariant 6. `ResourceSpec` n'a ni `Default`, ni quota optionnel, ni
variante « sans limite » : une borne absente n'est pas une borne large, c'est une borne que personne
n'a choisie. Les quatre quotas sont ceux que §32.3 exige de vérifier par self-tests
(CPU/RAM/PID/disque) ; le cinquième, le temps, est là parce qu'une exécution sans horizon consomme
les quatre autres indéfiniment. Zéro disque reste licite — une exécution sans droit d'écriture est
un choix, pas un oubli.

_Le placement compare quota par quota._ Un worker offrant beaucoup de mémoire et trop peu de PID ne
convient pas ; un score agrégé le laisserait passer.

_L'accélérateur est une `Option`._ Invariant 8 : le GPU est une capability, pas une dépendance
globale. Un champ obligatoire, fût-il « aucun », ferait de l'accélérateur une dimension de toute
exécution, et le premier scheduler écrit dessus en supposerait un partout.

_L'événement de sécurité refuse de porter un secret._ §21.9 : « sans enregistrer les secrets ». La
clause est exécutoire, pas documentaire — un journal de sécurité qui recopierait un token serait le
seul endroit du système où l'on aurait accumulé, exprès et durablement, ce qu'on cherche à protéger.
Les marqueurs sont assemblés par `concat!`, comme en W3.a, et un test vérifie que le filet ne crie
pas sur une preuve technique ordinaire — sans lui, une fonction refusant tout supprimerait la
garantie en ayant l'air de la renforcer.

**Un écart de documents relevé, non tranché ici.** §21.6 énumère **six** niveaux, `S0` à `S5` ;
`docs/10_V1_ROADMAP.md` écrit « suite de self-tests indexée par niveau **S0–S4** » pour W4.b. La
spécification étant normative, les six sont transcrits. Ce que la suite de W4.b indexera se décidera
là-bas, en connaissance de l'écart plutôt qu'en le découvrant.

**Six mutations vérifiées rouges.** Le downgrade non approuvé qui passe ; l'écart approuvé qui ne
rend aucun événement ; le montage sous dérogation qui ne produit rien quand le niveau tient ; le
filet des montages interdits mis en veilleuse ; l'événement de sécurité qui accepte un secret ; un
quota nul accepté.

**Ce qui vient.** W4.b — la suite de self-tests indexée par niveau, **avant** le premier backend
d'exécution (ADR 0004) : c'est elle qui définit ce que « sandbox » veut dire dans ce projet, et W4.a
vient de lui donner les mots.

### W4.b `[R]` — la suite de self-tests de sandbox, indexée par niveau (ADR 0004, §21.6, §32.3)

**Livré.** `packages/execution/src/selftest.rs` : seize sondes, chacune déclarant le niveau à partir
duquel elle doit être contenue et **pourquoi ce niveau-là** ; les sept dimensions de §32.3 et §21.7
; `expectation`, `judge`, `standing`, `newly_contained`. Quinze tests.

**Test de sortie, arbitré ici.** **« Chaque niveau que la suite couvre contient strictement plus que
le précédent, et une sonde qu'on n'a pas su lancer ne compte jamais comme une réussite. »** Les deux
moitiés disent la même chose par les deux bouts : un niveau qui ne contient rien de plus que le
précédent est un synonyme, et une sonde non exécutée comptée comme bloquée fait d'un outil manquant
une preuve d'isolation.

**Écrite avant le premier backend, et c'est l'ordre de l'ADR 0004.** Un backend écrit d'abord
définirait la sandbox par ce qu'il sait faire, et la suite se contenterait ensuite de le décrire.

**Décisions prises.**

_Chaque sonde déclare sa frontière et la justifie._ `S1 os-write-contained` contient les écritures
et pas les lectures — c'est ce que son nom dit ; `S2 container-rootless` ajoute les espaces de noms,
donc les lectures de l'hôte, le socket de runtime, la vue sur les processus et les quotas cgroup ;
`S3` ajoute le réseau ; `S4` ajoute un noyau propre. Un test vérifie que la frontière déclarée est
exactement celle qu'`expectation` applique, et qu'aucune sonde ne redevient permise en montant :
sans cette monotonie, « exiger davantage » cesserait d'être une phrase qui a un sens.

_Le sur-confinement est un constat, pas un non-événement._ Une sonde bloquée là où le niveau ne
promettait rien n'est pas un trou de sécurité — et n'est pas rien : un backend plus strict que ce
qu'il annonce fera échouer des missions légitimes de façon inexplicable, et personne ne cherchera la
cause du côté de l'isolation puisque l'isolation « va bien ». `judge` le nomme ; `standing` ne
refuse pas la confiance pour autant.

_Une sonde non exécutée est un troisième verdict._ `Inconclusive`, distinct de « réussie » et de «
bloquée », et il **refuse la confiance** sur une sonde critique : ADR 0004 dit qu'un backend qui
échoue un test critique n'est pas `trusted`, et un test critique qu'on n'a pas su lancer n'a pas
réussi. Le compter comme neutre reviendrait à accorder la confiance faute de contre-preuve, alors
que c'est la preuve qui manque. Même famille que `WorkflowState::Unknown` en W3.d et que `UNKNOWN`
en W2.18.

_Une sonde absente du rapport vaut `Inconclusive`._ Le silence n'est pas un succès : une suite
tronquée — parce qu'un backend ne sait pas lancer une sonde, ou parce que le rapport a été écrit à
la main — ne doit pas se lire comme une suite passée.

_« Presque trusted » n'existe pas._ `Standing` n'a que deux variantes. Un backend qui laisse
échapper une sonde critique n'est pas un backend légèrement moins bon : c'est un backend dont les
missions ont tourné sans le confinement qu'elles croyaient avoir.

**L'écart de documents relevé en W4.a n'en était pas un.** §21.6 énumère six niveaux, la roadmap
écrit « S0–S4 » pour cette suite. La raison apparaît en l'écrivant : **`S5` n'est pas
self-testable**. Il promet une protection contre l'hôte lui-même, et une suite de self-tests
s'exécute sur cet hôte — une sonde qui prétendrait vérifier « l'opérateur ne peut pas lire ma
mémoire » rendrait le verdict que l'hôte aurait choisi de lui rendre. C'est une limite de méthode,
pas une sonde manquante : la garantie de `S5` se vérifie par attestation matérielle distante. Un
test affirme que `S5` ne gagne aucune sonde, pour que personne ne « complète » la suite en inventant
celle qui ne peut pas exister.

**La criticité est déclarée bien qu'aujourd'hui uniforme.** Les seize sondes sont critiques, et un
test l'affirme plutôt que de laisser le champ ambigu. Le jour où quelqu'un ajoutera une sonde non
critique, il devra le décider explicitement — une sandbox n'a pas de contenu accessoire.

**Cinq mutations vérifiées rouges.** La frontière de niveau glissée d'un cran ; une sonde non
exécutée comptée comme conforme ; une sonde absente du rapport comptée comme bloquée ; la seule
sonde `S4` ramenée à `S3`, ce qui fait de `S4` un synonyme ; un échappement qui n'empêche plus la
confiance.

**Une mutation qui n'avait pas rougi pour la mauvaise raison.** La troisième a d'abord semblé ne
rien casser. En vérifiant, la substitution ne s'était **pas appliquée** : `cargo fmt`, lancé juste
avant, avait reformaté la fonction et le motif cherché n'existait plus. Une mutation qui rate
ressemble exactement à une garde absente. Les mutations suivantes vérifient donc que le texte a bien
changé avant de conclure quoi que ce soit du vert.

**Ce qui vient.** W4.c — `locus-execd`, seul détenteur du socket runtime. La suite existe désormais
pour dire s'il tient ce qu'il annonce.

### W4.c `[R]` — `locus-execd`, seul détenteur du socket runtime (ADR 0004, §12.2, §21.6)

**Livré.** `apps/locus-execd` (crate `locus-execd`, binaire du même nom) : `runtime.rs`
(`RuntimePort`, la seule description dans tout le dépôt de ce qu'on demande à un runtime de
containers), `admission.rs` (`HostCapabilities`, `admit`, `RefusalReason`), `main.rs`. Huit tests.
Amendement de `tooling/repo/layout.ts` et deux tests de layout.

**Test de sortie, arbitré ici.** **« `locus-execd` refuse proprement une mission qu'il ne peut pas
honorer — en nommant _toutes_ les conditions qui manquent — et il est le seul endroit du dépôt qui
parle d'un socket de runtime. »** Les deux moitiés sont la même décision vue de deux côtés : si
`locusd` pouvait parler au runtime, l'admission ne serait qu'une politesse.

**Décisions prises.**

_Toutes les raisons, pas la première._ ADR 0004 dit « refuse **proprement** ». Un refus qui ne
nommerait que la première condition manquante ferait corriger une chose, réessayer, découvrir la
suivante — un aller-retour par condition. Un test vérifie aussi que chaque condition se constate
**seule**, sans quoi une fonction rendant toujours les quatre raisons passerait le premier.

_Aucun downgrade silencieux à l'admission._ Quand l'hôte ne sait pas confiner assez fort, la mission
est refusée, pas admise au niveau que l'hôte sait offrir. Ce serait le downgrade que §21.6 interdit,
pris au moment où personne ne regarde et sans l'approbation nommée que W4.a exige. Symétriquement,
le niveau admis est **celui qu'exige la mission**, jamais le meilleur de l'hôte : appliquer
davantage serait le sur-confinement que W4.b nomme.

_Le port ne décide rien._ L'admission se fait sur des capacités **déclarées**, avant tout appel. Un
broker qui apprendrait ses limites en échouant les découvrirait après avoir créé la moitié d'une
sandbox, et laisserait derrière lui ce qu'il avait déjà créé.

_Le binaire refuse de prétendre servir._ Sans driver, `main` échoue avec un message qui dit
pourquoi. Un binaire qui démarrerait en annonçant un service qu'il ne rend pas serait le « sandbox
factice » que le plan de rollback de l'ADR 0004 interdit nommément.

_Le nom de crate d'un répertoire déjà préfixé est le répertoire lui-même._ §5 fixe
`apps/locus-execd/` ; la règle de W0.2 en déduisait `locus-locus-execd`. La spécification étant
normative sur les noms de répertoires, c'est la règle de nommage qui est amendée : un répertoire qui
porte déjà le préfixe est déjà namespacé. Deux tests encadrent la dérogation — elle porte sur le
préfixe, pas sur la correspondance.

**Une duplication trouvée par un test qui échouait.** L'accélérateur était déclaré à deux endroits :
dans `ResourceSpec` de l'hôte et dans une liste `accelerators` à côté. Un GPU manquant produisait
alors **deux** raisons de refus pour un seul fait, dont l'une était fausse — les quatre quotas
tenaient parfaitement. `ResourceSpec::fits_within` est donc scindé en `quotas_fit_within` et
`accelerator_fits_within`, la liste redondante supprimée, et chaque cause est dite une fois sous le
bon nom.

**Quatre mutations vérifiées rouges.** Le refus qui s'arrête à la première condition ; l'admission
qui ne vérifie plus le niveau ; l'admission qui applique le meilleur niveau de l'hôte au lieu de
celui exigé ; un crate hors `locus-execd` qui ouvre une connexion vers un runtime.

**La quatrième mutation a trouvé une garde vide, puis une garde trop large.** Deux corrections
successives, et les deux méritent d'être écrites.

D'abord : `workspace_root()` construisait la racine par `join("..").join("..")`, si bien que
**tous** les chemins balayés contenaient `locus-execd` — et l'exclusion « ce paquet-ci est le seul
auquel c'est permis », écrite par sous-chaîne, excluait l'arbre entier. La garde ne regardait rien
et restait verte. Corrigé par `ancestors().nth(2)` et une exclusion par **préfixe de chemin** ; le
décompte des fichiers réellement examinés fait désormais partie de la garde elle-même, pas d'un test
à côté.

Ensuite, une fois qu'elle regardait vraiment, elle a signalé `packages/execution` — qui nomme
`docker.sock` et `podman.sock` dans `FORBIDDEN_MOUNT_MARKERS` **pour les refuser**, le contraire
exact de ce qu'on traque. La table cherche donc maintenant des **actes** — `bollard::`,
`UnixStream::connect`, `DOCKER_HOST`, `Command::new("docker")` — et non des chemins. Un chemin est
une donnée ; ouvrir une connexion est un acte. Confondre les deux aurait forcé à exempter le paquet
qui écrit la politique de sécurité, c'est-à-dire à trouer la garde là où elle sert le plus.

**Ce qui reste dû.** ADR 0004 : « une fixture de refus d'admission fait partie du corpus de
conformance ». Elle n'est pas livrée : les codes de refus doivent d'abord entrer dans `lep/1.0`, et
c'est l'un des trois arbitrages de schéma restés ouverts depuis W2. Le refus existe et est testé en
Rust ; sa forme sur le fil attend le protocole.

**Ce qui vient.** W4.d — backend Linux rootless, cgroups v2, seccomp : le premier driver, que la
suite de W4.b jugera.

---

## 2026-08-17 — W13.a — ADR 0016, sixième frontière, et l'ouverture de W13 à W18

**Périmètre.** `docs/adr/0016-coordination-agentique.md` et
`docs/13_GRAPHES_AGENTIQUES_ETAT_DE_LART_ET_CIBLE.md`, neufs. Insertions dans `docs/10` (cible, trou
de couverture, dépendance de W7, W13 découpé au commit près, W14 à W18, items de recherche),
`docs/11` (sept lignes), `CLAUDE.md` (sixième frontière + règle de vocabulaire), `README.md` et
`START_HERE_CLAUDE.md` (une ligne chacun pour `docs/13`). `boundaries.json` : deux catalogues, deux
règles. Trois fixtures sous `tests/boundaries/fixtures/imports/`, et
`tests/boundaries/contract.test.ts` élargi. **Aucun code de domaine, aucun package créé, aucun
commit dans `canterel`.**

**Tests exécutés.** `npm run check` vert. Test de sortie de l'item, en trois temps :

- _Sens graphe → coordination, sur l'arbre réel._ `use locus_coordination::AgentInstance;` posé dans
  `packages/graph/src/relation.rs` : `check:boundaries` a échoué avec
  `[epistemic-graph-imports-no-coordination] packages/graph/src/relation.rs: importe locus-coordination/AgentInstance`,
  et rien d'autre. Retiré.
- _Sens coordination → graphe, sur l'arbre réel._ `packages/coordination/src/lib.rs` créé
  temporairement avec `use locus_graph::Graph;` : finding `coordination-imports-no-epistemic-graph`,
  seul. Répertoire **supprimé entièrement** ; il n'entre pas dans ce commit.
- _La garde regarde quelque chose._ `check:boundaries` déclare « vérifiée sur 6 fichier(s) » pour le
  premier sens, ce qui correspond aux quatre `.rs` de `packages/graph/src/`, plus
  `tests/hyperedges.rs`, plus le `Cargo.toml`. Le second sens déclare 0 tant que le crate n'existe
  pas, et l'imprime plutôt que de le taire — même état que la règle 4 avant `apps/locusd`.

**Deux mutations vérifiées rouges.** Les deux règles neutralisées par `"deny": []` : exactement les
trois fixtures neuves échouent, les treize autres restent vertes. Puis la sixième frontière
reformulée dans `CLAUDE.md` seul : le test d'énoncé échoue, les quatre autres passent. Dans les deux
cas le motif visé a été vérifié présent dans le fichier **avant** substitution — une mutation qui ne
s'applique pas ressemble trait pour trait à une garde absente, et c'est l'erreur de W4.b.

**Une garantie qui manquait, trouvée en franchissant la barre.** Le test du contrat affirmait
`[1, 2, 3, 4, 5]` en dur, si bien qu'ajouter une frontière le faisait échouer sans rien apprendre.
Il lit maintenant la section « Frontières vérifiées par la CI » de `CLAUDE.md` et vérifie deux
choses : que chaque frontière numérotée porte au moins une règle, dans l'ordre, et que l'énoncé de
chaque règle est celui de `CLAUDE.md` **mot pour mot**, balisage retiré. `boundaries.json` affirmait
depuis W0.3 que « si les deux divergent, `CLAUDE.md` fait foi » ; personne ne le vérifiait, et une
frontière reformulée d'un côté aurait laissé l'autre décrire une garantie que plus rien ne portait.
Le cas d'une frontière à deux sens — un scope par sens, deux entrées, même énoncé — est celui de la
règle 6 et est couvert explicitement.

**Décisions prises.** ADR 0016, treize décisions, plus l'arbitrage d'emplacement que le handoff
laissait ouvert : **les agrégats de §7.1 iront dans `packages/coordination`, crate distinct**, et
non dans `packages/domain`. Le motif est mécanique et non esthétique : `rules.ts` n'accepte que les
`kind` `"imports"` et `"emacs-isolation"`, il n'existe aucune garde par absence d'identifiant, et
`packages/graph/Cargo.toml` déclare déjà `locus-domain`. Loger les agrégats dans `packages/domain`
rendrait la décision 1 inapplicable par la CI. Le crate n'est pas créé aujourd'hui — un répertoire
apparaît quand il porte une garantie testée — il apparaît en W13.c.

**Écart avec le document reçu.** Le handoff demandait de déposer son §7 « à l'octet près » tout en
posant que « le dépôt fait foi ». Quatre écarts, tous du second côté :

1. _La clause LEP était fausse le jour même._ Le texte reçu affirmait que « `schemas/lep/1.0` ne
   change pas, et aucun mineur n'est ouvert », le seul cas justifiant un mineur étant en W16. Deux
   arbitrages du 2026-08-17 la démentent : la permission de fonctionnement hors ligne, activable et
   désactivable, que la `MissionEnvelope` ne sait pas exprimer — ses quinze propriétés n'ont rien
   pour ça — et les codes de refus d'admission sur le fil, déjà notés comme dus par l'entrée W4.c.
   L'ADR affirme donc seulement ce qui est vrai : **aucun item de W13 ne touche le protocole**, et
   le mineur `lep/1.1` a son propre ADR, dont W13 ne dépend pas.
2. _Contradiction interne du handoff sur l'emplacement._ Son §10 recommande le crate séparé ; son
   §13, prompt de W13.c, dit « livre dans `packages/domain`, sans créer de nouveau package ».
   Tranché pour §10, qui est le seul des deux à porter une raison mécanique. Une session future qui
   reprendrait le prompt tel quel produirait une frontière invérifiable.
3. _La décision 13 avait une borne manquante._ « Repli, jamais prioritaire » et « W13.c est en amont
   de W7 » ne peuvent pas coexister sans dire lequel décide du moment : une chose en amont de deux
   workstreams et jamais prioritaire n'a aucun moment où elle se fait. La dépendance est donc écrite
   dans `docs/10` §W7, et c'est elle qui ordonne, pas l'étiquette.
4. _Les mises en garde bibliographiques entrent avec les affirmations._ `docs/13` reprend §1–§5 du
   handoff, plus un §6 « Statut des sources » tiré de son §14. Déposer les thèses sans leurs
   réserves — corpus de préprints de moins de six mois non répliqués, attribution de deux termes à
   Yue et al. non vérifiée, chiffres d'ATM inutilisables — aurait laissé les secondes hors du dépôt.

**Trois vérifications que le handoff déclarait dues, faites ici.** La normalisation de `imports.ts`
se comporte comme décrit : `normaliseRustPath` rend les deux formes, à souligné et à tiret, dès que
le nom de crate contient `_` ; le motif à tiret suffit donc, et `manifests.ts` lit les dépendances
de `Cargo.toml` — les deux surfaces sont couvertes par fixture. Le décompte de fichiers examinés
**existait déjà** pour les règles d'import : `check-boundaries.ts` l'imprime par règle depuis W0.3,
avec le commentaire qui en donne la raison — « la différence entre _vérifié_ et _il n'y avait rien à
vérifier_, et une seule des deux est une garantie ». Aucun travail d'outillage n'a donc été
nécessaire, contrairement à ce que le handoff supposait. Enfin, son avertissement sur un finding
`unit-placeholder` produit par `check:repo` pour un répertoire incomplet sous `packages/` ne s'est
pas matérialisé : `repo-layout: ok`, et le finding de la règle 6 est apparu seul.

**Écart avec la spec.** Aucun. `SPEC_V1.md` n'est pas réécrit ; ADR 0016 amende §14 sur deux points
et le déclare dans son statut.

**Vérification du constat de couverture.** `AgentTemplate`, `AgentInstance` et `ApprovalRequest` :
zéro occurrence dans `packages/**.rs`. `packages/domain/src/task.rs` ne porte que `TaskState`,
`transition` et `ForbiddenTransition` — l'agrégat `Task` de §7.1 et ses champs `assigned_agent_id` /
`assigned_worker_id` sont absents, d'où W13.d comme dépendance explicite de W13.g.
`EVENT_NAMESPACES` contient déjà `agent`, `team`, `policy`, `approval` et `decision` : l'event-store
ne change pas. `RelationKind::ALL` est bien `[Self; 28]`, `ObjectionTarget` a bien quatre variantes.

**Prochain item.** **W4.d** — backend Linux rootless, cgroups v2, seccomp. W13 ne prend pas sa place
: c'est la décision 13, et ce commit n'est pas un début de W13.b.

---

## 2026-08-17 — W4.d.1 — La traduction Linux rootless, et la lecture de ce que l'hôte permet

**Périmètre.** `apps/locus-execd/src/linux/{mod,plan,probe}.rs`, neufs ; `src/lib.rs` (trois lignes
de réexport) ; `tests/linux.rs`, neuf ; `docs/10` gagne le découpage de W4.d en deux commits. Aucun
processus lancé, aucun socket ouvert.

**Pourquoi la traduction d'abord.** ADR 0012 a posé le port avant le driver, ADR 0015 la traduction
avant le fil. Le motif vaut ici plus qu'ailleurs : le plan de rollback d'ADR 0004 dit qu'il n'y a «
aucun chemin de repli acceptable — un raccourci ici est exactement le _sandbox factice_ que le
handoff interdit ». Un driver écrit avant que la traduction soit vérifiée confinerait de travers
sans que rien ne le dise, et c'est le seul échec de ce workstream qui ne se rattrape pas.

**Tests exécutés.** `npm run check` vert. 23 tests dans `tests/linux.rs`. Test de sortie de W4.d.1 :
le plan ne concède jamais plus que le niveau exigé, il refuse par leur nom ce qu'un conteneur
rootless ne sait pas faire, et la lecture de l'hôte nomme ce qui manque.

**Ce que le test de stricte croissance a trouvé, et qui n'était pas prévu.** Deux tests encadrent
l'échelle : chaque niveau confine **au moins autant** que le précédent, et **strictement plus**. Le
second a échoué du premier coup, sur `S2` → `S3`. La cause n'était pas le test : sous
`NetworkMode::Full`, le plan `S3` était **identique** au plan `S2`, puisque la seule chose que `S3`
ajoute est le namespace réseau et que `full` ne le crée pas. Un niveau qui ne change rien à ce qui
est appliqué est un niveau qu'on revendique sans le tenir — exactement ce que §21.6 appelle un
downgrade, pris du côté où personne ne regarde.

D'où une règle qui n'était pas dans le plan initial, et qui est une **équivalence** plutôt que deux
conditions : `S3` s'appelle `container-isolated-network`, donc en deçà de `S3` un mode autre que
`full` n'a rien pour le porter, et en `S3` `full` viderait le niveau. Une mission qui veut
l'isolation des processus et le réseau de l'hôte demande `S2` et `full` — elle l'obtient, sous son
vrai nom. `PlanError::IsolationContradictsNetwork` porte ce refus et le dit.

**Décisions prises.** Le plafond du backend est `S3`, en constante, refusée au-delà par un nom. Les
limites cgroup sont écrites **à tous les niveaux**, `S0` compris : l'invariant 6 dit que les
ressources sont réservées avant exécution, pas qu'elles le sont quand on isole. Le quota disque et
l'horizon vivent **hors** des limites cgroup — cgroup v2 borne un débit, pas un espace, et l'horizon
est compté par le broker ; les mettre dans la même liste ferait croire qu'écrire trois fichiers les
applique. `S0` refuse un montage et un quota disque : il n'a ni vue du système de fichiers ni rien
pour contenir une écriture.

**Le doute ne s'arrondit pas vers le haut.** `Support` a trois variantes : `Available`,
`Unavailable { reason }` et `Undetermined { reason }`. Les deux dernières mènent au même refus —
`ceiling` est conservateur — mais ne se disent pas pareil, parce que « le noyau refuse » et « je
n'ai pas su regarder » envoient chercher à deux endroits différents. Cas concret conservé comme test
: `unprivileged_userns_clone` n'existe que sur les noyaux portant le correctif Debian, donc son
**absence** ne dit rien et le lire comme un refus interdirait `S1` sur la plupart des noyaux amont ;
présent et à zéro, en revanche, c'est bien un refus.

**`S2` et `S3` demandent les mêmes primitives à l'hôte**, et c'est écrit plutôt que corrigé. Un
namespace réseau non privilégié s'obtient par le namespace utilisateur, comme celui de montage : il
n'existe aucun fichier qui distinguerait les deux. Ce qui les sépare est ce que le plan applique,
pas ce que l'hôte permet ; inventer ici un test qui les distinguerait donnerait une fausse
précision.

**Cinq mutations vérifiées rouges**, motif vérifié présent avant chaque substitution : le plan
appliquant le plafond au lieu du niveau exigé (14 tests) ; un mode réseau non-`full` accepté sans
namespace (1) ; la détection de l'hôte rendue optimiste (6) ; l'absence du correctif Debian lue
comme un refus (5) ; un fichier illisible traité comme un refus plutôt que comme un doute (1).

**Une dépendance implicite évitée.** Le test contre l'hôte réel n'affirme rien sur _cette_ machine —
la CI, un poste de développement et un conteneur ne répondent pas la même chose. Il vérifie ce qui
est vrai partout : la lecture ne panique pas, ne revendique jamais au-delà du plafond, et rend une
preuve non vide. Les huit autres tests de détection tournent contre un arbre de fixtures.

**Écart avec la spec.** Aucun. `docs/03` fixe « rootless Podman/containerd », cgroups v2 et seccomp
; le plan les traduit sans en choisir un — c'est W4.d.2 qui branchera Podman.

**Prochain item.** **W4.d.2** — le driver rootless proprement dit : `RuntimePort` implémenté, la
sandbox créée, l'attestation lue de ce qui tourne. Ses dépendances sont satisfaites : le port existe
(W4.c), la suite qui le jugera existe (W4.b), et la traduction qu'il appliquera existe désormais.

---

## 2026-08-17 — W4.d.2 — Le driver rootless : demander ce que le plan a décidé, attester ce qu'on observe

**Périmètre.** `apps/locus-execd/src/linux/{invocation,driver}.rs`, neufs ; `linux/mod.rs` et
`src/lib.rs` (réexports et deux paragraphes de doc) ; `tests/podman.rs`, neuf ; `docs/10` gagne
W4.d.3. C'est le premier commit du dépôt qui lance un processus de runtime.

**Tests exécutés.** `npm run check` vert. 18 tests dans `tests/podman.rs`, 41 en tout sur
`locus-execd`. Test de sortie de W4.d.2 : le driver demande au runtime exactement ce que le plan a
décidé, et il atteste de ce qu'il observe — jamais de ce qu'il a demandé.

**La propriété qui décide de la valeur de ce module.** `runtime.rs` l'écrivait déjà en W4.c : « un
broker qui composerait l'attestation à partir de ce qu'il avait demandé attesterait de sa propre
demande ». `attestation` dérive donc le niveau des **observations** rendues par `podman inspect`, et
un test le met à l'épreuve sur le cas qui compte : le plan demandait `S3`, le runtime a rendu un
conteneur au réseau de l'hôte, l'attestation dit `S2`, et `locus_execution::conformance` refuse. Un
driver qui aurait rendu `plan.level()` aurait passé toute la conformité de W4.a en ayant tout raté.
La mutation correspondante fait rougir trois tests.

**Le lanceur est un port, comme `TemporalGateway` en W3.** `Runner::run` prend `&self` et non
`&mut self` : lancer un processus ne mute rien du lanceur, et `RuntimePort::attestation` prend
`&self` — un port mutant aurait forcé l'attestation à pouvoir changer ce dont elle témoigne. Le
double de test enregistre les arguments et rejoue des sorties, ce qui rend vérifiables la
construction des arguments, l'analyse des sorties et tous les chemins d'erreur **sans Podman**, donc
en CI, où aucun runtime rootless n'est garanti. Ce qui reste hors test est `SystemRunner::run` :
trois lignes qui lancent un processus.

**Trois décisions de forme, chacune contre une facilité.**

_L'image est désignée par digest._ `Workload::new` refuse une étiquette : `docs/03` l'exige au titre
de l'attestation, §19.3 au titre de la reproductibilité. Une commande vide est refusée aussi — le
point d'entrée de l'image déciderait, et l'attestation ne dirait pas ce qui a tourné.

_L'horizon n'est pas passé au runtime._ Un test vérifie qu'aucun argument ne le porte. Le passer
ferait croire qu'un runtime le tient, alors que c'est le broker qui compte et qui annule.

_Un champ d'inspection absent empêche d'attester._ Podman peut renommer un champ ; le traiter comme
une valeur par défaut ferait attester un confinement sur une lecture qui n'a pas eu lieu. Le refus
nomme le champ.

**La forme négative des arguments Podman mérite d'être dite.** Ne rien passer ne laisse pas un
namespace partagé : ça le crée. C'est `--userns=host` qui partage. Un plan qui oublierait un
argument confinerait donc **plus** que demandé — le sur-confinement de W4.b — et non moins ; le test
vérifie les deux sens.

**Une capacité manquante, refusée plutôt que revendiquée.** `SeccompPosture::Restricted` promet plus
que le profil par défaut de Podman : refuser depuis l'intérieur la création de namespaces et le
chargement de code noyau. Ce refus vit dans un fichier de profil que ce dépôt n'écrit pas encore.
`create_arguments` refuse donc la posture restreinte quand aucun profil n'est configuré, au lieu de
la revendiquer avec le profil par défaut. C'est la règle du plafond `S3` appliquée à une capacité
que l'opérateur apporte, et c'est W4.d.3 qui la lèvera — inscrite à `docs/10` avec son test de
sortie.

**Cinq mutations vérifiées rouges**, motif vérifié présent avant chaque substitution : le driver
attestant sa propre demande (3 tests) ; le niveau observé ignorant le réseau (1) ; un champ absent
toléré (1) ; un namespace partagé ne produisant aucun argument (1) ; le code de sortie ignoré (1).

**Écart avec la spec.** Aucun. `docs/03` fixe « rootless Podman/containerd » ; c'est Podman qui est
branché, containerd reste ouvert derrière le même port.

**Prochain item.** **W4.d.3** — le profil seccomp restreint, et la suite de self-tests de W4.b
passée contre ce backend pour qu'il obtienne un `Standing`. Ses dépendances sont satisfaites : la
suite existe (W4.b), le backend existe désormais.

---

## 2026-08-17 — W4.d.3 — La suite de W4.b contre le backend, et le `Standing` qui en sort

**Périmètre.** `apps/locus-execd/src/linux/selftest.rs`, neuf ; `linux/mod.rs` (réexports) ;
`tests/selftest.rs`, neuf ; `docs/10` sépare W4.d.3 (la suite) de W4.d.4 (le profil seccomp).

**Ce que ce commit referme.** W4.b avait écrit ce qu'il faut tenter et à quel niveau ça doit
échouer, sans backend pour le tenter. W4.d.2 avait écrit le backend, sans rien qui le juge. Ici la
suite passe contre le driver et rend un `Standing`.

**La moitié qui manquait à `Probe`.** Une sonde porte un nom, une dimension, un niveau et une
justification — pas une commande. C'est délibéré : la façon de tenter dépend du backend, et une
commande dans le crate de vocabulaire aurait supposé un Linux. `PROBE_COMMANDS` fournit cette moitié
pour le backend rootless, et deux tests l'encadrent dans les deux sens : aucune sonde sans commande,
aucune commande orpheline. Une sonde sans commande serait silencieusement absente du rapport, et
`standing` la rendrait `Inconclusive` sans que personne sache pourquoi.

**La convention de sortie va dans ce sens-là, et c'est le sujet.** Chaque commande **réussit quand
la sonde réussit**, c'est-à-dire quand le confinement n'a pas tenu : code 0 devient `Succeeded`,
code non nul devient `Blocked`. Le sens inverse aurait fait d'une commande absente de l'image — `sh`
introuvable, code 127 — une preuve d'isolation. La mutation qui inverse la convention fait rougir
quatre tests.

**Ce qui n'a pas pu être lancé n'a rien prouvé.** Un runtime qui disparaît en cours de campagne
produit seize `NotRun`, pas seize `Blocked`. `standing` en fait des `Inconclusive`, et
`denies_trust` refuse la confiance parce que les seize sondes sont critiques. Le test le formule
comme le seul résultat interdit : « accorder la confiance faute de contre-preuve ». C'est la
propriété que `Observed::NotRun` existait pour porter, et c'est la première fois qu'un consommateur
la met à l'épreuve.

**La campagne arrête la sandbox même quand la suite s'est mal passée.** Une sonde qui échoue laisse
derrière elle un conteneur qui tourne, et un hôte qui accumule des conteneurs d'épreuve finit par ne
plus pouvoir en créer. `certify` ignore délibérément l'erreur de `stop` : elle ne doit pas masquer
le verdict, qui est ce qu'on est venu chercher.

**Le sur-confinement ne retire pas la confiance.** Un backend qui contient tout à `S0` reste
`Trusted` : W4.b avait tranché que `OverContained` se signale sans être bloquant. Le test le
consigne pour que personne ne « corrige » ce comportement en croyant durcir la garde.

**Cinq mutations vérifiées rouges**, motif vérifié présent avant chaque substitution : un runtime
injoignable compté comme un blocage (1 test) ; la convention de sortie inversée (4) ; le rapport
perdant sa dernière sonde (5) ; la campagne laissant le conteneur tourner (1) ; une sonde perdant sa
commande (8).

**Ce qui est repoussé, et déclaré.** Les commandes de six sondes visent des sondes compilées
(`/usr/libexec/locus/probe-*`) que l'image de base devra porter : dépasser un quota CPU, mémoire,
PID ou disque, et ouvrir une connexion, ne se tentent pas honnêtement en une ligne de shell. Tant
que l'image ne les porte pas, ces commandes échouent en 127 et la sonde est lue comme `Blocked` —
c'est-à-dire exactement le piège que la convention de sortie évitait ailleurs. La parade est en W5,
qui construit l'image : la suite ne doit pas tourner contre une image qui ne porte pas les sondes,
et c'est à inscrire comme dépendance de la première campagne réelle.

**Écart avec la spec.** Aucun.

**Prochain item.** **W4.d.4** — le profil seccomp restreint, refusé s'il ne refuse pas ce que la
posture promet.

---

## 2026-08-17 — W4.d.4 — Le profil seccomp restreint : vérifié, jamais fourni

**Périmètre.** `apps/locus-execd/src/linux/seccomp.rs`, neuf ; `linux/{mod,invocation}.rs` ;
`Cargo.toml` gagne `serde` et `serde_json` ; `tests/seccomp.rs`, neuf ; `tests/{podman,selftest}.rs`
construisent désormais un profil vérifié ; `docs/10` corrige la description de W4.d.4.

**Le refus d'écrire le profil est la décision de ce commit.** Un profil seccomp par défaut-refus est
une liste de plusieurs centaines d'appels système autorisés, dont l'exactitude ne se démontre qu'en
l'exécutant contre des charges réelles. En écrire un ici, sans hôte pour l'éprouver, produirait soit
une sandbox qui casse tout, soit — bien pire — une sandbox qui autorise ce qu'elle prétend refuser.
C'est nommément le « sandbox factice » que le plan de rollback d'ADR 0004 interdit. Le déploiement
apporte le profil ; ce dépôt le **vérifie**.

**La vérification porte sur ce que la posture promet, ni plus ni moins.** `MUST_DENY` tient huit
appels, en deux familles : créer ou rejoindre un namespace (`unshare`, `setns`), charger ou
décharger du code noyau (`init_module`, `finit_module`, `delete_module`, `kexec_load`,
`kexec_file_load`, `bpf`). Chacun est un appel dont il n'existe pas d'usage légitime dans une
sandbox de mission, ce qui permet de le refuser **par son nom**, sans lire ses arguments.

**Ce que la vérification ne regarde pas, et pourquoi c'est écrit.** Les filtres d'arguments. Un
profil peut autoriser `clone` en refusant `CLONE_NEWUSER` par un filtre sur le premier argument, ce
qui refuse bien la création d'un namespace utilisateur sans refuser `clone` — dont tout programme à
threads a besoin. Cette vérification demanderait un second interpréteur, du modèle d'argument cette
fois, c'est-à-dire un second endroit où se tromper. `clone` est donc hors de la liste, et un test
l'affirme pour que personne ne « complète » la liste sans savoir ce qu'il casse.

**Le type porte la garantie.** `RestrictedProfile` ne se construit que par `parse` ou `read`, qui
vérifient. Il n'existe aucun chemin qui produise cette valeur sans la vérification, donc aucune
consigne d'« appeler le validateur » à oublier.

**Le parti pris sur les règles contradictoires.** La première règle qui nomme l'appel décide ; s'il
n'en existe aucune, l'action par défaut décide. Un profil dont deux règles se contredisent est un
profil dont le comportement dépend de l'implémentation, et le supposer favorable reviendrait à lui
accorder le bénéfice de sa propre ambiguïté.

**Une correction à un texte écrit en W4.d.2.** Le module d'invocation affirmait que la posture
restreinte « promet plus que le profil par défaut de Podman ». C'est une affirmation sur le contenu
du profil par défaut d'un runtime tiers, que ce dépôt ne lit pas et ne vérifie pas. Le texte dit
maintenant ce qui est vrai : la posture promet ces refus-là, ce dépôt vérifie le profil qu'on lui
donne, et il ne fait aucune promesse sur celui du runtime. `docs/10` est corrigé de la même façon.

**Cinq mutations vérifiées rouges** — plus une sixième reprise après correction du test.
`SCMP_ACT_ALLOW` compté comme un refus (4 tests) ; une règle qui refuse n'importe où suffisant (2) ;
la vérification ne vérifiant rien (4) ; la forme à nom unique ignorée (1) ; la liste perdant `bpf`
(2).

**La sixième mutation a d'abord été muette, et c'est le constat utile.** Retirer `bpf` de
`MUST_DENY` ne compilait pas — la longueur du tableau est déclarée — donc aucun test ne tournait, et
un tableau qui ne compile pas n'est pas un tableau vérifié. En ajustant la longueur, la mutation
compilait et **tous les tests passaient** : ils comparaient `permitted` à `MUST_DENY` lui-même, ce
qui reste vrai quelle que soit la liste. Ils vérifiaient la mécanique, pas ce qu'elle refuse. Un
test épingle désormais les huit noms un par un, avec la raison de chaque famille en commentaire ; la
mutation le fait rougir.

**Écart avec la spec.** Aucun. `docs/03` demande « seccomp/AppArmor/SELinux lorsque disponibles » ;
AppArmor et SELinux restent ouverts derrière la même configuration.

**Prochain item.** **W4.e** — backend macOS : VM Linux légère et containers rootless par mission.
Ses dépendances sont satisfaites : le port existe, la suite existe, et le backend Linux donne la
forme.

---

## 2026-08-17 — W4.e.1 — La machine macOS : lire le noyau qui confine, et ne pas prendre une VM pour une micro-VM

**Périmètre.** `apps/locus-execd/src/macos/{mod,machine}.rs`, neufs ; `linux/probe.rs` gagne un port
de lecture ; `src/lib.rs` ; `tests/macos.rs`, neuf ; `docs/10` gagne W4.e.1 et la raison pour
laquelle W4.e n'ouvre pas `S4`.

**Ce que macOS ne change pas.** Le confinement. `docs/03` fixe le profil — « host macOS + VM Linux
légère + containers rootless par mission » — et le conteneur tourne dans un noyau Linux : le plan de
W4.d.1 s'applique tel quel. Écrire un second traducteur aurait produit deux façons de dire la même
chose, et le jour où elles auraient divergé, rien n'aurait dit laquelle était appliquée.

**Ce que macOS change : où on regarde.** Le noyau qui confine n'est pas celui du processus. Lire
`/sys/fs/cgroup` sur macOS répond « rien » pour une machine parfaitement capable, et un backend qui
s'y fierait refuserait tout. `HostFacts` prend donc un port de lecture — `Reader` — au lieu d'une
racine : `LocalReader` lit un système de fichiers monté, `MachineReader` lit à travers
`podman machine ssh`. La déduction est partagée, pas dupliquée ; seule la façon d'obtenir les
fichiers diffère. C'est un élargissement de W4.d.1, pas une reprise : `HostFacts::read(root)` reste,
et tous ses tests aussi.

**La règle qui décide de ce commit : une VM partagée n'est pas une micro-VM par mission.** `S4`
s'appelle `microvm-high-risk` et sa promesse est qu'une mission à haut risque ait **son propre**
noyau. Un déploiement macOS ordinaire fait tourner toutes ses missions dans la même VM, où le voisin
d'une mission est un conteneur et non une machine. Le plafond reste `S3`, et le refus le dit : «
`S4` exige une VM par mission ; celle-ci est partagée ». C'est le genre de plafond qu'on relève par
inadvertance, parce que l'existence d'une VM est exactement l'argument qui donne envie de le faire.

**Trois états de machine, et le troisième est celui qu'on oublie.** Une machine **arrêtée** existe :
elle apparaît dans les listes, elle a une configuration, un opérateur la croit là. Elle ne confine
rien. Le refus distingue donc « service à démarrer » de « noyau incapable » — sans quoi on cherche
un problème de cgroups là où il suffit de démarrer la machine. Et l'invité n'est pas interrogé quand
la machine ne tourne pas : les lectures seraient vides, donc lues comme des indéterminations,
c'est-à-dire un diagnostic exact sur une question qui n'avait pas lieu d'être posée.

**Cinq mutations vérifiées rouges**, motif vérifié présent avant chaque substitution : une VM
suffisant à revendiquer `S4` (1 test) ; le plafond ignorant ce que l'invité permet (4) ; une machine
arrêtée traitée comme une machine qui tourne (3) ; un provider en échec devenant une absence (1) ;
l'état ignorant la colonne `Running` (4).

**Une mutation n'a pas trouvé son motif, et le vert qui a suivi ne prouvait rien.** La première
version de la quatrième mutation visait une branche que le test n'exerce pas — le cas où le
lancement échoue, alors que le test simule un code de sortie non nul. Le fichier est resté intact et
la suite est passée. Reprise sur la bonne branche, elle rougit. C'est la même leçon qu'en W4.b, sous
une autre forme : une mutation qui ne s'applique pas ressemble trait pour trait à une garde qui
tient.

**Écart avec la spec.** Aucun. `docs/03` mentionne aussi un worker macOS de confiance annonçant
`mps` : c'est W4.f, et il ne dépend pas de celui-ci.

**Prochain item.** **W4.f** — worker macOS de confiance annonçant la capability `mps`, qui ne reçoit
que les tâches compatibles.

---

## 2026-08-17 — W4.f.1 — La portée de l'accélérateur : le conteneur ou Metal, jamais les deux

**Périmètre.** `apps/locus-execd/src/admission.rs` — `AcceleratorReach`,
`HostCapabilities::native_only`, `level_for`, une raison de refus de plus ; `src/lib.rs` ;
`tests/accelerator.rs`, neuf ; `docs/10` gagne W4.f.1.

**La contrainte est de la plateforme, pas de l'organisation.** `docs/05` : « les capacités macOS
natives telles que MPS/MLX sont exposées par un worker de confiance **séparé** ». Metal est une API
de macOS ; un invité Linux dans une VM n'y a pas accès. Sur un tel hôte, une mission peut avoir le
conteneur **ou** l'accélérateur — et c'est exactement le genre de chose qu'on fusionne par
optimisme, parce que « la machine a bien un GPU ».

`HostCapabilities` déclare donc d'où son accélérateur est atteignable. Par défaut il est dans la
sandbox — un GPU passé au conteneur est une ressource comme une autre. `native_only(level)` dit le
contraire, et `level_for(spec)` en tire le plafond **pour cette mission** : c'est la mission qui, en
demandant l'accélérateur, sort du conteneur. Mettre ce calcul dans `HostCapabilities` plutôt que
dans `admit` évite qu'un second appelant l'oublie — et W4.g en aura un.

**Ce qui rend le mot « confiance » mesurable.** Un worker qui offre `mps` tourne hors conteneur,
donc bas dans l'échelle. « De confiance » n'est pas un compliment : c'est la conséquence du fait
qu'on ne peut pas le confiner, donc qu'on ne lui confie que ce qu'on accepte de voir tourner sans
confinement. Le refus le dit en nommant les deux niveaux : celui que la mission exige, et celui que
l'exécution native obtient.

**Deux refus qui se ressemblent et n'appellent pas la même action.** `AcceleratorUnavailable` envoie
chercher du matériel ; `AcceleratorOutsideSandbox` demande de choisir entre le conteneur et
l'accélérateur. Les confondre ferait commander un Mac à quelqu'un qui en a déjà un. Un test tient
les deux côte à côte.

**Trois mutations vérifiées rouges** : la portée native ignorée, le plafond restant celui du
conteneur (3 tests) ; une mission sans accélérateur tombant elle aussi au niveau natif (2) ; le
refus hors-sandbox dit « absent » (2).

**Ce qui n'est pas dans ce commit, et pourquoi.** Le lien entre `Standing` (W4.d.3) et l'admission :
un hôte devrait ne pouvoir annoncer que le niveau qu'il a **prouvé** tenir. C'est de la confiance au
sens de `docs/10` §W4.g — « placement par capability + **trust** + localité + fit + budget » — et
c'est là que ça va. Le mettre ici aurait mêlé deux objectifs dans un commit.

**Écart avec la spec.** Aucun. §12.2 liste « sandbox disponible et attestée » et « mémoire, CPU, GPU
et espace disque » parmi les critères de placement ; ce commit rend le premier dépendant du second
quand la plateforme l'impose.

**Prochain item.** **W4.g** — le scheduler : placement par capability, trust, localité, fit et
budget ; admission, refus, reroutage. C'est là que `Standing` rejoint l'admission.

---

## 2026-08-17 — W4.g.1 — Le placement : la confiance ne se déclare pas, elle se prouve

**Périmètre.** `apps/locus-execd/src/placement.rs`, neuf ; `admission.rs` gagne une raison de refus
; `src/lib.rs` ; `tests/placement.rs`, neuf ; `docs/10` gagne W4.g.1 et W4.g.2.

**Le mot qui était resté sans consommateur.** §12.2 place parmi les critères de placement « sandbox
disponible **et attestée** ». `HostCapabilities` annonçait un niveau, et rien ne demandait à l'hôte
de l'avoir tenu. W4.d.3 a produit le juge — la suite de self-tests rend un `Standing` — et ce commit
le branche : un candidat sans `Trusted` au niveau exigé n'est pas placé, quoi qu'il annonce.

Le cas limite est celui qui compte, et c'est le même que celui de W4.d.3 sous un autre angle. Un
hôte qui n'a jamais passé la suite n'est pas un hôte dont on ignore la valeur : c'est un hôte dont
on n'a **aucune preuve**. `Verdict::denies_trust` avait déjà tranché que l'absence de preuve n'est
pas une preuve ; le placement ne fait qu'en tirer la conséquence. Un worker qui vient de s'enrôler
ne reçoit donc rien au-dessus de `S0` avant d'avoir passé la suite — et `S0` n'en demande pas,
puisqu'il ne promet rien.

**Une campagne perdue ne vaut pas une campagne gagnée.** `proven_level` ne retient que les
`Standing::Trusted`. Un `NotTrusted { level: S3 }` porte pourtant un niveau, et le lire comme une
preuve était la mutation la plus facile à écrire — elle rougit.

**Le refus porte tous les candidats.** C'est la règle de W4.c élargie d'un cran : là où un refus
d'admission qui ne nommerait que la première condition manquante ferait corriger une chose et
réessayer, un refus de placement qui ne garderait que « le plus proche » ferait corriger **un hôte**
et réessayer. Un aller-retour par candidat au lieu d'un par condition.

**Deux règles de choix, et la seconde est la plus importante.** Parmi les candidats qui conviennent,
le retenu est celui dont le plafond prouvé est le plus **bas** : un `S3` consommé par une mission
`S1` est un `S3` indisponible pour la mission qui en avait besoin. À plafond égal, l'ordre est celui
de l'identifiant — le placement doit être **reproductible**, sans quoi deux rejeux du même journal
placeraient différemment et la trace ne dirait plus ce qui s'est passé. C'est la même exigence que
celle qui a fait interdire `Date::now` dans les scripts de workflow.

**Six mutations vérifiées rouges** : l'attestation non exigée (3 tests) ; une campagne perdue
comptée comme une preuve (1) ; le refus ne gardant que le premier candidat (1) ; le choix prenant le
plafond le plus haut (1) ; l'ordre des candidats décidant à la place de l'identifiant (1) ; le
niveau placé étant celui de l'hôte plutôt que celui de la mission (2).

**Ce qui n'est pas implémenté, et déclaré.** §12.2 liste onze critères. Cinq ont un consommateur
aujourd'hui — capabilities, fit, réseau, niveau, attestation — et ce commit les traite. Les six
autres — localité et confidentialité des données, coût estimé, limites de parallélisme, indépendance
requise, affinité, santé et historique du worker — n'ont aucun objet dans le dépôt qui les porte :
`SandboxSpec` n'a pas de plafond de confidentialité, aucun journal de santé n'existe. Les inventer
ici aurait produit des critères que le placement sait pondérer et que rien n'alimente. Ils arrivent
avec leurs producteurs.

**Écart avec la spec.** Aucun. §12.2 est mis en œuvre sur les critères dont les entrées existent.

**Prochain item.** **W4.g.2** — le reroutage : une mission dont l'hôte tombe en cours de route, une
mission refusée partout. Il dépend de la lease (§12.3), déjà écrite côté worker en W2.9.

---

## 2026-08-17 — W4.g.2 — Le reroutage : la même tentative, déplacée

**Périmètre.** `apps/locus-execd/src/reroute.rs`, neuf ; `src/lib.rs` ; `tests/reroute.rs`, neuf ;
`docs/10` complète la ligne W4.g.2. **W4 est terminé.**

**La clause de §12.3 que ce commit met en œuvre, mot pour mot :** « une tâche réattribuée conserve
le numéro d'attempt ». C'est la moins intuitive et la plus structurante. Un reroutage n'est pas une
nouvelle tentative, c'est la **même**, déplacée. Incrémenter le numéro ferait croire au budget
qu'une seconde exécution a été demandée, compterait deux échecs là où il y en a un, et casserait
l'idempotence que `attempt.schema.json` construit autour de ce numéro. `Rerouting::attempt()` est le
seul chemin de lecture, des deux côtés du verdict : l'invariant est difficile à casser sans le voir.

**L'exclusion précède le choix, et l'ordre n'est pas indifférent.** Un hôte dont la lease a expiré
reste candidat au sens du placement : il annonce toujours, il a toujours ses preuves, rien dans ses
capacités ne dit qu'il vient de tomber. L'écarter **après** avoir choisi rendrait le verdict
dépendant de l'ordre des candidats — un hôte perdu mais mieux classé aurait été choisi, rejeté, et
le suivant jamais essayé. Un test le fixe en donnant au perdu le meilleur classement.

**L'épuisement porte deux listes qui ne se fondent pas.** `already_lost` nomme les hôtes qui ont
essayé et perdu ; `shortfalls` nomme ceux qui restaient et ce qui leur manquait. Première liste
pleine et seconde vide : panne d'infrastructure. L'inverse : mission mal dimensionnée. Les deux
vides : on ne m'a proposé personne, ce qui est encore une information. Les fondre ferait perdre la
question à poser, et la mutation qui les fond fait rougir deux tests.

**L'urgence n'est pas une preuve.** Un test vérifie qu'un hôte non attesté ne devient pas acceptable
parce qu'il ne reste que lui. C'est la garantie de W4.g.1 mise à l'épreuve dans la situation où on
serait le plus tenté de la relâcher.

**Quatre mutations vérifiées rouges** : le numéro incrémenté au reroutage (2 tests) ; l'hôte tombé
resté candidat (7) ; les deux listes d'épuisement fondues (2) ; le numéro zéro accepté (1).

**Ce que ce module ne fait pas, et pourquoi.** La quarantaine des résultats tardifs — le troisième
membre de §12.3. Elle appartient au control plane : c'est `locusd` qui décide qu'un résultat arrivé
après réattribution ne committe pas sans arbitrage. Le broker ne voit que le placement, et lui
donner ce pouvoir aurait fait du chemin d'exécution un second chemin d'écriture, ce qu'interdit
l'invariant 3.

**Écart avec la spec.** Aucun.

**Prochain item.** **W5** — Toolchains : `EnvironmentBlueprint`/Builder, chaîne lockfile → build OCI
→ SBOM → scan → health checks. Ses dépendances sont satisfaites, et W4.d.3 lui a laissé une dette
nommée : six sondes de la suite visent des binaires que l'image de base devra porter, sans quoi
elles échouent en 127 et se lisent comme des blocages.

---

## 2026-08-17 — W5.a — `EnvironmentBlueprint` : ce que le schéma ne peut pas refuser

**Périmètre.** `packages/environments/`, crate neuf — `toolchain.rs`, `blueprint.rs`, `lib.rs` et
`tests/blueprint.rs` ; `Cargo.toml` racine gagne un membre ; `docs/10` §W5 est découpé et corrigé.

**Ce que ce type ajoute au schéma de W0.5.** Le schéma existe depuis W0.5, à
`schemas/environments/1.0/` — délibérément hors de `lep/1.0`, parce que la `MissionEnvelope` ne
porte qu'un `environment_id`, un digest et des toolchains. Il refuse déjà les champs obligatoires
absents, un digest mal formé, un mode réseau inconnu. Un schéma JSON ne sait pas exprimer les
invariants **entre** champs : un profil répété — l'ordre de composition décidant de ce qui écrase
quoi — et un préféré inférieur au minimum — le blueprint disant qu'il préfère moins qu'il n'exige.
Le type les refuse tous les deux, et refuse aussi ce que le schéma refuse déjà, parce que rien ne
garantit qu'un blueprint construit en Rust soit passé par le schéma.

**Deux tables de secrets, et pourquoi ce n'est pas une duplication.** Le test de sortie a d'abord
échoué sur `HF_TOKEN`, que `locus_execution::secret_marker` ne reconnaît pas. Constat : cette table
répond à « cette **preuve** d'événement de sécurité porte-t-elle un secret ? », vise des valeurs —
`AKIA…`, `Bearer …`, `api_key=…` — et a **raison** de ne pas contenir `token`, sans quoi une preuve
qui dit « le token de session a expiré » deviendrait inécrivable. La question d'ici est autre : « ce
**nom** de variable annonce-t-il un secret ? ». D'où `SECRET_NAME_MARKERS`, appliqué au nom, la
table existante restant appliquée à la valeur. Un test vérifie que les deux ne se recouvrent pas —
ce que l'une attrape, l'autre laisse passer, et c'est voulu.

**Le premier essai de comparaison était trop grossier, et un test l'a dit.** Comparer le nom par
sous-chaîne refusait `TOKENIZERS_PARALLELISM`, variable HuggingFace parfaitement ordinaire, parce
qu'elle contient `token`. Un garde qui refuse des noms légitimes se fait désactiver, et un garde
désactivé ne garde rien. Le nom est donc découpé en segments — sur les séparateurs **et** sur les
changements de casse — puis comparé aux segments et à leurs concaténations contiguës : `API_KEY`,
`api-key`, `apikey` et `ApiKey` donnent tous `apikey`, et `TOKENIZERS_PARALLELISM` ne donne jamais
`token`.

**Six mutations vérifiées rouges** : le nom non examiné (1 test) ; la valeur non examinée (1) ; le
nom comparé par sous-chaîne (1) ; un profil répété accepté (1) ; le digest non vérifié (1) ; le
préféré autorisé sous le minimum (1).

**La sixième mutation a d'abord été muette**, pour la même raison qu'en W4.b : `cargo fmt` avait
reformaté la fermeture visée, donc le motif n'existait plus tel quel et la substitution ne s'est pas
appliquée. Reprise sur le texte formaté, elle rougit. Le réflexe — vérifier le motif présent avant
de conclure quoi que ce soit d'un vert — a servi une troisième fois.

**Ce que les templates reçus ne sont pas.** `docs/10` §W5 dit qu'ils sont « le point de départ », et
deux tests disent ce qui les en sépare : aucun des quatre ne porte `version:` ni `image:`, tous deux
obligatoires au schéma. `ml-mps.yaml` porte en outre un champ `trust: local-native` que le schéma ne
définit pas — vocabulaire reçu du handoff, qui rejoint la portée d'accélérateur de W4.f et qui
demandera sa propre décision. Un second test vérifie que tous les profils nommés par les templates
existent dans l'énumération : c'est ce qui aurait attrapé la divergence entre `docs/10` §W5, qui
listait six profils, et §19.4, qui en liste treize. `docs/10` est corrigé.

**Écart avec la spec.** Aucun. Les treize profils de §19.4 sont transcrits, `ml-mps` porté comme le
seul « non image Linux portable ».

**Prochain item.** **W5.b** — le Builder : lockfile → build OCI → SBOM → scan → health checks →
signature → digest. Il doit livrer les six sondes compilées que W4.d.3 attend dans l'image de base,
sans quoi elles échouent en 127 et se lisent comme des blocages.

---

## 2026-08-17 — W5.b — La chaîne de construction, tenue par les types

**Périmètre.** `packages/environments/src/build.rs`, neuf ; `lib.rs` ; `tests/build.rs`, neuf ;
`docs/10` §W5 gagne W5.c.

**Ce que la chaîne garantit, et par quel moyen.** §19.5 énumère la suite : « lockfile, SBOM, scan,
tests, signature et publication par digest ». Une suite écrite en prose se saute — il suffit
d'appeler la dernière fonction. Ici chaque étape **consomme** la preuve de la précédente et rend la
sienne : `Locked → Built → Inventoried → Scanned → Tested → Published`. Signer sans scanner n'est
donc pas un chemin à interdire, c'est un chemin qui n'existe pas.

**La garantie est vérifiée par le compilateur, et cette vérification est elle-même testée.** Un bloc
`compile_fail` dans la documentation du module tente de sauter du `Built` au `published` ; il doit
ne pas compiler. Une septième mutation — ajouter un `published` à `Built` — le fait compiler, et le
doctest échoue en disant « Test compiled successfully, but it's marked `compile_fail` ». C'est la
première garantie du dépôt dont la mise à l'épreuve passe par `cargo test --doc` ; un test
d'intégration ne pouvait pas la porter, les doctests ne tournant que sur la lib.

**Six autres mutations vérifiées rouges** : le plafond de gravité atteint mais pas dépassé (1 test)
; le refus nommant la première trouvaille au lieu de la pire (1) ; une vérification non lancée
comptée comme passée (1) ; un build sans lockfile (1) ; un SBOM vide (1) ; une publication non
signée (1).

**Quatre refus qui méritent leur nom.**

_Sans lockfile, la chaîne ne démarre pas._ §19.7 fait de `R2` « l'environnement verrouillé » : une
image construite sans lockfile ne se reconstruit pas à l'identique, et la publier promettrait `R2`
sans le tenir.

_« Scanné » ne veut pas dire « propre »._ Le plafond de gravité est un **argument** : la politique
décide de ce qu'elle tolère. Un scan qui rendrait des vulnérabilités et laisserait passer l'image
donnerait à la chaîne l'apparence d'un contrôle sans le contrôle. Le refus nomme la **pire**
trouvaille, pas la première — corriger la première laisserait la pire.

_Sous le plafond ne veut pas dire aucune._ `Published::findings_tolerated` emporte les trouvailles
tolérées. Publier une image porteuse de vulnérabilités connues sans les emporter reviendrait à les
oublier au moment précis où quelqu'un pourrait encore décider de ne pas s'en servir.

_Une vérification non lancée est distincte d'un échec._ Troisième apparition du même refus, après
`Observed::NotRun` (W4.b) et `Support::Undetermined` (W4.d.1) : « la commande a échoué » et « je
n'ai pas su la lancer » envoient chercher à deux endroits différents, et compter la seconde comme un
succès ferait d'un outil manquant une preuve de santé.

**Ce que la chaîne ne garantit pas, et qui est écrit.** Qu'une `Image` vienne d'une chaîne.
`Image::new` reste publique parce que décrire un environnement **déjà publié** — lu d'un registre,
reçu d'un pair — est un autre acte que le construire. La garantie porte sur _ce build-ci_, pas sur
toute image qui existe ; prétendre le contraire aurait rendu indescriptible ce qui existait avant
nous.

**Écart avec la spec.** Aucun. L'ordre est celui de §19.5, « tests » compris entre le scan et la
signature.

**Prochain item.** **W5.c** — le driver de build derrière un port, sur la forme de W4.d.2, et les
six sondes compilées que W4.d.3 attend dans l'image de base.

---

## 2026-08-17 — W5.c — Une sonde absente n'est pas une sonde bloquée : correction de W4.d.3

**Périmètre.** `apps/locus-execd/src/linux/selftest.rs` — trois codes de sortie lus autrement, une
table et une fonction ; `linux/mod.rs` ; `tests/selftest.rs` gagne quatre tests ; `docs/10` §W4.d
gagne W5.c et repousse le driver de build en W5.d.

**La dette de W4.d.3 était réelle, mais je l'avais écrite dans le mauvais sens.** Son entrée de
ledger dit : « six sondes visent des sondes compilées que l'image de base devra porter … tant que
l'image ne les porte pas, ces commandes échouent en 127 et la sonde est lue comme `Blocked` —
c'est-à-dire exactement le piège que la convention de sortie évitait ailleurs ». Le constat était
juste et la conclusion fausse : j'en avais fait une dépendance de W5, à régler en construisant
l'image. Or `Blocked` est exactement ce qu'un niveau **promet**. Une image incomplète rendait donc
le backend **plus** digne de confiance, et une campagne contre elle produisait un `Trusted` que
personne n'avait mérité. Ce n'était pas une dette d'ordonnancement, c'était un défaut, et il allait
dans le sens dangereux.

**Ce que ce commit change.** Trois codes ne sont pas des verdicts sur le confinement : 127, que
POSIX réserve à la commande introuvable ; 126, à la commande non exécutable ; 125, que Podman
réserve à son propre échec de démarrage. Ils deviennent `Observed::NotRun` avec, chacun, ce qui
manque. Tout autre code non nul reste un blocage franc — un refus du noyau est un verdict, et le
lire comme une absence serait l'erreur symétrique.

**Quatre mutations vérifiées rouges** : le retour au comportement de W4.d.3, tout code non nul étant
un blocage (2 tests) ; tout code non nul devenant une absence (7) ; la table perdant le 127 (2) ; et
la version muette de cette dernière, reprise après correction.

**La table était épinglée par un test tautologique, et c'est la leçon de W4.d.4 qui a resservi.**
`les_trois_codes_reserves_disent_chacun_ce_qui_manque` itère sur `UNRUNNABLE_EXIT_CODES` : il reste
vrai quelle que soit la table. Retirer le 127 n'a d'abord pas compilé — la longueur du tableau est
déclarée — puis, la longueur ajustée, seul un autre test rougissait. Un test nomme désormais les
trois codes un par un, avec la raison de chaque réservation.

**Écart avec la spec.** Aucun. §32.3 et ADR 0004 exigent qu'un test critique non passé interdise
`trusted` ; ce commit rend cette exigence vraie pour la façon dont l'échec se présente le plus
souvent.

**Prochain item.** **W5.d** — le driver de build derrière un port, sur la forme de W4.d.2, et les
cinq sondes compilées que la suite attend dans l'image de base. Elles restent nécessaires : leur
absence est maintenant **visible** au lieu d'être flatteuse, ce qui est la bonne façon d'avoir une
dette.

---

## 2026-08-17 — W5.d — Les sondes voyagent avec le harnais, pas avec l'image

**Périmètre.** `apps/locus-execd/src/linux/selftest.rs` — cinq sondes réécrites en shell embarqué,
un code de sortie réservé de plus ; `linux/mod.rs` ; `tests/selftest.rs` gagne trois tests ;
`docs/10` §W4.d gagne W5.d, W5.e et W5.f.

**Ce que ce commit supprime.** La dépendance elle-même. W4.d.3 visait cinq binaires à
`/usr/libexec/locus/probe-*`, W5.c a rendu leur absence **visible**, celui-ci la rend **impossible**
: les scripts sont dans le harnais. Une sonde embarquée est en outre versionnée avec le code qui la
juge — une image construite il y a six mois est éprouvée par la suite d'aujourd'hui, ce qui est le
bon sens de la dépendance. L'item « cinq sondes compilées » de la roadmap disparaît.

**Un troisième code réservé, pour le même refus une couche plus bas.** Une sonde lit parfois quelque
chose qui n'est pas là — `cpu.stat` absent, `curl` introuvable. Sans code réservé, elle rendrait un
code non nul ordinaire, lu comme un blocage, donc comme une preuve d'isolation. C'est exactement le
piège du 127 de W5.c, sauf que cette fois ce n'est pas la sonde qui manque, c'est ce dont la sonde
avait besoin. `INCONCLUSIVE_EXIT_CODE` vaut 120, hors des plages que POSIX, les signaux et Podman
réservent.

**Trois mutations vérifiées rouges** : une boucle qui perd son `done` (1 test) ; une sonde qui
redevient un binaire de l'image (1) ; l'inconclusion relue comme un blocage (1).

**Une mutation est passée verte, et elle a corrigé ce que le test prétendait.** La première version
retirait le crochet fermant d'un `[ … ]` ; les tests sont restés verts. `sh -n` analyse le
**langage** : il attrape un `do` sans `done`, un guillemet non fermé, une substitution non terminée.
Il n'attrape pas la mauvaise utilisation d'une **commande** — `[` est un programme, et son crochet
manquant échoue à l'exécution, pas à l'analyse. Le commentaire du test survendait la garde ; il dit
maintenant précisément ce qu'elle couvre, et la mutation a été reprise sur un cas que `sh -n` voit
réellement. Une garde dont on croit qu'elle couvre plus qu'elle ne couvre est pire qu'une garde
absente : on cesse de chercher.

**Ce qui n'est pas vérifié, et qui devient un item.** La **sémantique** des sondes. Rien ici ne
prouve que `nr_throttled` bouge quand `cpu.max` mord, ni que `pids.max` refuse le fork au bon rang.
Ni `sh -n` ni un double de runtime ne peuvent le dire : il faut un hôte capable de `S2`. C'est W5.f,
inscrit avec son test de sortie, et c'est le premier travail à faire sur une machine qui le permet.

**Écart avec la spec.** Aucun.

**Prochain item.** **W5.e** — le driver de build derrière un port, sur la forme de W4.d.2.

---

## 2026-08-17 — W5.e — Le driver de build : lire le digest, jamais le composer

**Périmètre.** `apps/locus-execd/src/build.rs`, neuf ; `Cargo.toml` du paquet gagne
`locus-environments` ; `src/lib.rs` ; `tests/build.rs`, neuf ; `packages/environments` expose le
blueprint d'un `Locked` ; la garde de W4.c gagne deux actes ; `docs/10` §W4.d complète W5.e.

**Pourquoi le driver est dans `locus-execd`.** Construire une image est un acte de runtime : ça
lance un builder, ça écrit dans le stockage de conteneurs, ça peut pousser vers un registre. ADR
0004 réserve ces actes au broker, et la raison vaut autant pour le build que pour l'exécution — un
processus qui sait construire une image sait produire celle qu'il veut. `packages/environments`
garde le vocabulaire et la chaîne ; ce module lance le premier maillon. La garde de W4.c gagne donc
`Command::new("buildah")` et `Command::new("nerdctl")` : construire est un acte, au même titre
qu'exécuter.

**Ce que le driver ne peut pas faire, et par construction.** Dépasser `Built`. La chaîne de W5.b est
une suite de types et `build` rend son deuxième état ; le SBOM, le scan, les tests et la signature
viennent d'autres outils. Aucun raccourci ne mène d'ici à une image publiée, et ce n'est pas une
discipline à tenir — c'est ce que les types permettent.

**Le digest vient de la sortie du runtime, jamais du blueprint.** Même règle qu'en W4.d.2 pour
l'attestation : composer à partir de ce qu'on attendait attesterait de sa propre attente. Le
blueprint porte le digest d'une image **déjà publiée** ; un build en produit une **nouvelle**.

**Le test disait moins que ce qu'il prétendait, et une mutation l'a montré.** La première version
employait le même digest des deux côtés : la mutation qui recopie le digest du blueprint au lieu de
lire la sortie ne faisait rougir qu'un test collatéral. Deux digests distincts plus tard, elle
rougit sur le test qui porte la garantie. C'est la troisième fois de la session qu'une mutation
corrige un test plutôt qu'un code, et le motif est toujours le même : un test qui compare une valeur
à elle-même vérifie la mécanique et pas la propriété.

**Cinq mutations vérifiées rouges** : le digest composé depuis le blueprint (2 tests) ; la première
couche l'emportant sur l'image finale (1) ; un code non nul pris pour un succès (1) ; le réseau du
build disparu (1) ; le tiret d'un nom de profil laissé dans un nom de variable (1).

**Le réseau du build n'est pas celui d'une mission.** §19.5 : « un build séparé **avec réseau
autorisé** […] une mission standard ne peut pas `curl | bash` ». Le build résout des dépendances,
donc il sort ; c'est précisément pour cela qu'il est séparé de la mission, et qu'il finit par un
scan. Un test le fixe pour que personne ne l'aligne sur le `deny` des missions en croyant durcir.

**Écart avec la spec.** Aucun.

**Prochain item.** **W5.f** — validation sémantique des sondes contre une sandbox réelle, seul item
de W5 qui exige une machine capable de `S2`. À défaut, **W6** — artefacts et reproductibilité — dont
les dépendances sont satisfaites.

---

## 2026-08-17 — W6.a — `ArtifactManifest` : le hash promis, la promotion qui ne se saute pas

**Périmètre.** `packages/artifacts`, neuf et sans aucune dépendance : `src/state.rs`,
`src/manifest.rs`, `src/lib.rs`, `tests/manifest.rs` ; `Cargo.toml` de l'espace de travail gagne le
membre ; `docs/10` gagne le découpage de W6 (a→d).

**Un champ d'état, pas une suite de types — et c'est un choix, pas une facilité.** W5.b a porté la
chaîne de build par une suite de types, et ce serait le réflexe ici. Un build est un **processus** :
il se déroule une fois, au même endroit, l'ordre de ses étapes est celui des appels, et rien de lui
ne survit hors du programme qui le mène. Un état d'artefact est un **fait** : il voyage dans un
manifeste, se sérialise, se relit six mois plus tard, se compare entre pairs fédérés. Un typestate
le rendrait inexprimable en JSON, et `artifact-manifest.schema.json` le déclare bel et bien comme
une énumération. Ce qui reste à tenir est donc la légalité des **transitions**, sous la forme de
`TaskState::transition` du domaine, pour la même raison.

**Le hash est déclaré avant l'upload, et l'arrivée le confronte.** C'est la garantie de ADR 0005, et
c'est la même forme que l'attestation de W4.d.2 et que le digest de build de W5.e : ce qui prouve
vient de l'observation, jamais de la demande. Un manifeste écrit après coup à partir du contenu reçu
dit seulement que ce qui est arrivé est ce qui est arrivé. `uploaded()` est le seul endroit où la
comparaison a lieu, et le seul endroit qui ait besoin que la déclaration précède.

**Ce que la table refuse.** `declared → promoted` n'existe pas — sauter d'un bout à l'autre
servirait un contenu que personne n'a vu. `uploaded → promoted` non plus : un contenu arrivé n'est
pas un contenu vérifié. La quarantaine, elle, est évitable : un contenu de source fiable se vérifie
sans passer par elle, et l'histoire garde la différence. `Promoted` est terminal ; retirer un
artefact promu n'est pas une transition d'état — ce serait effacer qu'il a été cité — mais un acte
de revue, qui viendra avec W7 avec sa propre trace. L'invariant 12 vaut ici : rien ne disparaît pour
faire propre.

**`is_servable()` plutôt qu'une comparaison éparpillée.** Un seul état autorise à servir le contenu.
Le dire par une fonction évite qu'on écrive un jour `state != Rejected` en croyant dire la même
chose — ce qui servirait cinq états sur six. Un test l'énumère.

**`parse` rend `None` et non un défaut.** Un état inconnu traité comme `declared` ferait réuploader
un artefact promu ; traité comme `promoted`, il servirait un contenu que personne n'a vérifié. Aucun
des deux défauts n'est sûr, donc il n'y en a pas.

**Cinq mutations vérifiées rouges** : le hash reçu remplaçant le hash promis (1 test) ;
`declared → promoted` rendu légal (1) ; un artefact promu redevenu déprommable (2) ; tout sauf le
refus rendu servable (1) ; l'histoire des états cessant d'être tenue (2). Restauration confirmée
verte.

**Ce que ce paquet ne fait pas.** Aucun octet n'est écrit, aucun object store n'est branché. C'est
W6.b, derrière un port — même ordre qu'en W5 : le vocabulaire et ses refus d'abord.

**Écart avec la spec.** Aucun.

**Prochain item.** **W6.b** — l'object store derrière un port, avec un backend en mémoire.

---

## 2026-08-17 — W6.b — Le manifeste dit ce que le schéma dit

**Périmètre.** `packages/artifacts` : `src/derivation.rs` neuf, `src/wire.rs` neuf,
`src/manifest.rs` et `src/lib.rs` réécrits, `tests/manifest.rs` migré, `tests/wire.rs` neuf ; le
`Cargo.toml` du paquet gagne `locus-domain`, `locus-lep`, `locus-protocol` ;
`packages/domain/src/hash.rs` élargit sa table d'algorithmes et gagne `tests/hash_vocabulary.rs` ;
deux fixtures dans `schemas/examples/` et leur entrée au registre ; `tooling/schemas/validate.ts`
déduplique la liste des schémas à enregistrer ; `docs/10` renumérote W6.

**D'où vient cet item.** De la lecture du schéma en préparant l'object store. W6.a avait été écrit
depuis ADR 0005 et §19.2 — le texte — sans confronter le type à `artifact-manifest.schema.json`.
Quatre écarts en sont sortis, tous du même genre : le type et le contrat disent presque la même
chose, et « presque » ne se voit pas.

1. **`classification` était une `String` libre**, l'énumération du schéma en a quatre valeurs.
   `"publique"` passait, et un artefact restreint mal orthographié n'aurait été refusé par personne.
2. **`derived_from` n'était qu'une liste de hashes.** Le schéma exige, pour chaque parent, un
   `artifact_id` **et** une relation prise dans un sous-ensemble de §7.5. `reproduces` est ce
   qu'inscrit une reproduction indépendante (§19.7, R4), `supersedes` ce qu'inscrit une correction :
   une liste de hashes nus les rend indistinguables, et un graphe qui ne sait plus qui reproduit qui
   ne peut plus dire ce qui est reproduit.
3. **`ContentHash` était réimplémenté ici**, en plus permissif que celui du domaine : `sha256`
   seulement, et l'hexadécimal majuscule accepté — donc deux écritures d'un même hash, donc deux
   formes canoniques d'un même contenu, ce que §7.7 exclut. C'est exactement la duplication de
   contrat que `CLAUDE.md` interdit, et elle s'est écrite sans qu'aucune règle ne se déclenche.
4. **Six champs du schéma n'existaient pas** dans le type : `filename`, `rights`, `viewer_hints`,
   `integrity`, `declared_at`, `uploaded_at`. Un manifeste traversant un service qui n'en connaît
   que le noyau serait ressorti amputé de sa licence, sans erreur nulle part. Un champ qu'on ne
   comprend pas se **transporte** ; il ne se laisse pas tomber.

**Le domaine était plus étroit que le contrat, et c'est le plus intéressant des quatre.**
`locus_domain::ContentHash` connaissait `sha256` et `sha512` ; le vocabulaire LEP en déclare
**trois**, `blake3` compris. Un manifeste parfaitement conforme, hashé en blake3, était donc refusé
à la lecture par le seul pair censé le comprendre — et un refus de lecture ressemble en tout point à
un document invalide, donc personne n'aurait cherché du côté de la table. Le test ajouté
(`packages/domain/tests/hash_vocabulary.rs`) **lit le schéma**, pas une copie du schéma : une liste
recopiée dans un test vérifie que le code est d'accord avec le test, ce qui est vrai par
construction et ne dit rien.

**L'histoire ne traverse pas le fil, et le dire est la décision.** `history` n'est sur aucun schéma.
Deux issues : l'ajouter au contrat, ou constater qu'elle n'a pas sa place là. C'est la seconde —
l'historique des transitions vit dans l'event store, qui est la vérité institutionnelle
(invariant 2) ; le manifeste porte l'**état**, pas le chemin. `from_wire` reconstruit donc une
histoire à un seul élément, et un test le fixe : rejouer les transitions depuis `declared`
produirait quatre états inventés indiscernables de quatre états observés.

**Une liste vide s'écrit absente.** `option_vec` rend `None` pour un vecteur vide. Les deux formes
ont le même sens pour un lecteur et des octets différents pour un hash de document : réémettre `[]`
là où l'entrée n'avait rien ferait diverger deux pairs sur une donnée que ni l'un ni l'autre n'a
écrite. C'est le même constat que le `skip_serializing_if` de W0.8, du côté de la traduction.

**Six mutations vérifiées rouges** : les droits qui ne traversent plus (1 test) ; une liste vide
réécrite `[]` (1) ; une relation inconnue avalée par `derived_from` (1) ; l'histoire rejouée depuis
`declared` (1) ; le type MIME plus vérifié (1) ; la table d'algorithmes du domaine re-rétrécie (6
tests dans `locus-artifacts`, 2 dans `locus-domain`). Restauration confirmée verte.

**Un incident de méthode, noté parce qu'il se reproduira.** La boucle de mutation restaurait par
`git checkout -- packages/…/src`, ce qui rend les fichiers à **HEAD** — donc au sprint précédent,
puisque le travail en cours n'est pas commité. Les trois premières mutations ont donc laissé un
arbre incohérent et les trois suivantes n'ont pas trouvé leur motif. Restauration correcte : une
copie des fichiers **avant** mutation, dans le scratchpad. `git checkout` restaure une référence,
pas un état de travail.

**`tooling/schemas/validate.ts`.** `artifact-manifest.schema.json` était déclaré dans `shared` ;
l'ajouter aux `documents` pour que ses exemples soient validés l'enregistrait deux fois, ce qu'Ajv
refuse. Le doublon est écarté à la compilation plutôt qu'interdit au registre : l'interdire
obligerait à choisir entre « d'autres schémas peuvent le référencer » et « ses exemples sont
vérifiés ».

**Écart avec la spec.** Aucun. Le schéma n'a pas bougé — il est le contrat, et deux consommateurs en
dépendent déjà.

**Prochain item.** **W6.c** — l'object store derrière un port, avec un backend en mémoire.

---

## 2026-08-17 — W6.c — L'object store derrière un port : ce qui entre, et ce qui ne reste pas

**Périmètre.** `packages/artifacts` : `src/store.rs` (le port), `src/memory.rs` (l'implémentation de
référence), `src/ingest.rs` (l'ordre des appels), `tests/store.rs` (la suite de contract tests),
tous neufs ; `src/lib.rs` les expose. Aucune dépendance nouvelle.

**La forme est celle de l'ADR 0012, transposée.** Le port, une implémentation en mémoire, et une
suite écrite **contre le trait** — jamais contre `MemoryObjectStore`. Un driver sur système de
fichiers ou sur S3 passera cette même suite, et c'est elle qui décidera s'il est conforme, pas sa
documentation. Aucun fichier n'est ouvert ici.

**Trois temps, parce qu'un artefact peut peser des gigaoctets.** `begin` / `write` / `commit`, et
non un `put(bytes)`. Une API qui prend le contenu entier oblige à le tenir en mémoire avant de
savoir s'il est acceptable — l'inverse exact de ce qu'on cherche. La borne de taille mord
**pendant** l'écriture, au fragment qui la dépasse, et ce fragment n'est pas absorbé : un store qui
accepterait puis tronquerait aurait déjà lu ce qu'il refuse.

**Le store ne hashe pas.** Choisir une implémentation de hash est une décision d'infrastructure —
`locus_domain::ContentHash` vérifie la forme et ne calcule rien, et ce paquet fait pareil. Le calcul
passe par le port `Digest`, fourni par l'appelant ; le double des tests est déterministe et
**injectif sur ce que les tests emploient**, ce qui suffit : un double qui rendrait toujours la même
valeur rendrait « ce qui est arrivé n'est pas ce qui avait été promis » intestable.

**L'ordre est la garantie, et il n'appartient à aucune des trois pièces.** Le manifeste sait quel
hash avait été promis, le `Digest` sait calculer celui du contenu qui arrive, le store sait ranger
des octets. Ouvrir, écrire en hashant, **confronter, puis seulement** conclure. Confronter après
avoir conclu rangerait d'abord et vérifierait ensuite — c'est-à-dire ferait entrer le contenu puis
espérerait pouvoir l'oublier. C'est `ingest` qui tient cet ordre, et deux mutations le visent.

**La garantie la moins évidente et la plus importante.** Un contenu refusé ne doit pas non plus être
rangé **sous son propre hash**. Sans cela, déclarer un faux hash suffirait à faire entrer un contenu
arbitraire dans le store, adressable ensuite par qui en connaît le hash : la déclaration préalable
ne filtrerait plus rien, elle enregistrerait juste un refus. Un `abort` sur ce chemin n'est donc pas
de l'hygiène, c'est le contrôle d'accès.

**Adressage par contenu, donc déduplication.** Deux artefacts de même contenu partagent leurs
octets, et reconclure sur un hash déjà présent n'est pas une erreur — le contenu adressé est le même
par définition, et refuser obligerait à distinguer « déjà là » de « conflit », ce qui n'a pas de
sens sur un stockage adressé par hash.

**Un jeton conclu est un jeton inconnu.** Rejouer un `commit` réécrirait sous un hash choisi après
coup : c'est la faille de la contrebande par un autre chemin, et le test la ferme explicitement.

**Six mutations vérifiées rouges** : le contenu refusé conclu sous son propre hash (1 test) ;
l'ordre inversé, ranger puis vérifier (1) ; n'importe quel état acceptant des octets (1) ; le
fragment absorbé puis tronqué (3) ; un contenu incomplet conclu quand même (1) ; l'abandon laissant
les octets en attente (3). Restauration confirmée verte, par copie du scratchpad — voir l'incident
de méthode de W6.b.

**Ce qui n'est pas là, et qui n'est pas oublié.** Aucun scan de contenu : la quarantaine est un état
du manifeste (W6.a) et le scanner est un acte d'infrastructure, qui viendra avec le driver. Aucune
reprise de téléversement interrompu non plus — elle demande une durabilité que l'implémentation de
référence n'a pas, donc elle appartient au driver et à sa propre suite.

**Écart avec la spec.** Aucun.

**Prochain item.** **W6.d** — `RunManifest` : ce qu'il faut pour rejouer une exécution.

---

## 2026-08-17 — W6.d — Un niveau de reproductibilité se calcule, il ne se déclare pas

**Périmètre.** `packages/artifacts` : `src/reproducibility.rs` et `src/run.rs`, neufs ;
`tests/run.rs`, neuf ; `src/lib.rs` les expose ; deux fixtures `run-manifest-*` et leur entrée au
registre ; `docs/10` précise le test de sortie de W6.d. Aucune dépendance nouvelle.

**La règle vient du schéma lui-même.** `run-manifest.schema.json` dit du champ
`reproducibility_level` qu'il est « déclaré par le producteur et **vérifiable depuis le reste du
manifeste** — c'est précisément ce qui le rend contestable ». Ce sprint est cette vérification. Un
niveau déclaré au-dessus de ce que le document soutient est refusé, et le refus nomme ce qui manque.

**Troisième occurrence de la même forme, et il faut la nommer.** L'attestation de sandbox (W4.d.2)
vient de l'observation et non de la demande ; le digest de build (W5.e) se lit sur la sortie du
runtime et ne se compose pas depuis le blueprint ; un niveau de reproductibilité se calcule depuis
ce qui est consigné. Un champ qui s'auto-atteste n'atteste rien, et c'est le même défaut à chaque
fois : la chose à prouver et la chose qui prouve deviennent la même.

**Un lecteur validant, pas un second modèle.** W6.b a montré ce que coûte un type de domaine écrit à
côté du schéma. Ici le document **est** `locus_lep::RunManifest` ; `RunManifest` le tient et y
ajoute ce que le schéma ne peut pas dire : les refus et le calcul. Un run relu se réécrit à
l'identique par construction, et non par une correspondance champ à champ à maintenir.

**Ce que chaque cran demande, et pourquoi R2 est le plafond.**

- **R1** — « inputs et code identifiés » : au moins un input par hash, une révision avec un commit,
  un arbre propre. Le schéma dit lui-même qu'« un run dirty ne peut pas prétendre à R1 ».
- **R2** — « environnement verrouillé » : image par digest et toolchains, que le schéma exige de
  **tout** manifeste. Donc tout run qui atteint R1 atteint R2. Ce n'est pas une simplification :
  c'est ce que le contrat garantit déjà, et prétendre le revérifier ferait croire à une garde là où
  il n'y a qu'une conséquence.
- **R3 et R4** ne sont pas atteignables depuis un document. « Reproduction automatisée sur backend
  compatible » et « reproduction indépendante sur worker distinct » sont des **événements** : ils se
  constatent en rejouant, pas en lisant. `Level::FROM_A_MANIFEST_ALONE` vaut donc `R2`, et
  `Missing::ReproductionNotEvidenced` reste dans le verdict **même quand tout le reste est en
  ordre** — ce qui sépare de R3 doit rester visible plutôt que de disparaître parce que le reste va
  bien. W6.e produira la trace qui les porte.

**Un caveat plutôt qu'un silence ou un refus.** Rien dans le manifeste ne dit si un run est
stochastique, et rien ne peut le dire — ce n'est pas une propriété du document. Sans seeds, le
niveau calculé est donc optimiste, et `Caveat::NoSeeds` le garde auprès du verdict. Même forme que
`Support::Undetermined` de W4.d.1 : ce qu'on sait ne pas savoir voyage, au lieu d'être arrondi dans
un sens ou dans l'autre.

**Le test de niveau défait les conditions une à une.** Un calcul qui ne regarderait qu'un des trois
champs rendrait la même réponse sur la fixture complète. Retirer les inputs, retirer la révision,
retirer le commit, salir l'arbre : chacun ramène à R0, séparément.

**`4` et `4.0` sont le même nombre, et le test l'a rappelé.** La comparaison d'aller-retour porte
sur les documents **décodés**, pas sur leur écriture : `cpu` est un `number` au schéma — cœurs
fractionnaires — donc un `4` lu ressort en `4.0`, et aucun lecteur conforme ne rapporte lequel a été
écrit. `packages/lep/tests/round_trip.rs` avait rencontré le même fait en W0.8 et en avait tiré la
conséquence qui compte : les octets à hasher viennent d'un canonicaliseur, jamais de la sortie d'un
sérialiseur.

**Six mutations vérifiées rouges** : le niveau déclaré cru sur parole (2 tests) ; le plafond d'un
document porté à R4 (4) ; l'arbre sale sans effet (3) ; les inputs absents sans effet (3) ; la
reproduction manquante effacée du verdict (2) ; l'absence de seed cessant d'être notée (1).
Restauration confirmée verte.

**Écart avec la spec.** Aucun.

**Prochain item.** **W6.e** — workflow de reproduction sur le backend déterministe de W3, qui est ce
qui peut porter R3 et R4.

---

## 2026-08-17 — W6.e — Une divergence est un résultat, pas une panne

**Périmètre.** `packages/artifacts/src/reproduction.rs` et `tests/reproduction.rs`, neufs ;
`src/lib.rs` les expose ; `packages/workflow-backends/tests/reproduction.rs`, neuf, avec
`locus-artifacts`, `locus-lep` et `serde_json` en dev-dependencies de ce paquet.

**C'est l'événement que W6.d refusait de lire dans un document.** W6.d a posé que `R3` et `R4` ne
s'établissent pas depuis un manifeste seul : ce sont des événements. Ce sprint les produit, en
confrontant deux runs. `compare` ne lit aucun champ de niveau — il regarde ce que le rejeu a rendu.

**Une divergence est une valeur, détaillée sortie par sortie.** Invariant 12 : « les résultats
négatifs et conflits ne sont jamais supprimés pour rendre le graphe propre ». Un rejeu qui ne
retrouve pas les mêmes sorties est une **information scientifique**, souvent la plus intéressante
des deux ; la traiter en erreur la ferait remonter comme un incident technique, c'est-à-dire
disparaître. Le second volet du test le vérifie là où ça compte : sur le `ReproductionWorkflow` de
§11.2, un rejeu divergent **termine** son workflow et son verdict est consigné.

**Ce qui, en revanche, est une erreur.** Comparer deux runs qui ne font pas la même chose. Là il n'y
a rien à conclure, ni dans un sens ni dans l'autre : « les sorties diffèrent » ne dit rien sur la
reproductibilité du premier si le second n'exécutait pas la même chose. `NotAReproduction` nomme ce
qui diffère — image, inputs, commandes.

**L'ordre compte pour les commandes et pas pour les inputs.** Deux runs qui consomment les mêmes
contenus consomment les mêmes contenus, quel que soit l'ordre où le manifeste les a listés. Mais
`train` puis `evaluate` n'est pas `evaluate` puis `train`, et un rejeu qui les intervertit n'exécute
pas le même run.

**Une sortie en trop est une divergence.** L'ignorer parce que « rien ne manque » laisserait passer
un rejeu qui produit silencieusement autre chose en plus.

**Ce que la spec ne permet pas de savoir, et d'où vient la réponse.** `run-manifest.schema.json` ne
nomme **aucun worker**. Rien dans deux `RunManifest` ne dit s'ils ont tourné sur la même machine —
et `R4` exige un « worker distinct ». Cette connaissance appartient au plan de contrôle, qui a émis
les leases : c'est lui qui sait, donc c'est lui qui le dit, par le type `Independence` plutôt que
par une déduction que les documents ne permettent pas. `Independence::Unknown` plafonne à `R3` :
monter faute d'information reviendrait à conclure d'un silence. **Candidat d'évolution de schéma**
pour une future ligne `lep/1.1` : un `worker_id` au `RunManifest` rendrait R4 lisible sans paramètre
hors-bande. Le schéma n'est pas touché ici — il est gelé et deux consommateurs en dépendent.

**Le niveau après une divergence ne descend pas et ne monte pas.** La reproduction a eu lieu, elle
n'a rien établi de plus que ce que le manifeste soutenait : un rejeu raté ne défait pas un
environnement verrouillé.

**Le test de workflow n'est pas une mise en scène.** Le résultat consigné par
`record_reproduction_verdict` n'est pas une chaîne écrite pour le test : c'est ce que `compare` a
rendu, passé tel quel au moteur. Et le pas `compare_outputs` est vérifié **déterministe** : le jour
où la comparaison irait rechercher un artefact pour le rehasher, elle devrait devenir une activity,
et ce test est ce qui obligerait à s'en apercevoir.

**Le test a trouvé un accroc de composition, et c'était W6.d qui avait raison.** Les premiers rejeux
construits pour différer des inputs gardaient le `reproducibility_level: R2` de l'original — que
W6.d refuse, puisqu'un run sans input ne le soutient plus. Le rejeu était donc invalide bien avant
d'être comparé. Corrigé côté test : un rejeu qui perd ses inputs perd aussi sa déclaration. Les deux
sprints se tiennent, et c'est le genre de vérification croisée qu'aucun des deux ne pouvait faire
seul.

**Six mutations vérifiées rouges** : la divergence redevenue erreur (3 tests) ; l'indépendance
inconnue valant un worker distinct (1) ; la divergence n'empêchant plus de monter (1) ; deux images
différentes rendues comparables (2) ; l'ordre des commandes cessant de compter (1) ; une sortie en
trop cessant d'être une divergence (1). Restauration confirmée verte.

**Un détail de CI tranché en passant, parce qu'il a coûté un sprint.** Le job `check` porte un
budget de dix minutes, et l'installation d'`emacs-nox` — nécessaire parce que la frontière 5 se
vérifie en démarrant un Emacs, pas en lisant du code — l'a consommé entièrement sur un miroir apt
lent : la CI a été **annulée** alors que rien n'était rouge. Chaque tentative est désormais bornée
et il y en a trois, et le budget du job passe à vingt minutes pour que la reprise ait la place de
servir. Sans borne, la lenteur du réseau est indistinguable d'un test qui échoue.

**Écart avec la spec.** Aucun.

**Prochain item.** W6 est complet. Reste **W5.f**, bloqué sur un hôte capable de `S2`. Le suivant
est donc **W7** — mémoire, revue indépendante, budgets, portefeuille — qui dépend de W13.c et W13.d
(ADR 0016, décision 13) : la revue indépendante suppose des instances d'agent distinctes et une
assignation. **W13.b** est donc le prochain item exécutable.

---

## 2026-08-17 — W13.b — Le pli : `lep/1.0` journalise une exécution, pas une organisation

**Périmètre.** `tests/graph/fold.ts` et `tests/graph/fold.test.ts`, neufs. Aucun schéma modifié,
aucun champ ajouté au protocole, aucun paquet touché.

**Pourquoi un pli, et pourquoi sous `tests/`.** Ce n'est pas une projection : il lit des documents
et rend un graphe, sans état, sans journal, sans base. Il existe pour répondre à une question
**avant** que W13.f engage une projection sur la réponse — le graphe d'exécution est-il dérivable de
`lep/1.0` tel quel ? La réponse est oui, et la découvrir après avoir écrit la projection l'aurait
coûtée.

**La réponse tient en une phrase.** `lep/1.0` journalise une **exécution**, pas une
**organisation**. Attempt, tâche, worker, lease, outil, artefact, run et leurs relations se
reconstituent sans rien ajouter. Qui — quel **agent** — a agi ne s'y trouve pas.

**Ce que le test prouve par l'absence, et ce qu'il ne demande pas.** Le schéma de l'événement ne
déclare aucune propriété d'agent : aucun producteur conforme n'a d'endroit où en mettre une. Une
première version exigeait en plus `additionalProperties: false`, et **c'était une erreur** : aucun
schéma LEP ne ferme, et pour une raison — `docs/06` fait des champs optionnels compatibles un ajout
mineur, ce qui suppose qu'un consommateur `1.0` tolère les champs d'un producteur `1.1`. Le schéma
de l'événement le dit lui-même à propos d'`event_type`, « fermé exprès, **contrairement aux
documents** ». Ce qui protège réellement un consommateur est ailleurs : le SDK est **généré** depuis
le schéma, donc un champ non modélisé n'existe pas pour qui l'emploie — et un second test le vérifie
sur le fichier généré plutôt que sur une liste recopiée.

**La nuance qui décide de W13.g.** `attempt.schema.json` porte bien un `agent_id`, **facultatif**.
Mais une projection consomme le **flux d'événements**, pas les documents d'attempt : l'assignation
ne lui parvient jamais. C'est exactement ce que W13.d comble en faisant de l'assignation un
événement, et le test est ce qui empêche de croire que le champ existant suffisait.

**Une arête est un fait, pas une occurrence.** « `task-nominal#1` appartient à `task-nominal` » est
écrit par la mission, par la lease, par l'attempt et par chaque événement. La première version
empilait quatre arêtes identiques, ce qui aurait fait d'un graphe de dépendances un histogramme de
mentions et faussé le premier calcul de degré venu. Trouvé par le test, pas par relecture.

**Deux pertes déclarées plutôt que masquées.** `lep/1.0` ne donne aucun identifiant d'appel d'outil
: deux appels du même outil dans le même attempt sont **un seul nœud**. Et un document partiel ne
produit rien — le pli s'abstient au lieu de fabriquer un attempt « inconnu », ce qui est le cas
normal d'une projection qui rattrape un journal.

**La mutation qui est passée verte, et ce qu'elle a montré.** Neutraliser `orphanEdges` pour qu'il
rende toujours la liste vide laissait la suite **verte** : aucun test ne lui donnait d'orpheline à
trouver. Un détecteur muet est indiscernable d'un graphe sain. C'est la troisième fois de ce
chantier qu'une garde passe sans être elle-même vérifiée, et la forme est toujours la même — on
teste ce que la garde protège, jamais qu'elle protège. Un test lui donne maintenant une arête
pendante dans chaque sens et exige que le nœud manquant soit nommé.

**Six mutations vérifiées rouges** : le préfixe de sorte supprimé (2 tests) ; une arête redevenue
occurrence (1) ; un attempt lié à une tâche jamais créée (2) ; un attempt fabriqué pour un document
sans identité (1) ; la dérivation d'artefact cessant d'être une arête (1) ; le détecteur
d'orphelines rendu muet (1, après correction du test). Restauration confirmée verte.

**Écart avec la spec.** Aucun.

**Prochain item.** **W13.c** — `packages/coordination` : `AgentTemplate`, `AgentInstance`, `Team`,
`Decision`, `ApprovalRequest`, et les quatre identifiants dans `packages/protocol`.

---

## 2026-08-17 — W13.c — Les agrégats de coordination, et l'intersection qui n'est pas une union

**Périmètre.** `packages/coordination`, neuf : `src/{capability,agent,team,decision}.rs`,
`src/lib.rs`, `tests/{capability,aggregates}.rs` ; `packages/protocol/src/id.rs` gagne quatre
natures d'identifiant ; le `Cargo.toml` de l'espace de travail gagne le membre. Seule dépendance :
`locus-protocol`.

**La phrase de §14.2, rendue opposable.** « Une instance n'hérite **jamais** tacitement des
permissions du modèle ou du worker. Les capacités effectives sont l'**intersection** de la mission,
du template, de la politique locale et de l'attestation du worker. » Sous l'union, une politique
locale permissive suffirait à rendre un outil accessible à une mission qui ne l'a jamais demandé, et
l'attestation d'un worker deviendrait une **source de droits** au lieu d'être une borne. C'est cette
inversion que le test rend impossible.

**Le test parcourt tout l'espace, pas un échantillon.** Quatre capacités, quatre sources, un bit
d'appartenance chacun : 16⁴ = 65 536 configurations, énumérées **exhaustivement**. Une propriété
vérifiée partout n'a pas de cas restant où elle serait fausse — là où un tirage aléatoire laisse
toujours la question de savoir s'il a produit la configuration qui compte. L'attendu est calculé
indépendamment de l'implémentation : une capacité est effective quand ses quatre bits sont posés.

**Les quatre sources sont un type, pas quatre paramètres.** Quatre `BTreeSet` en arguments se
permutent silencieusement, et surtout rien n'empêche d'en oublier un. `Sources::effective` itère sur
`Source::ALL` : une cinquième source entrerait dans le calcul sans qu'on ait à y penser, et en
retirer une demande d'éditer une liste que le compilateur affiche. Un test tient en outre la place
de la mutation dans le code du test : il calcule ce que rendrait un calcul incomplet et montre que
la réponse diffère.

**Le refus nomme sa source.** `withholding` rend les sources qui retiennent une capacité. « Le
worker ne peut pas le faire » et « la mission ne l'a pas demandée » n'appellent pas la même suite,
et un refus muet obligerait à interroger quatre politiques à la main.

**L'instance fige la version du template.** §7.1 : « l'identité d'un agent comprend le template,
**sa version**, le modèle exact… ». Une instance qui ne garderait que `template_id` changerait
d'identité rétroactivement à chaque révision, et une revue d'il y a six mois cesserait de dire ce
qu'elle disait. Elle hérite aussi du groupe d'indépendance, sans quoi la vérification de §14.4
devrait remonter au template — donc à sa version courante, donc à une réponse qui change avec le
temps.

**`deprecated` n'est pas `disabled`.** §7.1 les distingue ; les confondre arrêterait des campagnes
en cours au lieu d'en décourager de nouvelles. Un template déprécié reste instanciable.

**Un seul mode retient le partage.** `independent_pool`, et lui seul — c'est l'invariant 11 lu dans
le mode de coordination, dit une fois plutôt que réécrit à l'envers ailleurs. Et le mode
`coordinator` exige un coordinateur **membre** : §14.3 en fait la définition du mode, et l'omettre
laisserait une équipe qui se dit coordonnée sans que personne ne coordonne.

**Une décision approuvée se révoque, une décision rejetée non.** Il n'y a rien à défaire dans un
rejet. Et une révocation ne ramène pas à `proposed` : la trace de l'approbation reste, invariant 12.

**Une demande d'approbation sans rôle habilité est refusée.** « Suspendre **durablement** » (§7.1)
suppose que quelqu'un puisse reprendre ; une demande que personne n'est désigné pour trancher ne
suspend pas, elle enterre.

**Les quatre identifiants sont provisoires, et c'est dit.** §7.1 nomme les agrégats, §10.1 ne montre
aucun exemple de leurs identifiants — contrairement aux dix natures qui apparaissent littéralement
sous la forme `evt_01…`. `Task`, `Team`, `Decision` et `Approval` rejoignent donc `provisional`,
comme `Mission` et `Error` avant eux. Les y mettre plutôt que parmi les fixées est ce qui empêche de
croire qu'un document les a arbitrés ; W13.e, qui écrira les événements de coordination, le fera.

**La sixième frontière examine enfin du code.** W13.a l'avait écrite contre des fixtures, faute de
crate à surveiller : `coordination-imports-no-epistemic-graph` annonçait « 0 fichier ». Elle en
examine maintenant **8**, et une mutation le confirme — un `use locus_graph::Relation` dans
`team.rs` fait échouer `check:boundaries` en nommant le fichier et l'import. C'est le même moment
qu'en W1.a pour la règle 1 et qu'en W3.c pour les gardes de workflow : une règle ne vaut que le jour
où elle a quelque chose à examiner.

**Six mutations vérifiées rouges** : l'intersection devenue union (4 tests) ; l'attestation du
worker ignorée (4) ; la version du template non figée (1) ; un template désactivé redevenu
instanciable (1) ; le pool indépendant qui partage (2) ; un rejet devenu révocable (1). Plus la
mutation de frontière ci-dessus. Restauration confirmée verte.

**Écart avec la spec.** Aucun. Les champs de §7.1 non modélisés — `prompt_overlay_ref`,
`memory_policy_id`, `budget_envelope_id`, `evidence_refs`, `policy_evaluation_id` — désignent des
objets qu'aucun consommateur exécutable ne porte encore ; ADR 0016 demande précisément qu'une chose
n'entre que lorsqu'un consommateur testé existe. Ils viendront avec W7 et W13.e.

**Prochain item.** **W13.d** — complétion de l'agrégat `Task` de §7.1, dont `assigned_agent_id` et
`assigned_worker_id`, sans toucher la machine à états existante.

---

## 2026-08-17 — W13.d — L'assignation est un événement, pas une transition

**Périmètre.** `packages/coordination/src/task.rs` et `tests/task.rs`, neufs ; `src/lib.rs` les
expose ; le `Cargo.toml` du paquet gagne `locus-domain`. **`packages/domain/src/task.rs` n'est pas
touché.**

**La décision du sprint, en clair.** Un état dit **où en est** le travail ; une assignation dit
**qui le fait**. Les deux changent indépendamment : une tâche `running` peut être réassignée après
la perte d'un lease sans jamais quitter `running`, et elle peut passer de `leased` à `running` sans
changer d'exécutant. Faire de l'assignation une transition obligerait à croiser quinze états avec
autant d'agents, et le premier changement d'agent en cours d'exécution rendrait la table fausse.

**Conséquence directe pour W13.g.** Le graphe organisationnel **réalisé** se dérive d'une suite
d'assignations, pas d'un champ courant. Une tâche qui a changé de main trois fois a trois faits à
consigner, et le dernier n'efface pas les deux premiers (invariant 12). `assigned_agent_id` et
`assigned_worker_id` de §7.1 existent — ce sont des **lectures** de la dernière assignation, pas le
lieu où l'information vit.

**Les deux identités, pas une.** Un worker est une machine, un agent est un rôle situé : deux agents
peuvent tourner sur le même worker, et un agent peut être réassigné d'un worker à un autre. §7.1
porte les deux champs, et n'en garder qu'un rendrait indécidable l'une des deux questions que W13.g
doit trancher — « qui a fait ce travail » et « où a-t-il tourné ».

**Ce module n'a pas de table à lui.** `moved_to` délègue à `locus_domain::transition` et rend
l'erreur du domaine telle quelle. Une divergence entre deux tables est donc impossible : elle n'a
pas d'endroit où s'écrire. La mutation qui remplace la délégation par une affectation directe le
confirme.

**Le test du diagramme énumère les arêtes de §7.1 plutôt que de parcourir la table.** Un parcours ne
vérifierait que la cohérence de la table avec elle-même. Les seize transitions du diagramme sont
donc écrites une par une, et quatre transitions qu'il ne dessine pas sont exigées refusées — c'est
ce qui rend le test capable de **voir** un changement de table, là où un parcours resterait vert.

**La clé d'idempotence est exigée dès la proposition.** C'est elle qui empêche qu'une reprise après
incident crée une seconde tâche pour le même travail ; une clé attribuée plus tard arriverait après
le doublon. Et le numéro d'attempt ne redescend jamais : `orphaned → queued` fait repartir la tâche
« sur un autre attempt », et réutiliser le numéro rendrait deux exécutions indiscernables dans le
journal.

**Six mutations vérifiées rouges** : l'assignation devenue transition (1 test) ; l'histoire écrasée
au profit de la dernière assignation (3) ; le worker disparu de l'assignation (2) ; une tâche finie
qu'on confie quand même (1) ; le numéro d'attempt remis à un (1) ; une table locale plus permissive
que celle du domaine (1). Restauration confirmée verte.

**Écart avec la spec.** Aucun. Les champs de §7.1 qui désignent des politiques — `success_contract`,
`review_policy_id`, `budget_reservation_id`, `capability_requirements` — restent absents pour la
raison d'ADR 0016 : rien d'exécutable ne les consomme encore. `capability_requirements` rencontrera
`Sources` de W13.c quand l'admission les confrontera, en W7.

**Prochain item.** **W13.e** — la relation de coordination (`kind` fermé à `review`), le payload de
`team.modify`, le CAS par `expected_revision`, l'annulation par commit inverse, l'autorité de
proposition agentique. C'est le premier item où une **relation** entre agents s'écrit, et ADR 0016
exige qu'elle ait un consommateur exécutable et testé.

---

## 2026-08-17 — W13.e — La proposition de coordination : un seul chemin, quatre bornes

**Périmètre.** `packages/coordination/src/proposal.rs` et `tests/proposal.rs`, neufs ; `src/lib.rs`
les expose. Aucune dépendance nouvelle.

**Une seule sorte de relation, et c'est la décision 4 appliquée.** `review`, parce qu'elle a un
consommateur exécutable et testé — l'indépendance de §14.4 et l'invariant 11 s'y appuient.
`mentors`, `delegates_to`, `supervises` n'en ont pas : les écrire en ferait du vocabulaire que rien
ne vérifie, et un test refuse explicitement de les relire.

**Le CAS n'invente rien.** La base d'une proposition **est** l'`expected_revision` de §22.2, et
`Expected` de `packages/event-store` « n'a pas de variante “peu importe” ». Ce module compare, et
c'est tout : aucun compteur, aucun magasin, aucun bus (décision 5). Le refus **dit quoi faire** —
`needs_rebase` et le message en toutes lettres — parce qu'un « conflit » sans consigne laisse
l'appelant réessayer à l'identique jusqu'à ce que quelqu'un lise le code.

**L'annulation est un commit inverse.** Chaque variante de `Change` a son inverse exact, et
`inverse().inverse()` est l'identité — sans quoi annuler une annulation dériverait. Aucune version
n'est supprimée : retirer une version rendrait l'histoire fausse, puisqu'on ne pourrait plus dire
qu'une mission a tourné sous une organisation qui, désormais, n'aurait jamais existé.

**La justification cite une révision, et l'existence est vérifiée par un port.** `EpistemicIndex`
pose la seule question dont une proposition a besoin — « cette révision existe-t-elle ? » — sans
traverser quoi que ce soit. C'est exactement ce que le commentaire de `boundaries.json` annonçait :
« une justification de proposition cite un objet épistémique par son `RevisionId`, obtenu de
`locus-domain` : elle ne traverse jamais le graphe. » Par **révision** et non par concept : citer un
`stable_id` désignerait « la dernière version, quelle qu'elle soit », donc une justification qui
change après coup.

**L'ordre des deux refus est une décision, pas un hasard.** Le mode est vérifié **avant** la
citation : un agent en `observed` ne doit pas apprendre, par la nature du refus, quelles révisions
existent. C'est peu, et c'est gratuit à tenir — une mutation qui inverse les deux fait rougir un
test écrit pour ça.

**Le même chemin pour un agent et pour un humain.** Décision 7 : « une proposition écrite par un
agent est **le même objet** qu'une proposition humaine et suit le même chemin ». Le test le vérifie
en faisant parcourir aux deux la même suite d'appels et en comparant ce qui en sort. Ce qui les
distingue est le **mode**, pas un second circuit.

**Le défaut est `observed`, et c'est §33.** « Rendre toute action autonome sans seuil humain » est
un non-objectif explicite de la V1 : le mode fermé n'est pas une précaution. `bounded` et `operator`
n'existent pas ici — ils demandent la classe de risque dérivée et l'anti-gaming, qui sont W14 et
W16. Un humain, lui, propose sous tout mode : le mode borne ce que les **agents** peuvent faire, pas
ce que l'institution peut décider d'elle-même.

**`forbid_self_approval` vaut pour tout le monde.** §20.3 le porte déjà, et ADR 0016 en fait une
borne qui ne se relâche dans aucun mode : c'est ce qui empêche un agent de contrôler les règles
décidant de son propre remplacement. Le test vérifie aussi le cas humain — ce n'est pas une méfiance
envers les agents.

**La troisième garantie se prouve par absence, à deux niveaux.** Aucun chemin de code ne modifie une
`MissionEnvelope` émise ni le hash de sa `ContextView` : d'abord parce que ce crate ne dépend ni de
`locus-lep` ni de `locus-graph` — il n'a aucun type de mission sous la main — ensuite parce qu'aucun
fichier source ne nomme la mission ni la vue de contexte, ce qui ferme la manipulation par chaîne.
Le test lit le `Cargo.toml` et les sources, pas une liste recopiée : c'est ce qui le rend capable de
voir arriver la dépendance qu'il interdit.

**Sept mutations vérifiées rouges** : le CAS qui ne compare plus (1 test) ; le refus qui cesse de
dire de rebaser (1) ; la justification non confrontée à l'index (1) ; l'auto-approbation redevenue
possible (1) ; le mode `observed` laissant proposer (2) ; l'inverse qui n'inverse plus (2) ; l'ordre
des refus inversé, révélant l'index (1). Restauration confirmée verte.

**Écart avec la spec.** Aucun. Ce que W13.e ne livre pas et que la spec nomme : le moteur de
politique lui-même (§20), qui « peut accepter, refuser, modifier ou soumettre à approbation ». Ici
la politique est réduite au mode et à `forbid_self_approval` — les deux bornes qu'ADR 0016 déclare
non relâchables. Le reste est W14.

**Prochain item.** **W13.f** — la projection du graphe d'exécution dans `packages/projections`,
reconstruite depuis zéro, avec la quarantaine conforme à ADR 0013. W13.b a déjà répondu à la
question qu'elle posait : le pli tient sur `lep/1.0` inchangé.

---

## 2026-08-17 — W13.f — La projection du graphe d'exécution, et deux gardes muettes

**Périmètre.** `packages/projections/src/execution_graph.rs` et `tests/execution_graph.rs`, neufs ;
`src/lib.rs` les expose. Aucune dépendance nouvelle. C'est la **troisième** projection du paquet,
après « état de validation » et « registre des conflits » (ADR 0013, décision 5).

**W13.b avait déjà payé la question.** Le pli avait établi que le graphe d'exécution est dérivable
de `lep/1.0` **tel quel** ; cette projection le confirme depuis le journal, sans qu'aucun champ ait
été ajouté au protocole. C'était l'ordre voulu par `docs/10` — « le pli décide si la projection
s'écrit contre `lep/1.0` inchangé, et le découvrir après aurait coûté la projection ».

**Aucun nœud d'agent, et c'est le sujet de W13.g.** Rien dans l'événement ne dit quel agent a agi.
La projection ne peut donc pas en fabriquer, et un `worker_id` ne fait pas un agent — confondre la
machine et le rôle rendrait le graphe organisationnel faux dès sa première jointure.

**Deux mutations sont passées vertes au premier essai, et les deux disaient la même chose.**

1. **Le préfixe de sorte retiré** laissait la suite verte : les tests employaient `node_id` pour
   composer leurs attentes, donc ils **suivaient** la mutation au lieu de la voir. Un test qui
   construit son attendu avec la fonction qu'il vérifie compare une valeur à elle-même. Le test
   ajouté écrit les identifiants en toutes lettres — `task:x`, `run:x`, `attempt:x#1` — et exige
   qu'une tâche et un run de même clé restent deux nœuds.
2. **Le résumé rendu constant** laissait la suite verte : `verify` compare le résumé courant à celui
   de la reconstruction, et deux constantes s'accordent toujours. Un résumé qui ne dépend pas de
   l'état ferait passer la propriété de reconstruction **pour n'importe quelle projection**. Le test
   ajouté exige que deux graphes différents aient des résumés différents, et qu'un même état en
   rende un stable.

C'est la même leçon qu'en W13.b — une garde muette est indiscernable d'un système sain — et c'est la
quatrième fois de ce chantier. Le motif se précise : **la garde et le test qui l'emploie ne doivent
pas partager de code**, sans quoi ils bougent ensemble.

**Ce que le test d'orphelines vaut, et comment on le sait.** La propriété tient par construction :
aucune arête n'est posée sans que ses deux nœuds aient été créés, et `ExecutionGraph` n'expose aucun
moyen d'en ajouter une autrement. Un test ne peut donc pas fabriquer d'orpheline sans écrire une
mise en scène — j'en avais commencé une, elle ne construisait rien. Ce qui donne prise au test est
la **mutation** : poser une arête avant de créer son nœud le fait rougir, ainsi que deux autres.

**La quarantaine est celle d'ADR 0013.** Décision 3 : une projection en défaut **s'arrête**, elle ne
saute pas — le test vérifie que l'événement suivant n'a pas été consommé, parce que sauter
présenterait un état amputé comme s'il était complet. Décision 4 : l'écriture canonique continue
pendant ce temps, et c'est ce qui distingue une projection en défaut d'une panne du système.

**Six mutations vérifiées rouges** : une arête posée avant son nœud (3 tests) ; le préfixe de sorte
retiré (1, après correction du test) ; l'attempt oubliant sa tâche (1) ; un artefact sans identité
accepté au lieu de mettre en quarantaine (2) ; `reset` gardant le watermark (1) ; le résumé rendu
constant (1, après correction du test). Restauration confirmée verte.

**Écart avec la spec.** Aucun. §9.3 liste douze projections ; ce paquet en porte trois, et ADR 0013
décision 5 dit pourquoi — « ne crée pas 34 stubs vides ». Les neuf autres attendent ce qui leur
donnera de quoi projeter.

**Prochain item.** **W13.g** — la projection du graphe organisationnel réalisé, par jointure
`assigned_agent_id` × événements. Ses deux dépendances, W13.b et W13.d, sont mergées.

---

## 2026-08-17 — W13.g — Le graphe organisationnel réalisé, et la fin de W13

**Périmètre.** `packages/projections/src/organisation_graph.rs` et `tests/organisation_graph.rs`,
neufs ; `src/lib.rs` les expose. Quatrième projection du paquet. Aucune dépendance nouvelle.

**C'est la jointure que W13 existait pour rendre possible.** W13.b avait établi que rien dans
l'événement `lep/1.0` ne dit quel agent a agi ; W13.d y a répondu en faisant de l'assignation un
**événement** plutôt qu'une transition d'état ; W13.g joint les deux. La chaîne complète a tenu, et
chaque maillon avait été posé pour celui-ci.

**Réalisé, et non prévu.** Un organigramme dit qui **devrait** faire quoi ; ce graphe dit qui l'a
**fait**. Les deux divergent dès qu'un lease se perd et qu'une tâche change de main, et c'est
précisément l'écart qu'on veut pouvoir lire. `current_agent` répond à « qui la fait », `assignments`
répond à « qui l'a faite » — la seconde question est celle qu'un graphe réalisé doit savoir
trancher, et l'invariant 12 interdit d'y répondre en effaçant.

**Aucun instantané n'est reçu du worker, et c'est l'invariant 3 appliqué à la lecture.**
L'assignation est une décision du plan de contrôle. Un agent qui en annoncerait une décrirait **sa
propre affectation**, et un graphe qui le croirait serait un graphe que les workers écrivent. Seul
un acteur `System` est source. L'événement d'un agent n'est pas une erreur — il reste journalisé, et
c'est bien ainsi — il n'est simplement pas une source. Un test le vérifie aussi pour un acteur
humain : la distinction n'est pas « agent contre humain », c'est « le plan de contrôle décide ».

**L'ordre fait partie de l'état.** « A puis B » n'est pas « B puis A », et le résumé le porte : un
résumé qui les confondrait rendrait la reconstruction incapable de détecter une inversion. Deux
journaux inverses produisent donc deux résumés différents, et le test l'exige.

**Une mutation est passée verte, et elle a montré un angle mort du harnais.** Un `reset` qui
garderait les assignations laissait la suite verte, parce que `verify` reconstruit une projection
**neuve** : `reset` y est appelé sur un état déjà vide, où oublier de le vider ne se voit pas. Or la
reconstruction **en place** est le cas réel — c'est ainsi qu'une projection sort de quarantaine. Le
test ajouté reconstruit un runner déjà peuplé et exige que les faits ne doublent pas.

C'est la cinquième garde muette de ce chantier, et la première qui ne vienne pas de mon code mais du
**harnais partagé** : `verify` ne peut pas, par construction, éprouver `reset` sur un état peuplé.
Les trois projections antérieures ont le même angle mort ; deux d'entre elles s'en sortent par un
test de reconstruction en place écrit pour d'autres raisons. À vérifier lors du prochain passage sur
`packages/projections`.

**Six mutations vérifiées rouges** : le graphe croyant les workers sur parole (2 tests) ; l'histoire
écrasée au profit de la dernière assignation (3) ; `current_agent` rendant la première au lieu de la
dernière (2) ; une assignation sans agent acceptée au lieu de mettre en quarantaine (1) ; le résumé
oubliant l'ordre (1) ; `reset` gardant l'histoire (1, après correction du test). Restauration
confirmée verte.

**Écart avec la spec.** Aucun.

**W13 est complet** — a, b, c, d, e, f, g. Le socle de coordination agentique existe : les agrégats
de §7.1, l'intersection des capacités de §14.2, l'assignation comme événement, la proposition avec
son CAS et ses bornes, et les deux graphes. Restent **W5.f** (bloqué sur un hôte capable de `S2`) et
**W7**, dont les dépendances W13.c et W13.d sont désormais satisfaites.

**Prochain item.** **W7** — mémoire, revue indépendante, budgets, portefeuille. Deux points que
`docs/10` signale comme faciles à rater : la prévention de contamination (§16.6) doit être testée
par un cas adverse explicite et pas seulement par construction, et l'anti-gaming du portefeuille
(§13.6) doit exister **avant** que la fonction de valeur pilote des décisions automatiques.

---

## 2026-08-17 — W7 découpé, et W7.a — la revue comme protocole

**Périmètre.** `docs/10` : W7 est redécoupé au commit près, comme le document l'exige de tout
workstream qui devient le prochain. Puis `packages/review`, neuf : `src/{dossier,review}.rs`,
`src/lib.rs`, `tests/review.rs` ; le `Cargo.toml` de l'espace de travail gagne le membre.
Dépendances : `locus-domain`, `locus-protocol`.

**L'ordre de W7 suit une seule idée.** Ce qu'un relecteur **ne voit pas** est décidé avant qu'il
relise. Le dossier se fige avant l'attribution (§17.3), l'indépendance se vérifie avant la remise,
et l'anti-gaming existe avant que la valeur pilote quoi que ce soit — dans chaque cas, l'inverse
produit un système qui a l'air de fonctionner. D'où deux contraintes d'ordre inscrites : W7.b (cas
adverses de contamination) **avant** W7.c (`ContextView`), parce qu'un cas adverse écrit contre une
`ContextView` déjà là serait écrit pour passer ; et W7.f (anti-gaming) avant tout usage automatique
de la fonction de valeur, ce qui est la mise en garde du workstream transformée en ordre de commits.

**Le dossier figé est une suite de types, pas un booléen.** §17.3 : « le dossier est figé **avant
attribution** ; toute modification entraîne une nouvelle version ou un addendum explicitement
visible ». `Draft` puis `Frozen` : un `Frozen` n'a aucune méthode qui change ce que le relecteur
consultera, et les deux issues de la phrase — addendum visible, nouvelle version — sont les deux
seules qui existent. Un dossier retouchable après attribution rendrait toute revue incontestable :
on ne saurait jamais si le relecteur a vu ce que le dossier dit aujourd'hui. Même forme que la
chaîne de build de W5.b, pour la même raison — un processus qui se déroule une fois, dans un ordre
qui est la garantie.

**L'indépendance est constatée, jamais déclarée.** `attest` confronte le relecteur au générateur,
exigence par exigence. C'est la **quatrième** occurrence de cette forme après l'attestation de
sandbox (W4.d.2), le digest de build (W5.e) et le niveau de reproductibilité (W6.d) ; le motif est
maintenant assez établi pour être une règle de conception du dépôt : _ce qui prouve ne peut pas être
ce qui est demandé_.

**Deux groupes inconnus ne sont pas deux groupes distincts.** Cinquième apparition de « l'absence de
preuve n'est pas une preuve », et ici elle a du mordant : conclure l'inverse ferait de l'ignorance
une garantie d'indépendance, c'est-à-dire exactement le contraire de ce que §14.4 demande.

**Trois exigences d'indépendance sur les dix de §14.4.** Celles qui ont un consommateur exécutable :
le groupe vient de W13.c, le worker distinct de W13.d, l'absence de transcript de l'invariant 11.
Les sept autres — familles de modèles, fournisseurs, corpus, outils, randomisation, anonymisation,
mémoire partagée — n'ont rien qui les vérifie, et les écrire en ferait du vocabulaire inerte (ADR
0016, décision 4).

**Une revue non indépendante reste une revue.** Elle est rendue, consignée, ses constats restent
lisibles. Ce qu'elle ne peut pas faire est **compter comme** la revue indépendante que la politique
exigeait. L'écarter effacerait un travail réel ; la confondre avec une revue indépendante serait
pire. `is_independent` dit la différence d'une seule voix.

**Un finding sans preuve ne décide de rien, même déclaré bloquant.** §17.5 le dit en toutes lettres,
et sans cette règle il suffirait d'écrire `blocking` pour bloquer. `is_binding` exige les deux
conditions — une preuve **et** une gravité — et un test vérifie que chacune manque à son tour.

**Six mutations vérifiées rouges** : deux groupes inconnus passant pour distincts (1 test) ; le
transcript cessant d'empêcher l'indépendance (1) ; un finding sans preuve devenu liant (1) ; la
revue de son propre travail redevenue possible (1) ; l'addendum réécrivant le dossier (1) ; un
dossier sans question qui se fige (1). Restauration confirmée verte.

**Écart avec la spec.** Aucun. Ce que W7.a ne livre pas et que §17 nomme : le rebuttal (§17.6) et la
méta-revue (§17.7), qui sont W7.d ; `provenance_view_id` et `reviewer_context_view_id`, qui
attendent la `ContextView` de W7.c ; `severity_schema`, qui est une politique et relève de W14.

**Prochain item.** **W7.b** — la prévention de contamination de §16.6, par **cas adverses**. Cinq
cas, un par forme nommée, et chacun doit échouer **avant** son correctif : c'est la mise en garde de
`docs/10` prise au mot.

---

## 2026-08-17 — W7.b — La contamination, par cinq cas adverses

**Périmètre.** `packages/review/src/contamination.rs` et `tests/contamination.rs`, neufs ;
`src/lib.rs` les expose. Aucune dépendance nouvelle.

**« Pas seulement par construction », pris au mot.** `docs/10` signale ce point comme facile à rater
et coûteux à réparer. La différence entre les deux façons de le traiter est celle entre « je ne vois
pas comment ça arriverait » et « voici comment on le fait arriver, et voici pourquoi ça échoue ».
Chaque cas de `tests/contamination.rs` **construit** la contamination, puis exige qu'elle soit vue.
Un test qui vérifierait qu'un contexte propre reste propre ne dirait rien : c'est le cas facile, et
c'est celui qu'on obtient sans y penser.

**Les cinq formes de §16.6, une par une.**

1. **Le transcript du générateur dans le contexte d'un relecteur aveugle** — l'invariant 11 pris de
   face. C'est la forme la plus banale, parce qu'elle ressemble à « donner du contexte utile ».
2. **Un claim réfuté servi par défaut.** Il n'est pas effacé du graphe — l'invariant 12 l'interdit —
   mais le garder et le **servir** sont deux choses différentes.
3. **Une donnée confidentielle sur un worker non habilité.** §16.2 parle de **plafond**, donc d'un
   ordre : la comparaison est décidable sans énumérer les combinaisons. Un test vérifie que le
   plafond laisse passer ce qui est en dessous, sinon ce serait une égalité stricte.
4. **Le consensus circulaire.** Un cycle de citations dont **aucun** membre ne cite de source
   externe. Deux agents qui se citent **et** citent le monde extérieur ne sont pas circulaires : ils
   s'appuient sur quelque chose. Sans cette nuance, la détection interdirait toute citation
   mutuelle, c'est-à-dire la discussion. Un cas à trois membres vérifie que la détection n'est pas «
   A cite B qui cite A » écrit en dur.
5. **La contradiction perdue à la synthèse.** La plus difficile à voir, parce qu'une synthèse
   amputée est **plus lisible** que celle qui garde la contradiction — elle a l'air meilleure.

**Un contexte contaminé de trois façons produit trois constats.** S'arrêter au premier ferait
réparer une fuite en laissant les suivantes, et le rapport donnerait l'impression du contraire. Une
mutation qui tronque la liste à un élément fait rougir trois tests.

**Ce que ce module est, et ce qu'il n'est pas.** Un ensemble de **constats**, pas un filtre : il
regarde un contexte déjà constitué et dit ce qui y est contaminé. Un filtre serait préférable — et
W7.c le fera pour la `ContextView` — mais un filtre non éprouvé est un filtre qu'on croit efficace.
C'est la raison de l'ordre inscrit en tête de W7 : l'adversaire d'abord, la construction ensuite.

**Sept mutations vérifiées rouges** : la fuite de transcript non vue (2 tests) ; le claim réfuté
redevenu ordinaire (2) ; le plafond devenu égalité stricte (1) ; l'ordre de sensibilité inversé (3)
; la source externe cessant de compter dans un cycle (1) ; l'inspection tronquée au premier constat
(3) ; la contradiction oubliée non signalée (1). Restauration confirmée verte.

**Écart avec la spec.** Aucun.

**Prochain item.** **W7.c** — la `ContextView` de §16.2 : ce qui a été vu, arrêté par hash et par
watermark. Les cas adverses de ce sprint existent maintenant **avant** elle, ce qui est exactement
ce que l'ordre de W7 cherchait à obtenir.

---

## 2026-08-17 — W7.c — La `ContextView` : ce qu'on savait, et de quand

**Périmètre.** `packages/review/src/context_view.rs` et `tests/context_view.rs`, neufs ;
`src/lib.rs` les expose. Aucune dépendance nouvelle.

**Deux mots portent tout : immuable, et watermark.** §16.2 : « une `ContextView` est immuable,
adressée par hash et rattachée à l'exécution. Elle permet de savoir exactement ce que l'agent
**pouvait** connaître. » Sans immuabilité, la vue dit ce qu'on sait aujourd'hui plutôt que ce qu'on
savait ; sans watermark, elle ne dit pas de quand date ce « aujourd'hui ».

**Une vue ne contient pas l'avenir.** Un élément venu d'une position postérieure au watermark est
refusé, et le refus le dit. C'est la faute qu'on ne peut plus détecter après coup si on ne la refuse
pas à la construction : une vue qui contiendrait un événement plus récent qu'elle-même paraîtrait
simplement mieux informée.

**La borne est inclusive**, et un test le fixe : le watermark est « jusqu'où on a lu », pas « avant
où ». La mutation qui la rend exclusive fait rougir.

**Le filtre est celui de W7.b, écrit avant.** C'est l'ordre inscrit en tête de W7 qui paie ici : les
cas adverses existaient avant cette vue, donc ils n'ont pas pu être écrits pour qu'elle passe. Et si
W7.b apprend une sixième forme de contamination, la vue s'en protège sans être modifiée.

**Ce qui est écarté est nommé.** §16.2 porte `redactions` : ce qui a été retiré fait partie de ce
que la vue dit. Une exclusion silencieuse rendrait indiscernables deux vues vides — celle qui
n'avait rien à écarter et celle qui a tout écarté. Un test compare exactement ces deux-là.

**Une vue ne s'augmente pas.** Aucune méthode n'ajoute d'élément après construction : voir plus
demande une **autre** vue, avec son propre watermark et son propre hash. C'est la même forme que le
dossier figé de W7.a, et pour la même raison — ce qui a été vu doit rester ce qui a été vu.

**Six mutations vérifiées rouges** : la vue acceptant l'avenir (1 test) ; la borne rendue exclusive
(1) ; le filtre de contamination débranché (3) ; la rédaction rendue silencieuse (2) ; `could_know`
répondant toujours oui (2) ; le plafond de la vue cessant d'être celui du destinataire (1).
Restauration confirmée verte.

**Écart avec la spec.** Aucun. §16.2 porte dix-huit champs ; six sont modélisés — ceux dont un
consommateur exécutable existe. `query`, `included_types`, `max_depth`, `diversity_policy`,
`token_budget` et les autres appartiennent au retrieval hybride de §16.3, qui n'est pas dans W7 tel
que découpé : ils arriveront avec le moteur qui les lit.

**Prochain item.** **W7.d** — rebuttal et méta-revue (§17.6, §17.7).

---

## 2026-08-17 — W7.d — Le rebuttal et la méta-revue : le désaccord survit à la synthèse

**Périmètre.** `packages/review/src/rebuttal.rs` et `tests/rebuttal.rs`, neufs ; `src/lib.rs` les
expose. Aucune dépendance nouvelle.

**Ce que ces deux objets ajoutent.** W7.a a livré la revue : un dossier figé, des constats, une
attestation d'indépendance. Il y manquait la **réponse**. §17.1 exige que la revue rende explicites
« les findings, **les réponses** et la décision finale » — sans rebuttal, un constat est un verdict
sans recours, et le protocole cesse d'en être un.

**Un rebuttal ne s'écrit pas sans constat.** §17.6 fait du `finding_id` un champ obligatoire, donc
le type l'exige à la construction : une réponse qui ne répond à rien est une prise de parole, pas un
rebuttal. C'est la différence entre un protocole et un fil de discussion.

**La politique décide qui reprend, et le défaut suit le texte.** §17.6 dit « le reviewer initial
**peut** effectuer un recheck » avant de mentionner la politique plus stricte : `RecheckPolicy`
défaut à `InitialReviewer`, et `FreshReviewer` se demande. Un défaut caché dans le code déciderait à
la place de la politique — et déciderait dans le sens le plus sévère, ce que la spec ne dit pas.

**Une méta-revue relit les revues, pas le travail.** C'est la distinction qui décide de tout le
reste : une méta-revue qui refait la revue n'est qu'une revue de plus. Elle ne rouvre aucun dossier
et ne produit aucun constat propre. Elle mesure l'indépendance effective, signale les avis corrélés,
garde les minoritaires, et recommande.

**L'indépendance effective n'est pas le nombre de revues.** §17.7 : elle « mesure l'indépendance
**effective** ». Trois revues dont deux partagent un groupe n'en font pas trois indépendantes, et
compter les revues ferait passer un consensus pour une convergence. Quand elle tombe à zéro, la
recommandation est `human_escalation` : recommander `validate` ferait d'un **défaut de procédure**
un verdict scientifique, ce qui est la façon la plus discrète de rendre une revue inutile.

**Le minoritaire est le côté le moins nombreux, quel qu'il soit.** §17.7 : la méta-revue « ne masque
**jamais** les opinions minoritaires ». L'implémentation évidente — garder les opposants — ferait
disparaître l'unique voix favorable au milieu de réfutations, ce qui est exactement ce que la règle
cherche à empêcher. Deux tests symétriques le fixent dans les deux sens.

**La corrélation se signale, elle ne se conclut pas.** §17.7 demande de « détecter les findings
corrélés ou copiés ». Deux relecteurs qui rendent les mêmes verdicts sur les mêmes cibles
n'apportent pas deux avis — ce qui ne prouve pas la copie. La méta-revue le signale donc, et
s'arrête là.

**Ce que la neuvième mutation a trouvé.** Deux relecteurs qui n'ont **rien** trouvé avaient des
signatures égales, donc étaient signalés comme corrélés. La garde existait ; aucun test ne la
tenait, et elle passait verte une fois mutée. Une revue sans constat est légitime — `Review::render`
n'exige que la couverture — donc le cas se produit sans qu'on le cherche. Ne rien trouver deux fois
n'est pas se copier : le test manquant a été écrit.

**Une variante inerte retirée.** `RebuttalError::ContestsWithoutSaying` était déclarée, documentée
comme retournable, et produite par rien : `to_finding` refuse la réponse vide avant qu'une partie
puisse être contestée, et `contesting()` ne peut pas échouer. C'est la sémantique inerte que l'ADR
0016 refuse — un cas d'erreur qu'aucun consommateur ne peut produire est une promesse que le code ne
tient pas. Retirée.

**L'absence de preuve n'est pas une preuve, et ce n'est pas un rien non plus.** §17.7 demande que la
méta-revue « distingue absence de preuve, contradiction et réfutation ». Un constat liant qui dit «
il n'y a pas de quoi conclure » ne réfute pas — et valider serait faire de ce manque un résultat.
C'est `Revise` : la première des trois confusions que §17.7 nomme est celle-là, et elle est écartée
par une règle, pas par un commentaire. §17.5 continue de valoir en dessous : une insuffisance **non
étayée** reste un commentaire, sans quoi un doute non argumenté suffirait à bloquer une validation.

**Douze mutations vérifiées rouges** : la réponse vide acceptée (1 test) ; la politique de recheck
ignorée (1) ; le défaut basculé vers `FreshReviewer` (1) ; la méta-revue de sa propre revue
autorisée (1) ; l'indépendance effective ramenée au nombre de revues (2) ; la détection de
corrélation débranchée (1) ; le minoritaire toujours pris du côté des réfutations (1) ; un constat
sans preuve pesant autant qu'un autre (1) ; la garde de signature vide retirée (1) ; la révision
ramenée à une validation (1) ; l'insuffisance confondue avec un autre verdict (1) ; l'insuffisance
non liante comptée quand même (1). Les trois dernières lignes et la garde de signature vide ont
d'abord été **muettes** : les tests manquants ont été écrits avant de les recompter. Restauration
confirmée verte.

**Écart avec la spec.** Un, nommé. §17.7 nomme six recommandations, les six existent, et cinq sont
produites par une règle. `Reproduce` ne l'est pas : elle appartient au moteur de reproduction de
W6.e, qui n'est pas encore branché sur la revue — recommander de reproduire demande de savoir ce
qu'une reproduction déciderait, et l'inventer ici serait simuler une capacité. Elle reste donc
**nommée** : `slug()` la rend, l'énumération la porte, et la méta-revue ne la choisit pas encore. La
différence avec la sémantique inerte retirée plus haut est qu'un producteur non automatique existe —
un méta-relecteur humain rapporte une recommandation — alors qu'une variante d'erreur ne peut venir
que du code.

**Prochain item.** **W7.e** — budgets : réservation avant exécution, dépassement (§17, invariant 6).

---

## 2026-08-18 — W7.e — Le budget : ce qui empêche, et ce qui constate

**Périmètre.** `packages/budget`, neuf : `dimension.rs`, `limits.rs`, `ledger.rs`, `account.rs`,
`tests/budget.rs`. `packages/protocol/src/id.rs` gagne deux préfixes provisoires (`budg`, `resv`),
comme W13.c l'avait fait pour `task`, `team`, `dec` et `apr` — §10.1 n'en donne pas d'exemple.
Aucune dépendance nouvelle : le crate ne dépend que de `locus-protocol`.

**Deux rôles, et il faut les deux.** La **réservation** empêche : elle est refusée quand la borne ne
suit pas. Le **registre** constate : il écrit ce qui a été dépensé, y compris au-delà de ce qui
était retenu. Les confondre casse l'un des deux — un registre qui empêche ment sur le passé, une
réservation qui constate n'empêche rien. C'est pour cela que le dépassement s'**écrit** : refuser
l'écriture laisserait le journal en désaccord avec le monde, les ressources ayant bien été
dépensées, et rendrait le dépassement invisible là où il fallait le voir.

**Le budget est un registre, pas un compteur.** §7.2 ouvre par cette phrase et elle décide de la
forme du type : aucun solde n'est un champ. `allocated`, `held` et `spent` se déduisent des
écritures à chaque lecture. Un compteur entretenu à côté du journal serait une seconde vérité, et
c'est toujours la seconde qui ment.

**`Reservation` n'a pas de constructeur public.** Invariant 6 — « les ressources sont réservées
avant exécution » — devient indéfaisable plutôt que documenté : seul `BudgetAccount::reserve`
produit la valeur, et `consume`/`release` la prennent **par valeur**, donc une retenue se solde une
fois. Clippy propose une référence ; l'`#[expect]` dit pourquoi c'est non : par valeur, la double
dépense est une erreur de compilation, par référence elle ne serait qu'une erreur d'exécution.

**Ce qui n'est pas nommé n'est pas libre.** Une dimension hors des bornes du compte n'est pas «
illimitée », elle est **hors budget** : rien ne peut y être réservé. C'est la moitié de l'invariant
6 qu'on perd le plus facilement — borner deux ressources sur six et croire les six bornées. De même,
une borne n'est pas une dotation : rien n'est disponible avant d'avoir été alloué.

**Une correction ne réécrit rien.** `reconcile` compare la consommation enregistrée aux métriques du
worker et écrit l'écart : `adjustment` à la hausse, `refund` à la baisse. Un test relit l'écriture
corrigée **après** la correction et la trouve intacte — sans quoi un budget dépassé puis corrigé
serait indistinguable d'un budget jamais dépassé.

**Ce que la roadmap disait de travers.** La ligne W7.e renvoyait à « §17 » ; §17 est le système de
revue, que W7.a–W7.d viennent de construire. Les budgets sont en **§7.2**. La référence venait de la
numérotation de Canterel, où W2.13 portait le budget local sous §17. Corrigée dans `docs/10`.

**Seize mutations vérifiées rouges** : un compte sans borne qui s'ouvre (1 test) ; une dimension non
bornée devenue illimitée (1) ; une retenue vide acceptée (1) ; un identifiant de retenue réemployé
(1) ; la retenue ouverte ne comptant plus contre la borne (2) ; l'allocation dépassant la borne dure
(1) ; la borne devenue dotation (1) ; le dépassement non rapporté (1) ; le dépassement rendu
réessayable (1) ; le code d'erreur cessant d'être `budget_exhausted` (1) ; l'erreur de budget rendue
sensible (1) ; la retenue d'un autre compte acceptée (1) ; la correction réécrivant au lieu de
compenser (1) ; un rapprochement qui confirme écrivant quand même (3) ; le rapprochement d'une
retenue jamais consommée (1) ; le franchissement de borne cessant d'être visible (1). Restauration
confirmée verte.

**Écart avec la spec.** Aucun. §7.2 porte six champs `limit_*` : les six dimensions existent. Les
six écritures obligatoires existent. `currency` et `policy_id` ne sont pas modélisés — aucun
consommateur exécutable ne les lit encore, et `Dimension::Amount` compte en micro-unités de la
devise du compte sans avoir à la nommer tant que rien ne convertit.

**Prochain item.** **W7.f** — portefeuille : les indicateurs de §13, et **l'anti-gaming de §13.6
d'abord**. L'ordre est inscrit dans la roadmap et il est le point du sprint.

---

## 2026-08-18 — W7.f — Le portefeuille : l'anti-gaming d'abord, la valeur ensuite

**Périmètre.** `packages/portfolio`, neuf : `activity.rs`, `gaming.rs`, `value.rs`, et deux fichiers
de tests. Deux commits, dans cet ordre : le criblage seul, puis la fonction de valeur. Aucune
dépendance nouvelle.

**L'ordre est le sprint.** `docs/10` l'inscrit : « l'anti-gaming doit exister avant que la fonction
de valeur pilote des décisions automatiques ». Ce n'est pas de la prudence, c'est un ordre de
dépendance : une fonction de valeur mise en service avant ses garde-fous **enseigne** ce qu'il faut
optimiser, et ce qu'elle enseigne alors est la faille. Ajouter les détecteurs ensuite ne défait pas
ce qui a été appris.

**Ce que le squash détruit, et ce qui survit.** La roadmap demande que l'ordre soit attesté par un
test. Un test qui lirait `git log` n'attesterait rien après un merge écrasé — l'ordre des commits ne
survit pas au squash. Ce qui survit est le **type** : `Screening` n'a pas d'autre constructeur que
`screen`, et `value` l'exige. Une branche jamais criblée n'a donc pas une valeur haute, elle n'a
**pas de valeur**. L'ordre des commits l'atteste une fois ; le type l'atteste toujours.

**Le test qui porte le sprint se lit en trois temps.** La manœuvre gonfle l'indicateur visé ; elle
gonfle aussi `V(b)` **brut** ; et elle perd une fois §13.6 appliqué. Le deuxième temps est celui
qu'on serait tenté d'omettre, et sans lui le test serait creux : on ne saurait plus si la pénalité a
renversé quelque chose ou si la stratégie était mauvaise dès le départ. Les deux branches passent
par la **même** règle d'indicateur — poser à la main des indicateurs plus bas pour la tricheuse
aurait supposé la conclusion.

**La pénalité porte sur les termes positifs, et c'est un piège évité.** Une pénalité multiplicative
sur la valeur nette rapprocherait de zéro une branche de valeur **négative**, c'est-à-dire
l'améliorerait : tricher paierait précisément sur les branches qu'il faut abandonner. En pénalisant
ce que la manœuvre gonfle, la pénalité ne peut jamais remonter une valeur — un test le fixe.

**La similarité est un port.** « Duplications paraphrastiques » demande de savoir si deux énoncés
disent la même chose ; le domaine ne le sait pas et ne le simule pas. `LexicalSimilarity` est un
**plancher** lexical, le nom le dit, et un test montre qu'un autre index change le verdict sans
qu'on touche au détecteur. Ce que le plancher attrape — la reformulation cosmétique — est la forme
la moins coûteuse à produire, donc la plus probable.

**Les seuils et les coefficients sont des politiques, pas des vérités.** §13.4 le dit de la formule
et cela vaut des seuils : ils sont explicites, remplaçables, et enregistrés avec le résultat. Les
coefficients par défaut valent tous 1 — §13.4 donne la **forme** de la formule, pas ses nombres, et
inventer ici des coefficients réglés les ferait passer pour la spec parce qu'ils seraient écrits en
Rust. Le défaut neutre dit « aucune pondération n'a été décidée », ce qui est vrai.

**Le refus du non-fini appartient à ce sprint, pas au suivant.** Un `NaN` ne se compare à rien, pas
même à lui-même : une branche qui en porte un ne serait ni meilleure ni pire que les autres, donc
invisible au tri, sans qu'aucune erreur ne le dise. W7.g triera sur ce nombre — le refuser ici évite
d'avoir à expliquer là-bas pourquoi une branche a disparu.

**Vingt-six mutations vérifiées rouges.** Dix-sept sur le criblage : le compte de claims triviaux
non vu (4 tests) ; le seuil de volume disparu (10) ; la confiance comparée à elle-même (1) ;
l'absence de verdict valant échec (7) ; la duplication non vue (2) ; le port de similarité
court-circuité (1) ; le taux d'aboutissement ne comptant plus (3) ; la collusion à sens unique
suffisante (2) ; un refus ne défaisant plus l'entente (2) ; l'unité logique ne comptant plus (1) ;
la taille d'artefact ne comptant plus (1) ; les métriques tues (2) puis ajoutées (3) ne comptant
plus ; l'absence de pré-enregistrement devenue un aveu (1) ; le criblage s'arrêtant au premier
constat (2) ; la pression non bornée (1) ; les seuils employés non enregistrés (1). Neuf sur la
valeur : la pénalité débranchée (1) ; la pénalité rendue multiplicative sur la valeur nette (1) ; le
non-fini accepté (2) ; les coefficients échappant au contrôle de finitude (1) ; `p_s` cessant d'être
une probabilité (1) ; la borne 1 exclue (1) ; les paramètres non enregistrés (1) ; le coût cessant
d'être soustrait (1) ; un coefficient cessant de compter (1). La garde de taille d'artefact était
d'abord **muette** — le test manquant a été écrit avant de la recompter. Restauration confirmée
verte.

**Écart avec la spec.** Un, nommé. §13.2 liste quinze indicateurs ; `Indicators` en porte **dix** —
ceux que `V(b)` consomme. Les cinq autres (vélocité, couverture de l'espace des stratégies, risque
de verrouillage conceptuel, niches méthodologiques, part d'exploitation) appartiennent à la
qualité-diversité de §13.3, qui décide d'un **portefeuille** et non d'une branche : les valoriser
ici reviendrait à noter une branche sur ce que font les autres. Ils arrivent avec W7.g. §13.4
mentionne aussi les « incertitudes » et les « overrides » parmi ce qui doit être enregistré : ni
l'un ni l'autre n'a de consommateur exécutable, et `p_s` porte seule la part d'incertitude
modélisée. §13.5 (les dix actions) n'est pas dans ce sprint.

**Prochain item.** **W7.g** — scheduler qualité-diversité : deux propositions de valeur égale et de
diversité inégale ne se départagent pas au hasard, et le choix est reproductible.

---

## 2026-08-18 — W7.g — Le scheduler qualité-diversité : rien n'est départagé au hasard

**Périmètre.** `packages/portfolio/src/scheduler.rs` et `tests/scheduler.rs`, neufs ; `src/lib.rs`
les expose. Aucune dépendance nouvelle. W7 est terminé.

**La phrase qui interdit le tri simple.** §13.3 : « La V1 **NE DOIT PAS** sélectionner uniquement
les branches au score moyen le plus élevé. » Un scheduler qui trierait par `V(b)` et couperait à N
serait conforme à §13.4 et en violation de §13.3 — et c'est le code qu'on écrit sans y penser, parce
qu'il est le plus simple à écrire et le plus facile à défendre ligne à ligne.

**« Reproductible » demande plus que « déterministe ».** Un scheduler qui garde le premier arrivé en
cas d'égalité est déterministe : la même liste donne le même résultat. Il n'est pas reproductible
pour autant — c'est l'**ordre d'arrivée** qui décide, en silence. L'ordre de sélection est donc
total : valeur décroissante, diversité décroissante, identifiant croissant. Le dernier barreau ne
dit rien de scientifique, et c'est exactement son rôle. Le test central mélange la liste d'entrée —
toutes les rotations, plus l'ordre inverse — et exige le même portefeuille.

**Trois phases, dans cet ordre.** La part d'exploitation au meilleur score ajusté ; la réserve
exploratoire au plus **loin** de ce qui est retenu, sans regarder le score ; le devoir de
falsification, qui déplace au besoin. La réserve après l'exploitation parce qu'une réserve remplie
en premier serait remplie contre rien ; le devoir en dernier parce qu'il faut savoir quelles
hypothèses sont finalement retenues.

**Un plancher trouvé par un test rouge.** Avec une seule place, `1 × 60 / 100` donne zéro place
d'exploitation : le portefeuille devenait **entièrement** exploratoire, l'inverse de « une part
d'exploitation ». Au moins une place d'exploitation dès qu'il y a une place ; la réserve apparaît à
partir de deux. À une seule place, les deux exigences ne peuvent pas tenir ensemble et c'est
l'exploitation qui l'emporte — c'est écrit dans le code plutôt que subi.

**Un bug attrapé par le premier essai.** `max_by` avec un comparateur inversé rend le **minimum** :
le scheduler choisissait la pire branche à chaque place. Six tests rouges d'un coup. La forme juste
est `min_by` avec le comparateur inversé — le « minimum » est alors le meilleur score, et `min_by`
rend le **premier**, donc le mieux classé. Une mutation le refixe : la remettre en `max_by` fait
rougir sept tests.

**Ce qui manque est rendu.** Quand aucun candidat ne falsifie une hypothèse retenue, l'exigence de
§13.3 ne peut pas être tenue : `unfalsified_hypotheses()` la nomme. La taire ferait passer un
portefeuille incomplet pour un portefeuille conforme. Et une hypothèse **non retenue** n'entraîne
aucun devoir — le portefeuille ne doit pas de contradiction à une piste qu'il n'a pas prise.

**Ce que les mutations ont trouvé.** Cinq gardes étaient muettes, et toutes pour la **même** raison
: la limite de concentration écartait déjà le candidat que le test croyait voir écarté par la
corrélation ou par le critère de la réserve. Les familles des fixtures étaient trop semblables. Les
tests ont été refaits avec des familles distinctes et un nombre de places suffisant, de sorte que
chaque garde soit seule à décider — c'est le même piège que W7.f : un test qui passe pour la
mauvaise raison passe quand même.

**Quatorze mutations vérifiées rouges** : l'égalité cessant d'être tranchée par la diversité (1
test) ; le dernier barreau de l'ordre disparu (1) ; la valeur cessant de primer sur la diversité (1)
; la réserve devenue une seconde exploitation (1) ; toutes les places données à l'exploitation (2) ;
la limite de concentration disparue (1) ; la pénalité de corrélation débranchée (2) ; la corrélation
ignorant la famille de méthode (1) puis les niches (3) ; la prime au négatif informatif disparue (1)
; le devoir de falsification non servi (2) ; l'hypothèse non falsifiée non signalée (1) ; le devoir
valant aussi pour les hypothèses non retenues (1) ; `min_by` redevenu `max_by` (7). Restauration
confirmée verte.

**Écart avec la spec.** Aucun. Les sept exigences de §13.3 sont tenues : part d'exploitation,
réserve exploratoire, niches méthodologiques (elles entrent dans la corrélation et dans l'ordre),
branche de falsification par hypothèse majeure, pénalité de corrélation, prime aux négatifs
informatifs, limite de concentration par famille de modèle **et** de méthode. §13.5 — les dix
actions du portefeuille — reste hors de W7 : le scheduler dit _quoi retenir_, pas _quoi faire
ensuite_.

**Prochain item.** W7 est terminé (a → g). Le prochain item non terminé de `docs/10` dont les
dépendances sont satisfaites reprend la file ordinaire.

---

## 2026-08-18 — W8.a — Le test de séparation, au seul moment où il est gratuit

**Périmètre.** `apps/emacs/` gagne son premier Elisp : `locus.el`, `locus-protocol.el`,
`test/locus-separation-test.el`, `test/locus-protocol-test.el`, `README.md`.
`tooling/emacs/run-tests.ts` est neuf ; `tooling/boundaries/emacs.ts`,
`tests/boundaries/contract.test.ts`, `package.json`, `.github/workflows/ci.yml`,
`docs/10_V1_ROADMAP.md` et `apps/emacs/SPEC.md` sont modifiés. Aucune dépendance nouvelle : le
paquet ne dépend que d'Emacs.

**Ce que le sprint établit.** La frontière 5 passe de « sans objet » à « vérifiée sur 2 fichier(s)
». `docs/10` fixe ce commit en premier de W8 parce que la dépendance qu'on veut interdire ne
s'ajoute jamais délibérément : elle s'installe le jour où une fonction du cockpit a besoin d'une
chose que la configuration de l'auteur fournit déjà, et elle est alors invisible dans le diff.

**Charger ne coûte rien.** Le paquet n'ouvre aucune connexion, n'arme aucun timer, ne lance aucun
processus. C'est `SPEC.md` §7.1 et le critère qui prime dans le `CLAUDE.md` de `emacs-config` — le
startup reste fonctionnel sans réseau et sans que Locus tourne.

**Le paquet est petit exprès.** Les neuf options publiques de `SPEC.md` §5 ne sont pas déclarées :
aucun code ne les lit encore, et une option que personne ne consulte est une promesse d'API que rien
ne tient. Elles arriveront avec leur lecteur, comme les recommandations de §17.7 en W7.d.

**Une contradiction de la spec tranchée.** §3 nommait les fichiers `locusolus-*.el` ; §5 nomme les
options publiques `locus-*`. En Emacs Lisp le préfixe des symboles est celui du fichier d'entrée :
les deux moitiés ne pouvaient pas être vraies ensemble, et `package-lint` refuse le décalage, donc
les canaux d'installation que §4.4 exige de garder ouverts. L'arbitrage garde §5 **mot pour mot** —
ce sont les options qu'une configuration consommatrice écrit, les renommer casserait des
utilisateurs — et fait suivre les noms de fichiers. Amendement inscrit dans `SPEC.md` §3 plutôt que
silencieux.

**Ce que les mutations ont trouvé, et c'est le vrai résultat du sprint.** Deux mutations —
`(load "/chemin/vers/la/config/perso.el")` et un `require` sous un `load-path` lié par `let` — font
rougir la suite ERT et laissent la **garde TypeScript verte**. Elle ne comparait que `load-path`
avant et après ; un `load` par chemin absolu, ou un `load-path` restauré en sortant du `let`, ne
laisse aucune trace là. Or c'est exactement la forme réaliste de la dépendance que la règle 5 existe
pour interdire. La garde lit désormais aussi `load-history` : les deux mutations la font rougir. Une
garde dont l'angle mort couvre le cas nominal de ce qu'elle interdit ne garde rien.

**Deux gardes, aucun code partagé.** La suite ERT depuis l'intérieur du paquet, la règle 5 depuis
l'extérieur, en TypeScript. Elles vérifient maintenant la même propriété par deux implémentations
indépendantes — ce n'est pas une redondance : la suite ERT disparaîtrait avec le paquet, la garde
non, et l'inverse pour ce qui est de tourner sans Node.

**Trois bugs de test attrapés avant de compter quoi que ce soit.** La racine du dépôt calculée à
l'exécution, où `load-file-name` est nil et le repli remonte d'un niveau de trop. `timer-idle-list`
exigé vide, alors qu'Emacs y met les siens — le test échouait sur le comportement d'Emacs, pas sur
celui du client. Et surtout : `--feature-file` cherchait la `feature` comme **clé** de
`load-history`, où elle figure en réalité sous `(provide . FEATURE)` parmi les valeurs. Il rendait
donc toujours nil, ce qui faisait passer n'importe quelle bibliothèque tierce pour un composant
d'Emacs : le test des dépendances était vert en ne regardant rien.

**Un test de W0.3 périmé par ce sprint.** « une règle sans objet est déclarée comme telle »
employait le dépôt lui-même comme fixture, et disait vrai tant qu'`apps/emacs` était vide. La
prémisse est désormais construite dans un répertoire temporaire, et un test complémentaire fixe
l'acquis : sur le dépôt réel, la règle 5 doit être `enforced`, jamais sautée.

**Neuf mutations vérifiées rouges** : le client se déclarant connecté sans l'être (2 tests) ; le
chargement armant un timer de reconnexion (1) ; le chargement sortant du répertoire du paquet (1) ;
le client ne disant plus sa version (1) ; la version de protocole dérivant du schéma (1) ; le client
se mettant à arbitrer la compatibilité (1) ; le paquet chargeant une bibliothèque de la config
personnelle (1 en ERT, **et la garde TS après correction**) ; le même par `load-path` élargi en
silence (1 en ERT, **et la garde TS après correction**). Restauration confirmée verte.

**Le dixième portail.** `npm run check` compte désormais dix portes : `check:emacs` s'insère après
`check:boundaries`. En CI elle porte `--require-emacs`, pour la même raison que la garde — une suite
qui se saute en silence ressemble en tout point à une suite qui passe.

**Écart avec la spec.** Un, nommé. `Package-Requires` déclare Emacs 30.1, comme `SPEC.md` §4.1
l'exige, mais la CI vérifie la séparation sous l'Emacs que fournit le runner — 29.3 ici. Le paquet
n'emploie rien qui manque à 29, donc le chargement prouve ce qu'il prouve ; épingler un runner Emacs
30 est une question de CI, pas de code, et elle n'est pas tranchée dans ce sprint.

**Prochain item.** **W8.b** — client HTTP/stream et authentification abstraite (§6). C'est le
premier item qui lira une option publique de §5, donc le premier qui en déclarera.

---

## 2026-08-18 — W8.b — L'identité : le credential est prêté, jamais gardé

**Périmètre.** `apps/emacs/locus-auth.el` et `test/locus-auth-test.el`, neufs ; `locus.el` déclare
`locus-endpoint`, sa première option publique de `SPEC.md` §5 — parce qu'un lecteur existe
désormais. Aucune dépendance nouvelle : `auth-source` et `url-parse` sont dans Emacs.

**La forme du module vient de sa liste d'interdits.** §6.2 énumère quatre choses qui ne doivent pas
arriver : aucun token dans Git, dans `custom-file`, dans un message de debug, dans le kill-ring. Les
quatre ont la même cause — un secret rangé dans une variable finit par être sauvegardé, affiché ou
copié, parce que c'est ce qu'on fait des variables. Le module ne les traite donc pas un par un : il
ne garde jamais le secret. `locus-auth-call-with-credential` le **prête** le temps d'un appel au
lieu de le rendre. Un `locus-auth-credential` qui renverrait la chaîne serait plus commode et ferait
dépendre les quatre interdits de la discipline de chaque appelant.

**Une identité absente n'est pas une panne.** C'est le cas le plus fréquent — première installation,
machine neuve, entrée expirée — et le pire endroit pour un backtrace. `locus-auth-missing` nomme le
fichier et donne la ligne à y écrire. « Actionnable » se teste : le message contient le geste
suivant.

**Le refus de changement d'origine est dans le code, pas dans une invite.** §6.2 demande une
confirmation si l'endpoint change d'origine ; `locus-auth-check-endpoint` **signale**, et c'est
l'appelant qui rattrape pour demander. Une invite qu'un appelant oublie d'afficher n'empêche rien,
alors qu'une erreur non rattrapée s'entend. L'origine comprend le schéma et le port : passer de
`https` à `http` ou changer de port change l'interlocuteur autant que changer de domaine, et c'est
le cas qu'une comparaison de noms d'hôte rate. Symétriquement, le port implicite vaut le port écrit
— une confirmation qui se déclenche pour rien finit par être cliquée sans être lue.

**Ce que les mutations ont trouvé, en trois temps.** La mutation « le secret est mis en cache » est
passée **verte deux fois** avant d'être rouge, et chaque échec disait quelque chose de différent.

1. Le balayage ne regardait que les symboles déclarés par les fichiers du paquet, via
   `load-history`. Un cache créé par `setq` sans `defvar` n'y entre pas — et c'est la façon négligée
   d'ajouter un cache, donc exactement celle qu'il faut attraper. Le balayage réunit désormais deux
   critères : l'emplacement, qui attrape ce que le paquet déclare quel que soit son nom, et le
   préfixe, qui attrape ce qu'un `setq` fabrique.
2. Le test ne cherchait que des **chaînes**. Or `auth-source` rend le credential sous forme d'une
   **fonction** d'accès : mettre l'accesseur en cache ne stocke aucun texte, et c'est pourtant
   garder le secret — il suffit de l'appeler. C'est même le cache le plus naturel à écrire, puisque
   c'est la valeur que `auth-source` pose sous la main. Le test appelle donc les fonctions qu'il
   croise.
3. Trois mutations le vérifient maintenant séparément : l'accesseur caché par `setq`, l'accesseur
   caché par `defvar`, et le secret résolu caché.

**La même leçon que W8.a, sous un autre jour.** Là, le critère d'appartenance au paquet devait être
l'emplacement plutôt qu'une liste de noms. Ici, c'est l'inverse qui manquait : l'emplacement seul ne
voit pas ce qui n'est pas déclaré. Aucun des deux critères ne suffit ; c'est leur réunion qui tient.

**Douze mutations vérifiées rouges** : l'identité absente devenue un plantage ordinaire (2 tests) ;
le message d'erreur cessant d'être actionnable (1) ; l'expurgation débranchée (2) ; l'expurgation
réduite à `Authorization` (1) ; l'expurgation devenue sensible à la casse (2) ; l'origine réduite à
l'hôte (1) ; le port implicite non appliqué (1) ; le changement d'origine non refusé (2) ; le
principal rendant le secret (1) ; l'accesseur mis en cache par `setq` (1) puis par `defvar` (1) ; le
secret résolu mis en cache (1). Restauration confirmée verte.

**Écart avec la spec.** Un, nommé. Le **transport** n'est pas dans ce sprint : `locus-auth` produit
une requête autorisée, personne ne l'envoie encore. C'est l'ordre « ports avant drivers » de l'ADR
0012 appliqué au client — et cela garde ce sprint testable sans serveur, donc sans dépendance à une
machine. Le HTTP et le stream arrivent avec W8.c, qui a des événements à recevoir. §6.4 (les
commandes d'administration masquées faute de scope) attend le transient de W8.e : masquer une
commande qui n'existe pas serait une sémantique inerte.

**Prochain item.** **W8.c** — événements et curseurs (§12) : une déconnexion ne perd ni ne duplique
un événement.

---

## 2026-08-18 — W8.c — Le flux : une déconnexion ne perd ni ne duplique

**Périmètre.** `apps/emacs/locus-events.el` et `test/locus-events-test.el`, neufs. `docs/10` corrigé
: la ligne W8.c renvoyait à `SPEC.md` §12, qui est le **dialogue avec l'orchestrateur** ; les
événements temps réel sont en **§14.1**, la reconnexion en **§7.5**. Aucune dépendance nouvelle.

**Les deux moitiés se testent ensemble ou pas du tout.** Un flux qui refuse tout ne duplique rien,
et un flux qui accepte tout ne perd rien : chaque propriété prise seule s'obtient par un défaut. Le
scénario central coupe après le cinquième événement, reprend avec le chevauchement que le serveur
rejoue, et vérifie les deux sur la **même** trace.

**Le curseur suffit pour dédupliquer, et c'est pour cela qu'il est employé.** Un rang déjà passé est
déjà traité : exact, et à mémoire constante. Une fenêtre d'identifiants récents serait plus souple
et introduirait une limite au-delà de laquelle un doublon redeviendrait neuf — un défaut qui ne se
manifeste que sur les longues coupures, c'est-à-dire les seules où il compte. Un test envoie mille
événements puis rejoue le premier.

**Ce que le curseur ne dit pas, c'est ce qui manque.** Recevoir le rang 8 quand on en était à 5
n'est pas une erreur de transport — le serveur peut avoir élagué, la reprise peut être partielle —
mais le taire ferait passer un historique troué pour un historique complet, et c'est la seule faute
de ce module qu'on ne pourrait plus détecter après coup. Une reconnexion ne comble donc pas les
trous : elle permet de les demander.

**Le chevauchement est un doublon, pas une erreur.** Le serveur rejoue depuis le curseur demandé et
le recouvrement est **voulu** : c'est lui qui garantit qu'aucun rang n'a été sauté. `accept` rend
donc trois valeurs distinctes — confondre `duplicate` et `malformed` ferait journaliser une avarie à
chaque reconnexion réussie, et on cesserait de lire ces journaux.

**L'élagage épargne les critiques, quitte à dépasser la taille nominale.** Un tampon borné qui
élague le plus ancien perd les alertes en premier quand le flux s'emballe — exactement quand elles
arrivent. Dépasser une limite d'affichage est un désagrément ; perdre une alerte de sécurité n'en
est pas un. La liste des sortes critiques vient des notifications par défaut de §14.2, elle n'est
pas inventée.

**Le jitter est un port.** Aléatoire en production, fixé en test : une suite qui dépendrait de
`random` ne serait pas rejouable, et un test de backoff qui échoue une fois sur vingt finit par être
ignoré. Trois propriétés le tiennent : croissance exponentielle, plafond, et jamais zéro — un
backoff qui peut rendre zéro ferait revenir toute la flotte à la même seconde, reproduisant la panne
qu'on attendait.

**Une erreur d'API attrapée au premier essai.** `should` d'ERT prend **une** forme, pas une forme et
un message : j'avais écrit du `assert`-style, et la macro-expansion a échoué au chargement. Les
messages sont passés en `ert-info`, qui est la façon de les attacher sans mentir sur l'arité.

**Quatorze mutations vérifiées rouges** : la déduplication débranchée (3 tests) ; le curseur cessant
d'avancer (6) ; les trous non marqués (2) ; le marquage décalé d'un rang (2) ; le premier événement
compté comme un trou (2) ; la reprise repartant d'un rang trop loin (3) ; la coupure non marquée (1)
; la reconnexion comblant les trous en silence (1) ; l'élagage emportant les critiques (1) ;
l'élagage débranché (2) ; le backoff non plafonné (2) ; le jitter pouvant rendre zéro (1) ; le
backoff cessant d'être exponentiel (2) ; la liste des sortes critiques tronquée (1). Restauration
confirmée verte, aucune muette.

**Écart avec la spec.** Deux, nommés. Le **transport** n'est toujours pas là : ce module est le pli
d'un flux, pas sa source — il est donc testable sans serveur, ce qui est aussi ce qui le rend
rejouable. Et `batch le rendu` de §14.1 n'est pas modélisé : il n'y a pas encore de rendu à grouper,
et un tampon de batch sans afficheur serait une sémantique inerte. Il arrive avec W8.d.

**Prochain item.** **W8.d** — dashboard et buffers (§9) : un buffer se reconstruit depuis le cache
sans réseau.

---

## 2026-08-18 — W8.d — Le tableau de bord : ce qui manque au cache manque à l'écran, et s'y voit

**Périmètre.** `apps/emacs/locus-cache.el`, `locus-dashboard.el` et `test/locus-dashboard-test.el`,
neufs. Aucune dépendance nouvelle : `tabulated-list` est dans Emacs. 49 tests ERT sur le paquet.

**« Sans réseau » se vérifie en empoisonnant, pas en débranchant.** Le test entoure le rendu de
`url-retrieve`, `url-retrieve-synchronously`, `make-network-process` et `open-network-stream`
remplacés par des erreurs. Un rendu qui parlerait à quiconque **échoue** au lieu de réussir plus
lentement — c'est la différence entre une propriété et une habitude. Et le poison lui-même est
éprouvé par un test dédié : sans lui, `--offline` pourrait n'empoisonner rien du tout et le test de
sortie passerait pour la mauvaise raison, ce que W7.f et W7.g ont chacune produit une fois.

**La propriété est plus forte qu'un confort hors ligne.** Un tableau de bord qui interroge le
serveur est aussi disponible que le réseau, alors qu'il sert précisément à savoir ce qui se passe
**quand quelque chose ne va pas**. §14.1 le dit de l'autre côté — « n'effectue pas une query
complète à chaque événement ».

**« Ne sert pas de source canonique » est une contrainte de type.** §21.3 le demande, et une note en
documentation ne l'obtient pas : dès qu'une lecture rend la valeur nue, l'appelant suivant la traite
comme un fait. `locus-cache-get` rend donc une **entrée** — valeur, instant, curseur — et aucun
accesseur ne rend la valeur seule. Lire le cache oblige à voir de quand date ce qu'on lit.

**Les secrets sont refusés, pas détectés.** Une heuristique qui fouillerait les valeurs raterait le
premier format qu'elle ne connaît pas tout en donnant l'impression d'avoir vérifié. L'appelant
déclare `:sensitive`, le cache refuse. Le cache survit à l'arrêt d'Emacs et se copie avec le
répertoire : ce qui y entre est durable, et un secret durable est un secret perdu.

**Une entrée périmée est rendue, pas supprimée.** §22.1 autorise la lecture offline ; effacer au
premier dépassement de TTL priverait le mode offline de ce qu'il existe pour montrer. La péremption
prend en revanche le pas sur le statut rapporté à l'écran : afficher `active` sur une donnée vieille
d'un jour serait exact au moment de la lecture et faux à l'écran, ce qui est la seule des deux
choses que l'utilisateur voit.

**Ce que la mutation a trouvé.** L'en-tête doit annoncer la synchronisation la **plus ancienne**, et
aucun test ne donnait deux entrées d'âges différents : la mutation qui prend la plus récente passait
verte. Or l'en-tête résume la confiance qu'on peut accorder à l'écran entier, et cette confiance
vaut celle de sa donnée la plus vieille — annoncer la plus récente ferait passer pour frais un
tableau où une seule ligne l'est. Test écrit, mutation recomptée rouge.

**Treize mutations vérifiées rouges** : le refus des valeurs sensibles disparu (1 test) ; une entrée
périmée supprimée au lieu d'être rendue (3) ; la péremption jamais déclarée (3) ; la purge qui ne
purge pas (2) ; l'oubli qui n'oublie pas (1) ; l'âge toujours nul (1) ; une clé absente produisant
une ligne vide (2) ; la péremption cessant de primer sur le statut (1) ; l'en-tête taisant l'état
`stale` (1), puis le curseur (1) ; l'en-tête confondant « jamais synchronisé » et « synchronisé à
l'instant » (1) ; l'en-tête prenant la synchro la plus récente (1, après écriture du test manquant).
Restauration confirmée verte.

**Écart avec la spec.** Deux, nommés. §9 décrit **dix** tampons ; seul `*Locus Solus Dashboard*` de
§9.1 existe. Les neuf autres suivent le même moule — une projection du cache, un en-tête de
fraîcheur — et les écrire tous avant qu'un seul soit branché sur des données réelles produirait neuf
fois la même erreur si le moule est mauvais. Et « batch le rendu » de §14.1 attend toujours : le
rendu existe désormais, mais rien ne l'appelle encore en rafale.

**Prochain item.** **W8.e** — commandes et transient (§10, §11) : toute action mutante passe par
l'API avec `expected_revision`, et un conflit est rendu plutôt qu'écrasé.

---

## 2026-08-18 — CI — Les portes qui exigent un Emacs sortent du chemin des autres

**Périmètre.** `.github/workflows/ci.yml` seul. Aucun code produit touché.

**Une mesure, pas une intuition.** Depuis que la frontière 5 est vérifiable (W8.a), l'installation
d'Emacs a pris **quinze secondes, dix minutes, puis onze minutes** selon le runner. Tant qu'elle
était dans `check`, elle retardait huit portes qui n'en ont aucun besoin. Un verdict de format qui
arrive en trente secondes est un verdict qu'on lit ; le même onze minutes plus tard arrive après
qu'on est passé à autre chose.

Les deux portes qui exigent un Emacs — la garde de frontières et la suite ERT — vivent donc dans un
job `emacs` parallèle. La couverture est inchangée, et `--require-emacs` continue de leur interdire
de se sauter en silence. `npm run check` reste la chaîne complète en local : c'est la CI qui
parallélise, pas le contrat.

**Ce que le découpage a fait apparaître.** En listant les étapes de `check` pour les répartir, deux
portes de `npm run check` se sont révélées **absentes de la CI depuis toujours** : `check:schemas`
et `check:generated`. Elles n'étaient tenues qu'en local. `lep/1.0` est gelé — un exemple qui cesse
de valider ou un SDK qui diverge de son schéma sont exactement les régressions qu'on ne veut pas
apprendre en aval. Les deux étapes sont ajoutées.

C'est la leçon des gardes muettes, appliquée à la CI elle-même : une porte qu'on croit tenue et qui
ne tourne pas ressemble en tout point à une porte verte.

**Prochain item.** **W8.e** — commandes et transient (§10, §11).

---

## 2026-08-18 — W8.e — Les commandes mutantes : un conflit est rendu, pas écrasé

**Périmètre.** `apps/emacs/locus-command.el` et `test/locus-command-test.el`, neufs. Aucune
dépendance nouvelle. 64 tests ERT sur le paquet.

**Le refus qui porte le module.** §11.3 : « **ne jamais** resoumettre automatiquement avec la
nouvelle révision ». C'est la seule règle du fichier qui protège quelque chose d'irrattrapable —
resoumettre avec la révision courante applique la mutation à un état que l'utilisateur n'a pas vu,
et efface silencieusement le travail de quelqu'un d'autre. Le confort d'un retry automatique est
réel ; ce qu'il coûte ne se voit qu'après.

**La propriété ne se lit pas dans la valeur de retour.** Un module qui resoumettrait rendrait un
**succès**, ce qui a l'air bien. Le test compte donc les appels au transport : une resoumission
serait un second appel, et il n'y en a qu'un. C'est la même famille de vérification que le poison
réseau de W8.d — regarder ce qui a été fait plutôt que ce qui a été rendu.

**`expected_revision` refusée au constructeur, pas à l'envoi.** Une commande sans révision attendue
écrase par construction : elle réussit quel que soit l'état trouvé. Refuser à la construction
déplace la faute du moment de l'envoi, où elle est invisible, à celui de l'écriture, où elle est
évidente. La clé d'idempotence est exigée pour la même raison : sans elle, une réponse perdue ne
peut être retrouvée que par une nouvelle soumission, c'est-à-dire par un doublon possible.

**Rebaser est un geste, pas un effet de bord.** §11.3 propose « refresh, rebase ou nouvelle commande
» — à l'utilisateur. `locus-command-rebased` existe donc et s'appelle explicitement ; sa clé
d'idempotence change, sans quoi la nouvelle commande retrouverait le résultat de l'ancienne au lieu
de partir.

**La graduation de §11.2 est asymétrique exprès.** `sensitive` demande une confirmation, `critical`
demande une confirmation **et** une raison écrite. Exiger une raison partout ferait taper « ok »
quatre fois par heure, et la raison cesserait d'en être une. Une raison faite d'espaces est refusée
: un formulaire qu'on remplit de blancs est un formulaire qu'on contourne.

**La prévisualisation est une alist, pas un texte.** Ce qui s'affiche se teste : un formateur qui
oublierait un des neuf champs de §11.1 le ferait disparaître de la prévisualisation sans que rien ne
le dise.

**Douze mutations vérifiées rouges** : le conflit déclenchant une resoumission (1 test) ;
`expected_revision` devenue optionnelle (1) ; la clé d'idempotence devenue optionnelle (1) ; le type
et la cible non exigés (1) ; la confirmation non exigée (1) ; la raison non exigée (2) ; une raison
blanche acceptée (1) ; `critical` cessant de se distinguer de `sensitive` (2) ; toutes les sévérités
exigeant une confirmation (6) ; le résultat connu rejoué au lieu d'être retrouvé (1) ; la clé du
rebase inchangée (1) ; la prévisualisation perdant un champ (1). Restauration confirmée verte,
aucune muette.

**Écart avec la spec.** Deux, nommés. Le **transient** de §10 n'est pas là : c'est une interface,
elle suppose des commandes à offrir, et ce sprint fabrique les commandes. Le dispatcher viendra
quand il aura de quoi dispatcher — l'écrire d'abord produirait des entrées de menu qui n'appellent
rien. Et §11.5, l'édition structurée en tampon JSON/YAML, attend un éditeur : le module garantit
déjà que rien de sensible n'entre dans une commande, mais tant qu'aucun tampon n'ouvre un payload,
l'exclusion des secrets n'aurait rien à exclure.

**Prochain item.** **W8.f** — artefacts et inspecteur de sandbox.

---

## 2026-08-18 — W8.f — Les artefacts : ce qui est promu se voit, rien n'est exécuté

**Périmètre.** `apps/emacs/locus-artifact.el` et `test/locus-artifact-test.el`, neufs. `locus.el`
porte désormais `locus-error` ; `locus-auth.el` ne la définit plus. Un test ajouté à
`locus-separation-test.el`. 79 tests ERT sur le paquet.

**Ce qui est promu se voit, et ce qui ne l'est pas aussi.** Un artefact `staged` affiché comme
`promoted` fait citer un résultat qui n'a pas été validé — l'invariant 4 ne tient pas si l'écran
aplatit la différence. Six états, six badges distincts, et **aucun défaut rassurant** : un état
inconnu rend « ? inconnu » et non le badge le plus neutre. Rendre l'inconnu comme du connu est la
façon la plus discrète de faire citer un résultat non validé.

**Le plan d'ouverture est rendu, pas exécuté.** §21.2 exige qu'un fichier ne soit pas exécuté
automatiquement ; `locus-artifact-open-plan` rend une **décision** — ouvrir, en lecture seule, sans
exécution, avec ou sans quarantaine — ce qui rend le refus testable sans écrire un octet sur le
disque. La liste d'extensions douteuses est volontairement large : le coût d'une quarantaine indue
est une commande de plus, celui d'un faux négatif est une exécution sur la machine de l'utilisateur.
Elle n'est pas une garantie pour autant, et c'est pourquoi `:execute` vaut nil quelle que soit
l'extension.

**Le hash se confronte avant, pas après.** Ce qui prouve ne peut pas être ce qui est demandé : seul
le hash **déclaré avant** l'upload sert de preuve. W6.a tient cette règle côté serveur, le client la
tient dans le même sens, et le hasher est un port — dupliquer le vocabulaire de hachage de
`packages/domain` serait la duplication cross-repo que le `CLAUDE.md` interdit.

**Une propriété qui se vérifie sur le texte, pas sur le comportement.** §20A : « le package ne parle
jamais directement à Docker/Podman ». Un client qui contournerait le control plane ne se trahirait
pas à l'exécution — il n'appellerait le runtime que sur une machine qui en a un — mais il se trahit
par ce qui est écrit. Le test lit donc les sources du paquet, comme la frontière 4 du dépôt le fait
pour `locusd`.

**Le vrai défaut trouvé par ce sprint.** Deux tests échouaient alors que les erreurs étaient bien
signalées : `should-error` ne les attrapait pas. Cause : `locus-error` était définie dans
`locus-auth.el`, et `locus-artifact`, `locus-cache` et `locus-command` en héritent en ne requérant
que `locus`. Leurs conditions d'erreur ne contenaient donc **pas `error`** — elles échappaient à
tout `condition-case` ordinaire, c'est-à-dire aux gardes écrites pour les attraper.

Deux des trois modules avaient l'air corrects **parce que l'ordre alphabétique des fichiers de test
chargeait `locus-auth` en premier**. Une correction qui dépend de l'ordre de chargement n'en est pas
une. `locus-error` vit maintenant dans `locus.el`, que tout module requiert, et un test balaie
l'obarray : toute condition `locus-*` doit contenir `locus-error` **et** `error`. Chaque module a
ensuite été chargé seul pour le vérifier.

**Onze mutations vérifiées rouges** : promu et vérifié partageant un badge (1 test) ; un état
inconnu rendu comme promu (1) ; un état non servable ouvert quand même (1) ; tous les états devenus
servables (2) ; le plan autorisant l'exécution (2) ; le plan cessant d'être en lecture seule (1) ;
la quarantaine devenue sensible à la casse (2) ; la quarantaine ne signalant plus rien (1) ; le hash
non confronté (1) ; la liste des états divergeant de celle du serveur (2) ; la racine d'erreurs
perdant son parent `error` (**15**). Restauration confirmée verte.

**Écart avec la spec.** Un, nommé. Le tampon `*Locus Sandboxes*` de §20A n'existe pas : ses colonnes
sont des données que le serveur rend, et le rendre avant d'avoir un transport produirait un tableau
dont chaque ligne serait inventée. Ce que ce sprint prend de §20A est la seule chose qui ne dépende
pas du transport — l'interdiction de parler à un runtime — et elle est vérifiée.

**Prochain item.** **W8.g** — intégrations Org/Magit/Jupyter/xiiif : chaque intégration absente
dégrade sans casser le démarrage.

---

## 2026-08-18 — W8.g — Les intégrations : un mécanisme, pas six

**Périmètre.** `apps/emacs/locus-integration.el` et `test/locus-integration-test.el`, neufs. Aucune
dépendance nouvelle. 87 tests ERT sur le paquet.

**§4.3 est vérifiée une fois, sur le mécanisme.** Quatre règles — détectée, commandes ajoutées
seulement si disponible, erreur actionnable, rien de cassé au démarrage — pour chaque dépendance
optionnelle. Écrites six fois (Org, Magit, xiiif, Jupyter, `eat`, Denote), elles seraient tenues
cinq fois et demie, et c'est la sixième qu'on découvrirait sur la machine de quelqu'un qui n'a pas
installé le paquet.

**Détecter n'est pas charger.** La règle la moins évidente, et celle qui décide de la forme du
module. `(require 'magit nil t)` détecte parfaitement — et charge Magit. Le démarrage paierait alors
toutes les dépendances optionnelles du cockpit, ce que §7.1 interdit et ce qui ferait de «
facultatif » un synonyme de « chargé quand même ». La détection emploie donc `featurep` et
`locate-library`, qui n'évaluent rien.

**Une collision de noms supprimée plutôt que contournée.** `cl-defstruct` engendre
`locus-integration-commands` pour un champ nommé `commands` ; ma première version gardait ce nom
pour la fonction publique et lisait le champ par `aref` derrière deux alias. Le champ s'appelle
`provides`, et les deux noms disent maintenant deux choses distinctes : ce que l'intégration
apporterait, et ce qu'elle apporte ici et maintenant.

**Ce que les mutations ont trouvé, et c'est la même maladie qu'en W8.f.** Trois mutations sont
passées vertes, dont les deux qui portent la règle centrale. La sonde des tests était `cl-extra` —
présente, et **chargée par ailleurs dans la suite**. Un `require` n'y changeait donc rien de
visible, et les tests passaient quoi qu'il arrive.

La sonde est maintenant `hexl`, que rien dans le cockpit ni dans la suite n'entraîne, et une
prémisse est **affirmée** avant chaque emploi : si la sonde venait à être chargée, les tests
échoueraient bruyamment au lieu de devenir vides. C'est la leçon de W8.f sous un autre jour — là,
une correction dépendait de l'ordre de chargement ; ici, c'étaient deux tests.

**Neuf mutations vérifiées rouges** : la détection chargeant la dépendance (1 test) ; la déclaration
la chargeant (2) ; tout déclaré disponible (4) ; les commandes offertes même absentes (2) ; les
commandes jamais offertes (1) ; l'erreur ne disant plus quoi installer (1) ; l'erreur nommant la
`feature` au lieu du paquet (1) ; une intégration inconnue confondue avec une absente (1) ;
l'absence cessant d'être une erreur (2). Restauration confirmée verte.

**Écart avec la spec.** Un, nommé, et il est large. §15 à §20 décrivent le **contenu** de six
intégrations — captures Org, ouverture Magit, régions xiiif, cellules Jupyter, terminal `eat`, notes
Denote. Ce sprint livre le mécanisme qui les rendra facultatives, pas les intégrations elles-mêmes :
chacune suppose un transport pour aller chercher ce qu'elle affiche. Les écrire maintenant
produirait six façades qui n'ouvrent rien, et le mécanisme aurait été vérifié sur des coquilles
plutôt que sur ce qu'il doit porter.

**Prochain item.** **W8.h** — 3D et WebView : la 3D reste une projection, aucune vue n'écrit dans le
graphe.

---

## 2026-08-18 — W8.h — Les vues : abîmer la projection ne touche pas le graphe

**Périmètre.** `apps/emacs/locus-view.el` et `test/locus-view-test.el`, neufs. Aucune dépendance
nouvelle. 96 tests ERT sur le paquet.

**La propriété ne se vérifie pas en cherchant une fonction d'écriture.** Il n'y en a pas — et c'est
précisément le problème. Une projection qui partagerait ses structures avec la source ferait du
premier `setcdr` d'une vue une écriture dans le graphe, **sans qu'aucune ligne de code ne s'appelle
« écrire »**, donc sans rien à voir dans un diff. Le test abîme donc la projection de toutes les
façons possibles — `setcdr` sur un nœud, `setcar` sur la liste, `setcdr` sur une arête — et compare
le graphe à ce qu'il était.

Le détachement vaut dans les deux sens, et le second sens compte autant : une vue affichée doit
continuer de montrer ce qu'elle montrait, sans quoi elle changerait sous les yeux du lecteur au
premier événement reçu.

**Une action de vue est une description, pas une fermeture.** Une fermeture pourrait faire n'importe
quoi, et le contrôle de §11 — `expected_revision`, confirmation graduée, conflit rendu — serait
contourné par le chemin le plus court. `locus-view-project` refuse toute action qui est une
fonction.

**La troncature se dit.** §13.2 limite le nombre de nœuds ; une vue tronquée **sans le dire** se lit
comme un graphe complet, et c'est la conclusion qu'on en tire qui est fausse, pas l'affichage. Le
total reste rendu, sans quoi « sept de plus » ne se situe pas. Et une arête vers un nœud écarté ne
survit pas : elle dessinerait un lien vers rien, indistinguable d'une relation cassée.

**Le handoff est une charge, pas un canal.** Ce qui part vers un viewer 3D ou une WebView est ce qui
s'affiche, et rien n'est prévu pour revenir. La charge ne porte aucune fonction — un viewer qui ne
peut recevoir qu'un document ne peut pas recevoir de pouvoir — ni aucune action : les proposer
inviterait à les déclencher. Elle porte en revanche la troncature, sinon celle-ci serait dite dans
Emacs et tue dans la fenêtre, c'est-à-dire tue là où on regarde.

**Dix mutations vérifiées rouges** : la projection partageant ses structures (3 tests) ; le
détachement réduit à une copie de surface (3) ; les fermetures acceptées comme actions (1) ; la
troncature non dite (2) ; le total non rendu (2) ; la limite de nœuds disparue (3) ; les arêtes vers
un nœud écarté survivant (1) ; le handoff portant les actions (1), taisant la troncature (1),
cessant de se déclarer en lecture seule (1). Restauration confirmée verte, aucune muette.

**Écart avec la spec.** Un, nommé. §13.2 demande aussi zoom/pan, couleurs par faces, légende,
filtres et accessibilité sans souris — ce sont des propriétés d'un **rendu**, et ce sprint livre ce
qui est rendu. Le module tient la contrainte qui survit à tous les rendus : la projection est
détachée et sans pouvoir. Les sept vues spécialisées de §13.3 sont sept requêtes différentes sur la
même projection ; elles arrivent avec le transport.

---

## W8 est terminé

Les huit items sont faits. `apps/emacs` compte **neuf fichiers Elisp** et **96 tests ERT**, et la
frontière 5 — le paquet démarre sous `emacs -Q` avec sa seule `load-path` — est passée de « sans
objet » à vérifiée en CI, sur chacun d'eux.

**Ce que W8 a appris, et qui n'était pas dans le plan.** Cinq fois sur huit, le sprint a trouvé un
défaut ailleurs que là où il regardait :

- **W8.a** — la garde de frontières avait un angle mort qui couvrait le cas nominal de ce qu'elle
  interdit : un `load` par chemin absolu ne touche pas `load-path`.
- **CI** — deux des dix portes n'avaient **jamais** tourné en CI. Une porte qu'on croit tenue et qui
  ne tourne pas ressemble en tout point à une porte verte.
- **W8.b** — une mutation passée verte deux fois : le balayage ne voyait pas les variables créées
  par `setq`, puis le test ne cherchait que des chaînes alors qu'`auth-source` rend une **fonction**
  d'accès.
- **W8.f** — trois modules avaient une hiérarchie d'erreurs cassée, et deux avaient l'air corrects
  **parce que l'ordre alphabétique des fichiers de test** chargeait le bon module en premier.
- **W8.g** — deux tests reposaient sur une sonde que la suite chargeait par ailleurs, donc passaient
  quoi qu'il arrive.

Trois de ces cinq sont la même faute sous trois formes : **un test qui dépend de ce que le reste de
la suite a fait**. C'est la dette que W8 laisse identifiée et corrigée là où elle a été vue, et le
réflexe qu'elle installe pour la suite — affirmer la prémisse plutôt que la supposer.

**Le transport reste dû.** Sept des huit items l'ont déclaré en écart : le paquet sait construire
des requêtes autorisées, plier un flux, projeter un graphe, cribler une commande — et n'a encore
parlé à personne. C'est un choix tenu sprint après sprint, et il a payé : chaque module est testable
sans serveur, donc rejouable, donc muté. C'est aussi la dette la plus visible du paquet.

**Prochain item.** Le premier item non terminé de `docs/10` dont les dépendances sont satisfaites,
hors W8.

---

## 2026-08-18 — W8.i — Le transport : construire, relire, et une seule socket

**Périmètre.** `apps/emacs/locus-http.el` et `test/locus-http-test.el`, neufs. `docs/10` : la ligne
W8.b portait « client HTTP/stream **et** authentification abstraite » ; seule l'authentification
avait été livrée, et les sept items suivants ont chacun déclaré le transport en écart. La ligne est
recadrée sur ce qu'elle a livré, et W8.i porte ce qui restait dû. 114 tests ERT sur le paquet.

**Trois responsabilités, séparées exprès.** Construire une requête, relire une réponse, parler à une
socket. Les deux premières sont **pures** — et c'est là que vivent les fautes : en-tête mal formé,
corps mal cadré, statut mal interprété. Elles s'éprouvent donc au cas par cas, sans serveur. Un
client qui mélangerait les trois se testerait à travers une socket : lentement, par intermittence,
et jamais sur le cas rare.

**Mais la socket est éprouvée aussi.** Deux tests montent un vrai serveur sur `localhost` et font un
vrai aller-retour, dans les deux sens : la réponse est relue, et le serveur reçoit bien ce qui a été
construit. Sans eux, le module serait vérifié partout sauf là où il touche le monde — et une requête
bien construite mal envoyée est indistinguable, côté client, d'une requête mal construite.

**L'erreur structurée n'est pas un code.** `packages/protocol` fait de l'erreur une enveloppe —
catégorie, code, politique de reprise. Rendre « 409 » jetterait tout cela pour garder le seul
chiffre. Et la reprise se lit **dans l'enveloppe**, jamais déduite du statut : un 409 de conflit de
révision ne se réessaie jamais (§11.3), un 409 de verrou temporaire se réessaie, et le chiffre ne
les distingue pas. Un serveur qui rend une erreur nue ne dit rien de la reprise — la supposer
possible ferait boucler sur une faute définitive.

**Deux fautes que seule la sérialisation révèle.** `json-serialize` rend le mot-clé `:a` comme
`":a"` — **avec le deux-points** — c'est-à-dire un champ que le serveur ne reconnaîtra jamais, et
l'échec apparaîtrait comme un 400 énigmatique loin d'ici. Les clés mot-clé sont donc refusées plutôt
que converties : convertir supposerait une correspondance entre les mots-clés d'Elisp et les noms du
fil, et cette correspondance serait une seconde définition du protocole — celle qui dérive.

Et `Content-Length` se compte en **octets**, pas en caractères. Une longueur fausse fait attendre le
serveur ou tronque le corps ; c'est la faute qu'un accent révèle et qu'une suite en ASCII rate. Le
test emploie « évaluation » exprès.

**Ce que la suite a cassé chez les autres.** Les serveurs de test laissaient leurs **connexions
acceptées** derrière eux, et trois tests du paquet affirment qu'aucun processus ne tourne : ils ont
échoué au premier essai. Le nettoyage porte désormais sur la descendance, pas seulement sur le
serveur. Une suite qui salit l'état global fait échouer les tests des autres, et c'est le genre de
rouge qu'on impute d'abord au mauvais endroit.

**Douze mutations vérifiées rouges** : la longueur comptée en caractères (1 test) ; les clés mot-clé
passant en silence (1) ; la clé d'idempotence ne partant plus (2) ; un GET annonçant une longueur de
zéro (6) ; la ligne de statut non vérifiée (1) ; un corps illisible devenu une panne de transport
(1) ; les en-têtes gardant la casse du serveur (1) ; l'enveloppe d'erreur jetée (2) ; l'enveloppe
lue même sur un succès (1) ; la reprise déduite du statut (1) ; une erreur sans enveloppe supposée
réessayable (1) ; le transport ajoutant l'autorisation (1). Restauration confirmée verte, aucune
muette.

**Écart avec la spec.** Un, nommé. Le **stream** de §14 n'est pas là : ce sprint livre la requête et
la réponse, pas la connexion longue. Le pli d'un flux existe depuis W8.c et se branchera dessus ;
les écrire ensemble aurait mêlé deux protocoles de cadrage dans un même fichier, et c'est le second
qu'on aurait mal fait.

**Prochain item.** Le premier item non terminé de `docs/10` dont les dépendances sont satisfaites.

---

## 2026-08-18 — W6.f — `RemoteArtifactRef` : le snapshot prouve, la source live constate

**Périmètre.** `schemas/artifacts/1.0/remote-artifact-ref.schema.json`,
`packages/artifacts/src/remote_ref.rs` et `tests/remote_ref.rs`, neufs ; trois fixtures et leur
entrée au registre ; le SDK régénéré. `docs/10` gagne W6.f, W10.7 et W10.8.

**Un trou de couverture, pas une dépendance en retard.** `docs/10` disait le reste de xiiif « bloqué
sur W0.6 » — or W0.6 est terminé depuis longtemps, et `RemoteArtifactRef` n'existait **nulle part**.
W0.6 a livré les schémas LEP ; ce type-ci est un contrat **entre** locusolus et xiiif, et aucun item
ne le portait. Même forme que le trou de §7.1 relevé en tête de roadmap : ce qui n'est assigné à
personne n'est pas en retard, il est invisible.

**Ce que §19 refuse de laisser confondre.** « Une ressource distante modifiée après le run ne doit
**jamais** faire croire que la preuve historique a changé. » Deux verdicts distincts, donc, et aucun
accesseur qui les résumerait : `proof_standing` parle de la preuve, `live_drift` n'en parle jamais.
Un « intégrité : divergente » unique laisserait croire que le résultat scientifique est en cause
quand c'est la source qui a bougé — et, dans l'autre sens, tairait la divergence d'une source qu'on
continuerait de citer. Le test les tient séparés sur les **quatre** combinaisons.

`Drift::Unknown` quand rien n'a été relevé au run : l'absence de relevé n'est pas l'absence de
dérive, et répondre `Unchanged` ferait passer une ignorance pour un constat.

**Un seul locator, et le type le rend indéfaisable.** §19 en nomme cinq et n'en autorise qu'un. Une
énumération ne porte qu'une variante, là où une structure à cinq champs facultatifs en accepterait
deux — et laisserait au viewer le soin de choisir, donc de choisir différemment d'une fois sur
l'autre.

**Le motif de W6.b, répété là où il fallait.** Le schéma porte `maxProperties: 1` ; Rust ne sait pas
l'exprimer, donc le type engendré offre **cinq champs facultatifs**. Un document à deux locators le
traverse sans bruit, et l'exclusivité ne serait tenue que par le validateur JSON — c'est-à-dire
nulle part, dès qu'un producteur construit la valeur en mémoire. `from_wire` est donc le lecteur
validant qui refuse zéro locator comme il en refuse deux. Sans lui, le test de sortie de cet item
aurait été tenu par le schéma seul, et j'aurais reproduit exactement la faute que W6.b avait
corrigée pour le manifeste.

**Le schéma vit sous `artifacts/1.0`, pas sous `lep/1.0`.** Ce n'est pas un message du protocole
d'exécution, c'est une référence qu'un viewer reçoit et relit. `lep/1.0` est gelé, et l'y ajouter
aurait mêlé deux cycles de vie qui n'ont aucune raison d'avancer ensemble.

**Dix mutations vérifiées rouges** : la preuve suivant la ressource live (3 tests) ; une dérive
cassant la preuve (2) ; l'absence de relevé devenue absence de dérive (1) ; la dérive ne se
constatant plus (2) ; deux locators passant (1) ; zéro locator passant (1) ; l'instantané local
demandant le réseau (1) ; un media type malformé passant (1) ; une identité vide passant (1) ; un
locator vide passant (1). Restauration confirmée verte, aucune muette.

**Écart avec la spec.** Un, nommé. `to_wire` n'existe pas : rien ne produit encore de
`RemoteArtifactRef` côté locusolus — c'est l'ingestion d'artefacts qui le fera, quand un artefact
portera un `viewer_hint: iiif`. Écrire la traduction sortante avant son producteur donnerait une
fonction que seuls ses tests appellent.

**Prochain item.** **W10.7** — xiiif consomme `RemoteArtifactRef` : `xiiif-open-locus-artifact` et
l'affichage séparé des cinq facettes de §19. Le dépôt est débloqué.

## 2026-08-18 — W7.h — `HumanReviewFinding` : un verdict humain ne vaut jamais une validation

Item ouvert en cours de route, comme W6.f et pour la même raison. W10.8 (xiiif) demandait « un
finding attachable à un `ReviewDossier` » et aucun item ne portait ce contrat : `packages/review`
avait le dossier, la revue et les findings de §17, mais rien qui dise ce qu'un humain enregistre
depuis une visionneuse. Trou de couverture, pas dépendance en retard.

**Périmètre.** `packages/review/src/human.rs` (neuf), `packages/review/tests/human.rs` (neuf, 20
tests), `packages/review/src/lib.rs`, `packages/review/Cargo.toml`,
`schemas/review/1.0/human-review-finding.schema.json` (neuf, et une famille de schémas neuve), trois
exemples, `schemas/registry.json`, les deux SDK régénérés, `docs/10_V1_ROADMAP.md`.

**Tests exécutés.** `cargo test -p locus-review --test human` → 20 conformes. `npm run check` → les
dix portes vertes. Mutation : treize mutants sur les gardes, **treize tués, aucun survivant**.

**Décisions prises.**

_Une seule porte fermée._ §20 demande deux choses opposées : que le finding soit réel — il
s'attache, il se compte, il ne se perd pas (invariant 12) — et qu'il ne puisse jamais tenir lieu de
validation. Fermer une seule porte suffit : **aucun verdict humain ne rend `Supports`**. `accept`
rend `Insufficient`, parce qu'un relecteur sans objection n'est pas une preuve ; c'est le même geste
que `Drift::Unknown` et `Verdict::Insufficient` avant lui, et le test le vérifie sur les quatre
verdicts croisés avec commentaire et preuve citée, plus le commentaire seul.

_`source-changed` ne réfute rien._ C'est §19 vu par un humain. Le rendre `Refutes` ferait douter
d'un run correct chaque fois qu'une bibliothèque remanie son site — exactement ce que W10.7 vient
d'interdire côté viewer. Il rend `NotApplicable` : le relecteur répond à une autre question que
celle du dossier, et il faut que cela se voie. Les deux dépôts tiennent donc la même règle, chacun
dans son vocabulaire, sans partager une ligne.

_La règle d'opposabilité ne connaît pas la qualité du relecteur._ §17.5 dit qu'un finding sans
preuve concrète est un commentaire non bloquant ; elle vaut pour un humain comme pour un agent.
`wrong-target` sans preuve citée ne bloque donc pas, et un commentaire libre avec preuve citée ne
bloque pas davantage. Deux tests, un par sens : n'en écrire qu'un laisserait passer une garde qui a
perdu l'autre moitié.

_Le dossier ne s'élargit pas en silence._ `attach_to` refuse une cible absente du dossier et un
dossier qui n'est pas celui que la revue nomme. Sans ce refus, une revue humaine ajouterait des
findings sur des révisions qu'un dossier figé avant attribution (§17.3) ne couvre pas — la forme de
dérive qui ne contredit jamais rien ouvertement.

_Lecteur validant, troisième occurrence._ Le schéma porte `anyOf` sur verdict/commentaire et une
énumération de quatre valeurs ; Rust n'exprime ni l'un ni l'autre, donc le type engendré offre deux
`Option<String>` indépendants. `from_wire` ajoute les deux refus, et celui qui compte le plus est
`UnknownVerdict` : `validated` est précisément le mot que §20 interdit, et le laisser entrer comme
une chaîne libre le ferait figurer au dossier sous un nom que personne n'a défini.

_Une inconsistance de W6.f corrigée au passage._ `remote-artifact-ref.schema.json` portait un `$id`
en URL là où les quatorze autres schémas portent une URN. Laisser deux précédents en place, c'est
laisser le prochain auteur choisir — donc choisir autrement.

**Écart avec la spec.** §20 nomme quatre autres exigences de revue — juxtaposer original et dérivé,
superposer la région revendiquée, afficher OCR source/correction, ouvrir le rapport interprétatif
sans l'injecter dans le rendu. Ce sont des exigences d'affichage : elles appartiennent à xiiif et
sont le contenu de W10.8. Ce qui est livré ici est le contrat que xiiif écrira, pas l'écran.

**Prochain item.** W10.8 — xiiif produit ce finding : les quatre verdicts et le commentaire libre
enregistrés depuis la visionneuse, sans importer une ligne de Locus, avec sa propre implémentation
des refus.

## 2026-08-18 — W9.a — La vue de visualisation : versionnée, hashée, et jamais le graphe

W9 était en prose et ne portait aucun item. Décomposée en quatre lignes (W9.a–d) avant d'écrire : ce
qui n'a pas de test de sortie n'a pas d'état, et une section « à faire » sans items se re-découvre
indéfiniment.

**Périmètre.** `packages/visualization/` (neuf : `Cargo.toml`, `src/lib.rs`, `tests/view.rs` avec 14
tests), `Cargo.toml` du workspace, `docs/10_V1_ROADMAP.md` (décomposition de W9), ce fichier.

**Tests exécutés.** `cargo test -p locus-visualization` → 14 conformes. `npm run check` → les dix
portes vertes. Mutation : quatorze mutants, **quatorze tués, aucun survivant**.

**Décisions prises.**

_« Jamais une copie mutable du graphe » se teste comme une identité, pas comme une interdiction._
Aucun test ne peut vérifier qu'une méthode n'existe pas. En revanche, une vue dont on a changé un
nœud n'a plus la même forme canonique, donc plus le même condensat, donc elle ne peut plus être
présentée comme la projection dont elle vient. Un frontend peut travailler sur ce qu'il a reçu ; il
ne peut pas faire passer le résultat pour la source. C'est la formulation qui survit à une
réécriture.

_L'ordre d'insertion est un accident du producteur._ Un rebuild complet et un rattrapage incrémental
ne remplissent pas leurs vecteurs de la même façon. S'il entrait dans la forme canonique, deux
viewers montrant la même chose ne pourraient pas le prouver — d'où le tri, et d'où un test qui
construit la même vue dans deux ordres.

_Le condensat est un port, pas une dépendance._ Le crate ne hache rien : il produit la forme
canonique et confie le reste à `Digest`. Le jour où l'algorithme change, rien ici ne bouge. La
fixture de test ne hache pas non plus — elle enregistre ce qu'on lui donne, ce qui permet de
vérifier que le port voit **la forme canonique et rien d'autre**, là où un vrai algorithme
n'ajouterait qu'une couche opaque entre le test et ce qu'il vérifie.

_La forme canonique est lisible._ Une vue qui se prétend hashée doit pouvoir dire de **quoi** : deux
implémentations dont les condensats diffèrent ne peuvent, sans cela, que constater leur désaccord.
Elle porte aussi sa version (`view/1`), le genre et le watermark — deux vues de genres différents
sur la même matière seraient sinon interchangeables dans un cache.

_Trois états de fraîcheur, pas deux._ « En retard » et « à jour » ne couvrent pas le cas où le point
de comparaison est **antérieur** à la vue : le journal ne recule pas, donc c'est l'appelant qui
compare à un état périmé. Répondre `Current` ferait passer sa méprise pour un accord. `Inconsistent`
est produit par un test, pas réservé pour plus tard.

_Deux refus de forme._ Une arête vers un nœud absent est refusée — une visualisation rend
irrésistible l'inférence d'un objet qu'on voit au bout d'un trait — et dans les deux sens, une arête
qui _vient_ de nulle part se lisant aussi mal. Deux nœuds de même identité aussi : §23 demande des
IDs stables parce qu'une sélection doit désigner la même chose d'un rendu à l'autre, et l'ambiguïté
se résoudrait autrement selon le viewer.

**Écart avec la spec.** §23.3 nomme les huit projections ; ce commit livre le **type** de vue et sa
forme canonique, pas les producteurs qui les remplissent depuis le graphe. Chaque genre attend la
projection correspondante de §9.3, dont quatre existent (W1). Écrire les huit producteurs maintenant
produirait six vues qu'aucune donnée ne traverse.

**Prochain item.** W9.b — `ArtifactViewerRegistry` de §23.5 : l'artefact suggère, le client choisit,
et l'absence de tout viewer laisse l'artefact atteignable.

## 2026-08-18 — W9.b — `ArtifactViewerRegistry` : la capacité admet, la suggestion ordonne

**Périmètre.** `packages/visualization/src/registry.rs` (neuf),
`packages/visualization/tests/registry.rs` (neuf, 15 tests), `src/lib.rs` (les réexports), ce
fichier.

**Tests exécutés.** `cargo test -p locus-visualization` → 29 conformes (14 + 15). `npm run check` →
les dix portes vertes. Mutation : douze mutants, **douze tués** — après correction de deux trous
réels dans la suite (voir plus bas).

**Décisions prises.**

_Une seule règle tient §23.5 et l'invariant 10 : la capacité admet, la suggestion ordonne._ Un hint
ne peut que reclasser des viewers qui savent déjà rendre le media type ; il ne peut jamais en faire
entrer un qui ne le sait pas. Un artefact ne peut donc pas forcer l'ouverture de xiiif — le seul
pouvoir qu'il a est de trier une liste que le client a constituée. Si la suggestion pouvait
_admettre_, un producteur d'artefacts déciderait à distance de ce qui s'ouvre chez un lecteur, et «
le client choisit » serait faux.

_Choisir ne rend pas de `Result`._ Ce n'est pas une commodité de signature : un artefact qu'aucun
viewer ne sait rendre reste **atteignable** — on le télécharge, on l'ouvre ailleurs. Rendre une
erreur ferait d'une absence de confort une panne, et un appelant qui propage afficherait « échec »
là où il fallait afficher un lien. Le media type voyage avec le refus pour que l'appelant ait de
quoi proposer autre chose.

_Deux ordres, deux propriétaires._ L'ordre des hints appartient à l'artefact — c'est sa préférence.
L'ordre de déclaration appartient au client — c'est la sienne, et c'est la moitié de « le client
choisit » qui n'est pas dans les hints. Le premier départage entre suggestions, le second entre
viewers à suggestion égale.

_La table de `docs/07` est exécutable._ `ArtifactViewerRegistry::reference()` la porte, et un test
parcourt les dix familles. Une table de routage que rien n'exécute se désaccorde du code sans que
personne ne le voie.

**Deux trous réels trouvés par mutation.**

1. `candidates.first()` → `candidates.last()` a **survécu** : le test d'ordre de déclaration
   n'éprouvait que la branche où une suggestion est honorée, jamais celle du repli. Une préférence
   client inversée sur le chemin le plus fréquent passait inaperçue. Test ajouté.
2. Donner à Potree la capacité glTF a **survécu** aussi, parce que Three.js est déclaré avant lui :
   le premier candidat restait le bon. Un viewer qui s'étend sur le territoire d'un autre est donc
   invisible tant que l'autre est déclaré en premier — et le jour où l'ordre change, la destination
   change. Un test vérifie maintenant que chaque media type de la table de référence n'a **qu'un**
   viewer capable.

Le second est le plus instructif : ce n'était pas une garde manquante mais une propriété de la table
— l'exclusivité — que rien ne tenait. Une table de routage à recouvrements n'est pas une table de
routage, c'est un ordre de priorité déguisé.

**Écart avec la spec.** Les media types du registre de référence sont ceux que `docs/07` implique ;
trois d'entre eux (`application/vnd.locus.graph+json`, `application/vnd.laszip`,
`application/vnd.vtk`) sont des noms de convenance, faute de type enregistré. Ils sont ici pour que
le routage soit exécutable, pas pour figer un contrat de fil ; le jour où un artefact réel en porte
un autre, c'est le registre du client qui l'accueille, pas ce défaut.

**Prochain item.** W9.c — l'interaction de §23 : `focus`, `filter`, `select` vers le viewer,
`node_selected` en retour, et aucun chemin par lequel un événement de viewer devienne une mutation.

## 2026-08-18 — W9.c — L'interaction viewer : ce qui revient ne s'écrit pas

**Périmètre.** `packages/visualization/src/interaction.rs` (neuf),
`packages/visualization/tests/interaction.rs` (neuf, 13 tests), `src/lib.rs` (`derived_from` dans la
forme canonique, `render_derived`), ce fichier.

**Tests exécutés.** `cargo test -p locus-visualization` → 42 conformes (14 + 15 + 13).
`npm run check` → les dix portes vertes. Mutation : onze mutants, **onze tués** — après correction
d'un trou réel (voir plus bas).

**Décisions prises.**

_Un événement de viewer porte une identité, jamais un contenu._ C'est par le canal de retour que «
la vue devient éditable en place » reviendrait si on la chassait de la vue : non pas en modifiant la
projection, mais en laissant le viewer **dire** au control plane ce qu'un nœud vaut désormais. Un
`node_selected` qui porterait un label remplacerait une lecture par une écriture sans jamais toucher
au graphe. `ViewerEvent` n'a donc aucun champ où mettre autre chose, et un consommateur qui voudrait
faire écrire un viewer devrait d'abord changer le type — c'est-à-dire éditer le fichier qui explique
pourquoi il ne faut pas.

_Deux événements, pas « etc. »._ `docs/07` écrit « `node_selected`, `artifact_opened`, etc. ». L'«
etc. » est une invitation à inventer, et ADR 0016 dit qu'une sorte entre dans son énumération quand
un consommateur exécutable et testé existe. `node_edited` accepté aujourd'hui serait le canal
d'écriture que §23 interdit, ouvert par avance et sans que personne l'ait décidé — le test le refuse
nommément.

_Une vue dérivée déclare toujours son parent._ `focus` et `filter` produisent une autre vue, plus
petite. Le danger n'est pas qu'elle existe, c'est qu'on la prenne pour la projection : qui compte
les objections d'un claim dans une vue filtrée en compte moins, et rien à l'écran ne le lui dit. La
forme canonique porte donc le condensat du parent **sans exception**, y compris quand le filtre ne
retire rien — les exceptions sont exactement là où la confusion se loge.

_Ce qui sort du cadrage emporte ses arêtes._ En garder une ferait supposer un nœud absent, et un
trait qui mène hors de l'écran est l'invitation la plus forte qu'une visualisation puisse faire.
C'est aussi ce qui permet à `focused` de ne jamais produire le `DanglingEdge` que W9.a refuse.

_Cadrer sur un nœud absent rend une vue vide._ Élargir silencieusement à tout serait la pire des
réponses : elle aurait l'air de marcher.

_`select` désigne, il ne réduit pas._ Il n'existe aucune opération de vue qui lui corresponde : la
sélection vit dans le canal d'interaction. Confondre les deux ferait disparaître de l'écran ce qu'on
voulait seulement montrer du doigt.

**Un trou réel trouvé par mutation.** « Le cadrage ne suit que le sens des arêtes » a **survécu** :
la fixture était une chaîne `a → b → c`, donc suivre le sens sortant suffisait à tout trouver. Or
une objection pointe **vers** ce qu'elle conteste : un `focus` qui ne suivrait que le sens sortant
montrerait un claim débarrassé de tout ce qui lui est opposé — l'invariant 12 défait par un détail
de parcours, et sans que rien n'ait l'air cassé. La fixture porte maintenant une objection entrante,
et trois tests tombent quand le sens inverse disparaît.

**Écart avec la spec.** §23 dit « toute mutation passe ensuite par command API et confirmation
appropriée ». Ce crate ne contient ni l'API de commandes ni la confirmation : il tient sa moitié —
rien ne sort d'ici qui puisse être écrit. L'autre moitié est §22, et elle n'a pas encore d'item.

**Prochain item.** W9.d — `apps/web` sur la vue hashée, en TypeScript.

## 2026-08-18 — W9.d — `apps/web` : lire une vue, la vérifier, et ne rien détenir de modifiable

**Périmètre.** `apps/web/` (neuf : `package.json`, `src/{index,view,layout,store,commands}.ts`),
`tests/web/view.test.ts` (neuf, 16 tests), `schemas/visualization/1.0/view.schema.json` (neuf, et
une famille de schémas neuve), deux exemples, `schemas/registry.json`,
`packages/visualization/tests/{wire.rs,fixtures/argument-map.canonical.txt}` (neufs),
`tsconfig.json`, les deux SDK régénérés, `package-lock.json`.

**Tests exécutés.** `node --test tests/web` → 16 conformes. `cargo test -p locus-visualization` → 43
conformes. `npm run check` → les dix portes vertes. Mutation : treize mutants, **treize tués** —
après correction d'un trou réel et d'un mutant faux.

**Décisions prises.**

_Le document ne transporte pas sa preuve._ Le schéma porte le condensat, pas la forme canonique. Le
lecteur la **reconstruit** et compare. Transporter la forme canonique ferait de la preuve une donnée
reçue, et un document tronqué se relirait comme un graphe plus petit mais authentique — exactement
la perte qu'une visualisation rend invisible, puisqu'on ne voit pas ce qui n'est pas dessiné. Un
test tronque le document et vérifie le refus.

_Deux implémentations qui ne se consultent pas, et une fixture entre elles._ Rust construit la forme
canonique dans `packages/visualization`, TypeScript la reconstruit dans `apps/web`. Ni l'un ni
l'autre ne lit le code de son homologue : ils se rencontrent sur
`tests/fixtures/argument-map.canonical.txt`, que chacun compare depuis le même document. Si l'une
des deux formes change, un test tombe de chaque côté — jamais l'un sans l'autre en silence.

_Ce que le lecteur ne sait pas calculer, il ne le déclare pas vérifié._ Un condensat blake3 est
refusé plutôt que traversé. Dire « vérifié » de ce qu'on n'a pas vérifié serait la seule faute
vraiment grave de ce fichier.

_Le store n'applique jamais localement._ Côté web, la tentation n'est pas d'écrire dans le graphe :
c'est d'appliquer tout de suite ce qu'on vient de demander, pour que l'écran réponde. Un store qui
ferait cela afficherait, entre la demande et la réponse, un graphe que personne n'a validé — et si
la réponse n'arrivait jamais, il l'afficherait pour toujours. `dispatch` rend un nouveau store dont
la vue est la **même référence**, et seul `adopt` change ce qui est affiché.

_La disposition est déterministe._ Rien d'aléatoire ni d'horodaté : deux ouvertures du même document
donnent les mêmes positions. Sans cela, deux captures d'écran d'un même graphe ne se comparent pas,
et c'est en les comparant qu'on voit ce qui a bougé.

**Un vrai bug trouvé par un test, avant la mutation.** `readView` rendait les nœuds dans l'ordre du
**document**, alors que la forme canonique les trie. La disposition héritait donc de l'ordre
d'insertion d'un producteur : deux rendus du même contenu ne se superposaient pas. Corrigé en
extrayant l'ordre canonique dans une fonction que la lecture et le condensat partagent.

**Deux ratés du harnais.** `assert.throws` ne rend pas l'erreur attrapée —
`const e = assert.throws(...)` vaut `undefined`, et cinq tests validaient donc leur moitié
inintéressante. Remplacé par un helper qui attrape et rend la raison. Et un mutant sur le tri ne
mutait rien (`.sort(() => 0)` suivi du vrai tri) ; refait, il tue neuf tests. Un mutant qui ne mute
pas est un faux négatif aussi silencieux qu'un test qui n'assère rien — c'est la deuxième fois de la
journée.

**Un trou réel.** Le dédoublonnage des arêtes n'était testé que côté Rust. Une arête répétée aurait
compté deux fois côté web, et qui compte les soutiens d'un claim en aurait lu un de plus. Test
ajouté.

**Écart avec la spec.** §23.4 demande une scène 3D Three.js/WebGL et un pont xwidget. Rien de cela
ici : ce commit livre le **modèle** du workspace — lecture vérifiée, disposition déterministe, store
sans graphe modifiable — et pas le rendu. Une scène WebGL ne se teste pas dans ce harnais, et
l'écrire sans test la rendrait vraie par déclaration. Elle demande son propre item, avec son propre
moyen de vérification.

**Prochain item.** W9 est couvert pour ce qui est testable ici. La suite vient de W11 (profils de
déploiement) ou de la 3D de §23.4, qui demande d'abord de décider comment on la vérifie.

## 2026-08-18 — W11.a — Un profil ne se déclare pas exécutable, il est vérifié

W11 était en prose et ne portait aucun item, comme W9 avant elle. Décomposée en trois lignes
(W11.a–c) avant d'écrire.

**Périmètre.** `packages/deployment/` (neuf : `Cargo.toml`, `src/lib.rs`, `tests/doctor.rs` avec 13
tests), `Cargo.toml` du workspace, `docs/10_V1_ROADMAP.md`, ce fichier.

**Tests exécutés.** `cargo test -p locus-deployment` → 13 conformes. `npm run check` → les dix
portes vertes. Mutation : treize mutants, **treize tués, aucun survivant**.

**Décisions prises.**

_Le profil ne sait pas répondre « suis-je exécutable »._ §27.2 dit que `locus doctor` **vérifie** ;
`docs/05` ajoute « avant d'accepter des campagnes ». La faute que ces phrases préviennent est
courante et silencieuse : un fichier qui énumère des adaptateurs, personne qui vérifie qu'ils sont
là, et une campagne acceptée qui échoue trois heures plus tard sur le premier appel. Seul le
croisement d'un `Profile` et d'un `Inventory` produit un verdict — cinquième occurrence de « ce qui
prouve ne peut pas être ce qui est demandé », après l'attestation de sandbox, le digest de build, le
niveau de reproductibilité et l'attestation d'indépendance.

_Trois présences, pas deux._ Une sonde qui n'a pas pu répondre n'a rien constaté. Compter cette
ignorance comme un succès ferait déclarer un profil exécutable **par une panne de la sonde**,
c'est-à-dire au moment précis où il ne faut pas. Et un adaptateur dont l'inventaire ne parle pas du
tout est inconnu, jamais absent : ne pas avoir regardé et avoir regardé sans rien trouver appellent
deux gestes différents — sonder, ou installer. Les deux listes restent séparées jusqu'à l'impression
pour la même raison.

_Un « non exécutable » sans raison ne se corrige pas._ Le verdict nomme ce qui manque et ce qui n'a
pas été vérifié, séparément.

_Le client voit une URL, pas une topologie._ Deux profils aussi éloignés qu'un poste personnel et un
hybride distribué rendent la **même** valeur, à l'égalité près. Un test vérifie en plus qu'aucun nom
d'adaptateur ne filtre : « postgres-rds-interne » dirait déjà quel fournisseur est derrière, et
rendrait un client dépendant d'un détail que §27.3 lui promet de ne pas voir. Les capabilities, en
revanche, traversent : §27.1 demande que les limites soient déclarées plutôt que contournées.

_Un profil sans adaptateur est refusé._ Il passerait toute vérification sans rien avoir vérifié — la
façon la plus discrète de rendre `locus doctor` inutile, puisque la commande répondrait « exécutable
» et aurait raison.

**Écart avec la spec.** §27.2 nomme aussi `locus up` et `locus deployment explain`, et §27.4 la
sauvegarde cohérente. Rien de cela ici : ce commit livre le **verdict**, pas les commandes ni le
format `deployment.yaml`, qui sont W11.b et W11.c. Les sondes elles-mêmes n'existent pas non plus —
`Inventory` est ce qu'une sonde remplit, et écrire les sondes avant d'avoir un profil à vérifier
aurait produit des constats que rien ne lit.

**Prochain item.** W11.b — `deployment.yaml` : le schéma, les secrets dehors,
`locus deployment explain`.

## 2026-08-18 — W11.b — `deployment.yaml` : les secrets sont dehors, et il n'y a pas d'endroit où les mettre

**Périmètre.** `schemas/deployment/1.0/deployment.schema.json` (neuf, et une famille de schémas
neuve), deux exemples, `schemas/registry.json`, `packages/deployment/src/config.rs` (neuf),
`packages/deployment/tests/config.rs` (neuf, 12 tests), `Cargo.toml` du paquet, les deux SDK
régénérés, ce fichier.

**Tests exécutés.** `cargo test -p locus-deployment` → 25 conformes (13 + 12). `npm run check` → les
dix portes vertes. Mutation : douze mutants, **douze tués, aucun survivant**.

**Décisions prises.**

_« Les secrets sont externes » n'est pas une consigne d'hygiène, c'est une propriété du format._ Le
document n'offre **aucun champ** où écrire une valeur : `secret_refs` ne prend que des références,
le motif du schéma refuse ce qui n'en est pas une, et `additionalProperties: false` ferme la porte à
un `password` ajouté à la main. La raison est qu'un secret écrit dans un fichier de configuration ne
s'arrête pas là — il part dans un dépôt, dans une sauvegarde, dans un rapport de bug, dans le
presse-papier de qui diagnostique, et aucune de ces copies ne se révoque.

_`explain` dit où chercher, jamais ce que ça vaut._ Savoir qu'un déploiement lit son mot de passe
dans `env:LOCUS_PGPASSWORD` fait partie du diagnostic ; le résoudre « pour aider » le mettrait sur
le terminal de qui lance la commande, et dans le journal de session qui va avec. Ce module ne résout
donc rien, et un test vérifie que tout ce qui suit la flèche est une référence.

_Une liste d'adaptateurs, pas un objet._ Un objet JSON aurait laissé un rôle déclaré deux fois
écraser le premier **sans bruit**, et personne n'aurait su lequel des deux backends était actif — la
question exacte que `explain` existe pour trancher. La liste rend le doublon détectable, et le
domaine le refuse.

_Un schéma de secret inconnu est refusé, pas accepté en espérant._ `s3:bucket/key` ressemble à une
référence sans en être une ici ; l'accepter ferait échouer le déploiement au premier démarrage
plutôt qu'à la lecture, c'est-à-dire loin de la ligne fautive.

_`explain` nomme « exactement » les backends actifs_ (§27.2) : un test compare le nombre de lignes
au nombre d'adaptateurs. Un backend oublié à l'écran est un backend qu'on croit absent.

**Écart avec la spec.** Le format est décrit en JSON Schema et relu depuis JSON ; `docs/05` parle
d'un `deployment.yaml`. Aucun lecteur YAML n'est ajouté : YAML est une surface d'entrée, pas un
contrat, et le contrat est celui du schéma. Le jour où la CLI lit un fichier, elle convertira avant
de valider — et c'est le bon ordre.

**Prochain item.** W11.c — la sauvegarde cohérente de §27.4 et la restauration sur un backend
différent.

## 2026-08-18 — W11.c — Sauvegarde cohérente et restauration ailleurs

**Périmètre.** `packages/deployment/src/backup.rs` (neuf), `packages/deployment/tests/backup.rs`
(neuf, 11 tests), `src/lib.rs` (les réexports), ce fichier.

**Tests exécutés.** `cargo test -p locus-deployment` → 36 conformes (13 + 12 + 11). `npm run check`
→ les dix portes vertes. Mutation : dix mutants, **dix tués, aucun survivant**.

**Décisions prises.**

_« Cohérente » se calcule, jamais ne se déclare._ Il n'existe aucun champ qu'un producteur pourrait
cocher. Une sauvegarde qui se dirait complète le resterait jusqu'au jour où on la restaure —
c'est-à-dire le seul jour où c'est trop tard. Le test éprouve les cinq parties **séparément** :
chacune retirée seule suffit, ce qui empêche qu'une devienne facultative sans que personne ne s'en
aperçoive.

_Les clés sont à part, mais jamais silencieuses._ §27.4 les subordonne à une procédure plutôt qu'à
la liste des cinq. « Selon procédure » n'autorise pourtant pas le silence : une sauvegarde d'où les
clés sont absentes **sans qu'on sache pourquoi** est indiscernable d'une sauvegarde où on les a
oubliées, et les deux se restaurent pareil. `KeyHandling` force à nommer la procédure dans les deux
sens — incluses ou exclues sont deux décisions, pas une présence et une absence.

_Les sandboxes sont refusées nommément._ §27.4 les exclut ; le refus est explicite plutôt
qu'implicite, parce que quelqu'un essaiera de les inclure en croyant être exhaustif. Une sauvegarde
qui porte l'état d'une sandbox invite à la restaurer, donc à traiter du jetable comme une source.

_Restaurer ailleurs se déclare, ne se fait pas._ §27.5 pose la réserve : « sous réserve des
capabilities requises par ses runs historiques ». Restaurer sur un hôte qui n'a pas ce que les runs
exigeaient produirait une campagne qu'on croit intacte et qui ne se rejoue pas — l'écart ne se
verrait qu'à la première reproduction, des semaines plus tard.

_Troisième occurrence de la même distinction dans ce paquet._ Une sauvegarde qui n'a pas relevé ce
que ses runs exigeaient rend `RequirementsUnknown`, pas `Ready`. Après `Presence::Unknown` (W11.a)
et l'ignorance des sondes, c'est la même règle : personne n'a regardé n'est pas rien à signaler. Et
relever une liste **vide** est une réponse, elle — le test tient les deux cas côte à côte.

_L'incohérence se dit avant l'hôte._ Une sauvegarde à qui il manque l'event store ne se juge pas sur
le GPU de la machine cible : répondre « il manque un GPU » ferait chercher du matériel quand il
manque une base.

**Écart avec la spec.** Rien ici ne prend ni ne restaure quoi que ce soit : le module dit ce qu'une
sauvegarde **est** et à quelles conditions elle se restaure ailleurs. Les procédures elles-mêmes
appartiennent aux adaptateurs, qui n'existent pas encore. §27.5 demande aussi que chaque release
majeure soit testée sur macOS local et Linux VM — c'est une exigence de CI, pas de code, et elle n'a
pas d'item.

**Prochain item.** W11 est couvert. La suite vient de W12 (évaluation et release) ou de la 3D de
§23.4, qui demande d'abord de décider comment on la vérifie.

## 2026-08-18 — W12.a — Les épreuves closes de §29 : ce qui n'a pas été éprouvé se nomme

W12 était en prose et ne portait aucun item, comme W9 et W11 avant elle. Décomposée en trois lignes
(W12.a–c) avant d'écrire.

**Périmètre.** `packages/evaluation/` (neuf : `Cargo.toml`, `src/lib.rs`, `tests/registry.rs` avec
12 tests), `Cargo.toml` du workspace, `docs/10_V1_ROADMAP.md`, ce fichier.

**Tests exécutés.** `cargo test -p locus-evaluation` → 12 conformes. `npm run check` → les dix
portes vertes. Mutation : treize mutants, **treize tués, aucun survivant**.

**Décisions prises.**

_Les listes de §29 sont closes, et c'est ce qui les rend vérifiables._ Treize fautes (§29.4),
quatorze attaques (§29.5), huit ablations (§29.8). Une liste nommée permet de dire ce qui n'a
**pas** été éprouvé ; une intention générale — « on testera l'injection de fautes » — ne le permet
jamais. Une release qui part sans avoir éprouvé le disque plein n'est pas nécessairement une faute :
la faute est de ne pas le savoir.

_Écartée n'est pas oubliée, et c'est tout l'objet du module._ Les deux se ressemblent dans un
rapport — aucune épreuve n'a été menée — et ne se ressemblent pas du tout : l'une est une décision
qu'on peut contester, l'autre est un oubli que personne ne voit. Une renonciation sans raison est
donc refusée, et « éprouvé » sans dire par quoi l'est aussi : ce qui ne se vérifie pas ne vaut pas
mieux que ce qui n'a pas été fait.

_Un registre neuf porte les trente-cinq épreuves en « non traité »._ C'est ce qui empêche d'en
oublier une en omettant simplement de l'inscrire — l'oubli le plus facile de tous, et celui qu'un
registre à remplir soi-même encourage.

_Le verdict nomme la section, pas seulement l'épreuve._ Savoir qu'il manque `disk-full` sans savoir
que c'est de §29.4 oblige à chercher dans trois listes.

_Les renonciations se comptent._ Une release « prête » avec dix-sept renonciations n'est pas la même
qu'une release prête sans aucune, et le chiffre est ce qui pousse à les relire.

_Le test parcourt les trente-cinq une par une._ Chacune, laissée seule non traitée, doit bloquer et
être nommée. C'est le même geste que pour les cinq parties d'une sauvegarde (W11.c) : éprouver le
groupe entier laisserait une épreuve devenir facultative sans que personne ne s'en aperçoive.

**Écart avec la spec.** Ce module ne mène aucune épreuve : il dit lesquelles §29 exige et refuse de
déclarer une release prête tant qu'une reste sans réponse. Les épreuves elles-mêmes appartiennent
aux paquets qu'elles éprouvent, et plusieurs demandent une infrastructure qui n'existe pas encore —
une base à rendre indisponible, un worker à tuer. §29.1 à §29.3 (domaine, contrats, workflows) ne
sont pas dans ce registre : ce sont des suites de tests qui existent déjà et qui échouent d'
elles-mêmes, là où §29.4, §29.5 et §29.8 sont des exercices qu'on peut simplement ne pas faire.

**Prochain item.** W12.b — l'endurance de §29.6 : les huit seuils, et le constat qui les confronte.

## 2026-08-18 — W12.b — L'endurance de §29.6 : trois façons de ne pas tenir, trois gestes

**Périmètre.** `packages/evaluation/src/endurance.rs` (neuf),
`packages/evaluation/tests/endurance.rs` (neuf, 9 tests), `src/lib.rs` (les réexports), ce fichier.

**Tests exécutés.** `cargo test -p locus-evaluation` → 21 conformes (12 + 9). `npm run check` → les
dix portes vertes. Mutation : quatorze mutants, **quatorze tués, aucun survivant**.

**Décisions prises.**

_Neuf exigences, pas huit._ §29.6 met les sept jours dans la phrase qui introduit la liste, pas dans
la liste. Une campagne de six jours qui aurait atteint les huit puces n'est pourtant pas celle que
§29.6 demande, et laisser la durée vivre ailleurs reviendrait à l'oublier. Elle est donc traitée
comme les autres.

_Trois causes séparées, parce qu'elles n'appellent pas le même geste._ Un seuil **mesuré et sous la
barre** demande de prolonger la campagne. Un seuil **non relevé** demande d'instrumenter : personne
n'a compté, et c'est une panne de mesure, pas de tenue. Un invariant **violé** — une perte, une
double application — demande de corriger le produit, et tourner plus longtemps n'y changera rien.
Les fondre en un seul « échec » ferait chercher au mauvais endroit dans deux cas sur trois.

_Non relevé ne vaut pas zéro._ Compter zéro pour ce que personne n'a compté ferait passer une
absence d'instrumentation pour une campagne ratée. Deux mutants distincts l'éprouvent — « compte
comme atteint » et « compte comme zéro » — parce que les deux erreurs opposées mènent au même
endroit : un verdict qui ne dit pas la vérité sur ce qu'on sait.

_La reprise ne se compte pas, les seuils ne se constatent pas._ « La reprise s'est bien passée 4
fois » ne dit rien de la cinquième, et c'est exactement la question ; répondre « oui » à « avez-vous
eu 5 000 tâches ? » ne dit pas combien. Les deux confusions sont refusées, chacune par son nom.

_Redémarrages et pertes de workers valent un._ §29.6 les veut « réguliers » sans chiffrer. Zéro est
la seule valeur dont on soit sûr qu'elle ne les exerce pas ; fixer un chiffre plus haut serait
inventer une exigence que le texte ne pose pas.

**Écart avec la spec.** Rien ici ne mène de campagne : le module dit ce que §29.6 exige et confronte
un relevé. Instrumenter la campagne — compter les événements, constater une double application — est
le travail des paquets qui les produisent, et l'endurance elle-même demande sept jours de machine.

**Prochain item.** W12.c — les benchmarks de §29.7 : six configurations, onze mesures.

## 2026-08-18 — W12.c — Les benchmarks de §29.7 : une mesure absente n'est pas une mesure nulle

**Périmètre.** `packages/evaluation/src/benchmark.rs` (neuf),
`packages/evaluation/tests/benchmark.rs` (neuf, 12 tests), `src/lib.rs` (les réexports), ce fichier.

**Tests exécutés.** `cargo test -p locus-evaluation` → 33 conformes (12 + 9 + 12). `npm run check` →
les dix portes vertes. Mutation : onze mutants, **onze tués, aucun survivant**.

**Décisions prises.**

_Le classement refuse de trancher tant qu'une lecture manque._ C'est la faute que ce module existe
pour empêcher, et elle est silencieuse : une configuration dont on n'a pas relevé les faux positifs
les aurait à zéro dans un classement naïf, donc gagnerait. §29.7 compare six architectures dont la
dernière est celle qu'on construit — ce qui rend la tentation permanente et les trous de mesure
dangereux. Refuser est le comportement utile ici : un classement partiel a l'air d'un résultat, et
se cite comme tel.

_Plus n'est pas toujours mieux._ Quatre mesures sur onze se lisent à l'envers. Se tromper de sens
ferait élire la configuration la plus chère, et le classement aurait l'air parfaitement sain — deux
mutants l'éprouvent, l'un qui inverse tout, l'autre qui déplace seulement le coût.

_Une lecture interprétative, déclarée comme telle._ §29.7 ne dit pas dans quel sens lire le « taux
de rejet en revue ». Il est pris ici comme « moins est mieux » — une architecture dont les
productions se font rejeter plus souvent produit un travail moins bon — et le doc-comment le dit,
pour qu'on puisse le contester plutôt que le découvrir.

_`NaN` est refusé._ Il rendrait le classement **muet** plutôt que faux, ce qui est pire : un
classement faux se remarque, un classement qui élit toujours le premier venu non.

_Le compilateur a tué quatre mutants avant les tests._ Retirer une configuration ou une mesure ne
compile pas — les tableaux sont dimensionnés, le `match` est exhaustif. C'est le type qui tient la
liste close de §29.7, et les mutants ont été refaits en dupliquant une entrée plutôt qu'en la
retirant, pour éprouver ce que les tests tiennent réellement. Un mutant qui ne compile pas n'est pas
un mutant tué.

**Écart avec la spec.** Ce module ne mesure rien : il dit ce que §29.7 compare et refuse de conclure
sur une comparaison trouée. Produire les lectures demande de faire tourner six architectures sur un
corpus commun, ce qui est une campagne, pas une fonction.

**Prochain item.** W12 est couvert. La roadmap n'a plus de section en prose ; ce qui reste est W14 à
W18, et la 3D de §23.4 qui demande d'abord de décider comment on la vérifie.

## 2026-08-18 — W14.a — Le moteur de politique : la priorité est déclarée, la trace est produite

W14 est décomposée ici en quatre lignes. §13 était déjà couvert pour l'essentiel par W7.e à W7.g —
budgets, anti-gaming, qualité-diversité, `V(b)` — donc ce qui restait de W14 est §20, le moteur de
politique.

**Périmètre.** `packages/policy/` (neuf : `Cargo.toml`, `src/lib.rs`, `tests/engine.rs` avec 16
tests), `Cargo.toml` du workspace, `docs/10_V1_ROADMAP.md`, ce fichier.

**Tests exécutés.** `cargo test -p locus-policy` → 16 conformes. `npm run check` → les dix portes
vertes. Mutation : treize mutants, **treize tués, aucun survivant**.

**Décisions prises.**

_Trois exigences de §20.2 se tiennent l'une l'autre, et le module les traite comme une seule chose._
Les faits séparés rendent le déterminisme vrai ; le déterminisme rend la décision rejouable ; la
trace rend le rejeu compréhensible. Un moteur qui perdrait l'une des trois garderait l'air de
marcher — et c'est pour cela que `Facts` est un type et pas une convention : ce qui n'y est pas
n'entre pas dans la décision, ni l'heure, ni un compteur, ni le résultat de la fois d'avant.

_La priorité est déclarée, jamais héritée de l'ordre._ Trancher par la position dans un fichier
ferait d'un réordonnancement un changement de comportement — et personne ne relit un diff de
réordonnancement comme tel. Le test construit la même politique dans les deux sens et exige le même
verdict.

_Un conflit est rendu, pas résolu._ À priorité égale et verbes contraires, le moteur s'arrête et
nomme les règles. Choisir tout de même reviendrait à décider à la place de qui a écrit les règles,
et à le faire en silence. Deux règles **d'accord** à priorité égale ne sont en revanche pas un
conflit : le signaler serait un faux positif qui pousserait à supprimer une règle utile.

_La trace porte toutes les règles déclenchées, pas seulement la gagnante._ §20.5 demande « les
règles déclenchées » ; savoir ce qui a failli s'appliquer est la moitié de ce qui rend une décision
contestable. Elle porte aussi la **version** de chaque règle : sans elle, on relirait une règle qui
a changé depuis, donc on reconstituerait une décision qui n'a pas eu lieu.

_`NoRule` n'est pas `allow`._ Personne n'a autorisé quoi que ce soit. C'est à l'appelant de décider
ce qu'il fait d'un silence, et le lui dire est le seul moyen qu'il ait le choix. Septième occurrence
de la même distinction dans ce chantier.

_Seul `allow` laisse passer la demande telle quelle._ `modify` laisse passer **autre chose**, et les
confondre ferait appliquer la demande d'origine alors qu'une contrainte avait été imposée.

**Écart avec la spec.** §20.2 demande aussi le dry-run et la conservation des overrides ; §20.1
nomme seize catégories ; §20.4 définit la `Delegation` ; §20.5 énumère huit facettes
d'explicabilité. Ce commit livre le cœur — verbes, faits, trace, priorité, conflits, déterminisme —
et les trois autres items de W14 portent le reste. La DSL elle-même n'est pas écrite : les règles se
construisent en mémoire, et un lecteur YAML viendra quand un producteur en écrira.

**Prochain item.** W14.b — la `Delegation` de §20.4 : portée, plafonds, expiration, révocation.

## 2026-08-18 — W14.b — La délégation de §20.4 : deux principals, jamais un

**Périmètre.** `packages/policy/src/delegation.rs` (neuf), `packages/policy/tests/delegation.rs`
(neuf, 12 tests), `src/lib.rs` (les réexports), ce fichier.

**Tests exécutés.** `cargo test -p locus-policy` → 28 conformes (16 + 12). `npm run check` → les dix
portes vertes. Mutation : quatorze mutants, **quatorze tués, aucun survivant**.

**Décisions prises.**

_Deux attributions, et rien qui les résume._ §20.4 : « les actions d'un agent sont attribuées au
principal agentique **et** à la délégation humaine ou institutionnelle qui les autorise ». Un
journal qui ne retiendrait que l'agent ferait porter à un programme une décision qu'un humain a
autorisée ; un journal qui ne retiendrait que le délégant effacerait qui a agi. Les deux erreurs
sont symétriques et toutes deux invisibles à la relecture — c'est la même forme que les deux
verdicts de §19, et deux mutants distincts l'éprouvent dans les deux sens.

_L'agent attribué est celui qui a **demandé**, pas le délégataire nommé._ Un sous-agent qui agit
sous une délégation garde son identité ; les confondre ferait porter ses actes à un autre.

_Cinq motifs de refus, pas un._ Agir hors portée est une erreur d'aiguillage, dépasser un plafond
est une demande trop grande, agir après expiration est une autorisation périmée, agir sous
révocation est une autorisation retirée. Un « non autorisé » sans motif ferait chercher au mauvais
endroit dans quatre cas sur cinq.

_La révocation prime sur tout le reste._ Une demande par ailleurs fautive est refusée **pour
révocation** : un motif secondaire ferait croire qu'en corrigeant la demande on retrouverait
l'autorisation.

_Les bornes sont des bornes._ Le plafond atteint exactement passe, la fenêtre inclut son début et
exclut sa fin. Une inégalité stricte de trop rendrait inutilisable la dernière unité de budget
accordée ; une borne d'expiration inclusive ferait durer la délégation un instant de plus, et cet
instant est exactement celui où quelqu'un croit qu'elle a cessé.

_`revocable` décide de quelque chose._ Une délégation irrévocable refuse la révocation au lieu de
l'accepter silencieusement — accepter en apparence et continuer d'autoriser serait la pire des deux
réponses, puisque le délégant croirait avoir agi. Le test vérifie les deux moitiés : le refus, et le
fait qu'elle continue d'autoriser, ce qui est cohérent avec lui.

_Aucune horloge n'est lue._ `valid_from` et `expires_at` sont des instants que l'appelant fournit,
pour la même raison que le moteur de politique ne lit aucun fait qu'on ne lui a pas donné : une
autorisation qui dépendrait de l'heure qu'il est ne se rejouerait pas.

**Écart avec la spec.** §20.4 nomme aussi la chaîne de délégation — une délégation qui en autorise
une autre. Rien ici ne la porte : le texte ne dit pas si elle est permise, et l'inventer donnerait à
un agent le moyen de s'étendre lui-même. À trancher quand un consommateur en aura besoin.

**Prochain item.** W14.c — l'explicabilité de §20.5, dont les alternatives rejetées.

## 2026-08-18 — W14.c — L'explicabilité de §20.5 : la facette qu'on omet toujours

**Périmètre.** `packages/policy/src/explanation.rs` (neuf), `packages/policy/tests/explanation.rs`
(neuf, 10 tests), `src/lib.rs` (les réexports), ce fichier.

**Tests exécutés.** `cargo test -p locus-policy` → 38 conformes (16 + 12 + 10). `npm run check` →
les dix portes vertes. Mutation : quatorze mutants, **quatorze tués, aucun survivant**.

**Décisions prises.**

_Sept facettes se remplissent seules, une seule demande d'être décidée._ §20.5 en énumère huit ;
sept sortent naturellement de la construction de la décision. Les **alternatives rejetées** sont la
seule qui n'existe nulle part une fois la décision prise — donc la seule qu'il faut vouloir garder.
C'est aussi celle qui rend une décision contestable : savoir qu'un moteur a choisi A ne dit rien
tant qu'on ignore s'il a même envisagé B.

_Une alternative rejetée sans motif n'en est pas une._ « Nous avons envisagé B » sans dire pourquoi
ne se conteste pas — il n'y a rien à objecter. Et une case cochée dans un rapport d'explicabilité
est **pire que son absence**, parce qu'elle donne l'apparence d'un examen qui n'a pas eu lieu.

_Conserver un override veut dire garder les deux verdicts._ §20.2 exige de « conserver les overrides
humains ». `machine_outcome` rend ce que le moteur avait conclu, `effective_outcome` ce qui
s'applique, et rien ne les fond. Les confondre effacerait la conclusion du moteur, et personne ne
pourrait plus distinguer une erreur corrigée d'une garde contournée. Le cas où cela compte le plus
est l'override d'un **conflit** : c'est le moment où un humain tranche ce que le moteur a refusé de
trancher, et où il faut pouvoir relire ce refus.

_Un override anonyme ou muet est refusé._ Il serait indiscernable d'un défaut du moteur, et c'est
précisément ce qu'il ne faut pas confondre.

_Deux facettes vides seulement sont toujours un manquement._ Sans données d'entrée la décision ne se
rejoue pas ; sans règle déclenchée elle ne s'explique par rien. Les autres peuvent être légitimement
vides — une décision sans override n'a pas d'override à montrer, et crier au manquement sur un
exposé complet apprend à ignorer l'alarme. Un mutant qui déclare tout exposé incomplet meurt sur ce
test.

**Une collision de noms.** `Rejected::because(option, because)` et l'accesseur `because()` ne
peuvent pas coexister ; le constructeur est devenu `considered`. Le nom dit mieux ce qu'il fait de
toute façon : on consigne qu'une option **a été envisagée**, et le motif est ce qu'elle porte.

**Écart avec la spec.** Deux facettes de §20.5 restent structurellement vides : « scores et
incertitudes » n'a pas de producteur — le moteur de W14.a décide par règles, pas par score — et «
politique et version » vit dans la trace plutôt que dans un champ propre, puisque c'est chaque règle
déclenchée qui porte sa version. `Facet` les nomme toutes les huit pour que la liste soit close ;
les remplir demande des consommateurs qui n'existent pas.

**Prochain item.** W14.d — les seize catégories de §20.1 et le dry-run de §20.2.

## 2026-08-18 — W14.d — Les seize catégories de §20.1 et le dry-run qui est le même calcul

**Périmètre.** `packages/policy/src/category.rs` (neuf), `packages/policy/tests/category.rs` (neuf,
10 tests), `src/lib.rs` (les réexports), ce fichier.

**Tests exécutés.** `cargo test -p locus-policy` → 48 conformes (16 + 12 + 10 + 10). `npm run check`
→ les dix portes vertes. Mutation : onze mutants, **onze tués, aucun survivant**.

**Décisions prises.**

_Le dry-run n'est pas une seconde évaluation._ §20.2 demande « dry-run et simulation ». La faute que
cette exigence prévient est courante : un chemin de simulation écrit à part, qui diverge du chemin
réel le jour où l'un des deux est corrigé — et la simulation cesse alors de dire ce que fera le run,
la seule chose qu'on lui demande. `Run::dry` et `Run::live` partagent donc **exactement** le même
calcul, et un mutant qui donne au dry-run son propre chemin meurt.

_Ce qui change n'est pas le calcul mais ce qu'on a le droit d'en faire._ Une `Simulation` n'expose
rien qui produise un effet : elle prête l'exposé, quand un run réel le **rend par valeur** pour
qu'on y rattache des événements. La garantie n'est pas une discipline d'appel — il n'y a pas de
méthode à ne pas appeler.

_Le dry-run reproduit aussi les conflits et les silences._ Ce sont exactement les états qu'un chemin
de simulation bâclé simplifierait, et un mutant qui remplace un conflit simulé par « aucune règle »
meurt.

_La liste des seize est close, et c'est le seul moyen de dire ce qui manque._ Le résultat utile de
`Coverage` est la liste des **absentes** : le risque n'est pas d'écrire une mauvaise politique de
secrets, c'est de n'y pas penser. Une liste ouverte ne saurait pas le dire. Le test parcourt les
seize une par une — même geste que pour les cinq parties d'une sauvegarde et les trente-cinq
épreuves de §29.

**Écart avec la spec.** §20.2 demande aussi « simulation », qu'on pourrait lire comme davantage
qu'un dry-run : évaluer contre des faits hypothétiques, comparer deux versions de politique. Le
dry-run est livré ; la simulation contrefactuelle attend un consommateur, et l'inventer produirait
une API que rien n'appelle. W14 est couvert pour ce que §20 exige et que le dépôt peut éprouver.

**Prochain item.** W15 — le cœur du graphe agentique et la contestabilité.

## 2026-08-18 — W15.a — La version canonique immuable et les sept opérations qui ont un lecteur

**Périmètre.** `docs/10_V1_ROADMAP.md` (décomposition de W15 en six items),
`packages/coordination/src/version.rs` (neuf), `packages/coordination/tests/version.rs` (neuf, 20
tests), `src/lib.rs` (les réexports), `src/proposal.rs` (`Ord` sur `Relation` et `RelationKind`,
pour l'ordre canonique), ce fichier.

**Tests exécutés.** `cargo test -p locus-coordination` → 60 conformes. `npm run check` → les dix
portes vertes. Mutation : vingt-sept mutants, **vingt-six tués**, un équivalent devenu un test.

**Décisions prises.**

_Deux hashes, et c'est tout le sujet._ Une version porte un hash de **contenu** — qui ne dépend que
de ce qu'elle contient — et un hash de **version**, qui ajoute le parent. La séparation rend
testable la phrase de l'ADR 0016 décision 5 : défaire une opération rend le **même contenu** et une
**autre version**. L'état revient, l'histoire non. Avec un seul hash il aurait fallu choisir : ou
bien défaire ramène littéralement à la version d'avant, et l'histoire devient fausse — on ne
pourrait plus dire qu'une mission a tourné sous une organisation qui, désormais, n'aurait jamais
existé ; ou bien défaire produit un état que personne ne peut reconnaître comme celui d'avant, et
plus rien ne vérifie qu'une annulation a annulé. Deux assertions par opération dans le test, et
l'une sans l'autre serait fausse.

_L'identité tient aux deux bouts._ Un mutant retirant le contenu de l'identité a d'abord survécu :
rien n'éprouvait deux opérations différentes menées depuis la même base. C'est pourtant le cas qui
compte — deux organisations distinctes se seraient citées sous le même nom.

_Sept opérations, pas onze, et la règle qui tranche._ `docs/13` nomme onze opérations cibles. La
règle « aucune sémantique inerte » (décision 4) vaut pour une opération comme pour une sorte de
relation, et elle trace ici une frontière nette : une opération **structurelle** a son effet
entièrement défini par l'état que ce crate détient, donc un consommateur exécutable et testé, qui
est `Version::apply` ; une opération **attributaire** écrit un champ dont le lecteur vit ailleurs.
`SET_ROLE` attend l'overlay additif du worker, `SET_VISIBILITY` la construction de `ContextView`,
`SET_VALIDATOR` qu'un validateur soit un nœud, et `SET_EXECUTION_ORDER` qu'une chose ordonne des
attempts entre instances d'agent — ce que la décision 4 a déjà vérifié absent en instruisant
`dependency`. Un test tient les quatre par l'absence, en les nommant, pour que l'échec dise
**laquelle** est entrée sans son consommateur.

_Aucune cascade._ Retirer un nœud qui porte encore des arêtes est **refusé**, pas exécuté en
emportant les arêtes. Une cascade est un script : elle fait au commit des choses que le diff ne
montrait pas, et l'approbation aurait porté sur autre chose que ce qui s'applique. C'est aussi ce
qui rend `ADD_NODE` et `REMOVE_NODE` exactement inverses l'un de l'autre.

_Fusionner se compense, ne se défait pas._ Six opérations sur sept ont un inverse exact ; la fusion
n'en a pas, et la raison se lit dans sa définition : elle perd la partition. Deux arêtes
`X → premier` et `X → second` deviennent une seule, et aucune scission ne saurait dire laquelle
était laquelle. La scission, elle, **énonce** sa partition, donc sa fusion inverse la restitue —
l'asymétrie est réelle et deux tests la montrent plutôt que de l'affirmer. `Undo::Compensating` la
nomme au lieu de la cacher derrière une fonction qui rendrait une scission plausible, et
`Undo::exact()` rend `None` : il n'existe dans le module aucune fonction qui prétende défaire une
fusion.

_Un refus qui n'était pas prévu : fusionner un relecteur avec son relu._ La substitution en ferait
une relation d'un agent vers lui-même — un agent qui se relit, obtenu sans qu'aucune opération ne
l'ait demandé, contre §14.4 et l'invariant 11. La fusion est refusée et l'appelant retire l'arête
d'abord, dans le diff, où l'approbateur la voit. Un mutant n'examinant qu'un seul sens a survécu au
premier tour : le test ne fusionnait que dans l'ordre où l'arête est écrite.

_Les identités produites sont neuves des deux côtés._ Une scission ne réutilise pas l'identité du
nœud scindé, une fusion ne reprend pas celle de l'un des deux absorbés. Sinon l'histoire ne
distinguerait plus « ils ont fusionné » de « l'autre a été retiré », et rien en aval ne saurait
laquelle des deux moitiés d'une scission est l'originale.

**Un mutant équivalent, devenu un test.** Échanger les deux moitiés dans l'inverse d'une scission ne
change rien, parce que la fusion ne distingue pas ses deux absorbés. Ce n'était donc pas un trou de
test — mais la propriété n'est pas gratuite : elle tient tant que la substitution envoie les deux
vers `into` et que le refus de l'auto-relation examine les deux sens. Elle est désormais affirmée
par `merging_does_not_distinguish_its_two_nodes` plutôt que laissée à un survivant inexpliqué.

**Sur la frontière.** `packages/coordination` n'importe pas `packages/graph`, et n'en a pas eu
besoin : les nœuds sont des `Id<Agent>`, les arêtes la `Relation` de W13.e. La sixième frontière
tient sans effort, ce qui est le signe qu'elle passait au bon endroit.

**Sur le nom.** Aucun type collectif n'est introduit — pas de `CoordinationGraph`, pas de
`Topology`. `Version` porte ses membres et ses relations directement, comme la décision 10 le
demande : les types sont nommés individuellement, et le nom du domaine reste en aval de la thèse.

**Écart avec la spec.** Le hachage reste un **port** : ce crate produit la forme canonique et confie
le condensat à `Digest`, comme `packages/artifacts` et `packages/visualization` avant lui. Le test
en fournit un jouet ; ce qui est vérifié est la stabilité de la forme canonique, figée octet pour
octet en fixture.

**Prochain item.** W15.b — le diff comme objet de première classe.

## 2026-08-18 — W15.b — Le diff comme objet de première classe

**Périmètre.** `packages/coordination/src/diff.rs` (neuf), `packages/coordination/tests/diff.rs`
(neuf, 13 tests), `src/version.rs` (`Operation::canonical`, additif), `tests/version.rs` (un test de
plus), `src/lib.rs` (les réexports), `docs/10_V1_ROADMAP.md` (formulation du test de sortie), ce
fichier.

**Tests exécutés.** `cargo test -p locus-coordination` → 74 conformes. `npm run check` → les dix
portes vertes. Mutation : vingt-trois mutants, **vingt-trois tués, aucun survivant**.

**Décisions prises.**

_Une base est une version, une cible est un contenu._ L'asymétrie est la conséquence directe des
deux hashes de W15.a. Rejouer se fait sur une **histoire précise**, donc la base est une `VersionId`
; ce que le rejeu produira n'a pas encore d'histoire — son identité dépend du rejeu — donc la cible
ne peut être qu'un `ContentHash`. Annoncer la cible sous forme d'identité de version obligerait à la
deviner avant de l'avoir produite.

_Le test de sortie a été reformulé en conséquence, et il en sort plus fort._ Il disait « rend
exactement la cible, hash de version compris » ; ce qui est vrai et qui compte est : le rejeu rend
exactement le **contenu** visé, et **deux rejeux du même diff sur la même base rendent la même
version, identité comprise**. C'est cela que `docs/10` W17 demande — « diff calculé une fois côté
serveur, donc identique dans Emacs et dans le web, sinon l'approbation porte sur ce que chaque
client a cru voir ». Un test vérifie l'égalité de deux rejeux jusqu'à l'identifiant, un autre qu'une
base de contenu identique mais d'histoire différente est refusée.

_Un diff n'invente pas d'intention._ `Diff::between` n'émet que les quatre opérations qui décrivent
un écart d'états. Il n'infère jamais `REPLACE_NODE`, `SPLIT_NODE` ni `MERGE_NODES` : au niveau des
états, un remplacement est indiscernable d'un retrait suivi d'un ajout, et deviner ferait lire à
l'approbateur une intention que personne n'a écrite. C'est la règle de §7.5 sur les relations, « ne
doivent pas être inférées en sens inverse », appliquée aux opérations. Les opérations riches
viennent du proposeur par `Diff::declaring`, et le diff les garde telles quelles — deux chemins, un
seul type.

_L'ordre est le diff._ Une version est un ensemble, un diff est une **suite**. Sa forme canonique
n'est donc pas triée, contrairement à celle d'une version : le refus de la cascade impose de retirer
les arêtes avant les nœuds et d'ajouter les nœuds avant les arêtes, et la même liste dans un autre
ordre ne s'applique pas. Trier ferait signer deux clients sur un document qui ne décrit pas ce qui
sera commité. Deux mutants échangeant les phases meurent.

_Ce qui prouve n'est pas ce qui est annoncé._ `Diff::from_wire` lit un diff venu d'ailleurs — §22.4
le sert sur `/branches/:id/diff` — et ne vérifie rien, faute de base sous la main. C'est le rejeu
qui confronte, et il confronte le contenu **produit** à celui que le document déclare. Un diff
flatteur est refusé, pas cru sur parole. Septième occurrence de cette discipline dans le dépôt.

_Vide n'est pas absent._ Le diff d'une version vers elle-même existe, et `is_empty()` le dit. Rendre
`None` obligerait chaque appelant à un cas particulier, et surtout un approbateur ne verrait _rien_
au lieu de lire que la proposition ne change rien — ce qui est une information, et souvent une
surprise. Rejouer une suite vide rend la base **elle-même** : produire une version au contenu
identique inscrirait qu'il s'est passé quelque chose là où il ne s'est rien passé, pendant exact de
la cascade qui inscrit moins que ce qui arrive.

_Un refus nomme laquelle et où._ `Inapplicable` porte la **position** dans la suite et la forme
canonique complète de l'opération fautive. Un mutant ne rapportant que le nom a survécu au premier
tour : dans une suite qui ajoute plusieurs nœuds, « `ADD_NODE` » fait deviner lequel.

_La forme canonique d'une opération porte tout ce qui décide de son effet._ Deux mutants ont montré
que ce n'était pas acquis — une scission qui n'écrivait pas sa partition, un remplacement qui
perdait sa cible — et ils survivaient parce que le test de diff les distinguait par leur ligne
`target`, pas par leur ligne `op`. Un test fait désormais varier **un champ à la fois** sur les sept
opérations et exige quatorze formes distinctes.

**Un mutant qu'on ne peut pas écrire.** « Rejouer le vide inscrit une nouvelle version » n'a pas de
mutant : la propriété découle de l'absence de branche — `replay_onto` part de `base.clone()` et n'a
aucun cas particulier à se tromper. Le test l'affirme quand même, parce que le jour où quelqu'un
ajoutera ce cas particulier, c'est lui qui le rattrapera.

**Prochain item.** W15.c — les régions mutables bornées de GRAFT.

## 2026-08-18 — W15.c — Les régions mutables bornées de GRAFT

**Périmètre.** `packages/coordination/src/region.rs` (neuf), `packages/coordination/tests/region.rs`
(neuf, 17 tests), `src/lib.rs` (les réexports), `docs/10_V1_ROADMAP.md` (précision du test de
sortie), ce fichier.

**Tests exécutés.** `cargo test -p locus-coordination` → 91 conformes. `npm run check` → les dix
portes vertes. Mutation : vingt-sept mutants, **vingt-sept tués, aucun survivant**.

**Décisions prises.**

_Le veto n'est pas un second critère local, et c'est tout le dispositif._ C'est le seul endroit où
GRAFT peut être implémenté de travers sans que rien ne le montre. Le test qui le tient : la région
contient `1`, `2` et `3` ; l'organisation porte déjà `2 → 4 → 1` avec `agent(4)` **dehors** ;
ajouter `1 → 2` ne touche que des nœuds de la région, donc le critère local accepte — et le cycle
`1 → 2 → 4 → 1` vient de se fermer. Trois agents qui se relisent en rond ne sont relus par personne.
Un veto qui ne regarderait que la région serait un critère local écrit deux fois, et coûterait
exactement la garantie qu'on croyait avoir. Deux mutants le vérifient, dont un qui remplace le veto
par une constante « cohérent ».

_Quatre bornes interdisent, deux obligent._ `docs/13` en donne six. `allowed_ops`, `risk_ceiling`,
`max_nodes_delta` et `max_edges_delta` refusent ; `approval_mode` et `require_shadow` **exigent**.
Les mélanger ferait croire qu'une région à `require_shadow` bloque une proposition, alors qu'elle
demande une étape de plus — et un opérateur qui attend un refus qui ne vient jamais finit par croire
que la borne n'existe pas. Le test de sortie a été précisé en ce sens : il disait « laquelle des six
bornes mord », ce qui aurait poussé à fabriquer deux refus qui n'ont pas lieu d'être. Un test
énumère les bornes qui peuvent mordre et vérifie **par l'absence** que les deux obligations n'y sont
pas.

_« Delta » se mesure en différence symétrique, pas en solde._ Un solde net est jouable : ajouter
cinq agents et en retirer cinq passe sous un plafond de zéro alors que dix identités ont changé. Ce
que GRAFT veut borner est le rayon d'explosion. Deux tests montrent le solde nul avant de constater
le refus — un pour les nœuds, un pour les arêtes, parce que le premier seul laissait survivre le
mutant du second.

_Le risque est dérivé, jamais déclaré._ `docs/10` W18 demande une classe de risque « **dérivée** des
invariants menacés ». Un risque que le proposeur déclarerait serait une auto-évaluation sous
plafond, c'est-à-dire la définition d'une borne qu'on contourne. Ici le risque d'une opération est
le **nombre d'invariants globaux qu'elle peut menacer**. Aujourd'hui zéro ou un ; l'échelle
s'élargira d'elle-même quand un deuxième invariant entrera. C'est peu, et c'est vrai.

Seules deux opérations peuvent fermer un cycle, et le constater a demandé de le vérifier une par une
: ajouter une arête, évidemment ; et **fusionner**, qui rapproche deux extrémités — `A → B → C`
fusionné sur `A` et `C` devient un cycle de longueur deux **sans qu'aucune arête n'ait été
ajoutée**. Retirer ne crée aucun chemin, remplacer est un isomorphisme, scinder ne fait que répartir
des arêtes existantes.

_Un seul invariant global._ `ReviewAcyclicity`, avec un vérificateur exécutable et testé, pour la
même raison que `RelationKind` n'a qu'une valeur. En nommer d'autres produirait un veto qui aurait
l'air de protéger ce que rien ne regarde. C'est le consensus circulaire de §16.6 transposé au
domaine de la coordination : chacun est relu, le groupe ne l'est par personne, et l'invariant 11 est
vidé de son sens.

_Le veto nomme les agents pris dans le cycle._ La détection est itérative — élimination des nœuds
sans arête entrante, à la Kahn — et ce qui reste **est** exactement l'ensemble des nœuds pris dans
un cycle. Le rendre coûte zéro et évite de le chercher à la main. Un veto qui dirait « il y a un
cycle » sans dire lequel ne se corrige pas.

_L'acceptation locale n'expose rien qui commite._ `Acceptance` énonce ce qui reste à obtenir —
l'approbation, et l'ombre si la région l'exige — et n'a aucune méthode d'écriture. La garantie est
dans le type, pas dans une discipline d'appel, comme la `Simulation` de W14.d. Un lot **vetoé**
garde son acceptation : la perdre ferait croire à une borne de région trop lâche alors que la région
a fait son travail, et la version corrigée repartirait sans ombre ni approbation.

_Une région ne se prononce pas sur un lot qui ne s'applique pas._ `admits` rejoue d'abord ; si le
rejeu échoue, la région ne dit rien. Elle parlerait d'un état qui n'existera jamais, et son verdict
serait cité comme s'il portait sur quelque chose.

_Une région ne peut pas autoriser une opération qui n'existe pas._ `SET_ROLE` attend son lecteur en
W15.e ; l'accepter dans `allowed_ops` ne permettrait rien pendant que l'auteur de la région croirait
le contraire — le pire des deux états.

**Une décision assumée sur le périmètre d'une scission.** Les nœuds « touchés » par une opération
sont ceux dont l'appartenance change, plus les extrémités des arêtes qu'elle nomme. La **partition**
d'une scission n'y entre pas : exiger que tout le voisinage d'un agent soit dans la région rendrait
toute région inutile dès qu'un agent est relu depuis l'extérieur, ce qui est le cas courant. Les
conséquences externes d'une scission sont ce que le veto global attrape — c'est exactement le
partage du travail que GRAFT décrit.

**Trois trous trouvés par la mutation, tous de la même forme.** Une opération peut faire **entrer**
une identité hors de la région : la cible d'une arête, le remplaçant d'un `REPLACE_NODE`, l'identité
produite par une fusion. Vérifier la source d'une arête sans vérifier sa cible laisserait une région
recâbler vers n'importe qui. Un test couvre les trois cas ensemble, parce qu'ils se ratent
séparément.

**Prochain item.** W15.d — la contestabilité d'une décision de coordination.

## 2026-08-18 — W15.d — La contestabilité d'une décision de coordination

**Périmètre.** `packages/coordination/src/objection.rs` (neuf),
`packages/coordination/tests/objection.rs` (neuf, 7 tests), `src/lib.rs` (les réexports) ;
**septième frontière** : `CLAUDE.md`, `boundaries.json` (deux catalogues, une règle),
`tooling/boundaries/rules.ts` et `analyze.ts` (une nature de règle nouvelle),
`tests/boundaries/fixtures/imports/objection-families-converted/` (la violation délibérée),
`tests/boundaries/contract.test.ts` (deux assertions) ; `docs/adr/0016` (amendement des
conséquences), ce fichier.

**Tests exécutés.** `cargo test -p locus-coordination` → 98 conformes.
`node --test tests/boundaries/*.test.ts` → 37 conformes. `npm run check` → les dix portes vertes.
Rouge puis vert sur le vrai dépôt : une conversion écrite dans `packages/projections` fait échouer
`check:boundaries`, son retrait la fait passer. Mutation : vingt-trois mutants, **vingt-trois tués,
aucun survivant**.

**Décisions prises.**

_Le test d'absence ne peut pas vivre dans le crate._ C'est le constat qui a décidé de tout le reste,
et il n'était pas dans l'ADR. La décision 9 demandait « un test vérifiant l'absence de conversion »
sans dire où. Or la sixième frontière interdit à `packages/coordination` d'importer `packages/graph`
: un test écrit là-bas ne pourrait pas **nommer** `ObjectionTarget`, fût-ce pour affirmer qu'il ne
le convertit pas. Un test qui dirait « la conversion n'existe pas » sans pouvoir voir l'autre
famille n'affirmerait rien du tout.

Et la règle 6 rend la conversion impossible **dans les deux crates**. Elle ne peut donc naître que
dans un **troisième** fichier qui importe les deux — un cockpit qui « unifierait l'affichage des
objections », par exemple. C'est celui-là qu'il faut regarder, et c'est la septième frontière : «
aucun fichier ne voit les deux familles d'objection à la fois ».

_Une nature de règle nouvelle, parce que ce qui est interdit est une conjonction._ Aucun des deux
catalogues n'est interdit pris seul — `locusd` aura légitimement besoin du graphe et des objets de
coordination dans le même fichier. Ce qui est interdit est de voir les deux familles
d'**objection**. `no-co-import` ne s'exprime donc pas avec deux règles `imports`, et le refus cite
**les deux** spécificateurs : « ce fichier voit les deux » sans dire lesquelles des lignes est un
rapport sur lequel personne n'agit.

_Les catalogues sont au grain du symbole, et l'évasion par glob a été vérifiée close._ Un
`use locus_graph::*` contournerait un catalogue au grain du symbole. Il est déjà impossible ici :
`clippy::wildcard_imports` est `pedantic` et la CI passe `-D warnings`. La vérification est
consignée dans `boundaries.json` pour qu'elle ne soit pas refaite, comme celle de `dependency` dans
l'ADR.

_Quatre cibles, et elles demandent trois corrections différentes._ §7.6 donne l'argument dans
l'autre domaine — « sur trois arêtes indépendantes, _la règle est fausse_ n'a aucun endroit où
s'accrocher ». Ici de même : objecter au **déclencheur** demande d'établir un fait ; objecter à la
**politique** demande de la reprendre, le fait étant admis ; objecter au **périmètre** demande de le
restreindre, la politique étant admise. Une seule « objection à la décision » rendrait la réponse
indéterminée. `Remedy` le rend explicite et un test exige quatre corrections distinctes.

_Le déclencheur est nommé, pas générique._ Objecter au déclencheur `x` d'une décision déclenchée par
`y` est **refusé** plutôt que consigné : le dossier porterait une contestation sans objet. « Le
déclencheur est faux » sans dire lequel obligerait à retrouver dans le dossier ce qui avait été
invoqué, et personne ne le fait.

_Le vocabulaire est celui de ce domaine._ `decision`, `trigger`, `policy`, `perimeter` — jamais
`premise`, `rule`, `scope`, `inference`. Un test le tient par l'absence : c'est par le vocabulaire
qu'une unification recommence, avant qu'une seule ligne de conversion soit écrite.

_Pourquoi la duplication est le bon choix, écrit dans le code._ Une conversion, même correcte à
l'écriture, ferait circuler une objection organisationnelle dans la machinerie épistémique — où
`packages/validation` propage l'invalidation sur les niveaux de §8.1. Une objection au périmètre
d'un recâblage n'a rien à propager sur un claim ; l'y faire entrer affaiblirait un résultat
scientifique au motif qu'une équipe a été mal composée.

**Un trou trouvé dans la garde elle-même, et qui la précédait.** Un mutant faisant déclarer à la
règle 7 l'état `skipped` a survécu : `check-boundaries` imprime « NON VÉRIFIÉE » sans faire échouer
la CI. C'est le **bon** comportement pour la règle 5, qui démarre un vrai Emacs et se saute là où il
n'y en a pas — et c'était un trou pour toutes les autres, qui lisent des imports déjà en mémoire et
ne peuvent pas légitimement se dérober. Un test ferme le trou sans durcir la sortie, pour que la
ligne « NON VÉRIFIÉE » de la règle 5 continue d'exister.

**Prochain item.** W15.e — `role`, deuxième membre de l'énumération, avec son consommateur
exécutable dans `canterel` (ADR 0016, clause de falsification de la décision 10).

## 2026-08-18 — W15.e — `visibility`, deuxième sorte, et le verdict de la clause de falsification

**Arbitrage préalable, validé.** W15.e devait livrer `role`. En l'instruisant, il est apparu que
`role` **n'est pas une relation** : `SPEC_V1.md` §7.1 en fait un champ d'`AgentTemplate`, §20 une
classification dans une exigence de reviewers, §6.3 un attribut d'appartenance héritable, et
`packages/coordination/src/agent.rs` le portait déjà comme attribut avant que la question soit
posée. W15.a l'avait classé indépendamment parmi les opérations **attributaires** différées, et les
deux analyses concordent. La clause de falsification de l'ADR 0016 décision 10 a donc été réorientée
sur `visibility`, qui est réellement de forme paire, et W15.e/W15.f échangés. `role` reste dû comme
`SET_ROLE`.

**Périmètre.** `packages/coordination/src/visibility.rs` (neuf),
`packages/coordination/tests/visibility.rs` (neuf, 13 tests), `src/proposal.rs` (la deuxième sorte),
`src/region.rs` (deux points d'application), `src/version.rs` (un message), `tests/proposal.rs` (la
garde de liste close), `Cargo.toml` (une dev-dependency), `packages/review/src/context_view.rs` (le
port `Visible`), `src/lib.rs` des deux crates ; `docs/adr/0016` et `docs/10` (l'amendement et
l'échange), ce fichier.

**Tests exécutés.** `cargo test -p locus-coordination` → 111 conformes. `npm run check` → les dix
portes vertes. Mutation : vingt mutants, **vingt tués, aucun survivant**.

---

### Le verdict de la clause de falsification : **l'abstraction tient**

La décision 10 posait deux branches. C'est la première : l'ajout « se branche en modifiant
l'énumération, la projection et un point d'application ». Aucun type n'a changé de forme —
`Relation { from, to, kind }` accueille `Visibility` sans une ligne de modification, et toute la
machinerie de W15.a à W15.c — version, hash, diff, régions, veto — l'a prise sans être touchée. Ce
qui a bougé tient en une énumération, un module de vingt lignes utiles, trois points d'application
et un port.

Le nom collectif peut donc être choisi en connaissance de cause. Ce ledger ne le choisit pas : la
décision 10 dit que le nom est en aval de la thèse, et une troisième sorte l'instruira mieux qu'une
deuxième.

### Ce que la deuxième sorte a révélé, et qui aurait été livré faux

Trois endroits où « relation » voulait dire « revue » sans le dire. Aucun n'était visible tant
qu'une seule valeur existait, et deux auraient produit un comportement faux plutôt qu'une erreur :

1. **Le veto d'acyclicité vetoait tous les cycles.** Un cycle de **visibilité** est normal — deux
   agents qui voient le travail l'un de l'autre coopèrent. Les vetoer aurait interdit la
   collaboration au nom de l'indépendance, avec un message parlant de consensus circulaire. Le veto
   ne regarde plus que les arêtes `review`.
2. **Le risque dérivé ne distinguait pas les sortes.** Ajouter une arête de visibilité ne peut pas
   fermer un cycle de revue, donc ne menace rien : une région à `risk_ceiling` nul peut désormais
   recâbler la visibilité sans rien relâcher. C'est le premier endroit où la deuxième sorte
   **gagne** quelque chose au lieu de coûter.
3. **Le message d'auto-relation ne parlait que de revue.** Sous `visibility`, une auto-relation est
   une redondance, pas une faute d'indépendance ; le refus reste, la phrase dit maintenant les deux.

### Décisions prises

_Elle retire, elle n'ajoute jamais._ §16.3 exige que les embeddings ne contournent pas les ACL ; une
relation de coordination ne le peut pas davantage. La garantie est **structurelle** plutôt que
promise : le port ne rend qu'un `bool` que la vue compose par un **et** avec le filtre de
contamination. Il n'existe aucun chemin par lequel une visibilité déclarée fasse entrer ce qu'un
autre refus écarte — parce qu'il n'y a rien à faire entrer, seulement quelque chose à laisser
sortir. Deux tests le tiennent, dont un qui exige que **les deux motifs** soient consignés quand les
deux mordent : réparer la contamination sans savoir que la visibilité écartait aussi ferait croire
le problème résolu.

_Un agent voit son propre travail, sans arête._ Il ne peut pas en avoir : `Version` refuse les
auto-relations depuis W15.a. Ce n'est pas une exception arrangeante, c'est la conséquence directe
d'une règle antérieure — et l'oublier ferait disparaître d'une vue le travail de celui qui la
reçoit.

_Ce qui n'est pas déclaré n'est pas vu._ Le défaut permissif ferait qu'ajouter un agent lui
donnerait accès à tout, et qu'il faudrait penser à l'en priver. Personne n'y pense. En revanche, ce
qu'**aucun agent** n'a produit — source externe, saisie humaine — n'est pas concerné : couper une
vue de ses sources sous couvert d'organisation serait une autre faute.

_Relire n'est pas voir._ Une relation `review` ne donne aucune visibilité. Les confondre donnerait à
tout relecteur le contexte de son relu, exactement ce que §12.4 et l'invariant 11 refusent. C'est la
faute que deux sortes rendent possible et qu'une seule rendait inconcevable.

_Un port, pas une dépendance._ `packages/review` ne connaît pas la coordination : il déclare
`Visible` et pose la seule question dont il a besoin, comme `EpistemicIndex` le fait dans l'autre
sens. Le branchement vit dans le test, et `locus-review` est une **dev-dependency** de
`locus-coordination` — donc la sorte est éprouvée contre la vraie `ContextView`, pas contre une
imitation, sans coupler les deux crates de production.

_Un seul calcul._ `ContextView::build` délègue à `build_under` avec un port sans contrainte. Un
chemin « sans visibilité » écrit à part divergerait le jour où l'un des deux est corrigé — la même
discipline que le dry-run de W14.d, et un mutant qui lui donne son propre chemin meurt.

**La garde de W13.e a fait son travail : elle a échoué.** Le test qui figeait la liste close à une
valeur est tombé à l'arrivée de `visibility`. Il liste désormais deux valeurs **avec le consommateur
qui honore chacune**, pour qu'élargir la liste sans écrire le consommateur reste ce que la décision
4 interdit.

**Prochain item.** W15.f — `SET_ROLE` comme opération attributaire, avec son lecteur dans
`canterel`.

## 2026-08-18 — W15.f — Bloqué : le rôle n'a pas de chemin jusqu'à son lecteur

**Aucun code écrit.** L'item est consigné bloqué plutôt que livré incomplet.

**Le blocage.** Le lecteur du rôle est `selectOverlay` dans `canterel`, qui ne connaît d'une mission
que ce que la `MissionEnvelope` lui livre. `schemas/lep/1.0/mission-envelope.schema.json` porte
`review_policy`, `required_capabilities` et `confidentiality_ceiling` — **aucun rôle d'agent**.
Acheminer le rôle jusqu'au worker demande donc un **mineur `lep/1.1`**.

**Pourquoi ne pas l'ouvrir ici.** ADR 0016 : « Ce mineur a son propre ADR ; W13 n'en dépend pas et
ne l'ouvre pas. » W15 hérite de cette posture, et deux arbitrages du 2026-08-17 attendent déjà le
même mineur — la permission de fonctionnement hors ligne et les codes de refus d'admission sur le
fil. Ouvrir `lep/1.1` au détour d'un item de coordination le ferait entrer sans son ADR et sans les
deux autres besoins qui le motivent.

**Pourquoi ne pas écrire `SET_ROLE` quand même.** Ce serait précisément la sémantique inerte que la
décision 4 interdit : un attribut que le système saurait versionner, différencier, approuver et
afficher, et que **rien n'honorerait**. W15.a a posé la règle en différant les quatre opérations
attributaires ; la relâcher pour celle-ci lui retirerait tout sens.

**Ce que l'item coûtera quand il sera débloqué**, vérifié en instruisant la question : `Version`
gagnera une table de rôles, donc sa forme canonique gagnera des lignes et les hashes changeront —
d'où l'étiquette `[M]`. Et pour que les inverses restent exacts, retirer, scinder ou fusionner un
nœud qui porte encore un rôle devra être **refusé**, comme pour une arête : sinon `REMOVE_NODE`
perdrait le rôle et son `AddNode` inverse ne le rendrait pas.

**W15 est clos sans lui.** W15.a à W15.e sont livrés et verts.

**Prochain item.** W16 — reconfiguration vivante et scheduler dynamique. W4.e et W4.g, dont il
dépend, sont livrés.

## 2026-08-18 — W16.a — Les commandes de cycle de vie du scheduler, et la quiescence locale

**Périmètre.** `docs/10_V1_ROADMAP.md` (décomposition de W16 en cinq items, dont deux bloqués),
`packages/coordination/src/lifecycle.rs` (neuf), `packages/coordination/tests/lifecycle.rs` (neuf,
12 tests), `src/lib.rs` (les réexports), ce fichier.

**Tests exécutés.** `cargo test -p locus-coordination` → 123 conformes. `npm run check` → les dix
portes vertes. Mutation : dix-neuf mutants, **dix-neuf tués, aucun survivant**.

**Décisions prises.**

_Quatre commandes, pas treize._ `docs/13` énumère « spawn, suspend, drain, kill, replace, split,
merge, connect, disconnect, rerouter l'état, rejouer, migrer le contexte, et livrer les messages ».
La liste mélange deux choses. `replace`, `split`, `merge`, `connect` et `disconnect` **sont déjà**
`REPLACE_NODE`, `SPLIT_NODE`, `MERGE_NODES`, `ADD_EDGE` et `REMOVE_EDGE` de W15.a : les réécrire ici
produirait un second chemin qui divergerait du premier, et personne ne saurait lequel décrit ce qui
sera commité. Le scheduler les **compose**. Les quatre dernières supposent une messagerie
inter-agents qui n'existe pas. Restent `spawn`, `suspend`, `drain`, `kill` — celles qui portent sur
l'**instance qui tourne** et non sur la structure. Un test tient les neuf autres par l'absence.

_Ce module n'est pas une seconde machine à états._ Les états restent ceux de §7.1 et
`AgentInstance::moved_to` reste seul à les porter. Ce qui est écrit ici est ce que le **scheduler**
a le droit de demander, question différente : `waiting → active` est une transition légitime de
l'instance, mais aucune commande de scheduler ne s'appelle « reprendre » — c'est le lease qui la
reprend, et inventer le verbe aurait fait un doublon de plus.

_La quiescence se constate, elle ne s'attend pas._ `docs/13` demande « quiescence locale d'un nœud
plutôt que drain global ». La quiescence est donc une **lecture** : `Quiescence::of` prend le nombre
de tentatives en vol et rend un constat. Aucune fonction n'attend, et c'est délibéré — un
`wait_for_quiescence` ferait tenir au scheduler une promesse dont il n'a pas les moyens, puisque
rien n'oblige un nœud à devenir quiescent, et l'appelant croirait que le drain finit toujours.

_Drainer ne change pas l'état tant que le nœud travaille._ `Outcome::Draining { remaining }` dit ce
qu'il reste ; un drain qui rendrait `Completed` sur un nœud encore occupé mentirait sur ce qui
tourne. Et le test qui compte est celui qui vérifie que **les voisins n'ont pas bougé** : un drain
global aurait le même effet apparent sur le nœud visé, et personne ne verrait la différence avant de
perdre le travail des autres.

_Tuer dit ce que ça coûte, même quand ça ne coûte rien._ `Outcome::Killed { abandoned }` porte le
compte y compris à zéro. C'est ce qui distingue un arrêt propre d'un arrêt coûteux, et un opérateur
qui ne lit pas la différence n'aura aucune raison de chercher le travail perdu.

_Un refus nomme les deux états._ « Interdit » sans dire d'où ni vers où ne se corrige pas ; deux
mutants qui effacent l'un ou l'autre meurent.

**La règle qui relie ce module à W15.a.** `may_leave_the_version` : un nœud ne quitte pas
l'organisation tant que son instance tourne. `REMOVE_NODE` ne détient que des identités — la version
ne peut pas savoir seule qu'une instance travaille encore — donc la question se pose ici, et le
scheduler compose les deux. Sans elle, une organisation dirait qu'un agent est parti alors qu'il
produit toujours, et le graphe institutionnel cesserait de décrire ce qui se passe. Un test parcourt
le chemin complet : drainer jusqu'à la quiescence, **puis** retirer.

**Le survivant de mutation.** Un seul, et il montrait un trou réel : je testais le **refus** de
`suspend` et jamais sa réussite. Un `suspend` qui laisserait le nœud `active` n'écarterait rien du
tour, et le scheduler aurait continué de lui donner du travail en croyant l'avoir mis de côté.

**Prochain item.** W16.b — les barrières par invariant menacé plutôt que par lieu.

## 2026-08-18 — W16.b — Les barrières par invariant menacé plutôt que par lieu

**Périmètre.** `packages/coordination/src/barrier.rs` (neuf),
`packages/coordination/tests/barrier.rs` (neuf, 9 tests), `src/lib.rs` (les réexports), ce fichier.

**Tests exécutés.** `cargo test -p locus-coordination` → 132 conformes. `npm run check` → les dix
portes vertes. Mutation : quatorze mutants, **treize tués**, un équivalent documenté.

**Décisions prises.**

_Ce que la barrière par lieu rate, dans les deux sens._ `docs/13` demande des « barrières par
invariant menacé plutôt que par lieu », et les deux tests qui comptent montrent les deux fautes que
cela évite :

- **elle bloque trop peu.** Deux reconfigurations sur des nœuds entièrement disjoints — `1 → 2` et
  `5 → 6` — menacent le même invariant. Une barrière par lieu les aurait laissées passer toutes les
  deux, et l'acyclicité serait tombée entre elles.
- **elle bloque trop.** Deux reconfigurations sur **les mêmes nœuds** — `1 → 2` deux fois — dont
  l'une ajoute une revue et l'autre une visibilité. Depuis W15.e la seconde ne menace rien : une
  barrière par lieu les aurait sérialisées sans qu'elles puissent rien se casser.

Le second test n'était pas écrivable avant W15.e : il faut deux sortes de relation pour que deux
lots au même endroit puissent différer par ce qu'ils menacent.

_La portée est dérivée, jamais déclarée._ C'est ce qui empêche la barrière par lieu de revenir
déguisée. `Barriers::raise` **calcule** ce qu'un lot met en jeu, par le `threatens` de W15.c — le
même calcul que le plafond de risque, donc les deux ne peuvent pas diverger. Écrire un second calcul
aurait produit deux vérités sur la même question.

_Une barrière ne nomme aucun lieu._ Elle porte un invariant et celui qui la tient, rien d'autre. Un
accesseur qui rendrait des identités ferait écrire, un jour, « barrer aussi ceux-là ». Un test le
tient par l'absence, sur la représentation `Debug` — et le mutant qui glisse le lot dans le champ
`held_by` meurt dessus.

_Une barrière sans invariant menacé est refusée._ Elle ne pourrait barrer que par lieu, faute
d'autre chose à nommer : elle serait exactement ce que `docs/13` écarte. Et un lot qui ne menace
rien n'a besoin d'aucune barrière — le lui refuser ne coûte rien et empêche la mauvaise habitude.

_Le refus nomme l'invariant et qui le tient._ « Cette équipe est gelée » n'apprend rien ; «
l'acyclicité de revue est tenue par alice » dit ce qu'il faut attendre et à qui demander.

**Un mutant équivalent, et pourquoi il le restera un temps.** Remplacer `retain` par `clear` dans
`release` ne change rien : `raise` refusant une seconde barrière sur un invariant déjà tenu, et
l'énumération n'en comptant qu'un, le jeu n'en tient jamais plus d'une. La distinction deviendra
observable le jour où un deuxième invariant entrera, et c'est ce jour-là qu'un test devra la tenir.
La limitation est écrite dans le module plutôt que laissée à un survivant inexpliqué.

**Prochain item.** W16.c — le plan de simulation.

## 2026-08-18 — W16.c — Le plan de simulation, et l'absence de type qui le tient

**Périmètre.** `packages/coordination/src/simulation.rs` (neuf),
`packages/coordination/tests/simulation.rs` (neuf, 10 tests), `src/lib.rs` (les réexports), ce
fichier.

**Tests exécutés.** `cargo test -p locus-coordination` → 142 conformes. `npm run check` → les dix
portes vertes. Mutation : quatorze mutants, **douze tués**, deux inexprimables (voir plus bas).

**Décisions prises.**

_Un substitut qui n'a pas la réponse le dit._ C'est la faute que ce module existe pour empêcher, et
elle est silencieuse : un défaut — chaîne vide, zéro, « inconnu » — ferait **réussir** la simulation
là où le run réel aurait échoué. Or prédire est la seule chose qu'on demande à une simulation ;
celle qui se trompe dans ce sens-là est pire qu'absente, puisqu'on s'appuie dessus. `Recorded::ask`
n'a donc aucune variante prenant un défaut : `ask_or` serait la porte par laquelle une valeur
inventée entrerait, et personne ne la verrait passer.

_Et la simulation ne conclut rien._ Une question sans réponse enregistrée rend
`Verdict::Incomplete { unanswered }` — les questions manquantes sont nommées, et aucun verdict n'est
rendu. Huitième occurrence de « pas vérifié n'est jamais réussi » dans ce dépôt.

_Le degré atteint, jamais celui qui était visé._ Le canari est facultatif, donc le cas où un plan
s'arrête avant le dernier degré est courant. `Outcome::reached` porte ce qui a réellement été fait :
un rejeu ne dit pas ce qu'un canari dirait, et rendre le degré visé ferait citer une simulation pour
ce qu'elle n'a pas fait. Le degré entre aussi dans la forme observée, de sorte que deux degrés sur
le même plan ne produisent pas le même résultat.

_Le déterminisme n'est pas une promesse, c'est une conséquence._ Le rejeu ne consulte **rien**
d'autre que le substitut : ni horloge, ni ordre d'itération d'un conteneur non ordonné, ni
environnement. Un mutant qui trie le plan au lieu de le suivre meurt, parce que l'ordre des
questions fait partie de ce qui est observé — le prétendre indifférent effacerait une dépendance.

_Un objet simulé n'existe pas comme type dans le domaine épistémique._ ADR 0016 décision 9 : « la
garantie est une absence de type, pas un champ de classification ». Un `Outcome` désigne une
**proposition** par son identifiant de décision et rien d'autre ; il ne peut pas nommer une
`RevisionId`, donc il ne peut pas être cité comme preuve à propos d'un claim. Deux tests le tiennent
par l'absence : la représentation ne contient aucun mot du domaine épistémique, et **l'échelle de
validation de §8.1 n'a aucun barreau** où faire entrer une simulation — le test nomme les cinq mots
qu'on serait tenté d'ajouter, pour que l'échec dise lequel est entré.

**Une collision de noms, résolue sans renommer.** `region`, `lifecycle` et `simulation` disent
chacun « verdict » ou « outcome » de leur propre domaine. Aplatir les trois au niveau du crate
aurait forcé à renommer celui qui perdrait le mot juste ; `simulation::Verdict` et
`simulation::Outcome` restent donc sous leur chemin de module, qui porte la distinction sans coûter
un nom.

**Deux mutants inexprimables, et pourquoi.** Faire porter à un `Outcome` un champ `evidence_class`
demande **deux** modifications — déclarer le champ et le remplir — que ce harnais applique une par
une, donc aucune des deux ne compile seule. La moitié intéressante est de toute façon refusée par le
typage : `run` ne reçoit jamais de `RevisionId`, donc aucun champ de ce type ne pourrait être
rempli. C'est plus fort qu'un test, et c'est exactement la forme que la décision 9 demandait — une
absence de type.

**Prochain item.** W16 est clos pour ce qui est faisable : W16.d attend le mineur `lep/1.1` et son
ADR, W16.e une messagerie inter-agents. Suit W17 — cockpit et orchestration de la mémoire.

## 2026-08-18 — W17.a — `packages/memory` : les sept niveaux, et la frontière canonique/projection

**Périmètre.** `docs/10_V1_ROADMAP.md` (décomposition de W17 en six items, dont un bloqué),
`packages/memory/` (crate neuf : `Cargo.toml`, `src/lib.rs`, `src/level.rs`, `tests/level.rs`, 9
tests), `Cargo.toml` de l'espace de travail, ce fichier.

**Tests exécutés.** `cargo test -p locus-memory` → 9 conformes. `npm run check` → les dix portes
vertes. Mutation : seize mutants, **seize tués, aucun survivant au premier tour**.

**Décisions prises.**

_La liste des sept est close, et pour une raison précise._ Un niveau décide de **qui peut lire**.
Une mémoire dont la portée n'est pas nommée finit par être lue par tout le monde, faute de raison de
refuser — ce n'est pas un oubli de rigueur, c'est le chemin par défaut. `Level::parse` refuse donc
`global`, `session`, `scratch`, `shared` et `public`, nommés dans le test pour que l'échec dise
lequel est entré.

_L'ordre de §16.1 est celui de la portée, et il se compare._ Le rendre comparable évite qu'un
appelant réénumère les sept pour poser une question à laquelle la liste répond déjà — et une
réénumération ailleurs finirait par diverger de celle-ci. Un test vérifie aussi que la liste est
**déjà triée**, de sorte que l'ordre déclaré et l'ordre de portée ne puissent pas se contredire.

_La frontière canonique/projection est portée par le type._ §16.1, dernière ligne : « le graphe, les
événements et les artefacts sont canoniques. Les résumés et embeddings sont des projections
régénérables. » Ce n'est pas une nuance de vocabulaire : perdre une projection coûte un recalcul,
perdre un canonique coûte la vérité institutionnelle (invariant 2). `Shelf` répond donc à la
question qu'un opérateur se pose avant une purge — ce qui se régénère, ce qui ne se régénère pas —
et un test exige que les deux listes ne se recoupent jamais. C'est le pendant de §9.1, qui pose la
même distinction depuis l'autre côté.

_Ranger n'écrase jamais._ Une clé déjà prise est refusée, et le refus dit **où** l'entrée est déjà
rangée. Écraser en silence ferait disparaître un canonique derrière une projection du même nom :
rien n'échouerait, et la source serait devenue son propre résumé. C'est exactement la forme que
prend la perte de la vérité institutionnelle.

**§16.6 n'est pas ici, et c'est délibéré.** Les cinq préventions de contamination vivent dans
`packages/review/src/contamination.rs` depuis W7.b, écrites par cas adverses. Les réécrire
produirait deux listes de cinq qui divergeraient, et la seconde aurait l'air aussi vraie que la
première. Un test tient l'absence en nommant trois des cinq.

**Sur `locusd`.** `apps/` porte `emacs`, `locus-execd` et `web` — pas le daemon. Tout ce que W17
demande de surface HTTP attend donc, et c'est W17.f. Le **domaine** de la mémoire et la discipline
du cockpit n'en dépendent pas : ce sont W17.a à W17.e.

**Prochain item.** W17.b — le retrieval hybride de §16.3.

## 2026-08-18 — W17.b — Le retrieval hybride : deux obligations tenues par la forme des types

**Périmètre.** `packages/memory/src/retrieval.rs` (neuf), `packages/memory/tests/retrieval.rs`
(neuf, 10 tests), `src/lib.rs` (les réexports), ce fichier.

**Tests exécutés.** `cargo test -p locus-memory` → 19 conformes. `npm run check` → les dix portes
vertes. Mutation : dix-neuf mutants, **dix-neuf tués, aucun survivant au premier tour**.

**Décisions prises.**

§16.3 porte les deux seules obligations en majuscules de la section, et chacune est tenue par la
forme des types plutôt que par une discipline d'appel.

_« Le ranking DOIT exposer ses facteurs. »_ `Ranking` ne se construit **pas** sans ses contributions
: le constructeur refuse une liste vide. Il n'existe donc aucun chemin par lequel un score arrive
sans dire d'où il vient — pas parce qu'on a pensé à l'interdire, mais parce que le type n'a pas
d'autre porte. Un flottant nu se compare, se trie et se cite, et personne ne peut dire pourquoi il
vaut ce qu'il vaut : c'est ainsi que l'obligation se perd sans que rien n'échoue.

Corollaire tenu par un test : un signal qui n'a pas contribué rend `None`, **jamais zéro**. « Il n'a
rien dit » et « il a dit zéro » ne sont pas la même information, et les confondre ferait lire une
absence comme un jugement.

_« Les embeddings ne peuvent pas contourner les ACL. »_ L'habilitation écarte **avant** le
classement. Un filtre appliqué après le tri dépendrait de son ordre d'exécution, et il suffirait
d'un `sort` déplacé pour qu'un document restreint sorte en tête. Ici le classement ne voit jamais
les candidats refusés : un score maximal n'a rien à contourner, il n'est pas dans la course. Le test
l'exerce avec `f64::MAX`, et le mutant qui déplace le filtre après le tri meurt.

_Ce qui est écarté est nommé, toujours._ Deux motifs — au-delà de l'habilitation, sous le budget de
contexte — et un test vérifie que **rien ne disparaît entre les deux listes** : un candidat qui ne
serait ni rendu ni écarté aurait disparu sans que personne le sache. C'est la discipline des
`redactions` de §16.2, appliquée au retrieval : une exclusion silencieuse rend deux résultats
indiscernables, celui qui n'avait rien à écarter et celui qui a tout écarté.

_Le budget tronque et le dit._ Une troncature muette se lit comme « il n'y avait que cela », et le
chercheur ne saura pas qu'il doit élargir. Un mutant qui tronque en silence meurt.

_Un résultat négatif est retrouvé comme un autre._ §16.3 en fait un **signal**, pas un filtre.
L'invariant 12 refuse qu'on supprime les résultats négatifs pour rendre le graphe propre ; les taire
au retrieval reviendrait au même, en moins visible.

_Le tri est déterministe jusqu'aux égalités_ — total décroissant, puis clé. Un résultat qui
changerait d'ordre à contenu égal ferait douter de la mémoire plutôt que du tri.

**Une redondance délibérée.** La table de rang de `Confidentiality` est écrite ici **et** dans
`packages/review/src/contamination.rs`. Le type ne la dérive pas, et un `match` recopié qui en
changerait l'ordre ne se verrait pas : les deux copies se recoupent exprès, dans l'esprit de la
double liste de coalescence du worker. Un mutant qui inverse l'ordre ici meurt.

**Prochain item.** W17.c — deux retrievals séparés, épistémique et organisationnel, sans conversion.

## 2026-08-18 — W17.c — Deux retrievals séparés, et la conversion qui n'est pas écrivable

**Périmètre.** `packages/memory/src/separated.rs` (neuf), `packages/memory/tests/separated.rs`
(neuf, 7 tests), `src/lib.rs` (les réexports), `docs/10_V1_ROADMAP.md` (correction du test de
sortie), ce fichier.

**Tests exécutés.** `cargo test -p locus-memory` → 26 conformes. `npm run check` → les dix portes
vertes. Mutation : onze mutants, **onze tués**.

**Correction du test de sortie, et pourquoi.** Il annonçait que « la septième frontière l'étend à ce
cas ». En l'écrivant, il est apparu que la frontière n'est **pas** le bon outil ici : les deux
familles vivent dans le **même crate**, donc le module qui les expose doit forcément voir les deux,
et la règle aurait exigé une exception sur la racine — une garde qui s'excepte à l'endroit exact où
la faute s'écrirait ne garde rien.

Ce qui tient la séparation est plus fort. `packages/protocol` fait du **préfixe une partie de
l'identité** — « `evt_01ARZ…` et `cmd_01ARZ…` ne sont pas le même identifiant, et le type les
empêche d'être confondus à la compilation » — et `Id::parse` refuse un préfixe étranger. Une
conversion devrait fabriquer une identité qu'elle n'a pas : ni directement, puisque les types
diffèrent, ni par un aller-retour en chaîne de caractères, puisque `rev_…` ne se relit pas comme un
`agent_…`. C'est une **impossibilité à la compilation** là où la frontière n'aurait été qu'un motif
cherché dans du texte. La garantie était déjà là, posée par W0.4.

**Décisions prises.**

_Partager le calcul, séparer les réponses._ Le moteur — habilitation, budget, classement de W17.b —
est commun. Deux moteurs divergeraient, et l'un des deux finirait par laisser passer ce que l'autre
refuse : c'est la faute que la duplication doit **éviter**, à l'opposé de celle qu'elle sert à
éviter pour les résultats. Quatre mutants le vérifient, un par borne et par côté — vérifier d'un
seul côté laisserait l'autre s'en affranchir, et c'est exactement ce que le premier tour a montré.

_Les identités reviennent d'une table, jamais d'une clé relue._ Le retour se fait par lecture d'une
table construite à l'aller. Reparser la clé aurait rendu la conversion écrivable pour de bon : il
aurait suffi de reparser avec l'autre type. Un test lit le source pour l'exiger.

_Aucun trait ne les factorise._ Un trait « ce qui peut être cherché et classé » serait la conversion
reconstruite : dès qu'un appelant écrit une fonction sur `impl Searchable`, les deux domaines se
retraversent sans qu'aucune ligne ne s'appelle « convertir ». C'est l'argument de l'ADR 0016
décision 9 pour les familles d'objection, et il vaut mot pour mot ici.

**Un défaut réel trouvé par la mutation.** Un mutant qui forçait `is_negative` à `true` a survécu —
non pas parce qu'un test manquait, mais parce que **le champ ne servait à rien** : la signature du
corpus ne permettait même pas d'exprimer un résultat négatif, et les deux retrievals l'aplatissaient
à `false`. Le signal `NegativeResults` de W17.b et l'invariant 12 étaient donc perdus une couche
au-dessus du moteur, là où plus personne ne regarde, sans qu'aucun filtre ne s'écrive. Le corpus
prend désormais des entrées nommées — `EpistemicEntry`, `OrganisationalEntry` — qui portent la
marque, et elle traverse entière jusqu'au résultat. Trois mutants la tiennent.

**Prochain item.** W17.d — déduplication non automatique et compaction.

## 2026-08-18 — W17.d — Déduplication non automatique et compaction

**Périmètre.** `packages/memory/src/dedup.rs` et `src/compaction.rs` (neufs),
`packages/memory/tests/dedup.rs` (12 tests) et `tests/compaction.rs` (8 tests), `src/lib.rs` (les
réexports), ce fichier.

**Tests exécutés.** `cargo test -p locus-memory` → 46 conformes. `npm run check` → les dix portes
vertes. Mutation : dix-neuf mutants, **dix-neuf tués**.

**Décisions prises — §16.4.**

_Les deux sortes de doublon ne se traitent pas pareil._ Un duplicata **exact** est un constat : deux
contenus de même hash sont le même contenu, et le dire n'engage personne. Un candidat **sémantique**
est une ressemblance, et la ressemblance n'est pas l'identité. `Candidate` n'expose donc aucune
méthode qui fusionne ; le seul chemin est `Resolution::decide`, qui exige confiance, provenance et
décideur. Ce n'est pas une discipline d'appel : il n'y a pas de `merge()` à ne pas appeler.

_« Distinct » est une réponse, pas une absence._ Sans cette variante, un candidat non fusionné
serait indiscernable d'un candidat jamais examiné — et quelqu'un le réexaminerait, puis un autre,
jusqu'à ce que l'un d'eux tranche dans l'autre sens. C'est la « possibilité de _mêmes mots, concepts
différents_ » que §16.4 nomme en dernière ligne, et elle ne tient que si le refus se consigne.

_Une fusion se défait par une nouvelle décision, pas par suppression._ L'originale reste et se relit
**à travers** celle qui la renverse. Un renversement qui conclurait comme l'original est refusé : il
ne renverse rien, et le consigner ferait croire à un changement. Même forme que l'ADR 0016
décision 5.

_Un groupe de doublons est trié, pas rendu dans l'ordre d'arrivée._ Le premier tour de mutation l'a
laissé passer parce que ma fixture insérait déjà les entités dans l'ordre trié — un corpus relu
autrement aurait montré un groupe différent, et deux opérateurs n'auraient pas vu le même doublon.

**Décisions prises — §16.5.**

_Une compaction signale ce qu'elle a omis._ C'est la moitié de ce qu'elle dit : un résumé qui ne
signale pas ses omissions se lit comme complet, et personne ne va chercher ce qu'il ignore avoir
perdu.

_Elle ne promeut rien._ Un objet que **personne n'a évalué** ne peut pas être consigné comme fait.
Le refus s'arrête là, délibérément : exiger davantage — une revue indépendante, une reproduction —
serait fixer un seuil que §16.5 ne fixe pas. C'est une question de politique (§20), et l'inventer
ici la mettrait hors de portée de la politique. Deux mutants qui élargissent le refus, l'un à toutes
les sortes et l'autre à tous les niveaux, meurent tous les deux.

Corollaire testé : le même objet non évalué entre **sans difficulté** sous une autre sorte. Le refus
porte sur la promotion, pas sur l'objet — l'interdire partout ferait disparaître les questions
ouvertes d'un résumé, ce qui est l'inverse du but.

_Le niveau de validation voyage à côté de la sorte, jamais fondu dedans._ C'est ce qui permet de
constater après coup qu'une compaction n'a rien promu ; sans lui il faudrait remonter à la source,
et personne ne le fait.

_Une compaction est toujours une projection._ « Peut être régénérée » n'est pas une faculté
optionnelle : c'est ce qui la range du côté des projections de §16.1 (W17.a). Il n'existe aucun
chemin par lequel elle se déclare canonique — elle deviendrait la source, et l'invariant 2 tomberait
sans qu'aucune ligne ne l'annonce.

**Une garde morte retirée.** Un mutant a montré que `is_finite()` était inatteignable devant le test
de bornes : toute comparaison avec `NaN` étant fausse, `contains` l'est aussi. La garde est retirée
et la subtilité écrite en commentaire — une garde morte finit par être lue comme la seule qui
protège, et le jour où les bornes changent, personne ne saura qu'elle ne servait à rien.

**Prochain item.** W17.e — les quatre vues du cockpit et la sélection synchronisée.

## 2026-08-18 — W17.e — Les quatre vues du cockpit, et le canvas qui ne peut pas écrire

**Périmètre.** `packages/visualization/src/cockpit.rs` (neuf),
`packages/visualization/tests/cockpit.rs` (neuf, 9 tests), `src/lib.rs` (les réexports),
`Cargo.toml` (`locus-protocol`), ce fichier.

**Tests exécutés.** `cargo test -p locus-visualization` → 48 conformes. `npm run check` → les dix
portes vertes. Mutation : douze mutants, **onze tués**, un inécrivable.

**Décisions prises.**

_Il n'y a pas quatre sélections qu'on synchronise, il y en a une que quatre vues lisent._ C'est la
décision qui porte tout l'item. Quatre états plus un mécanisme de notification dérivent dès qu'un
chemin oublie de notifier — et la dérive est **silencieuse**, puisque chaque vue reste cohérente
avec elle-même : un opérateur lirait la trace d'un agent en croyant lire celle qu'il a sélectionnée
dans le plan. `Cockpit` ne détient qu'un champ, donc il n'existe aucun chemin par lequel deux vues
divergent — il n'y a rien à faire diverger.

Le test parcourt les quatre origines × les quatre vues, seize lectures, parce que la faute que la
forme prévient serait asymétrique et qu'un seul couple ne la montrerait pas.

_L'origine se conserve sans rien décider._ Savoir qu'une sélection vient de la trace plutôt que du
plan aide à relire une session ; le mutant qui l'efface meurt. Mais elle ne change pas ce que les
autres vues montrent, et le mutant qui la fait décider meurt aussi.

_Le canvas produit une demande, jamais une écriture._ `Requested` nomme un verbe et un sujet, et
n'expose rien qui l'applique — même forme que la `Simulation` de W14.d et l'`Acceptance` de W15.c.
Un geste qui écrirait ferait du canvas un chemin de mutation parallèle à la command API : sans
approbation, sans trace et sans `expected_revision`.

_Le verbe reste opaque, délibérément._ Ce que les verbes signifient appartient à la command API
(§22), pas au canvas. Les énumérer ici ferait du cockpit l'endroit où l'on décide de ce qui peut
être demandé, alors que c'est l'endroit où l'on demande.

**Un test qui s'est retourné contre sa propre explication.** La vérification par l'absence lisait le
source et refusait le mot `expected_revision` — qui apparaît dans le commentaire **expliquant
pourquoi il est absent**. Un test qui confond le code et la prose interdit d'écrire la raison. Il
lit désormais le code seul, commentaires retirés.

**Un mutant inécrivable.** « L'agent sélectionné n'est pas celui désigné » n'a pas de mutant :
`Selection` porte un champ d'agent et un accesseur qui le rend, sans branche où se tromper. Comme
pour « rejouer le vide » en W16.c, la propriété découle de l'absence de choix.

**Prochain item.** W17 est clos pour ce qui est faisable : W17.f attend `locusd`. Suit W18 —
adaptation automatique et admission de capacité.

---

## 2026-08-18 — W18 décomposée, et W18.a — les onze déclencheurs, la borne, et le silence qui n'est pas un accord

**Périmètre.** `docs/10_V1_ROADMAP.md` (décomposition de W18 en six items), `packages/adaptation`
(crate neuf : `Cargo.toml`, `src/lib.rs`, `src/spawn.rs`, `tests/spawn.rs`), `Cargo.toml` racine.

**Tests exécutés.** `cargo test -p locus-adaptation` → 19 conformes. `npm run check` → les dix
portes vertes. Mutation : quatorze mutants, **quatorze tués**.

### La décomposition

Deux moitiés de la prose de W18 étaient **déjà livrées** et ne sont pas redemandées : les
indicateurs de §13.2 vivent dans `portfolio::Indicators` depuis W14 — dix des quinze, le module
disant pourquoi les cinq autres appartiennent à §13.3 — et l'anti-gaming de §13.6 dans
`portfolio::gaming`. C'est ce dernier que l'ADR 0016 décision 8 pose en condition du mode `bounded`,
avec W16 ; les deux sont satisfaites, donc `bounded` s'ouvre en W18.c au lieu d'attendre.

L'admission de capacité se coupe en deux, et c'est l'ADR qui dit où : « Locusolus possède déjà le
blueprint, l'artefact, l'attestation et le refus nommant toutes ses conditions. Ce qui manque est la
proposition, la politique et l'approbation : du travail de gouvernance. » Ce travail-là ne demande
pas d'hôte et devient W18.d. Ce qui attend `S3`/`S4` attesté est l'admission **exercée de bout en
bout contre un hôte réel** — W18.f, bloqué pour la raison exacte de W5.f.

### Décisions prises

_Un crate séparé, parce que c'est la séparation des deux boucles._ La boucle rapide change la
capacité d'un agent, la lente change la structure de l'organisation. Loger la rapide dans
`packages/coordination` lui donnerait le vocabulaire de la lente, et rien n'empêcherait ensuite un
routage de modèle de s'écrire comme une opération de graphe. `packages/adaptation` dépend de
`coordination`, jamais l'inverse.

_`reason` est l'un des onze, pas une phrase._ La prose dit mieux pourquoi _maintenant_, sur _cette_
branche — et c'est exactement pourquoi elle ne convient pas : un `reason` libre laisse entrer un
douzième déclencheur que personne n'a déclaré, et la liste de §14.5 cesse de décrire ce que le
système fait. Le _maintenant_ a sa place dans les faits que le moteur évalue et dans la trace qu'il
produit.

_Les neuf clés sont un `Draft`, pas neuf arguments._ Rust exige qu'une structure soit construite
d'un coup : il n'existe pas de `Draft` partiel, pas de `Default`, pas de `with_*`. « Une proposition
à qui il manque un champ n'existe pas » est donc vrai **avant** la validation, qui ne rattrape que
ce qui a été écrit. Un test lit la déclaration et compare les neuf noms de champs au bloc YAML de
§14.5.

_La borne de §14.5 est un chemin de types, pas une discipline d'appel._ « Aucun agent ne crée
librement une flotte non bornée. » Un agent qui observe `high_uncertainty` a par construction une
raison de vouloir un agent de plus, et rien dans sa situation ne le pousse à s'arrêter. Une
`SpawnProposal` ne sait donc pas fabriquer d'agent ; seul un `Admitted` le sait, et le seul
producteur d'`Admitted` est `dispose`, qui exige un verdict de moteur. Même forme que la
`Simulation` de W14.d, l'`Acceptance` de W15.c et le `Requested` de W17.e.

_Le silence n'est pas un accord, et c'est le cas qui fait la phrase._ Un spawn qu'aucune règle ne
couvre est précisément la flotte libre. `policy::Outcome::NoRule` était déjà écrit « distinct
d'`allow` » ; ici il devient `Undecided::Silent`, qui n'admet rien. Trois façons de ne pas répondre
sont distinguées — conflit, silence, tâches préalables — parce qu'elles se réparent différemment :
un conflit se tranche en écrivant une priorité, un silence en écrivant une règle, des tâches en les
menant.

_Le cinquième verbe de §20.2 n'est pas une cinquième réponse de §14.5._ `require_tasks` existe au
moteur et §14.5 n'en fait pas une réponse à un spawn. Le lire comme une admission serait la faute la
plus discrète du module ; il vaut `Undecided::TasksFirst`.

_Ce que le moteur a le droit de savoir, et ce qu'il ne doit pas savoir._ `facts()` ne livre pas
`expected_information_gain` ni `diversity_contribution`. Ce sont des **prétentions de valeur**, et
§13.4 en fait les termes `G` et `D` d'une fonction que le portefeuille calcule lui-même ; une règle
qui s'y accrocherait laisserait le proposeur choisir son verdict en choisissant son chiffre — la
faute que `forbid_self_approval` interdit sur l'approbation, commise sur l'admission. Le coût, lui,
reste un fait : c'est une **borne**, et un proposeur qui la sous-estime ne gagne rien, l'invariant 6
réservant les ressources avant l'exécution.

_Les deux politiques sont des références, jamais inlinées._ Les laisser s'écrire dans la proposition
ferait rédiger au proposeur les règles qui jugeront sa descendance : `forbid_self_approval`
contourné d'une génération.

_`Modified` ne porte pas la proposition réécrite._ Le moteur impose une contrainte, il ne réécrit
pas la proposition à la place de son auteur. La rendre déjà réécrite ferait disparaître la
différence entre ce qui a été demandé et ce qui a été concédé — et c'est cette différence qui se
conteste.

**Deux tests corrigés en cours de route.** Le premier interdisait la sous-chaîne `fn admit`, qui
attrapait l'accesseur `admitted()` : un accesseur **lit** une admission, il n'en fabrique pas. Le
test extrait désormais le bloc `impl Admitted` par appariement d'accolades et vérifie qu'il ne rend
ni ne construit de `Self`. Le second comptait les occurrences de `Admitted {` en oubliant que la
déclaration et le bloc `impl` portent la même sous-chaîne ; il les retire par leur nom plutôt qu'en
ajustant un compte, qu'un ajout ultérieur ferait « corriger » sans réfléchir.

**Un `is_finite` qui aurait été mort.** La vérification de plage refuse `NaN` toute seule : une
comparaison avec `NaN` est fausse, donc la plage n'est pas réputée contenir la valeur. Le mutant qui
ajoute `value.is_nan() ||` meurt ; un `is_finite` de plus n'aurait jamais rien refusé. C'est la
leçon de W14 appliquée avant d'écrire le garde.

**Prochain item.** W18.b — la boucle rapide sur la capacité et la boucle lente sur la structure.

---

## 2026-08-18 — W18.b — Boucle rapide sur la capacité, boucle lente sur la structure

**Périmètre.** `packages/adaptation/src/fast.rs`, `packages/adaptation/src/slow.rs`,
`packages/adaptation/src/lib.rs`, `packages/adaptation/tests/loops.rs`, `Cargo.toml` du crate.

**Tests exécutés.** `cargo test -p locus-adaptation` → 42 conformes (19 W18.a + 22 W18.b + un
doctest). `cargo clippy --workspace --all-targets -D warnings` → propre. Mutation : seize mutants,
**seize tués**.

**Décisions prises.**

_Tout expire, pas seulement les routes._ La roadmap ne qualifie d'« éphémères » que les routes. Or
elles ne sont pas le seul ajustement qui deviendrait une structure en durant : un routage de modèle
permanent est une spécialisation d'agent, une sélection de skill permanente est un rôle. Chaque
adaptation porte donc sa fenêtre, et il n'existe pas d'adaptation sans fin. C'est ce qui rend vraie
la phrase de l'item — deux adaptations rapides ne s'accumulent jamais en une structure que personne
n'a approuvée — parce qu'aucune n'est là pour l'accumulation suivante.

_La fenêtre est semi-ouverte, `[from, until)`._ Une borne haute incluse ferait se chevaucher deux
fenêtres consécutives sur exactement un instant, et à cet instant-là deux routages de modèle
seraient vivants pour le même agent. Un défaut d'une milliseconde par transition est celui qu'on ne
reproduit jamais. Deux mutants le tiennent, un par borne.

_Exclusif contre additif._ Un agent a **un** modèle et **un** budget de réessai ; un routage qui en
chevauche un autre est refusé, pas arbitré. Les départager en silence — la dernière adoptée gagne,
la plus longue gagne — ferait dépendre le modèle qui répond de l'ordre dans lequel deux ajustements
sont arrivés, que personne ne relit. Un outil, un skill et une route sont au contraire additifs : en
ouvrir un second n'invalide pas le premier. Le refus est **par agent et par sorte**, et deux tests
tiennent chacune de ces deux bornes du refus — un refus trop large empêcherait d'ajuster une flotte,
et on relâcherait la règle entière pour la contourner.

_La garantie qui porte l'item._ Trois routes successives ne font jamais une topologie à trois
arêtes. C'est la faute que la boucle rapide invite : chaque route est licite, et leur **union** est
une structure que personne n'a proposée, approuvée ni commitée. Le test balaie tous les instants de
la plage et vérifie qu'à aucun la vue ne dépasse une seule route.

_La route éphémère est dans la boucle rapide parce qu'elle expire._ C'est l'ajustement qui ressemble
le plus à une arête ; une route qui durerait **serait** une arête, et devrait alors se proposer,
s'approuver et se commiter. Sa fenêtre est tout ce qui l'en distingue, et c'est assez.

_La boucle rapide ne nomme aucun objet de la boucle lente._ Ni `Operation`, ni `Change`, ni
`Relation`, ni `Version`, ni `Proposal`, ni `locus_coordination`. Elle s'exécute sans approbation, à
la latence d'un appel de modèle ; une seule fonction qui rendrait une opération de coordination
ferait d'elle un chemin de mutation du graphe sans décision, sans trace et sans révision de base.
Les deux boucles ont la même forme dans l'esprit de qui les écrit — « ajuster quelque chose » — et
rien d'autre que l'absence de vocabulaire ne les tient séparées. La vérification symétrique existe
aussi : `slow.rs` ne connaît ni `Adaptation`, ni `Adjustment`, ni `Fast`. Une seule des deux
absences laisserait la porte de l'autre côté.

**`slow.rs` a rétréci en cours d'écriture, et c'est le résultat.** Il portait d'abord un `adapt()`
enveloppant `Proposal::write`. Clippy l'a refusé à huit arguments, et le refus avait raison pour une
raison qu'il ne connaissait pas : une deuxième signature des sept champs de W13 divergerait de la
première au premier champ ajouté, et divergerait **en silence** puisqu'elle compilerait encore. Le
module n'expose donc qu'une fonction, `justify`, et deux tests le tiennent — aucun `pub struct`,
`pub enum` ni `pub trait`, et exactement un `pub fn`. Une adaptation lente **est** une
`coordination::Proposal`, écrite par `Proposal::write`, approuvée par `approve`, commitée par
`commit` ; le test le montre de bout en bout, self-approval et base périmée compris.

_Ce que `justify` ajoute, et c'est tout._ Le déclencheur d'une adaptation automatique vient de la
liste close de §14.5, alors que `Justification` porte un `&str` ouvert — ouvert à dessein, son
commentaire de W13 le dit. Un humain justifie une proposition par ce qu'il veut ; un agent, non.

**Prochain item.** W18.c — `bounded` et `operator`, avec la classe de risque dérivée des invariants
menacés.

---

## 2026-08-18 — W18.c — `bounded` et `operator`, et la classe de risque qui ne se déclare pas

**Périmètre.** `packages/coordination/src/proposal.rs` (`Mode` passe de deux à quatre membres),
`packages/adaptation/src/bounded.rs`, `packages/adaptation/src/lib.rs`,
`packages/adaptation/tests/bounded.rs`.

**Tests exécutés.** `cargo test -p locus-adaptation` → 60 conformes (19 + 22 + 18 + un doctest).
`cargo test --workspace` → propre. `cargo clippy --workspace --all-targets -D warnings` → propre.
Mutation : seize mutants, **seize tués**.

**Décisions prises.**

_Les quatre modes ne sont pas une échelle._ L'ADR 0016 décision 8 les présente dans un tableau, et
un tableau se lit de haut en bas ; la tentation est de leur donner un rang — `Ord`, un `level()`, un
`is_at_least()` — et c'est exactement l'échelle d'autorité à barreaux que `CLAUDE.md` interdit
nommément. `operator` en est la réfutation : c'est le mode le **plus** privilégié et celui qui
permet à un agent le **moins**, rien du tout. Il décrit la session d'un humain nommé qui répare, pas
une autonomie de plus accordée à la flotte. `Mode` ne dérive donc ni `PartialOrd` ni `Ord`, n'expose
aucun rang, et un test lit le source pour le tenir.

_La classe de risque est dérivée, et il n'y a nulle part où l'écrire._ `RiskClass` n'a qu'un
constructeur, `of(&Diff)`, qui unit ce que `region::threatens` attribue à chaque opération. Ni
`new`, ni champ public, ni `From`. C'est la seule forme qui tienne : la classe décide de ce qu'un
agent peut committer sans humain, et la laisser déclarer reviendrait à lui laisser choisir son
propre plafond. L'union, pas le maximum — deux opérations qui menacent chacune un invariant
différent en menacent deux ensemble, et un maximum en cacherait un.

_Le plafond est un ensemble, pas un compte._ `Region` a déjà un `risk_ceiling`, et c'est un nombre —
`docs/13` le définit ainsi et il reste ce qu'il est. Mais son refus dit « risque 1 pour un plafond
de 0 », ce qui ne renseigne personne, et sous `bounded` ce refus est la **seule** chose qu'un humain
lira jamais de la décision. Le plafond de ce module nomme donc les invariants qu'un agent a le droit
de menacer, et le refus les nomme aussi. C'est la règle de W16.b un cran plus loin : le refus nomme
l'invariant, pas le lieu, et pas davantage le compte. Un test vérifie que le message ne contient
aucun chiffre.

_`bounded` retire l'approbation, il ne la confie pas au proposeur._ Rien dans ce module ne produit
d'`Approved` — cette valeur enregistre le jugement d'une personne, et en fabriquer une au nom d'un
agent mettrait un nom sur un jugement que personne n'a porté. `forbid_self_approval` reste donc vrai
sans qu'on ait à le vérifier ici : il n'y a pas d'approbation à détourner. Un test refuse `Approved`
et `fn approve` dans le source ; il ne refuse **pas** le mot `Approval`, parce que lire le mode
d'approbation qu'une région déclare n'est pas produire une approbation.

_Qui approuve, quand personne n'approuve._ L'ADR donne le mécanisme : « `allow` dans les bornes de
`scope` et `budget_ceiling` ». Le `allow` du moteur tient la place que `ApprovalMode::Peer` réserve
à « n'importe qui d'autre que l'auteur », et il la tient sans effort, n'étant l'auteur de rien. Ce
module ne réévalue aucune règle : il lit l'`Outcome` rendu, avec sa trace. Une politique de
`bounded` écrite ici serait une deuxième politique, invisible du moteur et non tracée.

_Un mode ne surclasse pas un périmètre._ Une région qui déclare `ApprovalMode::Human` a dit, dans
son propre périmètre, qu'une personne doit regarder ; `bounded` ne le lève pas. `require_shadow` non
plus. Les six conditions d'`autonomously` sont donc, dans l'ordre : le mode, le `allow` du moteur,
le verdict de la région, l'approbation humaine, l'ombre, et enfin la classe de risque.

_Le veto global et le plafond sont deux refus distincts._ Là un invariant est **rompu** par un
chemin passant hors de la région ; ici il est seulement **menacé** et le plafond ne le tolère pas.
Les confondre ferait chercher un plafond mal réglé quand c'est la cohérence globale qui a mordu. Un
test exerce le veto avec un plafond large exprès.

_`Ceiling::untouchable()` est le défaut._ Élargir un plafond est une décision qui se lit dans un
diff ; ne pas l'avoir resserré ne se lit nulle part.

**Deux tests corrigés en cours de route.** Le premier interdisait la sous-chaîne `Approval`, qui
attrape `ApprovalMode` et `HumanApprovalRequired` — deux lectures, pas une production. Le second
cherchait `review_acyclicity` là où le slug de W15.c s'écrit `review-acyclicity` ; l'assertion «
aucun chiffre dans le message » est restée, et c'est elle qui porte la garantie.

**Prochain item.** W18.d — l'admission de capacité comme gouvernance.

---

## 2026-08-18 — W18.d — L'admission de capacité comme gouvernance

**Périmètre.** `packages/adaptation/src/admission.rs`, `packages/adaptation/src/lib.rs`,
`packages/adaptation/tests/admission.rs`, `packages/adaptation/Cargo.toml`.

**Tests exécutés.** `cargo test -p locus-adaptation` → 71 conformes. `npm run check` → les dix
portes vertes. Mutation : onze mutants, **onze tués**.

**Ce que l'item n'a pas fait, et c'est le point.** L'ADR 0016 décision 8 dit exactement ce qui
manquait : « Locusolus possède déjà le blueprint, l'artefact, l'attestation et le refus nommant
toutes ses conditions. Ce qui manque est la proposition, la politique et l'approbation : du travail
de gouvernance. » Ce module ne construit aucune image, ne scanne rien, ne signe rien. Refaire un
maillon de la chaîne de W5.b ici en aurait fait un deuxième chemin, plus court — donc celui qu'on
prend.

**Décisions prises.**

_Une capacité n'entre que par un `Published`._ `admit` en exige un, et cette valeur « est la preuve
que les six étapes ont eu lieu, dans l'ordre : elle ne se construit pas autrement ». Il n'existe
donc aucun argument par lequel une capacité entrerait sans lockfile, sans SBOM, sans scan, sans
tests et sans signature — non parce qu'on les vérifie ici, mais parce que la valeur exigée ne peut
pas exister sans eux.

_L'extension est un axe orthogonal, pas un cinquième barreau._ L'ADR : « un déploiement peut être en
`bounded` sur la coordination et interdire toute capacité nouvelle, ou l'inverse ». `Extension` est
donc son propre interrupteur et non une valeur de `Mode`. Le ranger dans `Mode` aurait fait de
l'extension une conséquence de l'autonomie de coordination, alors que les deux se décident
séparément et n'ont pas les mêmes conséquences quand elles se trompent. Le défaut est `Forbidden`,
et un mutant qui déplace le `#[default]` meurt.

_« Démontré » est la condition qui manquait au raisonnement._ `Published` garantit déjà que
**toutes** les vérifications de santé sont passées — `tested` refuse un échec, et séparément une
vérification qu'on n'a pas su lancer. La question qui reste est _laquelle_ a été faite : la capacité
doit être **nommée** par l'une d'elles. Une capacité qu'aucune vérification ne nomme n'a pas été
démontrée par cette image, et la laisser passer parce que l'image est signée confondrait la
**provenance** avec l'**aptitude**. La comparaison est exacte : un rapprochement par préfixe
admettrait `sparql-write` sur la foi d'une vérification nommée `sparql`, et un test exerce les deux
sens du préfixe.

_Le refus dit ce que l'image a effectivement démontré._ Un refus qui dirait « non » enverrait relire
quatre politiques à la main, et « l'image ne l'a pas démontrée » n'appelle pas du tout la même suite
que « le déploiement n'admet aucune capacité nouvelle ». Un test vérifie que les quatre refus
produisent quatre phrases distinctes.

_Le demandeur n'approuve pas sa propre extension._ `forbid_self_approval` est l'une des trois bornes
que l'ADR déclare inrelâchables ; ici elle empêche un agent d'élargir seul ce qu'il a le droit de
faire, ce qui est la forme la plus directe du problème de l'agent auto-modifiant.

_Du code injecté n'est pas une valeur exprimable._ Ce que la littérature appelle « système de
plugins » fait circuler du **code** qu'un processus charge ; une admission fait circuler un **digest
d'image** que `locus-execd` fait tourner sous sandbox. Aucun type du module ne porte de source, de
script, de chemin de bibliothèque ni d'expression à évaluer. Ce n'est pas une garantie partielle :
il n'y a pas de champ à remplir.

_L'admission ne duplique pas la preuve._ Elle porte le digest et la clé de signature, pas le
`Published` entier : ce qui suit est une mission, et une mission a besoin de savoir quelle image
lancer, pas de relire le SBOM. Un second dépôt de la preuve divergerait du premier. Un test lit la
déclaration de la structure et refuse `Published`, `Sbom`, `Lockfile`, `EnvironmentBlueprint`.

**Prochain item.** W18.e — la métrique d'acceptation : taux d'annulation humaine des adaptations
agentiques.

---

## 2026-08-18 — W18.e — La métrique d'acceptation, et le silence qui n'est pas un accord

**Périmètre.** `packages/adaptation/src/acceptance.rs`, `packages/adaptation/src/lib.rs`,
`packages/adaptation/tests/acceptance.rs`.

**Tests exécutés.** `cargo test -p locus-adaptation` → 82 conformes. `npm run check` → les dix
portes vertes. Mutation : treize mutants, **treize tués**.

**Pourquoi cette métrique-là.** Un système qui s'adapte tout seul se juge mal de l'intérieur. Le
nombre d'adaptations produites mesure l'activité, pas l'utilité — et §13.6 range précisément « la
production de tâches pour maximiser l'activité » parmi les sept formes de gaming. Le taux
d'annulation **humaine** est le contraire : il ne peut monter que si quelqu'un a regardé et n'a pas
voulu, et aucun agent ne peut l'améliorer en travaillant davantage.

**Décisions prises.**

_Le silence n'est pas un accord, et c'est toute la difficulté du calcul._ `annulées / total` compte
au dénominateur les adaptations que personne n'a regardées, donc les compte comme acceptées. Un
déploiement que plus personne ne surveille verrait son taux tomber vers zéro et lirait cette chute
comme une réussite — au moment exact où il perd son seuil humain. Une adaptation non regardée est
donc **hors mesure** : ni au numérateur, ni au dénominateur, comptée à part et rendue visible.

_Sans aucun prononcé, il n'y a pas de taux — pas un taux nul._ `ratio()` rend `None`. Rendre `0.0`
se lirait « aucun humain n'a jamais annulé », donc une acceptation parfaite, tirée de zéro
observation. C'est la même règle que partout ailleurs dans ce dépôt : « pas vérifié » n'est jamais «
réussi ».

_Trois choses ne sont pas une annulation humaine._ Une annulation **par le système** — fenêtre
expirée, budget épuisé, rollback automatique — ne dit rien de ce qu'un humain aurait voulu ; un test
vérifie qu'en ajouter trois ne change **pas** le taux, par égalité stricte des deux ratios. Une
adaptation **d'auteur humain** n'est pas agentique, et la mesurer ferait varier le score du système
avec ce que ses opérateurs font eux-mêmes. Et la troisième est le silence ci-dessus.

_Le taux garde ses deux entiers._ `1/2` et `500/1000` valent le même nombre et ne sont pas la même
preuve. `Ratio` porte le numérateur et le dénominateur ; le flottant est calculé à la demande et
jamais stocké, parce qu'un flottant conservé se recopie dans un rapport sans ses deux entiers et
qu'on ne sait plus sur combien il porte. Un test compare `1/2` et `500/1000` : même `value()`,
`Ratio` différents.

_Les deux boucles se mesurent séparément._ Une adaptation rapide expire d'elle-même et se corrige en
attendant ; une adaptation lente entre dans l'histoire de l'organisation. Les additionner ferait
disparaître le second signal dans le premier, qui est bien plus nombreux — le test le montre avec
trois adaptations rapides gardées contre une lente annulée.

_Combien de personnes portent la mesure._ Cent adaptations toutes jugées par la même personne ne
sont pas cent observations. `reviewers` rend l'ensemble des principaux qui se sont prononcés — pour
le savoir, pas pour noter qui que ce soit : §14.6 dit que la réputation « ne doit pas devenir un
score social unique ».

**Prochain item.** W18 est clos pour ce qui est faisable — W18.f attend un hôte capable d'attester
`S3`/`S4`, comme W5.f. Restent les items de recherche R1 à R6, sans dépendance de chemin critique.

---

## 2026-08-18 — R1 — Le consensus circulaire, lu sur le graphe

**Périmètre.** `packages/graph/src/consensus.rs`, `packages/graph/src/graph.rs`
(`relations_of_kind`), `packages/graph/src/lib.rs`, `packages/graph/tests/consensus.rs`.

**Tests exécutés.** `cargo test -p locus-graph` → 17 conformes pour cet item. `npm run check` → les
dix portes vertes. Mutation : dix mutants, **dix tués** — après en avoir laissé un survivre, voir
plus bas.

**Ce que W7.b couvrait déjà, et ce qui manquait.** `packages/review/src/contamination.rs` détecte le
consensus circulaire **dans un contexte qu'on s'apprête à livrer** : la question y est « ce dossier
peut-il partir vers ce destinataire ». Ce module pose la même question du graphe lui-même : _quelles
parties de ce qui est cru ne tiennent que sur elles-mêmes ?_ — une propriété permanente du dossier
institutionnel, qu'on interroge sans destinataire et sans livraison en cours. Trois différences en
découlent.

_L'ancrage est dérivé, pas déclaré._ Il n'y a pas de booléen `is_external_source` : il y a des
arêtes `AnchoredIn`, et l'ancrage se lit dans le graphe. Un drapeau qu'un appelant pose est un
drapeau qu'un appelant peut oublier de poser — ou poser à tort sur ce qu'il vient d'écrire.

_Le résultat nomme le groupe, pas chacun de ses membres._ Un cycle de cinq est **un** problème ; le
rapporter cinq fois donne cinq fois la même chose à corriger et fait paraître un petit graphe malade
cinq fois plus qu'il ne l'est. La détection passe donc par les composantes fortement connexes du
sous-graphe `Cites`.

_Un ancrage interne n'est pas un ancrage._ Si les membres d'un cycle s'ancrent les uns dans les
autres, le groupe ne s'appuie sur rien de plus qu'avant. C'est la règle de W15.c sur « un chemin
passant hors de la région », transposée : ce qui compte est ce qui **sort**. Un compte
d'`AnchoredIn` — « ce groupe a trois ancrages » — laisserait exactement ce cas passer. Et le constat
dit **lesquels** sont internes, pour répondre à l'objection qu'il appelle : « mais nous avons des
ancrages ».

**Décisions prises.**

_Un cycle ancré n'est pas un consensus circulaire, et il est quand même rendu._ `citation_cycles` et
`circular_consensus` sont deux fonctions. Deux travaux qui se citent mutuellement et tiennent tous
deux à une source extérieure s'appuient sur quelque chose ; les confondre ferait signaler la moitié
d'une bibliographie.

_Un seul membre ancré suffit pour tout le groupe._ Exiger que chacun s'ancre ferait du constat une
exigence de forme bibliographique, alors que §16.6 vise l'absence de fondation.

_L'auto-citation compte._ C'est le plus petit cycle possible et le plus facile à écrire par accident
; une détection qui ne regarderait que les composantes de taille deux le manquerait entièrement.

_Le module ne supprime rien et ne fait descendre aucun niveau._ L'invariant 12 interdit d'effacer un
conflit pour rendre le graphe propre, et un consensus circulaire **est** un constat, pas une faute
prouvée : deux résultats indépendants peuvent se citer en rond sans qu'on ait pensé à écrire les
ancrages.

_Tarjan itératif, pas récursif._ La profondeur de pile suivrait la profondeur du graphe, et une
détection qui déborderait la pile sur un grand dossier serait absente exactement quand elle sert. Un
test la passe sur un cycle de 201 membres.

**Un mutant a survécu, et il avait raison.** Remplacer `on_stack.contains(&successor)` par
`!on_stack.is_empty()` passait toute la suite. Une arête vers une révision déjà visitée est de deux
sortes : un retour dans le groupe qu'on ferme, ou un renvoi vers un groupe **déjà clos**. Les
confondre ne signale aucune erreur — cela fait simplement **disparaître** le second groupe du
rapport, et un consensus circulaire non rapporté se lit comme un graphe sain. Aucun test ne
construisait un cycle citant un autre cycle déjà fermé ; il en existe un maintenant, et le mutant
meurt.

**Prochain item.** `R2` à `R6`, ou les items bloqués si leur condition se lève.

---

## 2026-08-18 — R2 — Le crédit structurel, et le hasard qui est une issue nommée

**Périmètre.** `packages/evaluation/src/credit.rs`, `packages/evaluation/src/lib.rs`,
`packages/evaluation/tests/credit.rs`.

**Tests exécutés.** `cargo test -p locus-evaluation` → 16 conformes pour cet item. `npm run check` →
les dix portes vertes. Mutation : quinze mutants, **quinze tués**.

**Décisions prises.**

_Le hasard d'échantillonnage est une issue nommée, jamais un reste._ C'est la phrase entière de
l'item. Une attribution qui rend toujours l'un des trois facteurs donne une histoire à chaque
fluctuation : on a changé quelque chose, la mesure a bougé, donc le changement a marché. Rien dans
ce raisonnement ne distingue une amélioration d'un tirage favorable — et un système qui l'applique
en boucle garde tous ses changements, dont la moitié n'a rien fait. `Credit::SamplingNoise` porte
donc l'écart **et** la bande : elle ne dit pas « on ne sait pas », elle dit « voici de combien la
même configuration varie toute seule, et votre écart est dedans ».

_Le hasard n'est pas un quatrième facteur._ `Factor` en compte trois — relation, rôle, budget —
parce qu'on ne **change** pas le hasard, on le mesure. Le ranger dans l'énumération donnerait un
quatrième bouton à tourner, qui n'existe pas. Un test lit la déclaration et refuse `Noise`,
`Sampling`, `Random`, `Unknown`, `Other`.

_La bande se mesure, elle ne se suppose pas._ `Baseline::from_replays` exige au moins deux rejeux de
la **même** configuration. Il n'existe ni bande par défaut, ni `Default`, ni seuil constant — un
test lit le source et refuse `const BAND`, `const THRESHOLD`, `const EPSILON`. Une bande inventée
ferait passer pour du bruit ce qui n'en est pas, ou l'inverse, selon un chiffre que personne n'a
mesuré.

_La bande est l'étendue, pas un écart-type._ Trois rejeux ne renseignent pas un écart-type, et en
calculer un donnerait à trois mesures l'apparence d'une distribution. Le nombre de rejeux voyage
avec la bande : une bande tirée de deux et une bande tirée de deux cents ne se lisent pas pareil.

_Deux facteurs changés n'attribuent rien, et le refus les nomme._ La suite est d'aller mesurer
chacun séparément, et un « non attribuable » sans liste envoie tout remesurer. Le refus tombe
**avant** le calcul de l'écart : un gros écart n'excuse pas la confusion, et un test l'exerce à
mille.

_Deux bras identiques ne sont pas du bruit._ Rendre `SamplingNoise` ferait croire qu'un facteur a
été mis à l'épreuve et n'a rien donné, alors qu'aucun ne l'a été. C'est la distinction de W18.e
entre « non regardé » et « regardé et gardé », dans un autre domaine.

_Une régression s'attribue comme une amélioration._ `Credit::Attributed` porte un écart **signé**.
L'invariant 12 interdit de supprimer les résultats négatifs pour rendre le dossier propre — et c'est
aussi le seul moyen de **défaire** un changement inutile plutôt que de l'oublier. `factor()` et
`is_improvement()` répondent donc à deux questions distinctes ; les réunir ferait disparaître les
régressions attribuées, qui ont un facteur et ne sont pas des améliorations.

**Une assertion corrigée.** Le premier test comparait `Credit::SamplingNoise { gain: 0.8, .. }` par
égalité exacte ; `10.8 - 10.0` vaut `0.8000000000000007`. L'égalité exacte de deux flottants issus
d'une soustraction est une assertion sur la représentation, pas sur le verdict. Le test compare
désormais à la tolérance près, et les autres cas emploient des écarts exactement représentables.

**Prochain item.** `R3` — évaluation structurelle et regret structurel, calculable en rejeu.

---

## 2026-08-18 — R3 — Métriques structurelles et regret structurel

**Périmètre.** `packages/coordination/src/metrics.rs`, `packages/coordination/src/lib.rs`,
`packages/coordination/tests/metrics.rs`, `packages/evaluation/src/regret.rs`,
`packages/evaluation/src/lib.rs`, `packages/evaluation/tests/regret.rs`.

**Tests exécutés.** `cargo test -p locus-coordination -p locus-evaluation` → 15 + 12 conformes pour
cet item. `npm run check` → les dix portes vertes. Mutation : dix-huit mutants, **dix-huit tués** —
après deux survivants, voir plus bas.

**Deux moitiés, deux crates.** Les métriques mesurent une `Version` : elles vivent dans
`packages/coordination`. Le regret compare des utilités nommées : il vit dans `packages/evaluation`,
sans dépendance nouvelle, et réutilise la `Baseline` de `R2`.

### La moitié structurelle

_Une métrique qui mesure une propriété déjà garantie ne dit rien._ Elle rend la même valeur sur tout
ce que le système accepte, et son passage au vert n'a jamais été en jeu. Les cinq ont donc été
choisies pour ce qu'aucun invariant ne force : couverture de revue, profondeur, concentration, revue
mutuelle, isolement de visibilité.

_Aucune ne juge._ Pas de seuil, pas de verdict, pas de note. Un seuil écrit ici deviendrait la
définition d'une bonne organisation, alors que c'est une question de politique et de portefeuille —
et qu'un chiffre écrit en Rust a l'air d'une décision prise. Un test refuse `const MIN`,
`const MAX`, `fn is_healthy`, `fn score`, `enum Verdict`.

**Une erreur de raisonnement corrigée en cours de route, et c'est la meilleure trouvaille de
l'item.** J'avais écarté la réciprocité de revue comme « métrique morte » : `A relit B` et
`B relit A` est un cycle de longueur deux, que `region` veto déjà par `ReviewAcyclicity`. Le test
écrit pour justifier cette absence a échoué, et il avait raison. **Le veto s'applique à un `diff`**
; `Version::root` ne refuse que les arêtes pendantes et les auto-relations. Une version racine porte
parfaitement l'aller-retour — c'est exactement l'état qu'aucune transition n'a gardé. La métrique
est donc la plus intéressante des cinq, et non la plus morte : la revue mutuelle est la forme à deux
du consensus circulaire de §16.6, transposée de l'épistémique à la coordination. Elle est comptée
par **paire**, jamais par arête.

Conséquence sur la profondeur : elle ne peut pas supposer l'acyclicité. Le parcours borne sa
profondeur par le nombre de nœuds, et un test l'exerce sur une version cyclique — une métrique qui
ne termine pas est pire qu'une métrique absente, elle emporte l'appelant avec elle.

### La moitié « regret »

_« Disponible », et pas « imaginable »._ Le regret se mesure contre le meilleur du **menu**.
Comparer à un optimum théorique donnerait un nombre qu'aucune décision n'aurait pu améliorer, et qui
grandirait à mesure qu'on imagine mieux. D'où la borne : le choisi doit être **parmi** les
candidats, et le calcul refuse sinon — un regret contre un menu dont on n'a rien pris ne veut rien
dire, et rendrait quand même un nombre.

_« Sur fixtures identiques » est une condition, pas une recommandation._ Deux utilités mesurées sur
deux fixtures comparent les fixtures autant que les structures. Chaque candidat porte le nom de sa
fixture, et le lot est refusé s'il n'en partage pas une seule — en les nommant. Sans cela, le regret
est un nombre qu'on peut faire baisser en changeant de fixture.

_Deux mesures d'une même structure sont des rejeux, pas deux candidats._ Ce sont une `Baseline`
qu'elles font, et les compter comme deux options ferait battre une structure par elle-même.

_Les deux items de recherche se tiennent._ `Regret::exceeds` confronte l'écart à la bande de `R2` :
un regret qui tient dans la bande n'est pas un regret, et le poursuivre ferait changer
d'organisation pour suivre le tirage.

**Deux mutants ont survécu, et les deux avaient raison.**

Le premier échangeait arêtes sortantes et entrantes dans l'isolement de visibilité. La fixture
d'origine était symétrique — avec la seule arête `1 → 2`, deux membres ne voient personne et deux
membres ne sont vus de personne, donc le même compte. « Ne voir personne » et « n'être vu de
personne » sont deux situations opposées ; il fallait une fixture asymétrique pour que la différence
se voie. Elle existe.

Le second retirait le `.max(0.0)` du regret, et **passait** — parce que la garde était morte. Le pli
part du choisi et ne le remplace que par strictement mieux : l'écart est positif ou nul par
construction. Le `.max` a été retiré plutôt que couvert par un test. C'est la leçon de W14, et sa
deuxième application dans ce chantier après le `is_finite` de W18.a.

**Prochain item.** `R4` à `R6`, ou les items bloqués si leur condition se lève.

---

## 2026-08-18 — R4 — Le substitut d'environnement : unilatéral en rejet, et une fidélité inconnue

**Périmètre.** `packages/evaluation/src/counterfactual.rs`, `packages/evaluation/src/lib.rs`,
`packages/evaluation/tests/counterfactual.rs`.

**Tests exécutés.** `cargo test -p locus-evaluation` → 15 conformes pour cet item. `npm run check` →
les dix portes vertes. Mutation : treize mutants, **treize tués**.

**Ce que W16.c avait déjà, et ce que celui-ci ajoute.** `coordination::simulation` porte les quatre
fidélités du plan de simulation et le substitut d'environnement qui **dit** ne pas savoir plutôt que
d'inventer une réponse. Il regarde une reconfiguration. Celui-ci regarde une **trajectoire** — la
suite de pas d'une mission —, et il répond à une autre question : ce changement aurait-il fait une
différence.

**Décisions prises.**

_Unilatéral en rejet, et c'est un chemin de types._ `Outcome` a deux variantes et une seule conclut
: `Refuted`. L'autre s'appelle `NotRefuted`, **pas** « confirmé ». Deux trajectoires qui coïncident
sur une graine et un préfixe donnés peuvent parfaitement diverger sur la suivante, et rien dans la
comparaison ne dit le contraire. Un `is_confirmed()` ferait de l'absence de contre-exemple une
preuve, ce que `R4` interdit en toutes lettres — « jamais un juge, jamais une preuve ». Un test lit
le source et refuse `Confirmed`, `Validated`, `Proven`, `fn proves`, `fn judge`, `struct Proof`,
`fn accept`. Un autre compte les variantes de l'énumération.

_Le non-rejet porte le nombre de pas comparés._ « Non réfuté sur trois pas » et « non réfuté sur
trois mille » ne sont pas la même chose ; un verdict qui tairait la différence les rendrait
interchangeables. Troisième occurrence de la même forme dans ce bloc, après le nombre de rejeux
d'une `Baseline` et le nombre de candidats d'un `Regret`.

_Graine et préfixe identiques sont une condition, pas une recommandation._ Deux trajectoires qui ne
partagent ni le tirage ni le début ne se comparent pas : leur divergence s'explique par tout, donc
par rien. La graine est vérifiée **avant** le préfixe, parce que refixer la graine vient d'abord. Et
un préfixe qui est le **début** de l'autre est refusé aussi : « l'un continue » n'est pas « les deux
partent du même endroit », et le pas de plus peut expliquer toute la suite.

_Zéro pas comparé se dit._ Deux suites vides rendent `NotRefuted { compared: 0 }`. Zéro n'est pas
une absence de comparaison : c'est une comparaison qui n'a rien pu regarder, et le dire évite qu'un
rapport la confonde avec un accord.

_La fidélité est inconnue, et le type le dit._ La roadmap est explicite. Il n'existe donc **aucun**
moyen d'exprimer une fidélité mesurée : pas d'énumération à deux variantes dont une serait vide — ce
serait la sémantique inerte que l'ADR 0016 décision 4 interdit —, pas de `f64` par défaut.
`fidelity` rend un `Unmeasured`, et c'est le seul type qu'elle sait rendre. Le jour où quelqu'un
mesure, le type change et **tous** les appelants sont forcés de regarder ; un champ qui attendait
déjà la valeur les en aurait dispensés.

**Prochain item.** `R5` — prototype externe de harnais tiers — ou `R6`.

---

## 2026-08-18 — R6 — L'évolution inter-exécutions, et R5 qui ne se fait pas ici

**Périmètre.** `packages/evaluation/src/evolution.rs`, `packages/evaluation/src/lib.rs`,
`packages/evaluation/tests/evolution.rs`, `docs/10_V1_ROADMAP.md` (note sur `R5`).

**Tests exécutés.** `cargo test -p locus-evaluation` → 15 conformes pour cet item. `npm run check` →
les dix portes vertes. Mutation : douze mutants, **douze tués**.

### `R6`

« Une adaptation **récurrente** et **gagnante en validation appariée** propose une amélioration de
template. » Trois mots, trois bornes.

_Récurrente._ Vue dans plusieurs exécutions **distinctes**, et le seuil ne peut pas descendre sous
deux — accepter un seuil de un ferait promouvoir le tirage d'une observation unique. La même
exécution consignée trois fois reste une exécution : un mutant qui les distingue par leur rang
meurt.

_Gagnante en validation appariée._ Chaque occurrence est un `Credit::Attributed` de `R2`, avec un
gain positif. Ce module **ne rejuge rien** : il compte des verdicts déjà rendus. Refaire
l'attribution ici serait une seconde attribution, avec sa propre bande de bruit, qui divergerait de
la première — un test refuse `Baseline`, `fn attribute`, `band`, `utility` dans le source. Le bruit
ne compte donc pas, et un autre facteur n'est ni au numérateur ni au dénominateur.

_Propose._ Le résultat est une `Improvement`, et il n'existe aucun chemin qui l'applique. Même forme
que la boucle lente de W18.b. Elle **nomme** les exécutions plutôt que de les compter : une
proposition de template se conteste, et la contester demande de pouvoir aller relire ce qui est
cité.

_Ce qui ne se moyenne pas._ Deux exécutions qui gagnent et une qui régresse ne font pas «
globalement positif » : `Evolution::Contradictory` les rend telles quelles. Moyenner reviendrait à
supprimer un résultat négatif pour rendre le dossier lisible — invariant 12 — et à promouvoir un
template dont on sait qu'il a déjà nui une fois, sans savoir pourquoi. La contradiction l'emporte
même quand les gains dépassent largement le seuil : c'est le cas où l'oubli serait le plus tentant.

_Deux absences distinctes._ `NothingAttributed` — le facteur n'a jamais rien gagné — n'est pas
`NotRecurrent` — il gagnait, sans assez d'exécutions. Les confondre ferait attendre d'autres
exécutions d'un facteur que personne n'a vu marcher.

**Un test corrigé.** Le compte des sites de construction retirait la déclaration et le bloc `impl`,
mais pas `impl fmt::Display for Improvement`. Corrigé par le préfixe plutôt qu'en ajustant le
nombre, qu'un ajout ultérieur ferait « corriger » sans réfléchir. Même correction qu'en W18.a.

### `R5` — pourquoi il ne se fait pas ici

`R5` demande un **dépôt jetable** : créé pour poser une seule question, supprimé si la réponse est
non. Créer un dépôt engage un compte au-delà de ce chantier, et le supprimer plus encore ; c'est une
décision qui appartient à qui possède le compte, pas à la session qui écrit le code. Le faire dans
`locusolus` contredirait « jetable » — le code resterait —, et dans `canterel` cela violerait deux
règles à la fois : « tout le code local vit sous `backend/cli/src/locus/**` » et l'interdiction de
`R5` lui-même.

Sa contrainte, en revanche, est tenue sans rien faire : « aucune ligne dans `backend/cli/src/locus/`
avant la réponse » est une **interdiction**, et elle est respectée tant que le worker LEP séparé
n'est pas écrit. Le constat est écrit dans `docs/10`, à côté de l'item.

**État de la roadmap.** W13 à W18 sont livrés pour ce qui est faisable, avec cinq items bloqués et
leur raison écrite : W15.f et W16.d attendent le mineur `lep/1.1` et son ADR, W16.e une messagerie
inter-agents, W17.f `locusd`, W5.f et W18.f un hôte capable d'attester une sandbox. Des six items de
recherche, cinq sont livrés et `R5` attend une décision qui n'est pas technique.

---

## 2026-08-18 — R5 — La sonde de harnais tiers, et sa réponse : oui

**Périmètre.** `docs/10_V1_ROADMAP.md` (réponse de `R5` et ouverture de `W0.9-bis`). Aucun code
produit : c'est ce que l'item demande, et la sonde a vécu hors de tout dépôt permanent.

**Le dépôt jetable.** L'intégration GitHub de cette session n'a pas le droit de créer un dépôt —
`403 Resource not accessible by integration` sur `POST /user/repos`. La sonde a donc tourné dans le
conteneur de session, qui est lui-même reclamé à l'inactivité. Ce que l'item protège est tenu : rien
n'a été laissé dans `locusolus`, `canterel`, `xiiif` ni `emacs-config`, et **aucune ligne n'a été
écrite dans `canterel/backend/cli/src/locus/` avant la réponse**.

**La méthode, et pourquoi elle a quatre passages.** Un flux qui passe les huit vérifications ne
prouve rien tant qu'on n'a pas montré que le harnais **mord** : « aucun constat » se lit alors « le
flux est conforme » ou « le harnais ne regarde pas », et c'est la première qu'on veut croire. La
règle de `R4` s'applique à la sonde elle-même. D'où :

| Passage                  | Ce qu'il apporte                                     | Constats        |
| ------------------------ | ---------------------------------------------------- | --------------- |
| **C** — contrôle négatif | une violation injectée par vérification              | **8/8 mordent** |
| **A** — plan seul        | les neuf champs du `SessionPlan`, rien d'autre       | 4               |
| **B** — plan équipé      | + séquence, horloge, clés de rejeu, identité, lease  | **0**           |
| **D** — flux incohérent  | rang faux, `worker_id` substitué, `task_id` étranger | 0               |

Le passage A n'invente rien : sans état de connexion, la séquence reste à zéro, l'horodatage à
l'époque, la clé d'idempotence constante. C'est ce que « le plan seul » veut dire, et c'est ce qui
rend ses quatre constats mesurables plutôt que rhétoriques.

**La réponse : OUI.** Les quatre constats du passage A portent tous sur des choses qu'un plan n'a
pas à porter — niveaux de sandbox et modes réseau du worker (annoncés au handshake, antérieurs au
plan), l'admission qui en découle, et l'absence de heartbeat (le plan n'a ni horloge ni intervalle
de lease). Aucun ne dit « le plan aurait dû porter ceci ». Le `SessionPlan` est donc une base
suffisante, et ce qui s'y ajoute appartient au lien et au serveur. **Conséquence, telle que l'item
la formule : un worker LEP séparé.**

**Ce que le rang d'attempt a failli être.** En lisant les types, `event.attempt` semblait la
trouvaille : le plan porte `attempt_id`, un **identifiant**, l'événement veut `attempt`, un
**rang**, et §11.1 interdit de substituer l'un à l'autre. La sonde a montré que ce n'est pas un
manque du plan — le rang vient de `Lease.attempt`, que le serveur émet. Elle a montré autre chose,
que je ne cherchais pas.

**Trouvaille annexe — un angle mort de W0.9.** Le passage D fabrique un flux dont le rang d'attempt
est faux, dont le `worker_id` est un `attempt_id` substitué et dont le `task_id` désigne une autre
tâche. **Il passe les huit vérifications.** Le harnais ne regarde aucune des trois identités. Ce
n'est pas un défaut du plan ; c'est une dette de W0.9, et c'est précisément ce qu'une sonde jetable
est bien placée pour trouver — elle a le droit de fabriquer des flux que personne n'écrirait. Ouvert
en `W0.9-bis` dans `docs/10`.

**Prochain item.** `W0.9-bis` — les trois identités que le harnais ne regarde pas.

---

## 2026-08-18 — W0.9-bis — Les trois identités que le harnais ne regardait pas

**Périmètre.** `packages/testing/src/harness.ts` (trois vérifications, de huit à onze),
`tests/testing/harness.test.ts`.

**Tests exécutés.** `node --test "tests/testing/*.test.ts"` → 24 conformes (18 + 6). `npm run check`
→ les dix portes vertes. Mutation : neuf mutants, **neuf tués**.

**D'où vient l'item.** Du passage D de la sonde `R5`, qui fabriquait un flux dont le rang d'attempt
était faux, dont le `worker_id` était un `attempt_id` substitué et dont le `task_id` désignait une
autre tâche. Il passait les huit vérifications. Une sonde jetable a le droit de fabriquer des flux
que personne n'écrirait ; c'est ce qui lui a permis de trouver ce que les tests de W0.9, écrits pour
des flux plausibles, ne pouvaient pas voir.

**Décisions prises.**

_Trois vérifications, pas une._ §11.1 : « aucune de ces identités ne doit être substituée aux
autres. » Une vérification unique qui dirait « identités incohérentes » enverrait comparer trois
paires à la main — et c'est précisément le travail que la substitution rend difficile, les trois
valeurs étant toutes des identifiants préfixés qui se ressemblent. Chaque identité a donc sa
vérification, et chaque constat **nomme** celle qui a été substituée. Deux mutants le tiennent :
l'un remplace le message par « identités incohérentes », l'autre efface la source.

_Le constat dit aussi où relire l'identité de référence._ « `attempt` vaut 99 alors que **la lease**
dit 1 » envoie au bon endroit ; « 99 ≠ 1 » laisse chercher lequel des trois documents fait foi.

_Absent n'est pas substitué._ Les trois champs sont facultatifs dans le schéma de l'événement. Un
champ absent n'est donc pas une substitution : c'est une absence, et exiger sa présence ici ferait
du harnais un vérificateur de complétude que LEP ne demande pas. Les deux fautes ne se réparent pas
pareil — l'une en corrigeant une valeur, l'autre en décidant si le champ doit devenir obligatoire,
ce qui est un mineur de protocole. Un mutant qui confond les deux meurt.

**Quatre mutants ne compilaient pas, et ce n'étaient pas des kills.** Ils comparaient le `worker_id`
à une `lease` hors de portée, ou laissaient une variable inutilisée. Un mutant qui ne compile pas ne
dit rien du test : ils ont été réécrits pour viser la même faute avec des valeurs en portée. Un
cinquième était **inerte** — il remplaçait le lecteur du rang par `event.attempt ?? lease.attempt`,
ce qui ne change rien puisque l'absent est déjà écarté. Il a été remplacé par deux mutants qui
mordent vraiment : le rang n'est plus lu, et le rang est comparé à lui-même.

**Ce que la sonde `R5` a confirmé après coup.** Rejouée contre le harnais corrigé, elle passe de 0 à
**24 constats** sur le passage D — huit événements × trois identités — et reste à **0** sur le
passage B, le flux piloté par le plan. La réponse de `R5` tient donc toujours, et l'angle mort est
fermé.

**La sonde a été supprimée**, sa réponse étant consignée. C'est ce que « dépôt jetable » veut dire.

---

## 2026-08-18 — ADR 0017 — Le mineur `lep/1.1` : ouvert une fois, livré par tranches

**Périmètre.** `docs/adr/0017-lep-1-1-le-mineur.md` (neuf) et `docs/10_V1_ROADMAP.md`. **Aucun
schéma n'est modifié**, aucun SDK régénéré, aucune fixture touchée. Cet ADR décide ce que le mineur
a le droit de faire ; il n'ajoute pas un champ.

**Ce qui l'a déclenché.** Quatre besoins attendaient le même feu vert, depuis des dates différentes
: deux nommés par l'ADR 0016 dans ses conséquences — la permission de fonctionnement hors ligne et
les codes de refus d'admission sur le fil, avec la phrase « ce mineur a son propre ADR » — un
troisième trouvé par `W15.f` en s'y cassant les dents, un quatrième par `W16.d`. Le péage d'une
ouverture de protocole est presque entièrement **fixe** : régénération du SDK dans les deux
langages, entrée de registre, fixtures, harnais. Quatre ouvertures pour quatre champs, c'est le
payer quatre fois.

**Décisions prises.**

_Le numéro une fois, les champs un par un._ Un seul `lep/1.1` pour les quatre ajouts, et aucun champ
n'entre avant que quelque chose d'exécutable et de testé le lise. C'est la décision 4 de l'ADR 0016
— « aucune sémantique inerte » — appliquée au protocole plutôt qu'à une énumération de relations. La
conséquence n'est pas confortable et elle est écrite comme telle : l'ADR ne débloque pas les quatre
items d'un coup.

_Aucun répertoire `schemas/lep/1.1/`, et ce n'est pas une interprétation._ La ligne `1.x` est
ouverte depuis `W0.5` : `protocol_version` est un motif `^lep/1\.[0-9]+$` et non un `const`, avec la
raison écrite en commentaire dans le schéma ; `schemas/README.md` pose « les documents restent
ouverts » ; et `grep -rl additionalProperties schemas/lep/1.0/` ne rend **rien**, zéro fichier sur
douze. La règle n'est pas seulement énoncée, elle est tenue. Dupliquer douze fichiers dont huit ne
changeraient pas serait la duplication de contrats que `CLAUDE.md` interdit, appliquée au protocole
contre lui-même, et sa dérive serait silencieuse.

_Le répertoire garde son nom._ `1.0` nomme la version **fondatrice de la ligne**. Le renommer en
`1.x` ne changerait rien de ce qu'un pair voit — les `$id` sont des URN, indépendantes du chemin —
tout en touchant chaque chemin de la chaîne d'outils. Le prix serait payé pour un gain nul ;
`x-since` sur chaque champ ajouté rattrape la confusion à l'endroit exact où elle se produirait.

_Un mineur ajoute des champs, jamais des valeurs._ L'interdit qui ne va pas de soi. `docs/06` écrit
« minor = champs optionnels compatibles » ; le mot **champs** se lit strictement, parce que
`packages/lep/src/generated.rs` émet des `enum` Rust **fermés, sans variante fourre-tout** —
`SandboxLevel`, `NetworkMode`, `AcceleratorType`, `Os`, `Arch`, `DataClass`, `ContainmentResult`,
`LimitResult`. Ajouter `"S6"` à `sandbox_level` ferait échouer la désérialisation chez tout
consommateur `1.0`, en silence pour l'émetteur. Ce n'est pas une préférence de style : c'est la
forme du SDK qui force la règle, et les quatre ajouts s'y conforment sans effort — trois champs
nouveaux et un document nouveau.

_Le mineur se teste, il ne se constate pas._ Deux tests le **définissent**, et le second est le plus
important. Un document `1.1` accepté par un consommateur `1.0` : la moitié facile, écrite pour que
refermer un document devienne un échec bruyant. Et un document `1.0` reçu par un consommateur `1.1`
qui laisse le champ nouveau **absent**, jamais rempli par un défaut — un `role` valant `research`
faute de mieux rendrait « l'institution n'a pas dit » indiscernable de « l'institution a dit
`research` », et c'est le second qui se croit tenu. Même règle que `SandboxLevel::parse`, qui rend
`None` plutôt qu'un niveau par défaut. Ici l'aveu s'appelle l'absence.

_Le rôle ne prend jamais le pas sur l'invariant 11._ `selectOverlay` envoie déjà toute revue
`independent` vers `reviewer` « quelles que soient les capacités demandées ». Un `role` qui pourrait
renvoyer une revue indépendante vers le profil du générateur reconstruirait exactement le trou que
ce test bouche. L'ordre est fixé dans l'ADR : politique de revue, **puis** rôle, **puis** capacités.

_Deux distinctions à ne pas perdre en traduisant les refus sur le fil._ Jamais un seul motif à la
fois — `admit` accumule et rend `Refused { reasons }` au pluriel, et un fil qui ne transmettrait que
le premier ferait corriger une condition pour retomber aussitôt sur la suivante. Et
`LevelNotAttested` n'est pas `LevelUnavailable` : « l'hôte ne sait pas faire » et « l'hôte l'annonce
sans l'avoir prouvé » envoient chercher deux choses différentes, et les fondre ferait acheter du
matériel pour un problème d'attestation.

_Ce qui reste ouvert délibérément._ Ce que l'institution voit d'un sous-agent (tranche 4). Le voir
exister et voir son contexte sont deux choses, et la seconde traverse l'invariant 11. Trancher cela
sans consommateur sous les yeux serait de la spéculation ; le sprint le tranchera avec son test de
sortie.

**Clause de falsification.** L'ADR affirme que le coût d'un mineur est **fixe** — le péage, pas le
champ. `W19.a` est le test : elle ajoute un **document** là où la tranche 1 n'ajoute qu'une
**propriété**. Si elle coûte à peu près la même chose hors rédaction du document, la décision de
grouper tient ; si elle coûte substantiellement plus **par nature**, alors quatre ajouts hétérogènes
auront été groupés sous un numéro à tort, et la décision 1 est rouverte pour les mineurs suivants.
Le constat s'écrit ici **dans un sens ou dans l'autre**.

**Ce que ça débloque, et ce que ça ne débloque pas.** `W15.f` est débloqué : c'est la tranche 1, son
lecteur — `selectOverlay` dans `canterel` — existe et est testé. `W16.d` **reste bloqué, mais le
blocage a changé de nature** : il n'attend plus une décision, il attend un consommateur. C'est un
blocage qui se lève par du travail plutôt que par un arbitrage, et le distinguer importe — le
premier attendait quelqu'un, le second attend quelque chose.

**Vérifications faites avant d'écrire, pas après.** L'ADR repose sur trois faits, tous vérifiés dans
le dépôt : aucun `additionalProperties` sous `schemas/lep/1.0/` (0 fichier sur 12) ; le motif de
`protocol_version` couvre la ligne `1.x` ; les `enum` générés en Rust sont fermés. Si le premier
avait été faux, la décision 3 l'aurait été aussi — c'est exactement le genre de chose à découvrir
avant d'empiler quatre décisions dessus.

**Roadmap.** `W19` est créé pour les deux tranches qui n'avaient pas d'item — `W19.a` les codes de
refus, `W19.b` la permission hors ligne. Les tranches 1 et 4 restent chez `W15.f` et `W16.d`.
`W19.a` avant `W19.b` parce qu'elle porte la clause de falsification, et `W15.f` avant les deux
parce qu'elle porte les deux tests qui définissent le mineur.

---

## 2026-08-18 — W5.f — L'épreuve des seize sondes contre une sandbox réelle, et la question posée à la CI

**Périmètre.** `apps/locus-execd/tests/host_sandbox.rs` (neuf), un job `sandbox` dans
`.github/workflows/ci.yml`, `docs/10_V1_ROADMAP.md`. Aucun code de production modifié : tout ce que
l'item demande existait déjà — `SUITE`, `PROBE_COMMANDS`, `run_suite`, `judge`, `standing`,
`SystemRunner`. Ce qui manquait était **un hôte**, et de quoi savoir si la CI en est un.

**Ce que les autres tests ne pouvaient pas dire.** `tests/podman.rs` pilote un `ScriptedRunner` et
l'écrit lui-même : c'est ce qui permet de vérifier les arguments et les chemins d'erreur « là où
aucun runtime rootless n'est garanti — c'est-à-dire en CI ». Un double rend ce qu'on lui a dit de
rendre. Il ne sait pas si `cpu.max` mord, si `--userns` ferme la vue sur les processus de l'hôte, ni
si le profil seccomp refuse `unshare`. Et `sh -n` vérifie une syntaxe :
`[ "${after:-0}" -eq "${before:-0}" ]` se parse parfaitement et ne prouve rien tant que personne n'a
vu `nr_throttled` bouger. Le module le disait déjà comme dette nommée — « c'est le premier travail
d'un hôte capable de S2 ».

**Décisions prises.**

_Le test est `#[ignore]`, pas conditionnel._ Un test qui se sauterait tout seul quand l'hôte ne
convient pas ressemblerait en tout point à un test qui passe — la leçon que `--require-emacs` a déjà
coûtée. `ignored` apparaît dans la sortie de `cargo test` ; « sauté en silence » n'y apparaît pas.

_La table s'imprime avant les assertions._ Un échec doit dire **laquelle** des seize n'a pas tenu et
**comment**. C'est la moitié utile d'un premier passage sur un hôte qu'on ne connaît pas : le
verdict agrégé ne se lit qu'après coup, la table se lit tout de suite.

_Trois états qui ne se réparent pas pareil._ Pas de runtime rootless, un runtime sans cgroups v2, un
confinement qui ne tient pas. La première assertion du test est donc qu'**au moins une sonde a
tourné** — sans elle, seize `NotRun` produiraient un `NotTrusted` qu'on lirait comme une faille
alors qu'il manque une machine. Le job imprime en plus, avant toute tentative, ce que l'hôte annonce
: noyau, version de podman, type de `/sys/fs/cgroup`, contrôleurs, plages `subuid`.

_`NetworkMode::Full` et non `Deny`._ `plan` refuse explicitement autre chose que `full` en deçà de
`S3` — « un processus rootless sans namespace réseau voit le réseau de l'hôte, et dire "deny"
là-dessus serait un mensonge ». Les deux sondes réseau sont donc `Allowed` à `S2` et **doivent
réussir** ; les compter comme contenues ferait de `S2` un `S3`.

_Un profil seccomp écrit par le test, dans un fichier temporaire._ `S2` exige la posture
`Restricted`, donc un profil sur disque. `tests/seccomp.rs` explique pourquoi le dépôt **ne livre
pas** de profil : « en écrire un sans hôte pour l'éprouver produirait soit une sandbox qui casse
tout, soit une sandbox qui autorise ce qu'elle prétend refuser ». Celui du test ne prétend pas être
celui-là : il est défaut-permissif et refuse nommément les huit appels de `MUST_DENY`, la posture
exacte que `RestrictedProfile` vérifie et rien de plus. Il vit dans un fichier temporaire plutôt que
dans le dépôt, précisément pour que personne ne le prenne pour le profil de production.

_L'image porte un digest et `curl`._ `Workload::new` refuse une image par tag, et il a raison : une
attestation qui nomme un tag n'atteste de rien de reproductible. `curl` est nécessaire parce que les
deux sondes réseau rendent `120` sans lui, que le harnais lit comme `NotRun` — honnête et inutile,
on saurait que la sonde n'a pas conclu, pas si le réseau est joignable. D'où une image
**construite** dans le job, sans `ENTRYPOINT` qui se substituerait à la commande du workload, avec
un repli déclaré si la référence locale par digest ne se résout pas.

**`continue-on-error: true`, délibéré et temporaire.** Ce job est une question, pas une garde.
Personne ne sait encore ce qu'un runner GitHub sait confiner, et un job rouge pour cette raison
ferait rejeter des PR qui n'y sont pour rien. Le commentaire du workflow écrit l'arbitrage à faire
au passage suivant plutôt que de le laisser à la mémoire : vert → `continue-on-error` tombe et
`W5.f` est clos ; rouge → la table dit pourquoi, et l'item part vers une VM dédiée ou un report
écrit.

**Une question sémantique que ce passage va trancher, et qu'aucun test ne pouvait poser avant.**
`read_host_filesystem` lit `/host-root/etc/hostname`. À `S2` elle doit être contenue, et elle le
sera — rien ne monte `/host-root`. Mais en dessous elle doit **réussir**, et si aucun profil ne
monte jamais la racine de l'hôte à cet endroit, la sonde ne peut réussir à aucun niveau : elle
serait « contenue » partout, y compris là où le niveau ne promet rien. C'est exactement le genre de
chose qu'un double ne peut pas dire, et c'est pourquoi l'item existe. Le constat sera écrit ici
quand le job aura tourné.

**Le premier passage a répondu, et il a trouvé un défaut avant d'exercer une seule sonde.** Le
runner GitHub **fait tourner Podman rootless** : l'image s'est construite, la référence locale par
digest s'est résolue, `podman create` a été atteint. Il a rendu 125 : « storage option overlay.size
and overlay.inodes only supported for backingFS XFS. Found extfs ». `ConfinementPlan::disk_bytes`
devient un `--storage-opt size=`, que Podman ne sait appliquer que sur XFS ; le runner est en ext4.

_Mon propre message mentait, et c'est le premier correctif._ L'assertion disait « cet hôte n'a pas
de runtime rootless en état de marche » sur seize `NotRun`. C'était faux : le runtime marchait et
avait refusé la **spécification**. Les trois états que le test prétendait distinguer ne l'étaient
donc pas — il confondait « il manque une machine » avec « il manque une capacité d'hôte ». `probe`
rend désormais un `Result` dont l'erreur porte le message du runtime mot pour mot, et une sandbox
qui n'a pas démarré ne produit plus une table de seize `NotRun` — zéro observation et une raison ne
sont pas seize observations.

_Deux tests, parce qu'un seul aurait dû mentir dans un sens ou dans l'autre._ Le premier demande si
l'hôte tient `S2` sous une mission qui réserve du disque ; sur ce runner il n'observe rien et le
dit. Le second éprouve les **quinze** sondes qui ne dépendent pas du quota disque — quinze seizièmes
de la question, contre zéro auparavant — et **n'établit jamais `S2`** : `exceed_disk_quota` est
`contained_from: S2`, sans quota elle réussirait pour une raison qui ne dit rien du confinement, et
l'exclure en concluant « `S2` tient » serait la façon exacte de croire une sandbox qu'on n'a pas
testée. L'exclusion est nommée et le test vérifie que la sonde écartée **existe** : l'écarter par un
nom mort n'écarterait rien, et le test croirait couvrir seize sondes.

_Le profil seccomp porte un `tag`._ Les deux tests tournent dans le même processus, donc
`process::id()` ne les sépare pas, et le nettoyage de l'un effacerait le fichier que l'autre
utilise.

**Le défaut trouvé devient `W5.g`.** `probe.rs` a pour doctrine « ce que l'hôte permet réellement —
lu, jamais supposé », et son en-tête dit pourquoi : « un broker qui apprendrait ses limites en
échouant les découvrirait après avoir créé la moitié d'une sandbox ». C'est mot pour mot ce qui se
passe pour le quota disque. Le module frôle le sujet sans en tirer la conséquence —
`REQUIRED_CONTROLLERS` note que « le quatrième, le disque, ne se borne pas par cgroup » et s'arrête
là. Le refus devra être **distinct** de `CapacityExceeded` : « la capacité manque » et « la borne
n'est pas applicable ici » n'envoient pas chercher la même chose. Ce défaut n'était pas trouvable
par un double ; il fallait un vrai `podman create` sur un vrai système de fichiers, et c'est
précisément ce que `W5.f` existe pour obtenir.

**L'arbitrage annoncé, rendu.** Un runner GitHub n'est pas un hôte `S2` pour ce dépôt, et la raison
n'est pas réparable en CI : le système de fichiers d'un runner n'est pas un réglage. `W5.f` demande
donc une VM à système de fichiers XFS, ou un report écrit. Le job reste `continue-on-error` en
attendant, parce que son second test rapporte les quinze sondes — et c'est de l'information qu'on
n'avait pas.

**Le second passage a fait tourner la suite entière, et c'est là que tout s'est joué.** Sans quota
disque, la sandbox démarre, et les seize sondes s'exercent pour la première fois dans un conteneur
rootless réel. La table :

| Verdict                 | Sondes                                                                                                                                                                                                                                                                                        |
| ----------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **tient** (12)          | `write_outside_workspace`, `write_host_home`, `persist_after_teardown`, `read_host_filesystem`, `read_host_secret_files`, `access_container_runtime_socket`, `escalate_to_root`, `observe_host_processes`, `exceed_cpu_quota`, `exceed_memory_quota`, `exceed_pid_quota`, `exceed_disk_quota` |
| **échappement** (1)     | `read_process_environment`                                                                                                                                                                                                                                                                    |
| **sur-confinement** (3) | `open_outbound_connection`, `reach_cloud_metadata_service`, `reach_host_kernel_interfaces`                                                                                                                                                                                                    |

Douze preuves là où il n'y avait qu'une syntaxe vérifiée. Et quatre démentis, qui deviennent `W5.h`.

_`read_process_environment` échappait **parce que le confinement était correct**._ Elle lit
`/proc/1/environ`. Dans un namespace PID, `/proc/1` est l'init **du conteneur** — le `sleep 600` du
workload — appartenant à l'utilisateur mappé. La lecture réussit, et elle réussit d'autant plus
sûrement que le namespace fait son travail. La sonde est donc inversée : elle ne peut pas échouer
sur un hôte correctement confiné, et comme elle est `critical`, tout hôte bien configuré se voyait
refuser la confiance. C'est le contraire du défaut qu'on cherchait, et très exactement ce qu'un
double ne pouvait pas dire.

_Les trois sur-confinements confondent deux choses que le module distingue déjà ailleurs._ Un
résolveur absent dans l'image, une route que le réseau de l'hôte n'ouvre pas, un fichier que l'hôte
lui-même refuse en 0400 : dans les trois cas la commande rend un code non nul ordinaire, que le
harnais lit comme `Blocked`, c'est-à-dire comme une preuve d'isolation. C'est le piège du 127 de
`W5.c` et du 120 de `W5.d`, une couche plus loin — cette fois ce n'est ni la sonde ni ce qu'elle
lisait qui manque, c'est **ce qu'elle voulait atteindre**. Les causes exactes restent à établir par
la sonde elle-même, et c'est le sujet de `W5.h` : une sonde qui ne peut pas dire pourquoi elle a
échoué ne peut pas être crue quand elle réussit.

**La cinquième trouvaille, et le seul endroit où le second test bat le premier.**
`exceed_disk_quota` est ressortie « bloquée → tient » sous une mission qui **ne déclarait aucun
quota**. Elle écrit à la racine (`dd of=/locus-probe-disk`), que `S2` monte en lecture seule : elle
mesure donc la racine en lecture seule, jamais le quota. Une sonde qui passe alors que ce qu'elle
teste n'existe pas est le pire des trois états, parce qu'elle ne se plaint jamais — et il fallait
précisément un passage **sans quota** pour le voir. Le contrôle négatif était accidentel ; il n'en
est pas moins un contrôle négatif, et c'est lui qui a trouvé le plus.

**Ce que la question sémantique annoncée est devenue.** Le passage devait trancher si
`read_host_filesystem` peut réussir à un niveau quelconque. Elle est ressortie « bloquée → tient » à
`S2`, ce qui ne dit encore rien de son comportement en dessous : la question reste due, et elle est
de la même famille que les quatre de `W5.h` — une sonde ne se juge pas seulement sur le niveau où
elle doit être contenue, mais sur celui où elle doit réussir.

**Ce que cela ouvre, et qui n'était pas prévu.** Une fois `W5.h` faite, le second test peut devenir
**vert et bloquant en CI** : quinze sondes de sandbox réellement exercées à chaque passage, sur un
runner ordinaire. Le dépôt n'a jamais eu ça. `W5.f` — les seize, `S2` établi — continue d'attendre
un hôte XFS, et c'est une bien plus petite dette que celle du départ.

**Ce que ce sprint ne prétend pas.** `W5.f` n'est pas clos. Ce qui est livré est l'épreuve et la
question ; la réponse vient du prochain passage de CI, et l'arbitrage qu'elle appelle est écrit
d'avance pour qu'il ne dépende pas de qui le lit.

---

## 2026-08-18 — Roadmap — `locusd` décomposé en W20, et deux reports écrits plutôt que devinés

**Périmètre.** `docs/10_V1_ROADMAP.md` seul. Aucun code. Ce sprint donne à `locusd` sa propre entrée
décomposée, et transforme deux « bloqué » en « reporté » avec la raison et la condition de levée.

**Pourquoi `locusd` méritait son entrée plutôt qu'un bricolage sous `W17.f`.** `W17.f` demande six
points d'entrée dont la logique est déjà écrite. La tentation est d'écrire un serveur à six URL pour
la débloquer. Ce ne serait pas un daemon : sans authentification, sans session, sans flux
d'événements, c'est une démonstration, et la suite s'y greffe au coup par coup. `locusd` est le plus
gros morceau restant de la V1 ; il se décompose comme tel.

**Deux constats vérifiés avant d'écrire.**

_La règle 4 garde le vide, et l'outil le dit._ `npm run check:boundaries` rend « 4.
locusd-holds-no-runtime-socket — vérifiée sur **0 fichier(s)** ». La règle « `apps/locusd` n'importe
aucun SDK de runtime de containers » n'a jamais rien gardé, parce que le répertoire n'existe pas. Ce
n'est pas un défaut de la garde : `check-boundaries.ts` imprime le compte pour chaque règle
précisément pour que « vérifiée sur rien » ne se lise pas comme « vérifiée ». C'est le jalon de
`W20.d`, et il est plus honnête qu'un compte de lignes.

_Le transport est une décision d'ADR._ `Cargo.toml` ne déclare que `serde` et `serde_json` comme
dépendances d'espace de travail. Faire entrer un runtime asynchrone et un cadre HTTP est le plus
gros choix de dépendance depuis l'ADR 0011, et il a son propre item — `W20.c` — avec la même forme :
conditions énoncées, plan de rollback, et une première dépendance hors `serde` qui cite l'ADR dans
son diff.

**La conséquence non évidente : `locusd` commence avant l'ADR du transport, pas après.** `W20.a` —
le `CommandEnvelope` de §22.2 et les huit familles d'erreurs de §22.5 — et `W20.b` — le handler
transactionnel comme port — sont du domaine pur. C'est exactement l'ordre que `CLAUDE.md` impose, «
construire domain/protocol/event-store d'abord, avec des ports purs », et cela veut dire que le plus
gros morceau restant n'est bloqué par aucune décision.

_Ce que `W20.b` rend opposable._ « Toute mutation passe par un command handler transactionnel » est
une règle de `CLAUDE.md` qu'aucun test ne tient aujourd'hui. `packages/event-store` porte déjà
l'ordre total, la concurrence optimiste et l'idempotence par commande (§10.2) ; ce qui manque est le
chemin qui les rend obligatoires. Le test de sortie le demande par l'absence — aucun chemin de type
ne permet d'écrire sans handler.

_Et ce que `W20.a` refuse d'avance._ Un conflit rend l'**état courant** et un code structuré, jamais
un entier nu. §22.5 le demande, et la raison est mécanique : un client qui doit relire pour retenter
a besoin de ce qu'il relit. Un `409` seul le renvoie faire un aller-retour que le serveur pouvait
lui épargner.

**Deux reports, écrits parce qu'un « bloqué » sans durée se lit comme une attente courte.**

`W16.e` — epochs, messages tardifs, transfert d'état — attend une messagerie inter-agents. Il n'y en
a pas : les agents parlent à l'institution, pas entre eux. Le problème des messages tardifs n'existe
qu'une fois que A envoie à B et que B a changé d'état entre-temps. Construire la messagerie **pour**
débloquer l'item reviendrait à construire une fonctionnalité afin de justifier un test. C'est le
seul item de la roadmap bloqué correctement **et** définitivement pour cette version.

`W18.f` attendait « un hôte capable, comme W5.f ». `W5.f` vient de rendre la condition précise, et
elle est plus dure qu'on ne le croyait : un système de fichiers qui porte les quotas — XFS avec
pquota, faute de quoi `podman create` refuse dès la création —, une isolation réseau pour `S3`, une
micro-VM pour `S4`, et de quoi **attester**. Un runner GitHub échoue sur le premier et ne tient
aucun des trois autres. La condition est une machine dédiée : c'est un fait de déploiement, pas une
dette de code, et les deux items le disent maintenant sous cette forme.

---

## 2026-08-18 — W5.h — Les sondes que le premier hôte réel a démenties : trois corrigées, deux renvoyées

**Périmètre.** `apps/locus-execd/src/linux/selftest.rs` et son test. Trois sondes réécrites, un code
de sortie réservé de plus, cinq tests. Les deux sondes restantes sont renvoyées, chacune vers l'item
qui peut réellement la traiter.

**La sonde inversée, et c'est la plus grave des cinq trouvailles.** `read_process_environment`
lisait `/proc/1/environ`. Dans un namespace PID — que `S2` pose — `/proc/1` est l'init **du
conteneur**, c'est-à-dire le workload, appartenant à l'utilisateur mappé. La lecture réussissait
donc, et elle réussissait **d'autant plus sûrement que le confinement était correct**. La sonde ne
pouvait pas échouer sur un hôte bien configuré ; comme elle est `critical`, tout hôte bien configuré
se voyait refuser la confiance. Un an de `sh -n` vert n'aurait jamais montré ça.

_Le discriminant retenu est le cgroup._ La dimension est `HostSecret` et le motif dit «
l'environnement d'un **autre** processus » : « autre » veut dire hors de cette sandbox. `S2` pose
aussi un namespace cgroup, donc tout ce que le conteneur voit porte le même chemin que lui ; un
processus dont le cgroup diffère du nôtre est, par construction, un processus que nous n'avons pas
créé. Sans namespace PID les processus de l'hôte sont visibles avec leurs cgroups propres et la
sonde en trouve un — elle réussit, comme le niveau le permet. Avec, il n'y a plus rien d'étranger à
lire, **et c'est cela le confinement**.

**Un code réservé de plus, et il ne se confond avec aucun autre.** `121` dit « ce que je devais
**atteindre** n'a pas répondu ». `120` disait « ce que je devais **lire** n'était pas là ». Les deux
sont des ignorances, aucune n'est un blocage, et elles ne se réparent pas pareil : la première en
complétant l'image, la seconde en changeant d'hôte ou en renonçant à la mesure. Les fondre ferait
disparaître la seconde — ce qui est exactement l'état d'avant, où trois sondes ressortaient
**bloquées**, c'est-à-dire lues comme une preuve d'isolation, alors que le réseau de l'hôte ne
menait nulle part. C'est le piège du 127 de `W5.c` et du 120 de `W5.d`, une couche plus loin.

**Les deux sondes réseau constatent d'abord s'il y a une route.** `S3` s'appelle
`container-isolated-network` : ce qu'il contient **est** le namespace, et un namespace réseau vide
n'a pas de route par défaut. Sans ce constat, un `curl` qui échoue ne distingue pas « la sandbox a
coupé le réseau » de « l'hôte ne mène nulle part ». Le constat lit `/proc/net/route` et non
`ip route` : le fichier existe toujours, le binaire non, et `W5.d` interdit à une sonde d'attendre
quoi que ce soit de l'image. Un test le tient dans les deux sens.

**Ce que ce sprint ne fait pas, et pourquoi ce n'est pas de la paresse.**

`exceed_disk_quota` appartient à `W5.g`. La corriger demande de savoir **où** un quota s'applique,
et le premier hôte réel a montré que la réponse n'est pas celle du code actuel : à `S2` la racine
est en lecture seule, donc `--storage-opt size=` dimensionne une couche inscriptible que personne
n'écrit. Déplacer la sonde vers l'espace de travail sans déplacer le quota ferait mesurer un système
de fichiers hôte non borné — on remplacerait une sonde qui ment par une autre. La sonde suit le
quota ; le quota d'abord.

`reach_host_kernel_interfaces` devient `W5.i`, parce qu'elle demande un arbitrage sur ce que `S4`
promet. Elle lit `/sys/kernel/vmcoreinfo`, réservé à root : elle échoue pour cette raison-là sur
tout hôte, à tout niveau. Mais la retarger vers une interface lisible ne suffirait pas — `S4`
apporte **un autre noyau**, ce qui n'empêche personne de lire les interfaces de ce noyau-là. Ce que
la sonde doit constater est que le noyau atteint **n'est pas celui de l'hôte**, et c'est une autre
mesure que « la lecture est refusée ». Trancher cela change ce que `microvm-high-risk` veut dire ;
ça ne se fait pas en corrigeant une commande shell.

**Ce que le job `sandbox` va dire, et ce qu'il ne dira pas.** Il reste `continue-on-error`, et pas
seulement en attendant `W5.g`. `reach_cloud_metadata_service` doit **réussir** à `S2`, et un runner
GitHub filtre `169.254.169.254` : sur cet hôte-là elle rendra désormais `121` — « la cible n'a pas
répondu » — au lieu de « bloquée ». C'est le bon verdict, et il reste un `Inconclusive`, donc un
refus de confiance. Une sonde ne peut pas conclure sur un hôte qui n'offre pas ce que le niveau
permet, et le dire est mieux que de compter un filtrage réseau comme une preuve d'isolation.

**Vérifié sur l'hôte réel, et c'est le point.** Le passage de CI qui suit ce commit rend
`read_process_environment` **« bloquée → tient »**. La sonde inversée est corrigée, et elle est
corrigée _contre un vrai conteneur_, pas contre un double. Treize sondes sur seize tiennent
désormais, contre douze.

**Ce que le constat de route a répondu, et ce qu'il ne tranche pas.** Les deux sondes réseau
ressortent toujours « bloquées », c'est-à-dire que le constat n'a **pas** trouvé de route par
défaut. Deux lectures possibles, et elles se réparent à des endroits opposés : soit le conteneur n'a
réellement aucune route — et alors c'est le **driver** qui ne délivre pas le réseau que
`NetworkMode::Full` demande à `S2`, ce qui serait une trouvaille du même ordre que le quota disque —
soit le constat lit mal `/proc/net/route` dans ce contexte, et alors c'est la sonde. Affirmer l'un
sans regarder serait exactement ce que cet item reproche aux sondes.

Le job imprime donc désormais ce que le conteneur voit : `/proc/net/route`, `resolv.conf`, et le
code de retour de `curl` sur les deux cibles. Une exécution répondra. Ce qui n'est pas fait ici
n'est pas présenté comme fait.

**Tests exécutés.** `cargo test -p locus-execd` → 24 conformes dans `selftest`, dont cinq neufs.
`npm run check` → les dix portes vertes. Mutation : neuf mutants, **neuf tués** — après correction
d'un survivant qui était un vrai trou. Le mutant remplaçait `mine=$(cut … /proc/self/cgroup …)` par
`mine=1` : la sonde comparait alors chaque processus à une constante, n'en trouvait aucun qui
corresponde, et lisait le premier venu — elle serait redevenue un échappement systématique. Le test
n'exigeait que la présence du mot « cgroup » ; il exige maintenant que la sonde lise **son propre**
cgroup, sans quoi la comparaison n'a qu'un côté.

---

## 2026-08-18 — W5.g — Le quota disque devient un fait lu, et un refus qui nomme le système de fichiers

**Périmètre.** `apps/locus-execd/src/linux/probe.rs` (la lecture),
`apps/locus-execd/src/admission.rs` (la décision), `apps/locus-execd/tests/linux.rs` (huit tests
neufs, deux mis à jour).

**Ce que l'item réparait.** L'en-tête de `probe.rs` pose la doctrine : « ce que l'hôte permet
réellement — lu, jamais supposé », parce qu'« un broker qui apprendrait ses limites en échouant les
découvrirait après avoir créé la moitié d'une sandbox ». C'était vrai de tous les faits **sauf un**.
`ConfinementPlan::disk_bytes` devient un `--storage-opt size=` que Podman n'applique que sur XFS, et
personne ne le vérifiait : `W5.f` l'a appris de `podman create`, qui a rendu 125 après avoir
commencé à configurer le stockage. `REQUIRED_CONTROLLERS` frôlait le sujet — « le quatrième, le
disque, ne se borne pas par cgroup » — sans en tirer la conséquence.

**Décisions prises.**

_Le quota est une propriété d'un chemin, pas d'un hôte._ D'où `with_storage(reader, storage_root)`
plutôt qu'un champ de plus dans `probe`. Les autres faits se lisent sur le noyau — cgroup v2 est
monté ou non, seccomp existe ou non. Sans racine de stockage déclarée, il n'y a pas de question bien
posée, et un chemin deviné rendrait un fait sur un autre système de fichiers que celui qui sera
écrit. Le défaut est donc `Undetermined`, avec sa raison.

_Le doute ne s'arrondit pas vers le haut, ici non plus._ `unenforceable_disk_quota` rend `Some` pour
`Undetermined` comme pour `Unavailable` : « je n'ai pas su regarder » ne vaut pas « c'est disponible
». Les deux mènent au même refus, pas au même texte, parce qu'ils ne s'inspectent pas au même
endroit. Un mutant qui les sépare meurt.

_Le montage retenu est le plus long préfixe._ Prendre le premier qui correspond rendrait `/` pour un
stockage vivant sur un volume monté plus bas — donc un verdict sur le mauvais système de fichiers,
et un verdict flatteur. La comparaison se fait au **segment** : `/var` couvre `/var/lib` mais pas
`/variable`.

_Deux conditions indépendantes, et c'est un mutant qui l'a établi._ Le système de fichiers doit être
XFS **et** le montage porter `prjquota`. Le premier passage de mutation a laissé survivre « ext4
passe pour capable » : désactiver le contrôle de système de fichiers ne cassait rien, parce que
toutes les fixtures ext4 étaient **aussi** sans `prjquota` et tombaient sur l'autre refus. Or ext4
avec `prjquota` est une configuration réelle et ordinaire, que Podman refuse quand même. Un test la
couvre maintenant, et il vérifie en plus que le refus **ne parle pas** de `prjquota` : le quota de
projet est activé là, et le dire manquant enverrait remonter un volume avec une option qu'il porte
déjà.

_Le refus est distinct de `CapacityExceeded`, et la distinction n'est pas cosmétique._ « La capacité
manque » envoie libérer de la place ou réduire la réservation ; « la borne n'est pas applicable ici
» envoie changer de système de fichiers, ou de machine. Les fondre ferait réduire une réservation
qui aurait échoué de la même façon à un octet. Le test le tient dans les deux sens : le motif de
quota est présent, `CapacityExceeded` est absent, et la **même mission sans réservation de disque
est admise** — c'est la borne qui est refusée, pas l'hôte.

_« Aucun chemin ne laisse `podman create` être l'endroit où on l'apprend » se tient par l'absence._
Suivre les appels n'y suffirait pas : il en suffirait d'un, ajouté plus tard, pour rouvrir le trou.
Le test lit le code d'`admission.rs`, commentaires retirés, et refuse `Runner`, `PodmanBackend`,
`RuntimePort`, `process::Command`, `podman`, `storage-opt`. Le module ne peut donc pas apprendre en
essayant, quelle que soit la bonne volonté de son prochain lecteur.

_La preuve gagne un cinquième constat._ §21.6 veut un témoignage, pas une affirmation. Un fait lu
qui n'apparaîtrait pas dans `evidence()` serait un fait que l'attestation tait, donc un fait que
personne ne peut contester. Le test qui comptait quatre lignes en compte cinq, et vérifie que celle
du quota y est.

**Le pont lu → déclaré → refusé, et pourquoi il est nommé.** `HostFacts` lit,
`HostCapabilities::without_disk_quota(why)` déclare, `admit` décide. Sans ce pont le fait serait lu
et jamais consulté, ce qui reviendrait exactement à ne pas le lire. Le paramètre est la **raison**
et non un booléen : un refus qui dirait seulement « pas de quota disque » enverrait chercher une
option de configuration là où c'est le système de fichiers qui décide. Un test parcourt la chaîne
entière sur une fixture ext4.

**Ce que ça ne fait pas, et qui devient `W5.j`.** Le quota est maintenant **lisible** ; il n'est pas
**applicable**. À `S2` la racine est montée en lecture seule, donc `--storage-opt size=` dimensionne
une couche que personne n'écrit, et le seul endroit inscriptible est l'espace de travail monté — un
bind mount, qui hérite du système de fichiers de l'hôte, non borné. Autrement dit : même sur un hôte
XFS avec `prjquota`, il n'est pas établi que le quota morde. C'est une décision de driver — volume
dimensionné, tmpfs borné, ou renoncement déclaré — et `exceed_disk_quota` la suivra.

**Tests exécutés.** `cargo test -p locus-execd --test linux` → 31 conformes, dont huit neufs.
`npm run check` → les dix portes vertes. Mutation : onze mutants, **onze tués** — après correction
du survivant décrit plus haut, qui était un vrai trou.

---

## 2026-08-18 — W5.k — L'instrument a démenti l'hypothèse qui l'avait fait construire

**Périmètre.** `apps/locus-execd/tests/host_sandbox.rs` (un test `#[ignore]` de plus) et
`docs/10_V1_ROADMAP.md`. Aucun correctif : ce sprint **établit** un fait et livre l'instrument qui
le tranchera.

**D'où vient la question.** Le constat de route ajouté par `W5.h` devait distinguer « la sandbox a
coupé le réseau » de « l'hôte ne mène nulle part ». Sur le runner, il a rendu « pas de route par
défaut ». Deux lectures s'offraient, réparables à des endroits opposés — la sonde lit mal, ou la
sandbox n'a réellement pas de réseau — et le ledger de `W5.h` disait explicitement qu'affirmer l'une
sans regarder serait exactement ce que cet item reproche aux sondes.

**Trois vérifications, aucune n'a coûté un passage de CI supplémentaire.**

_Les arguments sont bons._ `create_arguments` pour `S2` + `NetworkMode::Full` porte
`--network=host`, et `plan` rend `NetworkPosture::Host` sans namespace réseau. Vérifié hors
conteneur, sur les arguments eux-mêmes, en imprimant ce que le plan produit.

_Le constat de route est bon._ Rejoué hors ligne sur la sortie réelle de `/proc/net/route` capturée
dans le conteneur, l'`awk` trouve la route et rend 0 ; sur un `/proc/net/route` réduit à son en-tête
— ce que présente un namespace réseau vide — il rend 1. La sonde fait donc exactement ce qu'elle
annonce.

_Le monde est joignable._ Le job imprime désormais ce qu'un `podman run --network=host` nu voit sur
le runner : une route par défaut sur `eth0`, un `resolv.conf` qui résout, `curl example.org` →
**200**, et même `169.254.169.254` → **400**, c'est-à-dire un service qui répond.

**Ce qui reste, et c'est le fait.** La sandbox créée par le plan ne voit pas la route, alors que les
arguments la demandent et que l'hôte l'offre. **Une permission déclarée n'est pas accordée**, en
silence. C'est le miroir exact de `W5.g` : là une borne déclarée n'était pas applicable, ici une
permission déclarée n'est pas honorée. Les deux ne se voient qu'en regardant depuis l'intérieur, et
c'est pourquoi aucun double ne pouvait les trouver.

**Ce qui est livré.** Un test `#[ignore]` qui crée la sandbox du plan, lit `/proc/net/route`
**depuis l'intérieur**, l'imprime, puis affirme que la route par défaut y est. Il imprime avant
d'affirmer parce que la suite du travail est de trouver **lequel** des drapeaux du plan produit cet
écart — `--read-only`, la posture seccomp, les douze `--cap-drop`, les quotas cgroup — et que cela
se bissecte sur une table, pas sur un verdict.

**Et le premier passage du test a trouvé un défaut dans le test lui-même.** Il a rapporté, à la
place de la table de routage : « le nom de conteneur `locus-0001` est déjà utilisé ». Les trois
tests du fichier tournent dans le même processus et chacun construit son propre `PodmanBackend`,
dont le compteur de noms repart à zéro ; lancés en parallèle, ils se disputent le même nom. Le test
a alors **affirmé que la sandbox ne voyait pas la route, alors qu'aucune sandbox n'existait**.

C'est mot pour mot la faute que cet item reproche aux sondes : présenter une absence d'observation
comme une observation. Deux corrections. `inspect_network` rend un `Result` — le type empêche
désormais de confondre « créée, et voici ce qu'elle voit » avec « pas créée, et voici pourquoi » —
et le job lance la suite en `--test-threads=1`, avec la raison écrite dans le workflow plutôt que
laissée à deviner.

Le fait de fond n'en est pas affecté : les verdicts « bloquée » des deux sondes réseau viennent du
**second** test, celui qui a bien créé sa sandbox et exercé les seize sondes. C'est le nouveau test
de diagnostic qui n'avait rien observé, et il le dit maintenant.

**Ce que ce sprint ne fait pas.** Il ne corrige rien, et il ne prétend pas savoir quel drapeau est
en cause. Nommer un coupable sans l'avoir isolé serait la même faute que celle des trois sondes qui
lisaient un réseau muet comme une preuve d'isolation.

---

## 2026-08-18 — W5.k, suite — L'hypothèse est fausse, et le vrai défaut est que « arrêter » n'est pas « retirer »

**Ce sprint retire une affirmation.** L'entrée précédente concluait que la sandbox du plan
n'obtenait pas le réseau déclaré. **C'est faux.** Le test construit pour le vérifier — celui qui lit
`/proc/net/route` depuis l'intérieur — **passe** sur le runner : la sandbox voit la route par défaut
de l'hôte, sur `eth0`, avec la passerelle. L'instrument a démenti l'hypothèse qui l'avait fait
construire, et c'est exactement ce pour quoi il avait été écrit.

**Ce que les trois passages illisibles cachaient.** `RuntimePort::stop` lance `podman stop`. **Rien
ne lance `podman rm`.** Un conteneur arrêté garde son nom et sa couche inscriptible ; le suivant qui
demande le même nom échoue avec « the container name `locus-0001` is already in use ». Et comme
chaque test construit son propre `PodmanBackend`, dont le compteur de noms repart à zéro, ils se
disputent tous le premier nom.

Le module `selftest` avait vu la **conséquence** sans voir la **cause**. Sa documentation de
`certify` dit : « la sandbox est arrêtée même quand la suite s'est mal passée : une sonde qui a
échoué laisse derrière elle un conteneur qui tourne, et un hôte qui accumule des conteneurs
d'épreuve finit par ne plus pouvoir en créer. » La phrase est juste et la précaution ne suffit pas :
**arrêter n'est pas retirer**, et c'est le nom, pas l'exécution, qui manque au suivant.

**Ce que cela invalide.** Les tables de sondes lues jusqu'ici viennent de passages où **un seul**
des conteneurs existait ; les autres rapportaient une erreur de nom là où on attendait un verdict de
confinement. Les verdicts « bloquée » des deux sondes réseau ne sont donc plus établis : ils peuvent
venir d'un conteneur réel comme d'une collision. Il faut un passage propre avant de croire une ligne
de plus de ces tables, et c'est pourquoi rien de ce qui en découlait n'est reporté ici comme acquis.

**Ce qui est fait, et ce qui est ouvert.** Les tests retirent désormais leur conteneur après l'avoir
arrêté, par le runner et non par le port — parce que **le port n'a pas cette opération**. C'est une
dette assumée et nommée : un test qui laisse derrière lui de quoi faire échouer le suivant ne mesure
plus rien, mais l'endroit correct est le port. `W5.l` l'y met, et son test de sortie demande qu'un
`certify` ne laisse **rien** derrière lui, constaté en redemandant le même nom.

_Une conséquence à ne pas manquer._ `persist_after_teardown` — « un fichier écrit dans la sandbox ne
survit pas au démontage » — tient aujourd'hui parce que l'écriture est refusée par la racine en
lecture seule, jamais parce qu'un démontage a eu lieu : **il n'y en a pas**. C'est la même famille
que `exceed_disk_quota` de `W5.h`, une sonde qui passe un test qu'elle ne fait pas tourner. `W5.l`
la rendra mesurable.

**Ce que ce sprint garde de bon.** Un instrument qui regarde depuis l'intérieur, et qui a servi
exactement une fois à réfuter celui qui l'avait écrit. Et deux corrections de méthode :
`inspect_network` rend un `Result`, de sorte qu'« aucune sandbox » ne puisse plus se présenter comme
« aucune route » ; et le job lance la suite en `--test-threads=1`, la raison écrite dans le
workflow.

**Le passage propre a eu lieu, et il resserre la question au lieu de la fermer.** Avec
`--test-threads=1` et le retrait des conteneurs, plus aucune collision : le test de réseau **passe**
— la sandbox voit la route par défaut — et `les_quinze_sondes` s'exécute pour de bon. Les mêmes
**trois** sondes ressortent sur-confinées, `open_outbound_connection`,
`reach_cloud_metadata_service` et `reach_host_kernel_interfaces`.

Ce qui subsiste est donc une contradiction nette, et elle est plus intéressante que l'hypothèse
qu'elle remplace : **dans le même conteneur, `cat /proc/net/route` montre la route par défaut, et le
constat `awk` de la sonde ne la trouve pas.** Les deux passent par `podman exec` sur la même
sandbox, avec la même spécification.

Trois suites possibles, et aucune n'est acquise : l'`awk` de busybox ne découpe pas ces champs comme
celui du poste où le motif a été mis au point ; la sonde n'atteint pas la branche du constat et sort
avant, par un chemin qui rend un code lu comme un blocage ; ou les deux lectures ne voient pas le
même `/proc`. Ce qui les départage est le **code de sortie de la sonde elle-même**, que le harnais
réduit aujourd'hui à trois états — et c'est précisément ce qu'il faut instrumenter au prochain
passage.

Ce n'est pas écrit comme un défaut de plus, mais comme une question posée correctement : la
précédente ne l'était pas, et elle a coûté une hypothèse fausse.

---

## 2026-08-18 — W5.l — Arrêter n'est pas retirer

**Périmètre.** `apps/locus-execd/src/runtime.rs` (le port), `src/linux/driver.rs`
(l'implémentation), `src/linux/selftest.rs` (`certify`), et les deux fichiers de tests.

**Le port promettait ce qu'il ne tenait pas.** La documentation de `RuntimePort::stop` disait «
arrêter la sandbox **et rendre ce qu'elle tenait** ». `podman stop` arrête les processus et laisse
le **nom** et la **couche inscriptible**. La promesse était donc fausse dans son texte même, et
personne ne l'avait relue depuis le double qui la satisfaisait sans rien tenir.

**Deux méthodes, et elles restent séparées.** `stop` arrête, `remove` rend. L'entre-deux est un état
légitime : une sandbox arrêtée se réinspecte, et `attestation` se lit après l'arrêt. Les fondre
supprimerait cette lecture ; ne pas les distinguer était le défaut.

_`remove` force._ Un retrait doit aboutir même sur une sandbox en marche : son rôle est de rendre le
nom, et exiger un arrêt préalable laisserait le nom pris exactement dans le cas où l'on a le plus
besoin de le libérer — celui où la suite s'est mal passée.

_Le registre se met à jour **après** le succès du runtime._ L'inverse rendrait la sandbox inconnue
du backend alors qu'elle existe encore sur l'hôte, et plus personne n'aurait de quoi la retirer.

**`certify` démonte sur tous les chemins, y compris celui du démarrage raté.** La version précédente
rendait l'erreur par `?` et abandonnait un conteneur créé mais jamais démarré : le cas le plus
silencieux, puisque rien ne tourne et que rien ne signale la fuite — mais le nom reste pris. Un test
le tient, avec un double qui crée volontiers et refuse de démarrer.

_Les erreurs du démontage sont écartées, et c'est délibéré._ Un démontage est du nettoyage ; le
verdict qu'on est en train de rendre porte sur le confinement, pas sur la capacité du runtime à
ranger. Les masquer serait grave si rien d'autre ne les voyait — mais un nom resté pris se signale
au suivant, très bruyamment, et c'est exactement comme cela que le défaut a été trouvé.

**Le test de sortie constate le nom, pas le registre.** « Le conteneur n'existe plus » ne se vérifie
pas en interrogeant le backend qui vient de l'oublier : il répondrait ce qu'il a noté, pas ce que
l'hôte tient. Ce qui se vérifie est ce qui manquait au suivant — **le nom**. Deux backends
successifs, chacun avec son compteur repartant de zéro, redemandent donc `locus-0001`, et le second
doit l'obtenir. C'est le scénario exact qui a rendu trois passages de CI illisibles.

**Le test qui pinçait l'ancien comportement a été réécrit, pas supprimé.** Il s'arrêtait à `stop`,
avec la bonne raison — « une campagne qui laisserait le conteneur tourner finirait par saturer
l'hôte » — et une conclusion insuffisante. Il épingle maintenant les deux appels **dans l'ordre** :
un retrait sans arrêt marcherait, `rm --force` y pourvoit, mais ferait disparaître la distinction
que le port porte délibérément.

**Ce que cela débloque.** Les tables de sondes redeviennent lisibles, et `persist_after_teardown`
devient mesurable — elle demande qu'un fichier écrit ne survive pas au démontage, et il n'y avait
pas de démontage. Ce qu'elle rendra une fois mesurée n'est pas supposé ici.

**Tests exécutés.** `cargo test -p locus-execd` → 25 conformes dans `selftest`, dont deux neufs, et
31 dans `linux`. `npm run check` → les dix portes vertes.

---

## 2026-08-19 — W5.m — Le code de sortie voyage à côté du verdict, jamais dedans

**Périmètre.** `apps/locus-execd/src/linux/selftest.rs` (le type `Trial`), le module qui l'exporte,
et les deux fichiers de tests. Aucune sonde modifiée, aucun verdict changé.

**La question que le rapport ne pouvait pas poser.** `Observed` a trois valeurs, et c'est le bon
compte pour **juger** : réussie, bloquée, pas lancée. Mais plusieurs codes de sortie très différents
tombent dans « bloquée », et quand `open_outbound_connection` est ressortie bloquée sur un hôte dont
un autre test montrait la route par défaut, rien ne permettait de dire **où** la sonde s'était
arrêtée — au constat de route, à `curl`, ou avant. Trois suites étaient possibles et le rapport n'en
départageait aucune.

**Décisions prises.**

_Le code brut voyage à côté du verdict, jamais dedans._ L'y mettre ferait entrer un détail de Podman
dans le vocabulaire de `packages/execution`, qui ne connaît aucun runtime — et un verdict à
quatre-vingt-dix valeurs n'est plus un verdict. D'où `Trial { name, observed, code }`, et une
conversion **explicite** `verdicts()` pour ce que `standing` attend : juger se fait sur les trois
valeurs, et un jugement qui dépendrait d'un code de Podman ne serait plus transposable.

_`code` est une `Option`._ Un runtime qui n'a pas répondu **n'a pas** de code. Inventer un `-1`
produirait une valeur que quelqu'un finirait par lire comme un vrai code, et un `0` signifierait un
succès.

_Le code d'un succès est rapporté aussi._ Sans cela, « pas de code » voudrait dire deux choses —
réussi, ou pas lancé — et le rapport reconstruirait à l'intérieur de lui-même l'ambiguïté qu'il est
censé lever. Un test le tient.

**Un survivant de mutation rendu inexprimable plutôt que couvert.** Le mutant prêtait un `127` à la
sonde sans commande. Aucun test ne mordait, et pour une raison qui n'est pas une négligence : ce
chemin est **inatteignable** tant qu'aucune sonde n'est orpheline, ce qu'un autre test garantit dans
les deux sens. Le couvrir aurait demandé de fabriquer une sonde orpheline, c'est-à-dire de casser
l'invariant pour tester ce qui arrive quand il est cassé.

Les deux façons de ne pas tourner passent donc par un seul constructeur, `Trial::not_run`, qui pose
`code: None` par construction. Cela ne rend pas le chemin testable ; cela rend la faute inexprimable
sans réécrire le constructeur — et le test qui couvre l'**autre** chemin garde alors les deux. Le
mutant réécrit sur le constructeur meurt.

**Ce que ça donne au prochain passage.** La table du job `sandbox` porte désormais une colonne
`code`. Les trois sondes sur-confinées diront enfin où elles s'arrêtent : `1` au constat de route,
un code de `curl` après lui, ou autre chose avant. La réponse n'est pas devinée ici — c'est
précisément la faute qui a coûté une hypothèse fausse à `W5.k`.

**Tests exécutés.** `cargo test -p locus-execd` → 28 conformes dans `selftest`, dont trois neufs.
`npm run check` → les dix portes vertes. Mutation : cinq mutants, **cinq tués**, zéro survivant.

_Note de méthode._ Le premier passage de mutation avait restauré un instantané pris **avant**
l'édition, annulant silencieusement le correctif — l'instantané périmé a été supprimé et la
vérification refaite. Un harnais de mutation qui écrit dans l'arbre de travail doit être relu comme
tout le reste.

---

## 2026-08-19 — W5.n — 255, et la sonde PID qui faisait mentir les quatre suivantes

**Périmètre.** `apps/locus-execd/src/linux/selftest.rs` et son test. Un code réservé de plus, une
sonde qui rend ce qu'elle prend.

**L'instrument de `W5.m` a répondu au premier passage.** La colonne `code` a montré le motif d'un
coup : **toutes** les sondes situées après `exceed_pid_quota` rendaient **255**.

| Sonde                          | Code | Ce qui en était conclu |
| ------------------------------ | ---- | ---------------------- |
| `exceed_pid_quota`             | 2    | tient                  |
| `exceed_disk_quota`            | 255  | « tient »              |
| `open_outbound_connection`     | 255  | « sur-confinement »    |
| `reach_cloud_metadata_service` | 255  | « sur-confinement »    |
| `reach_host_kernel_interfaces` | 255  | « sur-confinement »    |

**Le mécanisme.** `exceed_pid_quota` sature délibérément le quota de PID en forkant `sleep 5 &`
jusqu'au plafond, et sortait sans les attendre. Le cgroup restait saturé cinq secondes ;
`podman exec` ne pouvait plus forker et abandonnait avec son code générique. Les quatre sondes
suivantes **n'ont pas tourné du tout**.

Et comme 255 n'était pas catalogué, le harnais le lisait comme un **blocage** — c'est-à-dire comme
une preuve d'isolation. Trois « sur-confinements » n'existaient pas, et le « tient »
d'`exceed_disk_quota` n'était pas mérité. C'est le piège du 127 de `W5.c` pour la troisième fois,
sur un code que personne n'avait catalogué.

**Ce que cela rétracte.** Les trois sur-confinements que `W5.h` avait renvoyés à `W5.i` et `W5.m`,
et sur lesquels `W5.k` a bâti une hypothèse fausse, **n'étaient pas des observations**. La question
« pourquoi la sonde ne trouve-t-elle pas la route que `cat` montre ? » n'avait pas lieu d'être : la
sonde n'a jamais lu `/proc/net/route`. Elle n'a jamais démarré.

**Décisions prises.**

_255 rejoint 125, 126 et 127._ Le test qui épingle la table nomme désormais quatre codes et dit
pourquoi chacun est réservé. Un second test vérifie qu'**aucune sonde de la suite ne sort
volontairement en 255** — c'est ce qui autorise à lire ce code comme « n'a pas été lancée » plutôt
que comme un verdict, et si une sonde venait à l'utiliser le catalogage deviendrait faux et
masquerait son résultat.

_La sonde PID tue et attend ses enfants._ Sur le chemin où elle va au bout, elle ne laisse plus rien
derrière elle. **Ce n'est pas une garantie**, et c'est écrit comme tel : si le shell meurt lui-même
de ne pouvoir forker, le nettoyage ne tourne pas.

**Ce que le catalogage ne fait pas, et qui devient `W5.o`.** Il ne rend pas les sondes suivantes
mesurables : il les fait passer de « fausse preuve » à « aveu d'ignorance ». C'est la seule des deux
valeurs qu'on ait le droit d'écrire, et ce n'est pas une mesure. Tant que l'ordre de `SUITE` décide
de ce que les sondes voient, la suite ne mesure pas ce qu'elle prétend.

**Tests exécutés.** `cargo test -p locus-execd --test selftest` → 29 conformes, dont deux neufs.
`npm run check` → les dix portes vertes.

---

## 2026-08-19 — W5.o — Une sonde ne contamine plus la suivante

**Périmètre.** `apps/locus-execd/src/linux/selftest.rs` (la reprise), `src/linux/driver.rs` (la
pause, réglable), et le test. Aucune sonde modifiée.

**Ce que `W5.n` avait laissé.** Cataloguer 255 faisait passer les sondes contaminées de « fausse
preuve » à « aveu d'ignorance ». C'était la bonne valeur, et ce n'était toujours pas une mesure :
tant que l'ordre de `SUITE` décide de ce que les sondes voient, la suite ne mesure pas ce qu'elle
prétend.

**Décisions prises.**

_Deux familles parmi les codes réservés, et elles ne se confondent pas._ 126 et 127 sont des
propriétés de l'**image** : une sonde absente ne le sera pas moins à la deuxième tentative, et
réessayer ne ferait que retarder l'aveu — en rendant six fois plus lente chaque campagne sur une
image incomplète. 125 et 255 sont des échecs du **runtime au moment où il a essayé** : il n'a pas pu
forker, il n'a pas su démarrer. Ceux-là peuvent tenir à ce que la sonde précédente était en train de
faire, et ce sont les seuls qu'on retente. Un test tient les deux fautes symétriques : une liste
transitoire vide désactiverait toute reprise, une qui contiendrait 127 ferait boucler pour rien.

_La reprise est bornée, et le budget se lit._ `LAUNCH_ATTEMPTS` vaut six, avec des pauses qui
doublent depuis cent millisecondes : la somme couvre à la seconde près le pire cas connu —
`exceed_pid_quota` tenant le cgroup le temps de ses `sleep 5` si son propre nettoyage n'a pas
tourné. Un budget plus court laisserait la contamination passer une fois sur deux, ce qui est pire
qu'un budget nul : on croirait le problème réglé. Le coût n'est payé que lorsque quelque chose ne va
pas — une sonde qui se lance du premier coup ne dort jamais.

_Le code rapporté est celui de la tentative qui a abouti._ Pas celui des refus : ce qui intéresse le
lecteur est ce que la sonde a mesuré, et les refus n'ont rien mesuré. Un test le tient.

**La pause appartient au backend, pas à l'algorithme — et c'est le test qui l'a exigé.** Le premier
passage a fait durer la suite **quarante-neuf secondes** : le test du refus persistant dormait le
budget entier pour chacune des seize sondes. Contre un double, ces pauses ne mesurent rien et
coûtent tout, puisque chaque itération y est immédiate. `PodmanBackend::with_launch_pause` les met à
zéro dans les tests, et cela n'affaiblit pas ce qu'ils vérifient : le **nombre** de tentatives, qui
est ce qui décide si une sonde a été mesurée. La suite est repassée à trois centièmes de seconde.

**Tests exécutés.** `cargo test -p locus-execd --test selftest` → 33 conformes, dont quatre neufs.
`npm run check` → les dix portes vertes. Mutation : sept mutants, **sept tués**, zéro survivant.

**Ce que le premier passage réel a rendu, et la question qu'il aiguise.** La reprise a fait la
moitié du travail : les trois faux « sur-confinements » sont devenus des **« non concluant »**, ce
qui est le verdict honnête. Mais les trois sondes rendent toujours 255 **après six tentatives
étalées sur 6,3 secondes**.

Un cgroup occupé se libère ; ceci ne se libère pas. L'hypothèse à instruire devient donc : le
**conteneur lui-même** ne répond plus. `exceed_pid_quota` le tuerait — directement ou par épuisement
— auquel cas aucune reprise ne peut aboutir, et ce n'est pas de la contamination mais une
**destruction**. C'est `W5.p`, et son test de sortie constate l'état du conteneur après chaque sonde
: une sandbox morte doit être dite morte, et les sondes qui suivent ne doivent pas être rapportées
comme « pas lancées » — elles ne doivent **pas être rapportées du tout**, puisqu'il n'y avait plus
rien pour les lancer.

L'hypothèse n'est pas retenue ici. `W5.k` a coûté assez cher pour qu'on ne conclue plus avant
d'avoir regardé.

---

## 2026-08-19 — W5.p — Une sandbox morte est dite morte, et on cesse de lui parler

**Périmètre.** `apps/locus-execd/src/linux/driver.rs` (`is_running`), `src/linux/selftest.rs` (la
troisième ignorance), et le test. Aucune sonde modifiée.

**Ce que `W5.o` supposait, et que l'hôte a démenti.** La reprise faisait retenter les lancements que
le runtime refusait, en supposant la cause **transitoire** — un cgroup occupé se libère. Le premier
passage réel a rendu les trois sondes toujours en 255 après six tentatives étalées sur plus de six
secondes. **Ce qui ne se libère pas n'était pas occupé.**

**Une troisième ignorance, et elle ne se range avec aucune des deux autres.** 120 dit « ce que je
devais lire n'était pas là ». 121 dit « ce que je devais atteindre n'a pas répondu ». `SANDBOX_GONE`
dit **« il n'y avait rien pour me lancer »** — et elle se répare encore ailleurs : pas en complétant
l'image, pas en changeant d'hôte, mais en comprenant ce qui a tué la sandbox. Rapporter ces sondes
comme « le runtime n'a pas pu » enverrait chercher un runtime fatigué là où il n'y a plus de
conteneur.

**Décisions prises.**

_`is_running` rend trois réponses, parce qu'il y a trois états._ `Some(true)` : elle tourne.
`Some(false)` : le runtime a répondu, et elle ne tourne plus. `None` : le runtime n'a pas répondu,
ou a répondu quelque chose qu'on ne sait pas lire. Un booléen forcerait la troisième dans l'une des
deux autres — et vers `false`, un runtime muet ferait déclarer mortes des sandboxes bien vivantes.
C'est exactement la faute que `W5.n` et `W5.o` ont passé deux sprints à retirer d'ici.

_Une réponse illisible n'est pas une mort._ La première rédaction lisait `stdout.trim() == "true"`,
ce qui range tout ce qui n'est pas « true » — y compris `<no value>` — avec « ne tourne plus ».
Trois mutants l'ont montrée en même temps. La lecture est désormais explicite : « true », « false »,
et **rien d'autre**.

_Le constat de vie est dans la boucle de reprise, pas après elle._ Placé après, il laissait brûler
le budget entier — six tentatives — contre un conteneur mort, pour chacune des sondes restantes. Un
mutant a montré qu'il devait aussi valoir pour la **dernière** tentative : sans cela, une sandbox
qui meurt au sixième essai serait rapportée « le runtime n'a pas pu ». Un double qui refuse cinq
fois puis meurt le tient.

_Le rapport reste complet._ Les sondes d'après la mort y figurent toutes, avec leur raison, et ne
sont **pas lancées** — pas même une fois. Une suite tronquée se lirait comme une suite passée ; une
suite qui relancerait seize fois six tentatives contre un conteneur mort paierait une minute pour
réapprendre ce qu'elle sait.

**Tests exécutés.** `cargo test -p locus-execd --test selftest` → 37 conformes, dont quatre neufs.
`npm run check` → les dix portes vertes. Mutation : sept mutants, **sept tués**, zéro survivant —
après trois passages, chacun ayant révélé un vrai trou plutôt qu'un mutant équivalent.

_Note de méthode, la seconde de la session._ Deux éditions ont été perdues parce que `cargo fmt`
reformate entre l'écriture du motif et sa recherche : un script qui remplace du texte exact doit
relire le fichier après chaque formatage. Et le harnais de mutation restaure son instantané à la fin
— un instantané pris avant une correction l'annule silencieusement.

**Le passage réel a démenti l'hypothèse qui a fait écrire ce sprint — pour la seconde fois.** Les
trois sondes rendent toujours « le runtime a rendu son code d'erreur générique », **jamais**
`SANDBOX_GONE`. Autrement dit `is_running` a répondu autre chose que `Some(false)` : le conteneur
est vivant, et `podman exec` refuse quand même.

Ce n'est donc ni un cgroup occupé qui se libère — `W5.o` l'a écarté — ni une sandbox morte. Ce que
ce sprint a livré reste vrai et utile : la troisième ignorance est nommable, le constat de vie ne
sur-affirme plus, et la reprise ne brûle plus son budget contre un conteneur mort. Ce qu'il n'a pas
livré est la cause.

**Ce qu'on sait maintenant, et qui est étroit.** Après `exceed_pid_quota`, sur ce runner : le
conteneur tourne, il répond à `inspect`, et `podman exec` rend 255 pendant plus de six secondes
d'affilée. Trois hypothèses restent debout, et aucune n'est retenue ici — le cgroup PID reste saturé
au-delà du budget parce que le nettoyage de la sonde n'a pas tourné ; `podman exec` échoue pour une
raison indépendante des PID ; ou le 255 vient d'ailleurs que d'un refus de fork. Les départager
demande de lire ce que `podman exec` écrit sur son erreur, que le harnais jette aujourd'hui.

C'est `W5.q`, et c'est la troisième fois que cette enquête resserre sa question au lieu de la
résoudre. Chaque tour a laissé le harnais plus honnête ; aucun n'a eu le droit de conclure à sa
place.

---

## 2026-08-19 — W5.q — Ce que le runtime écrit en refusant cesse d'être jeté

**Périmètre.** `apps/locus-execd/src/linux/selftest.rs` (`Trial::detail`), le rendu de la table dans
`tests/host_sandbox.rs`, et quatre tests. Aucune sonde modifiée, aucun verdict déplacé.

**Ce qu'il restait à lire.** Trois sprints ont resserré la question sans la résoudre. `W5.n` a
catalogué 255 ; `W5.o` a supposé la cause transitoire et l'hôte l'a démentie ; `W5.p` a supposé la
sandbox morte et l'hôte l'a démentie aussi — `is_running` répond, le conteneur **tourne**, et
`podman exec` refuse pendant plus de six secondes. À ce point le harnais avait montré tout ce qu'il
gardait. Il gardait le code de sortie. Il **jetait** ce que le runtime écrit sur son erreur, et
c'était la seule pièce jamais lue.

**Décisions prises.**

_Le détail est une chaîne facultative, pas une chaîne._ `detail: Option<String>`. Un refus muet rend
`None` ; il ne rend pas `""`. Les deux se ressembleraient dans un rapport où chaque ligne porte une
chaîne, et la distinction qu'on est venu chercher — qui parle, qui se tait — repasserait à l'œil du
lecteur au lieu d'être portée par le rapport. Un runtime qui ferme son flux sur un `\n` n'a rien dit
non plus : le détail est nettoyé, et de l'espace n'est pas une parole.

_Trois absences, trois noms, et aucune ne se collapse dans les autres._ Une sonde qui **aboutit** ne
porte pas de détail — commenter les succès noierait les refus qui, eux, s'expliquent. Une sonde
**jamais lancée** n'en porte pas davantage, et surtout n'emprunte pas sa `reason` : la `reason` est
ce que _nous_ avons constaté, le détail est ce que le _runtime_ a écrit. Les recopier l'une dans
l'autre donnerait à une absence l'allure d'un refus motivé. C'est la règle de tout ce fichier — «
pas vérifié n'est jamais réussi », et ici « pas lancé n'est jamais refusé ».

_La table imprime le détail sous la ligne, pas dedans._ `↳` en retrait, sous sonde/code/verdict. Le
message d'un runtime fait parfois plusieurs lignes ; l'aligner en colonne casserait la table
exactement quand elle devient utile.

**Ce que le compilateur garantit, et ce qu'un test doit garantir.** `Trial::not_run` est une
`const fn` : y fabriquer une `String` ne compile pas, donc une absence **ne peut pas** porter de
détail par inadvertance. Mais un mutant qui ne compile pas n'est pas un mutant tué — la garantie
disparaîtrait avec la constance, sans qu'aucun test proteste. Le mutant lève donc la `const` en même
temps, et c'est un test qui répond.

**Tests exécutés.** `cargo test -p locus-execd` → 42 conformes, dont quatre neufs. `npm run check` →
les dix portes vertes. Mutation : cinq mutants, **cinq tués**, zéro survivant. Deux passages : le
premier a laissé survivre « le détail n'est plus nettoyé » — aucun test ne fixait le nettoyage,
parce que le double d'alors écrivait un message sans saut de ligne, ce qu'aucun runtime réel ne
fait.

**L'instrument a répondu du premier coup, et en une ligne.** Le passage de CI de cette PR même a
rendu le `stderr`, et il dit autre chose que les trois hypothèses tombées :

- `exceed_pid_quota` → code **2**, `sh: can't fork: Resource temporarily unavailable`. Le shell de
  la sonde est **mort** au premier fork refusé : son `kill $pids; wait` n'a jamais tourné. Le
  commentaire de `PID_QUOTA` annonçait exactement ce risque résiduel — « si le shell lui-même meurt
  de ne pouvoir forker, le nettoyage ne tourne pas » — et le voilà constaté plutôt que craint.
- les **quatre** sondes suivantes → code **255**, `container create failed (no logs from conmon)`.
  `podman exec` crée un `conmon` par session ; ce `conmon` naît dans le cgroup PID du conteneur, qui
  est encore à `pids.max` ; il meurt avant d'écrire son JSON de synchronisation, et Podman lit un
  tuyau vide comme son code générique.

Ni cgroup transitoire, ni sandbox morte : **un cgroup saturé que plus personne ne peut vider**. Ce
qui explique aussi pourquoi la reprise de `W5.o` n'aboutissait pas — il n'y avait rien à attendre.

**Ce que cela ouvre, et qui n'est pas une réparation de sonde.** Aucune sonde ne peut promettre de
survivre à ce qu'elle épuise ; un nettoyage plus soigneux resterait une discipline. La suite est
donc structurelle : **une sonde par sandbox**, et la contamination cesse d'être évitée pour devenir
inexprimable. C'est `W5.r`.

_Un défaut de ce sprint, trouvé par le même passage._ Le détail s'imprimait avec un `\n` en tête et
aucun en queue : une ligne vide avant chaque détail, et la ligne suivante du tableau collée derrière
lui. Le tableau devenait illisible exactement là où il devient utile, et c'était le livrable. Le
rendu est corrigé, le retrait vaut pour **toutes** les lignes d'un détail, et il est désormais tenu
par un test **non `#[ignore]`** — sinon la faute aurait attendu un hôte pour être vue une seconde
fois.

---

## 2026-08-19 — W5.r — Une sonde par sandbox, et la contamination devient inexprimable

**Périmètre.** `apps/locus-execd/src/linux/selftest.rs` (`run_suite`, `run_alone`, `certify`,
`Trial::refused`, `SANDBOX_REFUSED`), l'export, et les deux fichiers de tests. Aucune sonde
modifiée, aucun verdict déplacé.

**Le défaut, après quatre sprints à tourner autour.** `exceed_pid_quota` sature délibérément le
quota de PID. `W5.n` a découvert que les sondes suivantes n'étaient plus lançables, `W5.o` a fait
retenter en supposant la cause transitoire, `W5.p` a écarté la sandbox morte, `W5.q` a fini par lire
ce que le runtime écrivait — et la réponse tient en deux lignes de `stderr` :

- `exceed_pid_quota` rend **2**, `sh: can't fork: Resource temporarily unavailable` — son propre
  shell meurt au premier fork refusé, donc son `kill $pids; wait` ne tourne jamais ;
- les quatre suivantes rendent **255**, `container create failed (no logs from conmon)` —
  `podman exec` crée un `conmon` par session, ce `conmon` naît dans le cgroup PID du conteneur, il y
  est encore à `pids.max`, et il meurt avant d'écrire sa synchronisation.

Un cgroup saturé que **plus personne ne peut vider**.

**Pourquoi ce n'est pas une réparation de sonde.** On pouvait rendre `PID_QUOTA` plus prudente —
s'arrêter avant la limite, attraper le refus dans un sous-shell. Cela aurait marché, et cela serait
resté une **discipline** : quelque chose qui tient jusqu'à ce qu'il ne tienne plus. Aucune sonde ne
peut promettre de survivre à ce qu'elle épuise ; c'est la définition de ce qu'elle épuise.

Ce qui peut être promis, c'est qu'il n'y ait **rien à nettoyer**. Seize sandboxes, seize noms, aucun
état partagé. La contamination cesse d'être évitée : elle devient inexprimable, faute d'endroit où
se produire. Et avec elle disparaissent le drapeau de propagation, la question de savoir si la
reprise est assez longue, et l'ordre de `SUITE` comme variable cachée de ce que les sondes mesurent.

**Décisions prises.**

_`run_suite` prend la spécification, pas un identifiant._ Il n'y a plus de sandbox à ouvrir
d'avance, donc plus rien à passer. C'est la signature qui rend la faute impossible : on ne peut plus
demander à deux sondes de partager un conteneur, parce qu'il n'y a pas d'argument pour le dire.

_`certify` absorbe `assess`._ Avec l'ouverture et le démontage rendus à chaque sonde, « créer,
démarrer, éprouver, retirer » et « juger ce qu'une sandbox a rendu » sont la même opération. Deux
noms pour elle seraient du vocabulaire parallèle, ce que ce dépôt refuse ailleurs.

_Le `Result` de `certify` disparaît, et le rapport y gagne._ L'ancienne signature rendait `Err`
quand le runtime refusait la spécification, avec la bonne raison — « un `Standing` sur zéro
observation serait un verdict sur rien ». Sauf que ce n'est plus zéro observation : c'est seize
absences nommées, chacune portant le message en clair, et le verdict rendu là-dessus est juste —
`NotTrusted`, parce que rien n'a été vérifié. Le `Err` cachait le rapport.

_`SANDBOX_REFUSED` a été renommé en cours de route, et c'est le point le plus intéressant du
sprint._ Il disait d'abord « le runtime a refusé d'ouvrir une sandbox ». Les doubles ont montré que
c'était faux une fois sur deux : `VanishingRuntime` modélise un Podman **tué**, et l'échec
d'ouverture qui s'ensuit n'est pas un refus, c'est un silence. La cause est en dessous —
`expect_success` rend `RuntimeError::Unavailable` pour « binaire introuvable » comme pour « podman a
répondu 125 ». Plutôt que d'inventer ici une distinction que la couche du dessous ne fait pas, le
nom dit ce qu'il couvre vraiment — « la sandbox n'a pas pu être ouverte » — le message part en
`detail`, et la séparation des deux erreurs devient `W5.s`. Un nom qu'on ne peut pas honorer est
pire qu'un nom large.

_Un test s'est révélé vrai par accident, et a été réécrit plutôt qu'assoupli._
`une_sonde_jamais_lancee_ne_porte_aucun_detail` affirmait qu'aucune absence ne porte de détail — ce
qui tenait parce qu'aucune absence n'avait alors de message à porter. Il est devenu
`un_detail_n_est_jamais_une_copie_de_la_raison`, qui énonce l'invariant réel sur les trois chemins
qui produisent des absences. Baisser une assertion parce que le code a changé est la façon ordinaire
de perdre un test ; ici l'assertion visait à côté depuis le début.

**Tests exécutés.** `cargo test -p locus-execd` → 43 conformes. `npm run check` → les dix portes
vertes. Le test de sortie est `aucune_sonde_ne_partage_sa_sandbox_avec_une_autre` : seize
lancements, seize noms **distincts**.

**Ce que ce sprint ne prétend pas.** Il ne dit pas que les quinze sondes tiennent sur le runner —
`exceed_pid_quota` sature toujours son propre cgroup, et ce qu'elle mesure dans sa propre sandbox
reste à constater. Il dit qu'elle ne peut plus faire mentir les autres.

**Le passage réel, et il confirme cette fois.** Les quatre sondes qui suivaient `exceed_pid_quota`
rendent **1, 1, 0 et 0** au lieu de 255. Quatorze des quinze tiennent. `open_outbound_connection` et
`reach_cloud_metadata_service` **réussissent**, ce qui clôt du même coup la question ouverte de
`W5.m` : la route était bien là, et ces deux sondes ne la trouvaient pas parce qu'elles ne
**tournaient pas**. Trois sprints d'hypothèses sur l'`awk` de busybox et sur deux `/proc` différents
portaient sur des sondes qui n'avaient jamais été lancées.

Reste une seule dissidente, `reach_host_kernel_interfaces`, et son motif est en clair sous la ligne
— `head: /sys/kernel/vmcoreinfo: No such file or directory`. C'est `W5.i`, déjà écrit.

Coût mesuré : **180 s contre 39 s**, pour seize fois plus de conteneurs.

_Un défaut trouvé en chemin, et un faux coupable._ Le job a **paru** pendre pendant vingt minutes.
L'hypothèse construite là-dessus — une sandbox saturée en PID bloquant son propre démontage — était
**fausse** : le job avait fini en 3 min 36, et c'est l'état rapporté par l'API qui était périmé. Ce
que la fausse piste a fait trouver est réel : `SystemRunner::run` appelait `Command::output()`, sans
borne, au seul endroit du dépôt qu'aucun test ne traversait et contre sa propre règle — « timeouts
et cancellation ». `W5.r` fait passer ces appels non bornés d'une poignée à quatre-vingts par
campagne. La borne est posée, elle vaut 60 s, et deux tests la tiennent **sans Podman** — en visant
`sleep` pour l'abandon et `echo` pour la sortie intacte, parce qu'une borne qui tuerait tout
passerait le premier test seul. C'est `W5.t`.

---

## 2026-08-19 — W5.j — Le quota disque s'applique là où la sandbox peut écrire

**Périmètre.** `packages/execution/src/selftest.rs` (`Requirement`, `expectation`, `judge`,
`standing`), `apps/locus-execd/src/linux/plan.rs` (`QuotaTarget`, le refus),
`src/linux/invocation.rs` (l'application), `src/linux/selftest.rs` (la sonde, la variable, le code
122), et les quatre fichiers de tests.

**Le défaut, et pourquoi il était invisible.** `disk_bytes` devenait un `--storage-opt size=`, qui
dimensionne la couche inscriptible du conteneur. C'est juste tant que la racine est inscriptible. À
partir de `S2`, le plan la monte en lecture seule : la couche dimensionnée est alors une couche que
**personne n'écrit**, et le seul endroit inscriptible est l'espace de travail monté, qui hérite du
système de fichiers de l'hôte et n'est borné par rien. Le quota était déclaré, accepté, transmis au
runtime — et sans effet. C'est la forme la plus tranquille d'une garantie absente : tout le chemin a
l'air de fonctionner.

Et la sonde chargée de le constater écrivait à la racine. Elle était donc bloquée **avec ou sans
quota**, et ressortait « bloquée → tient » sous une mission qui n'en réservait aucun. `W5.f` l'avait
nommé sans pouvoir le réparer : « une sonde qui passe sans que ce qu'elle teste existe est le pire
des trois états, parce qu'elle ne se plaint jamais ».

**L'arbitrage, rendu.** Le quota mord **là où la sandbox peut écrire**, et le plan le nomme.

| Cas                              | Où                              | Comment               |
| -------------------------------- | ------------------------------- | --------------------- |
| Aucun quota réservé              | nulle part                      | rien n'est émis       |
| Racine inscriptible (`S0`, `S1`) | la couche du conteneur          | `--storage-opt size=` |
| Racine en lecture seule (`S2`+)  | le premier montage inscriptible | volume dimensionné    |

_Le tmpfs borné est écarté._ Il marcherait sur tout système de fichiers, et c'est ce qui le rend
tentant. Mais c'est de la **RAM** : une réservation de disque viendrait manger la réservation de
mémoire, deux budgets pour une ressource. Ce dépôt refuse ce genre de collapse partout ailleurs.

_Le volume dimensionné demande XFS avec quotas de projet_, c'est-à-dire exactement le fait que
`W5.g` fait déjà lire à l'hôte **avant toute création** et refuser à l'admission quand il manque. Le
chemin est cohérent de bout en bout : l'hôte est interrogé, la mission refusée si l'hôte ne sait
pas, et le volume dimensionné seulement là où il mordra.

_Un quota là où rien n'est inscriptible est refusé au plan._ C'est le troisième cas, celui qu'on
aurait oublié : une sandbox à `S2` sans espace de travail n'a aucun endroit où écrire, et accepter
un quota en silence recommencerait la faute qu'on répare.

_Le premier montage inscriptible est **désigné**, pas réparti._ Une mission peut en monter plusieurs
; répartir un quota unique entre eux demanderait une règle que rien dans `SandboxSpec` ne donne, et
l'inventer produirait une borne que personne n'a demandée. Un test l'épingle : le jour où une
mission en aura besoin de deux, ce sera un item, pas une surprise.

**La moitié qu'on n'avait pas vue venir : l'attente dépend de la mission.** `contained_from` dit à
partir de quel **niveau** une sonde doit être contenue, en supposant que ce qu'elle éprouve existe
toujours. Vrai pour quinze sondes sur seize — un namespace, un profil seccomp, une capability
retirée sont des propriétés du niveau. Le disque est l'exception, et une seule chose la crée :
`ResourceSpec` **refuse** un quota nul pour le CPU, la mémoire, les PID et l'horizon, et **accepte**
zéro pour le disque. C'est la seule ressource facultative du système.

Sans `Probe::requires`, `exceed_disk_quota` serait `Contained` dès `S2` quoi qu'ait déclaré la
mission — donc `Escaped` chaque fois qu'une mission ordinaire, sans quota, écrit dans son espace de
travail. Le harnais le cachait en la laissant bloquer par la racine en lecture seule.

`expectation`, `judge` et `standing` prennent donc la réservation. C'est `ResourceSpec` entier qui
voyage, et non un booléen « quota déclaré » : le booléen serait un second vocabulaire pour ce que la
réservation dit déjà, et il faudrait le tenir à jour le jour où une deuxième ressource devient
facultative.

**Un code réservé de plus, et il ne se range avec aucun des deux autres.** 120 dit « ce que je
devais **lire** n'était pas là ». 121 dit « ce que je devais **atteindre** n'a pas répondu ». 122
dit « ce sur quoi je devais **écrire** ne s'écrit pas » — et il se répare **dans le plan**, qui a
désigné une cible que la sandbox ne peut pas écrire. Sans lui, déplacer la sonde vers l'espace de
travail n'aurait fait que déplacer le piège d'un cran.

**Le second test hôte passe de quinze à seize sondes.** Son exclusion nommée était honnête tant que
l'attente ne dépendait que du niveau ; elle n'a plus lieu d'être. Le succès de la sonde sans quota
déclaré dit maintenant quelque chose de vrai : le plan n'a désigné aucune cible, la sonde l'a lu, et
elle n'a pas prétendu mesurer une borne absente.

**Tests exécutés.** `cargo test` → 47 conformes pour `locus-execd`, 19 pour le domaine.
`npm run check` → les dix portes vertes. Mutation : **onze mutants, onze tués**, zéro survivant.

_Deux notes de méthode, et la seconde a coûté un fichier._ `cargo fmt` a de nouveau remodelé trois
motifs entre leur écriture et leur recherche — la leçon est connue, elle se répète. Plus grave : le
harnais nommait ses instantanés d'après le seul **nom de fichier**, et deux fichiers du dépôt
s'appellent `selftest.rs`. Les deux ont partagé le même instantané, et la restauration a écrit le
contenu du domaine dans le harnais. Le fichier a été repris de `HEAD` et les six éditions
réappliquées ; le script nomme désormais ses instantanés d'après le chemin relatif entier.

_Une conséquence non vue, et le job de CI l'a payée en direct._ Remplacer `--storage-opt size=` par
un volume dimensionné change ce que fait un hôte **non-XFS** : `podman create` y échouait, il
réussit désormais. La sonde, qui écrivait quatre gigaoctets en dur, s'est donc mise à les écrire
pour de bon sur le disque d'un runner. Une sonde qui éprouve une borne doit écrire **juste au-delà**
de cette borne, pas un nombre rond choisi d'avance : `LOCUS_QUOTA_BYTES` voyage maintenant avec la
cible — aucun des deux n'ayant de sens sans l'autre — et le dépassement vaut la réservation plus
soixante-quatre mébioctets. Ce que l'épreuve coûte est ainsi proportionné à ce que la mission a
demandé.

_Un survivant qui écrivait quatre gigaoctets pour passer._ Le mutant remplaçait « pas de cible →
sortir » par « pas de cible → écrire à la racine », et le test, qui ne vérifiait que le code de
sortie, le laissait vivre. Le test épingle désormais **qu'elle n'essaie pas** : `PATH` vidé, `dd`
introuvable, et la sonde correcte n'en a pas besoin parce qu'elle sort avant.

---

## 2026-08-19 — W5.s — Un runtime qui ne répond pas n'a rien refusé

**Périmètre.** `apps/locus-execd/src/runtime.rs` (`RuntimeError::Refused`), `src/linux/driver.rs`
(`expect_success`), `src/linux/selftest.rs` (`Trial::refused`), et trois tests.

**Le défaut, et où il était visible.** `expect_success` rendait `Unavailable` pour « le binaire est
introuvable » comme pour « podman a répondu 125 ». Les deux tests qui l'épinglaient étaient **côte à
côte** dans `podman.rs` — `un_hote_sans_podman_le_dit_au_lieu_de_pretendre` et
`un_code_de_sortie_non_nul_devient_une_erreur_qui_porte_stderr` — et affirmaient la **même**
variante pour les deux causes. Le défaut était écrit, lisible d'un coup d'œil, et il passait.

Il a fallu `W5.r` pour qu'il coûte quelque chose : en faisant remonter le motif jusqu'au rapport de
sondes, un Podman tué s'est mis à produire « la sandbox a été refusée », alors qu'il n'y avait eu
aucun refus — seulement un silence. Le nom du motif avait dû être élargi faute de pouvoir tenir la
distinction ; il la tient maintenant.

**Décisions prises.**

_Le verbe et le code voyagent séparément du texte._ `Refused { verb, code, detail }` plutôt qu'un
message formaté. Un appelant qui veut décider — retenter, abandonner, changer d'hôte — ne devrait
pas avoir à analyser une phrase pour retrouver un entier. C'est la leçon de `W5.m`, une couche plus
bas : le code voyage **à côté** du verdict, jamais dedans.

_`Trial::refused` choisit sur la variante, plus sur un texte._ `Unavailable` rend
`UNREACHABLE_RUNTIME` et aucun code, parce qu'il n'y en a pas eu ; `Refused` rend `SANDBOX_REFUSED`
et le code du runtime. `Unsupported` — le backend refuse avant de demander — reste un refus.

_Le `Display` ne change pas d'un caractère._ « podman create a rendu 125 : … » est le même texte
qu'avant, donc rien de ce qui lit le message ne bouge. Ce qui change est ce que le **type** permet
de distinguer.

**Tests exécutés.** `cargo test -p locus-execd` → 47 + 22 conformes. `npm run check` → les dix
portes vertes. Le test de sortie est une paire, et c'est le point : sans la seconde moitié — « un
runtime absent reste `Unavailable` » — faire rendre `Refused` à tout le monde passerait la première.
