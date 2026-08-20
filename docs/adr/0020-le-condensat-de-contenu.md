# ADR 0020 — Le condensat de contenu

**Statut :** accepté. Ajoute `sha2` à `dependencies.json`, portée `packages/domain`. Débloque la
chaîne qui va de `W17.h` à `/branches/{id}/diff`, et rend calculable le `payload_hash` que §10.1
exige déjà.

**Contexte, et il n'a pas été cherché — il a été rencontré.** `W17.g` voulait brancher les quatre
lectures de branche par HTTP. Une seule l'a pu, et remonter la chaîne des « pourquoi pas » a mené
plus bas que prévu :

```text
GET /branches/{id}/diff
  ← il faut une `Version` depuis une `VersionId`
  ← ADR 0016 décision 5 interdit un magasin : « aucun compteur, aucun magasin, aucun bus »,
    donc c'est un rejeu du journal
  ← il faut que les opérations soient dans le journal — rien ne les y écrit
  ← écrire un commit de version demande `Version::apply`, qui demande un `Digest`
  ← **aucun `Digest` n'existe**, et rien dans le workspace ne calcule un condensat.
```

**Le fait vérifié, et il est plus large que `W17`.** `ContentHash` **parse** et ne calcule jamais.
`Digest` est un trait déclaré deux fois — `coordination::version` et `visualization` — sans une seule
implémentation de production ; les seules qui existent sont des `Fnv` jouets, dans des tests, qui
disent d'ailleurs eux-mêmes qu'ils ne vérifient « pas la qualité du hachage ». Et les deux seuls
écrivains d'événements du dépôt — `locusd::branch` et `locusd::messaging` — reçoivent leur
`payload_hash` **de l'appelant**, tous les deux, en le documentant comme une lacune nommée.

Autrement dit : §10.1 exige un `payload_hash` sur chaque événement, et le système ne sait pas en
produire un. Un appelant peut aujourd'hui passer n'importe quelle chaîne de la bonne forme, et rien
ne s'en apercevrait. Ce n'est pas une dette de `W17` ; c'est un trou sous plusieurs items.

**Décision.**

1. **`sha2`, sans features par défaut, portée `packages/domain`.** Une seule implémentation, à un
   seul endroit — celui qui possède déjà `ContentHash`, donc l'identité de contenu.
2. **`ContentHash::of` est le seul chemin.** Les autres crates n'obtiennent pas `sha2` : ils
   obtiennent `ContentHash::of`. La portée étroite dans `dependencies.json` le rend opposable, et
   pas seulement recommandé.
3. **SHA-256, et pas un autre algorithme**, quand il s'agit de vérifier ce qui vient du dehors.
   `ContentHash` accepte trois algorithmes ; celui qu'on **calcule** est sha256.

**Motifs.**

**SHA-256 n'est pas un choix esthétique, c'est un choix d'interopérabilité.** Le dépôt écrit
`sha256:` partout où un condensat vient d'ailleurs : le digest d'image d'`EnvironmentBlueprint`,
qui refuse un tag « parce qu'une image par tag peut changer sous l'environnement », et qui vient du
registre OCI ; les artefacts de §19. Un système qui doit **vérifier** un digest fourni par un
registre doit calculer le même algorithme que le registre. Choisir blake3 obligerait à porter les
deux, et à décider à chaque appel lequel s'applique.

**La surface est mesurée, comme l'ADR 0018 l'a fait pour le transport.** `cargo tree`, sur ce
workspace, `sha2` sans features par défaut :

| Paquet | avant | après | delta |
| --- | --- | --- | --- |
| `locus-domain` | 13 | 21 | **+8** |
| `locusd` | 52 | 60 | **+8** |

Les huit : `sha2`, `digest`, `block-buffer`, `crypto-common`, `hybrid-array`, `typenum`,
`cpufeatures`, `cfg-if`. Tous en Rust pur, tous de RustCrypto, aucun code C, aucune dépendance
système.

**blake3 a été mesuré aussi, et écarté sur un motif qui n'est pas le compte.** Huit paquets
également — mais parmi eux `cc`, `shlex` et `find-msvc-tools` : blake3 compile du C et de
l'assembleur, donc exige un compilateur C sur toute machine qui construit le projet. `CLAUDE.md`
refuse « toute dépendance implicite à une machine de développeur », et un compilateur C en est une.
Sa variante `pure` lèverait l'objection et perdrait ce qui rend blake3 intéressant.

**Écrire SHA-256 à la main a été considéré**, comme l'encodage hexadécimal de `cursor.rs` l'a été et
retenu. La comparaison tranche dans l'autre sens, et le motif est net : douze lignes d'hexadécimal se
relisent et se vérifient à l'œil ; une compression SHA-256 fait soixante-quatre tours, des constantes
tabulées, et une faute d'un bit y produit un condensat qui a **exactement l'air correct**. Une
primitive cryptographique fausse ne se voit pas — c'est précisément ce qui la distingue d'un encodage.

**Conditions, sans lesquelles la décision est mauvaise.**

1. **La portée reste `packages/domain`.** Une seconde entrée de `sha2` ailleurs signifierait une
   seconde implémentation, donc deux réponses possibles à « quel est le condensat de ceci ». Un test
   d'absence tient la règle, et `check:deps` refuse déjà la portée non déclarée.
2. **`ContentHash::of` est déterministe et sur des octets.** Elle prend ce qu'on lui donne et ne
   canonicalise rien : la forme canonique est la responsabilité de l'appelant, et
   `coordination::version` en a déjà une, écrite et gelée par un test de fixture. Mêler les deux
   ferait dépendre l'identité d'une version d'un détail de la fonction de hachage.
3. **Aucun usage en sécurité sans un ADR de plus.** Un condensat de contenu n'est ni un MAC, ni une
   signature, ni un jeton. `cursor.rs` dit déjà pourquoi son cursor n'est pas protégé par un MAC ;
   cet ADR n'ouvre pas ce sujet, et l'arrivée de `sha2` ne doit pas servir d'occasion pour le
   rouvrir en passant.
4. **`payload_hash` reste fourni par l'appelant tant qu'un item ne le reprend pas.** Rendre
   `ContentHash::of` disponible ne corrige pas les deux écrivains d'événements ; les corriger est un
   item, et le faire dans celui-ci mélangerait une décision de dépendance avec un changement de
   contrat d'écriture.

**Conséquences.**

`W17.h` cesse d'être « un résolveur de versions » et redevient ce qu'il est réellement : d'abord un
**producteur**. La chaîne est inscrite au plan, dans l'ordre où elle se dénoue, plutôt que devinée
une marche à la fois.

Le trou de `payload_hash` est nommé, et il ne l'était pas. Deux modules le documentaient chacun comme
une lacune locale ; c'était la même, et elle avait une cause commune.

**Rollback.** Retirer l'entrée de `dependencies.json`, la ligne du manifeste et `ContentHash::of`.
Aucun format n'est figé par cet ADR : un condensat calculé est une chaîne que `ContentHash::parse`
lisait déjà. Ce qui aurait été écrit avec resterait lisible — et c'est le point d'avoir choisi
l'algorithme que le reste du monde écrit.
