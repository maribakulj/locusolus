# ADR 0021 — Une seule grammaire de mutation de coordination

**Statut :** accepté. **Amende** l'ADR 0016 décision 3 sur un point — le payload de `team.modify` —
et rouvre `W13.c` et `W13.e`, livrés et verts. Ouvre `W13.h` à `W13.j`. Débloque `W17.i`.

**Contexte.** `W17.i` demande que le commit d'une version de coordination écrive un fait. En
instruisant **où** ce fait s'écrit, deux vocabulaires apparaissent pour le même acte, tous deux dans
du code livré, tous deux dans le même crate :

| L'acte | `proposal::Change` (W13.e) | `version::Operation` (W15.a) |
| --- | --- | --- |
| un agent entre | `AddMember(Id<Agent>)` | `AddNode(Id<Agent>)` |
| un agent sort | `RemoveMember(Id<Agent>)` | `RemoveNode(Id<Agent>)` |
| une relation entre | `AddRelation(Relation)` | `AddEdge(Relation)` |
| une relation sort | `RemoveRelation(Relation)` | `RemoveEdge(Relation)` |
| le mode change | `SetMode { from, to }` | — |
| scinder, fusionner, remplacer, poser un rôle | — | quatre opérations |

`CLAUDE.md` interdit le vocabulaire parallèle, `proposal.rs` dit porter « le chemin **unique** par
lequel une proposition devient un fait », et l'ADR 0016 décision 2 dit que deux vocabulaires de
gouvernance pour la même chose sont « deux constitutions pour une république ». La question ne
pouvait donc pas être tranchée en passant.

---

## L'analyse, parce que la symétrie du tableau est trompeuse

Le tableau ci-dessus suggère deux énumérations qui se valent, dont l'une aurait des exclusives et
l'autre une. Le dépôt dit autre chose, et il le dit par les **consommateurs**.

### `Operation` est honorée cinq fois, et jusque dans le démon

`Version::apply` l'applique ; `Diff` la compose et la rejoue ; `region::threatens` la confronte aux
`allowed_ops` d'une région ; `Barriers::admits` en décide l'admission ; `metrics` mesure ce qu'elle
produit ; et `locusd::branch::DiffView` la sert sur `/branches/:id/diff`. Une opération écrite se
traduit par un contenu qui change, un condensat qui change, un veto qui se déclenche ou une réponse
HTTP qui diffère.

### `Change` n'est honorée par rien

Sept sites d'usage en tout, dont cinq dans son propre fichier de test. `Proposal` la porte ;
`commit()` la reçoit, **ne l'applique à rien**, et rend `Committed { proposal, revision + 1 }`. Aucun
`Team` n'est modifié, aucune `Version` n'est produite. Une proposition qui déclare `AddMember` puis
commite laisse le système exactement dans l'état où elle l'a trouvé.

C'est très précisément ce que l'ADR 0016 décision 4 refuse : « des relations que le système sait
versionner, différencier, approuver et afficher, et que **rien n'honore** — un graphe décoratif, pire
qu'un graphe absent parce qu'un humain croira l'avoir modifié ». La décision 4 l'écrivait des sortes
de relation ; elle vaut à l'identique d'une grammaire de changement.

### `Team` non plus n'est lu

`Team` porte `members`, `mode`, `coordinator` et `revision` en champs **privés**, et expose
`new()` et `title()`. Rien ne peut lire ses membres, personne ne peut les changer, et son unique
consommateur est son propre fichier de test. Ce n'est pas un agrégat : c'est un constructeur qui
valide puis se tait.

### Ce que le tableau cachait

La duplication n'est pas symétrique. D'un côté une grammaire vivante et la structure qu'elle fait
bouger ; de l'autre une grammaire inerte et une structure que personne ne lit. Le choix n'est donc
pas « laquelle des deux garder », mais « qu'est-ce qui manque pour qu'il n'y en ait qu'une ».

---

## Décision 1 — `Operation` est la grammaire, `Change` disparaît

Un seul jeu d'opérations décrit ce qu'une mutation de coordination fait, et c'est celui de
`docs/13` §3, « tiré de la revue ». `proposal::Change` est retirée.

**Motifs.** L'une a des consommateurs dans cinq modules et le démon ; l'autre n'en a aucun. Garder
les deux, c'est garder la duplication ; garder `Change`, c'est jeter le code qui marche. Il n'y a pas
de troisième lecture.

Et §22.3 ne dit rien du payload de `team.modify` : il nomme la commande. L'ADR 0016 décision 3 dit
que la proposition **est** ce payload et énumère ce qu'elle en garde — `trigger`, `rationale`,
`evidence_refs`, `proposer.kind` — sans jamais nommer de grammaire de changement. `Change` n'a donc
pas de source normative : elle a été inventée par `W13.e` pour remplir une case que l'ADR laissait
ouverte, et `Operation` est arrivée deux workstreams plus tard pour remplir la même.

---

## Décision 2 — La proposition porte un `Diff`, et le commit produit une `Version`

`Proposal` porte un `Diff` d'`Operation`s à la place de ses `Change`s. `commit()` rejoue ce diff sur
la version courante et rend la version suivante, dont le parent est celle d'avant.

**Motifs.** C'est ce que l'ADR 0016 décision 5 décrivait déjà — « une nouvelle version dont le parent
est la version courante » — et que le code ne faisait pas. `Diff::replay` existe, est testée, et
refuse une base périmée en disant qu'il faut rebaser. Le CAS par `expected_revision` ne disparaît
pas : il reste la concurrence optimistique de §22.2, et le condensat reste l'identité de contenu. Les
deux ne se remplacent pas — l'un dit « personne n'a écrit entre-temps », l'autre dit « voici quoi ».

---

## Décision 3 — La structure vit dans la `Version` ; `Team` la projette

`Version` porte les membres, les relations, les rôles, **le mode** et **le coordinateur**. `Team`
cesse de les stocker et les **sert** depuis la version courante.

**Motifs.** `Team.members` et `Version.members` sont deux stockages du même fait, dans le même crate.
Deux stockages du même fait sont deux vérités, qui divergent le jour où l'une est corrigée — c'est
l'argument par lequel l'ADR 0019 a écarté le courtier de messages, et il ne dépend pas du sujet.

§7.1 donne bien `member_ids` et `coordination_mode` à `Team`, et `CLAUDE.md` exige les objets de
§7.1 sous leur nom. Ils y restent : `docs/13` §3 nomme la taxonomie qui le permet — « version
canonique immuable avec hash et parent, **graphe réalisé comme projection**, trace comme histoire ».
La `Version` est le canonique ; `Team` est le réalisé. Servir `member_ids` depuis la version courante
est cette taxonomie appliquée, pas une dérogation à §7.1.

**`SET_MODE` et `SET_COORDINATOR` entrent ensemble, et sous la règle de la décision 4 de l'ADR
0016** — une sémantique n'entre que si un consommateur exécutable existe. §14.3 est ce
consommateur, et il les lie : le mode `coordinator` n'est bien formé que si quelqu'un coordonne, ce
que `Team::new` vérifiait déjà et que la `Version` vérifiera. Les inscrire séparément aurait permis
un état que §14.3 déclare impossible.

Les deux sont des opérations **attributaires**, au sens que `version.rs` a déjà fixé pour `SET_ROLE` :
elles écrivent un champ dont le lecteur vit ailleurs, et elles n'entrent que parce que ce lecteur
existe. La conséquence que `SET_ROLE` porte vaut pour elles — retirer ou remplacer un nœud qui est
coordinateur est refusé, parce que l'opération inverse ne saurait pas le rendre.

---

## Conditions, sans lesquelles la décision est mauvaise

1. **Aucun second stockage de la structure.** Ni dans `Team`, ni ailleurs. Un test d'absence tient la
   règle, comme `W20.b` le fait pour les écritures.
2. **`Team` garde ce que §7.1 lui donne et qui n'est pas de la structure** — identité, branche,
   titre, politiques, état, révision. Le retirer serait sortir de §7.1, ce que `CLAUDE.md` interdit
   et que cet ADR ne fait pas.
3. **Les validations ne se perdent pas en déménageant.** `NoMembers`, `CoordinatorNotAMember`,
   `CoordinatorRequired` sont des règles de §14.3, pas des détails de `Team::new` : elles deviennent
   des refus de `Version`, et les tests qui les tenaient sont réécrits plutôt que supprimés. Une
   validation qui disparaît pendant un refactor est le mode d'échec de ce genre de chantier.
4. **Le commit reste transactionnel.** `W20.b` n'est pas contourné : le fait s'écrit par un `Decide`,
   et `W17.i` en est la livraison.

---

## Ce qui est rouvert, et ce que ça coûte

`W13.c` (agrégats) et `W13.e` (relation, payload de `team.modify`, CAS, annulation) sont marqués
**fait** et leurs tests passent. Ils ne sont pas faux : ils sont **débranchés**. Le chantier les
rebranche plutôt qu'il ne les corrige, et il se fait en quatre items pour que chacun ait un test de
sortie observable :

- `W13.h` — `Version` porte le mode et le coordinateur ; `SET_MODE` et `SET_COORDINATOR` entrent avec
  leurs refus, dont les trois règles de §14.3 déménagées depuis `Team::new`.
- `W13.i` — `Proposal` porte un `Diff` ; `Change` est retirée ; `commit()` rend une `Version`.
- `W13.j` — `Team` projette la version courante au lieu de la stocker, et expose enfin ce que §7.1
  lui donne.
- `W17.i` — le commit écrit son fait, par un `Decide`.

**Rollback.** Chaque item est un commit séparé et réversible. Le plus cher à défaire est `W13.i`, qui
retire un type public ; les trois autres sont additifs ou internes. Aucun événement n'est encore
écrit sous ce chemin — `W17.i` est précisément ce qui commencera à en écrire — donc aucune donnée
n'est en jeu, et c'est la raison de faire ce chantier **maintenant** plutôt qu'après.

---

## Alternative écartée : scoper les deux vocabulaires

`Change` pour l'agrégat `Team` de §7.1, `Operation` pour le graphe versionné de `docs/13` §3, et une
phrase qui les sépare. C'était la lecture la moins chère, et elle a été sérieusement instruite :
chacune a des exclusives que l'autre n'exprime pas, ce qui ressemble au signe que deux objets
distincts sont décrits.

Écartée parce que les exclusives ne tiennent pas à l'examen. Celles d'`Operation` — scinder,
fusionner, remplacer, poser un rôle — sont des actes sur **les mêmes membres** que ceux de `Change` ;
elles ne décrivent pas un autre objet, elles décrivent plus finement le même. Et l'exclusive de
`Change`, `SetMode`, porte sur un champ que §14.3 lie au coordinateur, donc à un membre. Deux
grammaires dont l'une est un sous-ensemble strict de l'autre, à un attribut près, ne sont pas deux
objets : c'est une grammaire et un doublon appauvri.

La phrase qui les aurait séparées aurait donc été une phrase fausse, et `CLAUDE.md` demande qu'une
propriété écrite soit une propriété tenue.

## Alternative écartée : garder `Change` et lui donner des consommateurs

Symétrique de la décision 1 : faire de `Change` la grammaire, et réécrire `Version`, `Diff`,
`region`, `metrics` et `DiffView` sur elle. Écartée sans hésitation — il faudrait ajouter à `Change`
les quatre opérations qui lui manquent, c'est-à-dire reconstruire `Operation` sous un autre nom, en
jetant cinq modules testés au passage.
