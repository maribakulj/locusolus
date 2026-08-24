# ADR 0031 — Vérifier une signature d'enrôlement, et ce qu'un token ne devient jamais

**Statut :** accepté. Ouvre `W20.n`.

**Contexte.** `W2.4` a livré la moitié cliente de §7.2 : `canterel` génère une identité Ed25519,
signe une demande d'enrôlement liant `worker_id`, endpoint et nonce, et garde la créance obtenue.
**Personne ne l'écoute.** `W20.k` a dû livrer un `WorkerRegistry` que seul un test remplit, donc un
worker réel ne peut pas obtenir de créance, donc les trois chemins de §15.2 sont injoignables en
pratique. C'est l'une des trois conditions de `W12.d` que `W20.i` a mises au jour.

---

## Décision 1 — `ed25519-dalek`, et non `ring`

Le client signe en **Ed25519**, avec une clé publique en **SPKI DER encodé base64**. Le serveur doit
lire ce format et vérifier cette signature. Deux candidats sérieux, mesurés arbre complet :

| Candidat | Paquets | Compilateur C au build |
| --- | --- | --- |
| `ed25519-dalek` (`pkcs8`, `alloc`, sans défaut) | **25** | non |
| `ring` | 8 | **oui** — `cc` est dans son arbre |

`ring` est trois fois plus léger en nombre de paquets et il est écarté quand même, pour une raison
que ce dépôt a déjà tranchée : l'ADR 0020 a choisi `sha2` plutôt que `blake3` parce que le second
« coûterait un compilateur C ». Le même argument vaut ici, et il vaut plus fort — une dépendance de
build non-Rust traverse tous les profils de §27.1, y compris ceux qui compilent ailleurs que sur la
machine d'un développeur.

Neuf des vingt-cinq paquets de `ed25519-dalek` sont **déjà** dans le workspace par `sha2`
(`block-buffer`, `cpufeatures`, `crypto-common`, `digest`, `hybrid-array`, `sha2`, `typenum`,
`cfg-if`, `const-oid`). Le coût marginal réel est donc de seize.

**Features nommées, défauts coupés.** `pkcs8` pour lire le SPKI DER — sans elle il faudrait
découper les octets à la main, ce qui marche pour Ed25519 dont l'en-tête fait douze octets fixes, et
ce qui est exactement le genre de raccourci qu'on paie le jour où un client émet un encodage
légèrement différent. `alloc` pour les erreurs. Ni `rand_core` — ce module ne **génère** aucune clé —
ni `signature` en défaut.

---

## Décision 2 — Un token d'enrôlement est un port, pas une table inventée

§7.2 : un token est court-terme, à usage unique, et **porte un scope**. Rien dans ce dépôt n'en émet.

Trois réponses, une seule admise, et c'est la même que pour `W20.k` : reporter reviendrait à dire
« aucun appelant » — refusé par l'ADR 0022 décision 0 ; inventer un émetteur de tokens en passant
serait bâtir une fonctionnalité pour justifier une surface. `EnrollmentTokens` entre donc comme
**port**, avec son implémentation de référence en mémoire, et ce qui l'alimentera — une commande
d'administration de §22.3 — est nommé, pas simulé.

**Le token ne devient jamais le secret permanent.** §7.2 l'écrit, et ce module le rend vrai par
construction : la créance émise est une valeur **distincte**, tirée de la source d'identifiants de
`W20.k`, et le token est consommé au premier usage. Un serveur qui renverrait le token comme
créance passerait tous les tests fonctionnels et donnerait à un secret court-terme la durée de vie
d'un secret permanent.

---

## Décision 3 — Le nonce lie la demande à **son** serveur, et ne se rejoue pas

Le client signe `worker_id\nendpoint\nnonce`. Les trois comptent :

- sans `endpoint`, une demande capturée se resservirait vers un **autre** serveur — c'est ce que
  `W2.4` a écrit dans son propre commentaire, et le serveur doit tenir sa moitié ;
- sans `nonce`, la même demande se resservirait vers le **même** serveur.

Le serveur vérifie donc que l'endpoint signé est le sien, et refuse un nonce déjà vu. Le registre
des nonces vit dans le port de tokens : il a la même durée de vie et la même question — « ceci a-t-il
déjà servi ? ».

---

## Décision 4 — Une révocation est un fait du journal, jamais une ligne supprimée

Invariant 12. Un worker révoqué **garde** son identité — `W2.4` le dit du côté client : « il ne
l'oublie pas, il la sait révoquée ». Le serveur écrit `worker.revoked` et le registre cesse de
reconnaître la créance ; rien n'est effacé, et l'histoire reste lisible.

Conséquence testable : un worker enrôlé puis révoqué reçoit un refus **typé** sur les trois chemins
de §15.2, et le test le vérifie depuis le client de `W2.21` **sans le modifier** — c'est ce qui
distingue une révocation qui fonctionne d'une révocation qu'on croit avoir posée.

---

## Décision 5 — Ce que cet item ne fait pas

Ni rotation de créance, ni expiration vérifiée à l'usage : `Credential.expires_at` existe côté
client depuis `W2.4` et le serveur l'émet, mais **personne ne la fait respecter à la réclamation**.
L'écrire ici demanderait une horloge dans le chemin d'admission et une politique de renouvellement,
c'est-à-dire deux décisions de plus. Elles sont nommées plutôt que prises, et `expires_at` vaut
`null` — ce qui est exact et se lit, au lieu d'une date que rien n'honore.
