# ADR 0001 — Naming

**Statut :** accepté.

**Contexte :** trois choses distinctes portaient des noms confondus — l'institution logicielle, le runtime qui exécute les missions, et le contrat qui les relie.

**Décision :** Locus Solus nomme le laboratoire ; le dépôt est `locusolus`. Canterel nomme le runtime anciennement `openscienceDH`. LEP nomme le protocole.

**Motif :** séparer institution, chercheur et contrat générique.

**Conséquences :** aucun document utilisant `OpenScience Lab`, l'ancien sens de `Canterel` ou `CWP` n'est normatif. « Canterel » désigne le worker et son déploiement, pas le fork amont (ADR 0010).

**Rollback :** aucun coût technique ; renommage documentaire.
