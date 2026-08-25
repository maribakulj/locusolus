# ADR 0026 — Une population virtuelle, des cellules qui se justifient, et rien qui se paie deux fois

**Statut :** accepté. **Amende** `docs/13` sur l'échelle visée. Ouvre `W23`, `W24` et `W25`.

**Contexte.** Un audit externe du 2026-08-21 a inspecté au code les systèmes qui revendiquent des
populations d'agents de grande taille. Le résultat est net et il contraint la conception : **aucun ne
démontre de cognition agentique massivement concurrente.** OASIS tient un million d'objets Python
dans une liste, avec un consommateur unique et SQLite, et n'active qu'une fraction par pas de temps.
AgentSociety 2 — le seul dont le runtime soit transférable — ne tient que des identités et
reconstruit les agents depuis le disque à chaque tick. AgentPrune énumère 2N² paires et exécute
séquentiellement. G-Designer évalue sa scalabilité de 5 à 20 agents. RAPS fixe N = 5. Project Sid ne
publie aucun code.

Trois travaux indépendants établissent en outre que le **nombre effectif** de contributions
indépendantes sature très bas : `N_eff` mesuré à 1,8 sur GSM-Hard et ~1,2 sur MCQA, la même borne
retrouvée par la théorie de l'information, et l'observation industrielle de 68 000 commits produisant
70 000 conflits.

**Ce que cet ADR ne fait pas reposer sur ces sources.** Presque tout le corpus cité est constitué de
préprints récents et non répliqués, et l'audit a trouvé un bug réel dans l'un des dépôts publiés.
Aucune décision ci-dessous ne repose sur un résultat quantitatif de préprint : ce qui en est tiré est
une **lecture de code**, une méthode, ou une convergence entre sources indépendantes dont une
industrielle. Les deux chiffres qui portent réellement sont l'un mesuré en production avec test
caché, l'autre confirmé par trois méthodes distinctes — et le second, précisément, sert à **interdire
de décréter une constante** plutôt qu'à en fixer une.

---

## Décision 0 — Ce que l'audit affirmait sur les dépendances, et ce que la décision 0 de l'ADR 0022 en garde

Le document d'audit ordonnait la phase entière derrière la fermeture verticale : « `W23` attend les
écritures de `locusd`, la persistance, et une boucle de worker non inerte ». Confronté à la règle que
ce dépôt s'est donnée, cet ordre ne survit qu'en partie, et l'écart est écrit ici plutôt que corrigé
en silence.

`CLAUDE.md` et l'ADR 0022 décision 0 disent : « *Aucun appelant ne l'utilise encore* n'est pas un
motif de report ; les deux seuls motifs admis sont une dépendance technique nommée et un hôte externe
absent. » Un port avec son implémentation de référence est une **capacité finie**, pas une promesse.
`packages/event-store` a été construit exactement ainsi, avant tout écrivain.

L'ordre retenu est donc plus fin que celui de l'audit :

- **`W23.a` n'est pas bloqué.** Le port de persistance d'instance et son implémentation mémoire se
  testent contre eux-mêmes — reconstruire, comparer, condensat compris. Le reporter faute
  d'appelant serait précisément ce que la décision 0 refuse.
- **`W23.b` l'est**, et pour une raison technique nommée : le compteur `generating` compte un fait
  qu'aucun journal n'écrit encore. Les six états de `InstanceState` (§7.1) n'en portent pas
  l'équivalent. **Inventer ce fait pour avoir quoi compter** serait bâtir une fonctionnalité afin de
  justifier une métrique, ce que `W21.g` a déjà refusé sous ce nom.

  **Précisé le 2026-08-25 (`W0.19`), après un déblocage qui avait visé à côté.** Le motif ci-dessus
  était juste et trop vague, et son imprécision a coûté un faux déblocage : la ligne de `W23.b` a
  annoncé « débloqué par `W20.k` », au motif que `task.leased` ouvre et `run.completed` referme, et
  que les deux sont désormais écrits par un command handler transactionnel. **C'est exact, et ça ne
  débloque rien** : un bail nomme un **worker**, pas une `AgentInstance`. Compter `generating` sur des
  baux compterait des **machines** là où `nominal` et `active` comptent des **identités** — l'une des
  quatre confusions que cette décision existe pour interdire.

  Le fait qui manque est donc nommable : `task.assigned`, dont le champ `agent_id` est la seule source
  d'un lien instance × tâche. La projection de `W13.g` le lit déjà ; **aucun handler ne l'écrit**
  (`W20.ad`). Et le journaliser n'est pas « inventer un fait » : `Task::assigned` et `Assignment`
  existent dans `packages/coordination` depuis `W13.d`. Ce qui manque n'est pas le fait, c'est son
  producteur.
- **`W23.d`** attend des instances qui s'exécutent : c'est une **campagne**, donc la chaîne
  complète, et elle ne se teste pas sur des fixtures seules.
- ~~**`W23.c`** attend des instances qui s'exécutent.~~ **Amendé le 2026-08-24, en marquant
  `W2.21`.** Voir ci-dessous.

  **Rebloqué le 2026-08-25 (`W0.20`), et cette fois sur une dépendance technique nommée.**
  L'amendement du 2026-08-24 avait raison sur le motif — « aucun appelant ne l'utilise encore » n'en
  est pas un — et n'avait pas relu le test de sortie qu'il citait pour se justifier. Deux de ses
  trois clauses ne sont pas satisfaisables telles qu'écrites : le verbe `remplacer` de la clause 1
  n'habite pas `coordination::lifecycle` mais `crate::version`, sous le nom `REPLACE_NODE`, et
  l'en-tête du module le dit lui-même ; la clause 3 demande de comparer les **namespaces émis**, et
  le module n'en émet aucun. `W23.c` attend donc `W20.ae` — un producteur, pas un appelant, et la
  distinction est exactement celle que cette décision fait vivre.
- **`W24` et `W25` ne sont pas bloqués du tout.** L'audit les plaçait après `W20.h` sans nommer de
  dépendance ; ce sont du domaine pur et des tests d'absence, exerçables aujourd'hui.

La leçon est la même que celle de l'ADR 0025 : une affirmation sur l'état du système se vérifie,
fût-elle dans un audit qu'on suit par ailleurs.

### Amendement du 2026-08-24 — la décision 0 s'était appliquée à elle-même de travers

Cette décision existe pour **retirer** de l'ordre de l'audit tout ce qui n'était que « aucun appelant
ne l'utilise encore ». Elle a fait ce tri pour `W23.a`, pour `W24` et pour `W25`, et l'a manqué pour
`W23.c` : la formule « attendent des instances qui s'exécutent » y range ensemble deux items dont un
seul en a besoin.

Le test de sortie de `W23.c`, écrit à la même heure et dans le même document, se lit ainsi : les
quatre verbes de cycle de vie viennent de `coordination::lifecycle` et de nulle part ailleurs ;
`place`, qui vit chez `locus-execd`, reste **seul juge de l'hôte** ; une décision locale n'émet
**aucun** événement de portefeuille. Trois vérifications sur fixtures, dont deux sont des tests
d'absence. Aucune n'exige qu'une instance tourne — un ordonnanceur décide *au sujet* d'instances, il
ne les exécute pas.

`W23.c` est donc débloqué, et la mention « aucun des deux ne se teste sur des fixtures seules »
retirée : elle était fausse pour l'un des deux, et vérifiable en lisant la ligne d'à côté.

Deux choses valent d'être retenues plutôt que la correction elle-même. La première : une règle
appliquée trois fois de suite avec succès ne s'applique pas toute seule la quatrième, et c'est
précisément après les trois réussites qu'on cesse de regarder. La seconde : ce qui a fini par lever
l'erreur n'est pas une relecture mais le garde de `W0.16`, qui a exigé qu'un marqueur périmé soit
réexaminé. Une affirmation fausse **inerte** peut vivre indéfiniment dans un document ; celle-ci a
été rattrapée parce qu'un outil avait une raison mécanique d'y revenir.

---

## Décision 1 — « Supporter N agents » est défini, et ce n'est pas N générations concurrentes

Le dépôt s'engage sur une définition unique, écrite dans `docs/13` :

> **N identités** capables de mémoriser, de recevoir des événements, d'être ordonnancées et de
> participer à une campagne, dont un sous-ensemble variable raisonne concurremment.

Trois compteurs distincts sont exigés partout où une taille est rapportée : `nominal`, `active`,
`generating`. Un rapport qui n'en donne qu'un est refusé.

**Motifs.** Le mot « supporter » recouvre au moins quatre choses incompatibles — identités stockées,
objets en mémoire, agents simulés dans le temps, acteurs concurremment actifs — et l'audit montre que
la confusion est la norme du champ. Un dépôt dont la discipline est de distinguer « une sonde non
exécutée » d'un échec ne peut pas se permettre ce flou.

---

## Décision 2 — `AgentInstance` est une identité durable ; l'exécution est un emplacement

Une instance existe indépendamment de tout processus. Elle est **reconstruite** depuis son état
persisté au moment où on l'exécute, et **rejetée** ensuite ; aucun objet d'agent ne traverse une
frontière de processus.

**Motifs.** C'est le seul modèle de virtualisation qu'un dépôt vérifié implémente réellement, et sa
propre docstring l'énonce : le driver ne tient que des enregistrements — spécifications et
identifiants —, jamais des objets d'agent, et rien de tel ne traverse la frontière de processus.

**Et ce dépôt en a davantage que les deux tiers.** `packages/coordination/src/agent.rs` porte déjà
`AgentInstance` **sous son nom**, avec `Id<Agent>` pour identité, les six `InstanceState` de §7.1,
`provision`, `moved_to` et ses transitions refusées ; `Task` porte `assigned_agent_id` ;
~~`coordination::lifecycle` journalise les transitions ; `W21.j` mesure déjà la durée de vie d'une
instance à partir d'elles.~~ Ce qui manque est le **port de persistance d'état d'instance** et le
protocole de reconstruction — et rien d'autre.

**Corrigé le 2026-08-25 (`W0.20`).** La phrase barrée ci-dessus est **fausse**, et elle l'était à
l'écriture. Vérifiée trois fois plutôt qu'une :

- `coordination::lifecycle` **n'émet aucun événement**. Il rend un `Outcome` — `Spawned`,
  `Suspended`, `Draining { remaining }`, `Killed { abandoned }` — et rien de plus.
- **Aucun crate hors `packages/coordination` ne l'importe** : ni `locusd`, ni `packages/projections`.
  Ses seuls consommateurs sont `messaging.rs` et `transfer.rs`, dans le même crate.
- `W21.j`, cité ici comme lecteur, reçoit ses instants **en données** — il ne lit aucune transition,
  et ne saurait pas où en lire.

Le module est donc une machine à états de domaine, correcte et éprouvée, **dont les décisions ne
sortent jamais**. C'est la même forme que le paragraphe précédent décrit pour `task.assigned` : le
fait existe dans le domaine, son **producteur** manque. Le producteur est `W20.ae`, et il débloque
`W23.c` — dont la clause 3 demandait de comparer des namespaces émis par un module qui n'émet rien.

Ce que la correction change pour la décision elle-même : rien. « Ce qui manque est le port de
persistance et rien d'autre » restait vrai **pour `W23.a`**, qui est ce que cette décision ordonnance ;
la phrase fausse portait sur ce qui était déjà là, pas sur ce qui restait à faire. C'est précisément
pourquoi elle a survécu : une affirmation inexacte sur l'**acquis** ne fait rougir aucun test, et
personne n'a de raison mécanique d'y revenir — sauf l'item d'après, qui s'appuie dessus.

**Le port, pas le backend.** `AgentStateStore` est un trait avec une implémentation de référence en
mémoire, exactement comme `packages/event-store` l'a fait. L'audit donne la raison de ne pas figer un
backend : le seul système vérifié persiste un répertoire par agent, ce qui tient à 10 000 et charge
lourdement la couche de métadonnées du système de fichiers à 100 000.

**Et le port n'a pas de variante « peu importe l'état ».** Reconstruire sans savoir depuis quelle
révision n'a pas de sens, et `Expected` de l'event store — `NoStream`, `Exact` — le dit déjà pour
l'écriture. La lecture hérite de la même exigence.

---

## Décision 3 — Une cellule n'existe que si elle porte une frontière déjà exigée ailleurs

Une `Cell` est un regroupement borné qui porte **son propre budget, son propre périmètre de
`ContextView`, sa propre enveloppe de politique et son propre ordonnanceur local**. Elle n'est pas un
artefact d'échelle : elle est la co-implantation de bornes qui existent déjà séparément.

**Motifs.** Une cellule qui ne serait qu'un regroupement de commodité serait de la sémantique inerte
au sens de l'ADR 0016 décision 4. Elle se justifie si et seulement si elle évite un aller-retour au
plan de contrôle global pour une décision locale — réveiller un agent trente secondes ne doit pas
remonter au portefeuille, consommer cinquante heures-GPU doit remonter.

**Et la taille ne se décrète pas.** Les résultats sur `N_eff` interdisent de fixer une taille de
cellule par doctrine. Elle est **mesurée** par `W23.d`, et le dépôt a déjà l'outillage : `R2` attribue
le crédit, `R3` calcule le regret, `W21.f` mesure l'entropie de degré, `W21.g` et `W21.h` le chemin
critique et le parallélisme moyen.

---

## Décision 4 — Le routage par intention route **parmi des pairs déjà autorisés**

Un mécanisme de publication-souscription sémantique peut sélectionner un destinataire dans un
ensemble autorisé. Il ne détermine **jamais** l'autorisation. Une souscription est **dérivée de la
`ContextView`**, jamais déclarée par l'agent.

**Motifs.** C'est la borne que la source omet et qui la rend inutilisable telle quelle : chez elle,
la souscription « s'aligne naturellement sur le *system prompt* configuré quand chaque agent rejoint
le réseau » — c'est-à-dire que l'agent déclare lui-même ce qu'il veut recevoir. Transposé ici, cela
ferait négocier aux agents leur propre accès à l'information, ce qui casse §12.4 (isolation de
branche), l'invariant 11 (aveuglement du reviewer) et §16.6 (contaminations) d'un seul geste.

**Note d'implémentation à ne pas perdre.** Le modèle de réputation de la même source est
**inutilisable en l'état** : `s^F = 1` signifie *faute*, donc `E[P]` est une probabilité de mauvais
comportement et l'algorithme filtre `E[P] < τ` — mais `T` est de polarité inverse, admissible si
`E[T] ≥ τ`, sur la même machinerie Beta et la même règle de mise à jour. Deux conventions opposées
dans un même mécanisme. Transcrire cela sans le résoudre produirait un filtre inversé silencieux, et
c'est ce que `W24.c` existe pour rendre impossible.

---

## Décision 5 — Le journal reste l'unique chemin durable ; aucun bus éphémère n'est créé

La messagerie demeure ce que l'ADR 0019 en a fait : un **usage du journal**. Aucun second stockage
durable, aucun bus parallèle.

**Motifs.** La proposition d'un « bus éphémère » pour les messages sans effet durable est le Mutation
Bus par une autre porte. Et la preuve industrielle va contre elle : l'organisation qui gère le plus
gros volume de coordination agentique connu n'a pas sorti la coordination de son substrat versionné,
elle l'y a fait **entrer** — chaque changement passe par le VCS, donc c'est là que les collisions
deviennent visibles, et plusieurs mécanismes de coordination y sont implémentés directement.

Ce qui **est** admis, et qui existe déjà : un message volumineux entre comme artefact et le journal
en porte la référence et le condensat (§9.1, `packages/artifacts`).

**Le partitionnement du journal n'est pas un item.** `packages/event-store` n'a jamais eu d'ordre
total global : `Expected` est par stream. Il n'y a rien à démonter. La sérialisation que l'audit a
réellement observée — consommateur unique, `max_concurrency=1` — est une sérialisation de **runtime**,
et elle vise `locusd` : c'est `W20.h`, et le code de `locusd` la nomme déjà lui-même, « sérialiser les
écritures — verrou, file, acteur — est une décision qui mérite son item ».

---

## Décision 6 — Le modèle est une dimension d'ordonnancement, pas une constante

Une mission déclare une **classe de cognition** et non un modèle. L'affectation modèle → classe est
une valeur de politique, versionnée et visible, jamais une constante de code.

**Motifs.** C'est la mesure la plus actionnable du dossier, et elle est industrielle : à qualité
identique vérifiée par test caché, un facteur 7,9 sur le coût total et 22 sur la flotte de workers
seule. Le levier n'est pas le modèle, c'est **l'affectation** — frontière pour planifier, bon marché
pour exécuter.

Le dépôt a déjà `packages/budget` avec les six dimensions de §7.2 et, depuis `W21.m`, la
classification de dépense qui distingue `Coordination`, `Work` et **non classé**. Ce qui manque est
la dimension de cognition et son plafond. Et la règle de `W21.m` s'y applique telle quelle : une
dépense non classée n'entre dans aucun plafond, elle ne devient pas « travail » par défaut.

---

## Décision 7 — Ce qui n'est pas construit, et pourquoi

**Aucune fabric d'inférence propriétaire.** Le cache de préfixes partagé et la désagrégation
prefill/decode sont réels et mesurés — 75 % de requêtes en plus sous SLO, intégrés à deux moteurs de
service majeurs. Mais c'est une **capacité admise** au sens de `W18.d`, derrière un `Published`, pas
un sous-système du dépôt. Locus Solus l'ordonnance ; il ne la réimplémente pas, exactement comme
`W18.h` a admis le raisonneur d'ontologie.

**Aucun optimiseur de topologie global.** L'audit est sans appel : 2N² candidats et exécution
séquentielle d'un côté, matrice dense et scalabilité évaluée à 20 agents de l'autre, et l'un des deux
a un chemin synchrone cassé. Ce sont des outils de **région**, au sens de `coordination::region`,
dans la boucle lente de `W18`.

**Aucun ordonnanceur d'agents dans `locusd`.** Le placement d'hôte existe déjà, et il est **chez
`locus-execd`** — `apps/locus-execd/src/placement.rs::place` choisit sur ce qu'un hôte a *prouvé*.
Ce qui manque est un cran au-dessus, côté instances, et c'est `W23.c` : il lit `coordination::lifecycle`
et ne choisit **jamais** d'hôte, sans quoi la troisième frontière du dépôt tomberait par le haut.

**Aucune taille de cellule décrétée.** Voir décision 3 : `W23.d` mesure avant que `W23.e` construise,
et un résultat qui ne tranche pas est rendu comme tel plutôt qu'arrondi vers l'hypothèse de départ.

---

## Conséquences

`docs/13` reçoit la définition de la décision 1 et cesse de parler d'une taille sans dire de quoi.
Trois workstreams s'ouvrent — `W23`, `W24`, `W25` — dont l'ordre interne est celui de la décision 0
et non celui du document d'audit. `W23.d` précède `W23.e` : on mesure si la collaboration bat la
réexécution indépendante **avant** de construire la machinerie qui suppose que oui.

Et si les branches gagnent cette mesure, `W23.e` et une partie de `W24` deviennent inutiles — §18 et
la qualité-diversité de §13.3 deviennent alors la contribution du projet, parce qu'elles sont un
mécanisme de **fabrication d'indépendance causale**, ce que personne dans le corpus ne formalise.
C'est un résultat acceptable, et il est écrit ici pour qu'il ne se lise pas comme un échec.

## Plan de rollback

Les décisions 0, 1, 4, 5, 6 et 7 sont documentaires ou négatives et se retirent par un diff. La
décision 2 introduit un port avec implémentation mémoire : le retirer coûte la suppression d'un
module tant qu'aucun backend externe n'existe. La décision 3 est la seule dont le rollback coûte une
garantie plutôt qu'un diff, et c'est pourquoi la taille de cellule est mesurée et non décrétée : tant
que `W23.d` n'a pas rendu, il n'y a rien à défaire.
