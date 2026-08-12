# ADR 0004 — Execution Fabric

**Statut :** accepté.

**Contexte :** un runtime agentique qui choisit lui-même son isolation et ses ressources ne peut pas offrir de garantie vérifiable.

**Décision :** sandbox, ressources et résolution d'environnement appartiennent à Locus Solus ; Canterel exécute dans ce cadre et ne peut pas augmenter unilatéralement ses privilèges.

**Décision liée :** `locus-execd` est un service séparé. `locusd` ne détient jamais de socket Docker/Podman.

**Conséquences :** le worker annonce son niveau réel dans le `CapabilityManifest` et refuse proprement une mission dont il ne peut pas honorer le `SandboxSpec`. Une fixture de refus d'admission fait partie du corpus de conformance.

**Rollback :** aucun chemin de repli acceptable — un raccourci ici est exactement le « sandbox factice » que le handoff interdit.
