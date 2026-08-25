# ADR 0028 — Le lien entre `locusd` et `locus-execd` : un tube local, et rien de plus

**Statut :** accepté. Débloque `W4.h`. **N'autorise aucune dépendance externe nouvelle** — et c'est
la propriété principale de cette décision, pas un effet de bord.

**Contexte.** `W22.c` a découvert le quatrième maillon manquant de la fermeture verticale : **aucun
code du dépôt ne construit de client vers `locus-execd`, et le broker n'écoute rien.** Les deux
binaires existent, chacun est cohérent séparément, et il n'y a pas de couloir entre eux. C'est
précisément pourquoi l'absence ne se voyait dans aucun décompte d'items faits.

`locus-execd` existe pour une raison écrite dans son propre `lib.rs` : « `locusd` parle au monde
entier — cockpit, workers, pairs fédérés, contenu récupéré sur le Web. Un socket de runtime dans ce
processus-là donne à qui le compromet le pouvoir de créer des conteneurs privilégiés, c'est-à-dire
d'annuler tout le confinement par l'intérieur. » Toute décision de transport se juge contre cette
phrase.

**Les deux chiffres qui décident, mesurés le 2026-08-22 par `cargo tree --edges normal` :**

| Binaire | Paquets externes |
| --- | --- |
| `locusd` — exposé au monde | **53** |
| `locus-execd` — privilégié | **11** |

Le programme privilégié porte aujourd'hui **cinq fois moins** de code tiers que l'autre. C'est la
propriété que la séparation achète, et aucun transport ne doit l'inverser.

---

## Décision 1 — Une socket de domaine Unix, du JSON par lignes, derrière un port

`locusd` et `locus-execd` se parlent par une socket de domaine Unix. Les messages sont des objets
JSON terminés par un saut de ligne. Le tout vit derrière un **port** — un trait — dont la socket est
le premier et pour l'instant l'unique backend, exactement comme `packages/event-store` a un trait et
une implémentation mémoire avant tout driver.

**Motifs.**

*Zéro dépendance nouvelle, des deux côtés.* La bibliothèque standard de Rust porte
`std::os::unix::net`, et `serde_json` est déjà autorisé en portée `*` depuis l'ADR 0011. Le tableau
ci-dessus ne bouge pas d'une ligne, et `check:deps` le vérifie sans qu'on lui ajoute une entrée.

*Injoignable depuis le réseau par construction, pas par configuration.* Une socket locale n'a pas
d'adresse routable. Ce n'est pas une option qu'un exploitant peut mal régler ni qu'une régression
peut rouvrir : elle n'existe pas. Pour le seul processus privilégié du système, c'est la propriété
qui vaut le plus cher.

*Le format est déjà celui du dépôt.* `apps/locus-execd/src/wire.rs` traduit déjà les refus
d'admission vers des formes de fil JSON, et `packages/lep` porte déjà l'écriture des six niveaux de
confinement. Rien de nouveau n'est inventé.

**Les trois options écartées, et pourquoi.**

**HTTP dans la socket locale.** Aurait demandé un serveur HTTP dans `locus-execd`, qui serait passé
de 11 à environ 53 paquets — **on aurait quintuplé la surface de code tiers du seul processus
privilégié pour un confort de format**, et il aurait fallu en plus donner un client HTTP à `locusd`,
qui n'en a pas. Cette option a le coût de la suivante sans en avoir le bénéfice ; elle n'a aucune
version acceptable.

**HTTP sur TCP avec certificats mutuels.** C'est la seule qui couvre le profil `distributed-hybrid`
de §27.1, et `placement::place` anticipe déjà un monde à plusieurs hôtes. Mais elle ajoute au coût
précédent une pile de chiffrement et toute une gestion de certificats, **et elle rend le broker
joignable depuis le réseau**. On créerait délibérément la surface d'attaque que la séparation existe
pour éviter, avant qu'un seul déploiement en ait besoin. Voir décision 6 : elle n'est pas refusée,
elle est conditionnée.

**Un sous-processus, par entrée/sortie standard.** Écartée par deux décisions déjà prises.
`dependencies.json` interdit à `tokio` la feature `full` précisément parce qu'elle apporte `process`
et `signal`, que la règle 4 refuse à `locusd` ; lancer un processus est exactement cela. Et faire du
programme non privilégié le **parent** du programme privilégié lui donnerait prise sur lui, ce qui
inverse la relation que l'ADR 0004 établit.

**Le port est synchrone.** Une commande de cycle de vie de sandbox est une opération lente et rare,
et rien ne gagne à l'entrelacer. Rendre le port asynchrone imposerait un runtime à `locus-execd`,
c'est-à-dire à celui des deux qui doit rester le plus petit. Quand `W20.h` rendra §22.3 servable et
qu'un appel partira d'un contexte asynchrone, `spawn_blocking` de `tokio` — déjà présent chez
`locusd`, et absent de `locus-execd` — est la réponse, et elle ne coûte aucune dépendance.

---

## Décision 2 — Deux barrières à l'entrée, et la seconde est une vérification

> **Amendée par `W4.i` (2026-08-25).** Ce qui suit en italique est ce que la décision disait ; les
> deux affirmations soulignées se sont révélées fausses à l'écriture de `W4.h`, et la rédaction
> corrigée vient après.
>
> *« La socket est créée en `0600`, et le broker vérifie l'identité du processus appelant avant de
> répondre : `SO_PEERCRED` sur Linux, `getpeereid` sur macOS, tous deux derrière
> `UnixStream::peer_cred` **de la bibliothèque standard**. La politique retenue est le même
> utilisateur que le broker […] Les deux sont **gratuites** ; il n'y a aucune raison de n'en prendre
> qu'une. »*

La socket est créée en `0600`, et un appelant refusé l'est **en le disant**.

**Ce que la rédaction d'origine affirmait à tort, et qui a été mesuré.**

1. `UnixStream::peer_cred` n'est **pas** dans la bibliothèque standard stable : elle est **instable**
   — vérifié sur `rustc 1.94.1`, issue rust-lang#42839 —, et `unsafe_code = "forbid"` dans les lints
   d'espace de travail ne se contourne ni par `allow` ni par `expect` : c'est le sens de `forbid`.
   La créance coûte donc un **crate externe dans le processus privilégié**, ce que tout le reste de
   cet ADR passe son temps à éviter. Elle n'est pas gratuite.
2. La politique annoncée — « le même utilisateur que le broker » — admet **exactement** l'ensemble
   que `0600` admet déjà. Deux barrières qui laissent passer les mêmes appelants ne sont pas une
   défense en profondeur : c'est une redondance qui coûte une dépendance. Elle n'ajoutait rien.

**Ce qui est décidé à la place, et que `W4.i` a livré.** La créance de pair entre le jour où elle
sépare quelque chose, c'est-à-dire quand `locusd` et `locus-execd` tournent sous **deux
utilisateurs différents** : socket en `0660` avec un groupe partagé, et politique qui nomme l'uid
attendu — « **celui-là** », et non « le même ». Les deux barrières admettent alors des ensembles
distincts :

| Barrière | Qui passe |
| --- | --- |
| permissions `0660` + groupe partagé | le propriétaire, **et tout membre du groupe** |
| la politique de créance | **un** uid, nommé |

Le mode `0660` et la politique entrent **ensemble**, par la signature de `listen_shared` : les
offrir séparément laisserait poser le mode large sans la barrière qui le compense, ce qui est pire
que les deux états d'avant. `0600` reste le défaut, et reste le bon quand les deux binaires tournent
sous le même compte.

La dépendance est `rustix`, en portée `packages/broker`, **arbre mesuré à 3 paquets** et sans
`libc` ; `dependencies.json` en porte le motif et les mesures des concurrents.

**Motifs.** Les permissions de fichier protègent des autres utilisateurs de la machine, pas des
autres programmes du même utilisateur — et sur un poste de travail, tout ce que l'utilisateur lance
tourne sous son compte. Or ce tube commande la création de conteneurs. La créance de pair, elle,
vient du noyau : elle ne se falsifie pas depuis l'espace utilisateur, et c'est ce qui en fait une
**vérification** plutôt qu'une hypothèse. Ce motif-là tient toujours ; ce qui est tombé est l'idée
qu'elle serait gratuite, et l'idée qu'une politique « le même utilisateur » en ferait une seconde
barrière.

**Pas de secret partagé.** Pour une socket locale il serait plus faible que la créance de pair — un
secret se stocke, se copie et fuit, une identité de processus demandée au noyau non. Le jour où le
lien devient distant, ce sont des certificats qui entrent, et ils remplacent la créance de pair au
lieu de s'ajouter à un secret.

**Un refus d'authentification ne ressemble pas à une panne.** Il porte son propre nom sur le fil.
Sans cela, la première mise en service se passerait à chercher un problème de réseau qui n'existe
pas.

---

## Décision 3 — Un seul sens : `locusd` demande, `locus-execd` répond

Le broker n'initie **jamais** de connexion. Il n'a pas de client, il ne pousse rien, il ne rappelle
personne.

**Motifs.** Un programme qui répond n'a besoin que d'écouter ; un programme qui appelle a besoin en
plus de résoudre, de se connecter, de réessayer, de gérer des échéances. Doubler la surface du
processus privilégié pour lui donner l'initiative va contre tout le reste de cet ADR.

Et le besoin que cela laisse ouvert a déjà sa réponse : les nouvelles d'une exécution en cours
remontent par le canal que le worker tient **déjà** vers `locusd` — c'est toute la passerelle
d'événements de `W2.12`. Les faire remonter aussi par le broker créerait deux chemins pour le même
fait, donc deux versions de la vérité, ce que l'invariant 2 refuse.

---

## Décision 4 — Un broker injoignable se dit, et ne se confond avec rien

Trois issues distinctes, et le type ne permet pas de les confondre :

| Issue | Ce qu'elle veut dire | Ce qu'un exploitant doit faire |
| --- | --- | --- |
| une réponse | le broker a parlé, voici son verdict | lire le verdict |
| **injoignable** | on n'a pas pu demander | démarrer le service, vérifier le chemin de la socket |
| **rejeté** | le broker a refusé de nous parler | corriger l'identité ou les permissions |

**Motifs.** « Je n'ai pas pu demander » et « j'ai demandé et on m'a dit non » envoient chercher à des
endroits opposés. Les fondre est la faute que `W22` a passé une phase entière à corriger ailleurs, et
que `W5.h` avait déjà nommée pour les sondes d'hôte : une absence de réponse n'est pas une réponse
négative.

**Et `locusd` démarre quand même.** Il déclare le broker absent, **bruyamment et au démarrage**, et
refuse ensuite uniquement ce qui en dépend. Refuser de démarrer punirait quinze fonctions — lire le
graphe, consulter la mémoire, servir une revue — pour l'absence d'une seule. Le motif existe déjà
dans ce binaire, qui refuse d'ouvrir son port sur une projection en quarantaine tout en le disant et
en sortant sur un code qu'un superviseur lit.

Ce que cette décision interdit explicitement : un `locusd` qui aurait l'air d'aller bien et qui
échouerait à la première mission réelle. L'état du lien est une **valeur** que des tests exercent,
pas une ligne de journal.

---

## Décision 5 — La première opération est la disponibilité, et c'est une capacité finie

Le lien porte d'abord une seule question : *broker, sais-tu confiner, et sinon que te manque-t-il ?*
Elle se répond par le `Readiness` que `W22.c` a déjà livré, avec son plafond de niveau et sa liste
de manques.

**Motifs.** C'est la question que `locusd` doit poser **avant** toute autre : sans elle, il placerait
une mission sur un hôte dont il ne sait rien. Elle exerce l'aller-retour complet, les trois issues de
la décision 4, et la vérification de pair — sans exiger qu'un conteneur démarre, donc en CI comme
ailleurs.

Ce n'est pas un jalon partiel : au sens de l'ADR 0022 décision 0, c'est une **capacité finie**. Le
lien existe, il est testé de bout en bout, et l'admission puis le cycle de vie des sandboxes
s'ajouteront comme des variantes de requête sur un tube qui marche — pas comme la construction du
tube.

**Le vocabulaire de fil ne duplique rien.** Les six niveaux de confinement s'écrivent avec ceux de
`packages/lep`, qui existent déjà. Une troisième orthographe de `S0`–`S5` serait exactement le
« vocabulaire parallèle » que `CLAUDE.md` interdit.

---

## Décision 6 — Ce qui n'est pas construit, et la condition qui l'ouvrira

**Le lien distant n'existe pas**, donc le profil `distributed-hybrid` de §27.1 n'est pas couvert par
cet ADR. Ce n'est pas un oubli et c'est écrit ici pour que personne ne le découvre en le supposant.

La condition est nommée et vérifiable : **le jour où un profil de déploiement place `locus-execd` sur
une machine autre que celle de `locusd`**, un second backend s'ajoute derrière le même port, et
l'ADR qui l'ajoute mesure son arbre de dépendances comme l'ADR 0018 a mesuré le sien. Le port existe
pour que ce jour-là ne demande pas de réécrire les appelants.

**Aucune borne de débit, aucune file, aucun parallélisme** dans cette version. Le broker traite une
connexion à la fois. Écrire un ordonnanceur pour un lien qui porte aujourd'hui une question de
démarrage serait de l'abstraction spéculative ; la borne qui **est** posée est celle qui protège —
voir la décision 7.

---

## Décision 7 — Une ligne a une longueur maximale, et la dépasser est un refus

Le cadre JSON par lignes porte une borne de taille explicite. Une ligne plus longue est **refusée en
le disant**, jamais accumulée.

**Motifs.** Sans borne, n'importe quoi qui écrit dans la socket peut faire allouer sans fin au
processus privilégié en n'envoyant jamais de saut de ligne. C'est la même règle que xiiif s'est
donnée pour les corps de réponse et pour la profondeur JSON, et pour la même raison : le danger n'est
pas le format, c'est l'absence de fin.

La borne est un fait du protocole, pas un réglage : les deux côtés la connaissent, et un test
l'exerce de part et d'autre.

---

## Conséquences

Un crate neuf, `packages/broker`, porte le port, le protocole, le cadre et le backend socket. Il ne
dépend que de `serde`, `serde_json` et `packages/lep` — aucune entrée nouvelle dans
`dependencies.json`, et c'est vérifiable.

`apps/locusd` gagne un client et une valeur d'état de lien ; la **quatrième frontière** —
« `apps/locusd` n'importe aucun SDK de runtime de containers » — reste tenue et s'exerce désormais
sur davantage de fichiers. `apps/locus-execd` gagne une boucle d'écoute et reste sans runtime
asynchrone.

`locusd` ne dépend **pas** de `locus-execd` : les deux dépendent du crate de protocole. Faire
dépendre le daemon du crate qui contient la seule fonction du dépôt exécutant `podman` aurait été
tenir la règle 4 à la lettre contre son objet.

## Plan de rollback

Les décisions 3, 5 et 6 sont documentaires ou négatives et se retirent par un diff. La décision 1
introduit un crate : le retirer coûte sa suppression et celle de deux câblages, et aucune donnée
n'est en jeu — le lien ne persiste rien. Les décisions 2, 4 et 7 sont des bornes ; les retirer
élargit ce que le broker accepte, donc aucune ne peut être retirée seule sans que la précédente soit
réexaminée, et c'est pourquoi elles sont écrites ensemble.

Le seul rollback qui coûterait une garantie serait de revenir sur la décision 1 après qu'un backend
distant existe : il faudrait alors décider lequel des deux disparaît. La condition de la décision 6
est écrite pour que ce choix soit fait à l'entrée du second backend, pas après.
