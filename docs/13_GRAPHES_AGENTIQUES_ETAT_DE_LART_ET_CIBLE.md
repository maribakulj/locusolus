# Graphes agentiques : état de l'art vérifié et cible

**Statut : document subordonné.** Il vient après `SPEC_V1.md` dans l'ordre de priorité de
`START_HERE_CLAUDE.md` et n'entre pas dans la chaîne d'arbitrage : ce qui décide est
`docs/adr/0016`, ce qui planifie est `docs/10`. Ce document porte le **raisonnement**, ce qui permet
à l'ADR d'être court et empêche la cible de vivre hors du dépôt.

Sur le statut des sources citées ici, lire §6 avant de s'appuyer sur un chiffre.

---

## 1. La réorientation, en une page

### Ce que le projet n'est pas

Ce n'est pas « ajouter des graphes dynamiques à Locusolus ». Le champ compte une vingtaine de systèmes
qui font de la topologie agentique dynamique. Être le vingt-et-unième n'a aucun intérêt.

### Ce que le projet est

`SPEC_V1.md` §36 le dit déjà : un système d'exploitation de recherche orchestrant durablement des
sociétés de chercheurs humains et artificiels au-dessus d'un graphe épistémique versionné. Ce que la
présente réorientation ajoute est une propriété que **personne d'autre n'a** :

> L'organisation, les capacités et une partie du runtime peuvent évoluer pendant le fonctionnement,
> avec ou sans supervision selon le mode déclaré, et **les transformations de l'appareil de recherche
> ont elles-mêmes une provenance, une justification et une histoire critique** — c'est-à-dire qu'on
> peut objecter à une décision de réorganisation comme on objecte à un claim.

La preuve que c'est le bon positionnement est quantitative. Le tableau comparatif de la revue de
littérature recense vingt systèmes sur neuf critères. Sur la colonne « validation/sûreté explicite »,
les valeurs sont : faible, limitée, contraintes du modèle, exécutabilité, espace contraint, espace de
pruning, supergraphe, objectifs contraints, DAG, sparse routing, validité + budget, budget d'édition,
bounded updates, acceptation locale, validation pairée — et **une seule mention « très forte »**, pour
ATM (`arXiv:2607.20488`). La lecture par familles de la même revue le confirme : la famille « hot
reconfiguration sûre » a un seul membre, et l'auteur note que « c'est probablement là que se trouve le
vrai problème d'ingénierie que la plupart des frameworks agentiques actuels n'ont pas encore
sérieusement résolu ».

Le champ sait muter des graphes. Il ne sait pas les gouverner. C'est le trou, et Locusolus a déjà
l'event store, la provenance, la revue, les budgets et les objections pour le combler.

### L'échelle sur laquelle se mesurer

La revue distingue cinq degrés de dynamisme, et c'est la seule échelle du corpus qui soit à la fois
précise et mesurable. **Chaque workstream déclare le niveau qu'il fait franchir, et sur quelle sorte
de relation.** Un workstream qui ne sait pas le dire n'a pas de critère de fin.

| Niveau | Ce qui change | Où le champ en est | Locusolus |
|---|---|---|---|
| 0 | routage conditionnel dans un graphe écrit d'avance | tout framework qui se dit « dynamic » | dépassé par construction |
| 1 | sélection ou pruning d'un super-graphe fixe | AgentDropout, AGP, MaAS | insuffisant : ne crée rien |
| 2 | génération d'un graphe par requête, figé pendant l'exécution | ARG-Designer, GTD | insuffisant : figé pendant la mission |
| 3 | reconfiguration entre rounds, sur feedback | AgentConductor, MetaGen, SelfOrg, DyTopo | **atteint quand W13.e passe son test de sortie**, sur la relation de revue |
| 4 | mutation structurelle live : spécialisation, factorisation, recâblage, reroutage d'état, livraison consciente de la version | ATM seul, partiellement | **la cible**, sur toutes les sortes de relation |

**Le niveau 4 ne signifie pas muter un step en cours d'exécution.** La revue a vérifié la
documentation officielle de Langflow et de n8n : les deux exposent une API CRUD complète sur les
définitions de workflow et un assistant capable de les construire, et **ni l'un ni l'autre ne
documente une sémantique de hot-swap atomique de l'instance en cours**. Le motif réel est
`exécuter G1 → observer → patcher → valider → sauver G2 → reprendre à l'étape suivante avec G2`,
l'alternative étant qualifiée par l'auteur de la revue de « type de monstruosité que les humains
découvrent après avoir baptisé la première MVP ». Le niveau 4 est donc : **livraison de message
consciente de la version, plus des frontières d'activation sûres.** Cette borne n'est pas une
prudence, c'est l'état de l'art industriel.

---

## 2. Le constat décisif : la spec couvre déjà l'essentiel

C'est le résultat le plus important de l'analyse, et il change ce qu'il y a à faire. Une analyse
antérieure, ainsi qu'un dossier de recherche externe, ont proposé d'inventer un vocabulaire de
gouvernance — `MutationPolicy`, `MutationGrant`, `AgentAuthorityMode`, `TopologyPatch`, un
`Mutation Controller`, une échelle d'autorité à cinq barreaux. **Tout cela existe déjà dans
`SPEC_V1.md`, sous d'autres noms, et n'est pas implémenté.** Le dossier externe ne citait jamais
§7.1, §13, §16 ni §22 : c'est la cause racine de l'intégralité de ses doublons.

| Section | Ce qu'elle donne, vérifié | Conséquence |
|---|---|---|
| **§7.1** | Schéma complet, champ par champ, de `AgentTemplate`, `AgentInstance`, `Team`, `Decision`, `ApprovalRequest` | C'est la source normative de W13.c, pas §14.2 qui n'en est qu'une esquisse. `AgentTemplate` porte `prompt_overlay_ref`, `tool_policy_id`, `sandbox_profile_id`, `memory_policy_id`, `review_independence_group`. `AgentInstance` porte `context_view_id`, `private_memory_id`, `model_identity`, `independence_group`. `Team` porte **`coordination_mode` comme champ** avec les cinq valeurs, plus `coordinator_id`, `information_sharing_policy_id`, `review_independence_policy_id` |
| **§7.1 `Task`** | `assigned_agent_id` **et** `assigned_worker_id` | Le graphe organisationnel réalisé est dérivable **sans toucher LEP**, par jointure côté serveur |
| **§13.5** | Actions du portefeuille : « composer ou renforcer une équipe », « créer un agent-pont », et « les actions dépassant les seuils de coût, de confidentialité ou d'impact exigent une `ApprovalRequest` » | **L'organe de décision existe.** Ce n'est pas un contrôleur à inventer |
| **§13.2, §13.4** | Quinze indicateurs par branche, fonction de valeur `V(b)` à neuf termes | Le vocabulaire de justification d'une mutation existe |
| **§13.6** | Anti-gaming, dont « production de tâches pour maximiser l'activité », inflation de confiance, collusion de reviewers | **Le mode autonome dépend de l'anti-gaming**, et `docs/10` §W7 avertit déjà que « l'anti-gaming du portefeuille doit exister avant que la fonction de valeur pilote des décisions automatiques » |
| **§14.5** | Onze déclencheurs nommés, et la structure d'une proposition de spawn avec `reason`, `missing_capability`, `expected_information_gain`, `cost_estimate`, `time_to_live`, `termination_condition` | Écrit **pour l'auteur agentique** : les onze déclencheurs sont des faits que l'agent observe |
| **§20.2, §20.4, §20.5** | Cinq verbes `allow`/`deny`/`modify`/`require_approval`/`require_tasks`, trace d'évaluation, déterminisme, `Delegation` avec `scope`/`budget_ceiling`/`confidentiality_ceiling`/`revocable`, et explicabilité incluant les **alternatives rejetées** | Les modes d'autorité sont des configurations, pas une machinerie |
| **§22.2** | `CommandEnvelope` avec **`expected_revision`** | Le CAS est normatif au niveau API |
| **§22.3** | `agent.spawn`, `agent.terminate`, `team.create`, **`team.modify`**, `decision.propose`, `decision.approve`, `approval.respond` | La surface de commandes existe |
| **§22.4** | `GET /teams/:id`, `GET /agents/:id`, `GET /branches/:id/graph`, **`GET /branches/:id/diff`** | Le diff est déjà un endpoint prévu |
| **§9.3** | Projections **obligatoires**, dont « graphe des équipes et agents » et « index mémoire hybride » | Les deux projections en question sont déjà exigées par la V1 |
| **§9.4** | Requêtes obligatoires, dont « vue temporelle telle qu'elle était connue à la date T » et « comparaison structurelle de deux branches » | Navigation temporelle et diff structurel déjà exigés |
| **§12.1** | Classe de tâche `review-isolated` — « contexte aveugle et lecture seule » | **Troisième** consommateur de la relation de revue |
| **§12.3** | Un résultat tardif est « stocké en quarantaine et ne peut committer sans arbitrage » ; « une tâche réattribuée conserve le numéro d'attempt » | Plus strict que la note `late-results` de `features.json` |
| **§16** | Sept niveaux de mémoire ; retrieval hybride à dix facteurs ; `ContextView` immuable et hashée ; interdiction de fusion automatique ; cinq contaminations à empêcher | Voir §4 |
| **§23.3** | Visualization Projection Service produisant des projections « versionnées et hashées », dont une vue **« société d'agents »** | La vue LIVE du cockpit est déjà une projection V1 |
| **§33** | « Rendre toute action autonome sans seuil humain » = **non-objectif de la V1** | Le mode fermé n'est pas une option : c'est une exigence |

### Ce qui manque réellement

Trois choses, et trois seulement.

1. **La relation de coordination deux à deux.** §14.3 et `Team.coordination_mode` ne donnent que des
   modes *globaux*, applicables à une équipe entière. Aucun objet ne dit « cet agent-ci révise cet
   agent-là ».
2. **L'objet de proposition structurelle versionné.** §13.5 dit *ce que* le portefeuille peut faire ;
   il ne dit pas *comment* une action structurelle est proposée, versionnée, validée et annulée.
   §14.5 donne la proposition de spawn — l'ajout d'un nœud — sans son symétrique, la modification
   d'une relation.
3. **La contestabilité d'une décision de coordination.** `Decision` (§7.1) porte `rationale`,
   `evidence_refs`, `policy_evaluation_id` et `overrides` — la moitié du chemin. Ce qui manque est la
   famille d'objection permettant de contester le déclencheur, la politique ou le périmètre.

**Tout le reste du travail est de l'implémentation de spec, pas de la conception.**

---

## 3. La cible complète, composant par composant

Ce qui suit est de la V1 : `docs/10` ligne 3 dit « ce n'est pas une roadmap de MVP ; tous les
workstreams appartiennent à la V1 finale ; l'ordre reflète les dépendances de construction ».

**Le graphe organisationnel versionné.** Nœuds : `AgentInstance`, et plus tard vérificateurs,
routeurs, mémoires, outils. Relations typées, l'énumération s'ouvrant **une valeur à la fois** selon
la règle « aucune sémantique inerte » (§7, décision 4). Version canonique immuable avec hash et
parent, graphe réalisé comme projection, trace comme histoire — la taxonomie template / réalisé /
trace vient du survey `arXiv:2603.22386` §2.2 et doit être citée comme état de l'art, pas comme
invention du projet. Le quatrième graphe, l'épistémique, est ce qui est propre à Locusolus. Le jeu
d'opérations cible, tiré de la revue : `ADD_NODE`, `REMOVE_NODE`, `REPLACE_NODE`, `ADD_EDGE`,
`REMOVE_EDGE`, `SPLIT_NODE`, `MERGE_NODES`, `SET_ROLE`, `SET_VISIBILITY`, `SET_VALIDATOR`,
`SET_EXECUTION_ORDER`.

**Le chemin de mutation.** Quatre origines de proposition — humain, agent, télémétrie, politique —
convergeant vers un objet unique, puis validation statique, décision du moteur de politique et du
portefeuille, simulation ou ombre facultative, approbation si nécessaire, commit atomique par
`expected_revision`, annulation par commit inverse. Le pipeline `PROPOSE → VALIDATE → SHADOW →
COMMIT → OBSERVE` est celui de la revue ; le commit est le command handler transactionnel de §9.2.

**Les régions mutables.** GRAFT (`arXiv:2608.02353`) donne la structure exacte, et la fonde sur une
nécessité de **coût** et non sur une préférence de design : région déclarée, critère d'acceptation
local, veto de cohérence globale, parce que la ré-optimisation globale à chaque incident est
prohibitive. Chaque région porte `allowed_ops`, `risk_ceiling`, `max_nodes_delta`, `max_edges_delta`,
`approval_mode`, `require_shadow`.

**Le moteur de politique et le portefeuille.** §20 en entier, plus §13. C'est le plus large trou du
dépôt et il est déjà normatif. Sans lui, pas de classe de risque dérivée, pas de mode `bounded`, pas
d'anti-gaming, et la moitié de W7 reste bloquée.

**Le scheduler dynamique.** La revue donne la liste, et elle est plus longue que ce que W4.g prévoit :
spawn, suspend, drain, kill, replace, split, merge, connect, disconnect, rerouter l'état, rejouer,
migrer le contexte, et **livrer les messages en connaissance de la version**. C'est là que le niveau 4
se gagne ou se perd.

**L'admission de capacité.** Un agent constate qu'il lui manque un outil, propose une capacité ; elle
est construite, scannée, signée, attestée, testée en ombre, approuvée, puis disponible, et le graphe
se recâble vers elle. La capacité arrive comme **`EnvironmentBlueprint`** (§19.3), jamais comme code
injecté dans un processus. Locusolus a déjà le blueprint (W5), l'artefact et son manifeste (W6),
l'attestation et le refus nommant toutes ses conditions (W4.a, W4.c). Ce qui manque est la
proposition, la politique et l'approbation : du travail de gouvernance, pas d'injection. C'est
l'héritier honnête de ce que les runtimes réflexifs appellent « plugins ».

**Le cockpit.** Quatre vues — plan, vivant, trace, épistémique — avec sélection synchronisée par une
identité unique, qui est `Id<Agent>` et `actor.principal_id`. Le canvas produit une **commande**,
jamais une écriture. Diff calculé une fois côté serveur, donc identique dans Emacs et dans le web —
sinon l'approbation porte sur ce que chaque client a cru voir. Preview statique, ombre, approbation,
rollback, navigation dans le temps comme propriété du pli et non comme fonctionnalité. La revue
établit qu'**aucun** outil existant ne combine canvas, graphe comme état mutable de première classe,
mutations proposées par les agents, commit atomique et invariants : AgentCoord fait la visualisation,
Langflow le canvas, aucun la gouvernance. Langflow n'est pas retenu comme base : `docs/01`, ADR 0006
et §23.3 exigent Emacs cockpit, web pour le rendu riche, et des projections versionnées et hashées
qu'un canvas tiers ne produit pas. On garde l'idée que le canvas est une **vue**, pas la vérité.

**L'adaptation automatique.** Boucle rapide sur la **capacité** — routage de modèle, choix d'outil,
sélection de skill, retry, routes éphémères ; boucle lente sur la **structure** — nœuds, relations,
rôles, visibilité, ordre. TacoMAS (`arXiv:2605.09539`) montre « empiriquement et théoriquement » que
les deux échelles de temps doivent différer, et dans ce sens précis. Un spawn est une addition de
nœud, donc lent — cohérent avec le passage obligé par la politique de §14.5.

**Le plan de simulation.** Rejeu déterministe, puis substitut d'environnement enregistré, puis
éventuellement world model appris, puis ombre en sandbox réelle, puis canari facultatif. ATM a validé
ses trois invariants sur 720 exécutions avec des **stubs déterministes**, plus une petite sonde à
outils réels comme contrôle de validité externe : c'est l'ordre honnête, et le stub apporte la plus
grande part de la valeur pour une fraction du coût.

---

## 4. Mémoire, RAG et graph RAG

La réponse est que **l'essentiel est déjà normatif, et plus strict que la plupart des systèmes RAG
industriels**, et que le point intéressant est ailleurs.

### Ce qui existe

§16.1 définit **sept niveaux** : mémoire privée d'agent, d'équipe, de branche, de workstream, de
programme, inter-programmes, disciplinaire. C'est l'orchestration multi-niveaux demandée.

§16.3 définit un retrieval hybride combinant **traversée de graphe**, recherche lexicale, recherche
vectorielle, identifiants exacts et citations, temporalité, niveau de validation, branche et
confidentialité, diversité des sources, résultats négatifs, budget de contexte. C'est du graph RAG.
Deux clauses le rendent plus rigoureux que l'usage courant : « le ranking **DOIT** exposer ses
facteurs » et « les embeddings **ne peuvent pas** contourner les ACL ». La seconde est la faille
classique de tout RAG d'entreprise ; elle est interdite ici par écrit.

§9.1 range vecteurs, index plein texte, vues matérialisées, **graph databases** et caches du côté des
projections reconstructibles. §9.3 fait de l'« index mémoire hybride » une projection **obligatoire**.
§16.2 donne le schéma complet de `ContextView` : `included_types`, `included_relations`, `max_depth`,
`time_range`, `branch_scope`, `validation_levels`, `confidentiality_ceiling`, `artifact_policy`,
`negative_result_policy`, `diversity_policy`, `token_budget`, `redactions`, `source_event_watermark`,
`content_hash`. §16.4 interdit la fusion automatique de quasi-duplicats. §16.5 impose qu'une
compaction signale ce qui a été omis et ne transforme jamais un objet non validé en connaissance
établie.

### Le point qui change l'architecture

**La mémoire est la substance de ce qu'une mutation de coordination déplace.** `AgentInstance` porte
`private_memory_id` et `context_view_id` ; `Team` porte `information_sharing_policy_id`. Recâbler une
relation change qui peut lire quoi. Le `state-routing completeness` d'ATM — chaque fragment d'état a
une destination autorisée ou un abandon journalisé avec sa raison — porte précisément sur des atomes
mémoire. Et le `content_hash` de `ContextView` est ce qui rend l'opération vérifiable.

Le graphe de coordination **est** le graphe de circulation de la mémoire. C'est exactement ce que MANTA
(`arXiv:2607.28527`) appelle « visibilité de l'information » et « chemins de validation » quand il dit
que la topologie n'est plus une matrice d'adjacence. La convergence entre §16 et MANTA est totale.

Conséquence : la sorte de relation `visibility` n'est pas une arête décorative en attente d'un bus de
messages. Son consommateur est la construction de `ContextView`, qui existe. C'est probablement la
**troisième** sorte à implémenter, après `review` et `role`.

### Ce qu'il faut ajouter

**La détection du consensus circulaire.** §16.6 exige d'empêcher « le consensus circulaire où des
agents se citent mutuellement sans source externe », sans dire comment. C'est une propriété de graphe
calculable : un cycle de `Cites` dont aucun nœud n'a d'arête `AnchoredIn` vers une source externe.
`packages/graph` porte déjà `Cites` et `AnchoredIn` parmi ses vingt-huit relations, et
`minimal_premise_sets` pour les hyperarêtes. Aucun système RAG ni MAS recensé par la revue ne le fait.
C'est publiable seul, c'est bon marché, et ça ne dépend ni de W4 ni de `locusd`.

**La contamination comme invariant vérifié au recâblage.** Aujourd'hui §16.6 est une exigence prose.
Elle devient une garde : une proposition qui créerait une route donnant au reviewer aveugle accès au
raisonnement du générateur est refusée avant validation sémantique.

**La séparation des deux retrievals.** Interroger le graphe épistémique et interroger le graphe de
coordination sont deux opérations différentes, et les mélanger est le mode de défaillance de §16.6.
Deux index, aucune conversion : la frontière de l'ADR appliquée à la mémoire.

### Ce qu'il faut refuser

Un vecteur ou un graph store comme source de vérité — §9.1 en fait des projections. Un « agent
mémoire » propriétaire — la possession est distribuée sur sept niveaux, et un propriétaire unique
serait un second chemin d'écriture. La fusion automatique de quasi-duplicats — §16.4 l'interdit. Un
RAG dont le ranking est opaque — §16.3 l'interdit.

---

## 5. Ce qui est écarté, et la raison mécanique de chaque écart

La distinction est essentielle : **« pas maintenant » est un ordonnancement et se justifie par une
dépendance ; « pas prévu » est un renoncement et exige une raison.** Trois choses seulement sont
écartées.

**Le bus de mutation.** `CLAUDE.md` : toute mutation passe par un command handler transactionnel, et
§9.2 fixe la chaîne `Command → autorisation → invariants → événements → outbox`. Un bus serait ce
handler renommé, ou un second chemin d'écriture — ce qu'interdisent l'invariant 3 et la règle 3 de
`boundaries.json`. Et un état tenu en mémoire dans un bus ne survit pas à un crash, alors qu'une
transition écrite comme commande y survit gratuitement.

**Un instantané de graphe réalisé transmis par le worker.** Si le worker envoie un instantané *et* des
événements, les deux peuvent diverger et rien ne dira lequel est vrai. C'est ADR 0013 appliqué au fil :
la topologie réalisée se dérive, elle ne se transmet pas.

**L'injection de code dans un processus non confiné.** Le harnais tiers étudié documente lui-même que
son sandbox de packages dynamiques « isole les globals mais n'est pas une frontière de sécurité », que
« des helpers du realm hôte rendent l'échappement possible », et qu'il faut « traiter ce toolset comme
un accès bash ». Si c'est équivalent à bash, le problème est déjà résolu par `locus-execd` en S3/S4, et
l'habiller en réflexion de runtime ajoute une surface sans acheter d'isolation. La forme retenue est
l'admission de capacité par `EnvironmentBlueprint`.

**Tout le reste est gaté, avec une dépendance technique nommée :** ~~epochs et transfert d'état~~
**dégaté par l'ADR 0019** — la messagerie inter-agents est un **usage du journal**, pas un transport
parallèle, et un epoch est la `Version` de configuration qui existe déjà. Le reste de la clause reste
vrai et n'est pas amendé : l'unité de concurrence demeure l'attempt, déjà versionné, leasé, idempotent
et acquitté par séquence, et un message ne remplace ni un lease ni un attempt — il les accompagne ;
migration d'état en cours de mission (le `content_hash` de
`ContextView` est obligatoire et l'enveloppe est immuable — la règle V1 est : nouvel attempt, nouvelle
vue, nouveau hash ; un protocole ultérieur avec agents persistants pourra rouvrir la question) ;
exécution en ombre (exige un driver de sandbox, W4.d–e) ; cockpit (exige `locusd`, la persistance et
W9) ; mode `bounded` (exige le moteur de politique et l'anti-gaming de §13.6) ; world model (exige un
substitut déterministe d'abord) ; harnais tiers (prototype externe jetable, une seule question
falsifiable).

**Sur le harnais tiers, un fait à connaître.** Au commit `99f6f02` (`release/dsh-0.1.0-rc.7`), trois
nomenclatures d'outils coexistent dans le même arbre : `src/index.ts` enregistre sept noms, le
`README.md` du même package en annonce cinq avec une décomposition différente, et un troisième nom
circule dans le client, les tests et le texte pédagogique du runner — lequel instruit le modèle
d'appeler un outil que rien n'enregistre sous ce nom. Le README annonce par ailleurs en majuscules
« THERE WILL BE COMPATIBILITY-BREAKING CHANGES ». Aucune dépendance, aucun couplage.

**Deux mécanismes de ce harnais sont néanmoins à retenir comme principes**, sans dépendance : le
protocole d'activation à deux pointeurs (`current` / `next`, promotion après succès complet, ancienne
version conservée sur échec, révision adressable exactement) ; et le registre d'invariants possédés par
package, où chaque paquet publie un compagnon et où **l'absence d'invariant est une déclaration
motivée** vérifiée mécaniquement.

---

## 6. Statut des sources

Ce document cite un corpus dont la solidité est inégale, et l'ADR qui en découle ne repose sur aucun de
ses chiffres. Les limites sont reprises ici pour qu'elles ne restent pas hors du dépôt.

**Vérifiées par consultation des pages arXiv :** `2603.22386` (survey), `2607.20488` (ATM),
`2607.28527` (MANTA), `2605.09539` (TacoMAS), `2602.06039` (DyTopo), `2608.02353` (GRAFT),
`2608.08605` (ForestBench). **Non vérifiées :** `2602.17100` (AgentConductor), `2608.07196` (EMAS) —
rien ici n'en dépend.

**L'attribution de « Graph Determination Time » et « Graph Plasticity Mode » à Yue et al. n'est pas
vérifiée** : la table des matières du survey nomme « Structure determination » et trois dimensions
différentes. Les concepts sont utilisables ; les citer comme terminologie de ces auteurs demande une
vérification dans le texte avant publication.

Presque tout le corpus est constitué de préprints de moins de six mois, non répliqués. Les seules
références relues et publiées sont les fondations — K-Component (2001), Autonomic Computing (2003),
Rainbow (2004), Runtime Software Adaptation (2008), Models@run.time (2009) — et c'est de la dernière
que vient le critère causal de la décision 1 d'ADR 0016. **Aucune décision ne repose sur un résultat
quantitatif de préprint.** Ce qui en est tiré est du vocabulaire, une méthode et une structure
d'argument. En particulier, les chiffres d'ATM ne sont pas utilisés : sa baseline de 3,3 % sur tâche de
code est une configuration cassée, et sa métrique de confidentialité est mesurée par un classifieur
regex, donc elle mesure l'exposition *détectable*.

**Non lu au moment de la rédaction :** `SPEC_V1.md` §18 (branches, forks, fusions), §19 au-delà de
§19.3, §24 à §31. **§18 est le plus susceptible de contredire quelque chose ici** : la coordination
d'une branche interagit avec le fork, le merge et le rebase, et §18.4 pourrait imposer des règles sur
ce qu'une fusion fait d'une équipe. À lire avant W13.e.
