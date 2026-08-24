# ADR 0034 — D'où `locusd` tire son entropie

**Statut :** accepté. Ouvre `W20.x`.

**Contexte.** `Identities` est un port depuis `W20.a`, et son défaut refuse : « aucune source
d'identifiants n'est câblée : `locusd` ne tire pas d'entropie — cela demande un crate, donc un ADR et
une entrée dans `dependencies.json` — et il refuse d'inventer un identifiant de commande ». Le refus
**nomme son propre remède**, ce qui est le meilleur cas de figure : il n'y a rien à deviner, seulement
à mesurer.

Le besoin est devenu concret en enrôlant un worker `canterel` réel contre un `locusd` réel, par le
harnais de `W12.f` : la demande, **correctement signée** — vérifié en la construisant à la main avec
une vraie paire Ed25519 —, traverse la vérification, consomme son token, et s'arrête à l'écriture du
fait sur ce `503`. Conséquence exacte : **aucune écriture déclenchée par un worker n'aboutit**, donc
`W12.d` s'arrête à son premier acte.

Un `Id` de `packages/protocol` est un ULID : 48 bits d'horodatage et **10 octets d'aléa**.
L'horodatage, `locusd` l'a. Les dix octets, non.

---

## Décision 1 — `getrandom`, et la mesure dit pourquoi

`CLAUDE.md` exige qu'une dépendance externe du workspace Rust entre par `dependencies.json` avec
l'ADR qui la motive, et que **l'ADR mesure** les transitives. Voici la mesure, lue dans
`Cargo.lock`, pas estimée :

| Fait | Valeur |
| --- | --- |
| `getrandom` est-il déjà dans l'arbre ? | **oui**, `0.4.3`, tiré par `rand` |
| Qui tire `rand` ? | `postgres-protocol` et `tokio-postgres` — donc le driver de l'ADR 0030 |
| Dépendances propres de `getrandom` | `cfg-if`, `libc`, `r-efi`, `rand_core` |
| Combien d'entre elles manquent à l'arbre ? | **zéro** — les quatre y sont déjà |
| Paquets ajoutés en le **déclarant** | **zéro** |

Le déclarer ne fait donc qu'écrire noir sur blanc une dépendance que le workspace **compile déjà**.
C'est le cas le moins cher qu'un ADR de dépendance puisse rencontrer, et il vaut d'être noté : la
question « quel crate » n'avait pas de réponse évidente a priori, et c'est la mesure qui l'a rendue
évidente.

**Pourquoi pas `rand`**, qui est là aussi. `rand` est un générateur — algorithmes, distributions,
graines reproductibles. Ce dont `locusd` a besoin est dix octets que personne ne peut prédire, ce qui
est exactement la surface de `getrandom` et rien de plus. Prendre `rand` reviendrait à faire entrer
une bibliothèque de génération pseudo-aléatoire dans un daemon qui n'a **jamais** de raison
d'utiliser un générateur ensemencé — et un générateur ensemencé est précisément ce qu'il ne faut pas
pour des identifiants qui distinguent des actes institutionnels.

**Pourquoi pas `/dev/urandom` lu à la main.** Ce serait zéro dépendance et un pari sur l'hôte :
Windows n'a pas ce fichier, un conteneur peut ne pas l'exposer, et la gestion d'un descripteur
ouvert à la première demande est exactement le genre de code que `getrandom` existe pour ne pas
réécrire. La règle du dépôt — « aucune dépendance implicite à une machine de développeur » — vise
cela.

---

## Décision 2 — L'entropie est un port, et le défaut continue de refuser

`SystemIdentities` implémente `Identities` en tirant de l'OS. Il **ne remplace pas**
`NoIdentities` : le défaut du composition root reste celui qui refuse, et c'est le binaire qui câble
la source réelle.

Deux raisons, et la seconde est celle qui compte.

1. Un test qui veut des identifiants **prévisibles** doit pouvoir en fournir, et il le peut déjà.
2. Un daemon assemblé sans source doit continuer à le **dire**. Si `SystemIdentities` devenait le
   défaut, plus personne ne rencontrerait jamais le refus, et le jour où une plateforme sans entropie
   apparaîtrait — un conteneur durci, un environnement embarqué —, le message qui explique quoi faire
   aurait disparu du chemin. Un refus qu'on ne peut plus atteindre est un refus qu'on ne peut plus
   maintenir.

---

## Décision 3 — Le refus reste `Unavailable`, et la roadmap se trompait

La ligne `W20.x` annonçait qu'il fallait aussi corriger le code de statut : « il est en `503`, donc
"réessaie", donc un worker qui boucle sur une panne définitive ».

**C'est faux, et l'ADR le corrige plutôt que de le traîner.** `Unavailable` veut dire « ce service ne
peut pas répondre **maintenant**, et cela se répare par configuration » — c'est exactement la
situation, et c'est ce que la documentation du port disait déjà. Un `Internal` enverrait chercher un
défaut dans le code là où il n'y en a pas. Et la panne n'est « définitive » que tant que personne ne
câble la source : un opérateur qui redémarre le daemon avec la source branchée fait réussir la même
requête, ce qui est la définition d'un `503`.

Ce qui reste vrai, et qui est traité ailleurs : un worker qui réessaie indéfiniment sans borne est un
problème de **worker**, pas de code de statut.

---

## Conséquences

- `dependencies.json` porte `getrandom`, avec cet ADR pour motif et la mesure ci-dessus.
- `apps/locusd` gagne `SystemIdentities` ; `main.rs` le câble ; `composition.rs` ne nomme rien.
- L'enrôlement d'un worker réel aboutit, et le harnais de `W12.f` le vérifie de bout en bout.
- `NoIdentities` reste atteignable et testé.
