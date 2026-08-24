# ADR 0029 — La sérialisation des écritures : le verrou vit dans le journal, pas au-dessus

**Statut :** accepté. Débloque `W20.h`. **Amende** la signature de `EventStore::append` et le format
d'enveloppe de §10.1 par un champ facultatif. N'autorise aucune dépendance externe nouvelle.

**Contexte.** `apps/locusd/src/main.rs` nomme lui-même le blocage depuis `W20.g` :

> `W20.g` sert §22.4 et §22.1. Aucune commande de §22.3 : `Transaction::submit` prend `&mut self`,
> et la couche HTTP ne tient qu'un `&Runtime`. Sérialiser les écritures — verrou, file, acteur — est
> une décision qui mérite son item.

Le daemon sait lire — le graphe, la mémoire, les revues, le fil d'événements. Il ne sait pas écrire,
et ce n'est pas un oubli : Rust refuse de compiler tant que personne n'a dit comment plusieurs
requêtes concurrentes obtiennent l'accès exclusif qu'une écriture demande. C'est le langage qui force
l'arbitrage au lieu de laisser une corruption arriver en production.

**Ce qui demande réellement l'exclusion, mesuré sur le code plutôt que supposé.** `submit` fait
quatre choses :

| Étape | Ce qu'elle est | Exclusion nécessaire |
| --- | --- | --- |
| `Submission::of` | lecture de l'enveloppe | aucune |
| `ledger.recall` | lecture d'un registre partagé | partagée, en lecture |
| `handler.decide` | **pure** — `&self`, `&State`, rend des `Draft` | **aucune** |
| `store.append` | l'écriture | oui |

`Decide::decide` ne mute rien. Le travail de domaine — le plus coûteux des quatre — n'a donc jamais
eu besoin d'être sérialisé, et c'est ce constat qui rend la décision 2 possible.

---

## Décision 1 — `EventStore::append` prend `&self` : chaque backend possède sa propre concurrence

La signature devient `fn append(&self, command: Append, recorded_at: Timestamp)`. Le backend décide
comment il se protège, et le trait l'exige plutôt que de l'espérer. `MemoryEventStore` se protège par
un verrou interne — il **est** globalement sérialisé, et il le dit.

**Motifs.** L'alternative était un verrou dans `Transaction`, au-dessus du journal. Elle échoue pour
une raison présente, pas spéculative : `read_stream`, `feed` et `revision` prennent déjà `&self`, donc
les queries de §22.4 et le fil de §22.1 lisent pendant qu'on écrit. Un verrou au-dessus du journal les
bloquerait **toutes** pendant la durée d'une écriture. En mémoire cette durée est de quelques
microsecondes et personne ne s'en apercevrait ; avec le driver `PostgreSQL` de `W20.i`, c'est une
entrée/sortie, et une requête de lecture attendrait qu'une écriture distante finisse.

Autrement dit : mettre le verrou au-dessus **crée un goulot que le stockage n'a pas.**
`packages/event-store` n'a jamais eu d'ordre total global — `Expected` est par stream, et deux streams
n'ont aucune raison de s'attendre. Le verrou appartient donc à l'endroit qui sait ce qui doit
réellement s'exclure, c'est-à-dire au backend.

**Ce que la signature perd, et ce qui le remplace.** `&mut self` donnait une garantie **de type** :
deux appels ne peuvent pas se recouvrir. `&self` la déplace dans chaque implémentation. Elle est donc
rendue **vérifiable** plutôt que garantie : un test de contrat écrit depuis plusieurs fils, sur des
streams distincts et sur un même stream, et vérifie qu'aucun événement ne se perd, que les révisions
d'un stream sont contiguës, et qu'un seul gagnant existe par révision. Ce test est celui que `W20.i`
rejouera contre `PostgreSQL` — c'est exactement ce que sa ligne exige, « la suite de contract tests
passe à l'identique contre les deux backends ».

---

## Décision 2 — La sérialisation est **par stream**, et la décision reste dehors

`Transaction::submit` prend `&self`. Ce qui s'exclut est le couple `(consultation du registre,
écriture)` **pour un stream donné**, et rien d'autre. `handler.decide` s'exécute hors de toute
exclusion.

**Motifs.** Une commande lente sur une mission ne doit pas retarder une commande sur une autre
mission. C'est mesurable et c'est mesuré : le test rend `decide` lent pour un stream, et vérifie
qu'une commande sur un autre stream aboutit **sans attendre** — mesuré, pas décrit.

`decide` étant pure, l'exclusion ne commence qu'une fois les événements décidés, donc une fois le
stream **connu** : c'est le premier `Draft` qui le nomme. Vouloir verrouiller plus tôt aurait
demandé que l'enveloppe déclare son agrégat, ce qu'elle ne fait pas — et le lui faire déclarer aurait
créé un champ que rien d'autre n'utilise, dont la fausseté serait indétectable.

**La table des verrous se nettoie.** Une entrée par stream jamais réclamée est une fuite qui ne se
voit qu'après des mois. Un verrou dont plus personne ne tient de référence est retiré, et un test
l'exerce sur un grand nombre de streams éphémères.

---

## Décision 3 — La sérialisation ordonne l'accès ; le contrôle de révision garde la correction

Les deux ne font pas double emploi, et aucun ne remplace l'autre.

La sérialisation répond à « qui écrit maintenant ». Le contrôle optimiste de `Expected` — déjà livré,
déjà testé — répond à « ce que tu croyais savoir est-il encore vrai ». Deux commandes concurrentes
sur le même stream font donc la queue **puis** la seconde découvre que sa révision attendue est
périmée : elle est refusée avec `Conflict`, qui porte l'état courant, comme `W20.a` l'exige déjà de
tout conflit.

**Motifs.** Retirer le contrôle de révision au motif que l'accès est sérialisé serait faux dès qu'il
y a plus d'un processus — un second `locusd`, une migration, un outil d'exploitation. Et retirer la
sérialisation au motif que le contrôle de révision refuse les perdants produirait des rafales de
retentes sur un agrégat sollicité, c'est-à-dire du travail jeté.

Le refus rend l'état courant, jamais « conflit » seul : un client qui doit relire pour retenter a
besoin de savoir **quoi** relire.

---

## Décision 4 — L'idempotence du client devient un fait du journal

Le registre des clés d'idempotence de §22.5 cesse d'être une mémoire de processus. Il se **reconstruit
depuis le journal**, comme les quatre projections de §9.5.

**Motifs, et la promesse qui était fausse.** Le registre vit aujourd'hui dans `Transaction`, en
mémoire vive. Un redémarrage l'oublie — et un redémarrage est précisément ce qui coupe les connexions
et déclenche les retentes. La garantie promise au client était donc fausse **au moment exact où elle
sert**. Au sens de l'ADR 0022 décision 0, c'est une promesse : un mécanisme qui annonce un effet qui
n'a pas toujours lieu.

L'invariant 2 donne la forme de la réparation : le journal est la vérité institutionnelle. Un
registre qui vit ailleurs et qui prétend le résumer est un second stockage durable, ce que l'ADR 0019
a déjà refusé pour la messagerie.

**La migration est minuscule, et c'est délibéré.** L'enveloppe de §10.1 porte déjà le `workspace_id`
et l'`actor.principal_id`, c'est-à-dire **toute la portée** que `IdempotencyScope` définit. Il ne
manque que la clé choisie par le client. Un seul champ facultatif entre donc — `idempotency_key` —,
et le registre devient une projection qui le lit.

Facultatif, et le mot est chargé : un événement écrit avant cette migration n'en porte pas, et se
relit **sans clé** plutôt qu'avec une clé vide. Une commande dont la clé est inconnue n'est pas une
commande dont la clé est `""` — c'est la règle que `W21.m` a posée pour la classification de dépense
et `W22.e` pour les sondes d'hôte : une absence n'est pas une valeur.

---

## Décision 5 — La durée de rétention d'une clé est une valeur de politique

Le registre ne peut pas grossir indéfiniment. §22.5 dit que les clés « expirent selon la catégorie ».
La catégorie n'existe pas encore, donc **rien n'expire dans cette version**, et le dire est le point :
inventer ici une durée en Rust lui donnerait l'apparence d'un fait mesuré alors que c'est une
décision.

C'est la décision 9 de l'ADR 0024 appliquée hors des métriques : ce qui transforme une quantité en
règle est le moteur de politique, où le seuil se voit, se discute et se change.

**Conséquence assumée et écrite :** en V1, le registre croît avec le journal. C'est acceptable parce
qu'il se reconstruit et ne se stocke pas séparément — sa taille est celle de ce que le journal porte
déjà.

---

## Décision 6 — Une borne explicite, et un refus typé quand elle est franchie

Le nombre d'écritures en attente est borné. Au-delà, la commande est refusée sous la famille
`unavailable` des huit de §22.5, en nommant la borne.

**Motifs.** Une attente sans limite est une panne qui ne se déclare pas. Sous charge, tout le monde
attendrait et personne ne saurait pourquoi — c'est exactement ce que le dépôt refuse partout ailleurs,
des timeouts de `CLAUDE.md` aux bornes de taille de l'ADR 0028 décision 7.

`W20.a` a déjà livré la famille d'erreur : le refus a sa forme, il n'y a rien à inventer. Et
`unavailable` dit ce qu'il faut au client — retente plus tard —, là où `internal` l'enverrait ouvrir
un ticket et `validation` chercher une faute dans sa requête.

**La borne est un fait du service, pas un réglage caché.** Elle se lit dans le refus, et un exploitant
qui la voit franchie sait qu'il regarde une saturation et non une lenteur.

---

## Décision 7 — Ce qui n'est pas construit, et pourquoi

**Aucun écrivain-acteur, aucune file durable.** Un fil dédié possédant la transaction rendrait la
règle « toute mutation passe par un command handler » structurelle plutôt que testée, ce qui est
séduisant. Mais il faudrait un écrivain **par stream** pour tenir la décision 2, donc toute la
complexité des verrous **plus** un cycle de vie de fils à superviser — pour un daemon qui ne sert
encore aucune commande. La forme reste possible plus tard : elle se substitue derrière la même
signature.

**Aucun ordre total global.** `Expected` est par stream et l'a toujours été. Il n'y a rien à
partitionner, et la sérialisation observée dans l'audit du 2026-08-21 était de runtime, pas de
journal.

**Aucune expiration de clé.** Voir décision 5.

**Aucun second `locusd`.** Cette version suppose un seul processus écrivain. Ce n'est pas une
hypothèse cachée : le contrôle de révision de la décision 3 reste correct à plusieurs, et c'est
précisément pourquoi il n'est pas retiré. Ce qui manquerait à plusieurs est le partage de la table de
verrous, et ce jour-là c'est le journal qui l'arbitrerait — pas un verrou de processus.

---

## Conséquences

`packages/event-store` change une signature publique ; son seul implémenteur est le backend mémoire,
et les appelants qui tiennent un `&mut` continuent de compiler. Un test de contrat concurrent entre,
et `W20.i` le rejouera.

`apps/locusd` gagne le chemin d'écriture : §22.3 devient servable, ce que `W20.g` avait explicitement
laissé de côté. `Runtime::transaction` cesse d'exiger `&mut self`, donc la couche HTTP peut écrire
depuis son `Arc<Runtime>`.

Le registre d'idempotence devient une cinquième projection à côté des quatre de §9.5, et se reconstruit
au démarrage comme elles.

## Plan de rollback

La décision 1 se défait en rendant `&mut self` à `append` et en retirant le verrou interne du backend
mémoire : le code redevient exactement ce qu'il était, au prix de rendre §22.3 inservable. Les
décisions 2, 3 et 6 sont des ajouts dans `locusd` et se retirent par un diff.

La décision 4 est la seule migration, et c'est la seule dont le rollback demande une précaution : un
champ facultatif retiré rend illisibles les enveloppes qui le portent, sauf si la désérialisation
l'ignore. Elle entre donc en `#[serde(default)]` **des deux côtés** — un lecteur ancien ignore le
champ, un lecteur neuf accepte son absence —, et c'est ce qui rend le retour arrière possible sans
réécrire le journal. La garantie perdue au rollback est nommée : l'idempotence du client cesse de
survivre à un redémarrage, et elle doit alors être **déclarée absente** plutôt que promise en silence.
