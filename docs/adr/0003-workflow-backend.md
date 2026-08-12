# ADR 0003 — Workflow backend abstrait

**Statut :** accepté.

**Contexte :** Temporal fournit la durabilité dont l'orchestration a besoin, mais son modèle de programmation contamine facilement le domaine.

**Décision :** Temporal n'apparaît pas dans le domaine ; il implémente `WorkflowBackend`.

**Conséquence de construction :** le **backend déterministe de test est écrit avant** le backend Temporal. Si Temporal vient en premier, le domaine s'y adapte silencieusement et l'ADR devient une intention.

**Vérification :** `@temporalio/*` n'apparaît que sous `packages/workflow-backends`. Test de frontières en CI.

**Rollback :** remplacer le backend, pas le domaine. C'est précisément ce que l'ADR achète.
