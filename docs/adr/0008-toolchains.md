# ADR 0008 — Toolchain Registry

**Statut :** accepté.

**Contexte :** un runtime qui devine si Lean, PyTorch ou SageMath existent sur l'hôte produit des résultats non reproductibles et des échecs opaques.

**Décision :** Lean, PyTorch, browser, DH et les autres outils sont fournis par des environnements versionnés, lockés, scannés et testés. Locus résout un `EnvironmentBlueprint` vers une image attestée.

**Conséquences :** pas d'image universelle géante, pas de `sudo` dans une mission, pas de package flottant sans version dans un environnement promu. Une dépendance absente déclenche `environment.extension.requested` et un build séparé, puis la mission redémarre sur l'environnement immuable.

**Rollback :** un environnement se révoque par digest ; les runs passés conservent le leur.
