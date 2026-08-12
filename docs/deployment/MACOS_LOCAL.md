# Profil macOS local

Topologie de référence pour un MacBook Apple Silicon :

```text
macOS host
├── Emacs / locusolus apps/emacs
├── locusd (natif ou service local)
├── PostgreSQL / workflow backend / artifact store
├── Canterel de confiance (OAuth/MPS possible)
└── locus-execd
    └── VM Linux légère
        └── containers rootless par mission
```

## Pourquoi une VM Linux

Elle fournit une boundary plus nette que de simples permissions shell et rapproche les environnements de ceux utilisés sur VM/cloud. Le home macOS n’est pas monté. Seul un workspace de mission et un canal d’artefacts sont exposés.

## Ressources

Locus réserve une marge hôte avant de proposer une mission. Sur une machine 16 Go, les valeurs par défaut doivent être prudentes et la concurrence adaptative. MPS est exposé par un worker natif explicitement trusted, pas par une fausse promesse de GPU passthrough dans la VM.
