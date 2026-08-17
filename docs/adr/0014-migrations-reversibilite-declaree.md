# ADR 0014 — Une migration déclare sa réversibilité, ou elle n'en a pas

**Statut :** accepté. Met en œuvre `docs/SPEC_V1.md` §10.2 (« migration upcaster pour les consommateurs ») et §10.4. Concerne `packages/migrations` (W1.h). Dernier ADR de W1.

**Contexte :** W1.h est marqué `[M]`. Ce qui est migré ici n'est pas un schéma existant — aucun journal persistant n'existe encore — mais la **forme que prendront toutes les migrations futures** du journal canonique. Un upcaster écrit maintenant sera copié par les suivants ; ce qu'il autorise, ils l'autoriseront.

§10.4 pose quatre règles d'évolution, dont deux qui décident de ce paquet :

- « les changements incompatibles créent une nouvelle version de message » ;
- « les producteurs supportent au minimum la version courante et la version précédente pendant une fenêtre de migration ».

---

## Décision 1 — Deux constructeurs, et l'irréversibilité s'écrit

`Migration::reversible` prend une montée **et** une descente. `Migration::lossy` prend une montée et un [`Loss`] — la liste des champs perdus, plus la raison.

Il n'existe pas de troisième forme. Une migration sans descente et sans perte déclarée ne se construit pas.

**Motifs.**

Une migration qui monte sait rarement redescendre, et **le prétendre est pire que l'admettre**. Une chaîne qui redescendrait à travers une étape destructive rendrait un document ancien qui n'a jamais existé — et il aurait l'air authentique. Sur un journal dont la raison d'être est la provenance, c'est le faux dont on ne se remet pas : il ne se distingue d'un vrai par aucune inspection.

L'alternative habituelle — une descente « au mieux », qui remplit les champs manquants par des valeurs par défaut — produit exactement ce document. Elle est aussi ce que tout le monde écrit quand rien ne l'en empêche, parce qu'elle fait passer les tests de round-trip qu'on avait sous la main.

Déclarer la perte rend l'irréversibilité **exécutoire** plutôt que documentée : `downcast` refuse, et le refus porte la liste des champs. Un commentaire « attention, destructif » n'aurait arrêté personne.

**Coût assumé.** Écrire une migration destructive demande d'énumérer ce qu'on perd. C'est une minute de travail et la seule occasion où quelqu'un y réfléchit.

---

## Décision 2 — La chaîne refuse, elle ne saute pas

`MigrationChain::downcast` échoue dès qu'une étape du chemin est irréversible. Elle ne saute pas l'étape, ne rend pas un résultat partiel, et ne descend pas « aussi loin que possible ».

**Motifs.**

Un résultat partiel serait un document d'une version que l'appelant n'a pas demandée, sans que rien dans sa forme ne le dise. Le refus, lui, se traite : `irreversible_between` permet de **demander avant de tenter**, et de décider en connaissance de cause.

Une migration destructive n'est pas une faute — c'est parfois la seule façon d'avancer. Ce qui est une faute, c'est de le découvrir au moment où l'on avait besoin de redescendre.

---

## Décision 3 — Les étapes sont contiguës, et la chaîne panique sinon

`MigrationChain::with` refuse une étape qui ne part pas de la version d'arrivée de la précédente, ou qui saute des versions. Le refus est une panique, pas une erreur.

**Motifs.**

Une chaîne trouée est une erreur de **programmation**, pas une entrée : elle se construit en dur, au démarrage, dans du code que quelqu'un a écrit. La découvrir au premier document migré coûterait plus cher que de la refuser à la construction, et la rendre récupérable inviterait à la rattraper à l'exécution — c'est-à-dire à migrer un document en sautant une forme qu'il a réellement eue.

---

## Décision 4 — La portabilité de §4.1 se vérifie sur les noms, pas seulement sur les imports

`portability::provider_findings` cherche des marqueurs de fournisseur (`s3_`, `gcp_`, `k8s_`…) dans les sources des paquets de domaine.

**Motifs.**

`boundaries.json` vérifie les **imports**, et c'est la bonne garde pour les dépendances. Elle ne voit ni les noms de champ ni les littéraux : un `Claim` qui porterait `s3_bucket` ne violerait aucune règle d'import et rendrait pourtant l'objet indéplaçable, ce que §4.1 interdit en toutes lettres — « aucun objet `Project`, `Branch`, `Claim`, `Review`, `Task` ou `Artifact` ne doit dépendre d'un fournisseur d'infrastructure ».

Les lignes de commentaire sont exclues du balayage : nommer un fournisseur pour dire qu'on n'en dépend pas n'est pas une dépendance, et l'inverse ferait échouer la garde sur sa propre documentation. Un test vérifie que le filet **attrape** — sans lui, une fonction rendant toujours la liste vide passerait le test principal.

---

## Conséquences

`Cargo.toml` du workspace gagne un membre. Toute migration future du journal passe par ce paquet et doit choisir entre les deux constructeurs — c'est la décision 1 assumée jusqu'au bout.

La fenêtre de compatibilité de §10.4 est vérifiable : `covers_minimum_window` dit si la chaîne sait encore lire la version précédente. Une chaîne qui ne saurait lire que la version courante refuserait du jour au lendemain un producteur qui n'a pas encore migré.

---

## Plan de rollback

**Avant qu'un journal persistant existe**, revenir sur cet ADR coûte la suppression de `packages/migrations` et d'une ligne de `Cargo.toml`. Rien ne le consomme : `packages/event-store` ne le connaît pas, et la dépendance va dans l'autre sens en test seulement.

**Après le premier journal persistant**, les décisions ont des coûts différents :

1. **décision 1** — ajouter un troisième constructeur « descente au mieux » est **additif** : les migrations existantes continuent de compiler. Le coût est la perte de la garantie, pas une réécriture. C'est aussi le rollback le plus dangereux des quatre, parce qu'il ne casse rien visiblement ;
2. **décision 2** — faire sauter les étapes irréversibles rend deux tests rouges, qui disent exactement ce qui a été perdu ;
3. **décision 3** — remplacer la panique par un `Result` touche tous les points de construction de chaîne ;
4. **décision 4** — retirer le balayage de portabilité ne casse rien et ne se remarque pas, ce qui est précisément la raison de l'écrire maintenant.

**Aucune donnée n'est en jeu aujourd'hui.** C'est la fenêtre où ces décisions se prennent au prix d'un diff, et elle se referme au premier journal écrit sur disque.
