# Roadmap V1 multi-repos

Ce n'est pas une roadmap de MVP. Tous les workstreams appartiennent à la V1 finale ; l'ordre
reflète les dépendances de construction.

Granularité décroissante assumée. W0 et W2 sont découpés au commit près, parce qu'ils sont
immédiats et que leurs dépendances sont connues. W3 à W7 sont au groupe de commits. W8 à W12
restent au workstream : les découper finement aujourd'hui produirait un plan faux. **Chaque
workstream est redécoupé au commit près quand il devient le prochain**, dans son
`IMPLEMENTATION_STATE.md`.

La cible que cette V1 doit atteindre est énoncée dans `docs/13`, sur une échelle de dynamisme à cinq
niveaux : chaque workstream de W13 à W18 déclare le niveau qu'il fait franchir et sur quelle sorte de
relation de coordination. Un workstream qui ne saurait pas le dire n'aurait pas de critère de fin.

Un item est terminé quand son **test de sortie passe en CI**, pas quand le code est écrit.

Convention : `[R]` = réversible par `git revert` seul ; `[M]` = comporte une migration de schéma
ou de données et exige un plan de rollback dans le ledger.

---

## État de départ, vérifié

Ne pas repartir des suppositions du package d'origine — elles ont été confrontées aux dépôts
réels le 2026-08-07.

| Dépôt | État | Conséquence |
|---|---|---|
| `locusolus` | vide (LICENSE + README, commit `7dc4dd1`) | greenfield intégral ; rien à préserver, aucun audit antérieur |
| `canterel` | fork non divergé de `synthetic-sciences/OpenScience`, HEAD `c3f734c` | rename fait, rebrand interdit (ADR 0010), `src/locus/` absent |
| `xiiif` | v0.4.0 mûr, 23 `.el`, 33 fichiers de tests | l'essentiel de sa V1 existe ; six items déverrouillés |
| `emacs-config` | privé, non inventorié | l'inventaire est la première tâche de ce dépôt |

Décisions déjà tranchées : le projet s'appelle `locusolus` ; quatre dépôts, client Emacs dans
`apps/emacs/` (ADR 0009) ; fork suiveur pour Canterel (ADR 0010).

Un second constat, établi le 2026-08-17 : **§7.1 (agrégats d'agents et d'équipes), §13 (orchestrateur
de portefeuille), §16 (mémoire) et §20 (policy engine) ne sont couverts par aucun workstream.** W7
attrape le portefeuille de §13 par ses seuls indicateurs et la revue de §17 ; rien n'assigne
`AgentTemplate`, `AgentInstance`, `Team`, `Decision`, `ApprovalRequest`, les actions de §13.5,
l'anti-gaming de §13.6, les sept niveaux de mémoire, ni la DSL de politique. C'est un trou de
couverture, pas un choix : le dépôt sait décrire ce qui est cru et pourquoi, et ne sait rien dire de
qui travaille. W13 comble le socle, W14 à W18 portent la cible, et ADR 0016 en fixe les bornes.

Une décision reste ouverte : **le langage de `locusd`**. `SPEC_V1.md` §4.5 fixe TypeScript comme
référence en précisant que le choix est remplaçable. Il l'est moins qu'il n'y paraît, puisque le
worker vit dans un fork TypeScript et qu'un SDK TS existera donc de toute façon. Une autre langue
côté serveur est architecturalement propre — c'est le cas prévu par `docs/06` (« schemas JSON
versionnés et fixtures inter-SDK ») — mais double la suite de contract tests, qui devient alors le
vrai garde-fou du projet plutôt qu'un filet. W0 est agnostique : il produit d'abord les JSON
Schemas, communs aux deux options. La décision peut donc attendre la fin de W0, pas au-delà — W1
commence par du code de domaine.

---

## W0 — Baseline et contrats

Le seul workstream dont tout le reste dépend. Objectif : que `packages/protocol` existe, soit figé
en `lep/1.0`, et soit consommable par deux implémentations indépendantes.

| # | Commit | Dépôt | Test de sortie |
|---|---|---|---|
| W0.1 `[R]` | placement de la doc, `CLAUDE.md` par repo, ADR 0001–0010 | les 4 | `grep -r "locus-solus"` ne renvoie rien hors historique Git |
| W0.2 `[R]` | squelette monorepo : `apps/`, `packages/`, `schemas/`, `tests/`, tooling, CI qui passe à vide | locusolus | CI verte sur un dépôt sans code |
| W0.3 `[R]` | garde de frontières architecturales (les 5 règles du `CLAUDE.md`) branchée sur le squelette vide | locusolus | une violation délibérée fait échouer la CI |
| W0.4 `[R]` | `packages/protocol` : IDs, enveloppe d'erreur structurée, politique de versionnement, horodatage | locusolus | unitaires |
| W0.5 `[M]` | JSON Schemas LEP : `CapabilityManifest`, `MissionEnvelope`, `ContextView`, `EnvironmentBlueprint`, `SandboxSpec`, `ResourceSpec` | locusolus | les exemples de `schemas/examples/` valident |
| W0.6 `[M]` | JSON Schemas LEP, suite : `Lease`, `Attempt`, événements, `ArtifactManifest`, `RunManifest`, `SandboxAttestation`, `EpistemicCommit` | locusolus | validation |
| W0.7 `[R]` | corpus de fixtures : nominal, refus d'admission, reconnexion, résultat tardif, dépassement de budget | locusolus | chaque fixture valide **ou invalide intentionnellement**, selon son `expect` déclaré |
| W0.8 `[R]` | SDK généré depuis les schémas + `schema-registry` avec négociation de features au handshake | locusolus | round-trip sur toutes les fixtures |
| W0.9 `[R]` | `packages/testing` : **harness de conformance LEP côté serveur** — handshake, offre, lease, heartbeat, expiration, acquittements | locusolus | le harness se teste contre un worker factice |
| W0.10 `[R]` | `IMPLEMENTATION_LEDGER.md` dans les quatre dépôts | les 4 | présent, avec l'entrée d'étape 0 |

Fin de W0 : `lep/1.0` est gelé. Toute évolution suit `docs/06` — majeur = rupture, mineur = champs
optionnels compatibles.

---

## W2 — LEP et worker Canterel — **exécutable dès la fin de W0**

Numéroté W2 par dépendance logique, mais **il ne dépend pas de W1**. Le harness de W0.9 joue le
rôle de serveur. C'est la parallélisation principale du chantier : `locusd` part de zéro et
représente l'essentiel du travail, alors que le worker a un socle existant. Écrire le worker
contre un faux serveur oblige en outre le protocole à être suffisant avant que le vrai serveur
puisse compenser ses lacunes.

| # | Commit | Test de sortie |
|---|---|---|
| W2.1 `[R]` | remote `upstream` + `docs/locus/upstream.md` + politique de sync | un merge amont à blanc ne touche aucun fichier local |
| W2.2 `[R]` | non-régression standalone en CI (§28.8) — **avant tout code `locus/`** | passe sur le HEAD actuel |
| W2.3 `[R]` | `src/locus/{index,config,errors}.ts` + `canterel worker --locus` qui ne fait rien | `bun run check` vert ; standalone intact |
| W2.4 `[R]` | `identity.ts`, `auth.ts`, enrôlement, révocation (§7) | identité persistante après redémarrage |
| W2.5 `[R]` | `protocol.ts`, `schema-registry.ts`, `connection.ts` sur le SDK de W0.8 | contract tests contre le harness |
| W2.6 `[R]` | `capability-manifest.ts` + `capability-watch.ts` — détection réelle des toolchains, modèles, accélérateurs **et du niveau de sandbox effectif** | sur macOS : annonce `["S1","S2"]` et `mps`, jamais plus |
| W2.7 `[R]` | `registration.ts`, handshake complet | conformance §8.2 |
| W2.8 `[R]` | `admission.ts` — validation, refus structuré (§10.2), politique locale plus restrictive | la fixture de refus de W0.7 produit le bon code d'erreur |
| W2.9 `[R]` | `lease.ts`, `attempt.ts`, heartbeats, perte de lease (§11) | expiration et reprise contre le harness |
| W2.10 `[R]` | `context-materializer.ts` + isolation informationnelle (§12.4) | un contexte de branche A n'atteint jamais une mission de branche B |
| W2.11 `[R]` | `session-map.ts`, `agent-overlay.ts`, `model-policy.ts`, `tool-policy.ts` — couche d'adaptation vers l'amont, à garder mince | mission → session **sans modifier `src/session/`** |
| W2.12 `[R]` | `event-bridge.ts`, `event-spool.ts`, coalescence (§18) | perte de connexion : rien perdu, rien dupliqué |
| W2.13 `[R]` | `usage-meter.ts`, budget local, dépassement (§17) | arrêt propre au dépassement |
| W2.14 `[R]` | `artifact-client.ts`, `artifact-scanner.ts`, déclaration avant upload (§19.1) | hash déclaré ≠ hash reçu → rejet |
| W2.15 `[R]` | `epistemic-commit.ts` — jamais au-delà de `staged` (§2.3) | tentative de promotion → erreur structurée |
| W2.16 `[R]` | `recovery.ts`, `resume-store.ts`, offline, résultats partiels (§24) | redémarrage du worker en cours de mission |
| W2.17 `[R]` | `human-input.ts` (§22) | suspension sans processus coûteux maintenu |
| W2.18 `[R]` | `ui/worker-status.ts`, `mission-view.ts`, `security-view.ts` | rendu |
| W2.19 `[R]` | suite de conformance complète + consumer-driven contracts (§28.2/28.3) | verte contre le harness |

La liste de fichiers de `repos/canterel/SPEC_V1.md` §4 est une **annexe indicative**, pas un
gabarit. Ne crée pas 34 stubs vides : chaque commit ci-dessus livre une garantie testée, et les
fichiers apparaissent quand ils portent du comportement.

---

## W1 — Locus domain / event store — parallèle à W2

| Groupe | Contenu | Test de sortie |
|---|---|---|
| W1.a `[R]` | enveloppe commune d'objet épistémique (§7.4) : identité, version, statut, niveau de validation, portée de branche, provenance, supersession | property tests sur les invariants |
| W1.b `[R]` | agrégats organisationnels (§7.1) et objets épistémiques (§7.3) | property tests |
| W1.c `[M]` | `packages/event-store` : enveloppe (§10.1), append-only logique, concurrence optimiste | replay complet + conflit de concurrence détecté |
| W1.d `[M]` | projections reconstructibles | reconstruction depuis zéro = état courant |
| W1.e `[R]` | `packages/graph` : relations typées, **hyperarêtes** pour les inférences multi-prémisses (§7.6) | une inférence à 3 prémisses n'est pas 3 liens |
| W1.f `[R]` | validation épistémique (§8) et propagation de l'invalidation (§8.3) | invalider une prémisse propage correctement |
| W1.g `[R]` | résultats négatifs et conflits (§18.7) | aucun chemin de code ne supprime un conflit |
| W1.h `[M]` | migrations de schéma + tests de portabilité | migration aller-retour |

---

## W3 — Workflow abstraction

Backend déterministe de test **avant** Temporal (ADR 0003). Si Temporal vient en premier, le
domaine s'y adapte silencieusement et l'ADR devient une intention.

`W3.a` définitions indépendantes du backend + règles de déterminisme (§11.3) · `W3.b` port
`WorkflowBackend` + backend déterministe de test · `W3.c` workflows obligatoires (§11.2) sur le
backend de test · `W3.d` backend Temporal, sous `packages/workflow-backends` uniquement ·
`W3.e` crash/restart/replay sur les deux backends ; compensation qui n'efface aucun fait observé.

---

## W4 — Execution Fabric

`W4.a` `SandboxSpec`, `ResourceSpec`, `SandboxAttestation` · `W4.b` **suite de self-tests indexée
par niveau S0–S4**, chaque test déclarant le niveau auquel il doit échouer côté sandbox ·
`W4.c` `locus-execd`, seul détenteur du socket runtime · `W4.d` backend Linux rootless, cgroups v2,
seccomp · `W4.e` backend macOS : VM Linux légère + containers rootless par mission ·
`W4.f` worker macOS de confiance annonçant `mps` · `W4.g` scheduler : placement par capability +
trust + localité + fit + budget, admission, refus, reroutage.

W4.b avant W4.c : la suite de tests définit ce que « sandbox » veut dire dans ce projet, et un
backend qui échoue un test critique n'est pas `trusted`.

**W4.d se découpe en deux commits**, sur la forme d'ADR 0012 (le port avant le driver) et d'ADR 0015
(la traduction avant le fil). Elle vaut ici plus qu'ailleurs : le plan de rollback d'ADR 0004 dit
qu'il n'y a « aucun chemin de repli acceptable », et un driver écrit avant sa traduction confinerait
de travers en silence.

| # | Commit | Test de sortie |
|---|---|---|
| W4.d.1 `[R]` | la traduction `SandboxSpec` → plan de confinement rootless, et la lecture de ce que l'hôte permet | le plan ne concède jamais plus que le niveau exigé — monter d'un niveau ne relâche rien et change quelque chose — il refuse par leur nom la micro-VM, l'enclave et un mode réseau sans namespace pour le porter, et la lecture de l'hôte nomme ce qui manque au lieu de le supposer |
| W4.d.2 `[R]` | le driver rootless : `RuntimePort` implémenté sur Podman rootless (`docs/03`), la sandbox créée, l'attestation lue de ce qui tourne | le driver demande au runtime exactement ce que le plan a décidé, et il atteste de ce qu'il **observe** — un confinement plus faible que demandé apparaît dans l'attestation et `conformance` le refuse |
| W4.d.3 `[R]` | la suite de W4.b passée contre le backend : une commande par sonde, et le `Standing` qui en sort | la suite rend un `Standing` pour ce backend ; une sonde non exécutée est `NotRun` avec sa raison, jamais un succès, et un hôte sans runtime n'obtient jamais `Trusted` |
| W4.d.4 `[R]` | la vérification du profil seccomp restreint apporté par le déploiement | un profil qui ne refuse pas ce que la posture promet est refusé, et le refus nomme **tous** les appels manquants |

Le plafond de ce backend est `S3` et c'est une constante, pas une ambition : `S4` est une micro-VM
et `S5` une enclave distante.

**W4.e n'ouvre pas `S4`.** `docs/03` recommande sur macOS « host macOS + VM Linux légère + containers
rootless par mission » : le conteneur tourne dans un noyau Linux, donc le plan de confinement est
celui de W4.d.1, et ce qui change est **où on lit les faits** — dans l'invité, pas sur l'hôte. Une VM
partagée entre les missions ne tient pas la promesse de `S4 microvm-high-risk`, qui est qu'une mission
à haut risque ait **son propre** noyau. Le jour où un déploiement créera une VM par mission, ce sera
un autre backend, avec son propre plafond et sa propre suite.

| # | Commit | Test de sortie |
|---|---|---|
| W4.e.1 `[R]` | la machine macOS : son état, la lecture des faits **dans l'invité**, et le plafond qui en découle | les faits viennent du noyau qui confine et non de celui qui appelle ; une machine arrêtée se distingue d'un noyau incapable ; une VM partagée ne fait jamais franchir `S3` |
| W4.f.1 `[R]` | la portée de l'accélérateur : `mps` n'existe qu'en natif, donc hors du conteneur | sur un hôte natif, une mission a le conteneur **ou** l'accélérateur, jamais les deux, et le refus dit lequel des deux il faut lâcher — distinct de « accélérateur absent », qui appelle une autre action |
| W4.g.1 `[R]` | le placement : choisir parmi plusieurs candidats, sur ce que chacun a **prouvé** | un hôte ne reçoit que le niveau qu'il a prouvé tenir ; le refus dit ce qui manquait à **chaque** candidat ; deux placements du même journal placent au même endroit |
| W4.g.2 `[R]` | le reroutage : une tentative dont l'hôte tombe, une tentative refusée partout | une tâche réattribuée **conserve son numéro d'attempt** (§12.3) ; l'hôte qui l'a perdue n'est jamais rechoisi ; l'épuisement distingue « tous tombés » de « aucun ne convenait » |

La posture seccomp **restreinte** promet le refus, depuis l'intérieur, de la création de namespaces
et du chargement de code noyau. Ce dépôt ne **fournit pas** ce profil : un profil par défaut-refus
est une liste de plusieurs centaines d'appels autorisés dont l'exactitude ne se démontre qu'en
l'exécutant contre des charges réelles, et en écrire un sans hôte pour l'éprouver produirait soit une
sandbox qui casse tout, soit une sandbox qui autorise ce qu'elle prétend refuser. Le déploiement
l'apporte, W4.d.4 le **vérifie**, et tant qu'aucun profil vérifié n'est fourni le backend refuse les
niveaux qui en dépendent — c'est la règle du plafond `S3`, appliquée à une capacité que l'opérateur
apporte.

---

## W5 — Toolchains

`EnvironmentBlueprint`/Builder ; chaîne lockfile → build OCI → SBOM → scan → health checks →
signature → digest ; les treize profils de §19.4 ; capabilities MPS/CUDA de worker. Les fichiers de
`templates/environment/` sont le point de départ — et ce sont bien des points de départ : aucun ne
porte `version:` ni `image:`, tous deux obligatoires au schéma, et `ml-mps.yaml` porte un champ
`trust:` que le schéma ne définit pas.

| # | Commit | Test de sortie |
|---|---|---|
| W5.a `[R]` | `packages/environments` : `ToolchainProfile` et `EnvironmentBlueprint`, avec les invariants que le schéma de W0.5 ne peut pas exprimer | un profil répété, un préféré inférieur au minimum, une variable dont le nom annonce un secret et une image par tag sont refusés, chacun par son nom |
| W5.b `[R]` | la chaîne : lockfile → build → SBOM → scan → tests → signature → digest, comme suite de types | aucun chemin ne signe une image non scannée ni ne publie un digest dont les tests n'ont pas tourné — vérifié par un `compile_fail`, pas seulement par un test |
| W5.c `[R]` | **correction de W4.d.3** : une sonde absente de l'image n'est pas une sonde bloquée | les codes 125, 126 et 127 sont lus comme `NotRun` et non comme `Blocked` ; une image incomplète ne rend jamais un backend `Trusted` |
| W5.d `[R]` | les sondes voyagent avec le harnais : plus aucun binaire attendu dans l'image | aucune sonde ne nomme un chemin de l'image, chacune est du shell que `sh -n` accepte, et une sonde qui n'a pas pu conclure est `NotRun` |
| W5.e `[R]` | le driver de build derrière un port, dans `apps/locus-execd` | le digest vient de la sortie du runtime et non du blueprint ; un build muet n'est pas un succès ; le driver ne sait pas dépasser le deuxième maillon de la chaîne |
| W5.f `[R]` | validation **sémantique** des sondes contre une sandbox réelle | sur un hôte capable de `S2`, chaque sonde produit le verdict que son `contained_from` annonce — c'est la seule chose que ni `sh -n` ni un double ne peuvent dire |

## W6 — Artifact / reproductibilité

Object store, manifests, quarantaine/promotion, `RunManifest`, workflows de reproduction.

L'ordre est celui de W5 : le vocabulaire et ses refus d'abord, dans un paquet sans dépendance ; le
stockage ensuite, derrière un port (ADR 0012). Un manifeste qui ne sait pas dire non n'a pas besoin
d'un object store pour être faux.

| # | Commit | Test de sortie |
|---|---|---|
| W6.a `[R]` | `packages/artifacts` : `ArtifactManifest` et la machine à états quarantaine/promotion | un contenu dont le hash n'est pas celui qui avait été déclaré est refusé ; `declared → promoted` n'existe pas ; un artefact promu ne se déprome pas ; l'histoire des états traversés ne s'efface pas |
| W6.b `[R]` | **correction de W6.a** : le manifeste dit ce que le schéma dit, et la traduction vers le fil existe | un manifeste portant tous les champs facultatifs fait un aller-retour **exact** par `artifact-manifest.schema.json` ; un état, une relation ou une taille hors bornes sont refusés par leur nom ; le domaine ne construit rien que le schéma refuserait |
| W6.c `[R]` | l'object store derrière un port, avec un backend en mémoire pour les tests | aucun octet n'entre sans manifeste déclaré ; la taille annoncée borne l'upload avant de l'accepter ; un contenu non conforme ne laisse rien derrière lui |
| W6.d `[R]` | `RunManifest` relu et jugé, et le niveau de reproductibilité de §19.7 | le niveau se **calcule** depuis ce que le manifeste consigne ; un niveau déclaré au-dessus est refusé en nommant ce qui manque ; `R3` et `R4` ne se lisent dans aucun manifeste seul |
| W6.e `[R]` | workflow de reproduction sur le backend déterministe de W3 | rejouer un `RunManifest` produit les mêmes hashes d'artefacts, et une divergence est un résultat rendu, pas une erreur avalée |

## W7 — Memory / review / portfolio

`ContextView`, retrieval hybride, revue indépendante, budgets, scheduler qualité-diversité.

Deux points faciles à rater et coûteux à réparer : la prévention de contamination (§16.6) doit
être testée par un cas adverse explicite et pas seulement par construction ; l'anti-gaming du
portefeuille (§13.6) doit exister avant que la fonction de valeur pilote des décisions
automatiques.

**Dépend de W13.c et W13.d** (ADR 0016, décision 13) : la revue indépendante suppose des instances
d'agent distinctes et une assignation, sans quoi « qui relit qui » n'a pas d'objet où s'écrire.
**Les deux sont satisfaites depuis la fin de W13.**

L'ordre suit une seule idée : ce qu'un relecteur **ne voit pas** est décidé avant qu'il relise. Le
dossier se fige avant l'attribution (§17.3), l'indépendance se vérifie avant la remise, et
l'anti-gaming existe avant que la valeur pilote quoi que ce soit — dans chaque cas, l'inverse
produit un système qui a l'air de fonctionner.

| # | Commit | Test de sortie |
|---|---|---|
| W7.a `[R]` | `packages/review` : `ReviewDossier` figé, `Review`, `Finding`, et l'attestation d'indépendance de §14.4 | un dossier modifié après attribution change de version ou porte un addendum visible ; deux relecteurs du même groupe d'indépendance ne comptent pas comme indépendants ; une revue sans attestation n'est pas une revue indépendante |
| W7.b `[R]` | prévention de contamination (§16.6), par **cas adverses** | cinq cas, un par forme nommée : transcript du générateur atteignant un relecteur aveugle ; claim réfuté propagé comme contexte par défaut ; donnée confidentielle atteignant un worker non autorisé ; consensus circulaire sans source externe ; contradiction perdue à la synthèse. Chacun échoue **avant** le correctif |
| W7.c `[R]` | `ContextView` : ce qui a été vu, arrêté par hash et par watermark (§16.2) | deux vues du même instant du journal ont le même hash ; une vue qui aurait vu un événement postérieur à son watermark est refusée |
| W7.d `[R]` | rebuttal et méta-revue (§17.6, §17.7) | un rebuttal ne s'écrit pas sans finding ; une méta-revue ne relit pas sa propre revue ; le désaccord survit à la synthèse |
| W7.e `[R]` | budgets : réservation avant exécution, dépassement (§7.2, invariant 6) | une mission sans borne n'est pas admissible ; une réservation refusée n'exécute rien ; un dépassement arrête proprement et le dit |
| W7.f `[R]` | portefeuille : indicateurs de §13, et **l'anti-gaming de §13.6 d'abord** | une stratégie qui optimise l'indicateur sans produire de connaissance est détectée par un test qui la met en œuvre ; l'anti-gaming précède la fonction de valeur dans l'ordre des commits, et un test l'atteste |
| W7.g `[R]` | scheduler qualité-diversité | deux propositions de valeur égale et de diversité inégale ne sont pas départagées au hasard ; le choix est reproductible |

W7.b avant W7.c : un cas adverse écrit contre une `ContextView` déjà là serait écrit pour passer.
W7.f avant tout usage automatique de la fonction de valeur — c'est la mise en garde de ce
workstream, transformée en ordre de commits.

## W8 — Clients

Web workspace + `apps/emacs` (monorepo) ; sandbox inspector ; decisions ; artifacts.

**Premier commit : le test de séparation** (`emacs -Q` avec la seule `load-path` du package). Il
fixe la frontière avant qu'il y ait quoi que ce soit à séparer — le seul moment où c'est gratuit.

Ordre : client/événements → dashboard et buffers → commandes et transient → artefacts et
inspecteur de sandbox → intégrations Org/Magit/Jupyter/xiiif → 3D et WebView. L'inventaire de
`emacs-config` (W0.10) a répondu : **zéro** occurrence de `locus`, `canterel` ou `iiif` dans son
Elisp, donc rien à extraire et rien à réordonner — `apps/emacs` se construit depuis zéro.

| # | Commit | Test de sortie |
|---|---|---|
| W8.a `[R]` | le test de séparation : `apps/emacs` existe, se charge sous `emacs -Q` avec sa seule `load-path` | la frontière 5 passe de « sans objet » à vérifiée ; charger le paquet n'ouvre aucune connexion, n'arme aucun timer et ne tire aucune bibliothèque hors du paquet et d'Emacs ; la version de protocole annoncée est celle de `schemas/`, lue et non recopiée |
| W8.b `[R]` | authentification abstraite (§6) | aucun secret hors `auth-source` ; une identité absente est une erreur actionnable, pas un plantage |
| W8.c `[R]` | événements, curseurs et reprise (§14.1, §7.5) | une déconnexion ne perd ni ne duplique un événement ; un trou est marqué, pas tu ; l'élagage du tampon n'emporte jamais un événement critique |
| W8.d `[R]` | dashboard et buffers (§9) | un buffer se reconstruit depuis le cache sans réseau |
| W8.e `[R]` | commandes et transient (§10, §11) | toute action mutante passe par l'API avec `expected_revision` ; un conflit est rendu, pas écrasé |
| W8.f `[R]` | artefacts et inspecteur de sandbox | un artefact non promu se distingue d'un artefact promu à l'écran |
| W8.g `[R]` | intégrations Org/Magit/Jupyter/xiiif | chaque intégration absente dégrade sans casser le démarrage |
| W8.h `[R]` | 3D et WebView | la 3D reste une projection ; aucune vue n'écrit dans le graphe |
| W8.i `[R]` | le **transport** : requête HTTP construite, réponse relue, socket isolée derrière un port | une requête se construit et se relit sans réseau ; l'erreur structurée du serveur arrive au client comme une erreur structurée, pas comme un code ; un aller-retour réel contre un serveur local passe |

**W8.i vient en dernier, et ce n'est pas un oubli.** La ligne W8.b portait d'abord « client HTTP/stream
et authentification abstraite » ; seule l'authentification a été livrée, et les sept items suivants ont
chacun déclaré le transport en écart. Le choix a payé — chaque module s'est trouvé testable sans
serveur, donc rejouable, donc mutable — mais il laissait une ligne à moitié honorée. W8.i la finit, et
la sépare pour que ce qui a été fait et ce qui restait dû se lisent.

**W8.a en premier, et c'est la roadmap qui l'impose** : la frontière se fixe avant qu'il y ait quoi
que ce soit à séparer — le seul moment où c'est gratuit. La dépendance qu'on veut interdire ne
s'ajoute jamais délibérément : elle s'installe le jour où une fonction du cockpit a besoin d'une
chose que la configuration de l'auteur fournit déjà, et elle est alors invisible dans le diff.

## W9 — Visualization

Service de projection, 2D, Three.js 3D, viewer registry, pont xwidget/navigateur.

Contrainte à retenir dès maintenant : le service produit une **projection**, jamais une copie
mutable du graphe. Si une vue devient éditable en place, l'invariant « aucun frontend n'écrit
directement dans le graphe » est perdu.

Cette section était en prose et ne portait aucun item. Décomposée ici comme W6, W7 et W8 l'ont
été : ce qui n'a pas de test de sortie n'a pas d'état.

| # | Commit | Dépôt | Test de sortie |
|---|---|---|---|
| W9.a `[R]` | `packages/visualization` : les huit projections de §23.3, versionnées et hashées, derrière un port de condensat | deux rendus du même contenu ont la **même** forme canonique quel que soit l'ordre d'insertion ; une vue modifiée n'est plus la vue — sa forme canonique change ; une vue en retard sur le journal le **déclare** au lieu de se dire à jour ; une arête sans extrémité est refusée |
| W9.b `[R]` | `ArtifactViewerRegistry` (§23.5) : l'artefact suggère, le client choisit | un hint que le client ne sait pas honorer se replie sans échouer ; aucun artefact n'impose de viewer (invariant 10) ; l'absence de tout viewer laisse l'artefact atteignable |
| W9.c `[R]` | interaction de §23 : `focus`, `filter`, `select` vers le viewer, `node_selected` en retour | un événement de viewer ne produit **jamais** de mutation ; le type ne laisse aucun chemin qui contourne l'API de commandes |
| W9.d `[R]` | `apps/web` : la scène de référence, 2D d'abord, sur la vue hashée de W9.a | l'application ne détient aucun graphe modifiable : elle rend une `View` et toute interaction repart par l'API de commandes |

## W10 — xiiif — **déverrouillé aujourd'hui**

Six items ne dépendent d'aucun autre dépôt : dispatcher `xiiif-open`, alias d'API §15, sélection
numérique de région, politique d'URL, limites de taille et de redirections, bridge OpenSeadragon.
Bon travail de repli quand une décision bloque ailleurs.

**Le blocage cité était W0.6, qui est terminé** — et pourtant `RemoteArtifactRef` n'existe nulle
part. W0.6 a livré les schémas LEP ; ce type-ci est un contrat **entre** locusolus et xiiif, et aucun
item ne le portait. C'est un trou de couverture, comme celui de §7.1 relevé plus haut, pas une
dépendance en retard.

| # | Commit | Dépôt | Test de sortie |
|---|---|---|---|
| W6.f `[R]` | `RemoteArtifactRef` (§19 de `xiiif/SPEC_V1.md`) : identité Locus, media type, hashes attendus, **un seul** locator | un document à deux locators est refusé, un document sans locator aussi ; le snapshot prouve la reproduction, la ressource live ne prouve rien ; une divergence entre les deux ne rend jamais la preuve historique douteuse |
| W7.h `[R]` | `HumanReviewFinding` (§20 de `xiiif/SPEC_V1.md`) : les quatre verdicts humains, le commentaire libre, et le schéma que xiiif écrit | un verdict humain ne rend **jamais** `supports` ; `source-changed` ne réfute rien ; un finding dont la cible n'est pas dans le dossier est refusé ; un enregistrement qui ne dit ni verdict ni commentaire est refusé par son schéma |
| W10.7 `[R]` | xiiif consomme `RemoteArtifactRef` : `xiiif-open-locus-artifact`, affichage séparé des cinq facettes | les cinq facettes de §19 sont distinctes à l'écran ; une ressource live modifiée après le run ne fait pas croire que la preuve a changé |
| W10.8 `[R]` | revue humaine de §20 : `accept`, `needs-correction`, `wrong-target`, `source-changed` | un verdict produit un finding attachable à un `ReviewDossier`, et xiiif ne valide rien lui-même |

W10.8 dépend de W7.h : §20 nomme quatre verdicts humains et un `ReviewDossier` auquel les
attacher, et rien ne portait ce contrat — même trou de couverture que W6.f, relevé au même endroit
et pour la même raison. W7.h le comble côté locusolus ; xiiif le consomme ensuite sans importer une
ligne de Locus.

W6.f et W10.7 sont livrés : `xiiif-open-locus-artifact` existe, et les cinq facettes de §19
s'affichent séparément — l'intégrité de la preuve et la dérive de la source sont deux verdicts que
rien ne résume en un seul, de part et d'autre de la frontière. Reste W10.8, la revue humaine de §20,
qui attend le `ReviewDossier` de W7.a côté xiiif.

## W11 — Deployment profiles

Local, personal-node, VM, adapter cloud, hybride distribué ; backup/restore/migration.

Section en prose, décomposée ici comme W9 l'a été. La phrase qui décide de tout est §27.2 :
`locus doctor` **vérifie** dépendances, ports, versions, ressources, attestations et accès. Un
profil qui se déclare exécutable sans avoir été vérifié est exactement ce que cette commande
existe pour empêcher.

| # | Commit | Dépôt | Test de sortie |
|---|---|---|---|
| W11.a `[R]` | `packages/deployment` : les cinq profils de §27.1 sous leur nom, `DeploymentProfile` et le verdict de `locus doctor` | un adaptateur déclaré mais absent rend le profil **inexécutable**, en nommant ce qui manque ; « pas vérifié » n'est jamais « présent » ; deux profils de topologies différentes exposent la même surface cliente |
| W11.b `[R]` | `deployment.yaml` : le schéma, les secrets **dehors**, `locus deployment explain` | un document qui porte un secret en clair est refusé par son schéma ; `explain` nomme les backends actifs sans nommer d'hôte interne |
| W11.c `[R]` | sauvegarde cohérente de §27.4 et restauration sur un backend différent | une sauvegarde qui omet une des cinq parties se refuse à s'appeler cohérente ; une campagne restaurée sur un backend qui n'a pas les capabilities de ses runs historiques le déclare au lieu de rejouer |

## W12 — Evaluation / release

Tests de sécurité, injection de fautes, endurance, benchmarks, ablations, docs, release candidate.

Section en prose, décomposée ici comme W9 et W11 l'ont été. §29 nomme des listes closes — treize
fautes à injecter (§29.4), quatorze attaques (§29.5), huit ablations (§29.8) — et c'est cette
clôture qui rend l'exercice vérifiable : une liste nommée permet de dire ce qui n'a **pas** été
éprouvé, ce qu'une intention générale ne permet jamais.

| # | Commit | Dépôt | Test de sortie |
|---|---|---|---|
| W12.a `[R]` | `packages/evaluation` : le registre d'épreuves de §29.4, §29.5 et §29.8, et le verdict de préparation | une épreuve non traitée empêche la préparation, en la nommant ; une épreuve **écartée** exige sa raison, et une raison vide ne passe pas ; « pas éprouvé » n'est jamais « réussi » |
| W12.b `[R]` | endurance de §29.6 : les huit seuils, et le constat qui les confronte | une campagne qui manque un seul seuil ne se dit pas endurante, et le verdict nomme lequel ; un compteur non relevé n'est pas un seuil atteint |
| W12.c `[R]` | benchmarks de §29.7 : les six configurations et les onze mesures | une comparaison à laquelle il manque une configuration se déclare partielle ; une mesure absente n'est pas une mesure nulle |

---

## W13 — Socle de coordination agentique — **repli, jamais prioritaire** — niveau 3 sur `review`

Couvre le socle de §7.1 et les deux seuls manques de §14 (ADR 0016, décision 3). Ne prend jamais la
priorité sur W4 ; les périmètres ne se recoupent pas. Aucun item ne modifie `canterel`. W13.c et
W13.d sont néanmoins des **dépendances de W7** : « repli » ordonne, il ne déprogramme pas.

Les modes `observed` et `assisted` (ADR 0016, décision 8) existent dès W13.e. Le mode fermé est une
exigence de §33, pas une précaution.

| # | Commit | Test de sortie |
|---|---|---|
| W13.a `[R]` | ADR 0016, sixième frontière (`CLAUDE.md` + `boundaries.json` + garde), ce workstream, `docs/11`, `docs/13` | une violation délibérée **dans chacun des deux sens** fait échouer la CI, et la garde déclare le nombre de fichiers réellement examinés |
| W13.b `[R]` | pli des fixtures `lep/1.0` en graphe d'exécution, sous `tests/` — aucun champ ajouté au protocole | le pli rend un graphe attempt/outil/artefact **sans arête orpheline**, et un test affirme **par l'absence** qu'aucun champ d'agent n'existe dans l'événement LEP |
| W13.c `[R]` | `packages/coordination` : `AgentTemplate`, `AgentInstance`, `Team`, `Decision`, `ApprovalRequest` selon §7.1 ; `Id<Team>`, `Id<Task>`, `Id<Decision>`, `Id<Approval>` dans `packages/protocol` | property test : la capacité effective est l'**intersection** des quatre sources de §14.2, jamais leur union ; ignorer une source fait rougir. Round-trip des quatre nouveaux identifiants |
| W13.d `[R]` | complétion de l'agrégat `Task` de §7.1 — dont `assigned_agent_id` et `assigned_worker_id` — sans toucher la machine à états existante | l'assignation est un événement ; la machine à états de `task.rs` est inchangée et ses tests passent |
| W13.e `[R]` | relation de coordination (`kind` fermé à `review`), payload de `team.modify`, CAS par `expected_revision`, annulation par commit inverse, autorité de proposition agentique | quatre : deux propositions concurrentes sur la même base ne committent pas toutes deux et le refus dit s'il faut rebaser ; une proposition sans justification citant un objet épistémique existant est refusée ; aucun chemin de code ne modifie une `MissionEnvelope` émise ni le hash de sa `ContextView` ; une proposition d'origine agentique suit le même chemin qu'une proposition humaine et son proposeur ne peut pas l'approuver |
| W13.f `[R]` | `packages/projections` : projection du graphe d'exécution | reconstruction depuis zéro = état courant ; quarantaine conforme à ADR 0013 |
| W13.g `[R]` | projection du graphe organisationnel réalisé, par jointure `assigned_agent_id` × événements | **dépend de W13.b et W13.d.** Le graphe se reconstruit depuis le journal seul ; aucun instantané n'est reçu du worker |

W13.b avant W13.f : le pli décide si la projection s'écrit contre `lep/1.0` inchangé, et le découvrir
après aurait coûté la projection. W13.c avant W13.e : une relation entre deux agents suppose que
l'agent soit un objet. W13.d avant W13.g : c'est l'assignation qui rend le graphe réalisé dérivable,
l'événement LEP ne portant aucun champ d'agent.

§18 est à lire avant W13.e : la coordination d'une branche interagit avec le fork, le merge et le
rebase, et §18.4 pourrait imposer des règles sur ce qu'une fusion fait d'une équipe.

Deux pièges qui coûteraient une garantie chacun : ajouter une variante à `ObjectionTarget` ou à toute
énumération de `packages/graph` ; ouvrir l'énumération des sortes de relation à une valeur qu'aucun
consommateur exécutable n'honore.

## W14 — Moteur de politique et orchestrateur de portefeuille

§20 en entier et §13 en entier — le plus large trou du dépôt, et déjà normatif. DSL déclarative
versionnée ; séparation des faits et de la décision ; trace d'évaluation ; cinq verbes ; détection des
conflits de politiques ; priorité explicite ; dry-run ; déterminisme à entrées identiques ;
conservation des overrides. `Delegation` et sa révocation. Explicabilité de §20.5, y compris les
**alternatives rejetées**. Côté portefeuille : les quinze indicateurs de §13.2, la qualité-diversité de
§13.3, `V(b)` de §13.4, les actions de §13.5, et l'**anti-gaming de §13.6**.

Attend W13.c et W13.e. Débloque les modes `bounded` et `operator`, la classe de risque dérivée, et la
moitié de W7. §W7 avertit déjà que l'anti-gaming doit exister avant que la fonction de valeur pilote
des décisions automatiques.

Décomposée ici. §13 est déjà couvert pour l'essentiel par W7.e à W7.g — budgets, anti-gaming,
qualité-diversité, `V(b)`. Ce qui reste est §20, et c'est le moteur de politique.

| # | Commit | Test de sortie |
|---|---|---|
| W14.a `[R]` | `packages/policy` : les cinq verbes de §20.2, la séparation faits/décision, la trace d'évaluation, la priorité explicite et le déterminisme | deux évaluations des mêmes faits rendent la **même** décision et la même trace ; deux règles qui se contredisent sont détectées et tranchées par priorité déclarée, jamais par ordre de déclaration ; une décision sans trace n'existe pas |
| W14.b `[R]` | `Delegation` de §20.4 : portée, plafonds, expiration, révocation | une action hors portée, au-delà d'un plafond ou après expiration n'est pas autorisée ; une délégation révoquée n'autorise plus rien, et l'attribution nomme **les deux** principals |
| W14.c `[R]` | explicabilité de §20.5, dont les **alternatives rejetées** | une décision expose ses huit facettes ; une alternative rejetée sans motif n'est pas une alternative rejetée ; un override humain reste visible après coup |
| W14.d `[R]` | les seize catégories de §20.1 et le dry-run de §20.2 | un dry-run ne produit aucun événement, et sa décision est identique à celle du run réel sur les mêmes faits |

## W15 — Cœur du graphe agentique et contestabilité — **niveau 3 → 4, structure**

Généralisation de la relation unique de W13.e : version canonique immuable avec hash et parent, diff
comme objet de première classe, régions mutables bornées à la façon de GRAFT — région déclarée,
acceptation locale, veto de cohérence globale. L'énumération des sortes s'ouvre **une valeur à la
fois** (ADR 0016 décision 4) ; `role` exerce la clause de falsification de la décision 10, puis
`visibility` dont le consommateur est la construction de `ContextView`. Jeu d'opérations cible :
`ADD_NODE`, `REMOVE_NODE`, `REPLACE_NODE`, `ADD_EDGE`, `REMOVE_EDGE`, `SPLIT_NODE`, `MERGE_NODES`,
`SET_ROLE`, `SET_VISIBILITY`, `SET_VALIDATOR`, `SET_EXECUTION_ORDER`.

Contestabilité des décisions de coordination : famille d'objection parallèle à celle du domaine
épistémique, même forme logique, domaines disjoints, avec un test vérifiant l'**absence** de
conversion. C'est la contribution originale du projet et le geste le plus facile à faire de travers.

Attend W14. Un IR déclaratif contraint, jamais un script : la représentation détermine ce qui est
vérifiable.

Décomposée ici. Le jeu d'opérations est un jeu **cible** : la règle « aucune sémantique inerte » de
l'ADR 0016 décision 4 vaut pour une opération comme pour une sorte de relation. Une opération dont
l'effet sur l'état que ce crate détient est entièrement défini y entre ; une opération qui écrit un
attribut dont le lecteur vit ailleurs attend son lecteur. Sept structurelles entrent en W15.a ;
`SET_ROLE` et `SET_VISIBILITY` arrivent avec leur consommateur en W15.e et W15.f ; `SET_VALIDATOR`
attend qu'un validateur soit un nœud, et `SET_EXECUTION_ORDER` attend qu'une chose ordonne des
attempts entre instances d'agent — ce que la décision 4 a déjà vérifié absent en instruisant
`dependency`.

| # | Commit | Test de sortie |
|---|---|---|
| W15.a `[R]` | `packages/coordination` : la version canonique immuable — hash de contenu, hash de version, parent — et les sept opérations structurelles comme IR déclaratif fermé | le hash de contenu ne dépend que du contenu et se reproduit octet pour octet sur une forme canonique figée en fixture ; appliquer une opération puis son défaire rend le **même contenu** et une **autre version**, parce que l'histoire ne se défait pas ; une opération dont le défaire perdrait de l'information se déclare compensable et aucune fonction ne rend d'opération qui prétende la défaire ; les quatre opérations attributaires sont absentes, et un test le tient par l'absence |
| W15.b `[R]` | le diff comme objet de première classe, calculé une fois | un diff se rejoue sur sa base et rend exactement le contenu visé, et deux rejeux du même diff sur la même base rendent la **même version**, identité comprise ; rejoué sur une autre base il est refusé et le refus dit s'il faut rebaser ; le diff d'une version vers elle-même est **vide**, jamais absent |
| W15.c `[R]` | les régions mutables bornées de GRAFT — `allowed_ops`, `risk_ceiling`, `max_nodes_delta`, `max_edges_delta`, `approval_mode`, `require_shadow` — acceptation locale et veto de cohérence globale | une opération hors de la région déclarée ou hors de `allowed_ops` est refusée en nommant laquelle des bornes mord — quatre interdisent, les deux autres (`approval_mode`, `require_shadow`) **obligent** ; un lot accepté localement mais qui casse un invariant global **par un chemin passant hors de la région** est vetoé, et le veto nomme l'invariant et les agents pris dedans ; l'acceptation locale seule ne commit jamais, et rien dans son type ne le permet |
| W15.d `[R]` | contestabilité d'une décision de coordination : famille d'objection parallèle, domaines disjoints | une décision de coordination offre ses cibles — déclencheur, politique, périmètre, décision — ; **aucune fonction ne convertit** une objection de coordination en `ObjectionTarget` ni l'inverse, et un test le tient par l'absence ; aucun trait générique ne factorise les deux familles, ce qui serait la conversion reconstruite |
| W15.e `[R]` | `visibility`, **deuxième** membre de l'énumération des sortes (ADR 0016 décisions 4 et 10, amendées le 2026-08-18), dont le consommateur est la construction de `ContextView` | deux `ContextView` construites sous deux versions de coordination différentes diffèrent exactement des révisions que `visibility` retire ; aucune relation `visibility` n'élargit ce qu'une ACL refuse ; le constat de la clause de falsification est écrit au ledger, **dans un sens ou dans l'autre** |
| W15.f `[M]` **bloqué** | `SET_ROLE` comme opération **attributaire**, avec son lecteur exécutable dans `canterel` | l'overlay additif du worker lit le rôle de l'instance et un test l'exerce de bout en bout ; sans ce lecteur, l'opération reste hors de l'énumération |

W15.a avant W15.b : un diff se rejoue contre une identité de version, et l'identité décide de ce
qu'un rejeu peut affirmer. W15.b avant W15.c : une région borne un lot d'opérations, donc un diff.
W15.d ne dépend d'aucun des trois et peut se faire en parallèle ; il ne dépend surtout pas de
`packages/graph`, et c'est la moitié de son objet.

**W15.e et W15.f ont été échangés le 2026-08-18.** `role` devait être le deuxième membre de
l'énumération des sortes de relation ; en l'instruisant, il est apparu que ce n'est pas une relation
— `SPEC_V1.md` §7.1 en fait un champ d'`AgentTemplate`, §20 une classification, §6.3 un attribut
d'appartenance, et `agent.rs` le portait déjà comme attribut. La clause de falsification de l'ADR
0016 est donc réorientée sur `visibility`, qui est réellement de forme paire et dont le consommateur
vit dans le dépôt ; `role` reste dû comme `SET_ROLE`, l'opération attributaire que W15.a avait déjà
différée. L'amendement daté est dans `docs/adr/0016`.

**W15.f est bloqué, et bloqué correctement.** Le lecteur du rôle est `selectOverlay` dans le worker,
qui ne connaît d'une mission que ce que la `MissionEnvelope` lui livre — et `mission-envelope.schema.json`
porte `review_policy` et `required_capabilities`, **pas** de rôle d'agent. Faire passer le rôle
jusqu'au worker demande donc un **mineur `lep/1.1`**, dont l'ADR 0016 dit qu'il « a son propre ADR »
et que W13 « ne l'ouvre pas ». Écrire `SET_ROLE` avant ce mineur produirait exactement la sémantique
inerte que la décision 4 interdit : un attribut que le système saurait versionner, différencier,
approuver et afficher, et que rien n'honorerait. L'item attend donc cet ADR, et W15 est clos sans
lui.

## W16 — Reconfiguration vivante et scheduler dynamique — **niveau 4**

Le scheduler doit savoir spawn, suspend, drain, kill, replace, split, merge, connect, disconnect,
rerouter l'état, rejouer, migrer le contexte, et livrer les messages **en connaissance de la version**.
Barrières par invariant menacé plutôt que par lieu ; quiescence locale d'un nœud plutôt que drain
global. Epochs, messages tardifs et transfert d'état : ils n'ont un problème réel à résoudre qu'une
fois une messagerie inter-agents existante. Visibilité institutionnelle facultative des sous-agents
internes du harnais — le cas de W16 justifiant un mineur LEP, avec son ADR.

Plan de simulation : rejeu déterministe, substitut d'environnement enregistré, ombre en sandbox réelle,
canari facultatif. Un objet simulé n'existe pas comme type dans le domaine épistémique.

Attend W15, W4.e et W4.g. **Les trois sont satisfaits** : W4.e et W4.g sont livrés, et W15 est clos
à W15.e — W15.f attend un mineur LEP qui a son propre ADR.

Décomposée ici. Deux items de la prose n'entrent pas, et pour la même raison que les opérations
attributaires de W15.a : **epochs et messages tardifs** n'ont « un problème réel à résoudre qu'une
fois une messagerie inter-agents existante », et il n'y en a pas ; la **visibilité institutionnelle
des sous-agents internes du harnais** est « le cas de W16 justifiant un mineur LEP, avec son ADR »,
donc elle attend cet ADR comme W15.f.

| # | Commit | Test de sortie |
|---|---|---|
| W16.a `[R]` | les transitions de cycle de vie du scheduler — `spawn`, `suspend`, `drain`, `kill`, `replace`, `connect`, `disconnect` — comme machine à états explicite, et la **quiescence locale** d'un nœud | une transition interdite est refusée en nommant l'état de départ et celui visé ; un nœud est drainé **sans** que rien d'autre soit arrêté, et la quiescence se constate au lieu de s'attendre ; `kill` sur un nœud quiescent et sur un nœud actif ne disent pas la même chose |
| W16.b `[R]` | les **barrières par invariant menacé** plutôt que par lieu | une reconfiguration ne barre que les nœuds dont elle menace un invariant, et le refus nomme l'invariant, pas le lieu ; deux reconfigurations qui ne menacent pas le même invariant ne se bloquent pas l'une l'autre ; une barrière posée sans invariant menacé est refusée |
| W16.c `[R]` | le plan de simulation : rejeu déterministe, substitut d'environnement enregistré, ombre en sandbox réelle, canari facultatif | deux rejeux de la même trace rendent le même résultat ; un substitut d'environnement qui n'a pas la réponse le **dit** au lieu d'en inventer une ; un objet simulé n'existe **pas** comme type dans le domaine épistémique, et un test le tient par l'absence |
| W16.d `[M]` **bloqué** | visibilité institutionnelle facultative des sous-agents internes du harnais | attend le mineur `lep/1.1` et son ADR |
| W16.e `[R]` **bloqué** | epochs, messages tardifs et transfert d'état | attend une messagerie inter-agents, qui n'existe pas |

W16.a avant W16.b : une barrière borne des transitions, donc les transitions d'abord. W16.c ne
dépend d'aucun des deux.

## W17 — Cockpit et orchestration de la mémoire

Cockpit à quatre vues — plan, vivant, trace, épistémique — avec sélection synchronisée par `Id<Agent>`.
Le canvas produit une commande, jamais une écriture. Diff calculé une fois côté serveur, donc identique
dans Emacs et dans le web. Preview statique, ombre, approbation, rollback, navigation dans le temps
comme propriété du pli. Se branche sur le Visualization Projection Service de §23.3, dont la vue
« société d'agents » est déjà nommée, et consomme `/branches/:id/diff` de §22.4.

Mémoire : les sept niveaux de §16.1 ; le retrieval hybride de §16.3 avec ranking dont les facteurs sont
exposés et embeddings ne contournant pas les ACL ; l'index mémoire hybride comme projection obligatoire
de §9.3 ; la déduplication non automatique de §16.4 ; la compaction de §16.5 ; les cinq préventions de
§16.6. Deux retrievals séparés, épistémique et organisationnel, sans conversion.

Attend W16, W9 et `locusd`. Aucun outil existant ne combine canvas, graphe comme état mutable de
première classe, mutations proposées par les agents, commit atomique et invariants.

Décomposée ici. **`locusd` n'existe pas** — `apps/` porte `emacs`, `locus-execd` et `web`, pas le
daemon — donc tout ce qui suppose une surface HTTP attend. Ce qui n'en dépend pas est le **domaine**
de la mémoire (§16.1 à §16.5) et la discipline du cockpit (§23.3, W9), et cela se fait maintenant.
Les cinq préventions de §16.6 sont déjà livrées, par `packages/review/src/contamination.rs` (W7.b) :
elles ne sont pas redemandées ici.

| # | Commit | Test de sortie |
|---|---|---|
| W17.a `[R]` | `packages/memory` : les sept niveaux de §16.1 comme liste close, et la distinction canonique/projection de la dernière phrase de la section | les sept se lisent sous leur nom ; ce qui est **canonique** — graphe, événements, artefacts — ne se déclare jamais régénérable, et ce qui est projection le déclare toujours ; une mémoire dont le niveau n'est pas nommé n'existe pas |
| W17.b `[R]` | le retrieval hybride de §16.3 : les dix signaux, le ranking dont les facteurs sont **exposés**, et les ACL que les embeddings ne contournent pas | un résultat porte la contribution de **chacun** des signaux qui l'ont produit, et un ranking sans facteurs exposés est refusé ; un élément qu'une ACL refuse reste absent quel que soit son score vectoriel, et le test l'exerce avec un score maximal |
| W17.c `[R]` | deux retrievals séparés, épistémique et organisationnel, **sans conversion** | les deux répondent à des questions différentes sur des types disjoints ; **aucune fonction ne convertit** un résultat de l'un en résultat de l'autre, et la septième frontière l'étend à ce cas |
| W17.d `[R]` | déduplication non automatique (§16.4) et compaction (§16.5) | un duplicata exact par hash est détecté ; un candidat **sémantique** n'est jamais fusionné automatiquement, et sa résolution porte confiance et provenance ; une fusion se défait par une nouvelle décision ; une compaction signale ce qu'elle a omis et ne transforme jamais un objet non validé en connaissance établie |
| W17.e `[R]` | les quatre vues du cockpit et la sélection synchronisée par `Id<Agent>` ; le canvas produit une **commande**, jamais une écriture | une sélection dans une vue désigne le même agent dans les trois autres ; un geste de canvas rend une commande que rien n'applique sur place, et aucun chemin de type ne permet à une vue d'écrire |
| W17.f `[M]` **bloqué** | `/branches/:id/diff`, la preview, l'ombre, l'approbation, le rollback et la navigation dans le temps | attend `locusd`, qui n'existe pas |

W17.a avant W17.b : un retrieval cherche dans des niveaux. W17.c après W17.b, pour la même raison
que W15.d après le reste : la conversion ne devient tentante qu'une fois les deux moitiés écrites.

## W18 — Adaptation automatique et admission de capacité

Boucle rapide sur la capacité — routage de modèle, choix d'outil, sélection de skill, retry, routes
éphémères ; boucle lente sur la structure. Les onze déclencheurs de §14.5 et les indicateurs de §13.2.
`bounded` sur les seules opérations dont la classe de risque est **dérivée** des invariants menacés.

Admission de capacité : proposition, politique et approbation faisant entrer une capacité nouvelle sous
forme d'`EnvironmentBlueprint` construit, scanné, signé et attesté (§19.3), jamais sous forme de code
injecté. Attend W5, W6 et S3/S4 attesté.

Métrique d'acceptation propre : le taux d'annulation humaine des adaptations agentiques.

## Recherche — sans dépendance de chemin critique, abandonnable sans coût

`R1` **consensus circulaire** — cycle de `Cites` sans `AnchoredIn` externe, exigé par §16.6, calculable
sur les types existants de `packages/graph`. Le moins cher des items de recherche, ne dépend ni de W4
ni de `locusd`, publiable seul. · `R2` **crédit structurel** — attribuer une amélioration à une
relation, un rôle, un budget ou au hasard d'échantillonnage ; sans cela un système évolutionnaire
accumule des changements inutiles. · `R3` **évaluation structurelle** — métriques de graphe et regret
structurel `R_s = U(meilleur candidat disponible) − U(graphe choisi)`, calculable en **rejeu** sur
fixtures identiques. · `R4` **substitut d'environnement puis world model** — générateur de trajectoires
contrefactuelles, comparatif à graine et préfixe identiques, unilatéral en rejet, jamais un juge,
jamais une preuve ; et la fidélité sur les environnements du domaine — IIIF, SPARQL, ALTO/PAGE,
notebooks, prouveurs — est inconnue. · `R5` **prototype externe de harnais tiers** — dépôt jetable, une
seule question : un `SessionPlan` peut-il produire un flux conforme au harnais de conformance de W0.9 ?
Si oui, un worker LEP séparé ; si non, on supprime le dépôt. Aucune ligne dans
`backend/cli/src/locus/` avant la réponse. · `R6` **évolution inter-exécutions** — une adaptation
récurrente et gagnante en validation appariée propose une amélioration de template.

---

## Règle de session

Lire ce fichier, prendre le premier item non terminé dont les dépendances sont satisfaites, lire
le code concerné, exécuter les tests de son périmètre, modifier **ce périmètre seul**, mettre à
jour `IMPLEMENTATION_LEDGER.md`.
