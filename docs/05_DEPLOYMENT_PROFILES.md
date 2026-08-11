# Profils de déploiement

## `personal-local`

Tout sur le MacBook. PostgreSQL + Temporal/service équivalent + object storage local ; sandboxes dans VM Linux légère ; Canterel natif ou sandboxé. Limiter concurrence selon RAM disponible.

## `personal-node`

MacBook = cockpit ; Mac mini/nœud dédié = Locus + Canterel/workers. Connexion via réseau privé/Zero Trust. Aucun changement de modèle de domaine.

## `single-node-vm`

VM Linux : Locus, PostgreSQL, Temporal, object storage et workers CPU. Bon profil serveur permanent.

## `cloud-platform`

Adapters : durable workflows, object store, PostgreSQL, container/sandbox runtime et identity du cloud. Doit passer la suite de conformance Locus. Les limites du fournisseur sont déclarées au scheduler.

## `distributed-hybrid`

Exemple : control plane cloud, Canterel OAuth sur MacBook, worker Lean CPU sur VM, GPU RunPod/Modal/Kubernetes, données sensibles sur worker on-prem.

## Règle

Les clients se connectent à une URL Locus. Ils ne connaissent pas la topologie interne.

## Configuration

`deployment.yaml` sélectionne adapters et defaults. Secrets sont externes. `locus doctor` vérifie que le profil est réellement exécutable avant d’accepter des campagnes.
