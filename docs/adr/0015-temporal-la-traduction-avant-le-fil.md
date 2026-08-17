# ADR 0015 — Temporal : la traduction d'abord, la liaison au fil quand elle sera possible

**Statut :** accepté, avec un point d'arbitrage laissé ouvert et nommé. Met en œuvre
`docs/SPEC_V1.md` §11.1. Concerne `packages/workflow-backends` (W3.d). Prolonge l'ADR 0003
(déterministe avant Temporal) et touche l'ADR 0011 (Rust pour `locusd`).

---

## Contexte

§11.1 nomme trois implémentations de `WorkflowBackend` pour la V1, dont
`TemporalWorkflowBackend`. L'ADR 0011 a mis `locusd` en Rust. Les deux décisions se rencontrent
ici, et le fait qu'elles se contrarient n'était visible d'aucune des deux.

**Constat vérifié pendant W3.d :**

- le SDK Rust officiel de Temporal est publié en `0.1.0-alpha.1` (`temporal-sdk-core`,
  `temporal-sdk`). Ce n'est pas une lecture de documentation : les versions ont été relevées sur
  l'index crates.io ;
- `temporal-sdk-core-api` **ne se construit pas** dans cet environnement. Son script de build
  panique — la génération protobuf n'a pas ce qu'il lui faut. Ajouter la dépendance ajouterait donc
  aussi `protoc` et une étape de génération à toutes les machines qui construisent le projet, ce que
  « aucune dépendance implicite à une machine de développeur » interdit en toutes lettres ;
- il n'existe par ailleurs aucun cluster Temporal dans la CI, et aucun test d'intégration ne
  pourrait donc valider la liaison même si elle compilait.

Les forks tiers (`squads-temporal-*`, en `0.3.x`) existent et sont plus avancés. Prendre une
dépendance de fork pour le cœur durable du projet est une décision d'un autre ordre, qui ne se prend
pas en passant dans un sprint.

---

## Décision 1 — Livrer la traduction, la nommer traduction

`packages/workflow-backends/src/temporal.rs` contient la correspondance complète entre les six
opérations de §11.1 et les concepts de Temporal, testée contre un faux cluster. La liaison au fil
n'est pas livrée.

**Motifs.**

Le mensonge facile aurait été d'appeler « backend Temporal » ce qui parle à un faux. Un
`TemporalWorkflowBackend` qui n'a jamais vu de cluster n'est pas un backend Temporal ; c'est une
traduction testée, ce qui est utile et n'est pas la même chose. Le module le dit dans sa première
phrase, le test de sortie le redit dans la sienne, et cette ADR le redit ici — trois fois, parce que
c'est le genre d'imprécision qui devient vraie à force d'être répétée par omission.

La couture — [`TemporalGateway`] — a la forme de l'API réelle : une méthode par RPC de
`WorkflowService`, pas une de plus. Ce qui la traverse est exactement ce que la liaison devra
implémenter. Elle serait nécessaire de toute façon : un client gRPC ne se teste pas sans une couture
à ce niveau.

**Ce qui n'est donc pas prouvé.** Que le cluster réponde comme le faux. Le faux code des refus
plausibles (`NotFound`, `NotRunning`, `QueryUnsupported`), pas les refus observés. Toute divergence
entre les deux se paiera à la première liaison, et c'est le prix assumé de ne pas prendre une
dépendance alpha aujourd'hui.

---

## Décision 2 — `suspend` et `resume` sont des signaux réservés, et `inspect` peut répondre « je ne
sais pas »

Temporal n'a pas de pause côté serveur pour une exécution de workflow. `PauseActivity` existe,
`PauseWorkflow` non. Suspendre est donc de la **logique de workflow**, et la seule façon de la
demander de l'extérieur est un signal : `__locus_suspend`, `__locus_resume`.

Conséquence immédiate : le serveur ne sait pas qu'un workflow est suspendu. `DescribeWorkflowExecution`
dira « running ». Un adaptateur qui recopierait ce statut annoncerait une exécution en cours là où
elle est en pause.

La suspension se lit donc par une **query** réservée, `__locus_state`, à laquelle un workflow écrit
pour ce control plane répond. Un workflow qui n'y répond pas est parfaitement valide — et rend alors
la suspension inobservable. Dans ce cas, `inspect` rend `WorkflowState::Unknown` avec le détail de
ce qui manquait.

**Motifs.**

Rendre `Running` aurait été un **défaut plausible** : la réponse la plus probable, indiscernable
d'une observation, et fausse une fois sur deux exactement quand la question compte. C'est la même
décision qu'en W2.18, où `UNKNOWN` a été distingué d'une valeur par défaut vraisemblable, et pour la
même raison : un inconnu qui a l'air d'un résultat ne se remarque jamais.

---

## Décision 3 — Le port change, et c'est le second moteur qui l'a exigé

Trois amendements à `WorkflowState`, tous imposés par la traduction :

1. **`step` devient `Option<usize>`.** Temporal rend un statut, pas une position ; la déduire
   demanderait de tirer tout l'historique à chaque inspection. `None` dit « en cours, position non
   observable » plutôt qu'un zéro qui aurait l'air d'un début.
2. **`Failed` apparaît à côté de `Terminated`.** « On l'a arrêtée » et « elle a cassé » n'appellent
   pas la même compensation (§11.4). Les replier l'une sur l'autre ferait disparaître la question.
   `TimedOut` est rangé sous `Failed` : une expiration est une casse, pas une décision.
3. **`Unknown` apparaît.** Deux cas le produisent : la suspension inobservable de la décision 2, et
   `ContinuedAsNew` — où l'exécution continue sous un `run_id` que la référence en main ne désigne
   plus. Prétendre `Completed` ferait croire à un aboutissement ; prétendre `Running` ferait croire
   que la référence est encore bonne.

**Motifs.**

Les trois manques étaient invisibles tant que le seul moteur était en mémoire : un moteur en mémoire
connaît toujours son indice de pas, ne casse jamais tout seul, et n'ignore jamais son propre état.
Le port avait donc pris la forme de son unique implémentation sans que personne ne le décide — la
panne exacte que l'ADR 0003 nomme, trouvée en écrivant le second moteur, ce qui est la raison pour
laquelle l'ADR en demande deux.

C'est la première fois de ce chantier qu'une interface est corrigée par l'usage plutôt que par
relecture. Le coût a été de trois lignes dans les tests de W3.b et W3.c, parce que la correction est
arrivée avant le premier consommateur. Elle serait arrivée après, avec un journal en production, si
Temporal avait été branché en W3.d au lieu de W3.a.

---

## Décision 4 — L'adaptateur ne garde aucun état d'exécution

Il ne mémorise que la correspondance entre l'identifiant du port et la référence cluster
(`workflow_id` **et** `run_id`). Tout le reste est demandé au cluster à chaque appel.

**Motifs.**

Un cache local serait une seconde vérité. Elle diverge au premier redémarrage du control plane, et
elle diverge en silence — l'adaptateur continuerait d'annoncer ce qu'il croit savoir. Un test le
vérifie en changeant le statut du faux cluster derrière l'adaptateur.

Le `run_id` est conservé parce que Temporal réutilise le `workflow_id` d'une exécution à l'autre :
une opération qui l'omettrait viserait « la dernière en date », c'est-à-dire une cible qui change
toute seule.

---

## L'arbitrage laissé ouvert

**Ce qui manque à la V1 :** un `TemporalWorkflowBackend` qui parle à un cluster.

**Les trois voies, et ce qu'elles coûtent :**

1. **`temporal-sdk-core` en alpha.** Coût : une dépendance `0.1.0-alpha.1` au cœur durable du
   projet, plus `protoc` sur toute machine qui construit. Gain : le SDK officiel, qui suivra.
2. **Un fork tiers (`squads-temporal-*`, `0.3.x`).** Coût : dépendre d'un fork pour l'orchestration
   durable, et de sa maintenance. Gain : ça compile probablement aujourd'hui.
3. **gRPC direct sur `WorkflowService`.** Coût : `tonic` + protos vendorés + `protoc`, et le travail
   de suivre l'API. Gain : aucune dépendance de SDK, et le contrôle exact de ce qui passe sur le
   fil — la couture actuelle a précisément cette forme.

Aucune ne se tranche dans un sprint : les trois engagent la V1 sur ce qu'elle peut promettre en
matière de durabilité. La couture livrée ici est commune aux trois, donc aucun travail n'est perdu
quelle que soit l'issue.

---

## Plan de rollback

`packages/workflow-backends/src/temporal.rs` se supprime sans rien casser : rien ne le consomme
encore, et `boundaries.json` reste inchangé — la règle 2 réservait déjà le SDK Temporal à ce paquet,
où il n'est pas entré.

Les trois amendements au port de la décision 3, eux, **ne se rollback pas** — et c'est voulu. Y
revenir reviendrait à réintroduire un `step` obligatoire qu'un moteur réel ne peut pas fournir, une
confusion entre casse et arrêt, et un `Running` là où l'on ne sait pas. Ce sont des corrections, pas
des choix réversibles ; l'ADR les enregistre pour qu'on sache **pourquoi** elles ont l'air d'un
détour.
