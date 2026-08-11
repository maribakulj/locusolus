# ADR 0007 — Périmètre xiiif

**Statut :** accepté.

**Contexte :** xiiif est un outil Emacs mûr (v0.4.0). La pression à en faire un serveur MCP ou un worker agentique existe et le dénaturerait.

**Décision :** xiiif reste viewer/éditeur IIIF humain. Les agents utilisent des outils IIIF headless. Locus Solus ne dépend jamais de xiiif.

**Conséquences :** l'intégration Locus est facultative et passe par le client public `locusolus/apps/emacs`, jamais par un accès direct à la base. xiiif lit `ArtifactManifest` comme une donnée ; il n'importe aucun code Locus.

**Rollback :** l'intégration est retirable sans toucher au cœur de xiiif — c'est le test de la décision.
