# Vision, naming et invariants

## Vision

Locus Solus est un **research operating system** : il transforme une question de recherche en portefeuille durable de branches, équipes, hypothèses, expériences, reviews, artefacts et décisions, tout en conservant explicitement l’incertitude, les contradictions et la provenance.

Le système n’essaie pas de créer “un super-agent”. Il crée une institution logicielle capable de faire travailler plusieurs agents et outils, d’allouer des ressources, de séparer les contextes, de contester les résultats et de reprendre après interruption.

## Noms finaux

- `Locus Solus` : système global et dépôt central.
- `Canterel` : agent/runtime scientifique.
- `LEP` : Locus Execution Protocol.
- `locusd`, `locus-execd`, `locus`.
- `locusolus/apps/emacs`, paquet `locusolus.el`, commandes préfixées `locus-`.
- `xiiif` reste inchangé.

## Invariants produit

- local-first mais deployment-agnostic ;
- durable ;
- versionné ;
- evidence/artifact-first ;
- multi-agent mais humain gouvernable ;
- adversarial by design ;
- reproductible ;
- sandboxed ;
- model/provider agnostic ;
- visualisation découplée du stockage ;
- interopérable ;
- aucune vérité cachée uniquement dans un prompt ou un transcript.

## Non-objectifs

Locus Solus n’est ni un IDE généraliste, ni une plateforme de chat, ni un moteur de notebook, ni un viewer IIIF, ni un cloud GPU, ni un gestionnaire de packages universel. Il orchestre ces capacités via contrats.
