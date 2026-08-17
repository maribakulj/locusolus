# ADR 0013 — Une projection se reconstruit, s'arrête sur faute, et n'écrit jamais

**Statut :** accepté. Met en œuvre `docs/SPEC_V1.md` §9.3 et §9.5. Concerne `packages/projections` (W1.d). Suit ADR 0012, dont il consomme le port.

**Contexte :** W1.d est marqué `[M]` — une migration, donc un ADR et un plan de rollback. Ce qui est migré est la **forme** des projections, dont dépendront le graphe (W1.e), la validation (W1.f) et toutes les requêtes de §9.4. §9.1 range explicitement les projections du côté du reconstructible : « les vecteurs, index plein texte, vues matérialisées, graph databases et caches sont des projections reconstruisibles ». Le mot porte tout ce qui suit.

---

## Décision 1 — Le journal gagne une position globale, hors de l'enveloppe

§9.5 demande que « chaque projection expose son dernier `event_sequence` appliqué ». Encore faut-il qu'il y en ait un : §10.1 ne met pas de rang global dans l'enveloppe, et `stream_revision` est le rang **dans un stream** — deux streams différents portent tous deux une révision 1. Une projection qui suivrait `stream_revision` ne saurait pas où elle en est du journal.

Le port `EventStore` gagne donc `feed(from) -> Vec<Sequenced>`, où `Sequenced` accole une position à une enveloppe.

**La position vit à côté de l'enveloppe, pas dedans.** L'enveloppe est le document normatif de §10.1, partagé entre pairs ; y ajouter un champ ferait diverger cette implémentation du schéma que deux SDK doivent produire à l'identique. La position est un fait de **ce** journal, comme `stream_revision` et pour la même raison.

---

## Décision 2 — `reset` et `checksum` sont dans le port, pas dans les implémentations

Le trait `Projection` exige quatre choses : appliquer, dire son watermark, **se réinitialiser**, et **résumer son état**.

**Motifs.**

`reset` n'est pas une facilité d'opérateur. C'est la propriété qui rend une projection *secondaire* : une projection qu'on ne saurait pas reconstruire serait une seconde source de vérité, ce que §9.1 réserve au journal. La mettre dans le port oblige chaque projection à répondre à la question au moment où elle est écrite, plutôt qu'à la découvrir le jour d'un incident.

`checksum` donne à la vérification de quoi comparer. Une projection qui ne sait pas résumer son état ne peut pas être confrontée à sa reconstruction, et l'outil de §9.5 n'aurait rien à comparer.

**Choix de forme :** les checksums livrés sont des chaînes **lisibles**, pas des hashes. Le but de §9.5 est de détecter une divergence ; une chaîne qu'on peut lire dit aussi *où* elle est, là où un `sha256` dirait seulement « ce n'est pas pareil ». Le jour où une projection porte assez d'état pour que ce soit impraticable, elle passera au hash — c'est une décision par projection, pas une décision de port.

---

## Décision 3 — Une projection en défaut s'arrête, elle ne saute pas

Sur erreur, le pilote met la projection en quarantaine et **cesse de la faire avancer**. Il ne saute pas l'événement fautif.

**Motifs.**

Une projection qui sauterait l'événement fautif aurait un état que la reconstruction ne reproduirait pas : la reconstruction, elle, rencontrerait la même faute au même endroit. « Reconstruction depuis zéro = état courant » deviendrait faux — et c'est précisément la propriété que W1.d livre.

Sauter, c'est aussi décider unilatéralement qu'un événement du journal canonique n'a pas d'importance. §9.1 fait du journal la source de vérité ; une projection n'a pas autorité pour en écarter une partie.

**Une reconstruction lève la quarantaine.** Une projection qui resterait en quarantaine après avoir été reconstruite ne pourrait jamais s'en sortir, même une fois la cause corrigée dans son code. Si la faute est dans le journal, la reconstruction la rencontre à nouveau et la quarantaine revient — ce qui est le comportement voulu, et ce que le test vérifie.

---

## Décision 4 — La quarantaine ne bloque pas l'écriture, et cela tient par la forme

§9.5 : « les erreurs de projection sont mises en quarantaine **sans bloquer l'écriture canonique**, sauf si elles concernent une projection synchrone nécessaire à un invariant. »

Le pilote reçoit le journal par **référence partagée** et n'a aucun chemin d'écriture. Une projection en défaut ne peut donc pas empêcher un append parce qu'il n'existe aucun moyen par lequel elle l'atteindrait. La promesse tient par la forme du code, pas par la discipline de qui l'écrit.

`catch_up` ne rend jamais d'erreur : une faute se lit dans `Progress::health`. Faire remonter l'erreur inviterait un appelant à la propager jusqu'à un chemin d'écriture, ce que la phrase interdit.

**Le cas réservé n'est pas implémenté.** Aucune projection de ce paquet n'est synchrone ; écrire le mécanisme avant d'avoir le cas produirait une abstraction que rien ne teste. Il est nommé ici pour qu'on ne le croie pas oublié.

---

## Décision 5 — Deux projections, pas douze

§9.3 liste douze projections obligatoires. Ce paquet en livre deux : « état de validation » et « registre des conflits ».

**Motifs.** Ce sont celles que le domaine de W1.a et W1.b permet d'écrire honnêtement, et deux suffisent à éprouver le port — la propriété de reconstruction est vérifiée sur les deux, donc elle porte sur le trait et non sur une implémentation. Les dix autres attendent le graphe (W1.e) et la validation (W1.f), qui leur donneront de quoi projeter. `docs/10` est explicite : « ne crée pas 34 stubs vides ».

Le registre des conflits porte en plus l'**invariant 12** : un conflit résolu garde la position de sa résolution au lieu de disparaître. Le mot « propre » de l'invariant vise exactement ce que ferait une projection ordinaire — ne garder que les conflits ouverts, parce que ce sont les seuls qu'on interroge.

---

## Conséquences

`Cargo.toml` du workspace gagne un membre. `boundaries.json` mentionne déjà `packages/projections/**` comme site autorisé pour un client `PostgreSQL` (règle 3) ; le paquet n'en contient pas à ce stade, et la règle n'a pas besoin d'être amendée.

Toute projection future implémente quatre méthodes, dont deux qu'elle n'aurait pas écrites spontanément. C'est la décision 2 assumée.

---

## Plan de rollback

**La décision 1 est additive** : `feed` s'ajoute au trait `EventStore` sans modifier `append`, `read_stream`, `revision` ni `export`. La retirer casse `packages/projections` et rien d'autre.

**Les décisions 2 à 5 sont contenues dans `packages/projections`**, que rien ne consomme encore. Avant W1.e, revenir coûte la suppression du crate et d'une ligne de `Cargo.toml`.

**Après W1.e**, le graphe lira des projections. Un retour sur la décision 3 — sauter au lieu de s'arrêter — rendrait deux tests rouges qui diraient exactement ce qui a été perdu. Un retour sur la décision 2 demanderait de retirer `reset` du port, ce qui laisserait chaque projection libre de ne pas être reconstructible : c'est le seul rollback qui coûte une garantie plutôt qu'un diff, et c'est pourquoi la décision est prise maintenant.

**Aucune donnée n'est en jeu** : les projections sont en mémoire, reconstructibles par construction, et le journal qui les alimente l'est aussi.
