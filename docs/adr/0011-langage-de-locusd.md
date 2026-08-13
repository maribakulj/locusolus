# ADR 0011 — Langage d'implémentation du control plane

**Statut :** accepté. **Amende** `docs/SPEC_V1.md` §4.5 et la règle 2 de `CLAUDE.md`. Clôt la décision laissée ouverte par `docs/10_V1_ROADMAP.md`, « État de départ ».

**Contexte :** `SPEC_V1.md` §4.5 donnait TypeScript et Node.js LTS comme technologies de référence, en précisant que ces choix sont remplaçables sans changer les invariants métier. La roadmap notait que le choix l'est moins qu'il n'y paraît, puisque le worker vit dans un fork TypeScript et qu'un SDK TS existera de toute façon. Aucun document du paquet de handoff ne nomme d'autre langage.

L'argument qui portait TypeScript était la simplicité : un seul langage côté serveur. **Il est faux.** `locus-execd` détient le socket runtime, pose les namespaces, les cgroups v2 et les filtres seccomp. Ce composant ne peut pas raisonnablement être écrit en TypeScript — il faudrait des bindings natifs ou des appels de binaires externes dans le binaire le plus sensible du système, celui dont ADR 0004 dit qu'aucun chemin de repli n'est acceptable. Le chantier est donc multi-langage quel que soit le choix retenu pour `locusd`, et « TypeScript partout » n'existe pas comme option.

| | `locusd` | `locus-execd` | `locus` | worker | web | Emacs |
|---|---|---|---|---|---|---|
| TypeScript côté control plane | TS | Rust | TS | TS | TS | elisp |
| Rust côté control plane | Rust | Rust | Rust | TS | TS | elisp |

La seconde ligne comporte **moins** de langages, pas davantage.

**Décision : Rust pour `apps/locusd`, `apps/locus-execd` et `apps/cli`.** TypeScript reste le langage de `apps/web`, du SDK client généré et du worker Canterel, qui est un fork amont non divergé (ADR 0010). `apps/emacs` reste en Emacs Lisp.

**Motifs.**

Le domaine est une machine à états — niveaux de validation (§8.1), états de lease (§12.3), statuts de commit épistémique (§2.3), issues d'admission (§10.2). Les types somme de Rust avec exhaustivité vérifiée transforment « un état non traité » en erreur de compilation. Pour un système dont la thèse est l'intégrité épistémique, la propriété est structurante, pas cosmétique.

La surface de dépendances d'un control plane dont la raison d'être est la provenance et l'attestation gagne à rester auditable par un humain. Cargo n'est pas immunisé contre les compromissions de chaîne d'approvisionnement ; l'ordre de grandeur du graphe de dépendances n'est simplement pas le même.

Le binaire statique unique rend le profil de déploiement `personal-local` (`docs/05`) trivial à distribuer.

**Conditions, sans lesquelles la décision est mauvaise.**

1. **Les JSON Schemas de W0.5 et W0.6 sont écrits en Draft 7.** `typify`, la voie de référence pour JSON Schema → Rust, supporte réellement Draft 7 ; sur 2020-12 il fonctionne parfois et casse souvent, et sa refonte est en cours. `schemas/` est vide aujourd'hui : le dialecte ne coûte rien maintenant et se migre mal plus tard. Un prototype `typify` sur 2020-12 qui passerait lève la condition.
2. **L'ordre d'ADR 0003 est respecté à la lettre.** Backend déterministe de test d'abord (W3.b), Temporal ensuite (W3.d). Le SDK Rust de Temporal est en *Public Preview* depuis mai 2026 et annonce que son API continuera d'évoluer ; W3.d est à plusieurs mois, et la décision de liaison se prend à ce moment-là, pas maintenant.
3. **Repli Temporal documenté d'avance :** si le SDK Rust n'est pas stabilisé à W3.d, le backend Temporal tourne dans un processus worker TypeScript séparé, derrière le port `WorkflowBackend`. Coût : un processus et une frontière de sérialisation. Pas une réécriture — c'est précisément ce que l'abstraction d'ADR 0003 achète.
4. **W0.8 génère deux SDK** depuis les mêmes schémas, TypeScript pour le worker et Rust pour le serveur, et le corpus de fixtures de W0.7 devient la suite de conformance inter-langages annoncée par `docs/06`. C'est un coût à budgéter, pas à découvrir : il vaut **+1 SDK**, pas le doublement de toute la suite de tests.

**Conséquences.**

La règle 2 de `CLAUDE.md` nommait `@temporalio/*`, un package npm ; elle est reformulée pour désigner le SDK Temporal indépendamment de l'écosystème. `boundaries.json` porte déjà les motifs Go et Rust ; il reçoit un extracteur d'imports Rust et la lecture des dépendances déclarées dans `Cargo.toml`.

Jusqu'à ce que cet extracteur existe, tout fichier `.rs` fait échouer la CI avec le finding `boundary-blind-spot`. C'est le comportement voulu et non un défaut : une frontière qu'on ne sait pas vérifier n'est pas une frontière.

Le coût diffus assumé : refactorer un modèle de domaine encore en cours de découverte est plus lent en Rust qu'en TypeScript. W1 est la phase où ce coût se paiera.

**Alternative écartée : Go.** Meilleur placé sur deux points — le SDK Temporal Go est l'implémentation de référence, et l'écosystème OCI/containers est natif Go. Écarté parce qu'il n'a pas de types somme, donc pas la garantie d'exhaustivité qui est le premier motif de cette décision.

**Condition de réexamen :** si l'itération sur W1 se révèle assez coûteuse pour menacer la livraison de la V1, ou si `typify` et le SDK Rust de Temporal restent tous deux hors d'usage à l'entrée de W3.

**Rollback :** W0.1 à W0.3 sont agnostiques par construction et ne sont pas concernés. Avant W1, revenir sur cette décision ne coûte que l'annulation de cet ADR. Après W1, le coût est celui de la réécriture du domaine — c'est la raison pour laquelle la décision est prise maintenant et pas plus tard.
