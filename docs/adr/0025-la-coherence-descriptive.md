# ADR 0025 — Ce que le dépôt dit de lui-même est vérifié comme le reste

**Statut :** accepté. N'amende aucune section de `SPEC_V1.md`. Ouvre `W22`.

**Contexte.** Un audit externe du 2026-08-21 a trouvé quatre affirmations fausses sur l'état du
système, dans deux dépôts, toutes de la même forme : une déclaration en prose qu'aucun test ne
vérifie.

`apps/locus-execd/src/main.rs:12` imprime « aucun driver de runtime n'est encore branché (W4.d) » et
rend `ExitCode::FAILURE`, tandis que `lib.rs` du même crate déclare « Depuis W4.d.2, ce paquet
**lance** un runtime ». `docs/10` ne porte aucun marqueur `**fait**` sur `W4.d.1` à `W4.d.4` alors
que le code existe. Côté worker, `cli/cmd/worker.ts:96` implémente `bubblewrapWorks` par
`Bun.which("bwrap") !== null` alors que `locus/capability-manifest.ts:33` exige « Vrai quand `bwrap`
**démarre réellement** — l'existence du binaire ne suffit pas », et `worker.ts:99` renseigne
`diskFreeMb: 0` en dur. Enfin `locus/index.ts:374` justifie l'inertie du worker par une condition
que `W2.4` a levée il y a longtemps.

Les quatre ont été **revérifiés à la ligne** avant l'écriture de cet ADR, et les quatre tiennent.

Le dépôt applique une discipline exemplaire aux invariants de code : gardes de frontières, tests
d'absence, mutations vérifiées rouges, décompte des fichiers réellement examinés. Il n'en applique
aucune à ce qu'il dit de lui-même.

**Le corollaire que cet ADR ajoute à l'ADR 0022 décision 0 :**

> **Une affirmation sur l'état du système est une promesse.** Un `main.rs` qui déclare absente une
> capacité que son `lib.rs` exporte ment aussi sûrement qu'une arête `message` qu'aucun routeur
> n'honore. La règle « on ne livre jamais une promesse » vaut pour les **déclarations** autant que
> pour les types, et une capacité **niée** est une promesse négative : elle induit en erreur dans
> l'autre sens.

---

## Décision 0 — La cause de `D2` n'est pas celle que l'audit nomme, et elle est plus grave

L'audit constate que `W4.d.1` à `W4.d.4` n'ont pas de marqueur. La vérification faite ici montre
**pourquoi** : la garde de roadmap ne voit pas ces lignes. Son motif d'identifiant lit
`W\d+\.[a-z0-9]+` et s'arrête devant le second point, donc `W4.d.1` n'entre ni dans `planned`, ni
dans `marked`, ni dans aucune réconciliation avec le ledger.

Lecture faite au commit `dfd2824` :

```
W4.d.1   planned=false   marked=false   delivered=false
```

**Huit lignes** sont dans ce cas : `W4.d.1` à `W4.d.4`, `W4.e.1`, `W4.f.1`, `W4.g.1` et `W4.g.2`.
Le « frontière vide — toute ligne du plan est faite » que la garde imprimait portait donc sur 179
lignes sur 187, et il ne le disait pas.

**C'est la troisième occurrence du même défaut.** `W0.17` avait déjà réparé une cécité de cette
garde — elle ne lisait qu'une des deux familles d'identifiants et déclarait « frontière vide » sur
un plan qui ne l'était pas. La réparation avait porté sur la famille `R<n>` ; elle n'avait pas
demandé si d'autres formes existaient. C'est la règle du dépôt appliquée à son propre outillage :
**un compteur qui n'a rien lu ne vaut pas zéro**, et une garde qui ne voit pas une ligne ne la
déclare pas faite — elle la déclare inexistante, ce qui est pire, parce qu'aucun décompte ne baisse.

**Conséquence sur l'ordre de `W22` :** la garde se répare **avant** que les marqueurs soient posés.
Marquer `W4.d` à la main pendant que la garde reste aveugle rendrait la roadmap vraie une fois, sans
que rien n'empêche la faute de revenir — et c'est exactement ce qui s'est passé entre `W0.17` et
aujourd'hui.

---

## Décision 1 — Une déclaration de capacité est un fait vérifiable, pas de la prose

Un binaire ne peut pas déclarer absente une capacité que son propre crate exporte. Une garde le
vérifie mécaniquement.

**Motifs.** Un exploitant qui lance `locus-execd` aujourd'hui conclut que la fabric d'exécution
n'existe pas. C'est faux, et rien dans le dépôt ne l'attrape.

La garde cherche un **acte**, comme les gardes de frontières : la présence d'un motif de refus dans
un point d'entrée dont le crate exporte le symbole que le refus déclare manquant. Pas une recherche
de texte sur la prose — ce serait la faute que `W4.c` a commise puis corrigée, où une garde
signalait le paquet qui **écrivait** la politique de sécurité.

---

## Décision 2 — Un adaptateur de production ne contredit pas le contrat qu'il implémente

Quand un port déclare une condition — « vrai quand `bwrap` démarre réellement » — l'adaptateur réel
est exercé par un test de contrat, et non seulement par une implémentation simulée.

**Motifs.** `capability-manifest.ts` est impeccablement testé contre un `HostProbe` injecté ; c'est
son adaptateur de production qui est faux. Un port parfaitement testé dont l'unique implémentation
réelle ment ne vaut rien, et le déséquilibre est invisible parce que la CI est verte.

Ce défaut est de surcroît le seul des quatre qui porte sur un **niveau d'isolation**. Une sonde qui
annonce `bubblewrap` là où il n'y a que le binaire fait remonter un `CapabilityManifest` erroné
jusqu'à l'admission, où `place` de `W4.g` choisira un hôte sur une preuve qui n'en est pas une.

Et ce n'est pas un oubli : la sonde fautive porte un arbitrage écrit — « l'appel direct suffit pour
l'inventaire » — soixante lignes sous le contrat qu'elle viole. Un oubli se corrige ; un arbitrage
écrit contre son propre contrat signale que le contrat n'a pas été relu, ce qui est un défaut de
processus et non de code.

---

## Décision 3 — Un item de roadmap dont le code existe porte son marqueur, ou dit ce qui manque

La garde de roadmap gagne deux vérifications : elle voit **toutes** les formes d'identifiant du plan
(décision 0), et un item sans marqueur `**fait**` dont une entrée de ledger existe est un défaut —
comme l'inverse, déjà tenu.

**Motifs.** `W4.d` est le chemin critique de tout le projet ; le croire ouvert a orienté deux audits
successifs vers un mauvais diagnostic. Le marqueur n'est pas décoratif : c'est la seule source qui
dise ce qui reste.

---

## Décision 4 — Ce qui est écarté, et pourquoi

**Une vérification de cohérence par recherche de texte dans les commentaires.** Elle attraperait
`D4` et manquerait `D1`, tout en produisant des faux positifs sur toute prose historique légitime —
le ledger est append-only et décrit délibérément des états passés. La garde porte sur des **couples
déclaration/symbole**, jamais sur des mots.

**Une interdiction de la prose historique.** Le ledger existe pour ça. Ce qui est interdit est
qu'une prose historique occupe le point d'entrée d'un binaire ou l'en-tête d'un adaptateur de
production.

**Un motif d'identifiant plus permissif « au cas où ».** La décision 0 corrige une cécité constatée,
sur huit lignes lues. Élargir le motif à des formes qu'aucune ligne n'emploie créerait une garde qui
prétend couvrir ce qu'elle n'a jamais vu — la même faute, dans l'autre sens. Le test de sortie de
`W22.a` déclare donc le nombre de lignes réellement reconnues, et une baisse de ce nombre est un
échec.

## Conséquences

`tooling/` gagne deux gardes ; la garde de roadmap en gagne deux. `apps/locus-execd/src/main.rs`,
`docs/10`, et côté Canterel `cli/cmd/worker.ts` et `locus/index.ts` sont corrigés. Aucun type de
domaine n'est touché.

**Vérifié avant d'écrire cet ADR**, sur la question que l'audit laissait ouverte : la garde de
frontières expose déjà un décompte réutilisable — `check-boundaries.ts` imprime
`vérifiée sur N fichier(s)` et distingue `NON VÉRIFIÉE` de `sans objet`. `W22.d` peut s'appuyer
dessus sans le reconstruire.

## Plan de rollback

Entièrement additif. Les gardes se retirent par un diff ; les corrections de vérité ne se retirent
pas, puisque revenir à une déclaration fausse n'a pas de sens.
