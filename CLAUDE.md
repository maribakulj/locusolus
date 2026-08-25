# CLAUDE.md — locusolus

## Où tu es

`maribakulj/locusolus`. Monorepo : control plane, event store, graphe, Execution Fabric,
cockpit web et **cockpit Emacs** (`apps/emacs/`, ADR 0009 — pas de dépôt séparé).

**État au handoff : le dépôt est vide.** `LICENSE` + `README.md`, commit `7dc4dd1`. Il n'y a rien
à préserver, rien à adapter, aucun audit antérieur à consulter. Tout est greenfield ici, et c'est
le seul dépôt du chantier dans ce cas.

## Ordre de lecture

`docs/00` → `docs/01` → `docs/02` → `docs/03` → `docs/06` → `SPEC_V1.md` → `docs/10` →
`docs/11`. Les ADR arbitrent : lis `docs/adr/0009` et `docs/adr/0010` avant de toucher aux
frontières inter-repos.

## Règles propres à ce dépôt

- Construire domain/protocol/event-store d'abord, avec des ports purs. Ne brancher Temporal,
  containers ou cloud qu'après les interfaces et les contract tests.
- `locusd` ne détient jamais de socket runtime. C'est le rôle de `locus-execd`. La tentation de
  parler à Podman « juste pour le profil local » est exactement ce que cette séparation empêche.
- Toute mutation passe par un command handler transactionnel.
- Le backend de workflow déterministe de test s'écrit **avant** le backend Temporal (ADR 0003).
- La suite de self-tests de sandbox s'écrit **avant** le premier backend d'exécution (ADR 0004) :
  elle définit ce que « sandbox » veut dire ici.
- `packages/protocol` est le goulot du projet entier. Il se fige en `lep/1.0` avant que deux
  consommateurs en dépendent.
- `locusd`, `locus-execd` et la CLI sont en **Rust** (ADR 0011, qui amende `SPEC_V1.md` §4.5).
  `apps/web`, le SDK client généré et le worker Canterel restent en TypeScript ; `apps/emacs` en
  Emacs Lisp. Les JSON Schemas s'écrivent en **Draft 7** tant qu'un prototype `typify` sur
  2020-12 n'a pas levé la condition 1 de l'ADR.
- Une dépendance externe du workspace Rust entre par `dependencies.json`, avec l'ADR qui la motive,
  et `check:deps` refuse ce qui n'y est pas. La liste ne porte que les dépendances **déclarées** ;
  les transitives sont l'affaire de l'ADR, qui les mesure. `tokio` n'a jamais la feature `full`
  (ADR 0018) : elle apporte `process` et `signal`, que la règle 4 interdit à `locusd`.
- Les objets d'organisation, de coordination et de gouvernance sont ceux de `SPEC_V1.md` §7.1, §13,
  §16, §20 et §22, sous leur nom. Aucun vocabulaire parallèle — pas de `MutationPolicy`, pas de
  `MutationGrant`, pas de `TopologyNode`, pas d'échelle d'autorité à cinq barreaux (ADR 0016). Une
  sorte de relation de coordination n'entre dans son énumération que lorsqu'un consommateur
  exécutable et testé existe.
- **On ne livre jamais une promesse ; on livre toujours une capacité** (ADR 0022, décision 0). Une
  *promesse* est un type qui annonce un effet qui n'a pas lieu — une arête `message` qu'aucun routeur
  n'honore ment à qui croit l'avoir posée, et la règle précédente la refuse toujours. Une *capacité*
  est un sous-système complet et testé dont personne n'a encore eu besoin ; elle est finie.
  « Aucun appelant ne l'utilise encore » **n'est donc pas un motif de report** : les deux seuls
  motifs admis sont une dépendance technique nommée et un hôte externe absent. La règle du dessus
  vaut pour une **valeur d'énumération**, qui affirme qu'un effet existe ; elle n'a jamais valu pour
  un module, qui n'affirme rien tant qu'on ne l'appelle pas, et c'est cette généralisation-là que
  l'ADR 0022 corrige.

## Citer une section

`§N.M` **nu** désigne toujours `docs/SPEC_V1.md`, celui de ce dépôt, et `check:citations` refuse un
numéro qui n'y existe pas.

Cette convention a un coût qu'il faut connaître : `canterel/docs/locus/SPEC_V1.md` porte le **même
nom de fichier** et numérote lui aussi depuis 1, et ce ne sont pas deux copies — sur 147 numéros de
section communs, **cinq** portent le même titre. Ce sont deux spécifications différentes, l'une du
plan de contrôle, l'autre du worker. Une citation nue vers l'autre dépôt y désigne donc autre chose
**sans erreur visible** : `§12.4` est la backpressure ici et l'isolation informationnelle là-bas, et
la roadmap comme l'ADR 0026 s'y sont trompées jusqu'au 2026-08-25 (`W0.21`).

Une citation vers un autre document **le nomme**, et c'est ce qui la dispense de la garde. Deux formes
sont en usage : `` `repos/canterel/SPEC_V1.md` §12.4 `` pour un renvoi inter-dépôts, et `ADR 0017 §5.1`
pour un ADR qui cite ses propres sections. Le nom doit rester **sur la même ligne** que le `§` — la
garde lit ligne à ligne, et elle a attrapé cette page-ci quand le formatage a séparé les deux.

Ce que la garde ne sait pas voir : un numéro qui existe ici et qui visait ailleurs. Elle n'aurait pas
attrapé le défaut qui l'a motivée ; celui-là s'est trouvé en lisant, et rien ne remplace ça.

## Frontières vérifiées par la CI

1. `packages/domain` n'importe aucun package d'infrastructure.
2. Le SDK Temporal, quel que soit son écosystème, seulement sous `packages/workflow-backends`.
3. Aucun client PostgreSQL hors `packages/event-store` et projections.
4. `apps/locusd` n'importe aucun SDK de runtime de containers.
5. `apps/emacs` démarre en `emacs -Q` avec sa seule `load-path`.
6. `packages/graph` et le domaine des objets de coordination ne s'importent pas l'un l'autre.
7. Aucun fichier ne voit les deux familles d'objection à la fois.
8. Aucun crate n'acquiert de dépendance vers un moteur de service d'inférence.

---

## Identité

- Locus Solus = laboratoire/control plane.
- Canterel = runtime scientifique agentique.
- LEP = protocole générique d’exécution.
- `locusd` = daemon Locus Solus.
- `locus-execd` = broker d’exécution privilégié lorsque nécessaire.
- `locus` = CLI.
- `locusolus/apps/emacs` = client Emacs produit, dans le monorepo (ADR 0009).
- xiiif = viewer IIIF humain.

## Invariants non négociables

1. Le domaine ne dépend pas du backend de déploiement.
2. PostgreSQL/event store et graphe Locus sont la vérité institutionnelle, pas les transcripts.
3. Un worker ne modifie jamais directement la base canonique.
4. Tout résultat scientifique majeur est artifact-first et provenance-first.
5. L’exécution non fiable se fait dans une sandbox réelle avec limites et attestation.
6. Les ressources sont réservées avant exécution ; elles ne sont pas supposées illimitées.
7. Temporal est un backend, pas une abstraction métier.
8. Le GPU est une capability, pas une dépendance globale.
9. Emacs commande et inspecte ; le web rend les visualisations riches.
10. xiiif n’est pas requis par les agents.
11. Les reviewers indépendants ne reçoivent pas le raisonnement privé ou le contexte non autorisé du générateur.
12. Les résultats négatifs et conflits ne sont jamais supprimés pour rendre le graphe “propre”.

## Qualité du code

- simplicité avant abstraction spéculative ;
- types stricts ;
- schémas versionnés ;
- pas de fonctions géantes ;
- pas de duplication cross-repo des contrats ;
- erreurs structurées ;
- timeouts et cancellation ;
- logs corrélés sans secrets ;
- tests unitaires + contract + integration selon couche ;
- aucune dépendance implicite à une machine de développeur.

## Rythme de session

Quand une session travaille la roadmap en boucle, elle ne s'arrête **ni sur une CI verte, ni après
un bilan**. Merger puis reprendre l'item suivant ; le bilan s'écrit en passant. Les trois seuls
arrêts — arbitrage hors cadre, CI rouge non réparée en une tentative, demande explicite — sont
énumérés dans « Règle de session » de `docs/10_V1_ROADMAP.md`.

**Cette consigne, énoncée seule, n'a pas tenu.** Elle dit de ne pas s'arrêter sans donner de quoi
continuer : pendant qu'une CI tourne, aucun appel n'avance, il ne reste que le sondage, et un tour
qui n'a plus d'appel à faire produit du texte — ce qui **est** l'arrêt. Trois règles mécaniques la
remplacent, parce qu'elles se vérifient et qu'une exhortation ne se vérifie pas :

1. **Une PR ouverte sans réveil armé est un arrêt.** Avant de finir un tour alors qu'une PR attend
   sa CI, une commande d'attente doit tourner en arrière-plan : elle réveille la session quand elle
   se termine. Le tour a le droit de finir ; la boucle n'a pas le droit de dépendre de mon envie de
   resonder. `tools/attendre-ci.sh` le fait là où l'API GitHub **répond** — un jeton lisible ne
   suffit pas, et la session qui a écrit cette règle en a fait l'expérience au tour suivant : le
   jeton était présent et l'API a rendu `403`, l'App GitHub n'étant pas connectée pour cette
   organisation en HTTP direct alors que le serveur MCP, lui, répondait. Le script a refusé
   bruyamment, comme prévu ; le réveil, lui, n'existait plus. Là où l'API ne répond pas, une simple
   temporisation de fond suffit : ce qui compte est qu'**un réveil existe**, pas lequel. Un réveil
   grossier qui rend la main vaut mieux qu'un réveil fin qui n'est pas armé. Corollaire : ne jamais
   passer une commande d'attente dans un tube — le code de sortie devient celui de `tail`, et un
   refus bruyant se lit « exit 0 ».
2. **Un bilan ne finit pas un tour.** Il va dans le corps de la PR et dans le ledger, écrits en
   passant. Un bilan en dernière position est le signe qu'il n'y avait plus de réveil.
3. **Un compteur qui n'a rien lu ne vaut pas zéro.** Le premier réveil de cette session lisait
   `check_runs` dans une réponse `401` : la clé manquait, la somme valait 0, et « personne ne
   tourne » a été lu comme « tout est fini ». Toute attente bâtie sur une requête distingue **« la
   réponse est zéro »** de **« il n'y a pas eu de réponse »**, et échoue bruyamment sur la seconde.
   C'est la règle du dépôt — pas vérifié n'est jamais réussi — appliquée à l'outillage de session.

## Git

Un commit = objectif cohérent et testable. Ne mélange pas rename massif, refactor, nouvelle fonctionnalité et bugfix sans nécessité. Les migrations importantes ont un ADR et un plan de rollback.

## Sécurité

Ne monte jamais le home utilisateur, le socket Docker/Podman ou un répertoire de secrets dans une sandbox par défaut. Ne logge ni OAuth token, API key, cookie ni contenu classifié. Réseau deny-by-default pour code non fiable.

---

## Note d'origine du handoff

Lire la spec et les documents maîtres. Construire d’abord domain/protocol/event-store avec ports purs. Ne brancher Temporal/containers/cloud qu’après les interfaces et contract tests. `locusd` ne doit pas contrôler directement un socket root. Toute mutation passe par command handlers transactionnels.
