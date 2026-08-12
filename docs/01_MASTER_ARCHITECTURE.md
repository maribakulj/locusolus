# Architecture maître

## Couches

1. **Control plane** : domaine, commands, workflows, scheduler, policies, portfolio, review.
2. **Execution plane** : Canterel et workers spécialisés dans environnements bornés.
3. **Evidence plane** : event store, graph, artifacts, manifests, memory.
4. **Presentation plane** : Emacs, web, viewers spécialisés.

## Données canoniques

PostgreSQL conserve événements et projections transactionnelles. L’event store est append-only logique avec optimistic concurrency. Les projections peuvent être reconstruites. Les embeddings, caches et index externes sont dérivés et supprimables.

## Graphe

Nœuds organisationnels, épistémiques, critiques et de production. Relations typées. Les inférences multi-prémisses sont des objets/hyperedges, pas de simples liens. Tout objet a identité stable, version, status, validation level, branch scope, provenance, timestamps et supersession.

## Workflows

Le domaine émet des intentions ; `WorkflowBackend` fournit durabilité. Les activités externes sont idempotentes. Une compensation technique n’efface jamais un fait scientifique déjà observé.

## Execution Fabric

Le scheduler choisit un worker selon capability + trust + data locality + resource fit + budget. L’execution broker crée l’environnement, applique les limites et produit une attestation. Canterel exécute ensuite la mission.

## Présentation

Les clients reçoivent queries/projections et soumettent commands. Aucun frontend n’écrit directement dans le graphe.
