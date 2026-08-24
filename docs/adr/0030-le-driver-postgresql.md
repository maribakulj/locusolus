# ADR 0030 — Le driver PostgreSQL du journal, et l'ordre global qu'il coûte

**Statut :** accepté. **Amende** la documentation de `EventStore::append` sur un point mesuré ici.
Ouvre `W20.i`.

**Contexte.** `packages/event-store` définit le port du journal canonique et son implémentation de
référence en mémoire. `W1.c` a écrit la suite de contract tests qui définit ce que « journal » veut
dire ici, et elle affirme dans sa propre documentation qu'elle est « écrite contre le **port**,
jamais contre l'implémentation en mémoire : le jour où un second journal existe, cette suite tourne
sur lui **sans être modifiée** ». Ce document commence par vérifier cette affirmation, parce que
c'est elle qui décide de la forme de l'item.

---

## Décision 0 — L'affirmation de `W1.c` était fausse, et de peu

La suite est bien écrite contre les **méthodes** du port. Elle est aussi écrite contre **un
constructeur** : `MemoryEventStore::new()` apparaît quatorze fois, une par test. Elle ne pouvait donc
pas « tourner sur un second journal sans être modifiée » — il fallait changer quatorze lignes.

L'écart est petit et la leçon ne l'est pas. Une suite paramétrée par son backend l'aurait été dès
`W1.c` si quelqu'un avait essayé ; personne n'avait de second backend, donc la propriété est restée
**affirmée et jamais éprouvée** — le motif de l'ADR 0025, dans la suite de tests qui sert de juge à
tout le reste.

La correction est celle que l'affirmation décrivait : les corps de test sont paramétrés par une
fabrique, et **chaque test s'exécute deux fois**, une par backend. Ce qui change, ce sont les
quatorze constructeurs ; aucune assertion n'est touchée, et c'est ce qui permet de dire que le driver
passe « la même » suite plutôt qu'une suite adaptée à lui.

---

## Décision 1 — `postgres` (synchrone), et non `tokio-postgres` ni `sqlx`

Le port est **synchrone** : `fn append(&self, …) -> Result<…>`. Trois façons de le servir, et une
seule qui ne demande pas de refaire le port.

**Le coût est mesuré, pas estimé** — arbre de dépendances complet, feature `with-serde_json-1`, sans
TLS :

| Candidat | Paquets transitifs | Ce qu'il demanderait au port |
| --- | --- | --- |
| `postgres` 0.19 | **63** | rien |
| `tokio-postgres` 0.7 | 62 | rendre `EventStore` asynchrone — donc `packages/projections`, `apps/locusd`, et la suite de contract tests |
| `sqlx` (postgres, tokio, json, sans TLS) | 115 | idem, plus une macro de vérification à la compilation qui exige une base au build |

`postgres` est le wrapper synchrone de `tokio-postgres` : il coûte **un paquet de plus** que le
client asynchrone et évite de propager `async` dans tout le workspace. `sqlx` coûte presque le double
et son principal argument — la vérification des requêtes à la compilation — exige une base
accessible au moment du build, ce qui rendrait la CI dépendante d'un service pour **compiler** et non
seulement pour tester.

**Ce que ce choix reporte.** Un `locusd` qui servirait ses écritures sur ce driver bloquerait un fil
du runtime asynchrone à chaque appel. Ce n'est pas un problème aujourd'hui — `W20.i` ne câble pas le
driver dans `locusd`, l'implémentation mémoire restant le backend de test et d'exécution — et le jour
où il le sera, la réponse est `tokio::task::spawn_blocking`, pas un changement de port. Le dire ici
évite de le redécouvrir.

---

## Décision 2 — L'ordre global se paie, et la documentation du port le disait autrement

`EventStore::append` documente ceci depuis l'ADR 0029 :

> « Le backend mémoire prend un verrou global et le documente ; un driver relationnel **s'en remettra
> au verrouillage de ligne, qui laisse deux streams distincts avancer ensemble.** »

C'était une prévision, écrite avant qu'un driver relationnel existe. Elle est **vraie pour la
concurrence optimiste** — la contrainte d'unicité sur `(stream_id, revision)` fait exactement cela,
et deux streams distincts ne s'y rencontrent jamais. Elle est **fausse pour l'ordre global**, et
c'est ce que cet ADR amende.

`Sequenced::position` est « le rang dans l'ordre d'écriture global, à partir de 1 », et les
projections s'en servent comme filigrane : elles demandent « ce qui est arrivé après *n* ». Une
séquence PostgreSQL (`bigserial`) ne donne pas cette garantie. Les numéros sont attribués **avant**
le commit et les transactions valident dans un ordre quelconque : un lecteur peut voir 1, 2, 4,
avancer son filigrane à 4, et ne **jamais** voir 3 quand sa transaction valide ensuite. Un événement
écrit mais invisible aux projections est pire qu'une écriture refusée — c'est précisément ce que le
dernier test de concurrence de `W1.c` protège en exigeant que le flux global porte chaque événement.

Trois réponses possibles :

1. **Tolérer les trous, et rendre le filigrane sûr** en ne servant que les positions inférieures au
   plus ancien identifiant de transaction en cours (`pg_snapshot_xmin`). Correct, et c'est la
   solution du champ. Elle ajoute une subtilité que rien ne demande encore.
2. **Assigner la position sous un verrou de ligne** : un compteur à une seule ligne, incrémenté par
   `UPDATE … RETURNING` dans la transaction d'écriture. Le verrou est tenu jusqu'au commit, donc les
   positions sont attribuées dans l'ordre des commits, sans trou.
3. **Renoncer à l'ordre global.** Écarté : les projections et les cursors de §22.6 en dépendent.

**La 2 est retenue.** Elle rend le driver **plus sérialisé** que la prévision de l'ADR 0029 ne le
laissait croire — les écritures se suivent au commit, comme dans le backend mémoire, qui le dit
aussi de lui-même. Ce n'est donc pas une régression par rapport à ce qui existe : c'est une prévision
corrigée.

**Condition de révision.** Passer à la solution 1 le jour où une **mesure** montre que l'attribution
de position est le goulot — pas avant. La mesure manque aujourd'hui, et optimiser sans elle
choisirait la complexité sur une intuition.

---

## Décision 3 — Le schéma, et ce qu'il rend impossible

Deux tables, et les contraintes portent les garanties de §10.2 plutôt que le code :

- `event` — une ligne par événement. `unique (stream_id, stream_revision)` **est** la concurrence
  optimiste : deux écrivains qui visent la même révision ne peuvent pas gagner tous les deux, quoi
  que fasse le code au-dessus. `unique (position)` porte l'ordre global.
- `command_applied` — une ligne par commande, clé primaire `command_id`, portant l'empreinte du lot
  et sa révision. C'est l'idempotence de §10.2, et la clé primaire la rend inviolable.

**Aucune migration destructive.** Le schéma se crée par `create table if not exists` et rien dans ce
driver n'écrit `update`, `delete`, `truncate` ni `drop` sur `event` : l'immutabilité logique de §10.2
est tenue par **l'absence**, comme dans le backend mémoire, et un test la vérifie en lisant le source.

---

## Décision 4 — Ce qui n'est pas vérifié n'est pas réussi, et un test sauté le dit

Le driver ne se teste pas sans base. Deux façons de le gérer, et une seule est admise ici.

Un test qui se **sauterait en silence** quand `LOCUS_TEST_POSTGRES` est absent rendrait « vert » un
dépôt où le driver n'a jamais tourné, et personne ne verrait la différence entre « conforme » et
« pas éprouvé ». C'est la faute que ce dépôt traque partout ailleurs sous le nom « un compteur qui
n'a rien lu ne vaut pas zéro ».

La règle retenue : la suite s'exécute contre les deux backends **quand la variable est présente**, et
**imprime ce qu'elle n'a pas fait** quand elle est absente. La CI, elle, la fournit : le job `rust`
gagne un service `postgres`, donc le chemin sauté n'existe pas là où le verdict compte. Un
développeur sans base voit ce qu'il n'a pas éprouvé, écrit noir sur blanc.

---

## Décision 5 — Plan de rollback

Le driver est **additif** : il n'est câblé nulle part. `Runtime::in_memory()` reste l'assemblage du
profil `personal-local`, `Runtime::assemble` reste générique sur `S: EventStore`, et aucun appelant
existant ne change de backend.

Le retrait tient donc en trois gestes, sans migration de données puisqu'aucune donnée de production
ne transite encore : retirer `postgres` de `packages/event-store/Cargo.toml` et de
`dependencies.json`, supprimer `src/postgres.rs` et son export, retirer le service `postgres` du job
`rust`. La suite de contract tests reste paramétrée et retombe sur le seul backend mémoire — ce qui
est exactement son état d'avant, en mieux : la paramétrisation est ce que `W1.c` affirmait déjà.

Le jour où des données existent, ce plan ne suffira plus, et ce sera le sujet de l'item qui câblera
le driver — pas de celui-ci.
