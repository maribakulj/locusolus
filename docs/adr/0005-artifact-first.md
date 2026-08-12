# ADR 0005 — Artifact-first

**Statut :** accepté.

**Contexte :** un résultat qui n'existe que dans un transcript n'est ni vérifiable, ni reproductible, ni citable.

**Décision :** les résultats durables sont des artefacts et des manifests, jamais seulement des messages. Les viewers ne sont jamais sources de vérité.

**Conséquences :** hash déclaré avant upload, quarantaine puis promotion, `RunManifest` et provenance par artefact. Un viewer est remplaçable sans perte. xiiif doit pouvoir ouvrir un bundle produit par un agent qui n'a jamais utilisé xiiif.

**Rollback :** aucun.
