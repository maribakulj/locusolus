# ADR 0035 — Ce qu'une campagne de self-tests atteste, et qui a le droit de s'en servir

**Statut :** accepté. Tranche l'arbitrage laissé ouvert par `W5.ac`, et corrige la réponse que
`W5.ac` lui-même proposait.

**Contexte.** `W5.z` a livré la lecture des attestations, `W5.aa` leur dépôt, `W5.ab` leur transport
d'un job de CI à l'autre. Le premier convoyeur réel a rapporté « 1 retenue, 0 écartée » pendant que,
sur le **même** runner et dans le **même** tour, `locusd` refusait un placement avec « confinement S2
annoncé mais jamais prouvé, aucune campagne n'a conclu ».

`W5.ac` a trouvé la cause immédiate — la campagne déposait sous un identifiant de worker par défaut,
`canterel-local`, que nul réclamant ne porte — et a laissé l'arbitrage ouvert en suggérant une
réponse : indexer une attestation d'hôte par **empreinte d'hôte** plutôt que par worker, puisque
`W5.x` avait mesuré que l'empreinte est stable d'un runner à l'autre.

**Cette réponse est fausse, et c'est la mesure du même tour qui la réfute.** Ce document dit pourquoi,
et ce qui la remplace.

---

## Ce qui a été établi, et comment

Tout ce qui suit vient de la lecture du code et d'exécutions réelles — le job `e2e` du run
32826289889 et la chaîne montée localement. Rien n'est déduit d'un commentaire.

| Fait | Où il se vérifie |
| --- | --- |
| `Ask` a **deux** variantes : `Readiness` et `Place`. Le broker n'exécute **rien**. | `packages/broker/src/protocol.rs` |
| Le placement juge le manifeste **annoncé** par le worker, plus ce qui a été **attesté pour ce worker** | `Candidate::shortfall` — `admit(spec, self.capabilities)` puis `proven_level` |
| Les faits d'hôte du broker n'entrent **pas** dans la décision par candidat ; ils servent `Readiness` | mêmes fichiers, `serve(…, facts, proven, …)` |
| `Proven::standing` est indexé **par worker** | `apps/locus-execd/src/announced.rs` |
| La seule campagne qui existe mesure l'hôte **sous podman rootless** | `apps/locus-execd/tests/host_sandbox.rs`, les seize sondes |
| Le `S2` que `canterel` annonce dépend de **bubblewrap** | `sandboxBackend`, `sandboxLevels` dans `capability-manifest.ts` |
| Sur un runner GitHub, le même hôte prouve `S2` au broker et n'annonce que `S1` au worker | run 32826289889 : seize sondes vertes côté `sandbox`, « l'hôte ne sait pas dépasser S1 » côté `e2e` |

**Le dernier fait est celui qui décide.** Il n'est pas une anomalie : `locus-execd` mesure ce que
**podman rootless** sait confiner sur cette machine, `canterel` annonce ce que **bubblewrap** y sait
faire, et `bwrap` n'est pas installé sur ce runner. Les deux ont raison, et ils parlent de deux
mécanismes différents.

---

## Pourquoi l'indexation par empreinte d'hôte est refusée

C'était la réponse que `W5.ac` proposait, et elle avait pour elle une mesure : l'empreinte d'hôte est
identique, caractère pour caractère, d'un runner à l'autre (`W5.x`). Elle réglait proprement le
problème d'adressage — une campagne s'exécute sur un hôte, l'identité d'un worker naît à son
enrôlement, plus tard, et une campagne ne peut donc pas connaître son consommateur.

Elle est refusée parce qu'elle ferait exactement ce que le tableau ci-dessus rend visible :
**un worker hériterait d'une preuve portant sur un mécanisme qu'il n'emploie pas.** Sur ce runner,
`canterel` — sans `bwrap` — recevrait le `S2` que podman a prouvé, et `locusd` le placerait sur une
mission qu'il ne saurait pas confiner. Une preuve qui autorise un confinement que personne
n'appliquera est pire qu'une absence de preuve : l'invariant 5 dit « sandbox réelle avec limites et
attestation », et celle-là serait attestée sans être réelle.

L'empreinte d'hôte reste **nécessaire** — elle empêche une attestation de voyager vers une machine
différente, et `W5.v` l'a rendue stable pour cela. Elle n'est simplement pas **suffisante**.

---

## Décision 1 — Une attestation nomme le **mécanisme** qu'elle atteste

Un enregistrement porte aujourd'hui le worker, le niveau, l'empreinte d'hôte et l'instant. Il lui
manque la seule chose qui rend les autres interprétables : **par quoi** le confinement a été obtenu.

`podman-rootless` et `bubblewrap` ne sont pas deux façons d'écrire la même garantie. Ils échouent
différemment, ils s'installent différemment, et — mesuré ci-dessus — ils sont présents
indépendamment l'un de l'autre sur une même machine.

Conséquence : le champ est **obligatoire**, pas optionnel. Un enregistrement sans mécanisme n'est pas
un enregistrement dégradé, c'est un enregistrement dont on ne sait pas ce qu'il affirme, et la règle
du dépôt vaut ici comme ailleurs — une ignorance ne se range pas du bon côté par défaut.

---

## Décision 2 — `Proven` reste indexé par worker

La question que le placement pose est « **ce worker-ci** peut-il porter cette mission », et
`Candidate::shortfall` la pose déjà ainsi. Changer la clé pour l'hôte remplacerait une question juste
par une question voisine, au seul motif que la seconde est plus facile à alimenter.

Le problème d'adressage que `W5.ac` a trouvé est réel et il reste entier ; il ne se règle pas en
changeant la clé, il se règle en décidant **quelle campagne** a le droit de remplir cette clé — ce
qui est la décision suivante.

---

## Décision 3 — Un worker ne tire d'une attestation que si le mécanisme est un des siens

C'est la règle qui remplace l'indexation par hôte. Une attestation vaut pour un worker quand les
trois tiennent ensemble : même hôte (l'empreinte, déjà vérifiée), même worker (la clé, décision 2),
**et** un mécanisme que ce worker emploie.

Le troisième terme se lit dans le manifeste, qui voyage déjà entier jusqu'au répondant — `Ask::Place`
le transporte, et son commentaire dit pourquoi : « le manifeste voyage entier, et il reste une
**annonce** […] c'est le répondant, qui sait ce qui a été prouvé, qui décide ». Le répondant a donc
déjà les deux moitiés sous la main ; il ne les rapproche pas encore.

**Conséquence assumée, et elle est immédiate** : aucun worker `canterel` ne peut aujourd'hui tirer de
la campagne du job `sandbox`, qui atteste `podman-rootless`. Ce n'est pas un manque — c'est le
constat exact, rendu enfin lisible. Ce qui manquait n'était pas une attestation, c'était une campagne
qui exerce le mécanisme du worker.

---

## Décision 4 — Ce que la campagne du job `sandbox` atteste est le chemin du **broker**, qui n'exécute rien

`Ask` n'a pas de variante d'exécution. `locus-execd` répond « sais-tu confiner » et « ce worker
peut-il porter cette mission » ; il ne reçoit jamais de mission à faire tourner. La campagne des
seize sondes mesure donc, avec exactitude, un chemin d'exécution que **rien n'emprunte encore**.

Cela n'invalide ni la campagne ni le convoyeur de `W5.ab` : le transport et la lecture sont exercés
de bout en bout, et ils le resteront. Cela nomme simplement leur consommateur, qui n'existe pas
encore. Deux chemins peuvent le faire exister, et ils sont différents :

1. **Une campagne qui exerce le mécanisme du worker** — `bubblewrap` là où `canterel` tourne. C'est
   elle qui remplirait `Proven` pour un worker réel, et c'est la voie courte.
2. **Une variante d'exécution dans `Ask`**, qui ferait exécuter une mission *par* le broker. C'est un
   changement de rôle du broker, pas une addition, et il demande son propre ADR.

Ce document ne choisit pas entre les deux. Il refuse seulement qu'on continue de remplir `Proven`
avec la mesure d'un mécanisme que le réclamant n'emploie pas — ce que le défaut `canterel-local`
faisait sans que personne le voie.

---

## Ce que cet ADR ne décide pas

- **Comment une campagne apprend l'identité du worker.** Elle s'exécute sur un hôte ; l'identité naît
  à l'enrôlement. La décision 3 rend la question moins pressante — une campagne qui exerce le
  mécanisme du worker tourne, elle, *là où le worker est* — mais elle ne la fait pas disparaître.
- **Si `podman-rootless` et `bubblewrap` doivent être comparables.** Ils portent le même nom de
  niveau, `S2`, et ce document les traite comme incomparables. Décider qu'un mécanisme en « couvre »
  un autre demanderait de mesurer les deux contre les seize sondes, ce qui n'a pas été fait.

## Ce que cet ADR débloque et ce qu'il ferme

Il ferme l'arbitrage de `W5.ac` et retire la réponse que cet item suggérait. Il ouvre deux items
nommés — le mécanisme porté par l'enregistrement et vérifié au placement, puis la campagne qui exerce
le mécanisme du worker — et il donne à `W12.d` la raison exacte pour laquelle sa clause de placement
ne peut pas encore aboutir sur un runner de CI.
