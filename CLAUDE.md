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
- Les objets d'organisation, de coordination et de gouvernance sont ceux de `SPEC_V1.md` §7.1, §13,
  §16, §20 et §22, sous leur nom. Aucun vocabulaire parallèle — pas de `MutationPolicy`, pas de
  `MutationGrant`, pas de `TopologyNode`, pas d'échelle d'autorité à cinq barreaux (ADR 0016). Une
  sorte de relation de coordination n'entre dans son énumération que lorsqu'un consommateur
  exécutable et testé existe.

## Frontières vérifiées par la CI

1. `packages/domain` n'importe aucun package d'infrastructure.
2. Le SDK Temporal, quel que soit son écosystème, seulement sous `packages/workflow-backends`.
3. Aucun client PostgreSQL hors `packages/event-store` et projections.
4. `apps/locusd` n'importe aucun SDK de runtime de containers.
5. `apps/emacs` démarre en `emacs -Q` avec sa seule `load-path`.
6. `packages/graph` et le domaine des objets de coordination ne s'importent pas l'un l'autre.

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

## Git

Un commit = objectif cohérent et testable. Ne mélange pas rename massif, refactor, nouvelle fonctionnalité et bugfix sans nécessité. Les migrations importantes ont un ADR et un plan de rollback.

## Sécurité

Ne monte jamais le home utilisateur, le socket Docker/Podman ou un répertoire de secrets dans une sandbox par défaut. Ne logge ni OAuth token, API key, cookie ni contenu classifié. Réseau deny-by-default pour code non fiable.

---

## Note d'origine du handoff

Lire la spec et les documents maîtres. Construire d’abord domain/protocol/event-store avec ports purs. Ne brancher Temporal/containers/cloud qu’après les interfaces et contract tests. `locusd` ne doit pas contrôler directement un socket root. Toute mutation passe par command handlers transactionnels.
