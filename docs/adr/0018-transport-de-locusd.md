# ADR 0018 — Transport de `locusd` : runtime asynchrone et cadre HTTP

**Statut :** accepté. Débloque `W20.d`. Autorise les premières dépendances externes de `apps/locusd`
au-delà de `serde`, et rien d'autre : la liste est close et vérifiée par `check:deps`.

**Contexte.** `SPEC_V1.md` §22 décrit une API — commandes, queries, événements clients — et §22.1
nomme WebSocket et SSE pour le fil d'événements. `W20.a` et `W20.b` ont livré la moitié domaine de ce
contrat : la forme d'une commande, la forme d'un refus, le handler transactionnel. Aucune de ces
livraisons n'a eu besoin d'un transport, et c'était l'objet du découpage — `CLAUDE.md` demande de
« construire domain/protocol/event-store d'abord, avec des ports purs ».

Le moment est venu parce que `W20.d` câble un composition root, et qu'un composition root sans
transport ne se distingue pas d'une bibliothèque.

**La surface actuelle, mesurée et non supposée.** Le workspace Rust entier déclare **deux** crates
externes, `serde` et `serde_json`. `cargo tree` sur `locusd` rend **11** paquets transitifs. C'est
une surface qu'un humain relit, et l'ADR 0011 en a fait un de ses motifs : « la surface de
dépendances d'un control plane dont la raison d'être est la provenance et l'attestation gagne à
rester auditable ». Toute décision de transport détruit cette propriété ou la ménage ; il faut donc
savoir de combien.

| Configuration | Paquets transitifs |
| --- | --- |
| `locusd` aujourd'hui | **11** |
| `tokio` seul, features `rt-multi-thread,net,macros,time,sync` | 11 |
| `axum` + `tokio` features `full` | **56** |
| `axum` avec la feature `ws` + `tokio` `full` | **76** |

Mesuré le 2026-08-19 avec `cargo tree --edges normal`, sur `axum 0.8.9` et `tokio 1.53.1`.

Le troisième chiffre est celui qui décide quelque chose. **La feature `ws` coûte vingt paquets à elle
seule** — `tokio-tungstenite`, `tungstenite`, `sha1`, `rand`, `getrandom`, `digest`, `typenum`,
`generic-array`, `crypto-common`, `block-buffer`, `base64`, `data-encoding`, `zerocopy`,
`ppv-lite86`, `rand_chacha`, `rand_core`, `cpufeatures`, `futures-sink`, `thiserror`,
`thiserror-impl`. Ce ne sont pas des dépendances gratuites : le handshake WebSocket exige un SHA-1 et
un générateur aléatoire, et ils traînent leurs tours de traits.

**Décision.**

1. **`tokio` comme runtime asynchrone**, avec des features **nommées une par une**, jamais `full`.
2. **`axum` comme cadre HTTP.**
3. **La feature `ws` n'entre pas avec `W20.d`.** Elle est différée à `W20.f`, l'item qui livre le fil
   d'événements de §22.1, et son entrée y sera pesée contre SSE — que §22.1 nomme comme alternative,
   et qui ne coûte aucun des vingt paquets ci-dessus.
4. **La liste des dépendances externes autorisées devient un fichier**, `dependencies.json`, où
   chaque crate porte l'ADR qui l'autorise. `check:deps` échoue sur toute dépendance externe absente
   de la liste.

**Motifs.**

`tokio` est le runtime sur lequel l'écosystème asynchrone Rust s'est aligné : `hyper`, et donc tout
serveur HTTP sérieux, en dépend. Choisir autre chose ne réduirait pas la surface, cela ajouterait une
couche d'adaptation à une surface déjà présente par transitivité.

`axum` est bâti sur `hyper` et `tower`. Ce qui le distingue ici n'est pas l'ergonomie mais la
**forme** : un handler `axum` est une fonction qui prend des extracteurs et rend une réponse. C'est
exactement la forme de `Decide::decide`, qui prend une commande et un état et rend des événements. La
couche de transport se réduit donc à une traduction, et `W20.b` a déjà posé la règle qu'elle ne peut
pas faire davantage — elle n'a pas de journal en main.

`tower` apporte les middlewares — timeout, limite de concurrence, tracing — que `CLAUDE.md` exige
sous « timeouts et cancellation » et « logs corrélés sans secrets », sans qu'il faille les écrire.

**Conditions, sans lesquelles la décision est mauvaise.**

1. **Les features de `tokio` sont énumérées, jamais `full`.** `full` est la différence entre un
   runtime et un système d'exploitation portable : il apporte `process`, `signal`, `fs`, `io-std`,
   dont un control plane n'a pas l'usage — et dont `locusd` **ne doit pas** avoir l'usage, puisque
   c'est `locus-execd` qui parle aux processus (ADR 0004). Un `full` dans `Cargo.toml` mettrait entre
   les mains de `locusd` exactement ce que la règle 4 de `boundaries.json` lui interdit.
2. **`axum` n'entre pas dans `packages/`.** Il entre dans `apps/locusd`, et seulement là. Un
   extracteur `axum` dans un package de domaine ferait dépendre le domaine du cadre HTTP, ce que
   l'invariant 1 interdit. La règle est vérifiée par `check:deps`, qui lit chaque `Cargo.toml`.
3. **Le domaine ne devient pas asynchrone.** `Decide::decide` et `Transaction::submit` restent
   synchrones. Une méthode `async` dans le port du handler propagerait le runtime dans tout le
   domaine et rendrait `W20.a` et `W20.b` dépendants d'un choix que cet ADR vient à peine de faire —
   l'inverse exact de l'ordre que `CLAUDE.md` impose. Le passage à l'asynchrone se fait **au bord**,
   dans la couche `axum`, qui appelle un domaine synchrone sur un pool bloquant si nécessaire.
4. **Le fil d'événements est réexaminé à `W20.f`, pas décidé ici.** SSE suffit à §22.1 si le fil
   client est unidirectionnel ; WebSocket est nécessaire s'il ne l'est pas. La question se tranche
   avec le besoin sous les yeux, et les vingt paquets sont le prix qu'elle met en jeu.

**Conséquences.**

`dependencies.json` et `check:deps` naissent avec cet ADR, et la douzième porte entre dans
`npm run check`. Le fichier est vide de tout crate de transport tant que `W20.d` n'a pas commencé :
l'ADR autorise, il n'ajoute pas.

La CI compile un arbre de dépendances qui passe de 11 à une cinquantaine de paquets. Le temps de
build augmente ; c'est le coût annoncé, et il se paie une fois par cache.

`locus-execd` n'est pas concerné. Il n'a pas de surface HTTP, et cet ADR ne lui en donne pas.

**Alternative écartée : pas de cadre, `hyper` nu.** Elle réduirait la surface d'une quinzaine de
paquets. Écartée parce que ce qu'on n'importe pas, on l'écrit : routage, extraction, gestion des
erreurs, et surtout la sérialisation des refus de §22.5 vers des statuts HTTP. Ce code-là serait
moins relu que celui d'`axum`, et il porterait la partie du système où une erreur se voit le moins —
un refus rendu avec le mauvais statut ressemble à un service en panne.

**Alternative écartée : `actix-web`.** Comparable en maturité. Écartée sur un point non technique et
assumé : `axum` partage `hyper` et `tower` avec le reste de l'écosystème, donc son graphe de
dépendances **recoupe** celui de ce qui viendra — un client HTTP, un exporteur de traces — au lieu de
s'y ajouter.

**Condition de réexamen.** Si la traduction entre le domaine synchrone et le bord asynchrone se
révèle coûteuse au point d'imposer un port `async` — c'est-à-dire si la condition 3 devient
intenable — la décision est mauvaise et l'ADR est réexaminé plutôt que la condition abandonnée en
silence.

**Rollback.** Tant que `W20.d` n'a pas livré, annuler cet ADR ne coûte que la suppression de trois
lignes de `Cargo.toml` et d'une entrée de `dependencies.json` : `W20.a` et `W20.b` n'ont aucune
dépendance de transport, et c'est précisément pour cela qu'ils ont été écrits d'abord. Après `W20.d`,
le coût est celui de la réécriture de la couche de bord — qui reste bornée, puisque la condition 3
interdit au runtime d'entrer dans le domaine. C'est la seconde raison d'être de cette condition : elle
protège la décision d'aujourd'hui contre son propre coût de sortie.
