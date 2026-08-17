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
