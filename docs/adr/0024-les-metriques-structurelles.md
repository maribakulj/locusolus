# ADR 0024 — Une métrique se définit avant de se calculer

**Statut :** accepté. **Amende** `docs/11_ACCEPTANCE_MATRIX.md`, dont la ligne « Métriques
structurelles » énumérait treize noms. N'amende aucune section de `SPEC_V1.md` — les familles §28.2
(système) et §28.3 (scientifiques) sont distinctes de celle-ci et ne sont pas touchées.

**Contexte.** La matrice d'acceptation V1 exige treize métriques structurelles « calculées depuis le
seul journal ». Un audit du 2026-08-21 a établi trois faits.

D'abord, **une seule a un producteur** : `structural_regret`, livrée par `R3` dans
`packages/evaluation/src/regret.rs`. Les douze autres n'existent nulle part. Les cinq métriques que
`R3` a livrées dans `packages/coordination/src/metrics.rs` — couverture de revue, profondeur,
concentration, revue mutuelle, isolement de visibilité — sont bonnes et motivées, mais aucune n'est
l'une des treize : elles parlent toutes de la structure de **revue**.

Ensuite, **le matériau brut existe pour presque toutes**. Les opérations sont dans `version.rs`, le
diff dans `diff.rs`, le cycle de vie dans `lifecycle.rs`, les passages de témoin dans `messaging.rs`,
le cadre de campagne longue dans `endurance.rs`. Ce sont des calculs sur des faits déjà écrits.

Enfin, et c'est le fait qui commande cet ADR : **aucune des treize n'est définie**. Les treize noms
ont été cherchés dans l'ensemble de `docs/` ; ils n'apparaissent qu'à cette ligne. Il n'existe donc
ni formule, ni numérateur, ni dénominateur, ni périmètre — seulement des intitulés.

C'est ce dernier point qui rend l'ordre « ADR d'abord, code ensuite » obligatoire plutôt que
prudent, et c'est l'objet de la décision 1.

---

## Décision 1 — Une grandeur nommée mais non définie ne s'implémente pas

Aucune métrique de cette famille n'entre dans le code avant que le présent ADR ne fixe sa formule,
ce qu'elle compte au numérateur, ce qu'elle compte au dénominateur, et **ce qu'elle ne prétend pas
dire**.

Le motif n'est pas la rigueur pour elle-même. Une métrique implémentée depuis un nom non défini est
**pire qu'une métrique absente**. Une métrique absente n'induit personne en erreur : elle manque, et
son absence se voit. Un nombre affiché, lui, sera lu, cité, mis dans un tableau de bord, et finira
par orienter une décision — sans que personne sache ce qu'il compte. Le lecteur ne peut pas
distinguer un nombre bien défini d'un nombre mal défini : les deux ont la même apparence.

C'est le motif que ce dépôt rencontre à chaque passe de mutants, d'un cran plus haut. « Une propriété
décrite sans être testée est une propriété qu'on croit tenir » ; ici, **une grandeur nommée sans être
définie est une grandeur qu'on croira mesurer**.

La conséquence pratique est que la moitié du travail de cet ADR consiste à écrire des phrases
négatives. Elles ne sont pas du remplissage : ce qu'une métrique refuse de dire est ce qui empêche
un lecteur de lui faire dire autre chose.

## Décision 2 — Un nom qui promet plus que son calcul est renommé

Quatre des treize portent un nom qui affirme davantage que ce que le calcul produit. Un nom est une
promesse faite au lecteur du tableau de bord, qui ne lira jamais la formule ; le corriger coûte un
renommage aujourd'hui et rien du tout, tandis que le garder coûte une interprétation fausse à chaque
lecture.

**`graph_edit_distance` → `applied_edit_length`.** La distance d'édition entre deux graphes est le
nombre **minimal** d'opérations pour passer de l'un à l'autre, et son calcul est NP-difficile dans le
cas général. Ce que le dépôt possède est le `diff` de `W17.h` : *une* suite d'opérations qui mène de
`a` à `b`, sans garantie qu'elle soit la plus courte. Publier cette longueur sous le nom de
« distance » affirmerait une minimalité qu'aucun code ne calcule. Le nom retenu dit ce que c'est, et
la définition ajoute qu'elle est une **borne supérieure** de la distance véritable.

**`parallelism` → `average_parallelism`.** `Dimension::Parallelism` existe déjà : c'est l'une des six
dimensions de budget de §7.2, donc un **plafond qu'on fixe**. Une métrique homonyme serait une
**mesure de ce qui s'est produit**. Deux choses de même nom dont l'une borne l'autre est exactement
le vocabulaire parallèle que `CLAUDE.md` refuse, et la confusion serait pire qu'ailleurs : « le
parallélisme vaut 4 » ne dirait plus si c'est la limite ou le constat.

**`state_transfer_volume` → `handed_over_attempts`.** Voir la décision 7 : le mot « volume » appelle
des octets, et l'architecture interdit délibérément ce qui en produirait.

**`topology_entropy` → `degree_entropy`.** Deux défauts. Le nom ne dit pas **de quelle distribution**
on prend l'entropie, alors qu'au moins quatre candidates existent — degrés, charge de travail, types
d'arêtes, tailles des partitions de visibilité — et qu'elles ne classent pas les organisations dans
le même ordre. Et il fait écho à `TopologyNode`, que `CLAUDE.md` proscrit ; bien qu'une mesure ne
crée pas de vocabulaire d'objet parallèle, un nom qui oblige à refaire ce raisonnement à chaque
relecture est un nom mal choisi.

## Décision 3 — Le chemin parcouru et la destination sont deux mesures, jamais une

`mutations_per_run` compte les opérations **effectivement appliquées** au cours d'une exécution.
`applied_edit_length` mesure le diff entre l'état de départ et l'état d'arrivée. Sur une exécution
qui ne revient jamais en arrière, les deux rendent le même nombre — ce qui donne l'impression d'une
redondance, et la première rédaction de cet ADR a failli en supprimer une.

Elles diffèrent exactement là où c'est intéressant. Ajouter une arête puis la retirer coûte deux
opérations au chemin et zéro à la destination. Leur **écart** est donc le détour : le travail de
coordination qui n'a laissé aucune trace dans la structure finale.

C'est la même distinction que `edge_churn` fait à l'intérieur d'une fenêtre — un solde nul n'est pas
un churn nul — remontée d'un niveau. Aucune des deux ne se déduit de l'autre, et les fondre
supprimerait la seule mesure du travail inutile que cette famille contient.

**Amendement du 2026-08-21, apporté par `W21.c`.** Le paragraphe ci-dessus énonce le détour comme un
**écart**, ce qui suppose que le chemin est toujours au moins aussi long que le diff. C'est faux, et
l'implémentation l'a mesuré plutôt que déduit.

`Diff::between` n'émet que **quatre** sortes d'opérations et n'infère jamais un `REPLACE_NODE`, un
`SPLIT_NODE` ni un `MERGE_NODES` : au niveau des états, un remplacement est indiscernable d'un
retrait suivi d'un ajout, et deviner ferait lire à un approbateur une intention que personne n'a
écrite. Un remplacement de nœud coûte donc **une** opération au chemin et **quatre** au diff.

La soustraction change alors de signe, et un détour de `-3` serait affiché comme une quantité alors
qu'il signifie « ces deux mesures ne comptent pas dans le même vocabulaire ». `detour_from` rend donc
`None` dans ce cas, jamais un entier signé — et `Some(0)` reste distinct de `None`, parce que « il
n'y a pas eu de détour » et « la comparaison n'a pas de sens ici » sont deux constats différents.

Ce que le paragraphe d'origine décrit reste juste **tant que le chemin s'exprime dans le vocabulaire
du diff**, ce qui est le cas courant. Il lui manquait sa condition, et c'est l'implémentation qui l'a
nommée : la borne supérieure de la décision 2 vaut contre le vocabulaire du diff, pas contre les dix
opérations.

## Décision 4 — Un taux dont le dénominateur contient l'indécis est faux

`accepted_mutation_rate` a pour dénominateur les propositions **parvenues à une décision terminale**,
et non les propositions soumises.

Une proposition encore en attente n'est ni acceptée ni refusée. La compter au dénominateur fait
baisser le taux pour une raison qui n'a rien à voir avec la qualité des propositions : la lenteur des
décideurs. Un système dont la gouvernance prend du retard verrait son « taux d'acceptation » chuter
sans qu'aucun agent n'ait changé de comportement, et la lecture naturelle — « les agents proposent
n'importe quoi » — serait fausse.

Les propositions en attente sont donc **comptées à part**, et rendues avec le taux. C'est la règle
que `W18.e` a déjà posée pour la métrique d'acceptation des adaptations : une adaptation que personne
n'a regardée est déclarée **hors mesure**, jamais comptée comme acceptée, parce que le silence n'est
pas un accord.

## Décision 5 — Un taux d'annulation sans cohorte est censuré

`rollback_rate` se rend **par cohorte**, jamais comme un nombre unique.

Une mutation acceptée aujourd'hui peut être annulée demain. Un taux calculé à l'instant `T` divise
donc des annulations qui ont eu le temps de survenir par des acceptations qui, pour les plus
récentes, ne l'ont pas eu. Le taux paraît d'autant plus bas que le système est actif, et la
conclusion — « on annule de moins en moins » — se produit toute seule quand on accélère.

Une cohorte est un ensemble d'acceptations délimité, plus une fenêtre d'observation : « des mutations
acceptées dans la fenêtre `W`, la part annulée dans les `N` opérations qui ont suivi ». Une cohorte
dont la fenêtre n'est pas close est rendue comme **incomplète**, avec le nombre d'acceptations encore
observables — et non comme un taux provisoire, qu'un lecteur comparerait à un taux définitif.

## Décision 6 — Une entropie non normalisée ne se compare pas

`degree_entropy` est l'entropie de Shannon de la distribution des degrés du graphe de coordination,
**divisée par `log n`**, où `n` est le nombre de nœuds.

Sans normalisation, l'entropie croît mécaniquement avec la taille : une organisation de trente agents
a presque toujours une entropie supérieure à une organisation de cinq, quelle que soit leur forme.
Comparer les deux nombres bruts revient à comparer des tailles en croyant comparer des structures.

Et ce que la métrique ne dit pas : **elle ne mesure pas l'équité de la charge de travail.** La
concentration est déjà mesurée par `busiest_reviewer_load`, livrée par `R3`. Une organisation peut
avoir une entropie de degrés élevée et une charge très concentrée — un nœud relié à tout le monde
dans un graphe par ailleurs varié. Les deux nombres répondent à deux questions, et le second est
celui qu'on veut quand on cherche un goulot.

## Décision 7 — Le transfert d'état se mesure en tentatives, pas en octets

`handed_over_attempts` compte les tentatives en vol qu'un nœud sortant transmet à son successeur, lu
du `Handover` de `W16.e`.

Le nom d'origine appelait un volume de données, et il n'y en a pas — non par omission, mais par
décision. L'ADR 0019 condition 3 tranche que le passage de témoin porte ce que le nœud sortant
**tenait**, jamais ce qu'il **savait** : `docs/13` fixe « nouvel attempt, nouvelle vue, nouveau
hash », et un contexte de mission qui voyagerait contournerait cette immuabilité sans la nommer.

Une métrique de volume en octets aurait donc deux issues, toutes deux mauvaises. Ou bien elle vaudrait
zéro en permanence, puisque rien n'est copié — un cadran qui n'a jamais bougé et qu'on finit par
croire cassé plutôt que juste. Ou bien on ajouterait la copie pour avoir quelque chose à mesurer, et
la métrique aurait créé le coût qu'elle prétend observer.

Ce que le passage de témoin coûte réellement dans cette architecture est le nombre de tentatives
qu'un successeur doit reprendre. C'est cela qui rend une reconfiguration chère, et c'est cela qu'on
mesure.

## Décision 8 — `communication_tokens` est reporté sur une dépendance technique nommée

La métrique demande de séparer les tokens dépensés **à se coordonner** de ceux dépensés **à
travailler**. Le dépôt sait compter les tokens : `Dimension::Tokens` est l'une des six dimensions de
§7.2, et `packages/budget/src/ledger.rs` en tient les écritures. Il ne sait pas les **classer**.

`EntryKind` distingue six sortes d'écritures — allocation, réservation, libération, consommation,
ajustement, remboursement — qui décrivent le **mouvement**, pas son objet. Ce qui distingue une
dépense de coordination d'une dépense de travail n'existe que dans le champ `reason`, qui est du
texte libre. Une métrique qui classerait en analysant cette chaîne rendrait un nombre dont la
justesse dépendrait de la façon dont chaque appelant a rédigé sa phrase, et se dégraderait
silencieusement au premier appelant qui écrit autrement.

Le blocage est donc **une classification de dépense absente du modèle de budget**, ce qui est une
dépendance technique nommée — l'un des deux seuls motifs de report qu'admet la décision 0 de l'ADR
0022. Il ne s'agit pas de « personne ne l'utilise encore », qui n'en est pas un.

La ligne de plan portera `attend:W21.m`, l'item qui ajoute cette classification, et se lèvera d'elle
même quand il sera livré — la règle de `W0.16`.

### Addendum — le blocage est levé

`W21.m` est livré : `Entry` porte une [`Classification`], `reserve_for` et `allocate_for` la
déclarent, et les soldes en héritent. La dépendance technique nommée n'existe plus, et `W21.l` est
redevenu un item ordinaire.

Deux choses que l'implémentation a fixées, et qui n'étaient pas décidées ici :

- **L'ignorance n'est pas un objet de dépense.** `Spend` porte deux valeurs, `Coordination` et
  `Work` ; « non classé » vit dans un second type. Une énumération à trois barreaux laisserait un
  appelant *déclarer* non classé, ce qui est une affirmation, alors que l'absence se **constate**.
- **Les soldes héritent, ils ne redéclarent pas.** Rendre, constater et rapprocher reprennent la
  classification de la retenue qu'ils soldent : rembourser de la coordination reste de la
  coordination. Redemander l'objet à chaque solde ouvrirait la porte à deux réponses pour la même
  retenue, et le journal porterait une contradiction que personne n'aurait voulue.

## Décision 9 — Aucune métrique de cette famille ne juge

Aucun module de métriques ne contient de seuil, de note, de verdict ni de qualificatif. C'est la
règle que `R3` a déjà posée et qu'un test tient en refusant `const MIN`, `const MAX`, `fn is_healthy`,
`fn score` et `enum Verdict` dans la source ; elle s'étend à tout ce que le présent ADR fait naître.

Un seuil écrit en Rust a l'apparence d'un fait mesuré alors que c'est une décision de politique. « Le
churn est trop élevé » dépend du domaine, de la phase du projet et de ce qu'on cherche ; l'inscrire
dans le calcul le soustrait à la discussion et le rend invisible à qui lit le nombre.

Les métriques rendent des quantités. Ce qui en fait un jugement est le moteur de politique, où un
seuil est une valeur qu'on peut voir, discuter et changer.

---

## Les treize, telles qu'elles sont arrêtées

| Nom arrêté | Origine | Définition | Ce qu'elle ne dit pas |
|---|---|---|---|
| `mutations_per_run` | inchangé | Nombre d'opérations de coordination **appliquées** au cours d'une exécution, par sorte | Rien sur leur qualité ; un nombre élevé n'est pas une faute |
| `edge_churn` | inchangé | Sur une fenêtre : arêtes ajoutées **plus** arêtes retirées, jamais le solde | Si le renouvellement était utile |
| `applied_edit_length` | ex-`graph_edit_distance` | Longueur du `diff` de `W17.h` entre deux versions | Ce n'est **pas** la distance minimale : c'en est une borne supérieure |
| `accepted_mutation_rate` | inchangé | Propositions approuvées ÷ propositions parvenues à une décision **terminale** ; les indécises sont comptées à part | Si les mutations acceptées étaient bonnes — c'est `rollback_rate` |
| `rollback_rate` | inchangé | Par cohorte : des mutations acceptées dans `W`, la part annulée dans les `N` opérations suivantes ; une cohorte ouverte est rendue **incomplète** | Rien tant que la fenêtre n'est pas close |
| `structural_regret` | **livré par `R3`** | `U(meilleur candidat disponible) − U(graphe choisi)`, mesuré contre le **menu** | Rien sur un optimum qui n'était pas au menu |
| `degree_entropy` | ex-`topology_entropy` | Entropie de Shannon des degrés, divisée par `log n` | **Pas** l'équité de charge : c'est `busiest_reviewer_load` |
| `critical_path_length` | inchangé | Plus longue chaîne de dépendances du graphe de tâches ; un cycle est **refusé en le nommant**, jamais parcouru | Rien sur le temps réel — c'est un compte d'étapes |
| `average_parallelism` | ex-`parallelism` | Travail total ÷ `critical_path_length` | **Pas** le nombre d'agents qui tournaient ; et à ne pas confondre avec `Dimension::Parallelism`, qui est un plafond |
| `communication_tokens` | **débloqué** | Tokens de coordination ÷ tokens totaux | — `W21.m` livré, voir l'addendum de la décision 8 |
| `handed_over_attempts` | ex-`state_transfer_volume` | Tentatives en vol transmises par `Handover` | **Pas** un volume d'octets : ADR 0019 condition 3 interdit la copie qui en produirait |
| `agent_lifetime` | inchangé | Durée entre l'entrée d'une instance dans une version et sa sortie, lue des transitions de `lifecycle.rs` | Rien sur ce que l'instance a accompli pendant ce temps |
| `failure_recovery_time` | inchangé | Durée entre un fait de panne et le fait de reprise correspondant | Rien tant qu'aucune campagne longue n'a produit de pannes réelles |

Onze à écrire, une livrée, une reportée.

## Conditions, sans lesquelles ces décisions sont mauvaises

1. **Chaque métrique se calcule depuis le seul journal**, et se rejoue à l'identique sur le même
   préfixe. Une métrique qui lirait un état vivant rendrait deux valeurs différentes pour le même
   passé, et ne pourrait pas être contestée. C'est la contrainte que la matrice porte déjà.
2. **Aucune ne se moyenne à travers des fixtures différentes.** `R3` a rendu ce refus explicite pour
   le regret — un lot dont les candidats ne partagent pas une fixture est refusé en la nommant — et
   il vaut ici : un nombre agrégé sur des situations incomparables est un nombre qu'on fait bouger en
   changeant l'échantillon.
3. **Les deux métriques de durée** — `agent_lifetime`, `failure_recovery_time` — sont calculables sur
   fixtures et le seront testées ainsi, mais leur **interprétation** demande des campagnes longues.
   L'écrire ici évite qu'une valeur mesurée sur trois transitions soit lue comme un fait de
   production.

## Conséquences

- `docs/11_ACCEPTANCE_MATRIX.md` est amendée : la ligne « Métriques structurelles » porte les noms
  arrêtés ci-dessus, et sa note de clôture cesse de dire que ces lignes attendent W15 et W17, qui
  sont faits.
- Une phase `W21` entre au plan, un item par métrique retenue, plus `W21.m` pour la classification
  de dépense dont `communication_tokens` dépend.
- Les quatre renommages ne touchent aucun code existant : aucune des quatre n'était implémentée.
  `structural_regret`, la seule livrée, garde son nom.

## Plan de rollback

Aucune migration de schéma, aucun format persistant. Un module de métrique est un calcul pur sur des
faits déjà écrits : le retirer ne perd rien, puisque la source reste le journal et qu'aucune valeur
n'est stockée. Un `git revert` de l'item suffit, métrique par métrique.

Le seul élément non trivialement réversible est `W21.m` — la classification de dépense — qui ajoute
un champ au modèle de budget. Il porte donc `[M]` et son propre plan : le champ est optionnel à la
lecture, une écriture ancienne sans classification se lit **non classée** plutôt que « coordination »
ou « travail », et la métrique compte les non classées à part exactement comme la décision 4 compte
les indécises.
