# ADR 0009 — Le client Emacs vit dans le monorepo

**Statut :** accepté. **Révise D003** (« cinq repos ») de `docs/DECISIONS.md`.

**Contexte :** le découpage initial séparait `locus-solus-emacs` (client produit, publiable) de `emacs-config` (configuration personnelle). La distinction est juste ; elle ne justifie pas pour autant un dépôt distinct.

**Décision :** le client Emacs générique vit dans `locusolus/apps/emacs/`. Le chantier compte **quatre** dépôts : `locusolus`, `canterel`, `xiiif`, `emacs-config`.

**Motif :** le client consomme le protocole, les types du graphe, le flux d'événements, la surface de commandes et le registre de viewers d'artefacts. Ces cinq contrats sont instables jusqu'à la V1. Deux dépôts imposent à chaque évolution du protocole une séquence de six étapes — modifier, versionner, publier, modifier le client, synchroniser les CI, gérer la compatibilité croisée — pour un projet développé comme un tout. Dans le monorepo, un changement de `GraphNode` touche backend, web, Emacs et tests dans le même commit.

**Indépendance logique ≠ dépôt séparé.** `apps/emacs/` reste un package Emacs propre : `locusolus-pkg.el`, `README.md`, `CHANGELOG.md`, `tests/`, aucun chemin ni secret personnel, aucune dépendance à `emacs-config`. Installable par `:load-path` seul.

**Condition de réexamen :** cycle de release propre, plusieurs contributeurs, usage indépendant de Locus Solus, publication MELPA, ou support simultané de plusieurs versions de Locus Solus. La première atteinte rouvre la question, et l'extraction se fera sans changer l'architecture du package.

**Test de non-régression de la décision :** `emacs -Q` avec pour seule `load-path` celle de `apps/emacs/` doit démarrer. Il garantit que l'intégration au monorepo n'a pas introduit de dépendance implicite au reste de l'arbre — le vrai risque de ce choix. À écrire **avant** la première ligne de `apps/emacs/`.

**Rollback :** `git subtree split` sur `apps/emacs/` produit un dépôt autonome avec son historique. Coût réel : mettre à jour le `:load-path` en `:vc` dans `emacs-config`.
