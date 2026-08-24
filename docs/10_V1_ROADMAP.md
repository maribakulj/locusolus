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
| W0.1 `[R]` **fait** | placement de la doc, `CLAUDE.md` par repo, ADR 0001–0010 | les 4 | `grep -r "locus-solus"` ne renvoie rien hors historique Git |
| W0.2 `[R]` **fait** | squelette monorepo : `apps/`, `packages/`, `schemas/`, `tests/`, tooling, CI qui passe à vide | locusolus | CI verte sur un dépôt sans code |
| W0.3 `[R]` **fait** | garde de frontières architecturales (les 5 règles du `CLAUDE.md`) branchée sur le squelette vide | locusolus | une violation délibérée fait échouer la CI |
| W0.4 `[R]` **fait** | `packages/protocol` : IDs, enveloppe d'erreur structurée, politique de versionnement, horodatage | locusolus | unitaires |
| W0.5 `[M]` **fait** | JSON Schemas LEP : `CapabilityManifest`, `MissionEnvelope`, `ContextView`, `EnvironmentBlueprint`, `SandboxSpec`, `ResourceSpec` | locusolus | les exemples de `schemas/examples/` valident |
| W0.6 `[M]` **fait** | JSON Schemas LEP, suite : `Lease`, `Attempt`, événements, `ArtifactManifest`, `RunManifest`, `SandboxAttestation`, `EpistemicCommit` | locusolus | validation |
| W0.7 `[R]` **fait** | corpus de fixtures : nominal, refus d'admission, reconnexion, résultat tardif, dépassement de budget | locusolus | chaque fixture valide **ou invalide intentionnellement**, selon son `expect` déclaré |
| W0.8 `[R]` **fait** | SDK généré depuis les schémas + `schema-registry` avec négociation de features au handshake | locusolus | round-trip sur toutes les fixtures |
| W0.9 `[R]` **fait** | `packages/testing` : **harness de conformance LEP côté serveur** — handshake, offre, lease, heartbeat, expiration, acquittements | locusolus | le harness se teste contre un worker factice |
| W0.10 `[R]` **fait** | `IMPLEMENTATION_LEDGER.md` dans les quatre dépôts | les 4 | présent, avec l'entrée d'étape 0 |
| W0.11 `[R]` **fait** | **la roadmap ne peut plus mentir sur son état** — garde qui confronte les registres des quatre dépôts à ce tableau | locusolus | un item livré dont la ligne ne porte pas **fait** échoue, et un item marqué sans entrée aussi ; un registre non lu est **nommé** et suspend la seconde règle plutôt que de conclure |
| W0.12 `[R]` **fait** | **`bloqué` n'est pas `à faire`** — la garde apprend les trois états du tableau, calcule la frontière et l'imprime | locusolus | une ligne `bloqué` ou `reporté` ne figure pas dans la frontière ; un item décidé qui a pourtant son entrée au ledger est rapporté sous son nom propre, et une seule fois ; la frontière s'imprime même vide, parce qu'un silence se lirait « je n'ai pas regardé » |
| W0.13 `[R]` **fait** | **une entrée qui consigne un blocage n'est pas une livraison** — la garde trie les entrées du registre au lieu de les compter | locusolus | une entrée titrée `— Bloqué :` ne vaut pas livraison, et une ligne **fait** au-dessus d'elle est rapportée sous son nom propre ; un titre qui **cite** le mot livre quand même ; le blocage d'un dépôt voisin en est un aussi |
| W0.14 `[R]` **fait** | **un item qui traverse deux dépôts se livre en plusieurs fois**, et une moitié consignée ne vaut pas l'item | locusolus | une entrée titrée `— Partiel :` ne compte pas comme livraison, au même titre que `Bloqué` et `Reporté` ; les trois préfixes restent distincts, parce qu'ils disent quoi attendre |
| W0.15 `[R]` **fait** | **le générateur de SDK apprend les unions discriminées** — un `oneOf` dont chaque branche épingle la même propriété | locusolus | Rust rend un `enum` étiqueté en interne **sans** l'étiquette dans ses variantes, TypeScript une union discriminée **avec** l'étiquette en type littéral, et les trois formes qu'on ne sait pas générer — `oneOf` non étiqueté, branches homonymes, branche par `$ref` — sont refusées chacune sous son nom |
| W0.16 `[R]` **fait** | **un blocage qui nomme ce qu'il attend se périme tout seul** — `attend:<id>` dans la raison d'une ligne bloquée, et `check:roadmap` refuse quand ce qu'elle attend est livré | locusolus | une ligne bloquée dont la dépendance déclarée est **entièrement livrée** fait échouer le garde, vu en rouge sur `W17.f` avant d'être vu en vert ; `attend:externe` ne se périme **jamais**, parce qu'un hôte ou un consommateur qui n'existent pas n'ont pas de date ; une ligne **sans marqueur** n'est pas vérifiée du tout — deviner l'identifiant dans la prose aurait crié au blocage périmé sur `W18.f`, dont la raison **cite** `W5.f` sans l'attendre |
| W0.17 `[R]` **fait** | **le garde lit les deux familles d'identifiants**, `W<phase>.<item>` et `R<n>`, dans le registre comme dans le plan — et la section « Recherche » passe en tableau, seule forme qu'il sache confronter | locusolus | les six items de recherche, livrés le 2026-08-18 et décrits au présent trois jours de plus, sont vus **en rouge** — six `livre-non-marque` — avant d'être vus en vert ; la frontière les nomme, ce qu'elle ne faisait pas quand ils vivaient en prose ; l'alphabet est écrit **une fois** et partagé par les quatre motifs, l'avoir écrit quatre fois étant ce qui a permis à une famille d'échapper aux quatre en même temps ; `attend:R<n>` entre dans le marqueur **et** dans `satisfied` du même geste — un `R<n>` est un item et non une phase, et sans cette règle le marqueur s'analysait pour rester insatisfiable à jamais, ce qui est pire que les deux états qu'il départage

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
| W2.1 `[R]` **fait** | remote `upstream` + `docs/locus/upstream.md` + politique de sync | un merge amont à blanc ne touche aucun fichier local |
| W2.2 `[R]` **fait** | non-régression standalone en CI (§28.8) — **avant tout code `locus/`** | passe sur le HEAD actuel |
| W2.3 `[R]` **fait** | `src/locus/{index,config,errors}.ts` + `canterel worker --locus` qui ne fait rien | `bun run check` vert ; standalone intact |
| W2.4 `[R]` **fait** | `identity.ts`, `auth.ts`, enrôlement, révocation (§7) | identité persistante après redémarrage |
| W2.5 `[R]` **fait** | `protocol.ts`, `schema-registry.ts`, `connection.ts` sur le SDK de W0.8 | contract tests contre le harness |
| W2.6 `[R]` **fait** | `capability-manifest.ts` + `capability-watch.ts` — détection réelle des toolchains, modèles, accélérateurs **et du niveau de sandbox effectif** | sur macOS : annonce `["S1","S2"]` et `mps`, jamais plus |
| W2.7 `[R]` **fait** | `registration.ts`, handshake complet | conformance §8.2 |
| W2.8 `[R]` **fait** | `admission.ts` — validation, refus structuré (§10.2), politique locale plus restrictive | la fixture de refus de W0.7 produit le bon code d'erreur |
| W2.9 `[R]` **fait** | `lease.ts`, `attempt.ts`, heartbeats, perte de lease (§11) | expiration et reprise contre le harness |
| W2.10 `[R]` **fait** | `context-materializer.ts` + isolation informationnelle (§12.4) | un contexte de branche A n'atteint jamais une mission de branche B |
| W2.11 `[R]` **fait** | `session-map.ts`, `agent-overlay.ts`, `model-policy.ts`, `tool-policy.ts` — couche d'adaptation vers l'amont, à garder mince | mission → session **sans modifier `src/session/`** |
| W2.12 `[R]` **fait** | `event-bridge.ts`, `event-spool.ts`, coalescence (§18) | perte de connexion : rien perdu, rien dupliqué |
| W2.13 `[R]` **fait** | `usage-meter.ts`, budget local, dépassement (§17) | arrêt propre au dépassement |
| W2.14 `[R]` **fait** | `artifact-client.ts`, `artifact-scanner.ts`, déclaration avant upload (§19.1) | hash déclaré ≠ hash reçu → rejet |
| W2.15 `[R]` **fait** | `epistemic-commit.ts` — jamais au-delà de `staged` (§2.3) | tentative de promotion → erreur structurée |
| W2.16 `[R]` **fait** | `recovery.ts`, `resume-store.ts`, offline, résultats partiels (§24) | redémarrage du worker en cours de mission |
| W2.17 `[R]` **fait** | `human-input.ts` (§22) | suspension sans processus coûteux maintenu |
| W2.18 `[R]` **fait** | `ui/worker-status.ts`, `mission-view.ts`, `security-view.ts` | rendu |
| W2.19 `[R]` **fait** | suite de conformance complète + consumer-driven contracts (§28.2/28.3) | verte contre le harness |
| W2.20 `[R]` **fait** | la boucle du worker : de `inert` à une mission exécutée | **débloqué par `W20.h`** : `locusd` accepte désormais des écritures, sérialisées par stream. Le blocage a été levé par la garde de roadmap plutôt que par une relecture — c'est exactement ce que `W0.16` existe pour faire. Test de sortie : `runWorker` ne rend plus `inert` — enrôlement, offre, lease, mission, session amont **réellement initialisée**, événements, résultat ; le `SessionPlan` de `session-map.ts` reste **une donnée** et aucun handle ne traverse la couture, comme l'ADR 0010 l'exige ; aucun fichier des cinq répertoires amont intouchables n'est modifié, tenu par `upstream.test.ts` ; une interruption au milieu reprend sous le **même** numéro de tentative ; `W22.f` a rendu vraie la **raison** de l'inertie, cet item lève l'inertie elle-même, et confondre les deux ferait passer une correction de vérité pour une livraison |
| W2.21 `[R]` **fait** | le **client de réclamation** : par où le worker demande du travail | canterel | `W2.20` a livré la boucle, son ouvreur de session et le test qui ouvre une **vraie** session amont ; ce qui lui manque est un appelant, et il est nommé. `connection.ts` sait parler LEP contre le harnais de `W0.9`, mais rien ne réclame de mission. Test de sortie : `canterel worker` cesse de rendre `inert` **sur une installation configurée** — donc `ports` sort de `missing` — et un tour aboutit contre le harnais de `W0.9` ; « rien pour toi » et « je n'ai pas pu demander » restent deux issues distinctes, comme l'ADR 0028 décision 4 les sépare pour le broker ; la couture de `LOCUS_SEAMS` reste mince et son import de `@/locus` dynamique, tenu par la CI de §28.8 |
| W2.22 `[R]` | le **composition root** du worker : assembler les vrais ports et tourner contre un `locusd` réel | canterel | vérifié au code en tentant `W12.d` : `runWorker(config, ports)` exige que l'appelant **fournisse** les ports, et sans eux il rend `inert` en les nommant — c'est ce que `W2.3` a livré, délibérément. `workerPorts()` sait les construire depuis `W2.21`, et **personne ne les assemble** : aucun chemin du binaire ne mène d'une configuration à une boucle qui tourne. Conséquence exacte : le worker réel ne peut pas être lancé contre un `locusd` réel, donc aucune des clauses « la session tourne », « les outils s'exécutent », « les événements repartent » de `W12.d` n'a de sujet exécutable. Test de sortie : `openscience worker --locus` sur une configuration complète **tourne** au lieu de rendre `inert`, et l'issue de la boucle est celle de `W2.20` ; une configuration incomplète rend toujours `inert` en nommant ce qui manque, et un test le tient — c'est la garantie de `W2.3` et elle ne doit pas se perdre en devenant vraie ; le démarrage reste **sans réseau** — assembler les ports n'ouvre aucune connexion, seule la boucle en ouvre ; périmètre `backend/cli/src/locus/**` |

La liste de fichiers de `repos/canterel/SPEC_V1.md` §4 est une **annexe indicative**, pas un
gabarit. Ne crée pas 34 stubs vides : chaque commit ci-dessus livre une garantie testée, et les
fichiers apparaissent quand ils portent du comportement.

---

## W1 — Locus domain / event store — parallèle à W2

| Groupe | Contenu | Test de sortie |
|---|---|---|
| W1.a `[R]` **fait** | enveloppe commune d'objet épistémique (§7.4) : identité, version, statut, niveau de validation, portée de branche, provenance, supersession | property tests sur les invariants |
| W1.b `[R]` **fait** | agrégats organisationnels (§7.1) et objets épistémiques (§7.3) | property tests |
| W1.c `[M]` **fait** | `packages/event-store` : enveloppe (§10.1), append-only logique, concurrence optimiste | replay complet + conflit de concurrence détecté |
| W1.d `[M]` **fait** | projections reconstructibles | reconstruction depuis zéro = état courant |
| W1.e `[R]` **fait** | `packages/graph` : relations typées, **hyperarêtes** pour les inférences multi-prémisses (§7.6) | une inférence à 3 prémisses n'est pas 3 liens |
| W1.f `[R]` **fait** | validation épistémique (§8) et propagation de l'invalidation (§8.3) | invalider une prémisse propage correctement |
| W1.g `[R]` **fait** | résultats négatifs et conflits (§18.7) | aucun chemin de code ne supprime un conflit |
| W1.h `[M]` **fait** | migrations de schéma + tests de portabilité | migration aller-retour |

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
| W4.d.1 `[R]` **fait** | la traduction `SandboxSpec` → plan de confinement rootless, et la lecture de ce que l'hôte permet | le plan ne concède jamais plus que le niveau exigé — monter d'un niveau ne relâche rien et change quelque chose — il refuse par leur nom la micro-VM, l'enclave et un mode réseau sans namespace pour le porter, et la lecture de l'hôte nomme ce qui manque au lieu de le supposer |
| W4.d.2 `[R]` **fait** | le driver rootless : `RuntimePort` implémenté sur Podman rootless (`docs/03`), la sandbox créée, l'attestation lue de ce qui tourne | le driver demande au runtime exactement ce que le plan a décidé, et il atteste de ce qu'il **observe** — un confinement plus faible que demandé apparaît dans l'attestation et `conformance` le refuse |
| W4.d.3 `[R]` **fait** | la suite de W4.b passée contre le backend : une commande par sonde, et le `Standing` qui en sort | la suite rend un `Standing` pour ce backend ; une sonde non exécutée est `NotRun` avec sa raison, jamais un succès, et un hôte sans runtime n'obtient jamais `Trusted` |
| W4.d.4 `[R]` **fait** | la vérification du profil seccomp restreint apporté par le déploiement | un profil qui ne refuse pas ce que la posture promet est refusé, et le refus nomme **tous** les appels manquants |

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
| W4.e.1 `[R]` **fait** | la machine macOS : son état, la lecture des faits **dans l'invité**, et le plafond qui en découle | les faits viennent du noyau qui confine et non de celui qui appelle ; une machine arrêtée se distingue d'un noyau incapable ; une VM partagée ne fait jamais franchir `S3` |
| W4.f.1 `[R]` **fait** | la portée de l'accélérateur : `mps` n'existe qu'en natif, donc hors du conteneur | sur un hôte natif, une mission a le conteneur **ou** l'accélérateur, jamais les deux, et le refus dit lequel des deux il faut lâcher — distinct de « accélérateur absent », qui appelle une autre action |
| W4.g.1 `[R]` **fait** | le placement : choisir parmi plusieurs candidats, sur ce que chacun a **prouvé** | un hôte ne reçoit que le niveau qu'il a prouvé tenir ; le refus dit ce qui manquait à **chaque** candidat ; deux placements du même journal placent au même endroit |
| W4.g.2 `[R]` **fait** | le reroutage : une tentative dont l'hôte tombe, une tentative refusée partout | une tâche réattribuée **conserve son numéro d'attempt** (§12.3) ; l'hôte qui l'a perdue n'est jamais rechoisi ; l'épuisement distingue « tous tombés » de « aucun ne convenait » |
| W4.h `[R]` **fait** | la **surface** du broker : par où `locusd` lui parle | locusolus | découvert en écrivant `W22.c` — **aucun code du dépôt ne construit de client vers `locus-execd`, et le broker n'écoute rien.** L'audit du 2026-08-21 comptait trois maillons manquants ; c'est le quatrième, et il ne se voyait pas parce que les deux côtés sont cohérents séparément. Test de sortie : `locusd` atteint le broker par un transport nommé dans un ADR, et un test exerce l'aller-retour de bout en bout ; la quatrième frontière tient — `apps/locusd` n'acquiert aucune dépendance vers un SDK de runtime, et la garde le vérifie sur le nouveau chemin ; un broker injoignable se dit, et ne se confond pas avec un broker qui refuse |
| W4.i `[R]` | la **créance de pair** sur le lien du broker — quand les deux binaires tournent sous deux utilisateurs | locusolus | `W4.h` devait la livrer et ne l'a pas fait, pour deux raisons trouvées à l'écriture et écrites dans l'ADR 0028 décision 2 : `UnixStream::peer_cred` est **instable** et `unsafe_code = "forbid"` ne se contourne pas, donc l'obtenir coûte un crate externe **dans le processus privilégié** ; et la politique envisagée — « le même utilisateur que le broker » — admet **exactement** l'ensemble que `0600` admet déjà, donc elle n'aurait rien séparé. Test de sortie : les deux barrières admettent des ensembles **différents**, exercé sur une socket en `0660` avec un groupe partagé où un appelant du groupe est admis par les permissions et **refusé** par la politique ; le refus est un `Verdict::Refused`, jamais une fermeture, et un test le distingue d'un broker éteint ; la dépendance que la créance exige entre par `dependencies.json` avec son arbre **mesuré**, comme l'ADR 0018 l'a fait pour le sien |

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
| W5.a `[R]` **fait** | `packages/environments` : `ToolchainProfile` et `EnvironmentBlueprint`, avec les invariants que le schéma de W0.5 ne peut pas exprimer | un profil répété, un préféré inférieur au minimum, une variable dont le nom annonce un secret et une image par tag sont refusés, chacun par son nom |
| W5.b `[R]` **fait** | la chaîne : lockfile → build → SBOM → scan → tests → signature → digest, comme suite de types | aucun chemin ne signe une image non scannée ni ne publie un digest dont les tests n'ont pas tourné — vérifié par un `compile_fail`, pas seulement par un test |
| W5.c `[R]` **fait** | **correction de W4.d.3** : une sonde absente de l'image n'est pas une sonde bloquée | les codes 125, 126 et 127 sont lus comme `NotRun` et non comme `Blocked` ; une image incomplète ne rend jamais un backend `Trusted` |
| W5.d `[R]` **fait** | les sondes voyagent avec le harnais : plus aucun binaire attendu dans l'image | aucune sonde ne nomme un chemin de l'image, chacune est du shell que `sh -n` accepte, et une sonde qui n'a pas pu conclure est `NotRun` |
| W5.e `[R]` **fait** | le driver de build derrière un port, dans `apps/locus-execd` | le digest vient de la sortie du runtime et non du blueprint ; un build muet n'est pas un succès ; le driver ne sait pas dépasser le deuxième maillon de la chaîne |
| W5.f `[R]` **fait** | validation **sémantique** des sondes contre une sandbox réelle | sur un hôte capable de `S2`, chaque sonde produit le verdict que son `contained_from` annonce — c'est la seule chose que ni `sh -n` ni un double ne peuvent dire |
| W5.g `[R]` **fait** | le **quota disque comme fait d'hôte lu**, et non appris en échouant — trouvé par le premier passage de W5.f | un hôte dont le système de fichiers ne peut pas porter `--storage-opt size=` est constaté **avant** toute création, et une mission qui réserve du disque y est refusée à l'**admission**, par un motif qui nomme le système de fichiers ; le refus est distinct de `CapacityExceeded` — « la capacité manque » et « la borne n'est pas applicable ici » n'envoient pas chercher la même chose ; aucun chemin ne laisse `podman create` être l'endroit où on l'apprend |
| W5.h `[R]` **fait** | **les sondes que le premier hôte réel a démenties** — trois faites, deux restent | `read_process_environment` ne vise plus `/proc/1`, qui dans un namespace PID désigne l'init **du conteneur** : la sonde réussissait précisément parce que le confinement était correct, et comme elle est `critical`, tout hôte bien configuré se voyait refuser la confiance. Le discriminant est désormais le cgroup — un processus dont le cgroup diffère du nôtre est un processus que cette sandbox n'a pas créé. Les deux sondes réseau constatent l'absence de **route par défaut** avant de conclure : sans ce constat, un `curl` qui échoue ne distingue pas « la sandbox a coupé le réseau » de « l'hôte ne mène nulle part ». Un code réservé de plus, `121`, dit « ce que je devais **atteindre** n'a pas répondu » — distinct de `120`, « ce que je devais **lire** n'était pas là » : deux ignorances, deux réparations |
| W5.j `[M]` **fait** | **où le quota disque s'applique**, quand la racine est en lecture seule | `W5.g` a rendu le quota **lisible** ; reste à le rendre **applicable**. À `S2` la racine est montée en lecture seule, donc `--storage-opt size=` dimensionne une couche que personne n'écrit : le seul endroit inscriptible est l'espace de travail monté, et un bind mount hérite du système de fichiers de l'hôte, non borné. L'arbitrage nomme le mécanisme — volume dimensionné, tmpfs borné, ou renoncement déclaré — et `exceed_disk_quota` écrit ensuite là où le quota mord : sans quota déclaré elle doit **réussir**, avec quota être bloquée. L'arbitrage est rendu : le quota mord **là où la sandbox peut écrire**, et le plan le nomme — `QuotaTarget` à trois cas, dont le troisième est celui qu'on aurait oublié. À `S0`/`S1` la couche inscriptible (`--storage-opt size=`) ; à partir de `S2` le premier montage inscriptible, par un **volume dimensionné**, que Podman ne tient que sur XFS avec quotas de projet — exactement le fait que `W5.g` fait déjà lire à l'hôte et refuser à l'admission. Le **tmpfs borné est écarté** : c'est de la RAM, donc une réservation de disque viendrait manger celle de mémoire, deux budgets pour une ressource. Un quota là où **rien** n'est inscriptible est refusé au plan, parce qu'il serait accepté, transmis, et sans effet. La sonde reçoit sa cible par `LOCUS_QUOTA_TARGET` et y écrit ; sans cible déclarée elle **réussit sans rien tenter**, et un code réservé de plus — `122`, « ce sur quoi je devais écrire ne s'écrit pas » — l'empêche de repasser un test qu'elle ne fait pas tourner. Enfin, `Probe::requires` fait dépendre son attente de ce que la **mission** a réservé : le disque est la seule ressource que `ResourceSpec` laisse valoir zéro, et une borne que personne n'a demandée ne peut pas être franchie. Le second test hôte passe de quinze à **seize** sondes, l'exclusion nommée n'ayant plus lieu d'être |
| W5.k `[R]` **fait — et il a démenti son propre titre** | l'instrument qui regarde le réseau **depuis l'intérieur** de la sandbox du plan | la sandbox **voit** la route par défaut de l'hôte : l'hypothèse « une permission déclarée n'est pas accordée » est **fausse**, et c'est le test lui-même qui l'a démentie. Ce que les trois passages illisibles cachaient est ailleurs — voir `W5.l` |
| W5.l `[R]` **fait** | **arrêter n'est pas retirer** | `RuntimePort::stop` lance `podman stop` et rien ne lance `podman rm` : le conteneur garde son **nom** et sa **couche inscriptible**. Le suivant qui demande le même nom échoue — « the container name `locus-0001` is already in use » — et c'est ce qui a rendu trois passages de CI illisibles. `selftest` avait vu la conséquence sans voir la cause : « un hôte qui accumule des conteneurs d'épreuve finit par ne plus pouvoir en créer ». Test de sortie : le port porte le retrait sous son nom, un `certify` ne laisse **rien** derrière lui — ni conteneur, ni couche — et un test le constate en redemandant le même nom ; la sonde `persist_after_teardown` cesse alors de tenir pour une raison qui n'est pas la sienne, puisqu'il y aura enfin un teardown |
| W5.m `[R]` **fait — l'instrument est posé, la réponse vient du prochain passage** | **la route est là, et la sonde ne la trouve pas** | passage propre, sans collision : `cat /proc/net/route` dans la sandbox montre la route par défaut — le test de `W5.k` l'affirme et passe — tandis que le constat `awk` de `open_outbound_connection` et de `reach_cloud_metadata_service`, dans **la même** sandbox et par le même `podman exec`, ne la trouve pas. Trois suites possibles, aucune acquise : l'`awk` de busybox ne découpe pas ces champs comme attendu ; la sonde sort avant d'atteindre le constat, par un chemin dont le code se lit comme un blocage ; ou les deux lectures ne voient pas le même `/proc`. Test de sortie : le code de sortie de chaque sonde est **rapporté tel quel** à côté de son verdict — le harnais le réduit aujourd'hui à trois états, et c'est ce qui empêche de départager |
| W5.n `[R]` **fait** | **255, et la sonde PID qui faisait mentir les suivantes** | la colonne `code` de `W5.m` a montré le motif d'un coup : **toutes** les sondes situées après `exceed_pid_quota` rendaient 255. Elle sature délibérément le quota de PID, `podman exec` ne peut plus forker, et il abandonne avec son code générique — que le harnais ne cataloguait pas et lisait donc comme un **blocage**, c'est-à-dire comme une preuve d'isolation. Trois « sur-confinements » n'existaient pas et un « tient » n'était pas mérité. 255 rejoint 125, 126 et 127 ; la sonde PID tue et attend ses enfants sur le chemin où elle va au bout |
| W5.o `[R]` **fait** | **une sonde ne contamine pas la suivante** | tuer les enfants ne suffit pas : si le shell meurt de ne pouvoir forker, le nettoyage ne tourne pas, et le cgroup reste saturé. Le catalogage de 255 fait passer les sondes suivantes de « fausse preuve » à « aveu d'ignorance » — c'est la seule des deux valeurs qu'on ait le droit d'écrire, et ce n'est pas une mesure. Test de sortie : après une sonde qui épuise délibérément une ressource, la suivante est **lancée pour de bon** — constaté sur un hôte réel, et par un code qui n'est pas 255 ; l'ordre de `SUITE` cesse de décider de ce que les sondes mesurent |
| W5.p `[R]` **fait** | **la contamination n'est pas transitoire** | `W5.o` a rendu les verdicts honnêtes — les trois faux « sur-confinements » sont devenus des « non concluant » — mais les trois sondes restent en **255 après six tentatives étalées sur 6,3 s**. Un cgroup occupé se libère ; ceci ne se libère pas. L'hypothèse à instruire est que le **conteneur lui-même** ne répond plus — `exceed_pid_quota` le tue, directement ou par épuisement — auquel cas aucune reprise ne peut aboutir et ce n'est pas de la contamination mais une **destruction**. Test de sortie : après chaque sonde, l'état du conteneur est constaté ; une sandbox morte est dite morte, et les sondes qui suivent ne sont pas rapportées comme « pas lancées » — elles ne sont **pas rapportées du tout**, parce qu'il n'y avait plus rien pour les lancer |
| W5.q `[R]` **fait — et l'instrument a répondu du premier coup** | **ce que `podman exec` dit quand il refuse** | le conteneur est **vivant** — `is_running` répond, et les sondes rendent « code générique », jamais « sandbox morte » — et `podman exec` refuse malgré tout pendant plus de six secondes. Ni cgroup transitoire (`W5.o`), ni sandbox morte (`W5.p`). Le harnais **jetait** ce que le runtime écrit sur son erreur, et c'était la seule chose qui restait à lire. `Trial` porte désormais un `detail` : le `stderr` du refus, nettoyé, rapporté sous le code dans la table. Trois absences distinctes tiennent, et aucune ne se collapse dans les deux autres — un refus **muet** rend `None` et non une chaîne vide, une sonde qui **aboutit** ne porte rien à expliquer, une sonde **jamais lancée** n'emprunte pas sa `reason` comme détail : ce que *nous* constatons n'est pas ce que le *runtime* a dit. **Le premier passage a nommé la cause** — voir `W5.r` |
| W5.r `[R]` **fait** | **une sonde par sandbox**, parce que le nettoyage ne peut pas être garanti | le détail de `W5.q` a répondu en une ligne, et il dit autre chose que les trois hypothèses tombées. `exceed_pid_quota` rend **2** avec `sh: can't fork: Resource temporarily unavailable` : le shell de la sonde est **mort** au premier fork refusé, donc son `kill $pids; wait` n'a jamais tourné — ce que le commentaire de `PID_QUOTA` annonçait comme risque résiduel. Les quatre sondes suivantes rendent **255** avec `container create failed (no logs from conmon)` : `podman exec` crée un `conmon` par session, ce `conmon` naît dans le cgroup PID du conteneur, il est encore à `pids.max`, et il meurt avant d'écrire son JSON de synchronisation — d'où un tuyau vide, lu comme le code générique. Ni cgroup transitoire, ni sandbox morte : **un cgroup saturé que plus personne ne peut vider**. Réparer le nettoyage de la sonde ne suffit pas, parce qu'aucune sonde ne peut promettre de survivre à ce qu'elle épuise. Test de sortie : chaque sonde s'exécute dans une sandbox **qu'aucune autre n'a touchée**, créée et retirée pour elle seule ; la contamination cesse d'être évitée pour devenir **inexprimable**, et sur l'hôte réel les quatre sondes qui suivent `exceed_pid_quota` rendent un code qui n'est pas 255. Le coût — seize créations au lieu d'une — est borné et mesuré dans la PR. **Constaté sur l'hôte réel :** les quatre sondes qui suivaient `exceed_pid_quota` rendent 1, 1, 0 et 0 au lieu de 255, et **quatorze des quinze tiennent**. `open_outbound_connection` et `reach_cloud_metadata_service` **réussissent** — ce qui clôt du même coup la question ouverte de `W5.m` : la route était bien là, et ces sondes ne la trouvaient pas parce qu'elles ne tournaient pas. Reste une seule dissidente, `reach_host_kernel_interfaces`, avec son motif en clair sous la ligne — `head: /sys/kernel/vmcoreinfo: No such file or directory` — c'est-à-dire exactement `W5.i`. Coût : 180 s contre 39 s, pour seize fois plus de conteneurs |
| W5.u `[R]` **fait** | **le second test hôte devient bloquant** | la roadmap l'avait annoncé : « s'il devient vert, `continue-on-error` tombe ». Il est vert — les seize sondes tiennent sur un runner ordinaire. Le job `sandbox` ne peut pas perdre `continue-on-error` tel quel, parce qu'il porte aussi `cet_hote_tient_il_s2_sous_une_mission_qui_reserve_du_disque`, qui exige un hôte XFS que le runner n'est pas. Test de sortie : les seize sondes sont exercées à chaque passage et un échec **fait rougir la CI** ; l'épreuve du quota disque reste tolérée à part, et son exclusion est nommée dans le workflow plutôt que subie. **Fait** : `continue-on-error` est tombé du job, le pas des seize sondes fait rougir la CI, et l'épreuve XFS a son propre pas — `continue-on-error` sur lui seul, sa sortie dans le résumé, son motif écrit. Le jour où le runner devient XFS, ce pas rejoint le précédent au lieu d'être supprimé |
| W5.t `[R]` **fait, dans la PR de `W5.r`** | **aucun appel au runtime ne dure indéfiniment** | trouvé en cherchant un faux coupable : le job de `W5.r` a paru pendre, l'hypothèse était qu'une sandbox saturée en PID bloquait son démontage, et **l'hypothèse était fausse** — le job avait fini en 3 min 36, l'état rapporté était périmé. Le défaut trouvé en route est réel : `SystemRunner::run` appelait `Command::output()`, sans borne, contre la règle du dépôt — « timeouts et cancellation » — au seul endroit qu'aucun test ne traversait. `W5.r` fait passer ces appels non bornés d'une poignée à quatre-vingts par campagne. Test de sortie : un appel qui ne rend pas la main est abandonné avec un motif qui le dit, et un appel qui répond dans son budget rend sa sortie intacte — les deux tenus sans Podman, en visant `sleep` puis `echo` |
| W5.s `[R]` **fait** | **« absent » et « il a refusé » sont deux erreurs, le driver n'en a qu'une** | trouvé en écrivant `W5.r`, par ses propres doubles. `PodmanBackend::expect_success` rend `RuntimeError::Unavailable` aussi bien quand le binaire est introuvable que quand `podman create` répond 125 : « je n'ai pas pu demander » et « on m'a répondu non » se réparent pourtant ailleurs — l'un en installant un runtime, l'autre en lisant ce qu'il reproche à la spécification. `SANDBOX_REFUSED` a donc dû être nommé pour ce qu'il couvre réellement — « la sandbox n'a pas pu être ouverte » — au lieu de la distinction qu'on voulait y mettre, parce qu'un nom que la couche du dessous ne sait pas honorer ment une fois sur deux. Test de sortie : une variante d'erreur distincte pour « le runtime a répondu un code non nul », portant le verbe et le code ; un runtime absent et un runtime qui refuse ne rendent plus la même erreur, et `Trial::refused` choisit sa raison dessus plutôt que sur un texte. **Fait** : `RuntimeError::Refused` porte le verbe, le code et le `stderr` séparément — un appelant qui veut décider n'a pas à lire une phrase pour retrouver un entier. `SANDBOX_REFUSED` cesse de couvrir le silence, qui redevient `UNREACHABLE_RUNTIME`. Les deux tests étaient déjà **côte à côte** dans `podman.rs`, affirmant la même variante pour les deux causes : le défaut était écrit, et il passait |
| W5.i `[M]` **fait** | **ce que `S4` promet**, et la sonde qui doit le constater | `reach_host_kernel_interfaces` constate que le noyau atteint **n'est pas celui de l'hôte**, et non qu'une lecture est refusée : un conteneur partage le noyau, une micro-VM en apporte un autre, et « je n'ai pas le droit de lire » ne distingue pas les deux. L'arbitrage nomme ce qui est observable de l'intérieur — version, `boot_id`, ou autre chose — et le refus de lecture, s'il subsiste, devient `NotRun` avec sa raison plutôt qu'un blocage. **Arbitrage rendu : `boot_id`.** Le noyau en régénère un à chaque démarrage ; un conteneur partage celui de son hôte parce qu'il partage son noyau, une micro-VM démarre le sien. La version du noyau ne discrimine pas — une micro-VM peut faire tourner la même. La sonde ne pouvant pas connaître seule le `boot_id` de l'hôte, le harnais le lui dit par `LOCUS_HOST_BOOT_ID`, comme il lui dit où le quota mord ; sans lui elle rend **120**, parce que ne pas savoir comparer n'est pas avoir comparé. `microvm-high-risk` veut donc dire, de l'intérieur : « le noyau que j'atteins n'est pas celui de l'hôte ». **Constaté sur l'hôte réel : les seize sondes tiennent.** `reach_host_kernel_interfaces` rend 0 et réussit, ce que `S2` promet — un conteneur partage bien le noyau de son hôte. Le second test hôte est **vert**, et seul reste celui qui exige XFS |

**L'épreuve est écrite ; ce qui manque est l'hôte, et on va savoir si la CI en est un.**
`apps/locus-execd/tests/host_sandbox.rs` fait tourner les seize sondes dans un conteneur rootless
réel à `S2`, imprime la table complète — sonde, attente, observation, verdict — **avant** toute
assertion, puis affirme que chacune tient et que le `Standing` est `Trusted`. Le test est
`#[ignore]` : un test qui se sauterait tout seul quand l'hôte ne convient pas ressemblerait en tout
point à un test qui passe, et `ignored` apparaît dans la sortie de `cargo test` là où « sauté en
silence » n'apparaît pas.

Le job de CI `sandbox` le lance. Il a longtemps été **`continue-on-error: true` délibérément et
temporairement**, avec sa condition de sortie écrite d'avance : « s'il devient vert,
`continue-on-error` tombe ». **Il est vert, et la tolérance est levée** (`W5.u`) — les seize sondes
sont exercées à chaque passage et un confinement qui cesse de tenir fait rougir la CI. Ce qui reste
toléré est nommé plutôt que subi : l'épreuve du quota disque exige un hôte XFS, elle a son propre
pas, et sa sortie reste au résumé.

Trois états se distinguent, et ils ne se réparent pas pareil : pas de runtime rootless (il manque
une machine), un runtime qui refuse la spécification (il manque une capacité d'hôte), un confinement
qui ne tient pas (il manque une garantie).

**Le premier passage a répondu, et la réponse n'était dans aucune des cases attendues.** Le runner
GitHub **fait tourner Podman rootless** — l'image s'est construite, la référence par digest s'est
résolue — et `podman create` a rendu 125 : « storage option overlay.size and overlay.inodes only
supported for backingFS XFS. Found extfs ». Le quota disque de `ConfinementPlan::disk_bytes` devient
un `--storage-opt size=`, que Podman ne sait appliquer que sur XFS ; le runner est en ext4.

Deux conséquences, et elles sont de natures différentes.

**Un défaut, qui devient `W5.g`.** `probe.rs` a pour doctrine « ce que l'hôte permet réellement — lu,
jamais supposé », et son en-tête écrit pourquoi : « un broker qui apprendrait ses limites en échouant
les découvrirait après avoir créé la moitié d'une sandbox ». C'est exactement ce qui se passe pour le
quota disque, et le module le frôle sans en tirer la conséquence — `REQUIRED_CONTROLLERS` note que
« le quatrième, le disque, ne se borne pas par cgroup » et s'arrête là. Ce défaut n'était pas
trouvable par un double : il fallait un vrai `podman create` sur un vrai système de fichiers.

**Une trouvaille de plus, et elle a démenti l'hypothèse qui l'a fait chercher.** Le constat de route
ajouté par `W5.h` a rendu « pas de route par défaut » dans la sandbox. Trois vérifications l'ont instruit sans coûter un
seul passage de CI supplémentaire : les arguments produits par `create_arguments` portent bien
`--network=host` ; le constat de route, rejoué hors ligne sur la sortie réelle de `/proc/net/route`,
trouve la route et ne la trouve pas sur un namespace vide ; et un `podman run --network=host` nu sur
le runner voit la route, résout les noms et rend `200` sur `example.org`. Ce qui reste est que **la
sandbox du plan n'obtenait pas le réseau que la mission déclare. **L'instrument construit pour le
vérifier l'a démenti** : la sandbox voit la route, et le test qui l'affirme passe sur le runner.

Ce que les trois passages cachaient était plus banal et plus grave. `RuntimePort::stop` lance
`podman stop` et **rien ne lance `podman rm`** : le conteneur garde son nom, le suivant qui demande le
même nom échoue, et chaque test construisant son propre backend, tous repartent de `locus-0001`. Les
tables de sondes lues jusqu'ici viennent donc de passages où l'un des trois conteneurs seulement
existait — les autres rapportaient une erreur de nom là où on attendait un verdict de confinement.
C'est `W5.l`, et c'est ce qu'il faut réparer avant de croire une seule ligne de plus de ces tables.

**Une réponse à l'arbitrage.** Un runner GitHub n'est pas un hôte `S2` pour ce dépôt, et la raison
n'est pas réparable en CI — le système de fichiers du runner n'est pas un réglage. `W5.f` demande
donc une **VM à système de fichiers XFS**, ou un report écrit — et cette moitié-là attend toujours un
hôte, pas une décision. L'autre moitié, elle, n'attend plus : depuis `W5.u`, les seize sondes sont
bloquantes, et seule l'épreuve du quota disque reste tolérée, dans un pas qui la nomme.

**Et ces quinze ont parlé.** Le second passage a fait tourner la suite entière dans un conteneur
rootless réel. **Douze sondes tiennent** : les écritures hors espace de travail, le home de
l'utilisateur, la persistance après démontage, la lecture de la racine et des secrets de l'hôte, le
socket de runtime, l'élévation à root, la vue sur les processus de l'hôte, et les trois quotas
cgroup. C'est la première fois que ce dépôt a une preuve, plutôt qu'une syntaxe vérifiée.

**Quatre sont démenties, et elles deviennent `W5.h`** — voir son test de sortie ci-dessus pour le
détail. La forme du résultat vaut d'être notée : `read_process_environment` échappait **parce que le
confinement était correct**, et elle est `critical`, donc tout hôte bien configuré se voyait refuser
la confiance. C'est le contraire du défaut qu'on cherchait, et exactement ce qu'un double ne pouvait
pas dire.

**Une cinquième tenait pour la mauvaise raison**, et seul le second test pouvait le montrer :
`exceed_disk_quota` est ressortie « bloquée → tient » sous une mission qui **ne déclarait aucun
quota**. Elle écrit à la racine, que `S2` monte en lecture seule ; elle mesure donc la racine, pas le
quota. Une sonde qui passe sans que ce qu'elle teste existe est le pire des trois états, parce qu'elle
ne se plaint jamais.

**`W5.h` est faite pour trois sondes sur cinq ; les deux autres attendent, et pas par paresse.**

`exceed_disk_quota` appartient à **`W5.g`**. La corriger demande de savoir **où** un quota disque
s'applique, et le premier hôte réel a montré que la réponse n'est pas celle du code actuel : à `S2` la
racine est montée en lecture seule, donc `--storage-opt size=` dimensionne une couche inscriptible que
personne n'écrit. Déplacer la sonde vers l'espace de travail sans déplacer le quota ferait mesurer un
système de fichiers hôte non borné. La sonde suit le quota ; le quota d'abord.

**`W5.g` est faite, et elle a réglé la moitié « lecture ».** L'hôte dit désormais, **avant toute
création**, si le système de fichiers qui portera le stockage sait tenir un quota de projet :
`HostFacts` lit `/proc/self/mountinfo`, retient le montage effectivement traversé — le plus long
préfixe, pas le premier venu — et rend `Available`, `Unavailable` ou `Undetermined`. L'admission
refuse une mission qui réserve du disque sur un hôte qui ne sait pas la borner, par un motif
`DiskQuotaNotEnforceable` **distinct** de `CapacityExceeded` : « la capacité manque » envoie libérer
de la place, « la borne n'est pas applicable ici » envoie changer de machine, et réduire la
réservation ne changerait rien.

Ce qui reste dû à `exceed_disk_quota` n'est donc plus un fait mais une **décision de driver** : où le
quota s'applique quand la racine est en lecture seule. C'est le sujet de `W5.j`.

`reach_host_kernel_interfaces` demande un **arbitrage sur ce que `S4` promet**, et devient `W5.i`.
Elle lit `/sys/kernel/vmcoreinfo`, réservé à root — elle échoue donc pour cette raison-là, sur tout
hôte, à tout niveau. Mais la corriger vers une interface lisible ne suffit pas : `S4` apporte **un
autre noyau**, ce qui n'empêche personne de lire les interfaces de ce noyau-là. Ce que la sonde doit
constater est donc que le noyau atteint **n'est pas celui de l'hôte**, ce qui est une autre mesure que
« la lecture est refusée ». Trancher cela change ce que `microvm-high-risk` veut dire, et ne se fait
pas en corrigeant une commande shell.

**Ce que cela ouvre.** Une fois `W5.h` faite, le second test peut devenir **vert et bloquant en CI** :
quinze sondes de sandbox réellement exercées à chaque passage, sur un runner ordinaire. `W5.f` — les
seize, `S2` établi — continue d'attendre un hôte XFS.

Le fichier de test porte donc **deux tests** : le premier demande si l'hôte tient `S2` sous une
mission qui réserve du disque et, quand la sandbox ne démarre pas, rend le message du runtime mot
pour mot ; le second éprouve les quinze autres sondes et **n'établit jamais `S2`**, l'exclusion y
étant nommée plutôt que silencieuse. Un seul test aurait dû choisir entre ne rien observer et
conclure sur `S2` sans le quota : le premier n'apprend rien, le second serait faux.

## W6 — Artifact / reproductibilité

Object store, manifests, quarantaine/promotion, `RunManifest`, workflows de reproduction.

L'ordre est celui de W5 : le vocabulaire et ses refus d'abord, dans un paquet sans dépendance ; le
stockage ensuite, derrière un port (ADR 0012). Un manifeste qui ne sait pas dire non n'a pas besoin
d'un object store pour être faux.

| # | Commit | Test de sortie |
|---|---|---|
| W6.a `[R]` **fait** | `packages/artifacts` : `ArtifactManifest` et la machine à états quarantaine/promotion | un contenu dont le hash n'est pas celui qui avait été déclaré est refusé ; `declared → promoted` n'existe pas ; un artefact promu ne se déprome pas ; l'histoire des états traversés ne s'efface pas |
| W6.b `[R]` **fait** | **correction de W6.a** : le manifeste dit ce que le schéma dit, et la traduction vers le fil existe | un manifeste portant tous les champs facultatifs fait un aller-retour **exact** par `artifact-manifest.schema.json` ; un état, une relation ou une taille hors bornes sont refusés par leur nom ; le domaine ne construit rien que le schéma refuserait |
| W6.c `[R]` **fait** | l'object store derrière un port, avec un backend en mémoire pour les tests | aucun octet n'entre sans manifeste déclaré ; la taille annoncée borne l'upload avant de l'accepter ; un contenu non conforme ne laisse rien derrière lui |
| W6.d `[R]` **fait** | `RunManifest` relu et jugé, et le niveau de reproductibilité de §19.7 | le niveau se **calcule** depuis ce que le manifeste consigne ; un niveau déclaré au-dessus est refusé en nommant ce qui manque ; `R3` et `R4` ne se lisent dans aucun manifeste seul |
| W6.e `[R]` **fait** | workflow de reproduction sur le backend déterministe de W3 | rejouer un `RunManifest` produit les mêmes hashes d'artefacts, et une divergence est un résultat rendu, pas une erreur avalée |

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
| W7.a `[R]` **fait** | `packages/review` : `ReviewDossier` figé, `Review`, `Finding`, et l'attestation d'indépendance de §14.4 | un dossier modifié après attribution change de version ou porte un addendum visible ; deux relecteurs du même groupe d'indépendance ne comptent pas comme indépendants ; une revue sans attestation n'est pas une revue indépendante |
| W7.b `[R]` **fait** | prévention de contamination (§16.6), par **cas adverses** | cinq cas, un par forme nommée : transcript du générateur atteignant un relecteur aveugle ; claim réfuté propagé comme contexte par défaut ; donnée confidentielle atteignant un worker non autorisé ; consensus circulaire sans source externe ; contradiction perdue à la synthèse. Chacun échoue **avant** le correctif |
| W7.c `[R]` **fait** | `ContextView` : ce qui a été vu, arrêté par hash et par watermark (§16.2) | deux vues du même instant du journal ont le même hash ; une vue qui aurait vu un événement postérieur à son watermark est refusée |
| W7.d `[R]` **fait** | rebuttal et méta-revue (§17.6, §17.7) | un rebuttal ne s'écrit pas sans finding ; une méta-revue ne relit pas sa propre revue ; le désaccord survit à la synthèse |
| W7.e `[R]` **fait** | budgets : réservation avant exécution, dépassement (§7.2, invariant 6) | une mission sans borne n'est pas admissible ; une réservation refusée n'exécute rien ; un dépassement arrête proprement et le dit |
| W7.f `[R]` **fait** | portefeuille : indicateurs de §13, et **l'anti-gaming de §13.6 d'abord** | une stratégie qui optimise l'indicateur sans produire de connaissance est détectée par un test qui la met en œuvre ; l'anti-gaming précède la fonction de valeur dans l'ordre des commits, et un test l'atteste |
| W7.g `[R]` **fait** | scheduler qualité-diversité | deux propositions de valeur égale et de diversité inégale ne sont pas départagées au hasard ; le choix est reproductible |

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
| W8.a `[R]` **fait** | le test de séparation : `apps/emacs` existe, se charge sous `emacs -Q` avec sa seule `load-path` | la frontière 5 passe de « sans objet » à vérifiée ; charger le paquet n'ouvre aucune connexion, n'arme aucun timer et ne tire aucune bibliothèque hors du paquet et d'Emacs ; la version de protocole annoncée est celle de `schemas/`, lue et non recopiée |
| W8.b `[R]` **fait** | authentification abstraite (§6) | aucun secret hors `auth-source` ; une identité absente est une erreur actionnable, pas un plantage |
| W8.c `[R]` **fait** | événements, curseurs et reprise (§14.1, §7.5) | une déconnexion ne perd ni ne duplique un événement ; un trou est marqué, pas tu ; l'élagage du tampon n'emporte jamais un événement critique |
| W8.d `[R]` **fait** | dashboard et buffers (§9) | un buffer se reconstruit depuis le cache sans réseau |
| W8.e `[R]` **fait** | commandes et transient (§10, §11) | toute action mutante passe par l'API avec `expected_revision` ; un conflit est rendu, pas écrasé |
| W8.f `[R]` **fait** | artefacts et inspecteur de sandbox | un artefact non promu se distingue d'un artefact promu à l'écran |
| W8.g `[R]` **fait** | intégrations Org/Magit/Jupyter/xiiif | chaque intégration absente dégrade sans casser le démarrage |
| W8.h `[R]` **fait** | 3D et WebView | la 3D reste une projection ; aucune vue n'écrit dans le graphe |
| W8.i `[R]` **fait** | le **transport** : requête HTTP construite, réponse relue, socket isolée derrière un port | une requête se construit et se relit sans réseau ; l'erreur structurée du serveur arrive au client comme une erreur structurée, pas comme un code ; un aller-retour réel contre un serveur local passe |
| W8.j `[R]` **fait** | Emacs **auteur** : rédaction de politiques et de propositions, et l'analyseur qui produit la forme canonique | **deux propriétés distinctes, deux tests** — `canonical(parse(t))` est invariante par ajout de commentaire et par réordonnancement, et `parse(write(p))` rend la **valeur** `p` ; la forme d'écriture n'est donc **jamais** la forme canonique ; une commande rend une `Proposal` que **rien n'applique sur place** ; le test de séparation `emacs -Q` reste vert |

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
| W9.a `[R]` **fait** | `packages/visualization` : les huit projections de §23.3, versionnées et hashées, derrière un port de condensat | deux rendus du même contenu ont la **même** forme canonique quel que soit l'ordre d'insertion ; une vue modifiée n'est plus la vue — sa forme canonique change ; une vue en retard sur le journal le **déclare** au lieu de se dire à jour ; une arête sans extrémité est refusée |
| W9.b `[R]` **fait** | `ArtifactViewerRegistry` (§23.5) : l'artefact suggère, le client choisit | un hint que le client ne sait pas honorer se replie sans échouer ; aucun artefact n'impose de viewer (invariant 10) ; l'absence de tout viewer laisse l'artefact atteignable |
| W9.c `[R]` **fait** | interaction de §23 : `focus`, `filter`, `select` vers le viewer, `node_selected` en retour | un événement de viewer ne produit **jamais** de mutation ; le type ne laisse aucun chemin qui contourne l'API de commandes |
| W9.d `[R]` **fait** | `apps/web` : la scène de référence, 2D d'abord, sur la vue hashée de W9.a | l'application ne détient aucun graphe modifiable : elle rend une `View` et toute interaction repart par l'API de commandes |
| W9.e `[R]` **fait** | la scène **3D** du graphe épistémique dans `apps/web` — `W9.d` s'était arrêté à la 2D | la scène rend la `View` hashée de `W9.a` et n'en détient **aucune** copie modifiable ; une hyperarête se distingue visuellement d'une relation binaire, et un test le tient sur la structure rendue et non sur des pixels ; le graphe de **coordination** reste en 2D et la scène 3D le refuse en le nommant ; toute interaction repart par l'API de commandes, comme `W9.c` l'exige |

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
| W6.f `[R]` **fait** | `RemoteArtifactRef` (§19 de `xiiif/SPEC_V1.md`) : identité Locus, media type, hashes attendus, **un seul** locator | un document à deux locators est refusé, un document sans locator aussi ; le snapshot prouve la reproduction, la ressource live ne prouve rien ; une divergence entre les deux ne rend jamais la preuve historique douteuse |
| W6.g `[R]` **fait** | formats d'artefact **3D** et suggestions de viewer | un maillage entre comme artefact avec son `ArtifactManifest`, son `Integrity` et son `ProducedBy` ; il **suggère** un viewer par `ViewerHints` sans l'imposer, et l'invariant 10 est exercé par un client qui ignore la suggestion ; un artefact 3D dont la `reproducibility::Assessment` porte un `Missing` ne se promeut pas, et le refus nomme ce qui manque |
| W7.h `[R]` **fait** | `HumanReviewFinding` (§20 de `xiiif/SPEC_V1.md`) : les quatre verdicts humains, le commentaire libre, et le schéma que xiiif écrit | un verdict humain ne rend **jamais** `supports` ; `source-changed` ne réfute rien ; un finding dont la cible n'est pas dans le dossier est refusé ; un enregistrement qui ne dit ni verdict ni commentaire est refusé par son schéma |
| W10.7 `[R]` **fait** | xiiif consomme `RemoteArtifactRef` : `xiiif-open-locus-artifact`, affichage séparé des cinq facettes | les cinq facettes de §19 sont distinctes à l'écran ; une ressource live modifiée après le run ne fait pas croire que la preuve a changé |
| W10.8 `[R]` **fait** | revue humaine de §20 : `accept`, `needs-correction`, `wrong-target`, `source-changed` | un verdict produit un finding attachable à un `ReviewDossier`, et xiiif ne valide rien lui-même |

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
| W11.a `[R]` **fait** | `packages/deployment` : les cinq profils de §27.1 sous leur nom, `DeploymentProfile` et le verdict de `locus doctor` | un adaptateur déclaré mais absent rend le profil **inexécutable**, en nommant ce qui manque ; « pas vérifié » n'est jamais « présent » ; deux profils de topologies différentes exposent la même surface cliente |
| W11.b `[R]` **fait** | `deployment.yaml` : le schéma, les secrets **dehors**, `locus deployment explain` | un document qui porte un secret en clair est refusé par son schéma ; `explain` nomme les backends actifs sans nommer d'hôte interne |
| W11.c `[R]` **fait** | sauvegarde cohérente de §27.4 et restauration sur un backend différent | une sauvegarde qui omet une des cinq parties se refuse à s'appeler cohérente ; une campagne restaurée sur un backend qui n'a pas les capabilities de ses runs historiques le déclare au lieu de rejouer |

## W12 — Evaluation / release

Tests de sécurité, injection de fautes, endurance, benchmarks, ablations, docs, release candidate.

Section en prose, décomposée ici comme W9 et W11 l'ont été. §29 nomme des listes closes — treize
fautes à injecter (§29.4), quatorze attaques (§29.5), huit ablations (§29.8) — et c'est cette
clôture qui rend l'exercice vérifiable : une liste nommée permet de dire ce qui n'a **pas** été
éprouvé, ce qu'une intention générale ne permet jamais.

| # | Commit | Test de sortie |
|---|---|---|
| W12.a `[R]` **fait** | `packages/evaluation` : le registre d'épreuves de §29.4, §29.5 et §29.8, et le verdict de préparation | une épreuve non traitée empêche la préparation, en la nommant ; une épreuve **écartée** exige sa raison, et une raison vide ne passe pas ; « pas éprouvé » n'est jamais « réussi » |
| W12.b `[R]` **fait** | endurance de §29.6 : les huit seuils, et le constat qui les confronte | une campagne qui manque un seul seuil ne se dit pas endurante, et le verdict nomme lequel ; un compteur non relevé n'est pas un seuil atteint |
| W12.c `[R]` **fait** | benchmarks de §29.7 : les six configurations et les onze mesures | une comparaison à laquelle il manque une configuration se déclare partielle ; une mesure absente n'est pas une mesure nulle |
| W12.d `[R]` | `e2e/minimal_science` et son jumeau avec panne — **le centre de gravité du projet** | **tenté une première fois, et voici ce que la tentative a trouvé.** Le marqueur de la quatrième reprise avait été levé — les huit items qu'il attendait étaient livrés, plus `W20.p`, `W20.q` et `W20.r`. Aucun item nommé ne bloquait donc plus celui-ci, et la conclusion écrite alors était de **tenter le test** plutôt que d'écrire une cinquième liste. C'est ce qui a été fait, et la tentative s'est arrêtée **avant la première ligne de code** : `e2e/minimal_science` n'est nommé nulle part hors de cette roadmap, et le seul worker du chantier vit dans un autre dépôt, dans un autre langage. Cinq conditions manquaient, **toutes vérifiées au code** et aucune déduite du texte : `lep_propose` et `lep_queue` n'ont aucune route, donc le test ne peut pas *commencer* (`W20.s`) ; `apps/locusd` ne dépend pas de `locus-artifacts`, donc « les artefacts sont hashés » n'a pas de sujet (`W20.t`) ; aucune route ne sert le graphe et aucune projection ne porte le coût, donc la clause la plus longue n'en a pour aucun de ses six termes (`W20.u`) ; `runWorker` exige qu'on lui **fournisse** ses ports et personne ne les assemble, donc le worker réel ne se lance pas (`W2.22`) ; et le harnais lui-même n'existe pas, sa place étant une décision à prendre et non un oubli (`W12.f`). **Ceci n'est pas une cinquième liste de blocage, et la ligne ne déclare toujours rien : elle reste dans la frontière.** La différence est de nature. Les quatre listes précédentes *devinaient* la précondition de la chaîne entière ; ces cinq items sont ce qu'une tentative a **constaté**, chacun tient debout seul, et leur complétude n'est toujours pas affirmée — la prochaine tentative en trouvera vraisemblablement d'autres, et c'est le fonctionnement voulu plutôt qu'un échec. C'est la leçon de `W0.16` prise par le bon bout : un marqueur qui *anticipe* se périme, un item qui *constate* ne se périme pas.

Ce qui change ici est la méthode : chaque entrée de la nouvelle liste a été **vérifiée au code** avant d'être écrite, et non déduite du test de sortie. `MemoryQueue::take` ignorait le `worker_id` — sa documentation le disait — donc « placé sur ce qu'il a prouvé » n'avait pas de sujet ; `W20.q` l'a livré, et la réclamation demande désormais le placement au broker ; rien dans `apps/locusd` ne connaissait `EpistemicCommit` hors d'une traduction de type, donc « mis en scène puis intégré » n'en avait pas non plus ; `W20.r` l'a livré, et a trouvé au passage qu'un commit remonté par §15.6 mettait la projection de §9.3 en quarantaine — donc empêchait le daemon de redémarrer — après avoir répondu `202` au worker. `W20.p` était le dernier, et il est livré ; il a démontré la famine **rouge avant vert** avant de la corriger. **Cette liste-ci ne s'est donc pas périmée** — c'est la première, après quatre. Ce qui ne s'affirme toujours pas est sa complétude : ce qui y figurait manquait réellement, et il peut rester ce que personne n'a encore cherché. La preuve est `W12.d` lui-même, qui n'a jamais tourné. Les phases `W5` et `W7`, elles, sont **entièrement livrées** — vérifié, pas supposé.

**La complétude de cette liste n'est pas affirmée.** C'est la seule chose que quatre expirations autorisent à dire : ce qui y figure manque réellement, et il peut en manquer d'autres que personne n'a encore cherchées. Confronté clause par clause à son propre test de sortie, il manquait alors trois choses — **les trois sont livrées depuis**, et aucune n'avait d'item : **une question produit une mission** — rien ne crée de `MissionEnvelope`, la file de `W20.k` n'est remplie que par un test (`W20.o`) ; **un worker s'enregistre** — l'enrôlement de §7.2 n'est servi par personne, le registre de `W20.k` n'est rempli que par un test (`W20.n`) ; **`locusd` redémarre et tout est encore là** — `W20.i` a livré le driver sans le câbler, ce qui était son périmètre (`W20.m`). La leçon est celle de `W0.16`, apprise pour la troisième fois : un marqueur qui nomme un jalon au lieu d'une condition se périme en annonçant un déblocage qui n'a pas lieu. Test de sortie : une question produit une mission ; un worker s'enregistre et est placé sur ce qu'il a **prouvé** ; la session tourne ; les outils s'exécutent dans le confinement promis ; les événements repartent ; les artefacts sont hashés ; un `EpistemicCommit` est mis en scène puis intégré ; les projections se mettent à jour ; le graphe rend la conclusion, ses prémisses, son expérience, ses artefacts, ses objections et son coût ; `locusd` redémarre et **tout est encore là**. Le jumeau tue le worker au milieu : la lease est revalidée, la reprise se fait sous le même numéro de tentative, **zéro perte et zéro doublon**. Le confinement est celui qu'un runner peut tenir — `S1`/`S2` —, et il n'est pas **attesté** : c'est `W12.e` |
| W12.e `[M]` **reporté** | l'**attestation** du confinement et la clause externe de `e2e/minimal_science` | `attend:externe` — même hôte que `W18.f`, et pour les mêmes trois raisons rendues précises par `W5.f`. Test de sortie : les outils s'exécutent dans un confinement `S3`/`S4` **attesté**, et l'attestation est vérifiée et non crue ; un tiers exécute les artefacts et retrouve les résultats — la clause qui compte, puisqu'un compilateur passant 99 % du torture test de GCC a produit 40 784 erreurs d'édition de liens chez qui l'exécutait. Séparé de `W12.d` parce qu'un test de bout en bout qui attend une machine dédiée n'aurait jamais tourné, et que l'attendre aurait retardé tout ce qu'il vérifie par ailleurs |
| W12.f `[R]` | le **harnais** de `e2e/minimal_science` : où il vit, et qui joue le worker | il n'existe pas, et la première tentative de `W12.d` l'a établi avant d'écrire une ligne : `e2e/minimal_science` n'est nommé **nulle part** hors de cette roadmap — pas de répertoire, pas de job de CI, pas d'ADR. Et le seul worker du chantier est dans un autre dépôt, dans un autre langage ; `packages/testing` ne peut pas le remplacer, puisqu'il joue le **serveur** par construction (`W0.9`, et `W20.k` s'y est heurté). Ce que cet item tranche : où le test vit, et comment le worker réel y arrive. Les deux options ont un coût réel et opposé — faire entrer `canterel` dans la CI de `locusolus` (une chaîne Bun et un checkout épinglé, coût ponctuel) ou faire entrer `locusolus` dans celle de `canterel` (une chaîne Rust et un checkout, hors du périmètre local, donc **payé à chaque synchronisation amont**, ADR 0010). La décision demande un ADR. Elle n'est **pas** prise ici : la prendre avant que `W20.s`, `W20.t`, `W20.u` et `W2.22` existent serait décider avant l'évidence, ce que ce dépôt refuse ailleurs sous le nom d'abstraction spéculative. Test de sortie : un ADR tranche la question en mesurant les deux coûts plutôt qu'en les estimant ; le harnais démarre un `locusd`, un `locus-execd` et le worker réel, et les arrête ; il échoue **bruyamment** quand l'un des trois ne démarre pas, au lieu de rendre vert un test qui n'a rien exercé — c'est la règle du dépôt, et `W20.i` a montré ce qu'elle coûte quand on l'oublie |

---

## W13 — Socle de coordination agentique — **repli, jamais prioritaire** — niveau 3 sur `review`

Couvre le socle de §7.1 et les deux seuls manques de §14 (ADR 0016, décision 3). Ne prend jamais la
priorité sur W4 ; les périmètres ne se recoupent pas. Aucun item ne modifie `canterel`. W13.c et
W13.d sont néanmoins des **dépendances de W7** : « repli » ordonne, il ne déprogramme pas.

Les modes `observed` et `assisted` (ADR 0016, décision 8) existent dès W13.e. Le mode fermé est une
exigence de §33, pas une précaution.

| # | Commit | Test de sortie |
|---|---|---|
| W13.a `[R]` **fait** | ADR 0016, sixième frontière (`CLAUDE.md` + `boundaries.json` + garde), ce workstream, `docs/11`, `docs/13` | une violation délibérée **dans chacun des deux sens** fait échouer la CI, et la garde déclare le nombre de fichiers réellement examinés |
| W13.b `[R]` **fait** | pli des fixtures `lep/1.0` en graphe d'exécution, sous `tests/` — aucun champ ajouté au protocole | le pli rend un graphe attempt/outil/artefact **sans arête orpheline**, et un test affirme **par l'absence** qu'aucun champ d'agent n'existe dans l'événement LEP |
| W13.c `[R]` **fait** | `packages/coordination` : `AgentTemplate`, `AgentInstance`, `Team`, `Decision`, `ApprovalRequest` selon §7.1 ; `Id<Team>`, `Id<Task>`, `Id<Decision>`, `Id<Approval>` dans `packages/protocol` | property test : la capacité effective est l'**intersection** des quatre sources de §14.2, jamais leur union ; ignorer une source fait rougir. Round-trip des quatre nouveaux identifiants |
| W13.d `[R]` **fait** | complétion de l'agrégat `Task` de §7.1 — dont `assigned_agent_id` et `assigned_worker_id` — sans toucher la machine à états existante | l'assignation est un événement ; la machine à états de `task.rs` est inchangée et ses tests passent |
| W13.e `[R]` **fait** | relation de coordination (`kind` fermé à `review`), payload de `team.modify`, CAS par `expected_revision`, annulation par commit inverse, autorité de proposition agentique | quatre : deux propositions concurrentes sur la même base ne committent pas toutes deux et le refus dit s'il faut rebaser ; une proposition sans justification citant un objet épistémique existant est refusée ; aucun chemin de code ne modifie une `MissionEnvelope` émise ni le hash de sa `ContextView` ; une proposition d'origine agentique suit le même chemin qu'une proposition humaine et son proposeur ne peut pas l'approuver |
| W13.f `[R]` **fait** | `packages/projections` : projection du graphe d'exécution | reconstruction depuis zéro = état courant ; quarantaine conforme à ADR 0013 |
| W13.g `[R]` **fait** | projection du graphe organisationnel réalisé, par jointure `assigned_agent_id` × événements | **dépend de W13.b et W13.d.** Le graphe se reconstruit depuis le journal seul ; aucun instantané n'est reçu du worker |
| W13.h `[R]` **fait** | **ADR 0021** — `Version` porte le mode et le coordinateur ; `SET_MODE` et `SET_COORDINATOR` entrent ensemble | les trois règles de §14.3 refusent depuis la `Version` et non plus depuis `Team::new` — une équipe sans membre, un coordinateur non membre, un mode `coordinator` sans coordinateur ; retirer ou remplacer le nœud coordinateur est refusé comme pour un rôle, parce que l'opération inverse ne saurait pas le rendre ; les deux opérations n'entrent qu'ensemble, et un test le tient — §14.3 les lie |
| W13.i `[M]` **fait** | **ADR 0021** — `Proposal` porte un `Diff` d'`Operation`s ; `proposal::Change` est retirée | un commit rend la **`Version` suivante** au lieu d'un compteur, et son parent est celle d'avant ; deux propositions concurrentes sur la même base ne committent toujours pas toutes deux, et le refus dit encore s'il faut rebaser ; `Change` n'existe plus, et un test d'absence le tient — un doublon retiré qui revient par une autre porte est un doublon |
| W13.j `[R]` **fait** | **ADR 0021** — `Team` projette la version courante au lieu de la stocker | `member_ids` et `coordination_mode` se lisent sur `Team` et **valent** ceux de la version courante, par égalité ; aucun champ de structure ne subsiste en propre, et un test d'absence le tient ; §7.1 est servi entièrement — ce que `Team` exposait était `title()` et rien d'autre |

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
| W14.a `[R]` **fait** | `packages/policy` : les cinq verbes de §20.2, la séparation faits/décision, la trace d'évaluation, la priorité explicite et le déterminisme | deux évaluations des mêmes faits rendent la **même** décision et la même trace ; deux règles qui se contredisent sont détectées et tranchées par priorité déclarée, jamais par ordre de déclaration ; une décision sans trace n'existe pas |
| W14.b `[R]` **fait** | `Delegation` de §20.4 : portée, plafonds, expiration, révocation | une action hors portée, au-delà d'un plafond ou après expiration n'est pas autorisée ; une délégation révoquée n'autorise plus rien, et l'attribution nomme **les deux** principals |
| W14.c `[R]` **fait** | explicabilité de §20.5, dont les **alternatives rejetées** | une décision expose ses huit facettes ; une alternative rejetée sans motif n'est pas une alternative rejetée ; un override humain reste visible après coup |
| W14.d `[R]` **fait** | les seize catégories de §20.1 et le dry-run de §20.2 | un dry-run ne produit aucun événement, et sa décision est identique à celle du run réel sur les mêmes faits |
| W14.e `[R]` **fait** | l'alignement d'ontologies comme **proposition** | un alignement proposé est une `Proposal` soumise à politique et approbation, donc portant un **diff d'opérations** qui se rejoue (ADR 0021) et non une déclaration inerte ; **aucun chemin n'écrit une équivalence sans décision**, tenu par l'absence ; le refus nomme la contrainte structurelle non satisfaite plutôt que de rendre un score ; deux propositions d'alignement contradictoires sur la même paire ne committent pas toutes deux ; §18.3 à §18.6 sont lus avant d'écrire l'item, et l'écart éventuel est consigné |

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
| W15.a `[R]` **fait** | `packages/coordination` : la version canonique immuable — hash de contenu, hash de version, parent — et les sept opérations structurelles comme IR déclaratif fermé | le hash de contenu ne dépend que du contenu et se reproduit octet pour octet sur une forme canonique figée en fixture ; appliquer une opération puis son défaire rend le **même contenu** et une **autre version**, parce que l'histoire ne se défait pas ; une opération dont le défaire perdrait de l'information se déclare compensable et aucune fonction ne rend d'opération qui prétende la défaire ; les quatre opérations attributaires sont absentes, et un test le tient par l'absence |
| W15.b `[R]` **fait** | le diff comme objet de première classe, calculé une fois | un diff se rejoue sur sa base et rend exactement le contenu visé, et deux rejeux du même diff sur la même base rendent la **même version**, identité comprise ; rejoué sur une autre base il est refusé et le refus dit s'il faut rebaser ; le diff d'une version vers elle-même est **vide**, jamais absent |
| W15.c `[R]` **fait** | les régions mutables bornées de GRAFT — `allowed_ops`, `risk_ceiling`, `max_nodes_delta`, `max_edges_delta`, `approval_mode`, `require_shadow` — acceptation locale et veto de cohérence globale | une opération hors de la région déclarée ou hors de `allowed_ops` est refusée en nommant laquelle des bornes mord — quatre interdisent, les deux autres (`approval_mode`, `require_shadow`) **obligent** ; un lot accepté localement mais qui casse un invariant global **par un chemin passant hors de la région** est vetoé, et le veto nomme l'invariant et les agents pris dedans ; l'acceptation locale seule ne commit jamais, et rien dans son type ne le permet |
| W15.d `[R]` **fait** | contestabilité d'une décision de coordination : famille d'objection parallèle, domaines disjoints | une décision de coordination offre ses cibles — déclencheur, politique, périmètre, décision — ; **aucune fonction ne convertit** une objection de coordination en `ObjectionTarget` ni l'inverse, et un test le tient par l'absence ; aucun trait générique ne factorise les deux familles, ce qui serait la conversion reconstruite |
| W15.e `[R]` **fait** | `visibility`, **deuxième** membre de l'énumération des sortes (ADR 0016 décisions 4 et 10, amendées le 2026-08-18), dont le consommateur est la construction de `ContextView` | deux `ContextView` construites sous deux versions de coordination différentes diffèrent exactement des révisions que `visibility` retire ; aucune relation `visibility` n'élargit ce qu'une ACL refuse ; le constat de la clause de falsification est écrit au ledger, **dans un sens ou dans l'autre** |
| W15.f `[M]` **fait** | `SET_ROLE` comme opération **attributaire**, avec son lecteur exécutable dans `canterel` — **tranche 1 du mineur `lep/1.1`** (ADR 0017 §5.1) | l'overlay additif du worker lit le rôle de l'instance et un test l'exerce de bout en bout ; sans ce lecteur, l'opération reste hors de l'énumération ; le rôle ne prend jamais le pas sur l'invariant 11 — une revue `independent` va au `reviewer` quel que soit le rôle demandé ; et un document `1.0` reçu par un consommateur `1.1` laisse le rôle **absent**, jamais rempli par un défaut |

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

**W15.f était bloqué, et bloqué correctement ; l'ADR 0017 le débloque.** Le lecteur du rôle est
`selectOverlay` dans le worker, qui ne connaît d'une mission que ce que la `MissionEnvelope` lui livre
— et `mission-envelope.schema.json` porte `review_policy` et `required_capabilities`, **pas** de rôle
d'agent. Faire passer le rôle jusqu'au worker demandait donc un **mineur `lep/1.1`**, dont l'ADR 0016
disait qu'il « a son propre ADR » et que W13 « ne l'ouvre pas ». Écrire `SET_ROLE` avant ce mineur
aurait produit exactement la sémantique inerte que la décision 4 interdit : un attribut que le système
saurait versionner, différencier, approuver et afficher, et que rien n'honorerait.

Cet ADR est écrit — `docs/adr/0017` — et il ouvre le mineur **une fois pour les quatre ajouts qui
l'attendaient**, en livrant un champ par tranche et jamais avant son lecteur. W15.f est la **tranche
1**, choisie en premier parce que son lecteur est le seul qui existe déjà en entier.

**Les quatre tranches et où elles tombent :**

| Tranche | Ajout | Item | Lecteur |
| --- | --- | --- | --- |
| 1 | `role` dans la `MissionEnvelope` | **W15.f** | `selectOverlay`, existe et est testé |
| 2 | les six codes de refus d'admission sur le fil | **W19.a** (ci-dessous) | `apps/locus-execd/src/admission.rs`, existe |
| 3 | la permission de fonctionnement hors ligne | **W19.b** (ci-dessous) | à écrire dans son sprint |
| 4 | visibilité facultative des sous-agents | **W16.d** | à écrire, et à border sur l'invariant 11 |

L'ADR 0017 porte une clause de falsification sur le coût d'un mineur : la tranche 2 ajoute un
**document** là où la tranche 1 ajoute une **propriété**, et le constat s'écrit au ledger dans un sens
ou dans l'autre.

## W16 — Reconfiguration vivante et scheduler dynamique — **niveau 4**

Le scheduler doit savoir spawn, suspend, drain, kill, replace, split, merge, connect, disconnect,
rerouter l'état, rejouer, migrer le contexte, et livrer les messages **en connaissance de la version**.
Barrières par invariant menacé plutôt que par lieu ; quiescence locale d'un nœud plutôt que drain
global. Epochs, messages tardifs et transfert d'état : **débloqués par l'ADR 0019**, qui tranche que
la messagerie inter-agents est un usage du journal et non un transport parallèle — un message est un
événement, un epoch est une `Version` de configuration. Visibilité institutionnelle facultative des sous-agents
internes du harnais — le cas de W16 justifiant un mineur LEP, désormais tranché par l'ADR 0017.

Plan de simulation : rejeu déterministe, substitut d'environnement enregistré, ombre en sandbox réelle,
canari facultatif. Un objet simulé n'existe pas comme type dans le domaine épistémique.

Attend W15, W4.e et W4.g. **Les trois sont satisfaits** : W4.e et W4.g sont livrés, et W15 est clos
à W15.e — W15.f attend un mineur LEP qui a son propre ADR.

Décomposée ici. Un item de la prose n'entre pas, et pour la même raison que les opérations
attributaires de W15.a : la **visibilité institutionnelle des sous-agents internes du harnais** est
« le cas de W16 justifiant un mineur LEP, avec son ADR », et l'ADR 0017 l'a écrit — mais le blocage
a changé de nature, et ce qui manque désormais est un consommateur.

**Epochs et messages tardifs entraient dans la même phrase et n'y sont plus.** Ils n'avaient « un
problème réel à résoudre qu'une fois une messagerie inter-agents existante », et le propriétaire du
produit a énoncé que le besoin l'est. L'ADR 0019 tranche la seule question que le gel laissait
ouverte — **quelle forme** cette messagerie prend — et refuse celle qui coûtait le gel : un courtier
dédié serait un second stockage durable du même fait, donc une seconde vérité, la conclusion même de
`W20.f`. Un message est un événement du journal ; l'item a donc un test de sortie qui ne demande
aucune fonctionnalité inventée pour le justifier.

| # | Commit | Test de sortie |
|---|---|---|
| W16.a `[R]` **fait** | les transitions de cycle de vie du scheduler — `spawn`, `suspend`, `drain`, `kill`, `replace`, `connect`, `disconnect` — comme machine à états explicite, et la **quiescence locale** d'un nœud | une transition interdite est refusée en nommant l'état de départ et celui visé ; un nœud est drainé **sans** que rien d'autre soit arrêté, et la quiescence se constate au lieu de s'attendre ; `kill` sur un nœud quiescent et sur un nœud actif ne disent pas la même chose |
| W16.b `[R]` **fait** | les **barrières par invariant menacé** plutôt que par lieu | une reconfiguration ne barre que les nœuds dont elle menace un invariant, et le refus nomme l'invariant, pas le lieu ; deux reconfigurations qui ne menacent pas le même invariant ne se bloquent pas l'une l'autre ; une barrière posée sans invariant menacé est refusée |
| W16.c `[R]` **fait** | le plan de simulation : rejeu déterministe, substitut d'environnement enregistré, ombre en sandbox réelle, canari facultatif | deux rejeux de la même trace rendent le même résultat ; un substitut d'environnement qui n'a pas la réponse le **dit** au lieu d'en inventer une ; un objet simulé n'existe **pas** comme type dans le domaine épistémique, et un test le tient par l'absence |
| W16.d `[M]` **bloqué** | visibilité institutionnelle facultative des sous-agents internes du harnais — **tranche 4 du mineur `lep/1.1`** (ADR 0017 §5.4) | `attend:W26.b` — **le blocage a changé de nature une seconde fois, et cette fois il se périme tout seul.** Il était une décision, puis un consommateur absent ; l'ADR 0027 décision 7 tranche la décision — l'institution voit qu'un sous-agent a existé, sa classe de cognition, son coût et son résultat, et ne voit son contexte que par les décisions 1 à 5 —, et `W26.b` fournit le lecteur qui manquait. Ce que la ligne posait comme question, « voir qu'un sous-agent existe et voir son contexte sont deux choses », est devenu la réponse. `attend:externe` aurait donc continué de dire « rien de ce plan ne le débloquera » sur un item qu'un item de ce plan débloque |
| W16.e `[R]` **fait** | epochs, messages tardifs et transfert d'état — la messagerie comme **usage du journal** (ADR 0019) | émettre un message rend un événement du namespace `message` et **rien d'autre** : aucun second stockage durable, tenu par l'absence comme `W20.b` tient les écritures ; un message reçu sous un epoch antérieur à celui du destinataire est rapporté `Late` **en nommant les deux epochs**, jamais appliqué ni jeté en silence, et un epoch inconnu rend `Unknown` et non `Late` — deviner et ignorer sont deux fautes distinctes, pas deux nuances de la même ; le passage de témoin d'un `drain` transmet ce que le nœud sortant tenait et **refuse** un contexte de mission, la règle « nouvel attempt, nouvelle vue, nouveau hash » de `docs/13` étant tenue par un test d'absence |

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
| W17.a `[R]` **fait** | `packages/memory` : les sept niveaux de §16.1 comme liste close, et la distinction canonique/projection de la dernière phrase de la section | les sept se lisent sous leur nom ; ce qui est **canonique** — graphe, événements, artefacts — ne se déclare jamais régénérable, et ce qui est projection le déclare toujours ; une mémoire dont le niveau n'est pas nommé n'existe pas |
| W17.b `[R]` **fait** | le retrieval hybride de §16.3 : les dix signaux, le ranking dont les facteurs sont **exposés**, et les ACL que les embeddings ne contournent pas | un résultat porte la contribution de **chacun** des signaux qui l'ont produit, et un ranking sans facteurs exposés est refusé ; un élément qu'une ACL refuse reste absent quel que soit son score vectoriel, et le test l'exerce avec un score maximal |
| W17.c `[R]` **fait** | deux retrievals séparés, épistémique et organisationnel, **sans conversion** | les deux répondent à des questions différentes sur des types disjoints ; **aucune conversion n'est écrivable**, parce que le préfixe fait partie de l'identité (`packages/protocol`) et qu'une conversion devrait fabriquer une identité qu'elle n'a pas ; aucun trait générique ne les factorise |
| W17.d `[R]` **fait** | déduplication non automatique (§16.4) et compaction (§16.5) | un duplicata exact par hash est détecté ; un candidat **sémantique** n'est jamais fusionné automatiquement, et sa résolution porte confiance et provenance ; une fusion se défait par une nouvelle décision ; une compaction signale ce qu'elle a omis et ne transforme jamais un objet non validé en connaissance établie |
| W17.e `[R]` **fait** | les quatre vues du cockpit et la sélection synchronisée par `Id<Agent>` ; le canvas produit une **commande**, jamais une écriture | une sélection dans une vue désigne le même agent dans les trois autres ; un geste de canvas rend une commande que rien n'applique sur place, et aucun chemin de type ne permet à une vue d'écrire |
| W17.f `[M]` **fait** | `/branches/:id/diff`, la preview, l'ombre, l'approbation, le rollback et la navigation dans le temps — **débloqué par `W20`**, dont la façade existe désormais | les six sont joignables depuis `apps/locusd`, chacun sous le nom que `SPEC_V1.md` lui donne ; un diff se lit **entre deux révisions nommées** et non « depuis le début », parce qu'une comparaison sans borne n'est pas une comparaison ; l'ombre et la preview ne **produisent aucun événement**, et un test le tient par le journal, faute de quoi prévisualiser deviendrait agir ; le rollback est une **commande**, pas une suppression, et laisse le journal plus long qu'avant |
| W17.g `[R]` **fait** | la **liaison HTTP** de l'histoire de branche — `GET /branches/{id}/history` | l'histoire se sert sur une réponse réelle et rend la **révision de stream**, pas la position globale ; un cursor rendu par cette route et représenté à `/timeline` est **refusé** en nommant les deux collections ; un cursor illisible rend `400` et non `500`, route par route et non par héritage ; l'histoire demandée est celle de la branche du chemin, une autre branche rendant une page **vide** et jamais celle du voisin ; les trois autres lectures ne sont **pas** câblées, et un test le tient par l'absence |
| W17.h `[R]` **fait** | **ADR 0020** — le condensat de contenu, et `ContentHash::of` | un vecteur connu de SHA-256 est rendu — `sha256("")` et `sha256("abc")`, publiés ailleurs et vérifiables — parce qu'aucune propriété ne distingue SHA-256 d'une fonction qui lui ressemble : le `Fnv` jouet des fixtures satisfait « déterministe », « injectif en pratique » et « soixante-quatre hexadécimaux » ; ce qui est calculé se relit par `parse` ; un condensat d'un algorithme non calculé rend **`None`** et non `false` — ne pas savoir vérifier n'est pas un échec de vérification ; `of` ne canonicalise rien, et un test le tient sur quatre paires d'octets voisines |
| W17.i `[R]` **fait** | le **commit d'une version de coordination écrit un fait** — le producteur qui manquait | un commit passe par un `Decide` et rend un événement portant l'opération et la `VersionId` produite, jamais un magasin : ADR 0016 décision 5 dit « aucun compteur, aucun magasin, aucun bus » ; la révision de version **est** le `stream_revision` que le journal attribue, et un commit sur une base périmée est refusé par `Expected::Exact` avant d'écrire ; rejouer le stream depuis la racine rend une `Version` de **même `content_hash`** que celle commitée, tenu par égalité stricte |
| W17.j `[R]` **fait** | le **résolveur de versions** et la liaison HTTP du diff et de la preview | une `VersionId` rendue par `/branches/{id}/history` se relit en `Version` par rejeu, et la version reconstruite a le même `content_hash` que l'originale — reconstruire un contenu qu'on ne peut pas reconnaître ne prouve rien ; une `VersionId` inconnue est **refusée** en `404`, jamais résolue en une racine plausible ; `GET /branches/{id}/diff?from=&to=` rend les opérations **et leur nature**, la propriété que deux mutants de `W17.f` avaient traversée ; le résolveur ne détient aucun état que le journal n'ait écrit, tenu par l'absence |
| W17.k `[R]` **fait** | `memory::Genre` — les dix genres de l'ADR 0022, orthogonaux à `Level` | les dix se lisent sous leur nom et un onzième n'existe pas ; le type s'appelle **`Genre`** et non `Kind`, que `compaction` occupe déjà — un test importe les deux sans renommage ; genre **et** niveau sont obligatoires, et une `Entry` sans genre n'est pas constructible ; **aucune conversion entre genres n'est écrivable**, tenu par l'absence, et aucun trait générique ne les factorise ; une promotion change le niveau et laisse le genre, tenu sur les dix ; un genre qui **contredit** un type résolu par le port de cohérence est refusé **en nommant les deux**, tandis qu'une clé qu'aucun port ne résout est **acceptée** — l'ignorance n'est pas un démenti ; le couple `(Genre::Formal, Signal::Vector)` est refusé **à la construction du `Candidate`**, dont les champs cessent d'être publics, et `Ranking::of` est inchangé ; un objet `MetaMemory` n'entre dans aucune `Support` ni prémisse d'`Inference` |
| W17.l `[R]` **fait** | `Intent`, `Plan`, `Escalation` — `retrieve` prend un plan | les six intentions se lisent sous leur nom ; trois intentions distinctes produisent trois **ordres de canaux** différents sur la même question, et le test compare les ordres et non les résultats ; un plan sans critère d'arrêt n'est pas constructible ; le plan porte **l'identité de la fonction de classement**, et un plan qui n'en déclare pas n'est pas constructible — sans quoi le reçu de `W17.n` promettrait un rejeu sur une entrée qu'il ne connaît pas ; une escalade est **enregistrée**, et un résultat post-escalade se distingue d'un résultat direct par son type et non par une convention ; un `Plan::compatible(budget)` reproduit **exactement** le comportement de `retrieve` d'avant cet item, réserve de négatifs à zéro comprise, tenu par égalité sur un corpus de fixtures |
| W17.m `[R]` **fait** | `Channel` — les quatre routes nouvelles, `Signal` inchangé | les dix `Signal` de §16.3 sont **inchangés**, tenu par un test sur leur nombre et leurs noms ; le canal `Structural` rend les inférences de **même forme de prémisses** via un port `RevisionId → ObjectType` fourni par l'appelant — `Graph` ne détient aucun type —, exercé sur trois hyperarêtes dont deux partagent la structure et non le contenu, et la troisième le contenu et non la structure ; le canal `Regional` rend des identités de région et **jamais** d'octets, tenu par l'absence de type ; `Community` n'est jamais sélectionné hors intention `Global`, et un test l'exerce sur les cinq autres intentions ; sous un plan **portant une réserve**, un budget saturé n'exclut jamais un objet `Negative` tant qu'un objet d'un autre genre reste inclus — et sous un plan sans réserve le comportement d'avant est inchangé, les deux moitiés étant testées |
| W17.n `[R]` **fait** | `RetrievalReceipt` — le reçu comme fait, et la **jonction** `Results → ContextView` qui n'existait pas | `packages/review` dépend désormais de `packages/memory`, et un chemin mène de `Results` à `ContextView` là où les deux crates s'ignoraient ; le reçu se rejoue et rend la **même** `ContextView`, condensat compris, tenu par égalité stricte sur `ContentHash` ; une exclusion sans motif n'est pas constructible ; une contestation vise le **reçu** et non la `ContextView`, et aucun chemin de type ne permet de contester une vue immuable ; le reçu ne détient rien que le journal n'ait écrit, tenu par reconstruction depuis zéro ; sa forme canonique refuse les caractères de contrôle, comme les quatre formes durcies par `W17.h` ; la couverture en contre-preuve est **rendue même quand elle vaut zéro**, `None` et `Some(0.0)` ne se confondant pas |

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

Décomposée ici. Deux moitiés de la prose sont **déjà livrées** et ne sont pas redemandées : les
indicateurs de §13.2 vivent dans `portfolio::Indicators` depuis W14 — dix des quinze, les cinq autres
appartenant à la qualité-diversité de §13.3, et le module le dit —, et l'anti-gaming de §13.6 dans
`portfolio::gaming`, dont l'ADR 0016 décision 8 fait la condition du mode `bounded`. Les deux
conditions de `bounded` — W14 et W16 — sont donc satisfaites, et le mode s'ouvre ici.

L'admission de capacité se coupe en deux, et l'ADR 0016 décision 8 dit où : « Locusolus possède déjà
le blueprint, l'artefact, l'attestation et le refus nommant toutes ses conditions. Ce qui manque est
la proposition, la politique et l'approbation : **du travail de gouvernance**. » Ce travail-là ne
demande pas d'hôte. Ce qui attend `S3`/`S4` attesté est l'admission **exercée de bout en bout contre
un hôte réel**, et elle attend pour exactement la raison de W5.f.

| # | Commit | Test de sortie |
|---|---|---|
| W18.a `[R]` **fait** | `packages/adaptation` : les onze déclencheurs de §14.5 comme liste close, la proposition de spawn avec ses neuf champs, et les quatre réponses du moteur de politique | les onze se lisent sous leur nom et un douzième n'existe pas ; une proposition à qui il manque un champ n'est pas construite, et le refus nomme le champ ; **aucun chemin** ne mène d'un déclencheur à une flotte sans passer par la réponse du moteur, et un test le tient par l'absence de constructeur |
| W18.b `[R]` **fait** | boucle **rapide** sur la capacité — routage de modèle, choix d'outil, sélection de skill, retry, routes éphémères — et boucle **lente** sur la structure | une adaptation rapide ne produit aucune opération de coordination et aucun chemin de type ne le permet ; une adaptation lente est une `Proposal` de W13 et suit son chemin entier ; une route éphémère **expire**, et deux adaptations rapides ne s'accumulent jamais en une structure que personne n'a approuvée |
| W18.c `[R]` **fait** | `bounded` et `operator`, les deux barreaux manquants de l'ADR 0016 décision 8, avec la classe de risque **dérivée** des invariants menacés | la classe de risque ne se déclare pas — elle se calcule de `region::threatens`, et un proposeur n'a nulle part où l'écrire ; en `bounded` une opération dont la classe dépasse le plafond est refusée **en nommant l'invariant**, pas le plafond ; `operator` n'est jamais tenu par un agent, et `Author::Agent` n'a pas de chemin vers lui |
| W18.d `[R]` **fait** | l'admission de capacité comme gouvernance : proposition, politique, approbation, et le blueprint publié comme **seule** entrée | une capacité nouvelle n'entre que par un `Published` de W5.b, et aucun constructeur ne la fabrique depuis autre chose ; le refus nomme laquelle des conditions manque plutôt que de dire « non » ; du code injecté n'est pas une valeur exprimable, et c'est un test d'absence qui le dit |
| W18.e `[R]` **fait** | la métrique d'acceptation : taux d'annulation **humaine** des adaptations agentiques | le taux ne compte que des annulations humaines, et une annulation par le système ne le fait pas monter ; une adaptation que personne n'a regardée est déclarée **hors mesure**, jamais comptée comme acceptée — le silence n'est pas un accord ; une adaptation d'auteur humain n'entre pas dans la mesure |
| W18.f `[M]` **reporté** | l'admission exercée de bout en bout contre une sandbox `S3`/`S4` réellement attestée | `attend:externe` — un hôte capable, et `W5.f` a rendu la condition précise : un système de fichiers qui porte les quotas, une isolation réseau, une micro-VM, et de quoi **attester**. Voir « Ce qui est reporté » |
| W18.g `[R]` **fait** | le producteur d'**observations** — le capteur qui manquait entre la mémoire et l'organisation | une observation se recalcule à l'identique depuis le journal, et deux calculs sur le même préfixe rendent la même valeur ; elle **cite** les révisions dont elle est tirée, et une observation sans citation n'est pas constructible ; **aucun chemin de type ne mène d'une `Observation` à un `Trigger`** sans passer par une politique, et c'est un test d'absence qui le dit ; un seuil n'a **nulle part où s'écrire** dans un capteur, tenu par l'absence de champ ; le type s'appelle `Observation` et non `Signal`, que `memory::retrieval` occupe pour autre chose ; les six sources sont exercées chacune sur une fixture, et une source muette produit une observation **absente** et non une valeur nulle |
| W18.h `[R]` **fait** | le raisonneur d'ontologie comme **première capacité réellement admise** | il n'entre que par un `Published` de W5.b, et aucun constructeur ne le fabrique depuis autre chose ; sa sortie entre comme claim **proposé** avec sa provenance — raisonneur, version d'ontologie, profil — et jamais comme fait, tenu par l'absence de chemin vers un `Inference` validé ; le verdict a **trois** valeurs et `Undetermined` refuse la confiance, exercé sur une entrée que le raisonneur ne sait pas trancher ; un moteur de règles ne peut alimenter aucun chemin de décision, tenu par l'absence ; la résolution se fait **par identité**, et un test tente de masquer une capacité par un homonyme et échoue ; les trois réserves de l'ADR 0023 sont inscrites au ledger avant l'admission, pas après |

W18.a avant W18.b : la boucle lente propose un spawn, donc la proposition d'abord. W18.c après W18.b :
`bounded` est un mode de la boucle lente. W18.d ne dépend d'aucun des trois. W18.e en dernier : elle
mesure des adaptations, et il en faut.

## W19 — Le mineur `lep/1.1` — **tranches 2 et 3**

Ouvert par l'**ADR 0017**, qui décide le numéro une fois pour quatre ajouts et livre un champ par
tranche, jamais avant son lecteur. Les tranches 1 et 4 vivent chez leurs items d'origine — W15.f et
W16.d ; les deux qui n'avaient pas d'item l'ont ici.

Rappel de ce qu'un mineur n'a pas le droit de faire (ADR 0017 décision 4), parce que c'est l'interdit
3 qui contraint la forme des deux items ci-dessous : **un mineur ajoute des champs, jamais des
valeurs**. `packages/lep/src/generated.rs` émet des `enum` Rust fermés sans variante fourre-tout, donc
un membre nouveau sur une énumération ancienne fait échouer la désérialisation chez tout consommateur
`1.0`, en silence pour l'émetteur.

Et aucun répertoire `schemas/lep/1.1/` : la ligne `1.x` est ouverte depuis W0.5 — motif
`^lep/1\.[0-9]+$` sur `protocol_version`, aucun `additionalProperties: false` sur les douze fichiers.
Les ajouts portent `x-since: "1.1"` là où ils tombent.

| # | Commit | Test de sortie |
| --- | --- | --- |
| W19.a `[M]` **fait** | les six motifs de refus d'admission sur le fil, comme **document** — `LevelUnavailable`, `CapacityExceeded`, `AcceleratorUnavailable`, `NetworkModeUnsupported`, `LevelNotAttested`, `AcceleratorOutsideSandbox` | un refus voyage avec **tous** ses motifs, jamais le premier seul — un fil qui n'en transmettrait qu'un ferait corriger une condition pour retomber sur la suivante ; `LevelNotAttested` et `LevelUnavailable` restent deux refus distincts sur le fil comme en mémoire, et un test le tient par égalité stricte : « l'hôte ne sait pas faire » et « l'hôte l'annonce sans l'avoir prouvé » envoient chercher deux choses différentes ; le document est nouveau et **aucune énumération existante ne gagne un membre** |
| W19.b `[M]` **fait** | la permission de fonctionnement hors ligne, activable et désactivable — `SPEC_V1.md` §1.2, dernier invariant | la permission est un champ **distinct** de `sandbox.network_mode`, et aucune fonction ne dérive l'une de l'autre : `deny` contraint le worker, la permission le dispense d'échouer quand le réseau manque, et les confondre ferait d'un confinement une autorisation ; un document `1.0` laisse la permission **absente**, jamais « accordée par défaut parce que le pair est ancien » |

**W19.a avant W19.b**, et l'ordre n'est pas arbitraire : W19.a porte la **clause de falsification** de
l'ADR 0017. Elle ajoute un document entier là où la tranche 1 n'ajoutait qu'une propriété, et l'ADR
affirme que le coût d'un mineur est fixe — que c'est le péage qui coûte, pas le champ. Le constat
s'écrit au ledger **dans un sens ou dans l'autre** ; si le document coûte substantiellement plus par
nature, la décision de grouper quatre ajouts hétérogènes sous un numéro est rouverte pour les mineurs
suivants.

W15.f (tranche 1) avant les deux : elle porte les deux tests qui **définissent** ce que « mineur »
veut dire ici — un document `1.1` accepté par un consommateur `1.0`, et un champ nouveau laissé
**absent** plutôt que rempli par un défaut lorsqu'un document `1.0` arrive chez un consommateur `1.1`.
Ces tests ont besoin d'un champ pour être écrits, et un seul suffit.

## W20 — `locusd` — le daemon, et la porte qui manque au bâtiment

Tout ce qui précède est une **bibliothèque**. Vingt-quatre crates savent décrire ce qui est cru,
qui travaille, ce qui est réservé, ce qui est confiné — et rien n'expose quoi que ce soit à un
client. `apps/` porte `emacs`, `locus-execd` et `web` ; le daemon n'existe pas. C'est le plus gros
morceau restant de la V1, et il bloque `W17.f`.

**Deux constats avant de décomposer.**

Le premier est mesurable : la **règle 4 de `boundaries.json`** — « `apps/locusd` n'importe aucun SDK
de runtime de containers » — est aujourd'hui « vérifiée sur **0 fichier(s)** ». Elle garde le vide, et
l'outil le dit à chaque passage plutôt que de l'afficher comme un succès. La première livraison qui
crée `apps/locusd` la rend non vide, et c'est un jalon plus honnête qu'un compte de lignes.

Le second est une dépendance : `Cargo.toml` ne déclare que `serde` et `serde_json` comme dépendances
d'espace de travail. Faire entrer un runtime asynchrone et un cadre HTTP est le plus gros choix de
dépendance depuis l'ADR 0011, et il a **son propre ADR** — même forme que le langage de `locusd` et
que le backend de workflow.

**Mais rien de tout cela ne bloque le début.** `W20.a` et `W20.b` sont du domaine pur, sans transport,
et c'est exactement l'ordre que `CLAUDE.md` impose : « construire domain/protocol/event-store d'abord,
avec des ports purs ». Le daemon commence donc **avant** l'ADR du transport, et pas après.

| #                | Commit                                                                                                                                                                        | Test de sortie                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ---------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| W20.a `[R]` **fait** | le `CommandEnvelope` de §22.2 et les huit familles d'erreurs typées de §22.5, **sans transport** | une commande mutante sans `expected_revision` n'est pas constructible, et le refus nomme le champ ; les huit familles — validation, authorization, conflict, unavailable, budget, policy, security, internal — sont une liste close lue sous leur nom, et une neuvième n'existe pas ; un conflit rend l'**état courant** et un code structuré, jamais un entier nu, parce qu'un client qui doit relire pour retenter a besoin de ce qu'il relit ; aucune variante ne permet à un refus de ressembler à un succès |
| W20.b `[R]` **fait** | le **handler transactionnel comme port**, et la règle « toute mutation passe par un command handler transactionnel » rendue opposable | aucun chemin de type ne permet d'écrire dans l'event store sans passer par un handler, et un test le tient par l'absence ; un handler qui échoue ne laisse **aucun** événement écrit ; une resoumission de la même clé d'idempotence rend le même résultat sans second effet, et deux portées différentes portant la même clé ne se confondent pas ; un lot n'est atomique que s'il se déclare tel |
| W20.c `[M]` **fait** | **ADR du transport** : runtime asynchrone et cadre HTTP | l'ADR décide, énonce ses conditions comme l'ADR 0011 énonce les siennes, et porte un plan de rollback ; `Cargo.toml` ne gagne sa première dépendance hors `serde` **qu'après** lui, et le diff qui l'ajoute cite l'ADR |
| W20.d `[R]` **fait** | `apps/locusd` : le composition root, sans surface HTTP | le binaire démarre et câble event-store, projections et moteur de politique ; **la règle 4 de `boundaries.json` cesse d'être vide** — elle passe de « 0 fichier(s) » à un compte réel, et un import de SDK de runtime la fait échouer, vérifié en rouge d'abord |
| W20.e `[R]` **fait** | les queries de §22.4 et les cursors de §22.6 | chaque collection rend un cursor **opaque** et stable dans une fenêtre cohérente ; une reprise depuis une séquence connue rend exactement la suite, sans trou ni doublon ; un cursor d'une autre collection est **refusé** au lieu d'être interprété, parce qu'un cursor mal interprété saute des pages en silence |
| W20.f `[R]` **fait** | les événements clients de §22.1 — WebSocket/SSE avec cursor | un client qui se reconnecte reprend depuis sa séquence et ne perd rien ; un client lent ne fait perdre aucun événement au journal, et ce qu'il n'a pas pu recevoir se relit — la coalescence de W2.12 vaut pour le fil client comme pour le fil worker |
| W20.g `[R]` **fait** | la **liaison HTTP** du fil et des queries — les premières dépendances de transport, autorisées par l'ADR 0018 | `Cargo.toml` gagne `tokio` aux features nommées et `axum`, et `dependencies.json` les porte avec leur ADR ; une route rend `text/event-stream` et le cadre porte le cursor en `id:`, vérifié sur une réponse réelle ; les queries de §22.4 se servent par HTTP et un cursor étranger rend un refus **typé**, pas un 500 ; `tokio/full` fait échouer `check:deps`, vérifié en rouge d'abord |

| W20.h `[R]` **fait** | la **sérialisation des écritures** — un écrivain par agrégat | `main.rs` de `locusd` nomme lui-même le blocage : « `Transaction::submit` prend `&mut self`, et la couche HTTP ne tient qu'un `&Runtime`. Sérialiser les écritures — verrou, file, acteur — est une décision qui mérite son item. » C'est une **décision** avant d'être du code, et elle a son ADR. Test de sortie : deux commandes concurrentes sur le même agrégat sérialisent ; sur deux agrégats distincts, elles ne s'attendent pas, et un test le **mesure** plutôt que de le décrire ; le refus de concurrence rend de quoi savoir s'il faut relire et retenter — l'état courant, comme `W20.a` l'exige déjà de tout conflit —, jamais « conflit » seul ; §22.3 devient servable, ce qui est exactement ce que `W20.g` a laissé de côté |
| W20.l `[R]` **fait** | les **projections avancent** pendant que le daemon sert | découvert en écrivant `W20.k`, et ce n'est pas un défaut qu'il introduit mais un défaut qu'il **rend visible** : `Runtime::catch_up` prend `&mut self`, la liaison HTTP ne tient qu'un `&Runtime`, et tant que la surface était en lecture seule rien n'était écrit pendant que le daemon servait — donc rien ne pouvait devenir périmé. Les trois routes de §15.2 écrivent, et les quatre projections de §9.5 ne les voient jamais : `/workers` reste vide alors qu'un worker a réclamé et rendu, ce qu'un test de `apps/locusd/tests/lep.rs` atteste **de façon à rougir** le jour où cet item est livré. C'est le frère de la décision que l'ADR 0029 a prise pour la sérialisation des écritures, et il la réclamera probablement aussi. Test de sortie : une écriture servie par HTTP est visible dans les quatre projections **sans redémarrage**, et un test le lit par `/workers` après une réclamation ; une projection en quarantaine ne bloque toujours pas l'écriture canonique — la promesse de `W1.d` tient, et un test le vérifie en fauteuse ; deux lectures concurrentes rendent le même état, une query ne faisant **pas** avancer les projections par effet de bord, ce que `readiness()` refuse déjà et qui doit le rester |
| W20.m `[R]` **fait** | le journal durable **câblé** : `locusd` redémarre et tout est encore là | `W20.i` a livré le driver et **ne l'a câblé nulle part** — c'était son périmètre, et `Runtime::in_memory()` reste l'assemblage du profil `personal-local`. Tant que rien ne le choisit, « `locusd` redémarre et tout est encore là » n'est vérifié par personne, et c'est la clause centrale de `W12.d`. Test de sortie : un profil de déploiement de `docs/05` assemble `Runtime` sur `PostgresEventStore` **sans toucher à `composition.rs`** — c'est la seule chose que le paramètre de type `S` est là pour garantir, et ce test est le premier à l'éprouver ; un daemon écrit, s'arrête, redémarre, et les queries de §22.4 rendent **exactement** ce qu'elles rendaient avant l'arrêt, projections comprises . **La troisième clause a été déplacée en `W20.p`**, et le motif est écrit plutôt que tu : appeler le driver bloquant hors du fil du runtime asynchrone change la convention d'appel de toute la couche HTTP, c'est une propriété de **latence sous charge** et non de correction, et les deux clauses ci-dessus forment une capacité complète sans elle. Découper vaut mieux que livrer à moitié en le taisant |
| W20.n `[R]` **fait** | `locusd` sert l'**enrôlement** de §7.2, et le registre cesse d'être rempli à la main | `W2.4` a livré la moitié cliente — `identity.ts`, la signature, le nonce lié au endpoint — et personne ne l'écoute : `W20.k` a dû livrer un `WorkerRegistry` que seul un test remplit. Un worker réel ne peut donc pas obtenir de créance. Test de sortie : `POST /lep/v1/enroll` vérifie la signature de `W2.4` **contre la clé publique annoncée**, et un nonce rejoué est refusé — sans quoi une demande capturée se resservirait ; la créance émise lie le worker à un workspace et à un principal, ce que `W20.k` exige déjà de son registre ; une révocation est un **fait du journal**, jamais une ligne supprimée (invariant 12) ; un worker enrôlé puis révoqué reçoit `403` sur les trois chemins de §15.2, et un test le vérifie depuis le client de `W2.21` sans le modifier |
| W20.o `[R]` **fait** | une **mission naît d'une question**, et la file cesse d'être remplie par un test | `W20.k` a livré `MissionQueue` comme port avec son implémentation de référence — une capacité finie, au sens de l'ADR 0022 décision 0. Ce qui manque est ce qui la remplit : aucune commande de §22.3 ne crée de tâche, donc rien ne produit de `MissionEnvelope`, donc « une question produit une mission » n'a pas de sujet. Test de sortie : une commande de §22.3 crée une tâche `proposed`, et le fait atteint le journal par le handler transactionnel comme tout le reste ; la `MissionEnvelope` produite porte **tous les champs que son schéma de `W0.5` exige**, la liste `required` étant lue du fichier de schéma au moment du test et non recopiée — **clause rétrécie, et le motif est écrit** : elle disait « valide contre son schéma, vérifié par le registre de schémas », or ce registre est en TypeScript (`ajv`) et valide les fixtures du corpus, pas une valeur produite en Rust ; un validateur Draft 7 côté Rust coûterait 88 paquets — plus que le driver PostgreSQL entier — pour une propriété de test. Ce qui n'est donc **pas** vérifié est nommé dans le fichier de test : contraintes de valeur, formats, `additionalProperties` ; une tâche `queued` devient claimable, et une tâche annulée ne l'est plus — les deux lus de `TaskState` de §7.1, jamais d'une seconde énumération ; le placement reste celui de `W4.g` chez `locus-execd`, et un test d'absence refuse que la création choisisse un hôte |
| W20.q `[R]` **fait** | la réclamation **demande le placement** à `locus-execd`, au lieu de servir la première offre | vérifié au code en marquant `W20.o` : `MemoryQueue::take` reçoit le `worker_id` et **l'ignore délibérément** — sa propre documentation le dit —, et rien dans `apps/locusd` n'appelle `place`. La clause de `W12.d` « un worker est placé sur ce qu'il a **prouvé** » n'a donc aucun sujet : la file sert la première offre à qui demande, quel que soit son manifeste. `W4.g` a livré `place` chez `locus-execd` et `W4.h` le tube pour lui parler ; ce qui manque est l'appel. Test de sortie : la réclamation passe le `CapabilityManifest` du worker au broker et n'offre que ce que `place` admet — un worker macOS ne reçoit pas une mission qui exige `S3`, et c'est la **paire de refus de `W0.7`** qui l'éprouve, pas un manifeste bricolé ; `locusd` ne décide toujours d'aucun hôte, tenu par un test d'absence comme `W20.o` le fait déjà ; un broker injoignable rend `unavailable` et **non** `204` — « je n'ai pas pu demander » n'est pas « rien pour toi », et l'ADR 0028 décision 4 sépare déjà les deux |
| W20.r `[R]` **fait** | un `EpistemicCommit` **entre au journal** et l'institution l'intègre | vérifié au code en marquant `W20.o` : rien dans `apps/locusd` ne connaît `EpistemicCommit` — seule la traduction de type d'événement le nomme. `W2.15` plafonne le worker à `staged` (invariant 3, §2.3), `packages/validation` porte la propagation, et **personne ne fait le pas suivant**. La clause de `W12.d` « un `EpistemicCommit` est mis en scène puis intégré » n'a donc pas de sujet côté serveur. Test de sortie : un commit reçu par §15.2 est écrit `staged` et **jamais au-delà** — un worker qui annoncerait `validated` est refusé en nommant le champ, ce que `packages/domain` sait déjà dire ; l'intégration est une commande **distincte**, sous une autorité distincte, et un test d'absence refuse qu'un chemin de worker l'atteigne ; les résultats négatifs et les conflits entrent au même titre que les autres — invariant 12, éprouvé par un commit qui contredit un commit déjà intégré |
| W20.p `[R]` **fait** | le driver bloquant sort du fil du runtime asynchrone | troisième clause de `W20.m`, déplacée plutôt que livrée à moitié. Avec un journal durable, les handlers de `W20.k` appellent un driver **bloquant** depuis un fil `tokio` : sous charge, un fil occupé à attendre la base ne sert personne. Ce n'est pas une faute de correction — le daemon répond juste — et l'ADR 0030 décision 1 nomme déjà `spawn_blocking` comme réponse. Ce qui manque est la **convention d'appel** de la couche HTTP, qui appartient à un item et non à un coin d'un autre. Test de sortie : un daemon dont le runtime n'a **qu'un** fil de travail sert une seconde requête pendant qu'une première attend la base — sans quoi la première la famine, et c'est ce que le test doit démontrer rouge avant d'être vert ; aucun handler n'appelle `Runtime` directement, tenu par une garde de source comme la règle 4 l'est pour les sockets ; le nombre de fils bloquants est **borné** et le dépassement rend le `unavailable` de §22.5 qui nomme la borne, comme `StreamLocks` le fait déjà pour les écritures |
| W20.s `[R]` **fait** | les commandes de §22.3 sur le fil : **proposer** et **mettre en file** une tâche | vérifié au code en tentant `W12.d` : `lep_propose` et `lep_queue` existent depuis `W20.o` et **aucune route ne les appelle** — `http.rs` en contient zéro occurrence. Conséquence exacte : « une question produit une mission » n'est déclenchable que depuis l'intérieur du processus, donc un test de bout en bout ne peut pas **commencer**. Test de sortie : deux chemins de §22.3 servis, sous une autorité d'administration et **non** sous une créance de worker — un worker qui pourrait se créer du travail choisirait le sien, et l'`Authority` de `W20.o` existe pour que ce soit inexprimable ; la mission déposée en file est celle que la proposition décrit, comparée champ pour champ et non par sa présence ; §7.1 refuse toujours la mise en file depuis un état qui n'y mène pas, et le refus le dit ; les deux commandes portent une clé d'idempotence, donc deux propositions identiques produisent **une** tâche |
| W20.t `[R]` | la chaîne d'artefacts atteint le daemon — §19.1 | vérifié au code en tentant `W12.d` : `apps/locusd/Cargo.toml` **ne dépend pas** de `locus-artifacts`. `W2.14` a livré la déclaration-avant-upload côté worker, `packages/artifacts` porte le manifeste et les hashes, et **rien côté serveur ne les reçoit** — la clause « les artefacts sont hashés » n'a donc aucun sujet institutionnel, exactement comme `EpistemicCommit` n'en avait pas avant `W20.r`. Test de sortie : un `ArtifactManifest` déclaré par un worker atteint le journal et son hash est **vérifié**, jamais cru — un manifeste dont le hash ne correspond pas est refusé en nommant le champ ; la déclaration précède l'upload, et un upload sans déclaration est refusé ; l'invariant 4 est tenu — le fait porte la provenance, et un artefact sans elle n'entre pas |
| W20.u `[R]` | le **graphe épistémique** servi : conclusion, prémisses, expérience, artefacts, objections, coût | vérifié au code en tentant `W12.d` : `served()` liste sept routes et **aucune n'est le graphe** ; `apps/locusd/Cargo.toml` ne dépend pas de `packages/graph` ; et **aucune projection ne porte le coût** — `grep` sur `packages/projections/src` rend zéro fichier. La clause la plus longue du test de sortie de `W12.d` — « le graphe rend la conclusion, ses prémisses, son expérience, ses artefacts, ses objections et son coût » — n'a donc de sujet pour aucun de ses six termes. Test de sortie : les six se lisent par une query de §22.4, depuis le journal seul et sans instantané reçu d'un worker (`W13.g` tient déjà cette règle pour le graphe organisationnel) ; une conclusion sans prémisse se lit comme telle plutôt que de manquer ; **les objections y sont** — invariant 12, et un graphe qui les tairait serait « propre » au sens que la spec refuse ; le coût vient de §17 et il est **absent** tant que personne ne l'a relevé, jamais nul |
| W20.v `[R]` **fait** | le **bail est émis à la réclamation**, jamais à la mise en file — §12.3 | vérifié au code en tentant `W20.s` : **rien, dans tout le dépôt, ne construit un `Lease` hors des tests**, et `lep_queue` en **exige** un de son appelant. Deux conséquences, et la seconde est la grave. Une route de §22.3 devrait donc faire fournir un bail par un client HTTP, c'est-à-dire lui laisser nommer le worker et l'échéance — alors que §12.3 en fait le travail du daemon, « ce qui distingue un worker en panne d'un worker lent ». Et surtout : `Claim` refuse un bail dont le `worker_id` n'est pas le réclamant — un test le tient depuis `W20.k` —, donc **la file ne peut contenir que des missions déjà attribuées**, et la question de placement que `W20.q` pose au broker ne peut jamais que confirmer ou refuser le worker que le bail avait déjà choisi. Le placement est décoratif tant que ceci n'est pas corrigé. Test de sortie : la file porte des `MissionEnvelope`, pas des paires — un bail n'a pas d'objet avant qu'un worker soit choisi ; le bail est frappé **après** que le placement a admis le worker, pour ce worker-là, et deux workers qui réclament la même file reçoivent chacun le sien ; le rang d'attempt vient de la proposition et non d'un compteur, parce que §12.3 veut qu'une tâche réattribuée **conserve** son numéro ; le TTL et l'intervalle de heartbeat sont des valeurs de politique nommées, et la relation de §12.3 — heartbeat sous le tiers du TTL — est tenue par un test, puisque Draft 7 ne sait pas l'énoncer et que le schéma le dit lui-même |
| W20.j `[M]` | l'**idempotence du client** devient un fait du journal | `W20.h` a livré le chemin d'écriture et laissé cette garantie fausse là où elle sert : le registre des clés de §22.5 vit en mémoire vive, et un redémarrage — précisément ce qui coupe les connexions et déclenche les retentes — l'oublie. L'ADR 0029 décision 4 tranche la forme ; c'est une **migration**, donc son propre item, et `CLAUDE.md` demande qu'une migration ne se mélange pas à un changement de signature et à une reprise de concurrence dans le même commit. Test de sortie : une commande retentée après un redémarrage **ne s'exécute pas deux fois**, vérifié en reconstruisant depuis le journal ; le registre est une projection de plus à côté des quatre de §9.5, et il se rattrape comme elles ; l'enveloppe gagne **un** champ facultatif — la portée est déjà là, `workspace_id` et `actor.principal_id` — et un événement écrit avant la migration se relit **sans clé** plutôt qu'avec une clé vide, comme `W21.m` traite une écriture non classée ; rien n'expire, et la décision 5 dit pourquoi la durée de rétention est une valeur de politique et non une constante ; `#[serde(default)]` des deux côtés, sans quoi le rollback demanderait de réécrire le journal |
| W20.k `[R]` **fait** | la **surface §15.2** de `locusd` : par où un worker réclame, remonte et rend | découvert en marquant `W2.21` — **aucun item de cette roadmap ne servait §15.2 côté `locusd`**, et `apps/locusd/src/http.rs` le confirme : sept routes, toutes en lecture, aucune sous `/lep/`. Trois lignes pointaient déjà ce trou sans le nommer — `W23.b` a fait porter son marqueur à `W2.20` puis à `W2.21`, `W12.d` ne listait ni l'un ni l'autre. C'est le **cinquième** maillon de la fermeture verticale, et il était invisible pour la même raison que les quatre autres : chaque côté est cohérent séparément. `W2.21` sait parler les trois chemins ; personne ne les écoute. Test de sortie — **la clause du harnais est corrigée** : `packages/testing` **joue le serveur** (« il n'y a personne pour compenser »), donc « le tourner contre le daemon » opposait deux serveurs et ne voulait rien dire ; la clause a été écrite en marquant `W2.21` et n'a pas survécu à la lecture du harnais, une heure plus tard. Ce qui la remplace tient la même propriété par un autre bout : `/lep/v1/claim`, `/lep/v1/events` et `/lep/v1/result` sont servis, et les corps de requête et de réponse sont les types **générés** de `packages/lep`, jamais des miroirs écrits à la main — les deux moitiés du fil viennent alors d'un seul schéma, et un changement de schéma casse les deux côtés à la compilation au lieu de les laisser diverger en silence ; une réclamation sans mission assignable rend `204` et **non** une erreur — la séparation « rien pour toi » / « je n'ai pas pu demander » de l'ADR 0028 décision 4 est tenue des **deux** côtés du fil, et un test la vérifie depuis le client de `W2.21` sans le modifier ; un résultat rendu passe par un command handler transactionnel, donc l'achèvement d'une tentative **atteint le journal** — c'est le fait exact que `W23.b` compte, et rien d'autre dans cette roadmap ne le fait exister ; deux remises portant la même `idempotency_key` de §15.5 produisent **un** fait, la durabilité du registre restant l'affaire de `W20.j` pour que les deux ne se confondent pas ; une créance de worker invalide reçoit un refus **typé** de §22.5, jamais une trace ; la règle 4 de `boundaries.json` reste tenue — servir un worker n'ouvre aucun socket de runtime |
| W20.i `[M]` **fait** | le driver PostgreSQL de `packages/event-store` | la suite de contract tests de `W1` passe **à l'identique** contre les deux backends, et l'implémentation mémoire reste le backend de test — un driver qui exigerait d'adapter les tests aurait changé le contrat ; une reconstruction depuis zéro rend l'état courant ; la troisième frontière tient — aucun client PostgreSQL hors `packages/event-store` et projections, et la garde le vérifie sur le nouveau chemin ; plan de rollback dans l'ADR |

`W20.g` après `W20.e` et `W20.f`, dont il est la liaison : il ne sert que ce qu'ils ont déjà rendu
vérifiable sans socket. `W20.a` avant `W20.b` : un handler reçoit une enveloppe. `W20.c` peut se faire en parallèle des deux
premiers et **doit** précéder `W20.d`. `W20.e` et `W20.f` après `W20.d`, qui est ce qu'elles exposent.

`W20.h` et `W20.i` sont les deux premiers des **cinq maillons de la fermeture verticale** — cinq
maillons qui, une fois tous livrés, n'ont **pas** suffi : voir `W12.d`, dont le marqueur les
attendait tous et qui reste impossible faute de trois conditions que personne n'avait nommées. Sans
eux, le bâtiment a des murs, une porte de lecture, et rien qui écrive ni qui dure. Les trois autres
sont `W2.20` — la boucle du worker —, `W4.h` — la surface du broker — et `W20.k` — la surface §15.2,
par où le worker parle au daemon. Aucun des cinq ne se voyait dans un décompte d'items faits, parce
que chaque côté est cohérent séparément ; `W20.k` est resté invisible **plus longtemps que les
autres**, jusqu'à ce que `W2.21` livre un client dont on pouvait constater que rien ne l'écoutait.

**Ce que `W20` débloque :** `W17.f` — `/branches/:id/diff`, la preview, l'ombre, l'approbation, le
rollback et la navigation dans le temps — dont la logique est déjà écrite et à qui il ne manque
qu'une façade.

## W21 — Les métriques structurelles, définies avant d'être calculées

**ADR 0024.** La matrice d'acceptation V1 exigeait treize métriques structurelles ; une seule avait un producteur — `structural_regret`, livrée par `R3` — et **aucune n'était définie nulle part**. Les treize noms n'apparaissaient qu'à la ligne 34 de `docs/11`, sans formule, sans numérateur, sans dénominateur.

D'où l'ordre : l'ADR d'abord, le code ensuite. Une métrique implémentée depuis un nom non défini est pire qu'une métrique absente — l'absente n'induit personne en erreur, tandis qu'un nombre affiché sera lu, cité et suivi sans que personne sache ce qu'il compte, et rien dans son apparence ne distingue un nombre bien défini d'un nombre mal défini.

L'ADR a arrêté **quatre renommages**, chacun parce que le nom promettait plus que le calcul : `graph_edit_distance` annonçait une minimalité NP-difficile que le `diff` ne calcule pas ; `parallelism` était déjà une dimension de budget de §7.2, donc un plafond homonyme d'une mesure ; `state_transfer_volume` appelait des octets que l'ADR 0019 condition 3 interdit délibérément de produire ; `topology_entropy` ne disait pas de quelle distribution, parmi au moins quatre qui ne classent pas dans le même ordre.

Onze items à écrire, plus `W21.m` dont `W21.l` dépendait — `W21.m` est livré et le blocage est levé.
`structural_regret` n'a pas d'item : elle est livrée.

| # | Commit | Dépôt | Test de sortie |
|---|---|---|---|
| W21.a `[R]` **fait** | `mutations_per_run` — les opérations de coordination **appliquées** au cours d'une exécution, par sorte | locusolus | le compte se rejoue à l'identique depuis le même préfixe de journal, et deux calculs rendent la même valeur ; il compte les opérations **appliquées**, jamais les proposées, et un test le tient sur une fixture où une proposition est refusée ; les sortes viennent de `Operation::NAMES`, et un test d'exhaustivité échoue si une sorte entre sans que le compteur la connaisse ; aucun seuil, aucune note, aucun verdict — l'absence est tenue par un test qui refuse `const MIN`, `const MAX`, `fn is_healthy`, `fn score` et `enum Verdict` dans la source |
| W21.b `[R]` **fait** | `edge_churn` — arêtes ajoutées **plus** arêtes retirées sur une fenêtre, jamais le solde | locusolus | deux arêtes qui entrent et deux qui sortent rendent **quatre**, pas zéro, et c'est le test qui porte l'item — le piège existe déjà sous ce nom dans `coordination/tests/region.rs`, où un plafond de deux refuse quatre identités changées à solde nul ; le churn ne se déduit pas du compte d'arêtes, et un test le montre sur deux versions de même cardinalité et de contenu disjoint |
| W21.c `[R]` **fait** | `applied_edit_length` — la longueur du `diff` de `W17.h` entre deux versions | locusolus | le nom ne promet pas la minimalité : un test vérifie que la valeur est une **borne supérieure** de la distance véritable, sur un cas où le chemin parcouru est strictement plus long que le diff direct — ajouter puis retirer la même arête coûte deux au chemin et zéro à la destination ; l'écart avec `W21.a` est le **détour**, et un test le calcule plutôt que de le décrire ; aucune fonction de ce module ne s'appelle du nom de la distance de graphe, tenu par un test d'absence sur les signatures |
| W21.d `[R]` **fait** | `accepted_mutation_rate` — approuvées ÷ **parvenues à une décision terminale** | locusolus | une proposition encore en attente n'entre **pas** au dénominateur, et un test le tient en montrant que le taux ne bouge pas quand on ajoute des propositions indécises — sans quoi la lenteur des décideurs se lirait « les agents proposent n'importe quoi » ; les indécises sont rendues **à part**, avec le taux, jamais fondues dedans ; c'est la règle de `W18.e`, où le silence n'est pas un accord, appliquée aux mutations de coordination |
| W21.e `[R]` **fait** | `rollback_rate` — par cohorte, jamais un nombre unique | locusolus | une cohorte est un ensemble d'acceptations délimité **plus** une fenêtre d'observation, et le type ne se construit pas sans les deux ; une cohorte dont la fenêtre n'est pas close est rendue **incomplète** avec le nombre d'acceptations encore observables, et non comme un taux provisoire — un test refuse qu'un `f64` sorte d'une cohorte ouverte, parce qu'un lecteur le comparerait à un taux définitif ; deux cohortes de fenêtres différentes ne se comparent pas, et l'API ne l'offre pas |
| W21.f `[R]` **fait** | `degree_entropy` — entropie de Shannon des degrés, divisée par `log n` | locusolus | la normalisation est exercée : deux organisations de même forme et de tailles différentes rendent la **même** valeur, ce qu'un test tient par égalité ; sans elle, comparer les nombres bruts reviendrait à comparer des tailles en croyant comparer des structures ; la métrique ne mesure **pas** l'équité de charge, et un test exhibe une organisation à entropie de degrés élevée et à charge très concentrée, où `busiest_reviewer_load` de `R3` répond et où celle-ci ne répond pas ; `n = 1` et « aucune arête » sont deux cas rendus explicitement et **distincts**, jamais une division par zéro ; `n = 0` s'est révélé **inexprimable** — `Version::root` le refuse et retirer le dernier nœud aussi —, donc lui donner une variante aurait annoncé un cas que rien ne peut produire, et un test tient l'absence par les deux chemins plutôt que par l'énumération |
| W21.g `[R]` **fait** | `critical_path_length` — la plus longue chaîne de dépendances du graphe de tâches | locusolus | un cycle est **refusé en le nommant**, jamais parcouru : `R3` a déjà montré qu'une version de coordination peut être cyclique, et une métrique qui ne termine pas emporte son appelant — le test l'exerce sur un cycle réel ; la valeur est un compte d'**étapes**, pas une durée, et le nom de la fonction ne parle pas de temps ; la relation de dépendance entre **en données** : vérification faite, aucun graphe de tâches n'existe — `Task` porte un état et des assignations, `Barrier` **n'expose délibérément aucun nœud**, et `depends_on`/`blocked_by` vivent dans `packages/graph`, que la sixième frontière interdit au domaine de coordination. En construire un **pour** avoir quoi mesurer aurait été bâtir une fonctionnalité afin de justifier une métrique ; la recevoir en données est une capacité finie, ce que la décision 0 de l'ADR 0022 autorise. Le module vit donc dans `packages/evaluation`, qui n'a aucune dépendance : l'impossibilité de le calculer sur le graphe de coordination est tenue par le **graphe de paquets** et non par une recherche de texte, et un test lit le `Cargo.toml` pour le vérifier |
| W21.h `[R]` **fait** | `average_parallelism` — travail total ÷ `critical_path_length` | locusolus | ce n'est **pas** le nombre d'agents qui tournaient, et un test l'exerce sur une fixture où les deux diffèrent ; à ne pas confondre avec `Dimension::Parallelism` de §7.2, qui est un **plafond** et non une mesure — un test d'absence refuse que le module importe `locus-budget`, la confusion entre la borne et le constat étant précisément ce que le renommage de l'ADR 0024 évite ; un chemin critique nul ne produit pas une division par zéro mais un cas rendu |
| W21.i `[R]` **fait** | `handed_over_attempts` — les tentatives en vol transmises par `Handover` | locusolus | la mesure se lit du `Handover` de `W16.e` et de rien d'autre ; elle ne compte **pas** d'octets, et un test d'absence refuse toute signature parlant de taille ou de volume — l'ADR 0019 condition 3 interdit la copie de contexte qui en produirait, donc une métrique de volume vaudrait zéro en permanence ou créerait le coût qu'elle prétend observer ; un `kill` ne produit aucune mesure, puisqu'il abandonne au lieu de passer la main, et un test le distingue d'un passage de témoin à zéro tentative — deux faits différents, jamais la même valeur |
| W21.j `[R]` **fait** | `agent_lifetime` — durée entre l'entrée d'une instance dans une version et sa sortie | locusolus | la durée se lit des transitions journalisées de `lifecycle.rs`, et se rejoue à l'identique ; une instance encore en place n'a **pas** de durée close, et est rendue comme telle plutôt que mesurée jusqu'à maintenant — un test le tient, parce qu'une durée arrêtée à l'instant de lecture change à chaque lecture et n'est pas un fait du journal ; la métrique ne dit rien de ce que l'instance a accompli, et son module n'a aucun chemin vers un résultat |
| W21.k `[R]` **fait** | `failure_recovery_time` — durée entre un fait de panne et le fait de reprise correspondant | locusolus | l'appariement panne/reprise est **explicite** : une panne sans reprise est rendue comme non reprise, jamais omise ni comptée avec une durée nulle, et un test tient les deux absences comme distinctes ; la mesure est calculable sur fixtures et testée ainsi, mais son **interprétation** demande une campagne longue — l'item l'écrit plutôt que de laisser trois transitions se lire comme un fait de production ; se branche sur le cadre de `evaluation/src/endurance.rs` sans le modifier |
| W21.m `[M]` **fait** | la **classification de dépense** dont `communication_tokens` dépend — une écriture de budget dit si elle paie de la coordination ou du travail | locusolus | `EntryKind` distingue le **mouvement** — allocation, réservation, libération, consommation, ajustement, remboursement — et jamais son objet ; ce qui manque est l'objet, et l'ajouter est une migration : une écriture ancienne se lit **non classée**, jamais « travail » par défaut, et un test le tient sur une écriture sans le champ ; aucune classification ne se déduit du texte libre de `reason`, tenu par un test d'absence qui refuse toute lecture de ce champ dans le classificateur — une justesse qui dépendrait de la rédaction de chaque appelant se dégraderait au premier qui écrit autrement ; plan de rollback dans l'ADR 0024 |
| W21.l `[R]` **fait** | `communication_tokens` — tokens de coordination ÷ tokens totaux | locusolus | débloqué par `W21.m`, qui a livré la classification qui manquait : `Entry::spend()` rend `Coordination`, `Work` ou **non classé**. Test de sortie : les écritures **non classées** sont comptées à part et n'entrent dans aucun des deux termes, exactement comme `W21.d` traite les indécises ; le dénominateur ne contient donc que ce que quelqu'un a déclaré, et une campagne entièrement non classée rend une **absence** de rapport, pas un zéro |

`W21.a` à `W21.c` mesurent l'ampleur du changement, `W21.d` et `W21.e` sa qualité, `W21.f` à `W21.h` la forme de l'organisation, `W21.i` le coût d'une reconfiguration, `W21.j` et `W21.k` ce qui ne se voit que dans la durée. Aucun ordre n'est imposé entre les groupes : chaque item se calcule sur des faits déjà écrits et ne dépend d'aucun autre — sauf `W21.l`, qui attend `W21.m`.

**Ce que la phase ne contient pas.** Aucun seuil, aucune note, aucun verdict, dans aucun des onze modules. La décision 9 de l'ADR 0024 étend à toute la famille la règle que `R3` avait posée pour ses cinq métriques : un seuil écrit en Rust a l'apparence d'un fait mesuré alors que c'est une décision de politique, et l'y inscrire le soustrait à la discussion tout en le rendant invisible à qui lit le nombre. Ce qui transforme une quantité en jugement est le moteur de politique, où le seuil est une valeur qu'on peut voir, discuter et changer.

## W22 — La cohérence descriptive, vérifiée comme le reste

**ADR 0025.** Un audit externe du 2026-08-21 a trouvé quatre affirmations fausses sur l'état du système, dans deux dépôts, toutes de la même forme : une déclaration en prose qu'aucun test ne vérifie. Les quatre ont été revérifiées à la ligne avant l'ouverture de cette phase. Le dépôt a des gardes pour ses frontières, ses imports, ses dépendances, ses schémas et son isolation Emacs ; il n'en a aucune pour ce qu'il dit de lui-même.

**La cause est plus profonde que le symptôme, et l'ordre des items en découle.** L'audit constate que `W4.d.1` à `W4.d.4` n'ont pas de marqueur `**fait**` alors que le ledger porte quatre entrées complètes. La vérification faite pour l'ADR montre pourquoi : la garde de roadmap **ne voit pas ces lignes**, son motif d'identifiant s'arrêtant devant le second point. Huit lignes sont dans ce cas, et le « frontière vide » que la garde imprimait portait sur 179 lignes sur 187 sans le dire. C'est la **troisième** occurrence de la même cécité — `W0.17` en avait réparé une autre sans demander si d'autres formes existaient. La garde se répare donc **avant** que les marqueurs soient posés : marquer à la main pendant qu'elle reste aveugle rendrait la roadmap vraie une fois, sans empêcher la faute de revenir.

| # | Commit | Dépôt | Test de sortie |
|---|---|---|---|
| W22.a `[R]` **fait** | la garde de roadmap voit **toutes** les formes d'identifiant du plan, et les huit lignes invisibles reçoivent leur marqueur | locusolus | la garde reconnaît `W4.d.1` comme elle reconnaît `W4.d`, et le **déclare** : elle imprime le nombre de lignes reconnues, et une baisse de ce nombre est un échec — sans quoi une cécité future se lirait « rien à vérifier » ; la vérification en rouge se fait sur une ligne réellement présente et non sur une ligne inventée pour l'occasion ; les huit lignes (`W4.d.1` à `W4.d.4`, `W4.e.1`, `W4.f.1`, `W4.g.1`, `W4.g.2`) portent ensuite leur marqueur, chacune contre l'entrée de ledger qui l'atteste, ou disent ce qui manque ; le motif n'est **pas** élargi à des formes qu'aucune ligne n'emploie, décision 4 de l'ADR 0025 |
| W22.b `[R]` **fait** | la garde de roadmap : le couple marqueur ↔ entrée de ledger, dans les **deux** sens | locusolus | **Trouvé déjà livré**, et c'est un constat de la phase elle-même : `livre-non-marque` est né en `W0.12`, `marque-non-livre` en `W0.13`, et les deux étaient justes. Ce qui manquait n'était pas une règle mais une **vue** — les huit lignes que le motif tronquait n'entraient dans aucun ensemble, donc aucune des deux ne parlait d'elles. Réparer `W22.a` les a fait protester sur les huit d'un coup, sans qu'une ligne de règle soit écrite. Ce que ce sprint ajoute est donc la **non-régression** : les deux règles sont exercées sur la forme à deux points, sans quoi la prochaine forme d'identifiant repasserait par le même trou. Écrire cet item comme du travail neuf était une affirmation fausse sur l'état du système — exactement ce que l'ADR 0025 existe pour rendre coûteux, rencontré dans le plan qui le met en œuvre |
| W22.c `[R]` **fait** | `main.rs` de `locus-execd` : le binaire ne nie plus ce que son crate exporte | locusolus | le binaire construit le driver, ou refuse en nommant ce qui manque **réellement** — un hôte, une configuration — et jamais un driver que son crate exporte ; la vérification en rouge est faite en niant une capacité réellement exportée, et non en éditant le message ; le refus, quand il a lieu, reste distinct d'un démarrage réussi par son code de sortie, comme `locusd` le fait déjà pour une projection en quarantaine |
| W22.d `[R]` **fait** | la garde de cohérence déclaration/plan dans `tooling/` | locusolus | **la formulation d'origine était fausse et l'implémentation l'a démentie** : chercher « le symbole que le refus déclare manquant » n'aurait pas attrapé le défaut qui a motivé l'item, puisque « aucun driver de runtime n'est encore branché (W4.d) » ne nomme aucun symbole ; et chercher la tournure aurait mordu sur le refus **calculé** de `locusd`, « le port n'est pas ouvert ». Le signal est que le message **cite un item du plan** — un couple déclaration/plan, pas un mot. Test de sortie : le message d'origine est attrapé mot pour mot ; le refus calculé de `locusd` passe ; un identifiant en commentaire ne déclenche rien ; les points d'entrée sont **découverts** des `[[bin]]` et nommés, et un décompte nul est un échec ; le motif reconnaît tous les identifiants que le plan déclare réellement, ce qui tient l'accord avec la garde de `W22.a` contre le plan et non contre une copie du motif |
| W22.e `[R]` **fait** | la sonde réelle de Canterel : `bubblewrapWorks` et `diskFreeMb` | canterel | `bubblewrapWorks` **démarre** `bwrap` comme le contrat de `capability-manifest.ts:33` l'exige, et un test de contrat exerce l'**adaptateur réel**, pas un `HostProbe` injecté ; `diskFreeMb` est mesuré ; une sonde qui ne peut pas conclure rend **l'absence** et non zéro, comme `W4.b` traite une sonde non exécutée — c'est le seul des quatre défauts qui porte sur un **niveau d'isolation**, et un `CapabilityManifest` erroné remonterait jusqu'à `place` de `W4.g`, qui choisirait un hôte sur une preuve qui n'en est pas une ; aucun fichier de `src/session/`, `agent/`, `permission/`, `provider/`, `tool/` n'est touché, tenu par `upstream.test.ts` |
| W22.f `[R]` **fait** | le commentaire d'inertie de `locus/index.ts` | canterel | la prose du point d'entrée décrit l'état **courant** ; l'historique va au ledger ; un test échoue si un identifiant d'item marqué livré dans `IMPLEMENTATION_LEDGER.md` est invoqué comme condition non satisfaite dans `src/locus/` ; l'inertie du worker n'est pas levée ici — elle est levée par `W2.20` —, seule la **raison** invoquée est rendue vraie, et confondre les deux ferait passer une correction de vérité pour une livraison |

**L'ordre importait pour les deux premiers, et il a payé tout de suite.** `W22.a` a réparé l'instrument ; `W22.b`, qu'on croyait être une règle à écrire, s'est révélé n'être qu'une règle à **rendre exerçable**. Les quatre suivants sont indépendants entre eux.

**Ce que la phase ne fait pas.** Elle ne livre aucune capacité nouvelle et ne lève aucun blocage fonctionnel. Elle rend vrai ce que le dépôt dit — ce qui, d'après l'ADR 0025, est une dette de la même nature qu'une promesse non tenue, et non un travail de confort.

---

## W23 — La population virtuelle

**ADR 0026.** Un audit du terrain a inspecté au code les systèmes qui revendiquent des populations
d'agents de grande taille, et le résultat contraint la conception : **aucun ne démontre de cognition
agentique massivement concurrente**, et le seul runtime transférable y parvient en rendant les agents
sans état entre deux pas. « Supporter N agents » est donc défini avant d'être visé — N identités
ordonnançables, dont un sous-ensemble variable raisonne concurremment — et trois compteurs distincts
sont exigés partout où une taille est rapportée.

**Ce que ce dépôt a déjà, et qu'il ne faut pas reconstruire.**
`packages/coordination/src/agent.rs` porte `AgentInstance` sous son nom, avec `Id<Agent>` pour
identité et les six `InstanceState` de §7.1 ; `Task` porte `assigned_agent_id` ; `lifecycle`
journalise les transitions ; `W21.j` en tire déjà la durée de vie d'une instance. Il manque le port
de persistance et le protocole de reconstruction, et rien d'autre.

**L'ordre des blocages n'est pas celui de l'audit, et la décision 0 de l'ADR 0026 dit pourquoi.**
L'audit bloquait la phase entière derrière la fermeture verticale. La règle du dépôt — « aucun
appelant ne l'utilise encore n'est pas un motif de report » — n'en garde que ce qui nomme une
dépendance technique : `W23.a` est un port avec son implémentation de référence, donc une capacité
finie, donc exerçable aujourd'hui ; `W23.b` attend un fait que rien n'écrit encore ; `W23.d` attend une **campagne**, donc la chaîne
complète. `W23.c` figurait ici et n'y a plus sa place : son test de sortie est fait de fixtures et de
deux tests d'absence, donc « aucun appelant » était son seul motif — celui que l'ADR 0022 décision 0
refuse. L'ADR 0026 est amendé en conséquence.

| # | Commit | Dépôt | Test de sortie |
|---|---|---|---|
| W23.a `[R]` | `AgentStateStore` : le port, et l'implémentation de référence en mémoire | locusolus | une instance se reconstruit depuis son état persisté et rend **exactement** le même état, condensat compris ; **aucun objet d'agent ne traverse une frontière de processus**, tenu par l'absence de type sérialisable portant un comportement ; le port n'a pas de variante « peu importe l'état » — reconstruire sans savoir depuis quelle révision n'a pas de sens, et `Expected` de l'event store le dit déjà pour l'écriture ; un backend externe n'est **pas** choisi, et l'ADR 0026 décision 2 dit pourquoi |
| W23.b `[R]` | les trois compteurs : `nominal`, `active`, `generating` | locusolus | **débloqué par `W20.k`, et c'est la première fois que la condition est réellement satisfaite.** Le marqueur avait attendu `W2.20`, puis `W2.21`, deux jalons voisins livrés sans rien changer à ce qui manquait ; la troisième nomination a visé `W20.k`, un item qui n'existait pas avant qu'on le crée — et c'est précisément pourquoi les deux premières avaient visé à côté : **il n'y avait rien à viser**. La condition, elle, n'a jamais bougé : `generating` compte un fait qui doit **atteindre le journal**. Il l'atteint depuis `W20.k` — `task.leased` l'ouvre, `run.completed` le referme, les deux écrits par un command handler transactionnel. Ce que `W20.l` ajoutera est que les **projections** le voient sans redémarrage ; le recalcul depuis le journal, lui, n'attend rien. `generating` compte donc un fait qu'aucun journal n'écrit, les six `InstanceState` de §7.1 n'en portant pas l'équivalent, et **inventer ce fait pour avoir quoi compter** serait ce que `W21.g` a refusé sous ce nom. Test de sortie : les trois se recalculent depuis le journal et se rejouent à l'identique ; un rapport qui n'en porte qu'un **n'est pas constructible**, tenu par le type ; `generating ≤ active ≤ nominal` est un invariant testé, et une violation nomme les trois valeurs ; aucun de ces compteurs ne porte de seuil, comme les treize métriques de `W21` |
| W23.c `[R]` | l'ordonnanceur d'instances, au-dessus du placement d'hôte | locusolus | **débloqué en marquant `W2.21`, et le motif n'en était pas un.** La ligne disait « il ordonnance des instances qui s'exécutent », ce que l'ADR 0026 décision 0 reprenait sous la forme « ne se teste pas sur des fixtures seules » — or son propre test de sortie ci-dessous est fait de trois vérifications sur fixtures, dont deux sont des tests d'absence. La condition était donc « aucun appelant ne l'utilise encore », que l'ADR 0022 décision 0 refuse explicitement comme motif de report, et l'ADR 0026 l'a écrite quand même. Amendement porté dans l'ADR 0026. Test de sortie : réveiller, suspendre, drainer, remplacer se lisent de `coordination::lifecycle` et de rien d'autre — le module n'en redéfinit aucun ; `place` de `W4.g`, qui vit chez `locus-execd`, reste **seul juge de l'hôte**, et un test d'absence refuse que l'ordonnanceur d'instances en choisisse un ; une décision locale ne produit **aucun** événement de portefeuille, et un test le tient en comparant les namespaces émis |
| W23.d `[R]` **bloqué** | la mesure de la taille de regroupement — **avant** toute cellule | locusolus | `attend:W12.d` — c'est une **campagne**, pas une fixture, et une campagne suppose la chaîne complète : `W2.20` n'en était qu'un maillon. Test de sortie : l'expérience compare, sur une même tâche et un même budget, un regroupement collaboratif contre des tentatives indépendantes réévaluées ; elle rend `N_eff` par `R2`, le regret par `R3`, l'entropie de degré par `W21.f` et le parallélisme par `W21.h` ; **aucune taille de cellule n'est écrite dans le code avant que cet item ait rendu un résultat**, tenu par l'absence de constante ; un résultat qui ne tranche pas est rendu comme tel, jamais arrondi vers l'hypothèse de départ. §18 est à lire **avant** : la fusion de branches est le mécanisme dont cette expérience teste l'hypothèse |
| W23.e `[R]` **bloqué** | `Cell` — **conditionnel au résultat de `W23.d`** | locusolus | `attend:W23.d` — et le mot « conditionnel » est littéral : si les tentatives indépendantes gagnent, cet item n'existe pas. Test de sortie : une cellule porte un budget, un périmètre de `ContextView`, une enveloppe de politique et un ordonnanceur local, et **n'est pas constructible sans les quatre** ; une décision dans les bornes ne remonte pas au portefeuille, une décision hors bornes y remonte en **nommant la borne franchie** ; la taille vient de `W23.d` et n'est écrite nulle part ailleurs |

## W24 — Le routage par intention, borné par l'autorisation

**ADR 0026 décision 4.** Un mécanisme sémantique peut **choisir** un destinataire dans un ensemble
autorisé ; il ne détermine jamais l'autorisation. C'est la borne que la source omet : chez elle, la
souscription s'aligne sur le prompt système que l'agent apporte en rejoignant le réseau, ce qui
reviendrait ici à faire négocier aux agents leur propre accès à l'information — §12.4, l'invariant 11
et §16.6 cassés d'un seul geste.

Aucun de ces trois items n'est bloqué : ce sont du domaine pur et des tests d'absence, et `ContextView`
existe déjà dans `packages/review`.

| # | Commit | Dépôt | Test de sortie |
|---|---|---|---|
| W24.a `[R]` | la souscription **dérivée** de la `ContextView` | locusolus | une souscription se calcule de la `ContextView` et de rien d'autre ; **aucun chemin de type ne permet à un agent d'écrire sa propre souscription**, tenu par l'absence ; un agent qui demande un élargissement passe par la demande d'extension existante, jamais par sa souscription — et un test exhibe les deux chemins pour montrer qu'ils ne se rejoignent pas |
| W24.b `[R]` | l'appariement sémantique **dans** l'ensemble autorisé | locusolus | l'ensemble candidat est calculé **avant** l'appariement, et l'appariement ne peut pas l'élargir, tenu par la signature ; un test exhibe un pair sémantiquement parfait et non autorisé, et vérifie qu'il n'est **jamais** sélectionné ; l'aveuglement du reviewer survit à tout appariement, exercé sur une revue indépendante réelle |
| W24.c `[R]` | la fiabilité observée — **une seule polarité** | locusolus | une seule convention dans tout le module : un test refuse qu'un seuil se compare dans un sens à un endroit et dans l'autre ailleurs — la source de l'ADR 0026 en porte deux opposées dans la même machinerie bayésienne, et les transcrire produirait un filtre **inversé en silence** ; le nom du type dit son sens, et ce qui compte des fautes ne s'appelle pas réputation ; la fiabilité observée **influence le rang, jamais la validité**, et aucun chemin ne mène d'une observation vers un `Support` ou une prémisse d'`Inference`, tenu par l'absence comme `MetaMemory` de l'ADR 0022 décision 2 |

## W25 — La cognition comme dimension d'ordonnancement

**ADR 0026 décision 6.** La mesure la plus directement rentable du dossier, et la seule industrielle :
à qualité identique vérifiée par test caché, un facteur 7,9 sur le coût total et 22 sur la flotte de
workers seule. Le levier n'est pas le modèle, c'est **l'affectation** — frontière pour planifier, bon
marché pour exécuter. `packages/budget` porte déjà les six dimensions de §7.2 et, depuis `W21.m`, la
classification `Coordination` / `Work` / non classé.

| # | Commit | Dépôt | Test de sortie |
|---|---|---|---|
| W25.a `[R]` | la classe de cognition dans la mission | locusolus | une mission déclare une **classe**, jamais un modèle, et un test d'absence refuse tout identifiant de modèle dans le domaine ; l'affectation classe → modèle est une valeur de politique versionnée, visible dans la trace d'évaluation de §20.5 ; **changer l'affectation ne change aucun type**, et c'est le test qui porte l'item |
| W25.b `[R]` | le plafond de cognition dans `packages/budget` | locusolus | le plafond est une dimension au sens de §7.2, et son dépassement refuse en **nommant la dimension** ; il se combine avec la classification de `W21.m` — une dépense de coordination et une dépense de travail ne s'imputent pas au même plafond, et un test le tient ; une dépense **non classée** n'entre dans aucun plafond, exactement comme `W21.l` la traite, et ne devient pas « travail » par défaut |
| W25.c `[R]` | la fabric d'inférence comme **capacité admise** | locusolus | elle n'entre que par un `Published` de `W5.b`, comme le raisonneur d'ontologie de `W18.h` ; **aucun crate n'acquiert de dépendance vers un moteur de service**, tenu par `dependencies.json` et par la garde de frontières ; son absence dégrade la latence et **jamais** la correction, et un test l'exerce en la retirant |

## W26 — La rétention du raisonnement, et le dévoilement réglé

**ADR 0027.** Le dépôt savait détecter la fuite — `Contamination::GeneratorReasoningLeaked`, première
des cinq de §16.6 —, il avait le rayonnage privé — `Level::AgentPrivate`, le plus étroit des sept de
§16.1 — et le genre qui empêche la contamination épistémique — `Genre::MetaMemory`. Et **rien
n'écrivait le raisonnement nulle part** : l'invariant 11, qui borne un ensemble de lecteurs, avait
été lu comme un ordre de destruction.

Retenir et diffuser sont deux actes. Détruire est le seul qu'aucun audit ne rattrape, et l'invariant
12 l'interdit déjà pour les résultats négatifs.

Aucun de ces items n'est bloqué : la matière est là.

| # | Commit | Dépôt | Test de sortie |
|---|---|---|---|
| W26.a `[R]` | la trace de raisonnement comme **artefact** | locusolus | elle entre par le chemin de §9.1 — déclarée avant dépôt, hashée, référencée par son condensat — et **aucun second stockage n'apparaît**, tenu par l'absence comme `W16.e` le tient pour les messages ; elle est rangée en `Level::AgentPrivate` et `Genre::MetaMemory`, et un test refuse tout autre couple ; **aucun résumé n'est stocké à la place**, et un test d'absence refuse toute signature qui condenserait avant écriture — un résumé est une lecture, il se refait |
| W26.b `[R]` | les trois classes de lecteurs, et la lecture institutionnelle journalisée | locusolus | générateur, institution, pair : trois et pas quatre, tenu par une énumération close qu'un test lit sous ses noms ; l'institution lit sans condition d'autorisation **et la lecture produit un fait**, vérifié en comptant les événements émis ; un lecteur qui n'est aucune des trois n'a pas de chemin, et un test d'absence le tient ; c'est ce lecteur qui débloque `W16.d` |
| W26.c `[R]` | `Disclosure` : motif, portée, échéance, journal | locusolus | les quatre sont exigés par le type, et il n'existe pas de constructeur qui en laisse un de côté ; l'énumération des motifs **commence vide** et reçoit ici son premier — l'objection non résolue après un nombre borné de tours —, avec le mécanisme qui le déclenche, jamais sans ; « toutes les traces de cette branche » n'est **pas** une portée constructible ; un dévoilement expiré ne donne plus rien, et un test le passe de part et d'autre de l'échéance |
| W26.d `[R]` | l'aveuglement du reviewer, et le second verdict qui paie le dévoilement | locusolus | un `Disclosure` vers un reviewer dont la revue est **ouverte** n'est pas constructible, et le test le tient par l'absence — l'invariant 11 est une borne sur le mécanisme, pas un défaut qu'un motif surclasse ; après un verdict enregistré, un dévoilement produit un **second** verdict qui le porte dans sa provenance, et **les deux sont conservés** — un test vérifie que le premier reste lisible, l'invariant 12 interdisant de le faire disparaître ; `contamination::inspect` distingue le dévoilement de la fuite, et **le défaut reste la fuite** : un élément sans dévoilement valide attaché reste `GeneratorReasoningLeaked`, vérifié en rouge d'abord |

`W26.a` avant `W26.b` : on ne règle pas la lecture de ce qui n'est pas écrit. `W26.c` après `W26.b`,
dont il borne une des trois classes. `W26.d` en dernier, parce qu'il modifie une garde existante et
que le rouge doit être vu sur une garde qui marchait.


## Ce qui est reporté, et pourquoi c'est écrit ici plutôt que deviné

Deux items de cette roadmap ne se feront pas en V1, et les laisser marqués « bloqué » sans dire
combien de temps ferait croire à une attente courte.

**`W16.e` était le second, et il ne l'est plus.** Ce paragraphe garde sa trace parce qu'un report
levé s'explique aussi mal qu'un report posé : la section disait qu'il attendait « une messagerie
inter-agents, et il n'y en a pas », et que la construire **pour** débloquer l'item reviendrait à
construire une fonctionnalité afin de justifier un test. Ce raisonnement était juste tant que le
besoin n'était pas énoncé — ce qui a changé n'est pas l'argument, c'est sa prémisse. Le propriétaire
du produit a énoncé le besoin ; l'ADR 0019 a tranché la forme, et a écarté celle qui coûtait le gel.
La leçon à ne pas perdre : un report qui nomme sa condition se lève quand la condition tombe, et un
report qui n'en nommait pas serait resté par inertie.

**`W18.f` — l'admission exercée de bout en bout contre une sandbox `S3`/`S4` réellement attestée — est
reporté faute d'hôte**, et `W5.f` a rendu la condition précise. Il ne s'agit plus de « un hôte
capable » : il faut un hôte dont le système de fichiers porte les quotas (XFS avec pquota, faute de
quoi `podman create` refuse), qui sache isoler le réseau (`S3`) et démarrer une micro-VM (`S4`), et qui
sache **attester** ce qu'il a fait. Un runner GitHub n'en tient aucun des trois derniers et échoue sur
le premier. La condition est donc une machine dédiée, et son absence est un fait de déploiement, pas
une dette de code.

**`W12.e` est le second, et il partage exactement l'hôte de `W18.f`.** Le test de bout en bout de
`W12` a été **coupé en deux** plutôt que reporté en entier : `W12.d` exerce la chaîne complète —
mission, placement, session, événements, artefacts, `EpistemicCommit`, projections, graphe, redémarrage,
et le jumeau qui tue le worker au milieu — dans le confinement qu'un runner peut tenir, `S1`/`S2` ;
`W12.e` porte les deux clauses qui demandent autre chose que du code, l'attestation `S3`/`S4` et la
reproduction par un tiers. Reporter l'ensemble aurait retardé tout ce que la chaîne vérifie par
ailleurs, ce qui aurait fait payer à quinze garanties l'absence d'une machine. Et le contraire — tout
tenir dans un seul item et le déclarer fait sur un runner — aurait déclaré attesté un confinement qui
ne l'est pas, c'est-à-dire exactement l'affirmation fausse que l'ADR 0025 existe pour rendre coûteuse.

## Recherche — sans dépendance de chemin critique, abandonnable sans coût

**Les six sont faits**, livrés le 2026-08-18 dans `packages/graph`, `packages/coordination` et
`packages/evaluation`, chacun avec ses tests et sa passe de mutants.

Ils sont restés décrits ici **au présent**, comme des travaux à considérer, jusqu'au 2026-08-21 —
et pas par négligence de marquage : cette section était de la **prose**, et `check:roadmap` ne lit
que des lignes de tableau. Le garde de `W0.11` répondait donc `frontière vide` et `ok` par-dessus
six items livrés, sans se tromper une seule fois sur ce qu'il regardait. C'est le premier des deux
sens que `W0.11` nomme — la roadmap sous-estime, et une session refait le travail —, et il a failli
s'exercer : la session du 2026-08-21 a lu « le moins cher des items de recherche, publiable seul »
et s'apprêtait à réécrire une détection de consensus circulaire vieille de trois jours.

D'où la forme. Un tableau n'est pas une préférence de mise en page : c'est la seule forme que le
garde sait confronter au registre. `W0.17` lui a appris à lire la famille `R<n>` dans les deux
documents, et cette section est passée en lignes le même jour — les deux moitiés d'un même défaut,
puisque l'une sans l'autre laisse le mensonge intact.

| # | Commit | Test de sortie |
|---|---|---|
| R1 `[R]` **fait** | `packages/graph/src/consensus.rs` : le consensus circulaire de §16.6 lu **sur le graphe**, et non déclaré par un appelant | un cycle de `Cites` qu'aucun `AnchoredIn` ne fait sortir est rendu **une fois pour le groupe**, jamais une fois par membre — un cycle de cinq révisions est un problème, et le rapporter cinq fois donne cinq fois la même chose à corriger ; un cycle **ancré** n'en est pas un et reste rendu à part, `citation_cycles` et `circular_consensus` étant deux fonctions — les confondre ferait signaler la moitié d'une bibliographie ; l'ancrage se **dérive** des arêtes, et aucun booléen `is_external_source` n'existe ici, un drapeau qu'un appelant pose étant un drapeau qu'il peut poser à tort sur ce qu'il vient d'écrire ; un ancrage **interne** au groupe n'ancre rien, et `internal_anchors` le montre plutôt que de laisser chercher la réponse à la main ; Tarjan est **itératif**, une détection qui déborderait la pile sur un grand graphe étant absente exactement quand elle sert ; le module ne supprime ni ne démarque rien, invariant 12 — un consensus circulaire est un constat, pas une faute prouvée |
| R2 `[R]` **fait** | `packages/evaluation/src/credit.rs` : attribuer une amélioration à une relation, un rôle, un budget — ou au **hasard d'échantillonnage** | le hasard est une issue **nommée** et jamais un reste : `Credit::SamplingNoise` porte l'écart **et** la bande, donc « voici de combien la même configuration varie toute seule, et votre écart est dedans » plutôt que « on ne sait pas » — une attribution qui rend toujours l'un des trois facteurs donne une histoire à chaque fluctuation, et un système qui l'applique en boucle garde tous ses changements, dont la moitié n'a rien fait ; `Factor` en compte **trois**, le hasard n'étant pas un quatrième facteur ; quinze mutants, quinze tués |
| R3 `[R]` **fait** | `packages/coordination/src/metrics.rs` et `packages/evaluation/src/regret.rs` : cinq métriques structurelles, et le regret `R_s = U(meilleur candidat disponible) − U(graphe choisi)` | les cinq mesurent ce qu'**aucun invariant ne force** — une métrique d'une propriété déjà garantie rend la même valeur sur tout ce que le système accepte, et son passage au vert n'a jamais été en jeu ; **aucune ne juge**, et un test refuse `const MIN`, `const MAX`, `fn is_healthy`, `fn score`, `enum Verdict` — un seuil écrit en Rust a l'air d'une décision prise alors que c'est une question de politique ; la revue **mutuelle** est comptée par paire et non par arête : c'est la forme à deux du consensus circulaire de §16.6 transposée à la coordination, et le veto `ReviewAcyclicity` ne l'attrape pas parce qu'il porte sur un `diff` quand `Version::root` porte parfaitement l'aller-retour ; la profondeur ne suppose donc **pas** l'acyclicité et borne son parcours par le nombre de nœuds, exercée sur une version cyclique — une métrique qui ne termine pas emporte son appelant ; le regret se mesure contre le meilleur du **menu** et non contre un optimum imaginable, le choisi devant être **parmi** les candidats ; un lot dont les candidats ne partagent pas une fixture est refusé en la **nommant**, sans quoi le regret est un nombre qu'on fait baisser en changeant de fixture ; deux mesures d'une même structure sont une `Baseline`, pas deux candidats qui feraient battre une structure par elle-même ; `Regret::exceeds` confronte l'écart à la bande de `R2` |
| R4 `[R]` **fait** | `packages/evaluation/src/counterfactual.rs` : le substitut d'environnement sur une **trajectoire**, là où `W16.c` regardait une reconfiguration | `Outcome` a deux variantes et une seule conclut — l'autre s'appelle `NotRefuted` et **pas** « confirmé », deux trajectoires coïncidant sur une graine et un préfixe donnés pouvant parfaitement diverger au pas suivant ; unilatéral en rejet par un **chemin de types**, jamais par convention d'appel ; jamais un juge, jamais une preuve ; treize mutants, treize tués ; la fidélité sur les environnements du domaine — IIIF, SPARQL, ALTO/PAGE, notebooks, prouveurs — reste **inconnue**, et l'item la consigne au lieu de la supposer |
| R5 `[R]` **fait** | la sonde de harnais tiers : un `SessionPlan` peut-il produire un flux conforme au harnais de conformance de `W0.9` ? | quatre passages, et le contrôle négatif **mord 8/8** — sans lui les trois autres ne se liraient pas ; le plan seul rend 4 constats, tous sur des choses qu'un plan **n'a pas à porter** ; plan + état de connexion + lease : **0** ; la contrainte « aucune ligne dans `backend/cli/src/locus/` avant la réponse » est tenue, et la sonde n'a rien laissé dans aucun dépôt permanent. La réponse est **oui**, et la conséquence est un worker LEP séparé — le récit ci-dessous porte la mesure, et cette ligne ne le remplace pas |
| R6 `[R]` **fait** | `packages/evaluation/src/evolution.rs` : une adaptation récurrente et gagnante en validation appariée **propose** une amélioration de template | le seuil de récurrence ne descend pas sous **deux**, un seuil de un promouvant le tirage d'une observation unique, et la même exécution consignée trois fois reste une exécution — un mutant qui les distingue par leur rang meurt ; le module **ne rejuge rien** et compte des `Credit::Attributed` déjà rendus, un test refusant `Baseline`, `fn attribute`, `band`, `utility` dans la source — une seconde attribution aurait sa propre bande de bruit, qui divergerait de la première ; le résultat est une `Improvement` qu'**aucun chemin n'applique**, et elle **nomme** les exécutions plutôt que de les compter, parce qu'une proposition se conteste et que la contester demande d'aller relire ce qui est cité ; deux exécutions qui gagnent et une qui régresse ne se moyennent pas — `Evolution::Contradictory` les rend telles quelles, et la contradiction l'emporte même quand les gains dépassent largement le seuil, moyenner revenant à supprimer un résultat négatif pour rendre le dossier lisible (invariant 12) ; `NothingAttributed` et `NotRecurrent` sont **deux absences distinctes** |

**`R5` a été instruit le 2026-08-18, et la réponse est OUI.** La sonde a tourné dans un
environnement jetable — le conteneur de session, reclamé à l'inactivité — plutôt que dans un dépôt
GitHub, l'intégration n'ayant pas le droit d'en créer un. Ce qui compte pour l'item est tenu :
la sonde n'a rien laissé dans aucun dépôt permanent, et aucune ligne n'a été écrite dans
`canterel/backend/cli/src/locus/` avant la réponse.

**Ce qui a été mesuré.** Un `WorkerUnderTest` piloté par les seuls neuf champs d'un `SessionPlan`,
passé aux huit vérifications de W0.9. Quatre passages, et le troisième est celui qui rend les
autres lisibles :

| Passage | Constats |
|---|---|
| **C** — contrôle négatif, une violation injectée par vérification | **8/8 mordent** |
| **A** — le plan seul, rien d'autre | 4 |
| **B** — plan + état de connexion + lease | **0** |
| **D** — rang d'attempt faux, `worker_id` substitué, `task_id` étranger | 0 |

Les quatre constats du passage A portent tous sur des choses qu'un plan **n'a pas à porter** :
les niveaux de sandbox et les modes réseau du worker (annoncés au handshake, antérieurs au plan),
l'admission qui en découle, et l'absence de heartbeat (le plan n'a ni horloge ni intervalle de
lease). Aucun ne dit « le plan aurait dû porter ceci et ne le porte pas ».

**Conséquence, telle que l'item la formule : un worker LEP séparé.** Le `SessionPlan` est une base
suffisante ; ce qui s'y ajoute — séquence monotone, horodatage, clés d'idempotence stables par
rejeu, identité du worker, lease — appartient au **lien** et au **serveur**, pas au plan. Cette
répartition est exactement ce qu'un worker séparé sait tenir.

**Trouvaille annexe, qui ne vient pas de la question posée.** Le passage D montre un angle mort du
harnais de W0.9 : il ne vérifie ni `event.attempt` (le **rang**), ni `event.worker_id`, ni la
cohérence de `event.task_id` avec la mission. Un flux dont le rang est faux, dont le `worker_id`
est un `attempt_id` substitué — la substitution que §11.1 interdit nommément — et dont le
`task_id` désigne une autre tâche passe les huit vérifications. Ce n'est pas un défaut du plan,
c'est une dette de W0.9, et elle est reprise en `W0.9-bis` ci-dessous.

`W0.9-bis` `[R]` **les trois identités que le harnais ne regarde pas** — `event.attempt` face au
rang de la lease, `event.worker_id` face au manifeste, `event.task_id` face à la mission. Test de
sortie : le flux du passage D est refusé, et chacun des trois refus nomme l'identité substituée
plutôt que « incohérent ».

---

## Règle de session

Lire ce fichier, prendre le premier item non terminé dont les dépendances sont satisfaites, lire
le code concerné, exécuter les tests de son périmètre, modifier **ce périmètre seul**, mettre à
jour `IMPLEMENTATION_LEDGER.md`.

### La boucle, et le seul endroit où elle s'arrête

Un sprint : brancher depuis un `main` à jour, implémenter jusqu'à ce que le test de sortie passe
localement, écrire l'entrée du ledger, ouvrir la PR, attendre la CI. **CI verte et PR ordinaire :
merger, puis reprendre au premier item suivant — immédiatement, dans le même tour.**

Le bilan de sprint s'écrit **en passant**, pas à l'arrêt. C'est une ligne dans la réponse, pas un
point final : le rendre puis attendre une confirmation transforme une boucle en une suite de
sessions courtes, et c'est le contraire de ce que cette règle demande.

Trois arrêts seulement, et ils sont exhaustifs :

1. **un arbitrage qui dévie du cadre initial** — pas un détail d'implémentation dans le cadre, qui
   se tranche, se documente dans la PR, et ne bloque pas ;
2. **une CI rouge non réparée en une tentative** — une seule, et les défaillances d'infrastructure
   connues (miroir apt lent sur le job `emacs`) se réparent par un nouveau SHA à arbre identique ;
3. **l'utilisateur le demande**.

Rien d'autre. Ni « la CI est verte », ni « le bilan est écrit », ni « l'item suivant est gros ».
Un item bloqué n'arrête pas la boucle non plus : il se marque bloqué avec sa condition de levée, et
la boucle passe au suivant.

**Si un item en découvre un autre** — et c'est arrivé six fois de suite dans W5 — le nouvel item
s'écrit dans ce fichier avec son test de sortie, et **la boucle continue** ; elle ne s'interrompt
pas pour faire valider la découverte.
