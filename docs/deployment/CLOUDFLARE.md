# Profil Cloudflare / plateforme serverless-container

Ce profil définit une **classe d’adapter**, pas une dépendance du domaine. Pour Cloudflare, l’implémentation peut mapper les ports Locus vers les services actuellement disponibles tels que Workers, durable Workflows, R2, Hyperdrive/PostgreSQL, Containers/Sandbox et Access lorsque leurs garanties correspondent aux interfaces.

## Règles

- vérifier les limites et fonctionnalités officielles au moment de l’implémentation ;
- ne jamais coder un quota fournisseur dans le domaine ;
- l’état canonique reste PostgreSQL/event store ;
- le filesystem d’une sandbox cloud est temporaire ; checkpoints/artefacts vont dans l’object store ;
- absence de GPU = capability absente, tâche reroutée ;
- toute sandbox cloud doit fournir attestation des limites réellement appliquées ;
- export/restore doit permettre de quitter ce backend.

## Topologie conceptuelle

```text
Access/HTTP
  → locus API adapter
  → workflow adapter
  → PostgreSQL
  → object store
  → cloud sandbox/container workers
  → LEP external workers (local/GPU) si nécessaire
```

Claude doit consulter la documentation Cloudflare actuelle avant de figer noms d’API, tailles d’instances, limites ou tarification.
