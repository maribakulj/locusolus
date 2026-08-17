# ADR 0012 — Le port du journal avant tout driver, et la concurrence optimiste sans échappatoire

**Statut :** accepté. Met en œuvre `docs/SPEC_V1.md` §10.2 et la règle de `CLAUDE.md` « construire domain/protocol/event-store d'abord, avec des ports purs ». Concerne `packages/event-store` (W1.c).

**Contexte :** W1.c est marqué `[M]` dans `docs/10_V1_ROADMAP.md` — une migration, donc un ADR et un plan de rollback. Ce qui est migré n'est pas un schéma existant : c'est la **forme** que prendra le journal canonique, dont dépendront les projections (W1.d), le graphe (W1.e) et toute écriture ultérieure. Revenir dessus après W1.d coûte la réécriture de tout ce qui lit le journal.

`SPEC_V1.md` §9.1 fait de PostgreSQL la source de vérité transactionnelle, et la règle 3 de `boundaries.json` autorise un client PostgreSQL sous `packages/event-store`. La question n'est donc pas *si* le driver arrive, mais *quand* — et ce qu'il doit satisfaire en arrivant.

---

## Décision 1 — Le port et sa suite de contract tests avant tout driver

`packages/event-store` livre le trait `EventStore`, une implémentation de référence en mémoire, et une suite de contract tests écrite **contre le port**. Aucun client PostgreSQL n'entre dans ce paquet à ce stade.

**Motifs.**

La suite de contract tests est ce qui décidera si le driver PostgreSQL est conforme — pas sa documentation, pas sa relecture. Écrite après lui, elle documenterait ce qu'il fait ; écrite avant, elle dit ce qu'il doit faire. C'est le même geste que la suite de self-tests de sandbox d'ADR 0004, écrite avant le premier backend d'exécution, et pour la même raison : elle définit le mot.

Une implémentation en mémoire rend la suite exécutable sans base de données, donc en CI sans service, donc à chaque commit. La conformité du driver se vérifiera en lui faisant passer exactement la même suite.

**Ce que ce paquet ne porte pas encore**, et qui appartient aux items suivants : signature de fédération, upcasters de migration (W1.h), snapshots reconstruisibles (W1.d). Les nommer ici évite qu'on les croie oubliés.

---

## Décision 2 — `Expected` n'a pas de variante « peu importe »

La concurrence optimiste s'exprime par `Expected::{NoStream, Exact(u64)}`. Il n'existe **pas** de variante `Any` acceptant l'écriture quel que soit l'état du stream.

**Motifs.**

§10.2 dit « optimistic concurrency **par `expected_stream_revision`** ». Un écrivain qui ne sait pas sur quelle révision il construit n'a rien vérifié : ce qu'il produit n'est pas un append concurrent réussi, c'est un conflit qu'on n'a pas regardé, et la mise à jour perdue qui va avec.

La plupart des journaux offrent `Any` par commodité, et c'est précisément par cette commodité que les invariants d'agrégat se perdent : le premier appelant pressé l'utilise, et plus personne ne sait quelles écritures ont été vérifiées.

**Coût assumé.** Chaque écrivain doit lire avant d'écrire. Le prix se paie une fois, à l'écriture, plutôt qu'indéfiniment en incohérences dont personne ne sait d'où elles viennent.

**Condition de réexamen :** un cas d'usage réel où la révision attendue est *impossible* à connaître — et non simplement coûteuse à obtenir. Aucun n'est identifié à ce jour ; W1.d et W1.e le diront.

---

## Décision 3 — L'idempotence est vérifiée avant la concurrence

Dans `append`, la commande déjà appliquée est détectée **avant** le contrôle de révision.

**Motifs.**

Une commande rejouée a fait avancer le stream. Son `expected_stream_revision` est donc périmé au moment du rejeu — par sa propre faute. Vérifier la concurrence d'abord lui opposerait sa propre écriture, ce qui est le comble de la concurrence optimiste : l'appelant relirait, retenterait, et obtiendrait un doublon.

Un rejeu rend le **résultat d'origine** avec `replayed: true`, et non une erreur : une commande réémise après une coupure réseau a déjà eu son effet, et le dire est plus utile que de faire échouer un appelant qui referait la même chose.

La réutilisation d'un identifiant de commande avec un **contenu différent** reste refusée. Deux lots distincts sous un même identifiant veulent dire que l'identifiant a été réutilisé, et l'accepter écrirait l'un des deux en croyant écrire l'autre.

---

## Décision 4 — `Draft` et `Envelope` sont deux types

Un producteur écrit un `Draft` ; le journal le scelle en `Envelope` en lui attribuant `stream_revision` et `recorded_at`.

**Motifs.**

« Ordre total par stream » (§10.2) n'est pas une propriété à vérifier après coup : c'est une propriété à rendre non violable. Le producteur ne peut pas poser un rang parce que le champ n'existe pas chez lui, et deux événements de même rang dans un stream ne sont donc pas représentables.

`recorded_at` suit la même logique : c'est l'instant de l'écriture, un fait du journal. Le demander au producteur reviendrait à lui faire promettre ce qu'il ne peut pas savoir. Sa distinction d'avec `occurred_at` n'est pas décorative — un worker hors ligne (§24.3) produit des actes dont l'écriture suit de plusieurs heures, et les confondre daterait tout un travail de son moment de synchronisation.

---

## Conséquences

`Cargo.toml` du workspace gagne un membre. La règle 3 de `boundaries.json` couvre déjà `packages/event-store/**` ; elle n'a pas besoin d'être amendée pour accueillir le driver plus tard.

Tout écrivain futur — command handlers de W1.d, projections, migrations — construit un `Append` avec une révision attendue explicite. C'est un coût d'écriture réel, et c'est la décision 2 assumée jusqu'au bout.

---

## Plan de rollback

**Avant W1.d**, revenir sur cet ADR coûte la suppression de `packages/event-store` et de sa ligne dans les membres du workspace. Rien d'autre ne dépend du paquet : `packages/domain` ne le connaît pas, et `boundaries.json` interdit explicitement l'inverse.

**Après W1.d**, les projections lisent le port. Un retour en arrière sur les décisions 2, 3 ou 4 demanderait :

1. pour la décision 2 — ajouter `Expected::Any` est **additif** : les écrivains existants continuent de compiler, et la suite de contract tests reste verte. Le coût est la perte de la garantie, pas une réécriture ;
2. pour la décision 3 — inverser l'ordre des contrôles est un déplacement de bloc, et deux contract tests deviennent rouges. Ils disent alors exactement ce qui a été perdu ;
3. pour la décision 4 — fusionner `Draft` dans `Envelope` demande de rendre `stream_revision` optionnel, ce qui touche tous les lecteurs. C'est la seule des quatre dont le rollback est coûteux, et c'est pourquoi elle est prise maintenant plutôt qu'après W1.d.

**Aucune donnée n'est en jeu** : l'implémentation de référence est en mémoire, et aucun journal persistant n'existe encore. C'est la fenêtre où ces décisions se prennent au prix d'un diff.
